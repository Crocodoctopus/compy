use std::mem::{size_of, transmute};

pub(super) struct GenVec([u8; size_of::<Vec<u8>>()]);

impl GenVec {
    pub(super) fn new() -> Self {
        GenVec(unsafe { transmute(Vec::<u8>::new()) })
    }

    pub(super) unsafe fn cast<'a, T>(&'a self) -> &'a Vec<T> {
        transmute(&self.0)
    }

    pub(super) unsafe fn cast_mut<'a, T>(&'a mut self) -> &'a mut Vec<T> {
        transmute(&mut self.0)
    }
}
