//! Bounded canonical leaf hashing under explicit CPU or required-device execution.

use rayon::prelude::*;
#[cfg(any(test, feature = "fastpq-gpu"))]
use zeroize::Zeroize;

use super::{
    compact_v1::{MAX_PREPARED_HASH_FRAME_BYTES, PreparedHashFrame},
    deep_binding::{BindingError, Context, Oracle},
    masked_quotient::{checked_add as add, checked_mul as mul},
};
#[cfg(feature = "fastpq-gpu")]
use crate::digest384_batch::execute_last_fields_with_cpu;
use crate::digest384_batch::{Digest384LastFieldJob, last_fields_payload_charge};
use crate::{DigestExecutionV1, Error, Result};

/// Independent of the worker count, witness, transcript and proof representation.
/// Fixed after bounded 32/256/1024-job device measurements; every build charges
/// the same capacity before processing private rows.
pub(super) const CAPACITY: usize = 1024;

/// Own returned device digests immediately, including a malformed result shape.
/// No pop/drain or growing allocation leaves private-derived cells unguarded.
#[cfg(any(test, feature = "fastpq-gpu"))]
struct ReturnedLeafDigests(Vec<fastpq_isi::GoldilocksDigest384V1>);

#[cfg(any(test, feature = "fastpq-gpu"))]
impl ReturnedLeafDigests {
    fn write_to(self, output: &mut [[u64; 6]]) -> Result<()> {
        if self.0.len() != output.len() {
            return Err(invalid("DEEP leaf executor returned another digest count"));
        }
        for (destination, digest) in output.iter_mut().zip(&self.0) {
            *destination = digest.words();
        }
        Ok(())
    }
}

#[cfg(any(test, feature = "fastpq-gpu"))]
impl Drop for ReturnedLeafDigests {
    fn drop(&mut self) {
        for digest in &mut self.0 {
            digest.zeroize();
        }
        // Observe real erased cells before Vec releases its allocation; no
        // allocation is read after free and no secret is retained by the hook.
        #[cfg(test)]
        LEAF_ERASURES.with(|observed| {
            let (cleared, uncleared) = observed.get();
            let clean = self
                .0
                .iter()
                .filter(|digest| digest.words() == [0; 6])
                .count();
            observed.set((cleared + clean, uncleared + self.0.len() - clean));
        });
    }
}

#[cfg(test)]
thread_local! {
    static LEAF_ERASURES: core::cell::Cell<(usize, usize)> = const { core::cell::Cell::new((0, 0)) };
}

/// Packed payloads, clearing digest slots, both caller/tree index arrays, ordered
/// error results and one exact private frame per possible concurrent hash. Worker stacks, allocator
/// metadata and the shared public Context are charged/excluded by their owners.
pub(super) fn payload_bytes(binding: &Context, oracle: Oracle, leaf_bytes: usize) -> Result<usize> {
    let bodies = mul(CAPACITY, MAX_PREPARED_HASH_FRAME_BYTES)?;
    let host = mul(
        CAPACITY,
        add(
            leaf_bytes,
            add(
                48 + 2 * size_of::<usize>()
                    + size_of::<Result<()>>()
                    + size_of::<Result<PreparedHashFrame>>()
                    + size_of::<PreparedHashFrame>()
                    + size_of::<Result<Digest384LastFieldJob<'_>>>()
                    + size_of::<Digest384LastFieldJob<'_>>(),
                binding
                    .tree_frame_bytes(oracle)
                    .map_err(|error| binding_error(&error))?,
            )?,
        )?,
    )?;
    add(
        host,
        add(bodies, last_fields_payload_charge(CAPACITY, bodies)?)?,
    )
}

/// Build ordered borrowed continuation jobs in parallel. The intermediate
/// result descriptors and final jobs are both included in `payload_bytes`.
/// Successful job owners clear their stream state on any later error.
#[cfg(any(test, feature = "fastpq-gpu"))]
pub(super) fn prepare_jobs(frames: &[PreparedHashFrame]) -> Result<Vec<Digest384LastFieldJob<'_>>> {
    if frames.is_empty() || frames.len() > CAPACITY {
        return Err(invalid(
            "DEEP prepared batch has another exact bounded shape",
        ));
    }
    frames
        .par_iter()
        .map(PreparedHashFrame::job)
        .collect::<Vec<_>>()
        .into_iter()
        .collect()
}

/// Execute already-framed leaves or parents into caller-owned clearing storage.
/// The shared frame/job/executor charge is identical for both call sites.
#[cfg(any(test, feature = "fastpq-gpu"))]
pub(super) fn execute_prepared(
    frames: &[PreparedHashFrame],
    output: &mut [[u64; 6]],
    execution: DigestExecutionV1,
) -> Result<()> {
    if frames.is_empty() || frames.len() > CAPACITY || frames.len() != output.len() {
        return Err(invalid(
            "DEEP prepared batch has another exact bounded shape",
        ));
    }
    #[cfg(feature = "fastpq-gpu")]
    if matches!(execution, DigestExecutionV1::Device(_)) {
        let bytes = frames
            .iter()
            .try_fold(0usize, |sum, frame| add(sum, frame.payload_len()))?;
        let digests = ReturnedLeafDigests(execute_last_fields_with_cpu(
            frames.len(),
            bytes,
            execution,
            |index| frames[index].hash_cpu(),
            || prepare_jobs(frames),
        )?);
        return digests.write_to(output);
    }
    let _ = execution;
    let results = output
        .par_iter_mut()
        .zip(frames.par_iter())
        .map(|(output, frame)| {
            *output = frame.hash_cpu()?.words();
            Ok(())
        })
        .collect::<Vec<Result<()>>>();
    for result in results {
        result?;
    }
    Ok(())
}

/// Hash a bounded prefix into caller-owned clearing digest storage. Every job
/// finishes before an ordered error is returned; no parallel completion order
/// influences commitment insertion or the first protocol error.
pub(super) fn hash(
    binding: &Context,
    oracle: Oracle,
    indices: &[usize],
    payloads: &[u8],
    leaf_bytes: usize,
    output: &mut [[u64; 6]],
    execution: DigestExecutionV1,
) -> Result<()> {
    if indices.is_empty()
        || indices.len() > CAPACITY
        || output.len() != indices.len()
        || leaf_bytes == 0
        || payloads.len() != mul(indices.len(), leaf_bytes)?
    {
        return Err(invalid("DEEP leaf batch has another exact bounded shape"));
    }
    #[cfg(feature = "fastpq-gpu")]
    if matches!(execution, DigestExecutionV1::Device(_)) {
        let frames = indices
            .par_iter()
            .zip(payloads.par_chunks_exact(leaf_bytes))
            .map(|(&index, bytes)| {
                let index = u32::try_from(index)
                    .map_err(|_| invalid("DEEP leaf batch index exceeds u32"))?;
                binding.prepare_leaf(oracle, index, bytes)
            })
            .collect::<Vec<_>>()
            .into_iter()
            .collect::<Result<Vec<_>>>()?;
        return execute_prepared(&frames, output, execution);
    }
    let _ = execution;
    let results: Vec<Result<()>> = output
        .par_iter_mut()
        .zip(payloads.par_chunks_exact(leaf_bytes))
        .zip(indices.par_iter())
        .map(|((output, bytes), &index)| {
            let index =
                u32::try_from(index).map_err(|_| invalid("DEEP leaf batch index exceeds u32"))?;
            *output = binding
                .hash_leaf(oracle, index, bytes)
                .map_err(|error| binding_error(&error))?
                .words();
            Ok(())
        })
        .collect();
    for result in results {
        result?;
    }
    Ok(())
}
fn binding_error(error: &BindingError) -> Error {
    Error::InvalidTraceShape {
        details: format!("DEEP leaf batch: {error}"),
    }
}
fn invalid(details: &'static str) -> Error {
    Error::InvalidTraceShape {
        details: details.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::{deep_geometry::LDE_ROWS, secret_polynomial::SecretPolynomial};
    use fastpq_isi::GoldilocksDigest384V1 as Digest;

    #[test]
    fn returned_leaf_digests_clear_on_success_shape_error_and_partial_unwind() {
        let source = [Digest::new([1, 2, 3, 5, 7, 11]).unwrap(); 4];
        let before = LEAF_ERASURES.with(core::cell::Cell::get);
        let mut output = SecretPolynomial::<[u64; 6]>::zeroed(4).unwrap();
        ReturnedLeafDigests(source.to_vec())
            .write_to(&mut output)
            .unwrap();
        assert!(output.iter().all(|&words| words == source[0].words()));
        assert_eq!(
            LEAF_ERASURES.with(core::cell::Cell::get),
            (before.0 + 4, before.1)
        );

        assert!(
            ReturnedLeafDigests(source.to_vec())
                .write_to(&mut output[..3])
                .is_err()
        );
        assert_eq!(
            LEAF_ERASURES.with(core::cell::Cell::get),
            (before.0 + 8, before.1)
        );

        let unwound = std::panic::catch_unwind(|| {
            let returned = ReturnedLeafDigests(source.to_vec());
            let mut partial = SecretPolynomial::<[u64; 6]>::zeroed(4).unwrap();
            for (destination, digest) in partial.iter_mut().zip(&returned.0).take(2) {
                *destination = digest.words();
            }
            panic!("test-only failure after partial returned-digest use");
        });
        assert!(unwound.is_err());
        assert_eq!(
            LEAF_ERASURES.with(core::cell::Cell::get),
            (before.0 + 12, before.1)
        );
    }

    #[test]
    fn prepared_parallel_jobs_and_execution_keep_canonical_order_and_shape() {
        let binding = Context::new(b"prepared parallel continuation order").unwrap();
        let left = Digest::new([1, 2, 3, 5, 7, 11]).unwrap();
        let right = Digest::new([13, 17, 19, 23, 29, 31]).unwrap();
        let frames = (0..=CAPACITY)
            .map(|index| {
                binding
                    .prepare_parent(
                        Oracle::Row,
                        1,
                        u32::try_from(CAPACITY - index).unwrap(),
                        left,
                        right,
                    )
                    .unwrap()
            })
            .collect::<Vec<_>>();
        for count in [1, 7, CAPACITY] {
            for threads in [1, 4] {
                rayon::ThreadPoolBuilder::new()
                    .num_threads(threads)
                    .build()
                    .unwrap()
                    .install(|| {
                        let jobs = prepare_jobs(&frames[..count]).unwrap();
                        let streamed = zeroize::Zeroizing::new(
                            crate::digest384_batch::hash_last_fields_cpu(&jobs).unwrap(),
                        );
                        let mut output = SecretPolynomial::<[u64; 6]>::zeroed(count).unwrap();
                        execute_prepared(&frames[..count], &mut output, DigestExecutionV1::Cpu)
                            .unwrap();
                        for (index, (&words, digest)) in
                            output.iter().zip(streamed.iter()).enumerate()
                        {
                            let expected = binding
                                .hash_parent(
                                    Oracle::Row,
                                    1,
                                    u32::try_from(CAPACITY - index).unwrap(),
                                    left,
                                    right,
                                )
                                .unwrap();
                            assert_eq!(words, expected.words());
                            assert_eq!(*digest, expected);
                        }
                    });
            }
        }
        assert!(prepare_jobs(&[]).is_err());
        assert!(prepare_jobs(&frames).is_err());
        let mut output = SecretPolynomial::<[u64; 6]>::zeroed(CAPACITY + 1).unwrap();
        for (input, count) in [
            (&frames[..0], 0),
            (&frames[..1], 0),
            (&frames[..], CAPACITY + 1),
        ] {
            assert!(execute_prepared(input, &mut output[..count], DigestExecutionV1::Cpu).is_err());
            assert!(output.iter().all(|&words| words == [0; 6]));
        }
    }

    #[test]
    fn bounded_parallel_leaves_match_serial_hashes_in_exact_input_order() {
        let binding = Context::new(b"fixed leaf-batch parity").unwrap();
        for count in [1, 7, CAPACITY] {
            let indices: Vec<_> = (0..count)
                .map(|i| if i == 0 { LDE_ROWS - 1 } else { i * 127 })
                .collect();
            let mut payloads = vec![0; count * 96];
            for (i, word) in payloads.chunks_exact_mut(8).enumerate() {
                word.copy_from_slice(&(i as u64 + 3).to_le_bytes());
            }
            let mut outputs = SecretPolynomial::<[u64; 6]>::zeroed(count).unwrap();
            for threads in [1, 4] {
                rayon::ThreadPoolBuilder::new()
                    .num_threads(threads)
                    .build()
                    .unwrap()
                    .install(|| {
                        hash(
                            &binding,
                            Oracle::QuotientAndMask,
                            &indices,
                            &payloads,
                            96,
                            &mut outputs,
                            DigestExecutionV1::Cpu,
                        )
                        .unwrap();
                    });
                for ((&index, bytes), &words) in indices
                    .iter()
                    .zip(payloads.chunks_exact(96))
                    .zip(outputs.iter())
                {
                    assert_eq!(
                        Digest::new(words).unwrap(),
                        binding
                            .hash_leaf(
                                Oracle::QuotientAndMask,
                                u32::try_from(index).unwrap(),
                                bytes
                            )
                            .unwrap()
                    );
                }
            }
            for ((&index, bytes), &words) in indices
                .iter()
                .zip(payloads.chunks_exact(96))
                .zip(outputs.iter())
            {
                assert_eq!(
                    Digest::new(words).unwrap(),
                    binding
                        .hash_leaf(
                            Oracle::QuotientAndMask,
                            u32::try_from(index).unwrap(),
                            bytes
                        )
                        .unwrap()
                );
            }
        }
        let bodies = CAPACITY * MAX_PREPARED_HASH_FRAME_BYTES;
        assert_eq!(
            payload_bytes(&binding, Oracle::QuotientAndMask, 96).unwrap(),
            CAPACITY
                * (96
                    + 48
                    + 2 * size_of::<usize>()
                    + size_of::<Result<()>>()
                    + size_of::<Result<PreparedHashFrame>>()
                    + size_of::<PreparedHashFrame>()
                    + size_of::<Result<Digest384LastFieldJob<'_>>>()
                    + size_of::<Digest384LastFieldJob<'_>>()
                    + binding.tree_frame_bytes(Oracle::QuotientAndMask).unwrap())
                + bodies
                + last_fields_payload_charge(CAPACITY, bodies).unwrap()
        );
    }

    #[test]
    fn malformed_batch_shapes_and_positions_never_return_commitments() {
        let binding = Context::new(b"leaf-batch rejection").unwrap();
        let mut outputs = SecretPolynomial::<[u64; 6]>::zeroed(CAPACITY + 1).unwrap();
        for (indices, bytes, width, count) in [
            (&[][..], &[][..], 96, 0),
            (&[0][..], &[0; 95][..], 96, 1),
            (&[0][..], &[0; 96][..], 96, 0),
            (&[0][..], &[][..], 0, 1),
        ] {
            assert!(
                hash(
                    &binding,
                    Oracle::QuotientAndMask,
                    indices,
                    bytes,
                    width,
                    &mut outputs[..count],
                    DigestExecutionV1::Cpu,
                )
                .is_err()
            );
        }
        assert!(
            hash(
                &binding,
                Oracle::QuotientAndMask,
                &[0; CAPACITY + 1],
                &vec![0; (CAPACITY + 1) * 96],
                96,
                &mut outputs,
                DigestExecutionV1::Cpu,
            )
            .is_err()
        );
        assert!(
            hash(
                &binding,
                Oracle::QuotientAndMask,
                &[0, LDE_ROWS],
                &[0; 192],
                96,
                &mut outputs[..2],
                DigestExecutionV1::Cpu,
            )
            .is_err()
        );
    }

    #[test]
    fn prepared_leaves_and_parents_match_independent_canonical_hashes() {
        use crate::backend::GOLDILOCKS_MODULUS;
        let binding = Context::new(&vec![73; 200 * 1024]).unwrap();
        for oracle in [
            Oracle::Row,
            Oracle::QuotientAndMask,
            Oracle::Fri(0),
            Oracle::Fri(4),
            Oracle::Terminal,
        ] {
            let (_, _, leaves, width) = oracle.shape().unwrap();
            for index in [0, leaves - 1] {
                let leaf_index = u32::try_from(index).unwrap();
                let payload: Vec<_> = (0..width / 8)
                    .flat_map(|i| (i as u64 + 1).to_le_bytes())
                    .collect();
                let prepared = binding.prepare_leaf(oracle, leaf_index, &payload).unwrap();
                assert_eq!(prepared.job().unwrap().prefix().received_len(), 0);
                assert!(
                    prepared.job().unwrap().final_field().len() <= MAX_PREPARED_HASH_FRAME_BYTES
                );
                assert_eq!(
                    prepared.hash_cpu().unwrap(),
                    binding.hash_leaf(oracle, leaf_index, &payload).unwrap()
                );
                let mut output = [[0; 6]];
                hash(
                    &binding,
                    oracle,
                    &[index],
                    &payload,
                    width,
                    &mut output,
                    DigestExecutionV1::Cpu,
                )
                .unwrap();
                assert_eq!(output[0], prepared.hash_cpu().unwrap().words());
                assert!(
                    binding
                        .prepare_leaf(oracle, u32::try_from(leaves).unwrap(), &payload)
                        .is_err()
                );
                assert!(
                    binding
                        .prepare_leaf(oracle, leaf_index, &payload[..width - 1])
                        .is_err()
                );
                let mut malformed = payload;
                malformed[..8].copy_from_slice(&GOLDILOCKS_MODULUS.to_le_bytes());
                assert!(
                    binding
                        .prepare_leaf(oracle, leaf_index, &malformed)
                        .is_err()
                );
                assert!(
                    hash(
                        &binding,
                        oracle,
                        &[index],
                        &malformed,
                        width,
                        &mut output,
                        DigestExecutionV1::Cpu
                    )
                    .is_err()
                );
            }
        }
        let left = Digest::new([1, 2, 3, 4, 5, 6]).unwrap();
        let right = Digest::new([6, 5, 4, 3, 2, 1]).unwrap();
        for (oracle, level, index) in [
            (Oracle::Row, 1, 257),
            (Oracle::Row, 23, 0),
            (Oracle::Fri(0), 2, 513),
        ] {
            let frame = binding
                .prepare_parent(oracle, level, index, left, right)
                .unwrap();
            assert_eq!(
                frame.hash_cpu().unwrap(),
                binding
                    .hash_parent(oracle, level, index, left, right)
                    .unwrap()
            );
            assert!(
                binding
                    .prepare_parent(oracle, 0, index, left, right)
                    .is_err()
            );
        }
        assert!(
            binding
                .prepare_parent(Oracle::Terminal, 1, 0, left, right)
                .is_err()
        );
        assert!(
            binding
                .prepare_parent(Oracle::Row, 24, 0, left, right)
                .is_err()
        );
    }

    #[cfg(all(feature = "fastpq-gpu", target_os = "macos"))]
    #[test]
    #[ignore = "requires actual Metal DEEP leaf execution; no CPU substitution"]
    fn bounded_masked_leaf_metal_matches_cpu_for_every_oracle() {
        let _lane = crate::backend::acquire_gpu_lane();
        let binding = Context::new(b"actual Metal masked DEEP leaves").unwrap();
        for oracle in [
            Oracle::Row,
            Oracle::QuotientAndMask,
            Oracle::Fri(0),
            Oracle::Fri(4),
            Oracle::Terminal,
        ] {
            let (_, _, leaves, width) = oracle.shape().unwrap();
            let count = leaves.min(CAPACITY);
            let indices = (0..count).collect::<Vec<_>>();
            let payloads = (0..count * width / 8)
                .flat_map(|value| (value as u64).to_le_bytes())
                .collect::<Vec<_>>();
            let mut cpu = vec![[0; 6]; count];
            let mut metal = vec![[0; 6]; count];
            hash(
                &binding,
                oracle,
                &indices,
                &payloads,
                width,
                &mut cpu,
                DigestExecutionV1::Cpu,
            )
            .unwrap();
            hash(
                &binding,
                oracle,
                &indices,
                &payloads,
                width,
                &mut metal,
                DigestExecutionV1::Device(crate::Digest384GpuBackendV1::Metal),
            )
            .unwrap();
            assert_eq!(metal, cpu);
        }
    }
}
