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
pub trait CompyIterate<Args, F, R> {
    fn iterate_mut(&mut self, pkey: Key, nkey: Key, f: F);
    //fn iterate_alive_mut(&mut self, pkey: Key, nkey: Key, f: F);
    //fn iterate_dead_mut(&mut self, pkey: Key, nkey: Key, f: F);
}

impl<'a, Func> CompyIterate<(), Func, bool> for Compy
where
    Func: FnMut() -> bool,
{
    fn iterate_mut(&mut self, pkey: Key, nkey: Key, mut f: Func) {
        let mut indices = Vec::<usize>::new();

        for (key, bucket) in self.buckets.get_mut() {
            let bucket = bucket;
            if key.contains(pkey) && key.excludes(nkey) {
                // get the size of the bucket
                let len = bucket.get_len();

                // do the thing
                indices.clear();
                for index in 0..len {
                    if f() {
                        indices.push(index);
                    }
                }
                if indices.len() > 0 {
                    bucket.flag_for_removal(&mut indices);
                    indices.clear();
                }
            }
        }
    }
}

impl<'a, A, Func> CompyIterate<(A,), Func, bool> for Compy
where
    A: Lock<Output = A> + 'static,
    Func: FnMut(A) -> bool,
{
    fn iterate_mut(&mut self, pkey: Key, nkey: Key, mut f: Func) {
        let id0 = self.typeid_to_compid[&A::base_type()];
        let mut indices = Vec::<usize>::new();

        for (key, bucket) in self.buckets.get_mut() {
            let bucket = bucket;
            if key.contains(pkey) && key.excludes(nkey) {
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
                    bucket.flag_for_removal(&mut indices);
                    indices.clear();
                }
            }
        }
    }
}

/*impl<'a, A, B, Func> CompyIterate<(A, B), Func, ()> for Compy
where
    A: Lock<Output = A> + 'static,
    B: Lock<Output = B> + 'static,
    Func: FnMut(A, B) -> (),
{
    fn iterate_mut(&mut self, pkey: Key, nkey: Key, mut f: Func) {
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
}*/

impl<'a, A, B, Func> CompyIterate<(A, B), Func, bool> for Compy
where
    A: Lock<Output = A> + 'static,
    B: Lock<Output = B> + 'static,
    Func: FnMut(A, B) -> bool,
{
    fn iterate_mut(&mut self, pkey: Key, nkey: Key, mut f: Func) {
        let id0 = self.typeid_to_compid[&A::base_type()];
        let id1 = self.typeid_to_compid[&B::base_type()];
        let mut indices = Vec::<usize>::new();

        for (key, bucket) in self.buckets.get_mut() {
            let bucket = bucket;
            if key.contains(pkey) && key.excludes(nkey) {
                // get the locks
                let mut a_lock = A::try_lock(bucket, id0).expect("Unreachable for &mut self");
                let mut b_lock = B::try_lock(bucket, id1).expect("Unreachable for &mut self");

                // get the size of the bucket
                let len = bucket.get_len();

                // do the thing
                indices.clear();
                for index in 0..len {
                    let a = A::get(&mut a_lock, index);
                    let b = B::get(&mut b_lock, index);
                    if f(a, b) {
                        indices.push(index);
                    }
                }
                if indices.len() > 0 {
                    bucket.flag_for_removal(&mut indices);
                    indices.clear();
                }
            }
        }
    }
}

impl<'a, A, B, C, Func> CompyIterate<(A, B, C), Func, bool> for Compy
where
    A: Lock<Output = A> + 'static,
    B: Lock<Output = B> + 'static,
    C: Lock<Output = C> + 'static,
    Func: FnMut(A, B, C) -> bool,
{
    fn iterate_mut(&mut self, pkey: Key, nkey: Key, mut f: Func) {
        let id0 = self.typeid_to_compid[&A::base_type()];
        let id1 = self.typeid_to_compid[&B::base_type()];
        let id2 = self.typeid_to_compid[&C::base_type()];
        let mut indices = Vec::<usize>::new();

        for (key, bucket) in self.buckets.get_mut() {
            let bucket = bucket;
            if key.contains(pkey) && key.excludes(nkey) {
                // get the locks
                let mut a_lock = A::try_lock(bucket, id0).expect("Unreachable for &mut self");
                let mut b_lock = B::try_lock(bucket, id1).expect("Unreachable for &mut self");
                let mut c_lock = C::try_lock(bucket, id2).expect("Unreachable for &mut self");

                // get the size of the bucket
                let len = bucket.get_len();

                // do the thing
                indices.clear();
                for index in 0..len {
                    let a = A::get(&mut a_lock, index);
                    let b = B::get(&mut b_lock, index);
                    let c = C::get(&mut c_lock, index);
                    if f(a, b, c) {
                        indices.push(index);
                    }
                }
                if indices.len() > 0 {
                    bucket.flag_for_removal(&mut indices);
                    indices.clear();
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

impl<A, B, C, D> CompyInsert<(A, B, C, D)> for Compy
where
    A: 'static,
    B: 'static,
    C: 'static,
    D: 'static,
{
    fn insert(&self, t: (A, B, C, D)) {
        // create a key from the types
        let i0 = self.typeid_to_compid[&TypeId::of::<A>()];
        let i1 = self.typeid_to_compid[&TypeId::of::<B>()];
        let i2 = self.typeid_to_compid[&TypeId::of::<C>()];
        let i3 = self.typeid_to_compid[&TypeId::of::<D>()];
        let key = i0 + i1 + i2 + i3;

        // get the bucket of said key
        let bucket = self.get_bucket_or_make(key);

        // insert tuple into the bucket
        bucket.insert(&[
            (i0, &t.0 as *const _ as *const _),
            (i1, &t.1 as *const _ as *const _),
            (i2, &t.2 as *const _ as *const _),
            (i3, &t.3 as *const _ as *const _),
        ]);
        std::mem::forget(t);
    }
}

impl<A, B, C, D, E> CompyInsert<(A, B, C, D, E)> for Compy
where
    A: 'static,
    B: 'static,
    C: 'static,
    D: 'static,
    E: 'static,
{
    fn insert(&self, t: (A, B, C, D, E)) {
        // create a key from the types
        let i0 = self.typeid_to_compid[&TypeId::of::<A>()];
        let i1 = self.typeid_to_compid[&TypeId::of::<B>()];
        let i2 = self.typeid_to_compid[&TypeId::of::<C>()];
        let i3 = self.typeid_to_compid[&TypeId::of::<D>()];
        let i4 = self.typeid_to_compid[&TypeId::of::<E>()];
        let key = i0 + i1 + i2 + i3 + i4;

        // get the bucket of said key
        let bucket = self.get_bucket_or_make(key);

        // insert tuple into the bucket
        bucket.insert(&[
            (i0, &t.0 as *const _ as *const _),
            (i1, &t.1 as *const _ as *const _),
            (i2, &t.2 as *const _ as *const _),
            (i3, &t.3 as *const _ as *const _),
            (i4, &t.4 as *const _ as *const _),
        ]);
        std::mem::forget(t);
    }
}

impl<A, B, C, D, E, F> CompyInsert<(A, B, C, D, E, F)> for Compy
where
    A: 'static,
    B: 'static,
    C: 'static,
    D: 'static,
    E: 'static,
    F: 'static,
{
    fn insert(&self, t: (A, B, C, D, E, F)) {
        // create a key from the types
        let i0 = self.typeid_to_compid[&TypeId::of::<A>()];
        let i1 = self.typeid_to_compid[&TypeId::of::<B>()];
        let i2 = self.typeid_to_compid[&TypeId::of::<C>()];
        let i3 = self.typeid_to_compid[&TypeId::of::<D>()];
        let i4 = self.typeid_to_compid[&TypeId::of::<E>()];
        let i5 = self.typeid_to_compid[&TypeId::of::<F>()];
        let key = i0 + i1 + i2 + i3 + i4 + i5;

        // get the bucket of said key
        let bucket = self.get_bucket_or_make(key);

        // insert tuple into the bucket
        bucket.insert(&[
            (i0, &t.0 as *const _ as *const _),
            (i1, &t.1 as *const _ as *const _),
            (i2, &t.2 as *const _ as *const _),
            (i3, &t.3 as *const _ as *const _),
            (i4, &t.4 as *const _ as *const _),
            (i5, &t.5 as *const _ as *const _),
        ]);
        std::mem::forget(t);
    }
}

impl<A, B, C, D, E, F, G> CompyInsert<(A, B, C, D, E, F, G)> for Compy
where
    A: 'static,
    B: 'static,
    C: 'static,
    D: 'static,
    E: 'static,
    F: 'static,
    G: 'static,
{
    fn insert(&self, t: (A, B, C, D, E, F, G)) {
        // create a key from the types
        let i0 = self.typeid_to_compid[&TypeId::of::<A>()];
        let i1 = self.typeid_to_compid[&TypeId::of::<B>()];
        let i2 = self.typeid_to_compid[&TypeId::of::<C>()];
        let i3 = self.typeid_to_compid[&TypeId::of::<D>()];
        let i4 = self.typeid_to_compid[&TypeId::of::<E>()];
        let i5 = self.typeid_to_compid[&TypeId::of::<F>()];
        let i6 = self.typeid_to_compid[&TypeId::of::<G>()];
        let key = i0 + i1 + i2 + i3 + i4 + i5 + i6;

        // get the bucket of said key
        let bucket = self.get_bucket_or_make(key);

        // insert tuple into the bucket
        bucket.insert(&[
            (i0, &t.0 as *const _ as *const _),
            (i1, &t.1 as *const _ as *const _),
            (i2, &t.2 as *const _ as *const _),
            (i3, &t.3 as *const _ as *const _),
            (i4, &t.4 as *const _ as *const _),
            (i5, &t.5 as *const _ as *const _),
            (i6, &t.6 as *const _ as *const _),
        ]);
        std::mem::forget(t);
    }
}

impl<A, B, C, D, E, F, G, H> CompyInsert<(A, B, C, D, E, F, G, H)> for Compy
where
    A: 'static,
    B: 'static,
    C: 'static,
    D: 'static,
    E: 'static,
    F: 'static,
    G: 'static,
    H: 'static,
{
    fn insert(&self, t: (A, B, C, D, E, F, G, H)) {
        // create a key from the types
        let i0 = self.typeid_to_compid[&TypeId::of::<A>()];
        let i1 = self.typeid_to_compid[&TypeId::of::<B>()];
        let i2 = self.typeid_to_compid[&TypeId::of::<C>()];
        let i3 = self.typeid_to_compid[&TypeId::of::<D>()];
        let i4 = self.typeid_to_compid[&TypeId::of::<E>()];
        let i5 = self.typeid_to_compid[&TypeId::of::<F>()];
        let i6 = self.typeid_to_compid[&TypeId::of::<G>()];
        let i7 = self.typeid_to_compid[&TypeId::of::<H>()];
        let key = i0 + i1 + i2 + i3 + i4 + i5 + i6 + i7;

        // get the bucket of said key
        let bucket = self.get_bucket_or_make(key);

        // insert tuple into the bucket
        bucket.insert(&[
            (i0, &t.0 as *const _ as *const _),
            (i1, &t.1 as *const _ as *const _),
            (i2, &t.2 as *const _ as *const _),
            (i3, &t.3 as *const _ as *const _),
            (i4, &t.4 as *const _ as *const _),
            (i5, &t.5 as *const _ as *const _),
            (i6, &t.6 as *const _ as *const _),
            (i7, &t.7 as *const _ as *const _),
        ]);
        std::mem::forget(t);
    }
}

impl<A, B, C, D, E, F, G, H, I> CompyInsert<(A, B, C, D, E, F, G, H, I)> for Compy
where
    A: 'static,
    B: 'static,
    C: 'static,
    D: 'static,
    E: 'static,
    F: 'static,
    G: 'static,
    H: 'static,
    I: 'static,
{
    fn insert(&self, t: (A, B, C, D, E, F, G, H, I)) {
        // create a key from the types
        let i0 = self.typeid_to_compid[&TypeId::of::<A>()];
        let i1 = self.typeid_to_compid[&TypeId::of::<B>()];
        let i2 = self.typeid_to_compid[&TypeId::of::<C>()];
        let i3 = self.typeid_to_compid[&TypeId::of::<D>()];
        let i4 = self.typeid_to_compid[&TypeId::of::<E>()];
        let i5 = self.typeid_to_compid[&TypeId::of::<F>()];
        let i6 = self.typeid_to_compid[&TypeId::of::<G>()];
        let i7 = self.typeid_to_compid[&TypeId::of::<H>()];
        let i8 = self.typeid_to_compid[&TypeId::of::<I>()];
        let key = i0 + i1 + i2 + i3 + i4 + i5 + i6 + i7 + i8;

        // get the bucket of said key
        let bucket = self.get_bucket_or_make(key);

        // insert tuple into the bucket
        bucket.insert(&[
            (i0, &t.0 as *const _ as *const _),
            (i1, &t.1 as *const _ as *const _),
            (i2, &t.2 as *const _ as *const _),
            (i3, &t.3 as *const _ as *const _),
            (i4, &t.4 as *const _ as *const _),
            (i5, &t.5 as *const _ as *const _),
            (i6, &t.6 as *const _ as *const _),
            (i7, &t.7 as *const _ as *const _),
            (i8, &t.8 as *const _ as *const _),
        ]);
        std::mem::forget(t);
    }
}

impl<A, B, C, D, E, F, G, H, I, J> CompyInsert<(A, B, C, D, E, F, G, H, I, J)> for Compy
where
    A: 'static,
    B: 'static,
    C: 'static,
    D: 'static,
    E: 'static,
    F: 'static,
    G: 'static,
    H: 'static,
    I: 'static,
    J: 'static,
{
    fn insert(&self, t: (A, B, C, D, E, F, G, H, I, J)) {
        // create a key from the types
        let i0 = self.typeid_to_compid[&TypeId::of::<A>()];
        let i1 = self.typeid_to_compid[&TypeId::of::<B>()];
        let i2 = self.typeid_to_compid[&TypeId::of::<C>()];
        let i3 = self.typeid_to_compid[&TypeId::of::<D>()];
        let i4 = self.typeid_to_compid[&TypeId::of::<E>()];
        let i5 = self.typeid_to_compid[&TypeId::of::<F>()];
        let i6 = self.typeid_to_compid[&TypeId::of::<G>()];
        let i7 = self.typeid_to_compid[&TypeId::of::<H>()];
        let i8 = self.typeid_to_compid[&TypeId::of::<I>()];
        let i9 = self.typeid_to_compid[&TypeId::of::<J>()];
        let key = i0 + i1 + i2 + i3 + i4 + i5 + i6 + i7 + i8 + i9;

        // get the bucket of said key
        let bucket = self.get_bucket_or_make(key);

        // insert tuple into the bucket
        bucket.insert(&[
            (i0, &t.0 as *const _ as *const _),
            (i1, &t.1 as *const _ as *const _),
            (i2, &t.2 as *const _ as *const _),
            (i3, &t.3 as *const _ as *const _),
            (i4, &t.4 as *const _ as *const _),
            (i5, &t.5 as *const _ as *const _),
            (i6, &t.6 as *const _ as *const _),
            (i7, &t.7 as *const _ as *const _),
            (i8, &t.8 as *const _ as *const _),
            (i9, &t.9 as *const _ as *const _),
        ]);
        std::mem::forget(t);
    }
}