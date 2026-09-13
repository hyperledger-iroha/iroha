//! Shared CPU/device execution of canonical frames and bounded Merkle pair batches.
use fastpq_isi::{GoldilocksDigest384FrameV1, GoldilocksDigest384V1, GoldilocksDigestDomainV1};

use crate::{Error, Result};
use rayon::prelude::*;

/// Maximum frames prepared at once and admitted to one device dispatch.
/// This shared local resource bound does not change canonical framing.
pub const MAX_DIGEST384_BATCH_FRAMES_V1: usize = 65_536;
/// Maximum cumulative canonical words (32 MiB) in one device dispatch.
/// This bounds canonical word staging, not total live host/device allocation.
pub const MAX_DIGEST384_BATCH_WORDS_V1: usize = 4_194_304;

/// Local computation policy for canonical six-lane hashing; never a consensus parameter.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DigestExecutionV1 {
    /// Compute canonical hashes on the CPU.
    Cpu,
    #[cfg(feature = "fastpq-gpu")]
    /// Require the selected hardware backend; execution failures never fall back to CPU.
    Device(crate::digest384_gpu::Digest384GpuBackendV1),
}

/// Execute canonical frames in order using the explicit local policy.
///
/// # Errors
/// Returns bounded-allocation, readiness, quarantine or device errors without CPU substitution.
pub fn execute_digest384_frames_v1(
    frames: &[GoldilocksDigest384FrameV1<'_>],
    execution: DigestExecutionV1,
) -> Result<Vec<GoldilocksDigest384V1>> {
    match execution {
        DigestExecutionV1::Cpu => {
            // Indexed independent jobs preserve the canonical frame order. Keep
            // small batches sequential to avoid Rayon scheduling overhead.
            if frames.len() < 64 {
                Ok(frames
                    .iter()
                    .map(GoldilocksDigest384FrameV1::hash)
                    .collect())
            } else {
                Ok(frames
                    .par_iter()
                    .map(GoldilocksDigest384FrameV1::hash)
                    .collect())
            }
        }
        #[cfg(feature = "fastpq-gpu")]
        DigestExecutionV1::Device(backend) => {
            use crate::digest384_gpu::try_hash_digest384_frames_v1;
            execute_bounded_digest384_frames_v1(
                frames,
                MAX_DIGEST384_BATCH_FRAMES_V1,
                MAX_DIGEST384_BATCH_WORDS_V1,
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

/// Prepare bounded canonical Merkle pair frames with absolute parent indices.
///
/// # Errors
/// Rejects odd child counts, allocation failures, invalid framing or any failed execution chunk.
pub fn hash_digest384_pairs_v1<'a>(
    children: &[GoldilocksDigest384V1],
    make_domain: impl Fn(usize) -> Result<GoldilocksDigestDomainV1<'a>>,
    execute: &mut impl FnMut(&[GoldilocksDigest384FrameV1<'_>]) -> Result<Vec<GoldilocksDigest384V1>>,
) -> Result<Vec<GoldilocksDigest384V1>> {
    hash_digest384_pairs_with_preparation_limit_v1(
        children,
        MAX_DIGEST384_BATCH_FRAMES_V1,
        make_domain,
        execute,
    )
}

fn hash_digest384_pairs_with_preparation_limit_v1<'a>(
    children: &[GoldilocksDigest384V1],
    preparation_pairs: usize,
    make_domain: impl Fn(usize) -> Result<GoldilocksDigestDomainV1<'a>>,
    execute: &mut impl FnMut(&[GoldilocksDigest384FrameV1<'_>]) -> Result<Vec<GoldilocksDigest384V1>>,
) -> Result<Vec<GoldilocksDigest384V1>> {
    if preparation_pairs == 0 || preparation_pairs > MAX_DIGEST384_BATCH_FRAMES_V1 {
        return Err(Error::NativeDigestExecution {
            details: "Merkle preparation exceeds the shared frame budget".into(),
        });
    }
    if !children.len().is_multiple_of(2) {
        return Err(Error::NativeDigestExecution {
            details: "Merkle pair input must already include odd-leaf duplication".into(),
        });
    }
    let allocation_error = || Error::NativeDigestExecution {
        details: "Merkle preparation allocation failed".into(),
    };
    let mut output = Vec::new();
    output
        .try_reserve_exact(children.len() / 2)
        .map_err(|_| allocation_error())?;
    // Only public commitments are copied here. Private row staging and exact
    // canonical-word partitioning retain their independent bounded owners.
    for (chunk_index, pairs) in children.chunks(preparation_pairs * 2).enumerate() {
        let pair_count = pairs.len() / 2;
        let mut encoded = Vec::new();
        encoded
            .try_reserve_exact(pair_count)
            .map_err(|_| allocation_error())?;
        for pair in pairs.chunks_exact(2) {
            encoded.push([pair[0].to_le_bytes(), pair[1].to_le_bytes()]);
        }
        let mut fields: Vec<[&[u8]; 2]> = Vec::new();
        fields
            .try_reserve_exact(pair_count)
            .map_err(|_| allocation_error())?;
        for pair in &encoded {
            fields.push([&pair[0][..], &pair[1][..]]);
        }
        let mut frames = Vec::new();
        frames
            .try_reserve_exact(pair_count)
            .map_err(|_| allocation_error())?;
        for (local_index, fields) in fields.iter().enumerate() {
            let index = chunk_index * preparation_pairs + local_index;
            frames.push(
                GoldilocksDigest384FrameV1::new(make_domain(index)?, fields)
                    .ok_or(Error::PayloadLengthOverflow { length: 96 })?,
            );
        }
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
    #[test]
    fn indexed_executor_cpu_matches_complete_hashes_and_rejects_wrapping_ranges() {
        let fields: &[&[u8]] = &[b"payload"];
        let frame = GoldilocksDigest384FrameV1::new(domain(0), fields).unwrap();
        let cached = frame.indexed_predicate_v1();
        for start in [0, (1_u64 << 56) - 1, u64::MAX - 2] {
            let actual =
                execute_digest384_indexed_coordinates_v1(&cached, start, 3, DigestExecutionV1::Cpu)
                    .unwrap();
            let expected = (0..3)
                .map(|offset| {
                    let mut d = domain(0);
                    d.index = start + offset;
                    GoldilocksDigest384FrameV1::new(d, fields)
                        .unwrap()
                        .hash()
                        .words()[0]
                })
                .collect::<Vec<_>>();
            assert_eq!(actual, expected);
        }
        for (start, count) in [(0, 0), (0, 4097), (u64::MAX, 2)] {
            assert!(
                execute_digest384_indexed_coordinates_v1(
                    &cached,
                    start,
                    count,
                    DigestExecutionV1::Cpu
                )
                .is_err()
            );
        }
    }
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
    fn digest_executor_default_pair_preparation_uses_shared_bound_and_absolute_indices() {
        use std::cell::Cell;

        let pair_count = MAX_DIGEST384_BATCH_FRAMES_V1 + 2;
        let leaves: Vec<_> = (0..pair_count * 2)
            .map(|index| GoldilocksDigest384V1::new([index as u64; 6]).unwrap())
            .collect();
        let visited = Cell::new(0);
        let mut dispatched = 0;
        let mut sizes = Vec::new();
        let result = hash_digest384_pairs_v1(
            &leaves,
            |index| {
                assert_eq!(index, visited.get(), "absolute preparation index");
                visited.set(index + 1);
                Ok(domain(index))
            },
            &mut |frames| {
                sizes.push(frames.len());
                // Geometry uses the real default ceiling; only chunk endpoints
                // are scalar-hashed. The small-cap oracle below hashes every pair.
                for local in [0, frames.len() - 1] {
                    let index = dispatched + local;
                    let left = leaves[2 * index].to_le_bytes();
                    let right = leaves[2 * index + 1].to_le_bytes();
                    let expected = GoldilocksDigest384FrameV1::new(domain(index), &[&left, &right])
                        .unwrap()
                        .hash();
                    assert_eq!(frames[local].hash(), expected);
                }
                dispatched += frames.len();
                // This callback checks preparation geometry, not digest execution.
                Ok(vec![
                    GoldilocksDigest384V1::new([0; 6]).unwrap();
                    frames.len()
                ])
            },
        )
        .unwrap();
        assert_eq!(sizes, [MAX_DIGEST384_BATCH_FRAMES_V1, 2]);
        assert_eq!(visited.get(), pair_count);
        assert_eq!(dispatched, pair_count);
        assert_eq!(result.len(), pair_count);
    }

    #[test]
    fn digest_executor_pair_preparation_refuses_invalid_limits_and_stops_on_failure() {
        use std::cell::Cell;

        let leaves = vec![GoldilocksDigest384V1::new([1; 6]).unwrap(); 10];
        for limit in [0, MAX_DIGEST384_BATCH_FRAMES_V1 + 1] {
            assert!(
                hash_digest384_pairs_with_preparation_limit_v1(
                    &leaves,
                    limit,
                    |_| panic!("invalid limit must reject before preparation"),
                    &mut |_| panic!("invalid limit must reject before execution"),
                )
                .is_err()
            );
        }
        let prepared = Cell::new(0);
        let mut calls = 0;
        let result = hash_digest384_pairs_with_preparation_limit_v1(
            &leaves,
            2,
            |index| {
                assert_eq!(index, prepared.get());
                prepared.set(index + 1);
                Ok(domain(index))
            },
            &mut |frames| {
                calls += 1;
                if calls == 2 {
                    Err(Error::NativeDigestExecution {
                        details: "pair execution failed".into(),
                    })
                } else {
                    execute_digest384_frames_v1(frames, DigestExecutionV1::Cpu)
                }
            },
        );
        assert!(
            matches!(result, Err(Error::NativeDigestExecution { details })
            if details == "pair execution failed")
        );
        assert_eq!(calls, 2);
        assert_eq!(prepared.get(), 4, "third chunk must not be prepared");
    }

    #[test]
    fn digest_executor_pair_preparation_preserves_absolute_indices_across_chunks() {
        let leaves: Vec<_> = (0..2052)
            .map(|i| GoldilocksDigest384V1::new([i; 6]).unwrap())
            .collect();
        let mut sizes = Vec::new();
        let result = hash_digest384_pairs_with_preparation_limit_v1(
            &leaves,
            1024,
            |index| Ok(domain(index)),
            &mut |frames| {
                sizes.push(frames.len());
                execute_digest384_frames_v1(frames, DigestExecutionV1::Cpu)
            },
        )
        .unwrap();
        assert_eq!(sizes, [1024, 2]);
        for (index, pair) in leaves.chunks_exact(2).enumerate() {
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

/// Maximum ordered indices submitted in one canonical nonce-search batch.
pub const MAX_DIGEST384_INDEXED_BATCH_V1: usize = 4096;

/// Execute exact first-coordinate values with a bounded nonwrapping index range.
/// This is a nonce predicate primitive, never a shortened commitment hash.
///
/// # Errors
/// Rejects invalid geometry and any explicitly selected device failure without substitution.
pub fn execute_digest384_indexed_coordinates_v1(
    predicate: &fastpq_isi::poseidon_digest384::GoldilocksDigest384IndexedPredicateV1<'_>,
    start: u64,
    count: usize,
    execution: DigestExecutionV1,
) -> Result<Vec<u64>> {
    if count == 0
        || count > MAX_DIGEST384_INDEXED_BATCH_V1
        || start.checked_add((count - 1) as u64).is_none()
    {
        return Err(Error::NativeDigestExecution {
            details: "invalid bounded indexed digest range".into(),
        });
    }
    match execution {
        DigestExecutionV1::Cpu => Ok((0..count)
            .into_par_iter()
            .map(|offset| predicate.first_coordinate_v1(start + offset as u64))
            .collect()),
        #[cfg(feature = "fastpq-gpu")]
        DigestExecutionV1::Device(backend) => {
            crate::digest384_indexed_gpu::try_indexed_coordinates_v1(
                backend, predicate, start, count,
            )
            .map_err(|error| Error::NativeDigestExecution {
                details: error.to_string(),
            })
        }
    }
}
