use embedded_graphics::mono_font::ascii::FONT_9X18_BOLD;
use embedded_graphics::mono_font::MonoTextStyle;
use embedded_graphics::pixelcolor::BinaryColor;
use embedded_graphics::prelude::*;
use embedded_graphics::text::Text;
use ssd1306::mode::BufferedGraphicsMode;
use ssd1306::prelude::*;
use ssd1306::Ssd1306;
use stm32l4xx_hal::gpio::{Alternate, OpenDrain};
use stm32l4xx_hal::stm32::I2C1;
use stm32l4xx_hal::i2c::I2c;
use crate::util::StrWriter;
use core::fmt::Write;
use crate::radio::HeartBeat;
use rtic::Mutex;

pub type SclPin = stm32l4xx_hal::gpio::Pin<Alternate<OpenDrain, 4_u8>, stm32l4xx_hal::gpio::L8, 'B', 6_u8>;
pub type SdaPin = stm32l4xx_hal::gpio::Pin<Alternate<OpenDrain, 4_u8>, stm32l4xx_hal::gpio::L8, 'B', 7_u8>;
pub type DisplayI2c = I2c<I2C1, (SclPin, SdaPin)>;
pub type Display = Ssd1306<I2CInterface<DisplayI2c>, DisplaySize128x64, BufferedGraphicsMode<DisplaySize128x64>>;

#[cfg(feature = "oled_bw_ssd1306")]
pub fn oled_ssd1306_task(mut cx: crate::app::oled_ssd1306_task::Context) {
    let display: &mut Display = cx.local.oled_ssd1306;
    let local_hb: HeartBeat = cx.shared.local_heartbeat.lock(|hb| hb.clone());
    let remote_hb: HeartBeat = cx.shared.remote_heartbeat.lock(|hb| hb.clone());

    let mut str_buf = [0u8; 128];
    let mut str_buf = StrWriter::new(&mut str_buf);
    let style = MonoTextStyle::new(&FONT_9X18_BOLD, BinaryColor::On);

    str_buf.clear();
    write!(str_buf, "Sig {} {}\nCnt {} {}", local_hb.remote_rssi, remote_hb.remote_rssi, local_hb.uptime, remote_hb.uptime);
    display.clear();
    Text::new(str_buf.as_str(), Point::new(5, 10), style).draw(display).unwrap();
    display.flush().unwrap();
}

#[cfg(not(feature = "oled_bw_ssd1306"))]
pub fn oled_ssd1306_task(_: crate::app::oled_ssd1306_task::Context) {}