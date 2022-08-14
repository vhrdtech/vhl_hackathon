pub mod array;
pub mod semver;
pub mod slice_array;

pub use array::{Vlu4U32Array, Vlu4U32ArrayIter};
pub use slice_array::Vlu4SliceArray;
pub use semver::{SemVer, SemVerReq, TraitSet};

pub trait SerializeVlu4 {
    fn ser_vlu4(&self, wgr: &mut crate::serdes::NibbleBufMut);
}

/// Deserialize trait implemented by all types that support deserializing from buffer of nibbles.
/// 'i lifetime refers to the byte slice used when creating NibbleBuf.
/// 'di lifetime is for mutably borrowing NibbleBuf only while deserializing,
///     deserialized objects can hold non mutable links to the original buffer ('i).
pub trait DeserializeVlu4<'i>: Sized {
    type Error;

    fn des_vlu4<'di>(rdr: &'di mut crate::serdes::NibbleBuf<'i>) -> Result<Self, Self::Error>;
}

