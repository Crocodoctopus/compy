use std::{
    mem::size_of,
    ops::{Add, Sub},
};

type Bits = u64;

/// Represents a component ID
#[derive(Copy, Clone, Eq, Ord, PartialEq, PartialOrd, Hash, Debug)]
pub struct CompId(Bits);

impl CompId {
    pub(super) fn inc(self) -> CompId {
        CompId(self.0 << 1)
    }
}

impl Default for CompId {
    fn default() -> Self {
        CompId(1)
    }
}

impl Add<CompId> for CompId {
    type Output = Key;

    fn add(self, rhs: CompId) -> Self::Output {
        Key(self.0 | rhs.0)
    }
}

impl Sub<CompId> for CompId {
    type Output = Key;

    fn sub(self, rhs: CompId) -> Self::Output {
        Key(!(!self.0 | rhs.0))
    }
}

/// Represents a group of component IDs
#[derive(Copy, Clone, Eq, Ord, PartialEq, PartialOrd, Debug)]
pub struct Key(Bits);

impl Key {
    pub(super) fn contains(self, key: Key) -> bool {
        self.0 & key.0 == key.0
    }

    pub(super) fn excludes(self, key: Key) -> bool {
        self.0 & key.0 == 0
    }

    pub fn for_each_comp_id<Func: FnMut(CompId)>(self, mut f: Func) {
        for id in (0..size_of::<Bits>() * 8)
            .map(|v| 1 << v)
            .filter(|v| self.0 & v > 0)
        {
            f(CompId(id))
        }
    }
}

impl Default for Key {
    fn default() -> Self {
        Key(0)
    }
}

impl From<CompId> for Key {
    fn from(comp_id: CompId) -> Self {
        Key(comp_id.0)
    }
}

impl Add<Key> for Key {
    type Output = Self;

    fn add(self, rhs: Key) -> Self::Output {
        Key(self.0 | rhs.0)
    }
}

impl Add<CompId> for Key {
    type Output = Self;

    fn add(self, rhs: CompId) -> Self::Output {
        Key(self.0 | rhs.0)
    }
}

impl Sub<Key> for Key {
    type Output = Self;

    fn sub(self, rhs: Key) -> Self::Output {
        Key(!(!self.0 | rhs.0))
    }
}

impl Sub<CompId> for Key {
    type Output = Self;

    fn sub(self, rhs: CompId) -> Self::Output {
        Key(!(!self.0 | rhs.0))
    }
}
