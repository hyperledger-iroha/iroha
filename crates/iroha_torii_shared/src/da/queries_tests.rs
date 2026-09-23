//! JSON and exact Norito-frame contracts for the shared DA query DTO owner.

use super::*;
use iroha_crypto::{Algorithm, Hash, KeyPair, Signature};
use iroha_data_model::da::{
    commitment::{
        DaCommitmentBundle, DaCommitmentRecord, DaProofScheme, commitment_leaf_hash,
        commitment_merkle_commitment,
    },
    types::{BlobDigest, RetentionPolicy},
};
use iroha_model_base::topology::LaneId;
use norito::json::{self, JsonDeserialize, JsonSerialize, Value};

fn snapshot() -> DaListSnapshot {
    DaListSnapshot {
        block_height: u64::MAX,
        block_hash: Some(HashOf::from_untyped_unchecked(Hash::prehashed([0xA1; 32]))),
    }
}

fn commitment_cursor() -> DaCommitmentListCursor {
    DaCommitmentListCursor {
        snapshot: snapshot(),
        after: DaCommitmentKey {
            lane_id: LaneId::new(4),
            epoch: u64::MAX,
            sequence: u64::MAX - 1,
        },
    }
}

fn pin_cursor() -> DaPinIntentListCursor {
    DaPinIntentListCursor {
        snapshot: snapshot(),
        after: DaCommitmentLocation {
            block_height: u64::MAX,
            index_in_bundle: u32::MAX,
        },
    }
}

fn commitment_proof() -> DaCommitmentProof {
    let signer = KeyPair::try_from_seed(vec![0xD3; 32], Algorithm::Ed25519)
        .expect("deterministic shared DA DTO fixture key");
    let commitment = DaCommitmentRecord {
        lane_id: LaneId::new(4),
        epoch: u64::MAX,
        sequence: u64::MAX - 1,
        client_blob_id: BlobDigest::new([0x11; 32]),
        manifest_hash: ManifestDigest::new([0x22; 32]),
        proof_scheme: DaProofScheme::MerkleSha256,
        chunk_root: Hash::prehashed([0x33; 32]),
        proof_digest: None,
        retention_class: RetentionPolicy::default(),
        storage_ticket: StorageTicketId::new([0x44; 32]),
        acknowledgement_sig: Signature::try_new(signer.private_key(), b"shared DA DTO fixture")
            .expect("sign non-secret fixture"),
    };
    let root = commitment_leaf_hash(&commitment);
    DaCommitmentProof {
        commitment,
        location: DaCommitmentLocation {
            block_height: u64::MAX,
            index_in_bundle: 0,
        },
        bundle_hash: commitment_merkle_commitment(DaCommitmentBundle::VERSION_V1, 1, &root),
        bundle_len: 1,
        root,
        path: Vec::new(),
    }
}

fn assert_contract<T>(value: &T, identity: &str)
where
    T: norito::NoritoSerialize
        + for<'de> norito::NoritoDeserialize<'de>
        + JsonSerialize
        + JsonDeserialize,
{
    let mut unknown = json::to_value(value).expect("typed JSON value");
    unknown
        .as_object_mut()
        .unwrap()
        .insert("retired_field".to_owned(), Value::Null);
    assert!(
        json::from_value::<T>(unknown).is_err(),
        "unknown field on {identity}"
    );

    let json = json::to_vec(value).expect("encode typed JSON");
    let decoded: T = json::from_slice(&json).expect("decode typed JSON");
    assert_eq!(
        json::to_value(&decoded).expect("re-encode typed JSON"),
        json::to_value(value).expect("original typed JSON"),
    );

    assert_eq!(T::nominal_name(), identity);
    assert_eq!(T::frame_name(), identity);
    let frame = norito::encode_canonical(value).expect("encode canonical frame");
    assert_eq!(&frame[6..22], &norito::schema::identity::frame_hash::<T>());
    let decoded: T = norito::decode_canonical(&frame).expect("decode exact canonical frame");
    assert_eq!(
        norito::codec::encode_adaptive(&decoded),
        norito::codec::encode_adaptive(value),
    );
    let mut wrong_owner = frame.clone();
    wrong_owner[6] ^= 1;
    assert!(matches!(
        norito::decode_canonical::<T>(&wrong_owner),
        Err(norito::Error::SchemaMismatch)
    ));
    assert!(norito::decode_canonical::<T>(&frame[..frame.len() - 1]).is_err());
    let mut trailing = frame;
    trailing.push(0);
    assert!(norito::decode_canonical::<T>(&trailing).is_err());
}

#[test]
fn all_commitment_dtos_keep_their_exact_json_and_frame_contracts() {
    assert_contract(&snapshot(), "iroha_torii::da::commitments::DaListSnapshot");
    assert_contract(
        &commitment_cursor(),
        "iroha_torii::da::commitments::DaCommitmentListCursor",
    );
    assert_contract(
        &DaCommitmentListRequest {
            limit: NonZeroU64::new(1_000),
            cursor: Some(commitment_cursor()),
        },
        "iroha_torii::da::commitments::DaCommitmentListRequest",
    );
    assert_contract(
        &DaCommitmentProofRequest {
            manifest_hash: Some(ManifestDigest::new([0x22; 32])),
            lane_id: Some(4),
            epoch: Some(u64::MAX),
            sequence: Some(u64::MAX - 1),
        },
        "iroha_torii::da::commitments::DaCommitmentProofRequest",
    );
    let proof = commitment_proof();
    assert_contract(
        &DaCommitmentListResponse {
            policies: DaProofPolicyBundle::new(Vec::new()),
            commitments: vec![DaCommitmentWithLocation {
                commitment: proof.commitment.clone(),
                location: proof.location,
            }],
            next_cursor: Some(commitment_cursor()),
        },
        "iroha_torii::da::commitments::DaCommitmentListResponse",
    );
    assert_contract(
        &DaCommitmentProofResponse {
            policies: DaProofPolicyBundle::new(Vec::new()),
            proof,
        },
        "iroha_torii::da::commitments::DaCommitmentProofResponse",
    );
    for response in [
        DaCommitmentVerifyResponse {
            valid: true,
            error: None,
        },
        DaCommitmentVerifyResponse {
            valid: false,
            error: Some("proof does not match committed block".to_owned()),
        },
    ] {
        assert_contract(
            &response,
            "iroha_torii::da::commitments::DaCommitmentVerifyResponse",
        );
    }
}

#[test]
fn all_pin_intent_dtos_keep_their_exact_json_and_frame_contracts() {
    assert_contract(
        &pin_cursor(),
        "iroha_torii::da::pin_intents::DaPinIntentListCursor",
    );
    assert_contract(
        &DaPinIntentListRequest {
            limit: NonZeroU64::new(1_000),
            cursor: Some(pin_cursor()),
        },
        "iroha_torii::da::pin_intents::DaPinIntentListRequest",
    );
    assert_contract(
        &DaPinIntentQueryRequest {
            manifest_hash: Some(ManifestDigest::new([0x22; 32])),
            storage_ticket: Some(StorageTicketId::new([0x44; 32])),
            alias: Some("δοκιμή".to_owned()),
            lane_id: Some(4),
            epoch: Some(u64::MAX),
            sequence: Some(u64::MAX - 1),
        },
        "iroha_torii::da::pin_intents::DaPinIntentQueryRequest",
    );
    assert_contract(
        &DaPinIntentListResponse {
            intents: Vec::new(),
            next_cursor: Some(pin_cursor()),
        },
        "iroha_torii::da::pin_intents::DaPinIntentListResponse",
    );
    for response in [
        DaPinIntentVerifyResponse {
            valid: true,
            error: None,
        },
        DaPinIntentVerifyResponse {
            valid: false,
            error: Some("block is unavailable".to_owned()),
        },
    ] {
        assert_contract(
            &response,
            "iroha_torii::da::pin_intents::DaPinIntentVerifyResponse",
        );
    }
}

#[test]
fn cursor_canonicality_requires_a_hash_exactly_for_nonempty_chain() {
    let hash = snapshot().block_hash;
    for (block_height, block_hash, canonical) in [
        (0, None, true),
        (0, hash, false),
        (1, None, false),
        (1, hash, true),
        (u64::MAX, hash, true),
    ] {
        assert_eq!(
            DaListSnapshot {
                block_height,
                block_hash,
            }
            .is_canonical(),
            canonical,
        );
    }
}

#[test]
fn cursor_json_retains_exact_field_names_and_full_unsigned_ranges() {
    let cursor = commitment_cursor();
    let encoded = json::to_value(&cursor).expect("cursor JSON");
    assert_eq!(encoded.as_object().unwrap().len(), 2);
    assert_eq!(encoded["snapshot"]["block_height"], Value::from(u64::MAX));
    assert_eq!(encoded["after"]["epoch"], Value::from(u64::MAX));
    assert_eq!(encoded["after"]["sequence"], Value::from(u64::MAX - 1));
    let pin = json::to_value(&pin_cursor()).expect("pin cursor JSON");
    assert_eq!(pin["after"]["index_in_bundle"], Value::from(u32::MAX));
    let invalid_limit = br#"{"limit":0,"cursor":null}"#;
    assert!(json::from_slice::<DaCommitmentListRequest>(invalid_limit).is_err());
    assert!(json::from_slice::<DaPinIntentListRequest>(invalid_limit).is_err());
}

#[test]
fn filtered_empty_pages_keep_their_continuation_and_explicit_nulls() {
    let page = DaPinIntentListResponse {
        intents: Vec::new(),
        next_cursor: Some(pin_cursor()),
    };
    let encoded = json::to_value(&page).expect("filtered page JSON");
    assert!(encoded["intents"].as_array().unwrap().is_empty());
    assert!(!encoded["next_cursor"].is_null());
    let terminal = DaPinIntentListResponse {
        intents: Vec::new(),
        next_cursor: None,
    };
    let encoded = json::to_value(&terminal).expect("terminal page JSON");
    assert_eq!(encoded.as_object().unwrap().len(), 2);
    assert!(encoded.as_object().unwrap().contains_key("next_cursor"));
    assert!(encoded["next_cursor"].is_null());
    let success = json::to_value(&DaCommitmentVerifyResponse {
        valid: true,
        error: None,
    })
    .expect("verification JSON");
    assert!(success.as_object().unwrap().contains_key("error"));
    assert!(success["error"].is_null());
}

#[test]
fn canonical_nullable_response_and_snapshot_fields_are_required() {
    fn missing<T: JsonSerialize + JsonDeserialize>(value: &T, field: &str) {
        let mut value = json::to_value(value).expect("typed response");
        value
            .as_object_mut()
            .unwrap()
            .remove(field)
            .expect("canonical field present");
        assert!(json::from_value::<T>(value).is_err(), "missing {field}");
    }
    missing(
        &DaListSnapshot {
            block_height: 0,
            block_hash: None,
        },
        "block_hash",
    );
    missing(
        &DaCommitmentListResponse {
            policies: DaProofPolicyBundle::new(Vec::new()),
            commitments: Vec::new(),
            next_cursor: None,
        },
        "next_cursor",
    );
    missing(
        &DaPinIntentListResponse {
            intents: Vec::new(),
            next_cursor: None,
        },
        "next_cursor",
    );
    missing(
        &DaCommitmentVerifyResponse {
            valid: true,
            error: None,
        },
        "error",
    );
    missing(
        &DaPinIntentVerifyResponse {
            valid: true,
            error: None,
        },
        "error",
    );
}

#[test]
fn list_request_validation_bounds_raw_scan_and_checks_cursor_shape() {
    for (limit, expected) in [
        (None, Ok(100)),
        (NonZeroU64::new(1), Ok(1)),
        (NonZeroU64::new(1_000), Ok(1_000)),
        (
            NonZeroU64::new(1_001),
            Err(DaQueryValidationError::LimitOutOfRange { provided: 1_001 }),
        ),
        (
            NonZeroU64::new(u64::MAX),
            Err(DaQueryValidationError::LimitOutOfRange { provided: u64::MAX }),
        ),
    ] {
        let commitments = DaCommitmentListRequest {
            limit,
            cursor: None,
        };
        let pins = DaPinIntentListRequest {
            limit,
            cursor: None,
        };
        assert_eq!(commitments.page_size(), expected);
        assert_eq!(pins.page_size(), expected);
        assert_eq!(commitments.validate(), expected.map(|_| ()));
        assert_eq!(pins.validate(), expected.map(|_| ()));
    }
    let hash = snapshot().block_hash;
    for (block_height, block_hash, canonical) in [
        (0, None, true),
        (0, hash, false),
        (1, None, false),
        (1, hash, true),
    ] {
        let snapshot = DaListSnapshot {
            block_height,
            block_hash,
        };
        let commitments = DaCommitmentListRequest {
            limit: None,
            cursor: Some(DaCommitmentListCursor {
                snapshot,
                ..commitment_cursor()
            }),
        };
        let pins = DaPinIntentListRequest {
            limit: None,
            cursor: Some(DaPinIntentListCursor {
                snapshot,
                after: DaCommitmentLocation {
                    block_height: 1,
                    index_in_bundle: 0,
                },
            }),
        };
        let expected = if canonical {
            Ok(())
        } else {
            Err(DaQueryValidationError::NonCanonicalSnapshot)
        };
        assert_eq!(commitments.validate(), expected);
        assert_eq!(
            pins.validate(),
            if canonical && block_height == 0 {
                Err(DaQueryValidationError::CursorOutsideSnapshot)
            } else {
                expected
            }
        );
    }
}

#[test]
fn pin_cursor_location_must_belong_to_its_snapshot() {
    for (tip, location, valid) in [
        (0, 0, false),
        (0, 1, false),
        (20, 0, false),
        (20, 1, true),
        (20, 20, true),
        (20, 21, false),
        (u64::MAX, u64::MAX, true),
    ] {
        let cursor = DaPinIntentListCursor {
            snapshot: DaListSnapshot {
                block_height: tip,
                block_hash: (tip > 0).then_some(snapshot().block_hash.unwrap()),
            },
            after: DaCommitmentLocation {
                block_height: location,
                index_in_bundle: u32::MAX,
            },
        };
        let expected = if valid {
            Ok(())
        } else {
            Err(DaQueryValidationError::CursorOutsideSnapshot)
        };
        assert_eq!(cursor.validate(), expected);
        assert_eq!(
            DaPinIntentListRequest {
                limit: None,
                cursor: Some(cursor),
            }
            .validate(),
            expected
        );
    }
    assert!(DaPinIntentListRequest::default().validate().is_ok());
}

#[test]
fn commitment_selector_requires_one_lookup_key_and_retains_extra_constraints() {
    for mask in 0_u8..8 {
        let tuple = DaCommitmentProofRequest {
            manifest_hash: None,
            lane_id: (mask & 1 != 0).then_some(4),
            epoch: (mask & 2 != 0).then_some(9),
            sequence: (mask & 4 != 0).then_some(12),
        };
        assert_eq!(
            tuple.validate(),
            if mask == 7 {
                Ok(())
            } else {
                Err(DaQueryValidationError::MissingCommitmentSelector)
            },
        );
        assert_eq!(
            DaCommitmentProofRequest {
                manifest_hash: Some(ManifestDigest::new([0xA2; 32])),
                ..tuple
            }
            .validate(),
            Ok(())
        );
    }
}

#[test]
fn pin_selector_requires_one_lookup_key_and_bounds_every_alias_in_utf8_bytes() {
    for mask in 0_u8..8 {
        let tuple = DaPinIntentQueryRequest {
            lane_id: (mask & 1 != 0).then_some(4),
            epoch: (mask & 2 != 0).then_some(9),
            sequence: (mask & 4 != 0).then_some(12),
            ..DaPinIntentQueryRequest::default()
        };
        assert_eq!(
            tuple.validate(),
            if mask == 7 {
                Ok(())
            } else {
                Err(DaQueryValidationError::MissingPinIntentSelector)
            },
        );
        for request in [
            DaPinIntentQueryRequest {
                manifest_hash: Some(ManifestDigest::new([0xA2; 32])),
                ..tuple.clone()
            },
            DaPinIntentQueryRequest {
                storage_ticket: Some(StorageTicketId::new([0xA3; 32])),
                ..tuple.clone()
            },
            DaPinIntentQueryRequest {
                alias: Some("pin/日".to_owned()),
                ..tuple
            },
        ] {
            assert_eq!(request.validate(), Ok(()));
        }
    }
    let exact_bound = "é".repeat(MAX_DA_PIN_INTENT_ALIAS_BYTES / 2);
    assert_eq!(
        DaPinIntentQueryRequest {
            alias: Some(exact_bound.clone()),
            ..DaPinIntentQueryRequest::default()
        }
        .validate(),
        Ok(())
    );
    let too_long = format!("{exact_bound}a");
    assert_eq!(
        DaPinIntentQueryRequest {
            alias: Some(too_long),
            storage_ticket: Some(StorageTicketId::new([0xA3; 32])),
            ..DaPinIntentQueryRequest::default()
        }
        .validate(),
        Err(DaQueryValidationError::AliasTooLong {
            provided: MAX_DA_PIN_INTENT_ALIAS_BYTES + 1,
        })
    );
}

#[test]
fn validation_errors_preserve_machine_readable_values_and_explanations() {
    for (error, text) in [
        (
            DaQueryValidationError::LimitOutOfRange { provided: 1_001 },
            "1001",
        ),
        (DaQueryValidationError::NonCanonicalSnapshot, "block hash"),
        (
            DaQueryValidationError::MissingCommitmentSelector,
            "manifest hash",
        ),
        (
            DaQueryValidationError::MissingPinIntentSelector,
            "storage ticket",
        ),
        (
            DaQueryValidationError::AliasTooLong { provided: 257 },
            "257 UTF-8 bytes",
        ),
    ] {
        assert!(error.to_string().contains(text));
        let boxed: Box<dyn std::error::Error> = Box::new(error);
        assert_eq!(boxed.downcast_ref::<DaQueryValidationError>(), Some(&error));
    }
}
