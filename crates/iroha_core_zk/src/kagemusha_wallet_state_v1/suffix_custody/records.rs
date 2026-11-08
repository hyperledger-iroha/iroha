//! Additive bounded records selected only by the suffix checkpoint namespace.
use super::*;

#[derive(Debug, Clone, norito::Encode, norito::Decode, norito::NoritoSchema)]
#[norito_schema(
    name = "iroha_core_zk::kagemusha_wallet_suffix_custody_v1::SuffixArchiveCheckpointV1"
)]
pub(in crate::kagemusha_wallet_state_v1) struct SuffixArchiveCheckpointV1 {
    pub version: u16,
    pub discriminator: u16,
    pub scheme_id: [u8; 32],
    pub wallet_id: [u8; 32],
    pub manifest_original: [u8; 32],
    pub suffix_manifest: [u8; 32],
}
impl SuffixArchiveCheckpointV1 {
    pub(in crate::kagemusha_wallet_state_v1) fn require(
        &self,
        binding: ([u8; 32], [u8; 32]),
    ) -> Result<(), Error> {
        if self.version != 1
            || self.discriminator != 1
            || (self.scheme_id, self.wallet_id) != binding
        {
            return Err(lost("suffix record binding"));
        }
        if self.manifest_original == [0; 32] {
            return Err(lost("suffix record manifest_original"));
        }
        if self.suffix_manifest == [0; 32] {
            return Err(lost("suffix record suffix_manifest"));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, norito::Encode, norito::Decode, norito::NoritoSchema)]
#[norito_schema(
    name = "iroha_core_zk::kagemusha_wallet_suffix_custody_v1::SuffixArchiveManifestV1"
)]
pub(in crate::kagemusha_wallet_state_v1) struct SuffixArchiveManifestV1 {
    pub version: u16,
    pub discriminator: u16,
    pub scheme_id: [u8; 32],
    pub wallet_id: [u8; 32],
    pub requests: IndexRoot,
    pub authorities: IndexRoot,
    pub operations: IndexRoot,
    pub source_pins: IndexRoot,
    pub ledger_attempts: IndexRoot,
    pub ledger_outcomes: IndexRoot,
    /// Request/ordinal key to the bounded per-stage attempts record.
    pub ledger_stages: IndexRoot,
    pub active_request: Option<[u8; 32]>,
}
impl SuffixArchiveManifestV1 {
    pub(in crate::kagemusha_wallet_state_v1) fn require(
        &self,
        binding: ([u8; 32], [u8; 32]),
    ) -> Result<(), Error> {
        if self.version != 1
            || self.discriminator != 2
            || (self.scheme_id, self.wallet_id) != binding
        {
            return Err(lost("suffix record binding"));
        }
        if self.active_request == Some([0; 32]) {
            return Err(lost("suffix record active_request"));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, norito::Encode, norito::Decode, norito::NoritoSchema)]
#[repr(align(16))]
#[norito_schema(name = "iroha_core_zk::kagemusha_wallet_suffix_custody_v1::SuffixRequestRecordV1")]
pub(in crate::kagemusha_wallet_state_v1) struct SuffixRequestRecordV1 {
    pub version: u16,
    pub discriminator: u16,
    pub scheme_id: [u8; 32],
    pub wallet_id: [u8; 32],
    pub request_id: [u8; 32],
    pub request_original: [u8; 32],
    pub source_checkpoint: [u8; 32],
    pub source_capsule: [u8; 32],
    pub source_operation: [u8; 32],
    pub source_sequence: u128,
    pub source_selected_generation: u128,
    pub source_custody: [u8; 32],
    pub anchor_original: [u8; 32],
    pub anchor_credit_tree: [u8; 32],
    pub anchor_pending: [u8; 32],
    pub anchor_sequence: u128,
    pub pins: IndexRoot,
    pub progress: Option<[u8; 32]>,
    pub selection: Option<[u8; 32]>,
    pub released: Option<[u8; 32]>,
}
impl SuffixRequestRecordV1 {
    pub(in crate::kagemusha_wallet_state_v1) fn require(
        &self,
        binding: ([u8; 32], [u8; 32]),
    ) -> Result<(), Error> {
        if self.version != 1
            || self.discriminator != 3
            || (self.scheme_id, self.wallet_id) != binding
        {
            return Err(lost("suffix record binding"));
        }
        if self.request_id == [0; 32] {
            return Err(lost("suffix record request_id"));
        }
        if self.request_original == [0; 32] {
            return Err(lost("suffix record request_original"));
        }
        if self.source_checkpoint == [0; 32] {
            return Err(lost("suffix record source_checkpoint"));
        }
        if self.source_capsule == [0; 32] {
            return Err(lost("suffix record source_capsule"));
        }
        if self.source_operation == [0; 32] {
            return Err(lost("suffix record source_operation"));
        }
        if self.progress == Some([0; 32]) {
            return Err(lost("suffix record progress"));
        }
        if self.selection == Some([0; 32]) {
            return Err(lost("suffix record selection"));
        }
        if self.released == Some([0; 32]) {
            return Err(lost("suffix record released"));
        }
        if self.source_selected_generation == 0 {
            return Err(lost("suffix source generation"));
        }
        if [
            self.source_custody,
            self.anchor_original,
            self.anchor_credit_tree,
            self.anchor_pending,
        ]
        .contains(&[0; 32])
        {
            return Err(lost("suffix request selected source originals"));
        }
        if self
            .source_sequence
            .checked_sub(self.anchor_sequence)
            .is_none_or(|steps| steps == 0 || steps > 4096)
            || self.released.is_some() && self.selection.is_none()
        {
            return Err(lost("suffix request phase"));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, norito::Encode, norito::Decode, norito::NoritoSchema)]
#[repr(align(16))]
#[norito_schema(
    name = "iroha_core_zk::kagemusha_wallet_suffix_custody_v1::SuffixPublicationSelectionV1"
)]
pub(in crate::kagemusha_wallet_state_v1) struct SuffixPublicationSelectionV1 {
    pub version: u16,
    pub discriminator: u16,
    pub scheme_id: [u8; 32],
    pub wallet_id: [u8; 32],
    pub request_id: [u8; 32],
    pub authority: [u8; 32],
    pub source_checkpoint: [u8; 32],
    pub source_capsule: [u8; 32],
    pub source_operation: [u8; 32],
    pub source_sequence: u128,
    pub anchor_original: [u8; 32],
    pub header_original: [u8; 32],
    /// Exact ordinal-zero instruction, available before the hardware debit.
    pub begin_instruction_original: [u8; 32],
    pub chunk_originals: IndexRoot,
    pub chunk_count: u128,
    pub stream_bytes: u128,
    pub step_count: u128,
    pub pins: IndexRoot,
    pub native_choices: [u8; 32],
    pub frozen_capsule: [u8; 32],
    pub successor_custody: [u8; 32],
    pub operation_id: [u8; 32],
}
impl SuffixPublicationSelectionV1 {
    pub(in crate::kagemusha_wallet_state_v1) fn require(
        &self,
        binding: ([u8; 32], [u8; 32]),
    ) -> Result<(), Error> {
        if self.version != 1
            || self.discriminator != 5
            || (self.scheme_id, self.wallet_id) != binding
        {
            return Err(lost("suffix record binding"));
        }
        if self.request_id == [0; 32] {
            return Err(lost("suffix record request_id"));
        }
        if self.authority == [0; 32] {
            return Err(lost("suffix record authority"));
        }
        if self.source_checkpoint == [0; 32] {
            return Err(lost("suffix record source_checkpoint"));
        }
        if self.source_capsule == [0; 32] {
            return Err(lost("suffix record source_capsule"));
        }
        if self.source_operation == [0; 32] {
            return Err(lost("suffix record source_operation"));
        }
        if self.anchor_original == [0; 32] {
            return Err(lost("suffix record anchor_original"));
        }
        if self.header_original == [0; 32] {
            return Err(lost("suffix record header_original"));
        }
        if self.begin_instruction_original == [0; 32] {
            return Err(lost("suffix record begin_instruction_original"));
        }
        if self.native_choices == [0; 32] {
            return Err(lost("suffix record native_choices"));
        }
        if self.successor_custody == [0; 32] {
            return Err(lost("suffix selection successor custody"));
        }
        if self.frozen_capsule == [0; 32] {
            return Err(lost("suffix record frozen_capsule"));
        }
        if self.operation_id == [0; 32] {
            return Err(lost("suffix record operation_id"));
        }
        if self.chunk_count == 0
            || self.chunk_count > 512
            || self.step_count < self.chunk_count
            || self.step_count > self.chunk_count * 8
            || self.stream_bytes == 0
        {
            return Err(lost("suffix selection counts"));
        }
        Ok(())
    }
}

/// The retained attempts for one immutable stage, in local intake order. The first
/// accepted attempt is pinned here so Advance reads Begin finality in bounded work.
/// The per-entrypoint outcome remains the authority; this record is only its index.
#[derive(Debug, Clone, norito::Encode, norito::Decode, norito::NoritoSchema)]
#[repr(align(16))]
#[norito_schema(name = "iroha_core_zk::kagemusha_wallet_suffix_custody_v1::SuffixStageAttemptsV1")]
pub(in crate::kagemusha_wallet_state_v1) struct SuffixStageAttemptsV1 {
    pub version: u16,
    pub discriminator: u16,
    pub scheme_id: [u8; 32],
    pub wallet_id: [u8; 32],
    pub request_id: [u8; 32],
    pub authority: [u8; 32],
    pub instruction_ordinal: u128,
    pub instruction_original: [u8; 32],
    pub attempts: IndexRoot,
    pub attempt_count: u64,
    pub accepted_entrypoint: Option<[u8; 32]>,
}
impl SuffixStageAttemptsV1 {
    pub(in crate::kagemusha_wallet_state_v1) fn require(
        &self,
        binding: ([u8; 32], [u8; 32]),
    ) -> Result<(), Error> {
        if self.version != 1
            || self.discriminator != 17
            || (self.scheme_id, self.wallet_id) != binding
            || self.request_id == [0; 32]
            || self.authority == [0; 32]
            || self.instruction_original == [0; 32]
            || self.instruction_ordinal > 513
            || (self.attempt_count == 0) != (self.attempts == IndexRoot::default())
            || self.accepted_entrypoint == Some([0; 32])
            || self.accepted_entrypoint.is_some() && self.attempt_count == 0
        {
            return Err(lost("suffix stage attempts binding"));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, norito::Encode, norito::Decode, norito::NoritoSchema)]
#[repr(align(16))]
#[norito_schema(
    name = "iroha_core_zk::kagemusha_wallet_suffix_custody_v1::SuffixReleasedOriginalV1"
)]
pub(in crate::kagemusha_wallet_state_v1) struct SuffixReleasedOriginalV1 {
    pub version: u16,
    pub discriminator: u16,
    pub scheme_id: [u8; 32],
    pub wallet_id: [u8; 32],
    pub request_id: [u8; 32],
    pub authority: [u8; 32],
    pub selection: [u8; 32],
    pub operation_id: [u8; 32],
    pub selected_generation: u128,
    pub capsule_original: [u8; 32],
    pub capsule_custody_digest: [u8; 32],
    pub completion_original: [u8; 32],
    pub completion_digest: [u8; 32],
    pub receipt_original: [u8; 32],
    pub finalize_original: [u8; 32],
    pub instruction_originals: IndexRoot,
    pub instruction_count: u128,
}
impl SuffixReleasedOriginalV1 {
    pub(in crate::kagemusha_wallet_state_v1) fn require(
        &self,
        binding: ([u8; 32], [u8; 32]),
    ) -> Result<(), Error> {
        if self.version != 1
            || self.discriminator != 6
            || (self.scheme_id, self.wallet_id) != binding
        {
            return Err(lost("suffix record binding"));
        }
        if self.request_id == [0; 32] {
            return Err(lost("suffix record request_id"));
        }
        if self.authority == [0; 32] {
            return Err(lost("suffix record authority"));
        }
        if self.selection == [0; 32] {
            return Err(lost("suffix record selection"));
        }
        if self.operation_id == [0; 32] {
            return Err(lost("suffix record operation_id"));
        }
        if self.capsule_original == [0; 32] {
            return Err(lost("suffix record capsule_original"));
        }
        if self.capsule_custody_digest == [0; 32] {
            return Err(lost("suffix record capsule_custody_digest"));
        }
        if self.completion_original == [0; 32] {
            return Err(lost("suffix record completion_original"));
        }
        if self.completion_digest == [0; 32] {
            return Err(lost("suffix record completion_digest"));
        }
        if self.receipt_original == [0; 32] {
            return Err(lost("suffix record receipt_original"));
        }
        if self.finalize_original == [0; 32] {
            return Err(lost("suffix record finalize_original"));
        }
        if !(3..=514).contains(&self.instruction_count) {
            return Err(lost("suffix released instruction count"));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, norito::Encode, norito::Decode, norito::NoritoSchema)]
#[repr(align(16))]
#[norito_schema(name = "iroha_core_zk::kagemusha_wallet_suffix_custody_v1::SuffixLedgerAttemptV1")]
pub(in crate::kagemusha_wallet_state_v1) struct SuffixLedgerAttemptV1 {
    pub version: u16,
    pub discriminator: u16,
    pub scheme_id: [u8; 32],
    pub wallet_id: [u8; 32],
    pub request_id: [u8; 32],
    pub authority: [u8; 32],
    pub instruction_ordinal: u128,
    pub instruction_original: [u8; 32],
    pub signed_wire_original: [u8; 32],
    pub signed_wire_length: u32,
    pub entrypoint: [u8; 32],
}
impl SuffixLedgerAttemptV1 {
    pub(in crate::kagemusha_wallet_state_v1) fn require(
        &self,
        binding: ([u8; 32], [u8; 32]),
    ) -> Result<(), Error> {
        if self.version != 1
            || self.discriminator != 7
            || (self.scheme_id, self.wallet_id) != binding
        {
            return Err(lost("suffix record binding"));
        }
        if self.request_id == [0; 32] {
            return Err(lost("suffix record request_id"));
        }
        if self.authority == [0; 32] {
            return Err(lost("suffix record authority"));
        }
        if self.instruction_original == [0; 32] {
            return Err(lost("suffix record instruction_original"));
        }
        if self.signed_wire_original == [0; 32] {
            return Err(lost("suffix record signed_wire_original"));
        }
        if self.entrypoint == [0; 32] {
            return Err(lost("suffix record entrypoint"));
        }
        if self.instruction_ordinal > 513
            || self.signed_wire_length == 0
            || self.signed_wire_length > 593_952
        {
            return Err(lost("suffix attempt bounds"));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, norito::Encode, norito::Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_core_zk::kagemusha_wallet_suffix_custody_v1::SuffixLedgerOutcomeV1")]
pub(in crate::kagemusha_wallet_state_v1) struct SuffixLedgerOutcomeV1 {
    pub version: u16,
    pub discriminator: u16,
    pub scheme_id: [u8; 32],
    pub wallet_id: [u8; 32],
    pub request_id: [u8; 32],
    pub authority: [u8; 32],
    pub attempt: [u8; 32],
    pub entrypoint: [u8; 32],
    pub accepted: bool,
    pub certified_height: u64,
    pub native_commit_hash: [u8; 32],
    pub finality_original: [u8; 32],
}
impl SuffixLedgerOutcomeV1 {
    pub(in crate::kagemusha_wallet_state_v1) fn require(
        &self,
        binding: ([u8; 32], [u8; 32]),
    ) -> Result<(), Error> {
        if self.version != 1
            || self.discriminator != 8
            || (self.scheme_id, self.wallet_id) != binding
        {
            return Err(lost("suffix record binding"));
        }
        if self.request_id == [0; 32] {
            return Err(lost("suffix record request_id"));
        }
        if self.authority == [0; 32] {
            return Err(lost("suffix record authority"));
        }
        if self.attempt == [0; 32] {
            return Err(lost("suffix record attempt"));
        }
        if self.entrypoint == [0; 32] {
            return Err(lost("suffix record entrypoint"));
        }
        if self.native_commit_hash == [0; 32] {
            return Err(lost("suffix record native_commit_hash"));
        }
        if self.finality_original == [0; 32] {
            return Err(lost("suffix record finality_original"));
        }
        if self.certified_height < 2 {
            return Err(lost("suffix outcome height"));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, norito::Encode, norito::Decode, norito::NoritoSchema)]
#[repr(align(16))]
#[norito_schema(
    name = "iroha_core_zk::kagemusha_wallet_suffix_custody_v1::SuffixPreparationProgressV1"
)]
pub(in crate::kagemusha_wallet_state_v1) struct SuffixPreparationProgressV1 {
    pub version: u16,
    pub discriminator: u16,
    pub scheme_id: [u8; 32],
    pub wallet_id: [u8; 32],
    pub request_id: [u8; 32],
    pub source_checkpoint: [u8; 32],
    pub pinned_steps: u128,
    pub processed_steps: u128,
    pub emitted_chunks: u128,
    pub emitted_stream_bytes: u128,
    /// Domain-separated digest of each already verified canonical chunk.
    pub chunk_digests: IndexRoot,
    pub chunk_originals: IndexRoot,
    /// Independent credit descriptor after all pinned semantic steps.
    pub credit_tree: crate::kagemusha_wallet_state_v1::credit_tree::CreditTree,
    pub partial_step_original: Option<[u8; 32]>,
    pub partial_step_length: u32,
}
impl SuffixPreparationProgressV1 {
    pub(in crate::kagemusha_wallet_state_v1) fn require(
        &self,
        binding: ([u8; 32], [u8; 32]),
    ) -> Result<(), Error> {
        if self.version != 1
            || self.discriminator != 9
            || (self.scheme_id, self.wallet_id) != binding
        {
            return Err(lost("suffix record binding"));
        }
        if self.request_id == [0; 32] {
            return Err(lost("suffix record request_id"));
        }
        if self.source_checkpoint == [0; 32] {
            return Err(lost("suffix record source_checkpoint"));
        }
        if self.partial_step_original == Some([0; 32]) {
            return Err(lost("suffix record partial_step_original"));
        }
        if self.processed_steps > self.pinned_steps
            || self.pinned_steps > 4096
            || self.emitted_chunks > 512
            || self.partial_step_length > 65536
            || self.partial_step_original.is_some() != (self.partial_step_length > 0)
        {
            return Err(lost("suffix progress bounds"));
        }
        Ok(())
    }
}
