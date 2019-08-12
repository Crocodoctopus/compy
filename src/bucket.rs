use crate::compy::CompyId;
use parking_lot::{
    MappedRwLockReadGuard, MappedRwLockWriteGuard, Mutex, MutexGuard, RwLock, RwLockReadGuard,
    RwLockWriteGuard,
};
use std::alloc::{alloc, realloc, Layout};
use std::any::TypeId;
use std::collections::HashMap;
use std::iter::FromIterator;
use std::mem::transmute;
use std::ptr::copy_nonoverlapping;
use std::sync::atomic::{AtomicUsize, Ordering};

pub(super) struct PointerVec {
    // data ptr
    ptr: *mut u8,

    // size of each element
    size: usize,

    // (in elements)
    len: usize,
    cap: usize,
}

impl PointerVec {
    fn new(size: usize) -> Self {
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
    pub(super) unsafe fn draining_push(&mut self, other: &mut PointerVec) {
        // ensure at least self.len elements can fit in other
        let e = other.reserve(self.len);

        // copy
        copy_nonoverlapping(self.ptr, e, self.len * self.size);
        other.len += self.len;
        self.len = 0;
    }
}

pub struct Bucket {
    // current bucket data
    // ids: RwLock<Vec<EntityId>>,
    len: usize,
    data: HashMap<CompyId, RwLock<PointerVec>>,

    // pending bucket data
    pdata: Mutex<HashMap<CompyId, PointerVec>>,
}

impl Bucket {
    pub(super) fn new(key: CompyId, compyid_to_padding: &HashMap<CompyId, usize>) -> Self {
        let mut data = HashMap::new();
        let mut pdata = Mutex::new(HashMap::new());

        // for each non-0 bit in ``key``
        for id in (0..64).map(|v| 1u64 << v).filter(|v| key & v > 0) {
            let padding = compyid_to_padding[&id];
            data.insert(id, RwLock::new(PointerVec::new(padding)));
            pdata.get_mut().insert(id, PointerVec::new(padding));
        }

        Self {
            len: 0,
            data,
            pdata,
        }
    }

    pub(super) fn try_read<'a>(
        &'a self,
        compy_id: CompyId,
    ) -> Option<RwLockReadGuard<'a, PointerVec>> {
        Some(
            self.data
                .get(&compy_id)
                .expect("Fatal: No data")
                .try_read()?,
        )
    }

    pub(super) fn try_write<'a>(
        &'a self,
        compy_id: CompyId,
    ) -> Option<RwLockWriteGuard<'a, PointerVec>> {
        Some(
            self.data
                .get(&compy_id)
                .expect("Fatal: No data")
                .try_write()?,
        )
    }

    pub(super) fn get_len(&self) -> usize {
        self.len
    }

    pub(super) fn insert(&self) -> MutexGuard<HashMap<CompyId, PointerVec>> {
        self.pdata.lock()
    }

    pub(super) fn update(&mut self) {
        for (id, pv) in self.pdata.get_mut() {
            let other = self.data.get_mut(id).unwrap().get_mut();
            unsafe {
                pv.draining_push(other);
            }
        }
    }
}

pub struct Reader<'a, T> {
    read: MappedRwLockReadGuard<'a, [T]>,
}

impl<'a, T> Reader<'a, T> {
    pub(super) fn new(bucket: &Bucket, compy_id: CompyId) -> Option<Self> {
        /*let read = bucket.try_read(compy_id)?;
        let read = RwLockReadGuard::map(read, |l| unsafe {
            std::slice::from_raw_parts(l.ptr as *const T, l.len)
        });
        Some(Self { read })*/
        unimplemented!()
    }
}

pub struct Writer<'a, T> {
    write: MappedRwLockWriteGuard<'a, [T]>,
}

pub trait Get<'a> {
    type Output;
    fn get(&'a mut self, index: usize) -> Self::Output;
}

impl<'a, T> Get<'a> for Reader<'a, T> {
    type Output = &'a T;
    fn get(&'a mut self, index: usize) -> Self::Output {
        &self.read[index]
    }
}

impl<'a, T> Get<'a> for Writer<'a, T> {
    type Output = &'a mut T;
    fn get(&'a mut self, index: usize) -> Self::Output {
        &mut self.write[index]
    }
}
