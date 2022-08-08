use rtt_target::rprintln;
use vhl_stdlib::xpi::*;
use rtic::Mutex;

pub type DispatcherContext<'c> = crate::app::link_process::Context<'c>;

// dispatcher still runs in the protocol task
// should be configurable by user what to do next with requests
// dispatcher should have access to all the resources to answer for ex. Read requests for props
// would be great to just put all the resources to rtic _resources_, so that different priority
// task can run without waiting
//
// Also need ability to send XpiReply(-s) back to the link from dispatcher
pub fn xpi_dispatch(ctx: &mut DispatcherContext, req: XpiRequest) {
    rprintln!(=>2, "{:?}", req);

    match req.kind {
        XpiRequestKind::Call { .. } => {
            let uri = 2;
            if uri == 2 {
                // Choice A
                // spawn a task
                let arg = 3;
                let r = crate::app::set_digit::spawn(arg);
                if r.is_err() {
                    rprintln!(=>2, "spawn_failed");
                }
            } else if uri == 3 {
                // Choice B
                // call directly
                crate::sync_fn();
            }
        }
        XpiRequestKind::ChainCall { .. } => {}
        XpiRequestKind::Read => {}
        XpiRequestKind::Write { .. } => {
            // Choice A - write into rtic resources
            ctx.shared.symbol.lock(|symbol| *symbol = 'X');
            crate::app::display_task::spawn().unwrap();
            // Notify someone
        }
        XpiRequestKind::OpenStreams => {}
        XpiRequestKind::CloseStreams => {}
        XpiRequestKind::Subscribe { .. } => {}
        XpiRequestKind::Unsubscribe => {}
        XpiRequestKind::Borrow => {}
        XpiRequestKind::Release => {}
        XpiRequestKind::Introspect => {}
    }
}
