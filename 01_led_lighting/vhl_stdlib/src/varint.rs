use core::marker::PhantomData;

pub struct VarInt<F> {
    _phantom: PhantomData<F>,
}

#[allow(non_camel_case_types)]
pub struct vlu4;