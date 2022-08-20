#![no_main]
#![no_std]

mod radio;
mod oled_bw_ssd1306;
mod util;
mod lora;
mod oled_color_ssd1331;

use panic_rtt_target as _;

#[rtic::app(device = stm32l4xx_hal::stm32, peripherals = true, dispatchers = [SAI1, SAI2])]
mod app {
    use cfg_if::cfg_if;

    use heapless::{consts::U8, spsc};
    use nb::block;
    use rtt_target::{DownChannel, rprint, rprintln, rtt_init};
    use stm32l4xx_hal::{i2c, pac::{LPUART1}, prelude::*, serial::{Config, Serial}, serial};
    use stm32l4xx_hal::gpio::{Edge, Floating, H8, Input, Output, Pin, PushPull};
    use stm32l4xx_hal::hal::blocking::delay::DelayMs;
    use stm32l4xx_hal::i2c::I2c;
    use stm32l4xx_hal::spi::Spi;
    use sx127x_lora::MODE;
    use crate::radio::{HeartBeat};
    use stm32l4xx_hal::rcc::{ClockSecuritySystem, CrystalBypass, LpUart1ClockSource, PllConfig, PllDivider, PllSource};
    use dwt_systick_monotonic::{DwtSystick, ExtU64};
    use embedded_graphics::Drawable;
    use embedded_graphics::geometry::{OriginDimensions, Point};
    use embedded_graphics::image::{Image, ImageRaw};
    use embedded_graphics::pixelcolor::BinaryColor;
    use embedded_graphics::prelude::Transform;
    use ssd1331::DisplayRotation::Rotate180;
    use tinybmp::Bmp;
    use crate::app;
    use crate::lora::lora_task;
    use crate::util::{FakeDelay};

    use crate::oled_bw_ssd1306::oled_ssd1306_task;
    use crate::oled_color_ssd1331::oled_ssd1331_task;

    #[shared]
    struct SharedResources {
        local_heartbeat: HeartBeat,
        remote_heartbeat: HeartBeat,
        tx_flag: bool,
    }

    #[local]
    struct LocalResources {
        rx: serial::Rx<LPUART1>,
        tx: serial::Tx<LPUART1>,

        lora: crate::lora::Radio,

        pps_input: Pin<Input<Floating>, H8, 'B', 14>,

        rtt_down: DownChannel,

        #[cfg(feature = "oled_bw_ssd1306")]
        oled_ssd1306: crate::oled_bw_ssd1306::Display,
        #[cfg(feature = "oled_color_ssd1331")]
        oled_ssd1331: crate::oled_color_ssd1331::Display,

        //
        // rx_prod: spsc::Producer<'static, u8, U8>,
        // rx_cons: spsc::Consumer<'static, u8, U8>,

        led_green: stm32l4xx_hal::gpio::Pin<Output<PushPull>, H8, 'A', 10_u8>,
    }

    pub const SYSCLK: u32 = 24_000_000;
    #[monotonic(binds = SysTick, default = true)]
    type DwtSystMono = DwtSystick<SYSCLK>;

    #[init(local = [rx_queue: spsc::Queue<u8, U8> = spsc::Queue(heapless::i::Queue::new())])]
    fn init(cx: init::Context) -> (SharedResources, LocalResources, init::Monotonics) {
        // static mut RX_QUEUE: spsc::Queue<u8, U8> = spsc::Queue(heapless::i::Queue::new());

        // rtt_target::rtt_init_print!();
        let channels = rtt_init! {
            up: {
                0: { // channel number
                    size: 1024 // buffer size in bytes
                    mode: NoBlockSkip // mode (optional, default: NoBlockSkip, see enum ChannelMode)
                    name: "Terminal" // name (optional, default: no name)
                }
            }
            down: {
                0: {
                    size: 64
                    name: "Terminal"
                }
            }
        };

        rtt_target::set_print_channel(channels.up.0);

        rprintln!("Initializing... ");

        // let p = pac::Peripherals::take().unwrap();
        let mut p = cx.device;

        let mut dcb = cx.core.DCB;
        let dwt = cx.core.DWT;
        let systick = cx.core.SYST;
        let mono = DwtSystick::new(&mut dcb, dwt, systick, SYSCLK);

        let mut rcc = p.RCC.constrain();
        let mut flash = p.FLASH.constrain();
        let mut pwr = p.PWR.constrain(&mut rcc.apb1r1);

        let clocks = rcc.cfgr
            .hse(16.MHz(), CrystalBypass::Disable, ClockSecuritySystem::Disable)
            .pll_source(PllSource::HSE)
            .sysclk_with_pll(SYSCLK.Hz(), PllConfig::new(1, 12, PllDivider::Div8))
            .lpuart1_clk_src(LpUart1ClockSource::Pclk)
            .freeze(&mut flash.acr, &mut pwr);
        // let mut delay = Delay::new(cp.SYST, clocks);

        let mut gpioa = p.GPIOA.split(&mut rcc.ahb2);
        let mut gpiob = p.GPIOB.split(&mut rcc.ahb2);
        let mut gpioc = p.GPIOC.split(&mut rcc.ahb2);
        let mut gpiod = p.GPIOD.split(&mut rcc.ahb2);

        let on_off = gpioc.pc4.into_push_pull_output(&mut gpioc.moder, &mut gpioc.otyper);
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

        let mut pps_input = gpiob.pb14.into_floating_input(&mut gpiob.moder, &mut gpiob.pupdr);
        pps_input.make_interrupt_source(&mut p.SYSCFG, &mut rcc.apb2);
        pps_input.enable_interrupt(&mut p.EXTI);
        pps_input.trigger_on_edge(&mut p.EXTI, Edge::Falling);


        let mut led_red = gpiob.pb2.into_push_pull_output(&mut gpiob.moder, &mut gpiob.otyper);
        let mut led_green = gpioa.pa10.into_push_pull_output(&mut gpioa.moder, &mut gpioa.otyper);
        led_red.set_high();
        led_green.set_high();

        let mut serial = Serial::lpuart1(
            p.LPUART1,
            (tx_pin, rx_pin),
            Config::default().baudrate(9600.bps()),
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
        let mut lora = sx127x_lora::LoRa::new(spi, rfmod_cs, rfmod_rst, 915_i64, &mut delay);
        let lora = match lora {
            Ok(mut l) => {
                rprintln!("LoRa init ok");
                let r = l.set_tx_power(17,1); //Using PA_BOOST. See your board for correct pin.
                rprintln!("set_tx_power: {:?}", r);
                Some(l)
            },
            Err(e) => {
                rprintln!("Lora error: {:?}", e);
                None
            }
        };

        cfg_if! {
            if #[cfg(feature = "oled_bw_ssd1306")] {
                use ssd1306::prelude::*;

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

                let interface = ssd1306::I2CDisplayInterface::new(i2c);
                let mut display = ssd1306::Ssd1306::new(interface, DisplaySize128x64, DisplayRotation::Rotate0)
                    .into_buffered_graphics_mode();
                display.init().unwrap();

                let raw: ImageRaw<BinaryColor> = ImageRaw::new(include_bytes!("../rust.raw"), 64);
                let im = Image::new(&raw, Point::new(32, 0));
                im.draw(&mut display).unwrap();
                display.flush().unwrap();
                rprintln!("Display init done.");

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
                    4.MHz(),
                    // 100.kHz(),
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

                let im: Image<tinybmp::Bmp<embedded_graphics::pixelcolor::Rgb565>> = Image::new(&bmp, Point::zero());

                // Position image in the center of the display
                let moved = im.translate(Point::new(
                    (w as u32 - bmp.size().width) as i32 / 2,
                    (h as u32 - bmp.size().height) as i32 / 2,
                ));

                moved.draw(&mut display).unwrap();

                display.flush().unwrap();

            }
        }
        delay.delay_ms(255u8);
        display.clear();
        display.flush().unwrap();

        let (tx, rx) = serial.split();
        let (rx_prod, rx_cons) = cx.local.rx_queue.split();

        // let mut delay = Delay::new(cp.SYST, clocks);
        // delay.delay_ms(1000u32);
        // delay(80_000_000);
        // on_off.set_high();
        // delay(1_000_000);
        // on_off.set_low();

        rprintln!("done.");

        let mut hb = HeartBeat {
            uptime: 0,
            remote_rssi: 0
        };



        oled_ssd1306_task::spawn();
        oled_ssd1331_task::spawn();

        (
            SharedResources {
                local_heartbeat: HeartBeat { uptime: 0, remote_rssi: 0 },
                remote_heartbeat: HeartBeat { uptime: 0, remote_rssi: 0 },
                tx_flag: false,
            },
            LocalResources {
                rx,
                tx,

                lora,

                pps_input,

                rtt_down: channels.down.0,

                #[cfg(feature = "oled_bw_ssd1306")]
                oled_ssd1306: display,
                #[cfg(feature = "oled_color_ssd1331")]
                oled_ssd1331: display,
                //
                // rx_prod,
                // rx_cons,

                led_green
            },
            init::Monotonics(mono)
        )
    }

    #[idle(shared = [local_heartbeat, remote_heartbeat, tx_flag], local = [rtt_down, tx, lora, led_green])]
    fn idle(mut cx: idle::Context) -> ! {
        rprintln!("idle entered");
        let radio = cx.local.lora;
        // let rx = cx.resources.rx_cons;
        let tx = cx.local.tx;
        let led_green: &mut Pin<Output<PushPull>, H8, 'A', 10> = cx.local.led_green;

        let rtt_down: &mut DownChannel = cx.local.rtt_down;
        let mut buf = [0u8; 64];
        let mut counter = 0;
        loop {
            led_green.toggle();

            rprintln!("idle");
            let tx_flag = cx.shared.tx_flag.lock(|flag| {
                let was = *flag;
                *flag = false;
                was
            });
            // rprintln!("idle 2");
            let update_display = cx.shared.local_heartbeat.lock(|local| {
                cx.shared.remote_heartbeat.lock(|remote| {

                    // rprintln!("idle 3");
                    lora_task(radio, !tx_flag, local, remote)
                })
            });
            if update_display {
                app::oled_ssd1331_task::spawn();
                app::oled_ssd1306_task::spawn();
            }

            let len = rtt_down.read(&mut buf);
            if len > 0 {
                rprintln!("Sending: {}", len);
                for c in &buf[0..len] {
                    block!(tx.write(*c));
                }
            }

            // if let Some(b) = rx.dequeue() {
            //     rprintln!("Echoing '{}'", b as char);
            //     block!(tx.write(b)).unwrap();
            // }
            // block!(tx.write('x' as u8)).unwrap();
            cortex_m::asm::delay(1_000_000);
            rprint!(=> 2, ".");
            counter += 1;
            if counter == 50 {
                rprintln!(=> 2, "");
                counter = 0;
            }
            //
            // if pps_input.is_high() {
            //     led1.set_high();
            // } else {
            //     led1.set_low();
            // }
        }
    }

    #[task(priority = 2, binds = EXTI15_10, local = [pps_input])]
    fn pps_sync_task(cx: pps_sync_task::Context) {
        rprintln!("pps");
        let pps_input: &mut Pin<Input<Floating>, H8, 'B', 14> = cx.local.pps_input;
        pps_input.clear_interrupt_pending_bit();


        #[cfg(feature = "oled_bw_ssd1306")]
        let r = phase_shift_tx::spawn_after(500_u64.millis());
        #[cfg(feature = "oled_color_ssd1331")]
        let r = phase_shift_tx::spawn_after(50_u64.millis());
        if r.is_err() {
            rprintln!("pps {:?}", r);
        }

    }

    #[task(priority = 2, shared = [tx_flag])]
    fn phase_shift_tx(mut cx: phase_shift_tx::Context) {
        cx.shared.tx_flag.lock(|flag| *flag = true);
    }

    #[task(priority = 3, binds = LPUART1, local = [rx])]
    fn lpuart1(cx: lpuart1::Context) {
        let rx = cx.local.rx;
        // let queue = cx.resources.rx_prod;

        let b = match rx.read() {
            Ok(b) => {
                rprint!(=> 1, "{}", b as char);
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

    extern "Rust" {
        #[task(priority = 2, local = [oled_ssd1306], shared = [local_heartbeat, remote_heartbeat])]
        fn oled_ssd1306_task(_: oled_ssd1306_task::Context);

        #[task(priority = 2, local = [oled_ssd1331], shared = [local_heartbeat, remote_heartbeat])]
        fn oled_ssd1331_task(_: oled_ssd1331_task::Context);
    }

}

use cortex_m_rt::exception;
use rtt_target::rprintln;

#[exception]
unsafe fn HardFault(ef: &cortex_m_rt::ExceptionFrame) -> ! {
    panic!("HF: {:#?}", ef);
}

#[exception]
unsafe fn DefaultHandler(irqn: i16) {
    rprintln!("Unhandled IRQ: {}", irqn);
}