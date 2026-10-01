//! Bounded SHA3 continuation jobs with paired CPU execution and exact work counts.
//!
//! The canonical framing owner lends both prefix and body. This module never
//! replaces complete context with a digest or chooses a protocol suffix.
use crate::{Error, Result};
use fastpq_isi::keccak256::Sha3_256V1;
#[cfg(any(test, feature = "fastpq-gpu", feature = "simd"))]
use fastpq_isi::keccak256::Sha3Digest256V1;
#[cfg(any(test, feature = "fastpq-gpu", feature = "simd"))]
use rayon::prelude::*;
/// Same fixed admission bounds for scalar, SIMD and device execution.
pub(crate) const MAX_JOBS: usize = 1024;
pub(crate) const MAX_BODY_BYTES: usize = 8192;
pub(crate) const RATE: usize = 136;
/// A borrowed complete canonical SHA3 continuation; caller owns private bytes.
#[cfg_attr(
    not(any(test, feature = "fastpq-gpu", feature = "simd")),
    expect(
        dead_code,
        reason = "scalar-only builds charge this exact borrowed-job layout without creating jobs"
    )
)]
#[derive(Clone, Copy)]
pub(crate) struct Job<'a> {
    prefix: &'a Sha3_256V1,
    body: &'a [u8],
}
#[cfg(any(test, feature = "fastpq-gpu", feature = "simd"))]
impl<'a> Job<'a> {
    pub(crate) fn new(prefix: &'a Sha3_256V1, body: &'a [u8]) -> Self {
        Self { prefix, body }
    }
    pub(crate) fn prefix(&self) -> &'a Sha3_256V1 {
        self.prefix
    }
    pub(crate) fn body(&self) -> &'a [u8] {
        self.body
    }
    pub(crate) fn scalar(&self) -> Sha3Digest256V1 {
        let mut hash = self.prefix.clone();
        hash.update(self.body);
        hash.finalize()
    }
    /// Each full absorbed rate plus one final padded permutation. Cached prefix
    /// work is charged once by its context owner, never once per continuation.
    #[cfg(test)]
    pub(crate) fn permutations(&self) -> Result<usize> {
        self.prefix.with_absorbed_state_v1(|_, position| {
            position
                .checked_add(self.body.len())
                .map(|n| n / RATE + 1)
                .ok_or_else(|| invalid("Keccak continuation work overflow"))
        })
    }
}
#[cfg(any(test, feature = "fastpq-gpu", feature = "simd"))]
pub(crate) fn validate(jobs: &[Job<'_>], output_count: usize) -> Result<usize> {
    if jobs.is_empty() || jobs.len() > MAX_JOBS || output_count != jobs.len() {
        return Err(invalid(
            "Keccak batch requires its exact bounded cardinality",
        ));
    }
    jobs.iter().try_fold(0usize, |bytes, job| {
        if job.body.len() > MAX_BODY_BYTES {
            return Err(invalid("Keccak body exceeds its fixed ceiling"));
        }
        bytes
            .checked_add(job.body.len())
            .ok_or_else(|| invalid("Keccak payload overflow"))
    })
}
/// Ordered output, no reduction and no fallback after a required-device error.
/// At most two live states and their bounded scratch exist in each CPU worker.
#[cfg(any(test, feature = "fastpq-gpu", feature = "simd"))]
pub(crate) fn hash_cpu(jobs: &[Job<'_>], output: &mut [[u8; 32]]) -> Result<()> {
    validate(jobs, output.len())?;
    output
        .par_chunks_mut(2)
        .zip(jobs.par_chunks(2))
        .for_each(|(out, pair)| {
            #[cfg(all(feature = "simd", any(target_arch = "aarch64", target_arch = "x86_64")))]
            if simd::available()
                && pair.len() == 2
                && pair[0].body().len() == pair[1].body().len()
                && pair[0].prefix().with_absorbed_state_v1(|_, p| p)
                    == pair[1].prefix().with_absorbed_state_v1(|_, p| p)
            {
                let values = simd::hash_pair(pair[0], pair[1]);
                out.copy_from_slice(&values);
                return;
            }
            for (out, job) in out.iter_mut().zip(pair) {
                *out = job.scalar().into_bytes();
            }
        });
    Ok(())
}
/// Fixed actual buffer extents, padded to the existing shared-page owner.
/// The enclosing prover separately charges the complete retained global pool.
/// Public KAT storage finishes before private staging; retain the greater peak.
pub(crate) fn device_payload_bytes(count: usize, body_bytes: usize) -> Result<usize> {
    if count == 0 || count > MAX_JOBS || body_bytes > count * MAX_BODY_BYTES {
        return Err(invalid(
            "Keccak device payload geometry exceeds fixed bounds",
        ));
    }
    let page = crate::gpu_memory::METAL_PAGE_BYTES;
    let mut total = 0usize;
    for bytes in [
        26 * count * 8,
        2 * count * 8,
        body_bytes.max(1),
        4 * count * 8,
        60 * count * 8,
    ] {
        total = total
            .checked_add(bytes.div_ceil(page) * page)
            .ok_or_else(|| invalid("Keccak shared page charge overflow"))?;
    }
    Ok(total.max(5 * page) + 24 * 1024)
}

fn invalid(details: &'static str) -> Error {
    Error::NativeDigestExecution {
        details: details.into(),
    }
}
#[cfg(all(feature = "simd", any(target_arch = "aarch64", target_arch = "x86_64")))]
#[path = "keccak_simd.rs"]
mod simd;
#[cfg(test)]
#[path = "keccak_batch_tests.rs"]
mod tests;
