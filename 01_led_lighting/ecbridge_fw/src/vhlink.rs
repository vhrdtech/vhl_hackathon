use rtt_target::rprintln;

use crate::ethernet::IpEndpointL;
use crate::xpi_dispatch::xpi_dispatch;
use vhl_stdlib::serdes::NibbleBuf;
use xpi::error::XpiError;
use xpi::xwfd::{Event};
use crate::log_error;

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

            let xpi_event: Result<Event, _> = rdr.des_vlu4();
            match xpi_event {
                Ok(ev) => {
                    match xpi_dispatch(&mut ctx, &ev) {
                        Ok(_) => {}
                        Err(e) => {
                            log_error!(=>1, "xpi_dispatch err: {:?}", e);
                        }
                    }
                },
                Err(e) => {
                    rprintln!(=>1, "{:?}", e);
                }
            };

            rgr.release(rgr_len);
        }
        Err(_) => {
            return;
        }
    }


}

