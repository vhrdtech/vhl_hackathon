use core::iter::FusedIterator;
use crate::serdes::{NibbleBuf, NibbleBufMut};

pub trait SerializeVlu4 {
    fn ser_vlu4(&self, wgr: &mut NibbleBufMut);
}

pub trait DeserializeVlu4 {
    fn des_vlu4(rdr: &mut NibbleBuf) -> Self;
}

/// Variable length array of u32 numbers based on vlu4 encoding without allocations.
#[derive(Copy, Clone, Debug)]
pub struct Vlu4U32Array<'i> {
    buf: &'i [u8],
    // number of vlu4 encoded numbers inside
    len: usize,
    // number of nibbles taken by len == data start position
    offset: usize,
}

impl<'i> Vlu4U32Array<'i> {
    pub fn new(buf: &'i [u8]) -> Option<Self> {
        let mut rgr = NibbleBuf::new(buf);
        let len = rgr.get_vlu4_u32();
        if rgr.is_past_end() {
            None
        } else {
            Some(Vlu4U32Array {
                buf, len: len as usize, offset: rgr.nibbles_pos()
            })
        }
    }

    pub fn iter(&self) -> Vlu4U32ArrayIter<'i> {
        let rgr = NibbleBuf::new_with_offset(self.buf, self.offset);
        Vlu4U32ArrayIter {
            array: self.clone(),
            rgr
        }
    }

    pub fn len(&self) -> usize {
        self.len
    }
}

impl<'i> IntoIterator for Vlu4U32Array<'i> {
    type Item = u32;
    type IntoIter = Vlu4U32ArrayIter<'i>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

pub struct Vlu4U32ArrayIter<'i> {
    array: Vlu4U32Array<'i>,
    rgr: NibbleBuf<'i>,
}

impl<'i> Iterator for Vlu4U32ArrayIter<'i> {
    type Item = u32;

    fn next(&mut self) -> Option<Self::Item> {
        if self.rgr.is_at_end() {
            None
        } else {
            Some(self.rgr.get_vlu4_u32())
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.array.len, Some(self.array.len))
    }
}

impl<'i> FusedIterator for Vlu4U32ArrayIter<'i> {}

#[cfg(test)]
mod test {
    use super::Vlu4U32Array;

    #[test]
    fn vlu4_u32_array_iter() {
        let buf = [0x51, 0x23, 0x45];
        let arr = Vlu4U32Array::new(&buf).unwrap();
        assert_eq!(arr.len(), 5);
        let mut iter = arr.into_iter();
        assert_eq!(iter.next(), Some(1));
        assert_eq!(iter.next(), Some(2));
        assert_eq!(iter.next(), Some(3));
        assert_eq!(iter.next(), Some(4));
        assert_eq!(iter.next(), Some(5));
        assert_eq!(iter.next(), None);
    }
}
