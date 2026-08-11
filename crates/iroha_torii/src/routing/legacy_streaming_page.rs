#[cfg(feature = "app_api")]
fn collect_page_streaming<K, T, I>(
    iter: I,
    offset: u64,
    limit: Option<u64>,
    cap: Option<u64>,
) -> (Vec<T>, usize)
where
    I: IntoIterator<Item = (K, T)>,
    K: Ord,
{
    let offset_usize = if offset > usize::MAX as u64 {
        usize::MAX
    } else {
        offset as usize
    };
    let limit_usize = limit
        .filter(|&lim| lim > 0)
        .map(|lim| cap.map_or(lim, |c| lim.min(c)))
        .map(|lim| lim.min(usize::MAX as u64) as usize);
    let page_cap = limit_usize.map(|lim| offset_usize.saturating_add(lim));

    let mut matched: usize = 0;
    let mut seq: usize = 0;
    let mut heap: BinaryHeap<PageEntry<K, T>> = BinaryHeap::new();
    let mut collected: Vec<PageEntry<K, T>> = Vec::new();

    for (key, item) in iter.into_iter() {
        let entry = PageEntry { key, seq, item };
        seq = seq.wrapping_add(1);
        matched = matched.saturating_add(1);
        if let Some(capacity) = page_cap {
            heap.push(entry);
            if heap.len() > capacity {
                heap.pop();
            }
        } else {
            collected.push(entry);
        }
    }

    let mut entries = if page_cap.is_some() {
        heap.into_vec()
    } else {
        collected
    };

    entries.sort_by(|a, b| match a.key.cmp(&b.key) {
        Ordering::Equal => a.seq.cmp(&b.seq),
        ord => ord,
    });

    let skip = offset_usize.min(entries.len());
    let mut page: Vec<T> = Vec::new();
    for entry in entries.into_iter().skip(skip) {
        if let Some(lim) = limit_usize {
            if page.len() >= lim {
                break;
            }
        }
        page.push(entry.item);
    }

    (page, matched)
}
