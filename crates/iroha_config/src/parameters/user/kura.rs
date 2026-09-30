/// User-level configuration container for `Kura`.
#[derive(Debug, ReadConfig)]
pub struct Kura {
    /// Startup validation policy for the canonical block journal.
    #[config(default)]
    pub init_mode: KuraInitMode,
    /// Directory where Kura stores blocks and auxiliary indices.
    #[config(
        env = "KURA_STORE_DIR",
        default = "PathBuf::from(defaults::kura::STORE_DIR)"
    )]
    pub store_dir: WithOrigin<PathBuf>,
    /// Maximum on-disk footprint for Kura (bytes, 0 = unlimited).
    #[config(
        env = "KURA_MAX_DISK_USAGE_BYTES",
        default = "defaults::kura::MAX_DISK_USAGE_BYTES"
    )]
    pub max_disk_usage_bytes: Bytes,
    /// Complete native context projection bytes per committed carrier; zero is invalid.
    /// This mandatory archive has no environment override or disable switch.
    #[config(default = "defaults::kura::NATIVE_CONTEXT_ARCHIVE_MAX_BYTES")]
    pub native_context_archive_max_bytes: NonZeroUsize,
    /// Number of most-recent blocks kept in memory for fast access.
    #[config(
        env = "KURA_BLOCKS_IN_MEMORY",
        default = "defaults::kura::BLOCKS_IN_MEMORY"
    )]
    pub blocks_in_memory: NonZeroUsize,
    /// Finite requested-allocation limit for State's shared hash-history generations.
    /// Zero is invalid; this policy has no environment override.
    #[config(default = "defaults::kura::BLOCK_HASH_HISTORY_BYTES")]
    pub block_hash_history_bytes: Bytes,
    /// Finite requested-allocation limit for State's transaction-membership generations.
    /// Zero is invalid; this policy has no environment override.
    #[config(default = "defaults::kura::TRANSACTION_HISTORY_BYTES")]
    pub transaction_history_bytes: Bytes,
    /// Finite membership segment and workspace limits, without an environment override.
    #[config(nested)]
    pub membership_storage: KuraMembershipStorage,
    /// Bounded FASTPQ artifact content store. No environment-based policy override is accepted.
    #[config(nested)]
    pub fastpq_artifacts: KuraFastpqArtifacts,
    /// Fsync policy for block persistence.
    #[config(env = "KURA_FSYNC_MODE", default = "defaults::kura::FSYNC_MODE")]
    pub fsync_mode: KuraFsyncMode,
    /// Interval for batched fsync operations.
    #[config(
        env = "KURA_FSYNC_INTERVAL_MS",
        default = "defaults::kura::FSYNC_INTERVAL.into()"
    )]
    pub fsync_interval_ms: DurationMs,
    /// Debug controls for development/testing scenarios.
    #[config(nested)]
    pub debug: KuraDebug,
}
impl Kura {
    fn parse(self, emitter: &mut Emitter<ParseError>) -> actual::Kura {
        let Self {
            init_mode,
            store_dir,
            max_disk_usage_bytes,
            native_context_archive_max_bytes,
            blocks_in_memory,
            block_hash_history_bytes,
            transaction_history_bytes,
            membership_storage,
            fastpq_artifacts,
            fsync_mode,
            fsync_interval_ms,
            debug:
                KuraDebug {
                    output_new_blocks: debug_output_new_blocks,
                },
        } = self;
        if block_hash_history_bytes.get() == 0
            || usize::try_from(block_hash_history_bytes.get()).is_err()
        {
            emitter.emit(Report::new(ParseError::InvalidKuraConfig).attach(
                "kura.block_hash_history_bytes must be nonzero and representable as usize",
            ));
        }
        if transaction_history_bytes.get() == 0
            || usize::try_from(transaction_history_bytes.get()).is_err()
        {
            emitter.emit(Report::new(ParseError::InvalidKuraConfig).attach(
                "kura.transaction_history_bytes must be nonzero and representable as usize",
            ));
        }
        let fastpq_artifacts = actual::KuraFastpqArtifactPolicy {
            max_artifact_bytes: fastpq_artifacts.max_artifact_bytes,
            max_artifacts: fastpq_artifacts.max_artifacts,
            max_total_bytes: fastpq_artifacts.max_total_bytes,
        };
        if let Err(error) = fastpq_artifacts.validate() {
            emitter.emit(Report::new(ParseError::InvalidKuraConfig).attach(error));
        }
        actual::Kura {
            init_mode,
            store_dir,
            max_disk_usage_bytes,
            native_context_archive_max_bytes,
            blocks_in_memory,
            block_hash_history_bytes,
            transaction_history_bytes,
            membership_storage: actual::KuraMembershipStoragePolicy {
                max_bytes: membership_storage.max_bytes,
                memory_bytes: membership_storage.memory_bytes,
            },
            fastpq_artifacts,
            debug_output_new_blocks,
            fsync_mode,
            fsync_interval: fsync_interval_ms.0,
        }
    }
}
/// File-configured finite limits for the single retained membership segment.
#[derive(Debug, Clone, Copy, ReadConfig)]
pub struct KuraMembershipStorage {
    /// Segment bytes, including incomplete append reservations; zero is invalid.
    #[config(default = "defaults::kura::MEMBERSHIP_STORAGE_MAX_BYTES")]
    pub max_bytes: NonZeroU64,
    /// Original controls and append workspace allocation bytes; zero is invalid.
    #[config(default = "defaults::kura::MEMBERSHIP_STORAGE_MEMORY_BYTES")]
    pub memory_bytes: NonZeroUsize,
}
/// File-configured FASTPQ content-store policy with explicit byte/count units.
#[derive(Debug, Clone, Copy, ReadConfig)]
#[expect(clippy::struct_field_names, reason = "operator-facing config keys consistently identify maximum resource limits")]
pub struct KuraFastpqArtifacts {
    /// Maximum complete artifact bytes, including the encoded wrapper.
    #[config(default = "defaults::kura::FASTPQ_ARTIFACT_MAX_BYTES")]
    pub max_artifact_bytes: NonZeroUsize,
    /// Maximum stable content records, plus one separately bounded temporary.
    #[config(default = "defaults::kura::FASTPQ_ARTIFACT_MAX_COUNT")]
    pub max_artifacts: NonZeroUsize,
    /// Maximum sum of stable and temporary file bytes.
    #[config(default = "defaults::kura::FASTPQ_ARTIFACT_MAX_TOTAL_BYTES")]
    pub max_total_bytes: NonZeroU64,
}
/// User-level configuration container for `KuraDebug`.
#[derive(Debug, Clone, Copy, ReadConfig)]
pub struct KuraDebug {
    #[config(env = "KURA_DEBUG_OUTPUT_NEW_BLOCKS", default)]
    output_new_blocks: bool,
}
