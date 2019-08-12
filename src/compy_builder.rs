use crate::{
    compy::Compy,
    key::CompId,
};
use std::{any::TypeId, collections::HashMap, mem::size_of};

pub struct CompyBuilder {
    id_counter: CompId,
    typeid_to_compid: HashMap<TypeId, CompId>,
    compid_to_padding: HashMap<CompId, usize>,
}

impl CompyBuilder {
    pub fn new() -> Self {
        Self {
            id_counter: CompId::default(),
            typeid_to_compid: HashMap::new(),
            compid_to_padding: HashMap::new(),
        }
    }

    pub fn with<T: 'static>(mut self) -> Self {
        let id = self.id_counter;
        self.id_counter = self.id_counter.inc();

        self.typeid_to_compid
            .insert(TypeId::of::<T>(), self.id_counter);
        self.compid_to_padding
            .insert(self.id_counter, size_of::<T>());
        self
    }

    pub fn build(self) -> Compy {
        Compy::new(self.typeid_to_compid, self.compid_to_padding)
    }
}
