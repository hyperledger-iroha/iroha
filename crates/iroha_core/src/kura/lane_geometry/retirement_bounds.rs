// Bounded lane-retirement inventory arithmetic.

/// Bound the aggregate retirement scan without treating legitimate route
/// multiplicity as corruption.
///
/// Each route can retain six ordinary histories (autonomous payload, input,
/// preflight, certificate, canonical merge bundle, and application receipt), plus two Native evidence
/// artifact families sharing one configured byte bound. Ordinary histories may also contain
/// the globally bounded pending-merge depth beyond their terminal frontier.
/// Historical autonomous recovery contributes one additional globally bounded
/// record inventory rather than a per-route multiplier.
/// Startup recovery may admit one entry beyond the compact Native window, but
/// retirement runs only after startup repair and therefore accepts exactly the
/// configured retained record count.
fn lane_retirement_aggregate_work_item_limit(
    route_count: usize,
    regular_retention: usize,
    native_retention: usize,
    pending_work_allowance: usize,
) -> Option<usize> {
    let regular_per_route = regular_retention
        .checked_add(pending_work_allowance)?
        .checked_mul(LANE_RETIREMENT_REGULAR_SIDECARS_PER_ROUTE)?;
    let native_per_route =
        native_retention.checked_mul(LANE_RETIREMENT_NATIVE_SIDECARS_PER_ROUTE)?;
    route_count
        .checked_mul(regular_per_route.checked_add(native_per_route)?)?
        .checked_add(HISTORICAL_AUTONOMOUS_RECOVERY_MAX_RECORDS)
}

/// Bound one route's complete retirement artifact namespace.
fn lane_retirement_per_route_artifact_file_limit(native_retention: usize) -> Option<usize> {
    native_retention
        .checked_mul(LANE_RETIREMENT_NATIVE_SIDECARS_PER_ROUTE)?
        .checked_add(MAX_AUTONOMOUS_LANE_ATTEMPT_NAMESPACE_FILES)?
        .checked_add(LANE_RETIREMENT_FIXED_ARTIFACT_FILES_PER_ROUTE)?
        .checked_add(LANE_RETIREMENT_HISTORICAL_RECOVERY_NAMESPACES_PER_ROUTE)
}

fn accumulate_lane_retirement_historical_recovery_records(
    current: usize,
    additional: usize,
) -> Option<usize> {
    current
        .checked_add(additional)
        .filter(|total| *total <= HISTORICAL_AUTONOMOUS_RECOVERY_MAX_RECORDS)
}

fn remaining_lane_retirement_historical_recovery_budget(
    records_seen: usize,
    bytes_seen: u64,
    aggregate_byte_limit: u64,
) -> Option<(usize, u64)> {
    Some((
        HISTORICAL_AUTONOMOUS_RECOVERY_MAX_RECORDS.checked_sub(records_seen)?,
        aggregate_byte_limit.checked_sub(bytes_seen)?,
    ))
}

/// Return whether Native manifest and receipt windows are the same complete,
/// contiguous retained suffix.
///
/// Publication may transiently leave one highest half-pair, but retirement
/// and archive validation run only after repair and pair pruning. Neither path
/// may accept family-skewed or punctured evidence.
fn native_amx_retained_windows_are_complete(
    manifest_heights: &BTreeSet<u64>,
    receipt_heights: &BTreeSet<u64>,
) -> bool {
    manifest_heights == receipt_heights
        && manifest_heights
            .iter()
            .copied()
            .collect::<Vec<_>>()
            .windows(2)
            .all(|pair| pair[0].checked_add(1) == Some(pair[1]))
}
