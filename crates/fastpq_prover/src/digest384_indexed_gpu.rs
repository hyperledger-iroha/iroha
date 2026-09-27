//! Bounded indexed first-coordinate execution for the canonical nonce predicate.
use crate::digest_executor::{MAX_DIGEST384_BATCH_WORDS_V1, MAX_DIGEST384_INDEXED_BATCH_V1};
use crate::digest384_gpu::{
    Digest384GpuBackendV1, Digest384GpuErrorV1, Digest384ReadinessV1, backend_readiness_v1,
};
use fastpq_isi::poseidon_digest384::GoldilocksDigest384IndexedPredicateV1;
use fastpq_isi::{GoldilocksDigest384FrameV1, GoldilocksDigestDomainV1};
use zeroize::Zeroizing;

pub(crate) struct StagedDigest384IndexedV1 {
    pub(crate) words: Zeroizing<Vec<u64>>,
    pub(crate) start: u64,
    pub(crate) count: usize,
}
fn validate_geometry_v1(
    predicate: &GoldilocksDigest384IndexedPredicateV1<'_>,
    start: u64,
    count: usize,
) -> Result<usize, Digest384GpuErrorV1> {
    if count == 0
        || count > MAX_DIGEST384_INDEXED_BATCH_V1
        || start.checked_add((count - 1) as u64).is_none()
    {
        return Err(Digest384GpuErrorV1::InvalidInput(
            "indexed range exceeds the bounded nonwrapping batch",
        ));
    }
    predicate
        .staged_word_count_v1()
        .filter(|words| *words <= MAX_DIGEST384_BATCH_WORDS_V1)
        .ok_or(Digest384GpuErrorV1::InvalidInput(
            "indexed cache exceeds the staging word budget",
        ))
}
fn zeroed_words_v1(len: usize) -> Result<Zeroizing<Vec<u64>>, Digest384GpuErrorV1> {
    let mut words = Zeroizing::new(Vec::new());
    words
        .try_reserve_exact(len)
        .map_err(|_| Digest384GpuErrorV1::InvalidInput("indexed staging allocation failed"))?;
    words.resize(len, 0);
    Ok(words)
}
impl StagedDigest384IndexedV1 {
    fn new(
        predicate: &GoldilocksDigest384IndexedPredicateV1<'_>,
        start: u64,
        count: usize,
    ) -> Result<Self, Digest384GpuErrorV1> {
        let size = validate_geometry_v1(predicate, start, count)?;
        let mut words = zeroed_words_v1(size)?;
        if !predicate.write_staged_words_v1(&mut words) {
            return Err(Digest384GpuErrorV1::InvalidInput(
                "canonical indexed cache emission failed",
            ));
        }
        Ok(Self {
            words,
            start,
            count,
        })
    }
}

pub(crate) fn try_indexed_coordinates_v1(
    backend: Digest384GpuBackendV1,
    predicate: &GoldilocksDigest384IndexedPredicateV1<'_>,
    start: u64,
    count: usize,
) -> Result<Vec<u64>, Digest384GpuErrorV1> {
    validate_geometry_v1(predicate, start, count)?;
    let mut readiness = backend_readiness_v1(backend)
        .lock()
        .map_err(|_| Digest384GpuErrorV1::Quarantined { backend })?;
    readiness.ensure_available_v1(backend)?;
    readiness.indexed.execute_indexed_v1(
        backend,
        || StagedDigest384IndexedV1::new(predicate, start, count),
        &mut |staged, output| {
            let result = match backend {
                Digest384GpuBackendV1::Metal => {
                    #[cfg(target_os = "macos")]
                    {
                        crate::metal::digest384_indexed_coordinates_v1(staged, output)
                            .map_err(|error| error.to_string())
                    }
                    #[cfg(not(target_os = "macos"))]
                    {
                        Err("Metal is unavailable on this platform".to_owned())
                    }
                }
                Digest384GpuBackendV1::Cuda => {
                    Err("indexed coordinate execution is unavailable on CUDA".to_owned())
                }
            };
            result.map_err(|message| Digest384GpuErrorV1::Execution { backend, message })
        },
    )
}

fn kat_domain_v1(index: u64) -> GoldilocksDigestDomainV1<'static> {
    GoldilocksDigestDomainV1 {
        catalog: b"iroha-privacy-exact12-v1",
        protocol: b"indexed-predicate-public-kat-v1",
        profile: b"stark-fri-poseidon-x7-goldilocks-6x64-v1",
        role: b"grinding",
        phase: b"proof-of-work-nonce",
        level: 0,
        index,
        counter: 0,
    }
}
const KAT_SEED_V1: [u8; 48] = [0xA7; 48];
fn kat_coordinate_v1(index: u64) -> u64 {
    GoldilocksDigest384FrameV1::new(kat_domain_v1(index), &[&KAT_SEED_V1])
        .expect("fixed public frame")
        .hash()
        .words()[0]
}
fn validate_coordinates_v1(words: &[u64]) -> Result<(), Digest384GpuErrorV1> {
    if words
        .iter()
        .any(|word| *word >= fastpq_isi::poseidon::FIELD_MODULUS)
    {
        return Err(Digest384GpuErrorV1::NoncanonicalOutput);
    }
    Ok(())
}
impl Digest384ReadinessV1 {
    fn execute_indexed_v1(
        &mut self,
        backend: Digest384GpuBackendV1,
        stage_request: impl FnOnce() -> Result<StagedDigest384IndexedV1, Digest384GpuErrorV1>,
        dispatch: &mut impl FnMut(
            &StagedDigest384IndexedV1,
            &mut [u64],
        ) -> Result<(), Digest384GpuErrorV1>,
    ) -> Result<Vec<u64>, Digest384GpuErrorV1> {
        if *self == Self::Quarantined {
            return Err(Digest384GpuErrorV1::Quarantined { backend });
        }
        if *self == Self::Unchecked {
            let fields: &[&[u8]] = &[&KAT_SEED_V1];
            let frame = GoldilocksDigest384FrameV1::new(kat_domain_v1(0), fields)
                .expect("public KAT frame");
            let predicate = frame.indexed_predicate_v1();
            for start in [0, (1_u64 << 56) - 1, u64::MAX - 2] {
                let staged = StagedDigest384IndexedV1::new(&predicate, start, 3)?;
                let mut output = zeroed_words_v1(3)?;
                *self = Self::Quarantined;
                dispatch(&staged, &mut output)?;
                validate_coordinates_v1(&output)?;
                if output
                    .iter()
                    .enumerate()
                    .any(|(offset, actual)| *actual != kat_coordinate_v1(start + offset as u64))
                {
                    return Err(Digest384GpuErrorV1::Conformance { backend });
                }
            }
            *self = Self::Ready;
        }
        let staged = stage_request()?;
        let mut output = zeroed_words_v1(staged.count)?;
        *self = Self::Quarantined;
        dispatch(&staged, &mut output)?;
        validate_coordinates_v1(&output)?;
        *self = Self::Ready;
        Ok(core::mem::take(&mut *output))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "requires real Metal indexed execution; never substitutes CPU output"]
    fn indexed_metal_matches_complete_scalar_hashes_at_nonce_boundaries() {
        let _gpu_lane = crate::backend::acquire_gpu_lane();
        for length in [0, 6, 7, 8, 256] {
            let payload: Vec<u8> = (0..length)
                .map(|i| ((i * 73 + length) & 255) as u8)
                .collect();
            let fields: &[&[u8]] = &[b"indexed-public-boundary", &payload, b""];
            let domain = kat_domain_v1(0);
            let frame = GoldilocksDigest384FrameV1::new(domain, fields).unwrap();
            let predicate = frame.indexed_predicate_v1();
            for start in [0, (1_u64 << 56) - 1, u64::MAX - 2] {
                let actual =
                    try_indexed_coordinates_v1(Digest384GpuBackendV1::Metal, &predicate, start, 3)
                        .expect("actual Metal indexed-coordinate completion");
                let expected = (0..3)
                    .map(|offset| {
                        let mut indexed_domain = domain;
                        indexed_domain.index = start + offset;
                        GoldilocksDigest384FrameV1::new(indexed_domain, fields)
                            .unwrap()
                            .hash()
                            .words()[0]
                    })
                    .collect::<Vec<_>>();
                assert_eq!(
                    actual, expected,
                    "payload length {length}, nonce start {start}"
                );
            }
        }
    }
    #[test]
    fn indexed_readiness_precedes_request_staging_and_quarantines_failure() {
        let backend = Digest384GpuBackendV1::Metal;
        let mut readiness = Digest384ReadinessV1::Unchecked;
        let error = readiness.execute_indexed_v1(
            backend,
            || panic!("no request before KAT"),
            &mut |_, _| {
                Err(Digest384GpuErrorV1::Execution {
                    backend,
                    message: "injected".into(),
                })
            },
        );
        assert!(error.is_err());
        assert_eq!(readiness, Digest384ReadinessV1::Quarantined);
        assert!(
            readiness
                .execute_indexed_v1(
                    backend,
                    || panic!("no quarantined staging"),
                    &mut |_, _| panic!("no quarantined dispatch")
                )
                .is_err()
        );
    }
    #[test]
    fn indexed_readiness_requires_exact_full_coordinate_kats() {
        let backend = Digest384GpuBackendV1::Metal;
        let mut readiness = Digest384ReadinessV1::Unchecked;
        assert!(matches!(
            readiness.execute_indexed_v1(
                backend,
                || panic!("wrong KAT must close"),
                &mut |_, output| {
                    output.fill(0);
                    Ok(())
                }
            ),
            Err(Digest384GpuErrorV1::Conformance { .. })
        ));
        assert_eq!(readiness, Digest384ReadinessV1::Quarantined);
    }
    #[test]
    fn indexed_kats_run_once_and_noncanonical_execution_closes_backend() {
        let fields: &[&[u8]] = &[&KAT_SEED_V1];
        let frame = GoldilocksDigest384FrameV1::new(kat_domain_v1(0), fields).unwrap();
        let predicate = frame.indexed_predicate_v1();
        let backend = Digest384GpuBackendV1::Metal;
        let mut readiness = Digest384ReadinessV1::Unchecked;
        let mut calls = 0;
        for _ in 0..2 {
            let actual = readiness
                .execute_indexed_v1(
                    backend,
                    || StagedDigest384IndexedV1::new(&predicate, 7, 2),
                    &mut |staged, output| {
                        calls += 1;
                        for (offset, word) in output.iter_mut().enumerate() {
                            *word = kat_coordinate_v1(staged.start + offset as u64);
                        }
                        Ok(())
                    },
                )
                .unwrap();
            assert_eq!(actual, [kat_coordinate_v1(7), kat_coordinate_v1(8)]);
        }
        assert_eq!(calls, 5);
        assert!(matches!(
            readiness.execute_indexed_v1(
                backend,
                || StagedDigest384IndexedV1::new(&predicate, 0, 1),
                &mut |_, output| {
                    output.fill(fastpq_isi::poseidon::FIELD_MODULUS);
                    Ok(())
                }
            ),
            Err(Digest384GpuErrorV1::NoncanonicalOutput)
        ));
        assert_eq!(readiness, Digest384ReadinessV1::Quarantined);
    }
    #[test]
    fn indexed_ranges_reject_empty_oversized_and_wrapping_batches() {
        let frame = GoldilocksDigest384FrameV1::new(kat_domain_v1(0), &[]).unwrap();
        let predicate = frame.indexed_predicate_v1();
        for (start, count) in [(0, 0), (0, 4097), (u64::MAX, 2)] {
            assert!(StagedDigest384IndexedV1::new(&predicate, start, count).is_err());
        }
        assert!(StagedDigest384IndexedV1::new(&predicate, u64::MAX, 1).is_ok());
    }
    #[test]
    fn indexed_failure_closes_both_backend_capabilities() {
        let state = crate::digest384_gpu::Digest384BackendReadinessV1 {
            frames: Digest384ReadinessV1::Ready,
            indexed: Digest384ReadinessV1::Quarantined,
            last_fields: Digest384ReadinessV1::Ready,
        };
        assert!(
            state
                .ensure_available_v1(Digest384GpuBackendV1::Metal)
                .is_err()
        );
    }
}
