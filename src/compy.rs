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

    pub fn entity_count(&self) -> usize {
        self.buckets
            .read()
            .values()
            .fold(0, |acc, bucket| acc + bucket.get_len())
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

    pub fn print_stats(&self) {
        let mut bucket_count = 0;
        let mut total_components = 0;
        let mut total_len = 0;
        for bucket in self.buckets.read().values() {
            bucket_count += 1;
            total_components += bucket.data.len();
            total_len += bucket.get_len();
        }

        println!("Compy printout:");
        println!("  buckets: {:?}", bucket_count);
        println!(
            "  avg comp: {:?}",
            total_components as f32 / bucket_count as f32
        );
        println!("  avg size: {:?}", total_len as f32 / bucket_count as f32);
    }
}

/// Overloadable function for iterating entities
pub trait CompyIterate<Args, F> {
    fn iterate_mut(&mut self, pkey: Key, nkey: Key, f: F);
}

macro_rules! impl_compy_iterate {
    ($($ts: ident), *) => {
        impl <$($ts,)* Func> CompyIterate<($($ts,)*), Func> for Compy
            where $($ts: Lock<Output = $ts> + 'static,)*
                Func: FnMut($($ts,)*) -> bool {
                fn iterate_mut(&mut self, pkey: Key, nkey: Key, mut f: Func) {
                    // get component ids
                    $(let mut $ts = self.typeid_to_compid[&$ts::base_type()];)*

                    //
                    for (key, bucket) in self.buckets.get_mut().iter_mut().filter(|(key, _)| key.contains(pkey) && key.excludes(nkey)) {
                        {
                            // get locks
                            $(let mut $ts = $ts::try_lock(bucket, $ts).unwrap();)*

                            // get the bucket size
                            let len = bucket.get_len();

                            // do the thing
                            for index in 0..len {
                                f($($ts::get(&mut $ts, index),)*);
                            }
                        }
                    }
                }
        }
    }
}

impl_compy_iterate!();
impl_compy_iterate!(A);
impl_compy_iterate!(A, B);
impl_compy_iterate!(A, B, C);
impl_compy_iterate!(A, B, C, D);

/// Overloadable function for inserting entities
pub trait CompyInsert<T> {
    fn insert(&self, t: T);
}

macro_rules! impl_compy_insert {
    ($(($ts: ident, $vs: tt)), *) => {
        impl <$($ts: 'static + std::fmt::Debug,)*> CompyInsert<($($ts,)*)> for Compy {
            fn insert(&self, t: ($($ts,)*)) {
                // generate key from parts
                $(let $ts = self.typeid_to_compid[&TypeId::of::<($ts)>()];)*
                let key = Key::default() $(+ $ts)*;

                // get bucket of said key
                let bucket = self.get_bucket_or_make(key);

                // insert
                unsafe {
                    $(bucket.insert(&[($ts, &t.$vs as *const _ as * const _)]);)*
                    $(println!("{:?}", t.$vs);)*
                    std::mem::forget(t);       
                }
            }
        }
    }
}

impl_compy_insert!((A, 0));
impl_compy_insert!((A, 0), (B, 1));
impl_compy_insert!((A, 0), (B, 1), (C, 2));
impl_compy_insert!((A, 0), (B, 1), (C, 2), (D, 3));
impl_compy_insert!((A, 0), (B, 1), (C, 2), (D, 3), (E, 4));
impl_compy_insert!((A, 0), (B, 1), (C, 2), (D, 3), (E, 4), (F, 5));
impl_compy_insert!((A, 0), (B, 1), (C, 2), (D, 3), (E, 4), (F, 5), (G, 6));
impl_compy_insert!((A, 0), (B, 1), (C, 2), (D, 3), (E, 4), (F, 5), (G, 6), (H, 7));
impl_compy_insert!((A, 0), (B, 1), (C, 2), (D, 3), (E, 4), (F, 5), (G, 6), (H, 7), (I, 8));
impl_compy_insert!((A, 0), (B, 1), (C, 2), (D, 3), (E, 4), (F, 5), (G, 6), (H, 7), (I, 8), (J, 9));
