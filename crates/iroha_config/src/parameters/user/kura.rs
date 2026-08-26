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
    pub max_disk_usage_bytes: Bytes<u64>,
    /// Number of most-recent blocks kept in memory for fast access.
    #[config(
        env = "KURA_BLOCKS_IN_MEMORY",
        default = "defaults::kura::BLOCKS_IN_MEMORY"
    )]
    pub blocks_in_memory: NonZeroUsize,
    /// Number of recent lane-history entries retained alongside the block store.
    #[config(
        env = "KURA_LANE_HISTORY_RETENTION",
        default = "defaults::kura::LANE_HISTORY_RETENTION"
    )]
    pub lane_history_retention: NonZeroUsize,
    /// Distinct remote peers that must advertise a canonical block before local body eviction.
    #[config(
        env = "KURA_EVICTION_REQUIRED_REPLICAS",
        default = "defaults::kura::EVICTION_REQUIRED_REPLICAS"
    )]
    pub eviction_required_replicas: NonZeroUsize,
    /// Number of authenticated historical advert keys retained immediately before the protected
    /// in-memory block tail.
    #[config(
        env = "KURA_REPLICA_ADVERT_EVICTABLE_WINDOW",
        default = "defaults::kura::REPLICA_ADVERT_EVICTABLE_WINDOW"
    )]
    pub replica_advert_evictable_window: NonZeroUsize,
    /// Lifetime in milliseconds of one authenticated remote replica observation.
    #[config(
        env = "KURA_REPLICA_ADVERT_TTL_MS",
        default = "defaults::kura::REPLICA_ADVERT_TTL.into()"
    )]
    pub replica_advert_ttl_ms: DurationMs,
    /// Cadence in milliseconds for proactively refreshing selected-keeper replica adverts.
    #[config(
        env = "KURA_REPLICA_ADVERT_REFRESH_INTERVAL_MS",
        default = "defaults::kura::REPLICA_ADVERT_REFRESH_INTERVAL.into()"
    )]
    pub replica_advert_refresh_interval_ms: DurationMs,
    /// Capacity of the merge-ledger cache used during compaction.
    #[config(
        env = "KURA_MERGE_LEDGER_CACHE_CAPACITY",
        default = "defaults::kura::MERGE_LEDGER_CACHE_CAPACITY"
    )]
    pub merge_ledger_cache_capacity: usize,
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
            blocks_in_memory,
            lane_history_retention,
            eviction_required_replicas,
            replica_advert_evictable_window,
            replica_advert_ttl_ms,
            replica_advert_refresh_interval_ms,
            merge_ledger_cache_capacity,
            fsync_mode,
            fsync_interval_ms,
            debug:
                KuraDebug {
                    output_new_blocks: debug_output_new_blocks,
                },
        } = self;
        let replica_advert = actual::KuraReplicaAdvertPolicy {
            eviction_required_replicas,
            evictable_window: replica_advert_evictable_window,
            ttl: replica_advert_ttl_ms.0,
            refresh_interval: replica_advert_refresh_interval_ms.0,
        };
        if let Err(error) = replica_advert.validate(blocks_in_memory) {
            emitter.emit(Report::new(ParseError::InvalidKuraConfig).attach(error));
        }
        actual::Kura {
            init_mode,
            store_dir,
            max_disk_usage_bytes,
            blocks_in_memory,
            lane_history_retention,
            replica_advert,
            debug_output_new_blocks,
            merge_ledger_cache_capacity,
            fsync_mode,
            fsync_interval: fsync_interval_ms.0,
        }
    }
}
/// User-level configuration container for `KuraDebug`.
#[derive(Debug, Clone, Copy, ReadConfig)]
pub struct KuraDebug {
    #[config(env = "KURA_DEBUG_OUTPUT_NEW_BLOCKS", default)]
    output_new_blocks: bool,
}
