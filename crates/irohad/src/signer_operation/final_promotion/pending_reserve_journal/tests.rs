//! Private pending-Reserve bounds, filesystem identity and signature recovery tests.

use super::*;
use crate::signer_operation::journal::{SignerReceiptJournalV1, SignerReceiptPurposeV1};
use iroha_crypto::{Algorithm, Hash, HashOf, KeyPair, Signature};
use iroha_data_model::{
    Level, NetworkId,
    block::BlockHeader,
    isi::Log,
    transaction::{FeePaymentIntent, TransactionBuilder},
};
use std::{
    fs::{self, Permissions},
    os::unix::fs::{MetadataExt as _, PermissionsExt as _},
};

const OPERATION: [u8; 32] = [0x76; 32];

fn private_sibling_directories() -> (tempfile::TempDir, std::path::PathBuf, std::path::PathBuf) {
    let parent = tempfile::tempdir().unwrap();
    fs::set_permissions(parent.path(), Permissions::from_mode(0o700)).unwrap();
    let receipts = parent.path().join("receipts");
    let pending = parent.path().join("pending-reserve-v1");
    for path in [&receipts, &pending] {
        fs::create_dir(path).unwrap();
        fs::set_permissions(path, Permissions::from_mode(0o700)).unwrap();
    }
    let receipts = receipts.canonicalize().unwrap();
    let pending = pending.canonicalize().unwrap();
    (parent, receipts, pending)
}

fn pending_file(path: &Path) -> std::path::PathBuf {
    path.join(format!("{}.pending-reserve.norito", hex::encode(OPERATION)))
}

#[test]
fn exact_intent_frame_and_record_byte_ceilings_are_inclusive() {
    assert!(validate_length_parts(4096, 65536, 65536, 136 * 1024).is_ok());
    for sizes in [
        (0, 1, 1, 1),
        (4097, 1, 1, 1),
        (1, 0, 1, 1),
        (1, 65537, 1, 1),
        (1, 1, 0, 1),
        (1, 1, 65537, 1),
        (1, 1, 1, 0),
        (1, 1, 1, 136 * 1024 + 1),
    ] {
        assert!(
            validate_length_parts(sizes.0, sizes.1, sizes.2, sizes.3).is_err(),
            "{sizes:?}"
        );
    }
}

#[test]
fn pending_journal_requires_dedicated_sibling_and_independent_lease() {
    let (_parent, receipts, pending) = private_sibling_directories();
    let receipt =
        SignerReceiptJournalV1::open(&receipts, SignerReceiptPurposeV1::FinalPromotionProvenance)
            .unwrap();
    assert!(FinalPromotionPendingReserveJournalV1::open(&receipts).is_err());
    let pending_writer = FinalPromotionPendingReserveJournalV1::open(&pending).unwrap();
    assert!(FinalPromotionPendingReserveJournalV1::open(&pending).is_err());
    assert!(SignerReceiptJournalV1::open(&pending, receipt.purpose()).is_err());
    drop(receipt);
    assert!(FinalPromotionPendingReserveJournalV1::open(&pending).is_err());
    drop(pending_writer);
    FinalPromotionPendingReserveJournalV1::open(&pending).unwrap();
}

#[test]
fn partial_record_is_durable_but_never_recovered_as_authority() {
    let (_parent, _receipts, pending) = private_sibling_directories();
    let journal = FinalPromotionPendingReserveJournalV1::open(&pending).unwrap();
    let staged = journal.files.stage(OPERATION, b"partial").unwrap();
    let file = pending_file(&pending);
    let metadata = fs::metadata(&file).unwrap();
    assert_eq!(metadata.mode() & 0o7777, 0o400);
    assert_eq!(metadata.nlink(), 1);
    assert_eq!(staged.bytes(), b"partial");
    assert!(journal.recover(OPERATION).is_err());
    assert!(journal.files.stage(OPERATION, b"replacement").is_err());
    drop(staged);
    drop(journal);
    let reopened = FinalPromotionPendingReserveJournalV1::open(&pending).unwrap();
    assert!(reopened.recover(OPERATION).is_err());
}

#[test]
fn pinned_pending_record_rejects_changed_bytes_and_replaced_path() {
    for attack in ["bytes", "path", "hardlink"] {
        let (parent, _receipts, pending) = private_sibling_directories();
        let journal = FinalPromotionPendingReserveJournalV1::open(&pending).unwrap();
        let pinned = journal
            .files
            .stage(OPERATION, b"private pending record")
            .unwrap();
        let file = pending_file(&pending);
        match attack {
            "bytes" => {
                fs::set_permissions(&file, Permissions::from_mode(0o600)).unwrap();
                fs::write(&file, b"changed pending record").unwrap();
                fs::set_permissions(&file, Permissions::from_mode(0o400)).unwrap();
            }
            "path" => {
                fs::rename(&file, parent.path().join("retained-original")).unwrap();
                fs::write(&file, b"private pending record").unwrap();
                fs::set_permissions(&file, Permissions::from_mode(0o400)).unwrap();
            }
            "hardlink" => fs::hard_link(&file, parent.path().join("second-link")).unwrap(),
            _ => unreachable!(),
        }
        assert!(pinned.recheck().is_err(), "{attack}");
        assert!(journal.recover(OPERATION).is_err(), "{attack}");
    }
}

#[test]
fn canonical_signed_frame_rejects_forged_authorization_proof() {
    let key = KeyPair::try_from_seed(vec![0x31; 32], Algorithm::Ed25519).unwrap();
    let network_id = NetworkId::from_genesis_hash(HashOf::<BlockHeader>::from_untyped_unchecked(
        Hash::prehashed([0x42; 32]),
    ));
    let signed = TransactionBuilder::new(
        network_id,
        AccountId::new(key.public_key().clone()),
        FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_instructions([Log::new(Level::INFO, "pending proof".to_owned())])
    .try_sign(key.private_key())
    .unwrap();
    let valid = final_promotion_native_signed_entry_frame_v1(&TransactionEntrypoint::External(
        signed.clone(),
    ))
    .unwrap();
    assert!(decode_signed_frame(&valid).is_ok());

    let forged = TransactionBuilder::from_payload(signed.payload().clone())
        .unwrap()
        .build_with_signature(Signature::from_bytes(&[1; 64]));
    let forged = norito::encode_canonical(&TransactionEntrypoint::External(forged)).unwrap();
    let _: TransactionEntrypoint = norito::decode_canonical(&forged).unwrap();
    assert!(decode_signed_frame(&forged).is_err());
}
