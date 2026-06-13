use std::borrow::Borrow;
use std::fmt;

#[derive(Clone)]
pub struct FlatMap<K, V>(Vec<(K, V)>);

impl<K, V> Default for FlatMap<K, V> {
    fn default() -> Self {
        Self::new()
    }
}

impl<K, V> FlatMap<K, V> {
    pub const fn new() -> Self {
        Self(Vec::new())
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn iter(&self) -> std::slice::Iter<'_, (K, V)> {
        self.0.iter()
    }

    pub fn keys(&self) -> Keys<'_, K, V> {
        Keys(self.0.iter())
    }

    pub fn values(&self) -> Values<'_, K, V> {
        Values(self.0.iter())
    }
}

impl<K: fmt::Debug, V: fmt::Debug> fmt::Debug for FlatMap<K, V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_map().entries(self.0.iter().map(|(k, v)| (k, v))).finish()
    }
}

#[allow(dead_code)] // public iterator type; currently has no constructor.
pub struct Iter<'a, K, V>(std::slice::Iter<'a, (K, V)>);

impl<'a, K, V> Iterator for Iter<'a, K, V> {
    type Item = (&'a K, &'a V);

    fn next(&mut self) -> Option<Self::Item> {
        self.0.next().map(|(k, v)| (k, v))
    }
}

impl<K: fmt::Debug, V: fmt::Debug> fmt::Debug for Iter<'_, K, V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list().entries(self.0.clone().map(|(k, v)| (k, v))).finish()
    }
}

pub struct Keys<'a, K, V>(std::slice::Iter<'a, (K, V)>);

impl<'a, K, V> Iterator for Keys<'a, K, V> {
    type Item = &'a K;

    fn next(&mut self) -> Option<Self::Item> {
        self.0.next().map(|(k, _)| k)
    }
}

impl<K: fmt::Debug, V> fmt::Debug for Keys<'_, K, V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_set().entries(self.0.clone().map(|(k, _)| k)).finish()
    }
}

pub struct Values<'a, K, V>(std::slice::Iter<'a, (K, V)>);

impl<'a, K, V> Iterator for Values<'a, K, V> {
    type Item = &'a V;

    fn next(&mut self) -> Option<Self::Item> {
        self.0.next().map(|(_, v)| v)
    }
}

impl<K, V: fmt::Debug> fmt::Debug for Values<'_, K, V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_set().entries(self.0.clone().map(|(_, v)| v)).finish()
    }
}

impl<K: Ord, V> FlatMap<K, V> {
    pub fn insert(&mut self, key: K, value: V) -> Option<V> {
        match self.0.binary_search_by(|(k, _)| k.cmp(&key)) {
            Ok(idx) => Some(std::mem::replace(&mut self.0[idx].1, value)),
            Err(idx) => {
                self.0.insert(idx, (key, value));
                None
            },
        }
    }

    pub fn try_insert(&mut self, key: K, value: V) -> Result<&mut V, OccupiedError<'_, K, V>> {
        match self.0.binary_search_by(|(k, _)| k.cmp(&key)) {
            Ok(idx) => {
                let pair = &mut self.0[idx];
                Err(OccupiedError {
                    entry: OccupiedEntry {
                        key: &pair.0,
                        value: &mut pair.1,
                    },
                    value,
                })
            },
            Err(idx) => {
                self.0.insert(idx, (key, value));
                Ok(&mut self.0[idx].1)
            },
        }
    }

    pub fn contains_key(&self, key: &K) -> bool {
        self.0.binary_search_by(|(k, _)| k.cmp(key)).is_ok()
    }

    pub fn get<Q: Borrow<K>>(&self, key: &Q) -> Option<&V> {
        self.0
            .binary_search_by(|(k, _)| k.cmp(key.borrow()))
            .ok()
            .map(|idx| &self.0[idx].1)
    }

    pub fn get_key_value(&self, key: &K) -> Option<(&K, &V)> {
        self.0
            .binary_search_by(|(k, _)| k.cmp(key))
            .ok()
            .map(|idx| &self.0[idx])
            .map(|(k, v)| (k, v))
    }
}

pub struct OccupiedError<'a, K: 'a, V: 'a> {
    pub entry: OccupiedEntry<'a, K, V>,
    pub value: V,
}

pub struct OccupiedEntry<'a, K: 'a, V: 'a> {
    pub key: &'a K,
    pub value: &'a mut V,
}

impl<'a, K: 'a, V: 'a> OccupiedEntry<'a, K, V> {
    pub fn get(&self) -> &V {
        self.value
    }

    pub fn get_mut(&mut self) -> &mut V {
        self.value
    }

    pub fn key(&self) -> &K {
        self.key
    }
}

#[cfg(test)]
mod tests {
    use crate::map::FlatMap;

    fn ensure_variance<'x, 'a: 'b, 'b>(v: &'x FlatMap<&'a str, &'a str>) -> &'x FlatMap<&'b str, &'b str> {
        v
    }

    #[test]
    fn a() {
        let map = FlatMap::new();
        ensure_variance(&map);
    }
}
