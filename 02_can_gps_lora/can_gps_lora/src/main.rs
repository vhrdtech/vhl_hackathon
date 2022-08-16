#![no_main]
#![no_std]

mod radio;

extern crate panic_rtt_target;

use core::fmt;
use cfg_if::cfg_if;
use cortex_m::asm::delay;
use embedded_graphics::Drawable;
use embedded_graphics::geometry::{OriginDimensions, Point};
use embedded_graphics::image::{Image, ImageRaw};
use embedded_graphics::mono_font::ascii::FONT_9X18_BOLD;
use embedded_graphics::mono_font::MonoTextStyle;
use embedded_graphics::pixelcolor::{BinaryColor, Rgb565};
use embedded_graphics::text::Text;
use embedded_graphics::transform::Transform;
use heapless::{consts::U8, spsc};
use nb::block;
use rtt_target::{rprint, rprintln};
use ssd1306::{I2CDisplayInterface, Ssd1306};
use ssd1306::mode::DisplayConfig;
use ssd1306::prelude::{DisplayRotation, DisplaySize128x64};
use ssd1331::DisplayRotation::{Rotate0, Rotate180};
use stm32l4xx_hal::{i2c, pac::{self, LPUART1}, prelude::*, serial::{self, Config, Serial}};
use stm32l4xx_hal::delay::Delay;
use stm32l4xx_hal::gpio::{Floating, H8, Input, L8, Output, Pin, PushPull};
use stm32l4xx_hal::hal::blocking::delay::DelayMs;
use stm32l4xx_hal::i2c::I2c;
use stm32l4xx_hal::spi::Spi;
use sx127x_lora::MODE;
use tinybmp::Bmp;
use crate::radio::{FrameType, RadioFrame, HeartBeat};

pub const SYSCLK: u32 = 64_000_000;

#[rtic::app(device = stm32l4xx_hal::pac)]
const APP: () = {
    struct Resources {
        rx: serial::Rx<LPUART1>,
        tx: serial::Tx<LPUART1>,

        pps_input: Pin<Input<Floating>, H8, 'B', 14>,

        //
        // rx_prod: spsc::Producer<'static, u8, U8>,
        // rx_cons: spsc::Consumer<'static, u8, U8>,
    }

    #[init]
    fn init(cx: init::Context) -> init::LateResources {
        static mut RX_QUEUE: spsc::Queue<u8, U8> = spsc::Queue(heapless::i::Queue::new());

        rtt_target::rtt_init_print!();
        rprint!("Initializing... ");

        let p = pac::Peripherals::take().unwrap();


        let mut rcc = p.RCC.constrain();
        let mut flash = p.FLASH.constrain();
        let mut pwr = p.PWR.constrain(&mut rcc.apb1r1);

        let clocks = rcc.cfgr.sysclk(SYSCLK.Hz()).freeze(&mut flash.acr, &mut pwr);
        // let mut delay = Delay::new(cp.SYST, clocks);

        let mut gpioa = p.GPIOA.split(&mut rcc.ahb2);
        let mut gpiob = p.GPIOB.split(&mut rcc.ahb2);
        let mut gpioc = p.GPIOC.split(&mut rcc.ahb2);
        let mut gpiod = p.GPIOD.split(&mut rcc.ahb2);

        let mut on_off = gpioc.pc4.into_push_pull_output(&mut gpioc.moder, &mut gpioc.otyper);
        let tx_pin = gpioc
            .pc1
            .into_alternate(&mut gpioc.moder, &mut gpioc.otyper, &mut gpioc.afrl);
        let rx_pin = gpioc
            .pc0
            .into_alternate(&mut gpioc.moder, &mut gpioc.otyper, &mut gpioc.afrl);

        // -> rts
        let cts_in = gpiob.pb13.into_floating_input(&mut gpiob.moder, &mut gpiob.pupdr);

        // -> cts
        let mut rts_out = gpiob.pb12.into_open_drain_output(&mut gpiob.moder, &mut gpiob.otyper);
        rts_out.set_low();
        // rts_out.set_high();

        let pps_input = gpiob.pb14.into_floating_input(&mut gpiob.moder, &mut gpiob.pupdr);

        let mut led_red = gpiob.pb2.into_push_pull_output(&mut gpiob.moder, &mut gpiob.otyper);
        let mut led_green = gpioa.pa10.into_push_pull_output(&mut gpioa.moder, &mut gpioa.otyper);


        let mut serial = Serial::lpuart1(
            p.LPUART1,
            (tx_pin, rx_pin),
            Config::default().baudrate(115200.bps()),
            clocks,
            &mut rcc.apb1r2,
        );
        serial.listen(serial::Event::Rxne);

        let sck = gpiob
            .pb3
            .into_alternate(&mut gpiob.moder, &mut gpiob.otyper, &mut gpiob.afrl);
        let miso = gpiob
            .pb4
            .into_alternate(&mut gpiob.moder, &mut gpiob.otyper, &mut gpiob.afrl);
        let mosi = gpiob
            .pb5
            .into_alternate(&mut gpiob.moder, &mut gpiob.otyper, &mut gpiob.afrl);
        let mut spi = Spi::spi1(
            p.SPI1,
            (sck, miso, mosi),
            MODE,
             1.MHz(),
            // 100.kHz(),
            clocks,
            &mut rcc.apb2,
        );
        let rfmod_cs = gpioa.pa8.into_push_pull_output(&mut gpioa.moder, &mut gpioa.otyper);
        let rfmod_rst = gpioc.pc8.into_push_pull_output(&mut gpioc.moder, &mut gpioc.otyper);

        let mut delay = FakeDelay{};
        let lora = sx127x_lora::LoRa::new(spi, rfmod_cs, rfmod_rst, 915_i64, &mut delay);
        let mut lora = match lora {
            Ok(l) => {
                rprintln!("LoRa init ok");
                l
            },
            Err(e) => {
                loop {
                    rprintln!("Lora error: {:?}", e);
                    delay.delay_ms(250u8);
                }
            }
        };
        let r = lora.set_tx_power(17,1); //Using PA_BOOST. See your board for correct pin.
        rprintln!("set_tx_power: {:?}", r);

        cfg_if! {
            if #[cfg(feature = "oled_bw_ssd1306")] {
                let mut scl =
                    gpiob
                        .pb6
                        .into_alternate_open_drain(&mut gpiob.moder, &mut gpiob.otyper, &mut gpiob.afrl);
                scl.internal_pull_up(&mut gpiob.pupdr, true);

                let mut sda =
                    gpiob
                        .pb7
                        .into_alternate_open_drain(&mut gpiob.moder, &mut gpiob.otyper, &mut gpiob.afrl);
                sda.internal_pull_up(&mut gpiob.pupdr, true);

                let mut i2c = I2c::i2c1(
                    p.I2C1,
                    (scl, sda),
                    i2c::Config::new(100.kHz(), clocks),
                    &mut rcc.apb1r1,
                );

                rprintln!("i2c scan...");
                for i in 0..=127 {
                    let mut buf = [0u8];
                    if i2c.read(i, &mut buf).is_ok() {
                        rprintln!("{:02x} found", i);
                    }
                }

                let interface = I2CDisplayInterface::new(i2c);
                let mut display = Ssd1306::new(interface, DisplaySize128x64, DisplayRotation::Rotate0)
                    .into_buffered_graphics_mode();
                display.init().unwrap();

                let raw: ImageRaw<BinaryColor> = ImageRaw::new(include_bytes!("../rust.raw"), 64);
                let im = Image::new(&raw, Point::new(32, 0));
                im.draw(&mut display).unwrap();
                display.flush().unwrap();
            } else if #[cfg(feature = "oled_color_ssd1331")] {
                let sck = gpioc
                    .pc10
                    .into_alternate(&mut gpioc.moder, &mut gpioc.otyper, &mut gpioc.afrh);
                let miso = gpioc
                    .pc11
                    .into_alternate(&mut gpioc.moder, &mut gpioc.otyper, &mut gpioc.afrh);
                let mosi = gpioc
                    .pc12
                    .into_alternate(&mut gpioc.moder, &mut gpioc.otyper, &mut gpioc.afrh);
                let mut spi3 = Spi::spi3(
                    p.SPI3,
                    (sck, miso, mosi),
                    MODE,
                    // 1.MHz(),
                    100.kHz(),
                    clocks,
                    &mut rcc.apb1r1,
                );
                let mut oled_reset = gpiod.pd2.into_push_pull_output(&mut gpiod.moder, &mut gpiod.otyper);
                oled_reset.set_low();
                delay.delay_ms(100u8);
                oled_reset.set_high();
                let oled_dc = gpioc.pc9.into_push_pull_output(&mut gpioc.moder, &mut gpioc.otyper);

                let mut display = ssd1331::Ssd1331::new(spi3, oled_dc, Rotate180);
                display.init().unwrap();
                display.clear();
                display.flush().unwrap();

                let (w, h) = display.dimensions();

                let bmp = tinybmp::Bmp::from_slice(include_bytes!("../rust_pride.bmp"))
                    .expect("Failed to load BMP image");

                let im: Image<Bmp<Rgb565>> = Image::new(&bmp, Point::zero());

                // Position image in the center of the display
                let moved = im.translate(Point::new(
                    (w as u32 - bmp.size().width) as i32 / 2,
                    (h as u32 - bmp.size().height) as i32 / 2,
                ));

                moved.draw(&mut display).unwrap();

                display.flush().unwrap();
            }
        }

        let mut buf = [0u8; 32];
        let mut buf = StrWriter::new(&mut buf);
        use core::fmt::Write;
        write!(buf, "N:{}", 123);

        let style = MonoTextStyle::new(&FONT_9X18_BOLD, BinaryColor::On);
        Text::new(buf.as_str(), Point::new(5, 10), style).draw(&mut display).unwrap();
        display.flush().unwrap();
        rprintln!("str: {}", buf.as_str());



        let (tx, rx) = serial.split();
        let (rx_prod, rx_cons) = RX_QUEUE.split();

        // let mut delay = Delay::new(cp.SYST, clocks);
        // delay.delay_ms(1000u32);
        // delay(80_000_000);
        // on_off.set_high();
        // delay(1_000_000);
        // on_off.set_low();

        rprintln!("done.");

        let mut buf = [0u8; 255];
        let mut hb = HeartBeat {
            uptime: 0,
            remote_rssi: 0
        };

        loop {
            cfg_if! {
                if #[cfg(feature = "oled_bw_ssd1306")] {
                    let poll = lora.poll_irq(Some(100), &mut delay);
                    match poll {
                        Ok(size) => {
                            match lora.read_packet(&mut buf) { // Received buffer. NOTE: 255 bytes are always returned
                                Ok(packet) => {
                                    rprintln!("LoRa packet:");
                                    led_green.toggle();
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
                                                    rprintln!("RSSI: {} Heartbeat: {:?}", lora.get_packet_rssi().unwrap_or(-777), hb);
                                                }
                                            }
                                        }
                                        Err(e) => {
                                            rprintln!("Deser err: {:?}", e);
                                            led_red.toggle();
                                        }
                                    }
                                },
                                Err(_) => {}
                            }
                        },
                        Err(_) => { rprintln!("LoRa rx timeout"); led_red.toggle(); }
                    }
                }
            }

            cfg_if! {
                if #[cfg(feature = "oled_color_ssd1331")] {
                    loop {
                        hb.remote_rssi = lora.get_packet_rssi().unwrap_or(-777);
                        hb.uptime += 1;

                        let frame = RadioFrame::new(10, 110, FrameType::HeartBeat(hb));
                        match frame.serialize(&mut buf) {
                            Ok(buf) => {
                                match lora.transmit_payload(buf) {
                                    Ok(_) => {
                                        while lora.transmitting().unwrap_or(false) {
                                            cortex_m::asm::delay(1000);
                                        }
                                        rprintln!("Sent LoRa packet");
                                        led_green.toggle();

                                    },
                                    Err(e) => {
                                        rprintln!("LoRa TX err: {:?}", e),
                                        led_red.toggle();
                                    }
                                }
                            },
                            Err(e) => {
                                rprintln!("Ser error: {:?}", e);
                                led_red.toggle();
                            }
                        }
                        delay.delay_ms(100_u8);
                    }
                }
            }
        }

        init::LateResources {
            rx,
            tx,

            pps_input,
            //
            // rx_prod,
            // rx_cons,
        }
    }

    #[idle(resources = [tx, pps_input,  ])]
    fn idle(cx: idle::Context) -> ! {
        // let rx = cx.resources.rx_cons;
        let tx = cx.resources.tx;
        let pps_input: &mut Pin<Input<Floating>, H8, 'B', 14> = cx.resources.pps_input;
        // let led1: &mut Pin<Output<PushPull>, L8, 'B', 2> = cx.resources.led1;

        loop {
            // if let Some(b) = rx.dequeue() {
            //     rprintln!("Echoing '{}'", b as char);
            //     block!(tx.write(b)).unwrap();
            // }
            // block!(tx.write('x' as u8)).unwrap();
            cortex_m::asm::delay(1_000_000);
            //
            // if pps_input.is_high() {
            //     led1.set_high();
            // } else {
            //     led1.set_low();
            // }
        }
    }

    #[task(binds = LPUART1, resources = [rx])]
    fn usart2(cx: usart2::Context) {
        let rx = cx.resources.rx;
        // let queue = cx.resources.rx_prod;

        let b = match rx.read() {
            Ok(b) => {
                rprintln!("Read: {}", b);
            },
            Err(err) => {
                rprintln!("Error reading from USART: {:?}", err);
                return;
            }
        };
        // match queue.enqueue(b) {
        //     Ok(()) => (),
        //     Err(err) => {
        //         rprintln!("Error adding received byte to queue: {:?}", err);
        //         return;
        //     }
        // }
    }
};

pub struct FakeDelay {}
impl DelayMs<u8> for FakeDelay {
    fn delay_ms(&mut self, ms: u8) {
        delay(ms as u32 * (SYSCLK / 1_000))
    }
}

pub struct StrWriter<'i> {
    buf: &'i mut [u8],
    pos: usize,
}

impl<'i> StrWriter<'i> {
    pub fn new(buf: &'i mut [u8]) -> Self {
        StrWriter {
            buf, pos: 0
        }
    }

    pub fn as_str(&'i self) -> &'i str {
        unsafe { core::str::from_utf8_unchecked(&self.buf[..self.pos]) }
    }
}

impl<'i> fmt::Write for StrWriter<'i> {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        rprintln!("write_str: {}", s);
        let s = s.as_bytes();
        if self.buf.len() - self.pos < s.len() {
            return Err(fmt::Error{})
        }
        self.buf[self.pos .. self.pos + s.len()].copy_from_slice(s);
        self.pos += s.len();
        rprintln!("ok");
        Ok(())
    }
}