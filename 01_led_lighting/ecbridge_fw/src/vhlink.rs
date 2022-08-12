use rtt_target::rprintln;

use vhl_stdlib::xpi::*;
use vhl_stdlib::discrete::*;
use crate::ethernet::IpEndpointL;
use crate::xpi_dispatch::xpi_dispatch;
use vhl_stdlib::nibble_buf::NibbleBuf;
use vhl_stdlib::xpi::XpiResourceSet::Uri;

// ethernet / can irq task -> put data onto bbqueue?
// protocol processing task: data slices comes in from bbq -> uavcan/webscoket -> packets arrive
// XpiRequest is deserialized from the packet -> goes to dispatcher
pub fn link_process(mut ctx: crate::app::link_process::Context) {
    rprintln!(=>1, "link_process");

    let eth_out_cons: &mut bbqueue::Consumer<512> = ctx.local.eth_out_cons;
    match eth_out_cons.read() {
        Ok(rgr) => {
            let rgr_len = rgr.len();
            // let endpoint = IpEndpoint::des(&rgr).expect("endpoint is wrong");
            // rprintln!(=>1, "{:?}", rgr);
            let endpoint: (IpEndpointL, usize) = ssmarshal::deserialize(&rgr).unwrap();
            let (endpoint, endpoint_size) = (endpoint.0, endpoint.1);
            let buf = &rgr[endpoint_size..];

            rprintln!(=>1, "link_process got: {}B from {:?} {:02x?}", rgr_len, endpoint, buf);

            {
                let mut rgr = NibbleBuf::new(&buf);
                rprintln!(=>1, "{} {} {}", rgr.get_nibble(), rgr.get_u8(), rgr.get_vlu4_u32());
            }

            let xpi_request = XpiRequest {
                source: 172,
                destination: NodeSet::Unicast(1),
                resource_set: XpiResourceSet::Uri(&[0]), // /0
                kind: XpiRequestKind::Call { args: &[ &rgr[1..=2] ] },
                request_id: 123,
                priority: Priority::Lossy(U7Sp1::new(1).unwrap())
            };

            xpi_dispatch(&mut ctx, xpi_request);

            rgr.release(rgr_len);
        }
        Err(_) => {
            return;
        }
    }


}

