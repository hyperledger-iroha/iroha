//! Complete-controller V1 decode and canonical-layout regressions.

use super::*;
use crate::account::{AccountController, AccountId, MultisigPolicyError};
use iroha_crypto::{Algorithm, KeyPair};
use norito::{codec::Encode, core::DecodeFromSlice};

fn policy() -> MultisigPolicy {
    MultisigPolicy::new(
        2,
        [0x11, 0x22]
            .into_iter()
            .enumerate()
            .map(|(index, seed)| {
                let pair = KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519).unwrap();
                MultisigMember::new(
                    pair.public_key().clone(),
                    u16::try_from(index + 1).expect("fixture value fits u16"),
                )
                .unwrap()
            })
            .collect(),
    )
    .unwrap()
}

fn invalid_policies() -> Vec<MultisigPolicy> {
    let valid = policy();
    let mut bad = Vec::new();
    let mut value = valid.clone();
    value.version = 2;
    bad.push(value);
    let mut value = valid.clone();
    value.threshold = 0;
    bad.push(value);
    let mut value = valid.clone();
    value.threshold = 4;
    bad.push(value);
    let mut value = valid.clone();
    value.members.clear();
    bad.push(value);
    let mut value = valid.clone();
    value.members.reverse();
    bad.push(value);
    let mut value = valid.clone();
    value.members[1] = value.members[0].clone();
    value.members[1].weight = 2;
    bad.push(value);
    let mut value = valid;
    value.members[0].weight = 0;
    bad.push(value);
    bad
}

#[test]
fn multisig_policy_constructor_normalizes_but_external_components_are_strict() {
    let valid = policy();
    let mut reversed = valid.members.clone();
    reversed.reverse();
    assert_eq!(MultisigPolicy::new(2, reversed.clone()).unwrap(), valid);
    assert_eq!(
        MultisigPolicy::from_serialized(1, 2, reversed).unwrap_err(),
        MultisigPolicyError::NonCanonicalMemberOrder
    );
    assert_eq!(
        MultisigPolicy::from_serialized(1, 2, valid.members.clone()).unwrap(),
        valid
    );
    assert_eq!(
        MultisigPolicy::from_serialized(
            1,
            1,
            vec![valid.members[0].clone(); usize::from(u16::MAX) + 1]
        )
        .unwrap_err(),
        MultisigPolicyError::TooManyMembers(usize::from(u16::MAX) + 1)
    );
}

#[test]
fn multisig_policy_member_and_full_account_roundtrip_under_declared_layouts() {
    let valid = policy();
    let account = AccountId::new_multisig(valid.clone());
    let canonical = norito::encode_canonical(&account).unwrap();
    for flags in [0, 1, 2, 3, 4, 5, 6, 7, 0x1b, 0x3f] {
        let _layout = norito::core::DecodeFlagsGuard::enter(flags);
        assert_eq!(norito::encode_canonical(&account).unwrap(), canonical);
        assert_eq!(
            norito::decode_canonical::<AccountId>(&canonical).unwrap(),
            account
        );
        let framed = norito::to_bytes(&account).unwrap();
        assert_eq!(
            norito::decode_from_bytes::<AccountId>(&framed).unwrap(),
            account
        );
        let framed = norito::to_bytes(&valid).unwrap();
        assert_eq!(
            norito::decode_from_bytes::<MultisigPolicy>(&framed).unwrap(),
            valid
        );
        let (raw, actual_flags) = norito::codec::encode_with_header_flags(&valid);
        let _actual = norito::core::DecodeFlagsGuard::enter(actual_flags);
        assert_eq!(
            MultisigPolicy::decode_from_slice(&raw).unwrap_or_else(|error| panic!(
                "requested flags {flags}, actual {actual_flags}: {error:?}"
            )),
            (valid.clone(), raw.len())
        );
        let member = &valid.members[0];
        let (raw, actual_flags) = norito::codec::encode_with_header_flags(member);
        let _actual_member = norito::core::DecodeFlagsGuard::enter(actual_flags);
        assert_eq!(
            MultisigMember::decode_from_slice(&raw).unwrap(),
            (member.clone(), raw.len())
        );
    }
}

#[test]
fn multisig_invalid_policies_reject_in_policy_controller_account_and_slice_decoders() {
    for invalid in invalid_policies() {
        let policy_frame = norito::encode_canonical(&invalid).unwrap();
        assert!(
            norito::decode_canonical::<MultisigPolicy>(&policy_frame).is_err(),
            "{invalid:?}"
        );
        let _layout = norito::core::DecodeFlagsGuard::enter(norito::core::default_encode_flags());
        let raw = invalid.encode();
        assert!(
            MultisigPolicy::decode_from_slice(&raw).is_err(),
            "{invalid:?}"
        );
        let controller = AccountController::Multisig(invalid.clone());
        let bytes = norito::encode_canonical(&controller).unwrap();
        assert!(
            norito::decode_canonical::<AccountController>(&bytes).is_err(),
            "{invalid:?}"
        );
        let account = AccountId::new_multisig(invalid);
        let bytes = norito::encode_canonical(&account).unwrap();
        assert!(norito::decode_canonical::<AccountId>(&bytes).is_err());
    }
    let mut member = policy().members[0].clone();
    member.weight = 0;
    let bytes = norito::encode_canonical(&member).unwrap();
    assert!(norito::decode_canonical::<MultisigMember>(&bytes).is_err());
    let raw = member.encode();
    assert!(MultisigMember::decode_from_slice(&raw).is_err());
}

#[test]
fn multisig_json_rejects_invalid_policies_members_and_unknown_fields() {
    let valid = policy();
    let json = norito::json::to_json(&valid).unwrap();
    assert_eq!(
        norito::json::from_str::<MultisigPolicy>(&json).unwrap(),
        valid
    );
    let member = &valid.members[0];
    let json_member = norito::json::to_json(member).unwrap();
    assert_eq!(
        norito::json::from_str::<MultisigMember>(&json_member).unwrap(),
        *member
    );
    for invalid in invalid_policies() {
        let json = norito::json::to_json(&invalid).unwrap();
        assert!(
            norito::json::from_str::<MultisigPolicy>(&json).is_err(),
            "{json}"
        );
        let controller = AccountController::Multisig(invalid);
        assert!(
            norito::json::from_str::<AccountController>(
                &norito::json::to_json(&controller).unwrap()
            )
            .is_err()
        );
    }
    assert!(
        norito::json::from_str::<MultisigPolicy>(&json.replacen('{', "{\"hint\":0,", 1)).is_err()
    );
    assert!(
        norito::json::from_str::<MultisigMember>(&json_member.replacen('{', "{\"hint\":0,", 1))
            .is_err()
    );
    let mut invalid_member = member.clone();
    invalid_member.weight = 0;
    assert!(
        norito::json::from_str::<MultisigMember>(&norito::json::to_json(&invalid_member).unwrap())
            .is_err()
    );
}

#[test]
fn multisig_direct_decode_validation_preserves_errors_and_recovers_across_layouts() {
    let valid = policy();
    let mut invalid = valid.clone();
    invalid.members.reverse();
    let expected = MultisigPolicyError::NonCanonicalMemberOrder.to_string();
    let mut invalid_member = valid.members[0].clone();
    invalid_member.weight = 0;
    let member_error = MultisigPolicyError::MemberWeightZero.to_string();
    for flags in [0, 1, 2, 3, 4, 5, 6, 7, 0x1b, 0x3f] {
        let _flags = norito::core::DecodeFlagsGuard::enter(flags);
        let (payload, actual_flags) = norito::codec::encode_with_header_flags(&valid);
        let canonical =
            norito::core::frame_bare_with_header_flags::<MultisigPolicy>(&payload, actual_flags)
                .unwrap();
        {
            let header = norito::core::Header::read(canonical.as_slice()).unwrap();
            let _actual = norito::core::DecodeFlagsGuard::enter(header.flags);
            assert_eq!(
                norito::decode_from_bytes::<MultisigPolicy>(&canonical).unwrap(),
                valid
            );
        }
        let (payload, actual_flags) = norito::codec::encode_with_header_flags(&invalid);
        let bad_policy =
            norito::core::frame_bare_with_header_flags::<MultisigPolicy>(&payload, actual_flags)
                .unwrap();
        let (payload, actual_flags) = norito::codec::encode_with_header_flags(&invalid_member);
        let bad_member =
            norito::core::frame_bare_with_header_flags::<MultisigMember>(&payload, actual_flags)
                .unwrap();
        // Decode complete frames under their actual advertised flags, which can
        // omit requested layout features unused by the individual record.
        {
            let header = norito::core::Header::read(bad_policy.as_slice()).unwrap();
            let _actual = norito::core::DecodeFlagsGuard::enter(header.flags);
            let error = norito::decode_from_bytes::<MultisigPolicy>(&bad_policy).unwrap_err();
            assert!(matches!(error, norito::Error::Message(message) if message == expected));
        }
        {
            let header = norito::core::Header::read(bad_member.as_slice()).unwrap();
            let _actual = norito::core::DecodeFlagsGuard::enter(header.flags);
            let error = norito::decode_from_bytes::<MultisigMember>(&bad_member).unwrap_err();
            assert!(matches!(error, norito::Error::Message(message) if message == member_error));
        }
        let header = norito::core::Header::read(canonical.as_slice()).unwrap();
        let _actual = norito::core::DecodeFlagsGuard::enter(header.flags);
        assert_eq!(
            norito::decode_from_bytes::<MultisigPolicy>(&canonical).unwrap(),
            valid
        );
    }
}

#[test]
fn invalid_multisig_policies_reject_inside_ordinary_and_funded_complete_uid_bindings() {
    use crate::asset::uid::{
        AssetUidBindingDecodeError, AssetUidBindingV1, decode_asset_uid_binding_with_owned_controls,
    };
    use norito::core::{OwnedPreparedDecodeError, PreparedDecodeError};
    let invalid = invalid_policies();
    assert_eq!(
        invalid.len(),
        7,
        "cover every existing semantic invalidity fixture"
    );
    for policy in invalid {
        let value = AssetUidBindingV1 {
            asset_definition_id: crate::sccp::escrow::sccp_taira_xor_asset_definition_id(),
            account_id: AccountId::new_multisig(policy),
            uid: [7; 32],
        };
        // Encoding retains a correct header, schema and checksum around malformed
        // controller semantics. Both decoders must reach the policy validator.
        let frame = norito::encode_canonical(&value).unwrap();
        let ordinary = norito::decode_canonical::<AssetUidBindingV1>(&frame).unwrap_err();
        let pool = iroha_allocation::AllocationBudget::new(1 << 20);
        let funded = match decode_asset_uid_binding_with_owned_controls(
            &frame,
            norito::canonical_decode_limits(frame.len()),
            &pool,
        ) {
            Err(error) => error,
            Ok(_) => panic!("funded binding accepted an invalid multisig policy"),
        };
        let OwnedPreparedDecodeError::Decode(PreparedDecodeError::Destination(
            AssetUidBindingDecodeError::Controller(original),
        )) = funded
        else {
            panic!(
                "malformed policy semantics must reach the shared controller validator: {funded}"
            )
        };
        assert_eq!(
            ordinary.to_string(),
            norito::Error::Message(original.to_string()).to_string()
        );
        assert_eq!(pool.reserved_bytes(), 0);
    }
}
