//! Terminal flow tests: abandonment of an unused enrollment (and its competition with
//! Bootstrap) and deliberate custody deletion.

use super::*;
use crate::kagemusha_wallet_advance_v1::*;
use crate::kagemusha_wallet_advance_v1::{
    KagemushaWalletAnchorPolicyV1, KagemushaWalletAnchorV1, KagemushaWalletChallengeLivenessV1,
    KagemushaWalletMarkerPhaseV1, kagemusha_wallet_markers_dir_v1, kagemusha_wallet_ops_dir_v1,
    kagemusha_wallet_tombstone_name_v1,
    test_support::{
        BOOT_A, DeviceV1, SimProviderV1, TestOwnerV1, advance_request, bootstrap_capsule,
        bootstrapped_device, enrolled_device, wallet_fixture,
    },
};

const ANDROID: KagemushaWalletAnchorPolicyV1 = KagemushaWalletAnchorPolicyV1::NotRequired;
const IOS: KagemushaWalletAnchorPolicyV1 = KagemushaWalletAnchorPolicyV1::Keychain;

fn marker_names(device: &DeviceV1, slot: &KagemushaWalletSlotIdV1) -> Vec<String> {
    device
        .fs
        .visible_names(&kagemusha_wallet_markers_dir_v1(slot))
}

fn anchored(device: &DeviceV1, slot: &KagemushaWalletSlotIdV1) -> Option<KagemushaWalletAnchorV1> {
    device.platform.with(|state| {
        state
            .anchors
            .get(slot)
            .map(|bytes| KagemushaWalletAnchorV1::decode(bytes).expect("anchor"))
    })
}

#[test]
fn wallet_advance_v1_terminal_confirmation_and_reason() {
    let f = wallet_fixture(0x90);
    let enrollment = f.enrollment_record(BOOT_A);
    let selected = enrollment
        .select(
            bootstrap_capsule(&f).head_marker_state().expect("head"),
            BOOT_A,
        )
        .expect("select");
    let terminal = selected
        .terminate(KagemushaWalletTerminalReasonV1::CustodyDeleted, BOOT_A)
        .expect("terminal");
    let slot = *enrollment.slot();
    for status in [
        KagemushaWalletSlotStatusV1::Released(selected.clone()),
        KagemushaWalletSlotStatusV1::Pending(selected.clone()),
    ] {
        assert_eq!(
            KagemushaWalletDestructiveConfirmationV1::for_status(slot, &status),
            Some(KagemushaWalletDestructiveConfirmationV1 {
                slot,
                marker_file_digest: *selected.marker_file_digest()
            })
        );
    }
    for status in [
        KagemushaWalletSlotStatusV1::Enrollment(enrollment.clone()),
        KagemushaWalletSlotStatusV1::Terminal(terminal.clone()),
        KagemushaWalletSlotStatusV1::Empty,
    ] {
        assert_eq!(
            KagemushaWalletDestructiveConfirmationV1::for_status(slot, &status),
            None
        );
    }
    assert_eq!(
        terminal_reason(&terminal),
        Some(KagemushaWalletTerminalReasonV1::CustodyDeleted)
    );
    assert_eq!(terminal_reason(&selected), None);
}

#[test]
fn wallet_advance_v1_terminal_abandonment_retains_one_signed_control() {
    for (policy, seed) in [(ANDROID, 0x91), (IOS, 0x92)] {
        let (device, f, slot) = enrolled_device(policy, seed);
        let mut provider = device.open();
        let frame = provider.abandon_enrollment(&slot).expect("abandon");
        let abandonment =
            KagemushaWalletAbandonmentV1::decode_canonical(&frame, &f.scheme_id()).expect("decode");
        let status = provider.status(&slot).expect("status");
        let KagemushaWalletSlotStatusV1::Terminal(terminal) = &status else {
            panic!("terminal: {status:?}");
        };
        assert_eq!(terminal.generation(), 1);
        assert_eq!(terminal.phase(), KagemushaWalletMarkerPhaseV1::Terminal);
        abandonment
            .require_terminal_marker(terminal.marker())
            .expect("names the durable terminal marker");
        assert_eq!(
            marker_names(&device, &slot),
            vec![terminal.file_name().as_str().to_owned()]
        );
        assert!(
            device.platform.key_of(&slot).is_some(),
            "abandonment keeps the key"
        );
        if policy == IOS {
            assert_eq!(
                anchored(&device, &slot),
                Some(KagemushaWalletAnchorV1::naming(terminal))
            );
        }
        // Every retry, here and after a restart, returns the retained bytes.
        assert_eq!(provider.abandon_enrollment(&slot), Ok(frame.clone()));
        drop(provider);
        device.fs.restart();
        let mut provider = device.open();
        assert_eq!(provider.abandon_enrollment(&slot), Ok(frame));
        // Bootstrap lost generation 1 for good.
        let enrollment = f.enrollment_record(BOOT_A);
        assert_eq!(
            provider.advance(
                &slot,
                &TestOwnerV1,
                &advance_request(&enrollment, &bootstrap_capsule(&f))
            ),
            Err(KagemushaWalletProviderErrorV1::Terminal)
        );
        assert_eq!(
            provider.test_resume_enrollment(&slot, KagemushaWalletChallengeLivenessV1::Live),
            Err(KagemushaWalletProviderErrorV1::Invalid {
                field: "enrollment.finished"
            })
        );
        assert!(device.platform.with(|state| state.violations.is_empty()));
    }
}

#[test]
fn wallet_advance_v1_terminal_abandonment_is_refused_after_bootstrap() {
    let (device, f, slot) = enrolled_device(ANDROID, 0x93);
    let mut provider: SimProviderV1 = device.open();
    let enrollment = provider.status(&slot).expect("status");
    let request = advance_request(enrollment.marker().expect("marker"), &bootstrap_capsule(&f));
    // A selected Bootstrap whose receipt is still pending already blocks abandonment.
    device.platform.with(|state| state.sign_unavailable = true);
    assert!(matches!(
        provider.advance(&slot, &TestOwnerV1, &request),
        Ok(crate::kagemusha_wallet_advance_v1::KagemushaWalletAdvanceOutcomeV1::Pending { .. })
    ));
    let refusal = KagemushaWalletProviderErrorV1::Invalid {
        field: "abandon.bootstrap",
    };
    let refused = Err(refusal);
    assert_eq!(provider.abandon_enrollment(&slot), refused);
    device.platform.with(|state| state.sign_unavailable = false);
    provider.reconcile(&slot, &TestOwnerV1).expect("released");
    assert_eq!(provider.abandon_enrollment(&slot), refused);
    // The NOREPLACE competition itself: generation 1 taken by Bootstrap.
    let (device, f, slot) = enrolled_device(ANDROID, 0x94);
    let mut provider = device.open();
    let enrollment = provider.status(&slot).expect("status");
    let enrollment = enrollment.marker().expect("marker").clone();
    let selected = enrollment
        .select(
            bootstrap_capsule(&f).head_marker_state().expect("head"),
            BOOT_A,
        )
        .expect("select");
    kagemusha_wallet_publish_marker_v1(provider.store(), selected).expect("publish");
    assert_eq!(
        provider.commit_abandonment(&slot, &enrollment),
        Err(refusal)
    );
    // Unenrolled and deleted slots.
    let empty = KagemushaWalletSlotIdV1([0x94; 32]);
    crate::kagemusha_wallet_advance_v1::kagemusha_wallet_prepare_slot_dirs_v1(
        provider.store(),
        &empty,
    )
    .expect("slot");
    assert_eq!(
        provider.abandon_enrollment(&empty),
        Err(KagemushaWalletProviderErrorV1::Invalid {
            field: "slot.not_enrolled"
        })
    );
    let (device, _f, slot, _) = bootstrapped_device(ANDROID, 0x95);
    let mut provider = device.open();
    let status = provider.status(&slot).expect("status");
    let confirmation =
        KagemushaWalletDestructiveConfirmationV1::for_status(slot, &status).expect("confirm");
    provider
        .delete_custody(&slot, &confirmation)
        .expect("delete");
    assert_eq!(
        provider.abandon_enrollment(&slot),
        Err(KagemushaWalletProviderErrorV1::Terminal)
    );
}

#[test]
fn wallet_advance_v1_terminal_custody_deletion_is_confirmed_and_ordered() {
    for (policy, seed) in [(ANDROID, 0x96), (IOS, 0x97)] {
        let (device, _f, slot, _bootstrap) = bootstrapped_device(policy, seed);
        let mut provider = device.open();
        let status = provider.status(&slot).expect("status");
        let confirmation =
            KagemushaWalletDestructiveConfirmationV1::for_status(slot, &status).expect("confirm");
        let other = KagemushaWalletDestructiveConfirmationV1 {
            slot: KagemushaWalletSlotIdV1([0x01; 32]),
            ..confirmation
        };
        assert_eq!(
            provider.delete_custody(&slot, &other),
            Err(KagemushaWalletProviderErrorV1::Invalid {
                field: "custody.confirmation_slot"
            })
        );
        let stale = KagemushaWalletDestructiveConfirmationV1 {
            marker_file_digest: [0x02; 32],
            ..confirmation
        };
        assert_eq!(
            provider.delete_custody(&slot, &stale),
            Err(KagemushaWalletProviderErrorV1::Invalid {
                field: "custody.confirmation_stale"
            })
        );
        // The key is kept while deletion is refused by the key store, then deleted.
        device.platform.with(|state| state.delete_refused = true);
        assert!(matches!(
            provider.delete_custody(&slot, &confirmation),
            Err(KagemushaWalletProviderErrorV1::Unavailable(_))
        ));
        assert!(device.platform.key_of(&slot).is_some());
        let deleted = provider.status(&slot);
        assert!(
            deleted.is_err(),
            "reconcile retries the key deletion: {deleted:?}"
        );
        device.platform.with(|state| state.delete_refused = false);
        let KagemushaWalletSlotStatusV1::Terminal(terminal) =
            provider.status(&slot).expect("status")
        else {
            panic!("terminal");
        };
        assert_eq!(device.platform.key_of(&slot), None);
        assert_eq!(
            terminal_reason(&terminal),
            Some(KagemushaWalletTerminalReasonV1::CustodyDeleted)
        );
        assert_eq!(
            marker_names(&device, &slot),
            vec![terminal.file_name().as_str().to_owned()]
        );
        for dir in [
            kagemusha_wallet_capsules_dir_v1(&slot),
            kagemusha_wallet_completion_dir_v1(&slot),
            kagemusha_wallet_archive_dir_v1(&slot),
        ] {
            assert!(device.fs.visible_names(&dir).is_empty(), "{}", dir.label());
        }
        assert!(
            device
                .fs
                .visible_names(&kagemusha_wallet_slot_dir_v1(&slot))
                .contains(&super::super::KAGEMUSHA_WALLET_INTENT_NAME_V1.to_owned())
        );
        if policy == IOS {
            assert_eq!(
                anchored(&device, &slot),
                Some(KagemushaWalletAnchorV1::naming(&terminal)),
                "the anchor is kept and names the terminal marker"
            );
        }
        assert_eq!(
            provider.delete_custody(&slot, &confirmation),
            Ok(KagemushaWalletSlotStatusV1::Terminal(terminal.clone()))
        );
        assert!(device.platform.with(|state| state.violations.is_empty()));
    }
}

#[test]
fn wallet_advance_v1_terminal_deletion_cases_and_file_removal() {
    // An enrollment marker is abandoned, not deleted.
    let (device, _f, slot) = enrolled_device(ANDROID, 0x98);
    let mut provider = device.open();
    let status = provider.status(&slot).expect("status");
    let confirmation = KagemushaWalletDestructiveConfirmationV1 {
        slot,
        marker_file_digest: *status.marker().expect("marker").marker_file_digest(),
    };
    assert_eq!(
        provider.delete_custody(&slot, &confirmation),
        Err(KagemushaWalletProviderErrorV1::Invalid {
            field: "custody.abandon_enrollment"
        })
    );
    provider.abandon_enrollment(&slot).expect("abandon");
    assert_eq!(
        provider.delete_custody(&slot, &confirmation),
        Err(KagemushaWalletProviderErrorV1::Terminal)
    );
    // A selected head whose receipt is pending can be deleted deliberately.
    let (device, f, slot, bootstrap) = bootstrapped_device(ANDROID, 0x99);
    let mut provider = device.open();
    let head = provider.status(&slot).expect("status");
    let request = advance_request(
        head.marker().expect("marker"),
        &crate::kagemusha_wallet_advance_v1::test_support::next_capsule(&f, &bootstrap),
    );
    device.platform.with(|state| state.sign_unavailable = true);
    provider
        .advance(&slot, &TestOwnerV1, &request)
        .expect("pending");
    let pending = provider.status(&slot).expect("status");
    assert!(matches!(pending, KagemushaWalletSlotStatusV1::Pending(_)));
    // Tombstones and archive subdirectories are kept; archive files are removed.
    let archive = kagemusha_wallet_archive_dir_v1(&slot);
    device.fs.place_unsynced(&archive, "outbox.bin", b"payment");
    provider.store().create_dir(
        &archive,
        &KagemushaWalletEntryNameV1::new("nested").expect("name"),
    );
    let tombstone = kagemusha_wallet_tombstone_name_v1(&[0x42; 32]);
    device.fs.place_unsynced(
        &kagemusha_wallet_ops_dir_v1(&slot),
        tombstone.as_str(),
        b"x",
    );
    let confirmation =
        KagemushaWalletDestructiveConfirmationV1::for_status(slot, &pending).expect("confirm");
    let KagemushaWalletSlotStatusV1::Terminal(terminal) = provider
        .delete_custody(&slot, &confirmation)
        .expect("delete")
    else {
        panic!("terminal");
    };
    assert_eq!(terminal.generation(), 4);
    assert_eq!(device.fs.visible_names(&archive), vec!["nested".to_owned()]);
    assert_eq!(
        device.fs.visible_names(&kagemusha_wallet_ops_dir_v1(&slot)),
        vec![tombstone.as_str().to_owned()]
    );
    provider.remove_files(&archive).expect("idempotent");
    provider.finish_custody_deletion(&slot).expect("idempotent");
    let durable = provider.cache.get(&slot).expect("cached").durable.clone();
    assert_eq!(durable.record(), &terminal);
    provider
        .finish_terminal_markers(&durable, &[])
        .expect("nothing to retire");
}

#[test]
fn abandonment_selected_original_loss_never_resigns_after_retry_or_restart() {
    for (policy, seed) in [(ANDROID, 0xb1), (IOS, 0xb2)] {
        let (device, _, slot) = enrolled_device(policy, seed);
        let mut provider = device.open();
        provider.abandon_enrollment(&slot).unwrap();
        let calls = device.platform.with(|state| state.sign_calls);
        let dir = kagemusha_wallet_slot_dir_v1(&slot);
        device
            .fs
            .unlink(&dir, KAGEMUSHA_WALLET_ABANDONMENT_NAME_V1)
            .unwrap();
        device.fs.sync_dir(&dir).unwrap();
        assert_eq!(
            provider.abandon_enrollment(&slot),
            Err(abandonment_custody_error())
        );
        drop(provider);
        device.fs.restart();
        let mut provider = device.open();
        assert_eq!(
            provider.abandon_enrollment(&slot),
            Err(abandonment_custody_error())
        );
        device
            .platform
            .with(|state| assert_eq!(state.sign_calls, calls));
    }
}

#[test]
fn abandonment_selection_recovers_without_resigning_and_rejects_substitution() {
    let (device, _, slot) = enrolled_device(ANDROID, 0xb3);
    let mut provider = device.open();
    let original = provider.abandon_enrollment(&slot).unwrap();
    let calls = device.platform.with(|state| state.sign_calls);
    let dir = kagemusha_wallet_slot_dir_v1(&slot);
    device.fs.unlink(&dir, ABANDONMENT_SELECTION_NAME).unwrap();
    device.fs.sync_dir(&dir).unwrap();
    assert_eq!(provider.abandon_enrollment(&slot).unwrap(), original);
    device
        .platform
        .with(|state| assert_eq!(state.sign_calls, calls));
    let bytes = device
        .fs
        .visible_file(&dir, ABANDONMENT_SELECTION_NAME)
        .unwrap();
    let mut selected: AbandonmentSelectionV1 =
        decode_envelope_v1(&bytes, ABANDONMENT_SELECTION_MAX).unwrap();
    selected.original[0] ^= 1;
    device.fs.place_unsynced(
        &dir,
        ABANDONMENT_SELECTION_NAME,
        &encode_envelope_v1(&selected, ABANDONMENT_SELECTION_MAX).unwrap(),
    );
    assert_eq!(
        provider.abandon_enrollment(&slot),
        Err(abandonment_custody_error())
    );
    device
        .platform
        .with(|state| assert_eq!(state.sign_calls, calls));
}

#[test]
fn abandonment_selection_partial_write_and_uncertain_publication_recover_exact_original() {
    let (device, _, slot) = enrolled_device(ANDROID, 0xb4);
    let original = {
        let mut provider = device.open();
        provider.abandon_enrollment(&slot).unwrap()
    };
    let dir = kagemusha_wallet_slot_dir_v1(&slot);
    device.fs.unlink(&dir, ABANDONMENT_SELECTION_NAME).unwrap();
    device.fs.sync_dir(&dir).unwrap();
    let baseline = device.fork();
    let mut provider = baseline.open();
    let start = baseline.fs.steps();
    assert_eq!(provider.abandon_enrollment(&slot).unwrap(), original);
    let trace = baseline.fs.trace_since(start);
    let write = trace
        .iter()
        .rposition(|step| *step == KagemushaWalletSimStepV1::Write)
        .unwrap();
    let sync = trace
        .iter()
        .rposition(|step| *step == KagemushaWalletSimStepV1::SyncDir)
        .unwrap();
    for (offset, fault) in [
        (write, KagemushaWalletSimFaultV1::PartialWrite),
        (sync, KagemushaWalletSimFaultV1::Error),
    ] {
        let trial = device.fork();
        let mut provider = trial.open();
        let calls = trial.platform.with(|state| state.sign_calls);
        trial.fs.inject(trial.fs.steps() + offset as u64, fault);
        assert!(provider.abandon_enrollment(&slot).is_err());
        trial.fs.clear_faults();
        drop(provider);
        trial.fs.restart();
        let mut provider = trial.open();
        assert_eq!(provider.abandon_enrollment(&slot).unwrap(), original);
        trial
            .platform
            .with(|state| assert_eq!(state.sign_calls, calls));
    }
}
