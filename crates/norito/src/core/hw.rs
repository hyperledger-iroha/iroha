//! Hardware capability detection and caching.
//!
//! This module centralizes runtime detection of CPU SIMD features and optional
//! GPU availability so higher-level code can make deterministic choices about
//! accelerated paths. All detection is best-effort and never changes the
//! observable Norito format — only performance.

use std::sync::{
    OnceLock,
    atomic::{AtomicBool, Ordering},
};

/// Snapshot of hardware capabilities relevant to Norito fast paths.
#[derive(Clone, Copy, Debug, Default)]
pub struct HwCaps {
    // CPU features (x86_64)
    pub sse42: bool,
    pub pclmul: bool,
    pub avx2: bool,
    pub avx512f: bool,
    // CPU features (aarch64)
    pub neon: bool,
    pub pmull: bool,
    pub crc32: bool,
    // GPU backends for compression (feature-gated)
    pub gpu_zstd: bool,
}

static CAPS: OnceLock<HwCaps> = OnceLock::new();
static GPU_ALLOWED: AtomicBool = AtomicBool::new(true);

#[cfg_attr(not(feature = "gpu-compression"), allow(dead_code))]
pub fn gpu_policy_allowed() -> bool {
    GPU_ALLOWED.load(Ordering::SeqCst)
}

/// Detect and cache hardware capabilities.
pub fn detect() -> &'static HwCaps {
    CAPS.get_or_init(|| {
        let mut caps = HwCaps::default();
        // x86_64 feature probes
        #[cfg(target_arch = "x86_64")]
        {
            caps.sse42 = std::is_x86_feature_detected!("sse4.2");
            caps.pclmul = std::is_x86_feature_detected!("pclmulqdq");
            caps.avx2 = std::is_x86_feature_detected!("avx2");
            caps.avx512f = std::is_x86_feature_detected!("avx512f");
        }
        // aarch64 feature probes
        #[cfg(target_arch = "aarch64")]
        {
            // NEON is mandatory on aarch64, but keep probe for completeness
            caps.neon = true;
            caps.pmull = std::arch::is_aarch64_feature_detected!("pmull");
            caps.crc32 = std::arch::is_aarch64_feature_detected!("crc");
        }
        // Optional GPU compression backend availability
        #[cfg(feature = "gpu-compression")]
        {
            caps.gpu_zstd = super::gpu_zstd::available();
        }
        caps
    })
}

/// True if x86_64 AVX2 is available at runtime.
#[inline]
pub fn has_avx2() -> bool {
    detect().avx2
}

/// True if aarch64 NEON is available.
#[inline]
pub fn has_neon() -> bool {
    #[cfg(target_arch = "aarch64")]
    {
        detect().neon
    }
    #[cfg(not(target_arch = "aarch64"))]
    {
        false
    }
}

/// True if a GPU compression backend is available (CUDA or Metal).
#[inline]
pub fn has_gpu_compression() -> bool {
    #[cfg(feature = "gpu-compression")]
    {
        if !gpu_policy_allowed() {
            return false;
        }
        detect().gpu_zstd
    }
    #[cfg(not(feature = "gpu-compression"))]
    {
        false
    }
}

/// Gate GPU compression availability via policy (default: allowed).
pub fn set_gpu_compression_allowed(allowed: bool) {
    GPU_ALLOWED.store(allowed, Ordering::SeqCst);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gpu_policy_toggle_updates_state() {
        let baseline_allowed = gpu_policy_allowed();
        let baseline_has_gpu = has_gpu_compression();

        set_gpu_compression_allowed(false);
        assert!(!gpu_policy_allowed(), "policy flag should reflect disable");
        assert!(
            !has_gpu_compression(),
            "GPU compression must report unavailable once disabled"
        );

        set_gpu_compression_allowed(true);
        assert!(gpu_policy_allowed(), "policy flag should allow GPUs again");
        if baseline_allowed {
            assert_eq!(
                has_gpu_compression(),
                baseline_has_gpu,
                "reenabling policy should restore previous detection state"
            );
        }

        // Restore original configuration for other tests.
        set_gpu_compression_allowed(baseline_allowed);
        assert_eq!(
            gpu_policy_allowed(),
            baseline_allowed,
            "policy flag should reset to baseline"
        );
    }
}
