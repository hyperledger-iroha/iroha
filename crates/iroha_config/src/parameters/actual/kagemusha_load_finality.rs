//! Explicit bounded terminal Load proving configuration; it grants no finality authority.

use std::{path::PathBuf, time::Duration};

/// Optional server proof source and resource policy. Actual committed genesis/history
/// and the installed complete proof graph independently authenticate every result.
#[derive(Clone, PartialEq, Eq)]
pub struct KagemushaLoadFinality {
    /// Exact installed Scheme identity.
    pub scheme_id: [u8; 32],
    /// Exact installed artifact manifest identity.
    pub manifest_digest: [u8; 32],
    /// Existing verifier-pack original file.
    pub verifier_pack: PathBuf,
    /// Existing producer-inventory original file.
    pub producer_inventory: PathBuf,
    /// Existing private directory of immutable verifier D/V originals.
    pub verifier_originals: PathBuf,
    /// Separately initialized private proving-key cache; serving never adopts an empty cache.
    pub proving_cache: PathBuf,
    /// Existing private retention journal; startup never creates it.
    pub journal_dir: PathBuf,
    /// Maximum queued, active and unread terminal results.
    pub max_pending_requests: usize,
    /// Maximum receipt height eligible for bounded native replay.
    pub maximum_receipt_height: u64,
    /// Maximum individual proving-key original extent.
    pub maximum_key_bytes: usize,
    /// Finite resident proving-key bound, at least maximum_key_bytes and at most 16 GiB.
    pub maximum_resident_proving_key_bytes: usize,
    /// Maximum aggregate original graph extent.
    pub maximum_original_bytes: usize,
    /// Maximum graph entries.
    pub maximum_artifacts: usize,
    /// Scratch budget for each proof operation.
    pub msm_bytes: usize,
    /// Maximum journal entries including partials and locks.
    pub maximum_journal_entries: usize,
    /// Maximum aggregate journal file extent.
    pub maximum_journal_bytes: u64,
    /// Allocation budget for actual native history acquisition.
    pub native_working_set_bytes: usize,
    /// Deadline for each native block acquisition.
    pub native_step_timeout: Duration,
}

impl std::fmt::Debug for KagemushaLoadFinality {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("KagemushaLoadFinality")
            .field("scheme_id", &self.scheme_id)
            .field("manifest_digest", &self.manifest_digest)
            .field("max_pending_requests", &self.max_pending_requests)
            .field("maximum_receipt_height", &self.maximum_receipt_height)
            .finish_non_exhaustive()
    }
}
