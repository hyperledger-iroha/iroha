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
/// Fail the next indexed append data sync after its payload write in recovery tests.
#[cfg(test)]
pub(crate) fn fail_next_bound_progress_append_data_sync_for_tests() {
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
/// Fail a selected retained intent directory sync in focused publication tests.
#[cfg(test)]
pub(crate) fn fail_bound_progress_intent_directory_sync_for_tests(
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
fn fail_after_next_native_amx_evidence_temp_prefix_for_tests(bytes: usize) {
    FAIL_AFTER_NEXT_NATIVE_AMX_EVIDENCE_TEMP_PREFIX.with(|flag| flag.set(Some(bytes)));
}

#[cfg(test)]
fn fail_after_next_native_amx_evidence_temp_sync_for_tests() {
    FAIL_AFTER_NEXT_NATIVE_AMX_EVIDENCE_TEMP_SYNC.with(|flag| flag.set(true));
}

#[cfg(test)]
fn fail_after_next_native_amx_publication_temp_prefix_at_path_for_tests(
    path: PathBuf,
    bytes: usize,
) {
    FAIL_AFTER_NEXT_NATIVE_AMX_PUBLICATION_TEMP_PREFIX_AT_PATH.with(|flag| {
        *flag.borrow_mut() = Some((path, bytes));
    });
}

#[cfg(test)]
impl Kura {
    fn fail_native_amx_publication_temp_prefix_at_path_for_tests(
        &self,
        temporary: &mut std::fs::File,
        temp_path: &Path,
        bytes: &[u8],
    ) -> Result<()> {
        let cut = FAIL_AFTER_NEXT_NATIVE_AMX_PUBLICATION_TEMP_PREFIX_AT_PATH.with(|flag| {
            let mut flag = flag.borrow_mut();
            if flag.as_ref().is_some_and(|(path, _)| path == temp_path) {
                flag.take().map(|(_, len)| len)
            } else {
                None
            }
        });
        if let Some(len) = cut {
            assert!(
                len < bytes.len(),
                "crash cut must precede the complete artifact"
            );
            temporary
                .write_all(&bytes[..len])
                .map_err(|error| Error::IO(error, temp_path.to_path_buf()))?;
            return Err(Error::IO(
                std::io::Error::other(
                    "injected Native evidence interruption during temporary write",
                ),
                temp_path.to_path_buf(),
            ));
        }
        Ok(())
    }
}
