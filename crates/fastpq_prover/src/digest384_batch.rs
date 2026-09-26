//! Canonical final-byte-field batches for six-lane native-STARK digests.
//!
//! Prepared jobs retain the typed CPU stream as the framing authority. The
//! accelerator only resumes a fresh final field; CPU execution remains an
//! explicit policy choice; required-device failures are returned to the caller.

use fastpq_isi::{GoldilocksDigest384LastFieldStreamV1, GoldilocksDigest384V1};
use rayon::prelude::*;

#[cfg(test)]
use crate::backend::GpuBackend;
use crate::digest_executor::{MAX_DIGEST384_BATCH_FRAMES_V1, MAX_DIGEST384_BATCH_WORDS_V1};
#[cfg(feature = "fastpq-gpu")]
use crate::digest384_gpu::{
    Digest384GpuBackendV1, Digest384GpuErrorV1, Digest384ReadinessV1, backend_readiness_v1,
};
use crate::{DigestExecutionV1, gpu::GpuError};

/// Maximum final byte payload admitted to one continuation dispatch.
pub(crate) const MAX_LAST_FIELD_BYTES: usize = MAX_DIGEST384_BATCH_WORDS_V1 * 8;
/// Existing sensitive Metal pool alignment, checked against its owner on Metal.
pub(crate) const STAGING_PAGE_BYTES: usize = 16 * 1024;

/// Bound the four shared backing buffers, returned digests and fixed readiness payload.
/// Caller-owned job descriptors and source bytes are charged by their caller.
/// The same bound applies to CPU policy; it does not include driver/allocator overhead.
pub(crate) fn last_fields_payload_charge(
    job_count: usize,
    total_final_field_bytes: usize,
) -> crate::Result<usize> {
    last_fields_charge(job_count, total_final_field_bytes).map_err(native_error)
}

fn last_fields_charge(job_count: usize, bytes: usize) -> Result<usize, GpuError> {
    if job_count > MAX_DIGEST384_BATCH_FRAMES_V1 || bytes > MAX_LAST_FIELD_BYTES {
        return Err(GpuError::InvalidInput(
            "Digest384 continuation batch exceeds its job or byte budget",
        ));
    }
    if job_count == 0 {
        return if bytes == 0 {
            Ok(0)
        } else {
            Err(GpuError::InvalidInput(
                "empty continuation batch has payload bytes",
            ))
        };
    }
    let mut total = job_count.checked_mul(48).ok_or(GpuError::InvalidInput(
        "Digest384 continuation charge overflow",
    ))?;
    for size in [
        192 * job_count,
        16 * job_count,
        bytes.max(1),
        48 * job_count,
    ] {
        let rounded = size
            .div_ceil(STAGING_PAGE_BYTES)
            .checked_mul(STAGING_PAGE_BYTES)
            .ok_or(GpuError::InvalidInput(
                "Digest384 continuation page charge overflow",
            ))?;
        total = total.checked_add(rounded).ok_or(GpuError::InvalidInput(
            "Digest384 continuation charge overflow",
        ))?;
    }
    // The independent eight-job public KAT finishes before request staging;
    // charge its four pages and result vector even for a smaller first request.
    // Two extra pages cover cached public lane constants/MDS, the bounded KAT
    // payload/jobs/oracle vectors and staging ownership descriptors. These are
    // counted even on CPU so admission is independent of hardware or warm state.
    Ok(total.max(4 * STAGING_PAGE_BYTES + 8 * 48) + 2 * STAGING_PAGE_BYTES)
}

#[cfg(any(test, feature = "fastpq-gpu"))]
fn validate_jobs(jobs: &[Digest384LastFieldJob<'_>]) -> Result<usize, GpuError> {
    if jobs.len() > MAX_DIGEST384_BATCH_FRAMES_V1 {
        return Err(GpuError::InvalidInput(
            "Digest384 continuation batch exceeds its job budget",
        ));
    }
    let bytes = jobs.iter().try_fold(0usize, |sum, job| {
        sum.checked_add(job.final_field.len())
            .filter(|size| *size <= MAX_LAST_FIELD_BYTES)
            .ok_or(GpuError::InvalidInput(
                "Digest384 continuation batch exceeds its byte budget",
            ))
    })?;
    last_fields_charge(jobs.len(), bytes)?;
    Ok(bytes)
}

fn native_error(error: impl core::fmt::Display) -> crate::Error {
    crate::Error::NativeDigestExecution {
        details: error.to_string(),
    }
}

/// Execute in canonical order under explicit CPU or required-device policy.
/// Common geometry and payload charging precede either policy. Only required
/// device execution constructs typed jobs; CPU preserves the optimized prefix
/// owner without repeating its suffix absorption for unused device state.
pub(crate) fn execute_last_fields_with_cpu<'a>(
    job_count: usize,
    total_final_field_bytes: usize,
    execution: DigestExecutionV1,
    cpu: impl Fn(usize) -> crate::Result<GoldilocksDigest384V1> + Sync,
    _prepare_device: impl FnOnce() -> crate::Result<Vec<Digest384LastFieldJob<'a>>>,
) -> crate::Result<Vec<GoldilocksDigest384V1>> {
    last_fields_charge(job_count, total_final_field_bytes).map_err(native_error)?;
    if job_count == 0 {
        return Ok(Vec::new());
    }
    match execution {
        DigestExecutionV1::Cpu => {
            let mut digests = Vec::new();
            digests.try_reserve_exact(job_count).map_err(|_| {
                native_error("Digest384 batch output exceeds available host memory")
            })?;
            if job_count < 64 {
                for index in 0..job_count {
                    digests.push(cpu(index)?);
                }
            } else {
                let mut results = Vec::new();
                results.try_reserve_exact(job_count).map_err(|_| {
                    native_error("Digest384 CPU result descriptors exceed available host memory")
                })?;
                (0..job_count)
                    .into_par_iter()
                    .map(&cpu)
                    .collect_into_vec(&mut results);
                for result in results {
                    digests.push(result?);
                }
            }
            Ok(digests)
        }
        #[cfg(feature = "fastpq-gpu")]
        DigestExecutionV1::Device(backend) => {
            let jobs = _prepare_device()?;
            let actual_bytes = validate_jobs(&jobs).map_err(native_error)?;
            if jobs.len() != job_count || actual_bytes != total_final_field_bytes {
                return Err(native_error(
                    "prepared continuation jobs differ from admitted geometry",
                ));
            }
            try_hash_last_fields_device(&jobs, backend).map_err(native_error)
        }
    }
}

/// Adapt already-typed diagnostic inputs to the shared lazy executor.
#[cfg(test)]
fn execute_prepared_jobs_with_cpu(
    jobs: &[Digest384LastFieldJob<'_>],
    execution: DigestExecutionV1,
    cpu: impl Fn(usize) -> crate::Result<GoldilocksDigest384V1> + Sync,
) -> crate::Result<Vec<GoldilocksDigest384V1>> {
    let bytes = validate_jobs(jobs).map_err(native_error)?;
    execute_last_fields_with_cpu(jobs.len(), bytes, execution, cpu, || Ok(jobs.to_vec()))
}

/// Diagnostic adapter using the independent canonical streaming CPU path.
#[cfg(test)]
pub(crate) fn execute_last_fields(
    jobs: &[Digest384LastFieldJob<'_>],
    execution: DigestExecutionV1,
) -> crate::Result<Vec<GoldilocksDigest384V1>> {
    execute_prepared_jobs_with_cpu(jobs, execution, |index| {
        hash_last_field_cpu(&jobs[index]).map_err(native_error)
    })
}

/// A fresh canonical typed prefix and its exact final byte field.
#[derive(Clone, Copy)]
pub(crate) struct Digest384LastFieldJob<'a> {
    prefix: GoldilocksDigest384LastFieldStreamV1,
    final_field: &'a [u8],
}

impl core::fmt::Debug for Digest384LastFieldJob<'_> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Digest384LastFieldJob")
            .field("expected_len", &self.prefix.expected_len())
            .finish_non_exhaustive()
    }
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
    #[cfg(any(test, all(feature = "fastpq-gpu", target_os = "macos")))]
    pub(crate) fn prefix(&self) -> &GoldilocksDigest384LastFieldStreamV1 {
        &self.prefix
    }

    /// Return the exact final byte field bound by the prefix.
    #[cfg(any(test, all(feature = "fastpq-gpu", target_os = "macos")))]
    pub(crate) fn final_field(&self) -> &'a [u8] {
        self.final_field
    }
}

/// Hash jobs in input order using the independent canonical CPU stream implementation.
#[cfg(test)]
pub(crate) fn hash_last_fields_cpu(
    jobs: &[Digest384LastFieldJob<'_>],
) -> Result<Vec<GoldilocksDigest384V1>, GpuError> {
    validate_jobs(jobs)?;
    jobs.iter().map(hash_last_field_cpu).collect()
}

#[cfg(test)]
fn hash_last_field_cpu(job: &Digest384LastFieldJob<'_>) -> Result<GoldilocksDigest384V1, GpuError> {
    let mut stream = job.prefix;
    stream.update(job.final_field).map_err(|_| {
        GpuError::InvalidInput("Digest384 prepared CPU payload violates its length bound")
    })?;
    stream
        .finalize()
        .map_err(|_| GpuError::InvalidInput("Digest384 prepared CPU payload is incomplete"))
}

/// Attempt actual Metal execution and propagate allocation, launch, or completion errors.
///
/// Empty input returns an empty result without initializing or dispatching a
/// device. Every nonempty successful result requires completed GPU work. The
/// caller may explicitly use [`hash_last_fields_cpu`] after an error.
#[cfg(test)]
pub(crate) fn try_hash_last_fields_metal(
    jobs: &[Digest384LastFieldJob<'_>],
) -> Result<Vec<GoldilocksDigest384V1>, GpuError> {
    if jobs.is_empty() {
        return Ok(Vec::new());
    }
    #[cfg(feature = "fastpq-gpu")]
    {
        try_hash_last_fields_device(jobs, Digest384GpuBackendV1::Metal).map_err(|error| {
            GpuError::Execution {
                backend: GpuBackend::Metal,
                message: error.to_string(),
            }
        })
    }
    #[cfg(not(feature = "fastpq-gpu"))]
    {
        Err(GpuError::Unsupported(GpuBackend::Metal))
    }
}

#[cfg(feature = "fastpq-gpu")]
fn try_hash_last_fields_device(
    jobs: &[Digest384LastFieldJob<'_>],
    backend: Digest384GpuBackendV1,
) -> Result<Vec<GoldilocksDigest384V1>, Digest384GpuErrorV1> {
    validate_jobs(jobs)
        .map_err(|_| Digest384GpuErrorV1::InvalidInput("invalid continuation batch geometry"))?;
    if jobs.is_empty() {
        return Ok(Vec::new());
    }
    let mut readiness = backend_readiness_v1(backend)
        .lock()
        .map_err(|_| Digest384GpuErrorV1::Quarantined { backend })?;
    readiness.ensure_available_v1(backend)?;
    readiness
        .last_fields
        .execute_last_fields(backend, jobs, &mut |_jobs| match backend {
            Digest384GpuBackendV1::Metal => {
                #[cfg(target_os = "macos")]
                {
                    crate::metal::digest384::hash_last_fields(_jobs).map_err(|error| {
                        Digest384GpuErrorV1::Execution {
                            backend,
                            message: error.to_string(),
                        }
                    })
                }
                #[cfg(not(target_os = "macos"))]
                {
                    Err(Digest384GpuErrorV1::Execution {
                        backend,
                        message: "Metal is unavailable on this platform".to_owned(),
                    })
                }
            }
            Digest384GpuBackendV1::Cuda => Err(Digest384GpuErrorV1::Execution {
                backend,
                message: "Digest384 continuation execution is unavailable on CUDA".to_owned(),
            }),
        })
}

#[cfg(feature = "fastpq-gpu")]
impl Digest384ReadinessV1 {
    fn execute_last_fields(
        &mut self,
        backend: Digest384GpuBackendV1,
        jobs: &[Digest384LastFieldJob<'_>],
        dispatch: &mut impl FnMut(
            &[Digest384LastFieldJob<'_>],
        ) -> Result<Vec<GoldilocksDigest384V1>, Digest384GpuErrorV1>,
    ) -> Result<Vec<GoldilocksDigest384V1>, Digest384GpuErrorV1> {
        validate_jobs(jobs).map_err(|_| {
            Digest384GpuErrorV1::InvalidInput("invalid continuation batch geometry")
        })?;
        if jobs.is_empty() {
            return Ok(Vec::new());
        }
        if *self == Self::Quarantined {
            return Err(Digest384GpuErrorV1::Quarantined { backend });
        }
        if *self == Self::Unchecked {
            use fastpq_isi::{GoldilocksDigestDomainV1, hash_bytes_384_v1};
            let payload = [0xA7; 256];
            let mut kat = Vec::with_capacity(8);
            let mut expected = Vec::with_capacity(8);
            for (index, length) in [0, 1, 7, 8, 14, 15, 96, 256].into_iter().enumerate() {
                let domain = GoldilocksDigestDomainV1 {
                    catalog: b"iroha-privacy-exact12-v1",
                    protocol: b"last-field-public-kat-v1",
                    profile: b"stark-fri-poseidon-x7-goldilocks-6x64-v1",
                    role: b"trace-merkle",
                    phase: b"leaf",
                    level: u64::MAX,
                    index: index as u64,
                    counter: u64::MAX,
                };
                let preceding: &[&[u8]] = if index % 2 == 0 { &[] } else { &[b""] };
                let bytes = &payload[..length];
                let prefix = GoldilocksDigest384LastFieldStreamV1::new(domain, preceding, length)
                    .expect("fixed public KAT prefix");
                kat.push(Digest384LastFieldJob::new(prefix, bytes).expect("fixed public KAT job"));
                let fields: Vec<&[u8]> = preceding.iter().copied().chain([bytes]).collect();
                expected.push(hash_bytes_384_v1(domain, &fields).expect("fixed public KAT hash"));
            }
            *self = Self::Quarantined;
            let actual = dispatch(&kat)?;
            if actual != expected {
                return Err(Digest384GpuErrorV1::Conformance { backend });
            }
            *self = Self::Ready;
        }
        *self = Self::Quarantined;
        let output = dispatch(jobs)?;
        if output.len() != jobs.len() {
            return Err(Digest384GpuErrorV1::Conformance { backend });
        }
        *self = Self::Ready;
        Ok(output)
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
    fn exact_batch_charge_includes_page_rounding_and_public_readiness() {
        let context = 2 * STAGING_PAGE_BYTES;
        assert!(
            (6 * 65 * 3 + 9) * 8
                + 256
                + 8 * (48 + core::mem::size_of::<Digest384LastFieldJob<'_>>())
                < context
        );
        assert!(core::mem::size_of::<crate::Result<GoldilocksDigest384V1>>() + 48 < 304);
        assert_eq!(last_fields_payload_charge(0, 0).unwrap(), 0);
        assert!(last_fields_payload_charge(0, 1).is_err());
        assert_eq!(
            last_fields_payload_charge(1, 0).unwrap(),
            4 * STAGING_PAGE_BYTES + 8 * 48 + context
        );
        for (jobs, bytes) in [(1, 7), (256, 256 * 2408), (65536, MAX_LAST_FIELD_BYTES)] {
            let page = |size: usize| size.max(1).div_ceil(STAGING_PAGE_BYTES) * STAGING_PAGE_BYTES;
            let expected =
                (page(192 * jobs) + page(16 * jobs) + page(bytes) + page(48 * jobs) + 48 * jobs)
                    .max(4 * STAGING_PAGE_BYTES + 8 * 48)
                    + context;
            assert_eq!(last_fields_payload_charge(jobs, bytes).unwrap(), expected);
        }
        for (jobs, bytes) in [
            (65537, 0),
            (1, MAX_LAST_FIELD_BYTES + 1),
            (usize::MAX, usize::MAX),
        ] {
            assert!(last_fields_payload_charge(jobs, bytes).is_err());
        }
    }

    #[test]
    fn explicit_cpu_policy_matches_direct_canonical_results() {
        let bytes = b"explicit CPU continuation";
        let prefix =
            GoldilocksDigest384LastFieldStreamV1::new(domain(9), &[], bytes.len()).unwrap();
        let job = Digest384LastFieldJob::new(prefix, bytes).unwrap();
        assert_eq!(
            execute_last_fields(&[job], DigestExecutionV1::Cpu).unwrap(),
            hash_last_fields_cpu(&[job]).unwrap()
        );
        let expected = hash_last_field_cpu(&job).unwrap();
        for threads in [1, 2, 6] {
            let pool = rayon::ThreadPoolBuilder::new()
                .num_threads(threads)
                .build()
                .unwrap();
            for count in [63, 64, 65, 256] {
                let jobs = vec![job; count];
                assert_eq!(
                    pool.install(|| execute_last_fields(&jobs, DigestExecutionV1::Cpu))
                        .unwrap(),
                    vec![expected; count]
                );
            }
        }
        let excessive = vec![job; MAX_DIGEST384_BATCH_FRAMES_V1 + 1];
        assert!(hash_last_fields_cpu(&excessive).is_err());
        assert!(execute_last_fields(&excessive, DigestExecutionV1::Cpu).is_err());
        let bytes = [0_u8; 1024];
        let prefix =
            GoldilocksDigest384LastFieldStreamV1::new(domain(0), &[], bytes.len()).unwrap();
        let large = Digest384LastFieldJob::new(prefix, &bytes).unwrap();
        let excessive_bytes = vec![large; MAX_LAST_FIELD_BYTES / bytes.len() + 1];
        assert!(validate_jobs(&excessive_bytes).is_err());
        assert!(
            execute_last_fields(&[], DigestExecutionV1::Cpu)
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn cpu_policy_never_prepares_device_prefixes_and_checks_common_geometry() {
        let expected = hash_bytes_384_v1(domain(0), &[b"public CPU callback"]).unwrap();
        for count in [0, 1, 63, 64, 256] {
            assert_eq!(
                execute_last_fields_with_cpu(
                    count,
                    count * 2408,
                    DigestExecutionV1::Cpu,
                    |_| Ok(expected),
                    || panic!("CPU must not prepare device prefixes")
                )
                .unwrap(),
                vec![expected; count]
            );
        }
        for (count, bytes) in [
            (0, 1),
            (MAX_DIGEST384_BATCH_FRAMES_V1 + 1, 0),
            (1, MAX_LAST_FIELD_BYTES + 1),
        ] {
            assert!(
                execute_last_fields_with_cpu(
                    count,
                    bytes,
                    DigestExecutionV1::Cpu,
                    |_| panic!("invalid CPU callback"),
                    || panic!("invalid device preparation")
                )
                .is_err()
            );
        }
    }

    #[cfg(feature = "fastpq-gpu")]
    #[test]
    fn lazy_device_jobs_must_match_admitted_geometry_before_dispatch() {
        let backend = DigestExecutionV1::Device(Digest384GpuBackendV1::Cuda);
        let prefix = GoldilocksDigest384LastFieldStreamV1::new(domain(0), &[], 1).unwrap();
        let job = Digest384LastFieldJob::new(prefix, &[9]).unwrap();
        for (count, bytes, prepared) in [(1, 1, vec![]), (1, 0, vec![job]), (2, 2, vec![job])] {
            let error = execute_last_fields_with_cpu(
                count,
                bytes,
                backend,
                |_| panic!("device must not call CPU"),
                || Ok(prepared),
            )
            .unwrap_err();
            assert!(error.to_string().contains("differ from admitted geometry"));
        }
        for (count, bytes) in [
            (0, 1),
            (MAX_DIGEST384_BATCH_FRAMES_V1 + 1, 0),
            (1, MAX_LAST_FIELD_BYTES + 1),
        ] {
            assert!(
                execute_last_fields_with_cpu(
                    count,
                    bytes,
                    backend,
                    |_| panic!("invalid CPU callback"),
                    || panic!("invalid device preparation")
                )
                .is_err()
            );
        }
        assert!(
            execute_last_fields_with_cpu(
                0,
                0,
                backend,
                |_| panic!("empty CPU callback"),
                || panic!("empty device preparation")
            )
            .unwrap()
            .is_empty()
        );
    }

    #[test]
    fn cpu_callback_order_and_errors_remain_deterministic() {
        let prefix = GoldilocksDigest384LastFieldStreamV1::new(domain(0), &[], 0).unwrap();
        let jobs = vec![Digest384LastFieldJob::new(prefix, &[]).unwrap(); 256];
        let digest = hash_last_field_cpu(&jobs[0]).unwrap();
        assert_eq!(
            format!("{:?}", jobs[0]),
            "Digest384LastFieldJob { expected_len: 0, .. }"
        );
        for threads in [1, 2, 6] {
            let pool = rayon::ThreadPoolBuilder::new()
                .num_threads(threads)
                .build()
                .unwrap();
            let actual = pool.install(|| {
                execute_prepared_jobs_with_cpu(&jobs, DigestExecutionV1::Cpu, |index| {
                    if index == 17 || index == 63 {
                        Err(native_error(format!("public diagnostic at {index}")))
                    } else {
                        Ok(digest)
                    }
                })
            });
            assert!(
                actual
                    .unwrap_err()
                    .to_string()
                    .ends_with("public diagnostic at 17")
            );
            let expected = (0..jobs.len())
                .map(|index| {
                    hash_bytes_384_v1(domain(index as u64), &[b"ordered callback".as_slice()])
                        .unwrap()
                })
                .collect::<Vec<_>>();
            let actual = pool
                .install(|| {
                    execute_prepared_jobs_with_cpu(&jobs, DigestExecutionV1::Cpu, |index| {
                        Ok(expected[index])
                    })
                })
                .unwrap();
            assert_eq!(actual, expected);
        }
        assert!(
            execute_prepared_jobs_with_cpu(&[], DigestExecutionV1::Cpu, |_| panic!(
                "empty batch callback"
            ))
            .unwrap()
            .is_empty()
        );
        let invalid = vec![jobs[0]; MAX_DIGEST384_BATCH_FRAMES_V1 + 1];
        assert!(
            execute_prepared_jobs_with_cpu(&invalid, DigestExecutionV1::Cpu, |_| panic!(
                "invalid batch callback"
            ))
            .is_err()
        );
    }

    #[test]
    fn cpu_callback_panic_joins_workers_before_returning() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        struct Active<'a>(&'a AtomicUsize);
        impl Drop for Active<'_> {
            fn drop(&mut self) {
                self.0.fetch_sub(1, Ordering::SeqCst);
            }
        }
        let prefix = GoldilocksDigest384LastFieldStreamV1::new(domain(0), &[], 0).unwrap();
        let jobs = vec![Digest384LastFieldJob::new(prefix, &[]).unwrap(); 256];
        let digest = hash_last_field_cpu(&jobs[0]).unwrap();
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(6)
            .build()
            .unwrap();
        let active = AtomicUsize::new(0);
        let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            pool.install(|| {
                execute_prepared_jobs_with_cpu(&jobs, DigestExecutionV1::Cpu, |index| {
                    active.fetch_add(1, Ordering::SeqCst);
                    let _active = Active(&active);
                    assert_ne!(index, 17, "injected public CPU failure");
                    std::thread::yield_now();
                    Ok(digest)
                })
            })
        }));
        assert!(panic.is_err());
        assert_eq!(active.load(Ordering::SeqCst), 0);
    }

    #[cfg(feature = "fastpq-gpu")]
    #[test]
    fn required_device_never_substitutes_cpu_callback() {
        let prefix = GoldilocksDigest384LastFieldStreamV1::new(domain(0), &[], 0).unwrap();
        let jobs = [Digest384LastFieldJob::new(prefix, &[]).unwrap()];
        assert!(
            execute_prepared_jobs_with_cpu(
                &jobs,
                DigestExecutionV1::Device(Digest384GpuBackendV1::Cuda),
                |_| panic!("required-device CPU substitution")
            )
            .is_err()
        );
    }

    #[cfg(feature = "fastpq-gpu")]
    #[test]
    fn continuation_readiness_precedes_request_and_runs_own_kernel_kat_once() {
        let bytes = b"request distinct from public KAT";
        let prefix =
            GoldilocksDigest384LastFieldStreamV1::new(domain(19), &[], bytes.len()).unwrap();
        let jobs = [Digest384LastFieldJob::new(prefix, bytes).unwrap()];
        let mut readiness = Digest384ReadinessV1::Unchecked;
        let mut calls = 0;
        let mut positions = [false; 2];
        for _ in 0..2 {
            let output = readiness
                .execute_last_fields(Digest384GpuBackendV1::Metal, &jobs, &mut |batch| {
                    if calls == 0 {
                        assert_eq!(batch.len(), 8);
                        assert!(batch.iter().all(|job| job.final_field() != bytes));
                        for job in batch {
                            positions
                                [job.prefix().lane_prefix_v1(0).unwrap().next_rate_position()] =
                                true;
                        }
                    } else {
                        assert_eq!(batch[0].final_field(), bytes);
                    }
                    calls += 1;
                    Ok(hash_last_fields_cpu(batch).unwrap())
                })
                .unwrap();
            assert_eq!(output, hash_last_fields_cpu(&jobs).unwrap());
            assert_eq!(readiness, Digest384ReadinessV1::Ready);
        }
        assert_eq!(calls, 3);
        assert_eq!(positions, [true, true]);
    }

    #[cfg(feature = "fastpq-gpu")]
    #[test]
    fn continuation_readiness_and_dispatch_failures_close_all_backend_capabilities() {
        let prefix = GoldilocksDigest384LastFieldStreamV1::new(domain(0), &[], 0).unwrap();
        let jobs = [Digest384LastFieldJob::new(prefix, &[]).unwrap()];
        for (initial, wrong_output) in [
            (Digest384ReadinessV1::Unchecked, false),
            (Digest384ReadinessV1::Unchecked, true),
            (Digest384ReadinessV1::Ready, false),
            (Digest384ReadinessV1::Ready, true),
        ] {
            let mut readiness = initial;
            let result =
                readiness.execute_last_fields(Digest384GpuBackendV1::Metal, &jobs, &mut |_| {
                    if wrong_output {
                        Ok(Vec::new())
                    } else {
                        Err(Digest384GpuErrorV1::Execution {
                            backend: Digest384GpuBackendV1::Metal,
                            message: "injected completion failure".to_owned(),
                        })
                    }
                });
            assert!(result.is_err());
            assert_eq!(readiness, Digest384ReadinessV1::Quarantined);
            assert!(
                readiness
                    .execute_last_fields(Digest384GpuBackendV1::Metal, &jobs, &mut |_| panic!(
                        "quarantined request dispatched"
                    ))
                    .is_err()
            );
            let backend = crate::digest384_gpu::Digest384BackendReadinessV1 {
                frames: Digest384ReadinessV1::Ready,
                indexed: Digest384ReadinessV1::Ready,
                last_fields: readiness,
            };
            assert!(
                backend
                    .ensure_available_v1(Digest384GpuBackendV1::Metal)
                    .is_err()
            );
        }
    }

    #[cfg(feature = "fastpq-gpu")]
    #[test]
    fn continuation_geometry_and_empty_batches_precede_readiness_or_payload_dispatch() {
        let prefix = GoldilocksDigest384LastFieldStreamV1::new(domain(0), &[], 0).unwrap();
        let job = Digest384LastFieldJob::new(prefix, &[]).unwrap();
        let mut readiness = Digest384ReadinessV1::Unchecked;
        let excessive = vec![job; MAX_DIGEST384_BATCH_FRAMES_V1 + 1];
        assert!(
            readiness
                .execute_last_fields(Digest384GpuBackendV1::Metal, &excessive, &mut |_| panic!(
                    "invalid request dispatched"
                ))
                .is_err()
        );
        assert!(
            readiness
                .execute_last_fields(Digest384GpuBackendV1::Metal, &[], &mut |_| panic!(
                    "empty request dispatched"
                ))
                .unwrap()
                .is_empty()
        );
        assert_eq!(readiness, Digest384ReadinessV1::Unchecked);
        let mut readiness = Digest384ReadinessV1::Ready;
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            readiness.execute_last_fields(Digest384GpuBackendV1::Metal, &[job], &mut |_| {
                panic!("injected device panic")
            })
        }));
        assert!(result.is_err());
        assert_eq!(readiness, Digest384ReadinessV1::Quarantined);
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

    #[cfg(not(feature = "fastpq-gpu"))]
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
