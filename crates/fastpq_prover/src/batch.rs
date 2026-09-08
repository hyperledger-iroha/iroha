use norito::{NoritoDeserialize, NoritoSerialize};
use std::borrow::Cow;
use std::collections::BTreeMap;
/// Canonical Norito root-frame identity for [`TransitionBatch`].
///
/// An explicit name keeps the release wire format independent of Cargo features
/// and Rust module refactors.
pub const TRANSITION_BATCH_SCHEMA_NAME: &str = "fastpq_prover::batch::FastpqStateTransitionBatchV1";
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
    PartialEq,
    Eq,
    NoritoSerialize,
    NoritoDeserialize,
    norito::derive::JsonSerialize,
    norito::derive::JsonDeserialize,
    norito::NoritoSchema,
)]
#[norito_schema(name = "fastpq_prover::batch::StateTransition")]
pub struct StateTransition {
    /// Schema-qualified logical key (e.g., account/asset path) encoded as bytes.
    pub key: Vec<u8>,
    /// Optional pre-state value; empty when a key is freshly created.
    pub pre_value: Vec<u8>,
    /// Optional post-state value; empty when a key is removed.
    pub post_value: Vec<u8>,
    /// Operation selector driving the AIR row semantics.
    pub operation: OperationKind,
}
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
    Copy,
    PartialEq,
    Eq,
    NoritoSerialize,
    NoritoDeserialize,
    norito::derive::JsonSerialize,
    norito::derive::JsonDeserialize,
)]
#[norito(tag = "kind", content = "payload")]
pub enum OperationKind {
    // The final V1 block starts at 32 so both the experimental 0..=5 wire and
    // the superseded two-operation 16/17 wire fail decoding.
    /// Asset transfer between two existing accounts.
    #[codec(index = 32)]
    Transfer,
    /// Asset mint increasing the committed circulating supply.
    #[codec(index = 33)]
    Mint,
    /// Asset burn decreasing the committed circulating supply.
    #[codec(index = 34)]
    Burn,
    /// Grant one exact permission to a role at the bound epoch.
    #[codec(index = 35)]
    RoleGrant {
        /// Canonical role identifier bytes.
        role_id: [u8; 32],
        /// Canonical permission identifier bytes.
        permission_id: [u8; 32],
        /// Epoch at which the grant becomes effective.
        epoch: u64,
    },
    /// Revoke one exact permission from a role at the bound epoch.
    #[codec(index = 36)]
    RoleRevoke {
        /// Canonical role identifier bytes.
        role_id: [u8; 32],
        /// Canonical permission identifier bytes.
        permission_id: [u8; 32],
        /// Epoch at which the revocation becomes effective.
        epoch: u64,
    },
    /// Opaque metadata effect whose meaning is authenticated by its outer statement.
    #[codec(index = 37)]
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

    /// Return whether this row participates in the permission-tree relation.
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
    norito::NoritoSchema,
)]
#[norito_schema(
    name = "fastpq_prover::batch::TransitionBatch",
    frame = "fastpq_prover::batch::FastpqStateTransitionBatchV1"
)]
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
    pub fn push(&mut self, transition: StateTransition) {
        self.transitions.push(transition);
    }
    /// Normalise transitions by sorting on keys to achieve deterministic encoding.
    pub fn sort(&mut self) {
        // `slice::sort_by` is stable, so rows with the same key and operation
        // retain their input order without carrying a separate local ordinal.
        self.transitions.sort_by(|lhs, rhs| {
            lhs.key
                .cmp(&rhs.key)
                .then_with(|| lhs.operation_rank().cmp(&rhs.operation_rank()))
        });
    }

    /// Borrow this batch when it is already canonical, otherwise return one
    /// sorted clone. This lets the prover canonicalise once and reuse the same
    /// batch across every commitment stage.
    pub(crate) fn canonicalized(&self) -> Cow<'_, Self> {
        if self.transitions.windows(2).all(|pair| {
            let [lhs, rhs] = pair else {
                unreachable!("windows(2) always contains two entries")
            };
            lhs.key < rhs.key
                || (lhs.key == rhs.key && lhs.operation_rank() <= rhs.operation_rank())
        }) {
            Cow::Borrowed(self)
        } else {
            let mut canonical = self.clone();
            canonical.sort();
            Cow::Owned(canonical)
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use norito::codec::{Decode, Encode};

    #[test]
    fn operation_wire_indices_reject_the_pre_release_enum() {
        assert_eq!(OperationKind::Transfer.encode(), 32_u32.to_le_bytes());
        assert_eq!(OperationKind::Mint.encode(), 33_u32.to_le_bytes());
        assert_eq!(OperationKind::Burn.encode(), 34_u32.to_le_bytes());
        let role_id = [0x11; 32];
        let permission_id = [0x22; 32];
        let grant = OperationKind::RoleGrant {
            role_id,
            permission_id,
            epoch: 9,
        }
        .encode();
        let revoke = OperationKind::RoleRevoke {
            role_id,
            permission_id,
            epoch: 10,
        }
        .encode();
        assert_eq!(&grant[..4], 35_u32.to_le_bytes().as_slice());
        assert_eq!(&revoke[..4], 36_u32.to_le_bytes().as_slice());
        assert_eq!(OperationKind::MetaSet.encode(), 37_u32.to_le_bytes());
        assert_eq!(
            OperationKind::decode(&mut grant.as_slice()).expect("decode role grant"),
            OperationKind::RoleGrant {
                role_id,
                permission_id,
                epoch: 9,
            }
        );
        assert_eq!(
            OperationKind::decode(&mut revoke.as_slice()).expect("decode role revoke"),
            OperationKind::RoleRevoke {
                role_id,
                permission_id,
                epoch: 10,
            }
        );
        for retired in 0_u32..32 {
            assert!(
                OperationKind::decode(&mut retired.to_le_bytes().as_slice()).is_err(),
                "retired pre-release operation index {retired} must not decode"
            );
        }
    }

    #[test]
    fn transition_batch_schema_identity_is_stable() {
        let expected = norito::core::schema_hash_for_name(TRANSITION_BATCH_SCHEMA_NAME);
        assert_eq!(
            norito::schema::identity::frame_hash::<TransitionBatch>(),
            expected
        );
        assert_eq!(
            <TransitionBatch as norito::NoritoSchema>::frame_name(),
            TRANSITION_BATCH_SCHEMA_NAME
        );
        assert_eq!(
            expected,
            [
                0xd2, 0x0b, 0xd9, 0x47, 0x90, 0x6a, 0xec, 0x99, 0xda, 0x9b, 0x2c, 0x49, 0x33, 0x46,
                0xb3, 0xa2,
            ]
        );

        let batch =
            TransitionBatch::new("fastpq-state-transition-stark-v1", PublicInputs::default());
        let encoded = norito::core::to_bytes(&batch).expect("encode release batch");
        assert_eq!(&encoded[6..22], expected.as_slice());
        for retired_name in [
            "fastpq_prover::batch::TransitionBatch",
            "fastpq_prover::batch::TransitionBatchV1",
        ] {
            let mut retired = encoded.clone();
            let retired_schema = norito::core::schema_hash_for_name(retired_name);
            retired[6..22].copy_from_slice(&retired_schema);
            assert!(
                norito::decode_from_bytes::<TransitionBatch>(&retired).is_err(),
                "retired batch schema {retired_name} must not decode as final V1"
            );
        }
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
            OperationKind::Burn,
        ));
        batch.push(StateTransition::new(
            b"key".to_vec(),
            vec![1],
            vec![2],
            OperationKind::Transfer,
        ));
        batch.push(StateTransition::new(
            b"key".to_vec(),
            vec![3],
            vec![4],
            OperationKind::Mint,
        ));
        batch.push(StateTransition::new(
            b"key".to_vec(),
            vec![],
            vec![4],
            OperationKind::RoleGrant {
                role_id: [0x11; 32],
                permission_id: [0x22; 32],
                epoch: 7,
            },
        ));
        batch.push(StateTransition::new(
            b"key".to_vec(),
            vec![4],
            vec![],
            OperationKind::RoleRevoke {
                role_id: [0x11; 32],
                permission_id: [0x22; 32],
                epoch: 8,
            },
        ));
        batch.push(StateTransition::new(
            b"key".to_vec(),
            vec![4],
            vec![5],
            OperationKind::MetaSet,
        ));
        batch.sort();
        let ranks: Vec<_> = batch
            .transitions
            .iter()
            .map(StateTransition::operation_rank)
            .collect();
        assert_eq!(ranks, vec![0, 1, 2, 3, 4, 5]);
    }
    #[test]
    fn stable_sort_and_norito_roundtrip_preserve_equal_row_order() {
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
        let encoded = norito::to_bytes(&batch).expect("encode transition batch");
        let decoded = norito::decode_from_bytes::<TransitionBatch>(&encoded)
            .expect("decode transition batch");
        assert_eq!(decoded, batch);
    }

    #[test]
    fn canonicalized_borrows_sorted_batches_and_owns_unsorted_batches() {
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

        assert!(matches!(batch.canonicalized(), Cow::Owned(_)));
        batch.sort();
        assert!(matches!(batch.canonicalized(), Cow::Borrowed(_)));
    }
}
