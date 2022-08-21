use core::hint::unreachable_unchecked;
use rtt_target::{rprintln};
use vhl_cg::point::Point;
use vhl_stdlib::serdes::buf::{Buf, BufMut};
use vhl_stdlib::serdes::xpi_vlu4::{UriIter};
use vhl_stdlib::serdes::xpi_vlu4::request::{XpiRequest, XpiRequestKind};
use crate::{log_error, log_info, log_trace, log_warn};
use vhl_stdlib::serdes::buf::Error as BufError;
use vhl_stdlib::serdes::xpi_vlu4::error::FailReason;

pub type DispatcherContext<'c> = crate::app::link_process::Context<'c>;

const T: u8 = 2;

#[derive(Debug)]
pub enum Error {
    BufError(BufError),
    XpiFailReason(FailReason),
}

impl From<BufError> for Error {
    fn from(e: BufError) -> Self {
        Error::BufError(e)
    }
}

// dispatcher still runs in the protocol task
// should be configurable by user what to do next with requests
// dispatcher should have access to all the resources to answer for ex. Read requests for props
// would be great to just put all the resources to rtic _resources_, so that different priority
// task can run without waiting
//
// Also need ability to send XpiReply(-s) back to the link from dispatcher
pub fn xpi_dispatch(ctx: &mut DispatcherContext, req: &XpiRequest) {
    rprintln!(=>T, "{}", req);

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

    match req.kind {
        XpiRequestKind::Call { args_set } => {
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
            // const REPLY_MTU: usize = 64;


            let mut args_set_iter = args_set.iter();
            for uri in req.resource_set.flat_iter() {
                match args_set_iter.next() {
                    Some(args) => {
                        let r = dispatch_call(uri, DispatchCallType::RealRun { args, result: &mut[] } );
                        if r.is_err() {
                            log_error!(=>T, "dispatch error: {:?}", r);
                        }
                    }
                    None => {
                        log_error!(=>T, "No args provided for {}", uri);
                    }
                }
            }
        }
        XpiRequestKind::ChainCall { .. } => {}
        XpiRequestKind::Read => {}
        XpiRequestKind::Write { .. } => {}
        XpiRequestKind::OpenStreams => {}
        XpiRequestKind::CloseStreams => {}
        XpiRequestKind::Subscribe { .. } => {}
        XpiRequestKind::Unsubscribe => {}
        XpiRequestKind::Borrow => {}
        XpiRequestKind::Release => {}
        XpiRequestKind::Introspect => {}
    }

    // match req.kind {
    //     XpiRequestKind::Call { args } => {
    //         let uri = 2;
    //         if uri == 2 {
    //             // Choice A
    //             // spawn a task
    //             let slice = args.iter().next().unwrap();
    //             let arg = slice[0];
    //             let r = crate::app::set_digit::spawn(arg);
    //             if r.is_err() {
    //                 rprintln!(=>2, "spawn_failed");
    //             }
    //
    //             // 0. get wgr, but for now just buf
    //             let mut buf = [0u8; 64];
    //
    //             // 1. serialize XpiReply to it
    //             // 1.1 get own NodeId from XpiNode or smth
    //             let self_node_id = NodeId::new(33).unwrap();
    //
    //             let reply_slice = [1, 2, 3];
    //             let reply_kind = XpiReplyKind::CallComplete(Ok(Vlu4Slice { slice: &reply_slice }));
    //             let reply = XpiReply {
    //                 source: self_node_id,
    //                 destination: NodeSet::Unicast(req.source),
    //                 kind: reply_kind,
    //                 resource_set: req.resource_set,
    //                 request_id: req.request_id,
    //                 priority: req.priority
    //             };
    //
    //
    //
    //             // 2. send back
    //
    //         } else if uri == 3 {
    //             // should be generated:
    //             let rdr = NibbleBuf::new()
    //             let a =
    //             let r = crate::sync_fn(a, b);
    //         }
    //     }
    //     XpiRequestKind::ChainCall { .. } => {}
    //     XpiRequestKind::Read => {}
    //     XpiRequestKind::Write { .. } => {
    //         // Choice A - write into rtic resources
    //         ctx.shared.symbol.lock(|symbol| *symbol = 'X');
    //         crate::app::display_task::spawn().unwrap();
    //         // Notify someone
    //     }
    //     XpiRequestKind::OpenStreams => {}
    //     XpiRequestKind::CloseStreams => {}
    //     XpiRequestKind::Subscribe { .. } => {}
    //     XpiRequestKind::Unsubscribe => {}
    //     XpiRequestKind::Borrow => {}
    //     XpiRequestKind::Release => {}
    //     XpiRequestKind::Introspect => {}
    // }
}

enum DispatchCallType<'i> {
    DryRun,
    RealRun {
        args: &'i [u8],
        result: &'i mut [u8],
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
fn dispatch_call(mut uri: UriIter, call_type: DispatchCallType) -> Result<usize, Error>
{
    use DispatchCallType::*;

    log_info!(=>T, "dispatch_call({})", uri);
    let at_root_level = match uri.next() {
        Some(p) => p,
        None => {
            log_error!(=>T, "Expected root level");
            return Err(Error::XpiFailReason(FailReason::BadUri));
        }
    };
    match at_root_level {
        0 | 1 | 2 => {
            log_error!(=>T, "Resource /{} is not a method", at_root_level);
            Err(Error::XpiFailReason(FailReason::NotAMethod))
        }
        3 => {
            // /sync< fn(a: u8, b: u8) -> u8, '3>
            // TODO: Need to be a proper Buf deserializing into expected types + error handling
            match call_type {
                DryRun => Ok(1),
                RealRun { args, result } => {
                    let a = args[0];
                    let b = args[1];
                    let r = crate::sync(a, b);
                    log_trace!(=>T, "Called /sync({}, {}) = {}", a, b, r);
                    result[0] = r;
                    Ok(1)
                }
            }
        }
        4 => {
            match call_type {
                DryRun => Ok(1),
                RealRun { args, result} => {
                    // /sync< fn(a: u8, b: u8) -> u8, '3>
                    // TODO: Need to be a proper Buf deserializing into expected types + error handling
                    let a = args[0];
                    let b = args[1];
                    let r = crate::sync_2(a, b);
                    log_trace!(=>T, "Called /sync_2({}, {}) = {}", a, b, r);
                    result[0] = r;
                    Ok(1)
                }
            }
        }
        5 => {
            match call_type {
                DryRun => Ok(4),
                RealRun { args, result } => {
                    let mut rdr = Buf::new(args);
                    let p1: Point = rdr.des_bytes()?;
                    let p2: Point = rdr.des_bytes()?;
                    if !rdr.is_at_end() {
                        log_warn!(=>T, "Unused {} bytes left after deserializing arguments", rdr.bytes_left());
                    }
                    let r = crate::sync_3(p1, p2);
                    log_trace!(=>T, "Called /sync3({:?}, {:?}) = {:?}", p1, p2, r);
                    // size of return type is known in advance = 8
                    // let mut wgr = BufMut::new();
                    // wgr.put(r)?;
                    Ok(4)
                }
            }
        }
        _ => {
            log_error!(=>T, "Resource /{} doesn't exist", at_root_level);
            Err(Error::XpiFailReason(FailReason::BadUri))
        }
    }
    // unreachable!()
}