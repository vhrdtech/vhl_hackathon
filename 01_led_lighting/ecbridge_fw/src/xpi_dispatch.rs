use core::cmp::max;
use rtt_target::{rprintln};
use vhl_cg::point::Point;
use vhl_stdlib::serdes::buf::{Buf, BufMut};
use crate::{log_error, log_info, log_trace, log_warn};
use vhl_stdlib::serdes::buf::Error as BufError;
use vhl_stdlib::serdes::{NibbleBuf, NibbleBufMut, SerDesSize, SerializeVlu4};
use vhl_stdlib::serdes::vlu4::{Vlu32, Vlu4VecIter};
use xpi::event_kind::{XpiEventDiscriminant, XpiGenericEventKind};
use xpi::xwfd::{NodeId, SerialUriIter, NodeSet, EventKind, EventBuilder, MultiUriFlatIter};
use xpi::error::XpiError;
use xpi::xwfd;
use vhl_stdlib::discrete::U4;

pub type DispatcherContext<'c> = crate::app::link_process::Context<'c>;

const T: u8 = 2;

// dispatcher still runs in the protocol task
// should be configurable by user what to do next with requests
// dispatcher should have access to all the resources to answer for ex. Read requests for props
// would be great to just put all the resources to rtic _resources_, so that different priority
// task can run without waiting
//
// Also need ability to send XpiReply(-s) back to the link from dispatcher
pub fn xpi_dispatch(ctx: &mut DispatcherContext, ev: &xwfd::Event) -> Result<(), XpiError> {
    rprintln!(=>T, "xpi_dispatch: {}", ev);

    let self_node_id = NodeId::new(1).unwrap();
    let eth_in_prod: &mut bbqueue::Producer<512> = ctx.local.eth_in_prod;

    // 1. scan over resources set
    // 2. decide which calls to batch into one reply based on maximum reply len and max len of each call result
    // 3. create XpiReplyBuilder and serialize resource subset into it
    // 4. advance builder to args_set state and dispatch every call
    // 5. finish serializing, submit reply
    // 6. repeat until all calls are processed or no more space for replies available

    const MAX_REPLY_BATCH_LEN: usize = 16; // TODO: move to config file
    const MAX_REPLY_BATCHES: usize = 8; // hard limit to not create an endless loop on erroneous requests
    const REPLY_MTU: usize = 64; // bytes

    let mut resource_set_lookahead_uri_iter = ev.resource_set.flat_iter().peekable();
    let mut resource_set_execute_uri_iter = ev.resource_set.flat_iter();
    let ev_kind = ev.kind.discriminant();
    for _ in 0..MAX_REPLY_BATCHES {
        let mut reply_lookahead: [Option<ReplySizeHint>; MAX_REPLY_BATCH_LEN] = [None; MAX_REPLY_BATCH_LEN];
        let mut run_out_of_requests = false;
        let mut batch_len = 0;
        let mut reply_nibbles_left = (REPLY_MTU - /* frame sync overhead */5) * 2 - /*header*/10 - /*tail*/2 - /*spare*/10;
        for idx in 0..MAX_REPLY_BATCH_LEN {
            match resource_set_lookahead_uri_iter.peek() {
                Some(uri) => {
                    let hint = reply_size_hint(uri.clone(), ev_kind);
                    match hint {
                        ReplySizeHint::Sync { max_size, .. } => {
                            let upper_bound = max_size.upper_bound(reply_nibbles_left);
                            if reply_nibbles_left >= upper_bound {
                                // reply will fit, take it
                                let _ = resource_set_lookahead_uri_iter.next();
                                reply_lookahead[idx] = Some(hint);
                                batch_len += 1;
                                reply_nibbles_left -= upper_bound;
                            } else {
                                // reply won't fit, stop and put it into next batch
                                break;
                            }
                        }
                        ReplySizeHint::Async => {
                            // async replies do not consume any space from this batch
                            let _ = resource_set_lookahead_uri_iter.next();
                            reply_lookahead[idx] = Some(hint);
                            batch_len += 1;
                        }
                    }

                }
                None => {
                    run_out_of_requests = true;
                    break;
                }
            }
        }
        if batch_len > 0 {
            let mut eth_wgr = eth_in_prod.grant_exact(REPLY_MTU).unwrap();
            let reply_builder = EventBuilder::new(
                NibbleBufMut::new_all(&mut eth_wgr),
                self_node_id,
                ev.request_id,
                ev.priority,
                U4::new(15).unwrap()
            ).unwrap();
            let reply_builder = reply_builder.build_node_set_with(|mut nwr| {
                let node_set = NodeSet::Unicast(ev.source);
                node_set.ser_vlu4(&mut nwr)?;
                Ok((node_set.ser_header(), nwr))
            }).unwrap();
            let reply_builder = reply_builder.build_resource_set_with(|mut nwr| {
                let resource_set = ev.resource_set.clone();
                resource_set.ser_vlu4(&mut nwr)?;
                Ok((resource_set.ser_header(), nwr))
            }).unwrap();
            // dispatch_batch(
            //     ctx,
            //     &mut resource_set_execute_uri_iter,
            //     &lookahead_reply_size[..batch_len],
            //     &ev.kind
            // );
            match &ev.kind {
                EventKind::Call { args_set } => {
                    let nwr = reply_builder.build_kind_with(|nwr| {
                        let mut vb = nwr.put_vec::<Result<NibbleBuf, XpiError>>();
                        let mut args_set_iter = args_set.iter();
                        for reply_size_hint in &reply_lookahead[..batch_len] {
                            let uri = resource_set_execute_uri_iter.next().expect("");
                            match reply_size_hint {
                                Some(ReplySizeHint::Sync { raw_size, preliminary_result, ..}) => {
                                    match preliminary_result {
                                        Ok(_) => {
                                            match args_set_iter.next() {
                                                Some(args_nrd) => {
                                                    vb.put_result_nib_slice_with(*raw_size, |result_nwr| {
                                                        dispatch_call(
                                                            uri.clone(),
                                                            args_nrd,
                                                            result_nwr,
                                                        ).map(|_| ()).map_err(|e| {
                                                            log_error!(=>T, "dispatch error: {:?}", e);
                                                            e
                                                        })
                                                    })?;
                                                }
                                                None => {
                                                    log_error!(=>T, "No args provided for {}", uri);
                                                    vb.put(&Err(XpiError::NoArgumentsProvided))?;
                                                }
                                            }
                                        }
                                        Err(e) => {
                                            vb.put(&Err(e.clone()))?;
                                            continue;
                                        }
                                    }
                                }
                                Some(ReplySizeHint::Async) => {
                                    match args_set_iter.next() {
                                        Some(args_nrd) => {
                                            match dispatch_call(uri.clone(), args_nrd, &mut NibbleBufMut::new_all(&mut [])) {
                                                Ok(_) => { log_trace!(=>T, "async call spawned"); }
                                                Err(e) => { log_error!(=>T, "dispatch error: {:?}", e); }
                                            }
                                        }
                                        None => {
                                            log_error!(=>T, "No args provided for {}", uri);
                                            vb.put(&Err(XpiError::NoArgumentsProvided))?;
                                        }
                                    }
                                }
                                None => {
                                    break; // shouldn't be reached, if batch_len is correct
                                }
                            }
                        }

                        let nwr = vb.finish()?;
                        Ok((XpiEventDiscriminant::CallResults, nwr))
                    })?;
                    log_trace!(=>T, "XpiReply {}, free space left={} expected:{}", nwr, nwr.nibbles_left(), reply_nibbles_left);
                    let (_, len, _) = nwr.finish();
                    log_trace!(=>T, "commit {}", len);
                    eth_wgr.commit(len);
                    rtic::pend(stm32h7xx_hal::pac::Interrupt::ETH);
                }
                u => {
                    rprintln!(=>T, "Unsupported: {}", u);
                }
            }

        }
        if run_out_of_requests || batch_len == 0 {
            break;
        }
    }
    if resource_set_lookahead_uri_iter.next().is_some() {
        log_error!(=>T, "Maximum request count({}) is reached, some requests are skipped", MAX_REPLY_BATCH_LEN * MAX_REPLY_BATCHES);
    }

    Ok(())
}

// fn dispatch_batch(
//     ctx: &mut DispatcherContext,
//     uri_iter: &mut MultiUriFlatIter,
//     reply_lookahead: &[Option<ReplySizeHint>],
//     request: &EventKind
// ) {
//
// }

/// Perform one method call on a resource.
///
/// if call_type == DryRun => return maximum length of the reply or an error if it is obvious right
/// away that call would fails. In RealRun return the amount of bytes written into result slice.
///
/// Dispatcher will decide how many replies to batch together based on that information.
/// It is ok to return less data than originally estimated.
/// Returning an error in dry run allows to batch more replies, as some of them might be invalid,
/// thus requiring space only for an error code.
fn dispatch_call(
    mut uri: SerialUriIter<Vlu4VecIter<Vlu32>>,
    mut args_nrd: NibbleBuf,
    result_nwr: &mut NibbleBufMut
) -> Result<(), XpiError>
{
    log_info!(=>T, "dispatch_call({})", uri);
    match uri.next() {
        None => {
            log_error!(=>T, "Expected root level");
            return Err(XpiError::BadUri);
        },
        id @ Some(0 | 1) => {
            log_error!(=>T, "Resource /{:?} is not a method", id);
            Err(XpiError::NotAMethod)
        }
        Some(2) => {
            let a = args_nrd.des_vlu4()?;
            let spawn_r = crate::app::set_digit::spawn(a);
            log_trace!(=>T, "Spawning /set_digit({}) {:?}", a, spawn_r);

            Ok(())
        }
        Some(5) => {
            let p1: Point = args_nrd.des_vlu4()?;
            let p2: Point = args_nrd.des_vlu4()?;
            if !args_nrd.is_at_end() { // TODO: remove this as semver compatible newer versions can contain more data
                log_warn!(=>T, "Unused {} nib left after deserializing arguments", args_nrd.nibbles_left());
            }
            let r = crate::sync(p1, p2);
            log_trace!(=>T, "Called /sync3({:?}, {:?}) = {:?}", p1, p2, r);

            result_nwr.put(&r)?;
            Ok(())
        }
        Some(6) => {
            let p1: Point = args_nrd.des_vlu4()?;
            let p2: Point = args_nrd.des_vlu4()?;
            let spawn_r = crate::app::async_task::spawn(p1, p2);
            log_trace!(=>T, "Spawning /async: {:?}", spawn_r);
            Ok(())
        }
        not_defined => {
            log_error!(=>T, "Resource /{:?} doesn't exist", not_defined);
            Err(XpiError::BadUri)
        }
    }
    // unreachable!()
}

#[derive(Copy, Clone)]
enum ReplySizeHint {
    /// Reply can be sent right away, if it is obvious that a resource doesn't support particular
    /// operation, error is sent right away without further calls to dispatchers. Otherwise
    /// appropriate dispatcher is called, but it can also return an error, which can be bigger in
    /// size than actual Ok return type, so max(ok_return_ty, XpiError::max_size()) is used.
    Sync {
        /// max( Ok(&[nib; raw_size]).len_nibbles(), Err(XpiError::<max_code>).len_nibbles() )
        max_size: SerDesSize,
        /// used to create result_nwr of correct size
        raw_size: SerDesSize,
        preliminary_result: Result<(), XpiError>,
    },
    /// Request is processed asynchronously, cannot reply right away. Appropriate task is spawned and
    /// is responsible to submit request
    Async
}

impl ReplySizeHint {
    pub fn sync(max: SerDesSize, raw: SerDesSize, preliminary: Result<(), XpiError>) -> Self {
        ReplySizeHint::Sync {
            max_size: max,
            raw_size: raw,
            preliminary_result: preliminary,
        }
    }
}

///
/// TODO: use proper max() or calculate in advance during code gen
fn reply_size_hint(mut uri: SerialUriIter<Vlu4VecIter<Vlu32>>, event_kind: XpiEventDiscriminant) -> ReplySizeHint {
    log_trace!(=>T, "reply_size_hint({})", uri);
    use XpiEventDiscriminant::*;
    let not_supported = Err(XpiError::OperationNotSupported);
    let not_supported = ReplySizeHint::sync(not_supported.len_nibbles(), SerDesSize::Sized(0), not_supported);
    let bad_uri = Err(XpiError::BadUri);
    let bad_uri = ReplySizeHint::sync(bad_uri.len_nibbles(), SerDesSize::Sized(0), bad_uri);
    match uri.next() {
        // /main
        None => {
            match event_kind {
                _ => not_supported,
            }
        },
        Some(5) => {
            match uri.next() {
                // dispatch /main/sync
                None => {
                    match event_kind {
                        Call => ReplySizeHint::sync(SerDesSize::Sized(8 + 3), SerDesSize::Sized(8), Ok(())),
                        _ => not_supported
                    }
                }
                // /main/sync : has no child resources
                Some(_) => bad_uri,
            }
        },
        Some(6) => {
            match uri.next() {
                // dispatch /main/async
                None => {
                    match event_kind {
                        Call => ReplySizeHint::Async,
                        _ => not_supported
                    }
                }
                // /main/async : has no child resources
                Some(_) => bad_uri,
            }
        }
        // /main : all defined resources are handled
        Some(_) => bad_uri,
    }
}