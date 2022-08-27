use embedded_graphics::image::{Image, ImageRaw};
use embedded_graphics::mono_font::ascii::{FONT_9X18_BOLD};
use embedded_graphics::mono_font::MonoTextStyle;
use embedded_graphics::pixelcolor::{BinaryColor};
use embedded_graphics::prelude::*;
use embedded_graphics::primitives::{Circle, PrimitiveStyleBuilder};
use embedded_graphics::text::Text;
use ssd1306::mode::BufferedGraphicsMode;
use ssd1306::prelude::I2CInterface;
use ssd1306::Ssd1306;
use stm32h7xx_hal::i2c::I2c;
use stm32h7xx_hal::pac::I2C4;
use ssd1306::prelude::*;
use rtic::Mutex;

pub type DisplayTy = Ssd1306<I2CInterface<I2c<I2C4>>, DisplaySize72x40, BufferedGraphicsMode<DisplaySize72x40>>;

pub fn display_task(mut cx: crate::app::display_task::Context) {
    let display = cx.local.display;
    display.clear();

    let raw: ImageRaw<BinaryColor> = ImageRaw::new(include_bytes!("../vhrd_tech_72_40.raw"), 72);
    let im = Image::new(&raw, Point::new(0, 0));
    im.draw(display).unwrap();

    if false {
        let size = 10;
        let offset = Point::new(40, (42 / 2) - (size / 2) - 1);
        let _spacing = size + 10;

        let style = PrimitiveStyleBuilder::new()
            .stroke_width(3)
            .stroke_color(BinaryColor::On)
            .build();

        Circle::new(Point::zero(), size as u32)
            .translate(offset)
            .into_styled(style)
            .draw(display)
            .unwrap();

        let style = MonoTextStyle::new(&FONT_9X18_BOLD, BinaryColor::On);
        let symbol: char = cx.shared.symbol.lock(|s| *s);
        let mut str_buf = [0u8; 32];
        let symbol_str = symbol.encode_utf8(&mut str_buf);
        Text::new(symbol_str, Point::new(5, 10), style).draw(display).unwrap();

        let digit: u8 = cx.shared.digit.lock(|d| *d);
        str_buf[0] = digit;

        let symbol_str = ((digit + '0' as u8) as char).encode_utf8(&mut str_buf);
        Text::new(symbol_str, Point::new(5, 30), style).draw(display).unwrap();
    }

    display.flush().unwrap();
}