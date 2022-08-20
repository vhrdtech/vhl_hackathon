use rtt_target::rprintln;
use stm32l4xx_hal::gpio::{Alternate, H8, Output, PushPull};
use stm32l4xx_hal::spi::Spi;
use stm32l4xx_hal::stm32::SPI1;
use sx127x_lora::LoRa;
use crate::radio::{FrameType, HeartBeat, RadioFrame};
use crate::util::FakeDelay;

pub type Miso = stm32l4xx_hal::gpio::Pin<Alternate<PushPull, 5_u8>, stm32l4xx_hal::gpio::L8, 'B', 4_u8>;
pub type Mosi = stm32l4xx_hal::gpio::Pin<Alternate<PushPull, 5_u8>, stm32l4xx_hal::gpio::L8, 'B', 5_u8>;
pub type Sck = stm32l4xx_hal::gpio::Pin<Alternate<PushPull, 5_u8>, stm32l4xx_hal::gpio::L8, 'B', 3_u8>;
pub type Cs = stm32l4xx_hal::gpio::Pin<Output<PushPull>, H8, 'A', 8_u8>;
pub type Reset = stm32l4xx_hal::gpio::Pin<Output<PushPull>, H8, 'C', 8_u8>;
pub type LoraSpi = Spi<SPI1, (Sck, Miso, Mosi)>;
pub type Radio = Option<LoRa<LoraSpi, Cs, Reset>>;

pub fn lora_task(radio: &mut Radio, is_rx: bool, local_heartbeat: &mut HeartBeat, remote_heartbeat: &mut HeartBeat) -> bool {
    rprintln!("lora_task is_rx: {}", is_rx);
    match radio {
        Some(radio) => {
            let mut buf = [0u8; 255];
            let mut delay = FakeDelay{};

            if is_rx {
                let poll = radio.poll_irq(Some(50), &mut delay);
                match poll {
                    Ok(_) => {
                        match radio.read_packet(&mut buf) { // Received buffer. NOTE: 255 bytes are always returned
                            Ok(packet) => {
                                rprintln!("LoRa packet:");
                                // led_green.toggle();
                                // for b in packet {
                                //     rprint!("{:02x} ", *b);
                                // }
                                let frame = RadioFrame::deserialize(packet);

                                match frame {
                                    Ok(frame) => {
                                        match frame.frame_type {
                                            // FrameType::CANBusForward(can_frame) => {
                                            //     rprintln!("Forwarding: {:?}", can_frame);
                                            //     can.transmit(&Frame::new_data(
                                            //         vhrdcanid2bxcanid(can_frame.id),
                                            //         Data::new(can_frame.data()).unwrap(),
                                            //     )).ok();
                                            // }
                                            FrameType::HeartBeat(hb) => {
                                                let rssi_rx = radio.get_packet_rssi().unwrap_or(-777);
                                                rprintln!("RSSI local: {} Heartbeat: {:?}", rssi_rx, hb);
                                                *remote_heartbeat = hb;
                                                return true;

                                            }

                                        }
                                    }
                                    Err(e) => {
                                        rprintln!("Deser err: {:?}", e);
                                        // led_red.toggle();
                                    }
                                }
                            },
                            Err(_) => {}
                        }
                    },
                    Err(_) => {
                        // rprintln!("LoRa rx timeout");
                        // str_buf.clear();
                        // write!(str_buf, "No Signal");
                        // display.clear();
                        // Text::new(str_buf.as_str(), Point::new(5, 10), style).draw(&mut display).unwrap();
                        // display.flush().unwrap();
                        // led_red.toggle();
                    }
                }
            } else {

                local_heartbeat.remote_rssi = radio.get_packet_rssi().unwrap_or(-777);
                local_heartbeat.uptime += 1;


                let frame = RadioFrame::new(10, 110, FrameType::HeartBeat(*local_heartbeat));
                match frame.serialize(&mut buf) {
                    Ok(buf) => {
                        match radio.transmit_payload(buf) {
                            Ok(_) => {
                                while radio.transmitting().unwrap_or(false) {
                                    cortex_m::asm::delay(1000);
                                }
                                rprintln!("Sent LoRa packet");
                                return true;

                            },
                            Err(e) => {
                                rprintln!("LoRa TX err: {:?}", e);
                            }
                        }
                    },
                    Err(e) => {
                        rprintln!("Ser error: {:?}", e);
                    }
                }
            }
        },
        None => {}
    }
    false
}