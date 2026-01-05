//! Exec-vote helpers: compute `post_state_root` via SMT, build votes, and assemble QCs.
//!
//! This module is internal and side-effect free; consumed by the Sumeragi execution pipeline.

use iroha_crypto::{Hash, HashOf};
use iroha_data_model::block::BlockHeader;

use super::{
    consensus::{CommitAggregate, ExecVote, ExecWitness, ExecutionQC},
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

/// Build an `ExecVote` by recomputing `post_state_root` from the witness.
#[allow(dead_code)] // integration staged; used in unit tests
pub fn build_exec_vote_from_witness(
    subject_block_hash: HashOf<BlockHeader>,
    parent_state_root: Hash,
    height: u64,
    view: u64,
    epoch: u64,
    signer: u32,
    witness: &ExecWitness,
) -> ExecVote {
    let post = post_state_from_witness(witness);
    ExecVote {
        block_hash: subject_block_hash,
        parent_state_root,
        post_state_root: post,
        height,
        view,
        epoch,
        signer,
        bls_sig: Vec::new(),
    }
}

/// Assemble an `ExecutionQC` from its scalar fields and an aggregate (caller constructs bitmap+agg).
#[allow(dead_code)] // integration staged; used in unit tests
pub fn assemble_execution_qc(
    subject_block_hash: HashOf<BlockHeader>,
    parent_state_root: Hash,
    post_state_root: Hash,
    height: u64,
    view: u64,
    epoch: u64,
    aggregate: CommitAggregate,
) -> ExecutionQC {
    ExecutionQC {
        subject_block_hash,
        parent_state_root,
        post_state_root,
        height,
        view,
        epoch,
        aggregate,
    }
}

#[cfg(test)]
mod tests {
    use iroha_crypto::Hash;
    use iroha_data_model::block::BlockHeader as DMHeader;

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
    fn build_exec_vote_uses_smt_root() {
        use super::super::consensus::{ExecKv, ExecWitness};
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
        // Construct a dummy block hash
        let header = DMHeader::new(
            core::num::NonZeroU64::new(1).unwrap(),
            None,
            None,
            None,
            0,
            0,
        );
        let vote = build_exec_vote_from_witness(header.hash(), Hash::new([]), 1, 1, 0, 0, &witness);
        // A write overrides the read: comparing with a recompute should match exactly.
        let recomputed = post_state_from_witness(&witness);
        assert_eq!(vote.post_state_root, recomputed);
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

    #[test]
    fn assemble_execution_qc_retains_fields() {
        let block_hash = HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([0xAB; 32]));
        let parent_root = Hash::new([0x11; 32]);
        let aggregate = CommitAggregate {
            signers_bitmap: vec![0xAA],
            bls_aggregate_signature: vec![0xBB],
        };
        let qc = assemble_execution_qc(
            block_hash,
            parent_root,
            Hash::new([0u8; 32]),
            5,
            6,
            7,
            aggregate.clone(),
        );
        assert_eq!(qc.height, 5);
        assert_eq!(qc.view, 6);
        assert_eq!(qc.epoch, 7);
        assert_eq!(qc.aggregate.signers_bitmap, aggregate.signers_bitmap);
        assert_eq!(qc.parent_state_root, parent_root);
    }
}
