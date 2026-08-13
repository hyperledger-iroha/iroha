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
fn fail_next_bound_progress_intent_file_sync_for_tests() {
    FAIL_NEXT_BOUND_PROGRESS_INTENT_FILE_SYNC.with(|flag| flag.set(true));
}
#[cfg(test)]
fn fail_next_bound_progress_append_data_sync_for_tests() {
    FAIL_NEXT_BOUND_PROGRESS_APPEND_DATA_SYNC.with(|flag| flag.set(true));
}
#[cfg(test)]
fn fail_next_bound_progress_append_index_sync_for_tests() {
    FAIL_NEXT_BOUND_PROGRESS_APPEND_INDEX_SYNC.with(|flag| flag.set(true));
}
#[cfg(test)]
fn fail_next_native_amx_latest_index_recovery_temp_sync_for_tests() {
    FAIL_NEXT_NATIVE_AMX_LATEST_INDEX_RECOVERY_TEMP_SYNC.with(|flag| flag.set(true));
}
#[cfg(test)]
fn fail_bound_progress_intent_directory_sync_for_tests(
    calls_before_failure: usize,
    target_index: usize,
) {
    FAIL_BOUND_PROGRESS_INTENT_DIRECTORY_SYNC.with(|fault| {
        fault.set(Some(ProgressIntentDirectorySyncFault {
            calls_before_failure,
            target_index,
        }));
    });
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
#[cfg(test)]
fn unique_retired_path(base: &Path, stem: &str, extension: Option<&str>) -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|dur| dur.as_secs())
        .unwrap_or(0);
    let mut counter = 0u32;
    loop {
        let mut name = format!("{stem}_{stamp}");
        if counter > 0 {
            name.push('_');
            name.push_str(&counter.to_string());
        }
        if let Some(ext) = extension {
            name.push('.');
            name.push_str(ext);
        }
        let candidate = base.join(&name);
        if !candidate.exists() {
            return candidate;
        }
        counter = counter.saturating_add(1);
    }
}
