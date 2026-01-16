//! Heuristics and adaptive selection for Norito fast paths.
//!
//! This module exposes the canonical heuristics profile used by Norito for
//! compression and adaptive layout selection. Thresholds are fixed at compile
//! time; overriding them requires rebuilding Norito with a different profile.
//! Decisions depend only on payload size and hardware capabilities and never
//! affect on-wire layout semantics.

use super::{Compression, hw};

/// Adaptive compression plan produced by the selector.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CompressionPlan {
    None,
    CpuZstd { level: i32 },
    GpuZstd { level: i32 },
}

/// Tunable thresholds and levels used by the selector.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Heuristics {
    /// Minimum payload size (bytes) to attempt CPU zstd.
    pub min_compress_bytes_cpu: usize,
    /// Minimum payload size (bytes) to attempt GPU zstd when available.
    pub min_compress_bytes_gpu: usize,
    /// zstd level for medium-size payloads.
    pub zstd_level_small: i32,
    /// zstd level for large payloads.
    pub zstd_level_large: i32,
    /// GPU zstd level (kept conservative to reduce latency).
    pub zstd_level_gpu: i32,
    /// Size threshold distinguishing small vs large for CPU zstd level.
    pub large_threshold: usize,

    /// Small-N threshold for AoS vs NCB adaptive selection in `columnar` helpers.
    ///
    /// For row counts `<= aos_ncb_small_n`, adaptive encoders will do a two-pass
    /// size probe (encode both AoS and NCB and pick the smaller). For row counts
    /// `> aos_ncb_small_n`, encoders will choose NCB directly (subject to minor
    /// content heuristics for deltas/dictionaries inside NCB). Defaults to 64.
    pub aos_ncb_small_n: usize,

    /// Columnar 4-column combos (u64, &str/bytes, u32, bool):
    /// If the number of rows is less than or equal to this threshold and there exists
    /// an empty string/byte element in the column, delta encoding for the corresponding
    /// column is disabled to keep small-N overheads and goldens stable.
    ///
    /// Default: 2 (only disable deltas for 1–2 rows with an empty element present).
    pub combo_no_delta_small_n_if_empty: usize,

    /// Minimum rows to consider ID delta encoding for 4-column combos.
    /// Default: 2
    pub combo_id_delta_min_rows: usize,

    /// Minimum rows to consider u32 delta encoding for 4-column combos.
    /// Default: 2
    pub combo_u32_delta_min_rows: usize,

    /// Enable/disable ID delta encoding in 4-column combos.
    /// Default: true
    pub combo_enable_id_delta: bool,

    /// Enable/disable u32 delta encoding in (u64, &str, u32, bool) combos.
    /// Default: true
    pub combo_enable_u32_delta_names: bool,
    /// Enable/disable u32 delta encoding in (u64, &[u8], u32, bool) combos.
    /// Default: true
    pub combo_enable_u32_delta_bytes: bool,

    /// Enable/disable name dictionary in (u64, &str, u32, bool) combos.
    /// Default: true
    pub combo_enable_name_dict: bool,

    /// Maximum distinct/name-count ratio to allow building a dictionary.
    /// Default: 0.40
    pub combo_dict_ratio_max: f64,

    /// Minimum average string length to use the dictionary.
    /// Default: 8.0
    pub combo_dict_avg_len_min: f64,

    /// Maximum number of distinct strings to include in a combo dictionary.
    /// A value of `0` disables the cap (allow unbounded dictionary growth).
    /// Defaults to 1024 to cap memory growth while preserving compression wins.
    pub combo_dict_max_entries: usize,
}

impl Heuristics {
    /// Canonical heuristics baked into this Norito release.
    pub const fn canonical() -> Self {
        // Defaults derived from profiling across typical Iroha payloads:
        // - Compress payloads >=256 B (covers genesis instructions) with CPU zstd level 1
        // - Prefer GPU for 1 MiB+ when available
        // - Switch CPU to zstd level 3 for payloads >=32 KiB (e.g., SignedBlock bodies)
        Self {
            // Based on profiling of genesis instructions (439–6.3 KiB) where even
            // sub-1 KiB payloads compress to ~45% with zstd level 1.
            min_compress_bytes_cpu: 256,
            min_compress_bytes_gpu: 1024 * 1024,
            zstd_level_small: 1,
            zstd_level_large: 3,
            zstd_level_gpu: 1,
            // Switch to the higher CPU level for 32 KiB+ payloads (blocks), where
            // level 3 trimmed genesis SignedBlock size by ~15% over level 1.
            large_threshold: 32 * 1024,
            // Adaptive AoS/NCB selection threshold
            aos_ncb_small_n: 64,
            // Columnar combo heuristics: disable deltas for tiny inputs with empties
            combo_no_delta_small_n_if_empty: 2,
            combo_id_delta_min_rows: 2,
            combo_u32_delta_min_rows: 2,
            combo_enable_id_delta: true,
            combo_enable_u32_delta_names: true,
            combo_enable_u32_delta_bytes: true,
            combo_enable_name_dict: true,
            combo_dict_ratio_max: 0.40,
            combo_dict_avg_len_min: 8.0,
            combo_dict_max_entries: 1024,
        }
    }
}

impl Default for Heuristics {
    fn default() -> Self {
        Self::canonical()
    }
}

/// Canonical heuristics baked into this build of Norito.
const CANONICAL_HEURISTICS: Heuristics = Heuristics::canonical();

/// Get the global heuristics (initialize with defaults on first use).
#[inline]
pub fn get() -> Heuristics {
    CANONICAL_HEURISTICS
}

/// Select a compression plan for a given payload length.
#[inline]
pub fn select_compression_for_len(len: usize) -> CompressionPlan {
    let h = get();
    if len < h.min_compress_bytes_cpu {
        return CompressionPlan::None;
    }
    if hw::has_gpu_compression() && len >= h.min_compress_bytes_gpu {
        return CompressionPlan::GpuZstd {
            level: h.zstd_level_gpu,
        };
    }
    let level = if len >= h.large_threshold {
        h.zstd_level_large
    } else {
        h.zstd_level_small
    };
    CompressionPlan::CpuZstd { level }
}

/// Compress payload according to heuristics and return the Norito compression tag
/// and compressed bytes (or the original payload for `None`).
pub fn compress_auto(payload: Vec<u8>) -> std::io::Result<(Compression, Vec<u8>)> {
    match select_compression_for_len(payload.len()) {
        CompressionPlan::None => Ok((Compression::None, payload)),
        CompressionPlan::GpuZstd { level: _level } => {
            #[cfg(all(feature = "compression", feature = "gpu-compression"))]
            {
                let out = super::gpu_zstd::encode_all(payload, _level)?;
                Ok((Compression::Zstd, out))
            }
            #[cfg(all(feature = "compression", not(feature = "gpu-compression")))]
            {
                // GPU path requested, but GPU compression is not compiled in: fall back to CPU zstd
                let out = zstd::encode_all(std::io::Cursor::new(payload), get().zstd_level_large)?;
                Ok((Compression::Zstd, out))
            }
            #[cfg(not(feature = "compression"))]
            {
                // Compression fully disabled: degrade gracefully with no compression
                Ok((Compression::None, payload))
            }
        }
        CompressionPlan::CpuZstd { level } => {
            #[cfg(feature = "compression")]
            {
                let out = zstd::encode_all(std::io::Cursor::new(payload), level)?;
                Ok((Compression::Zstd, out))
            }
            #[cfg(not(feature = "compression"))]
            {
                let _ = level;
                Ok((Compression::None, payload))
            }
        }
    }
}

/// Compute layout flags for a payload size estimate.
///
/// Norito v1 uses a fixed layout; size-based toggles are not applied.
pub fn select_layout_flags_for_size(len_estimate: usize) -> u8 {
    let h = get();
    select_layout_flags_for_size_with(&h, len_estimate)
}

/// Compute adaptive layout flag bits for a given payload size using the provided
/// heuristics, without mutating global overrides.
pub fn select_layout_flags_for_size_with(h: &Heuristics, len_estimate: usize) -> u8 {
    let _ = (h, len_estimate);
    super::default_encode_flags()
}
