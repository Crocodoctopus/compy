use crate::{
    bucket::{Bucket, Lock},
    id_set::IdSet,
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
    pub fn insert_all(&mut self) {
        for bucket in self
            .buckets
            .get_mut()
            .values_mut()
            .map(|b| Arc::get_mut(b).unwrap())
        {
            //bucket.remove_pending();
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
pub trait CompyIterate<In, Out, IdSet, Func> {
    fn iterate_mut(&mut self, pkey: Key, nkey: Key, f: Func) -> IdSet;
}

macro_rules! impl_compy_iterate {
    (($($arg_names: tt, $args: tt),*), ($($bool_names: tt, $bools: ty),*), ($($vec_names: tt, $id_set_names: tt, $id_sets: ty),*)) => {
        impl<$($args,)* Func> CompyIterate<($($args,)*), ($($bools),*), ($($id_sets),*), Func> for Compy
        where
            $($args: Lock<Output = $args>,)*
            Func: FnMut($($args),*) -> ($($bools),*),
        {
            fn iterate_mut(&mut self, pkey: Key, nkey: Key, mut f: Func) -> ($($id_sets),*) {
                // get component ids
                $(let $arg_names = self.typeid_to_compid[&$args::base_type()];)*

                // create id groups
                $(let mut $id_set_names = IdSet::new();)*

                // iterate
                for (_key, bucket) in self.buckets.get_mut().iter_mut().filter(|(key, _)| key.contains(pkey) && key.excludes(nkey)) {
                    // get locks
                    $(let mut $arg_names = $args::try_lock(bucket, $arg_names).unwrap();)*

                    // get bucket len
                    let len = bucket.get_len();

                    // create vecs
                    $(let mut $vec_names = Vec::<u32>::with_capacity(len);)*

                    // do the thing
                    for index in 0..len {
                        #[allow(unused_parens)]
                        let ($($bool_names),*) = f($($args::get(&mut $arg_names, index)),*);
                        $(if $bool_names == true { $vec_names.push(index as u32); })*
                    }

                    // insert
                    $($id_set_names.insert(*_key, $vec_names);)*
                }

                ($($id_set_names),*)
            }
        }
    };
}

impl_compy_iterate! {(aa, A), (), ()}
impl_compy_iterate! {(aa, A, ab, B), (), ()}
impl_compy_iterate! {(aa, A, ab, B, ac, C), (), ()}
impl_compy_iterate! {(aa, A, ab, B, ac, C, ad, D), (), ()}
impl_compy_iterate! {(aa, A, ab, B, ac, C, ad, D, ae, E), (), ()}
impl_compy_iterate! {(aa, A, ab, B, ac, C, ad, D, ae, E, af, F), (), ()}
impl_compy_iterate! {(), (ba, bool), (veca, ia, IdSet)}
impl_compy_iterate! {(aa, A), (ba, bool), (veca, ia, IdSet)}
impl_compy_iterate! {(aa, A, ab, B), (ba, bool), (veca, ia, IdSet)}
impl_compy_iterate! {(aa, A, ab, B, ac, C), (ba, bool), (veca, ia, IdSet)}
impl_compy_iterate! {(aa, A, ab, B, ac, C, ad, D), (ba, bool), (veca, ia, IdSet)}
impl_compy_iterate! {(aa, A, ab, B, ac, C, ad, D, ae, E), (ba, bool), (veca, ia, IdSet)}
impl_compy_iterate! {(aa, A, ab, B, ac, C, ad, D, ae, E, af, F), (ba, bool), (veca, ia, IdSet)}
impl_compy_iterate! {(), (ba, bool, bb, bool), (veca, ia, IdSet, vecb, ib, IdSet)}
impl_compy_iterate! {(aa, A), (ba, bool, bb, bool), (veca, ia, IdSet, vecb, ib, IdSet)}
impl_compy_iterate! {(aa, A, ab, B), (ba, bool, bb, bool), (veca, ia, IdSet, vecb, ib, IdSet)}
impl_compy_iterate! {(aa, A, ab, B, ac, C), (ba, bool, bb, bool), (veca, ia, IdSet, vecb, ib, IdSet)}
impl_compy_iterate! {(aa, A, ab, B, ac, C, ad, D), (ba, bool, bb, bool), (veca, ia, IdSet, vecb, ib, IdSet)}
impl_compy_iterate! {(aa, A, ab, B, ac, C, ad, D, ae, E), (ba, bool, bb, bool), (veca, ia, IdSet, vecb, ib, IdSet)}
impl_compy_iterate! {(aa, A, ab, B, ac, C, ad, D, ae, E, af, F), (ba, bool, bb, bool), (veca, ia, IdSet, vecb, ib, IdSet)}
impl_compy_iterate! {(), (ba, bool, bb, bool, bc, bool), (veca, ia, IdSet, vecb, ib, IdSet, vecc, ic, IdSet)}
impl_compy_iterate! {(aa, A), (ba, bool, bb, bool, bc, bool), (veca, ia, IdSet, vecb, ib, IdSet, vecc, ic, IdSet)}
impl_compy_iterate! {(aa, A, ab, B), (ba, bool, bb, bool, bc, bool), (veca, ia, IdSet, vecb, ib, IdSet, vecc, ic, IdSet)}
impl_compy_iterate! {(aa, A, ab, B, ac, C), (ba, bool, bb, bool, bc, bool), (veca, ia, IdSet, vecb, ib, IdSet, vecc, ic, IdSet)}
impl_compy_iterate! {(aa, A, ab, B, ac, C, ad, D), (ba, bool, bb, bool, bc, bool), (veca, ia, IdSet, vecb, ib, IdSet, vecc, ic, IdSet)}
impl_compy_iterate! {(aa, A, ab, B, ac, C, ad, D, ae, E), (ba, bool, bb, bool, bc, bool), (veca, ia, IdSet, vecb, ib, IdSet, vecc, ic, IdSet)}
impl_compy_iterate! {(aa, A, ab, B, ac, C, ad, D, ae, E, af, F), (ba, bool, bb, bool, bc, bool), (veca, ia, IdSet, vecb, ib, IdSet, vecc, ic, IdSet)}

/// Overloadable function for inserting entities
pub trait CompyInsert<T> {
    fn insert(&self, t: T);
}

macro_rules! impl_compy_insert {
    ($(($t_names: tt, $ts: tt, $vs: tt)), *) => {
        impl <$($ts: 'static,)*> CompyInsert<($($ts,)*)> for Compy {
            fn insert(&self, t: ($($ts,)*)) {
                // generate key from parts
                $(let $t_names = self.typeid_to_compid[&TypeId::of::<$ts>()];)*
                let key = Key::default() $(+ $t_names)*;

                // get bucket of said key
                let bucket = self.get_bucket_or_make(key);

                // insert
                unsafe {
                    bucket.insert(&[$( ($t_names, &t.$vs as *const _ as * const _), )*]);
                    std::mem::forget(t);
                }
            }
        }
    }
}

impl_compy_insert!((a, A, 0));
impl_compy_insert!((a, A, 0), (b, B, 1));
impl_compy_insert!((a, A, 0), (b, B, 1), (c, C, 2));
impl_compy_insert!((a, A, 0), (b, B, 1), (c, C, 2), (d, D, 3));
impl_compy_insert!((a, A, 0), (b, B, 1), (c, C, 2), (d, D, 3), (e, E, 4));
impl_compy_insert!(
    (a, A, 0),
    (b, B, 1),
    (c, C, 2),
    (d, D, 3),
    (e, E, 4),
    (f, F, 5)
);
impl_compy_insert!(
    (a, A, 0),
    (b, B, 1),
    (c, C, 2),
    (d, D, 3),
    (e, E, 4),
    (f, F, 5),
    (g, G, 6)
);
impl_compy_insert!(
    (a, A, 0),
    (b, B, 1),
    (c, C, 2),
    (d, D, 3),
    (e, E, 4),
    (f, F, 5),
    (g, G, 6),
    (h, H, 7)
);
impl_compy_insert!(
    (a, A, 0),
    (b, B, 1),
    (c, C, 2),
    (d, D, 3),
    (e, E, 4),
    (f, F, 5),
    (g, G, 6),
    (h, H, 7),
    (i, I, 8)
);
impl_compy_insert!(
    (a, A, 0),
    (b, B, 1),
    (c, C, 2),
    (d, D, 3),
    (e, E, 4),
    (f, F, 5),
    (g, G, 6),
    (h, H, 7),
    (i, I, 8),
    (j, J, 9)
);
impl_compy_insert!(
    (a, A, 0),
    (b, B, 1),
    (c, C, 2),
    (d, D, 3),
    (e, E, 4),
    (f, F, 5),
    (g, G, 6),
    (h, H, 7),
    (i, I, 8),
    (j, J, 9),
    (k, K, 10)
);
impl_compy_insert!(
    (a, A, 0),
    (b, B, 1),
    (c, C, 2),
    (d, D, 3),
    (e, E, 4),
    (f, F, 5),
    (g, G, 6),
    (h, H, 7),
    (i, I, 8),
    (j, J, 9),
    (k, K, 10),
    (l, L, 11)
);
impl_compy_insert!(
    (a, A, 0),
    (b, B, 1),
    (c, C, 2),
    (d, D, 3),
    (e, E, 4),
    (f, F, 5),
    (g, G, 6),
    (h, H, 7),
    (i, I, 8),
    (j, J, 9),
    (k, K, 10),
    (l, L, 11),
    (m, M, 12)
);
impl_compy_insert!(
    (a, A, 0),
    (b, B, 1),
    (c, C, 2),
    (d, D, 3),
    (e, E, 4),
    (f, F, 5),
    (g, G, 6),
    (h, H, 7),
    (i, I, 8),
    (j, J, 9),
    (k, K, 10),
    (l, L, 11),
    (m, M, 12),
    (n, N, 13)
);
