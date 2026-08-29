/// Validated production configuration for the SCCP replay archive reader.
///
/// The absence of this value disables the service. Signing keys are
/// deliberately not configurable: Torii only receives the three public keys
/// pinned by release policy.
#[derive(Debug, Clone)]
pub struct ToriiSccpReplayArchive {
    /// Owner-only directory containing content-addressed snapshots and the
    /// manifest-last accepted head.
    pub state_dir: PathBuf,
    /// Exact, replica-id-ordered set of independent archive origins and keys.
    pub replicas: [ToriiSccpReplayArchiveReplica; 3],
    /// Complete response byte ceiling applied before Norito decoding.
    pub max_response_bytes: usize,
    /// Per-snapshot byte ceiling applied before snapshot decoding.
    pub max_snapshot_bytes: usize,
    /// Per-snapshot leaf cardinality ceiling.
    pub max_snapshot_leaves: usize,
    /// Maximum accumulators accepted in one three-replica checkpoint set.
    pub max_accumulators: usize,
    /// Complete connect/read timeout for each pinned replica request.
    pub request_timeout: Duration,
}

/// One pinned SCCP replay archive replica.
#[derive(Debug, Clone)]
pub struct ToriiSccpReplayArchiveReplica {
    /// Stable, nonzero release-policy identity.
    pub replica_id: [u8; 32],
    /// Canonical HTTPS origin. Paths, credentials, queries, and fragments are
    /// rejected by the user-layer parser.
    pub origin: Url,
    /// Exact nonzero Ed25519 public-key bytes.
    pub ed25519_public_key: [u8; 32],
}
