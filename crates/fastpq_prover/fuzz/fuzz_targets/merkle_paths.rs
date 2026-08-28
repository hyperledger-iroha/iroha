#![no_main]
use arbitrary::{Arbitrary, Unstructured};
use fastpq_prover::{hash_lde_leaves, lde_chunk_size, merkle_paths_for_queries};
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
        let chunk_size = lde_chunk_size(input.arity).expect("accepted arity has a chunk size");
        let paths = merkle_paths_for_queries(
            &leaves,
            &input.queries,
            input.arity,
            input.evaluations.len(),
        );
        if let Ok(paths) = paths {
            assert_eq!(paths.len(), input.queries.len());
            if leaves.is_empty() {
                for path in paths {
                    assert!(path.is_empty(), "empty leaves should yield empty paths");
                }
                return;
            }
            let leaf_count = leaves.len();
            for (query, path) in input.queries.iter().zip(paths.iter()) {
                let leaf_index = (*query / chunk_size).min(leaf_count.saturating_sub(1));
                assert!(leaf_index < leaf_count);
                assert!(path.len() <= usize::BITS as usize);
            }
        }
    }
});
