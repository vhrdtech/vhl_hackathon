#![no_main]
#![no_std]

extern crate panic_rtt_target;

use cortex_m::asm::delay;
use embedded_graphics::Drawable;
use embedded_graphics::geometry::Point;
use embedded_graphics::image::{Image, ImageRaw};
use embedded_graphics::pixelcolor::BinaryColor;
use heapless::{consts::U8, spsc};
use nb::block;
use rtt_target::{rprint, rprintln};
use ssd1306::{I2CDisplayInterface, Ssd1306};
use ssd1306::mode::DisplayConfig;
use ssd1306::prelude::{DisplayRotation, DisplaySize128x64};
use stm32l4xx_hal::{i2c, pac::{self, LPUART1}, prelude::*, serial::{self, Config, Serial}};
use stm32l4xx_hal::delay::Delay;
use stm32l4xx_hal::gpio::{Floating, H8, Input, L8, Output, Pin, PushPull};
use stm32l4xx_hal::i2c::I2c;

#[rtic::app(device = stm32l4xx_hal::pac)]
const APP: () = {
    struct Resources {
        rx: serial::Rx<LPUART1>,
        tx: serial::Tx<LPUART1>,

        pps_input: Pin<Input<Floating>, H8, 'B', 14>,

        led1: Pin<Output<PushPull>, L8, 'B', 2>,
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

        let clocks = rcc.cfgr.sysclk(64.MHz()).freeze(&mut flash.acr, &mut pwr);

        let mut gpioa = p.GPIOA.split(&mut rcc.ahb2);
        let mut gpiob = p.GPIOB.split(&mut rcc.ahb2);
        let mut gpioc = p.GPIOC.split(&mut rcc.ahb2);

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

        let led1 = gpiob.pb2.into_push_pull_output(&mut gpiob.moder, &mut gpiob.otyper);
        let led2 = gpioa.pa10.into_push_pull_output(&mut gpioa.moder, &mut gpioa.otyper);


        let mut serial = Serial::lpuart1(
            p.LPUART1,
            (tx_pin, rx_pin),
            Config::default().baudrate(115200.bps()),
            clocks,
            &mut rcc.apb1r2,
        );
        serial.listen(serial::Event::Rxne);

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

        let (tx, rx) = serial.split();
        let (rx_prod, rx_cons) = RX_QUEUE.split();

        // let mut delay = Delay::new(cp.SYST, clocks);
        // delay.delay_ms(1000u32);
        delay(80_000_000);
        on_off.set_high();
        delay(1_000_000);
        on_off.set_low();

        rprintln!("done.");

        init::LateResources {
            rx,
            tx,

            pps_input,
            led1,
            //
            // rx_prod,
            // rx_cons,
        }
    }

    #[idle(resources = [tx, pps_input, led1, ])]
    fn idle(cx: idle::Context) -> ! {
        // let rx = cx.resources.rx_cons;
        let tx = cx.resources.tx;
        let pps_input: &mut Pin<Input<Floating>, H8, 'B', 14> = cx.resources.pps_input;
        let led1: &mut Pin<Output<PushPull>, L8, 'B', 2> = cx.resources.led1;

        loop {
            // if let Some(b) = rx.dequeue() {
            //     rprintln!("Echoing '{}'", b as char);
            //     block!(tx.write(b)).unwrap();
            // }
            // block!(tx.write('x' as u8)).unwrap();
            cortex_m::asm::delay(1_000_000);

            if pps_input.is_high() {
                led1.set_high();
            } else {
                led1.set_low();
            }
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
