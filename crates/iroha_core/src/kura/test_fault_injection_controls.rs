#[cfg(test)]
fn fail_next_sidecar_promotion_dir_sync_for_tests() {
    FAIL_NEXT_SIDECAR_PROMOTION_DIR_SYNC.with(|flag| flag.set(true));
}
#[cfg(test)]
fn fail_next_sidecar_temp_marker_dir_sync_for_tests() {
    FAIL_NEXT_SIDECAR_TEMP_MARKER_DIR_SYNC.with(|flag| flag.set(true));
}
#[cfg(test)]
fn fail_next_indexed_sidecar_data_sync_for_tests() {
    FAIL_NEXT_INDEXED_SIDECAR_DATA_SYNC.with(|flag| flag.set(true));
}
#[cfg(test)]
fn fail_next_indexed_sidecar_initial_data_sync_for_tests() {
    FAIL_NEXT_INDEXED_SIDECAR_INITIAL_DATA_SYNC.with(|flag| flag.set(true));
}
#[cfg(test)]
fn fail_next_indexed_sidecar_index_sync_for_tests() {
    FAIL_NEXT_INDEXED_SIDECAR_INDEX_SYNC.with(|flag| flag.set(true));
}
#[cfg(test)]
fn fail_next_indexed_sidecar_dir_sync_for_tests() {
    FAIL_NEXT_INDEXED_SIDECAR_DIR_SYNC.with(|flag| flag.set(true));
}
#[cfg(test)]
fn fail_progress_sidecar_ancestor_sync_at_for_tests(ancestor_index: usize) {
    fail_progress_sidecar_ancestor_sync_for_tests(ancestor_index, 1);
}
#[cfg(test)]
fn fail_progress_sidecar_ancestor_sync_for_tests(ancestor_index: usize, failures_remaining: usize) {
    assert!(
        failures_remaining > 0,
        "fault injection count must be non-zero"
    );
    FAIL_PROGRESS_SIDECAR_ANCESTOR_SYNC_AT.with(|fault| {
        fault.set(Some(ProgressAncestorSyncFault {
            target_index: ancestor_index,
            remaining_to_target: ancestor_index,
            failures_remaining,
        }));
    });
}




