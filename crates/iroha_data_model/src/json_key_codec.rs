//! Canonical JSON object key codecs for persisted data-model identities.
use iroha_crypto::Hash;
use norito::json;
use norito::json::JsonKeyCodec;
macro_rules! impl_id_key_codec {
    ($($ty:path),+ $(,)?) => {
        $(
            impl JsonKeyCodec for $ty {
                fn encode_json_key(&self, out: &mut String) {
                    json::write_json_string(&self.to_string(), out);
                }
                fn encode_json_key_to(
                    &self,
                    out: &mut dyn json::JsonWriteSink,
                ) -> Result<(), json::BoundedJsonError> {
                    json::write_json_display_to(self, out)
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
                fn encode_json_key_to(
                    &self,
                    out: &mut dyn json::JsonWriteSink,
                ) -> Result<(), json::BoundedJsonError> {
                    json::write_json_value_as_string_to(self, out)
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
    crate::nft::NftId,
    crate::role::RoleId,
    crate::trigger::TriggerId,
    crate::oracle::FeedId,
    crate::proof::ProofId,
);
// These compound identities use the canonical checked key visitor so account
// formatting never allocates an intermediate address or whole key string.
macro_rules! impl_object_key_codec {
    ($($ty:path),+ $(,)?) => { $(
        impl JsonKeyCodec for $ty {
            fn encode_json_key(&self, out: &mut String) {
                json::write_json_string(&self.to_string(), out);
            }
            fn encode_json_key_to(
                &self,
                out: &mut dyn json::JsonWriteSink,
            ) -> Result<(), json::BoundedJsonError> {
                json::write_json_object_key_to(self, out)
            }
            fn decode_json_key(encoded: &str) -> Result<Self, json::Error> {
                encoded.parse::<$ty>().map_err(|err| json::Error::Message(err.to_string()))
            }
        }
    )+ };
}
impl_object_key_codec!(crate::asset::AssetId, crate::isi::settlement::SettlementId);

// Parliament certificate identities are available without governance instructions;
// their canonical storage keys must have the same availability.
impl_id_key_codec!(
    crate::governance::types::GovernanceAttemptId,
    crate::governance::types::BallotAttemptId,
    crate::governance::types::TleKeySessionId,
);
// The anonymous Parliament family keys a credential by its assignment and a ballot board by
// its scope; both spell exactly their lowercase hexadecimal bytes.
impl_id_key_codec!(
    crate::governance::types::AssignmentId,
    crate::governance::anonymous_ballot::BallotScopeV1,
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
    fn encode_json_key_to(
        &self,
        out: &mut dyn json::JsonWriteSink,
    ) -> Result<(), json::BoundedJsonError> {
        json::write_json_value_as_string_to(self, out)
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
    fn encode_json_key_to(
        &self,
        out: &mut dyn json::JsonWriteSink,
    ) -> Result<(), json::BoundedJsonError> {
        json::write_json_value_as_string_to(self, out)
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
// Fresh issuer spend nonces are permanent consensus identities. The complete
// issuer context is part of the key and must have one exact snapshot spelling.
impl JsonKeyCodec for crate::nexus::AxtAnchoredSpendReplayKeyV1 {
    fn encode_json_key(&self, out: &mut String) {
        let mut encoded = String::new();
        norito::json::JsonSerialize::json_serialize(self, &mut encoded);
        json::write_json_string(&encoded, out);
    }
    fn encode_json_key_to(
        &self,
        out: &mut dyn json::JsonWriteSink,
    ) -> Result<(), json::BoundedJsonError> {
        json::write_json_value_as_string_to(self, out)
    }

    fn decode_json_key(encoded: &str) -> Result<Self, json::Error> {
        let mut parser = json::Parser::new(encoded);
        let decoded: Self = norito::json::JsonDeserialize::json_deserialize(&mut parser)?;
        let mut canonical = String::new();
        norito::json::JsonSerialize::json_serialize(&decoded, &mut canonical);
        if canonical != encoded {
            return Err(json::Error::Message(
                "AXT spend nonce key must use canonical JSON".into(),
            ));
        }
        decoded.validate().map_err(|error| {
            json::Error::Message(format!("invalid AXT spend nonce key: {error}"))
        })?;
        Ok(decoded)
    }
}
// A physical source-transfer coordinate is a permanent consensus identity.
// Reject alternate JSON spellings and malformed block/transaction selectors.
impl JsonKeyCodec for crate::nexus::AxtSourceTransferReplayKeyV1 {
    fn encode_json_key(&self, out: &mut String) {
        let mut encoded = String::new();
        norito::json::JsonSerialize::json_serialize(self, &mut encoded);
        json::write_json_string(&encoded, out);
    }
    fn encode_json_key_to(
        &self,
        out: &mut dyn json::JsonWriteSink,
    ) -> Result<(), json::BoundedJsonError> {
        json::write_json_value_as_string_to(self, out)
    }

    fn decode_json_key(encoded: &str) -> Result<Self, json::Error> {
        let mut parser = json::Parser::new(encoded);
        let decoded: Self = norito::json::JsonDeserialize::json_deserialize(&mut parser)?;
        let mut canonical = String::new();
        norito::json::JsonSerialize::json_serialize(&decoded, &mut canonical);
        if canonical != encoded {
            return Err(json::Error::Message(
                "AXT source transfer replay key must use canonical JSON".into(),
            ));
        }
        decoded.validate().map_err(|error| {
            json::Error::Message(format!("invalid AXT source transfer replay key: {error}"))
        })?;
        Ok(decoded)
    }
}
// Bond entitlement keys embed the bond, cash-flow index and holder. Require the
// one canonical JSON spelling so two snapshot keys cannot alias one entitlement.
impl JsonKeyCodec for crate::bond::BondEntitlementId {
    fn encode_json_key(&self, out: &mut String) {
        let mut encoded = String::new();
        norito::json::JsonSerialize::json_serialize(self, &mut encoded);
        json::write_json_string(&encoded, out);
    }
    fn encode_json_key_to(
        &self,
        out: &mut dyn json::JsonWriteSink,
    ) -> Result<(), json::BoundedJsonError> {
        json::write_json_value_as_string_to(self, out)
    }

    fn decode_json_key(encoded: &str) -> Result<Self, json::Error> {
        let mut parser = json::Parser::new(encoded);
        let decoded: Self = norito::json::JsonDeserialize::json_deserialize(&mut parser)?;
        let mut canonical = String::new();
        norito::json::JsonSerialize::json_serialize(&decoded, &mut canonical);
        if canonical != encoded {
            return Err(json::Error::Message(
                "bond entitlement key must use canonical JSON".into(),
            ));
        }
        Ok(decoded)
    }
}
impl JsonKeyCodec for crate::account::AccountId {
    fn encode_json_key(&self, out: &mut String) {
        json::write_json_string(&self.to_string(), out);
    }
    fn encode_json_key_to(
        &self,
        out: &mut dyn json::JsonWriteSink,
    ) -> Result<(), json::BoundedJsonError> {
        json::write_json_object_key_to(self, out)
    }
    fn decode_json_key(encoded: &str) -> Result<Self, json::Error> {
        <Self as json::JsonObjectKeyOwned>::from_json_key_text(encoded).map_err(|error| {
            if cfg!(all(test, sumeragi_model_mutation = "DM25"))
                && matches!(
                    error,
                    json::Error::ScopedDecodeResource(_) | json::Error::DecodeResource(_)
                )
            {
                return json::Error::Message(error.to_string());
            }
            error
        })
    }
}
impl JsonKeyCodec for crate::proof::VerifyingKeyId {
    fn encode_json_key(&self, out: &mut String) {
        let mut buf = String::new();
        norito::json::JsonSerialize::json_serialize(self, &mut buf);
        json::write_json_string(&buf, out);
    }
    fn encode_json_key_to(
        &self,
        out: &mut dyn json::JsonWriteSink,
    ) -> Result<(), json::BoundedJsonError> {
        json::write_json_value_as_string_to(self, out)
    }
    fn decode_json_key(encoded: &str) -> Result<Self, json::Error> {
        let mut parser = json::Parser::new(encoded);
        norito::json::JsonDeserialize::json_deserialize(&mut parser)
    }
}
impl JsonKeyCodec for crate::da::types::StorageTicketId {
    fn encode_json_key(&self, out: &mut String) {
        self.as_bytes().encode_json_key(out);
    }
    fn encode_json_key_to(
        &self,
        out: &mut dyn json::JsonWriteSink,
    ) -> Result<(), json::BoundedJsonError> {
        self.as_bytes().encode_json_key_to(out)
    }
    fn decode_json_key(encoded: &str) -> Result<Self, json::Error> {
        <[u8; 32] as JsonKeyCodec>::decode_json_key(encoded).map(Self::new)
    }
}
impl JsonKeyCodec for crate::runtime::definition::DefinitionId {
    fn encode_json_key(&self, out: &mut String) {
        <[u8; 32] as JsonKeyCodec>::encode_json_key(&self.0, out);
    }
    fn encode_json_key_to(
        &self,
        out: &mut dyn json::JsonWriteSink,
    ) -> Result<(), json::BoundedJsonError> {
        <[u8; 32] as JsonKeyCodec>::encode_json_key_to(&self.0, out)
    }
    fn decode_json_key(encoded: &str) -> Result<Self, json::Error> {
        <[u8; 32] as JsonKeyCodec>::decode_json_key(encoded).map(Self)
    }
}
impl JsonKeyCodec for crate::runtime::obligation::ObligationId {
    fn encode_json_key(&self, out: &mut String) {
        <[u8; 32] as JsonKeyCodec>::encode_json_key(&self.0, out);
    }
    fn encode_json_key_to(
        &self,
        out: &mut dyn json::JsonWriteSink,
    ) -> Result<(), json::BoundedJsonError> {
        <[u8; 32] as JsonKeyCodec>::encode_json_key_to(&self.0, out)
    }
    fn decode_json_key(encoded: &str) -> Result<Self, json::Error> {
        <[u8; 32] as JsonKeyCodec>::decode_json_key(encoded).map(Self)
    }
}
impl JsonKeyCodec for crate::runtime::lifecycle::ChangeSetId {
    fn encode_json_key(&self, out: &mut String) {
        <[u8; 32] as JsonKeyCodec>::encode_json_key(&self.0, out);
    }
    fn encode_json_key_to(
        &self,
        out: &mut dyn json::JsonWriteSink,
    ) -> Result<(), json::BoundedJsonError> {
        <[u8; 32] as JsonKeyCodec>::encode_json_key_to(&self.0, out)
    }
    fn decode_json_key(encoded: &str) -> Result<Self, json::Error> {
        <[u8; 32] as JsonKeyCodec>::decode_json_key(encoded).map(Self)
    }
}
impl JsonKeyCodec for crate::escrow::EscrowId {
    fn encode_json_key(&self, out: &mut String) {
        self.as_hash().encode_json_key(out);
    }
    fn encode_json_key_to(
        &self,
        out: &mut dyn json::JsonWriteSink,
    ) -> Result<(), json::BoundedJsonError> {
        self.as_hash().encode_json_key_to(out)
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
    fn encode_json_key_to(
        &self,
        out: &mut dyn json::JsonWriteSink,
    ) -> Result<(), json::BoundedJsonError> {
        json::write_json_value_as_string_to(self, out)
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
    fn encode_json_key_to(
        &self,
        out: &mut dyn json::JsonWriteSink,
    ) -> Result<(), json::BoundedJsonError> {
        json::write_json_string_to(self.as_ref(), out)
    }
    fn decode_json_key(encoded: &str) -> Result<Self, json::Error> {
        encoded
            .parse()
            .map_err(|err: iroha_model_base::error::ParseError| {
                json::Error::Message(err.reason().into())
            })
    }
}
impl JsonKeyCodec for crate::smart_contract::ContractAddress {
    fn encode_json_key(&self, out: &mut String) {
        json::write_json_string(self.as_ref(), out);
    }
    fn encode_json_key_to(
        &self,
        out: &mut dyn json::JsonWriteSink,
    ) -> Result<(), json::BoundedJsonError> {
        json::write_json_string_to(self.as_ref(), out)
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
    fn encode_json_key_to(
        &self,
        out: &mut dyn json::JsonWriteSink,
    ) -> Result<(), json::BoundedJsonError> {
        json::write_json_display_to(self, out)
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
    fn encode_json_key_to(
        &self,
        out: &mut dyn json::JsonWriteSink,
    ) -> Result<(), json::BoundedJsonError> {
        <[u8; 32] as JsonKeyCodec>::encode_json_key_to(&self.0, out)
    }
    fn decode_json_key(encoded: &str) -> Result<Self, json::Error> {
        <[u8; 32] as JsonKeyCodec>::decode_json_key(encoded).map(Self)
    }
}
impl JsonKeyCodec for crate::sorafs::pin_registry::ReplicationOrderId {
    fn encode_json_key(&self, out: &mut String) {
        <[u8; 32] as JsonKeyCodec>::encode_json_key(self.as_bytes(), out);
    }
    fn encode_json_key_to(
        &self,
        out: &mut dyn json::JsonWriteSink,
    ) -> Result<(), json::BoundedJsonError> {
        <[u8; 32] as JsonKeyCodec>::encode_json_key_to(self.as_bytes(), out)
    }
    fn decode_json_key(encoded: &str) -> Result<Self, json::Error> {
        <[u8; 32] as JsonKeyCodec>::decode_json_key(encoded).map(Self::new)
    }
}
impl JsonKeyCodec for crate::sorafs::pin_registry::ManifestAliasId {
    fn encode_json_key(&self, out: &mut String) {
        json::write_json_string(&self.as_label(), out);
    }
    fn encode_json_key_to(
        &self,
        out: &mut dyn json::JsonWriteSink,
    ) -> Result<(), json::BoundedJsonError> {
        out.push('"')?;
        json::write_json_string_content_to(&self.namespace, out)?;
        out.push('/')?;
        json::write_json_string_content_to(&self.name, out)?;
        out.push('"')
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
    fn encode_json_key_to(
        &self,
        out: &mut dyn json::JsonWriteSink,
    ) -> Result<(), json::BoundedJsonError> {
        <u64 as JsonKeyCodec>::encode_json_key_to(&self.0, out)
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
    fn encode_json_key_to(
        &self,
        out: &mut dyn json::JsonWriteSink,
    ) -> Result<(), json::BoundedJsonError> {
        json::write_json_value_as_string_to(self, out)
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
    fn encode_json_key_to(
        &self,
        out: &mut dyn json::JsonWriteSink,
    ) -> Result<(), json::BoundedJsonError> {
        json::write_json_value_as_string_to(self, out)
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
    fn encode_json_key_to(
        &self,
        out: &mut dyn json::JsonWriteSink,
    ) -> Result<(), json::BoundedJsonError> {
        self.0.encode_json_key_to(out)
    }
    fn decode_json_key(encoded: &str) -> Result<Self, json::Error> {
        <Hash as JsonKeyCodec>::decode_json_key(encoded).map(Self)
    }
}
impl JsonKeyCodec for crate::nexus::UniversalAccountId {
    fn encode_json_key(&self, out: &mut String) {
        <Hash as JsonKeyCodec>::encode_json_key(self.as_hash(), out);
    }
    fn encode_json_key_to(
        &self,
        out: &mut dyn json::JsonWriteSink,
    ) -> Result<(), json::BoundedJsonError> {
        <Hash as JsonKeyCodec>::encode_json_key_to(self.as_hash(), out)
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
    fn encode_json_key_to(
        &self,
        out: &mut dyn json::JsonWriteSink,
    ) -> Result<(), json::BoundedJsonError> {
        out.push('"')?;
        json::JsonObjectKey::visit_json_key_text_checked(&self.sponsor, |chunk| {
            json::write_json_string_content_to(chunk, out)
        })?;
        out.push('/')?;
        json::write_json_string_content_to(self.name.as_ref(), out)?;
        out.push('"')
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
                fn encode_json_key_to(
                    &self,
                    out: &mut dyn json::JsonWriteSink,
                ) -> Result<(), json::BoundedJsonError> {
                    json::write_json_value_as_string_to(self, out)
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
    fn encode_json_key_to(
        &self,
        out: &mut dyn json::JsonWriteSink,
    ) -> Result<(), json::BoundedJsonError> {
        <Hash as JsonKeyCodec>::encode_json_key_to(self.as_hash(), out)
    }
    fn decode_json_key(encoded: &str) -> Result<Self, json::Error> {
        <Hash as JsonKeyCodec>::decode_json_key(encoded).map(crate::account::OpaqueAccountId::from)
    }
}
impl JsonKeyCodec for crate::identifier::IdentifierPolicyId {
    fn encode_json_key(&self, out: &mut String) {
        json::write_json_string(&self.to_string(), out);
    }
    fn encode_json_key_to(
        &self,
        out: &mut dyn json::JsonWriteSink,
    ) -> Result<(), json::BoundedJsonError> {
        json::write_json_display_to(self, out)
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
    fn encode_json_key_to(
        &self,
        out: &mut dyn json::JsonWriteSink,
    ) -> Result<(), json::BoundedJsonError> {
        json::write_json_display_to(self, out)
    }
    fn decode_json_key(encoded: &str) -> Result<Self, json::Error> {
        encoded
            .parse::<crate::ram_lfe::RamLfeProgramId>()
            .map_err(|err| json::Error::Message(err.to_string()))
    }
}

#[cfg(test)]
mod tests {
    struct CheckedKey<'a, T>(&'a T);
    impl<T: JsonKeyCodec> norito::json::JsonSerialize for CheckedKey<'_, T> {
        fn json_serialize(&self, out: &mut String) {
            self.0.encode_json_key(out);
        }
        fn json_serialize_to(
            &self,
            out: &mut dyn norito::json::JsonWriteSink,
        ) -> Result<(), norito::json::BoundedJsonError> {
            self.0.encode_json_key_to(out)
        }
    }
    fn bounded_key_parity<T: JsonKeyCodec>(key: &T) {
        let value = CheckedKey(key);
        let expected = norito::json::to_json(&value).unwrap();
        assert_eq!(
            norito::json::to_json_bounded(&value, expected.len()).unwrap(),
            expected
        );
        for maximum in [0, 1, expected.len() - 1] {
            assert_eq!(
                norito::json::to_json_bounded(&value, maximum),
                Err(norito::json::BoundedJsonError::BodyTooLarge)
            );
        }
    }

    #[test]
    fn da_pin_keys_roundtrip_with_retained_undo() {
        use super::*;
        use crate::da::types::StorageTicketId;
        use iroha_model_base::topology::LaneId;
        use mv::storage::StorageReadOnly;
        let ticket = StorageTicketId::new([0xa7; 32]);
        bounded_key_parity(&ticket);
        let storage = mv::storage::Storage::<StorageTicketId, u64>::new();
        {
            let mut block = storage.block();
            block.insert(ticket, 7);
            block.commit();
        }
        let text = json::to_json(&storage).unwrap();
        let restored: mv::storage::Storage<StorageTicketId, u64> = json::from_str(&text).unwrap();
        assert_eq!(restored.view().get(&ticket), Some(&7));
        assert!(restored.block_and_revert().is_empty());
        let key = (LaneId::new(u32::MAX), u64::MAX, 0_u64);
        bounded_key_parity(&key);
        let mut encoded = String::new();
        key.encode_json_key(&mut encoded);
        let inner: String = json::from_str(&encoded).unwrap();
        assert_eq!(<(LaneId, u64, u64)>::decode_json_key(&inner).unwrap(), key);
        assert!(StorageTicketId::decode_json_key("00").is_err());
        assert!(StorageTicketId::decode_json_key(&"zz".repeat(32)).is_err());
    }
    use crate::account::AccountId;
    use crate::{
        governance::{
            anonymous_ballot::BallotScopeV1,
            types::{AssignmentId, BallotAttemptId, GovernanceAttemptId, TleKeySessionId},
        },
        musubi::{
            ArchiveId, MusubiInviteIdV1, MusubiMaintainerDirectoryKeyV1, MusubiPackageIdV1,
            MusubiPackageScopeV1, MusubiProviderBundleAttestationKeyV1,
        },
        sorafs::{capacity::ProviderId, pin_registry::ReplicationOrderId},
    };
    use iroha_crypto::KeyPair;
    use iroha_model_base::topology::DataSpaceId;
    use norito::json::JsonKeyCodec;
    use norito::json::Parser;
    fn checked_random_keypair() -> KeyPair {
        KeyPair::try_random().expect("generate checked JSON key codec fixture keypair")
    }
    #[test]
    fn checked_storage_keys_preserve_scalar_compound_and_escaped_identity_bytes() {
        use super::*;
        use crate::{asset::AssetDefinitionId, nexus::*};
        use iroha_crypto::HashOf;
        use iroha_model_base::{domain::DomainId, topology::LaneId};
        let hash = Hash::new(b"bounded storage key");
        let owner = AccountId::new(checked_random_keypair().public_key().clone());
        let network = crate::NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(hash));
        let asset = AssetDefinitionId::derive_from_components(
            DomainId::try_new("bounded", "universal").unwrap(),
            "asset".parse().unwrap(),
        );
        bounded_key_parity(&asset);
        bounded_key_parity(&crate::asset::AssetId::new(asset.clone(), owner.clone()));
        bounded_key_parity(&crate::runtime::definition::DefinitionId([1; 32]));
        bounded_key_parity(&crate::runtime::obligation::ObligationId([2; 32]));
        bounded_key_parity(&crate::runtime::lifecycle::ChangeSetId([3; 32]));
        bounded_key_parity(&crate::escrow::EscrowId::new(hash));
        bounded_key_parity(&crate::account::OpaqueAccountId::from(hash));
        bounded_key_parity(&UniversalAccountId::from_hash(hash));
        bounded_key_parity(&ProviderId([4; 32]));
        bounded_key_parity(&ReplicationOrderId::new([5; 32]));
        bounded_key_parity(&crate::oracle::OracleDisputeId(u64::MAX));
        bounded_key_parity(&crate::oracle::OracleChangeId(hash));
        bounded_key_parity(&crate::confidential::ConfidentialParamsId::from(u32::MAX));
        bounded_key_parity(&crate::sorafs::pin_registry::ManifestAliasId::new(
            "quote\"\\\n",
            "unicode😀",
        ));
        bounded_key_parity(&crate::proof::VerifyingKeyId::new("proof\"\\", "key\n😀"));
        bounded_key_parity(&crate::account::rekey::AccountAlias::domainless(
            "alias".parse().unwrap(),
            DataSpaceId::new(7),
        ));
        bounded_key_parity(
            &"bounded::universal"
                .parse::<crate::smart_contract::ContractAlias>()
                .unwrap(),
        );
        bounded_key_parity(
            &crate::smart_contract::ContractAddress::derive(
                &network,
                &owner,
                7,
                DataSpaceId::new(7),
            )
            .unwrap(),
        );
        bounded_key_parity(&crate::oracle::OracleProviderKey::new(
            "feed".parse().unwrap(),
            owner.clone(),
        ));
        bounded_key_parity(&crate::oracle::DefiOracleAttestationKey::new(4, u64::MAX));
        bounded_key_parity(&crate::bond::BondEntitlementId::new(
            asset.clone(),
            3,
            owner.clone(),
        ));
        let program = FeeSponsorProgramId::new(owner.clone(), "fees".parse().unwrap());
        bounded_key_parity(&program);
        bounded_key_parity(&FeeSponsorProgramRevisionKey::new(program.clone(), 2));
        bounded_key_parity(&FeeSponsorEnrollmentKey {
            program_id: program.clone(),
            beneficiary: owner.clone(),
        });
        bounded_key_parity(&FeeSponsorVaultKey {
            program_id: program,
            asset_definition_id: asset.clone(),
        });
        let incarnation = AxtAssetIncarnationV1::derive(
            &network,
            &asset,
            &HashOf::from_untyped_unchecked(hash),
            &hash,
            1,
        );
        let context = AxtHandleIssuerContextV1 {
            network_id: network,
            asset_dsid: DataSpaceId::new(7),
            asset_definition_incarnation: incarnation,
            issuer: UniversalAccountId::from_hash(hash),
            issuer_manifest_root: [6; 32],
            code_root: [7; 32],
            abi_version: 1,
            abi_hash: [8; 32],
        };
        bounded_key_parity(&AxtHandleBudgetKey::from_issuer_payload_v1(
            &AssetHandleIssuerPayloadV1 {
                context,
                asset_definition_id: asset,
                scope: vec!["transfer\"\\\n".into()],
                subject: HandleSubject {
                    account: owner.to_string(),
                    origin_dsid: Some(DataSpaceId::new(7)),
                },
                budget: HandleBudget {
                    remaining: 10_u32.into(),
                    per_use: Some(1_u32.into()),
                },
                active_handle_era: 1,
                next_handle_counter: 2,
                group_binding: GroupBinding {
                    composability_group_id: vec![1, 2],
                    epoch_id: 3,
                },
                target_lane: LaneId::new(4),
                axt_binding: AxtBinding::new([9; 32]),
                manifest_view_root: [10; 32],
                expiry_slot: 20,
                max_clock_skew_ms: Some(1),
            },
        ));
        bounded_key_parity(&AxtHandleReplayKey::from_parts(
            DataSpaceId::new(7),
            incarnation,
            [9; 32],
            1,
            2,
            LaneId::new(4),
        ));
        bounded_key_parity(&AxtAnchoredSpendReplayKeyV1 {
            issuer_context: context,
            nonce: AxtSpendNonceV1::try_new([11; 32]).unwrap(),
        });
        bounded_key_parity(&AxtSourceTransferReplayKeyV1 {
            network_id: network,
            dataspace_id: DataSpaceId::new(7),
            lane_id: LaneId::new(4),
            block_header_hash: HashOf::from_untyped_unchecked(hash),
            source_tx_index: 1,
            transcript_index: 2,
            delta_index: 3,
        });
    }
    #[test]
    fn governance_hash_ids_are_canonical_json_storage_keys() {
        fn check<T>(key: &T)
        where
            T: JsonKeyCodec + core::fmt::Debug + PartialEq,
        {
            bounded_key_parity(key);
            let mut encoded = String::new();
            key.encode_json_key(&mut encoded);
            let mut parser = Parser::new(&encoded);
            let raw = parser.parse_string().expect("parse governance storage key");
            assert_eq!(&T::decode_json_key(&raw).expect("decode storage key"), key);
            assert_eq!(raw.len(), 64, "canonical keys contain exactly 32 hex bytes");
            for invalid in [
                raw.to_uppercase(),
                format!("0x{raw}"),
                raw[..62].to_owned(),
                format!("{raw}00"),
                "zz".repeat(32),
            ] {
                assert!(T::decode_json_key(&invalid).is_err());
            }
        }

        check(&GovernanceAttemptId::new([0xab; 32]));
        check(&BallotAttemptId::new([0xbc; 32]));
        check(&TleKeySessionId::new([0xcd; 32]));
        check(&AssignmentId::new([0xde; 32]));
    }
    #[test]
    fn anonymous_ballot_scopes_are_canonical_json_storage_keys() {
        let scope = BallotScopeV1::new([0xef; 64]);
        bounded_key_parity(&scope);
        let mut encoded = String::new();
        scope.encode_json_key(&mut encoded);
        let mut parser = Parser::new(&encoded);
        let raw = parser
            .parse_string()
            .expect("parse ballot scope storage key");
        assert_eq!(
            raw,
            "ef".repeat(64),
            "a scope key is its 64 lowercase hex bytes"
        );
        assert_eq!(
            BallotScopeV1::decode_json_key(&raw).expect("decode ballot scope storage key"),
            scope
        );
        for invalid in [
            raw.to_uppercase(),
            format!("0x{raw}"),
            raw[..126].to_owned(),
            format!("{raw}00"),
            "zz".repeat(64),
        ] {
            assert!(BallotScopeV1::decode_json_key(&invalid).is_err());
        }
    }
    #[test]
    fn account_storage_keys_preserve_original_refusal_fields_scope_and_retry() {
        use norito::core::{
            DecodeAttemptErrorKind, DecodeBudgetContext, DecodeResourceError,
            classify_decode_attempt,
        };
        use norito::json;
        let expected = AccountId::new(
            KeyPair::from_seed(vec![42; 32], iroha_crypto::Algorithm::Ed25519)
                .public_key()
                .clone(),
        );
        let literal = expected.canonical_i105().unwrap();
        let storage = mv::storage::Storage::<AccountId, u64>::new();
        {
            let mut block = storage.block();
            block.insert(expected.clone(), 7);
            block.commit();
        }
        let wire = json::to_json(&storage).unwrap();
        let source_pointer = wire.as_ptr();
        let decode = |mode| -> Result<(), json::Error> {
            if mode == 0 {
                assert_eq!(AccountId::decode_json_key(&literal)?, expected);
            } else {
                let restored: mv::storage::Storage<AccountId, u64> = json::from_json(&wire)?;
                assert_eq!(restored.view().get(&expected), Some(&7));
            }
            Ok(())
        };
        let limits = |bytes| {
            norito::DecodeLimits::new(usize::MAX, usize::MAX, usize::MAX, bytes, usize::MAX)
        };
        let demands = [0, 1].map(|mode| {
            let (decoded, usage) =
                norito::core::with_decode_limits_measured(limits(usize::MAX), || decode(mode));
            decoded.expect("actual reader accepts the original canonical input");
            let demand = usage.total_allocated_bytes();
            assert!(demand > 0);
            demand
        });
        let context_limit = demands
            .iter()
            .try_fold(0_usize, |sum, demand| sum.checked_add(*demand))
            .unwrap()
            .checked_mul(2)
            .unwrap();
        let pool = iroha_allocation::AllocationBudget::new(
            DecodeBudgetContext::allocation_layout().size(),
        );
        let original = DecodeBudgetContext::try_new_owned(limits(context_limit), &pool).unwrap();
        let baseline = pool.reserved_bytes();
        // This original pool owns decoder counters. Ordinary Storage/DTO graphs are
        // not claimed to have physical original-pool admission by this control.
        for (mode, demand) in demands.into_iter().enumerate() {
            let before = original.consumed_allocated_bytes();
            let mut observed = None;
            let refusal = original.with(|| norito::with_decode_limits_scope(
                limits(demand - 1), || classify_decode_attempt(|| {
                    let error = decode(mode).expect_err("one-byte-short original reader must refuse");
                    let norito::json::Error::ScopedDecodeResource(origin) = &error else {
                        panic!("AccountId storage key must retain its original scoped refusal: {error:?}");
                    };
                    observed = Some(origin.clone());
                    Err::<(), _>(error.into_core_error())
                }))).unwrap_err();
            assert_eq!(refusal.kind(), DecodeAttemptErrorKind::EnclosingLimit);
            let refusal = refusal.into_error();
            assert_eq!(
                refusal.decode_resource_error(),
                Some(DecodeResourceError::TotalAllocationExceeded {
                    attempted: u64::try_from(demand).unwrap(),
                    limit: u64::try_from(demand - 1).unwrap(),
                })
            );
            let norito::Error::ScopedDecodeResource(returned) = refusal else {
                panic!("reader must return its exact original observer");
            };
            assert_eq!(returned, observed.unwrap());
            drop(returned);
            assert_eq!(pool.reserved_bytes(), baseline);
            let after_refusal = original.consumed_allocated_bytes();
            // Outer counters debit the attempted final charge before the narrower
            // inner ceiling refuses. Retry reuses that same cumulative owner.
            assert_eq!(after_refusal - before, u64::try_from(demand).unwrap());
            original
                .with(|| decode(mode))
                .expect("same input and original context retry");
            assert_eq!(
                original.consumed_allocated_bytes() - after_refusal,
                u64::try_from(demand).unwrap()
            );
            assert_eq!(pool.reserved_bytes(), baseline);
            let (refusal, usage) =
                norito::core::with_decode_limits_measured(limits(demand - 1), || decode(mode));
            let error = refusal
                .expect_err("same input unscoped refusal")
                .into_core_error();
            assert_eq!(
                error.decode_resource_error(),
                Some(DecodeResourceError::TotalAllocationExceeded {
                    attempted: u64::try_from(demand).unwrap(),
                    limit: u64::try_from(demand - 1).unwrap(),
                })
            );
            assert!(!matches!(error, norito::Error::ScopedDecodeResource(_)));
            assert!(usage.total_allocated_bytes() < demand);
        }
        drop(original);
        assert_eq!(pool.reserved_bytes(), 0);
        let error = AccountId::decode_json_key("alice@banka.dataspace").unwrap_err();
        assert!(error.into_core_error().decode_resource_error().is_none());
        assert_eq!(wire.as_ptr(), source_pointer);
        assert_eq!(storage.view().get(&expected), Some(&7));
    }

    #[test]
    fn account_id_json_key_codec_roundtrip() {
        let keypair = checked_random_keypair();
        let account = AccountId::new(keypair.public_key().clone());
        bounded_key_parity(&account);
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
        bounded_key_parity(&key);
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
        bounded_key_parity(&key);
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
            matches!(
                &err,
                norito::json::Error::WithPos {
                    msg: "invalid I105 account identifier",
                    byte: 0,
                    line: 1,
                    col: 1,
                }
            ),
            "unexpected error: {err}"
        );
        assert!(!err.is_decode_resource_limit());
    }
}
