//! Resolve Merkle attempt policy from one complete host configuration snapshot.

use crate::AccelerationConfig;

const DEFAULT_GPU_MIN_LEAVES: usize = 8_192;
// Qualified Metal profiles already compare complete CPU and GPU costs. Other
// targets retain the conservative default; an explicit CPU ceiling overrides it.
#[cfg(any(target_arch = "aarch64", target_arch = "x86", target_arch = "x86_64"))]
const DEFAULT_CPU_SHA2_MAX_LEAVES: usize = if cfg!(all(target_os = "macos", feature = "metal")) {
    0
} else {
    32_768
};

#[derive(Clone, Copy, Debug)]
struct BackendPolicy {
    enabled: bool,
    minimum: usize,
}

/// Immutable policy for one construction, root-only hash or retained-tree rehash.
/// It admits the original native attempt, never a result or qualified device.
#[derive(Clone, Copy, Debug)]
pub(super) struct ResolvedMerklePolicy {
    #[cfg(any(target_os = "macos", test))]
    metal: BackendPolicy,
    #[cfg(any(feature = "cuda", test))]
    cuda: BackendPolicy,
    cpu_prefer_max: Option<usize>,
}

impl ResolvedMerklePolicy {
    pub(super) fn current() -> Self {
        let config = crate::acceleration_config();
        Self::resolve(config, cpu_preference(config))
    }

    fn resolve(config: AccelerationConfig, cpu_prefer_max: Option<usize>) -> Self {
        let generic = config
            .merkle_min_leaves_gpu
            .unwrap_or(DEFAULT_GPU_MIN_LEAVES);
        let gpu_allowed = config.max_gpus != Some(0);
        Self {
            #[cfg(any(target_os = "macos", test))]
            metal: BackendPolicy {
                enabled: config.enable_metal && gpu_allowed,
                minimum: config.merkle_min_leaves_metal.unwrap_or(generic),
            },
            #[cfg(any(feature = "cuda", test))]
            cuda: BackendPolicy {
                enabled: config.enable_cuda && gpu_allowed,
                minimum: config.merkle_min_leaves_cuda.unwrap_or(generic),
            },
            cpu_prefer_max,
        }
    }

    #[cfg(any(target_os = "macos", test))]
    pub(super) fn try_metal<T>(
        self,
        leaves: usize,
        attempt: impl FnOnce() -> Option<T>,
    ) -> Option<T> {
        self.try_backend(self.metal, leaves, attempt)
    }

    #[cfg(any(feature = "cuda", test))]
    pub(super) fn try_cuda<T>(
        self,
        leaves: usize,
        attempt: impl FnOnce() -> Option<T>,
    ) -> Option<T> {
        self.try_backend(self.cuda, leaves, attempt)
    }

    fn try_backend<T>(
        self,
        backend: BackendPolicy,
        leaves: usize,
        attempt: impl FnOnce() -> Option<T>,
    ) -> Option<T> {
        if !backend.enabled
            || leaves < backend.minimum
            || self.cpu_prefer_max.is_some_and(|maximum| leaves <= maximum)
        {
            return None;
        }
        attempt()
    }
}

fn cpu_preference(config: AccelerationConfig) -> Option<usize> {
    #[cfg(target_arch = "aarch64")]
    {
        std::arch::is_aarch64_feature_detected!("sha2").then_some(
            config
                .prefer_cpu_sha2_max_leaves_aarch64
                .unwrap_or(DEFAULT_CPU_SHA2_MAX_LEAVES),
        )
    }
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    {
        std::is_x86_feature_detected!("sha").then_some(
            config
                .prefer_cpu_sha2_max_leaves_x86
                .unwrap_or(DEFAULT_CPU_SHA2_MAX_LEAVES),
        )
    }
    #[cfg(not(any(target_arch = "aarch64", target_arch = "x86", target_arch = "x86_64")))]
    {
        let _ = config;
        None
    }
}

#[cfg(test)]
mod tests;
