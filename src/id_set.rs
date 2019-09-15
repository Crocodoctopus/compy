use crate::key::Key;
use std::collections::BTreeMap;

pub struct IdSet(BTreeMap<Key, Vec<u32>>);

impl IdSet {
    pub fn new() -> Self {
        unimplemented!()
    }

    pub fn insert(&mut self, key: Key, vec: Vec<u32>) {
        self.0.insert(key, vec);
    }
}
