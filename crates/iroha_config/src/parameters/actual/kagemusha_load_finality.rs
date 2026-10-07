//! Explicit optional server-only ordinary Load proof production.

use std::{path::PathBuf, time::Duration};

/// Independent signed installation, existing custody and finite worker limits.
/// Absence disables the terminal-proof route; it selects no replacement proof.
#[derive(Clone, PartialEq, Eq)]
pub struct KagemushaLoadFinality {
    /// Independently selected exact Scheme identity.
    pub scheme_id: [u8; 32],
    /// Independently selected signed ArtifactManifest identity.
    pub manifest_digest: [u8; 32],
    /// Exact canonical signed verifier pack; public material with authenticated contents.
    pub verifier_pack: PathBuf,
    /// Exact complete producer inventory authenticated by that same signed manifest.
    pub producer_inventory: PathBuf,
    /// Existing private content-addressed server D/V/PK original directory.
    pub server_originals: PathBuf,
    /// Existing private, exclusively owned immutable proof journal.
    pub journal_dir: PathBuf,
    /// Maximum original server proving key length.
    pub maximum_key_bytes: usize,
    /// Aggregate D/V/PK graph extent ceiling, not process RSS.
    pub maximum_original_bytes: usize,
    /// Maximum exact source graph members.
    pub maximum_artifacts: usize,
    /// Scratch ceiling of one genuine proof/MSM kernel, not total RSS.
    pub msm_bytes: usize,
    /// Maximum actual journal files, including interrupted partials.
    pub maximum_journal_entries: usize,
    /// Maximum actual journal byte extent, including interrupted partials.
    pub maximum_journal_bytes: u64,
    /// Maximum queued, active and completed-but-unread requests together.
    pub max_pending_requests: usize,
    /// Physical allocation admission for the genuine native history cursor.
    pub native_working_set_bytes: usize,
    /// Finite monotonic deadline for each native history observation.
    pub native_step_timeout: Duration,
    /// Highest receipt height accepted for finite historical catch-up.
    pub maximum_receipt_height: u64,
}
impl std::fmt::Debug for KagemushaLoadFinality {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("KagemushaLoadFinality")
            .field("scheme_id", &self.scheme_id)
            .field("manifest_digest", &self.manifest_digest)
            .field("max_pending_requests", &self.max_pending_requests)
            .finish_non_exhaustive()
    }
}
