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
struct NativeAmxPrunePreUnlinkHook {
    calls_before_run: usize,
    hook: Option<Box<dyn FnOnce(&Path)>>,
}
#[cfg(test)]
std::thread_local! {
    static FAIL_NEXT_SIDECAR_PROMOTION_DIR_SYNC: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static FAIL_NEXT_SIDECAR_TEMP_MARKER_DIR_SYNC: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static FAIL_NEXT_INDEXED_SIDECAR_DATA_SYNC: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static FAIL_NEXT_INDEXED_SIDECAR_INITIAL_DATA_SYNC: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static FAIL_NEXT_INDEXED_SIDECAR_INDEX_SYNC: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static FAIL_NEXT_INDEXED_SIDECAR_DIR_SYNC: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static FAIL_NEXT_BOUND_PROGRESS_INTENT_FILE_SYNC: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static FAIL_AFTER_BOUND_PROGRESS_APPEND_BUILD_CALLS: std::cell::Cell<Option<usize>> = const { std::cell::Cell::new(None) };
    static FAIL_NEXT_BOUND_PROGRESS_APPEND_DATA_SYNC: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static FAIL_NEXT_BOUND_PROGRESS_APPEND_INDEX_SYNC: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static FAIL_NEXT_NATIVE_AMX_LATEST_INDEX_RECOVERY_TEMP_SYNC: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static FAIL_BOUND_PROGRESS_INTENT_DIRECTORY_SYNC: std::cell::Cell<Option<ProgressIntentDirectorySyncFault>> = const { std::cell::Cell::new(None) };
    static FAIL_PROGRESS_SIDECAR_ANCESTOR_SYNC_AT: std::cell::Cell<Option<ProgressAncestorSyncFault>> = const { std::cell::Cell::new(None) };
    static FAIL_NEXT_CERTIFIED_LANE_BLOCK_ARTIFACT_VALIDATION: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static FAIL_AFTER_NEXT_CERTIFIED_FRONTIER_BUILD: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static FAIL_AFTER_NEXT_AUTONOMOUS_CERTIFIED_FRONTIER: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static FAIL_NEXT_AUTONOMOUS_MERGE_BUNDLE_PERSISTENCE: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static FAIL_NEXT_AUTONOMOUS_MERGE_BUNDLE_APPEND_DATA_SYNC: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static FAIL_AFTER_NEXT_AUTONOMOUS_MERGE_BUNDLE_PAIR: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static LATEST_CERTIFIED_FRONTIER_POST_VALIDATION_HOOK: std::cell::RefCell<Option<Box<dyn FnOnce()>>> = const { std::cell::RefCell::new(None) };
    static NATIVE_AMX_PRUNE_PRE_UNLINK_HOOK: std::cell::RefCell<Option<NativeAmxPrunePreUnlinkHook>> = const { std::cell::RefCell::new(None) };
    static NATIVE_AMX_LATEST_INDEX_PRE_MUTATION_HOOK: std::cell::RefCell<Option<Box<dyn FnOnce(&Path)>>> = const { std::cell::RefCell::new(None) };
}
#[cfg(test)]
fn run_latest_certified_frontier_post_validation_hook_for_tests() {
    let hook = LATEST_CERTIFIED_FRONTIER_POST_VALIDATION_HOOK.with(|slot| slot.borrow_mut().take());
    if let Some(hook) = hook {
        hook();
    }
}
#[cfg(test)]
fn run_native_amx_prune_pre_unlink_hook_for_tests(path: &Path) {
    NATIVE_AMX_PRUNE_PRE_UNLINK_HOOK.with(|slot| {
        let mut state = slot.borrow_mut();
        let Some(hook) = state.as_mut() else {
            return;
        };
        if hook.calls_before_run > 0 {
            hook.calls_before_run -= 1;
            return;
        }
        let callback = hook.hook.take();
        *state = None;
        drop(state);
        if let Some(callback) = callback {
            callback(path);
        }
    });
}
#[cfg(test)]
fn set_native_amx_prune_pre_unlink_hook_for_tests(
    calls_before_run: usize,
    hook: impl FnOnce(&Path) + 'static,
) {
    NATIVE_AMX_PRUNE_PRE_UNLINK_HOOK.with(|slot| {
        let previous = slot.borrow_mut().replace(NativeAmxPrunePreUnlinkHook {
            calls_before_run,
            hook: Some(Box::new(hook)),
        });
        assert!(
            previous.is_none(),
            "Native AMX prune hook already installed"
        );
    });
}
#[cfg(test)]
fn run_native_amx_latest_index_pre_mutation_hook_for_tests(path: &Path) {
    let hook = NATIVE_AMX_LATEST_INDEX_PRE_MUTATION_HOOK.with(|slot| slot.borrow_mut().take());
    if let Some(hook) = hook {
        hook(path);
    }
}
#[cfg(test)]
fn set_native_amx_latest_index_pre_mutation_hook_for_tests(hook: impl FnOnce(&Path) + 'static) {
    NATIVE_AMX_LATEST_INDEX_PRE_MUTATION_HOOK.with(|slot| {
        let previous = slot.borrow_mut().replace(Box::new(hook));
        assert!(
            previous.is_none(),
            "Native AMX latest-index mutation hook already installed"
        );
    });
}
#[cfg(test)]
fn set_latest_certified_frontier_post_validation_hook_for_tests(hook: impl FnOnce() + 'static) {
    LATEST_CERTIFIED_FRONTIER_POST_VALIDATION_HOOK.with(|slot| {
        let previous = slot.borrow_mut().replace(Box::new(hook));
        assert!(previous.is_none(), "frontier test hook already installed");
    });
}
#[cfg(test)]
fn fail_next_certified_lane_block_artifact_validation_for_tests() {
    FAIL_NEXT_CERTIFIED_LANE_BLOCK_ARTIFACT_VALIDATION.with(|flag| flag.set(true));
}
#[cfg(test)]
fn fail_after_next_certified_frontier_build_for_tests() {
    FAIL_AFTER_NEXT_CERTIFIED_FRONTIER_BUILD.with(|flag| flag.set(true));
}
#[cfg(test)]
fn fail_after_bound_progress_append_build_for_tests(calls_before_failure: usize) {
    FAIL_AFTER_BOUND_PROGRESS_APPEND_BUILD_CALLS.with(|slot| {
        assert!(slot.replace(Some(calls_before_failure)).is_none());
    });
}
#[cfg(test)]
fn should_fail_after_bound_progress_append_build_for_tests() -> bool {
    FAIL_AFTER_BOUND_PROGRESS_APPEND_BUILD_CALLS.with(|slot| match slot.get() {
        Some(0) => {
            slot.set(None);
            true
        }
        Some(remaining) => {
            slot.set(Some(remaining - 1));
            false
        }
        None => false,
    })
}
#[cfg(test)]
fn fail_after_next_autonomous_certified_frontier_for_tests() {
    FAIL_AFTER_NEXT_AUTONOMOUS_CERTIFIED_FRONTIER.with(|flag| flag.set(true));
}
#[cfg(test)]
fn fail_next_autonomous_merge_bundle_persistence_for_tests() {
    FAIL_NEXT_AUTONOMOUS_MERGE_BUNDLE_PERSISTENCE.with(|flag| flag.set(true));
}
#[cfg(test)]
fn fail_next_autonomous_merge_bundle_append_data_sync_for_tests() {
    FAIL_NEXT_AUTONOMOUS_MERGE_BUNDLE_APPEND_DATA_SYNC.with(|flag| flag.set(true));
}
#[cfg(test)]
fn fail_after_next_autonomous_merge_bundle_pair_for_tests() {
    FAIL_AFTER_NEXT_AUTONOMOUS_MERGE_BUNDLE_PAIR.with(|flag| flag.set(true));
}
const CANONICAL_HASH_READER_OBSERVED: usize = 1 << 0;
const CANONICAL_BLOCK_READER_OBSERVED: usize = 1 << 1;
