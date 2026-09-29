#[cfg(test)]
#[derive(Clone, Copy)]
struct ProgressAncestorSyncFault {
    target_index: usize,
    remaining_to_target: usize,
    failures_remaining: usize,
}
#[cfg(test)]
#[derive(Clone, Copy)]
struct ProgressIntentDirectorySyncFault {
    calls_before_failure: usize,
    target_index: usize,
}
#[cfg(test)]
std::thread_local! {
    static FAIL_NEXT_SIDECAR_PROMOTION_DIR_SYNC: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static FAIL_NEXT_SIDECAR_TEMP_MARKER_DIR_SYNC: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static FAIL_NEXT_INDEXED_SIDECAR_DATA_SYNC: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static FAIL_NEXT_INDEXED_SIDECAR_INITIAL_DATA_SYNC: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static FAIL_NEXT_INDEXED_SIDECAR_INDEX_SYNC: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static FAIL_NEXT_INDEXED_SIDECAR_DIR_SYNC: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static FAIL_BOUND_PROGRESS_INTENT_DIRECTORY_SYNC: std::cell::Cell<Option<ProgressIntentDirectorySyncFault>> = const { std::cell::Cell::new(None) };
    static FAIL_PROGRESS_SIDECAR_ANCESTOR_SYNC_AT: std::cell::Cell<Option<ProgressAncestorSyncFault>> = const { std::cell::Cell::new(None) };
}

