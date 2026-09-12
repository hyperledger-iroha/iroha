//! Complete original-codec frame observations at FASTPQ's public wire boundaries.

use norito::{NoritoDeserialize, NoritoSchema, NoritoSerialize, json::Value};

use crate::{
    GOLDILOCKS_MODULUS_V1, GoldilocksFp4V1, OperationKind, PublicInputs, StateTransition,
    TransitionBatch, proof::PublicIO,
};

fn assert_frame<T>(row: &Value, shape: &str, value: T)
where
    T: NoritoSerialize + for<'de> NoritoDeserialize<'de> + std::fmt::Debug + Eq,
{
    assert_eq!(row["shape"].as_str(), Some(shape));
    assert_eq!(
        row["nominal"].as_str(),
        Some(<T as NoritoSchema>::nominal_name().as_str())
    );
    let hash = hex::encode(norito::schema::identity::frame_hash::<T>());
    assert_eq!(row["serialize_schema_hash"].as_str(), Some(hash.as_str()));
    assert_eq!(row["deserialize_schema_hash"].as_str(), Some(hash.as_str()));
    let original = hex::decode(row["frame_hex"].as_str().expect("captured frame")).unwrap();
    assert_eq!(norito::to_bytes(&value).unwrap(), original, "{shape}");
    assert_eq!(norito::decode_from_bytes::<T>(&original).unwrap(), value);

    let mut wrong_identity = original.clone();
    wrong_identity[6] ^= 1;
    assert!(matches!(
        norito::decode_from_bytes::<T>(&wrong_identity),
        Err(norito::Error::SchemaMismatch)
    ));
    assert!(norito::decode_from_bytes::<T>(&original[..original.len() - 1]).is_err());
    let mut trailing_byte = original;
    trailing_byte.push(0);
    assert!(norito::decode_from_bytes::<T>(&trailing_byte).is_err());
}

fn assert_group<T>(group: &Value, case: &str, value: T)
where
    T: NoritoSerialize + for<'de> NoritoDeserialize<'de> + Clone + std::fmt::Debug + Eq,
{
    assert_eq!(group["case"].as_str(), Some(case));
    let frames = group["frames"].as_array().unwrap();
    assert_eq!(frames.len(), 5);
    assert_frame(&frames[0], "root", value.clone());
    assert_frame(&frames[1], "option_none", None::<T>);
    assert_frame(&frames[2], "option_some", Some(value.clone()));
    assert_frame(&frames[3], "vec_empty", Vec::<T>::new());
    assert_frame(&frames[4], "vec_two", vec![value.clone(), value]);
}

#[test]
fn public_frames_match_original_codec_and_reject_malformed_envelopes() {
    let fixture: Value =
        norito::json::from_str(include_str!("../tests/fixtures/public_frames.json")).unwrap();
    assert_eq!(
        fixture["schema"].as_str(),
        Some("iroha.fastpq.public-frame-observations.v1")
    );
    assert_eq!(
        fixture["layout_flags"].as_u64(),
        Some(u64::from(norito::core::default_encode_flags()))
    );
    let groups = fixture["groups"].as_array().unwrap();
    assert_eq!(groups.len(), 9);
    let scalar = GoldilocksFp4V1::new([1, 2, GOLDILOCKS_MODULUS_V1 - 1, 0]).unwrap();
    assert_group(&groups[0], "goldilocks_fp4", scalar);
    let public_io = PublicIO {
        dsid: [1; 16],
        slot: 43,
        old_root: [2; 32],
        new_root: [3; 32],
        perm_root: [4; 32],
        tx_set_hash: [5; 32],
        ordering_hash: [6; 32],
    };
    assert_group(&groups[1], "public_io", public_io);
    let operations = [
        ("transfer", OperationKind::Transfer),
        ("mint", OperationKind::Mint),
        ("burn", OperationKind::Burn),
        (
            "role_grant",
            OperationKind::RoleGrant {
                role_id: [7; 32],
                permission_id: [8; 32],
                epoch: 44,
            },
        ),
        (
            "role_revoke",
            OperationKind::RoleRevoke {
                role_id: [9; 32],
                permission_id: [10; 32],
                epoch: 45,
            },
        ),
        ("metadata", OperationKind::MetaSet),
    ];
    let mut transitions = Vec::new();
    for (index, (case, operation)) in operations.into_iter().enumerate() {
        let transition = StateTransition::new(vec![1, 2, 3], vec![4, 5], vec![6, 7, 8], operation);
        assert_group(&groups[index + 2], case, transition.clone());
        transitions.push(transition);
    }
    let batch = TransitionBatch {
        parameter: fastpq_isi::CANONICAL_PARAMETER_SETS[0].name.to_owned(),
        public_inputs: PublicInputs {
            dsid: public_io.dsid,
            slot: public_io.slot,
            old_root: public_io.old_root,
            new_root: public_io.new_root,
            perm_root: public_io.perm_root,
            tx_set_hash: public_io.tx_set_hash,
        },
        transitions,
        metadata: [("capture".to_owned(), vec![11, 12])].into_iter().collect(),
    };
    assert_group(&groups[8], "transition_batch", batch);
}
