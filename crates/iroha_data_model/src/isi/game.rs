//! Generic deterministic multiplayer sessions and reusable compiled execution-proof instructions.
use super::*;
isi! {
 /// Native OpenGameSessionV1 operation with complete authorization in Core.
 #[derive (crate :: DeriveJsonSerialize , crate :: DeriveJsonDeserialize)]
 #[norito (deny_unknown_fields)]
 #[norito_schema(name = "iroha_data_model::isi::game::OpenGameSessionV1")]
 pub struct OpenGameSessionV1 {
  /// Exact session id for this operation.
  pub session_id: iroha_crypto::Hash,
  /// Exact manifest for this operation.
  pub manifest: crate::game::GameManifestV1,
  /// Exact asset definition for this operation.
  pub asset_definition: crate::asset::AssetDefinitionId,
  /// Exact stake for this operation.
  pub stake: iroha_primitives::numeric::Quantity,
  /// Exact join deadline height for this operation.
  pub join_deadline_height: u64,
 }
}
impl crate::seal::Instruction for OpenGameSessionV1 {}
impl OpenGameSessionV1 {
    /// Construct a session with immutable application rules and an exact entry stake.
    #[must_use]
    pub fn new(
        session_id: iroha_crypto::Hash,
        manifest: crate::game::GameManifestV1,
        asset_definition: crate::asset::AssetDefinitionId,
        stake: iroha_primitives::numeric::Quantity,
        join_deadline_height: u64,
    ) -> Self {
        Self {
            session_id,
            manifest,
            asset_definition,
            stake,
            join_deadline_height,
        }
    }
}
impl<'a> norito::core::DecodeFromSlice<'a> for OpenGameSessionV1 {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        let flags = norito::core::effective_decode_flags()
            .unwrap_or_else(norito::core::default_encode_flags);
        if flags & norito::core::header_flags::PACKED_STRUCT != 0 {
            return super::decode_packed_instruction_payload::<Self>(bytes);
        }
        let mut offset = 0usize;
        let session_id = super::decode_aos_canonical_field::<iroha_crypto::Hash>(
            super::read_aos_field(bytes, &mut offset, flags)?,
            flags,
        )?;
        let manifest = super::decode_aos_canonical_field::<crate::game::GameManifestV1>(
            super::read_aos_field(bytes, &mut offset, flags)?,
            flags,
        )?;
        let asset_definition = super::decode_aos_canonical_field::<crate::asset::AssetDefinitionId>(
            super::read_aos_field(bytes, &mut offset, flags)?,
            flags,
        )?;
        let stake = super::decode_aos_canonical_field::<iroha_primitives::numeric::Quantity>(
            super::read_aos_field(bytes, &mut offset, flags)?,
            flags,
        )?;
        let join_deadline_height = super::decode_aos_canonical_field::<u64>(
            super::read_aos_field(bytes, &mut offset, flags)?,
            flags,
        )?;
        if offset != bytes.len() {
            return Err(norito::core::Error::LengthMismatch);
        }
        norito::core::note_payload_access(bytes, offset);
        Ok((
            Self {
                session_id,
                manifest,
                asset_definition,
                stake,
                join_deadline_height,
            },
            offset,
        ))
    }
}
isi! {
 /// Native JoinGameSessionV1 operation with complete authorization in Core.
 #[derive (crate :: DeriveJsonSerialize , crate :: DeriveJsonDeserialize)]
 #[norito (deny_unknown_fields)]
 #[norito_schema(name = "iroha_data_model::isi::game::JoinGameSessionV1")]
 pub struct JoinGameSessionV1 {
  /// Exact session id for this operation.
  pub session_id: iroha_crypto::Hash,
  /// Exact input key for this operation.
  pub input_key: iroha_crypto::PublicKey,
  /// Exact application data for this operation.
  pub application_data: Vec<u8>,
  /// Explicit temporary equipment authorization; empty is required for stock games.
  pub resources: Vec<crate::game::GameResourceReservationClauseV1>,
  /// Exact invitation for this operation.
  pub invitation: Option<iroha_crypto::Signature>,
  /// Wallet-approved immutable manifest; a different ledger manifest rejects entry.
  pub expected_manifest_hash: iroha_crypto::Hash,
  /// Wallet-approved asset definition for the exact entry debit.
  pub expected_asset_definition: crate::asset::AssetDefinitionId,
  /// Wallet-approved exact entry debit, including zero for a free session.
  pub expected_stake: iroha_primitives::numeric::Quantity,
 }
}
impl crate::seal::Instruction for JoinGameSessionV1 {}
impl JoinGameSessionV1 {
    /// Authorize an input key and exact immutable entry terms.
    #[must_use]
    pub fn new(
        session_id: iroha_crypto::Hash,
        input_key: iroha_crypto::PublicKey,
        application_data: Vec<u8>,
        resources: Vec<crate::game::GameResourceReservationClauseV1>,
        invitation: Option<iroha_crypto::Signature>,
        expected_manifest_hash: iroha_crypto::Hash,
        expected_asset_definition: crate::asset::AssetDefinitionId,
        expected_stake: iroha_primitives::numeric::Quantity,
    ) -> Self {
        Self {
            session_id,
            input_key,
            application_data,
            resources,
            invitation,
            expected_manifest_hash,
            expected_asset_definition,
            expected_stake,
        }
    }
}
impl<'a> norito::core::DecodeFromSlice<'a> for JoinGameSessionV1 {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        let flags = norito::core::effective_decode_flags()
            .unwrap_or_else(norito::core::default_encode_flags);
        if flags & norito::core::header_flags::PACKED_STRUCT != 0 {
            return super::decode_packed_instruction_payload::<Self>(bytes);
        }
        let mut offset = 0usize;
        let session_id = super::decode_aos_canonical_field::<iroha_crypto::Hash>(
            super::read_aos_field(bytes, &mut offset, flags)?,
            flags,
        )?;
        let input_key = super::decode_aos_canonical_field::<iroha_crypto::PublicKey>(
            super::read_aos_field(bytes, &mut offset, flags)?,
            flags,
        )?;
        let application_data = super::decode_aos_canonical_field::<Vec<u8>>(
            super::read_aos_field(bytes, &mut offset, flags)?,
            flags,
        )?;
        let resources = super::decode_aos_canonical_field::<
            Vec<crate::game::GameResourceReservationClauseV1>,
        >(super::read_aos_field(bytes, &mut offset, flags)?, flags)?;
        let invitation = super::decode_aos_canonical_field::<Option<iroha_crypto::Signature>>(
            super::read_aos_field(bytes, &mut offset, flags)?,
            flags,
        )?;
        let expected_manifest_hash = super::decode_aos_canonical_field::<iroha_crypto::Hash>(
            super::read_aos_field(bytes, &mut offset, flags)?,
            flags,
        )?;
        let expected_asset_definition = super::decode_aos_canonical_field::<
            crate::asset::AssetDefinitionId,
        >(
            super::read_aos_field(bytes, &mut offset, flags)?, flags
        )?;
        let expected_stake = super::decode_aos_canonical_field::<
            iroha_primitives::numeric::Quantity,
        >(super::read_aos_field(bytes, &mut offset, flags)?, flags)?;
        if offset != bytes.len() {
            return Err(norito::core::Error::LengthMismatch);
        }
        norito::core::note_payload_access(bytes, offset);
        Ok((
            Self {
                session_id,
                input_key,
                application_data,
                resources,
                invitation,
                expected_manifest_hash,
                expected_asset_definition,
                expected_stake,
            },
            offset,
        ))
    }
}
isi! {
 /// Native StartGameSessionV1 operation with complete authorization in Core.
 #[derive (crate :: DeriveJsonSerialize , crate :: DeriveJsonDeserialize)]
 #[norito (deny_unknown_fields)]
 #[norito_schema(name = "iroha_data_model::isi::game::StartGameSessionV1")]
 pub struct StartGameSessionV1 {
  /// Exact session id for this operation.
  pub session_id: iroha_crypto::Hash,
 }
}
impl crate::seal::Instruction for StartGameSessionV1 {}
impl StartGameSessionV1 {
    /// Construct an instruction that freezes a session's joined roster.
    #[must_use]
    pub const fn new(session_id: iroha_crypto::Hash) -> Self {
        Self { session_id }
    }
}
impl<'a> norito::core::DecodeFromSlice<'a> for StartGameSessionV1 {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        let flags = norito::core::effective_decode_flags()
            .unwrap_or_else(norito::core::default_encode_flags);
        if flags & norito::core::header_flags::PACKED_STRUCT != 0 {
            return super::decode_packed_instruction_payload::<Self>(bytes);
        }
        let mut offset = 0usize;
        let session_id = super::decode_aos_canonical_field::<iroha_crypto::Hash>(
            super::read_aos_field(bytes, &mut offset, flags)?,
            flags,
        )?;
        if offset != bytes.len() {
            return Err(norito::core::Error::LengthMismatch);
        }
        norito::core::note_payload_access(bytes, offset);
        Ok((Self { session_id }, offset))
    }
}
isi! {
 /// Native CommitGameCheckpointV1 operation with complete authorization in Core.
 #[derive (crate :: DeriveJsonSerialize , crate :: DeriveJsonDeserialize)]
 #[norito (deny_unknown_fields)]
 #[norito_schema(name = "iroha_data_model::isi::game::CommitGameCheckpointV1")]
 pub struct CommitGameCheckpointV1 {
  /// Exact session id for this operation.
  pub session_id: iroha_crypto::Hash,
  /// Exact checkpoint for this operation.
  pub checkpoint: crate::game::SignedGameCheckpointV1,
  /// Exact frontier for this operation.
  pub frontier: Option<crate::game::GameCommitmentSetV1>,
 }
}
impl crate::seal::Instruction for CommitGameCheckpointV1 {}
impl CommitGameCheckpointV1 {
    /// Submit a certified checkpoint together with any certified pending input frontier.
    #[must_use]
    pub fn new(
        session_id: iroha_crypto::Hash,
        checkpoint: crate::game::SignedGameCheckpointV1,
        frontier: Option<crate::game::GameCommitmentSetV1>,
    ) -> Self {
        Self {
            session_id,
            checkpoint,
            frontier,
        }
    }
}
impl<'a> norito::core::DecodeFromSlice<'a> for CommitGameCheckpointV1 {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        let flags = norito::core::effective_decode_flags()
            .unwrap_or_else(norito::core::default_encode_flags);
        if flags & norito::core::header_flags::PACKED_STRUCT != 0 {
            return super::decode_packed_instruction_payload::<Self>(bytes);
        }
        let mut offset = 0usize;
        let session_id = super::decode_aos_canonical_field::<iroha_crypto::Hash>(
            super::read_aos_field(bytes, &mut offset, flags)?,
            flags,
        )?;
        let checkpoint = super::decode_aos_canonical_field::<crate::game::SignedGameCheckpointV1>(
            super::read_aos_field(bytes, &mut offset, flags)?,
            flags,
        )?;
        let frontier = super::decode_aos_canonical_field::<Option<crate::game::GameCommitmentSetV1>>(
            super::read_aos_field(bytes, &mut offset, flags)?,
            flags,
        )?;
        if offset != bytes.len() {
            return Err(norito::core::Error::LengthMismatch);
        }
        norito::core::note_payload_access(bytes, offset);
        Ok((
            Self {
                session_id,
                checkpoint,
                frontier,
            },
            offset,
        ))
    }
}
isi! {
 /// Native ChallengeGameSessionV1 operation with complete authorization in Core.
 #[derive (crate :: DeriveJsonSerialize , crate :: DeriveJsonDeserialize)]
 #[norito (deny_unknown_fields)]
 #[norito_schema(name = "iroha_data_model::isi::game::ChallengeGameSessionV1")]
 pub struct ChallengeGameSessionV1 {
  /// Exact session id for this operation.
  pub session_id: iroha_crypto::Hash,
  /// Exact epoch for this operation.
  pub epoch: u64,
  /// Exact slot for this operation.
  pub slot: u8,
  /// Exact signature for this operation.
  pub signature: iroha_crypto::Signature,
 }
}
impl crate::seal::Instruction for ChallengeGameSessionV1 {}
impl ChallengeGameSessionV1 {
    /// Submit an input-key signature that opens the specified session epoch's dispute.
    #[must_use]
    pub fn new(
        session_id: iroha_crypto::Hash,
        epoch: u64,
        slot: u8,
        signature: iroha_crypto::Signature,
    ) -> Self {
        Self {
            session_id,
            epoch,
            slot,
            signature,
        }
    }
}
impl<'a> norito::core::DecodeFromSlice<'a> for ChallengeGameSessionV1 {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        let flags = norito::core::effective_decode_flags()
            .unwrap_or_else(norito::core::default_encode_flags);
        if flags & norito::core::header_flags::PACKED_STRUCT != 0 {
            return super::decode_packed_instruction_payload::<Self>(bytes);
        }
        let mut offset = 0usize;
        let session_id = super::decode_aos_canonical_field::<iroha_crypto::Hash>(
            super::read_aos_field(bytes, &mut offset, flags)?,
            flags,
        )?;
        let epoch = super::decode_aos_canonical_field::<u64>(
            super::read_aos_field(bytes, &mut offset, flags)?,
            flags,
        )?;
        let slot = super::decode_aos_canonical_field::<u8>(
            super::read_aos_field(bytes, &mut offset, flags)?,
            flags,
        )?;
        let signature = super::decode_aos_canonical_field::<iroha_crypto::Signature>(
            super::read_aos_field(bytes, &mut offset, flags)?,
            flags,
        )?;
        if offset != bytes.len() {
            return Err(norito::core::Error::LengthMismatch);
        }
        norito::core::note_payload_access(bytes, offset);
        Ok((
            Self {
                session_id,
                epoch,
                slot,
                signature,
            },
            offset,
        ))
    }
}
isi! {
 /// Native CommitGameInputsV1 operation with complete authorization in Core.
 #[derive (crate :: DeriveJsonSerialize , crate :: DeriveJsonDeserialize)]
 #[norito (deny_unknown_fields)]
 #[norito_schema(name = "iroha_data_model::isi::game::CommitGameInputsV1")]
 pub struct CommitGameInputsV1 {
  /// Exact input for this operation.
  pub input: crate::game::GameInputCommitmentV1,
 }
}
impl crate::seal::Instruction for CommitGameInputsV1 {}
impl CommitGameInputsV1 {
    /// Submit a signed commitment for a bounded forced input round.
    #[must_use]
    pub fn new(input: crate::game::GameInputCommitmentV1) -> Self {
        Self { input }
    }
}
impl<'a> norito::core::DecodeFromSlice<'a> for CommitGameInputsV1 {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        let flags = norito::core::effective_decode_flags()
            .unwrap_or_else(norito::core::default_encode_flags);
        if flags & norito::core::header_flags::PACKED_STRUCT != 0 {
            return super::decode_packed_instruction_payload::<Self>(bytes);
        }
        let mut offset = 0usize;
        let input = super::decode_aos_canonical_field::<crate::game::GameInputCommitmentV1>(
            super::read_aos_field(bytes, &mut offset, flags)?,
            flags,
        )?;
        if offset != bytes.len() {
            return Err(norito::core::Error::LengthMismatch);
        }
        norito::core::note_payload_access(bytes, offset);
        Ok((Self { input }, offset))
    }
}
isi! {
 /// Native RevealGameInputsV1 operation with complete authorization in Core.
 #[derive (crate :: DeriveJsonSerialize , crate :: DeriveJsonDeserialize)]
 #[norito (deny_unknown_fields)]
 #[norito_schema(name = "iroha_data_model::isi::game::RevealGameInputsV1")]
 pub struct RevealGameInputsV1 {
  /// Exact reveal for this operation.
  pub reveal: crate::game::GameInputRevealV1,
 }
}
impl crate::seal::Instruction for RevealGameInputsV1 {}
impl RevealGameInputsV1 {
    /// Reveal public controls matching a previously admitted commitment.
    #[must_use]
    pub fn new(reveal: crate::game::GameInputRevealV1) -> Self {
        Self { reveal }
    }
}
impl<'a> norito::core::DecodeFromSlice<'a> for RevealGameInputsV1 {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        let flags = norito::core::effective_decode_flags()
            .unwrap_or_else(norito::core::default_encode_flags);
        if flags & norito::core::header_flags::PACKED_STRUCT != 0 {
            return super::decode_packed_instruction_payload::<Self>(bytes);
        }
        let mut offset = 0usize;
        let reveal = super::decode_aos_canonical_field::<crate::game::GameInputRevealV1>(
            super::read_aos_field(bytes, &mut offset, flags)?,
            flags,
        )?;
        if offset != bytes.len() {
            return Err(norito::core::Error::LengthMismatch);
        }
        norito::core::note_payload_access(bytes, offset);
        Ok((Self { reveal }, offset))
    }
}
isi! {
 /// Native AdvanceGameDeadlineV1 operation with complete authorization in Core.
 #[derive (crate :: DeriveJsonSerialize , crate :: DeriveJsonDeserialize)]
 #[norito (deny_unknown_fields)]
 #[norito_schema(name = "iroha_data_model::isi::game::AdvanceGameDeadlineV1")]
 pub struct AdvanceGameDeadlineV1 {
  /// Exact session id for this operation.
  pub session_id: iroha_crypto::Hash,
 }
}
impl crate::seal::Instruction for AdvanceGameDeadlineV1 {}
impl AdvanceGameDeadlineV1 {
    /// Request the deterministic transition allowed by the current block-height deadline.
    #[must_use]
    pub const fn new(session_id: iroha_crypto::Hash) -> Self {
        Self { session_id }
    }
}
impl<'a> norito::core::DecodeFromSlice<'a> for AdvanceGameDeadlineV1 {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        let flags = norito::core::effective_decode_flags()
            .unwrap_or_else(norito::core::default_encode_flags);
        if flags & norito::core::header_flags::PACKED_STRUCT != 0 {
            return super::decode_packed_instruction_payload::<Self>(bytes);
        }
        let mut offset = 0usize;
        let session_id = super::decode_aos_canonical_field::<iroha_crypto::Hash>(
            super::read_aos_field(bytes, &mut offset, flags)?,
            flags,
        )?;
        if offset != bytes.len() {
            return Err(norito::core::Error::LengthMismatch);
        }
        norito::core::note_payload_access(bytes, offset);
        Ok((Self { session_id }, offset))
    }
}
isi! {
 /// Native SettleGameSessionV1 operation with complete authorization in Core.
 #[derive (crate :: DeriveJsonSerialize , crate :: DeriveJsonDeserialize)]
 #[norito (deny_unknown_fields)]
 #[norito_schema(name = "iroha_data_model::isi::game::SettleGameSessionV1")]
 pub struct SettleGameSessionV1 {
  /// Exact session id for this operation.
  pub session_id: iroha_crypto::Hash,
  /// Exact proof for this operation.
  pub proof: crate::execution_proofs::ExecutionProofEnvelopeV1,
  /// Exact outcome for this operation.
  pub outcome: crate::game::GameOutcomeV1,
 }
}
impl crate::seal::Instruction for SettleGameSessionV1 {}
impl SettleGameSessionV1 {
    /// Submit an inline execution proof and its claimed outcome for atomic settlement.
    #[must_use]
    pub fn new(
        session_id: iroha_crypto::Hash,
        proof: crate::execution_proofs::ExecutionProofEnvelopeV1,
        outcome: crate::game::GameOutcomeV1,
    ) -> Self {
        Self {
            session_id,
            proof,
            outcome,
        }
    }
}
impl<'a> norito::core::DecodeFromSlice<'a> for SettleGameSessionV1 {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        let flags = norito::core::effective_decode_flags()
            .unwrap_or_else(norito::core::default_encode_flags);
        if flags & norito::core::header_flags::PACKED_STRUCT != 0 {
            return super::decode_packed_instruction_payload::<Self>(bytes);
        }
        let mut offset = 0usize;
        let session_id = super::decode_aos_canonical_field::<iroha_crypto::Hash>(
            super::read_aos_field(bytes, &mut offset, flags)?,
            flags,
        )?;
        let proof = super::decode_aos_canonical_field::<
            crate::execution_proofs::ExecutionProofEnvelopeV1,
        >(super::read_aos_field(bytes, &mut offset, flags)?, flags)?;
        let outcome = super::decode_aos_canonical_field::<crate::game::GameOutcomeV1>(
            super::read_aos_field(bytes, &mut offset, flags)?,
            flags,
        )?;
        if offset != bytes.len() {
            return Err(norito::core::Error::LengthMismatch);
        }
        norito::core::note_payload_access(bytes, offset);
        Ok((
            Self {
                session_id,
                proof,
                outcome,
            },
            offset,
        ))
    }
}
isi! {
 /// Native ExpireGameSessionV1 operation with complete authorization in Core.
 #[derive (crate :: DeriveJsonSerialize , crate :: DeriveJsonDeserialize)]
 #[norito (deny_unknown_fields)]
 #[norito_schema(name = "iroha_data_model::isi::game::ExpireGameSessionV1")]
 pub struct ExpireGameSessionV1 {
  /// Exact session id for this operation.
  pub session_id: iroha_crypto::Hash,
 }
}
impl crate::seal::Instruction for ExpireGameSessionV1 {}
impl ExpireGameSessionV1 {
    /// Request exact refunds for a lobby whose join deadline has expired.
    #[must_use]
    pub const fn new(session_id: iroha_crypto::Hash) -> Self {
        Self { session_id }
    }
}
isi! {
    /// Pay an exact portion of a finalized game prize or refund claim.
    #[derive (crate :: DeriveJsonSerialize , crate :: DeriveJsonDeserialize)]
    #[norito (deny_unknown_fields)]
    #[norito_schema(name = "iroha_data_model::isi::game::ClaimGamePayoutV1")]
    pub struct ClaimGamePayoutV1 {
        /// Session retaining the backed claim.
        pub session_id: iroha_crypto::Hash,
        /// Permanent roster slot that owns the claim.
        pub slot: u8,
        /// Existing destination account; redirection requires the original wallet authority.
        pub destination: crate::account::AccountId,
        /// Positive exact claim amount to transfer.
        pub amount: iroha_primitives::numeric::Quantity,
    }
}
impl crate::seal::Instruction for ClaimGamePayoutV1 {}
impl ClaimGamePayoutV1 {
    /// Construct a partial claim payment to an existing ledger account.
    #[must_use]
    pub fn new(
        session_id: iroha_crypto::Hash,
        slot: u8,
        destination: crate::account::AccountId,
        amount: iroha_primitives::numeric::Quantity,
    ) -> Self {
        Self {
            session_id,
            slot,
            destination,
            amount,
        }
    }
}
impl<'a> norito::core::DecodeFromSlice<'a> for ClaimGamePayoutV1 {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        let flags = norito::core::effective_decode_flags()
            .unwrap_or_else(norito::core::default_encode_flags);
        if flags & norito::core::header_flags::PACKED_STRUCT != 0 {
            return super::decode_packed_instruction_payload::<Self>(bytes);
        }
        let mut offset = 0usize;
        let session_id = super::decode_aos_canonical_field::<iroha_crypto::Hash>(
            super::read_aos_field(bytes, &mut offset, flags)?,
            flags,
        )?;
        let slot = super::decode_aos_canonical_field::<u8>(
            super::read_aos_field(bytes, &mut offset, flags)?,
            flags,
        )?;
        let destination = super::decode_aos_canonical_field::<crate::account::AccountId>(
            super::read_aos_field(bytes, &mut offset, flags)?,
            flags,
        )?;
        let amount = super::decode_aos_canonical_field::<iroha_primitives::numeric::Quantity>(
            super::read_aos_field(bytes, &mut offset, flags)?,
            flags,
        )?;
        if offset != bytes.len() {
            return Err(norito::core::Error::LengthMismatch);
        }
        norito::core::note_payload_access(bytes, offset);
        Ok((
            Self {
                session_id,
                slot,
                destination,
                amount,
            },
            offset,
        ))
    }
}
impl<'a> norito::core::DecodeFromSlice<'a> for ExpireGameSessionV1 {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        let flags = norito::core::effective_decode_flags()
            .unwrap_or_else(norito::core::default_encode_flags);
        if flags & norito::core::header_flags::PACKED_STRUCT != 0 {
            return super::decode_packed_instruction_payload::<Self>(bytes);
        }
        let mut offset = 0usize;
        let session_id = super::decode_aos_canonical_field::<iroha_crypto::Hash>(
            super::read_aos_field(bytes, &mut offset, flags)?,
            flags,
        )?;
        if offset != bytes.len() {
            return Err(norito::core::Error::LengthMismatch);
        }
        norito::core::note_payload_access(bytes, offset);
        Ok((Self { session_id }, offset))
    }
}
isi! {
 /// Native RegisterExecutionProofProfileV1 operation with complete authorization in Core.
 #[derive (crate :: DeriveJsonSerialize , crate :: DeriveJsonDeserialize)]
 #[norito (deny_unknown_fields)]
 #[norito_schema(name = "iroha_data_model::isi::game::RegisterExecutionProofProfileV1")]
 pub struct RegisterExecutionProofProfileV1 {
  /// Exact profile id for this operation.
  pub profile_id: iroha_crypto::Hash,
 }
}
impl crate::seal::Instruction for RegisterExecutionProofProfileV1 {}
impl RegisterExecutionProofProfileV1 {
    /// Register a compiled profile by its immutable identifier; no verifier code is supplied.
    #[must_use]
    pub const fn new(profile_id: iroha_crypto::Hash) -> Self {
        Self { profile_id }
    }
}
impl<'a> norito::core::DecodeFromSlice<'a> for RegisterExecutionProofProfileV1 {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        let flags = norito::core::effective_decode_flags()
            .unwrap_or_else(norito::core::default_encode_flags);
        if flags & norito::core::header_flags::PACKED_STRUCT != 0 {
            return super::decode_packed_instruction_payload::<Self>(bytes);
        }
        let mut offset = 0usize;
        let profile_id = super::decode_aos_canonical_field::<iroha_crypto::Hash>(
            super::read_aos_field(bytes, &mut offset, flags)?,
            flags,
        )?;
        if offset != bytes.len() {
            return Err(norito::core::Error::LengthMismatch);
        }
        norito::core::note_payload_access(bytes, offset);
        Ok((Self { profile_id }, offset))
    }
}
isi! {
 /// Native VerifyExecutionProofV1 operation with complete authorization in Core.
 #[derive (crate :: DeriveJsonSerialize , crate :: DeriveJsonDeserialize)]
 #[norito (deny_unknown_fields)]
 #[norito_schema(name = "iroha_data_model::isi::game::VerifyExecutionProofV1")]
 pub struct VerifyExecutionProofV1 {
  /// Exact proof for this operation.
  pub proof: crate::execution_proofs::ExecutionProofEnvelopeV1,
 }
}
impl crate::seal::Instruction for VerifyExecutionProofV1 {}
impl VerifyExecutionProofV1 {
    /// Verify an inline execution proof without performing application settlement.
    #[must_use]
    pub fn new(proof: crate::execution_proofs::ExecutionProofEnvelopeV1) -> Self {
        Self { proof }
    }
}
impl<'a> norito::core::DecodeFromSlice<'a> for VerifyExecutionProofV1 {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        let flags = norito::core::effective_decode_flags()
            .unwrap_or_else(norito::core::default_encode_flags);
        if flags & norito::core::header_flags::PACKED_STRUCT != 0 {
            return super::decode_packed_instruction_payload::<Self>(bytes);
        }
        let mut offset = 0usize;
        let proof = super::decode_aos_canonical_field::<
            crate::execution_proofs::ExecutionProofEnvelopeV1,
        >(super::read_aos_field(bytes, &mut offset, flags)?, flags)?;
        if offset != bytes.len() {
            return Err(norito::core::Error::LengthMismatch);
        }
        norito::core::note_payload_access(bytes, offset);
        Ok((Self { proof }, offset))
    }
}

isi! {
 /// Reserve an already-joined wallet's explicitly selected NFT before a game starts.
 #[derive (crate :: DeriveJsonSerialize , crate :: DeriveJsonDeserialize)]
 #[norito (deny_unknown_fields)]
 #[norito_schema(name = "iroha_data_model::isi::game::StakeGameItemV1")]
 pub struct StakeGameItemV1 {
  /// Exact session being joined.
  pub session_id: iroha_crypto::Hash,
  /// One wallet-owned NFT to escrow under the native sole-winner-or-return policy.
  pub nft_id: crate::nft::NftId,
  /// Wallet-approved immutable rules commitment; substitution is rejected.
  pub expected_manifest_hash: iroha_crypto::Hash,
 }
}
impl crate::seal::Instruction for StakeGameItemV1 {}
impl StakeGameItemV1 {
    /// Select an exact NFT for a joined slot under the approved immutable manifest.
    #[must_use]
    pub fn new(
        session_id: iroha_crypto::Hash,
        nft_id: crate::nft::NftId,
        expected_manifest_hash: iroha_crypto::Hash,
    ) -> Self {
        Self {
            session_id,
            nft_id,
            expected_manifest_hash,
        }
    }
}
impl<'a> norito::core::DecodeFromSlice<'a> for StakeGameItemV1 {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        let flags = norito::core::effective_decode_flags()
            .unwrap_or_else(norito::core::default_encode_flags);
        if flags & norito::core::header_flags::PACKED_STRUCT != 0 {
            return super::decode_packed_instruction_payload::<Self>(bytes);
        }
        let mut offset = 0usize;
        let session_id = super::decode_aos_canonical_field::<iroha_crypto::Hash>(
            super::read_aos_field(bytes, &mut offset, flags)?,
            flags,
        )?;
        let nft_id = super::decode_aos_canonical_field::<crate::nft::NftId>(
            super::read_aos_field(bytes, &mut offset, flags)?,
            flags,
        )?;
        let expected_manifest_hash = super::decode_aos_canonical_field::<iroha_crypto::Hash>(
            super::read_aos_field(bytes, &mut offset, flags)?,
            flags,
        )?;
        if offset != bytes.len() {
            return Err(norito::core::Error::LengthMismatch);
        }
        norito::core::note_payload_access(bytes, offset);
        Ok((
            Self {
                session_id,
                nft_id,
                expected_manifest_hash,
            },
            offset,
        ))
    }
}
