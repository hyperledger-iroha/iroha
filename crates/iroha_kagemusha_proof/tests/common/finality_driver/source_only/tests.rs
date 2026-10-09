//! Input-custody tests; these byte fixtures never claim canonical genesis or graph admission.

use super::*;

#[test]
fn pinned_setup_requires_exact_unique_bounded_originals() {
    let root = std::env::temp_dir().join(format!("kg-source-only-inputs-{}", std::process::id()));
    fs::create_dir(&root).unwrap();
    let records: Vec<_> = ORIGINALS.iter().enumerate().map(|(index, name)| {
        let bytes = vec![u8::try_from(index).unwrap(); index + 1];
        fs::write(root.join(name), &bytes).unwrap();
        norito::json!({"name": (*name), "bytes": (bytes.len()), "sha256": (hex_out(&Sha256::digest(&bytes)))})
    }).collect();
    let setup = norito::json!({"originals": (records.clone())});
    assert_eq!(read_originals(&root, &setup).unwrap().len(), 9);
    let mut missing = records.clone();
    missing.pop();
    assert!(read_originals(&root, &norito::json!({"originals": missing})).is_err());
    let mut duplicate = records.clone();
    duplicate[1] = duplicate[0].clone();
    assert!(read_originals(&root, &norito::json!({"originals": duplicate})).is_err());
    let mut escaped = records.clone();
    escaped[0].as_object_mut().unwrap().insert(
        "name".into(),
        Value::String("../signed-genesis.wire".into()),
    );
    assert!(read_originals(&root, &norito::json!({"originals": escaped})).is_err());
    let mut excess = records.clone();
    excess[0]
        .as_object_mut()
        .unwrap()
        .insert("bytes".into(), norito::json!((ORIGINAL_MAX + 1)));
    assert!(read_originals(&root, &norito::json!({"originals": excess})).is_err());
    fs::write(root.join(ORIGINALS[0]), [0, 1]).unwrap();
    assert!(read_originals(&root, &setup).is_err());
    let bytes = norito::json::to_json(&setup).unwrap().into_bytes();
    fs::write(root.join("setup.json"), &bytes).unwrap();
    assert!(validate(&root, [0; 32]).is_err());
    assert!(validate(&root, [1; 32]).is_err());
    assert!(validate(&root, Sha256::digest(bytes).into()).is_err());
    fs::remove_dir_all(root).unwrap();
}

/// Rehashed in-memory mutations cross the semantic checks after genuine pinned intake.
/// They are never published as source inputs or treated as authenticated setup evidence.
pub(super) fn check_verified_mutations(selected: &Setup) {
    let setup: Value = norito::json::from_slice(&selected.manifest).unwrap();
    assert_eq!(
        validate_contents(&setup, &selected.originals).unwrap(),
        selected.anchor
    );
    let mut originals = selected.originals.clone();
    let mut capture: Value = norito::json::from_slice(&originals["capture.json"]).unwrap();
    let block_time = capture["history_anchor"]["parameters"][0].as_u64().unwrap();
    capture
        .as_object_mut()
        .unwrap()
        .get_mut("history_anchor")
        .unwrap()
        .as_object_mut()
        .unwrap()
        .get_mut("parameters")
        .unwrap()
        .as_array_mut()
        .unwrap()[0] = norito::json!((block_time + 1));
    originals.insert(
        "capture.json".into(),
        norito::json::to_json(&capture).unwrap().into_bytes(),
    );
    assert!(
        validate_contents(&setup, &originals).is_err(),
        "changed complete anchor"
    );

    let mut originals = selected.originals.clone();
    let mut proofs: [SumeragiFinalityProof; 2] = norito::decode_canonical_with_limits(
        &originals["registration-proof.norito"],
        norito::canonical_decode_limits(originals["registration-proof.norito"].len()),
    )
    .unwrap();
    proofs.swap(0, 1);
    originals.insert(
        "registration-proof.norito".into(),
        norito::encode_canonical(&proofs).unwrap(),
    );
    assert!(
        validate_contents(&setup, &originals).is_err(),
        "reversed native prefix"
    );

    let mut originals = selected.originals.clone();
    let mut wire = originals["signed-genesis.wire"].clone();
    let index = wire.len() / 2;
    wire[index] ^= 1;
    originals.insert("signed-genesis.wire".into(), wire);
    assert!(
        validate_contents(&setup, &originals).is_err(),
        "changed signed genesis"
    );

    let mut originals = selected.originals.clone();
    let mut raw: Value = norito::json::from_slice(&originals["genesis-manifest.json"]).unwrap();
    raw.as_object_mut()
        .unwrap()
        .insert("chain".into(), norito::json!("foreign-selected-chain"));
    originals.insert(
        "genesis-manifest.json".into(),
        norito::json::to_json(&raw).unwrap().into_bytes(),
    );
    assert!(
        validate_contents(&setup, &originals).is_err(),
        "rebound raw manifest"
    );

    let mut originals = selected.originals.clone();
    originals.insert(
        "account-c.norito".into(),
        originals["account-b.norito"].clone(),
    );
    assert!(
        validate_contents(&setup, &originals).is_err(),
        "duplicate recipient identity"
    );

    let mut originals = selected.originals.clone();
    let mut asset: KagemushaWalletAssetScopeV1 = norito::decode_canonical_with_limits(
        &originals["asset.norito"],
        norito::canonical_decode_limits(originals["asset.norito"].len()),
    )
    .unwrap();
    asset.scale += 1;
    originals.insert(
        "asset.norito".into(),
        norito::encode_canonical(&asset).unwrap(),
    );
    let mut rebound = setup.clone();
    rebound.as_object_mut().unwrap().insert(
        "asset_digest_hex".into(),
        norito::json!((hex_out(&asset.asset_digest()))),
    );
    assert!(
        validate_contents(&rebound, &originals).is_err(),
        "rehashing cannot change signed asset scale"
    );

    let mut rebound = setup;
    rebound
        .as_object_mut()
        .unwrap()
        .insert("network_hex".into(), norito::json!((hex_out(&[0; 32]))));
    assert!(
        validate_contents(&rebound, &selected.originals).is_err(),
        "foreign network pin"
    );
}
