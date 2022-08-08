use embedded_graphics::pixelcolor::BinaryColor;
use embedded_graphics::prelude::*;
use embedded_graphics::primitives::{Circle, PrimitiveStyleBuilder};
use ssd1306::mode::BufferedGraphicsMode;
use ssd1306::prelude::I2CInterface;
use ssd1306::Ssd1306;
use stm32h7xx_hal::i2c::I2c;
use stm32h7xx_hal::pac::I2C4;
use ssd1306::prelude::*;

pub type DisplayTy = Ssd1306<I2CInterface<I2c<I2C4>>, DisplaySize72x40, BufferedGraphicsMode<DisplaySize72x40>>;

pub fn display_task(cx: crate::app::display_task::Context) {
    let size = 10;
    let offset = Point::new(10, (42 / 2) - (size / 2) - 1);
    let _spacing = size + 10;

    let style = PrimitiveStyleBuilder::new()
        .stroke_width(1)
        .stroke_color(BinaryColor::On)
        .build();

    Circle::new(Point::zero(), size as u32)
        .translate(offset)
        .into_styled(style)
        .draw(cx.local.display)
        .unwrap();

    cx.local.display.flush().unwrap();
}