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

impl<T: Clone> Iterator<T> {
    #[must_use]
    pub fn new(items: Vec<T>) -> Self {
        Self {
            items: Arc::new(items),
            index: 0,
            ended: false,
        }
    }

    #[must_use]
    #[expect(
        clippy::should_implement_trait,
        reason = "Terrane iteration returns an explicit typed step rather than Rust Option"
    )]
    pub fn next(&mut self) -> IterationStep<T> {
        if self.ended {
            return IterationStep::End;
        }
        if let Some(item) = self.items.get(self.index).cloned() {
            self.index += 1;
            IterationStep::Item(item)
        } else {
            self.ended = true;
            IterationStep::End
        }
    }
}

pub trait Iterable {
    type Item: Clone;
    fn terrane_iterator(&self) -> Iterator<Self::Item>;
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
    /// Returns [`IndexError`] when `index` is out of range.
    pub fn set(&mut self, index: usize, value: T) -> Result<Self, IndexError> {
        let Some(slot) = Arc::make_mut(&mut self.0).get_mut(index) else {
            return Err(IndexError { index });
        };
        *slot = value;
        Ok(self.clone())
    }
    #[expect(
        clippy::return_self_not_must_use,
        reason = "Terrane collection mutator results may be intentionally discarded"
    )]
    pub fn append(&mut self, value: T) -> Self {
        Arc::make_mut(&mut self.0).push(value);
        self.clone()
    }
}

impl<T: Clone> Iterable for List<T> {
    type Item = T;
    fn terrane_iterator(&self) -> Iterator<T> {
        Iterator::new(self.0.as_ref().clone())
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
impl<T: Clone> Iterable for Tuple<T> {
    type Item = T;
    fn terrane_iterator(&self) -> Iterator<T> {
        Iterator::new(self.0.as_ref().clone())
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Map<K, V>(Arc<Vec<Entry<K, V>>>);
impl<K: Eq + Clone, V: Clone> Map<K, V> {
    #[must_use]
    pub fn new(entries: Vec<Entry<K, V>>) -> Self {
        Self(Arc::new(entries))
    }
    #[must_use]
    pub fn length(&self) -> i128 {
        self.0.len() as i128
    }
    #[must_use]
    pub fn get(&self, key: &K) -> Option<&V> {
        self.0
            .iter()
            .find(|entry| &entry.key == key)
            .map(|entry| &entry.value)
    }
    /// Returns the mapped value or an error when the key is absent.
    ///
    /// # Errors
    /// Returns [`MissingKey`] when `key` is absent.
    pub fn get_or_error(&self, key: &K) -> Result<V, MissingKey> {
        self.get(key).cloned().ok_or(MissingKey)
    }
    #[expect(
        clippy::return_self_not_must_use,
        reason = "Terrane collection mutator results may be intentionally discarded"
    )]
    pub fn set(&mut self, key: K, value: V) -> Self {
        let entries = Arc::make_mut(&mut self.0);
        if let Some(entry) = entries.iter_mut().find(|entry| entry.key == key) {
            entry.value = value;
        } else {
            entries.push(Entry::new(key, value));
        }
        self.clone()
    }
    #[must_use]
    pub fn keys(&self) -> List<K> {
        List::new(self.0.iter().map(|entry| entry.key.clone()).collect())
    }
    #[must_use]
    pub fn values(&self) -> List<V> {
        List::new(self.0.iter().map(|entry| entry.value.clone()).collect())
    }
    #[must_use]
    pub fn entries(&self) -> List<Entry<K, V>> {
        List::new(self.0.as_ref().clone())
    }
}
impl<K: Eq + Clone, V: Clone> Iterable for Map<K, V> {
    type Item = Entry<K, V>;
    fn terrane_iterator(&self) -> Iterator<Self::Item> {
        Iterator::new(self.0.as_ref().clone())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Set<T>(Arc<Vec<T>>);
impl<T: Eq + Clone> Set<T> {
    #[must_use]
    pub fn new(items: Vec<T>) -> Self {
        let mut unique = Vec::new();
        for item in items {
            if !unique.contains(&item) {
                unique.push(item);
            }
        }
        Self(Arc::new(unique))
    }
    #[must_use]
    pub fn length(&self) -> i128 {
        self.0.len() as i128
    }
    #[must_use]
    pub fn contains(&self, item: &T) -> bool {
        self.0.contains(item)
    }
    #[expect(
        clippy::return_self_not_must_use,
        reason = "Terrane collection mutator results may be intentionally discarded"
    )]
    pub fn add(&mut self, item: T) -> Self {
        if !self.0.contains(&item) {
            Arc::make_mut(&mut self.0).push(item);
        }
        self.clone()
    }
    pub fn remove(&mut self, item: &T) -> bool {
        let items = Arc::make_mut(&mut self.0);
        let Some(index) = items.iter().position(|candidate| candidate == item) else {
            return false;
        };
        items.remove(index);
        true
    }
}
impl<T: Eq + Clone> Iterable for Set<T> {
    type Item = T;
    fn terrane_iterator(&self) -> Iterator<T> {
        Iterator::new(self.0.as_ref().clone())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UnorderedMap<K, V>(Map<K, V>);
impl<K: Eq + Clone, V: Clone> UnorderedMap<K, V> {
    #[must_use]
    pub fn new(entries: Vec<Entry<K, V>>) -> Self {
        Self(Map::new(entries))
    }
    #[must_use]
    pub fn length(&self) -> i128 {
        self.0.length()
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
        self.0.get_or_error(key)
    }
    #[expect(
        clippy::return_self_not_must_use,
        reason = "Terrane collection mutator results may be intentionally discarded"
    )]
    pub fn set(&mut self, key: K, value: V) -> Self {
        let _ = self.0.set(key, value);
        self.clone()
    }
    #[must_use]
    pub fn keys(&self) -> List<K> {
        self.0.keys()
    }
    #[must_use]
    pub fn values(&self) -> List<V> {
        self.0.values()
    }
    #[must_use]
    pub fn entries(&self) -> List<Entry<K, V>> {
        self.0.entries()
    }
}
impl<K: Eq + Clone, V: Clone> Iterable for UnorderedMap<K, V> {
    type Item = Entry<K, V>;
    fn terrane_iterator(&self) -> Iterator<Self::Item> {
        self.0.terrane_iterator()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UnorderedSet<T>(Set<T>);
impl<T: Eq + Clone> UnorderedSet<T> {
    #[must_use]
    pub fn new(items: Vec<T>) -> Self {
        Self(Set::new(items))
    }
    #[must_use]
    pub fn length(&self) -> i128 {
        self.0.length()
    }
    #[must_use]
    pub fn contains(&self, item: &T) -> bool {
        self.0.contains(item)
    }
    #[expect(
        clippy::return_self_not_must_use,
        reason = "Terrane collection mutator results may be intentionally discarded"
    )]
    pub fn add(&mut self, item: T) -> Self {
        let _ = self.0.add(item);
        self.clone()
    }
    pub fn remove(&mut self, item: &T) -> bool {
        self.0.remove(item)
    }
}
impl<T: Eq + Clone> Iterable for UnorderedSet<T> {
    type Item = T;
    fn terrane_iterator(&self) -> Iterator<T> {
        self.0.terrane_iterator()
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
impl Iterable for Range {
    type Item = Int;
    fn terrane_iterator(&self) -> Iterator<Int> {
        let zero = Int::from(0_i64);
        let ascending = self.step > zero;
        if (ascending && self.start > self.end) || (!ascending && self.start < self.end) {
            return Iterator::new(Vec::new());
        }
        let mut values = Vec::new();
        let mut current = self.start.clone();
        while if ascending {
            current < self.end || (self.inclusive && current == self.end)
        } else {
            current > self.end || (self.inclusive && current == self.end)
        } {
            values.push(current.clone());
            current = current + self.step.clone();
        }
        Iterator::new(values)
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
    fn list_assignment_separates_on_first_mutation() {
        let original = List::new(vec![1]);
        let mut copy = original.clone();
        let _ = copy.append(2);
        assert_eq!(original.length(), 1);
        assert_eq!(copy.length(), 2);
    }
}
