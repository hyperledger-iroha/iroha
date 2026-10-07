//! Selected snapshot and preparation-draft regressions with explicit mock proof authority.

use super::*;
use crate::kagemusha_wallet_state_v1::{
    preparation_custody::SourceCustodyV1, transition_custody::PreparedTransitionV1,
};

#[test]
fn bootstrap_snapshot_loss_refuses_folding_before_new_proof_work() {
    let mut wallet = wallet();
    wallet.commit(bootstrap()).unwrap();
    wallet.scheduler().set_activity(true, false);
    let (_, manifest) = wallet.manifest().unwrap();
    let address: [u8; 32] = manifest
        .capsule_sources
        .get(&mut wallet.archive, &manifest.capsule)
        .unwrap()
        .unwrap()
        .try_into()
        .unwrap();
    let bytes = wallet.archive.read_object(&address, 32 * 1024).unwrap();
    wallet.archive.remove(ArchiveKey::Object(address)).unwrap();
    assert!(matches!(wallet.fold_once(), Err(Error::WitnessLost(_))));
    assert_eq!(wallet.proofs.folds.load(Ordering::SeqCst), 0);
    // An unrelated durable snapshot is not a substitute for the selected original.
    wallet.archive.write_object(&[1, 2, 3], 32 * 1024).unwrap();
    assert!(matches!(wallet.fold_once(), Err(Error::WitnessLost(_))));
    wallet
        .archive
        .put(ArchiveKey::Object(address), &bytes)
        .unwrap();
    assert!(matches!(
        wallet.fold_once().unwrap(),
        FoldStatus::Checkpoint {
            sequence: 0,
            ordinal: 0
        }
    ));
    assert_eq!(wallet.fold_once().unwrap(), FoldStatus::Folded(0));
}

#[test]
fn an_unselected_snapshot_never_changes_current_source_custody() {
    let mut wallet = wallet();
    wallet.commit(bootstrap()).unwrap();
    let (root, manifest) = wallet.manifest().unwrap();
    let selected = wallet.indexed_step(&manifest, 0).unwrap();
    let source = wallet.source_custody(&manifest, &selected).unwrap();
    wallet.retain_source_custody([0x87; 32], &source).unwrap();
    let (after, selected_manifest) = wallet.manifest().unwrap();
    assert_eq!(after, root);
    assert!(
        selected_manifest
            .capsule_sources
            .get(&mut wallet.archive, &[0x87; 32])
            .unwrap()
            .is_none()
    );
    assert_eq!(
        archive::encode(
            &wallet
                .source_custody(&selected_manifest, &selected)
                .unwrap()
        )
        .unwrap(),
        archive::encode(&source).unwrap()
    );
    assert_eq!(wallet.custody.signatures, 1);
}

#[test]
fn preparation_draft_must_equal_the_rederived_draft_and_actual_successor() {
    let mut wallet = wallet();
    wallet.commit(bootstrap()).unwrap();
    let (_, manifest) = wallet.manifest().unwrap();
    let selected = wallet.indexed_step(&manifest, 0).unwrap();
    let source = wallet.source_custody(&manifest, &selected).unwrap();
    let mut after = selected.frozen.capsule.successor_state;
    let mut wrong = source.clone();
    let mut tree = map_tree::PersistentMapV1::default();
    tree.insert(&mut wallet.archive, field(17), field(18))
        .unwrap();
    wrong.maps.replace_pending(tree);
    let plan = |draft: SourceCustodyV1| PreparedTransitionV1 {
        request: NativeIntentV1::user(OperationRequestV1 {
            request_id: [41; 32],
            action: OperationActionV1::Retire,
        }),
        native: vec![1],
        source: manifest.capsule,
        draft,
    };
    let view = |store| {
        PreparationCustodyV1::new(
            store,
            &source,
            &selected.frozen.capsule.successor_state,
            KagemushaWalletOperationKindV1::Retiring,
            None,
            manifest.issued_requests,
            manifest.direct_anchors,
        )
        .unwrap()
    };
    let custody =
        TransitionCustodyV1::new(Some(plan(wrong)), Some(view(&mut wallet.archive))).unwrap();
    assert!(matches!(custody.finish(&after), Err(Error::WitnessLost(_))));
    let custody =
        TransitionCustodyV1::new(Some(plan(source.clone())), Some(view(&mut wallet.archive)))
            .unwrap();
    after.core.pending_outgoing_root = field(99);
    assert!(matches!(custody.finish(&after), Err(Error::WitnessLost(_))));
    let custody =
        TransitionCustodyV1::new(Some(plan(source.clone())), Some(view(&mut wallet.archive)))
            .unwrap();
    assert_eq!(
        archive::encode(
            &custody
                .finish(&selected.frozen.capsule.successor_state)
                .unwrap()
                .unwrap()
        )
        .unwrap(),
        archive::encode(&source).unwrap()
    );
    assert!(TransitionCustodyV1::new(Some(plan(source)), None).is_err());
}
