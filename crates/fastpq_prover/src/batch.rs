use norito::{NoritoDeserialize, NoritoSerialize};
use std::borrow::Cow;
use std::collections::BTreeMap;
/// Canonical nominal Norito schema identity for [`TransitionBatch`].
///
/// An explicit name keeps the release wire format independent of Cargo features
/// and Rust module refactors.
pub const TRANSITION_BATCH_SCHEMA_NAME: &str = "fastpq_prover::batch::TransitionBatchV1";
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
    Copy,
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
    #[codec(index = 16)]
    Transfer,
    /// Opaque metadata effect whose meaning is authenticated by its outer statement.
    #[codec(index = 17)]
    MetaSet,
}
impl OperationKind {
    /// Selector rank used for deterministic ordering.
    #[inline]
    pub const fn rank(&self) -> u8 {
        match self {
            Self::Transfer => 0,
            Self::MetaSet => 1,
        }
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
#[norito(schema_name = "fastpq_prover::batch::TransitionBatchV1")]
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
        assert_eq!(OperationKind::Transfer.encode(), 16_u32.to_le_bytes());
        assert_eq!(OperationKind::MetaSet.encode(), 17_u32.to_le_bytes());
        for retired in 0_u32..=5 {
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
                0xe0, 0x07, 0xd2, 0xe7, 0xbb, 0x2f, 0x1a, 0x08, 0xfe, 0x51, 0x81, 0x5a, 0x98, 0x9e,
                0x25, 0x2c,
            ]
        );

        let batch = TransitionBatch::new("fastpq-lane-balanced", PublicInputs::default());
        let mut encoded = norito::core::to_bytes(&batch).expect("encode release batch");
        assert_eq!(&encoded[6..22], expected.as_slice());
        let pre_release =
            norito::core::schema_hash_for_name("fastpq_prover::batch::TransitionBatch");
        encoded[6..22].copy_from_slice(&pre_release);
        assert!(
            norito::decode_from_bytes::<TransitionBatch>(&encoded).is_err(),
            "the pre-release batch schema must not decode as release V1"
        );
    }
    #[test]
    fn sort_orders_by_key() {
        let mut batch = TransitionBatch::new("fastpq-lane-balanced", PublicInputs::default());
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
        let mut batch = TransitionBatch::new("fastpq-lane-balanced", PublicInputs::default());
        batch.push(StateTransition::new(
            b"key".to_vec(),
            vec![0],
            vec![1],
            OperationKind::MetaSet,
        ));
        batch.push(StateTransition::new(
            b"key".to_vec(),
            vec![1],
            vec![2],
            OperationKind::Transfer,
        ));
        batch.sort();
        let ranks: Vec<_> = batch
            .transitions
            .iter()
            .map(StateTransition::operation_rank)
            .collect();
        assert_eq!(ranks, vec![0, 1]);
    }
    #[test]
    fn stable_sort_and_norito_roundtrip_preserve_equal_row_order() {
        let mut batch = TransitionBatch::new("fastpq-lane-balanced", PublicInputs::default());
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
        let mut batch = TransitionBatch::new("fastpq-lane-balanced", PublicInputs::default());
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
