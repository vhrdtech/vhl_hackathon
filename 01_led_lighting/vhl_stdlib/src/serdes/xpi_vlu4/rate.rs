use crate::q_numbers::UqC;
use crate::units::UnitStatic;

#[derive(Copy, Clone, Debug)]
pub struct Vlu4RateArray<'i> {
    pub data: &'i [u8],
    pub len: usize,
    pub pos: usize,
}






/// Observing or publishing rate in [Hz].
#[derive(Copy, Clone, Debug)]
pub struct Rate(UnitStatic<UqC<24, 8>, -1, 0, 0, 0, 0, 0, 0>);
