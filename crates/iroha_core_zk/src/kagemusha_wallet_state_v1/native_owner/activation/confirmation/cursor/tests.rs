//! Genuine Native checkpoint verification plus explicitly synthetic archive/custody publication.
use super::*;
use crate::kagemusha_wallet_state_v1::tests::{Wallet, bootstrap, fixture, wallet};
use iroha_data_model::sumeragi_finality::test_fixtures::NativeFinalityFixture;

fn setup() -> (Wallet, Plan, Cursor, NativeFinalityFixture) {
    let native = NativeFinalityFixture::start("activation-verifier-cursor");
    let mut wallet = wallet();
    wallet.commit(bootstrap()).unwrap();
    let asset = fixture("KagemushaWalletAssetScopeV1");
    let plan = wallet.activation_plan(&asset).unwrap();
    let original = super::super::super::tests::output(&plan);
    wallet.finish_activation(&plan, &original).unwrap();
    let plan = wallet.activation_plan(&asset).unwrap();
    let cursor = Cursor {
        activation: plan.output.unwrap(),
        signed_transaction: vec![7; 128], // Binding DATA only; real signed Activate tested separately.
        checkpoint: native.checkpoint().encode_canonical().unwrap(),
    };
    (wallet, plan, cursor, native)
}

fn select_original(wallet: &mut Wallet, plan: &Plan, bytes: &[u8]) -> [u8; 32] {
    let (root, mut manifest) = wallet.sync_manifest().unwrap();
    let mut selected = plan.clone();
    let address = wallet.archive.write_object(bytes, CURSOR_MAX).unwrap();
    selected.cursor = Some(address);
    manifest.activation = Some(
        wallet
            .archive
            .write_object(&archive::encode(&selected).unwrap(), PLAN_MAX)
            .unwrap(),
    );
    wallet.publish_manifest(root, &manifest).unwrap();
    address
}

fn reopen(wallet: Wallet) -> Wallet {
    Coordinator::new(
        wallet.custody,
        wallet.archive,
        wallet.proofs,
        wallet.scheme_id,
        wallet.wallet_id,
    )
    .unwrap()
}

#[test]
fn fresh_verifier_cursor_restores_genuine_native_checkpoint_and_exact_binding() {
    let (mut wallet, plan, cursor, native) = setup();
    let bytes = archive::encode(&cursor).unwrap();
    assert_eq!(decode_cursor(&bytes).unwrap().checkpoint, cursor.checkpoint);
    let (root, manifest) = wallet.sync_manifest().unwrap();
    wallet
        .publish_activation_cursor(root, manifest, &plan, &cursor)
        .unwrap();
    let mut wallet = reopen(wallet);
    let plan = wallet.activation_plan(&plan.asset).unwrap();
    let restored = wallet
        .retained_activation_cursor(&plan, &cursor.signed_transaction)
        .unwrap()
        .unwrap();
    let (_, checkpoint) = restored.restore(&native.verifier()).unwrap();
    assert_eq!(checkpoint.encode_canonical().unwrap(), cursor.checkpoint);
    assert!(wallet.retained_activation_cursor(&plan, &[8; 128]).is_err());
    let foreign = NativeFinalityFixture::start("foreign-activation-cursor");
    assert!(restored.restore(&foreign.verifier()).is_err());
}

#[test]
fn current_cursor_publication_recovers_before_and_after_durable_selection() {
    for after in [false, true] {
        let (mut wallet, plan, cursor, native) = setup();
        let (root, manifest) = wallet.sync_manifest().unwrap();
        wallet.custody.fail_publication = Some(after);
        assert!(
            wallet
                .publish_activation_cursor(root, manifest, &plan, &cursor)
                .is_err()
        );
        let mut wallet = reopen(wallet);
        wallet.clean_activation_cursor().unwrap();
        let plan = wallet.activation_plan(&plan.asset).unwrap();
        let (root, manifest) = wallet.sync_manifest().unwrap();
        wallet
            .publish_activation_cursor(root, manifest, &plan, &cursor)
            .unwrap();
        let plan = wallet.activation_plan(&plan.asset).unwrap();
        let restored = wallet
            .retained_activation_cursor(&plan, &cursor.signed_transaction)
            .unwrap()
            .unwrap();
        let (_, checkpoint) = restored.restore(&native.verifier()).unwrap();
        assert_eq!(checkpoint.encode_canonical().unwrap(), cursor.checkpoint);
        let selected = wallet
            .archive
            .read_object(&plan.cursor.unwrap(), CURSOR_MAX)
            .unwrap();
        assert_eq!(
            decode_cursor(&selected).unwrap().checkpoint,
            cursor.checkpoint
        );
    }
}

#[test]
fn current_cursor_missing_corrupt_or_foreign_binding_never_means_absent() {
    let (mut wallet, plan, cursor, native) = setup();
    let mut changed = cursor.clone();
    changed.activation[0] ^= 1;
    assert!(
        decode_cursor(&archive::encode(&changed).unwrap())
            .unwrap()
            .require(&plan, &cursor.signed_transaction)
            .is_err()
    );
    assert!(
        decode_cursor(&archive::encode(&cursor).unwrap())
            .unwrap()
            .require(&plan, &[8; 128])
            .is_err()
    );
    let mut missing_checkpoint = cursor.clone();
    missing_checkpoint.checkpoint.clear();
    let decoded = decode_cursor(&archive::encode(&missing_checkpoint).unwrap()).unwrap();
    assert!(decoded.require(&plan, &cursor.signed_transaction).is_err());
    assert!(decoded.restore(&native.verifier()).is_err());
    let mut changed_checkpoint = cursor.clone();
    changed_checkpoint.checkpoint[0] ^= 1;
    assert!(
        decode_cursor(&archive::encode(&changed_checkpoint).unwrap())
            .unwrap()
            .restore(&native.verifier())
            .is_err()
    );
    let old = archive::encode(&cursor).unwrap();
    let mut trailing = old.clone();
    trailing.push(0);
    let selected = select_original(&mut wallet, &plan, &trailing);
    let current = wallet.activation_plan(&plan.asset).unwrap();
    assert!(
        wallet
            .retained_activation_cursor(&current, &cursor.signed_transaction)
            .is_err()
    );
    assert_eq!(
        wallet.archive.read_object(&selected, CURSOR_MAX).unwrap(),
        trailing
    );
    wallet.archive.remove(ArchiveKey::Object(selected)).unwrap();
    assert!(
        wallet
            .retained_activation_cursor(&current, &cursor.signed_transaction)
            .is_err()
    );
    assert!(decode_cursor(b"unknown cursor schema").is_err());
}

#[test]
fn confirmation_cleanup_removes_only_its_retired_current_cursor() {
    let (mut wallet, plan, cursor, _native) = setup();
    let old = archive::encode(&cursor).unwrap();
    let address = select_original(&mut wallet, &plan, &old);
    let plan = wallet.activation_plan(&plan.asset).unwrap();
    let confirmation = Confirmation {
        version: 1,
        activation: cursor.activation,
        signed_transaction: cursor.signed_transaction.clone(),
        height: 2,
        block_hash: [8; 32], // Explicit publisher fault-test DATA, not a ledger verdict.
    };
    let (root, manifest) = wallet.sync_manifest().unwrap();
    wallet
        .publish_activation_confirmation(root, manifest, &plan, &confirmation)
        .unwrap();
    assert!(
        wallet
            .archive
            .get(ArchiveKey::Object(address), CURSOR_MAX)
            .unwrap()
            .is_none()
    );
    let current = wallet.activation_plan(&plan.asset).unwrap();
    assert!(current.cursor.is_none() && current.retired_cursor.is_none());
    assert_eq!(
        wallet
            .retained_activation_confirmation(&current)
            .unwrap()
            .unwrap()
            .signed_transaction,
        cursor.signed_transaction
    );
}

#[test]
fn unsupported_selected_cursor_is_preserved_and_never_treated_as_absent() {
    let (mut wallet, plan, cursor, _native) = setup();
    let bytes = b"unsupported activation cursor schema";
    let address = select_original(&mut wallet, &plan, bytes);
    let selected = wallet.activation_plan(&plan.asset).unwrap();
    let before = wallet.manifest().unwrap().0;
    assert!(
        wallet
            .retained_activation_cursor(&selected, &cursor.signed_transaction)
            .is_err()
    );
    assert_eq!(wallet.manifest().unwrap().0, before);
    assert_eq!(
        wallet.archive.read_object(&address, CURSOR_MAX).unwrap(),
        bytes
    );
}
