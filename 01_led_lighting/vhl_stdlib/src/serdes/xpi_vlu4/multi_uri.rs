use crate::serdes::NibbleBuf;
use crate::serdes::vlu4::Vlu4U32Array;
use crate::serdes::xpi_vlu4::{Uri, UriMask};

/// Allows to select any combination of resources in order to perform read/write or stream
/// operations on them all at once. Operations are performed sequentially in order of the resources
/// serial numbers, depth first. Responses to read requests or stream published values are arranged
/// in arbitrary order, that is deemed optimal at a time, all with proper uris attached, so it's possible
/// to distinguish them. In response to one request, one or many responses may arrive.
/// Maximum packets sizes, publishing and observing rates, maximum jitter is taken into account when
/// grouping responses together.
///
/// Examples:
/// (/a, bitfield: 0b110), (/b, bitfield: 0b011) selects /a/2, /a/3, /b/x, /b/y
/// (/b, bitfield: 0b100) select /b/z/u and /b/z/v
/// (/b/z, indexes: 1) selects /b/z/v
#[derive(Copy, Clone, Debug)]
pub struct MultiUri<'i> {
    rdr: NibbleBuf<'i>,
    len: usize,
}

impl<'i> MultiUri<'i> {
    pub fn new(mut rdr: NibbleBuf<'i>) -> MultiUri<'i> {
        let len = rdr.get_vlu4_u32() as usize;
        MultiUri {
            rdr,
            len
        }
    }

    /// Returns the amount of (Uri, UriMask) pairs
    pub fn len(&self) -> usize {
        self.len
    }

    pub fn iter(&self) -> MultiUriIter {
        MultiUriIter {
            rdr: self.rdr.clone(),
            len: self.len,
            pos: 0,
        }
    }
}

pub struct MultiUriIter<'i> {
    rdr: NibbleBuf<'i>,
    len: usize,
    pos: usize,
}

impl<'i> MultiUriIter<'i> {
    pub fn is_past_end(&self) -> bool {
        self.rdr.is_past_end()
    }
}

impl<'i> Iterator for MultiUriIter<'i> {
    type Item = (Uri<'i>, UriMask<'i>);

    fn next(&mut self) -> Option<Self::Item> {
        if self.pos >= self.len {
            return None;
        }
        self.pos += 1;

        let uri_arr = Vlu4U32Array::new(self.rdr);
        let rdr = uri_arr.lookahead();
        let (mask, rdr_after_mask) = UriMask::new(rdr);
        let mask = match mask {
            Some(mask) => mask,
            None => {
                self.rdr.fuse();
                self.pos = self.len;
                return None;
            }
        };
        self.rdr = rdr_after_mask;

        Some((Uri::MultiPart(uri_arr), mask))
    }
}

#[cfg(test)]
mod test {
    use crate::serdes::NibbleBuf;
    use crate::serdes::xpi_vlu4::multi_uri::MultiUri;
    use crate::serdes::xpi_vlu4::UriMask;

    #[test]
    fn one_pair_mask_u16() {
        let buf = [0x13, 0x12, 0x31, 0xab, 0xcd];
        let rdr = NibbleBuf::new(&buf);
        let multi_uri = MultiUri::new(rdr);
        let mut multi_uri_iter = multi_uri.iter();
        let (uri, mask) = multi_uri_iter.next().unwrap();
        assert!(multi_uri_iter.next().is_none());
        assert!(!multi_uri_iter.is_past_end());

        let mut uri_iter = uri.iter();
        assert_eq!(uri_iter.next(), Some(1));
        assert_eq!(uri_iter.next(), Some(2));
        assert_eq!(uri_iter.next(), Some(3));
        assert_eq!(uri_iter.next(), None);

        assert!(matches!(mask, UriMask::ByBitfield16(0xabcd)));
    }

    #[test]
    fn one_pair_mask_indices() {
        let buf = [0x12, 0x12, 0x52, 0x35];
        let rdr = NibbleBuf::new(&buf);
        let multi_uri = MultiUri::new(rdr);
        let mut multi_uri_iter = multi_uri.iter();
        let (uri, mask) = multi_uri_iter.next().unwrap();
        assert!(multi_uri_iter.next().is_none());
        assert!(!multi_uri_iter.is_past_end());

        let mut uri_iter = uri.iter();
        assert_eq!(uri_iter.next(), Some(1));
        assert_eq!(uri_iter.next(), Some(2));
        assert_eq!(uri_iter.next(), None);

        assert!(matches!(mask, UriMask::ByIndices(_)));
        if let UriMask::ByIndices(indices) = mask {
            let mut indices_iter = indices.iter();
            assert_eq!(indices_iter.next(), Some(3));
            assert_eq!(indices_iter.next(), Some(5));
            assert_eq!(indices_iter.next(), None);
        }
    }

    #[test]
    fn two_pairs_mask_all() {
        let buf = [0x22, 0x12, 0x63, 0x25, 0x66, 0x70];
        let rdr = NibbleBuf::new(&buf);
        let multi_uri = MultiUri::new(rdr);
        let mut multi_uri_iter = multi_uri.iter();

        let (uri0, mask0) = multi_uri_iter.next().unwrap();
        let (uri1, mask1) = multi_uri_iter.next().unwrap();
        assert!(multi_uri_iter.next().is_none());
        assert!(!multi_uri_iter.is_past_end());

        let mut uri_iter = uri0.iter();
        assert_eq!(uri_iter.next(), Some(1));
        assert_eq!(uri_iter.next(), Some(2));
        assert_eq!(uri_iter.next(), None);

        assert!(matches!(mask0, UriMask::All(3)));

        let mut uri_iter = uri1.iter();
        assert_eq!(uri_iter.next(), Some(5));
        assert_eq!(uri_iter.next(), Some(6));
        assert_eq!(uri_iter.next(), None);

        assert!(matches!(mask1, UriMask::All(7)));
    }
}