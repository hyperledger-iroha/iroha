use super::*;
use iroha_crypto::{Algorithm, KeyPair};
fn body() -> GameAdmissionBodyV1 {
    GameAdmissionBodyV1 {
        version: 1,
        participants: (0..2)
            .map(|seed| GameAdmissionParticipantV1 {
                account: AccountId::new(
                    KeyPair::try_from_seed(vec![seed + 10; 32], Algorithm::Ed25519)
                        .unwrap()
                        .public_key()
                        .clone(),
                ),
                input_key: KeyPair::try_from_seed(vec![seed + 1; 32], Algorithm::Ed25519)
                    .unwrap()
                    .public_key()
                    .clone(),
                application_data: vec![seed],
            })
            .collect(),
        wagers: vec![GameAdmissionWagerV1 {
            slot: 0,
            nft_id: "paint$equipment.universal".parse().unwrap(),
            metadata_hash: Hash::new(b"paint metadata"),
        }],
        resources: vec![GameAdmissionResourceV1 {
            slot: 1,
            nft_id: "engine$equipment.universal".parse().unwrap(),
            metadata_hash: Hash::new(b"engine metadata"),
            role_id: Hash::new(b"engine"),
            policy: GameResourceReturnPolicyV1::ReturnToOriginalOwnerAtTerminal,
        }],
    }
}
#[test]
fn compact_admission_roundtrips_bare_and_framed_without_mutable_fields() {
    let value = body();
    value.validate().unwrap();
    let bare = value.encode();
    assert_eq!(
        GameAdmissionBodyV1::decode(&mut bare.as_slice()).unwrap(),
        value
    );
    let framed = norito::to_bytes(&value).unwrap();
    assert_eq!(
        norito::decode_from_bytes::<GameAdmissionBodyV1>(&framed).unwrap(),
        value
    );
    let empty = GameAdmissionBodyV1 {
        version: 1,
        participants: vec![],
        wagers: vec![],
        resources: vec![],
    };
    empty.validate().unwrap();
}
#[test]
fn compact_admission_rejects_aliases_colliding_nfts_roles_and_unbounded_data() {
    for kind in 0..10 {
        let mut invalid = body();
        match kind {
            0 => invalid.version = 2,
            1 => invalid.participants[1].account = invalid.participants[0].account.clone(),
            2 => invalid.participants[1].input_key = invalid.participants[0].input_key.clone(),
            3 => {
                invalid.participants[0].application_data =
                    vec![0; GAME_ADMISSION_MAX_PARTICIPANT_DATA_BYTES_V1 + 1]
            }
            4 => invalid.wagers[0].slot = 2,
            5 => invalid.resources[0].nft_id = invalid.wagers[0].nft_id.clone(),
            6 => invalid.resources.push(invalid.resources[0].clone()),
            7 => invalid.resources[0].slot = 2,
            8 => invalid.wagers.push(invalid.wagers[0].clone()),
            _ => {
                invalid.participants[0].input_key =
                    KeyPair::try_from_seed(vec![7; 32], Algorithm::Secp256k1)
                        .unwrap()
                        .public_key()
                        .clone()
            }
        }
        assert!(invalid.validate().is_err(), "mutation {kind}");
    }
}
#[test]
#[cfg(feature = "json")]
fn compact_admission_json_rejects_omitted_authorization_vectors_and_mutable_record_fields() {
    let value = norito::json::to_value(&body()).unwrap();
    for field in ["version", "participants", "wagers", "resources"] {
        let mut missing = value.clone();
        missing.as_object_mut().unwrap().remove(field);
        assert!(norito::json::from_value::<GameAdmissionBodyV1>(missing).is_err());
    }
    let mut leaked = value;
    leaked
        .as_object_mut()
        .unwrap()
        .get_mut("participants")
        .unwrap()
        .as_array_mut()
        .unwrap()[0]
        .as_object_mut()
        .unwrap()
        .insert("dnf_at_tick".into(), norito::json::Value::Null);
    assert!(norito::json::from_value::<GameAdmissionBodyV1>(leaked).is_err());
}

#[test]
fn ambiguous_typed_nft_identities_are_rejected_before_admission() {
    use crate::game_resources::identity_test_support::assert_ambiguous_domain_label_rejected;
    use crate::{domain::DomainId, nft::NftId};

    assert!("kit$art.gallery.universal".parse::<NftId>().is_err());
    for (domain, dataspace, label) in [
        ("art-gallery", "universal", "art-gallery"),
        ("art", "gallery-universal", "gallery-universal"),
    ] {
        // Ambiguous components are rejected before a typed NFT can exist.
        assert!(DomainId::try_new(domain.replace('-', "."), dataspace.replace('-', ".")).is_err());
        let nft_id = NftId::new(
            DomainId::try_new(domain, dataspace).unwrap(),
            "kit".parse().unwrap(),
        );
        validate_game_nft_identity_v1(&nft_id).unwrap();
        assert_ambiguous_domain_label_rejected(&nft_id, label);
        let mut wager = body();
        wager.wagers[0].nft_id = nft_id.clone();
        wager.validate().unwrap();
        assert_ambiguous_domain_label_rejected(&wager, label);
        let mut resource = body();
        resource.resources[0].nft_id = nft_id.clone();
        resource.validate().unwrap();
        assert_ambiguous_domain_label_rejected(&resource, label);
        let clause = GameResourceReservationClauseV1 {
            nft_id: nft_id.clone(),
            expected_metadata_hash: Hash::new(b"metadata"),
            role_id: Hash::new(b"role"),
            policy: GameResourceReturnPolicyV1::ReturnToOriginalOwnerAtTerminal,
        };
        let clauses = vec![clause.clone()];
        validate_resource_clauses_v1(&clauses).unwrap();
        assert_ambiguous_domain_label_rejected(&clauses, label);
        let requirements = vec![GameResourceRequirementV1 {
            nft_id,
            expected_metadata_hash: clause.expected_metadata_hash,
            role_id: clause.role_id,
            policy: clause.policy,
        }];
        validate_resource_requirements_v1(&requirements).unwrap();
        assert_ambiguous_domain_label_rejected(&requirements, label);
    }
    let control = NftId::new(
        DomainId::try_new("art", "universal").unwrap(),
        "kit".parse().unwrap(),
    );
    validate_game_nft_identity_v1(&control).unwrap();
    let mut admissible = body();
    admissible.wagers[0].nft_id = control;
    admissible.validate().unwrap();
}
