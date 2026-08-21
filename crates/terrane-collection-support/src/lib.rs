use std::collections::{HashMap, HashSet};
use std::hash::{BuildHasherDefault, Hash, Hasher};
use std::sync::Arc;

use terrane_int_support::Int;
use unicode_segmentation::UnicodeSegmentation;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum IterationStep<T> {
    Item(T),
    End,
}

#[derive(Clone, Debug)]
pub struct Iterator<T> {
    items: Arc<Vec<T>>,
    index: usize,
    ended: bool,
}

impl<T> Iterator<T> {
    #[must_use]
    pub fn new(items: Vec<T>) -> Self {
        Self {
            items: Arc::new(items),
            index: 0,
            ended: false,
        }
    }
}

impl<T: Clone> Iterator<T> {
    #[must_use]
    #[expect(
        clippy::should_implement_trait,
        reason = "Terrane iteration returns an explicit typed step rather than Rust Option"
    )]
    pub fn next(&mut self) -> IterationStep<T> {
        next_indexed(&self.items, &mut self.index, &mut self.ended)
    }
}

#[derive(Clone, Debug)]
pub struct CollectionIterator<C> {
    collection: C,
    index: usize,
    ended: bool,
}

impl<C> CollectionIterator<C> {
    fn new(collection: C) -> Self {
        Self {
            collection,
            index: 0,
            ended: false,
        }
    }
}

impl<C: IndexedIteration> CollectionIterator<C> {
    #[must_use]
    #[expect(
        clippy::should_implement_trait,
        reason = "Terrane iteration returns an explicit typed step rather than Rust Option"
    )]
    pub fn next(&mut self) -> IterationStep<C::Item> {
        next_indexed(&self.collection, &mut self.index, &mut self.ended)
    }
}

fn next_indexed<S, T>(source: &S, index: &mut usize, ended: &mut bool) -> IterationStep<T>
where
    S: IndexedIteration<Item = T>,
{
    if *ended {
        return IterationStep::End;
    }
    if let Some(item) = source.item_at(*index) {
        *index += 1;
        IterationStep::Item(item)
    } else {
        *ended = true;
        IterationStep::End
    }
}

#[doc(hidden)]
pub trait IndexedIteration {
    type Item;
    fn item_at(&self, index: usize) -> Option<Self::Item>;
}

impl<T: Clone> IndexedIteration for Arc<Vec<T>> {
    type Item = T;
    fn item_at(&self, index: usize) -> Option<T> {
        self.get(index).cloned()
    }
}

pub trait Iterable {
    type Item: Clone + 'static;
    type Iter;
    fn terrane_iterator(&self) -> Self::Iter;
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct List<T>(Arc<Vec<T>>);

impl<T> List<T> {
    #[must_use]
    pub fn new(items: Vec<T>) -> Self {
        Self(Arc::new(items))
    }
    #[must_use]
    pub fn length(&self) -> i128 {
        self.0.len() as i128
    }
    #[must_use]
    pub fn get(&self, index: usize) -> Option<&T> {
        self.0.get(index)
    }
    /// Returns the indexed item or an error when the index is outside the list.
    ///
    /// # Errors
    /// Returns [`IndexError`] when `index` is out of range.
    pub fn get_or_error(&self, index: usize) -> Result<T, IndexError>
    where
        T: Clone,
    {
        self.0.get(index).cloned().ok_or(IndexError { index })
    }
}

impl<T: Clone> List<T> {
    /// Replaces an indexed item.
    ///
    /// # Errors
    ///
    /// Returns [`IndexError`] when `index` is outside the list.
    pub fn set(&mut self, index: usize, value: T) -> Result<(), IndexError> {
        let Some(slot) = Arc::make_mut(&mut self.0).get_mut(index) else {
            return Err(IndexError { index });
        };
        *slot = value;
        Ok(())
    }
    pub fn append(&mut self, value: T) {
        Arc::make_mut(&mut self.0).push(value);
    }
}

impl<T: Clone + 'static> IndexedIteration for List<T> {
    type Item = T;
    fn item_at(&self, index: usize) -> Option<T> {
        self.0.get(index).cloned()
    }
}

impl<T: Clone + 'static> Iterable for List<T> {
    type Item = T;
    type Iter = CollectionIterator<Self>;
    fn terrane_iterator(&self) -> Self::Iter {
        CollectionIterator::new(self.clone())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Tuple<T>(Arc<Vec<T>>);
impl<T> Tuple<T> {
    #[must_use]
    pub fn new(items: Vec<T>) -> Self {
        Self(Arc::new(items))
    }
    #[must_use]
    pub fn length(&self) -> i128 {
        self.0.len() as i128
    }
    #[must_use]
    pub fn get(&self, index: usize) -> Option<&T> {
        self.0.get(index)
    }
    /// Returns the indexed item or an error when the index is outside the tuple.
    ///
    /// # Errors
    /// Returns [`IndexError`] when `index` is out of range.
    pub fn get_or_error(&self, index: usize) -> Result<T, IndexError>
    where
        T: Clone,
    {
        self.0.get(index).cloned().ok_or(IndexError { index })
    }
}
impl<T: Clone + 'static> IndexedIteration for Tuple<T> {
    type Item = T;
    fn item_at(&self, index: usize) -> Option<T> {
        self.0.get(index).cloned()
    }
}
impl<T: Clone + 'static> Iterable for Tuple<T> {
    type Item = T;
    type Iter = CollectionIterator<Self>;
    fn terrane_iterator(&self) -> Self::Iter {
        CollectionIterator::new(self.clone())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Entry<K, V> {
    pub key: K,
    pub value: V,
}
impl<K, V> Entry<K, V> {
    #[must_use]
    pub fn new(key: K, value: V) -> Self {
        Self { key, value }
    }
}

#[derive(Clone, Debug)]
struct StableHasher(u64);

impl Default for StableHasher {
    fn default() -> Self {
        Self(0xcbf2_9ce4_8422_2325)
    }
}

impl Hasher for StableHasher {
    fn finish(&self) -> u64 {
        self.0
    }

    fn write(&mut self, bytes: &[u8]) {
        for byte in bytes {
            self.0 ^= u64::from(*byte);
            self.0 = self.0.wrapping_mul(0x0000_0100_0000_01b3);
        }
    }
}

type FixedState = BuildHasherDefault<StableHasher>;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Map<K: Eq + Hash, V>(Arc<indexmap::IndexMap<K, V, FixedState>>);
impl<K: Eq + Hash + Clone, V: Clone> Map<K, V> {
    #[must_use]
    pub fn new(entries: Vec<Entry<K, V>>) -> Self {
        let mut map = indexmap::IndexMap::with_hasher(FixedState::default());
        for entry in entries {
            map.insert(entry.key, entry.value);
        }
        Self(Arc::new(map))
    }
    #[must_use]
    pub fn length(&self) -> i128 {
        self.0.len() as i128
    }
    #[must_use]
    pub fn get(&self, key: &K) -> Option<&V> {
        self.0.get(key)
    }
    /// Returns the mapped value or an error when the key is absent.
    ///
    /// # Errors
    /// Returns [`MissingKey`] when `key` is absent.
    pub fn get_or_error(&self, key: &K) -> Result<V, MissingKey> {
        self.get(key).cloned().ok_or(MissingKey)
    }
    pub fn set(&mut self, key: K, value: V) {
        Arc::make_mut(&mut self.0).insert(key, value);
    }
    #[must_use]
    pub fn keys(&self) -> List<K> {
        List::new(self.0.keys().cloned().collect())
    }
    #[must_use]
    pub fn values(&self) -> List<V> {
        List::new(self.0.values().cloned().collect())
    }
    #[must_use]
    pub fn entries(&self) -> List<Entry<K, V>> {
        List::new(
            self.0
                .iter()
                .map(|(key, value)| Entry::new(key.clone(), value.clone()))
                .collect(),
        )
    }
}
impl<K: Eq + Hash + Clone + 'static, V: Clone + 'static> IndexedIteration for Map<K, V> {
    type Item = Entry<K, V>;
    fn item_at(&self, index: usize) -> Option<Self::Item> {
        self.0
            .get_index(index)
            .map(|(key, value)| Entry::new(key.clone(), value.clone()))
    }
}
impl<K: Eq + Hash + Clone + 'static, V: Clone + 'static> Iterable for Map<K, V> {
    type Item = Entry<K, V>;
    type Iter = CollectionIterator<Self>;
    fn terrane_iterator(&self) -> Self::Iter {
        CollectionIterator::new(self.clone())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Set<T: Eq + Hash>(Arc<indexmap::IndexSet<T, FixedState>>);
impl<T: Eq + Hash + Clone> Set<T> {
    #[must_use]
    pub fn new(items: Vec<T>) -> Self {
        let mut set = indexmap::IndexSet::with_hasher(FixedState::default());
        set.extend(items);
        Self(Arc::new(set))
    }
    #[must_use]
    pub fn length(&self) -> i128 {
        self.0.len() as i128
    }
    #[must_use]
    pub fn contains(&self, item: &T) -> bool {
        self.0.contains(item)
    }
    pub fn add(&mut self, item: T) {
        Arc::make_mut(&mut self.0).insert(item);
    }
    pub fn remove(&mut self, item: &T) -> bool {
        Arc::make_mut(&mut self.0).shift_remove(item)
    }
}
impl<T: Eq + Hash + Clone + 'static> IndexedIteration for Set<T> {
    type Item = T;
    fn item_at(&self, index: usize) -> Option<T> {
        self.0.get_index(index).cloned()
    }
}
impl<T: Eq + Hash + Clone + 'static> Iterable for Set<T> {
    type Item = T;
    type Iter = CollectionIterator<Self>;
    fn terrane_iterator(&self) -> Self::Iter {
        CollectionIterator::new(self.clone())
    }
}

fn stable_hash<T: Hash>(value: &T) -> u64 {
    let mut hasher = StableHasher::default();
    value.hash(&mut hasher);
    hasher.finish()
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct UnorderedMapData<K: Eq + Hash, V> {
    values: HashMap<K, V, FixedState>,
    iteration_keys: Vec<K>,
}

impl<K: Eq + Hash, V> UnorderedMapData<K, V> {
    fn indexed_value(&self, key: &K) -> &V {
        self.values.get(key).expect("indexed key must exist")
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UnorderedMap<K: Eq + Hash, V>(Arc<UnorderedMapData<K, V>>);

impl<K: Eq + Hash + Clone, V: Clone> UnorderedMap<K, V> {
    #[must_use]
    pub fn new(entries: Vec<Entry<K, V>>) -> Self {
        let mut values = HashMap::with_hasher(FixedState::default());
        let mut iteration_keys = Vec::new();
        for entry in entries {
            if !values.contains_key(&entry.key) {
                iteration_keys.push(entry.key.clone());
            }
            values.insert(entry.key, entry.value);
        }
        iteration_keys.sort_by_key(stable_hash);
        Self(Arc::new(UnorderedMapData {
            values,
            iteration_keys,
        }))
    }
    #[must_use]
    pub fn length(&self) -> i128 {
        self.0.values.len() as i128
    }
    #[must_use]
    pub fn get(&self, key: &K) -> Option<&V> {
        self.0.values.get(key)
    }
    /// Returns the mapped value or an error when the key is absent.
    ///
    /// # Errors
    /// Returns [`MissingKey`] when `key` is absent.
    pub fn get_or_error(&self, key: &K) -> Result<V, MissingKey> {
        self.get(key).cloned().ok_or(MissingKey)
    }
    pub fn set(&mut self, key: K, value: V) {
        let data = Arc::make_mut(&mut self.0);
        if !data.values.contains_key(&key) {
            data.iteration_keys.push(key.clone());
            data.iteration_keys.sort_by_key(stable_hash);
        }
        data.values.insert(key, value);
    }
    #[must_use]
    pub fn keys(&self) -> List<K> {
        List::new(self.0.iteration_keys.clone())
    }
    #[must_use]
    pub fn values(&self) -> List<V> {
        List::new(
            self.0
                .iteration_keys
                .iter()
                .map(|key| self.0.indexed_value(key).clone())
                .collect(),
        )
    }
    #[must_use]
    pub fn entries(&self) -> List<Entry<K, V>> {
        List::new(
            self.0
                .iteration_keys
                .iter()
                .map(|key| Entry::new(key.clone(), self.0.indexed_value(key).clone()))
                .collect(),
        )
    }
}

impl<K: Eq + Hash + Clone + 'static, V: Clone + 'static> IndexedIteration for UnorderedMap<K, V> {
    type Item = Entry<K, V>;
    fn item_at(&self, index: usize) -> Option<Self::Item> {
        let key = self.0.iteration_keys.get(index)?;
        Some(Entry::new(key.clone(), self.0.indexed_value(key).clone()))
    }
}

impl<K: Eq + Hash + Clone + 'static, V: Clone + 'static> Iterable for UnorderedMap<K, V> {
    type Item = Entry<K, V>;
    type Iter = CollectionIterator<Self>;
    fn terrane_iterator(&self) -> Self::Iter {
        CollectionIterator::new(self.clone())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct UnorderedSetData<T: Eq + Hash> {
    values: HashSet<T, FixedState>,
    iteration_items: Vec<T>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UnorderedSet<T: Eq + Hash>(Arc<UnorderedSetData<T>>);

impl<T: Eq + Hash + Clone> UnorderedSet<T> {
    #[must_use]
    pub fn new(items: Vec<T>) -> Self {
        let mut values = HashSet::with_hasher(FixedState::default());
        let mut iteration_items = Vec::new();
        for item in items {
            if values.insert(item.clone()) {
                iteration_items.push(item);
            }
        }
        iteration_items.sort_by_key(stable_hash);
        Self(Arc::new(UnorderedSetData {
            values,
            iteration_items,
        }))
    }
    #[must_use]
    pub fn length(&self) -> i128 {
        self.0.values.len() as i128
    }
    #[must_use]
    pub fn contains(&self, item: &T) -> bool {
        self.0.values.contains(item)
    }
    pub fn add(&mut self, item: T) {
        let data = Arc::make_mut(&mut self.0);
        if data.values.insert(item.clone()) {
            data.iteration_items.push(item);
            data.iteration_items.sort_by_key(stable_hash);
        }
    }
    pub fn remove(&mut self, item: &T) -> bool {
        let data = Arc::make_mut(&mut self.0);
        if !data.values.remove(item) {
            return false;
        }
        data.iteration_items.retain(|candidate| candidate != item);
        true
    }
}

impl<T: Eq + Hash + Clone + 'static> IndexedIteration for UnorderedSet<T> {
    type Item = T;
    fn item_at(&self, index: usize) -> Option<T> {
        self.0.iteration_items.get(index).cloned()
    }
}

impl<T: Eq + Hash + Clone + 'static> Iterable for UnorderedSet<T> {
    type Item = T;
    type Iter = CollectionIterator<Self>;
    fn terrane_iterator(&self) -> Self::Iter {
        CollectionIterator::new(self.clone())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Range {
    start: Int,
    end: Int,
    step: Int,
    inclusive: bool,
}
impl Range {
    /// Constructs a half-open range.
    ///
    /// # Errors
    /// Returns [`RangeStepError`] when `step` is zero.
    pub fn new(start: Int, end: Int, step: Int) -> Result<Self, RangeStepError> {
        if step == Int::from(0_i64) {
            return Err(RangeStepError);
        }
        Ok(Self {
            start,
            end,
            step,
            inclusive: false,
        })
    }
    /// Constructs an inclusive range.
    ///
    /// # Errors
    /// Returns [`RangeStepError`] when `step` is zero.
    pub fn through(start: Int, end: Int, step: Int) -> Result<Self, RangeStepError> {
        Self::new(start, end, step).map(|range| Self {
            inclusive: true,
            ..range
        })
    }
}
#[derive(Clone, Debug)]
pub struct RangeIterator {
    current: Int,
    end: Int,
    step: Int,
    inclusive: bool,
    ascending: bool,
    ended: bool,
}

impl RangeIterator {
    #[must_use]
    #[expect(
        clippy::should_implement_trait,
        reason = "Terrane iteration returns an explicit typed step rather than Rust Option"
    )]
    pub fn next(&mut self) -> IterationStep<Int> {
        if self.ended {
            return IterationStep::End;
        }
        let in_bounds = if self.ascending {
            self.current < self.end || (self.inclusive && self.current == self.end)
        } else {
            self.current > self.end || (self.inclusive && self.current == self.end)
        };
        if !in_bounds {
            self.ended = true;
            return IterationStep::End;
        }
        let item = self.current.clone();
        self.current = self.current.clone() + self.step.clone();
        IterationStep::Item(item)
    }
}

impl Iterable for Range {
    type Item = Int;
    type Iter = RangeIterator;
    fn terrane_iterator(&self) -> Self::Iter {
        RangeIterator {
            current: self.start.clone(),
            end: self.end.clone(),
            step: self.step.clone(),
            inclusive: self.inclusive,
            ascending: self.step > Int::from(0_i64),
            ended: false,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct IndexError {
    pub index: usize,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MissingKey;
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RangeStepError;
impl std::fmt::Display for IndexError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "index {} is out of range", self.index)
    }
}

impl std::error::Error for IndexError {}

impl std::fmt::Display for MissingKey {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("key is absent")
    }
}

impl std::error::Error for MissingKey {}
impl std::fmt::Display for RangeStepError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("range step is zero")
    }
}

impl std::error::Error for RangeStepError {}
/// Converts an adaptive integer to a collection index.
///
/// # Errors
/// Returns [`IndexError`] when `index` is negative or does not fit in `usize`.
pub fn index_from_int(index: &Int) -> Result<usize, IndexError> {
    index.as_usize().ok_or(IndexError { index: usize::MAX })
}

#[must_use]
pub fn string_iterator(value: &str) -> Iterator<String> {
    Iterator::new(value.graphemes(true).map(str::to_owned).collect())
}
#[must_use]
pub fn bytes_iterator(value: &[u8]) -> Iterator<u8> {
    Iterator::new(value.to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sticky_end_never_revisits_source() {
        let mut iterator = Iterator::new(vec![None::<u8>]);
        assert_eq!(iterator.next(), IterationStep::Item(None));
        assert_eq!(iterator.next(), IterationStep::End);
        assert_eq!(iterator.next(), IterationStep::End);
    }

    #[test]
    fn stable_hasher_owns_its_algorithm() {
        let mut hasher = StableHasher::default();
        hasher.write(b"terrane");
        assert_eq!(hasher.finish(), 0x3f87_dd9c_872a_eb2c);
    }

    #[test]
    fn collection_iteration_does_not_clone_items_before_advancing() {
        use std::sync::atomic::{AtomicUsize, Ordering};

        #[derive(Debug)]
        struct CountedClone(Arc<AtomicUsize>);

        impl Clone for CountedClone {
            fn clone(&self) -> Self {
                self.0.fetch_add(1, Ordering::Relaxed);
                Self(Arc::clone(&self.0))
            }
        }

        let clones = Arc::new(AtomicUsize::new(0));
        let list = List::new(vec![
            CountedClone(Arc::clone(&clones)),
            CountedClone(Arc::clone(&clones)),
        ]);
        let mut iterator = list.terrane_iterator();
        assert_eq!(clones.load(Ordering::Relaxed), 0);
        assert!(matches!(iterator.next(), IterationStep::Item(_)));
        assert_eq!(clones.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn list_assignment_separates_on_first_mutation() {
        let original = List::new(vec![1]);
        let mut copy = original.clone();
        copy.append(2);
        assert_eq!(original.length(), 1);
        assert_eq!(copy.length(), 2);
    }
}
