#![no_main]

use arbitrary::{Arbitrary, Unstructured};
use fastpq_prover::{hash_lde_leaves, lde_chunk_size, merkle_paths_for_queries, trace};
use libfuzzer_sys::fuzz_target;

const MAX_EVALUATIONS: usize = 256;
const MAX_QUERIES: usize = 64;

#[derive(Debug)]
struct MerkleInput {
    evaluations: Vec<u64>,
    arity: u32,
    queries: Vec<usize>,
}

impl<'a> Arbitrary<'a> for MerkleInput {
    fn arbitrary(u: &mut Unstructured<'a>) -> arbitrary::Result<Self> {
        let eval_len =
            usize::try_from(u.int_in_range(0..=MAX_EVALUATIONS as u32)?).expect("len fits usize");
        let mut evaluations = Vec::with_capacity(eval_len);
        for _ in 0..eval_len {
            evaluations.push(u.arbitrary()?);
        }

        let query_len =
            usize::try_from(u.int_in_range(0..=MAX_QUERIES as u32)?).expect("len fits usize");
        let mut queries = Vec::with_capacity(query_len);
        for _ in 0..query_len {
            queries.push(u.arbitrary()?);
        }

        Ok(Self {
            evaluations,
            arity: u.arbitrary()?,
            queries,
        })
    }
}

fuzz_target!(|input: MerkleInput| {
    if let Ok(leaves) = hash_lde_leaves(&input.evaluations, input.arity) {
        let chunk_size = lde_chunk_size(input.arity).max(1);
        let paths = merkle_paths_for_queries(&leaves, &input.queries, input.arity);
        if let Ok(paths) = paths {
            assert_eq!(paths.len(), input.queries.len());
            if leaves.is_empty() {
                for path in paths {
                    assert!(path.is_empty(), "empty leaves should yield empty paths");
                }
                return;
            }

            let root = trace::merkle_root(&leaves);
            let leaf_count = leaves.len();
            for (query, path) in input.queries.iter().zip(paths.iter()) {
                let mut leaf_index = (*query / chunk_size).min(leaf_count.saturating_sub(1));
                let mut acc = leaves[leaf_index];
                for &sibling in path {
                    acc = if leaf_index % 2 == 0 {
                        trace::merkle_root(&[acc, sibling])
                    } else {
                        trace::merkle_root(&[sibling, acc])
                    };
                    leaf_index /= 2;
                }
                assert_eq!(acc, root, "path reconstruction must reach root");
            }
        }
    }
});
