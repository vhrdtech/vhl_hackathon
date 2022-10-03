// #![deny(warnings)]
#![no_main]
#![no_std]

// TODO: remove, only for dev to see important warnings like unused results
// #![allow(unused_variables)]
#![allow(unused_imports)]
// #![allow(dead_code)]

mod ethernet;
mod vhlink;
mod xpi_dispatch;
mod oled;
mod vt100;
mod logging;
mod generated_goal;

pub const CORE_FREQ: u32 = 200_000_000;
#[allow(dead_code)]
type Instant = fugit::TimerInstantU64<CORE_FREQ>;
#[allow(dead_code)]
type Duration = fugit::TimerDurationU64<CORE_FREQ>;

#[rtic::app(device = stm32h7xx_hal::stm32, peripherals = true, dispatchers = [SAI1, SAI2, SAI3, SAI4])]
mod app {
    use dwt_systick_monotonic::DwtSystick;
    use stm32h7xx_hal::{gpio, prelude::*};

    use super::*;
    use bbqueue::BBBuffer;
    use rtt_target::rtt_init_print;

    use ethernet::{ethernet_event, smoltcp_poll_at};
    use vhlink::link_process;
    use oled::display_task;

    const T: u8 = 0;

    #[monotonic(binds = SysTick, default = true)]
    type DwtSystMono = DwtSystick<CORE_FREQ>;
    use dwt_systick_monotonic::ExtU64;
    use ssd1306::prelude::*;
    use stm32h7xx_hal::delay::DelayFromCountDownTimer;

    #[shared]
    struct SharedResources {
        poll_at_handle: Option<ethernet::PollAtHandle>,

        /// Example of a vhL property placed in RTIC resources
        /// Maybe possible to generate with a proc_macro!
        /// Even better if possible to add notify_task to it
        symbol: char,
        digit: u8,
    }
    #[local]
    struct LocalResources {
        net: ethernet::Net<'static>,
        eth_in_cons: bbqueue::Consumer<'static, 512>, // eth irq: take & tx
        eth_out_prod: bbqueue::Producer<'static, 512>, // eth irq: rx & put
        lan8742a: ethernet::Lan8742A,

        eth_out_cons: bbqueue::Consumer<'static, 512>, // dispatcher: take
        eth_in_prod: bbqueue::Producer<'static, 512>, // dispatcher: put

        led_link: gpio::gpioe::PE10<gpio::Output<gpio::PushPull>>,
        led_act: gpio::gpioe::PE11<gpio::Output<gpio::PushPull>>,

        display: oled::DisplayTy,
    }

    #[init(local = [
        eth_out_bb: BBBuffer<512> = BBBuffer::new(),
        eth_in_bb: BBBuffer<512> = BBBuffer::new(),
    ])]
    fn init(
        mut ctx: init::Context,
    ) -> (SharedResources, LocalResources, init::Monotonics) {
        rtt_init_print!();
        log_info!(=>T, "ecbridge_fw_hackathon");
        // Initialise power...
        let pwr = ctx.device.PWR.constrain();
        let pwrcfg = pwr.freeze();

        // Link the SRAM3 power state to CPU1
        ctx.device.RCC.ahb2enr.modify(|_, w| w.sram3en().set_bit());

        // Initialise clocks...
        let rcc = ctx.device.RCC.constrain();
        let ccdr = rcc
            .sys_ck(CORE_FREQ.Hz())
            .hclk(CORE_FREQ.Hz())
            .freeze(pwrcfg, &ctx.device.SYSCFG);

        // Initialise system...
        ctx.core.SCB.enable_icache();
        // TODO: ETH DMA coherence issues
        // ctx.core.SCB.enable_dcache(&mut ctx.core.CPUID);

        let mut dcb = ctx.core.DCB;
        let dwt = ctx.core.DWT;
        let systick = ctx.core.SYST;
        let mono = DwtSystick::new(&mut dcb, dwt, systick, CORE_FREQ);

        log_debug!(=>T, "Core init done");

        // Initialise IO...
        let gpioa = ctx.device.GPIOA.split(ccdr.peripheral.GPIOA);
        let gpiob = ctx.device.GPIOB.split(ccdr.peripheral.GPIOB);
        let gpioc = ctx.device.GPIOC.split(ccdr.peripheral.GPIOC);
        let gpiod = ctx.device.GPIOD.split(ccdr.peripheral.GPIOD);
        let _gpiog = ctx.device.GPIOG.split(ccdr.peripheral.GPIOG);
        let gpioe = ctx.device.GPIOE.split(ccdr.peripheral.GPIOE);
        let mut led_link = gpioe.pe10.into_push_pull_output();
        led_link.set_low();
        let mut led_act = gpioe.pe11.into_push_pull_output();
        led_act.set_low();

        let mut rmii_ref_clk_en = gpioc.pc7.into_push_pull_output();
        rmii_ref_clk_en.set_high();

        let rmii_ref_clk = gpioa.pa1.into_alternate();
        let rmii_mdio = gpioa.pa2.into_alternate();
        let rmii_mdc = gpioc.pc1.into_alternate();
        let rmii_crs_dv = gpioa.pa7.into_alternate();
        let rmii_rxd0 = gpioc.pc4.into_alternate();
        let rmii_rxd1 = gpioc.pc5.into_alternate();
        let rmii_tx_en = gpiob.pb11.into_alternate();
        let rmii_txd0 = gpiob.pb12.into_alternate();
        let rmii_txd1 = gpiob.pb13.into_alternate();
        let ethernet_pins = (
            rmii_ref_clk, rmii_mdio, rmii_mdc, rmii_crs_dv, rmii_rxd0,
            rmii_rxd1, rmii_tx_en, rmii_txd0, rmii_txd1,
        );

        assert_eq!(ccdr.clocks.hclk().raw(), 200_000_000); // HCLK 200MHz
        assert_eq!(ccdr.clocks.pclk1().raw(), 100_000_000); // PCLK 100MHz
        assert_eq!(ccdr.clocks.pclk2().raw(), 100_000_000); // PCLK 100MHz
        assert_eq!(ccdr.clocks.pclk4().raw(), 100_000_000); // PCLK 100MHz
        log_debug!(=>T, "Clocks ok");

        let (eth_mac, eth_mtl, eth_dma, eth_prec) = (
            ctx.device.ETHERNET_MAC,
            ctx.device.ETHERNET_MTL,
            ctx.device.ETHERNET_DMA,
            ccdr.peripheral.ETH1MAC
        );
        let (net, lan8742a) = ethernet::init(
            eth_mac, eth_mtl, eth_dma,
            ethernet_pins,
            eth_prec,
            &ccdr.clocks,
        );

        // Delay provider
        let timer2 = ctx.device
            .TIM2
            .timer(1.kHz(), ccdr.peripheral.TIM2, &ccdr.clocks);
        let mut delay = DelayFromCountDownTimer::new(timer2);

        // OLED
        let mut oled_pwr_dis = gpiod.pd15.into_push_pull_output();
        oled_pwr_dis.set_high();
        delay.delay_ms(5_u32); // garbage even with reset, rare
        oled_pwr_dis.set_low();

        let oled_scl = gpiod.pd12.into_alternate().set_open_drain();
        let oled_sda = gpiod.pd13.into_alternate().set_open_drain();
        let mut oled_rst = gpiod.pd14.into_push_pull_output();
        oled_rst.set_high();
        delay.delay_ms(10_u32); // garbage with 1ms

        let i2c_oled = ctx.device.I2C4.i2c(
            (oled_scl, oled_sda), 100.kHz(), ccdr.peripheral.I2C4, &ccdr.clocks);

        let oled_interface = ssd1306::I2CDisplayInterface::new(i2c_oled);
        let mut display = ssd1306::Ssd1306::new(
            oled_interface,
            ssd1306::size::DisplaySize72x40,
            ssd1306::rotation::DisplayRotation::Rotate0
        ).into_buffered_graphics_mode();
        display.init().unwrap();

        // Create queues
        let (eth_out_prod, eth_out_cons) = ctx.local.eth_out_bb.try_split().unwrap();
        let (eth_in_prod, eth_in_cons) = ctx.local.eth_in_bb.try_split().unwrap();

        // Spawn tasks
        rtic::pend(stm32h7xx_hal::pac::Interrupt::ETH); // start listening on sockets, etc
        // blinky::spawn_after(1u64.secs()).unwrap();
        display_task::spawn().unwrap();

        log_debug!(=>T, "All init done");
        (
            SharedResources {
                symbol: '-',
                digit: 0,
                poll_at_handle: None,
            },
            LocalResources {
                net,
                eth_in_cons,
                eth_out_prod,
                lan8742a,

                eth_out_cons,
                eth_in_prod,

                display,
                led_link,
                led_act,

            },
            init::Monotonics(mono),
        )
    }

    #[idle(local = [lan8742a, led_link])]
    fn idle(ctx: idle::Context) -> ! {
        loop {
            // Ethernet
            match ctx.local.lan8742a.poll_link() {
                true => ctx.local.led_link.set_high(),
                _ => ctx.local.led_link.set_low(),
            }
        }
    }

    #[task]
    fn blinky(_ctx: blinky::Context) {
        let time = crate::app::monotonics::now().duration_since_epoch().to_millis();
        log_info!(=>T, "now: {}ms", time);
        blinky::spawn_after(1u64.secs()).unwrap();
    }

    /// Must be spawned on Call to /set_digit
    #[task(shared = [digit])]
    fn set_digit(mut ctx: set_digit::Context, digit: u8) {
        log_info!(=>T, "set_digit task: {}", digit);
        ctx.shared.digit.lock(|d| *d = digit);
        display_task::spawn().unwrap();
    }

    extern "Rust" {
        // Challenge - how to assemble all the resources names automatically?
        #[task(binds = ETH, priority = 2, local = [net, eth_in_cons, eth_out_prod, led_act], shared = [poll_at_handle])]
        fn ethernet_event(_: ethernet_event::Context);

        // Must be ethernet_event + 1, otherwise rescheduling logic will not work correctly
        #[task(priority = 3, shared = [poll_at_handle])]
        fn smoltcp_poll_at(_: smoltcp_poll_at::Context);

        #[task(shared = [symbol], local = [eth_out_cons, eth_in_prod])]
        fn link_process(_: link_process::Context);

        #[task(local = [display], shared = [symbol, digit])]
        fn display_task(_: display_task::Context);
    }
}

use cortex_m_rt::exception;
use dwt_systick_monotonic::fugit;

#[exception]
unsafe fn HardFault(ef: &cortex_m_rt::ExceptionFrame) -> ! {
    panic!("HF: {:#?}", ef);
}

#[exception]
unsafe fn DefaultHandler(irqn: i16) {
    for i in 0..8 {
        log_error!(=>i, "Unhandled IRQ: {}", irqn);
    }
}

#[inline(never)]
#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    use cortex_m::interrupt;
    // use rtt_target::{UpChannel, ChannelMode};
    use core::sync::atomic::compiler_fence;
    use core::sync::atomic::Ordering::SeqCst;

    interrupt::disable();

    // if let Some(mut channel) = unsafe { UpChannel::conjure(0) } {
    //     channel.set_mode(ChannelMode::BlockIfFull);
    //
    //     writeln!(channel, "{}", info).ok();
    // }

    for i in 0..8 {
        log_error!(=>i, "{}", info);
    }

    loop {
        compiler_fence(SeqCst);
    }
}

// /// Must be called directly from dispatcher on Call to /sync
// fn sync(a: u8, b: u8) -> u8 {
//     log_trace!(=>3, "sync_fn({}, {}) called", a, b);
//
//     a + b
// }
//
// fn sync_2(a: u8, b: u8) -> u8 {
//     log_trace!(=>3, "sync_fn_2({}, {}) called", a, b);
//
//     a - b
// }

use vhl_cg::point::Point;
fn sync(p1: Point, p2: Point) -> Point {
    Point {
        x: p1.x + p2.x,
        y: p1.y + p2.y
    }
}