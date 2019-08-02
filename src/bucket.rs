use crate::compy::{CompyId, EntityId};
use crate::genvec::GenVec;
use parking_lot::{Mutex, MutexGuard, RwLock};
use std::any::TypeId;
use std::collections::HashMap;
use std::iter::FromIterator;

pub(super) struct Bucket {
    // current bucket data
    ids: RwLock<Vec<EntityId>>,
    data: HashMap<TypeId, RwLock<GenVec>>,

    // pending bucket data
    pdata: Mutex<HashMap<TypeId, GenVec>>,
}

impl Bucket {
    pub(super) fn new(key: CompyId, compyid_to_typeid: &HashMap<CompyId, TypeId>) -> Self {
        let c = (0..64)
            .into_iter()
            .map(|v| 1u64 << v)
            .filter(|v| key & v > 0)
            .map(|v| compyid_to_typeid[&v]);

        let data = HashMap::from_iter(c.clone().map(|v| (v, RwLock::new(GenVec::new()))));
        let pdata = Mutex::new(HashMap::from_iter(c.map(|v| (v, GenVec::new()))));

        Self {
            ids: RwLock::new(Vec::new()),
            data,
            pdata,
        }
    }

    pub(super) fn insert_lock(&self) -> MutexGuard<HashMap<TypeId, GenVec>> {
        self.pdata.lock()
    }
}
