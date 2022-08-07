use rtt_target::rprintln;

use vhl_stdlib::xpi::*;
use vhl_stdlib::discrete::*;
use crate::xpi_dispatch::xpi_dispatch;

// ethernet / can irq task -> put data onto bbqueue?
// protocol processing task: data slices comes in from bbq -> uavcan/webscoket -> packets arrive
// XpiRequest is deserialized from the packet -> goes to dispatcher
pub fn link_process(ctx: &mut crate::xpi_dispatch::DispatcherContext, buf: &[u8]) {
    rprintln!(=>1, "link_process");

    let xpi_request = XpiRequest {
        node_set: NodeSet::Unicast(Some(1)),
        resource_set: XpiResourceSet::Alpha(U4::new(0).unwrap()), // /0
        kind: XpiRequestKind::Call { args: &[ &buf[1..=2] ] },
        request_id: 123,
        priority: Priority::Lossy(U7Sp1::new(1).unwrap())
    };

    xpi_dispatch(ctx, xpi_request);
}

