#[derive(Debug)]
struct PageEntry<K, T> {
    key: K,
    seq: usize,
    item: T,
}

#[cfg(feature = "app_api")]
#[derive(Clone)]
enum SortKeyValue {
    Text(String),
    Numeric(iroha_primitives::numeric::Numeric),
}

#[cfg(feature = "app_api")]
impl SortKeyValue {
    fn variant_ord(&self) -> usize {
        match self {
            SortKeyValue::Text(_) => 0,
            SortKeyValue::Numeric(_) => 1,
        }
    }
}

#[cfg(feature = "app_api")]
impl From<String> for SortKeyValue {
    fn from(value: String) -> Self {
        SortKeyValue::Text(value)
    }
}

#[cfg(feature = "app_api")]
impl From<&String> for SortKeyValue {
    fn from(value: &String) -> Self {
        SortKeyValue::Text(value.clone())
    }
}

#[cfg(feature = "app_api")]
impl From<&str> for SortKeyValue {
    fn from(value: &str) -> Self {
        SortKeyValue::Text(value.to_owned())
    }
}

#[cfg(feature = "app_api")]
impl From<iroha_primitives::numeric::Numeric> for SortKeyValue {
    fn from(value: iroha_primitives::numeric::Numeric) -> Self {
        SortKeyValue::Numeric(value)
    }
}

#[cfg(feature = "app_api")]
impl From<&iroha_primitives::numeric::Numeric> for SortKeyValue {
    fn from(value: &iroha_primitives::numeric::Numeric) -> Self {
        SortKeyValue::Numeric(value.clone())
    }
}

#[cfg(feature = "app_api")]
impl PartialEq for SortKeyValue {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (SortKeyValue::Text(lhs), SortKeyValue::Text(rhs)) => lhs == rhs,
            (SortKeyValue::Numeric(lhs), SortKeyValue::Numeric(rhs)) => lhs == rhs,
            _ => false,
        }
    }
}

#[cfg(feature = "app_api")]
impl Eq for SortKeyValue {}

#[cfg(feature = "app_api")]
impl Ord for SortKeyValue {
    fn cmp(&self, other: &Self) -> Ordering {
        match (self, other) {
            (SortKeyValue::Text(lhs), SortKeyValue::Text(rhs)) => lhs.cmp(rhs),
            (SortKeyValue::Numeric(lhs), SortKeyValue::Numeric(rhs)) => lhs.cmp(rhs),
            _ => self.variant_ord().cmp(&other.variant_ord()),
        }
    }
}

#[cfg(feature = "app_api")]
impl PartialOrd for SortKeyValue {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

#[cfg(feature = "app_api")]
#[derive(Clone, Eq, PartialEq)]
struct SortKeyComponent {
    value: SortKeyValue,
    ascending: bool,
}

#[cfg(feature = "app_api")]
impl SortKeyComponent {
    fn asc<V: Into<SortKeyValue>>(value: V) -> Self {
        Self {
            value: value.into(),
            ascending: true,
        }
    }

    fn desc<V: Into<SortKeyValue>>(value: V) -> Self {
        Self {
            value: value.into(),
            ascending: false,
        }
    }
}

#[cfg(feature = "app_api")]
#[derive(Clone, Eq, PartialEq)]
struct MultiSortKey {
    components: Vec<SortKeyComponent>,
}

#[cfg(feature = "app_api")]
impl MultiSortKey {
    fn new(components: Vec<SortKeyComponent>) -> Self {
        Self { components }
    }

    fn push(&mut self, component: SortKeyComponent) {
        self.components.push(component);
    }

    fn is_empty(&self) -> bool {
        self.components.is_empty()
    }
}

#[cfg(feature = "app_api")]
impl Ord for MultiSortKey {
    fn cmp(&self, other: &Self) -> Ordering {
        for (lhs, rhs) in self.components.iter().zip(other.components.iter()) {
            let ord = if lhs.ascending {
                lhs.value.cmp(&rhs.value)
            } else {
                rhs.value.cmp(&lhs.value)
            };
            if !ord.is_eq() {
                return ord;
            }
        }
        self.components.len().cmp(&other.components.len())
    }
}

#[cfg(feature = "app_api")]
impl PartialOrd for MultiSortKey {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
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
