use crate::{
    bucket::{Bucket, Lock},
    key::{Key, CompId},
};
use parking_lot::RwLock;
use std::{
    any::TypeId,
    collections::{BTreeMap, HashMap},
    mem::size_of,
    sync::Arc,
};

//////////////////////
// compy
pub struct Compy {
    // type data
    typeid_to_compid: HashMap<TypeId, CompId>,
    compid_to_padding: HashMap<CompId, usize>,

    // buckets
    buckets: RwLock<BTreeMap<Key, Arc<Bucket>>>,
}

impl Compy {
    pub(super) fn new(
        typeid_to_compid: HashMap<TypeId, CompId>,
        compid_to_padding: HashMap<CompId, usize>,
    ) -> Self {
        Self {
            typeid_to_compid,
            compid_to_padding,
            buckets: RwLock::new(BTreeMap::new()),
        }
    }

    pub(super) fn get_bucket(&self, key: Key) -> Arc<Bucket> {
        let r = self.buckets.read();
        match r.get(&key) {
            Some(b) => b.clone(),
            None => {
                // the reader will have to be upgraded to a writer, so it may has well be dropped here
                drop(r);

                // generate a new bucket
                let b = Arc::new(Bucket::new(key, &self.compid_to_padding));

                // insert the bucket, return a clone of the Arc
                self.buckets.write().insert(key, b.clone());
                b
            }
        }
    }

    /// Gets the key associated with the given type ids.
    pub fn get_key(&self, type_ids: &[TypeId]) -> Key {
        type_ids
            .iter()
            .fold(Key::default(), |acc, id| acc + self.typeid_to_compid[id])
    }

    /// Performs all pending inserts and deletes. This function is singlethreaded, and requires mutable access to Compy.
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
    fn iterate_mut(&mut self, pkey: Key, nkey: Key, f: F);
}

impl<'a, A, B, Func> CompyIterate<(A, B), Func> for Compy
where
    A: Lock<Output = A> + 'static,
    B: Lock<Output = B> + 'static,
    Func: Fn(A, B),
{
    fn iterate_mut(&mut self, pkey: Key, nkey: Key, f: Func) {
        let id0 = self.typeid_to_compid[&A::base_type()];
        let id1 = self.typeid_to_compid[&B::base_type()];

        for (key, bucket) in self.buckets.get_mut() {
            let bucket = bucket;
            if key.contains(pkey) && !key.contains(nkey) {
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

///////
// insert
pub trait CompyInsert<T> {
    fn insert(&self, t: T);
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
        let bucket = self.get_bucket(key);

        // insert tuple into the bucket
        let mut l = bucket.insert();
        unsafe {
            if size_of::<A>() > 0 {
                l.1.get_mut(&i0).unwrap().typed_push(t.0);
            }
            if size_of::<B>() > 0 {
                l.1.get_mut(&i1).unwrap().typed_push(t.1);
            }
        }
        l.0 += 1;
    }
}
