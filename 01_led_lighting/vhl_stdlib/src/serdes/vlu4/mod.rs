pub mod array;
pub mod semver;
pub mod slice_array;

pub use array::{Vlu4U32Array, Vlu4U32ArrayIter};
pub use slice_array::Vlu4SliceArray;
pub use semver::{SemVer, SemVerReq, TraitSet};

// pub trait SerializeVlu4 {
//     fn ser_vlu4(&self, wgr: &mut NibbleBufMut);
// }
//
// pub trait DeserializeVlu4 {
//     fn des_vlu4(rdr: &mut NibbleBuf) -> Self;
// }