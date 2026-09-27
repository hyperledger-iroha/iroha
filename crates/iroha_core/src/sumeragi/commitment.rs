//! The execution result `R` of a block (`specs/sumeragi.md` §4.1; option R1 of the integration
//! plan): `R = H("iroha/sumeragi/result/v1" ‖ norito(ExecutionResultCommitment))`, where `H` is
//! the chain hash (`iroha_crypto::Hash`) and `norito(·)` the canonical Norito frame.
//!
//! [`ExecutionResultCommitment`] binds:
//! - the slim [`ExecutionCommitment`]: the witnessed pre- and post-state roots, the ordinary-write
//!   root, the KAGEMUSHA top-up root and count, the exact result-bearing block wire (length and
//!   hash; it carries every transaction result and trigger output) and the network-input and
//!   typed-output Merkle commitments;
//! - `committee_digest(C_{h+2})` and `ChainParams_{h+2}`: the configuration the block schedules
//!   (§10.1, [`super::schedule`]).
//!
//! The canonical preimage is stored as `CommitCertificate.result_preimage` next to the block, so a
//! proof (§11) or a KAGEMUSHA attestation (§3.7) can disclose it and anyone can re-hash it
//! ([`result_of_preimage`]).
//!
//! Deviation (Appendix E): the post-state root covers the witnessed write set only, not the full
//! state; divergence in unwitnessed state (roles, permissions, peers, parameters, triggers) shows
//! up in `R` only once it changes a witnessed value or an output, and events are bound only as far
//! as the result-bearing block carries them.
//!
//! Every function here is pure and deterministic: the inputs are the execution witness, the
//! executed block and the scheduled configuration; no clock, no node configuration and no
//! hash-map iteration order enter `R`. Each sparse Merkle tree is built once per block.

use std::collections::BTreeMap;

use iroha_crypto::{Hash, MerkleTreeCommitment};
use iroha_data_model::{
    block::{
        SignedBlock,
        consensus::ExecWitness,
        consensus_v2::{
            ExecutionCommitment as V2ExecutionCommitment, MAX_EXECUTED_BLOCK_WIRE_BYTES,
        },
        execution_output::ExecutionOutputV1,
    },
    execution_witness::KAGEMUSHA_RESERVE_RECEIPT_WITNESS_KEY_TAG_V1,
    isi::kagemusha_v1::{
        KagemushaOperationKindV1, KagemushaReserveReceiptV1, KagemushaReserveReceiptWitnessV1,
    },
    transaction::signed::TransactionEntrypoint,
};
use iroha_sumeragi::{
    preimage::committee_digest_preimage,
    types::{Hash32, HeightConfig},
};
use norito::{NoritoDeserialize, NoritoSerialize};
use thiserror::Error;

use super::schedule::ChainParamsRecord;
use crate::exec_witness::{
    roots::{parent_state_from_witness, witness_pairs},
    smt::compute_post_state_root,
};

/// Domain tag of `R` (§4.1).
pub const RESULT_TAG: &[u8] = b"iroha/sumeragi/result/v1";

/// The chain hash `H` (§1): `iroha_crypto::Hash` as a core [`Hash32`].
#[must_use]
pub fn chain_hash(bytes: &[u8]) -> Hash32 {
    Hash32(<[u8; 32]>::from(Hash::new(bytes)))
}

/// `R` of a canonical result preimage: `H(RESULT_TAG ‖ preimage)`.
#[must_use]
pub fn result_of_preimage(preimage: &[u8]) -> Hash32 {
    let mut bytes = Vec::with_capacity(RESULT_TAG.len() + preimage.len());
    bytes.extend_from_slice(RESULT_TAG);
    bytes.extend_from_slice(preimage);
    chain_hash(&bytes)
}

/// The deterministic outcome of executing one block: roots over the execution witness and the
/// identity of the result-bearing block. The v2 commitment without its native-AMX, lane-finality
/// and merge-carrier fields.
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "iroha_core::sumeragi::commitment::ExecutionCommitment")]
#[derive(Clone, Copy, Debug, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
pub struct ExecutionCommitment {
    /// Root of the witnessed pre-state values of the keys the block changed.
    pub parent_state_root: Hash,
    /// Post-state root of the witnessed writes (combined with the KAGEMUSHA top-up root when
    /// the block carries top-ups).
    pub post_state_root: Hash,
    /// Root of the canonical last-write-wins witnessed writes.
    pub ordinary_writes_root: Hash,
    /// Root of the KAGEMUSHA top-up tree, when the block carries top-ups.
    pub kagemusha_top_up_root: Option<Hash>,
    /// Number of KAGEMUSHA top-ups.
    pub kagemusha_top_up_count: u32,
    /// Byte length of the canonical result-bearing block wire.
    pub executed_block_wire_len: u64,
    /// Hash of the canonical result-bearing block wire (every transaction result and output).
    pub executed_block_wire_hash: Hash,
    /// Network-input Merkle commitment of the block.
    pub transaction_input_commitment: Option<MerkleTreeCommitment<TransactionEntrypoint>>,
    /// Typed-output Merkle commitment of the block, including internal invocations.
    pub transaction_output_commitment: Option<MerkleTreeCommitment<ExecutionOutputV1>>,
}

/// The preimage of `R` (§4.1): the execution commitment and the configuration of `h + 2`.
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "iroha_core::sumeragi::commitment::ExecutionResultCommitment")]
#[derive(Clone, Debug, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
pub struct ExecutionResultCommitment {
    /// What executing the block produced.
    pub execution: ExecutionCommitment,
    /// `committee_digest(C_{h+2})` (§2.1) under the chain hash.
    pub next_committee_digest: [u8; 32],
    /// `ChainParams_{h+2}`.
    pub next_params: ChainParamsRecord,
}

impl ExecutionResultCommitment {
    /// Bind `execution` to the configuration `next` scheduled for `h + 2`.
    #[must_use]
    pub fn new(execution: ExecutionCommitment, next: &HeightConfig) -> Self {
        Self {
            execution,
            next_committee_digest: chain_hash(&committee_digest_preimage(&next.committee)).0,
            next_params: ChainParamsRecord::from_core(&next.params),
        }
    }

    /// The canonical preimage bytes (stored as `CommitCertificate.result_preimage`).
    ///
    /// # Errors
    /// A Norito serialization failure.
    pub fn preimage(&self) -> Result<Vec<u8>, CommitmentError> {
        norito::encode_canonical(self).map_err(|error| CommitmentError::Encoding(error.to_string()))
    }

    /// Decode a canonical preimage (e.g. from a stored or received certificate).
    ///
    /// # Errors
    /// The bytes are not one canonical frame of this type.
    pub fn decode(preimage: &[u8]) -> Result<Self, CommitmentError> {
        norito::decode_canonical(preimage)
            .map_err(|error| CommitmentError::Encoding(error.to_string()))
    }

    /// `R` of this commitment.
    ///
    /// # Errors
    /// A Norito serialization failure.
    pub fn result(&self) -> Result<Hash32, CommitmentError> {
        self.preimage().map(|bytes| result_of_preimage(&bytes))
    }
}

/// Why `R` could not be computed. Every variant is a deterministic function of the executed
/// block and its witness (a local bug, never the proposer's fault alone).
#[derive(Clone, Debug, PartialEq, Eq, Error)]
pub enum CommitmentError {
    /// The executed block carries no execution result.
    #[error("the executed block has no execution result")]
    MissingResult,
    /// The executed block already carries a commit certificate.
    #[error("the executed block already carries a commit certificate")]
    CertifiedBlock,
    /// The block's outputs or their Merkle cache are malformed.
    #[error("malformed execution outputs: {0}")]
    InvalidOutputs(String),
    /// The result-bearing block wire is empty or above the protocol bound.
    #[error("the executed block wire length {0} is out of range")]
    WireLength(u64),
    /// The witness carries malformed or duplicate KAGEMUSHA receipts.
    #[error("invalid KAGEMUSHA top-ups: {0}")]
    KagemushaTopUps(String),
    /// A Norito encoding or decoding failure.
    #[error("encoding: {0}")]
    Encoding(String),
}

/// The execution commitment of `executed` (the result-bearing block, without a certificate)
/// whose execution produced `witness`.
///
/// # Errors
/// See [`CommitmentError`].
pub fn execution_commitment(
    witness: &ExecWitness,
    executed: &SignedBlock,
) -> Result<ExecutionCommitment, CommitmentError> {
    if !executed.has_results() {
        return Err(CommitmentError::MissingResult);
    }
    if executed.commit_certificate().is_some() {
        return Err(CommitmentError::CertifiedBlock);
    }
    executed
        .validate_output_merkle_cache()
        .map_err(|error| CommitmentError::InvalidOutputs(error.to_string()))?;
    let wire = executed
        .encode_wire()
        .map_err(|error| CommitmentError::Encoding(error.to_string()))?;
    let executed_block_wire_len =
        u64::try_from(wire.len()).map_err(|_| CommitmentError::WireLength(u64::MAX))?;
    if executed_block_wire_len == 0 || executed_block_wire_len > MAX_EXECUTED_BLOCK_WIRE_BYTES {
        return Err(CommitmentError::WireLength(executed_block_wire_len));
    }
    // One tree per root: the witnessed writes (or, for a read-only block, reads) and the
    // pre-values of the written keys.
    let (reads, writes) = witness_pairs(witness);
    let witnessed_root = compute_post_state_root(&reads, &writes);
    let ordinary_writes_root = if writes.is_empty() {
        compute_post_state_root(&[], &[])
    } else {
        witnessed_root
    };
    let parent_state_root = parent_state_from_witness(witness);
    let (post_state_root, kagemusha_top_up_root, kagemusha_top_up_count) =
        match kagemusha_top_ups(witness)? {
            None => (witnessed_root, None, 0),
            Some((root, count)) => (
                V2ExecutionCommitment::kagemusha_post_state_root_v1(
                    count,
                    ordinary_writes_root,
                    root,
                ),
                Some(root),
                count,
            ),
        };
    Ok(ExecutionCommitment {
        parent_state_root,
        post_state_root,
        ordinary_writes_root,
        kagemusha_top_up_root,
        kagemusha_top_up_count,
        executed_block_wire_len,
        executed_block_wire_hash: Hash::new(&wire),
        transaction_input_commitment: executed.network_input_merkle_commitment(),
        transaction_output_commitment: executed.output_merkle_commitment(),
    })
}

/// `R` of a block (§4.1): its execution commitment bound to the configuration `next` it
/// schedules for `h + 2`. Returns the commitment, its canonical preimage and `R`.
///
/// # Errors
/// See [`CommitmentError`].
pub fn execution_result(
    witness: &ExecWitness,
    executed: &SignedBlock,
    next: &HeightConfig,
) -> Result<(ExecutionResultCommitment, Vec<u8>, Hash32), CommitmentError> {
    let commitment = ExecutionResultCommitment::new(execution_commitment(witness, executed)?, next);
    let preimage = commitment.preimage()?;
    let result = result_of_preimage(&preimage);
    Ok((commitment, preimage, result))
}

/// The KAGEMUSHA top-up root and count of the witness (`None` without top-ups), from the
/// canonical last-write-wins receipt writes. No sparse-tree path is built: `R` needs the leaves
/// only. Duplicate receipt writes and receipts whose key does not match their operation fail
/// closed, as in the v2 projection.
fn kagemusha_top_ups(witness: &ExecWitness) -> Result<Option<(Hash, u32)>, CommitmentError> {
    let is_receipt =
        |key: &[u8]| key.first() == Some(&KAGEMUSHA_RESERVE_RECEIPT_WITNESS_KEY_TAG_V1);
    let tagged = witness
        .writes
        .iter()
        .filter(|entry| is_receipt(&entry.key))
        .count();
    if tagged == 0 {
        return Ok(None);
    }
    let receipts = witness
        .writes
        .iter()
        .filter(|entry| is_receipt(&entry.key))
        .map(|entry| (entry.key.as_slice(), entry.value.as_slice()))
        .collect::<BTreeMap<_, _>>();
    if receipts.len() != tagged {
        return Err(CommitmentError::KagemushaTopUps(
            "duplicate receipt writes".to_owned(),
        ));
    }
    let mut leaves = Vec::new();
    for (key, value) in receipts {
        let receipt: KagemushaReserveReceiptV1 = norito::decode_canonical(value)
            .map_err(|error| CommitmentError::KagemushaTopUps(error.to_string()))?;
        if key != KagemushaReserveReceiptWitnessV1::expected_key(receipt.operation_id).as_slice() {
            return Err(CommitmentError::KagemushaTopUps(
                "receipt key does not match its operation".to_owned(),
            ));
        }
        if receipt.kind == KagemushaOperationKindV1::TopUp {
            leaves.push(
                crate::zk::kagemusha_v1_recursion::kagemusha_top_up_leaf_from_receipt_v1(&receipt)
                    .map_err(|error| CommitmentError::KagemushaTopUps(error.to_string()))?,
            );
        }
    }
    if leaves.is_empty() {
        return Ok(None);
    }
    let tree = crate::zk::kagemusha_v1_recursion::KagemushaMintFinalityTreeV1::new(leaves)
        .map_err(|error| CommitmentError::KagemushaTopUps(error.to_string()))?;
    Ok(Some((tree.execution_root(), tree.leaf_count())))
}

#[cfg(test)]
mod tests {
    use iroha_crypto::KeyPair;
    use iroha_data_model::block::consensus::ExecKv;
    use iroha_sumeragi::types::{ChainParams, Committee, PublicKey};

    use super::*;
    use crate::block::ValidBlock;

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

    fn sample_witness() -> ExecWitness {
        witness(
            vec![kv("balance/alice", "10"), kv("balance/bob", "3")],
            vec![kv("balance/alice", "7"), kv("balance/bob", "6")],
        )
    }

    fn executed(key: &KeyPair) -> SignedBlock {
        ValidBlock::new_dummy(key.private_key()).into()
    }

    fn committee(bytes: &[u8]) -> Committee {
        Committee::new(
            bytes
                .iter()
                .map(|b| PublicKey::new(vec![*b; 48]).expect("key"))
                .collect(),
        )
        .expect("committee")
    }

    fn next() -> HeightConfig {
        HeightConfig {
            committee: committee(&[1, 2, 3, 4]),
            params: ChainParams::default(),
        }
    }

    #[test]
    fn result_is_deterministic_across_independent_computations() {
        let key = KeyPair::random();
        let block = executed(&key);
        // Two independent computations over separately built but equal inputs.
        let (first, first_preimage, first_r) =
            execution_result(&sample_witness(), &block.clone(), &next()).expect("R");
        let (second, second_preimage, second_r) =
            execution_result(&sample_witness(), &block, &next()).expect("R");
        assert_eq!(first, second);
        assert_eq!(first_preimage, second_preimage);
        assert_eq!(first_r, second_r);
        assert_eq!(result_of_preimage(&first_preimage), first_r);
        assert_eq!(first.result().expect("R"), first_r);
        // The preimage decodes canonically back to the commitment.
        assert_eq!(
            ExecutionResultCommitment::decode(&first_preimage).expect("decode"),
            first
        );
        assert!(ExecutionResultCommitment::decode(&first_preimage[1..]).is_err());
        // The chain hash is the LSB-marked iroha hash (every core hash is a valid iroha hash).
        assert_eq!(first_r.0[31] & 1, 1);
    }

    #[test]
    fn witness_order_and_incidental_reads_do_not_change_r() {
        let block = executed(&KeyPair::random());
        let reordered = witness(
            vec![kv("balance/bob", "3"), kv("balance/alice", "10")],
            vec![kv("balance/bob", "6"), kv("balance/alice", "7")],
        );
        let with_incidental = witness(
            vec![
                kv("balance/alice", "10"),
                kv("balance/bob", "3"),
                kv("permission-cache", "hit"),
            ],
            sample_witness().writes,
        );
        let base = execution_result(&sample_witness(), &block, &next())
            .expect("R")
            .2;
        assert_eq!(
            execution_result(&reordered, &block, &next()).expect("R").2,
            base
        );
        assert_eq!(
            execution_result(&with_incidental, &block, &next())
                .expect("R")
                .2,
            base
        );
    }

    #[test]
    fn every_bound_input_changes_r() {
        let key = KeyPair::random();
        let block = executed(&key);
        let base = execution_result(&sample_witness(), &block, &next())
            .expect("R")
            .2;
        let r_of = |witness: &ExecWitness, block: &SignedBlock, next: &HeightConfig| {
            execution_result(witness, block, next).expect("R").2
        };
        // A witnessed write.
        let mut changed_write = sample_witness();
        changed_write.writes[0].value = b"8".to_vec();
        assert_ne!(r_of(&changed_write, &block, &next()), base);
        // A pre-value of a written key.
        let mut changed_read = sample_witness();
        changed_read.reads[1].value = b"4".to_vec();
        assert_ne!(r_of(&changed_read, &block, &next()), base);
        // An execution result: the same proposal with another result (here the committed
        // fragment count) has another result-bearing wire.
        let mut other_result = block.canonical_resultless_proposal();
        other_result
            .set_execution_outputs(
                Vec::new(),
                1,
                Default::default(),
                Vec::new(),
                Default::default(),
                Default::default(),
                Vec::new(),
                &crate::execution_output_test_support::structural_output_limits(),
            )
            .expect("install another result");
        assert_eq!(other_result.hash(), block.hash());
        assert_ne!(r_of(&sample_witness(), &other_result, &next()), base);
        let other_signer = executed(&KeyPair::random());
        assert_ne!(r_of(&sample_witness(), &other_signer, &next()), base);
        // The next committee.
        let mut other_committee = next();
        other_committee.committee = committee(&[1, 2, 3, 5]);
        assert_ne!(r_of(&sample_witness(), &block, &other_committee), base);
        let mut smaller = next();
        smaller.committee = committee(&[1, 2, 3]);
        assert_ne!(r_of(&sample_witness(), &block, &smaller), base);
        // Every next chain parameter.
        let params = ChainParams::default();
        for changed in [
            ChainParams {
                block_time: params.block_time + 1,
                ..params
            },
            ChainParams {
                payload_retry_interval: params.payload_retry_interval + 1,
                ..params
            },
            ChainParams {
                e_max: params.e_max + 1,
                ..params
            },
            ChainParams {
                a_max: params.a_max + 1,
                ..params
            },
            ChainParams {
                max_block_bytes: params.max_block_bytes - 1,
                ..params
            },
            ChainParams {
                epoch_length: params.epoch_length + 1,
                ..params
            },
        ] {
            let config = HeightConfig {
                committee: next().committee,
                params: changed,
            };
            assert_ne!(
                r_of(&sample_witness(), &block, &config),
                base,
                "{changed:?}"
            );
        }
    }

    #[test]
    fn commitment_binds_the_result_bearing_wire_and_roots() {
        let block = executed(&KeyPair::random());
        let commitment = execution_commitment(&sample_witness(), &block).expect("commitment");
        let wire = block.encode_wire().expect("wire");
        assert_eq!(commitment.executed_block_wire_hash, Hash::new(&wire));
        assert_eq!(
            commitment.executed_block_wire_len,
            u64::try_from(wire.len()).unwrap()
        );
        assert_eq!(
            commitment.post_state_root,
            crate::exec_witness::roots::post_state_from_witness(&sample_witness())
        );
        assert_eq!(commitment.ordinary_writes_root, commitment.post_state_root);
        assert_eq!(
            commitment.parent_state_root,
            parent_state_from_witness(&sample_witness())
        );
        assert_eq!(commitment.kagemusha_top_up_root, None);
        assert_eq!(commitment.kagemusha_top_up_count, 0);
        assert_eq!(
            commitment.transaction_output_commitment,
            block.output_merkle_commitment()
        );
        // A read-only block: the ordinary-write root is the empty tree's.
        let read_only = witness(vec![kv("config", "1")], Vec::new());
        let commitment = execution_commitment(&read_only, &block).expect("commitment");
        assert_eq!(
            commitment.ordinary_writes_root,
            compute_post_state_root(&[], &[])
        );
        assert_ne!(commitment.post_state_root, commitment.ordinary_writes_root);
    }

    #[test]
    fn unexecuted_or_certified_blocks_are_refused() {
        let block = executed(&KeyPair::random());
        let proposal = block.canonical_resultless_proposal();
        assert_eq!(
            execution_commitment(&sample_witness(), &proposal),
            Err(CommitmentError::MissingResult)
        );
        let certified = block.with_commit_certificate(Some(
            iroha_data_model::block::CommitCertificate::new(vec![1], vec![2], vec![3]),
        ));
        assert_eq!(
            execution_commitment(&sample_witness(), &certified),
            Err(CommitmentError::CertifiedBlock)
        );
    }

    #[test]
    fn duplicate_or_malformed_kagemusha_receipts_fail_closed() {
        let block = executed(&KeyPair::random());
        let tag = KAGEMUSHA_RESERVE_RECEIPT_WITNESS_KEY_TAG_V1;
        let receipt_write = ExecKv {
            key: vec![tag, 1, 2, 3],
            value: vec![9, 9],
        };
        let duplicate = witness(
            Vec::new(),
            vec![receipt_write.clone(), receipt_write.clone()],
        );
        assert!(matches!(
            execution_commitment(&duplicate, &block),
            Err(CommitmentError::KagemushaTopUps(_))
        ));
        let malformed = witness(Vec::new(), vec![receipt_write]);
        assert!(matches!(
            execution_commitment(&malformed, &block),
            Err(CommitmentError::KagemushaTopUps(_))
        ));
    }

    #[test]
    fn chain_hash_is_the_iroha_hash() {
        assert_eq!(chain_hash(b"x").0, <[u8; 32]>::from(Hash::new(b"x")));
        let mut tagged = RESULT_TAG.to_vec();
        tagged.extend_from_slice(b"p");
        assert_eq!(result_of_preimage(b"p"), chain_hash(&tagged));
    }
}
