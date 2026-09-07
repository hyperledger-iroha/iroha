//! Shared CPU/device execution of canonical frames and bounded Merkle pair batches.
use fastpq_isi::{GoldilocksDigest384FrameV1, GoldilocksDigest384V1, GoldilocksDigestDomainV1};

use crate::{Error, Result};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DigestExecutionV1 {
    Cpu,
    #[cfg(feature = "fastpq-gpu")]
    Device(crate::digest384_gpu::Digest384GpuBackendV1),
}

pub(crate) fn execute_digest384_frames_v1(
    frames: &[GoldilocksDigest384FrameV1<'_>],
    execution: DigestExecutionV1,
) -> Result<Vec<GoldilocksDigest384V1>> {
    match execution {
        DigestExecutionV1::Cpu => Ok(frames
            .iter()
            .map(GoldilocksDigest384FrameV1::hash)
            .collect()),
        #[cfg(feature = "fastpq-gpu")]
        DigestExecutionV1::Device(backend) => {
            use crate::digest384_gpu::{
                MAX_DIGEST384_GPU_FRAMES_V1, MAX_DIGEST384_GPU_WORDS_V1,
                try_hash_digest384_frames_v1,
            };
            execute_bounded_digest384_frames_v1(
                frames,
                MAX_DIGEST384_GPU_FRAMES_V1,
                MAX_DIGEST384_GPU_WORDS_V1,
                &mut |chunk| {
                    try_hash_digest384_frames_v1(backend, chunk).map_err(|error| {
                        Error::NativeDigestExecution {
                            details: error.to_string(),
                        }
                    })
                },
            )
        }
    }
}

#[cfg(any(test, feature = "fastpq-gpu"))]
pub(crate) fn execute_bounded_digest384_frames_v1(
    frames: &[GoldilocksDigest384FrameV1<'_>],
    frame_limit: usize,
    word_limit: usize,
    dispatch: &mut impl FnMut(&[GoldilocksDigest384FrameV1<'_>]) -> Result<Vec<GoldilocksDigest384V1>>,
) -> Result<Vec<GoldilocksDigest384V1>> {
    if frame_limit == 0 || word_limit == 0 {
        return Err(Error::NativeDigestExecution {
            details: "empty dispatch geometry".into(),
        });
    }
    // Reject impossible individual frames before submitting any partial batch.
    if frames.iter().any(|frame| frame.word_count() > word_limit) {
        return Err(Error::NativeDigestExecution {
            details: "one canonical frame exceeds the device word budget".into(),
        });
    }
    let mut output = Vec::new();
    output
        .try_reserve_exact(frames.len())
        .map_err(|_| Error::NativeDigestExecution {
            details: "digest output allocation failed".into(),
        })?;
    let mut start = 0;
    while start < frames.len() {
        let mut end = start;
        let mut words = 0usize;
        while end < frames.len() && end - start < frame_limit {
            let Some(next) = words
                .checked_add(frames[end].word_count())
                .filter(|count| *count <= word_limit)
            else {
                break;
            };
            words = next;
            end += 1;
        }
        let chunk = dispatch(&frames[start..end])?;
        if chunk.len() != end - start {
            return Err(Error::NativeDigestExecution {
                details: "device returned an incorrect digest count".into(),
            });
        }
        output.extend(chunk);
        start = end;
    }
    Ok(output)
}

// Pair payloads are already public commitments. Bound their temporary copies
// independently of the number of tree nodes; private witness payload staging
// remains owned by the fallible zeroizing hardware dispatcher.
const MERKLE_FRAME_PREPARATION_PAIRS_V1: usize = 1024;

pub(crate) fn hash_digest384_pairs_v1<'a>(
    children: &[GoldilocksDigest384V1],
    make_domain: impl Fn(usize) -> Result<GoldilocksDigestDomainV1<'a>>,
    execute: &mut impl FnMut(&[GoldilocksDigest384FrameV1<'_>]) -> Result<Vec<GoldilocksDigest384V1>>,
) -> Result<Vec<GoldilocksDigest384V1>> {
    if !children.len().is_multiple_of(2) {
        return Err(Error::NativeDigestExecution {
            details: "Merkle pair input must already include odd-leaf duplication".into(),
        });
    }
    let mut output = Vec::with_capacity(children.len() / 2);
    for (chunk_index, pairs) in children
        .chunks(MERKLE_FRAME_PREPARATION_PAIRS_V1 * 2)
        .enumerate()
    {
        let encoded: Vec<_> = pairs
            .chunks_exact(2)
            .map(|pair| [pair[0].to_le_bytes(), pair[1].to_le_bytes()])
            .collect();
        let fields: Vec<[&[u8]; 2]> = encoded
            .iter()
            .map(|pair| [&pair[0][..], &pair[1][..]])
            .collect();
        let frames = fields
            .iter()
            .enumerate()
            .map(|(local_index, fields)| {
                let index = chunk_index * MERKLE_FRAME_PREPARATION_PAIRS_V1 + local_index;
                GoldilocksDigest384FrameV1::new(make_domain(index)?, fields)
                    .ok_or(Error::PayloadLengthOverflow { length: 96 })
            })
            .collect::<Result<Vec<_>>>()?;
        let digests = execute(&frames)?;
        if digests.len() != frames.len() {
            return Err(Error::NativeDigestExecution {
                details: "Merkle executor returned an incorrect digest count".into(),
            });
        }
        output.extend(digests);
    }
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn domain(index: usize) -> GoldilocksDigestDomainV1<'static> {
        GoldilocksDigestDomainV1 {
            catalog: b"exact12",
            protocol: b"fastpq:v1",
            profile: b"v1",
            role: b"merkle",
            phase: b"node",
            level: 3,
            index: index as u64,
            counter: 17,
        }
    }

    #[test]
    fn digest_executor_bounded_chunks_preserve_order_and_reject_impossible_frames() {
        let fields: &[&[u8]] = &[b"payload"];
        let frames: Vec<_> = (0..7)
            .map(|i| GoldilocksDigest384FrameV1::new(domain(i), fields).unwrap())
            .collect();
        let mut sizes = Vec::new();
        let actual = execute_bounded_digest384_frames_v1(
            &frames,
            3,
            frames[0].word_count() * 2,
            &mut |chunk| {
                sizes.push(chunk.len());
                execute_digest384_frames_v1(chunk, DigestExecutionV1::Cpu)
            },
        )
        .unwrap();
        assert_eq!(sizes, [2, 2, 2, 1]);
        assert_eq!(
            actual,
            frames.iter().map(|frame| frame.hash()).collect::<Vec<_>>()
        );
        assert!(
            execute_bounded_digest384_frames_v1(
                &frames,
                3,
                frames[0].word_count() - 1,
                &mut |_| panic!("oversized frame must not dispatch")
            )
            .is_err()
        );
        assert!(
            execute_bounded_digest384_frames_v1(&frames, 0, 1000, &mut |_| panic!(
                "invalid geometry must not dispatch"
            ))
            .is_err()
        );
        assert!(
            execute_bounded_digest384_frames_v1(&frames, 3, 1000, &mut |_| Ok(Vec::new())).is_err()
        );
    }

    #[test]
    fn digest_executor_failure_stops_without_cpu_substitution_or_later_dispatch() {
        let frames: Vec<_> = (0..5)
            .map(|i| GoldilocksDigest384FrameV1::new(domain(i), &[]).unwrap())
            .collect();
        let mut calls = 0;
        let result = execute_bounded_digest384_frames_v1(&frames, 2, 1000, &mut |chunk| {
            calls += 1;
            if calls == 2 {
                Err(Error::NativeDigestExecution {
                    details: "injected device failure".into(),
                })
            } else {
                execute_digest384_frames_v1(chunk, DigestExecutionV1::Cpu)
            }
        });
        assert!(
            matches!(result, Err(Error::NativeDigestExecution { details }) if details == "injected device failure")
        );
        assert_eq!(calls, 2);
    }

    #[test]
    fn digest_executor_pair_preparation_preserves_absolute_indices_across_chunks() {
        let leaves: Vec<_> = (0..2052)
            .map(|i| GoldilocksDigest384V1::new([i; 6]).unwrap())
            .collect();
        let mut sizes = Vec::new();
        let result = hash_digest384_pairs_v1(&leaves, |index| Ok(domain(index)), &mut |frames| {
            sizes.push(frames.len());
            execute_digest384_frames_v1(frames, DigestExecutionV1::Cpu)
        })
        .unwrap();
        assert_eq!(sizes, [1024, 2]);
        for (index, pair) in leaves.chunks_exact(2).enumerate().skip(1023) {
            let left = pair[0].to_le_bytes();
            let right = pair[1].to_le_bytes();
            let expected = GoldilocksDigest384FrameV1::new(domain(index), &[&left, &right])
                .unwrap()
                .hash();
            assert_eq!(result[index], expected);
        }
        assert!(
            hash_digest384_pairs_v1(&leaves[..3], |index| Ok(domain(index)), &mut |_| panic!(
                "odd pairs reject"
            ))
            .is_err()
        );
        assert!(
            hash_digest384_pairs_v1(&leaves[..2], |index| Ok(domain(index)), &mut |_| Ok(
                Vec::new()
            ))
            .is_err()
        );
    }
}
