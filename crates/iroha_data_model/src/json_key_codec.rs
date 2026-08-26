use iroha_crypto::Hash;
use mv::json::JsonKeyCodec;
use norito::json;
macro_rules! impl_id_key_codec {
    ($($ty:path),+ $(,)?) => {
        $(
            impl JsonKeyCodec for $ty {
                fn encode_json_key(&self, out: &mut String) {
                    json::write_json_string(&self.to_string(), out);
                }
                fn decode_json_key(encoded: &str) -> Result<Self, json::Error> {
                    encoded
                        .parse::<$ty>()
                        .map_err(|err| json::Error::Message(err.to_string()))
                }
            }
        )+
    };
}
macro_rules! impl_nested_json_key_codec {
    ($($ty:path),+ $(,)?) => {
        $(
            impl JsonKeyCodec for $ty {
                fn encode_json_key(&self, out: &mut String) {
                    let mut encoded = String::new();
                    norito::json::JsonSerialize::json_serialize(self, &mut encoded);
                    json::write_json_string(&encoded, out);
                }
                fn decode_json_key(encoded: &str) -> Result<Self, json::Error> {
                    let mut parser = json::Parser::new(encoded);
                    norito::json::JsonDeserialize::json_deserialize(&mut parser)
                }
            }
        )+
    };
}
impl_id_key_codec!(
    crate::asset::AssetDefinitionId,
    crate::asset::AssetId,
    crate::governance::types::GovernanceAttemptId,
    crate::governance::types::BallotAttemptId,
    crate::governance::types::TleKeySessionId,
    crate::nft::NftId,
    crate::role::RoleId,
    crate::trigger::TriggerId,
    crate::oracle::FeedId,
    crate::proof::ProofId,
    crate::isi::settlement::SettlementId,
);
// Musubi uses structural, versioned keys whose complete typed JSON form is
// embedded into the surrounding storage object's string key. This avoids
// delimiter ambiguity for nested package/account identities while keeping
// snapshot ordering identical to the underlying Rust `Ord` implementation.
impl_nested_json_key_codec!(
    crate::musubi::MusubiNamespaceV1,
    crate::musubi::MusubiPackageIdV1,
    crate::musubi::MusubiPackageSelectorV1,
    crate::musubi::MusubiPackageMemberKeyV1,
    crate::musubi::MusubiMaintainerDirectoryKeyV1,
    crate::musubi::MusubiInviteIdV1,
    crate::musubi::MusubiReleaseIdV1,
    crate::musubi::ArchiveId,
    crate::musubi::MusubiArchiveLocationKeyV1,
    crate::musubi::MusubiProviderLocationKeyV1,
    crate::musubi::MusubiProviderBundleAttestationKeyV1,
    crate::musubi::MusubiAliasNameV1,
    crate::musubi::MusubiAliasHistoryKeyV1,
);
// AXT budget families use their complete typed issuer-signed identity as the
// consensus storage key. Require the one canonical Norito JSON spelling so
// two snapshot keys cannot decode to the same budget family.
impl JsonKeyCodec for crate::nexus::AxtHandleBudgetKey {
    fn encode_json_key(&self, out: &mut String) {
        let mut encoded = String::new();
        norito::json::JsonSerialize::json_serialize(self, &mut encoded);
        json::write_json_string(&encoded, out);
    }

    fn decode_json_key(encoded: &str) -> Result<Self, json::Error> {
        let mut parser = json::Parser::new(encoded);
        let decoded = norito::json::JsonDeserialize::json_deserialize(&mut parser)?;
        let mut canonical = String::new();
        norito::json::JsonSerialize::json_serialize(&decoded, &mut canonical);
        if canonical != encoded {
            return Err(json::Error::Message(
                "AXT handle budget key must use canonical JSON".into(),
            ));
        }
        Ok(decoded)
    }
}
// Replay-ledger keys are consensus snapshot identities as well. Apply the
// same exact-spelling rule as budget keys so aliases cannot split replay state.
impl JsonKeyCodec for crate::nexus::AxtHandleReplayKey {
    fn encode_json_key(&self, out: &mut String) {
        let mut encoded = String::new();
        norito::json::JsonSerialize::json_serialize(self, &mut encoded);
        json::write_json_string(&encoded, out);
    }

    fn decode_json_key(encoded: &str) -> Result<Self, json::Error> {
        let mut parser = json::Parser::new(encoded);
        let decoded: Self = norito::json::JsonDeserialize::json_deserialize(&mut parser)?;
        let mut canonical = String::new();
        norito::json::JsonSerialize::json_serialize(&decoded, &mut canonical);
        if canonical != encoded {
            return Err(json::Error::Message(
                "AXT handle replay key must use canonical JSON".into(),
            ));
        }
        decoded.validate().map_err(|error| {
            json::Error::Message(format!("invalid AXT handle replay key: {error}"))
        })?;
        Ok(decoded)
    }
}
impl JsonKeyCodec for crate::domain::DomainId {
    fn encode_json_key(&self, out: &mut String) {
        json::write_json_string(&self.to_string(), out);
    }
    fn decode_json_key(encoded: &str) -> Result<Self, json::Error> {
        crate::domain::DomainId::parse_fully_qualified(encoded)
            .map_err(|err| json::Error::Message(err.to_string()))
    }
}
impl JsonKeyCodec for crate::account::AccountId {
    fn encode_json_key(&self, out: &mut String) {
        json::write_json_string(&self.to_string(), out);
    }
    fn decode_json_key(encoded: &str) -> Result<Self, json::Error> {
        crate::account::AccountId::parse_encoded(encoded)
            .map_err(|err| json::Error::Message(err.to_string()))
    }
}
impl JsonKeyCodec for crate::name::Name {
    fn encode_json_key(&self, out: &mut String) {
        json::write_json_string(self.as_ref(), out);
    }
    fn decode_json_key(encoded: &str) -> Result<Self, json::Error> {
        encoded
            .parse::<crate::name::Name>()
            .map_err(|err| json::Error::Message(err.reason.into()))
    }
}
impl JsonKeyCodec for crate::state_path::StatePath {
    fn encode_json_key(&self, out: &mut String) {
        json::write_json_string(self.as_ref(), out);
    }
    fn decode_json_key(encoded: &str) -> Result<Self, json::Error> {
        encoded
            .parse::<crate::state_path::StatePath>()
            .map_err(|err| json::Error::Message(err.reason.into()))
    }
}
impl JsonKeyCodec for crate::proof::VerifyingKeyId {
    fn encode_json_key(&self, out: &mut String) {
        let mut buf = String::new();
        norito::json::JsonSerialize::json_serialize(self, &mut buf);
        json::write_json_string(&buf, out);
    }
    fn decode_json_key(encoded: &str) -> Result<Self, json::Error> {
        let mut parser = json::Parser::new(encoded);
        norito::json::JsonDeserialize::json_deserialize(&mut parser)
    }
}
impl JsonKeyCodec for crate::runtime::RuntimeUpgradeId {
    fn encode_json_key(&self, out: &mut String) {
        <[u8; 32] as JsonKeyCodec>::encode_json_key(&self.0, out);
    }
    fn decode_json_key(encoded: &str) -> Result<Self, json::Error> {
        <[u8; 32] as JsonKeyCodec>::decode_json_key(encoded).map(Self)
    }
}
impl JsonKeyCodec for crate::escrow::EscrowId {
    fn encode_json_key(&self, out: &mut String) {
        self.as_hash().encode_json_key(out);
    }
    fn decode_json_key(encoded: &str) -> Result<Self, json::Error> {
        <Hash as JsonKeyCodec>::decode_json_key(encoded).map(Self::new)
    }
}
impl JsonKeyCodec for crate::account::rekey::AccountAlias {
    fn encode_json_key(&self, out: &mut String) {
        let mut buf = String::new();
        norito::json::JsonSerialize::json_serialize(self, &mut buf);
        json::write_json_string(&buf, out);
    }
    fn decode_json_key(encoded: &str) -> Result<Self, json::Error> {
        let mut parser = json::Parser::new(encoded);
        norito::json::JsonDeserialize::json_deserialize(&mut parser)
    }
}
impl JsonKeyCodec for crate::smart_contract::ContractAlias {
    fn encode_json_key(&self, out: &mut String) {
        norito::json::write_json_string(self.as_ref(), out);
    }
    fn decode_json_key(encoded: &str) -> Result<Self, json::Error> {
        encoded
            .parse()
            .map_err(|err: crate::ParseError| json::Error::Message(err.reason.into()))
    }
}
impl JsonKeyCodec for crate::smart_contract::ContractAddress {
    fn encode_json_key(&self, out: &mut String) {
        json::write_json_string(self.as_ref(), out);
    }
    fn decode_json_key(encoded: &str) -> Result<Self, json::Error> {
        encoded
            .parse()
            .map_err(|err: crate::smart_contract::ContractAddressError| {
                json::Error::Message(err.to_string())
            })
    }
}
impl JsonKeyCodec for crate::confidential::ConfidentialParamsId {
    fn encode_json_key(&self, out: &mut String) {
        json::write_json_string(&self.to_string(), out);
    }
    fn decode_json_key(encoded: &str) -> Result<Self, json::Error> {
        encoded
            .parse::<u32>()
            .map(crate::confidential::ConfidentialParamsId::from)
            .map_err(|err| json::Error::Message(err.to_string()))
    }
}
impl JsonKeyCodec for crate::sorafs::capacity::ProviderId {
    fn encode_json_key(&self, out: &mut String) {
        <[u8; 32] as JsonKeyCodec>::encode_json_key(&self.0, out);
    }
    fn decode_json_key(encoded: &str) -> Result<Self, json::Error> {
        <[u8; 32] as JsonKeyCodec>::decode_json_key(encoded).map(Self)
    }
}
impl JsonKeyCodec for crate::sorafs::pin_registry::ReplicationOrderId {
    fn encode_json_key(&self, out: &mut String) {
        <[u8; 32] as JsonKeyCodec>::encode_json_key(self.as_bytes(), out);
    }
    fn decode_json_key(encoded: &str) -> Result<Self, json::Error> {
        <[u8; 32] as JsonKeyCodec>::decode_json_key(encoded).map(Self::new)
    }
}
impl JsonKeyCodec for crate::sorafs::pin_registry::ManifestAliasId {
    fn encode_json_key(&self, out: &mut String) {
        json::write_json_string(&self.as_label(), out);
    }
    fn decode_json_key(encoded: &str) -> Result<Self, json::Error> {
        let (namespace, name) = encoded
            .split_once('/')
            .ok_or_else(|| json::Error::Message("invalid manifest alias key".into()))?;
        Ok(Self::new(namespace.to_owned(), name.to_owned()))
    }
}
impl JsonKeyCodec for crate::oracle::OracleDisputeId {
    fn encode_json_key(&self, out: &mut String) {
        <u64 as JsonKeyCodec>::encode_json_key(&self.0, out);
    }
    fn decode_json_key(encoded: &str) -> Result<Self, json::Error> {
        <u64 as JsonKeyCodec>::decode_json_key(encoded).map(Self)
    }
}
impl JsonKeyCodec for crate::oracle::OracleProviderKey {
    fn encode_json_key(&self, out: &mut String) {
        let mut buf = String::new();
        norito::json::JsonSerialize::json_serialize(self, &mut buf);
        json::write_json_string(&buf, out);
    }
    fn decode_json_key(encoded: &str) -> Result<Self, json::Error> {
        let mut parser = json::Parser::new(encoded);
        norito::json::JsonDeserialize::json_deserialize(&mut parser)
    }
}
impl JsonKeyCodec for crate::oracle::DefiOracleAttestationKey {
    fn encode_json_key(&self, out: &mut String) {
        let mut buf = String::new();
        norito::json::JsonSerialize::json_serialize(self, &mut buf);
        json::write_json_string(&buf, out);
    }
    fn decode_json_key(encoded: &str) -> Result<Self, json::Error> {
        let mut parser = json::Parser::new(encoded);
        norito::json::JsonDeserialize::json_deserialize(&mut parser)
    }
}
impl JsonKeyCodec for crate::oracle::OracleChangeId {
    fn encode_json_key(&self, out: &mut String) {
        self.0.encode_json_key(out);
    }
    fn decode_json_key(encoded: &str) -> Result<Self, json::Error> {
        <Hash as JsonKeyCodec>::decode_json_key(encoded).map(Self)
    }
}
impl JsonKeyCodec for crate::nexus::DataSpaceId {
    fn encode_json_key(&self, out: &mut String) {
        <u64 as JsonKeyCodec>::encode_json_key(&self.as_u64(), out);
    }
    fn decode_json_key(encoded: &str) -> Result<Self, json::Error> {
        <u64 as JsonKeyCodec>::decode_json_key(encoded).map(Self::from)
    }
}
impl JsonKeyCodec for crate::nexus::LaneId {
    fn encode_json_key(&self, out: &mut String) {
        <u64 as JsonKeyCodec>::encode_json_key(&u64::from(self.as_u32()), out);
    }
    fn decode_json_key(encoded: &str) -> Result<Self, json::Error> {
        <u64 as JsonKeyCodec>::decode_json_key(encoded).and_then(|value| {
            u32::try_from(value)
                .map(crate::nexus::LaneId::new)
                .map_err(|_| json::Error::Message("lane id out of range".into()))
        })
    }
}
impl JsonKeyCodec for crate::nexus::UniversalAccountId {
    fn encode_json_key(&self, out: &mut String) {
        <Hash as JsonKeyCodec>::encode_json_key(self.as_hash(), out);
    }
    fn decode_json_key(encoded: &str) -> Result<Self, json::Error> {
        <Hash as JsonKeyCodec>::decode_json_key(encoded)
            .map(crate::nexus::UniversalAccountId::from_hash)
    }
}
impl JsonKeyCodec for crate::nexus::FeeSponsorProgramId {
    fn encode_json_key(&self, out: &mut String) {
        json::write_json_string(&self.to_string(), out);
    }
    fn decode_json_key(encoded: &str) -> Result<Self, json::Error> {
        encoded
            .parse::<crate::nexus::FeeSponsorProgramId>()
            .map_err(|err| json::Error::Message(err.to_string()))
    }
}
macro_rules! impl_fee_sponsor_struct_key_codec {
    ($($ty:path),+ $(,)?) => {
        $(
            impl JsonKeyCodec for $ty {
                fn encode_json_key(&self, out: &mut String) {
                    let mut encoded = String::new();
                    norito::json::JsonSerialize::json_serialize(self, &mut encoded);
                    json::write_json_string(&encoded, out);
                }
                fn decode_json_key(encoded: &str) -> Result<Self, json::Error> {
                    let mut parser = json::Parser::new(encoded);
                    norito::json::JsonDeserialize::json_deserialize(&mut parser)
                }
            }
        )+
    };
}
impl_fee_sponsor_struct_key_codec!(
    crate::nexus::FeeSponsorProgramRevisionKey,
    crate::nexus::FeeSponsorEnrollmentKey,
    crate::nexus::FeeSponsorVaultKey,
    crate::nexus::FeeSponsorBudgetCounterKey,
);
impl JsonKeyCodec for crate::account::OpaqueAccountId {
    fn encode_json_key(&self, out: &mut String) {
        <Hash as JsonKeyCodec>::encode_json_key(self.as_hash(), out);
    }
    fn decode_json_key(encoded: &str) -> Result<Self, json::Error> {
        <Hash as JsonKeyCodec>::decode_json_key(encoded).map(crate::account::OpaqueAccountId::from)
    }
}
impl JsonKeyCodec for crate::identifier::IdentifierPolicyId {
    fn encode_json_key(&self, out: &mut String) {
        json::write_json_string(&self.to_string(), out);
    }
    fn decode_json_key(encoded: &str) -> Result<Self, json::Error> {
        encoded
            .parse::<crate::identifier::IdentifierPolicyId>()
            .map_err(|err| json::Error::Message(err.to_string()))
    }
}
impl JsonKeyCodec for crate::ram_lfe::RamLfeProgramId {
    fn encode_json_key(&self, out: &mut String) {
        json::write_json_string(&self.to_string(), out);
    }
    fn decode_json_key(encoded: &str) -> Result<Self, json::Error> {
        encoded
            .parse::<crate::ram_lfe::RamLfeProgramId>()
            .map_err(|err| json::Error::Message(err.to_string()))
    }
}
impl JsonKeyCodec for crate::bridge::sccp::SccpOutboundMessageKeyV1 {
    fn encode_json_key(&self, out: &mut String) {
        let encoded = format!(
            "sccp-outbound-v1:{}:{}:{}",
            self.lane.source.profile_key(),
            self.lane.target.profile_key(),
            hex::encode(self.message_id)
        );
        json::write_json_string(&encoded, out);
    }
    fn decode_json_key(encoded: &str) -> Result<Self, json::Error> {
        const PREFIX: &str = "sccp-outbound-v1";
        let mut parts = encoded.split(':');
        if parts.next() != Some(PREFIX) {
            return Err(json::Error::Message(
                "SCCP outbound message key must use the canonical V1 prefix".into(),
            ));
        }
        let source = parts
            .next()
            .and_then(crate::bridge::sccp::SccpNetworkV1::from_profile_key)
            .ok_or_else(|| {
                json::Error::Message("unknown or non-canonical SCCP source profile".into())
            })?;
        let target = parts
            .next()
            .and_then(crate::bridge::sccp::SccpNetworkV1::from_profile_key)
            .ok_or_else(|| {
                json::Error::Message("unknown or non-canonical SCCP target profile".into())
            })?;
        let message_id_hex = parts
            .next()
            .ok_or_else(|| json::Error::Message("missing SCCP message id".into()))?;
        if parts.next().is_some() {
            return Err(json::Error::Message(
                "too many SCCP outbound message key parts".into(),
            ));
        }
        if message_id_hex.len() != 64
            || !message_id_hex
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            return Err(json::Error::Message(
                "SCCP message id must be exactly 64 lowercase hexadecimal characters".into(),
            ));
        }
        let message_id = hex::decode(message_id_hex)
            .map_err(|err| json::Error::Message(err.to_string()))?
            .try_into()
            .map_err(|_| json::Error::Message("SCCP message id must be 32 bytes".into()))?;
        crate::bridge::sccp::SccpOutboundMessageKeyV1::new(
            crate::bridge::sccp::SccpLaneIdV1 { source, target },
            message_id,
        )
        .ok_or_else(|| {
            json::Error::Message(
                "SCCP outbound key must contain a SORA-to-external lane and nonzero message id"
                    .into(),
            )
        })
    }
}
impl JsonKeyCodec for crate::bridge::sccp::SccpOutboundMessageIndexKeyV1 {
    fn encode_json_key(&self, out: &mut String) {
        let encoded = format!(
            "sccp-outbound-index-v1:{}:{}:{}:{}:{}",
            self.recorded_at_height,
            self.commitment_index,
            self.lane.source.profile_key(),
            self.lane.target.profile_key(),
            hex::encode(self.message_id)
        );
        json::write_json_string(&encoded, out);
    }
    fn decode_json_key(encoded: &str) -> Result<Self, json::Error> {
        const PREFIX: &str = "sccp-outbound-index-v1";
        let mut parts = encoded.split(':');
        if parts.next() != Some(PREFIX) {
            return Err(json::Error::Message(
                "SCCP outbound index key must use the canonical V1 prefix".into(),
            ));
        }
        let height_text = parts
            .next()
            .ok_or_else(|| json::Error::Message("missing SCCP outbound index height".into()))?;
        if height_text.is_empty()
            || (height_text.len() > 1 && height_text.starts_with('0'))
            || !height_text.bytes().all(|byte| byte.is_ascii_digit())
        {
            return Err(json::Error::Message(
                "SCCP outbound index height must be canonical unsigned decimal".into(),
            ));
        }
        let recorded_at_height = height_text
            .parse::<u64>()
            .map_err(|err| json::Error::Message(err.to_string()))?;
        let commitment_index_text = parts
            .next()
            .ok_or_else(|| json::Error::Message("missing SCCP outbound commitment index".into()))?;
        if commitment_index_text.is_empty()
            || (commitment_index_text.len() > 1 && commitment_index_text.starts_with('0'))
            || !commitment_index_text
                .bytes()
                .all(|byte| byte.is_ascii_digit())
        {
            return Err(json::Error::Message(
                "SCCP outbound commitment index must be canonical unsigned decimal".into(),
            ));
        }
        let commitment_index = commitment_index_text
            .parse::<u32>()
            .map_err(|err| json::Error::Message(err.to_string()))?;
        let source = parts
            .next()
            .and_then(crate::bridge::sccp::SccpNetworkV1::from_profile_key)
            .ok_or_else(|| {
                json::Error::Message("unknown or non-canonical SCCP source profile".into())
            })?;
        let target = parts
            .next()
            .and_then(crate::bridge::sccp::SccpNetworkV1::from_profile_key)
            .ok_or_else(|| {
                json::Error::Message("unknown or non-canonical SCCP target profile".into())
            })?;
        let message_id_hex = parts
            .next()
            .ok_or_else(|| json::Error::Message("missing SCCP message id".into()))?;
        if parts.next().is_some() {
            return Err(json::Error::Message(
                "too many SCCP outbound index key parts".into(),
            ));
        }
        if message_id_hex.len() != 64
            || !message_id_hex
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            return Err(json::Error::Message(
                "SCCP message id must be exactly 64 lowercase hexadecimal characters".into(),
            ));
        }
        let message_id = hex::decode(message_id_hex)
            .map_err(|err| json::Error::Message(err.to_string()))?
            .try_into()
            .map_err(|_| json::Error::Message("SCCP message id must be 32 bytes".into()))?;
        let key = crate::bridge::sccp::SccpOutboundMessageIndexKeyV1 {
            recorded_at_height,
            commitment_index,
            lane: crate::bridge::sccp::SccpLaneIdV1 { source, target },
            message_id,
        };
        key.is_well_formed().then_some(key).ok_or_else(|| {
            json::Error::Message(
                "SCCP outbound index must contain a positive height, SORA-to-external lane, and nonzero message id"
                    .into(),
            )
        })
    }
}
impl JsonKeyCodec for crate::bridge::sccp::SccpInboundMessageKeyV1 {
    fn encode_json_key(&self, out: &mut String) {
        let encoded = format!(
            "sccp-inbound-v1:{}:{}:{}",
            self.lane.source.profile_key(),
            self.lane.target.profile_key(),
            hex::encode(self.message_id)
        );
        json::write_json_string(&encoded, out);
    }
    fn decode_json_key(encoded: &str) -> Result<Self, json::Error> {
        const PREFIX: &str = "sccp-inbound-v1";
        let mut parts = encoded.split(':');
        if parts.next() != Some(PREFIX) {
            return Err(json::Error::Message(
                "SCCP inbound message key must use the canonical V1 prefix".into(),
            ));
        }
        let source = parts
            .next()
            .and_then(crate::bridge::sccp::SccpNetworkV1::from_profile_key)
            .ok_or_else(|| {
                json::Error::Message("unknown or non-canonical SCCP source profile".into())
            })?;
        let target = parts
            .next()
            .and_then(crate::bridge::sccp::SccpNetworkV1::from_profile_key)
            .ok_or_else(|| {
                json::Error::Message("unknown or non-canonical SCCP target profile".into())
            })?;
        let message_id_hex = parts
            .next()
            .ok_or_else(|| json::Error::Message("missing SCCP message id".into()))?;
        if parts.next().is_some() {
            return Err(json::Error::Message(
                "too many SCCP inbound message key parts".into(),
            ));
        }
        if message_id_hex.len() != 64
            || !message_id_hex
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            return Err(json::Error::Message(
                "SCCP message id must be exactly 64 lowercase hexadecimal characters".into(),
            ));
        }
        let message_id = hex::decode(message_id_hex)
            .map_err(|err| json::Error::Message(err.to_string()))?
            .try_into()
            .map_err(|_| json::Error::Message("SCCP message id must be 32 bytes".into()))?;
        crate::bridge::sccp::SccpInboundMessageKeyV1::new(
            crate::bridge::sccp::SccpLaneIdV1 { source, target },
            message_id,
        )
        .ok_or_else(|| {
            json::Error::Message(
                "SCCP inbound key must contain an external-to-SORA lane and nonzero message id"
                    .into(),
            )
        })
    }
}
impl JsonKeyCodec for crate::bridge::sccp::SccpInboundAnchorHighWaterKeyV1 {
    fn encode_json_key(&self, out: &mut String) {
        let encoded = format!(
            "sccp-inbound-anchor-high-water-v1:{}:{}:{}",
            self.lane.source.profile_key(),
            self.lane.target.profile_key(),
            hex::encode(self.anchor_hash)
        );
        json::write_json_string(&encoded, out);
    }
    fn decode_json_key(encoded: &str) -> Result<Self, json::Error> {
        const PREFIX: &str = "sccp-inbound-anchor-high-water-v1";
        let mut parts = encoded.split(':');
        if parts.next() != Some(PREFIX) {
            return Err(json::Error::Message(
                "SCCP inbound anchor high-water key must use the canonical V1 prefix".into(),
            ));
        }
        let source = parts
            .next()
            .and_then(crate::bridge::sccp::SccpNetworkV1::from_profile_key)
            .ok_or_else(|| {
                json::Error::Message("unknown or non-canonical SCCP source profile".into())
            })?;
        let target = parts
            .next()
            .and_then(crate::bridge::sccp::SccpNetworkV1::from_profile_key)
            .ok_or_else(|| {
                json::Error::Message("unknown or non-canonical SCCP target profile".into())
            })?;
        let anchor_hash_hex = parts
            .next()
            .ok_or_else(|| json::Error::Message("missing SCCP anchor hash".into()))?;
        if parts.next().is_some() {
            return Err(json::Error::Message(
                "too many SCCP inbound anchor high-water key parts".into(),
            ));
        }
        if anchor_hash_hex.len() != 64
            || !anchor_hash_hex
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            return Err(json::Error::Message(
                "SCCP anchor hash must be exactly 64 lowercase hexadecimal characters".into(),
            ));
        }
        let anchor_hash = hex::decode(anchor_hash_hex)
            .map_err(|err| json::Error::Message(err.to_string()))?
            .try_into()
            .map_err(|_| json::Error::Message("SCCP anchor hash must be 32 bytes".into()))?;
        crate::bridge::sccp::SccpInboundAnchorHighWaterKeyV1::new(
            crate::bridge::sccp::SccpLaneIdV1 { source, target },
            anchor_hash,
        )
        .ok_or_else(|| {
            json::Error::Message(
                "SCCP inbound anchor high-water key must contain an external-to-SORA lane and nonzero anchor hash"
                    .into(),
            )
        })
    }
}
#[cfg(test)]
mod tests {
    use crate::account::AccountId;
    use crate::bridge::sccp::{
        SccpInboundAnchorHighWaterKeyV1, SccpInboundMessageKeyV1, SccpLaneIdV1, SccpNetworkV1,
        SccpOutboundMessageIndexKeyV1, SccpOutboundMessageKeyV1,
    };
    use crate::{
        governance::types::{BallotAttemptId, GovernanceAttemptId, TleKeySessionId},
        musubi::{
            ArchiveId, MusubiInviteIdV1, MusubiMaintainerDirectoryKeyV1, MusubiPackageIdV1,
            MusubiPackageScopeV1, MusubiProviderBundleAttestationKeyV1,
        },
        nexus::DataSpaceId,
        sorafs::{capacity::ProviderId, pin_registry::ReplicationOrderId},
    };
    use iroha_crypto::KeyPair;
    use mv::json::JsonKeyCodec;
    use norito::json::Parser;
    fn checked_random_keypair() -> KeyPair {
        KeyPair::try_random().expect("generate checked JSON key codec fixture keypair")
    }
    #[test]
    fn governance_hash_ids_are_canonical_json_storage_keys() {
        fn check<T>(key: T)
        where
            T: JsonKeyCodec + core::fmt::Debug + PartialEq,
        {
            let mut encoded = String::new();
            key.encode_json_key(&mut encoded);
            let mut parser = Parser::new(&encoded);
            let raw = parser.parse_string().expect("parse governance storage key");
            assert_eq!(T::decode_json_key(&raw).expect("decode storage key"), key);
            assert!(T::decode_json_key(&raw.to_uppercase()).is_err());
        }

        check(GovernanceAttemptId::new([0xab; 32]));
        check(BallotAttemptId::new([0xbc; 32]));
        check(TleKeySessionId::new([0xcd; 32]));
    }
    #[test]
    fn account_id_json_key_codec_roundtrip() {
        let keypair = checked_random_keypair();
        let account = AccountId::new(keypair.public_key().clone());
        let mut encoded = String::new();
        account.encode_json_key(&mut encoded);
        let mut parser = Parser::new(&encoded);
        let raw_key = parser.parse_string().expect("parse encoded json key");
        let decoded = AccountId::decode_json_key(&raw_key).expect("decode json key");
        assert_eq!(decoded, account);
    }
    #[test]
    fn musubi_maintainer_directory_key_json_codec_roundtrip() {
        let keypair = checked_random_keypair();
        let key = MusubiMaintainerDirectoryKeyV1::pending(
            MusubiPackageIdV1::new(
                DataSpaceId::new(7),
                MusubiPackageScopeV1::DataspaceRoot,
                "codec".parse().expect("package name"),
            ),
            AccountId::new(keypair.public_key().clone()),
            MusubiInviteIdV1::new([0x42; 32]),
        );
        let mut encoded = String::new();
        key.encode_json_key(&mut encoded);
        let mut parser = Parser::new(&encoded);
        let raw_key = parser.parse_string().expect("parse encoded JSON key");
        let decoded = MusubiMaintainerDirectoryKeyV1::decode_json_key(&raw_key)
            .expect("decode maintainer directory key");
        assert_eq!(decoded, key);
    }
    #[test]
    fn musubi_provider_bundle_attestation_key_json_codec_roundtrip() {
        let key = MusubiProviderBundleAttestationKeyV1 {
            archive_id: ArchiveId::new([0x41; 32]),
            replication_order: ReplicationOrderId::new([0x42; 32]),
            provider_id: ProviderId::new([0x43; 32]),
        };
        let mut encoded = String::new();
        key.encode_json_key(&mut encoded);
        let mut parser = Parser::new(&encoded);
        let raw_key = parser.parse_string().expect("parse encoded JSON key");
        let decoded = MusubiProviderBundleAttestationKeyV1::decode_json_key(&raw_key)
            .expect("decode provider bundle attestation key");
        assert_eq!(decoded, key);
        let with_unknown_field = raw_key
            .strip_suffix('}')
            .expect("structural Musubi key is a JSON object")
            .to_owned()
            + ",\"unexpected\":true}";
        assert!(
            MusubiProviderBundleAttestationKeyV1::decode_json_key(&with_unknown_field).is_err(),
            "provider attestation key must reject unknown fields"
        );
    }
    #[test]
    fn account_id_json_key_codec_rejects_domain_suffix_literal() {
        let err = crate::account::AccountId::decode_json_key("alice@banka.dataspace")
            .expect_err("domain suffix literal must be rejected");
        assert!(
            err.to_string().contains("canonical I105"),
            "unexpected error: {err}"
        );
    }
    #[test]
    fn sccp_outbound_message_key_json_key_codec_is_canonical() {
        let key = SccpOutboundMessageKeyV1::new(
            SccpLaneIdV1 {
                source: SccpNetworkV1::SoraTaira,
                target: SccpNetworkV1::BscTestnet,
            },
            [0x42; 32],
        )
        .expect("valid outbound key");
        let mut encoded = String::new();
        key.encode_json_key(&mut encoded);
        assert_eq!(
            encoded,
            concat!(
                "\"sccp-outbound-v1:sora-taira:bsc-testnet:",
                "4242424242424242424242424242424242424242424242424242424242424242\""
            )
        );
        let mut parser = Parser::new(&encoded);
        let raw_key = parser.parse_string().expect("parse encoded json key");
        let decoded = SccpOutboundMessageKeyV1::decode_json_key(&raw_key).expect("decode json key");
        assert_eq!(decoded, key);
    }
    #[test]
    fn sccp_outbound_message_key_json_key_codec_rejects_aliases_and_malleability() {
        let message_id = "4242424242424242424242424242424242424242424242424242424242424242";
        let zero_id = "0000000000000000000000000000000000000000000000000000000000000000";
        let uppercase_id = format!("ab{}", &message_id[2..]).to_uppercase();
        let hostile = [
            format!("sora-taira:bsc-testnet:{message_id}"),
            format!("SCCP-OUTBOUND-V1:sora-taira:bsc-testnet:{message_id}"),
            format!("sccp-outbound-v1:Sora-Taira:bsc-testnet:{message_id}"),
            format!("sccp-outbound-v1:sora_taira:bsc-testnet:{message_id}"),
            format!("sccp-outbound-v1:taira:bsc-testnet:{message_id}"),
            format!("sccp-outbound-v1:sora-taira:bsc_testnet:{message_id}"),
            format!("sccp-outbound-v1:sora-taira:unknown:{message_id}"),
            format!("sccp-outbound-v1:bsc-testnet:sora-taira:{message_id}"),
            format!("sccp-outbound-v1:sora-taira:sora-nexus:{message_id}"),
            format!("sccp-outbound-v1:sora-taira:ethereum-sepolia:{zero_id}"),
            format!("sccp-outbound-v1:sora-taira:bsc-testnet:{uppercase_id}"),
            format!("sccp-outbound-v1:sora-taira:bsc-testnet: {message_id}"),
            format!("sccp-outbound-v1:sora-taira:bsc-testnet:{message_id}:trailing"),
            String::new(),
        ];
        for encoded in hostile {
            assert!(
                SccpOutboundMessageKeyV1::decode_json_key(&encoded).is_err(),
                "accepted non-canonical or invalid key {encoded:?}"
            );
        }
    }
    #[test]
    fn sccp_outbound_index_key_json_key_codec_is_canonical() {
        let key = SccpOutboundMessageIndexKeyV1 {
            recorded_at_height: 42,
            commitment_index: 7,
            lane: SccpLaneIdV1 {
                source: SccpNetworkV1::SoraTaira,
                target: SccpNetworkV1::EthereumMainnet,
            },
            message_id: [0x7b; 32],
        };
        assert!(key.is_well_formed());
        let mut encoded = String::new();
        key.encode_json_key(&mut encoded);
        assert_eq!(
            encoded,
            concat!(
                "\"sccp-outbound-index-v1:42:7:sora-taira:ethereum-mainnet:",
                "7b7b7b7b7b7b7b7b7b7b7b7b7b7b7b7b7b7b7b7b7b7b7b7b7b7b7b7b7b7b7b7b\""
            )
        );
        let mut parser = Parser::new(&encoded);
        let raw = parser.parse_string().expect("parse encoded index key");
        assert_eq!(
            SccpOutboundMessageIndexKeyV1::decode_json_key(&raw)
                .expect("decode canonical outbound index key"),
            key
        );
    }
    #[test]
    fn sccp_outbound_index_key_json_key_codec_rejects_malleability() {
        let id = "7b7b7b7b7b7b7b7b7b7b7b7b7b7b7b7b7b7b7b7b7b7b7b7b7b7b7b7b7b7b7b7b";
        for encoded in [
            format!("sccp-outbound-index-v1:0:0:sora-taira:ethereum-mainnet:{id}"),
            format!("sccp-outbound-index-v1:01:0:sora-taira:ethereum-mainnet:{id}"),
            format!("sccp-outbound-index-v1:+1:0:sora-taira:ethereum-mainnet:{id}"),
            format!("sccp-outbound-index-v1: 1:0:sora-taira:ethereum-mainnet:{id}"),
            format!("sccp-outbound-index-v1:1::sora-taira:ethereum-mainnet:{id}"),
            format!("sccp-outbound-index-v1:1:00:sora-taira:ethereum-mainnet:{id}"),
            format!("sccp-outbound-index-v1:1:+1:sora-taira:ethereum-mainnet:{id}"),
            format!("sccp-outbound-index-v1:1:512:sora-taira:ethereum-mainnet:{id}"),
            format!("sccp-outbound-index-v1:1:4294967296:sora-taira:ethereum-mainnet:{id}"),
            format!("sccp-outbound-index-v1:1:0:ethereum-mainnet:sora-taira:{id}"),
            format!(
                "sccp-outbound-index-v1:1:0:sora-taira:ethereum-mainnet:{}",
                id.to_uppercase()
            ),
            format!("sccp-outbound-index-v1:1:0:sora-taira:ethereum-mainnet:{id}:tail"),
            format!(
                "sccp-outbound-index-v1:18446744073709551616:0:sora-taira:ethereum-mainnet:{id}"
            ),
        ] {
            assert!(
                SccpOutboundMessageIndexKeyV1::decode_json_key(&encoded).is_err(),
                "accepted non-canonical outbound index key {encoded:?}"
            );
        }
    }
    #[test]
    fn sccp_inbound_message_key_json_key_codec_is_canonical() {
        let key = SccpInboundMessageKeyV1::new(
            SccpLaneIdV1 {
                source: SccpNetworkV1::EthereumMainnet,
                target: SccpNetworkV1::SoraTaira,
            },
            [0xab; 32],
        )
        .expect("valid inbound key");
        let mut encoded = String::new();
        key.encode_json_key(&mut encoded);
        assert_eq!(
            encoded,
            concat!(
                "\"sccp-inbound-v1:ethereum-mainnet:sora-taira:",
                "abababababababababababababababababababababababababababababababab\""
            )
        );
        let mut parser = Parser::new(&encoded);
        let raw_key = parser.parse_string().expect("parse encoded json key");
        assert_eq!(
            SccpInboundMessageKeyV1::decode_json_key(&raw_key)
                .expect("decode canonical inbound key"),
            key
        );
    }
    #[test]
    fn sccp_inbound_message_key_json_key_codec_rejects_aliases_and_malleability() {
        let message_id = "abababababababababababababababababababababababababababababababab";
        let zero_id = "0000000000000000000000000000000000000000000000000000000000000000";
        let hostile = [
            format!("sccp-inbound-v1:ethereum-mainnet:sora-nexus:{message_id}"),
            format!("ethereum-mainnet:sora-taira:{message_id}"),
            format!("SCCP-INBOUND-V1:ethereum-mainnet:sora-taira:{message_id}"),
            format!("sccp-inbound-v1:Ethereum-Mainnet:sora-taira:{message_id}"),
            format!("sccp-inbound-v1:ethereum_mainnet:sora-taira:{message_id}"),
            format!("sccp-inbound-v1:eth-mainnet:sora-taira:{message_id}"),
            format!("sccp-inbound-v1:1:sora-taira:{message_id}"),
            format!("sccp-inbound-v1:unknown:sora-taira:{message_id}"),
            format!("sccp-inbound-v1:ethereum-mainnet:unknown:{message_id}"),
            format!("sccp-inbound-v1:ethereum-mainnet:SORA-TAIRA:{message_id}"),
            format!("sccp-inbound-v1:ethereum-mainnet:sora:{message_id}"),
            format!("sccp-inbound-v1:sora-taira:ethereum-mainnet:{message_id}"),
            format!("sccp-inbound-v1:ethereum-mainnet:bsc-mainnet:{message_id}"),
            format!("sccp-inbound-v1:sora-taira:sora-taira:{message_id}"),
            format!("sccp-inbound-v1:ethereum-mainnet:sora-taira:{zero_id}"),
            format!(
                "sccp-inbound-v1:ethereum-mainnet:sora-taira:{}",
                &message_id[..62]
            ),
            format!("sccp-inbound-v1:ethereum-mainnet:sora-taira:{message_id}ab"),
            format!(
                "sccp-inbound-v1:ethereum-mainnet:sora-taira:{}",
                message_id.to_uppercase()
            ),
            format!(
                "sccp-inbound-v1:ethereum-mainnet:sora-taira:g{}",
                &message_id[1..]
            ),
            format!("sccp-inbound-v1:ethereum-mainnet:sora-taira: {message_id}"),
            format!("sccp-inbound-v1:ethereum-mainnet:sora-taira:{message_id}:trailing"),
            format!(":sccp-inbound-v1:ethereum-mainnet:sora-taira:{message_id}"),
            "sccp-inbound-v1:ethereum-mainnet:sora-taira:".to_owned(),
            String::new(),
        ];
        for encoded in hostile {
            assert!(
                SccpInboundMessageKeyV1::decode_json_key(&encoded).is_err(),
                "accepted non-canonical or invalid key {encoded:?}"
            );
        }
    }
    #[test]
    fn sccp_inbound_anchor_high_water_key_json_key_codec_is_canonical() {
        let key = SccpInboundAnchorHighWaterKeyV1::new(
            SccpLaneIdV1 {
                source: SccpNetworkV1::BscMainnet,
                target: SccpNetworkV1::SoraTaira,
            },
            [0xcd; 32],
        )
        .expect("valid inbound anchor high-water key");
        let mut encoded = String::new();
        key.encode_json_key(&mut encoded);
        assert_eq!(
            encoded,
            concat!(
                "\"sccp-inbound-anchor-high-water-v1:bsc-mainnet:sora-taira:",
                "cdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcd\""
            )
        );
        let mut parser = Parser::new(&encoded);
        let raw_key = parser.parse_string().expect("parse encoded json key");
        assert_eq!(
            SccpInboundAnchorHighWaterKeyV1::decode_json_key(&raw_key)
                .expect("decode canonical inbound anchor high-water key"),
            key
        );
    }
    #[test]
    fn sccp_inbound_anchor_high_water_key_json_key_codec_rejects_malleability() {
        let anchor_hash = "cdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcd";
        let zero_hash = "0000000000000000000000000000000000000000000000000000000000000000";
        let hostile = [
            format!("bsc-mainnet:sora-taira:{anchor_hash}"),
            format!("SCCP-INBOUND-ANCHOR-HIGH-WATER-V1:bsc-mainnet:sora-taira:{anchor_hash}"),
            format!("sccp-inbound-anchor-high-water-v1:Bsc-Mainnet:sora-taira:{anchor_hash}"),
            format!("sccp-inbound-anchor-high-water-v1:bsc_mainnet:sora-taira:{anchor_hash}"),
            format!("sccp-inbound-anchor-high-water-v1:bsc:sora-taira:{anchor_hash}"),
            format!("sccp-inbound-anchor-high-water-v1:bsc-mainnet:sora-nexus:{anchor_hash}"),
            format!("sccp-inbound-anchor-high-water-v1:sora-taira:bsc-mainnet:{anchor_hash}"),
            format!("sccp-inbound-anchor-high-water-v1:bsc-mainnet:ethereum-mainnet:{anchor_hash}"),
            format!("sccp-inbound-anchor-high-water-v1:bsc-mainnet:sora-taira:{zero_hash}"),
            format!(
                "sccp-inbound-anchor-high-water-v1:bsc-mainnet:sora-taira:{}",
                &anchor_hash[..62]
            ),
            format!("sccp-inbound-anchor-high-water-v1:bsc-mainnet:sora-taira:{anchor_hash}cd"),
            format!(
                "sccp-inbound-anchor-high-water-v1:bsc-mainnet:sora-taira:{}",
                anchor_hash.to_uppercase()
            ),
            format!(
                "sccp-inbound-anchor-high-water-v1:bsc-mainnet:sora-taira:g{}",
                &anchor_hash[1..]
            ),
            format!("sccp-inbound-anchor-high-water-v1:bsc-mainnet:sora-taira: {anchor_hash}"),
            format!(
                "sccp-inbound-anchor-high-water-v1:bsc-mainnet:sora-taira:{anchor_hash}:trailing"
            ),
            "sccp-inbound-anchor-high-water-v1:bsc-mainnet:sora-taira:".to_owned(),
            String::new(),
        ];
        for encoded in hostile {
            assert!(
                SccpInboundAnchorHighWaterKeyV1::decode_json_key(&encoded).is_err(),
                "accepted non-canonical or invalid key {encoded:?}"
            );
        }
    }
}
