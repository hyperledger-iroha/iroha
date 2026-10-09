//! Enrollment tests: the create-new helpers, refusal before key generation, the key gate,
//! resumption of interrupted enrollments, the retained credential request and credentials.

use super::*;
use crate::kagemusha_wallet_advance_v1::*;
use crate::kagemusha_wallet_advance_v1::{
    KagemushaWalletSimFaultV1, KagemushaWalletSlotStatusV1, kagemusha_wallet_markers_dir_v1,
    test_support::{
        AnchorWriteV1, DeviceV1, PROFILE, SimProviderV1, bootstrapped_device, enrolled_device,
        enrollment_challenge,
    },
};

const ANDROID: KagemushaWalletAnchorPolicyV1 = KagemushaWalletAnchorPolicyV1::NotRequired;
const IOS: KagemushaWalletAnchorPolicyV1 = KagemushaWalletAnchorPolicyV1::Keychain;

fn enrolled_slot(step: KagemushaWalletEnrollmentStepV1) -> KagemushaWalletSlotIdV1 {
    match step {
        KagemushaWalletEnrollmentStepV1::Enrolled { slot, .. } => slot,
        other => panic!("not enrolled: {other:?}"),
    }
}

/// A slot whose intent is durable and whose key generation reported `Unavailable`.
fn interrupted(seed: u8, created: bool) -> (DeviceV1, SimProviderV1, KagemushaWalletSlotIdV1) {
    let device = DeviceV1::new(ANDROID, seed);
    device
        .platform
        .with(|state| state.generate_unavailable = Some(created));
    let mut provider = device.open();
    assert!(matches!(
        provider.begin_enrollment(&enrollment_challenge(seed), PROFILE),
        Err(KagemushaWalletProviderErrorV1::Unavailable(_))
    ));
    device
        .platform
        .with(|state| state.generate_unavailable = None);
    let slot = provider.slots().expect("slots")[0];
    (device, provider, slot)
}

#[test]
fn wallet_advance_v1_enrollment_write_once_and_read_record() {
    let device = DeviceV1::new(ANDROID, 0x80);
    let provider = device.open();
    let store = provider.store();
    let dir = KagemushaWalletCustodyDirV1::root();
    let name = kagemusha_wallet_fixed_name_v1(KAGEMUSHA_WALLET_INTENT_NAME_V1);
    let record = KagemushaWalletAbandonedSlotV1 {
        version: KAGEMUSHA_WALLET_ENROLLMENT_FILE_VERSION_V1,
        slot: [1; 32],
        reason: 2,
    };
    let bytes = encode_envelope_v1(&record, 256).expect("encode");
    assert_eq!(
        read_record::<_, KagemushaWalletAbandonedSlotV1>(store, &dir, &name, 256, "x"),
        Ok(None)
    );
    assert!(matches!(
        write_once(store, &dir, &name, &bytes, 256),
        Ok(WriteOnceV1::Written)
    ));
    assert!(matches!(
        write_once(store, &dir, &name, &bytes, 256),
        Ok(WriteOnceV1::Written)
    ));
    assert!(matches!(
        write_once(store, &dir, &name, b"other", 256),
        Ok(WriteOnceV1::Existing(existing)) if existing == bytes
    ));
    assert_eq!(
        read_record::<_, KagemushaWalletAbandonedSlotV1>(store, &dir, &name, 256, "x"),
        Ok(Some(record))
    );
    assert_eq!(
        read_record::<_, KagemushaWalletAbandonedSlotV1>(store, &dir, &name, 4, "x"),
        Err(KagemushaWalletProviderErrorV1::UnavailableCustodyData { object: "x" })
    );
    device.fs.place_unsynced(&dir, name.as_str(), b"corrupt");
    assert_eq!(
        read_record::<_, KagemushaWalletAbandonedSlotV1>(store, &dir, &name, 256, "x"),
        Err(KagemushaWalletProviderErrorV1::UnavailableCustodyData { object: "x" })
    );
    device
        .fs
        .inject(device.fs.steps(), KagemushaWalletSimFaultV1::Error);
    assert!(matches!(
        read_record::<_, KagemushaWalletAbandonedSlotV1>(store, &dir, &name, 256, "x"),
        Err(KagemushaWalletProviderErrorV1::Unavailable(_))
    ));
    assert!(request_file_max() > KAGEMUSHA_WALLET_ENROLLMENT_REQUEST_MAX_BYTES_V1);
    assert!(credential_file_max() > KAGEMUSHA_WALLET_CREDENTIAL_MAX_BYTES_V1);
}

#[test]
fn wallet_advance_v1_enrollment_refusals_happen_before_any_key() {
    let device = DeviceV1::new(ANDROID, 0x81);
    let mut provider = device.open();
    let mut foreign = enrollment_challenge(0x81);
    foreign.scheme_id = [0x22; 32];
    assert_eq!(
        provider.begin_enrollment(&foreign, PROFILE),
        Err(KagemushaWalletProviderErrorV1::Invalid {
            field: "challenge.scheme_id"
        })
    );
    let mut invalid = enrollment_challenge(0x81);
    invalid.issuer_nonce = [0; 32];
    assert_eq!(
        provider.begin_enrollment(&invalid, PROFILE),
        Err(KagemushaWalletProviderErrorV1::Invalid { field: "challenge" })
    );
    // A filesystem without create-new rename refuses enrollment; there is no fallback.
    device.fs.set_noreplace_supported(false);
    assert_eq!(
        provider.begin_enrollment(&enrollment_challenge(0x81), PROFILE),
        Err(KagemushaWalletProviderErrorV1::NoReplaceUnsupported)
    );
    device.fs.set_noreplace_supported(true);
    // No room for the ballast: refused before the slot exists.
    device.fs.set_capacity(Some(0));
    assert_eq!(
        provider.begin_enrollment(&enrollment_challenge(0x81), PROFILE),
        Err(KagemushaWalletProviderErrorV1::NoSpace)
    );
    device.fs.set_capacity(None);
    assert_eq!(provider.slots(), Ok(Vec::new()));
    // iOS without a device passcode: the anchor cannot be added, so no intent and no key.
    let ios = DeviceV1::new(IOS, 0x82);
    ios.platform
        .with(|state| state.anchor_write = AnchorWriteV1::Refused);
    let mut provider = ios.open();
    assert!(matches!(
        provider.begin_enrollment(&enrollment_challenge(0x82), PROFILE),
        Err(KagemushaWalletProviderErrorV1::Unavailable(_))
    ));
    let slots = provider.slots().expect("slots");
    assert_eq!(slots.len(), 1);
    assert_eq!(
        provider.status(&slots[0]),
        Ok(KagemushaWalletSlotStatusV1::Empty)
    );
    for platform in [&device.platform, &ios.platform] {
        assert_eq!(platform.with(|state| state.generate_calls), 0);
    }
}

#[test]
fn wallet_advance_v1_enrollment_begin_writes_the_marker_before_any_request() {
    for (policy, seed) in [(ANDROID, 0x83), (IOS, 0x84)] {
        let device = DeviceV1::new(policy, seed);
        let mut provider = device.open();
        let step = provider
            .begin_enrollment(&enrollment_challenge(seed), PROFILE)
            .expect("begin");
        let KagemushaWalletEnrollmentStepV1::Enrolled { slot, marker } = step else {
            panic!("enrolled");
        };
        assert_eq!(marker.generation(), 0);
        assert_eq!(
            marker.marker().wallet_id,
            enrollment_challenge(seed).wallet_id(marker.payment_key())
        );
        assert_eq!(
            device.platform.key_of(&slot).as_ref(),
            Some(marker.payment_key())
        );
        let intent = provider
            .read_intent(&slot)
            .expect("intent")
            .expect("present");
        assert_eq!(intent.challenge, enrollment_challenge(seed));
        assert_eq!(intent.anchor_kind, u8::from(policy == IOS));
        assert_eq!(provider.enrollment_record(&slot), Ok(None));
        if policy == IOS {
            let anchor = device
                .platform
                .with(|state| state.anchors.get(&slot).cloned())
                .expect("anchor");
            assert_eq!(
                KagemushaWalletAnchorV1::decode(&anchor),
                Ok(KagemushaWalletAnchorV1::naming(&marker))
            );
        }
        assert!(device.platform.with(|state| state.violations.is_empty()));
        assert_eq!(device.platform.with(|state| state.generate_calls), 1);
    }
}

#[test]
fn wallet_advance_v1_enrollment_key_without_marker_is_never_used() {
    // E3a with the key created: the slot is abandoned and a new slot gets a new key.
    let (device, mut provider, slot) = interrupted(0x85, true);
    assert_eq!(
        provider.resume_enrollment(&slot, KagemushaWalletChallengeLivenessV1::Live),
        Ok(KagemushaWalletEnrollmentStepV1::SlotAbandoned { slot })
    );
    assert_eq!(
        provider.status(&slot),
        Ok(KagemushaWalletSlotStatusV1::SlotAbandoned)
    );
    assert!(
        device
            .fs
            .visible_names(&kagemusha_wallet_markers_dir_v1(&slot))
            .is_empty()
    );
    assert_eq!(device.platform.with(|state| state.generate_calls), 1);
    device.platform.with(|state| state.next_key_seed = 0x86);
    let fresh = enrolled_slot(
        provider
            .begin_enrollment(&enrollment_challenge(0x86), PROFILE)
            .expect("new slot"),
    );
    assert_ne!(fresh, slot);
    // E3a without the key: the same slot continues while the challenge is live.
    let (device, mut provider, slot) = interrupted(0x87, false);
    assert_eq!(
        provider.status(&slot),
        Ok(KagemushaWalletSlotStatusV1::IntentOnly)
    );
    assert_eq!(
        enrolled_slot(
            provider
                .resume_enrollment(&slot, KagemushaWalletChallengeLivenessV1::Live)
                .expect("resume")
        ),
        slot
    );
    assert!(device.platform.with(|state| state.violations.is_empty()));
    // Expired challenge: abandoned, never generated.
    let (device, mut provider, slot) = interrupted(0x88, false);
    assert_eq!(
        provider.resume_enrollment(&slot, KagemushaWalletChallengeLivenessV1::Expired),
        Ok(KagemushaWalletEnrollmentStepV1::SlotAbandoned { slot })
    );
    assert_eq!(device.platform.with(|state| state.generate_calls), 1);
    assert_eq!(device.platform.key_of(&slot), None);
    // Unavailable probes never abandon or generate.
    let (device, mut provider, slot) = interrupted(0x89, false);
    device.platform.with(|state| state.probe_unavailable = true);
    assert!(matches!(
        provider.resume_enrollment(&slot, KagemushaWalletChallengeLivenessV1::Live),
        Err(KagemushaWalletProviderErrorV1::Unavailable(_))
    ));
    assert_eq!(device.platform.with(|state| state.generate_calls), 1);
    device
        .platform
        .with(|state| state.probe_unavailable = false);
    assert_eq!(
        provider.status(&slot),
        Ok(KagemushaWalletSlotStatusV1::IntentOnly)
    );
    // `abandon_slot` keeps an earlier record.
    provider
        .abandon_slot(&slot, KagemushaWalletSlotAbandonReasonV1::ChallengeExpired)
        .expect("abandon");
    provider
        .abandon_slot(&slot, KagemushaWalletSlotAbandonReasonV1::KeyWithoutMarker)
        .expect("again");
    let record = read_record::<_, KagemushaWalletAbandonedSlotV1>(
        provider.store(),
        &kagemusha_wallet_slot_dir_v1(&slot),
        &kagemusha_wallet_fixed_name_v1(KAGEMUSHA_WALLET_ABANDONED_NAME_V1),
        KAGEMUSHA_WALLET_INTENT_MAX_BYTES_V1,
        "abandoned",
    )
    .expect("read")
    .expect("present");
    assert_eq!(
        record.reason,
        KagemushaWalletSlotAbandonReasonV1::ChallengeExpired.tag()
    );
}

#[test]
fn wallet_advance_v1_enrollment_resume_answers_each_state() {
    let (device, f, slot) = enrolled_device(ANDROID, 0x8a);
    let mut provider = device.open();
    let KagemushaWalletEnrollmentStepV1::Enrolled { marker, .. } = provider
        .resume_enrollment(&slot, KagemushaWalletChallengeLivenessV1::Expired)
        .expect("enrolled")
    else {
        panic!("enrolled");
    };
    assert_eq!(*marker.marker(), f.enrollment);
    let empty = KagemushaWalletSlotIdV1([0x8a; 32]);
    kagemusha_wallet_prepare_slot_dirs_v1(provider.store(), &empty).expect("slot");
    assert_eq!(
        provider.resume_enrollment(&empty, KagemushaWalletChallengeLivenessV1::Live),
        Err(KagemushaWalletProviderErrorV1::Invalid {
            field: "slot.no_intent"
        })
    );
    let (device, _f, slot, _) = bootstrapped_device(ANDROID, 0x8b);
    assert_eq!(
        device
            .open()
            .resume_enrollment(&slot, KagemushaWalletChallengeLivenessV1::Live),
        Err(KagemushaWalletProviderErrorV1::Invalid {
            field: "enrollment.finished"
        })
    );
}

#[test]
fn wallet_advance_v1_enrollment_request_is_retained_byte_identical() {
    let device = DeviceV1::new(ANDROID, 0x8c);
    let mut provider = device.open();
    let slot = enrolled_slot(
        provider
            .begin_enrollment(&enrollment_challenge(0x8c), PROFILE)
            .expect("begin"),
    );
    assert_eq!(
        provider.retain_enrollment_request(&slot, b""),
        Err(KagemushaWalletProviderErrorV1::Invalid {
            field: "enrollment.request"
        })
    );
    assert_eq!(
        provider.store_credential(&slot, 0, b"credential"),
        Err(KagemushaWalletProviderErrorV1::Invalid {
            field: "credential.before_request"
        })
    );
    assert_eq!(
        provider.retain_enrollment_request(&slot, b"first"),
        Ok(b"first".to_vec())
    );
    assert_eq!(
        provider.retain_enrollment_request(&slot, b"second"),
        Ok(b"first".to_vec())
    );
    drop(provider);
    device.fs.restart();
    let mut provider = device.open();
    assert_eq!(
        provider.retain_enrollment_request(&slot, b"third"),
        Ok(b"first".to_vec())
    );
    let record = provider
        .enrollment_record(&slot)
        .expect("record")
        .expect("present");
    let enrollment = provider.status(&slot).expect("status");
    assert_eq!(
        &record.enrollment_marker_digest,
        enrollment.marker().expect("marker").marker_digest()
    );
    // Credentials: create-new, identical bytes accepted, different bytes refused.
    assert_eq!(provider.credential(&slot, 0), Ok(None));
    provider
        .store_credential(&slot, 0, b"credential")
        .expect("store");
    provider
        .store_credential(&slot, 0, b"credential")
        .expect("identical");
    assert_eq!(
        provider.store_credential(&slot, 0, b"other"),
        Err(KagemushaWalletProviderErrorV1::Invalid {
            field: "credential.differs"
        })
    );
    assert_eq!(
        provider.store_credential(&slot, 1, &[0; 2_000]),
        Err(KagemushaWalletProviderErrorV1::Invalid {
            field: "credential"
        })
    );
    assert_eq!(
        provider.credential(&slot, 0),
        Ok(Some(b"credential".to_vec()))
    );
    assert_eq!(provider.credential(&slot, 1), Ok(None));
    // A request is never retained for a slot without a current enrollment marker.
    let (_device, mut other, intent_slot) = interrupted(0x8d, false);
    assert_eq!(
        other.retain_enrollment_request(&intent_slot, b"request"),
        Err(KagemushaWalletProviderErrorV1::Invalid {
            field: "enrollment.phase"
        })
    );
    // A corrupt intent is unavailable custody data, never a fresh slot.
    device.fs.place_unsynced(
        &kagemusha_wallet_slot_dir_v1(&slot),
        KAGEMUSHA_WALLET_INTENT_NAME_V1,
        b"corrupt",
    );
    assert_eq!(
        provider.read_intent(&slot),
        Err(KagemushaWalletProviderErrorV1::UnavailableCustodyData { object: "intent" })
    );
}

#[test]
fn wallet_advance_v1_enrollment_key_generation_binds_challenge_and_profile() {
    // Regression: the platform receives the attestation challenge and the hardware key policy
    // (it used to receive only the slot), and a resumed enrollment uses the recorded profile.
    let device = DeviceV1::new(ANDROID, 0x8e);
    let mut provider = device.open();
    let challenge = enrollment_challenge(0x8e);
    let slot = enrolled_slot(
        provider
            .begin_enrollment(&challenge, KagemushaWalletKeyProfileV1::SecureElement)
            .expect("begin"),
    );
    assert_eq!(
        device.platform.with(|state| state.last_generation),
        Some(KagemushaWalletKeyGenerationRequestV1 {
            challenge_digest: challenge.challenge_digest(),
            profile: KagemushaWalletKeyProfileV1::SecureElement,
        })
    );
    let intent = provider
        .read_intent(&slot)
        .expect("intent")
        .expect("present");
    assert_eq!(
        intent.key_profile(),
        Ok(KagemushaWalletKeyProfileV1::SecureElement)
    );
    assert_eq!(intent.anchor(), Ok(ANDROID));
    // Resumption generates under the intent's profile, not a new one.
    let (device, mut provider, slot) = interrupted(0x8f, false);
    device.platform.with(|state| state.last_generation = None);
    enrolled_slot(
        provider
            .resume_enrollment(&slot, KagemushaWalletChallengeLivenessV1::Live)
            .expect("resume"),
    );
    assert_eq!(
        device
            .platform
            .with(|state| state.last_generation.map(|request| request.profile)),
        Some(PROFILE)
    );
    // An unknown profile tag is unavailable custody data, never a default.
    let invalid = KagemushaWalletIntentV1 {
        profile: 9,
        ..intent
    };
    assert_eq!(
        invalid.key_profile(),
        Err(KagemushaWalletProviderErrorV1::UnavailableCustodyData { object: "intent" })
    );
    let invalid = KagemushaWalletIntentV1 {
        anchor_kind: 9,
        ..intent
    };
    assert_eq!(
        invalid.anchor(),
        Err(KagemushaWalletProviderErrorV1::UnavailableCustodyData { object: "intent" })
    );
}

#[test]
fn wallet_advance_v1_enrollment_resume_refuses_a_changed_anchor_policy() {
    // The slot's anchor kind is fixed by its intent: a platform that now answers otherwise
    // never generates a key or writes a marker for it.
    let device = DeviceV1::new(IOS, 0x90);
    device
        .platform
        .with(|state| state.generate_unavailable = Some(false));
    let mut provider = device.open();
    assert!(matches!(
        provider.begin_enrollment(&enrollment_challenge(0x90), PROFILE),
        Err(KagemushaWalletProviderErrorV1::Unavailable(_))
    ));
    let slot = provider.slots().expect("slots")[0];
    device.platform.with(|state| {
        state.generate_unavailable = None;
        state.policy = ANDROID;
    });
    let calls = device.platform.with(|state| state.generate_calls);
    assert_eq!(
        provider.resume_enrollment(&slot, KagemushaWalletChallengeLivenessV1::Live),
        Err(KagemushaWalletProviderErrorV1::UnavailableCustodyData {
            object: "anchor policy"
        })
    );
    assert_eq!(device.platform.with(|state| state.generate_calls), calls);
    assert!(
        device
            .fs
            .visible_names(&kagemusha_wallet_markers_dir_v1(&slot))
            .is_empty()
    );
    device.platform.with(|state| state.policy = IOS);
    let KagemushaWalletEnrollmentStepV1::Enrolled { marker, .. } = provider
        .resume_enrollment(&slot, KagemushaWalletChallengeLivenessV1::Live)
        .expect("resume")
    else {
        panic!("enrolled");
    };
    assert_eq!(marker.anchor(), IOS);
}

#[test]
fn wallet_advance_v1_enrollment_record_reader_has_no_side_effects() {
    // Regression: the public reader used to rewrite the record durably without the slot
    // guard; adoption now happens only inside the mutating operations.
    let (device, _f, slot) = enrolled_device(ANDROID, 0x91);
    let provider = device.open();
    let start = device.fs.steps();
    let record = provider
        .enrollment_record(&slot)
        .expect("record")
        .expect("present");
    let trace = device.fs.trace_since(start);
    assert_eq!(
        trace,
        vec![crate::kagemusha_wallet_advance_v1::KagemushaWalletSimStepV1::Read]
    );
    assert_eq!(record.request, b"request".to_vec());
    // The mutating path adopts it on a fresh inode.
    let mut provider = provider;
    let dir = kagemusha_wallet_slot_dir_v1(&slot);
    let inode = device
        .fs
        .inode_of(&dir, KAGEMUSHA_WALLET_ENROLLMENT_NAME_V1);
    assert_eq!(
        provider.retain_enrollment_request(&slot, b"other"),
        Ok(b"request".to_vec())
    );
    assert_ne!(
        device
            .fs
            .inode_of(&dir, KAGEMUSHA_WALLET_ENROLLMENT_NAME_V1),
        inode
    );
    // An unknown rewrite outcome poisons the slot.
    drop(provider);
    let probe = device.fork();
    let mut probe_provider = probe.open();
    probe_provider.status(&slot).expect("status");
    let start = probe.fs.steps();
    probe_provider
        .retain_enrollment_request(&slot, b"other")
        .expect("probe");
    let rename = probe
        .fs
        .trace_since(start)
        .iter()
        .position(|step| {
            *step == crate::kagemusha_wallet_advance_v1::KagemushaWalletSimStepV1::RenameReplace
        })
        .expect("rename");
    drop(probe_provider);
    let mut provider = device.open();
    provider.status(&slot).expect("status");
    assert!(provider.cache.contains_key(&slot));
    device.fs.inject(
        device.fs.steps() + u64::try_from(rename).expect("index") + 1,
        KagemushaWalletSimFaultV1::Error,
    );
    assert!(matches!(
        provider.retain_enrollment_request(&slot, b"other"),
        Err(KagemushaWalletProviderErrorV1::Uncertain(_))
    ));
    assert!(!provider.cache.contains_key(&slot), "poisoned");
}

fn fresh_device(seed: u8) -> DeviceV1 {
    let device = DeviceV1::new(ANDROID, seed);
    device.platform.with(|state| {
        state.generation_policy = KagemushaWalletKeyGenerationPolicyV1::FreshEnrollmentOnly;
    });
    device
}

#[test]
fn wallet_advance_v1_enrollment_fresh_grant_is_bound_to_owner_slot_and_request() {
    let device = fresh_device(0xa0);
    let slot = KagemushaWalletSlotIdV1([0xa0; 32]);
    let request = KagemushaWalletKeyGenerationRequestV1 {
        challenge_digest: [0xa1; 32],
        profile: PROFILE,
    };
    let grant = KagemushaWalletFreshGenerationV1::new(&device.platform, slot, request);
    assert_eq!(grant.consume(&device.platform), Ok((slot, request)));
    // Sharing backend state does not make another owner object the same owner. A rejected
    // grant is consumed as well; neither branch can issue a second callback with it.
    let other_owner = device.platform.clone();
    let grant = KagemushaWalletFreshGenerationV1::new(&device.platform, slot, request);
    assert_eq!(
        grant.consume(&other_owner),
        Err(KagemushaWalletUnavailableV1::Platform(0))
    );
    assert_eq!(device.platform.with(|state| state.generate_calls), 0);
}

#[test]
fn wallet_advance_v1_enrollment_fresh_publishes_bound_records_before_generation() {
    let device = fresh_device(0xa1);
    let mut provider = device.open();
    let challenge = enrollment_challenge(0xa1);
    let slot = enrolled_slot(
        provider
            .begin_enrollment(&challenge, PROFILE)
            .expect("fresh begin"),
    );
    let intent = provider
        .read_intent(&slot)
        .expect("intent")
        .expect("present");
    assert_eq!(intent.version, KAGEMUSHA_WALLET_INTENT_FILE_VERSION_V1);
    assert_eq!(
        intent.key_generation_policy(),
        Ok(KagemushaWalletKeyGenerationPolicyV1::FreshEnrollmentOnly)
    );
    provider
        .validate_generation_attempt(&slot, &intent)
        .expect("bound attempt");
    assert_eq!(
        device.platform.with(|state| state.last_generation),
        Some(KagemushaWalletKeyGenerationRequestV1 {
            challenge_digest: challenge.challenge_digest(),
            profile: PROFILE,
        })
    );
    assert_eq!(
        device.platform.with(|state| state.fresh_generation_calls),
        1
    );
    assert!(device.platform.with(|state| state.violations.is_empty()));
    // Once a marker exists, an ordinary enrollment resume returns it without generation.
    assert!(matches!(
        provider.resume_enrollment(&slot, KagemushaWalletChallengeLivenessV1::Live),
        Ok(KagemushaWalletEnrollmentStepV1::Enrolled { .. })
    ));
    assert_eq!(device.platform.with(|state| state.generate_calls), 1);
    assert_eq!(device.platform.with(|state| state.enumerate_calls), 0);
}

#[test]
fn wallet_advance_v1_enrollment_fresh_failure_never_retries_after_upgrade() {
    let device = fresh_device(0xa2);
    device
        .platform
        .with(|state| state.generate_unavailable = Some(false));
    let mut provider = device.open();
    assert!(matches!(
        provider.begin_enrollment(&enrollment_challenge(0xa2), PROFILE),
        Err(KagemushaWalletProviderErrorV1::Unavailable(_))
    ));
    let slot = provider.slots().expect("slots")[0];
    assert!(matches!(
        provider.resume_enrollment(&slot, KagemushaWalletChallengeLivenessV1::Live),
        Err(KagemushaWalletProviderErrorV1::Unavailable(_))
    ));
    drop(provider);
    device.power_loss(
        KagemushaWalletSimPowerLossV1::DropUnsynced,
        test_support::BOOT_B,
    );
    device.platform.with(|state| {
        state.generate_unavailable = None;
        state.generation_policy = KagemushaWalletKeyGenerationPolicyV1::DefinitiveAbsence;
    });
    let mut provider = device.open();
    // The upgraded OS now really reports Absent; the retained policy still forbids a grant.
    assert_eq!(
        provider.status(&slot),
        Ok(KagemushaWalletSlotStatusV1::IntentOnly)
    );
    assert_eq!(
        provider.resume_enrollment(&slot, KagemushaWalletChallengeLivenessV1::Live),
        Ok(KagemushaWalletEnrollmentStepV1::Pending { slot })
    );
    assert_eq!(device.platform.with(|state| state.generate_calls), 1);
    assert_eq!(
        device.platform.with(|state| state.fresh_generation_calls),
        1
    );
    assert_eq!(device.platform.with(|state| state.delete_calls), 0);
    assert_eq!(device.platform.key_of(&slot), None);
}

#[test]
fn wallet_advance_v1_enrollment_fresh_lost_readback_preserves_existing_key() {
    let device = fresh_device(0xa3);
    device
        .platform
        .with(|state| state.generate_unavailable = Some(true));
    let mut provider = device.open();
    assert!(matches!(
        provider.begin_enrollment(&enrollment_challenge(0xa3), PROFILE),
        Err(KagemushaWalletProviderErrorV1::Unavailable(_))
    ));
    let slot = provider.slots().expect("slots")[0];
    let key = device
        .platform
        .key_of(&slot)
        .expect("created key survives readback loss");
    drop(provider);
    device.power_loss(
        KagemushaWalletSimPowerLossV1::DropUnsynced,
        test_support::BOOT_B,
    );
    device
        .platform
        .with(|state| state.generate_unavailable = None);
    let mut provider = device.open();
    assert_eq!(
        provider.resume_enrollment(&slot, KagemushaWalletChallengeLivenessV1::Live),
        Ok(KagemushaWalletEnrollmentStepV1::SlotAbandoned { slot })
    );
    assert_eq!(device.platform.key_of(&slot), Some(key));
    assert_eq!(device.platform.with(|state| state.generate_calls), 1);
    assert_eq!(device.platform.with(|state| state.delete_calls), 0);
    assert!(device.platform.with(|state| state.violations.is_empty()));
}

#[test]
fn wallet_advance_v1_enrollment_fresh_attempt_crashes_never_recreate_authority() {
    let probe = fresh_device(0xa4);
    let mut provider = probe.open();
    let start = probe.fs.steps();
    provider
        .begin_enrollment(&enrollment_challenge(0xa4), PROFILE)
        .expect("probe");
    let steps = probe.fs.steps() - start;
    drop(provider);
    for step in 0..steps {
        for fault in [
            KagemushaWalletSimFaultV1::CrashBefore,
            KagemushaWalletSimFaultV1::CrashAfter,
        ] {
            let device = fresh_device(0xa4);
            let mut provider = device.open();
            device.fs.inject(device.fs.steps() + step, fault);
            let _ = provider.begin_enrollment(&enrollment_challenge(0xa4), PROFILE);
            let calls = device.platform.with(|state| state.generate_calls);
            assert!(calls <= 1, "step {step} {fault:?}");
            drop(provider);
            device.fs.clear_faults();
            device.power_loss(
                KagemushaWalletSimPowerLossV1::DropUnsynced,
                test_support::BOOT_B,
            );
            // Also exercise the stronger probe supplied by an OS upgrade after the crash.
            device.platform.with(|state| {
                state.generation_policy = KagemushaWalletKeyGenerationPolicyV1::DefinitiveAbsence
            });
            let mut provider = device.open();
            for slot in provider.slots().expect("slots") {
                let _ = provider.resume_enrollment(&slot, KagemushaWalletChallengeLivenessV1::Live);
            }
            assert_eq!(
                device.platform.with(|state| state.generate_calls),
                calls,
                "step {step} {fault:?}"
            );
            assert_eq!(device.platform.with(|state| state.delete_calls), 0);
            assert!(
                device.platform.with(|state| state.violations.is_empty()),
                "step {step} {fault:?}"
            );
        }
    }
    assert!(steps > 0);
}

#[test]
fn wallet_advance_v1_enrollment_fresh_cannot_reset_an_active_marker_or_missing_key() {
    let (device, _fixture, slot, _capsule) = bootstrapped_device(ANDROID, 0xa5);
    let mut provider = device.open();
    let before = provider.status(&slot).expect("active status");
    let key = device.platform.key_of(&slot).expect("active key");
    let calls = device.platform.with(|state| state.generate_calls);
    device.platform.with(|state| {
        state.generation_policy = KagemushaWalletKeyGenerationPolicyV1::FreshEnrollmentOnly
    });
    assert!(matches!(
        provider.resume_enrollment(&slot, KagemushaWalletChallengeLivenessV1::Live),
        Err(KagemushaWalletProviderErrorV1::Invalid {
            field: "enrollment.finished"
        })
    ));
    assert_eq!(provider.status(&slot), Ok(before));
    assert_eq!(device.platform.key_of(&slot), Some(key));
    drop(provider);
    device.platform.with(|state| {
        state.keys.remove(&slot);
    });
    let mut provider = device.open();
    assert!(matches!(
        provider.status(&slot),
        Err(KagemushaWalletProviderErrorV1::Unavailable(_))
    ));
    assert!(matches!(
        provider.resume_enrollment(&slot, KagemushaWalletChallengeLivenessV1::Live),
        Err(KagemushaWalletProviderErrorV1::Unavailable(_))
    ));
    assert_eq!(device.platform.with(|state| state.generate_calls), calls);
    assert_eq!(
        device.platform.with(|state| state.fresh_generation_calls),
        0
    );
    assert_eq!(device.platform.with(|state| state.delete_calls), 0);
}

#[test]
fn wallet_advance_v1_enrollment_fresh_invalid_record_or_old_intent_refuses() {
    let device = fresh_device(0xa6);
    device
        .platform
        .with(|state| state.generate_unavailable = Some(false));
    let mut provider = device.open();
    let _ = provider.begin_enrollment(&enrollment_challenge(0xa6), PROFILE);
    let slot = provider.slots().expect("slots")[0];
    let intent = provider
        .read_intent(&slot)
        .expect("intent")
        .expect("present");
    let dir = kagemusha_wallet_slot_dir_v1(&slot);
    let wrong = KagemushaWalletKeyGenerationAttemptV1 {
        version: KAGEMUSHA_WALLET_ENROLLMENT_FILE_VERSION_V1,
        slot: slot.0,
        challenge_digest: [0xff; 32],
        profile: PROFILE.tag(),
    };
    device.fs.place_unsynced(
        &dir,
        KAGEMUSHA_WALLET_KEY_GENERATION_ATTEMPT_NAME_V1,
        &encode_envelope_v1(&wrong, KAGEMUSHA_WALLET_INTENT_MAX_BYTES_V1).expect("encode"),
    );
    assert_eq!(
        provider.status(&slot),
        Err(KagemushaWalletProviderErrorV1::UnavailableCustodyData {
            object: "key_generation_attempt"
        })
    );
    // Exact former schema/layout; this fixture is never a production compatibility decoder.
    #[derive(norito::Encode, norito::Decode, norito::NoritoSchema)]
    #[norito_schema(name = "iroha_core::zk::kagemusha_wallet_advance_v1::IntentV1")]
    struct OldIntent {
        version: u16,
        slot: [u8; 32],
        challenge: KagemushaWalletEnrollmentChallengeV1,
        anchor_kind: u8,
        profile: u8,
    }
    let old = OldIntent {
        version: 1,
        slot: intent.slot,
        challenge: intent.challenge,
        anchor_kind: intent.anchor_kind,
        profile: intent.profile,
    };
    device.fs.place_unsynced(
        &dir,
        KAGEMUSHA_WALLET_INTENT_NAME_V1,
        &encode_envelope_v1(&old, KAGEMUSHA_WALLET_INTENT_MAX_BYTES_V1).expect("encode"),
    );
    assert_eq!(
        provider.read_intent(&slot),
        Err(KagemushaWalletProviderErrorV1::UnavailableCustodyData { object: "intent" })
    );
    assert_eq!(device.platform.with(|state| state.generate_calls), 1);
    assert_eq!(device.platform.with(|state| state.delete_calls), 0);
}

#[test]
fn wallet_advance_v1_enrollment_fresh_current_intent_abandonment_retains_recovery() {
    let device = fresh_device(0xa7);
    let mut provider = device.open();
    let slot = enrolled_slot(
        provider
            .begin_enrollment(&enrollment_challenge(0xa7), PROFILE)
            .expect("fresh begin"),
    );
    let key = device.platform.key_of(&slot).expect("key");
    let intent_bytes = device
        .fs
        .visible_file(
            &kagemusha_wallet_slot_dir_v1(&slot),
            KAGEMUSHA_WALLET_INTENT_NAME_V1,
        )
        .expect("intent");
    let frame = provider
        .abandon_enrollment(&slot)
        .expect("abandon current intent");
    drop(provider);
    device.power_loss(
        KagemushaWalletSimPowerLossV1::DropUnsynced,
        test_support::BOOT_B,
    );
    let mut provider = device.open();
    assert_eq!(provider.abandon_enrollment(&slot), Ok(frame));
    assert_eq!(device.platform.key_of(&slot), Some(key));
    assert_eq!(
        device.fs.visible_file(
            &kagemusha_wallet_slot_dir_v1(&slot),
            KAGEMUSHA_WALLET_INTENT_NAME_V1
        ),
        Some(intent_bytes)
    );
    assert_eq!(device.platform.with(|state| state.generate_calls), 1);
    assert_eq!(device.platform.with(|state| state.delete_calls), 0);
    assert!(device.platform.with(|state| state.violations.is_empty()));
}
