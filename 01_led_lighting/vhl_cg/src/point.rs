use vhl_stdlib::serdes::buf::{Buf, BufMut};
use vhl_stdlib::serdes::traits::{DeserializeBytes, SerializeBytes};
use vhl_stdlib::serdes::buf::Error as BufError;

#[derive(Copy, Clone, Debug)]
pub struct Point {
    pub x: u16,
    pub y: u16
}

impl SerializeBytes for Point {
    type Error = BufError;

    fn ser_bytes(&self, wgr: &mut BufMut) -> Result<(), Self::Error> {
        wgr.put_u16_le(self.x)?;
        wgr.put_u16_le(self.y)
    }

    fn len_bytes(&self) -> usize {
        4
    }
}

impl<'i> DeserializeBytes<'i> for Point {
    type Error = BufError;

    fn des_bytes<'di>(rdr: &'di mut Buf<'i>) -> Result<Self, Self::Error> {
        Ok(Point {
            x: rdr.get_u16_le()?,
            y: rdr.get_u16_le()?
        })
    }
}