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

use core::fmt::{Debug, Display, Formatter};
use crate::serdes::vlu4::DeserializeVlu4;

/// Buffer reader that treats input as a stream of nibbles
#[derive(Copy, Clone)]
pub struct NibbleBuf<'i> {
    buf: &'i [u8],
    // Position in bytes
    idx: usize,
    is_at_byte_boundary: bool,
    is_past_end: bool,
}

impl<'i> NibbleBuf<'i> {
    pub fn new(buf: &'i [u8]) -> Self {
        NibbleBuf {
            buf, idx: 0, is_at_byte_boundary: true, is_past_end: false,
        }
    }

    pub fn new_with_offset(buf: &'i [u8], offset_nibbles: usize) -> Self {
        NibbleBuf {
            buf,
            idx: offset_nibbles / 2,
            is_at_byte_boundary: offset_nibbles % 2 == 0,
            is_past_end: offset_nibbles > buf.len() * 2
        }
    }

    pub fn nibbles_pos(&self) -> usize {
        if self.is_at_byte_boundary {
            self.idx * 2
        } else {
            self.idx * 2 + 1
        }
    }

    pub fn nibbles_left(&self) -> usize {
        self.buf.len() * 2 - self.nibbles_pos()
    }

    pub fn is_at_end(&self) -> bool {
        self.idx >= self.buf.len()
    }

    /// Return true of there was one or more read attempts after reaching an end of the buffer.
    pub fn is_past_end(&self) -> bool {
        self.is_past_end
    }

    /// Put this reader into error state, so that it can be detected later
    /// Used in MultiUriIter,
    pub fn fuse(&mut self) {
        self.idx = self.buf.len();
        self.is_at_byte_boundary = true;
        self.is_past_end = true;
    }
    //
    // /// Return the rest of the input buffer after vlu4_u32 number
    // pub fn lookahead_vlu4_u32(mut rdr: NibbleBuf) -> NibbleBuf {
    //     while rdr.get_nibble() & 0b1000 != 0 {}
    //     rdr
    // }

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
            self.is_past_end = true;
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

    pub fn skip_vlu4_u32(&mut self) {
        while self.get_nibble() & 0b1000 != 0 {}
    }

    pub fn get_u8(&mut self) -> u8 {
        if self.nibbles_left() < 2 {
            self.is_past_end = true;
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

    pub fn des_vlu4<'di, T: DeserializeVlu4<'i>>(&'di mut self) -> T {
        T::des_vlu4(self)
    }
}

impl<'i> Display for NibbleBuf<'i> {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        write!(f, "NibbleBuf(")?;
        let mut buf = self.clone();
        if buf.nibbles_pos() > 0 {
            write!(f, "<{}< ", buf.nibbles_pos())?;
        }
        while !buf.is_at_end() {
            write!(f, "{:01x}", buf.get_nibble())?;
            if buf.nibbles_left() >= 1 {
                write!(f, " ")?;
            }
        }
        write!(f, ")")
    }
}

impl<'i> Debug for NibbleBuf<'i> {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}", self)
    }
}

pub struct NibbleBufMut<'i> {
    buf: &'i mut [u8],
    idx: usize,
    is_at_byte_boundary: bool,
    is_past_end: bool,
}

impl<'i> NibbleBufMut<'i> {
    pub fn new(buf: &'i mut [u8]) -> Self {
        NibbleBufMut {
            buf, idx: 0, is_at_byte_boundary: true, is_past_end: false,
        }
    }

    pub fn nibbles_pos(&self) -> usize {
        if self.is_at_byte_boundary {
            self.idx * 2
        } else {
            self.idx * 2 + 1
        }
    }

    pub fn nibbles_left(&self) -> usize {
        self.buf.len() * 2 - self.nibbles_pos()
    }

    pub fn is_at_end(&self) -> bool {
        self.idx >= self.buf.len()
    }

    pub fn is_past_end(&self) -> bool {
        self.is_past_end
    }

    pub fn finish(self) -> &'i [u8] {
        &self.buf[0..self.idx]
    }

    pub fn put_nibble(&mut self, nib: u8) {
        if self.is_at_end() {
            self.is_past_end = true;
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
            self.is_past_end = true;
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
        if self.nibbles_left() < 2 {
            self.is_past_end = true;
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

    use alloc::format;
    use super::{NibbleBuf, NibbleBufMut};

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
        assert!(rdr.is_past_end());
    }

    #[test]
    fn read_vlu4_u32_single_nibble() {
        let buf = [0b0111_0010, 0b0000_0001];
        let mut rdr = NibbleBuf::new(&buf);
        assert_eq!(rdr.get_vlu4_u32(), 7);
        assert_eq!(rdr.get_vlu4_u32(), 2);
        assert_eq!(rdr.get_vlu4_u32(), 0);
        assert_eq!(rdr.get_vlu4_u32(), 1);
        assert!(rdr.is_at_end());
    }

    #[test]
    fn read_vlu4_u32_multi_nibble() {
        let buf = [0b1111_0111, 0b1001_0000, 0b1000_0111];
        let mut rdr = NibbleBuf::new(&buf);
        assert_eq!(rdr.get_vlu4_u32(), 63);
        assert_eq!(rdr.nibbles_pos(), 2);
        assert_eq!(rdr.get_vlu4_u32(), 0b001000);
        assert_eq!(rdr.nibbles_pos(), 4);
        assert_eq!(rdr.get_vlu4_u32(), 0b111);
        assert!(rdr.is_at_end());
    }

    #[test]
    fn read_vlu4_u32_max() {
        let buf = [0b1011_1111, 0xff, 0xff, 0xff, 0xff, 0x70];
        let mut rdr = NibbleBuf::new(&buf);
        assert_eq!(rdr.get_vlu4_u32(), u32::MAX);
        assert_eq!(rdr.get_nibble(), 0);
        assert!(rdr.is_at_end());
    }

    #[test]
    fn read_vlu4_u32_max_plus1() {
        // ignore bit 33
        let buf = [0b1111_1111, 0xff, 0xff, 0xff, 0xff, 0x70];
        let mut rdr = NibbleBuf::new(&buf);
        assert_eq!(rdr.get_vlu4_u32(), u32::MAX);
        assert_eq!(rdr.get_nibble(), 0);
        assert!(rdr.is_at_end());
    }

    #[test]
    fn read_vlu4_u32_max_plus_nibble() {
        // ignore bit 33
        let buf = [0xff, 0xff, 0xff, 0xff, 0xff, 0xf0];
        let mut rdr = NibbleBuf::new(&buf);
        assert_eq!(rdr.get_vlu4_u32(), 0);
        assert!(rdr.is_at_end());
    }

    #[test]
    fn write_nibbles() {
        let mut buf = [0u8; 2];
        {
            let mut wgr = NibbleBufMut::new(&mut buf);
            wgr.put_nibble(1);
            wgr.put_nibble(2);
            wgr.put_nibble(3);
            wgr.put_nibble(4);
            assert!(wgr.is_at_end());
            wgr.put_nibble(0);
            assert!(wgr.is_past_end());
        }
        assert_eq!(buf[0] , 0x12);
        assert_eq!(buf[1] , 0x34);
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

    #[test]
    fn buf_display() {
        let buf = [0x12, 0x34, 0x56];
        let buf = NibbleBuf::new(&buf);
        assert_eq!(format!("{}", buf), "NibbleBuf(1 2 3 4 5 6)")
    }

    #[test]
    fn buf_display_partly_consumed() {
        let buf = [0x12, 0x43, 0x21];
        let mut buf = NibbleBuf::new(&buf);
        let _ = buf.get_nibble();
        let _ = buf.get_nibble();
        assert_eq!(format!("{}", buf), "NibbleBuf(<2< 4 3 2 1)")
    }
}