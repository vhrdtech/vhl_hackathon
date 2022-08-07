use rtt_target::rprintln;

use vhl_stdlib::xpi::*;
use vhl_stdlib::discrete::*;

// ethernet / can irq task -> put data onto bbqueue?
// protocol processing task: data slices comes in from bbq -> uavcan/webscoket -> packets arrive
// XpiRequest is deserialized from the packet -> goes to dispatcher
fn link_process(buf: &[u8]) {

    let xpi_request = XpiRequest {
        node_set: NodeSet::Unicast(Some(1)),
        resource_set: XpiResourceSet::Alpha(U4::new(0).unwrap()), // /0
        kind: XpiRequestKind::Call { args: &[ &buf[1..=2] ] },
        request_id: 123,
        priority: Priority::Lossy(U7Sp1::new(1).unwrap())
    };
    xpi_dispatch(xpi_request);
}

// dispatcher still runs in the protocol task
// should be configurable by user what to do next with requests
// dispatcher should have access to all the resources to answer for ex. Read requests for props
// would be great to just put all the resources to rtic _resources_, so that different priority
// task can run without waiting
//
// Also need ability to send XpiReply(-s) back to the link from dispatcher
fn xpi_dispatch(req: XpiRequest) {
    rprintln!("{:?}", req);

    // Choice A
    //spawn a task

    // Choice B
    //call fn directly
}
