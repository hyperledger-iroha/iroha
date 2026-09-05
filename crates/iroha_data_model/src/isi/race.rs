//! Native race lifecycle instructions. Gameplay keys never authorize wallet debits.
use super::*;
isi! {
    /// Canonical native OpenRaceV1 transition; Core enforces its complete state and signature policy.
    pub struct OpenRaceV1 {
        /// Exact race id for this transition.
        pub race_id: iroha_crypto::Hash,
        /// Exact rules for this transition.
        pub rules: crate::race::RaceRulesV1,
        /// Exact asset definition for this transition.
        pub asset_definition: crate::asset::AssetDefinitionId,
        /// Exact stake for this transition.
        pub stake: iroha_primitives::numeric::Quantity,
        /// Exact join deadline height for this transition.
        pub join_deadline_height: u64,
    }
}
impl crate::seal::Instruction for OpenRaceV1 {}
impl<'a> norito::core::DecodeFromSlice<'a> for OpenRaceV1 {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        let flags = norito::core::effective_decode_flags()
            .unwrap_or_else(norito::core::default_encode_flags);
        if flags & norito::core::header_flags::PACKED_STRUCT != 0 {
            return super::decode_packed_instruction_payload::<Self>(bytes);
        }
        let mut offset = 0usize;
        let race_id = super::decode_aos_canonical_field::<iroha_crypto::Hash>(
            super::read_aos_field(bytes, &mut offset, flags)?,
            flags,
        )?;
        let rules = super::decode_aos_canonical_field::<crate::race::RaceRulesV1>(
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
                race_id,
                rules,
                asset_definition,
                stake,
                join_deadline_height,
            },
            offset,
        ))
    }
}
isi! {
    /// Canonical native JoinRaceV1 transition; Core enforces its complete state and signature policy.
    pub struct JoinRaceV1 {
        /// Exact race id for this transition.
        pub race_id: iroha_crypto::Hash,
        /// Exact input key for this transition.
        pub input_key: iroha_crypto::PublicKey,
        /// Exact car id for this transition.
        pub car_id: u8,
    }
}
impl crate::seal::Instruction for JoinRaceV1 {}
impl<'a> norito::core::DecodeFromSlice<'a> for JoinRaceV1 {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        let flags = norito::core::effective_decode_flags()
            .unwrap_or_else(norito::core::default_encode_flags);
        if flags & norito::core::header_flags::PACKED_STRUCT != 0 {
            return super::decode_packed_instruction_payload::<Self>(bytes);
        }
        let mut offset = 0usize;
        let race_id = super::decode_aos_canonical_field::<iroha_crypto::Hash>(
            super::read_aos_field(bytes, &mut offset, flags)?,
            flags,
        )?;
        let input_key = super::decode_aos_canonical_field::<iroha_crypto::PublicKey>(
            super::read_aos_field(bytes, &mut offset, flags)?,
            flags,
        )?;
        let car_id = super::decode_aos_canonical_field::<u8>(
            super::read_aos_field(bytes, &mut offset, flags)?,
            flags,
        )?;
        if offset != bytes.len() {
            return Err(norito::core::Error::LengthMismatch);
        }
        norito::core::note_payload_access(bytes, offset);
        Ok((
            Self {
                race_id,
                input_key,
                car_id,
            },
            offset,
        ))
    }
}
isi! {
    /// Canonical native StartRaceV1 transition; Core enforces its complete state and signature policy.
    pub struct StartRaceV1 {
        /// Exact race id for this transition.
        pub race_id: iroha_crypto::Hash,
    }
}
impl crate::seal::Instruction for StartRaceV1 {}
impl<'a> norito::core::DecodeFromSlice<'a> for StartRaceV1 {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        let flags = norito::core::effective_decode_flags()
            .unwrap_or_else(norito::core::default_encode_flags);
        if flags & norito::core::header_flags::PACKED_STRUCT != 0 {
            return super::decode_packed_instruction_payload::<Self>(bytes);
        }
        let mut offset = 0usize;
        let race_id = super::decode_aos_canonical_field::<iroha_crypto::Hash>(
            super::read_aos_field(bytes, &mut offset, flags)?,
            flags,
        )?;
        if offset != bytes.len() {
            return Err(norito::core::Error::LengthMismatch);
        }
        norito::core::note_payload_access(bytes, offset);
        Ok((Self { race_id }, offset))
    }
}
isi! {
    /// Canonical native CommitRaceCheckpointV1 transition; Core enforces its complete state and signature policy.
    pub struct CommitRaceCheckpointV1 {
        /// Exact race id for this transition.
        pub race_id: iroha_crypto::Hash,
        /// Exact checkpoint for this transition.
        pub checkpoint: crate::race::SignedRaceCheckpointV1,
        /// Exact frontier for this transition.
        pub frontier: Option<crate::race::RaceCommitmentSetV1>,
    }
}
impl crate::seal::Instruction for CommitRaceCheckpointV1 {}
impl<'a> norito::core::DecodeFromSlice<'a> for CommitRaceCheckpointV1 {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        let flags = norito::core::effective_decode_flags()
            .unwrap_or_else(norito::core::default_encode_flags);
        if flags & norito::core::header_flags::PACKED_STRUCT != 0 {
            return super::decode_packed_instruction_payload::<Self>(bytes);
        }
        let mut offset = 0usize;
        let race_id = super::decode_aos_canonical_field::<iroha_crypto::Hash>(
            super::read_aos_field(bytes, &mut offset, flags)?,
            flags,
        )?;
        let checkpoint = super::decode_aos_canonical_field::<crate::race::SignedRaceCheckpointV1>(
            super::read_aos_field(bytes, &mut offset, flags)?,
            flags,
        )?;
        let frontier = super::decode_aos_canonical_field::<Option<crate::race::RaceCommitmentSetV1>>(
            super::read_aos_field(bytes, &mut offset, flags)?,
            flags,
        )?;
        if offset != bytes.len() {
            return Err(norito::core::Error::LengthMismatch);
        }
        norito::core::note_payload_access(bytes, offset);
        Ok((
            Self {
                race_id,
                checkpoint,
                frontier,
            },
            offset,
        ))
    }
}
isi! {
    /// Canonical native ChallengeRaceV1 transition; Core enforces its complete state and signature policy.
    pub struct ChallengeRaceV1 {
        /// Exact race id for this transition.
        pub race_id: iroha_crypto::Hash,
        /// Exact epoch for this transition.
        pub epoch: u64,
        /// Exact slot for this transition.
        pub slot: u8,
        /// Exact signature for this transition.
        pub signature: iroha_crypto::Signature,
    }
}
impl crate::seal::Instruction for ChallengeRaceV1 {}
impl<'a> norito::core::DecodeFromSlice<'a> for ChallengeRaceV1 {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        let flags = norito::core::effective_decode_flags()
            .unwrap_or_else(norito::core::default_encode_flags);
        if flags & norito::core::header_flags::PACKED_STRUCT != 0 {
            return super::decode_packed_instruction_payload::<Self>(bytes);
        }
        let mut offset = 0usize;
        let race_id = super::decode_aos_canonical_field::<iroha_crypto::Hash>(
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
                race_id,
                epoch,
                slot,
                signature,
            },
            offset,
        ))
    }
}
isi! {
    /// Canonical native CommitRaceInputsV1 transition; Core enforces its complete state and signature policy.
    pub struct CommitRaceInputsV1 {
        /// Exact input for this transition.
        pub input: crate::race::RaceInputCommitmentV1,
    }
}
impl crate::seal::Instruction for CommitRaceInputsV1 {}
impl<'a> norito::core::DecodeFromSlice<'a> for CommitRaceInputsV1 {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        let flags = norito::core::effective_decode_flags()
            .unwrap_or_else(norito::core::default_encode_flags);
        if flags & norito::core::header_flags::PACKED_STRUCT != 0 {
            return super::decode_packed_instruction_payload::<Self>(bytes);
        }
        let mut offset = 0usize;
        let input = super::decode_aos_canonical_field::<crate::race::RaceInputCommitmentV1>(
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
    /// Canonical native RevealRaceInputsV1 transition; Core enforces its complete state and signature policy.
    pub struct RevealRaceInputsV1 {
        /// Exact reveal for this transition.
        pub reveal: crate::race::RaceInputRevealV1,
    }
}
impl crate::seal::Instruction for RevealRaceInputsV1 {}
impl<'a> norito::core::DecodeFromSlice<'a> for RevealRaceInputsV1 {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        let flags = norito::core::effective_decode_flags()
            .unwrap_or_else(norito::core::default_encode_flags);
        if flags & norito::core::header_flags::PACKED_STRUCT != 0 {
            return super::decode_packed_instruction_payload::<Self>(bytes);
        }
        let mut offset = 0usize;
        let reveal = super::decode_aos_canonical_field::<crate::race::RaceInputRevealV1>(
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
    /// Canonical native AdvanceRaceDeadlineV1 transition; Core enforces its complete state and signature policy.
    pub struct AdvanceRaceDeadlineV1 {
        /// Exact race id for this transition.
        pub race_id: iroha_crypto::Hash,
    }
}
impl crate::seal::Instruction for AdvanceRaceDeadlineV1 {}
impl<'a> norito::core::DecodeFromSlice<'a> for AdvanceRaceDeadlineV1 {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        let flags = norito::core::effective_decode_flags()
            .unwrap_or_else(norito::core::default_encode_flags);
        if flags & norito::core::header_flags::PACKED_STRUCT != 0 {
            return super::decode_packed_instruction_payload::<Self>(bytes);
        }
        let mut offset = 0usize;
        let race_id = super::decode_aos_canonical_field::<iroha_crypto::Hash>(
            super::read_aos_field(bytes, &mut offset, flags)?,
            flags,
        )?;
        if offset != bytes.len() {
            return Err(norito::core::Error::LengthMismatch);
        }
        norito::core::note_payload_access(bytes, offset);
        Ok((Self { race_id }, offset))
    }
}
isi! {
    /// Canonical native SubmitRaceProofV1 transition; Core enforces its complete state and signature policy.
    pub struct SubmitRaceProofV1 {
        /// Exact race id for this transition.
        pub race_id: iroha_crypto::Hash,
        /// Exact proof for this transition.
        pub proof: crate::execution_proofs::ExecutionProofEnvelopeV1,
    }
}
impl crate::seal::Instruction for SubmitRaceProofV1 {}
impl<'a> norito::core::DecodeFromSlice<'a> for SubmitRaceProofV1 {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        let flags = norito::core::effective_decode_flags()
            .unwrap_or_else(norito::core::default_encode_flags);
        if flags & norito::core::header_flags::PACKED_STRUCT != 0 {
            return super::decode_packed_instruction_payload::<Self>(bytes);
        }
        let mut offset = 0usize;
        let race_id = super::decode_aos_canonical_field::<iroha_crypto::Hash>(
            super::read_aos_field(bytes, &mut offset, flags)?,
            flags,
        )?;
        let proof = super::decode_aos_canonical_field::<
            crate::execution_proofs::ExecutionProofEnvelopeV1,
        >(super::read_aos_field(bytes, &mut offset, flags)?, flags)?;
        if offset != bytes.len() {
            return Err(norito::core::Error::LengthMismatch);
        }
        norito::core::note_payload_access(bytes, offset);
        Ok((Self { race_id, proof }, offset))
    }
}
isi! {
    /// Canonical native ExpireRaceV1 transition; Core enforces its complete state and signature policy.
    pub struct ExpireRaceV1 {
        /// Exact race id for this transition.
        pub race_id: iroha_crypto::Hash,
    }
}
impl crate::seal::Instruction for ExpireRaceV1 {}
impl<'a> norito::core::DecodeFromSlice<'a> for ExpireRaceV1 {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        let flags = norito::core::effective_decode_flags()
            .unwrap_or_else(norito::core::default_encode_flags);
        if flags & norito::core::header_flags::PACKED_STRUCT != 0 {
            return super::decode_packed_instruction_payload::<Self>(bytes);
        }
        let mut offset = 0usize;
        let race_id = super::decode_aos_canonical_field::<iroha_crypto::Hash>(
            super::read_aos_field(bytes, &mut offset, flags)?,
            flags,
        )?;
        if offset != bytes.len() {
            return Err(norito::core::Error::LengthMismatch);
        }
        norito::core::note_payload_access(bytes, offset);
        Ok((Self { race_id }, offset))
    }
}
