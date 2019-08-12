use std::{
    alloc::{alloc, realloc, Layout},
    mem::transmute,
    ptr::copy_nonoverlapping,
};

pub(super) struct ByteVec {
    // data pointer
    pub(super) ptr: *mut u8,

    // element size (in bytes)
    pub(super) size: usize,

    // length and capacity (in elements)
    pub(super) len: usize,
    pub(super) cap: usize,
}

impl ByteVec {
    pub(super) fn new(size: usize) -> Self {
        Self {
            ptr: unsafe { alloc(Layout::from_size_align(size * 32, size).unwrap()) },
            size,
            len: 0,
            cap: 32,
        }
    }

    unsafe fn reserve(&mut self, elements: usize) -> *mut u8 {
        // if len + element exceeds the cap, realloc (len + elements) * 2
        if self.len + elements > self.cap {
            let new_elements = (self.len + elements) * 2;
            self.ptr = realloc(
                self.ptr,
                Layout::from_size_align(self.len * self.size, self.size).unwrap(),
                new_elements * self.size,
            );
            self.cap = new_elements;
        }

        // return the new end of the segment
        self.ptr.add(self.len * self.size)
    }

    pub(super) unsafe fn typed_push<T>(&mut self, t: T) {
        // ensure at least 1 element can fit in self
        let e = self.reserve(1);

        // push
        *transmute::<*mut u8, *mut T>(e) = t;
        self.len += 1;
    }

    // drains this vec into another vec
    pub(super) unsafe fn draining_push(&mut self, other: &mut ByteVec) {
        // ensure at least self.len elements can fit in other
        let e = other.reserve(self.len);

        // copy
        copy_nonoverlapping(self.ptr, e, self.len * self.size);
        other.len += self.len;
        self.len = 0;
    }
}
