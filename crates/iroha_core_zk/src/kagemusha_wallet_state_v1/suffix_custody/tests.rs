//! Checkpoint namespace tests use mock custody only; they establish no financial proof.
use super::super::tests::{bootstrap, wallet};
use super::*;

fn suffix(binding: ([u8; 32], [u8; 32])) -> SuffixArchiveManifestV1 {
    SuffixArchiveManifestV1 {
        version: 1,
        discriminator: 2,
        scheme_id: binding.0,
        wallet_id: binding.1,
        requests: IndexRoot::default(),
        authorities: IndexRoot::default(),
        operations: IndexRoot::default(),
        source_pins: IndexRoot::default(),
        ledger_attempts: IndexRoot::default(),
        ledger_outcomes: IndexRoot::default(),
        ledger_stages: IndexRoot::default(),
        active_request: None,
    }
}
#[test]
fn ordinary_manifest_bytes_are_preserved_and_suffix_root_survives_ordinary_updates() {
    let mut wallet = wallet();
    wallet.commit(bootstrap()).unwrap();
    let (_, mut manifest) = wallet.manifest().unwrap();
    let original = archive::encode(&manifest).unwrap();
    assert_eq!(
        checkpoint_bytes(&mut wallet.archive, &manifest, None).unwrap(),
        original
    );
    let binding = (manifest.scheme_id, manifest.wallet_id);
    let mut retained = suffix(binding);
    retained.requests = retained
        .requests
        .set(&mut wallet.archive, [17; 32], &[19; 32])
        .unwrap();
    let bytes = checkpoint_bytes(&mut wallet.archive, &manifest, Some(&retained)).unwrap();
    assert_ne!(
        frame_schema(&bytes).unwrap(),
        frame_schema(&original).unwrap()
    );
    let selected = decode_checkpoint(
        &mut wallet.archive,
        binding,
        checkpoint_digest(&bytes),
        &bytes,
    )
    .unwrap();
    assert_eq!(archive::encode(&selected.manifest).unwrap(), original);
    assert_eq!(
        archive::encode(selected.suffix.as_ref().unwrap()).unwrap(),
        archive::encode(&retained).unwrap()
    );
    manifest.folded = manifest.indexed;
    let next = checkpoint_bytes(&mut wallet.archive, &manifest, selected.suffix.as_ref()).unwrap();
    let restored = decode_checkpoint(
        &mut wallet.archive,
        binding,
        checkpoint_digest(&next),
        &next,
    )
    .unwrap();
    assert_eq!(restored.manifest.folded, manifest.indexed);
    assert_eq!(restored.suffix.unwrap().requests, retained.requests);
}
#[test]
fn declared_schema_and_referenced_objects_fail_closed_without_fallback() {
    let mut wallet = wallet();
    wallet.commit(bootstrap()).unwrap();
    let (_, manifest) = wallet.manifest().unwrap();
    let binding = (manifest.scheme_id, manifest.wallet_id);
    let bytes = checkpoint_bytes(&mut wallet.archive, &manifest, Some(&suffix(binding))).unwrap();
    let envelope: SuffixArchiveCheckpointV1 = archive::decode(&bytes).unwrap();
    wallet.archive.hide_object(envelope.manifest_original);
    assert!(
        decode_checkpoint(
            &mut wallet.archive,
            binding,
            checkpoint_digest(&bytes),
            &bytes
        )
        .is_err()
    );
    wallet.archive.reveal_object(envelope.manifest_original);
    let mut wrong = envelope.clone();
    wrong.discriminator = 2;
    let invalid = archive::encode(&wrong).unwrap();
    assert!(
        decode_checkpoint(
            &mut wallet.archive,
            binding,
            checkpoint_digest(&invalid),
            &invalid
        )
        .is_err()
    );
    assert!(
        decode_checkpoint(
            &mut wallet.archive,
            ([55; 32], binding.1),
            checkpoint_digest(&bytes),
            &bytes
        )
        .is_err()
    );
    assert!(decode_checkpoint(&mut wallet.archive, binding, [66; 32], &bytes).is_err());
    // A valid different schema must not be heuristically parsed as an ordinary Manifest.
    let unknown = archive::encode(&suffix(binding)).unwrap();
    assert!(
        decode_checkpoint(
            &mut wallet.archive,
            binding,
            checkpoint_digest(&unknown),
            &unknown
        )
        .is_err()
    );
}
#[test]
fn records_reject_foreign_incarnations_and_inconsistent_release_progress() {
    let binding = ([1; 32], [2; 32]);
    let good = suffix(binding);
    good.require(binding).unwrap();
    assert!(good.require(([3; 32], binding.1)).is_err());
    assert!(
        SuffixArchiveManifestV1 {
            active_request: Some([0; 32]),
            ..good
        }
        .require(binding)
        .is_err()
    );
    let progress = SuffixPreparationProgressV1 {
        version: 1,
        discriminator: 9,
        scheme_id: binding.0,
        wallet_id: binding.1,
        request_id: [4; 32],
        source_checkpoint: [5; 32],
        pinned_steps: 50,
        processed_steps: 17,
        emitted_chunks: 2,
        emitted_stream_bytes: 123,
        chunk_digests: IndexRoot::default(),
        chunk_originals: IndexRoot::default(),
        credit_tree: crate::kagemusha_wallet_state_v1::credit_tree::CreditTree::default(),
        partial_step_original: None,
        partial_step_length: 0,
    };
    progress.require(binding).unwrap();
    assert!(
        SuffixPreparationProgressV1 {
            processed_steps: 51,
            ..progress.clone()
        }
        .require(binding)
        .is_err()
    );
    assert!(
        SuffixPreparationProgressV1 {
            partial_step_length: 1,
            ..progress
        }
        .require(binding)
        .is_err()
    );
}
