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

// Governance hash keys use the same canonical lowercase hexadecimal text as
// their scalar JSON representation.
#[cfg(feature = "governance")]
impl_display_object_key!(
    crate::governance::types::GovernanceAttemptId,
    crate::governance::types::AssignmentId,
    crate::governance::types::BodyElectionAttemptId,
    crate::governance::types::BodyInstanceId,
    crate::governance::types::BallotAttemptId,
    crate::governance::types::BeaconPulseId,
    crate::governance::types::TleSessionId,
    crate::governance::types::TleKeySessionId,
);

#[cfg(feature = "governance")]
macro_rules! impl_fixed_governance_object_key {
    ($($ty:path),+ $(,)?) => {
        $(
            impl JsonObjectKeyOwned for $ty {
                fn from_json_key_text(key: &str) -> Result<Self, json::Error> {
                    <$ty>::from_hex_str(key)
                        .map_err(|error| json::Error::Message(error.to_string()))
                }
            }
        )+
    };
}

#[cfg(feature = "governance")]
impl_fixed_governance_object_key!(
    crate::governance::types::GovernanceAttemptId,
    crate::governance::types::AssignmentId,
    crate::governance::types::BodyElectionAttemptId,
    crate::governance::types::BodyInstanceId,
    crate::governance::types::BallotAttemptId,
    crate::governance::types::BeaconPulseId,
    crate::governance::types::TleSessionId,
    crate::governance::types::TleKeySessionId,
);

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

    #[cfg(feature = "governance")]
    #[test]
    fn governance_hash_object_keys_roundtrip_and_reject_noncanonical_text() {
        use crate::governance::types::{
            AssignmentId, BallotAttemptId, BeaconPulseId, BodyElectionAttemptId, BodyInstanceId,
            GovernanceAttemptId, TleKeySessionId, TleSessionId,
        };
        use std::collections::BTreeMap;

        fn check<T>(key: T)
        where
            T: JsonObjectKeyOwned + Ord + core::fmt::Debug,
        {
            let canonical = "ab".repeat(32);
            let mut visited = String::new();
            key.visit_json_key_text::<core::convert::Infallible>(|chunk| {
                visited.push_str(chunk);
                Ok(())
            })
            .expect("visit governance key");
            assert_eq!(visited, canonical);

            let map = BTreeMap::from([(key, 7_u8)]);
            let expected = format!("{{\"{canonical}\":7}}");
            assert_eq!(
                json::to_json(&map).expect("encode governance map"),
                expected
            );
            assert_eq!(
                json::to_json_bounded(&map, expected.len()).expect("encode bounded governance map"),
                expected
            );
            assert!(matches!(
                json::to_json_bounded(&map, expected.len() - 1),
                Err(json::BoundedJsonError::BodyTooLarge)
            ));
            assert_eq!(
                json::from_json::<BTreeMap<T, u8>>(&expected).expect("decode governance map"),
                map
            );

            for invalid in [
                canonical.to_uppercase(),
                format!("0x{canonical}"),
                "ab".repeat(31),
                "ab".repeat(33),
                "ag".repeat(32),
                format!(" {canonical}"),
                format!("{canonical} "),
            ] {
                let invalid_map = format!("{{\"{invalid}\":7}}");
                assert!(
                    json::from_json::<BTreeMap<T, u8>>(&invalid_map).is_err(),
                    "governance object key must reject {invalid:?}"
                );
            }
        }

        check(GovernanceAttemptId::new([0xab; 32]));
        check(AssignmentId::new([0xab; 32]));
        check(BodyElectionAttemptId::new([0xab; 32]));
        check(BodyInstanceId::new([0xab; 32]));
        check(BallotAttemptId::new([0xab; 32]));
        check(BeaconPulseId::new([0xab; 32]));
        check(TleSessionId::new([0xab; 32]));
        check(TleKeySessionId::new([0xab; 32]));
    }

    #[test]
    fn parliament_body_object_keys_match_scalar_json_and_roundtrip() {
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
        for (body, label) in PARLIAMENT_BODIES_V1.into_iter().zip(labels) {
            let scalar = format!("\"{label}\"");
            assert_eq!(json::to_json(&body).expect("encode body"), scalar);
            assert_eq!(
                json::to_json_bounded(&body, scalar.len()).expect("encode bounded body"),
                scalar
            );
            assert_eq!(
                json::from_json::<ParliamentBody>(&scalar).expect("decode body"),
                body
            );

            let map = BTreeMap::from([(body, 7_u8)]);
            let expected = format!("{{\"{label}\":7}}");
            assert_eq!(json::to_json(&map).expect("encode body map"), expected);
            assert_eq!(
                json::to_json_bounded(&map, expected.len()).expect("encode bounded body map"),
                expected
            );
            assert!(matches!(
                json::to_json_bounded(&map, expected.len() - 1),
                Err(json::BoundedJsonError::BodyTooLarge)
            ));
            assert_eq!(
                json::from_json::<BTreeMap<ParliamentBody, u8>>(&expected)
                    .expect("decode body map"),
                map
            );
        }

        for invalid in [
            "PolicyJury",
            "POLICY-JURY",
            "policy_jury",
            "policy-jury ",
            "unknown",
            "",
        ] {
            let encoded = format!("{{\"{invalid}\":7}}");
            assert!(json::from_json::<BTreeMap<ParliamentBody, u8>>(&encoded).is_err());
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
