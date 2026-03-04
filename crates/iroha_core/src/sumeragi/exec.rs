//! Exec-vote helpers: compute `post_state_root` via SMT, build votes, and assemble QCs.
//!
//! This module is internal and side-effect free; consumed by the Sumeragi execution pipeline.

use iroha_crypto::Hash;

use super::{
    consensus::ExecWitness,
    smt::{KvPair, compute_post_state_root},
};

/// Convert an `ExecWitness` into SMT `KvPair` slices and compute the `post_state_root`.
pub fn post_state_from_witness(w: &ExecWitness) -> Hash {
    let reads: Vec<KvPair> = w
        .reads
        .iter()
        .map(|kv| KvPair::new(kv.key.clone(), kv.value.clone()))
        .collect();
    let writes: Vec<KvPair> = w
        .writes
        .iter()
        .map(|kv| KvPair::new(kv.key.clone(), kv.value.clone()))
        .collect();
    compute_post_state_root(&reads, &writes)
}

/// Compute the `parent_state_root` using only the witnessed reads (pre-values).
/// Writes are ignored to reflect the parent snapshot.
pub fn parent_state_from_witness(w: &ExecWitness) -> Hash {
    let reads: Vec<KvPair> = w
        .reads
        .iter()
        .map(|kv| KvPair::new(kv.key.clone(), kv.value.clone()))
        .collect();
    compute_post_state_root(&reads, &[])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn post_root_stable_for_order_variations() {
        use super::super::consensus::{ExecKv, ExecWitness};
        // Two reads, one write
        let w1 = ExecWitness {
            reads: vec![
                ExecKv {
                    key: b"a".to_vec(),
                    value: b"1".to_vec(),
                },
                ExecKv {
                    key: b"b".to_vec(),
                    value: b"2".to_vec(),
                },
            ],
            writes: vec![ExecKv {
                key: b"c".to_vec(),
                value: b"3".to_vec(),
            }],
            fastpq_transcripts: Vec::new(),
            fastpq_batches: Vec::new(),
        };
        let mut w2 = w1.clone();
        w2.reads.swap(0, 1); // reorder reads
        let r1 = post_state_from_witness(&w1);
        let r2 = post_state_from_witness(&w2);
        assert_eq!(r1, r2);
    }

    #[test]
    fn parent_root_uses_reads_only() {
        use super::super::consensus::{ExecKv, ExecWitness};
        // Same key read then written; parent root should reflect pre-value only.
        let witness = ExecWitness {
            reads: vec![ExecKv {
                key: b"k".to_vec(),
                value: b"old".to_vec(),
            }],
            writes: vec![ExecKv {
                key: b"k".to_vec(),
                value: b"new".to_vec(),
            }],
            fastpq_transcripts: Vec::new(),
            fastpq_batches: Vec::new(),
        };
        let parent = parent_state_from_witness(&witness);
        // If we compute a root with only the write, it must differ from the parent root
        let writes_only = ExecWitness {
            reads: vec![],
            writes: witness.writes.clone(),
            fastpq_transcripts: Vec::new(),
            fastpq_batches: Vec::new(),
        };
        let post_from_w_only = post_state_from_witness(&writes_only);
        assert_ne!(parent, post_from_w_only);
        // Computing the post root with reads present also differs whenever writes exist,
        // because the post root only commits to writes in that case.
        let post = post_state_from_witness(&witness);
        assert_ne!(parent, post);
    }
}
