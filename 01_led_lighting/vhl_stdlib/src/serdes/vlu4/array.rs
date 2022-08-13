use core::iter::FusedIterator;
use crate::serdes::{NibbleBuf, NibbleBufMut};

/// Variable length array of u32 numbers based on vlu4 encoding without allocations.
#[derive(Copy, Clone, Debug)]
pub struct Vlu4U32Array<'i> {
    rdr: NibbleBuf<'i>,
    // number of vlu4 encoded numbers inside
    len: usize,
}

impl<'i> Vlu4U32Array<'i> {
    pub fn new(mut rdr: NibbleBuf<'i>) -> Self {
        let len = rdr.get_vlu4_u32() as usize;
        Vlu4U32Array { rdr, len }
    }

    pub fn iter(&self) -> Vlu4U32ArrayIter<'i> {
        Vlu4U32ArrayIter {
            array: self.clone(), pos: 0
        }
    }

    pub fn len(&self) -> usize {
        self.len
    }

    /// Skip all elements of this array without reading them and return the rest of the input buffer
    pub fn lookahead(&self) -> NibbleBuf<'i> {
        let mut rdr = self.rdr.clone();
        for _ in 0..self.len {
            rdr = NibbleBuf::lookahead_vlu4_u32(rdr);
        }
        rdr
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
    pos: usize,
}

impl<'i> Iterator for Vlu4U32ArrayIter<'i> {
    type Item = u32;

    fn next(&mut self) -> Option<Self::Item> {
        if self.pos >= self.array.len {
            None
        } else {
            self.pos += 1;
            Some(self.array.rdr.get_vlu4_u32())
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.array.len, Some(self.array.len))
    }
}

impl<'i> FusedIterator for Vlu4U32ArrayIter<'i> {}

#[cfg(test)]
mod test {
    use crate::serdes::NibbleBuf;
    use super::Vlu4U32Array;

    #[test]
    fn vlu4_u32_array_iter() {
        let buf = [0x51, 0x23, 0x45];
        let arr = Vlu4U32Array::new(NibbleBuf::new(&buf));
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
