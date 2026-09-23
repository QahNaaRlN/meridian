//! Insertion-ordered keyed collections: the iteration order of a record's
//! own declaration, never of a hash. The Node references build their
//! cross-reference indexes with `Map`/`Set`, which iterate in insertion
//! order and, on a repeated key, keep the FIRST position with the LAST
//! value; diagnostics that walk such an index are reported in that order
//! here too, so output never depends on `HashMap`/`HashSet` iteration.

use std::collections::BTreeMap;

/// An insertion-ordered map. `insert` of an existing key replaces its value
/// and keeps its first position (`Map.prototype.set`).
#[derive(Debug, Clone)]
pub(crate) struct OrderedMap<K, V> {
    order: Vec<K>,
    values: BTreeMap<K, V>,
}

impl<K, V> Default for OrderedMap<K, V> {
    fn default() -> Self {
        Self {
            order: Vec::new(),
            values: BTreeMap::new(),
        }
    }
}

impl<K: Ord + Clone, V> OrderedMap<K, V> {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    /// `true` when the key was new.
    pub(crate) fn insert(&mut self, key: K, value: V) -> bool {
        let fresh = self.values.insert(key.clone(), value).is_none();
        if fresh {
            self.order.push(key);
        }
        fresh
    }

    pub(crate) fn get(&self, key: &K) -> Option<&V> {
        self.values.get(key)
    }

    pub(crate) fn contains(&self, key: &K) -> bool {
        self.values.contains_key(key)
    }

    pub(crate) fn len(&self) -> usize {
        self.order.len()
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.order.is_empty()
    }

    pub(crate) fn keys(&self) -> impl Iterator<Item = &K> {
        self.order.iter()
    }

    pub(crate) fn iter(&self) -> impl Iterator<Item = (&K, &V)> {
        self.order
            .iter()
            .filter_map(|k| self.values.get(k).map(|v| (k, v)))
    }
}

/// An insertion-ordered set (`Set`).
#[derive(Debug, Clone)]
pub(crate) struct OrderedSet<K>(OrderedMap<K, ()>);

impl<K> Default for OrderedSet<K> {
    fn default() -> Self {
        Self(OrderedMap::default())
    }
}

impl<K: Ord + Clone> OrderedSet<K> {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    /// `true` when the value was new.
    pub(crate) fn insert(&mut self, key: K) -> bool {
        self.0.insert(key, ())
    }

    pub(crate) fn contains(&self, key: &K) -> bool {
        self.0.contains(key)
    }

    pub(crate) fn len(&self) -> usize {
        self.0.len()
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub(crate) fn iter(&self) -> impl Iterator<Item = &K> {
        self.0.keys()
    }
}

impl<K: Ord + Clone> FromIterator<K> for OrderedSet<K> {
    fn from_iter<I: IntoIterator<Item = K>>(iter: I) -> Self {
        let mut set = Self::new();
        for key in iter {
            set.insert(key);
        }
        set
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_repeated_key_keeps_its_first_position_and_its_last_value() {
        let mut map = OrderedMap::new();
        assert!(map.insert("b", 1));
        assert!(map.insert("a", 2));
        assert!(!map.insert("b", 3));
        assert_eq!(map.iter().collect::<Vec<_>>(), [(&"b", &3), (&"a", &2)]);
        let set: OrderedSet<&str> = ["z", "a", "z"].into_iter().collect();
        assert_eq!(set.iter().collect::<Vec<_>>(), [&"z", &"a"]);
        assert_eq!(set.len(), 2);
    }
}
