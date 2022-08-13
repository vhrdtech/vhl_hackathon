/// Variable size array of u8 slices
#[derive(Copy, Clone, Debug)]
pub struct Vlu4SliceArray<'i> {
    buf: &'i [u8]
}