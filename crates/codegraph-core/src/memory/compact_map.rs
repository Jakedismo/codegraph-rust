use hashbrown::HashMap as HbHashMap;
use std::hash::BuildHasherDefault;
use rustc_hash::FxHasher;
use std::hash::Hash;

/// A drop-in HashMap replacement using hashbrown + Fx hasher.
/// Lower CPU overhead and slightly reduced metadata overhead vs std.
#[derive(Debug, Clone)]
pub struct CompactHashMap<K, V>(HbHashMap<K, V, BuildHasherDefault<FxHasher>>);

impl<K: Eq + Hash, V> Default for CompactHashMap<K, V> {
    fn default() -> Self {
        Self(HbHashMap::with_hasher(BuildHasherDefault::<FxHasher>::default()))
    }
}

impl<K: Eq + Hash, V> CompactHashMap<K, V> {
    pub fn with_capacity(n: usize) -> Self {
        Self(HbHashMap::with_capacity_and_hasher(n, BuildHasherDefault::<FxHasher>::default()))
    }
    pub fn len(&self) -> usize { self.0.len() }
    pub fn is_empty(&self) -> bool { self.0.is_empty() }
    pub fn clear(&mut self) { self.0.clear() }
    pub fn get<Q: ?Sized>(&self, k: &Q) -> Option<&V>
    where
        K: std::borrow::Borrow<Q>,
        Q: std::hash::Hash + Eq,
    { self.0.get(k) }
    pub fn get_mut<Q: ?Sized>(&mut self, k: &Q) -> Option<&mut V>
    where
        K: std::borrow::Borrow<Q>,
        Q: std::hash::Hash + Eq,
    { self.0.get_mut(k) }
    pub fn insert(&mut self, k: K, v: V) -> Option<V> { self.0.insert(k, v) }
    pub fn remove<Q: ?Sized>(&mut self, k: &Q) -> Option<V>
    where
        K: std::borrow::Borrow<Q>,
        Q: std::hash::Hash + Eq,
    { self.0.remove(k) }
    pub fn iter(&self) -> impl Iterator<Item = (&K, &V)> { self.0.iter() }
    pub fn iter_mut(&mut self) -> impl Iterator<Item = (&K, &mut V)> { self.0.iter_mut() }
}
