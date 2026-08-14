#[cfg(all(feature = "app_api", any(test, feature = "ws_integration_tests")))]
fn filter_report_summary(
    summary: &ProverReportSummary,
    q: &ProverListQuery,
    requested_id: Option<&str>,
    ok_req: bool,
    failed_req: bool,
) -> bool {
    if let Some(req_id) = requested_id {
        if summary.id != req_id {
            return false;
        }
    }
    if let Some(ct) = q.content_type.as_deref() {
        if !summary.content_type.contains(ct) {
            return false;
        }
    }
    if let Some(tag) = q.has_tag.as_deref() {
        let has_tag = summary
            .zk1_tags
            .as_ref()
            .map(|tags| tags.iter().any(|existing| existing == tag))
            .unwrap_or(false);
        if !has_tag {
            return false;
        }
    }
    if !q.since_ms.map_or(true, |th| summary.processed_ms >= th) {
        return false;
    }
    if !q.before_ms.map_or(true, |th| summary.processed_ms <= th) {
        return false;
    }
    match (ok_req, failed_req) {
        (true, false) => summary.ok,
        (false, true) => !summary.ok,
        _ => true,
    }
}
#[cfg(all(feature = "app_api", any(test, feature = "ws_integration_tests")))]
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct ReportOrderKey {
    processed_ms: u64,
    id: String,
}
#[cfg(all(feature = "app_api", any(test, feature = "ws_integration_tests")))]
enum BoundedReportKeys {
    Asc(BinaryHeap<ReportOrderKey>),
    Desc(BinaryHeap<std::cmp::Reverse<ReportOrderKey>>),
}
#[cfg(all(feature = "app_api", any(test, feature = "ws_integration_tests")))]
impl BoundedReportKeys {
    fn new(descending: bool) -> Self {
        if descending {
            Self::Desc(BinaryHeap::new())
        } else {
            Self::Asc(BinaryHeap::new())
        }
    }
    fn consider(&mut self, key: ReportOrderKey, capacity: usize) {
        if capacity == 0 {
            return;
        }
        match self {
            Self::Asc(heap) => {
                if heap.len() < capacity {
                    heap.push(key);
                } else if heap.peek().is_some_and(|largest| key < *largest) {
                    let _ = heap.pop();
                    heap.push(key);
                }
            }
            Self::Desc(heap) => {
                let key = std::cmp::Reverse(key);
                if heap.len() < capacity {
                    heap.push(key);
                } else if heap.peek().is_some_and(|smallest| key < *smallest) {
                    let _ = heap.pop();
                    heap.push(key);
                }
            }
        }
    }
    fn into_ordered(self) -> Vec<ReportOrderKey> {
        match self {
            Self::Asc(heap) => heap.into_sorted_vec(),
            Self::Desc(heap) => heap
                .into_sorted_vec()
                .into_iter()
                .map(|key| key.0)
                .collect(),
        }
    }
}
#[cfg(all(feature = "app_api", any(test, feature = "ws_integration_tests")))]
fn report_query_window(q: &ProverListQuery) -> Result<(usize, usize), &'static str> {
    if q.latest.unwrap_or(false) {
        return Ok((0, 1));
    }
    let offset = q.offset.unwrap_or(0) as usize;
    if offset > REPORT_QUERY_MAX_OFFSET {
        return Err("report offset exceeds the deterministic pagination ceiling");
    }
    let limit = q
        .limit
        .map_or(REPORT_QUERY_DEFAULT_LIMIT, |limit| limit as usize)
        .min(REPORT_QUERY_MAX_LIMIT);
    Ok((offset, limit))
}
#[cfg(all(feature = "app_api", any(test, feature = "ws_integration_tests")))]
fn select_report_summaries_locked(
    q: &ProverListQuery,
    requested_id: Option<&str>,
    ok_req: bool,
    failed_req: bool,
) -> Result<Vec<ProverReportSummary>, &'static str> {
    let (offset, limit) = report_query_window(q)?;
    if limit == 0 {
        return Ok(Vec::new());
    }
    let capacity = offset.saturating_add(limit);
    let descending =
        q.latest.unwrap_or(false) || matches!(q.order.as_deref(), Some("desc" | "DESC" | "Desc"));
    let mut keys = BoundedReportKeys::new(descending);
    let mut consider = |summary: ProverReportSummary| {
        if filter_report_summary(&summary, q, requested_id, ok_req, failed_req) {
            keys.consider(
                ReportOrderKey {
                    processed_ms: summary.processed_ms,
                    id: summary.id,
                },
                capacity,
            );
        }
        true
    };
    if let Some(id) = requested_id {
        if let Some(summary) = load_or_repair_report_summary_locked(id) {
            let _ = consider(summary);
        }
    } else {
        visit_report_summaries_locked(consider);
    }
    let mut selected = Vec::with_capacity(limit);
    for key in keys.into_ordered().into_iter().skip(offset).take(limit) {
        if let Some(summary) = load_or_repair_report_summary_locked(&key.id) {
            selected.push(summary);
        }
    }
    Ok(selected)
}
#[cfg(all(feature = "app_api", any(test, feature = "ws_integration_tests")))]
fn select_report_summaries(
    q: &ProverListQuery,
    requested_id: Option<&str>,
    ok_req: bool,
    failed_req: bool,
) -> Result<Vec<ProverReportSummary>, &'static str> {
    let _guard = report_summary_lock().lock();
    select_report_summaries_locked(q, requested_id, ok_req, failed_req)
}
#[cfg(all(feature = "app_api", any(test, feature = "ws_integration_tests")))]
fn count_report_summaries(
    q: &ProverListQuery,
    requested_id: Option<&str>,
    ok_req: bool,
    failed_req: bool,
) -> u64 {
    let _guard = report_summary_lock().lock();
    let mut count = 0_u64;
    let mut count_one = |summary: ProverReportSummary| {
        if filter_report_summary(&summary, q, requested_id, ok_req, failed_req) {
            count = count.saturating_add(1);
        }
        true
    };
    if let Some(id) = requested_id {
        if let Some(summary) = load_or_repair_report_summary_locked(id) {
            let _ = count_one(summary);
        }
    } else {
        visit_report_summaries_locked(count_one);
    }
    count
}
#[cfg(all(feature = "app_api", any(test, feature = "ws_integration_tests")))]
fn encode_full_report_page(summaries: Vec<ProverReportSummary>) -> Result<String, &'static str> {
    let mut output = String::from("[");
    for summary in summaries {
        let Some(report) = load_report(&summary.id) else {
            continue;
        };
        let encoded = norito::json::to_json(&report).map_err(|_| "failed to encode report")?;
        let separator_bytes = if output.len() > 1 { 1 } else { 0 };
        let projected = output
            .len()
            .saturating_add(separator_bytes)
            .saturating_add(encoded.len())
            .saturating_add(1);
        if projected > REPORT_QUERY_MAX_RESPONSE_BYTES {
            return Err("selected prover reports exceed the bounded response size");
        }
        if separator_bytes != 0 {
            output.push(',');
        }
        output.push_str(&encoded);
    }
    output.push(']');
    Ok(output)
}
#[cfg(all(feature = "app_api", any(test, feature = "ws_integration_tests")))]
fn validate_zk1_tag_filter(q: &ProverListQuery) -> Result<(), &'static str> {
    let Some(tag) = q.has_tag.as_deref() else {
        return Ok(());
    };
    if tag.len() == 4 && tag.as_bytes().iter().all(u8::is_ascii_graphic) {
        Ok(())
    } else {
        Err("invalid ZK1 tag filter (expected exactly four printable ASCII characters)")
    }
}
