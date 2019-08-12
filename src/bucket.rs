use crate::key::{Key, CompId};
use parking_lot::{
    MappedRwLockReadGuard, MappedRwLockWriteGuard, Mutex, MutexGuard, RwLock, RwLockReadGuard,
    RwLockWriteGuard,
};
use std::{
    alloc::{alloc, realloc, Layout},
    any::TypeId,
    collections::HashMap,
    mem::transmute,
    ptr::copy_nonoverlapping,
};

pub(super) struct PointerVec {
    // data pointer
    ptr: *mut u8,

    // element size (in bytes)
    size: usize,

    // length and capacity (in elements)
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
    data: HashMap<CompId, RwLock<PointerVec>>,

    // pending bucket data
    pdata: Mutex<(usize, HashMap<CompId, PointerVec>)>,
}

impl Bucket {
    pub(super) fn new(key: Key, compid_to_padding: &HashMap<CompId, usize>) -> Self {
        let mut data = HashMap::new();
        let mut pdata = Mutex::new((0, HashMap::new()));

        // for each non-0 bit in ``key``
        key.for_each_comp_id(|id| {
            let padding = compid_to_padding[&id];
            data.insert(id, RwLock::new(PointerVec::new(padding)));
            pdata.get_mut().1.insert(id, PointerVec::new(padding));
        });

        Self {
            len: 0,
            data,
            pdata,
        }
    }

    pub(super) fn try_read<'a>(
        &'a self,
        comp_id: CompId,
    ) -> Option<RwLockReadGuard<'a, PointerVec>> {
        Some(
            self.data
                .get(&comp_id)
                .expect("Fatal: No data")
                .try_read()?,
        )
    }

    pub(super) fn try_write<'a>(
        &'a self,
        comp_id: CompId,
    ) -> Option<RwLockWriteGuard<'a, PointerVec>> {
        Some(
            self.data
                .get(&comp_id)
                .expect("Fatal: No data")
                .try_write()?,
        )
    }

    pub(super) fn get_len(&self) -> usize {
        self.len
    }

    pub(super) fn insert(&self) -> MutexGuard<(usize, HashMap<CompId, PointerVec>)> {
        self.pdata.lock()
    }

    pub(super) fn update(&mut self) {
        let (count, hm) = self.pdata.get_mut();
        for (id, pv) in hm {
            let other = self.data.get_mut(id).unwrap().get_mut();
            unsafe {
                pv.draining_push(other);
            }
        }
        self.len += *count;
    }
}

pub struct Reader<'a, T> {
    read: MappedRwLockReadGuard<'a, [T]>,
}

impl<'a, T> Reader<'a, T> {
    pub(super) fn new(bucket: &'a Bucket, comp_id: CompId) -> Option<Self> {
        let read = bucket.try_read(comp_id)?;
        let read = RwLockReadGuard::map(read, |l| unsafe {
            std::slice::from_raw_parts(l.ptr as *const T, l.len)
        });
        Some(Self { read })
    }
}

pub struct Writer<'a, T> {
    write: MappedRwLockWriteGuard<'a, [T]>,
}

impl<'a, T> Writer<'a, T> {
    pub(super) fn new(bucket: &'a Bucket, comp_id: CompId) -> Option<Self> {
        let write = bucket.try_write(comp_id)?;
        let write = RwLockWriteGuard::map(write, |l| unsafe {
            std::slice::from_raw_parts_mut(l.ptr as *mut T, l.len)
        });
        Some(Self { write })
    }
}

///////////////
// HELL
// srsly don't go here
pub trait Lock {
    type Lock;
    type Output;

    fn base_type() -> TypeId;
    fn try_lock<'a>(bucket: &'a Bucket, comp_id: CompId) -> Option<Self::Lock>;
    fn get<'a>(lock: &'a mut Self::Lock, index: usize) -> Self::Output;
}

impl<T: 'static> Lock for &T {
    type Lock = Reader<'static, T>; // GAT <'a> in the future
    type Output = &'static T; // GAT <'a> in the future

    fn base_type() -> TypeId {
        TypeId::of::<T>()
    }

    fn try_lock<'a>(bucket: &'a Bucket, comp_id: CompId) -> Option<Self::Lock> {
        unsafe {
            std::mem::transmute::<Option<Reader<'a, T>>, Option<Reader<'static, T>>>(
                Reader::<'a, T>::new(bucket, comp_id),
            )
        }
    }

    fn get<'a>(lock: &'a mut Self::Lock, index: usize) -> Self::Output {
        unsafe { transmute::<&'a T, &'static T>(&lock.read[index]) }
    }
}

impl<T: 'static> Lock for &mut T {
    type Lock = Writer<'static, T>; // GAT <'a> in the future
    type Output = &'static mut T; // GAT <'a> in the future

    fn base_type() -> TypeId {
        TypeId::of::<T>()
    }

    fn try_lock<'a>(bucket: &'a Bucket, comp_id: CompId) -> Option<Self::Lock> {
        unsafe {
            std::mem::transmute::<Option<Writer<'a, T>>, Option<Writer<'static, T>>>(
                Writer::<'a, T>::new(bucket, comp_id),
            )
        }
    }

    fn get<'a>(lock: &'a mut Self::Lock, index: usize) -> Self::Output {
        unsafe { transmute::<&'a mut T, &'static mut T>(&mut lock.write[index]) }
    }
}
