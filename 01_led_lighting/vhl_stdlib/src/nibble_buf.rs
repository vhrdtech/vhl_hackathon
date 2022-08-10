// pub trait MessageSpec {
//     const ID: u8;
//     const SIZE: usize;
// }
//
// pub trait Serialize {
//     type Error;
//
//     fn ser(&self, buf: &mut BufMut) -> Result<(), Self::Error>;
//     fn size_hint(&self) -> usize;
// }
//
// pub trait Deserialize {
//     type Output;
//     type Error;
//
//     fn des(buf: &mut Buf) -> Result<Self::Output, Self::Error>;
// }

/// Buffer reader that treats input as a stream of nibbles
pub struct NibbleBuf<'a> {
    buf: &'a [u8],
    // Position in bytes
    idx: usize,
    is_at_byte_boundary: bool,
}

impl<'a> NibbleBuf<'a> {
    pub fn new(buf: &'a [u8]) -> Self {
        NibbleBuf {
            buf, idx: 0, is_at_byte_boundary: true
        }
    }

    pub fn nibbles_pos(&self) -> usize {
        if self.is_at_byte_boundary {
            self.idx * 2
        } else {
            self.idx * 2 + 1
        }
    }

    pub fn is_at_end(&self) -> bool {
        self.idx >= self.buf.len()
    }

    // pub fn slice_to(&self, len: usize) -> &'a [u8] {
    //     unsafe { core::slice::from_raw_parts(self.buf.get_unchecked(self.idx), len) }
    // }

    // pub fn advance(&mut self, cnt: usize) {
    //     self.idx += cnt;
    // }

    pub fn is_at_byte_boundary(&self) -> bool {
        self.is_at_byte_boundary
    }

    pub fn get_nibble(&mut self) -> u8 {
        if self.is_at_end() {
            return 0;
        }
        if self.is_at_byte_boundary {
            let val = unsafe { *self.buf.get_unchecked(self.idx) };
            self.is_at_byte_boundary = false;
            (val & 0xf0) >> 4
        } else {
            let val = unsafe { *self.buf.get_unchecked(self.idx) };
            self.is_at_byte_boundary = true;
            self.idx += 1;
            val & 0xf
        }
    }

    pub fn get_vlu4_u32(&mut self) -> u32 {
        let mut num = 0;
        for i in 0..=10 {
            let nib = self.get_nibble();
            if i == 10 {
                // maximum 32 bits in 11 nibbles, 11th nibble should be the last
                if nib & 0b1000 != 0 {
                    // fuse at end to not read corrupt data
                    self.idx = self.buf.len();
                    return 0;
                }
            }
            num = num | (nib as u32 & 0b111);
            if nib & 0b1000 == 0 {
                break;
            }
            num = num << 3;
        }
        num
    }

    pub fn get_u8(&mut self) -> u8 {
        if self.is_at_end() {
            return 0;
        }
        if self.is_at_byte_boundary {
            let val = unsafe { *self.buf.get_unchecked(self.idx) };
            self.idx += 1;
            val
        } else {
            let msn = unsafe { *self.buf.get_unchecked(self.idx) };
            self.idx += 1;
            let lsn = unsafe { *self.buf.get_unchecked(self.idx) };
            (msn << 4) | (lsn >> 4)
        }
    }
}

pub struct NibbleBufMut<'a> {
    buf: &'a mut [u8],
    idx: usize,
    is_at_byte_boundary: bool,
}

impl<'a> NibbleBufMut<'a> {
    pub fn new(buf: &'a mut [u8]) -> Self {
        NibbleBufMut {
            buf, idx: 0, is_at_byte_boundary: true
        }
    }

    pub fn nibbles_pos(&self) -> usize {
        if self.is_at_byte_boundary {
            self.idx * 2
        } else {
            self.idx * 2 + 1
        }
    }

    pub fn is_at_end(&self) -> bool {
        self.idx >= self.buf.len()
    }

    pub fn finish(self) -> &'a [u8] {
        &self.buf[0..self.idx]
    }

    pub fn put_nibble(&mut self, nib: u8) {
        if self.is_at_end() {
            return;
        }
        if self.is_at_byte_boundary {
            unsafe { *self.buf.get_unchecked_mut(self.idx) = nib << 4; }
            self.is_at_byte_boundary = false;
        } else {
            unsafe { *self.buf.get_unchecked_mut(self.idx) |= nib & 0xf; }
            self.is_at_byte_boundary = true;
            self.idx += 1;
        }
    }

    pub fn put_vlu4_u32(&mut self, val: u32) {
        if self.is_at_end() {
            return;
        }
        let mut val = val;
        let mut msb_found = false;
        let nib = (val >> 30) as u8; // get bits 31:30
        if nib != 0 {
            // println!("put 31 30");
            self.put_nibble(nib | 0b1000);
            msb_found = true;
        }
        val <<= 2;
        for i in 0..=9 {
            if (val & (7 << 29) != 0) || msb_found {
                let nib = (val >> 29) as u8;
                if i == 9 {
                    // println!("put last");
                    self.put_nibble(nib);
                } else {
                    // println!("put mid");
                    self.put_nibble(nib | 0b1000);
                }
                msb_found = true;
            }
            if i == 9 && !msb_found {
                // println!("put 0");
                self.put_nibble(0);
            }
            val <<= 3;
        }
    }

    pub fn put_u8(&mut self, val: u8) {
        if self.is_at_end() {
            return;
        }
        if self.is_at_byte_boundary {
            unsafe { *self.buf.get_unchecked_mut(self.idx) = val; }
            self.idx += 1;
        } else {
            unsafe { *self.buf.get_unchecked_mut(self.idx) |= val >> 4; }
            self.idx += 1;
            unsafe { *self.buf.get_unchecked_mut(self.idx) = val << 4; }
        }
    }
}

#[cfg(test)]
mod test {
    extern crate std;
    use crate::nibble_buf::NibbleBufMut;
    use super::NibbleBuf;

    #[test]
    fn read_nibbles() {
        let buf = [0xab, 0xcd, 0xef];
        let mut rdr = NibbleBuf::new(&buf);
        assert_eq!(rdr.get_nibble(), 0xa);
        assert_eq!(rdr.get_nibble(), 0xb);
        assert_eq!(rdr.get_nibble(), 0xc);
        assert_eq!(rdr.get_nibble(), 0xd);
        assert_eq!(rdr.get_nibble(), 0xe);
        assert_eq!(rdr.get_nibble(), 0xf);
        assert!(rdr.is_at_end());
    }

    #[test]
    fn read_u8() {
        let buf = [0x12, 0x34, 0x56];
        let mut rdr = NibbleBuf::new(&buf);
        assert_eq!(rdr.get_nibble(), 0x1);
        assert_eq!(rdr.get_u8(), 0x23);
        assert_eq!(rdr.get_nibble(), 0x4);
        assert_eq!(rdr.get_u8(), 0x56);
        assert!(rdr.is_at_end());
    }

    #[test]
    fn read_past_end() {
        let buf = [0xaa, 0xbb, 0xcc];
        let mut rdr = NibbleBuf::new(&buf[0..=1]);
        rdr.get_u8();
        rdr.get_u8();
        assert!(rdr.is_at_end());
        assert_eq!(rdr.get_u8(), 0);
    }

    #[test]
    fn read_vlu4_u32_single_nibble() {
        let buf = [0b0111_0010, 0b0000_0001];
        let mut rdr = NibbleBuf::new(&buf);
        assert_eq!(rdr.get_vlu4_u32(), Some(7));
        assert_eq!(rdr.get_vlu4_u32(), Some(2));
        assert_eq!(rdr.get_vlu4_u32(), Some(0));
        assert_eq!(rdr.get_vlu4_u32(), Some(1));
        assert!(rdr.is_at_end());
    }

    #[test]
    fn read_vlu4_u32_multi_nibble() {
        let buf = [0b1111_0111, 0b1001_0000, 0b1000_0111];
        let mut rdr = NibbleBuf::new(&buf);
        assert_eq!(rdr.get_vlu4_u32(), Some(63));
        assert_eq!(rdr.nibbles_pos(), 2);
        assert_eq!(rdr.get_vlu4_u32(), Some(0b001000));
        assert_eq!(rdr.nibbles_pos(), 4);
        assert_eq!(rdr.get_vlu4_u32(), Some(0b111));
        assert!(rdr.is_at_end());
    }

    #[test]
    fn read_vlu4_u32_max() {
        let buf = [0b1011_1111, 0xff, 0xff, 0xff, 0xff, 0x70];
        let mut rdr = NibbleBuf::new(&buf);
        assert_eq!(rdr.get_vlu4_u32(), Some(u32::MAX));
        assert_eq!(rdr.get_nibble(), 0);
        assert!(rdr.is_at_end());
    }

    #[test]
    fn read_vlu4_u32_max_plus1() {
        // ignore bit 33
        let buf = [0b1111_1111, 0xff, 0xff, 0xff, 0xff, 0x70];
        let mut rdr = NibbleBuf::new(&buf);
        assert_eq!(rdr.get_vlu4_u32(), Some(u32::MAX));
        assert_eq!(rdr.get_nibble(), 0);
        assert!(rdr.is_at_end());
    }

    #[test]
    fn read_vlu4_u32_max_plus_nibble() {
        // ignore bit 33
        let buf = [0xff, 0xff, 0xff, 0xff, 0xff, 0xf0];
        let mut rdr = NibbleBuf::new(&buf);
        assert_eq!(rdr.get_vlu4_u32(), None);
        assert!(rdr.is_at_end());
    }

    #[test]
    fn write_vlu4_u32_3() {
        let mut buf = [0u8; 4];
        let mut wgr = NibbleBufMut::new(&mut buf);
        wgr.put_vlu4_u32(3);
        assert_eq!(wgr.nibbles_pos(), 1);
        assert_eq!(buf[0], 0b0011_0000);
    }

    // ≈ 1.5M/s on Core i7 8700K
    // ≈ 47min to complete on all 32 bit numbers
    // #[test]
    // fn round_trip_vlu4_u32() {
    //     let mut buf = [0u8; 11];
    //     for i in 0..u32::MAX {
    //         {
    //             let mut wgr = NibbleBufMut::new(&mut buf);
    //             wgr.put_vlu4_u32(i);
    //             assert!(!wgr.is_at_end());
    //         }
    //         if i % 10_000_000 == 0 {
    //             println!("{}", i);
    //             std::io::stdout().flush().unwrap();
    //         }
    //
    //         let mut rgr = NibbleBuf::new(&mut buf);
    //         assert_eq!(rgr.get_vlu4_u32(), Some(i));
    //     }
    // }
}