//! Captured public scalar frames and checked reconstruction contracts.

use std::fmt::Debug;

use iroha_crypto::{Algorithm, KeyPair};
use norito::{
    NoritoDeserialize, NoritoSerialize,
    codec::Encode as _,
    core as ncore,
    json::{JsonDeserialize, JsonSerialize, Value},
};

use iroha_data_model::{
    account::AccountId,
    id::NetworkId,
    privacy::PrivacyX509KeyUsageRequirementV1,
    smart_contract::{ContractAddress, ContractAlias},
    sorafs::pin_registry::ManifestRootCid,
    transaction::executable::{ContractArgumentRecord, MAX_CONTRACT_ARGUMENT_RECORD_BYTES},
};
use iroha_model_base::topology::DataSpaceId;

use crate::frame_identity_test_support::record;

fn family<T>(rows: &mut Vec<Value>, name: &str, first: T, second: T)
where
    T: norito::NoritoSchema
        + Clone
        + Debug
        + PartialEq
        + NoritoSerialize
        + for<'de> NoritoDeserialize<'de>
        + JsonSerialize
        + JsonDeserialize,
{
    assert_ne!(first, second, "{name}: populated values must differ");
    record(rows, &format!("{name}/root_first"), &first);
    record(rows, &format!("{name}/root_second"), &second);
    record(rows, &format!("{name}/option_none"), &None::<T>);
    record(rows, &format!("{name}/option_first"), &Some(first.clone()));
    record(
        rows,
        &format!("{name}/option_second"),
        &Some(second.clone()),
    );
    record(rows, &format!("{name}/vec_empty"), &Vec::<T>::new());
    record(rows, &format!("{name}/vec_two"), &vec![first, second]);
}

fn reject_payload<T>(bytes: &[u8], flags: u8)
where
    T: Debug + NoritoSerialize + for<'de> NoritoDeserialize<'de>,
{
    let frame = ncore::frame_bare_with_header_flags::<T>(bytes, flags)
        .expect("frame the invalid field with a correct header and checksum");
    let view = ncore::from_bytes_view(&frame).expect("authenticate the malformed-field frame");
    assert_eq!(view.as_bytes(), bytes);
    view.decode_exact_with(ncore::decode_field_canonical::<T>)
        .expect_err("checked owner reconstruction must reject the invalid field");
    assert!(norito::decode_canonical::<T>(&frame).is_err());
    let _flags = ncore::DecodeFlagsGuard::enter(flags);
    let _payload = ncore::PayloadCtxGuard::enter_with_schema_and_flags(bytes, view.schema(), flags);
    let archived = ncore::from_bytes::<T>(&frame).expect("authenticate typed archive metadata");
    <T as ncore::DeserializePayload<'_>>::try_deserialize(archived)
        .expect_err("the owner's fallible decoder must reject without panicking");
}

fn privacy_requirement_values(rows: &mut Vec<Value>) {
    for required in [false, true] {
        let value = PrivacyX509KeyUsageRequirementV1::new(required);
        assert_eq!(value.is_required(), required);
        assert_eq!(value.encode(), required.encode());
        let frame = norito::encode_canonical(&value).unwrap();
        assert_eq!(frame, norito::encode_canonical(&required).unwrap());
        assert_eq!(norito::decode_canonical::<bool>(&frame).unwrap(), required);
        assert_eq!(
            norito::decode_canonical::<PrivacyX509KeyUsageRequirementV1>(
                &norito::encode_canonical(&required).unwrap(),
            )
            .unwrap(),
            value,
        );
    }
    // Only the root projects to bool. Generic parents retain the wrapper's
    // nominal identity even though the wrapped payload bytes are identical.
    for required in [None, Some(false), Some(true)] {
        let wrapped = required.map(PrivacyX509KeyUsageRequirementV1::new);
        assert_eq!(wrapped.encode(), required.encode());
        let wrapper_frame = norito::encode_canonical(&wrapped).unwrap();
        let bool_frame = norito::encode_canonical(&required).unwrap();
        assert_ne!(wrapper_frame, bool_frame);
        assert!(norito::decode_canonical::<Option<bool>>(&wrapper_frame).is_err());
        assert!(
            norito::decode_canonical::<Option<PrivacyX509KeyUsageRequirementV1>>(&bool_frame)
                .is_err()
        );
    }
    for required in [Vec::<bool>::new(), vec![false, true]] {
        let wrapped: Vec<_> = required
            .iter()
            .copied()
            .map(PrivacyX509KeyUsageRequirementV1::new)
            .collect();
        assert_eq!(wrapped.encode(), required.encode());
        let wrapper_frame = norito::encode_canonical(&wrapped).unwrap();
        let bool_frame = norito::encode_canonical(&required).unwrap();
        assert_ne!(wrapper_frame, bool_frame);
        assert!(norito::decode_canonical::<Vec<bool>>(&wrapper_frame).is_err());
        assert!(
            norito::decode_canonical::<Vec<PrivacyX509KeyUsageRequirementV1>>(&bool_frame).is_err()
        );
    }
    reject_payload::<PrivacyX509KeyUsageRequirementV1>(&[2], ncore::default_encode_flags());
    family(
        rows,
        "privacy_key_usage_requirement",
        PrivacyX509KeyUsageRequirementV1::new(false),
        PrivacyX509KeyUsageRequirementV1::new(true),
    );
}

fn contract_argument_values(rows: &mut Vec<Value>) {
    let first = ContractArgumentRecord::try_new(vec![0, 1, 2, 255]).unwrap();
    let second = ContractArgumentRecord::try_new(vec![0x4b, 0x4f, 0x54, 0x4f]).unwrap();
    assert_eq!(first.encode(), first.as_bytes().to_vec().encode());
    assert_eq!(second.encode(), second.as_bytes().to_vec().encode());
    assert!(
        ContractArgumentRecord::try_new(vec![0; MAX_CONTRACT_ARGUMENT_RECORD_BYTES + 1]).is_err()
    );
    // A complete otherwise-valid Vec frame separates the owner's size cap
    // from ordinary truncation and generic transport/allocation limits.
    let oversized = vec![0xA5; MAX_CONTRACT_ARGUMENT_RECORD_BYTES + 1];
    let ordinary_frame = norito::encode_canonical(&oversized).unwrap();
    assert_eq!(
        norito::decode_canonical::<Vec<u8>>(&ordinary_frame).unwrap(),
        oversized
    );
    reject_payload::<ContractArgumentRecord>(&oversized.encode(), ncore::default_encode_flags());
    let declared = u64::try_from(MAX_CONTRACT_ARGUMENT_RECORD_BYTES + 1)
        .unwrap()
        .to_le_bytes();
    reject_payload::<ContractArgumentRecord>(&declared, 0);
    let mut truncated = 4_u64.to_le_bytes().to_vec();
    truncated.extend_from_slice(&[1, 2]);
    reject_payload::<ContractArgumentRecord>(&truncated, 0);
    family(rows, "contract_argument_record", first, second);
}

fn contract_alias_values(rows: &mut Vec<Value>) {
    let first = ContractAlias::from_components("router", Some("dex"), "universal").unwrap();
    let second = ContractAlias::from_components("router", None, "universal").unwrap();
    assert_eq!(first.as_ref(), "router::dex.universal");
    assert_eq!(second.as_ref(), "router::universal");
    assert_eq!(first.encode(), first.as_ref().to_owned().encode());
    assert_eq!(second.encode(), second.as_ref().to_owned().encode());
    assert!("router".parse::<ContractAlias>().is_err());
    reject_payload::<ContractAlias>(&"router".to_owned().encode(), ncore::default_encode_flags());
    family(rows, "contract_alias", first, second);
}

fn contract_address_values(rows: &mut Vec<Value>) {
    // Reuse the existing cross-SDK derivation vector; this is a public fixture seed.
    let seed =
        hex::decode("CCF31D85E3B32A4BEA59987CE0C78E3B8E2DB93881468AB2435FE45D5C9DCD53").unwrap();
    let signer = KeyPair::try_from_seed(seed, Algorithm::Ed25519).unwrap();
    let deployer = AccountId::new(signer.public_key().clone());
    let network: NetworkId =
        "hash:32C903E5B3497E34C2B844EBFE8A39C19E6CF8F95D44C1FFB8BA9DCB42F91149#A2F0"
            .parse()
            .unwrap();
    let first = ContractAddress::derive(&network, &deployer, 7, DataSpaceId::UNIVERSAL).unwrap();
    assert_eq!(
        first.as_str(),
        "irohac1qyqqqqqqqqqqqq8y2pcrtkxvkrn5nt74kjjkjcst6kc56qcqa2dqp",
    );
    let second = ContractAddress::derive(&network, &deployer, 8, DataSpaceId::new(9)).unwrap();
    assert_eq!(first.dataspace_id().unwrap(), DataSpaceId::UNIVERSAL);
    assert_eq!(second.dataspace_id().unwrap(), DataSpaceId::new(9));
    assert_eq!(first.encode(), first.as_str().to_owned().encode());
    assert_eq!(second.encode(), second.as_str().to_owned().encode());
    assert!("not-an-address".parse::<ContractAddress>().is_err());
    reject_payload::<ContractAddress>(
        &"not-an-address".to_owned().encode(),
        ncore::default_encode_flags(),
    );
    family(rows, "contract_address", first, second);
}

fn manifest_root_cid_values(rows: &mut Vec<Value>) {
    let first = ManifestRootCid::from_blake3_digest([0xA5; 32]).unwrap();
    let second = ManifestRootCid::from_blake3_digest([0x56; 32]).unwrap();
    assert_eq!(first.encode(), first.as_bytes().encode());
    assert_eq!(second.encode(), second.as_bytes().encode());
    assert_eq!(ManifestRootCid::try_from_slice(first.as_bytes()), Ok(first));
    assert!(ManifestRootCid::try_from_slice(&first.as_bytes()[..35]).is_err());
    for (index, replacement) in [(0, 2), (1, 0x70), (2, 0x12), (3, 31)] {
        let mut malformed = *first.as_bytes();
        malformed[index] = replacement;
        assert!(ManifestRootCid::new(malformed).is_err());
        reject_payload::<ManifestRootCid>(&malformed.encode(), ncore::default_encode_flags());
    }
    let mut inert = *first.as_bytes();
    inert[4..].fill(0);
    assert!(ManifestRootCid::new(inert).is_err());
    assert!(ManifestRootCid::from_blake3_digest([0; 32]).is_err());
    reject_payload::<ManifestRootCid>(&inert.encode(), ncore::default_encode_flags());
    family(rows, "manifest_root_cid", first, second);
}

#[test]
fn manual_scalar_frames_match_capture() {
    let mut rows = Vec::new();
    privacy_requirement_values(&mut rows);
    contract_argument_values(&mut rows);
    contract_alias_values(&mut rows);
    contract_address_values(&mut rows);
    manifest_root_cid_values(&mut rows);
    assert_eq!(rows.len(), 35);
    let evidence = norito::json!({
        "format_version": 1,
        "purpose": "five public manual scalar owners before identity declaration",
        "default_encode_flags": (ncore::default_encode_flags()),
        "rows": rows,
    });
    let expected: Value = norito::json::from_json(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/manual_scalar_identity_frames.json"
    )))
    .expect("immutable pre-declaration scalar capture");
    assert_eq!(evidence, expected);
}
