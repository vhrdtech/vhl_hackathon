// #![deny(warnings)]
#![no_main]
#![no_std]

mod ethernet;
mod vhlink;
mod xpi_dispatch;

use panic_rtt_target as _;
use core::sync::atomic::{AtomicU32};
use rtt_target::rprintln;
use stm32h7xx_hal::{rcc::CoreClocks, stm32};

/// Configure SYSTICK for 1ms timebase
fn systick_init(mut syst: stm32::SYST, clocks: CoreClocks) {
    let c_ck_mhz = clocks.c_ck().to_MHz();

    let syst_calib = 0x3E8;

    syst.set_clock_source(cortex_m::peripheral::syst::SystClkSource::Core);
    syst.set_reload((syst_calib * c_ck_mhz) - 1);
    syst.enable_interrupt();
    syst.enable_counter();
}

/// TIME is an atomic u32 that counts milliseconds.
static TIME: AtomicU32 = AtomicU32::new(0);

#[rtic::app(device = stm32h7xx_hal::stm32, peripherals = true, dispatchers = [SAI1, SAI2, SAI3, SAI4])]
mod app {
    use stm32h7xx_hal::{gpio, prelude::*};

    use super::*;
    use core::sync::atomic::Ordering;
    use rtt_target::rtt_init_print;

    use ethernet::ethernet_event;

    #[shared]
    struct SharedResources {
        /// Example of a vhL property placed in RTIC resources
        /// Maybe possible to generate with a proc_macro!
        /// Even better if possible to add notify_task to it
        symbol: char,
    }
    #[local]
    struct LocalResources {
        net: ethernet::Net<'static>,
        lan8742a: ethernet::Lan8742A,

        led_link: gpio::gpioe::PE10<gpio::Output<gpio::PushPull>>,
        led_act: gpio::gpioe::PE11<gpio::Output<gpio::PushPull>>,


    }

    #[init]
    fn init(
        mut ctx: init::Context,
    ) -> (SharedResources, LocalResources, init::Monotonics) {
        rtt_init_print!();
        // Initialise power...
        let pwr = ctx.device.PWR.constrain();
        let pwrcfg = pwr.freeze();

        // Link the SRAM3 power state to CPU1
        ctx.device.RCC.ahb2enr.modify(|_, w| w.sram3en().set_bit());

        // Initialise clocks...
        let rcc = ctx.device.RCC.constrain();
        let ccdr = rcc
            .sys_ck(200.MHz())
            .hclk(200.MHz())
            .freeze(pwrcfg, &ctx.device.SYSCFG);

        // Initialise system...
        ctx.core.SCB.enable_icache();
        // TODO: ETH DMA coherence issues
        // ctx.core.SCB.enable_dcache(&mut ctx.core.CPUID);
        ctx.core.DWT.enable_cycle_counter();
        rprintln!("Core init done");

        // Initialise IO...
        let gpioa = ctx.device.GPIOA.split(ccdr.peripheral.GPIOA);
        let gpiob = ctx.device.GPIOB.split(ccdr.peripheral.GPIOB);
        let gpioc = ctx.device.GPIOC.split(ccdr.peripheral.GPIOC);
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
        rprintln!("Clocks ok");

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

        // 1ms tick
        systick_init(ctx.core.SYST, ccdr.clocks);

        // OLED
        // let mut oled_pwr_dis = gpiod.pd15.into_push_pull_output();
        // oled_pwr_dis.set_low();
        //
        // let oled_scl = gpiod.pd12.into_alternate().set_open_drain();
        // let oled_sda = gpiod.pd13.into_alternate().set_open_drain();
        // let mut oled_rst = gpiod.pd14.into_push_pull_output();
        // oled_rst.set_high();
        // delay.delay_ms(10_u16); // garbage with 1ms
        //
        // let mut i2c_oled = dp.I2C4.i2c(
        //     (oled_scl, oled_sda), 100.kHz(), ccdr.peripheral.I2C4, &ccdr.clocks);
        //
        // let oled_interface = I2CDisplayInterface::new(i2c_oled);
        // let mut display = Ssd1306::new(oled_interface, DisplaySize72x40, DisplayRotation::Rotate0)
        //     .into_buffered_graphics_mode();
        // display.init().unwrap();

        rprintln!("All init done");
        (
            SharedResources {
                symbol: '-'
            },
            LocalResources {
                net,
                lan8742a,

                led_link,
                led_act,

            },
            init::Monotonics(),
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

    /// Must be spawned on Call to /set_digit
    #[task]
    fn set_digit(_ctx: set_digit::Context, digit: u8) {
        rprintln!(=>3, "set_digit task: {}", digit);
    }

    extern "Rust" {
        // Challenge - how to assemble all the resources names automatically?
        #[task(binds = ETH, shared = [symbol], local = [net, led_act])]
        fn ethernet_event(_: ethernet_event::Context);
    }

    #[task(binds = SysTick, priority=15)]
    fn systick_tick(_: systick_tick::Context) {
        TIME.fetch_add(1, Ordering::Relaxed);
    }
}

/// Must be called directly from dispatcher on Call to /sync
fn sync_fn() {
    rprintln!(=>3, "sync_fn() called");
}