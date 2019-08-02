use crate::bucket::Bucket;
use parking_lot::RwLock;
use std::any::TypeId;
use std::collections::{BTreeMap, HashMap};
use std::iter::FromIterator;
use std::sync::Arc;

//////////////////////
// compy
pub type EntityId = u64;
pub type CompyId = u64;

pub struct Compy {
    // for converting typids to more convenient numbers
    typeid_to_compyid: HashMap<TypeId, CompyId>,
    compyid_to_typeid: HashMap<CompyId, TypeId>,

    // buckets
    buckets: RwLock<BTreeMap<CompyId, Arc<Bucket>>>,
}

impl Compy {
    pub fn new(type_ids: &[TypeId]) -> Self {
        Self {
            typeid_to_compyid: HashMap::from_iter(
                type_ids
                    .iter()
                    .enumerate()
                    .map(|(index, &ti)| (ti, 1 << index)),
            ),
            compyid_to_typeid: HashMap::from_iter(
                type_ids
                    .iter()
                    .enumerate()
                    .map(|(index, &ti)| (1 << index, ti)),
            ),
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
                let b = Arc::new(Bucket::new(key, &self.compyid_to_typeid));

                // insert the bucket, return a clone of the Arc
                self.buckets.write().insert(key, b).unwrap().clone()
            }
        }
    }

    pub fn get_key(&self, type_ids: &[TypeId]) -> u64 {
        type_ids
            .iter()
            .fold(0u64, |acc, id| acc | self.typeid_to_compyid[id])
    }

    pub fn update(&mut self) {}
}

///////
// interate impls
pub trait CompyIterate<T, F> {
    fn iterate(&self, pkey: CompyId, nkey: CompyId, f: F);
}

impl<A, FN: FnMut(EntityId, A) -> bool> CompyIterate<(A,), FN> for Compy {
    fn iterate(&self, pkey: CompyId, nkey: CompyId, f: FN) {
        unimplemented!()
    }
}

impl<A, B, FN: FnMut(EntityId, A, B) -> bool> CompyIterate<(A, B), FN> for Compy {
    fn iterate(&self, pkey: CompyId, nkey: CompyId, f: FN) {
        unimplemented!()
    }
}

impl<A, B, C, FN: FnMut(EntityId, A, B, C) -> bool> CompyIterate<(A, B, C), FN> for Compy {
    fn iterate(&self, pkey: CompyId, nkey: CompyId, f: FN) {
        unimplemented!()
    }
}

impl<A, B, C, D, FN: FnMut(EntityId, A, B, C, D) -> bool> CompyIterate<(A, B, C, D), FN> for Compy {
    fn iterate(&self, pkey: CompyId, nkey: CompyId, f: FN) {
        unimplemented!()
    }
}

impl<A, B, C, D, E, FN: FnMut(EntityId, A, B, C, D, E) -> bool> CompyIterate<(A, B, C, D, E), FN> for Compy {
    fn iterate(&self, pkey: CompyId, nkey: CompyId, f: FN) {
        unimplemented!()
    }
}

///////
// insert
pub trait CompyInsert<T> {
    fn insert(&self, t: T);
}

impl<A> CompyInsert<(A,)> for Compy
where
    A: 'static,
{
    fn insert(&self, t: (A,)) {
        // create a key from the types
        let i1 = self.typeid_to_compyid[&TypeId::of::<A>()];
        let key = i1;

        // get the bucket of said key
        let bucket = self.get_bucket(key);

        // insert tuple into the bucket
        let mut l = bucket.insert_lock();
        unsafe {
            l.get_mut(&TypeId::of::<A>()).unwrap().cast_mut().push(t.0);
        }
    }
}

impl<A, B> CompyInsert<(A, B)> for Compy
where
    A: 'static,
    B: 'static,
{
    fn insert(&self, t: (A, B)) {
        // create a key from the types
        let i1 = self.typeid_to_compyid[&TypeId::of::<A>()];
        let i2 = self.typeid_to_compyid[&TypeId::of::<B>()];
        let key = i1 | i2;

        // get the bucket of said key
        let bucket = self.get_bucket(key);

        // insert tuple into the bucket
        let mut l = bucket.insert_lock();
        unsafe {
            l.get_mut(&TypeId::of::<A>()).unwrap().cast_mut().push(t.0);
            l.get_mut(&TypeId::of::<B>()).unwrap().cast_mut().push(t.1);
        }
    }
}

impl<A, B, C> CompyInsert<(A, B, C)> for Compy
where
    A: 'static,
    B: 'static,
    C: 'static,
{
    fn insert(&self, t: (A, B, C)) {
        // create a key from the types
        let i1 = self.typeid_to_compyid[&TypeId::of::<A>()];
        let i2 = self.typeid_to_compyid[&TypeId::of::<B>()];
        let i3 = self.typeid_to_compyid[&TypeId::of::<C>()];
        let key = i1 | i2 | i3;

        // get the bucket of said key
        let bucket = self.get_bucket(key);

        // insert tuple into the bucket
        let mut l = bucket.insert_lock();
        unsafe {
            l.get_mut(&TypeId::of::<A>()).unwrap().cast_mut().push(t.0);
            l.get_mut(&TypeId::of::<B>()).unwrap().cast_mut().push(t.1);
            l.get_mut(&TypeId::of::<C>()).unwrap().cast_mut().push(t.2);
        }
    }
}
