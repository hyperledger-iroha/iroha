#[derive(Debug)]
struct PageEntry<K, T> {
    key: K,
    seq: usize,
    item: T,
}
impl<K: Ord, T> PartialEq for PageEntry<K, T> {
    fn eq(&self, other: &Self) -> bool {
        self.seq == other.seq && self.key == other.key
    }
}
impl<K: Ord, T> Eq for PageEntry<K, T> {}
impl<K: Ord, T> PartialOrd for PageEntry<K, T> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl<K: Ord, T> Ord for PageEntry<K, T> {
    fn cmp(&self, other: &Self) -> Ordering {
        match self.key.cmp(&other.key) {
            Ordering::Equal => self.seq.cmp(&other.seq),
            ord => ord,
        }
    }
}
fn collect_bounded_ranked_page<K, T, I>(
    iter: I,
    offset: usize,
    limit: usize,
    capacity: usize,
) -> (Vec<T>, usize)
where
    I: IntoIterator<Item = (K, T)>,
    K: Ord,
{
    debug_assert_eq!(offset.checked_add(limit), Some(capacity));
    let mut matched = 0usize;
    let mut seq = 0usize;
    let mut heap = BinaryHeap::with_capacity(capacity);
    for (key, item) in iter {
        matched = matched.saturating_add(1);
        heap.push(PageEntry { key, seq, item });
        seq = seq.saturating_add(1);
        if heap.len() > capacity {
            heap.pop();
        }
    }
    let mut entries = heap.into_vec();
    entries.sort_by(|left, right| match left.key.cmp(&right.key) {
        Ordering::Equal => left.seq.cmp(&right.seq),
        order => order,
    });
    let page = entries
        .into_iter()
        .skip(offset)
        .take(limit)
        .map(|entry| entry.item)
        .collect();
    (page, matched)
}
