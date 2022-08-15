use std::iter::FusedIterator;
use crate::serdes::NibbleBuf;
use crate::serdes::vlu4::DeserializeVlu4;

/// Variable size array of u8 slices, aligned to byte boundary.
///
/// 4 bit padding is inserted and skipped if needed before the slices data start.
#[derive(Copy, Clone, Debug)]
pub struct Vlu4SliceArray<'i> {
    rdr: NibbleBuf<'i>,
    // number of [u8] slices serialized
    len: usize,
}

impl<'i> Vlu4SliceArray<'i> {
    pub fn iter(&self) -> Vlu4SliceArrayIter {
        Vlu4SliceArrayIter {
            array: self.clone(), pos: 0
        }
    }

    pub fn len(&self) -> usize {
        self.len
    }
}

// impl<'i> IntoIterator for Vlu4SliceArray<'i> {
//     type Item = &'i [u8];
//     type IntoIter = Vlu4SliceArrayIter<'i>;
//
//     fn into_iter(self) -> Self::IntoIter {
//         self.iter()
//     }
// }

pub struct Vlu4SliceArrayIter<'i> {
    array: Vlu4SliceArray<'i>,
    pos: usize,
}

impl<'i> Iterator for Vlu4SliceArrayIter<'i> {
    type Item = &'i [u8];

    fn next(&mut self) -> Option<&'i [u8]> {
        if self.pos >= self.array.len {
            None
        } else {
            self.pos += 1;
            let slice_len = self.array.rdr
                .get_vlu4_u32()
                .or_else(|e| {
                    self.pos = self.array.len; // stop reading corrupt data
                    Err(e)
                }).ok()?;
            if !self.array.rdr.is_at_byte_boundary() {
                let _padding = self.array.rdr.get_nibble().ok()?;
            }
            Some(self.array.rdr.get_slice(slice_len as usize).ok()?)
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.array.len, Some(self.array.len))
    }
}

impl<'i> FusedIterator for Vlu4SliceArrayIter<'i> {}

impl<'i> DeserializeVlu4<'i> for Vlu4SliceArray<'i> {
    type Error = crate::serdes::nibble_buf::Error;

    fn des_vlu4<'di>(rdr: &'di mut NibbleBuf<'i>) -> Result<Self, Self::Error> {
        let len = rdr.get_vlu4_u32()? as usize;
        let rdr_before_elements = rdr.clone();
        for _ in 0..len {
            let slice_len = rdr.get_vlu4_u32()? as usize;
            if !rdr.is_at_byte_boundary() {
                let _padding = rdr.get_nibble()?;
            }
            let _slice = rdr.get_slice(slice_len);
        }
        Ok(Vlu4SliceArray {
            rdr: rdr_before_elements,
            len
        })
    }
}

#[cfg(test)]
mod test {
    use hex_literal::hex;
    use crate::serdes::NibbleBuf;
    use crate::serdes::vlu4::Vlu4SliceArray;

    #[test]
    fn aligned_start() {
        let input_buf = hex!("32 ab cd 30 ef ed cb 20 ab cd /* slices end */ 11 22");
        let mut buf = NibbleBuf::new_all(&input_buf);

        let slices: Vlu4SliceArray = buf.des_vlu4().unwrap();
        let mut iter = slices.iter();
        assert_eq!(iter.next(), Some(&input_buf[1..=2]));
        assert_eq!(iter.next(), Some(&input_buf[4..=6]));
        assert_eq!(iter.next(), Some(&input_buf[8..=9]));
        assert_eq!(iter.next(), None);

        assert_eq!(buf.get_u8(), Ok(0x11));
    }

    #[test]
    fn unaligned_start() {
        let input_buf = hex!("12 20 ab cd 20 ef fe 11");
        let mut buf = NibbleBuf::new_all(&input_buf);

        assert_eq!(buf.get_nibble(), Ok(1));

        let slices: Vlu4SliceArray = buf.des_vlu4().unwrap();
        let mut iter = slices.iter();
        assert_eq!(iter.next(), Some(&input_buf[2..=3]));
        assert_eq!(iter.next(), Some(&input_buf[5..=6]));
        assert_eq!(iter.next(), None);

        assert_eq!(buf.get_u8(), Ok(0x11));
    }
}