//! Canonical final-byte-field batches for six-lane native-STARK digests.
//!
//! Prepared jobs retain the typed CPU stream as the framing authority. The
//! accelerator only resumes a fresh final field; CPU execution remains an
//! explicit, identical fallback when Metal dispatch fails or is unavailable.

// TODO: adopt this independently qualified primitive in the prover only after
// supported-device and end-to-end measurements. Until then this internal API is
// intentionally exercised by tests without changing production dispatch.
#![allow(dead_code)]

use fastpq_isi::{GoldilocksDigest384LastFieldStreamV1, GoldilocksDigest384V1};

#[cfg(not(all(feature = "fastpq-gpu", target_os = "macos")))]
use crate::backend::GpuBackend;
use crate::gpu::GpuError;

/// A fresh canonical typed prefix and its exact final byte field.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Digest384LastFieldJob<'a> {
    prefix: GoldilocksDigest384LastFieldStreamV1,
    final_field: &'a [u8],
}

impl<'a> Digest384LastFieldJob<'a> {
    /// Validate a fresh stream handoff without inferring buffered sponge state.
    pub(crate) fn new(
        prefix: GoldilocksDigest384LastFieldStreamV1,
        final_field: &'a [u8],
    ) -> Result<Self, &'static str> {
        if prefix.received_len() != 0 {
            return Err("Digest384 batch prefixes must not have consumed final-field bytes");
        }
        if prefix.expected_len() != final_field.len() {
            return Err("Digest384 batch payload length differs from its canonical prefix");
        }
        Ok(Self {
            prefix,
            final_field,
        })
    }

    /// Return the immutable canonical prefix, including its pending rate words.
    pub(crate) fn prefix(&self) -> &GoldilocksDigest384LastFieldStreamV1 {
        &self.prefix
    }

    /// Return the exact final byte field bound by the prefix.
    pub(crate) fn final_field(&self) -> &'a [u8] {
        self.final_field
    }
}

/// Hash jobs in input order using the canonical CPU stream implementation.
pub(crate) fn hash_last_fields_cpu(
    jobs: &[Digest384LastFieldJob<'_>],
) -> Result<Vec<GoldilocksDigest384V1>, GpuError> {
    let mut digests = Vec::new();
    digests.try_reserve_exact(jobs.len()).map_err(|_| {
        GpuError::InvalidInput("Digest384 batch output exceeds available host memory")
    })?;
    for job in jobs {
        let mut stream = job.prefix;
        stream.update(job.final_field).map_err(|_| {
            GpuError::InvalidInput("Digest384 prepared CPU payload violates its length bound")
        })?;
        digests.push(
            stream.finalize().map_err(|_| {
                GpuError::InvalidInput("Digest384 prepared CPU payload is incomplete")
            })?,
        );
    }
    Ok(digests)
}

/// Attempt actual Metal execution and propagate allocation, launch, or completion errors.
///
/// Empty input returns an empty result without initializing or dispatching a
/// device. Every nonempty successful result requires completed GPU work. The
/// caller may explicitly use [`hash_last_fields_cpu`] after an error.
pub(crate) fn try_hash_last_fields_metal(
    jobs: &[Digest384LastFieldJob<'_>],
) -> Result<Vec<GoldilocksDigest384V1>, GpuError> {
    if jobs.is_empty() {
        return Ok(Vec::new());
    }
    #[cfg(all(feature = "fastpq-gpu", target_os = "macos"))]
    {
        crate::metal::digest384::hash_last_fields(jobs)
    }
    #[cfg(not(all(feature = "fastpq-gpu", target_os = "macos")))]
    {
        Err(GpuError::Unsupported(GpuBackend::Metal))
    }
}

#[cfg(test)]
mod tests {
    use fastpq_isi::{GoldilocksDigestDomainV1, hash_bytes_384_v1};

    use super::*;

    fn domain(index: u64) -> GoldilocksDigestDomainV1<'static> {
        GoldilocksDigestDomainV1 {
            catalog: b"iroha-privacy-exact12-v1",
            protocol: b"fastpq-state-transition-stark-v1",
            profile: b"fastpq-state-transition-stark-v1",
            role: b"fastpq:v1:air-trace",
            phase: b"leaf",
            level: u64::MAX,
            index,
            counter: u64::MAX,
        }
    }

    #[test]
    fn prepared_jobs_reject_consumed_or_wrong_length_streams() {
        let prefix = GoldilocksDigest384LastFieldStreamV1::new(domain(0), &[], 8).unwrap();
        assert!(Digest384LastFieldJob::new(prefix, b"1234567").is_err());
        assert!(Digest384LastFieldJob::new(prefix, b"123456789").is_err());
        for consumed in 1..=8 {
            let mut partially_consumed = prefix;
            partially_consumed.update(&b"12345678"[..consumed]).unwrap();
            assert!(Digest384LastFieldJob::new(partially_consumed, b"12345678").is_err());
        }
        let job = Digest384LastFieldJob::new(prefix, b"12345678").unwrap();
        assert_eq!(job.prefix().received_len(), 0);
        assert_eq!(job.final_field(), b"12345678");
    }

    #[test]
    fn cpu_batches_match_canonical_heterogeneous_frames() {
        let lengths = [0, 1, 6, 7, 8, 13, 14, 15, 27, 28, 29, 135, 136, 137, 512];
        let payloads: Vec<Vec<u8>> = lengths
            .iter()
            .map(|&len| {
                (0..len)
                    .map(|index| ((index * 73 + len) & 255) as u8)
                    .collect()
            })
            .collect();
        let jobs: Vec<_> = payloads
            .iter()
            .enumerate()
            .map(|(index, bytes)| {
                let prefix: &[&[u8]] = if index % 2 == 0 {
                    &[]
                } else {
                    &[b"", b"prefix"]
                };
                Digest384LastFieldJob::new(
                    GoldilocksDigest384LastFieldStreamV1::new(
                        domain(index as u64),
                        prefix,
                        bytes.len(),
                    )
                    .unwrap(),
                    bytes,
                )
                .unwrap()
            })
            .collect();
        let actual = hash_last_fields_cpu(&jobs).unwrap();
        for (index, bytes) in payloads.iter().enumerate() {
            let fields: Vec<&[u8]> = if index % 2 == 0 {
                vec![bytes]
            } else {
                vec![b"", b"prefix", bytes]
            };
            assert_eq!(
                actual[index],
                hash_bytes_384_v1(domain(index as u64), &fields).unwrap()
            );
        }
        assert_eq!(hash_last_fields_cpu(&jobs).unwrap(), actual);
    }

    #[test]
    fn empty_batches_never_require_a_device() {
        assert!(hash_last_fields_cpu(&[]).unwrap().is_empty());
        assert!(try_hash_last_fields_metal(&[]).unwrap().is_empty());
    }

    #[cfg(not(all(feature = "fastpq-gpu", target_os = "macos")))]
    #[test]
    fn unavailable_metal_reports_error_and_cpu_fallback_remains_identical() {
        let stream = GoldilocksDigest384LastFieldStreamV1::new(domain(0), &[], 0).unwrap();
        let jobs = [Digest384LastFieldJob::new(stream, &[]).unwrap()];
        assert!(matches!(
            try_hash_last_fields_metal(&jobs),
            Err(GpuError::Unsupported(GpuBackend::Metal))
        ));
        assert_eq!(
            hash_last_fields_cpu(&jobs).unwrap(),
            vec![stream.finalize().unwrap()]
        );
    }
}
