use crate::bucket::Bucket;
use parking_lot::{RwLock, RwLockWriteGuard};
use std::any::TypeId;
use std::collections::{BTreeMap, HashMap};
use std::mem::size_of;
use std::sync::Arc;

pub struct CompyBuilder {
    id_counter: CompyId,
    typeid_to_compyid: HashMap<TypeId, CompyId>,
    compyid_to_padding: HashMap<CompyId, usize>,
}

impl CompyBuilder {
    pub fn new() -> Self {
        Self {
            id_counter: 0,
            typeid_to_compyid: HashMap::new(),
            compyid_to_padding: HashMap::new(),
        }
    }

    pub fn with<T: 'static>(mut self) -> Self {
        self.id_counter <<= 1;
        if self.id_counter == 0 {
            self.id_counter = 1;
        }

        self.typeid_to_compyid
            .insert(TypeId::of::<T>(), self.id_counter);
        self.compyid_to_padding
            .insert(self.id_counter, size_of::<T>());
        self
    }

    pub fn build(self) -> Compy {
        Compy::new(self.typeid_to_compyid, self.compyid_to_padding)
    }
}

//////////////////////
// compy
pub type CompyId = u64;

pub struct Compy {
    // type data
    typeid_to_compyid: HashMap<TypeId, CompyId>,
    compyid_to_padding: HashMap<CompyId, usize>,

    // buckets
    buckets: RwLock<BTreeMap<CompyId, Arc<Bucket>>>,
}

impl Compy {
    pub fn new(
        typeid_to_compyid: HashMap<TypeId, CompyId>,
        compyid_to_padding: HashMap<CompyId, usize>,
    ) -> Self {
        Self {
            typeid_to_compyid,
            compyid_to_padding,
            buckets: RwLock::new(BTreeMap::new()),
        }
    }

    pub(super) fn get_bucket(&self, key: CompyId) -> Arc<Bucket> {
        let r = self.buckets.read();
        match r.get(&key) {
            Some(b) => b.clone(),
            None => {
                // the reader will have to be upgraded to a writer, so it may has well be dropped here
                drop(r);

                // generate a new bucket
                let b = Arc::new(Bucket::new(key, &self.compyid_to_padding));

                // insert the bucket, return a clone of the Arc
                self.buckets.write().insert(key, b.clone());
                b
            }
        }
    }

    pub fn get_key(&self, type_ids: &[TypeId]) -> u64 {
        type_ids
            .iter()
            .fold(0u64, |acc, id| acc | self.typeid_to_compyid[id])
    }

    pub fn update(&mut self) {
        for bucket in self
            .buckets
            .get_mut()
            .values_mut()
            .map(|b| Arc::get_mut(b).unwrap())
        {
            bucket.update();
        }
    }
}

///////
// interate impls
pub trait CompyIterate<Args, F> {
    fn iterate_mut(&mut self, pkey: CompyId, nkey: CompyId, f: F);
}

use crate::bucket::{Get, Reader, Writer};

pub trait Lock {
    type Lock;
    type Base;
    fn try_lock(bucket: &Bucket, compy_id: CompyId) -> Option<Self::Lock>;
}

impl<'a, A> Lock for &'a A {
    type Lock = Reader<'a, A>;
    type Base = A;
    fn try_lock(bucket: &Bucket, compy_id: CompyId) -> Option<Self::Lock> {
        Self::Lock::new(bucket, compy_id)
    }
}

impl<'a, A> Lock for &'a mut A {
    type Lock = Writer<'a, A>;
    type Base = A;
    fn try_lock(bucket: &Bucket, compy_id: CompyId) -> Option<Self::Lock> {
        unimplemented!()
    }
}

impl<'a, A, B, Func> CompyIterate<(A, B), Func> for Compy
where
    A: Lock + 'static,
    B: Lock + 'static,
    A::Lock: Get<'a, Output = A>,
    B::Lock: Get<'a, Output = B>,
    Func: Fn(A, B),
{
    fn iterate_mut(&mut self, pkey: CompyId, nkey: CompyId, mut f: Func) {
        let id0 = self.typeid_to_compyid[&TypeId::of::<A::Base>()];
        let id1 = self.typeid_to_compyid[&TypeId::of::<B::Base>()];

        for (key, bucket) in self.buckets.get_mut() {
            let bucket = bucket;
            if key & pkey == pkey && key & nkey == 0 {
                // get the locks
                let mut a_lock: A::Lock = A::try_lock(bucket, id0).expect("Unreachable for &mut self");
                let mut b_lock: B::Lock = B::try_lock(bucket, id1).expect("Unreachable for &mut self");

                // get the size of the bucket
                let len = bucket.get_len();

                // do the thing
                for index in 0..len {
                    let a = a_lock.get(index);
                    let b = b_lock.get(index);
                    //f(a, b);
                    drop(b);
                    drop(a);
                }
            }
        }
    }
}

/*
impl<'a, A, B, L1, L2, ARef, BRef, Func> CompyIterate<'a, (A, ARef, B, BRef, L1, L2), Func> for Compy
where
    A: 'static,
    B: 'static,
    L1: Get<ARef> + 'a,
    L2: Get<BRef> + 'a,
    ARef: IsRef<'a, A> + IntoLock<'a, L1, ARef>,
    BRef: IsRef<'a, B> + IntoLock<'a, L2, BRef>,
    Func: FnMut(ARef, BRef),
{
    fn iterate(&self, pkey: CompyId, nkey: CompyId, f: Func) {
        unimplemented!()
    }

    fn iterate_mut(&'a mut self, pkey: CompyId, nkey: CompyId, mut f: Func) {
        let id0 = self.typeid_to_compyid[&TypeId::of::<A>()];
        let id1 = self.typeid_to_compyid[&TypeId::of::<B>()];

        let buckets = self.buckets.get_mut();

        for (key, bucket) in buckets.iter_mut() {
            if key & pkey == pkey && key & nkey == 0 {
                unsafe {
                    let mut l0 = ARef::into_lock(bucket, id0).unwrap();
                    let mut l1 = BRef::into_lock(bucket, id1).unwrap();

                    let t0 = l0.get(0);
                    let t1 = l1.get(0);

                    f(t0, t1);

                    //let (len, l0): (usize, <Bucket as TryLock<BRef::Full>>::Lock) = bucket.try_lock(id1).unwrap();
                }
            }
        }

        unimplemented!()
        /*let id0 = self.typeid_to_compyid[&A::idof()];
        let id1 = self.typeid_to_compyid[&B::idof()];

        for (key, bucket) in self.buckets.get_mut() {
            if key & pkey == pkey && key & pkey == 0 {
                unsafe {
            }
        }*/
    }
}*/

///////
// insert
pub trait CompyInsert<T> {
    fn insert(&self, t: T);
}

/*impl<A> CompyInsert<(A,)> for Compy
where
    A: 'static,
{
    fn insert(&self, ts: &[(A,)]) {
        // create a key from the types
        let i0 = self.typeid_to_compyid[&TypeId::of::<A>()];
        let key = i0;

        // get the bucket of said key
        let bucket = self.get_bucket(key);

        // insert tuple into the bucket
        let mut l = bucket.insert_lock();
        for t in ts {
            unsafe {
                l.get_mut(&i0).unwrap().typed_push(t.0);
            }
        }
    }
}*/

impl<A, B> CompyInsert<(A, B)> for Compy
where
    A: 'static,
    B: 'static,
{
    fn insert(&self, t: (A, B)) {
        // create a key from the types
        let i0 = self.typeid_to_compyid[&TypeId::of::<A>()];
        let i1 = self.typeid_to_compyid[&TypeId::of::<B>()];
        let key = i0 | i1;

        // get the bucket of said key
        let bucket = self.get_bucket(key);

        // insert tuple into the bucket
        let mut l = bucket.insert();
        unsafe {
            l.get_mut(&i0).unwrap().typed_push(t.0);
            l.get_mut(&i1).unwrap().typed_push(t.1);
        }
    }
}

/*impl<A, B, C> CompyInsert<(A, B, C)> for Compy
where
    A: 'static,
    B: 'static,
    C: 'static,
{
    fn insert(&self, ts: &[(A, B, C)]) {
        // create a key from the types
        let i0 = self.typeid_to_compyid[&TypeId::of::<A>()];
        let i1 = self.typeid_to_compyid[&TypeId::of::<B>()];
        let i2 = self.typeid_to_compyid[&TypeId::of::<C>()];
        let key = i0 | i1 | i2;

        // get the bucket of said key
        let bucket = self.get_bucket(key);

        // insert tuple into the bucket
        let mut l = bucket.insert_lock();
        for t in ts {
            unsafe {
                l.get_mut(&i0).unwrap().typed_push(t.0);
                l.get_mut(&i1).unwrap().typed_push(t.1);
                l.get_mut(&i2).unwrap().typed_push(t.2);
            }
        }
    }
}
*/
