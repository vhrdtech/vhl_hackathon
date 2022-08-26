#![no_main]
#![no_std]

use cortex_m_rt::entry;
use stm32h7xx_hal::{pac, prelude::*};
use panic_rtt_target as _;
use rtt_target::{rprintln, rtt_init_print};
use vhl_stdlib::serdes::NibbleBuf;
use vhl_stdlib::serdes::xpi_vlu4::MultiUri;

#[entry]
fn main() -> ! {
    rtt_init_print!();

    let cp = cortex_m::Peripherals::take().unwrap();
    let dp = pac::Peripherals::take().unwrap();

    // Constrain and Freeze power
    let pwr = dp.PWR.constrain();
    let pwrcfg = pwr.freeze();

    // Constrain and Freeze clock
    let rcc = dp.RCC.constrain();
    let ccdr = rcc.sys_ck(100.MHz()).freeze(pwrcfg, &dp.SYSCFG);

    let gpioe = dp.GPIOE.split(ccdr.peripheral.GPIOE);

    // Configure PE1 as output.
    let mut led = gpioe.pe1.into_push_pull_output();

    // Get the delay provider.
    let mut delay = cp.SYST.delay(ccdr.clocks);

    let multi_uri: Result<MultiUri, _> = NibbleBuf::new_all(&[0x22, 0x12, 0x63, 0x25, 0x66, 0x20]).des_vlu4();
    match multi_uri {
        Ok(multi_uri) => {
            let flat_iter = multi_uri.flat_iter();
            for uri in flat_iter {
                // rprintln!("{}", uri);
            }
        }
        Err(_) => {}
    }


    loop {
        loop {
            led.set_high();
            delay.delay_ms(500_u16);

            led.set_low();
            delay.delay_ms(500_u16);

            rprintln!("loop");
        }
    }
}
