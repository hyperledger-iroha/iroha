//! JSON object-key identities for scalar data-model identifiers.

use norito::json::{self, JsonObjectKey, JsonObjectKeyOwned};

macro_rules! impl_display_object_key {
    ($($ty:path),+ $(,)?) => {
        $(
            impl JsonObjectKey for $ty {
                fn visit_json_key_text<E>(
                    &self,
                    mut visitor: impl FnMut(&str) -> Result<(), E>,
                ) -> Result<(), E> {
                    let canonical = self.to_string();
                    visitor(&canonical)
                }

                fn visit_json_key_text_checked(
                    &self,
                    visitor: impl FnMut(&str) -> Result<(), json::BoundedJsonError>,
                ) -> Result<(), json::BoundedJsonError> {
                    json::visit_json_display_text(self, visitor)
                }
            }
        )+
    };
}

macro_rules! impl_name_object_key {
    ($ty:path, $constructor:expr) => {
        impl_display_object_key!($ty);

        impl JsonObjectKeyOwned for $ty {
            fn from_json_key_text(key: &str) -> Result<Self, json::Error> {
                <crate::Name as JsonObjectKeyOwned>::from_json_key_text(key).map($constructor)
            }
        }
    };
}

impl_display_object_key!(crate::asset::AssetDefinitionId);
impl JsonObjectKeyOwned for crate::asset::AssetDefinitionId {
    fn from_json_key_text(key: &str) -> Result<Self, json::Error> {
        crate::asset::AssetDefinitionId::parse_address_literal(key)
            .map_err(|error| json::Error::Message(error.to_string()))
    }
}

impl JsonObjectKey for crate::compute::ComputePriceRiskClass {
    fn visit_json_key_text<E>(
        &self,
        mut visitor: impl FnMut(&str) -> Result<(), E>,
    ) -> Result<(), E> {
        visitor(match self {
            Self::Low => "low",
            Self::Balanced => "balanced",
            Self::High => "high",
        })
    }
}

impl JsonObjectKeyOwned for crate::compute::ComputePriceRiskClass {
    fn from_json_key_text(key: &str) -> Result<Self, json::Error> {
        match key {
            "low" => Ok(Self::Low),
            "balanced" => Ok(Self::Balanced),
            "high" => Ok(Self::High),
            _ => Err(json::Error::Message(
                "compute price risk class key must be `low`, `balanced`, or `high`".to_owned(),
            )),
        }
    }
}

impl JsonObjectKey for crate::asset::AssetId {
    fn visit_json_key_text<E>(
        &self,
        mut visitor: impl FnMut(&str) -> Result<(), E>,
    ) -> Result<(), E> {
        let canonical = self.canonical_literal();
        visitor(&canonical)
    }

    fn visit_json_key_text_checked(
        &self,
        mut visitor: impl FnMut(&str) -> Result<(), json::BoundedJsonError>,
    ) -> Result<(), json::BoundedJsonError> {
        JsonObjectKey::visit_json_key_text_checked(self.definition(), &mut visitor)?;
        visitor("#")?;
        JsonObjectKey::visit_json_key_text_checked(self.account(), &mut visitor)?;
        if let crate::asset::AssetBalanceScope::Dataspace(dataspace) = self.scope() {
            visitor("#dataspace:")?;
            JsonObjectKey::visit_json_key_text_checked(dataspace, visitor)?;
        }
        Ok(())
    }
}
impl JsonObjectKeyOwned for crate::asset::AssetId {
    fn from_json_key_text(key: &str) -> Result<Self, json::Error> {
        crate::asset::AssetId::parse_literal(key)
            .map_err(|error| json::Error::Message(error.to_string()))
    }
}

impl_display_object_key!(crate::domain::DomainId);
impl JsonObjectKeyOwned for crate::domain::DomainId {
    fn from_json_key_text(key: &str) -> Result<Self, json::Error> {
        crate::domain::DomainId::parse_json_object_key(key)
    }
}

impl_display_object_key!(crate::nft::NftId);
impl JsonObjectKeyOwned for crate::nft::NftId {
    fn from_json_key_text(key: &str) -> Result<Self, json::Error> {
        let (name, domain) = key.split_once('$').ok_or_else(|| {
            json::Error::Message("NFT key must use `name$domain.dataspace`".to_owned())
        })?;
        if name.is_empty() || domain.is_empty() || domain.contains('$') {
            return Err(json::Error::Message(
                "NFT key must use `name$domain.dataspace`".to_owned(),
            ));
        }
        let name = <crate::Name as JsonObjectKeyOwned>::from_json_key_text(name)?;
        let domain = <crate::domain::DomainId as JsonObjectKeyOwned>::from_json_key_text(domain)?;
        Ok(crate::nft::NftId::new(domain, name))
    }
}

impl_name_object_key!(crate::role::RoleId, crate::role::RoleId::new);
impl_name_object_key!(crate::trigger::TriggerId, crate::trigger::TriggerId::new);
impl_name_object_key!(
    crate::isi::settlement::SettlementId,
    crate::isi::settlement::SettlementId::new
);
impl_name_object_key!(crate::oracle::FeedId, crate::oracle::FeedId);

impl_display_object_key!(crate::proof::ProofId);
impl JsonObjectKeyOwned for crate::proof::ProofId {
    fn from_json_key_text(key: &str) -> Result<Self, json::Error> {
        let backend_len = key.rsplit_once(':').map_or(0, |(backend, _)| backend.len());
        norito::core::reserve_decode_allocation(backend_len)
            .map_err(json::Error::from_decode_resource)?;
        key.parse::<crate::proof::ProofId>()
            .map_err(|error| json::Error::Message(error.to_owned()))
    }
}

impl JsonObjectKey for crate::state_path::StatePath {
    fn visit_json_key_text<E>(
        &self,
        mut visitor: impl FnMut(&str) -> Result<(), E>,
    ) -> Result<(), E> {
        visitor(self.as_ref())
    }
}

impl JsonObjectKeyOwned for crate::state_path::StatePath {
    fn from_json_key_text(key: &str) -> Result<Self, json::Error> {
        crate::state_path::StatePath::parse_json_object_key(key)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_governance_hash_key_contract<K>(key: K)
    where
        K: JsonObjectKeyOwned + Ord + std::fmt::Debug,
    {
        use std::collections::BTreeMap;

        let canonical = "ab".repeat(32);
        let map = BTreeMap::from([(key, 1_u8)]);
        let expected = format!("{{\"{canonical}\":1}}");
        assert_eq!(
            json::to_json(&map).expect("canonical governance hash key"),
            expected
        );
        assert_eq!(
            json::from_json::<BTreeMap<K, u8>>(&expected)
                .expect("parse canonical governance hash key"),
            map,
        );
        assert_eq!(
            json::to_json_bounded(&map, expected.len()).expect("hash key at exact bound"),
            expected,
        );
        assert!(matches!(
            json::to_json_bounded(&map, expected.len() - 1),
            Err(json::BoundedJsonError::BodyTooLarge),
        ));
        for invalid in [
            canonical.to_uppercase(),
            format!("0x{canonical}"),
            format!(" {canonical}"),
            format!("{canonical} "),
            canonical[..63].to_owned(),
            format!("{canonical}0"),
            "gg".repeat(32),
            String::new(),
        ] {
            assert!(K::from_json_key_text(&invalid).is_err());
            let encoded = json::to_json(&BTreeMap::from([(invalid, 1_u8)]))
                .expect("encode rejected governance hash key");
            assert!(json::from_json::<BTreeMap<K, u8>>(&encoded).is_err());
        }
    }

    #[test]
    fn governance_hash_keys_roundtrip_and_reject_noncanonical_spellings() {
        use crate::governance::types::*;

        macro_rules! assert_hash_keys {
            ($($ty:ty),+ $(,)?) => {
                $(assert_governance_hash_key_contract(<$ty>::new([0xab; 32]));)+
            };
        }
        assert_hash_keys!(
            ContractCodeHash,
            ContractAbiHash,
            AgendaItemId,
            DraftId,
            ProposalContentId,
            GovernanceAttemptId,
            BodyInstanceId,
            BodyElectionAttemptId,
            AssignmentId,
            SortitionRequestId,
            BallotAttemptId,
            BeaconSessionId,
            BeaconPulseId,
            TleSessionId,
            TleKeySessionId,
            GovernanceCertificateId,
        );
    }

    #[test]
    fn parliament_body_keys_match_canonical_value_labels() {
        use crate::governance::types::{PARLIAMENT_BODIES_V1, ParliamentBody};
        use std::collections::BTreeMap;

        let labels = [
            "rules-committee",
            "agenda-council",
            "interest-panel",
            "review-panel",
            "coordination-council",
            "mpc-committee",
            "fma-committee",
            "oversight-committee",
            "policy-jury",
            "confirmation-jury",
        ];
        assert_eq!(PARLIAMENT_BODIES_V1.len(), labels.len());
        for (body, label) in PARLIAMENT_BODIES_V1.into_iter().zip(labels) {
            let map = BTreeMap::from([(body, 1_u8)]);
            let expected = format!("{{\"{label}\":1}}");
            let value = format!("\"{label}\"");
            assert_eq!(json::to_json(&body).expect("canonical body value"), value);
            assert_eq!(json::from_json::<ParliamentBody>(&value).unwrap(), body);
            assert_eq!(json::to_json(&map).expect("canonical body key"), expected);
            assert_eq!(
                json::from_json::<BTreeMap<ParliamentBody, u8>>(&expected).unwrap(),
                map,
            );
            assert_eq!(
                json::to_json_bounded(&map, expected.len()).expect("body key at exact bound"),
                expected,
            );
            assert!(matches!(
                json::to_json_bounded(&map, expected.len() - 1),
                Err(json::BoundedJsonError::BodyTooLarge),
            ));
            for invalid in [
                label.to_uppercase(),
                label.replace('-', "_"),
                format!("{body:?}"),
                format!(" {label}"),
                format!("{label} "),
                String::new(),
            ] {
                assert!(ParliamentBody::from_json_key_text(&invalid).is_err());
                let encoded_value = json::to_json(&invalid).unwrap();
                assert!(json::from_json::<ParliamentBody>(&encoded_value).is_err());
                let encoded_map = json::to_json(&BTreeMap::from([(invalid, 1_u8)])).unwrap();
                assert!(json::from_json::<BTreeMap<ParliamentBody, u8>>(&encoded_map).is_err());
            }
        }
    }

    #[test]
    fn compute_price_risk_class_keys_roundtrip_with_exact_bounded_output() {
        use crate::compute::ComputePriceRiskClass;
        use std::collections::BTreeMap;

        let map = BTreeMap::from([
            (ComputePriceRiskClass::Low, 1_u8),
            (ComputePriceRiskClass::Balanced, 2_u8),
            (ComputePriceRiskClass::High, 3_u8),
        ]);
        let expected = r#"{"low":1,"balanced":2,"high":3}"#;
        assert_eq!(
            json::to_json(&map).expect("canonical risk class keys"),
            expected
        );
        assert_eq!(
            json::from_json::<BTreeMap<ComputePriceRiskClass, u8>>(expected)
                .expect("parse canonical risk class keys"),
            map,
        );
        assert_eq!(
            json::to_json_bounded(&map, expected.len()).expect("risk class keys at exact bound"),
            expected,
        );
        assert!(matches!(
            json::to_json_bounded(&map, expected.len() - 1),
            Err(json::BoundedJsonError::BodyTooLarge),
        ));
    }

    #[test]
    fn compute_price_risk_class_keys_reject_noncanonical_spellings() {
        use crate::compute::ComputePriceRiskClass;
        use std::collections::BTreeMap;

        for key in [
            "",
            "Low",
            "Balanced",
            "HIGH",
            " low",
            "low ",
            "medium",
            "0",
            r#"{"class":"Low","value":null}"#,
        ] {
            assert!(
                ComputePriceRiskClass::from_json_key_text(key).is_err(),
                "noncanonical risk class key {key:?}",
            );
            let encoded = json::to_json(&BTreeMap::from([(key, 1_u8)]))
                .expect("encode rejected risk class spelling");
            assert!(
                json::from_json::<BTreeMap<ComputePriceRiskClass, u8>>(&encoded).is_err(),
                "map must reject noncanonical risk class key {key:?}",
            );
        }
    }

    fn allocation_limit(bytes: usize) -> norito::core::DecodeLimits {
        norito::core::DecodeLimits::new(usize::MAX, usize::MAX, usize::MAX, bytes, usize::MAX)
    }

    #[test]
    fn proof_key_charges_retained_backend_before_allocation() {
        let key = format!("halo2/ipa:{}", "AB".repeat(32));
        let backend_bytes = "halo2/ipa".len();
        let (decoded, usage) =
            norito::core::with_decode_limits_measured(allocation_limit(backend_bytes), || {
                <crate::proof::ProofId as JsonObjectKeyOwned>::from_json_key_text(&key)
            });
        assert_eq!(decoded.expect("proof key at exact budget").to_string(), key);
        assert_eq!(usage.total_allocated_bytes(), backend_bytes);

        let (rejected, usage) =
            norito::core::with_decode_limits_measured(allocation_limit(backend_bytes - 1), || {
                <crate::proof::ProofId as JsonObjectKeyOwned>::from_json_key_text(&key)
            });
        assert!(matches!(rejected, Err(json::Error::DecodeResourceLimit)));
        assert_eq!(usage.total_allocated_bytes(), 0);

        let escaped_key = format!("back\"end:{}", "CD".repeat(32));
        let proof = <crate::proof::ProofId as JsonObjectKeyOwned>::from_json_key_text(&escaped_key)
            .expect("proof key with escapable backend");
        let map = std::collections::BTreeMap::from([(proof, 2_u8)]);
        let expected = format!("{{\"back\\\"end:{}\":2}}", "CD".repeat(32));
        assert_eq!(
            json::to_json_bounded(&map, expected.len())
                .expect("escaped proof-key map at exact bound"),
            expected
        );
        assert!(matches!(
            json::to_json_bounded(&map, expected.len() - 1),
            Err(json::BoundedJsonError::BodyTooLarge)
        ));
    }

    #[test]
    fn domain_key_accounts_canonicalization_before_owner_allocations() {
        let key = "treasury.centralbank";
        let component_bytes = key.len() - 1;
        let expected_allocation = component_bytes * 2;
        let (decoded, usage) = norito::core::with_decode_limits_measured(
            allocation_limit(expected_allocation),
            || <crate::domain::DomainId as JsonObjectKeyOwned>::from_json_key_text(key),
        );
        assert_eq!(
            decoded
                .expect("domain key at exact allocation bound")
                .to_string(),
            key
        );
        assert_eq!(usage.total_allocated_bytes(), expected_allocation);

        let (rejected, usage) = norito::core::with_decode_limits_measured(
            allocation_limit(expected_allocation - 1),
            || <crate::domain::DomainId as JsonObjectKeyOwned>::from_json_key_text(key),
        );
        assert!(matches!(rejected, Err(json::Error::DecodeResourceLimit)));
        assert_eq!(usage.total_allocated_bytes(), 0);

        let (noncanonical, usage) =
            norito::core::with_decode_limits_measured(allocation_limit(usize::MAX), || {
                <crate::domain::DomainId as JsonObjectKeyOwned>::from_json_key_text(
                    "例え.centralbank",
                )
            });
        assert!(noncanonical.is_err());
        assert_eq!(usage.total_allocated_bytes(), 0);

        let uppercase = "Treasury.centralbank";
        let (noncanonical, usage) =
            norito::core::with_decode_limits_measured(allocation_limit(usize::MAX), || {
                <crate::domain::DomainId as JsonObjectKeyOwned>::from_json_key_text(uppercase)
            });
        assert!(noncanonical.is_err());
        assert_eq!(usage.total_allocated_bytes(), (uppercase.len() - 1) * 2);
    }

    #[test]
    fn state_path_key_accounts_nfc_scratch_before_normalization() {
        let key = "root/é";
        // Six source scalars have a 24-scalar audited decomposition bound,
        // which requests the first 32-element heap buffer (128 bytes).
        let expected_allocation = key.len() + 128;

        let (decoded, usage) = norito::core::with_decode_limits_measured(
            allocation_limit(expected_allocation),
            || <crate::state_path::StatePath as JsonObjectKeyOwned>::from_json_key_text(key),
        );
        assert_eq!(
            decoded
                .expect("canonical state path at exact allocation bound")
                .as_ref(),
            key
        );
        assert_eq!(usage.total_allocated_bytes(), expected_allocation);

        let (rejected, usage) = norito::core::with_decode_limits_measured(
            allocation_limit(expected_allocation - 1),
            || <crate::state_path::StatePath as JsonObjectKeyOwned>::from_json_key_text(key),
        );
        assert!(matches!(rejected, Err(json::Error::DecodeResourceLimit)));
        assert_eq!(usage.total_allocated_bytes(), 0);

        let decomposed = "root/e\u{301}";
        let (rejected, usage) =
            norito::core::with_decode_limits_measured(allocation_limit(usize::MAX), || {
                <crate::state_path::StatePath as JsonObjectKeyOwned>::from_json_key_text(decomposed)
            });
        assert!(rejected.is_err());
        assert_eq!(usage.total_allocated_bytes(), decomposed.len() + 128);
    }
}
