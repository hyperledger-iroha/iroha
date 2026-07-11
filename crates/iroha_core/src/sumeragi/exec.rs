//! Exec-vote helpers: compute `post_state_root` via SMT, build votes, and assemble QCs.
//!
//! This module is internal and side-effect free; consumed by the Sumeragi execution pipeline.

use iroha_crypto::Hash;
use iroha_data_model::block::consensus_v2 as wire;

use super::{
    consensus::ExecWitness,
    smt::{
        KvPair, build_kagemusha_topup_block_commitment, compute_consensus_post_state_root,
        compute_post_state_root,
    },
};

fn witness_pairs(witness: &ExecWitness) -> (Vec<KvPair>, Vec<KvPair>) {
    let reads = witness
        .reads
        .iter()
        .map(|kv| KvPair::new(kv.key.clone(), kv.value.clone()))
        .collect();
    let writes = witness
        .writes
        .iter()
        .map(|kv| KvPair::new(kv.key.clone(), kv.value.clone()))
        .collect();
    (reads, writes)
}

/// Convert an `ExecWitness` into SMT `KvPair` slices and compute the `post_state_root`.
pub fn post_state_from_witness(w: &ExecWitness) -> Hash {
    try_post_state_from_witness(w).unwrap_or_else(|error| {
        let mut preimage = b"iroha:sumeragi:invalid-exec-witness".to_vec();
        preimage.push(0);
        preimage.extend_from_slice(error.as_bytes());
        Hash::new(preimage)
    })
}

/// Checked variant used before a validator signs execution roots.
pub fn try_post_state_from_witness(w: &ExecWitness) -> Result<Hash, &'static str> {
    let (reads, writes) = witness_pairs(w);
    compute_consensus_post_state_root(&reads, &writes)
}

/// Derive the exact execution commitment authenticated by Sumeragi-v2 votes.
///
/// This is intentionally the only projection used by candidate validation and
/// decided application. It consumes the actual `StateBlock` witness and uses
/// the same bounded Kagemusha subtree builder as finality-proof generation.
pub(crate) fn execution_commitment_from_witness(
    witness: &ExecWitness,
) -> Result<wire::ExecutionCommitment, &'static str> {
    let (reads, writes) = witness_pairs(witness);
    let parent_state_root = parent_state_from_witness(witness);
    match build_kagemusha_topup_block_commitment(&writes)? {
        Some(kagemusha) => wire::ExecutionCommitment::new(
            parent_state_root,
            kagemusha.post_state_root,
            kagemusha.ordinary_writes_root,
            Some(kagemusha.topup_anchor_root),
            u32::try_from(kagemusha.leaves.len())
                .map_err(|_| "Kagemusha V2 top-up anchor count does not fit u32")?,
        )
        .map_err(|_| "Kagemusha V2 execution commitment is not canonical"),
        None => wire::ExecutionCommitment::new(
            parent_state_root,
            compute_consensus_post_state_root(&reads, &writes)?,
            compute_post_state_root(&[], &writes),
            None,
            0,
        )
        .map_err(|_| "Sumeragi V2 execution commitment is not canonical"),
    }
}

/// Compute the `parent_state_root` using only the witnessed reads (pre-values).
/// When a block writes state, only pre-values for written keys are included.
/// Read-only access witnesses can vary across execution strategies and should
/// not perturb the commit vote for an otherwise identical state transition.
pub fn parent_state_from_witness(w: &ExecWitness) -> Hash {
    let reads: Vec<KvPair> = if w.writes.is_empty() {
        w.reads
            .iter()
            .map(|kv| KvPair::new(kv.key.clone(), kv.value.clone()))
            .collect()
    } else {
        let write_keys: std::collections::BTreeSet<&[u8]> =
            w.writes.iter().map(|kv| kv.key.as_slice()).collect();
        w.reads
            .iter()
            .filter(|kv| write_keys.contains(kv.key.as_slice()))
            .map(|kv| KvPair::new(kv.key.clone(), kv.value.clone()))
            .collect()
    };
    compute_post_state_root(&reads, &[])
}

#[cfg(test)]
mod tests {
    use super::super::consensus::{ExecKv, ExecWitness};
    use super::*;

    fn kv(key: &str, value: &str) -> ExecKv {
        ExecKv {
            key: key.as_bytes().to_vec(),
            value: value.as_bytes().to_vec(),
        }
    }

    fn witness(reads: Vec<ExecKv>, writes: Vec<ExecKv>) -> ExecWitness {
        ExecWitness {
            reads,
            writes,
            fastpq_transcripts: Vec::new(),
            fastpq_batches: Vec::new(),
        }
    }

    #[test]
    fn post_root_projection_matches_formal_empty_pure_read_write_and_conflict_cases() {
        let empty = witness(Vec::new(), Vec::new());
        assert_eq!(
            post_state_from_witness(&empty),
            compute_post_state_root(&[], &[])
        );

        let pure_reads = witness(vec![kv("account", "old")], Vec::new());
        assert_eq!(
            post_state_from_witness(&pure_reads),
            compute_post_state_root(&[KvPair::new(b"account", b"old")], &[])
        );
        assert_ne!(
            post_state_from_witness(&pure_reads),
            post_state_from_witness(&empty)
        );

        let writes_with_incidental_reads = witness(
            vec![kv("account", "old"), kv("permission-cache", "true")],
            vec![kv("account", "new")],
        );
        let writes_only = witness(Vec::new(), vec![kv("account", "new")]);
        assert_eq!(
            post_state_from_witness(&writes_with_incidental_reads),
            post_state_from_witness(&writes_only)
        );
        assert_ne!(
            post_state_from_witness(&writes_with_incidental_reads),
            post_state_from_witness(&pure_reads)
        );
    }

    #[test]
    fn parent_root_projection_matches_formal_empty_read_only_and_write_filter_cases() {
        let empty = witness(Vec::new(), Vec::new());
        assert_eq!(
            parent_state_from_witness(&empty),
            compute_post_state_root(&[], &[])
        );

        let read_only = witness(vec![kv("config", "1"), kv("other", "2")], Vec::new());
        assert_eq!(
            parent_state_from_witness(&read_only),
            compute_post_state_root(
                &[KvPair::new(b"config", b"1"), KvPair::new(b"other", b"2")],
                &[]
            )
        );

        let witness_with_writes = witness(
            vec![kv("balance", "10"), kv("permission-cache", "true")],
            vec![kv("balance", "7"), kv("write-only", "created")],
        );
        let parent = parent_state_from_witness(&witness_with_writes);
        assert_eq!(
            parent,
            compute_post_state_root(&[KvPair::new(b"balance", b"10")], &[])
        );

        let changed_write_values = witness(
            witness_with_writes.reads.clone(),
            vec![kv("balance", "999"), kv("write-only", "different")],
        );
        assert_eq!(parent, parent_state_from_witness(&changed_write_values));
        assert_ne!(parent, post_state_from_witness(&witness_with_writes));
    }

    #[test]
    fn root_projection_is_order_independent_and_deduplicates_identical_keys() {
        let ordered = witness(
            vec![kv("a", "old-a"), kv("b", "old-b")],
            vec![kv("a", "new-a"), kv("b", "new-b")],
        );
        let reordered = witness(
            vec![kv("b", "old-b"), kv("a", "old-a")],
            vec![kv("b", "new-b"), kv("a", "new-a")],
        );
        assert_eq!(
            post_state_from_witness(&ordered),
            post_state_from_witness(&reordered)
        );
        assert_eq!(
            parent_state_from_witness(&ordered),
            parent_state_from_witness(&reordered)
        );

        let duplicated_reads = witness(vec![kv("config", "1"), kv("config", "1")], Vec::new());
        let single_read = witness(vec![kv("config", "1")], Vec::new());
        assert_eq!(
            post_state_from_witness(&duplicated_reads),
            post_state_from_witness(&single_read)
        );

        let duplicated_writes = witness(Vec::new(), vec![kv("balance", "7"), kv("balance", "7")]);
        let single_write = witness(Vec::new(), vec![kv("balance", "7")]);
        assert_eq!(
            post_state_from_witness(&duplicated_writes),
            post_state_from_witness(&single_write)
        );
    }

    #[test]
    fn v2_execution_commitment_exposes_exact_bounded_topup_projection() {
        let mut operation_key = vec![
            super::super::smt::KAGEMUSHA_V2_TOPUP_ANCHOR_WITNESS_KEY_TAG,
        ];
        operation_key.extend_from_slice(&[0xA1; 32]);
        let witness = ExecWitness {
            reads: vec![ExecKv {
                key: operation_key.clone(),
                value: Vec::new(),
            }],
            writes: vec![
                ExecKv {
                    key: b"ordinary".to_vec(),
                    value: b"write".to_vec(),
                },
                ExecKv {
                    key: operation_key,
                    value: vec![0xB2; 32],
                },
            ],
            fastpq_transcripts: Vec::new(),
            fastpq_batches: Vec::new(),
        };

        let commitment =
            execution_commitment_from_witness(&witness).expect("valid top-up commitment");
        assert_eq!(commitment.topup_anchor_count, 1);
        assert!(commitment.topup_anchor_root.is_some());
        assert_eq!(commitment.validate(), Ok(()));
        assert_eq!(
            commitment.post_state_root,
            try_post_state_from_witness(&witness).expect("same consensus post root")
        );
    }

    #[test]
    fn roots_ignore_fastpq_payloads_match_formal_gate() {
        use iroha_data_model::fastpq::{
            FastpqOperationKind, FastpqPublicInputs, FastpqStateTransition, FastpqTransitionBatch,
            TransferTranscriptBundle,
        };

        let base = witness(vec![kv("balance", "10")], vec![kv("balance", "7")]);
        let mut with_fastpq = base.clone();
        with_fastpq
            .fastpq_transcripts
            .push(TransferTranscriptBundle {
                entry_hash: Hash::prehashed([0x11; Hash::LENGTH]),
                transcripts: Vec::new(),
            });
        with_fastpq.fastpq_batches.push(FastpqTransitionBatch {
            parameter: String::from("test-params"),
            public_inputs: FastpqPublicInputs {
                dsid: [0x01; 16],
                slot: 7,
                old_root: [0x02; 32],
                new_root: [0x03; 32],
                perm_root: [0x04; 32],
                tx_set_hash: [0x05; 32],
            },
            transitions: vec![FastpqStateTransition {
                key: b"fastpq-key".to_vec(),
                pre_value: b"fastpq-pre".to_vec(),
                post_value: b"fastpq-post".to_vec(),
                operation: FastpqOperationKind::Transfer,
            }],
            metadata: std::collections::BTreeMap::from([(String::from("entry"), vec![0xAA])]),
        });

        assert_eq!(
            post_state_from_witness(&base),
            post_state_from_witness(&with_fastpq)
        );
        assert_eq!(
            parent_state_from_witness(&base),
            parent_state_from_witness(&with_fastpq)
        );
    }
}
