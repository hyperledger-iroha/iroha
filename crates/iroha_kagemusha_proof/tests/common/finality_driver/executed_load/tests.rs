//! Bounded capture custody and counted-path regressions; no finality grant fixtures.
use super::*;

#[test]
fn counted_path_preserves_nonzero_index_and_odd_missing_sibling() {
    use iroha_crypto::MerkleTree;
    let leaves: Vec<HashOf<EventBox>> = (0_u8..5)
        .map(|n| HashOf::from_untyped_unchecked(Hash::new([n])))
        .collect();
    let tree: MerkleTree<EventBox> = leaves.iter().copied().collect();
    for index in 0..5 {
        let original = tree.get_proof(index).unwrap();
        let mut siblings = [[0_u8; 32]; 32];
        for (out, hash) in siblings.iter_mut().zip(original.audit_path()) {
            if let Some(hash) = hash {
                *out = *hash.as_ref();
            }
        }
        let reconstructed = event_path(5, index, &siblings).unwrap();
        assert_eq!(reconstructed, original);
        assert!(reconstructed.verify(&leaves[index as usize], &tree.commitment().unwrap()));
        let mut changed = siblings;
        changed[31] = [1; 32];
        assert!(
            event_path(5, index, &changed).is_err(),
            "exhausted path is canonical zero"
        );
    }
}

#[test]
fn counted_path_rejects_geometry_absence_and_unmarked_hashes() {
    let mut siblings = [[0_u8; 32]; 32];
    assert!(event_path(0, 0, &siblings).is_err());
    assert!(event_path((1_u64 << 32) + 1, 0, &siblings).is_err());
    assert!(event_path(1, 1, &siblings).is_err());
    assert!(
        event_path(2, 0, &siblings).is_err(),
        "present sibling cannot be absent"
    );
    siblings[0] = [2; 32];
    assert!(
        event_path(2, 0, &siblings).is_err(),
        "hash marker never repaired"
    );
    siblings[0] = [1; 32];
    assert!(
        event_path(3, 2, &siblings).is_err(),
        "odd missing sibling must be zero"
    );
    assert!(event_path(1, 0, &siblings).is_err());
}

#[test]
fn executed_originals_are_exact_bounded_and_unique() {
    let root = std::env::temp_dir().join(format!("kg-executed-originals-{}", std::process::id()));
    fs::create_dir(&root).unwrap();
    fs::write(root.join("a"), b"exact").unwrap();
    let row =
        norito::json!({"name": "a", "bytes": 5, "sha256": (hex_out(&Sha256::digest(b"exact")))});
    let original = norito::json!({"originals": [(row.clone())]});
    assert_eq!(
        originals(&root, &original, "originals", &[("a", 5)]).unwrap()["a"],
        b"exact"
    );
    assert!(originals(&root, &original, "originals", &[("a", 4)]).is_err());
    assert!(
        originals(
            &root,
            &norito::json!({"originals": [(row.clone()), row]}),
            "originals",
            &[("a", 5), ("b", 5)]
        )
        .is_err()
    );
    assert!(originals(&root, &original, "originals", &[("../a", 5)]).is_err());
    assert!(pinned(&root.join("a"), 5, [0; 32]).is_err());
    fs::write(root.join("a"), b"other").unwrap();
    assert!(originals(&root, &original, "originals", &[("a", 5)]).is_err());
    fs::remove_dir_all(root).unwrap();
}

/// Semantic mutations of actual independently selected execution, not fabricated grants.
pub(super) fn check_verified_mutations(selected: &Executed) {
    let json: Value = norito::json::from_slice(&selected.capture).unwrap();
    let rejects = |mutated: &Value, originals: &BTreeMap<String, Vec<u8>>| {
        assert!(
            history(
                &selected.setup,
                mutated,
                originals,
                &selected.receipt,
                &mut native(&selected.setup).unwrap()
            )
            .is_err()
        );
    };
    let mut reversed = json.clone();
    reversed
        .as_object_mut()
        .unwrap()
        .get_mut("blocks")
        .unwrap()
        .as_array_mut()
        .unwrap()
        .swap(0, 1);
    rejects(&reversed, &selected.originals);
    let mut omitted = json.clone();
    omitted
        .as_object_mut()
        .unwrap()
        .get_mut("blocks")
        .unwrap()
        .as_array_mut()
        .unwrap()
        .remove(1);
    rejects(&omitted, &selected.originals);
    for field in [
        "result_preimage_hex",
        "result_hash_hex",
        "commit_vote_preimage_hex",
        "qc_bitmap_hex",
        "qc_aggregate_signature_hex",
    ] {
        let mut changed = json.clone();
        let row = &mut changed
            .as_object_mut()
            .unwrap()
            .get_mut("blocks")
            .unwrap()
            .as_array_mut()
            .unwrap()[1];
        let mut bytes = bytes(row, field).unwrap();
        bytes[0] ^= 1;
        row.as_object_mut()
            .unwrap()
            .insert(field.into(), hex_out(&bytes).into());
        rejects(&changed, &selected.originals);
    }
    for field in ["event_commitment_count", "event_index"] {
        let mut changed = json.clone();
        let row = changed.as_object_mut().unwrap().get_mut("load").unwrap();
        let current = value(row, field).unwrap().as_u64().unwrap();
        row.as_object_mut()
            .unwrap()
            .insert(field.into(), norito::json!((current + 1)));
        rejects(&changed, &selected.originals);
    }
    let mut changed = json.clone();
    changed
        .as_object_mut()
        .unwrap()
        .get_mut("load")
        .unwrap()
        .as_object_mut()
        .unwrap()
        .get_mut("event_siblings_hex")
        .unwrap()
        .as_array_mut()
        .unwrap()[31] = hex_out(&[1; 32]).into();
    rejects(&changed, &selected.originals);
    let mut originals = selected.originals.clone();
    let mut proof: SumeragiFinalityProof = canonical(&originals["native-proof-3.norito"]).unwrap();
    proof.block_wire[0] ^= 1;
    originals.insert(
        "native-proof-3.norito".into(),
        norito::encode_canonical(&proof).unwrap(),
    );
    rejects(&json, &originals);
}
