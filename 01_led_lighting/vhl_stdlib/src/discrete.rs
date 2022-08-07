/// 3 bit unsigned integer
#[derive(Copy, Clone, Debug)]
pub struct U3(u8);
impl U3 {
    pub const fn new(from: u8) -> Option<Self> {
        if from <= 7 {
            Some(U3(from))
        } else {
            None
        }
    }
}

/// 4 bit unsigned integer
#[derive(Copy, Clone, Debug)]
pub struct U4(u8);
impl U4 {
    pub const fn new(from: u8) -> Option<Self> {
        if from <= 15 {
            Some(U4(from))
        } else {
            None
        }
    }
}

/// 6 bit unsigned integer
#[derive(Copy, Clone, Debug)]
pub struct U6(u8);
impl U6 {
    pub const fn new(from: u8) -> Option<Self> {
        if from <= 63 {
            Some(U6(from))
        } else {
            None
        }
    }
}

/// 7 bit unsigned integer shifted +1
#[derive(Copy, Clone, Debug)]
pub struct U7Sp1(u8);
impl U7Sp1 {
    pub const fn new(from: u8) -> Option<Self> {
        if from >= 1 && from <= 128 {
            Some(U7Sp1(from - 1))
        } else {
            None
        }
    }

    pub fn to_u8(&self) -> u8 {
        self.0 + 1
    }
}