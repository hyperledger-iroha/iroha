//! Fixed-size parallel canonical leaf hashing with ordered outputs and exact scratch bounds.

use rayon::prelude::*;

use super::{
    deep_binding::{BindingError, Context, Oracle},
    masked_quotient::{checked_add as add, checked_mul as mul},
};
use crate::{Error, Result};

/// Independent of the worker count, witness, transcript and proof representation.
pub(super) const CAPACITY: usize = 32;

/// Packed payloads, clearing digest slots, ordered error results and one exact
/// private canonical frame per possible concurrent hash. Worker stacks, allocator
/// metadata and the shared public Context are charged/excluded by their owners.
pub(super) fn payload_bytes(binding: &Context, oracle: Oracle, leaf_bytes: usize) -> Result<usize> {
    mul(
        CAPACITY,
        add(
            leaf_bytes,
            add(
                48 + size_of::<Result<()>>(),
                binding.tree_frame_bytes(oracle).map_err(binding_error)?,
            )?,
        )?,
    )
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
) -> Result<()> {
    if indices.is_empty()
        || indices.len() > CAPACITY
        || output.len() != indices.len()
        || leaf_bytes == 0
        || payloads.len() != mul(indices.len(), leaf_bytes)?
    {
        return Err(invalid("DEEP leaf batch has another exact bounded shape"));
    }
    let results: Vec<Result<()>> = output
        .par_iter_mut()
        .zip(payloads.par_chunks_exact(leaf_bytes))
        .zip(indices.par_iter())
        .map(|((output, bytes), &index)| {
            let index =
                u32::try_from(index).map_err(|_| invalid("DEEP leaf batch index exceeds u32"))?;
            *output = binding
                .hash_leaf(oracle, index, bytes)
                .map_err(binding_error)?
                .words();
            Ok(())
        })
        .collect();
    for result in results {
        result?;
    }
    Ok(())
}
fn binding_error(error: BindingError) -> Error {
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
                            .hash_leaf(Oracle::QuotientAndMask, index as u32, bytes)
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
                        .hash_leaf(Oracle::QuotientAndMask, index as u32, bytes)
                        .unwrap()
                );
            }
        }
        assert_eq!(
            payload_bytes(&binding, Oracle::QuotientAndMask, 96).unwrap(),
            CAPACITY
                * (96
                    + 48
                    + size_of::<Result<()>>()
                    + binding.tree_frame_bytes(Oracle::QuotientAndMask).unwrap())
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
                    &mut outputs[..count]
                )
                .is_err()
            );
        }
        assert!(
            hash(
                &binding,
                Oracle::QuotientAndMask,
                &[0; CAPACITY + 1],
                &[0; (CAPACITY + 1) * 96],
                96,
                &mut outputs
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
                &mut outputs[..2]
            )
            .is_err()
        );
    }
}
