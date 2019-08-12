use crate::bucket::{Bucket, Lock};
use parking_lot::RwLock;
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

impl<'a, A, B, Func> CompyIterate<(A, B), Func> for Compy
where
    A: Lock<Output = A> + 'static,
    B: Lock<Output = B> + 'static,
    Func: Fn(A, B),
{
    fn iterate_mut(&mut self, pkey: CompyId, nkey: CompyId, f: Func) {
        let id0 = self.typeid_to_compyid[&A::base_type()];
        let id1 = self.typeid_to_compyid[&B::base_type()];

        for (key, bucket) in self.buckets.get_mut() {
            let bucket = bucket;
            if key & pkey == pkey && key & nkey == 0 {
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
        let i0 = self.typeid_to_compyid[&TypeId::of::<A>()];
        let i1 = self.typeid_to_compyid[&TypeId::of::<B>()];
        let key = i0 | i1;

        // get the bucket of said key
        let bucket = self.get_bucket(key);

        // insert tuple into the bucket
        let mut l = bucket.insert();
        unsafe {
            l.1.get_mut(&i0).unwrap().typed_push(t.0);
            l.1.get_mut(&i1).unwrap().typed_push(t.1);
        }
        l.0 += 1;
    }
}
