//! Explicit bounded hardware execution of the canonical six-lane digest frame.
use std::sync::{Mutex, OnceLock};

use fastpq_isi::poseidon_digest384::{
    GOLDILOCKS_DIGEST384_ROUNDS_V1, goldilocks_digest384_lane_initial_state_v1,
    goldilocks_digest384_lane_round_constants_v1,
};
use fastpq_isi::{GoldilocksDigest384FrameV1, GoldilocksDigest384V1};
use zeroize::Zeroizing;

/// Maximum number of independent frames in one hardware dispatch.
pub const MAX_DIGEST384_GPU_FRAMES_V1: usize = 65_536;
/// Maximum cumulative canonical words (32 MiB) in one hardware dispatch.
///
/// These resource limits do not change the protocol's canonical framing limits.
pub const MAX_DIGEST384_GPU_WORDS_V1: usize = 4_194_304;

/// Explicit hardware selection for six-lane frame hashing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Digest384GpuBackendV1 {
    /// Apple's Metal compute backend.
    Metal,
    /// NVIDIA CUDA compute backend (SM80 or newer).
    Cuda,
}

/// A rejected frame dispatch; no CPU substitution is performed.
#[derive(Debug, thiserror::Error)]
pub enum Digest384GpuErrorV1 {
    /// The input exceeds the bounded dispatch geometry or allocation budget.
    #[error("invalid six-lane GPU batch: {0}")]
    InvalidInput(&'static str),
    /// The explicitly requested backend could not execute the frame batch.
    #[error("six-lane {backend:?} execution failed: {message}")]
    Execution {
        /// Backend explicitly selected by the caller.
        backend: Digest384GpuBackendV1,
        /// Diagnostic containing no staged frame payload.
        message: String,
    },
    /// A device returned a noncanonical Goldilocks field word.
    #[error("six-lane GPU output contains a noncanonical field word")]
    NoncanonicalOutput,
    /// The requested backend failed the six-lane public known-answer check.
    #[error("six-lane {backend:?} backend failed its canonical known-answer check")]
    Conformance {
        /// Backend which returned an incorrect canonical digest.
        backend: Digest384GpuBackendV1,
    },
    /// A previous readiness or execution failure permanently closed this backend.
    #[error("six-lane {backend:?} backend is quarantined")]
    Quarantined {
        /// Backend which cannot receive further staged payloads.
        backend: Digest384GpuBackendV1,
    },
}

/// Hash canonical frames on exactly the requested device backend.
///
/// Every digest uses six distinct generated IV/round-constant lanes. Input staging
/// is wiped when ownership is released; uncertain device completion retains and
/// quarantines device-visible allocations. Empty batches and resource-limit
/// violations are rejected before device access. Each backend must pass the
/// canonical six-lane public known-answer check before receiving requested
/// payloads; failed readiness, execution, or canonical-output validation
/// permanently quarantines that backend. This primitive API does not qualify
/// or enable the complete native V1 GPU prover.
pub fn try_hash_digest384_frames_v1(
    backend: Digest384GpuBackendV1,
    frames: &[GoldilocksDigest384FrameV1<'_>],
) -> Result<Vec<GoldilocksDigest384V1>, Digest384GpuErrorV1> {
    validate_frame_batch_geometry(frames)?;
    static METAL: Mutex<Digest384ReadinessV1> = Mutex::new(Digest384ReadinessV1::Unchecked);
    static CUDA: Mutex<Digest384ReadinessV1> = Mutex::new(Digest384ReadinessV1::Unchecked);
    let state = match backend {
        Digest384GpuBackendV1::Metal => &METAL,
        Digest384GpuBackendV1::Cuda => &CUDA,
    };
    let mut readiness = state
        .lock()
        .map_err(|_| Digest384GpuErrorV1::Quarantined { backend })?;
    readiness.execute(
        backend,
        || StagedDigest384V1::new(frames),
        &mut |staged, output| {
            let result = match backend {
                Digest384GpuBackendV1::Metal => {
                    #[cfg(target_os = "macos")]
                    {
                        crate::metal::digest384_hash_frames_v1(staged, output)
                            .map_err(|e| e.to_string())
                    }
                    #[cfg(not(target_os = "macos"))]
                    {
                        Err("Metal is unavailable on this platform".to_owned())
                    }
                }
                Digest384GpuBackendV1::Cuda => {
                    crate::fastpq_cuda::digest384_hash_frames_v1(staged, output)
                        .map_err(|e| e.to_string())
                }
            };
            result.map_err(|message| Digest384GpuErrorV1::Execution { backend, message })
        },
    )
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Digest384ReadinessV1 {
    Unchecked,
    Ready,
    Quarantined,
}

impl Digest384ReadinessV1 {
    fn execute(
        &mut self,
        backend: Digest384GpuBackendV1,
        stage_request: impl FnOnce() -> Result<StagedDigest384V1, Digest384GpuErrorV1>,
        dispatch: &mut impl FnMut(&StagedDigest384V1, &mut [u64]) -> Result<(), Digest384GpuErrorV1>,
    ) -> Result<Vec<GoldilocksDigest384V1>, Digest384GpuErrorV1> {
        if *self == Self::Quarantined {
            return Err(Digest384GpuErrorV1::Quarantined { backend });
        }
        if *self == Self::Unchecked {
            use fastpq_isi::GoldilocksDigestDomainV1;
            let domain = GoldilocksDigestDomainV1 {
                catalog: b"iroha-privacy-exact12-v1",
                protocol: b"test-protocol-v1",
                profile: b"stark-fri-poseidon-x7-goldilocks-6x64-v1",
                role: b"trace-merkle",
                phase: b"leaf",
                level: 0,
                index: 7,
                counter: 0,
            };
            let fields: &[&[u8]] = &[b"payload"];
            let frame =
                GoldilocksDigest384FrameV1::new(domain, fields).expect("fixed public KAT frame");
            let staged = StagedDigest384V1::new(&[frame])?;
            let mut output = zeroed_words(6)?;
            // Mark closed before device access; even a panic cannot leave a
            // partially checked device admitted. No recursive public dispatch.
            *self = Self::Quarantined;
            dispatch(&staged, &mut output)?;
            let actual = decode_digest384_output_v1(&output)?;
            if actual != [frame.hash()] {
                return Err(Digest384GpuErrorV1::Conformance { backend });
            }
            *self = Self::Ready;
        }
        let staged = stage_request()?;
        let mut output = zeroed_words(staged.frame_count * 6)?;
        *self = Self::Quarantined;
        dispatch(&staged, &mut output)?;
        let digests = decode_digest384_output_v1(&output)?;
        *self = Self::Ready;
        Ok(digests)
    }
}

fn decode_digest384_output_v1(
    output: &[u64],
) -> Result<Vec<GoldilocksDigest384V1>, Digest384GpuErrorV1> {
    let mut digests = Vec::new();
    digests
        .try_reserve_exact(output.len() / 6)
        .map_err(|_| Digest384GpuErrorV1::InvalidInput("output allocation failed"))?;
    for words in output.chunks_exact(6) {
        digests.push(
            GoldilocksDigest384V1::new(words.try_into().expect("six-word chunk"))
                .ok_or(Digest384GpuErrorV1::NoncanonicalOutput)?,
        );
    }
    Ok(digests)
}

fn validate_frame_batch_geometry(
    frames: &[GoldilocksDigest384FrameV1<'_>],
) -> Result<usize, Digest384GpuErrorV1> {
    if frames.is_empty() || frames.len() > MAX_DIGEST384_GPU_FRAMES_V1 {
        return Err(Digest384GpuErrorV1::InvalidInput(
            "frame count outside 1..=65,536",
        ));
    }
    frames.iter().try_fold(0usize, |count, frame| {
        count
            .checked_add(frame.word_count())
            .filter(|n| *n <= MAX_DIGEST384_GPU_WORDS_V1)
            .ok_or(Digest384GpuErrorV1::InvalidInput(
                "canonical word count exceeds 4,194,304",
            ))
    })
}

// GPU descriptor layout is three u64 words: offset, padded length, lane-word
// index. All values are bounded by u32, with no raw payload Debug implementation.
pub(crate) struct StagedDigest384V1 {
    pub(crate) words: Zeroizing<Vec<u64>>,
    pub(crate) descriptors: Zeroizing<Vec<u64>>,
    pub(crate) frame_count: usize,
}

fn zeroed_words(len: usize) -> Result<Zeroizing<Vec<u64>>, Digest384GpuErrorV1> {
    let mut words = Zeroizing::new(Vec::new());
    words
        .try_reserve_exact(len)
        .map_err(|_| Digest384GpuErrorV1::InvalidInput("staging allocation failed"))?;
    words.resize(len, 0);
    Ok(words)
}

impl StagedDigest384V1 {
    fn new(frames: &[GoldilocksDigest384FrameV1<'_>]) -> Result<Self, Digest384GpuErrorV1> {
        let word_count = validate_frame_batch_geometry(frames)?;
        let mut words = zeroed_words(word_count)?;
        let mut descriptors = zeroed_words(frames.len() * 3)?;
        let mut offset = 0usize;
        for (index, frame) in frames.iter().enumerate() {
            let end = offset + frame.word_count();
            if !frame.write_lane_words(0, &mut words[offset..end]) {
                return Err(Digest384GpuErrorV1::InvalidInput(
                    "canonical frame emission failed",
                ));
            }
            descriptors[index * 3..index * 3 + 3].copy_from_slice(&[
                offset as u64,
                frame.word_count() as u64,
                frame.lane_word_index() as u64,
            ]);
            offset = end;
        }
        Ok(Self {
            words,
            descriptors,
            frame_count: frames.len(),
        })
    }
}

pub(crate) const DIGEST384_GPU_PARAMETER_WORDS_V1: usize = 6 * 3 + 6 * 65 * 3 + 9;

pub(crate) fn digest384_gpu_parameters_v1() -> &'static [u64; DIGEST384_GPU_PARAMETER_WORDS_V1] {
    static PARAMETERS: OnceLock<[u64; DIGEST384_GPU_PARAMETER_WORDS_V1]> = OnceLock::new();
    PARAMETERS.get_or_init(|| {
        let mut parameters = [0u64; DIGEST384_GPU_PARAMETER_WORDS_V1];
        for lane in 0..6 {
            parameters[lane * 3..lane * 3 + 3].copy_from_slice(
                &goldilocks_digest384_lane_initial_state_v1(lane).expect("canonical lane"),
            );
            for round in 0..GOLDILOCKS_DIGEST384_ROUNDS_V1 {
                let offset = 18 + (lane * 65 + round) * 3;
                parameters[offset..offset + 3].copy_from_slice(
                    &goldilocks_digest384_lane_round_constants_v1(lane, round)
                        .expect("canonical lane and round"),
                );
            }
        }
        for (row, values) in fastpq_isi::poseidon::MDS.iter().enumerate() {
            parameters[1188 + row * 3..1188 + row * 3 + 3].copy_from_slice(values);
        }
        parameters
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use fastpq_isi::GoldilocksDigestDomainV1;

    fn domain() -> GoldilocksDigestDomainV1<'static> {
        GoldilocksDigestDomainV1 {
            catalog: b"fastpq:v1",
            protocol: b"stark",
            profile: b"goldilocks-fp4",
            role: b"hardware-parity",
            phase: b"frame",
            level: u64::MAX,
            index: 7,
            counter: 9,
        }
    }

    #[test]
    fn digest384_gpu_staging_preserves_exact_prepared_frame_and_lane_position() {
        let fields: &[&[u8]] = &[b"secret payload", b"", &[255; 9]];
        let frame = GoldilocksDigest384FrameV1::new(domain(), fields).unwrap();
        let staged = StagedDigest384V1::new(&[frame, frame]).unwrap();
        assert_eq!(staged.frame_count, 2);
        assert_eq!(
            &staged.descriptors[..],
            &[
                0,
                frame.word_count() as u64,
                frame.lane_word_index() as u64,
                frame.word_count() as u64,
                frame.word_count() as u64,
                frame.lane_word_index() as u64
            ]
        );
        let mut expected = vec![0; frame.word_count()];
        assert!(frame.write_lane_words(0, &mut expected));
        assert_eq!(&staged.words[..expected.len()], expected);
        assert_eq!(&staged.words[expected.len()..], expected);
        for lane in 0..6 {
            let mut actual = expected.clone();
            actual[frame.lane_word_index()] = lane as u64;
            assert!(frame.write_lane_words(lane, &mut expected));
            assert_eq!(actual, expected);
            expected[frame.lane_word_index()] = 0;
        }
    }

    #[test]
    fn digest384_gpu_rejects_empty_and_oversized_batches_before_device_access() {
        let frame = GoldilocksDigest384FrameV1::new(domain(), &[]).unwrap();
        let payload = [19u8; 1024];
        let fields: &[&[u8]] = &[&payload];
        let large_frame = GoldilocksDigest384FrameV1::new(domain(), fields).unwrap();
        let word_limited =
            vec![large_frame; MAX_DIGEST384_GPU_WORDS_V1 / large_frame.word_count() + 1];
        assert!(word_limited.len() <= MAX_DIGEST384_GPU_FRAMES_V1);
        assert!(matches!(
            StagedDigest384V1::new(&word_limited),
            Err(Digest384GpuErrorV1::InvalidInput(
                "canonical word count exceeds 4,194,304"
            ))
        ));
        for frames in [
            vec![],
            vec![frame; MAX_DIGEST384_GPU_FRAMES_V1 + 1],
            word_limited,
        ] {
            for backend in [Digest384GpuBackendV1::Metal, Digest384GpuBackendV1::Cuda] {
                assert!(matches!(
                    try_hash_digest384_frames_v1(backend, &frames),
                    Err(Digest384GpuErrorV1::InvalidInput(_))
                ));
            }
        }
    }

    #[test]
    fn digest384_gpu_output_shape_is_checked_before_device_access() {
        let frame = GoldilocksDigest384FrameV1::new(domain(), &[]).unwrap();
        let staged = StagedDigest384V1::new(&[frame]).unwrap();
        assert!(matches!(
            crate::fastpq_cuda::digest384_hash_frames_v1(&staged, &mut [0; 5]),
            Err(crate::fastpq_cuda::CudaBackendError::InvalidInput(_))
        ));
        #[cfg(target_os = "macos")]
        assert!(matches!(
            crate::metal::digest384_hash_frames_v1(&staged, &mut [0; 5]),
            Err(crate::gpu::GpuError::InvalidInput(_))
        ));
    }

    #[test]
    fn digest384_gpu_readiness_failures_quarantine_before_payload_staging() {
        for fault in 0..3 {
            let mut readiness = Digest384ReadinessV1::Unchecked;
            let backend = Digest384GpuBackendV1::Metal;
            let mut calls = 0;
            let mut dispatch = |_: &StagedDigest384V1, output: &mut [u64]| {
                calls += 1;
                match fault {
                    0 => {
                        output.fill(0);
                        Ok(())
                    }
                    1 => {
                        output.fill(crate::FIELD_MODULUS);
                        Ok(())
                    }
                    _ => Err(Digest384GpuErrorV1::Execution {
                        backend,
                        message: "injected readiness failure".into(),
                    }),
                }
            };
            let error = readiness
                .execute(
                    backend,
                    || panic!("request staging must follow successful readiness"),
                    &mut dispatch,
                )
                .unwrap_err();
            assert!(match fault {
                0 => matches!(error, Digest384GpuErrorV1::Conformance { .. }),
                1 => matches!(error, Digest384GpuErrorV1::NoncanonicalOutput),
                _ => matches!(error, Digest384GpuErrorV1::Execution { .. }),
            });
            assert_eq!(readiness, Digest384ReadinessV1::Quarantined);
            assert!(matches!(
                readiness.execute(
                    backend,
                    || panic!("quarantined staging"),
                    &mut |_, _| panic!("quarantined dispatch")
                ),
                Err(Digest384GpuErrorV1::Quarantined { .. })
            ));
            assert_eq!(calls, 1);
        }
    }

    #[test]
    fn digest384_gpu_noncanonical_dispatch_quarantines_admitted_backend() {
        let backend = Digest384GpuBackendV1::Cuda;
        let frame = GoldilocksDigest384FrameV1::new(domain(), &[]).unwrap();
        let mut readiness = Digest384ReadinessV1::Ready;
        assert!(matches!(
            readiness.execute(
                backend,
                || StagedDigest384V1::new(&[frame]),
                &mut |_, output| {
                    output.fill(crate::FIELD_MODULUS);
                    Ok(())
                }
            ),
            Err(Digest384GpuErrorV1::NoncanonicalOutput)
        ));
        assert_eq!(readiness, Digest384ReadinessV1::Quarantined);
        assert!(matches!(
            readiness.execute(
                backend,
                || panic!("quarantined staging"),
                &mut |_, _| panic!("quarantined dispatch")
            ),
            Err(Digest384GpuErrorV1::Quarantined { .. })
        ));
    }

    #[test]
    fn digest384_gpu_partition_failure_quarantines_before_next_chunk_or_request() {
        use crate::{Error, digest_executor::execute_bounded_digest384_frames_v1};
        let backend = Digest384GpuBackendV1::Metal;
        let frame = GoldilocksDigest384FrameV1::new(domain(), &[]).unwrap();
        let frames = [frame; 5];
        let mut readiness = Digest384ReadinessV1::Ready;
        let mut staged = 0;
        let mut dispatched = 0;
        let result = execute_bounded_digest384_frames_v1(&frames, 2, 1000, &mut |chunk| {
            readiness
                .execute(
                    backend,
                    || {
                        staged += 1;
                        StagedDigest384V1::new(chunk)
                    },
                    &mut |_, output| {
                        dispatched += 1;
                        if dispatched == 2 {
                            Err(Digest384GpuErrorV1::Execution {
                                backend,
                                message: "injected second device dispatch failure".into(),
                            })
                        } else {
                            output.fill(17);
                            Ok(())
                        }
                    },
                )
                .map_err(|error| Error::NativeDigestExecution {
                    details: error.to_string(),
                })
        });
        assert!(
            matches!(result, Err(Error::NativeDigestExecution { details }) if details.contains("injected second device dispatch failure"))
        );
        assert_eq!((staged, dispatched), (2, 2));
        assert_eq!(readiness, Digest384ReadinessV1::Quarantined);
        let result = execute_bounded_digest384_frames_v1(&frames, 2, 1000, &mut |_| {
            readiness
                .execute(
                    backend,
                    || panic!("quarantined payload staging"),
                    &mut |_, _| panic!("quarantined device execution"),
                )
                .map_err(|error| Error::NativeDigestExecution {
                    details: error.to_string(),
                })
        });
        assert!(
            matches!(result, Err(Error::NativeDigestExecution { details }) if details.contains("quarantined"))
        );
    }

    #[test]
    fn digest384_gpu_readiness_runs_once_and_never_substitutes_cpu_output() {
        let backend = Digest384GpuBackendV1::Metal;
        let mut readiness = Digest384ReadinessV1::Unchecked;
        let frame = GoldilocksDigest384FrameV1::new(domain(), &[]).unwrap();
        let expected = [
            0x0A084D2765A9990B,
            0xD59F602C37B69E1B,
            0xDE9BB3357209FA18,
            0x3FAF16BA65A67BA3,
            0xE68CCC7D9933B79D,
            0xCAD66B9479314D52,
        ];
        let mut calls = 0;
        let mut dispatch = |_: &StagedDigest384V1, output: &mut [u64]| {
            calls += 1;
            if calls == 1 {
                output.copy_from_slice(&expected);
            } else {
                output.fill(17);
            }
            Ok(())
        };
        for _ in 0..2 {
            let actual = readiness
                .execute(backend, || StagedDigest384V1::new(&[frame]), &mut dispatch)
                .unwrap();
            assert_eq!(actual, [GoldilocksDigest384V1::new([17; 6]).unwrap()]);
            assert_ne!(actual, [frame.hash()]);
            assert_eq!(readiness, Digest384ReadinessV1::Ready);
        }
        assert_eq!(calls, 3);
    }

    #[test]
    fn digest384_gpu_parameters_use_distinct_generated_lanes() {
        let parameters = digest384_gpu_parameters_v1();
        for lane in 0..6 {
            assert_eq!(
                &parameters[lane * 3..lane * 3 + 3],
                &goldilocks_digest384_lane_initial_state_v1(lane).unwrap()
            );
            for round in 0..65 {
                let offset = 18 + (lane * 65 + round) * 3;
                assert_eq!(
                    &parameters[offset..offset + 3],
                    &goldilocks_digest384_lane_round_constants_v1(lane, round).unwrap()
                );
            }
        }
        assert_ne!(&parameters[..3], &parameters[3..6]);
        assert_ne!(&parameters[18..213], &parameters[213..408]);
        assert_eq!(parameters[1188..], fastpq_isi::poseidon::MDS.concat());
    }

    #[test]
    #[cfg(any(not(feature = "fastpq-gpu"), fastpq_cuda_unavailable))]
    fn digest384_gpu_unavailable_cuda_never_substitutes_cpu() {
        let frame = GoldilocksDigest384FrameV1::new(domain(), &[]).unwrap();
        assert!(matches!(
            try_hash_digest384_frames_v1(Digest384GpuBackendV1::Cuda, &[frame]),
            Err(Digest384GpuErrorV1::Execution {
                backend: Digest384GpuBackendV1::Cuda,
                ..
            })
        ));
    }

    /// Device execution is deliberately required; callers must select this test
    /// on a Metal host rather than count a skipped device as parity evidence.
    #[test]
    #[ignore = "requires a working Metal device"]
    #[cfg(target_os = "macos")]
    fn digest384_gpu_metal_matches_cpu_all_lanes_and_framing_boundaries() {
        assert_device_parity(Digest384GpuBackendV1::Metal);
    }

    /// CUDA compilation and real device execution are mandatory for this check.
    #[test]
    #[ignore = "requires a compiled CUDA backend and an SM80+ device"]
    fn digest384_gpu_cuda_matches_cpu_all_lanes_and_framing_boundaries() {
        assert_device_parity(Digest384GpuBackendV1::Cuda);
    }

    fn assert_device_parity(backend: Digest384GpuBackendV1) {
        let payloads: Vec<Vec<u8>> = [0, 1, 3, 4, 7, 8, 9, 135, 136, 137, 4096]
            .into_iter()
            .map(|len| (0..len).map(|i| (i % 256) as u8).collect())
            .collect();
        let fields: Vec<[&[u8]; 2]> = payloads
            .iter()
            .map(|payload| [payload.as_slice(), &[][..]])
            .collect();
        let frames: Vec<_> = fields
            .iter()
            .map(|fields| GoldilocksDigest384FrameV1::new(domain(), fields).unwrap())
            .collect();
        let expected: Vec<_> = frames.iter().map(|frame| frame.hash()).collect();
        assert_eq!(
            try_hash_digest384_frames_v1(backend, &frames)
                .expect("selected device execution required"),
            expected
        );
        assert!(!crate::preflight_native_v1_gpu_backend());
    }
}
