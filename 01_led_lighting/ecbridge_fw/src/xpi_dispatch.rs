use rtt_target::{rprintln};
use vhl_stdlib::serdes::xpi_vlu4::{UriIter};
use vhl_stdlib::serdes::xpi_vlu4::request::{XpiRequest, XpiRequestKind};
use crate::{log_error, log_info, log_trace};

pub type DispatcherContext<'c> = crate::app::link_process::Context<'c>;

const T: u8 = 2;

// dispatcher still runs in the protocol task
// should be configurable by user what to do next with requests
// dispatcher should have access to all the resources to answer for ex. Read requests for props
// would be great to just put all the resources to rtic _resources_, so that different priority
// task can run without waiting
//
// Also need ability to send XpiReply(-s) back to the link from dispatcher
pub fn xpi_dispatch(ctx: &mut DispatcherContext, req: &XpiRequest) {
    rprintln!(=>T, "{}", req);

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
            let mut args_set_iter = args_set.iter();
            for uri in req.resource_set.flat_iter() {
                match args_set_iter.next() {
                    Some(args) => {
                        dispatch_call(uri, args, );
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

fn dispatch_call(mut uri: UriIter, args: &[u8])
{
    log_info!(=>T, "dispatch_call({})", uri);
    let at_root_level = match uri.next() {
        Some(p) => p,
        None => {
            log_error!(=>T, "Expected root level");
            return;
        }
    };
    match at_root_level {
        0 | 1 | 2 => {
            log_error!(=>T, "Resource /{} is not a method", at_root_level);
        }
        3 => {
            // /sync< fn(a: u8, b: u8) -> u8, '3>
            // TODO: Need to be a proper Buf deserializing into expected types + error handling
            let a = args[0];
            let b = args[1];
            let r = crate::sync(a, b);
            log_trace!(=>T, "Called /sync({}, {}) = {}", a, b, r);
        }
        4 => {
            // /sync< fn(a: u8, b: u8) -> u8, '3>
            // TODO: Need to be a proper Buf deserializing into expected types + error handling
            let a = args[0];
            let b = args[1];
            let r = crate::sync_2(a, b);
            log_trace!(=>T, "Called /sync_2({}, {}) = {}", a, b, r);
        }
        _ => {
            log_error!(=>T, "Resource /{} doesn't exist", at_root_level);
        }
    }
}