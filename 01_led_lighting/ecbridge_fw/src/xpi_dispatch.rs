use rtt_target::{rprintln};
use vhl_cg::point::Point;
use vhl_stdlib::serdes::buf::{Buf, BufMut};
use crate::{log_error, log_info, log_trace, log_warn};
use vhl_stdlib::serdes::buf::Error as BufError;
use vhl_stdlib::serdes::{NibbleBuf, NibbleBufMut, SerDesSize, SerializeVlu4};
use vhl_stdlib::serdes::vlu4::{Vlu32, Vlu4VecIter};
use xpi::event_kind::{XpiEventDiscriminant, XpiGenericEventKind};
use xpi::xwfd::{NodeId, SerialUriIter, NodeSet, EventKind, EventBuilder};
use xpi::error::XpiError;
use xpi::xwfd;
use vhl_stdlib::discrete::U4;

pub type DispatcherContext<'c> = crate::app::link_process::Context<'c>;

const T: u8 = 2;

// #[derive(Debug)]
// pub enum Error {
//     BufError(BufError),
//     XpiXpiError(XpiError),
// }
//
// impl From<BufError> for Error {
//     fn from(e: BufError) -> Self {
//         Error::BufError(e)
//     }
// }

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

    // 1. check if req.destination applies to this node

    // match req.resource_set {
    //     XpiResourceSet::Uri(uri) => {
    //         dispatch_one(uri.iter(), req);
    //     }
    //     XpiResourceSet::MultiUri(multi_uri) => {
    //         for (uri, mask) in multi_uri.iter() {
    //             for m in mask {
    //                 let target = uri.iter().chain(core::iter::once(m));
    //                 dispatch_one(target, req);
    //             }
    //         }
    //     }
    // }

    match &ev.kind {
        EventKind::Call { args_set } => {
            // 1. scan over resources set
            // 2. decide which calls to batch into one reply based on maximum reply len and max len of each call result
            // 2a. batch only consecutive calls into one reply to make things simpler
            // 3. create XpiReplyBuilder and serialize resource subset into it
            // 4. advance builder to args_set state and dispatch every call
            // 5. finish serializing, submit reply
            // 6. repeat until all calls are processed or no more space for replies available
            // let mut resource_set_lookahead = req.resource_set.flat_iter();
            // let mut resource_set_process = req.resource_set.flat_iter();
            // let mut batch_amount = 0;
            const REPLY_MTU: usize = 64;

            let mut eth_wgr = eth_in_prod.grant_exact(REPLY_MTU).unwrap();
            let reply_builder = EventBuilder::new(
                NibbleBufMut::new_all(&mut eth_wgr),
                self_node_id,
                // NodeSet::Unicast(ev.source),
                // ev.resource_set.clone(),
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
            let nwr = reply_builder.build_kind_with(|nwr| {
                let mut vb = nwr.put_vec::<Result<NibbleBuf, XpiError>>();
                let mut args_set_iter = args_set.iter();
                for uri in ev.resource_set.flat_iter() {
                    match args_set_iter.next() {
                        Some(args) => {
                            // Speculatively assume error code = 0 (1 nibble, success) and try to dispatch
                            // a call. If it actually succeeds => great, otherwise step back and write an
                            // error code instead (which can take more than 1 nibble). There are no penalties
                            // in either case, no excessive copies or anything.
                            // need to know result_len already from DryRun -- won't work with variable len return types(:
                            // speculatively write len(rest of the buffer), then dispatch and re-write the size, possible sending some 0s
                            // or copy will be required to properly shift the data (from the separate buffer or the same)
                            let size_hint = match dispatch_call(uri.clone(), DispatchCallType::DryRun) {
                                Ok(size_hint) => size_hint,
                                Err(e) => {
                                    vb.put(&Err(e))?;
                                    continue;
                                }
                            };


                            // vb.put_result_with_slice_from(result_len, |result| {
                            //     dispatch_call(
                            //         uri.clone(),
                            //         DispatchCallType::RealRun { args, result }
                            //     ).map(|_| ()).map_err(|e| {
                            //         log_error!(=>T, "dispatch error: {:?}", e);
                            //         e
                            //     })
                            // }).unwrap();
                            vb.put_result_nib_slice_with(size_hint, |result_nwr| {
                                dispatch_call(
                                    uri.clone(),
                                    DispatchCallType::RealRun { args_nrd: args, result_nwr, }
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
                let nwr = vb.finish()?;
                Ok((XpiEventDiscriminant::CallResults, nwr))
            })?;

            log_trace!(=>T, "XpiReply {}", nwr);
            let (_, len, _) = nwr.finish();
            log_trace!(=>T, "commit {}", len);
            eth_wgr.commit(len);
            rtic::pend(stm32h7xx_hal::pac::Interrupt::ETH);


            // let mut args_set_iter = args_set.iter();
            // for uri in req.resource_set.flat_iter() {
            //     match args_set_iter.next() {
            //         Some(args) => {
            //             let r = dispatch_call(uri, DispatchCallType::RealRun { args, result: &mut[] } );
            //             if r.is_err() {
            //                 log_error!(=>T, "dispatch error: {:?}", r);
            //             }
            //         }
            //         None => {
            //             log_error!(=>T, "No args provided for {}", uri);
            //         }
            //     }
            // }
        }
        u => {
            rprintln!(=>T, "Unsupported: {}", u);
        }
    }

    Ok(())
}

enum DispatchCallType<'i, 'a, 'b> {
    DryRun,
    RealRun {
        args_nrd: NibbleBuf<'i>,
        result_nwr: &'a mut NibbleBufMut<'b>,
    }
}

/// Perform one method call on a resource.
///
/// if call_type == DryRun => return maximum length of the reply or an error if it is obvious right
/// away that call would fails. In RealRun return the amount of bytes written into result slice.
///
/// Dispatcher will decide how many replies to batch together based on that information.
/// It is ok to return less data than originally estimated.
/// Returning an error in dry run allows to batch more replies, as some of them might be invalid,
/// thus requiring space only for an error code.
fn dispatch_call(mut uri: SerialUriIter<Vlu4VecIter<Vlu32>>, call_type: DispatchCallType) -> Result<SerDesSize, XpiError>
{
    use DispatchCallType::*;

    log_info!(=>T, "dispatch_call({})", uri);
    let at_root_level = match uri.next() {
        Some(p) => p,
        None => {
            log_error!(=>T, "Expected root level");
            return Err(XpiError::BadUri);
        }
    };
    match at_root_level {
        0 | 1 => {
            log_error!(=>T, "Resource /{} is not a method", at_root_level);
            Err(XpiError::NotAMethod)
        }
        2 => {
            match call_type {
                DryRun => Ok(SerDesSize::Sized(0)),
                RealRun { args_nrd: mut args, .. } => {
                    let a = args.des_vlu4()?;
                    let spawn_r = crate::app::set_digit::spawn(a);
                    log_trace!(=>T, "Spawning /set_digit({}) {:?}", a, spawn_r);

                    Ok(SerDesSize::Sized(0))
                }
            }
        }
        5 => {
            match call_type {
                DryRun => Ok(SerDesSize::Sized(8)),
                RealRun { mut args_nrd, result_nwr } => {
                    let p1: Point = args_nrd.des_vlu4()?;
                    let p2: Point = args_nrd.des_vlu4()?;
                    if !args_nrd.is_at_end() { // TODO: remove this as semver compatible newer versions can contain more data
                        log_warn!(=>T, "Unused {} nib left after deserializing arguments", args_nrd.nibbles_left());
                    }
                    let r = crate::sync(p1, p2);
                    log_trace!(=>T, "Called /sync3({:?}, {:?}) = {:?}", p1, p2, r);

                    // size of return type is known in advance = 4
                    result_nwr.put(&r)?;
                    Ok(SerDesSize::Sized(result_nwr.nibbles_pos()))
                }
            }
        }
        _ => {
            log_error!(=>T, "Resource /{} doesn't exist", at_root_level);
            Err(XpiError::BadUri)
        }
    }
    // unreachable!()
}