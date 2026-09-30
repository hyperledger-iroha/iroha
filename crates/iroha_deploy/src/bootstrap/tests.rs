//! Release authority, replay and native-checkpoint binding regressions.
//!
//! Native fixtures use genuine BLS certificates over synthetic execution outputs; these tests
//! authenticate bootstrap material and do not qualify a running parent or private dataspace.
use super::*;
use iroha_crypto::KeyPair;
use iroha_data_model::sumeragi_finality::test_fixtures::NativeFinalityFixture;

fn key(seed: u8) -> KeyPair {
    KeyPair::from_seed(vec![seed; 32], Algorithm::Ed25519)
}

fn trust(key: &KeyPair, floor: u64) -> ReleaseTrust {
    ReleaseTrust::new("fixture".into(), key.public_key().clone(), floor).unwrap()
}

fn metadata(checkpoint: &SumeragiFinalityCheckpoint) -> NetworkRelease {
    NetworkRelease {
        network_name: "fixture".into(),
        serial: 5,
        generation: 1,
        network_id: checkpoint.network_id(),
        chain_id: checkpoint.chain_id().into(),
        issued_at_ms: 1_000,
        expires_at_ms: 10_000,
        torii_roots: vec!["https://torii.example/".into()],
        checkpoint_hash: Hash::new(checkpoint.encode_canonical().unwrap()),
        checkpoint_height: checkpoint.height(),
        checkpoint_block_hash: checkpoint.block_hash().into(),
    }
}

fn signed(
    release: NetworkRelease,
    checkpoint: &SumeragiFinalityCheckpoint,
    key: &KeyPair,
) -> Vec<u8> {
    SignedNetworkCheckpoint::sign(release, checkpoint, key.private_key())
        .unwrap()
        .encode_canonical()
        .unwrap()
}

#[test]
fn signed_native_checkpoint_roundtrips_and_resumes_after_exclusive_reopen() {
    let checkpoint = NativeFinalityFixture::new().checkpoint();
    let key = key(81);
    let trust = trust(&key, 5);
    let release = metadata(&checkpoint);
    let bytes = signed(release.clone(), &checkpoint, &key);
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("release");
    let store = ReleaseCheckpointStore::open(&path).unwrap();
    assert!(ReleaseCheckpointStore::open(&path).is_err());
    let result = store.authenticate(&trust, &bytes, 2_000).unwrap();
    assert_eq!(result.release(), &release);
    assert!(!result.is_network_reset());
    assert_eq!(result.into_verifier().checkpoint(), &checkpoint);
    drop(store);
    let store = ReleaseCheckpointStore::open(&path).unwrap();
    assert!(
        !store
            .authenticate(&trust, &bytes, 2_001)
            .unwrap()
            .is_network_reset()
    );
    assert!(store.authenticate(&trust, &bytes, 2_000).is_err());
    assert!(store.authenticate(&trust, &bytes, 10_000).is_err());
    assert!(store.authenticate(&trust, &bytes, 999).is_err());
}

#[test]
fn response_cannot_choose_authority_network_floor_or_checkpoint() {
    let checkpoint = NativeFinalityFixture::new().checkpoint();
    let key = key(82);
    let release = metadata(&checkpoint);
    let bytes = signed(release.clone(), &checkpoint, &key);
    let directory = tempfile::tempdir().unwrap();
    let store = ReleaseCheckpointStore::open(&directory.path().join("release")).unwrap();
    assert!(
        store
            .authenticate(&trust(&super::tests::key(83), 1), &bytes, 2_000)
            .is_err()
    );
    assert!(store.authenticate(&trust(&key, 6), &bytes, 2_000).is_err());
    let wrong_name = ReleaseTrust::new("other".into(), key.public_key().clone(), 1).unwrap();
    assert!(store.authenticate(&wrong_name, &bytes, 2_000).is_err());
    let mut changed: SignedNetworkCheckpoint =
        decode(&bytes, MAX_RELEASE_CHECKPOINT_BYTES).unwrap();
    changed.release.torii_roots[0] = "https://attacker.example/".into();
    assert!(
        store
            .authenticate(&trust(&key, 1), &changed.encode_canonical().unwrap(), 2_000)
            .is_err()
    );
    changed = decode(&bytes, MAX_RELEASE_CHECKPOINT_BYTES).unwrap();
    changed.checkpoint[0] ^= 1;
    assert!(
        store
            .authenticate(&trust(&key, 1), &changed.encode_canonical().unwrap(), 2_000)
            .is_err()
    );
    assert!(
        store
            .directory
            .entries(10)
            .unwrap()
            .iter()
            .all(|name| name == "release.lock")
    );
    let mut appended = bytes.clone();
    appended.push(0);
    assert!(
        store
            .authenticate(&trust(&key, 1), &appended, 2_000)
            .is_err()
    );
    assert!(decode::<SignedNetworkCheckpoint>(&[], 10).is_err());
    assert!(decode::<SignedNetworkCheckpoint>(&bytes, bytes.len() - 1).is_err());
}

#[test]
fn signed_metadata_still_requires_exact_native_identity_height_and_quorum() {
    let checkpoint = NativeFinalityFixture::new().checkpoint();
    let key = key(84);
    let base = metadata(&checkpoint);
    for mutation in 0..5 {
        let mut release = base.clone();
        match mutation {
            0 => release.chain_id = "foreign".into(),
            1 => {
                release.network_id = NetworkId::from_genesis_hash(
                    iroha_crypto::HashOf::from_untyped_unchecked(Hash::new(b"foreign")),
                )
            }
            2 => release.checkpoint_height += 1,
            3 => release.checkpoint_block_hash = Hash::new(b"other block"),
            4 => release.checkpoint_hash = Hash::new(b"other frame"),
            _ => unreachable!(),
        }
        assert!(SignedNetworkCheckpoint::sign(release, &checkpoint, key.private_key()).is_err());
    }
    // Signing a corrupt checkpoint digest cannot turn the payload into native finality.
    let directory = tempfile::tempdir().unwrap();
    let store = ReleaseCheckpointStore::open(&directory.path().join("release")).unwrap();
    let mut release = base;
    let bad = vec![0; 32];
    release.checkpoint_hash = Hash::new(&bad);
    let artifact = SignedNetworkCheckpoint {
        signature: Signature::new(key.private_key(), &release_preimage(&release).unwrap()),
        release,
        checkpoint: bad,
    };
    assert!(
        store
            .authenticate(
                &trust(&key, 1),
                &artifact.encode_canonical().unwrap(),
                2_000
            )
            .is_err()
    );
}

#[test]
fn monotonic_release_rejects_rollback_equivocation_and_allows_installation_floor_upgrade() {
    let mut fixture = NativeFinalityFixture::new();
    let old_checkpoint = fixture.checkpoint();
    let key = key(85);
    let original = metadata(&old_checkpoint);
    let directory = tempfile::tempdir().unwrap();
    let store = ReleaseCheckpointStore::open(&directory.path().join("release")).unwrap();
    store
        .authenticate(
            &trust(&key, 5),
            &signed(original.clone(), &old_checkpoint, &key),
            2_000,
        )
        .unwrap();
    for mutation in 0..4 {
        let mut release = original.clone();
        match mutation {
            0 => release.serial -= 1,
            1 => release.expires_at_ms += 1,
            2 => {
                release.serial += 1;
                release.issued_at_ms -= 1;
            }
            3 => {
                release.serial += 1;
                release.generation += 1;
            }
            _ => unreachable!(),
        }
        assert!(
            store
                .authenticate(
                    &trust(&key, 1),
                    &signed(release, &old_checkpoint, &key),
                    2_001
                )
                .is_err()
        );
    }
    fixture.certify(fixture.block_with_submitted_work(fixture.next_header()));
    let newer = fixture.checkpoint();
    let mut next = metadata(&newer);
    next.serial = 6;
    let result = store
        .authenticate(&trust(&key, 6), &signed(next.clone(), &newer, &key), 2_001)
        .unwrap();
    assert_eq!(result.into_verifier().checkpoint().height(), 3);
    let mut regressed = original;
    regressed.serial = 7;
    assert!(
        store
            .authenticate(
                &trust(&key, 6),
                &signed(regressed, &old_checkpoint, &key),
                2_002
            )
            .is_err()
    );
    let mut changed = next.clone();
    changed.checkpoint_block_hash = Hash::new(b"equivocation");
    changed.serial += 1;
    assert!(validate_successor(&next, &changed).is_err());
    changed = next.clone();
    changed.checkpoint_hash = Hash::new(b"different restart context");
    changed.serial += 1;
    assert!(validate_successor(&next, &changed).is_err());
}

#[test]
fn authenticated_reset_is_explicit_and_an_expired_prior_release_still_prevents_rollback() {
    let fixture = NativeFinalityFixture::new();
    let old_checkpoint = fixture.checkpoint();
    let key = key(86);
    let original = metadata(&old_checkpoint);
    // A different mode changes the signed genesis, producing a genuine independent network.
    let mut reset = NativeFinalityFixture::start_with_mode(
        "reset-fixture",
        iroha_data_model::parameter::system::SumeragiConsensusMode::Npos,
    );
    reset.certify(reset.block_with_submitted_work(reset.next_header()));
    let new_checkpoint = reset.checkpoint();
    assert_ne!(old_checkpoint.network_id(), new_checkpoint.network_id());
    let directory = tempfile::tempdir().unwrap();
    let store = ReleaseCheckpointStore::open(&directory.path().join("release")).unwrap();
    store
        .authenticate(
            &trust(&key, 1),
            &signed(original.clone(), &old_checkpoint, &key),
            2_000,
        )
        .unwrap();
    let mut next = metadata(&new_checkpoint);
    next.serial = 6;
    next.issued_at_ms = 11_000;
    next.expires_at_ms = 20_000;
    assert!(
        store
            .authenticate(
                &trust(&key, 1),
                &signed(next.clone(), &new_checkpoint, &key),
                12_000
            )
            .is_err()
    );
    next.generation = 2;
    let result = store
        .authenticate(
            &trust(&key, 1),
            &signed(next.clone(), &new_checkpoint, &key),
            12_000,
        )
        .unwrap();
    assert!(result.is_network_reset());
    assert_eq!(result.release().network_id, new_checkpoint.network_id());
    let mut resurrected = original;
    resurrected.serial = 7;
    resurrected.issued_at_ms = 12_000;
    resurrected.expires_at_ms = 20_000;
    assert!(
        store
            .authenticate(
                &trust(&key, 1),
                &signed(resurrected, &old_checkpoint, &key),
                12_001
            )
            .is_err()
    );
    assert!(
        store
            .authenticate(
                &trust(&key, 1),
                &signed(next, &new_checkpoint, &key),
                12_001
            )
            .is_ok()
    );
}

#[test]
fn release_policy_rejects_unbounded_or_ambiguous_metadata_and_noncanonical_custody() {
    let checkpoint = NativeFinalityFixture::new().checkpoint();
    let key = key(87);
    let base = metadata(&checkpoint);
    for url in [
        "http://torii.example/",
        "https://user:secret@torii.example/",
        "https://torii.example/?secret=yes",
        "https://torii.example/#fragment",
        "https://torii.example/path",
        "https://TORII.example/",
    ] {
        let mut release = base.clone();
        release.torii_roots = vec![url.into()];
        assert!(validate_release(&release).is_err(), "{url}");
    }
    for mutation in 0..7 {
        let mut release = base.clone();
        match mutation {
            0 => release.serial = 0,
            1 => release.generation = 0,
            2 => release.expires_at_ms = release.issued_at_ms,
            3 => release.expires_at_ms = release.issued_at_ms + MAX_RELEASE_VALIDITY_MS + 1,
            4 => release.torii_roots.push(release.torii_roots[0].clone()),
            5 => release.checkpoint_height = 1,
            6 => release.network_name = "../escape".into(),
            _ => unreachable!(),
        }
        assert!(validate_release(&release).is_err());
    }
    assert!(ReleaseTrust::new("fixture".into(), key.public_key().clone(), 0).is_err());
    assert!(ReleaseTrust::new("Fixture".into(), key.public_key().clone(), 1).is_err());
    assert!(
        ReleaseTrust::new(
            "fixture".into(),
            KeyPair::from_seed(vec![5; 32], Algorithm::BlsNormal)
                .public_key()
                .clone(),
            1
        )
        .is_err()
    );
    let directory = tempfile::tempdir().unwrap();
    let store = ReleaseCheckpointStore::open(&directory.path().join("release")).unwrap();
    store
        .directory
        .write_atomic("accepted.nrt", b"partial", PublishMode::CreateNew)
        .unwrap();
    assert!(
        store
            .authenticate(&trust(&key, 1), &signed(base, &checkpoint, &key), 2_000)
            .is_err()
    );
    assert_eq!(
        store.directory.read("accepted.nrt", 20).unwrap().as_slice(),
        b"partial"
    );
}
