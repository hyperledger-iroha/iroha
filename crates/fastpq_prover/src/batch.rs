use core::cmp::Ordering;
#[allow(unused_imports)]
use norito::json::{JsonDeserialize, JsonSerialize};
use norito::{NoritoDeserialize, NoritoSerialize};
use std::collections::BTreeMap;
/// Stable nominal Norito schema identity for [`TransitionBatch`].
///
/// This intentionally preserves the type-name hash carried by the existing canonical FASTPQ
/// fixtures while making it independent of Cargo features and Rust module refactors.
pub const TRANSITION_BATCH_SCHEMA_NAME: &str = "fastpq_prover::batch::TransitionBatch";
/// Public inputs supplied by the host for a FASTPQ batch.
#[derive(
    Debug,
    Copy,
    Clone,
    Default,
    PartialEq,
    Eq,
    NoritoSerialize,
    NoritoDeserialize,
    norito::derive::JsonSerialize,
    norito::derive::JsonDeserialize,
)]
pub struct PublicInputs {
    /// Data-space identifier (little-endian UUID bytes).
    pub dsid: [u8; 16],
    /// Slot timestamp (nanoseconds since epoch).
    pub slot: u64,
    /// Sparse Merkle tree root before executing the batch.
    pub old_root: [u8; 32],
    /// Sparse Merkle tree root after executing the batch.
    pub new_root: [u8; 32],
    /// Permission table commitment for this slot.
    pub perm_root: [u8; 32],
    /// Transaction set hash recorded by the scheduler.
    pub tx_set_hash: [u8; 32],
}
/// A single key-value transition touched by a transaction batch.
#[derive(
    Debug,
    Clone,
    NoritoSerialize,
    NoritoDeserialize,
    norito::derive::JsonSerialize,
    norito::derive::JsonDeserialize,
)]
pub struct StateTransition {
    /// Schema-qualified logical key (e.g., account/asset path) encoded as bytes.
    pub key: Vec<u8>,
    /// Optional pre-state value; empty when a key is freshly created.
    pub pre_value: Vec<u8>,
    /// Optional post-state value; empty when a key is removed.
    pub post_value: Vec<u8>,
    /// Operation selector driving the AIR row semantics.
    pub operation: OperationKind,
    /// Original insertion index used to preserve submission ordering during
    /// the canonical sort. Skipped from serialization to keep the Norito
    /// encoding stable irrespective of local batch construction.
    #[norito(skip)]
    pub(crate) ordinal: usize,
}
// `ordinal` is local sort state and is deliberately absent from canonical equality, matching its
// omission from Norito and JSON encodings.
impl PartialEq for StateTransition {
    fn eq(&self, other: &Self) -> bool {
        self.key == other.key
            && self.pre_value == other.pre_value
            && self.post_value == other.post_value
            && self.operation == other.operation
    }
}
impl Eq for StateTransition {}
impl StateTransition {
    /// Construct a new transition.
    pub fn new(
        key: Vec<u8>,
        pre_value: Vec<u8>,
        post_value: Vec<u8>,
        operation: OperationKind,
    ) -> Self {
        Self {
            key,
            pre_value,
            post_value,
            operation,
            ordinal: 0,
        }
    }
    /// Rank associated with the operation selector as defined by FASTPQ.
    #[inline]
    pub fn operation_rank(&self) -> u8 {
        self.operation.rank()
    }
}
/// FASTPQ selector describing the semantics of a transition row.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    NoritoSerialize,
    NoritoDeserialize,
    norito::derive::JsonSerialize,
    norito::derive::JsonDeserialize,
)]
#[norito(tag = "kind", content = "payload")]
pub enum OperationKind {
    /// Asset transfer between two existing accounts.
    Transfer,
    /// Asset mint increasing the circulating supply.
    Mint,
    /// Asset burn decreasing the circulating supply.
    Burn,
    /// Grant a permission to a role.
    RoleGrant {
        /// Canonical role identifier (little-endian bytes).
        role_id: Vec<u8>,
        /// Canonical permission identifier (little-endian bytes).
        permission_id: Vec<u8>,
        /// Epoch at which the change becomes effective (little-endian u64).
        epoch: u64,
    },
    /// Revoke a permission from a role.
    RoleRevoke {
        /// Canonical role identifier (little-endian bytes).
        role_id: Vec<u8>,
        /// Canonical permission identifier (little-endian bytes).
        permission_id: Vec<u8>,
        /// Epoch at which the change becomes effective (little-endian u64).
        epoch: u64,
    },
    /// Metadata mutation (domains, accounts, assets, etc.).
    MetaSet,
}
impl OperationKind {
    /// Selector rank used for deterministic ordering.
    #[inline]
    pub const fn rank(&self) -> u8 {
        match self {
            Self::Transfer => 0,
            Self::Mint => 1,
            Self::Burn => 2,
            Self::RoleGrant { .. } => 3,
            Self::RoleRevoke { .. } => 4,
            Self::MetaSet => 5,
        }
    }
    /// Returns true when the selector participates in the permission lookup
    /// grand-product (role grant/revoke).
    #[inline]
    pub const fn is_permission_selector(&self) -> bool {
        matches!(self, Self::RoleGrant { .. } | Self::RoleRevoke { .. })
    }
}
/// A batch of state transitions representing a single DS proof input.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    NoritoSerialize,
    NoritoDeserialize,
    norito::derive::JsonSerialize,
    norito::derive::JsonDeserialize,
)]
#[norito(schema_name = "fastpq_prover::batch::TransitionBatch")]
pub struct TransitionBatch {
    /// Canonical parameter set name expected for this proof.
    pub parameter: String,
    /// Public inputs committed by the prover and replayed by the verifier.
    pub public_inputs: PublicInputs,
    /// Deterministic, sorted transitions used to build the trace.
    pub transitions: Vec<StateTransition>,
    /// Optional metadata for higher-level schedulers (keyed map to keep the
    /// structure Norito-friendly without nested structs for now).
    pub metadata: BTreeMap<String, Vec<u8>>,
}
impl TransitionBatch {
    /// Create an empty batch for the given parameter set name.
    pub fn new(parameter: impl Into<String>, public_inputs: PublicInputs) -> Self {
        Self {
            parameter: parameter.into(),
            public_inputs,
            transitions: Vec::new(),
            metadata: BTreeMap::new(),
        }
    }
    /// Add a transition entry.
    pub fn push(&mut self, mut transition: StateTransition) {
        transition.ordinal = self.transitions.len();
        self.transitions.push(transition);
    }
    /// Normalise transitions by sorting on keys to achieve deterministic encoding.
    pub fn sort(&mut self) {
        for (idx, transition) in self.transitions.iter_mut().enumerate() {
            transition.ordinal = idx;
        }
        self.transitions
            .sort_by(|lhs, rhs| match lhs.key.cmp(&rhs.key) {
                Ordering::Equal => match lhs.operation_rank().cmp(&rhs.operation_rank()) {
                    Ordering::Equal => lhs.ordinal.cmp(&rhs.ordinal),
                    other => other,
                },
                other => other,
            });
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn transition_batch_schema_identity_is_stable() {
        let expected = norito::core::schema_hash_for_name(TRANSITION_BATCH_SCHEMA_NAME);
        assert_eq!(
            <TransitionBatch as NoritoSerialize>::schema_hash(),
            expected
        );
        assert_eq!(
            <TransitionBatch as NoritoDeserialize>::schema_hash(),
            expected
        );
        assert_eq!(
            expected,
            [
                0x51, 0x76, 0x0d, 0xe0, 0xa1, 0x40, 0x63, 0xa6, 0x83, 0xc3, 0x80, 0x1f, 0xe0, 0x79,
                0xd1, 0x1f,
            ]
        );
    }
    #[test]
    fn sort_orders_by_key() {
        let mut batch =
            TransitionBatch::new("fastpq-state-transition-stark-v1", PublicInputs::default());
        batch.push(StateTransition::new(
            b"b".to_vec(),
            vec![],
            vec![2],
            OperationKind::Transfer,
        ));
        batch.push(StateTransition::new(
            b"a".to_vec(),
            vec![],
            vec![1],
            OperationKind::Transfer,
        ));
        batch.sort();
        let ordered: Vec<_> = batch.transitions.iter().map(|t| t.key.clone()).collect();
        assert_eq!(ordered, vec![b"a".to_vec(), b"b".to_vec()]);
    }
    #[test]
    fn sort_respects_operation_rank() {
        let mut batch =
            TransitionBatch::new("fastpq-state-transition-stark-v1", PublicInputs::default());
        batch.push(StateTransition::new(
            b"key".to_vec(),
            vec![0],
            vec![1],
            OperationKind::Mint,
        ));
        batch.push(StateTransition::new(
            b"key".to_vec(),
            vec![1],
            vec![2],
            OperationKind::Transfer,
        ));
        batch.push(StateTransition::new(
            b"key".to_vec(),
            vec![2],
            vec![3],
            OperationKind::Burn,
        ));
        batch.sort();
        let ranks: Vec<_> = batch
            .transitions
            .iter()
            .map(StateTransition::operation_rank)
            .collect();
        assert_eq!(ranks, vec![0, 1, 2]);
    }
    #[test]
    fn sorted_batch_norito_roundtrip_ignores_local_ordinals() {
        let mut batch =
            TransitionBatch::new("fastpq-state-transition-stark-v1", PublicInputs::default());
        batch.push(StateTransition::new(
            b"b".to_vec(),
            vec![0],
            vec![1],
            OperationKind::Transfer,
        ));
        batch.push(StateTransition::new(
            b"a".to_vec(),
            vec![1],
            vec![2],
            OperationKind::Transfer,
        ));
        batch.push(StateTransition::new(
            b"a".to_vec(),
            vec![2],
            vec![3],
            OperationKind::Transfer,
        ));
        batch.sort();

        assert_eq!(
            batch
                .transitions
                .iter()
                .map(|transition| transition.pre_value.as_slice())
                .collect::<Vec<_>>(),
            vec![&[1_u8][..], &[2_u8][..], &[0_u8][..]],
            "equal key/operation rows must retain insertion order"
        );
        assert_eq!(
            batch
                .transitions
                .iter()
                .map(|transition| transition.ordinal)
                .collect::<Vec<_>>(),
            vec![1, 2, 0],
            "regression requires non-serialized local ordinals"
        );

        let encoded = norito::to_bytes(&batch).expect("encode transition batch");
        let decoded = norito::decode_from_bytes::<TransitionBatch>(&encoded)
            .expect("decode transition batch");
        assert_eq!(decoded, batch);
    }
}
