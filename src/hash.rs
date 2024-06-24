use rustc_hash::FxBuildHasher;

pub type HashMap<K, V> = hashbrown::HashMap<K, V, FxBuildHasher>;
pub type HashSet<T> = hashbrown::HashSet<T, FxBuildHasher>;
