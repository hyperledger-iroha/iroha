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

impl_id_key_codec!(
    crate::asset::AssetDefinitionId,
    crate::asset::AssetId,
    crate::nft::NftId,
    crate::role::RoleId,
    crate::trigger::TriggerId,
    crate::oracle::FeedId,
    crate::proof::ProofId,
    crate::isi::settlement::SettlementId,
);

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
            .map(crate::account::ParsedAccountId::into_account_id)
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

impl JsonKeyCodec for crate::nexus::FeeSponsorPolicyId {
    fn encode_json_key(&self, out: &mut String) {
        json::write_json_string(&self.to_string(), out);
    }

    fn decode_json_key(encoded: &str) -> Result<Self, json::Error> {
        encoded
            .parse::<crate::nexus::FeeSponsorPolicyId>()
            .map_err(|err| json::Error::Message(err.to_string()))
    }
}

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

impl JsonKeyCodec for crate::bridge::SccpOutboundMessageKey {
    fn encode_json_key(&self, out: &mut String) {
        let encoded = format!(
            "{}:{}:{}",
            self.source_domain,
            self.target_domain,
            hex::encode_upper(self.message_id)
        );
        json::write_json_string(&encoded, out);
    }

    fn decode_json_key(encoded: &str) -> Result<Self, json::Error> {
        let mut parts = encoded.split(':');
        let source_domain = parts
            .next()
            .ok_or_else(|| json::Error::Message("missing SCCP source domain".into()))?
            .parse::<u32>()
            .map_err(|err| json::Error::Message(err.to_string()))?;
        let target_domain = parts
            .next()
            .ok_or_else(|| json::Error::Message("missing SCCP target domain".into()))?
            .parse::<u32>()
            .map_err(|err| json::Error::Message(err.to_string()))?;
        let message_id_hex = parts
            .next()
            .ok_or_else(|| json::Error::Message("missing SCCP message id".into()))?;
        if parts.next().is_some() {
            return Err(json::Error::Message(
                "too many SCCP outbound message key parts".into(),
            ));
        }
        let message_id = hex::decode(message_id_hex)
            .map_err(|err| json::Error::Message(err.to_string()))?
            .try_into()
            .map_err(|_| json::Error::Message("SCCP message id must be 32 bytes".into()))?;
        Ok(Self {
            source_domain,
            target_domain,
            message_id,
        })
    }
}

#[cfg(test)]
mod tests {
    use iroha_crypto::KeyPair;
    use mv::json::JsonKeyCodec;
    use norito::json::Parser;

    use crate::account::AccountId;
    use crate::bridge::SccpOutboundMessageKey;

    fn checked_random_keypair() -> KeyPair {
        KeyPair::try_random().expect("generate checked JSON key codec fixture keypair")
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
    fn account_id_json_key_codec_rejects_domain_suffix_literal() {
        let err = crate::account::AccountId::decode_json_key("alice@banka.dataspace")
            .expect_err("domain suffix literal must be rejected");
        assert!(
            err.to_string().contains("canonical I105"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn sccp_outbound_message_key_json_key_codec_roundtrip() {
        let key = SccpOutboundMessageKey {
            source_domain: 1,
            target_domain: 2,
            message_id: [0x42; 32],
        };
        let mut encoded = String::new();
        key.encode_json_key(&mut encoded);
        let mut parser = Parser::new(&encoded);
        let raw_key = parser.parse_string().expect("parse encoded json key");
        let decoded = SccpOutboundMessageKey::decode_json_key(&raw_key).expect("decode json key");
        assert_eq!(decoded, key);
    }
}
