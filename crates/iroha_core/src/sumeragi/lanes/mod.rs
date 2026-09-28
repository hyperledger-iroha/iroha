//! Lanes of the global chain (`specs/sumeragi_lanes.md`): the identity and pinned configuration
//! of a lane instance, the lane block payload ([`LaneBatch`]), its result `R` ([`LaneResult`]) and
//! lane admission (§3.2).
//!
//! A lane never changes world state. Its execute-before-vote step is [`admit`]: a pure function of
//! the lane block, the lane's own committed chain and permanent facts of the global chain (the
//! anchor block's hash and creation time, and the lane record's immutable fields), so every
//! honest member computes the same `R` whenever it executes.

/// The durable block store of a lane instance.
pub mod store;

use std::collections::BTreeSet;

use iroha_crypto::HashOf;
use iroha_data_model::{
    NetworkId,
    sumeragi_lanes::SumeragiLaneRecord,
    transaction::{SignedTransaction, TransactionEntrypoint},
};
use iroha_sumeragi::{
    crypto::Crypto,
    preimage::{InstanceKind, committee_digest_preimage, instance_id},
    types::{Committee, Hash32, HeightConfig},
};
use norito::codec::{Decode, DecodeAll as _, Encode};
use thiserror::Error;

use super::{
    commitment::chain_hash,
    schedule::{ChainParamsRecord, ScheduleError, consensus_key},
};

/// Domain tag of a lane incarnation's genesis block hash.
pub const LANE_GENESIS_TAG: &[u8] = b"iroha/sumeragi/lane/genesis/v1";
/// Domain tag of a lane incarnation's genesis result.
pub const LANE_GENESIS_RESULT_TAG: &[u8] = b"iroha/sumeragi/lane/genesis-result/v1";
/// Domain tag of a lane block's result `R`.
pub const LANE_RESULT_TAG: &[u8] = b"iroha/sumeragi/lane/result/v1";

/// Genesis block hash of a lane incarnation: `H(TAG ‖ network ‖ be32(lane) ‖ incarnation)`.
///
/// Every incarnation has its own genesis hash, hence its own instance id: a recreated lane never
/// shares safety records or signatures with an earlier incarnation.
#[must_use]
pub fn lane_genesis_hash(network: &NetworkId, record: &SumeragiLaneRecord) -> Hash32 {
    let mut bytes = Vec::with_capacity(LANE_GENESIS_TAG.len() + 32 + 4 + 32);
    bytes.extend_from_slice(LANE_GENESIS_TAG);
    bytes.extend_from_slice(network.as_bytes());
    bytes.extend_from_slice(&record.lane.as_u32().to_be_bytes());
    bytes.extend_from_slice(&record.incarnation);
    chain_hash(&bytes)
}

/// Instance id `I` of a lane incarnation (`sumeragi.md` §3.5 with kind `Lane`).
#[must_use]
pub fn lane_instance(
    crypto: &dyn Crypto,
    network: &NetworkId,
    chain_id: &str,
    record: &SumeragiLaneRecord,
) -> Hash32 {
    instance_id(
        crypto,
        &lane_genesis_hash(network, record),
        chain_id.as_bytes(),
        InstanceKind::Lane,
        record.lane.as_u32(),
    )
}

/// Genesis result `R_0` of a lane incarnation: binds the fields fixed at creation.
#[must_use]
pub fn lane_genesis_result(record: &SumeragiLaneRecord) -> Hash32 {
    let created = (
        record.lane,
        record.dataspace,
        record.incarnation,
        record.params.clone(),
        record.committee.clone(),
        record.created_at,
        record.active_from,
    );
    let mut bytes = LANE_GENESIS_RESULT_TAG.to_vec();
    bytes.extend_from_slice(&created.encode());
    chain_hash(&bytes)
}

/// The height configuration of every height of a lane incarnation: its pinned committee and
/// chain parameters (§2.3).
///
/// # Errors
/// A pinned member whose key is not BLS-normal, an empty or oversized committee, or chain
/// parameters that fail §9.4 validation.
pub fn lane_height_config(record: &SumeragiLaneRecord) -> Result<HeightConfig, LaneError> {
    let keys = record
        .committee
        .iter()
        .map(|member| consensus_key(&member.peer))
        .collect::<Result<Vec<_>, _>>()
        .map_err(LaneError::Schedule)?;
    let committee = Committee::new(keys)
        .map_err(|error| LaneError::Schedule(ScheduleError::Committee(error)))?;
    let params = ChainParamsRecord::from_parameters(&record.params);
    params
        .validate()
        .map_err(|error| LaneError::Schedule(ScheduleError::Params(error)))?;
    Ok(HeightConfig {
        committee,
        params: params.to_core(),
    })
}

/// A lane block's payload: an ordered transaction batch anchored at a committed global block.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
pub struct LaneBatch {
    /// Global height of the anchor.
    pub anchor_height: u64,
    /// Global block hash of the anchor.
    pub anchor_hash: HashOf<iroha_data_model::block::BlockHeader>,
    /// The transactions, in lane order.
    pub transactions: Vec<SignedTransaction>,
}

impl LaneBatch {
    /// The canonical payload bytes.
    #[must_use]
    pub fn to_payload(&self) -> Vec<u8> {
        self.encode()
    }

    /// Decode canonical payload bytes; non-canonical bytes are rejected.
    ///
    /// # Errors
    /// The bytes are not the canonical encoding of a batch.
    pub fn from_payload(payload: &[u8]) -> Result<Self, AdmissionError> {
        let batch = Self::decode_all(&mut &payload[..])
            .map_err(|error| AdmissionError::Encoding(error.to_string()))?;
        if batch.encode() != payload {
            return Err(AdmissionError::Encoding("non-canonical batch".into()));
        }
        Ok(batch)
    }
}

/// What a lane block certifies (its result preimage): the anchor, the admitted transactions, the
/// payload size and the pinned next configuration (the core's lag-2 rule, §4.1).
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
pub struct LaneResult {
    /// Global height of the anchor.
    pub anchor_height: u64,
    /// Global block hash of the anchor.
    pub anchor_hash: HashOf<iroha_data_model::block::BlockHeader>,
    /// Entrypoint hashes of the admitted transactions, in batch order.
    pub tx_hashes: Vec<HashOf<TransactionEntrypoint>>,
    /// Payload length in bytes.
    pub payload_bytes: u64,
    /// `committee_digest(C_{x+2})`: the pinned committee.
    pub next_committee_digest: [u8; 32],
    /// `ChainParams_{x+2}`: the pinned parameters.
    pub next_params: ChainParamsRecord,
}

impl LaneResult {
    /// `R = H(LANE_RESULT_TAG ‖ norito(self))`.
    #[must_use]
    pub fn hash(&self) -> Hash32 {
        let mut bytes = LANE_RESULT_TAG.to_vec();
        bytes.extend_from_slice(&self.encode());
        chain_hash(&bytes)
    }
}

/// The facts of the global chain that admission reads (§3.2): all permanent once committed.
pub trait AnchorView {
    /// Hash of the global block at `height`, if this node has applied it.
    fn applied_hash(&self, height: u64) -> Option<HashOf<iroha_data_model::block::BlockHeader>>;
    /// Creation time (ms since the Unix epoch) of the applied global block at `height`.
    fn creation_time_ms(&self, height: u64) -> Option<u64>;
}

/// The lane chain that admission reads: the previous block's anchor and the hashes of committed
/// lane transactions the global chain has not merged yet (§3.2 steps 2 and 5).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LaneChainView {
    /// Anchor height of the lane's previous block (`0` before the first block).
    pub previous_anchor: u64,
    /// Transactions of committed, not yet merged lane blocks.
    pub unmerged: BTreeSet<HashOf<TransactionEntrypoint>>,
}

/// The outcome of admission.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Admission {
    /// The block is admissible; its result preimage.
    Valid(LaneResult),
    /// The node has not applied the anchor yet: execution waits (never a verdict).
    Pending,
}

/// Why a lane block is not admissible (`Invalid`).
#[derive(Clone, Debug, PartialEq, Eq, Error)]
pub enum AdmissionError {
    /// The payload is not a canonical [`LaneBatch`].
    #[error("lane payload encoding: {0}")]
    Encoding(String),
    /// The anchor's hash differs from the applied global block at its height.
    #[error("anchor {height} is not the committed global block")]
    AnchorMismatch {
        /// Anchor height.
        height: u64,
    },
    /// The anchor height regressed below the previous lane block's.
    #[error("anchor {height} is below the previous anchor {previous}")]
    AnchorRegressed {
        /// Anchor height.
        height: u64,
        /// Previous anchor height.
        previous: u64,
    },
    /// The lane is not active at the anchor (before activation, or at or after closing).
    #[error("the lane is not active at anchor {0}")]
    Inactive(u64),
    /// A transaction fails its intrinsic checks.
    #[error("transaction {index}: {reason}")]
    Transaction {
        /// Index in the batch.
        index: usize,
        /// Why.
        reason: String,
    },
    /// A transaction repeats one of this block or of the lane's unmerged blocks.
    #[error("transaction {0} is a duplicate")]
    Duplicate(usize),
}

/// Why a lane configuration cannot be formed.
#[derive(Clone, Debug, PartialEq, Eq, Error)]
pub enum LaneError {
    /// The pinned committee or parameters are not a valid core configuration.
    #[error("lane configuration: {0}")]
    Schedule(ScheduleError),
}

/// Intrinsic checks of one transaction (chain id, signature, limits), supplied by the caller so
/// admission stays independent of node configuration plumbing.
pub trait TransactionCheck {
    /// Check `tx` as of `anchor_time_ms`; `Err(reason)` if it is not admissible.
    ///
    /// # Errors
    /// The transaction fails an intrinsic check.
    fn check(&self, tx: &SignedTransaction, anchor_time_ms: u64) -> Result<(), String>;
}

/// Lane admission (§3.2): the result of a lane block with `payload`, or why it is invalid.
///
/// # Errors
/// See [`AdmissionError`].
pub fn admit(
    record: &SumeragiLaneRecord,
    anchors: &impl AnchorView,
    chain: &LaneChainView,
    checks: &impl TransactionCheck,
    config: &HeightConfig,
    payload: &[u8],
) -> Result<Admission, AdmissionError> {
    let batch = LaneBatch::from_payload(payload)?;
    let Some(applied) = anchors.applied_hash(batch.anchor_height) else {
        return Ok(Admission::Pending);
    };
    if applied != batch.anchor_hash {
        return Err(AdmissionError::AnchorMismatch {
            height: batch.anchor_height,
        });
    }
    if batch.anchor_height < chain.previous_anchor {
        return Err(AdmissionError::AnchorRegressed {
            height: batch.anchor_height,
            previous: chain.previous_anchor,
        });
    }
    if !record.admits_anchor(batch.anchor_height) {
        return Err(AdmissionError::Inactive(batch.anchor_height));
    }
    let anchor_time_ms =
        anchors
            .creation_time_ms(batch.anchor_height)
            .ok_or(AdmissionError::AnchorMismatch {
                height: batch.anchor_height,
            })?;
    let mut seen = BTreeSet::new();
    let mut tx_hashes = Vec::with_capacity(batch.transactions.len());
    for (index, tx) in batch.transactions.iter().enumerate() {
        checks
            .check(tx, anchor_time_ms)
            .map_err(|reason| AdmissionError::Transaction { index, reason })?;
        let hash = tx.hash_as_entrypoint();
        if chain.unmerged.contains(&hash) || !seen.insert(hash) {
            return Err(AdmissionError::Duplicate(index));
        }
        tx_hashes.push(hash);
    }
    Ok(Admission::Valid(LaneResult {
        anchor_height: batch.anchor_height,
        anchor_hash: batch.anchor_hash,
        tx_hashes,
        payload_bytes: u64::try_from(payload.len()).unwrap_or(u64::MAX),
        next_committee_digest: chain_hash(&committee_digest_preimage(&config.committee)).0,
        next_params: ChainParamsRecord::from_core(&config.params),
    }))
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use iroha_crypto::{Algorithm, Hash, KeyPair};
    use iroha_data_model::{
        account::AccountId,
        isi::{InstructionBox, Log},
        parameter::system::SumeragiParameters,
        sumeragi_lanes::{SumeragiLaneFrontier, SumeragiLaneMember},
        transaction::TransactionBuilder,
    };
    use iroha_model_base::{
        peer::PeerId,
        topology::{DataSpaceId, LaneId},
    };

    use super::*;
    use crate::sumeragi::crypto::BlsCrypto;

    struct Anchors(BTreeMap<u64, (HashOf<iroha_data_model::block::BlockHeader>, u64)>);

    impl AnchorView for Anchors {
        fn applied_hash(
            &self,
            height: u64,
        ) -> Option<HashOf<iroha_data_model::block::BlockHeader>> {
            self.0.get(&height).map(|(hash, _)| *hash)
        }
        fn creation_time_ms(&self, height: u64) -> Option<u64> {
            self.0.get(&height).map(|(_, time)| *time)
        }
    }

    struct AcceptAll;
    impl TransactionCheck for AcceptAll {
        fn check(&self, _tx: &SignedTransaction, _anchor_time_ms: u64) -> Result<(), String> {
            Ok(())
        }
    }

    fn block_hash(seed: u8) -> HashOf<iroha_data_model::block::BlockHeader> {
        HashOf::from_untyped_unchecked(Hash::prehashed([seed; 32]))
    }

    fn member(seed: u8) -> SumeragiLaneMember {
        let pair = KeyPair::from_seed(vec![seed; 32], Algorithm::BlsNormal);
        SumeragiLaneMember {
            peer: PeerId::new(pair.public_key().clone()),
            pop: iroha_crypto::bls_normal_pop_prove(pair.private_key()).expect("pop"),
        }
    }

    fn record(closing: Option<u64>) -> SumeragiLaneRecord {
        SumeragiLaneRecord {
            lane: LaneId::new(2),
            dataspace: DataSpaceId::new(0),
            incarnation: [9; 32],
            params: SumeragiParameters::default(),
            committee: vec![member(1), member(2), member(3), member(4)],
            created_at: 5,
            active_from: 7,
            closing,
            merged: SumeragiLaneFrontier::default(),
        }
    }

    fn tx(seed: u8) -> SignedTransaction {
        let pair = KeyPair::from_seed(vec![seed; 32], Algorithm::Ed25519);
        TransactionBuilder::new(
            NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(Hash::new(b"lane-test"))),
            AccountId::new(pair.public_key().clone()),
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_instructions([InstructionBox::from(Log::new(
            iroha_data_model::Level::INFO,
            format!("tx {seed}"),
        ))])
        .sign(pair.private_key())
    }

    fn payload(anchor: u64, txs: Vec<SignedTransaction>) -> Vec<u8> {
        LaneBatch {
            anchor_height: anchor,
            anchor_hash: block_hash(u8::try_from(anchor).expect("small")),
            transactions: txs,
        }
        .to_payload()
    }

    fn anchors() -> Anchors {
        Anchors(
            (1..=10)
                .map(|h| (h, (block_hash(u8::try_from(h).unwrap()), h * 1000)))
                .collect(),
        )
    }

    #[test]
    fn identities_differ_per_incarnation_and_bind_the_record() {
        let network =
            NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(Hash::prehashed([3; 32])));
        let crypto = BlsCrypto::new();
        let a = record(None);
        let b = SumeragiLaneRecord {
            incarnation: [10; 32],
            ..a.clone()
        };
        assert_ne!(
            lane_genesis_hash(&network, &a),
            lane_genesis_hash(&network, &b)
        );
        assert_ne!(
            lane_instance(&crypto, &network, "chain", &a),
            lane_instance(&crypto, &network, "chain", &b)
        );
        // Closing and merging do not change the genesis result.
        let later = SumeragiLaneRecord {
            closing: Some(40),
            merged: SumeragiLaneFrontier {
                height: 3,
                block_hash: [1; 32],
                result: [2; 32],
            },
            ..a.clone()
        };
        assert_eq!(lane_genesis_result(&a), lane_genesis_result(&later));
        assert_ne!(lane_genesis_result(&a), lane_genesis_result(&b));
    }

    #[test]
    fn the_pinned_committee_forms_the_configuration() {
        let config = lane_height_config(&record(None)).expect("config");
        assert_eq!(config.committee.n(), 4);
        let empty = SumeragiLaneRecord {
            committee: Vec::new(),
            ..record(None)
        };
        assert!(lane_height_config(&empty).is_err());
    }

    #[test]
    fn admission_checks_anchor_activity_and_duplicates() {
        let record = record(Some(9));
        let config = lane_height_config(&record).expect("config");
        let unmerged_tx = tx(50);
        let chain = LaneChainView {
            previous_anchor: 7,
            unmerged: BTreeSet::from([unmerged_tx.hash_as_entrypoint()]),
        };
        let admit = |payload: &[u8], chain: &LaneChainView| {
            admit(&record, &anchors(), chain, &AcceptAll, &config, payload)
        };
        let batch = payload(8, vec![tx(1), tx(2)]);
        // Valid at anchor 8.
        let valid = admit(&batch, &chain).expect("valid");
        let Admission::Valid(result) = valid else {
            panic!("expected a verdict");
        };
        assert_eq!(result.tx_hashes.len(), 2);
        assert_eq!(result.anchor_height, 8);
        // Deterministic: the same inputs give the same R.
        let again = admit(&batch, &chain).expect("valid");
        assert_eq!(Admission::Valid(result.clone()), again);
        // An anchor this node has not applied: pending, not a verdict.
        assert_eq!(
            admit(&payload(11, vec![tx(1)]), &chain),
            Ok(Admission::Pending)
        );
        // A wrong anchor hash, a regressing anchor, an inactive anchor.
        let wrong = LaneBatch {
            anchor_height: 8,
            anchor_hash: block_hash(99),
            transactions: vec![tx(1)],
        }
        .to_payload();
        assert!(matches!(
            admit(&wrong, &chain),
            Err(AdmissionError::AnchorMismatch { .. })
        ));
        assert!(matches!(
            admit(&payload(6, vec![tx(1)]), &LaneChainView::default()),
            Err(AdmissionError::Inactive(6))
        ));
        assert!(matches!(
            admit(&payload(9, vec![tx(1)]), &chain),
            Err(AdmissionError::Inactive(9))
        ));
        let regressed = LaneChainView {
            previous_anchor: 8,
            ..chain.clone()
        };
        assert!(matches!(
            admit(&payload(7, vec![tx(1)]), &regressed),
            Err(AdmissionError::AnchorRegressed { .. })
        ));
        // Duplicates within the block and against unmerged lane blocks.
        let repeated = tx(1);
        assert_eq!(
            admit(&payload(8, vec![repeated.clone(), repeated]), &chain),
            Err(AdmissionError::Duplicate(1))
        );
        assert_eq!(
            admit(&payload(8, vec![unmerged_tx.clone()]), &chain),
            Err(AdmissionError::Duplicate(0))
        );
        // A non-canonical payload.
        let mut bytes = payload(8, vec![tx(1)]);
        bytes.push(0);
        assert!(matches!(
            admit(&bytes, &chain),
            Err(AdmissionError::Encoding(_))
        ));
    }
}
