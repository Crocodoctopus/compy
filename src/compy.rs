use crate::{
    bucket::{Bucket, Lock},
    key::{CompId, Key},
};
use parking_lot::RwLock;
use std::{
    any::TypeId,
    collections::{BTreeMap, HashMap},
    sync::Arc,
};

/// The master type. Cotains all the component/entity data.
pub struct Compy {
    // type data
    typeid_to_compid: HashMap<TypeId, CompId>,
    compid_to_size: HashMap<CompId, usize>,

    // buckets
    buckets: RwLock<BTreeMap<Key, Arc<Bucket>>>,
}

impl Compy {
    pub(super) fn new(
        typeid_to_compid: HashMap<TypeId, CompId>,
        compid_to_size: HashMap<CompId, usize>,
    ) -> Self {
        Self {
            typeid_to_compid,
            compid_to_size,
            buckets: RwLock::new(BTreeMap::new()),
        }
    }

    pub(super) fn get_bucket_or_make(&self, key: Key) -> Arc<Bucket> {
        let r = self.buckets.read();
        match r.get(&key) {
            Some(b) => b.clone(),
            None => {
                // the reader will have to be upgraded to a writer, so it may has well be dropped here
                drop(r);

                // generate a new bucket
                let b = Arc::new(Bucket::new(key, &self.compid_to_size));

                // insert the bucket, return a clone of the Arc
                self.buckets.write().insert(key, b.clone());
                b
            }
        }
    }

    /// Gets the key associated with the given type ids. (deprecated?)
    pub fn get_key(&self, type_ids: &[TypeId]) -> Key {
        type_ids
            .iter()
            .fold(Key::default(), |acc, id| acc + self.typeid_to_compid[id])
    }

    /// Gets the key associated with the given type id.
    pub fn get_key_for<T: 'static>(&self) -> Key {
        Key::from(self.typeid_to_compid[&TypeId::of::<T>()])
    }

    /// Performs all pending inserts and deletes. This function is singlethreaded, and requires mutable access to Compy.
    pub fn update(&mut self) {
        println!("LOG: performing compy update");
        for bucket in self
            .buckets
            .get_mut()
            .values_mut()
            .map(|b| Arc::get_mut(b).unwrap())
        {
            bucket.remove_pending();
            bucket.insert_pending();
        }
    }
}

/// Overloadable function for iterating entities
pub trait CompyIterate<Args, F, R> {
    fn iterate_mut(&mut self, pkey: Key, nkey: Key, f: F);
    //fn iterate_alive_mut(&mut self, pkey: Key, nkey: Key, f: F);
    //fn iterate_dead_mut(&mut self, pkey: Key, nkey: Key, f: F);
}

impl<'a, A, Func> CompyIterate<(A,), Func, bool> for Compy
where
    A: Lock<Output = A> + 'static,
    Func: Fn(A) -> bool,
{
    fn iterate_mut(&mut self, pkey: Key, nkey: Key, f: Func) {
        println!("LOG: iterate_mut<A> called for p:{:?} n:{:?}", pkey, nkey);

        let id0 = self.typeid_to_compid[&A::base_type()];
        let mut indices = Vec::<usize>::with_capacity(1000);

        for (key, bucket) in self.buckets.get_mut() {
            let bucket = bucket;
            if key.contains(pkey) && key.excludes(nkey) {
                println!("LOG:   bucket found; k: {:?}", key);

                // get the locks
                let mut a_lock: A::Lock =
                    A::try_lock(bucket, id0).expect("Unreachable for &mut self");

                // get the size of the bucket
                let len = bucket.get_len();

                // do the thing
                indices.clear();
                for index in 0..len {
                    let a = A::get(&mut a_lock, index);
                    if f(a) {
                        indices.push(index);
                    }
                }
                if indices.len() > 0 {
                    println!("LOG:   indices scheduled for delete: {:?}", indices);
                    bucket.flag_for_removal(&mut indices);
                    indices.clear();
                }
            }
        }
    }
}

impl<'a, A, B, Func> CompyIterate<(A, B), Func, ()> for Compy
where
    A: Lock<Output = A> + 'static,
    B: Lock<Output = B> + 'static,
    Func: Fn(A, B) -> (),
{
    fn iterate_mut(&mut self, pkey: Key, nkey: Key, f: Func) {
        let id0 = self.typeid_to_compid[&A::base_type()];
        let id1 = self.typeid_to_compid[&B::base_type()];

        for (key, bucket) in self.buckets.get_mut() {
            let bucket = bucket;
            if key.contains(pkey) && key.excludes(nkey) {
                // get the locks
                let mut a_lock: A::Lock =
                    A::try_lock(bucket, id0).expect("Unreachable for &mut self");
                let mut b_lock: B::Lock =
                    B::try_lock(bucket, id1).expect("Unreachable for &mut self");

                // get the size of the bucket
                let len = bucket.get_len();

                // do the thing
                for index in 0..len {
                    let a = A::get(&mut a_lock, index);
                    let b = B::get(&mut b_lock, index);
                    f(a, b);
                }
            }
        }
    }
}

/// Overloadable function for inserting entities
pub trait CompyInsert<T> {
    fn insert(&self, t: T);
}

impl<A> CompyInsert<(A,)> for Compy
where
    A: 'static,
{
    fn insert(&self, t: (A,)) {
        // create a key from the types
        let i0 = self.typeid_to_compid[&TypeId::of::<A>()];
        let key = Key::from(i0);

        // get the bucket of said key
        let bucket = self.get_bucket_or_make(key);

        // insert tuple into the bucket
        bucket.insert(&[(i0, &t.0 as *const _ as *const _)]);
        std::mem::forget(t);
    }
}

impl<A, B> CompyInsert<(A, B)> for Compy
where
    A: 'static,
    B: 'static,
{
    fn insert(&self, t: (A, B)) {
        // create a key from the types
        let i0 = self.typeid_to_compid[&TypeId::of::<A>()];
        let i1 = self.typeid_to_compid[&TypeId::of::<B>()];
        let key = i0 + i1;

        // get the bucket of said key
        let bucket = self.get_bucket_or_make(key);

        // insert tuple into the bucket
        bucket.insert(&[
            (i0, &t.0 as *const A as *const u8),
            (i1, &t.1 as *const B as *const u8),
        ]);
        std::mem::forget(t);
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
        let i0 = self.typeid_to_compid[&TypeId::of::<A>()];
        let i1 = self.typeid_to_compid[&TypeId::of::<B>()];
        let i2 = self.typeid_to_compid[&TypeId::of::<C>()];
        let key = i0 + i1 + i2;

        // get the bucket of said key
        let bucket = self.get_bucket_or_make(key);

        // insert tuple into the bucket
        bucket.insert(&[
            (i0, &t.0 as *const _ as *const _),
            (i1, &t.1 as *const _ as *const _),
            (i2, &t.2 as *const _ as *const _),
        ]);
        std::mem::forget(t);
    }
}
