#[derive(Copy, Clone, Debug)]
pub struct Vlu4RateArray<'i> {
    data: &'i [u8],
    len: usize,
    pos: usize,
}






/// Observing or publishing rate in [Hz].
#[derive(Copy, Clone, Debug)]
pub struct Rate(UnitStatic<UqC<24, 8>, -1, 0, 0, 0, 0, 0, 0>);
