/// Read-only worker/corridor snapshot for pending-Kura Apply diagnostics.
#[cfg(test)]
#[derive(Debug)]
#[allow(dead_code)]
pub(in crate::sumeragi) struct PendingKuraApplyIoSnapshotV1 {
    queued_commands: usize,
    tracked_queued: usize,
    tracked_active: usize,
    tracked_completion_pending: usize,
    completion_owners: usize,
    local_completions: usize,
    held_completion: bool,
}
