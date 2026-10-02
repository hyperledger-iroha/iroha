//! Original transaction-bound multisig records from one complete certified World cut.
//! Decoding grants no finality authority. The consumer independently verifies the native
//! certified prefix, authenticates the complete snapshot and proves both raw Vec originals.
use iroha_crypto::{Hash, HashOf};
use iroha_data_model::{
    account::AccountId, isi::InstructionBox, sumeragi_finality::WorldStateSnapshotV1,
};
use norito::derive::{JsonDeserialize, JsonSerialize, NoritoDeserialize, NoritoSerialize};

mod path;
pub use path::MULTISIG_EXECUTION_EVIDENCE_PATH_V1;
/// Reader ceiling for the full snapshot and two immutable stored records.
pub const MULTISIG_EXECUTION_EVIDENCE_MAX_BYTES_V1: usize = 16 * 1024 * 1024;
/// Reader ceiling for each original multisig record.
pub const MULTISIG_EXECUTION_RECORD_MAX_BYTES_V1: usize = 64 * 1024;

/// Data-only publisher response; the height and context select independently verified evidence.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    JsonSerialize,
    JsonDeserialize,
    NoritoSerialize,
    NoritoDeserialize,
    norito::NoritoSchema,
)]
#[norito(deny_unknown_fields, no_fast_from_json)]
#[norito_schema(
    name = "iroha_torii_shared::multisig_execution_evidence::MultisigExecutionEvidenceV1",
    frame = "iroha.torii.v1.multisig.execution-evidence.response"
)]
pub struct MultisigExecutionEvidenceV1 {
    /// Height of the sole native certified cut containing both originals.
    pub height: u64,
    /// Certified native block id at that height.
    pub context_id: Hash,
    /// Exact account selected by the signed approval instruction.
    pub multisig_account_id: AccountId,
    /// Exact external entrypoint identity that executed the proposal.
    pub entrypoint_hash: [u8; 32],
    /// Exact inner instruction vector hash carried by that approval.
    pub instructions_hash: HashOf<Vec<InstructionBox>>,
    /// Complete canonical World hash preimages at the certified cut.
    pub world_snapshot: WorldStateSnapshotV1,
    /// Original stored Vec bytes of MultisigApprovalOutcomeV1, not a projection.
    pub approval_outcome: Vec<u8>,
    /// Original stored Vec bytes of MultisigProposalTerminalExecutionStateV1.
    pub terminal_execution: Vec<u8>,
}
/// Exact bounded decoding. No authority follows from successful decoding.
/// # Errors
/// Empty, oversized, malformed or noncanonical response or unbounded records.
pub fn decode_unverified_multisig_execution_evidence_v1(
    bytes: &[u8],
) -> Result<MultisigExecutionEvidenceV1, norito::Error> {
    if bytes.is_empty() || bytes.len() > MULTISIG_EXECUTION_EVIDENCE_MAX_BYTES_V1 {
        return Err(norito::Error::Message(
            "multisig execution evidence exceeds reader bound".into(),
        ));
    }
    let value: MultisigExecutionEvidenceV1 = norito::decode_canonical_with_limits(
        bytes,
        norito::canonical_decode_limits(MULTISIG_EXECUTION_EVIDENCE_MAX_BYTES_V1),
    )?;
    if value.height < 2
        || [&value.approval_outcome, &value.terminal_execution]
            .iter()
            .any(|record| {
                record.is_empty() || record.len() > MULTISIG_EXECUTION_RECORD_MAX_BYTES_V1
            })
    {
        return Err(norito::Error::Message(
            "multisig execution record exceeds reader bound".into(),
        ));
    }
    Ok(value)
}
/// Borrowed encoder of the sole owned layout, without cloning the complete World graph.
#[derive(NoritoSerialize, JsonSerialize)]
pub struct MultisigExecutionEvidenceRefV1<'a> {
    height: FieldRef<'a, u64>,
    context_id: FieldRef<'a, Hash>,
    multisig_account_id: FieldRef<'a, AccountId>,
    entrypoint_hash: FieldRef<'a, [u8; 32]>,
    instructions_hash: FieldRef<'a, HashOf<Vec<InstructionBox>>>,
    world_snapshot: FieldRef<'a, WorldStateSnapshotV1>,
    approval_outcome: FieldRef<'a, Vec<u8>>,
    terminal_execution: FieldRef<'a, Vec<u8>>,
}
impl<'a> MultisigExecutionEvidenceRefV1<'a> {
    /// Borrow exact admitted originals for bounded serialization; no authority is granted.
    #[must_use]
    pub fn new(
        height: &'a u64,
        context_id: &'a Hash,
        multisig_account_id: &'a AccountId,
        entrypoint_hash: &'a [u8; 32],
        instructions_hash: &'a HashOf<Vec<InstructionBox>>,
        world_snapshot: &'a WorldStateSnapshotV1,
        approval_outcome: &'a Vec<u8>,
        terminal_execution: &'a Vec<u8>,
    ) -> Self {
        Self {
            height: FieldRef(height),
            context_id: FieldRef(context_id),
            multisig_account_id: FieldRef(multisig_account_id),
            entrypoint_hash: FieldRef(entrypoint_hash),
            instructions_hash: FieldRef(instructions_hash),
            world_snapshot: FieldRef(world_snapshot),
            approval_outcome: FieldRef(approval_outcome),
            terminal_execution: FieldRef(terminal_execution),
        }
    }
}
impl norito::NoritoSchema for MultisigExecutionEvidenceRefV1<'_> {
    fn nominal_name() -> String {
        <MultisigExecutionEvidenceV1 as norito::NoritoSchema>::nominal_name()
    }
    fn frame_name() -> String {
        <MultisigExecutionEvidenceV1 as norito::NoritoSchema>::frame_name()
    }
}
// Payload-only forwarding preserves each field's exact codec and bounded JSON writer.
struct FieldRef<'a, T>(&'a T);
impl<T: norito::core::SerializePayload> norito::core::SerializePayload for FieldRef<'_, T> {
    fn serialize(&self, out: &mut norito::core::Encoder<'_>) -> Result<(), norito::Error> {
        norito::core::SerializePayload::serialize(self.0, out)
    }
    fn encoded_len_hint(&self) -> Option<usize> {
        norito::core::SerializePayload::encoded_len_hint(self.0)
    }
    fn encoded_len_exact(&self) -> Option<usize> {
        norito::core::SerializePayload::encoded_len_exact(self.0)
    }
}
impl<T: norito::json::JsonSerialize> norito::json::JsonSerialize for FieldRef<'_, T> {
    fn json_serialize(&self, out: &mut String) {
        self.0.json_serialize(out);
    }
    fn json_serialize_to(
        &self,
        out: &mut dyn norito::json::JsonWriteSink,
    ) -> Result<(), norito::json::BoundedJsonError> {
        self.0.json_serialize_to(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use iroha_crypto::{Algorithm, KeyPair};

    fn unverified_data() -> MultisigExecutionEvidenceV1 {
        // Explicit format-only specimen. No native execution or monetary authority is asserted.
        MultisigExecutionEvidenceV1 {
            height: 2,
            context_id: Hash::new(b"format-only certified-id specimen"),
            multisig_account_id: AccountId::new(
                KeyPair::from_seed(vec![19; 32], Algorithm::Ed25519)
                    .public_key()
                    .clone(),
            ),
            entrypoint_hash: *Hash::new(b"format-only input specimen").as_ref(),
            instructions_hash: HashOf::new(&Vec::<InstructionBox>::new()),
            world_snapshot: WorldStateSnapshotV1 {
                schema_hash: Hash::new(b"format-only registry specimen"),
                entries: Vec::new(),
            },
            approval_outcome: vec![1, 2, 3],
            terminal_execution: vec![4, 5, 6],
        }
    }
    #[test]
    fn sole_borrowed_layout_matches_owned_native_frame_and_rejects_trailing_bytes() {
        let data = unverified_data();
        let borrowed = MultisigExecutionEvidenceRefV1::new(
            &data.height,
            &data.context_id,
            &data.multisig_account_id,
            &data.entrypoint_hash,
            &data.instructions_hash,
            &data.world_snapshot,
            &data.approval_outcome,
            &data.terminal_execution,
        );
        let bytes = norito::encode_canonical(&data).unwrap();
        assert_eq!(norito::encode_canonical(&borrowed).unwrap(), bytes);
        assert_eq!(
            norito::json::to_json(&borrowed).unwrap(),
            norito::json::to_json(&data).unwrap()
        );
        assert_eq!(
            decode_unverified_multisig_execution_evidence_v1(&bytes).unwrap(),
            data
        );
        let mut trailing = bytes;
        trailing.push(0);
        assert!(decode_unverified_multisig_execution_evidence_v1(&trailing).is_err());
    }
    #[test]
    fn native_reader_refuses_empty_genesis_and_oversized_original_records() {
        assert!(decode_unverified_multisig_execution_evidence_v1(&[]).is_err());
        let mut data = unverified_data();
        data.height = 1;
        assert!(
            decode_unverified_multisig_execution_evidence_v1(
                &norito::encode_canonical(&data).unwrap()
            )
            .is_err()
        );
        data.height = 2;
        data.approval_outcome.clear();
        assert!(
            decode_unverified_multisig_execution_evidence_v1(
                &norito::encode_canonical(&data).unwrap()
            )
            .is_err()
        );
        data.approval_outcome = vec![0; MULTISIG_EXECUTION_RECORD_MAX_BYTES_V1 + 1];
        assert!(
            decode_unverified_multisig_execution_evidence_v1(
                &norito::encode_canonical(&data).unwrap()
            )
            .is_err()
        );
    }
}
