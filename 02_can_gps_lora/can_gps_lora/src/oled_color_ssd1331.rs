use embedded_graphics::Drawable;
use embedded_graphics::geometry::Point;
use embedded_graphics::mono_font::ascii::FONT_6X13_BOLD;
use embedded_graphics::mono_font::MonoTextStyle;
use embedded_graphics::pixelcolor::Rgb565;
use embedded_graphics::text::Text;
use ssd1331::Ssd1331;
use stm32l4xx_hal::gpio::{Alternate, Output, PushPull};
use stm32l4xx_hal::pac::SPI3;
use stm32l4xx_hal::spi::Spi;
use crate::util::StrWriter;
use embedded_graphics::prelude::WebColors;
use core::fmt::Write;
use crate::radio::HeartBeat;
use rtic::Mutex;

pub type Sck = stm32l4xx_hal::gpio::Pin<Alternate<PushPull, 6_u8>, stm32l4xx_hal::gpio::H8, 'C', 10_u8>;
pub type Miso = stm32l4xx_hal::gpio::Pin<Alternate<PushPull, 6_u8>, stm32l4xx_hal::gpio::H8, 'C', 11_u8>;
pub type Mosi = stm32l4xx_hal::gpio::Pin<Alternate<PushPull, 6_u8>, stm32l4xx_hal::gpio::H8, 'C', 12_u8>;
pub type Dc = stm32l4xx_hal::gpio::Pin<Output<PushPull>, stm32l4xx_hal::gpio::H8, 'C', 9_u8>;
pub type DisplaySpi = Spi<SPI3, (Sck, Miso, Mosi)>;
pub type Display = Ssd1331<DisplaySpi, Dc>;


#[cfg(feature = "oled_color_ssd1331")]
pub fn oled_ssd1331_task(mut cx: crate::app::oled_ssd1331_task::Context) {
    let display: &mut Display = cx.local.oled_ssd1331;
    let local_hb: HeartBeat = cx.shared.local_heartbeat.lock(|hb| hb.clone());
    let remote_hb: HeartBeat = cx.shared.remote_heartbeat.lock(|hb| hb.clone());

    let mut str_buf = [0u8; 128];
    let mut str_buf = StrWriter::new(&mut str_buf);
    let style = MonoTextStyle::new(&FONT_6X13_BOLD, Rgb565::CSS_DEEP_SKY_BLUE);

    str_buf.clear();
    write!(str_buf, "Sig {} {}\nCnt {} {}", local_hb.remote_rssi, remote_hb.remote_rssi, local_hb.uptime, remote_hb.uptime);
    display.clear();
    Text::new(str_buf.as_str(), Point::new(5, 10), style).draw(display).unwrap();
    display.flush().unwrap();
}

#[cfg(not(feature = "oled_color_ssd1331"))]
pub fn oled_ssd1331_task(_: crate::app::oled_ssd1331_task::Context) { }