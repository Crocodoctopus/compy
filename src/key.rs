use std::ops::{Add, Sub};

/// Represents a component ID
#[derive(Copy, Clone, Eq, Ord, PartialEq, PartialOrd, Hash)]
pub struct CompId(u64);

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
#[derive(Copy, Clone, Eq, Ord, PartialEq, PartialOrd)]
pub struct Key(u64);

impl Key {
	pub(super) fn new(key: u64) -> Self {
		Key(key)
	}

	pub fn contains(self, key: Key) -> bool {
		self.0 & key.0 == key.0
	}

	pub fn for_each_comp_id<Func: FnMut(CompId)>(self, mut f: Func) {
		for id in (0..64).map(|v| 1u64 << v).filter(|v| self.0 & v > 0) {
			f(CompId(id))
		}	
	}
}

impl Default for Key {
	fn default() -> Self {
		Key(0)
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