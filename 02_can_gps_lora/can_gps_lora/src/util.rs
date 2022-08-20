use core::fmt;
use cortex_m::asm::delay;
use stm32l4xx_hal::hal::blocking::delay::DelayMs;

pub struct FakeDelay {}
impl DelayMs<u8> for FakeDelay {
    fn delay_ms(&mut self, ms: u8) {
        delay(ms as u32 * (crate::app::SYSCLK / 1_000))
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

    pub fn clear(&mut self) {
        self.pos = 0;
    }
}

impl<'i> fmt::Write for StrWriter<'i> {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        let s = s.as_bytes();
        if self.buf.len() - self.pos < s.len() {
            return Err(fmt::Error{})
        }
        self.buf[self.pos .. self.pos + s.len()].copy_from_slice(s);
        self.pos += s.len();
        Ok(())
    }
}