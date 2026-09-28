//! Local evidence of completed production Metal dispatches, separate from self-tests.

use std::sync::atomic::{AtomicU64, Ordering};

/// Production Metal pipelines with independently observable completion counts.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(usize)]
pub enum MetalKernel {
    /// Four-lane wrapping 32-bit addition.
    Add32,
    /// Two-lane wrapping 64-bit addition.
    Add64,
    /// Four-lane bitwise conjunction.
    And,
    /// Four-lane bitwise exclusive disjunction.
    Xor,
    /// Four-lane bitwise disjunction.
    Or,
    /// One SHA-256 compression block.
    Sha256,
    /// Batched padded SHA-256 leaves.
    Sha256Leaves,
    /// One level of SHA-256 pair reduction.
    Sha256Pairs,
    /// Keccak-f1600 permutation.
    Keccak,
    /// One AES encryption round.
    AesEnc,
    /// One AES decryption round.
    AesDec,
    /// Batched AES encryption rounds with one shared key.
    AesEncBatch,
    /// Batched AES decryption rounds with one shared key.
    AesDecBatch,
    /// Batched AES encryption with multiple round keys.
    AesEncRounds,
    /// Batched AES decryption with multiple round keys.
    AesDecRounds,
    /// Batched Ed25519 signature verification.
    Ed25519,
}

impl MetalKernel {
    /// Every production pipeline; diagnostics and startup probes are excluded.
    pub const ALL: [Self; 16] = [
        Self::Add32,
        Self::Add64,
        Self::And,
        Self::Xor,
        Self::Or,
        Self::Sha256,
        Self::Sha256Leaves,
        Self::Sha256Pairs,
        Self::Keccak,
        Self::AesEnc,
        Self::AesDec,
        Self::AesEncBatch,
        Self::AesDecBatch,
        Self::AesEncRounds,
        Self::AesDecRounds,
        Self::Ed25519,
    ];
}

struct CompletionCounts([AtomicU64; MetalKernel::ALL.len()]);

impl CompletionCounts {
    const fn new() -> Self {
        Self([const { AtomicU64::new(0) }; MetalKernel::ALL.len()])
    }

    fn get(&self, kernel: MetalKernel) -> u64 {
        self.0[kernel as usize].load(Ordering::Relaxed)
    }

    #[cfg(any(test, all(target_os = "macos", feature = "metal")))]
    fn record(&self, kernel: Option<MetalKernel>, completed: bool) {
        if let Some(kernel) = kernel.filter(|_| completed) {
            // Telemetry saturation must never affect execution or wrap receipts.
            let _ = self.0[kernel as usize].fetch_update(
                Ordering::Relaxed,
                Ordering::Relaxed,
                |count| Some(count.saturating_add(1)),
            );
        }
    }
}

static COMPLETIONS: CompletionCounts = CompletionCounts::new();

/// Completed production dispatches for a pipeline since process startup.
///
/// Startup probes, diagnostic kernels, failed commands and CPU fallbacks do not
/// increment this counter. A completion attests driver completion only; parity,
/// performance and candidate provenance require separate qualification evidence.
/// Counters are local telemetry and never enter gas, state or commitments.
pub fn metal_completed_dispatches(kernel: MetalKernel) -> u64 {
    COMPLETIONS.get(kernel)
}

#[cfg(all(target_os = "macos", feature = "metal"))]
pub(super) fn record_completion(kernel: Option<MetalKernel>, completed: bool) {
    COMPLETIONS.record(kernel, completed);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn receipts_exclude_failed_commands_probes_and_other_pipelines() {
        let counts = CompletionCounts::new();
        counts.record(None, true);
        counts.record(Some(MetalKernel::Add32), false);
        assert!(
            MetalKernel::ALL
                .into_iter()
                .all(|kernel| counts.get(kernel) == 0)
        );
        for (index, kernel) in MetalKernel::ALL.into_iter().enumerate() {
            assert_eq!(kernel as usize, index);
            counts.record(Some(kernel), true);
            assert_eq!(counts.get(kernel), 1);
        }
        counts.record(Some(MetalKernel::Add32), true);
        assert_eq!(counts.get(MetalKernel::Add32), 2);
        assert_eq!(counts.get(MetalKernel::Add64), 1);
        counts.0[MetalKernel::Add32 as usize].store(u64::MAX, Ordering::Relaxed);
        counts.record(Some(MetalKernel::Add32), true);
        assert_eq!(counts.get(MetalKernel::Add32), u64::MAX);
    }

    #[test]
    fn receipts_share_exact_concurrent_completion_counts() {
        let counts = CompletionCounts::new();
        std::thread::scope(|scope| {
            for _ in 0..8 {
                let counts = &counts;
                scope.spawn(move || {
                    for _ in 0..100 {
                        counts.record(Some(MetalKernel::Sha256), true);
                    }
                });
            }
        });
        assert_eq!(counts.get(MetalKernel::Sha256), 800);
        assert_eq!(counts.get(MetalKernel::Keccak), 0);
    }
}
