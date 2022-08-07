use crate::units::Unit;

pub struct Uq {
    pub m: usize,
    pub n: usize,
    pub unit: Unit,
}

pub struct UqC<const M: usize, const N: usize> {

}