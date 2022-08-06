use crate::units::Unit;

pub struct UQ {
    m: usize,
    n: usize,
    unit: Unit,
}

pub struct UQ_C<const M: usize, const N: usize> {

}