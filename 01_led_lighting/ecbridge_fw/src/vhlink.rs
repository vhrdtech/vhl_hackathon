use rtt_target::rprintln;

use crate::ethernet::IpEndpointL;
use crate::xpi_dispatch::xpi_dispatch;
use vhl_stdlib::serdes::NibbleBuf;
use xpi::xwfd::{XpiRequestVlu4, Event};
use xpi::event::XpiGenericEventKind;

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

            let mut rdr = NibbleBuf::new_all(&buf);

            let xpi_event: Event = match rdr.des_vlu4() {
                Ok(req) => req,
                Err(e) => {
                    rprintln!("{:?}", e);
                    return;
                }
            };
            xpi_dispatch(&mut ctx, &xpi_event);

            rgr.release(rgr_len);
        }
        Err(_) => {
            return;
        }
    }


}

