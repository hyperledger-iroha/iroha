use iroha_primitives::numeric::Numeric;

use super::*;
use crate::{
    oracle::{
        DefiOracleAttestation, FeedConfig, FeedId, FeedSlot, KeyedHash, Observation,
        OracleChangeClass, OracleChangeId, OracleChangeStage, OracleDisputeId,
        OracleDisputeOutcome, OracleId, TwitterBindingAttestation,
    },
    prelude::Hash,
};

isi! {
    /// Register an oracle feed configuration on-chain.
    pub struct RegisterOracleFeed {
        /// Feed configuration to make available for oracle submissions.
        pub feed: FeedConfig,
    }
}

isi! {
    /// Submit a signed oracle observation for admission.
    pub struct SubmitOracleObservation {
        /// Observation payload and signature produced by the oracle provider.
        pub observation: Observation,
    }
}

isi! {
    /// Aggregate admitted observations for a feed slot into a feed event.
    pub struct AggregateOracleFeed {
        /// Target feed identifier.
        pub feed_id: FeedId,
        /// Slot index being aggregated.
        pub slot: FeedSlot,
        /// Canonical request hash for the aggregation window.
        pub request_hash: Hash,
        /// Optional hashes of external evidence (e.g., `SoraFS` bundles).
        #[cfg_attr(feature = "json", norito(default))]
        pub evidence_hashes: Vec<Hash>,
    }
}

isi! {
    /// Open a dispute against an oracle provider for a specific feed slot.
    pub struct OpenOracleDispute {
        /// Feed identifier for the disputed observation window.
        pub feed_id: FeedId,
        /// Slot index being challenged.
        pub slot: FeedSlot,
        /// Canonical request hash for the disputed window.
        pub request_hash: Hash,
        /// Provider being challenged.
        pub target: OracleId,
        /// Optional bond override (falls back to config when `None`).
        #[cfg_attr(feature = "json", norito(default))]
        pub bond: Option<Numeric>,
        /// Evidence hashes supplied by the challenger.
        #[cfg_attr(feature = "json", norito(default))]
        pub evidence_hashes: Vec<Hash>,
        /// Human-readable reason for the dispute.
        pub reason: String,
    }
}

isi! {
    /// Resolve an open oracle dispute.
    pub struct ResolveOracleDispute {
        /// Identifier of the dispute being resolved.
        pub dispute_id: OracleDisputeId,
        /// Outcome applied to the dispute.
        pub outcome: OracleDisputeOutcome,
        /// Optional operator notes.
        #[cfg_attr(feature = "json", norito(default))]
        pub notes: String,
    }
}

isi! {
    /// Propose a governance change for an oracle feed.
    pub struct ProposeOracleChange {
        /// Unique identifier for the change proposal.
        pub change_id: OracleChangeId,
        /// Feed configuration that will be enacted after approval.
        pub feed: FeedConfig,
        /// Change classification driving quorum thresholds.
        pub class: OracleChangeClass,
        /// Hash of the change manifest or external artefact.
        pub payload_hash: Hash,
        /// Optional evidence bundle hashes attached at intake.
        #[cfg_attr(feature = "json", norito(default))]
        pub evidence_hashes: Vec<Hash>,
    }
}

isi! {
    /// Record a stage vote for an oracle change proposal.
    pub struct VoteOracleChangeStage {
        /// Identifier of the change proposal being reviewed.
        pub change_id: OracleChangeId,
        /// Stage receiving the vote.
        pub stage: OracleChangeStage,
        /// Whether this vote approves (`true`) or rejects (`false`) the stage.
        pub approve: bool,
        /// Optional evidence hashes linked to this vote.
        #[cfg_attr(feature = "json", norito(default))]
        pub evidence_hashes: Vec<Hash>,
    }
}

isi! {
    /// Explicitly roll back an oracle change proposal.
    pub struct RollbackOracleChange {
        /// Identifier of the change being rolled back.
        pub change_id: OracleChangeId,
        /// Optional stage to tag the rollback against (defaults to the active stage).
        #[cfg_attr(feature = "json", norito(default))]
        pub stage: Option<OracleChangeStage>,
        /// Human-readable reason for the rollback.
        pub reason: String,
    }
}

isi! {
    /// Submit a native Soracles attestation carrying DeFi ABI-compatible oracle bytes.
    pub struct SubmitDefiOracleAttestation {
        /// Attestation payload and compatibility signature.
        pub attestation: DefiOracleAttestation,
    }
}

isi! {
    /// Record a twitter follow binding attestation.
    pub struct RecordTwitterBinding {
        /// Attestation payload produced by the oracle committee.
        pub attestation: TwitterBindingAttestation,
        /// Feed identifier expected for the attestation (e.g., `twitter_follow_binding`).
        pub feed_id: FeedId,
    }
}

isi! {
    /// Revoke a twitter follow binding record.
    pub struct RevokeTwitterBinding {
        /// Binding keyed hash to revoke.
        pub binding_hash: KeyedHash,
        /// Human-readable reason for the revocation.
        pub reason: String,
    }
}

impl crate::seal::Instruction for RegisterOracleFeed {}
impl crate::seal::Instruction for SubmitOracleObservation {}
impl crate::seal::Instruction for AggregateOracleFeed {}
impl crate::seal::Instruction for OpenOracleDispute {}
impl crate::seal::Instruction for ResolveOracleDispute {}
impl crate::seal::Instruction for ProposeOracleChange {}
impl crate::seal::Instruction for VoteOracleChangeStage {}
impl crate::seal::Instruction for RollbackOracleChange {}
impl crate::seal::Instruction for SubmitDefiOracleAttestation {}
impl crate::seal::Instruction for RecordTwitterBinding {}
impl crate::seal::Instruction for RevokeTwitterBinding {}

fn oracle_decode_flags() -> u8 {
    norito::core::effective_decode_flags().unwrap_or_else(norito::core::default_encode_flags)
}

impl<'a> norito::core::DecodeFromSlice<'a> for RegisterOracleFeed {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        let flags = oracle_decode_flags();
        if flags & norito::core::header_flags::PACKED_STRUCT != 0 {
            return super::decode_packed_instruction_payload::<Self>(bytes);
        }

        let mut offset = 0usize;
        let feed = super::decode_aos_canonical_field::<FeedConfig>(
            super::read_aos_field(bytes, &mut offset, flags)?,
            flags,
        )?;
        if offset != bytes.len() {
            return Err(norito::core::Error::LengthMismatch);
        }
        norito::core::note_payload_access(bytes, offset);
        Ok((Self { feed }, offset))
    }
}

impl<'a> norito::core::DecodeFromSlice<'a> for SubmitOracleObservation {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        let flags = oracle_decode_flags();
        if flags & norito::core::header_flags::PACKED_STRUCT != 0 {
            return super::decode_packed_instruction_payload::<Self>(bytes);
        }

        let mut offset = 0usize;
        let observation = super::decode_aos_canonical_field::<Observation>(
            super::read_aos_field(bytes, &mut offset, flags)?,
            flags,
        )?;
        if offset != bytes.len() {
            return Err(norito::core::Error::LengthMismatch);
        }
        norito::core::note_payload_access(bytes, offset);
        Ok((Self { observation }, offset))
    }
}

impl<'a> norito::core::DecodeFromSlice<'a> for AggregateOracleFeed {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        let flags = oracle_decode_flags();
        if flags & norito::core::header_flags::PACKED_STRUCT != 0 {
            return super::decode_packed_instruction_payload::<Self>(bytes);
        }

        let mut offset = 0usize;
        let feed_id = super::decode_aos_canonical_field::<FeedId>(
            super::read_aos_field(bytes, &mut offset, flags)?,
            flags,
        )?;
        let slot = super::decode_aos_canonical_field::<FeedSlot>(
            super::read_aos_field(bytes, &mut offset, flags)?,
            flags,
        )?;
        let request_hash = super::decode_aos_canonical_field::<Hash>(
            super::read_aos_field(bytes, &mut offset, flags)?,
            flags,
        )?;
        let evidence_hashes = if offset < bytes.len() {
            super::decode_aos_slice_field::<Vec<Hash>>(
                super::read_aos_field(bytes, &mut offset, flags)?,
                flags,
            )?
        } else {
            Vec::new()
        };
        if offset != bytes.len() {
            return Err(norito::core::Error::LengthMismatch);
        }
        norito::core::note_payload_access(bytes, offset);
        Ok((
            Self {
                feed_id,
                slot,
                request_hash,
                evidence_hashes,
            },
            offset,
        ))
    }
}

impl<'a> norito::core::DecodeFromSlice<'a> for OpenOracleDispute {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        let flags = oracle_decode_flags();
        if flags & norito::core::header_flags::PACKED_STRUCT != 0 {
            return super::decode_packed_instruction_payload::<Self>(bytes);
        }

        let mut offset = 0usize;
        let feed_id = super::decode_aos_canonical_field::<FeedId>(
            super::read_aos_field(bytes, &mut offset, flags)?,
            flags,
        )?;
        let slot = super::decode_aos_canonical_field::<FeedSlot>(
            super::read_aos_field(bytes, &mut offset, flags)?,
            flags,
        )?;
        let request_hash = super::decode_aos_canonical_field::<Hash>(
            super::read_aos_field(bytes, &mut offset, flags)?,
            flags,
        )?;
        let target = super::decode_aos_canonical_field::<OracleId>(
            super::read_aos_field(bytes, &mut offset, flags)?,
            flags,
        )?;
        let bond = super::decode_aos_canonical_field::<Option<Numeric>>(
            super::read_aos_field(bytes, &mut offset, flags)?,
            flags,
        )?;
        let evidence_hashes = super::decode_aos_slice_field::<Vec<Hash>>(
            super::read_aos_field(bytes, &mut offset, flags)?,
            flags,
        )?;
        let reason = super::decode_aos_slice_field::<String>(
            super::read_aos_field(bytes, &mut offset, flags)?,
            flags,
        )?;
        if offset != bytes.len() {
            return Err(norito::core::Error::LengthMismatch);
        }
        norito::core::note_payload_access(bytes, offset);
        Ok((
            Self {
                feed_id,
                slot,
                request_hash,
                target,
                bond,
                evidence_hashes,
                reason,
            },
            offset,
        ))
    }
}

impl<'a> norito::core::DecodeFromSlice<'a> for ResolveOracleDispute {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        let flags = oracle_decode_flags();
        if flags & norito::core::header_flags::PACKED_STRUCT != 0 {
            return super::decode_packed_instruction_payload::<Self>(bytes);
        }

        let mut offset = 0usize;
        let dispute_id = super::decode_aos_canonical_field::<OracleDisputeId>(
            super::read_aos_field(bytes, &mut offset, flags)?,
            flags,
        )?;
        let outcome = super::decode_aos_canonical_field::<OracleDisputeOutcome>(
            super::read_aos_field(bytes, &mut offset, flags)?,
            flags,
        )?;
        let notes = if offset < bytes.len() {
            super::decode_aos_slice_field::<String>(
                super::read_aos_field(bytes, &mut offset, flags)?,
                flags,
            )?
        } else {
            String::new()
        };
        if offset != bytes.len() {
            return Err(norito::core::Error::LengthMismatch);
        }
        norito::core::note_payload_access(bytes, offset);
        Ok((
            Self {
                dispute_id,
                outcome,
                notes,
            },
            offset,
        ))
    }
}

impl<'a> norito::core::DecodeFromSlice<'a> for ProposeOracleChange {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        let flags = oracle_decode_flags();
        if flags & norito::core::header_flags::PACKED_STRUCT != 0 {
            return super::decode_packed_instruction_payload::<Self>(bytes);
        }

        let mut offset = 0usize;
        let change_id = super::decode_aos_canonical_field::<OracleChangeId>(
            super::read_aos_field(bytes, &mut offset, flags)?,
            flags,
        )?;
        let feed = super::decode_aos_canonical_field::<FeedConfig>(
            super::read_aos_field(bytes, &mut offset, flags)?,
            flags,
        )?;
        let class = super::decode_aos_canonical_field::<OracleChangeClass>(
            super::read_aos_field(bytes, &mut offset, flags)?,
            flags,
        )?;
        let payload_hash = super::decode_aos_canonical_field::<Hash>(
            super::read_aos_field(bytes, &mut offset, flags)?,
            flags,
        )?;
        let evidence_hashes = if offset < bytes.len() {
            super::decode_aos_slice_field::<Vec<Hash>>(
                super::read_aos_field(bytes, &mut offset, flags)?,
                flags,
            )?
        } else {
            Vec::new()
        };
        if offset != bytes.len() {
            return Err(norito::core::Error::LengthMismatch);
        }
        norito::core::note_payload_access(bytes, offset);
        Ok((
            Self {
                change_id,
                feed,
                class,
                payload_hash,
                evidence_hashes,
            },
            offset,
        ))
    }
}

impl<'a> norito::core::DecodeFromSlice<'a> for VoteOracleChangeStage {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        let flags = oracle_decode_flags();
        if flags & norito::core::header_flags::PACKED_STRUCT != 0 {
            return super::decode_packed_instruction_payload::<Self>(bytes);
        }

        let mut offset = 0usize;
        let change_id = super::decode_aos_canonical_field::<OracleChangeId>(
            super::read_aos_field(bytes, &mut offset, flags)?,
            flags,
        )?;
        let stage = super::decode_aos_canonical_field::<OracleChangeStage>(
            super::read_aos_field(bytes, &mut offset, flags)?,
            flags,
        )?;
        let approve = super::decode_aos_canonical_field::<bool>(
            super::read_aos_field(bytes, &mut offset, flags)?,
            flags,
        )?;
        let evidence_hashes = if offset < bytes.len() {
            super::decode_aos_slice_field::<Vec<Hash>>(
                super::read_aos_field(bytes, &mut offset, flags)?,
                flags,
            )?
        } else {
            Vec::new()
        };
        if offset != bytes.len() {
            return Err(norito::core::Error::LengthMismatch);
        }
        norito::core::note_payload_access(bytes, offset);
        Ok((
            Self {
                change_id,
                stage,
                approve,
                evidence_hashes,
            },
            offset,
        ))
    }
}

impl<'a> norito::core::DecodeFromSlice<'a> for RollbackOracleChange {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        let flags = oracle_decode_flags();
        if flags & norito::core::header_flags::PACKED_STRUCT != 0 {
            return super::decode_packed_instruction_payload::<Self>(bytes);
        }

        let mut offset = 0usize;
        let change_id = super::decode_aos_canonical_field::<OracleChangeId>(
            super::read_aos_field(bytes, &mut offset, flags)?,
            flags,
        )?;
        let stage = super::decode_aos_canonical_field::<Option<OracleChangeStage>>(
            super::read_aos_field(bytes, &mut offset, flags)?,
            flags,
        )?;
        let reason = super::decode_aos_slice_field::<String>(
            super::read_aos_field(bytes, &mut offset, flags)?,
            flags,
        )?;
        if offset != bytes.len() {
            return Err(norito::core::Error::LengthMismatch);
        }
        norito::core::note_payload_access(bytes, offset);
        Ok((
            Self {
                change_id,
                stage,
                reason,
            },
            offset,
        ))
    }
}

impl<'a> norito::core::DecodeFromSlice<'a> for SubmitDefiOracleAttestation {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        let flags = oracle_decode_flags();
        if flags & norito::core::header_flags::PACKED_STRUCT != 0 {
            return super::decode_packed_instruction_payload::<Self>(bytes);
        }

        let mut offset = 0usize;
        let attestation = super::decode_aos_canonical_field::<DefiOracleAttestation>(
            super::read_aos_field(bytes, &mut offset, flags)?,
            flags,
        )?;
        if offset != bytes.len() {
            return Err(norito::core::Error::LengthMismatch);
        }
        norito::core::note_payload_access(bytes, offset);
        Ok((Self { attestation }, offset))
    }
}

impl<'a> norito::core::DecodeFromSlice<'a> for RecordTwitterBinding {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        let flags = oracle_decode_flags();
        if flags & norito::core::header_flags::PACKED_STRUCT != 0 {
            return super::decode_packed_instruction_payload::<Self>(bytes);
        }

        let mut offset = 0usize;
        let attestation = super::decode_aos_canonical_field::<TwitterBindingAttestation>(
            super::read_aos_field(bytes, &mut offset, flags)?,
            flags,
        )?;
        let feed_id = super::decode_aos_canonical_field::<FeedId>(
            super::read_aos_field(bytes, &mut offset, flags)?,
            flags,
        )?;
        if offset != bytes.len() {
            return Err(norito::core::Error::LengthMismatch);
        }
        norito::core::note_payload_access(bytes, offset);
        Ok((
            Self {
                attestation,
                feed_id,
            },
            offset,
        ))
    }
}

impl<'a> norito::core::DecodeFromSlice<'a> for RevokeTwitterBinding {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        let flags = oracle_decode_flags();
        if flags & norito::core::header_flags::PACKED_STRUCT != 0 {
            return super::decode_packed_instruction_payload::<Self>(bytes);
        }

        let mut offset = 0usize;
        let binding_hash = super::decode_aos_canonical_field::<KeyedHash>(
            super::read_aos_field(bytes, &mut offset, flags)?,
            flags,
        )?;
        let reason = super::decode_aos_slice_field::<String>(
            super::read_aos_field(bytes, &mut offset, flags)?,
            flags,
        )?;
        if offset != bytes.len() {
            return Err(norito::core::Error::LengthMismatch);
        }
        norito::core::note_payload_access(bytes, offset);
        Ok((
            Self {
                binding_hash,
                reason,
            },
            offset,
        ))
    }
}

#[cfg(test)]
mod tests {
    use norito::core::DecodeFromSlice;

    use super::*;
    use crate::{
        nexus::UniversalAccountId,
        oracle::{
            DefiOracleAttestationKey, DefiOracleAttestationSource, FeedConfigVersion,
            TwitterBindingStatus, kits,
        },
    };

    fn feed() -> FeedConfig {
        kits::price_xor_usd().feed_config
    }

    fn observation() -> Observation {
        kits::price_xor_usd()
            .observations
            .first()
            .expect("observation fixture")
            .clone()
    }

    fn request_hash() -> Hash {
        kits::price_xor_usd().connector_request.request_hash()
    }

    fn evidence_hash() -> Hash {
        Hash::new(b"oracle-evidence")
    }

    fn change_id() -> OracleChangeId {
        OracleChangeId::from(Hash::new(b"oracle-change"))
    }

    fn twitter_attestation() -> TwitterBindingAttestation {
        let kit = kits::twitter_follow_binding();
        TwitterBindingAttestation {
            binding_hash: KeyedHash::new("pepper-social-v1", b"pepper", b"twitter_user_123"),
            uaid: UniversalAccountId::from_hash(Hash::new(b"twitter-uaid")),
            status: TwitterBindingStatus::Following,
            tweet_id: Some("tweet-42".to_owned()),
            challenge_hash: Some(Hash::new(b"challenge")),
            expires_at_ms: 1_700_000_600_000,
            observed_at_ms: 1_700_000_000_000,
            request_hash: kit.connector_request.request_hash(),
            slot: kit.connector_request.slot,
            feed_config_version: FeedConfigVersion(1),
        }
    }

    fn defi_attestation() -> DefiOracleAttestation {
        DefiOracleAttestation {
            key: DefiOracleAttestationKey::new(1, 42),
            provider: observation().body.provider_id,
            oracle_slot: 11,
            status_flags: 0,
            attestation_hash: 17,
            oracle_payload: br#"{"domain":1,"market_id":42,"mark_price_bps":10000,"index_price_bps":10000,"confidence_bps":5,"oracle_slot":11,"status_flags":0,"attestation_hash":17}"#
                .to_vec(),
            oracle_signature: vec![7; 64],
            signer_public_key: vec![9; 32],
            oracle_scheme: 1,
            source_events: vec![DefiOracleAttestationSource {
                feed_id: feed().feed_id,
                slot: 11,
                request_hash: request_hash(),
                field: "mark_price_bps".to_owned(),
            }],
        }
    }

    fn assert_slice_roundtrip<T>(value: T)
    where
        T: Clone + PartialEq + core::fmt::Debug + norito::codec::Encode,
        for<'a> T: DecodeFromSlice<'a>,
    {
        let bytes = value.encode();
        let (decoded, used) = T::decode_from_slice(&bytes).expect("decode from slice");
        assert_eq!(used, bytes.len());
        assert_eq!(decoded, value);
    }

    fn assert_framed_rejects_truncated<T>(value: &T)
    where
        T: norito::core::NoritoSerialize,
        for<'de> T: norito::core::NoritoDeserialize<'de>,
    {
        let bytes = norito::to_bytes(value).expect("encode oracle instruction");
        let truncated_lengths = [0_usize, 1, bytes.len() / 2, bytes.len().saturating_sub(1)];
        for len in truncated_lengths {
            assert!(
                norito::decode_from_bytes::<T>(&bytes[..len]).is_err(),
                "truncated oracle instruction frame of length {len} must reject"
            );
        }
    }

    fn assert_registry_decodes<T>(registry: &crate::isi::InstructionRegistry, value: T)
    where
        T: crate::isi::Instruction
            + norito::codec::Encode
            + 'static
            + norito::core::NoritoSerialize,
        for<'de> T: norito::core::NoritoDeserialize<'de>,
    {
        let wire_id = std::any::type_name::<T>();
        let (payload, flags) = norito::codec::encode_with_header_flags(&value);
        let framed =
            norito::core::frame_bare_with_header_flags::<T>(&payload, flags).expect("frame");
        let decoded = crate::isi::InstructionRegistry::decode(registry, wire_id, &framed)
            .expect("registered")
            .expect("decode");
        assert_eq!(crate::isi::Instruction::dyn_encode(&*decoded), payload);
    }

    fn sample_values() -> (
        RegisterOracleFeed,
        SubmitOracleObservation,
        AggregateOracleFeed,
        OpenOracleDispute,
        ResolveOracleDispute,
        ProposeOracleChange,
        VoteOracleChangeStage,
        RollbackOracleChange,
        SubmitDefiOracleAttestation,
        RecordTwitterBinding,
        RevokeTwitterBinding,
    ) {
        let feed = feed();
        let observation = observation();
        let attestation = twitter_attestation();
        (
            RegisterOracleFeed { feed: feed.clone() },
            SubmitOracleObservation {
                observation: observation.clone(),
            },
            AggregateOracleFeed {
                feed_id: feed.feed_id.clone(),
                slot: observation.body.slot,
                request_hash: request_hash(),
                evidence_hashes: vec![evidence_hash()],
            },
            OpenOracleDispute {
                feed_id: feed.feed_id.clone(),
                slot: observation.body.slot,
                request_hash: request_hash(),
                target: observation.body.provider_id.clone(),
                bond: Some(Numeric::new(10_u128, 0)),
                evidence_hashes: vec![evidence_hash()],
                reason: "outlier".to_owned(),
            },
            ResolveOracleDispute {
                dispute_id: OracleDisputeId(7),
                outcome: OracleDisputeOutcome::Reduced,
                notes: "reduced penalty".to_owned(),
            },
            ProposeOracleChange {
                change_id: change_id(),
                feed: feed.clone(),
                class: OracleChangeClass::High,
                payload_hash: Hash::new(b"oracle-change-payload"),
                evidence_hashes: vec![evidence_hash()],
            },
            VoteOracleChangeStage {
                change_id: change_id(),
                stage: OracleChangeStage::TechnicalAudit,
                approve: true,
                evidence_hashes: vec![evidence_hash()],
            },
            RollbackOracleChange {
                change_id: change_id(),
                stage: Some(OracleChangeStage::CopReview),
                reason: "insufficient evidence".to_owned(),
            },
            SubmitDefiOracleAttestation {
                attestation: defi_attestation(),
            },
            RecordTwitterBinding {
                attestation: attestation.clone(),
                feed_id: kits::twitter_follow_binding().feed_config.feed_id,
            },
            RevokeTwitterBinding {
                binding_hash: attestation.binding_hash,
                reason: "expired".to_owned(),
            },
        )
    }

    #[test]
    fn oracle_decode_from_slice_roundtrips() {
        let (
            register,
            submit,
            aggregate,
            open_dispute,
            resolve_dispute,
            propose,
            vote,
            rollback,
            submit_defi,
            record_binding,
            revoke_binding,
        ) = sample_values();

        assert_slice_roundtrip(register);
        assert_slice_roundtrip(submit);
        assert_slice_roundtrip(aggregate);
        assert_slice_roundtrip(open_dispute);
        assert_slice_roundtrip(resolve_dispute);
        assert_slice_roundtrip(propose);
        assert_slice_roundtrip(vote);
        assert_slice_roundtrip(rollback);
        assert_slice_roundtrip(submit_defi);
        assert_slice_roundtrip(record_binding);
        assert_slice_roundtrip(revoke_binding);
    }

    #[test]
    fn oracle_framed_decode_rejects_truncated_payloads() {
        let (
            register,
            submit,
            aggregate,
            open_dispute,
            resolve_dispute,
            propose,
            vote,
            rollback,
            submit_defi,
            record_binding,
            revoke_binding,
        ) = sample_values();

        assert_framed_rejects_truncated(&register);
        assert_framed_rejects_truncated(&submit);
        assert_framed_rejects_truncated(&aggregate);
        assert_framed_rejects_truncated(&open_dispute);
        assert_framed_rejects_truncated(&resolve_dispute);
        assert_framed_rejects_truncated(&propose);
        assert_framed_rejects_truncated(&vote);
        assert_framed_rejects_truncated(&rollback);
        assert_framed_rejects_truncated(&submit_defi);
        assert_framed_rejects_truncated(&record_binding);
        assert_framed_rejects_truncated(&revoke_binding);
    }

    #[test]
    fn oracle_registry_decodes_type_names() {
        let registry = crate::isi::InstructionRegistry::new()
            .register_slice::<RegisterOracleFeed>()
            .register_slice::<SubmitOracleObservation>()
            .register_slice::<AggregateOracleFeed>()
            .register_slice::<OpenOracleDispute>()
            .register_slice::<ResolveOracleDispute>()
            .register_slice::<ProposeOracleChange>()
            .register_slice::<VoteOracleChangeStage>()
            .register_slice::<RollbackOracleChange>()
            .register_slice::<SubmitDefiOracleAttestation>()
            .register_slice::<RecordTwitterBinding>()
            .register_slice::<RevokeTwitterBinding>();
        let (
            register,
            submit,
            aggregate,
            open_dispute,
            resolve_dispute,
            propose,
            vote,
            rollback,
            submit_defi,
            record_binding,
            revoke_binding,
        ) = sample_values();

        assert_registry_decodes(&registry, register);
        assert_registry_decodes(&registry, submit);
        assert_registry_decodes(&registry, aggregate);
        assert_registry_decodes(&registry, open_dispute);
        assert_registry_decodes(&registry, resolve_dispute);
        assert_registry_decodes(&registry, propose);
        assert_registry_decodes(&registry, vote);
        assert_registry_decodes(&registry, rollback);
        assert_registry_decodes(&registry, submit_defi);
        assert_registry_decodes(&registry, record_binding);
        assert_registry_decodes(&registry, revoke_binding);
    }

    #[test]
    fn default_registry_encodes_defi_attestation_instruction_box() {
        crate::isi::set_instruction_registry(crate::instruction_registry::default());
        let instruction = crate::isi::InstructionBox::from(SubmitDefiOracleAttestation {
            attestation: defi_attestation(),
        });

        norito::to_bytes(&instruction)
            .expect("default registry should encode SubmitDefiOracleAttestation");
    }
}
