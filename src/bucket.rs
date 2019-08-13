use crate::{
    byte_vec::ByteVec,
    key::{CompId, Key},
};
use parking_lot::{
    MappedRwLockReadGuard, MappedRwLockWriteGuard, Mutex, MutexGuard, RwLock, RwLockReadGuard,
    RwLockWriteGuard,
};
use std::{any::TypeId, collections::HashMap, mem::transmute};

/// This is only public because traits (Self::Lock) don't allow private members
pub struct Bucket {
    // current bucket data
    // ids: RwLock<Vec<EntityId>>,
    len: usize,
    data: HashMap<CompId, RwLock<ByteVec>>,

    // pending bucket data
    prem: Mutex<Vec<usize>>,
    pdata: Mutex<(usize, HashMap<CompId, ByteVec>)>,
}

impl Bucket {
    pub(super) fn new(key: Key, compid_to_padding: &HashMap<CompId, usize>) -> Self {
        let mut data = HashMap::new();
        let mut pdata = Mutex::new((0, HashMap::new()));

        // for each non-0 bit in ``key``
        key.for_each_comp_id(|id| {
            let padding = compid_to_padding[&id];
            data.insert(id, RwLock::new(ByteVec::new(padding)));
            pdata.get_mut().1.insert(id, ByteVec::new(padding));
        });

        Self {
            len: 0,
            data,
            prem: Mutex::new(Vec::new()),
            pdata,
        }
    }

    pub(super) fn try_read<'a>(&'a self, comp_id: CompId) -> Option<RwLockReadGuard<'a, ByteVec>> {
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
    ) -> Option<RwLockWriteGuard<'a, ByteVec>> {
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

    pub(super) fn insert(&self) -> MutexGuard<(usize, HashMap<CompId, ByteVec>)> {
        self.pdata.lock()
    }

    pub(super) fn remove(&self, indices: &mut Vec<usize>) {
        // merge indices into prem, discard duplicates
        let mut v0 = self.prem.lock();
        let v1 = indices;

        // generate a new vec
        let mut new_v = Vec::with_capacity(v0.len() + v1.len());

        // add a "cap" to both vectors
        v0.push(usize::max_value());
        v1.push(usize::max_value());

        // merge
        let mut index0 = 0;
        let mut index1 = 0;
        let mut last = usize::max_value();
        loop {
            let t0 = v0[index0];
            let t1 = v1[index1];

            // if v0 is smaller
            if t0 < t1 {
                // if the last value isn't v0
                if last != t0 {
                    new_v.push(t0);
                    last = t0;
                }

                // inc
                index0 += 1;

                continue;
            }

            // if v1 is smaller, and not a "cap"
            if t1 < t0 && t1 != usize::max_value() {
                // if the last value isn't v0
                if last != t1 {
                    new_v.push(t1);
                    last = t1;
                }

                // inc
                index1 += 1;

                continue;
            }

            // if we make it this far, we done
            break;
        }

        // replace v0
        *v0 = new_v;
    }

    pub(super) fn update(&mut self) {
        // remove
        let prem = self.prem.get_mut();
        if prem.len() > 0 {
            // pushes the "cap" onto the remove slice
            prem.push(self.len);
            println!("LOG:   deleting indices {:?}", prem);

            // for each data array, remove the indices in prem
            for bvec in self.data.iter_mut().map(|(_, rw)| rw.get_mut()) {
                unsafe {
                    bvec.sorted_remove(prem);
                }
            }

            //
            self.len -= prem.len() - 1;
            prem.clear();
        }

        // add
        let (count, hm) = self.pdata.get_mut();
        if *count > 0 {
            for (id, pv) in hm {
                let other = self.data.get_mut(id).unwrap().get_mut();
                unsafe {
                    pv.draining_push(other);
                }
            }
            self.len += *count;
            *count = 0;
        }
    }
}

// 3 3 3 3 3 3
// 1 2 3 4 6

// last: 6
// 1 2 3 4 6

/// This is only public because traits (Self::Lock) don't allow private members
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

/// This is only public because traits (Self::Lock) don't allow private members
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
