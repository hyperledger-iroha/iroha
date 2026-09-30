//! Exact purpose ceilings and private journal ownership without a software signing path.
use super::*;
use std::fs;

fn private_directory() -> tempfile::TempDir {
    let directory = tempfile::tempdir().expect("temporary private directory");
    fs::set_permissions(directory.path(), Permissions::from_mode(0o700)).expect("private mode");
    directory
}

#[test]
fn path_preflight_caps_pinned_lineage_before_directory_io() {
    let profile = JournalProfile::receipt(SignerReceiptPurposeV1::StreamToken);
    let at_limit = format!("/{}", vec!["a"; MAX_JOURNAL_PATH_COMPONENTS].join("/"));
    assert_eq!(
        preflight_journal_path(Path::new(&at_limit), profile)
            .expect("inclusive component ceiling")
            .len(),
        MAX_JOURNAL_PATH_COMPONENTS
    );
    let too_deep = format!("{at_limit}/a");
    assert!(preflight_journal_path(Path::new(&too_deep), profile).is_err());

    let at_byte_limit = format!("/{}", "a".repeat(MAX_JOURNAL_PATH_BYTES - 1));
    assert_eq!(at_byte_limit.len(), MAX_JOURNAL_PATH_BYTES);
    assert_eq!(
        preflight_journal_path(Path::new(&at_byte_limit), profile)
            .expect("inclusive byte ceiling")
            .len(),
        1
    );
    let too_long = format!("{at_byte_limit}a");
    assert!(preflight_journal_path(Path::new(&too_long), profile).is_err());

    for malformed in [
        "relative/journal",
        "/",
        "/private//journal",
        "/private/./journal",
        "/private/../journal",
        "/private/journal/",
    ] {
        assert!(
            preflight_journal_path(Path::new(malformed), profile).is_err(),
            "preflight must refuse {malformed:?} before opening a directory"
        );
    }
    assert!(
        preflight_journal_path(
            Path::new("/private/not-pending-reserve"),
            JournalProfile::PENDING_RESERVE
        )
        .is_err()
    );
    assert_eq!(
        preflight_journal_path(
            Path::new("/private/pending-reserve-v1"),
            JournalProfile::PENDING_RESERVE
        )
        .unwrap()
        .len(),
        2
    );
}

#[test]
fn one_injected_pool_admits_every_purpose_and_refuses_before_path_io() {
    let first = private_directory();
    let first_path = first.path().canonicalize().unwrap();
    let second = private_directory();
    let second_path = second.path().canonicalize().unwrap();
    let depth = first_path
        .components()
        .filter(|part| matches!(part, Component::Normal(_)))
        .count();
    let pool =
        SignerJournalInventoryPoolV1::for_test(16 * 1024 * 1024, 300_000, (depth + 2) as u64);
    let first =
        SignerReceiptJournalV1::open(&first_path, SignerReceiptPurposeV1::ReleaseManifest, &pool)
            .expect("first purpose owns the shared descriptor credits");
    assert_eq!(
        SignerReceiptJournalV1::open(&second_path, SignerReceiptPurposeV1::StreamToken, &pool,)
            .unwrap_err(),
        SignerReceiptJournalErrorV1::Capacity
    );
    drop(first);
    SignerReceiptJournalV1::open(&second_path, SignerReceiptPurposeV1::StreamToken, &pool)
        .expect("released credits admit another purpose");

    let absent = second_path.join("not-opened");
    let refused = SignerJournalInventoryPoolV1::for_test(0, 300_000, 100);
    assert_eq!(
        SignerReceiptJournalV1::open(
            &absent,
            SignerReceiptPurposeV1::FinalPromotionProvenance,
            &refused,
        )
        .unwrap_err(),
        SignerReceiptJournalErrorV1::Capacity
    );
    assert!(
        !absent.exists(),
        "resource refusal preceded filesystem access"
    );
    assert_eq!(
        super::super::release_manifest::SignerReleaseManifestErrorV1::from(
            SignerReceiptJournalErrorV1::Capacity,
        ),
        super::super::release_manifest::SignerReleaseManifestErrorV1::LocalCapacity
    );
    assert_eq!(
        super::super::stream_token::SignerStreamTokenErrorV1::from(
            SignerReceiptJournalErrorV1::Capacity,
        ),
        super::super::stream_token::SignerStreamTokenErrorV1::LocalCapacity
    );
    assert_eq!(
        super::super::final_promotion::SignerFinalPromotionErrorV1::from(
            SignerReceiptJournalErrorV1::Capacity,
        ),
        super::super::final_promotion::SignerFinalPromotionErrorV1::LocalCapacity
    );
}

#[test]
fn closed_receipt_purposes_select_exact_public_limits_and_retain_exclusive_ownership() {
    for (purpose, limit) in [
        (
            SignerReceiptPurposeV1::FinalPromotionProvenance,
            SIGNER_FINAL_PROMOTION_RECEIPT_MAX_BYTES_V1,
        ),
        (
            SignerReceiptPurposeV1::ReleaseManifest,
            SIGNER_RELEASE_MANIFEST_RECEIPT_MAX_BYTES_V1,
        ),
        (
            SignerReceiptPurposeV1::StreamToken,
            SIGNER_STREAM_TOKEN_RECEIPT_MAX_BYTES_V1,
        ),
    ] {
        assert_eq!(purpose.max_bytes(), limit);
        let directory = private_directory();
        let path = directory.path().canonicalize().expect("canonical path");
        let journal = SignerReceiptJournalV1::open_test(&path, purpose).expect("private journal");
        assert_eq!(journal.purpose(), purpose);
        for competing in [
            SignerReceiptPurposeV1::FinalPromotionProvenance,
            SignerReceiptPurposeV1::ReleaseManifest,
            SignerReceiptPurposeV1::StreamToken,
        ] {
            assert_eq!(
                SignerReceiptJournalV1::open_test(&path, competing).unwrap_err(),
                SignerReceiptJournalErrorV1::Unavailable
            );
            let separate = private_directory();
            let separate_path = separate.path().canonicalize().unwrap();
            let independent = SignerReceiptJournalV1::open_test(&separate_path, competing).unwrap();
            assert_eq!(independent.purpose(), competing);
        }
        assert_eq!(
            journal.purpose(),
            purpose,
            "failed competing opens cannot relabel the owner"
        );
    }
}

#[test]
fn every_purpose_enforces_exact_stage_recovery_and_inventory_byte_ceiling() {
    for purpose in [
        SignerReceiptPurposeV1::FinalPromotionProvenance,
        SignerReceiptPurposeV1::ReleaseManifest,
        SignerReceiptPurposeV1::StreamToken,
    ] {
        let directory = private_directory();
        let path = directory.path().canonicalize().unwrap();
        let journal = SignerReceiptJournalV1::open_test(&path, purpose).unwrap();
        let maximum = purpose.max_bytes();
        // The journal enforces private transport bounds; canonical receipt semantics belong to
        // its sealed producer and are separately exercised by the real signed release fixture.
        let bytes = vec![0x57; maximum];
        for (id, invalid) in [([0; 32], bytes.as_slice()), ([0x71; 32], &[][..])] {
            assert!(matches!(
                journal.stage(id, invalid),
                Err(SignerReceiptJournalErrorV1::Unavailable)
            ));
        }
        assert!(matches!(
            journal.stage([0x71; 32], &vec![0x58; maximum + 1]),
            Err(SignerReceiptJournalErrorV1::Unavailable)
        ));
        assert_eq!(
            fs::read_dir(&path).unwrap().count(),
            0,
            "invalid admission must not create a tombstone"
        );
        let operation = [0x71; 32];
        let staged = journal
            .stage(operation, &bytes)
            .expect("inclusive exact byte ceiling");
        assert_eq!(staged.bytes(), bytes);
        staged.recheck().expect("same retained bytes and identity");
        let record_path = path.join(format!("{}{SUFFIX}", hex::encode(operation)));
        let metadata = fs::metadata(&record_path).unwrap();
        assert_eq!(metadata.len(), maximum as u64);
        assert_eq!(metadata.mode() & 0o7777, 0o400);
        assert_eq!(metadata.nlink(), 1);
        drop(staged);
        assert!(matches!(
            journal.stage(operation, &bytes),
            Err(SignerReceiptJournalErrorV1::Unavailable)
        ));
        assert_eq!(journal.recover(operation).unwrap().bytes(), bytes);
        assert_eq!(
            fs::read_dir(&path).unwrap().count(),
            1,
            "duplicate staging cannot create a second record"
        );
        drop(journal);
        let reopened = SignerReceiptJournalV1::open_test(&path, purpose)
            .expect("exact ceiling survives inventory and reopen");
        assert_eq!(reopened.recover(operation).unwrap().bytes(), bytes);
        fs::set_permissions(&record_path, Permissions::from_mode(0o600)).unwrap();
        fs::write(&record_path, vec![0x57; maximum + 1]).unwrap();
        fs::set_permissions(&record_path, Permissions::from_mode(0o400)).unwrap();
        assert!(
            matches!(
                reopened.recover(operation),
                Err(SignerReceiptJournalErrorV1::Unavailable)
            ),
            "fresh recovery must reject oversized private records"
        );
        drop(reopened);
        assert_eq!(
            SignerReceiptJournalV1::open_test(&path, purpose).unwrap_err(),
            SignerReceiptJournalErrorV1::Unavailable,
            "inventory must reject the same oversized private record"
        );
    }
}

#[test]
fn neutral_journal_errors_reveal_no_input_path_or_receipt_bytes() {
    let path = Path::new("credential-secret-material/private-journal");
    let error =
        SignerReceiptJournalV1::open_test(path, SignerReceiptPurposeV1::StreamToken).unwrap_err();
    assert_eq!(
        error.to_string(),
        "signer private receipt journal unavailable"
    );
    assert_eq!(format!("{error:?}"), "Unavailable");
    assert_eq!(
        super::super::release_manifest::SignerReleaseManifestErrorV1::from(error),
        super::super::release_manifest::SignerReleaseManifestErrorV1::Journal
    );
}

mod pending_reserve;
mod reader;
