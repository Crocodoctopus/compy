use crate::key::{CompId, Key};
use parking_lot::{
    MappedRwLockReadGuard, MappedRwLockWriteGuard, Mutex, RwLock, RwLockReadGuard, RwLockWriteGuard,
};
use std::{
    alloc::{alloc, dealloc, realloc, Layout},
    any::TypeId,
    collections::HashMap,
    mem::transmute,
    ptr::{copy, copy_nonoverlapping},
};

/// This is only public because traits (Self::Lock) don't allow private members
pub struct Bucket {
    // Bucket data
    len: usize,
    cap: usize,
    pub(super) data: HashMap<CompId, RwLock<(*mut u8, usize)>>, // ptr, size

    // Entities pending insertion
    // Mutex<(len, cap, HashMap<id, (ptr, size)>)>
    ins: Mutex<(usize, usize, HashMap<CompId, (*mut u8, usize)>)>,
}

impl Drop for Bucket {
    fn drop(&mut self) {
        // free ``data`` pointers
        for (ptr, size) in self
            .data
            .values_mut()
            .map(|lock| (lock.get_mut().0, lock.get_mut().1))
        {
            unsafe {
                dealloc(ptr, Layout::from_size_align_unchecked(size * self.cap, 8));
            }
        }

        // free ``ins`` pointers
        let lock = self.ins.get_mut();
        for &(ptr, size) in lock.2.values() {
            unsafe {
                dealloc(ptr, Layout::from_size_align_unchecked(size * lock.1, 8));
            }
        }
    }
}

impl Bucket {
    // Constructs a new Bucket based on the given key
    pub(super) fn new(key: Key, compid_to_size: &HashMap<CompId, usize>) -> Self {
        // some starting parameters
        let bucket_len = 0;
        let bucket_cap = 32;

        // construct the data/ins hashmaps
        let mut data = HashMap::new();
        let mut ins = HashMap::new();
        key.for_each_comp_id(|id| {
            // the size associated with this id
            let size = compid_to_size[&id];

            // if the component is a tag (size == 0), then we don't need a data entry for it
            if size == 0 {
                return;
            }

            // allocate memory for both the component data AND pending component data
            let (data_ptr, ins_ptr) = unsafe {
                let data_ptr = alloc(Layout::from_size_align_unchecked(bucket_cap * size, 8));
                let ins_ptr = alloc(Layout::from_size_align_unchecked(bucket_cap * size, 8));
                (data_ptr, ins_ptr)
            };
            data.insert(id, RwLock::new((data_ptr, size)));
            ins.insert(id, (ins_ptr, size));
        });

        // build the Bucket
        Self {
            len: bucket_len,
            cap: bucket_cap,
            data,
            ins: Mutex::new((bucket_len, bucket_cap, ins)),
        }
    }

    // Pretty self explanatory
    pub(super) fn get_len(&self) -> usize {
        self.len
    }

    // Remove elements from the Bucket. Note that indices must be in order of least to greatest
    pub(super) fn remove(&mut self, indices: &[u32]) {
        for (ptr, size) in self.data.values_mut().map(|lock| &*lock.get_mut()) {
            for index in indices.iter().rev() {
                unsafe {
                    let src = ptr.add(*index as usize * size);
                    let dst = ptr.add(self.len * size - size);
                    let count = *size;
                    copy(src, dst, count);
                }
            }
        }
    }

    // Inserts an entity (pending_insert?)
    // #[TODO: Better name]
    pub(super) unsafe fn queue_entity_insert(&self, data: &[(CompId, *const u8)]) {
        // extract
        let mut lock = self.ins.lock();
        let (len, cap, hmap) = &mut *lock;

        // extend capacities of the pending entity data if needed
        // #SORTA_UNTESTED
        if *len + 1 > *cap {
            let new_cap = (*len + 1) * 2;
            for (data_ptr, size) in hmap.values_mut() {
                *data_ptr = realloc(
                    *data_ptr,
                    Layout::from_size_align_unchecked(*cap * *size, 8),
                    new_cap * *size,
                );
            }
            *cap = new_cap;
        }

        // insert
        data.iter()
            .filter_map(|(comp_id, src_ptr)| {
                hmap.get(comp_id)
                    .and_then(|(dst_ptr, size)| Some((src_ptr, dst_ptr, size)))
            })
            .for_each(|(&src_ptr, &dst_ptr, &size)| {
                let src: *const u8 = src_ptr;
                let dst: *mut u8 = dst_ptr.add(*len * size);
                let count = size;
                copy_nonoverlapping(src, dst, count);
            });
        *len += 1;
    }

    pub(super) fn insert_pending_entities(&mut self) {
        // extract
        let lock = self.ins.get_mut();
        let (len, _, hmap) = &mut *lock;

        // return early if no work needs to be done
        if *len == 0 {
            return;
        }

        // extend capacities of the entity data if needed
        // #UNTESTED
        if self.len + *len > self.cap {
            let new_cap = (self.len + *len) * 2;
            for (data_ptr, size) in self.data.iter_mut().map(|(_, rw)| rw.get_mut()) {
                unsafe {
                    *data_ptr = realloc(
                        *data_ptr,
                        Layout::from_size_align_unchecked(self.cap * *size, 8),
                        new_cap * *size,
                    );
                }
            }
            self.cap = new_cap;
        }

        // drain
        // for each pair in pending insert hashmap
        let hmiter1 = self.data.values_mut().map(|lock| lock.get_mut());
        let hmiter2 = hmap.values();
        for (&mut data_lock, &(ins_ptr, size)) in hmiter1.zip(hmiter2) {
            unsafe {
                // move
                let src: *const u8 = ins_ptr;
                let dst: *mut u8 = data_lock.0.add(self.len * data_lock.1);
                let count = *len * size;
                copy_nonoverlapping(src, dst, count);
            }
        }

        // add the drained data len to the real len, set drained len to 0
        self.len += *len;
        *len = 0;
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
    type Lock = MappedRwLockReadGuard<'static, *mut u8>; // GAT <'a> in the future
    type Output = &'static T; // GAT <'a> in the future

    fn base_type() -> TypeId {
        TypeId::of::<T>()
    }

    fn try_lock<'a>(bucket: &'a Bucket, comp_id: CompId) -> Option<Self::Lock> {
        let read = bucket.data.get(&comp_id).expect("ERROR").try_read()?;
        let read = RwLockReadGuard::map(read, |(ptr, _)| ptr);

        let out: MappedRwLockReadGuard<'a, *mut u8> = read;
        let out: MappedRwLockReadGuard<'static, *mut u8> = unsafe { transmute(out) };
        Some(out)
    }

    fn get<'a>(lock: &'a mut Self::Lock, index: usize) -> Self::Output {
        unsafe { &*(*(lock as &*mut u8) as *const T).add(index) }
    }
}

impl<T: 'static> Lock for &mut T {
    type Lock = MappedRwLockWriteGuard<'static, *mut u8>; // GAT <'a> in the future
    type Output = &'static mut T; // GAT <'a> in the future

    fn base_type() -> TypeId {
        TypeId::of::<T>()
    }

    fn try_lock<'a>(bucket: &'a Bucket, comp_id: CompId) -> Option<Self::Lock> {
        let write = bucket.data.get(&comp_id).expect("ERROR").try_write()?;
        let write = RwLockWriteGuard::map(write, |(ptr, _)| ptr);

        let out: MappedRwLockWriteGuard<'a, *mut u8> = write;
        let out: MappedRwLockWriteGuard<'static, *mut u8> = unsafe { transmute(out) };
        Some(out)
    }

    fn get<'a>(lock: &'a mut Self::Lock, index: usize) -> Self::Output {
        unsafe { &mut *(*(lock as &mut *mut u8) as *mut T).add(index) }
    }
}
