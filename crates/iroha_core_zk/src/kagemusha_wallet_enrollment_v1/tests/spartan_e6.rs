//! E6 verification shared by both credential profiles, and the installed Spartan profile.
//!
//! The shared verifier is exercised with the real account-authorized E5 and issuer-signed E6
//! of the engineering fixture: no verdict, proof or credential is synthesized. The Spartan
//! profile needs genuinely admitted parameters, so its end-to-end test installs the real
//! authenticated program bundle and is ignored without it.

use super::*;
use crate::kagemusha_spartan_native::{FutureRuntime, native};
use p256::ecdsa::{Signature as P256Signature, signature::Signer as _};
use std::sync::Arc;

/// A real enrolled E5, its real issuer-signed E6 and the fields that E6 authenticates.
fn enrolled_e6(
    seed: u8,
) -> (
    Fixture,
    RequestV1,
    ResultV1,
    KagemushaWalletCredentialBodyV1,
) {
    let f = fixture();
    let d = DeviceV1::new(KagemushaWalletAnchorPolicyV1::NotRequired, seed);
    let mut owner = enrolled(&f, &d);
    let requested = RequestV1::decode(&request(&f, &mut owner)).unwrap();
    let result = issuer_result(&f, &requested);
    let fields = result
        .verify_for(&f.config.scheme, &f.config.enrollment_certificate, &requested)
        .unwrap()
        .body;
    (f, requested, result, fields)
}

/// A second Enrollment-role certificate genuinely rooted in the same scheme.
fn foreign_enrollment_certificate(f: &Fixture) -> KagemushaWalletSignerCertificateV1 {
    let body = KagemushaWalletSignerCertificateBodyV1 {
        version: 1,
        scheme_id: f.config.scheme.scheme_id(),
        role: KagemushaWalletSignerRoleV1::Enrollment,
        key: public_key(&signing_key(83)),
        serial: 2,
    };
    let signature: P256Signature = signing_key(17).sign(&body.signing_message());
    KagemushaWalletSignerCertificateV1::sign(
        body,
        &f.config.scheme,
        KagemushaWalletSignerOutputV1::Raw(signature.to_bytes().into()),
    )
    .unwrap()
}

fn certificate_set(certificates: Vec<KagemushaWalletSignerCertificateV1>) -> Vec<u8> {
    norito::encode_canonical(&KagemushaWalletCertificateSetV1::new(certificates).unwrap()).unwrap()
}

#[test]
fn shared_verifier_accepts_the_real_e6_fields_and_refuses_every_wrong_field() {
    let (f, request, result, fields) = enrolled_e6(0xa1);
    let scheme = &f.config.scheme;
    let issuer = &f.config.enrollment_certificate;
    result
        .verify_initial_fields(scheme, issuer, &request, &fields)
        .unwrap();
    verify_initial_credential_fields_v1(
        &fields,
        scheme,
        issuer,
        &request.body.challenge,
        &request.body.marker.payment_key,
    )
    .unwrap();
    let other_key = public_key(&signing_key(0x5d));
    for field in 0..16 {
        let mut changed = fields;
        match field {
            0 => changed.version = 2,
            1 => changed.scheme_id[0] ^= 1,
            2 => changed.asset_digest[0] ^= 1,
            3 => changed.wallet_id[0] ^= 1,
            4 => changed.account_digest[0] ^= 1,
            5 => changed.payment_key = other_key,
            6 => changed.provider_contract[0] ^= 1,
            7 => changed.evidence_kind = KagemushaWalletEvidenceKindV1::AppleAppAttest,
            8 => {
                // Still initial evidence, but not the digest of the retained originals.
                changed.enrollment_evidence.digest[0] ^= 1;
                changed.fresh_evidence = changed.enrollment_evidence;
            }
            9 => changed.fresh_evidence.time_ms += 1,
            10 => changed.app_policy[0] ^= 1,
            11 => changed.regulatory_policy.time_anchor_max_response_ms += 1,
            12 => changed.enrollment_id[0] ^= 1,
            13 => changed.renewal_sequence = 1,
            14 => changed.lease_expires_at_ms += 1,
            _ => changed.issuer_certificate[0] ^= 1,
        }
        assert!(
            result
                .verify_initial_fields(scheme, issuer, &request, &changed)
                .is_err(),
            "field {field}"
        );
    }
    // A key that is internally consistent with the same challenge is still not the E5 key.
    let mut rebound = fields;
    rebound.payment_key = other_key;
    rebound.enrollment_id = request.body.challenge.enrollment_id(&other_key);
    rebound.wallet_id = request.body.challenge.wallet_id(&other_key);
    rebound.validate().unwrap();
    assert!(
        result
            .verify_initial_fields(scheme, issuer, &request, &rebound)
            .is_err()
    );
    // The same fields do bind that key when it is the one the caller names.
    verify_initial_credential_fields_v1(
        &rebound,
        scheme,
        issuer,
        &request.body.challenge,
        &other_key,
    )
    .unwrap();
}

#[test]
fn shared_verifier_refuses_substituted_request_evidence_and_issuer_originals() {
    let (f, request, result, fields) = enrolled_e6(0xa3);
    let scheme = &f.config.scheme;
    let issuer = &f.config.enrollment_certificate;
    // Another device's real E5 and E6 under the same issuer never bind this wallet's.
    let (_, other_request, other_result, other_fields) = enrolled_e6(0xa5);
    assert_ne!(
        request.body.marker.payment_key,
        other_request.body.marker.payment_key
    );
    other_result
        .verify_initial_fields(scheme, issuer, &other_request, &other_fields)
        .unwrap();
    assert!(
        result
            .verify_initial_fields(scheme, issuer, &other_request, &fields)
            .is_err()
    );
    assert!(
        result
            .verify_initial_fields(scheme, issuer, &request, &other_fields)
            .is_err()
    );
    // An E5 whose account signature changed is not the retained request.
    let mut unsigned = request.clone();
    unsigned.account_signature[0] ^= 1;
    assert!(
        result
            .verify_initial_fields(scheme, issuer, &unsigned, &fields)
            .is_err()
    );
    // Substituted issuer-retained evidence.
    let mut evidence = result.clone();
    if let IssuerEvidenceV1::Android { certificates } = &mut evidence.evidence {
        certificates[0][0] ^= 1;
    }
    assert!(
        evidence
            .verify_initial_fields(scheme, issuer, &request, &fields)
            .is_err()
    );
    // A rooted Enrollment certificate other than the selected one, as the delivered set or
    // as the selected issuer, is refused.
    let foreign = foreign_enrollment_certificate(&f);
    let only_foreign = ResultV1 {
        certificates: certificate_set(vec![foreign]),
        ..result.clone()
    };
    assert!(
        only_foreign
            .verify_initial_fields(scheme, issuer, &request, &fields)
            .is_err()
    );
    assert!(
        result
            .verify_initial_fields(scheme, &foreign, &request, &fields)
            .is_err()
    );
    // The complete set may carry other rooted certificates, but must hold the selected one.
    let complete = ResultV1 {
        certificates: certificate_set(vec![*issuer, foreign]),
        ..result.clone()
    };
    complete
        .verify_initial_fields(scheme, issuer, &request, &fields)
        .unwrap();
    // The same certificates out of canonical order are not a certificate set.
    let mut unordered = KagemushaWalletCertificateSetV1::new(vec![*issuer, foreign]).unwrap();
    unordered.certificates.reverse();
    let noncanonical = ResultV1 {
        certificates: norito::encode_canonical(&unordered).unwrap(),
        ..result.clone()
    };
    assert!(
        noncanonical
            .verify_initial_fields(scheme, issuer, &request, &fields)
            .is_err()
    );
}

#[test]
fn model_owner_never_accepts_an_opaque_spartan_sized_credential() {
    let f = fixture();
    let d = DeviceV1::new(KagemushaWalletAnchorPolicyV1::NotRequired, 0xa7);
    let mut owner = enrolled(&f, &d);
    let requested = RequestV1::decode(&request(&f, &mut owner)).unwrap();
    let real = issuer_result(&f, &requested);
    // The owner's bound profile selects the verifier. An exact-length opaque original is not
    // retried under another profile, and nothing of it is retained.
    let opaque = ResultV1 {
        credential: vec![7; SPARTAN_CREDENTIAL_BYTES_V1],
        ..real.clone()
    };
    assert!(owner.accept_credential(&opaque.encode().unwrap()).is_err());
    assert!(owner.retained_result().unwrap().is_none());
    let bytes = real.encode().unwrap();
    assert_eq!(owner.accept_credential(&bytes).unwrap(), bytes);
}

#[test]
fn retained_enrollment_permit_pins_the_manifest_before_bootstrap() {
    let f = fixture();
    let d = DeviceV1::new(KagemushaWalletAnchorPolicyV1::NotRequired, 0xb1);
    let provider = enrolled(&f, &d).into_provider();
    let slot = provider.slots().unwrap()[0];
    let intent = provider.read_intent(&slot).unwrap().unwrap();
    let check = |digest| provider.require_enrollment_intent_originals(&slot, &intent, Some(digest));
    let manifest = &f.config.installation.manifest_digest;
    let relation = &f.config.scheme.relation_id;
    assert_ne!(manifest, relation);
    assert_eq!(check(manifest), Ok(()));
    assert_eq!(
        check(relation),
        Err(KagemushaWalletProviderErrorV1::Invalid {
            field: "enrolled producer manifest",
        })
    );
}

#[test]
fn durable_e6_winner_readback_is_exact_pinned_and_never_reports_failure_as_absence() {
    let f = fixture();
    let d = DeviceV1::new(KagemushaWalletAnchorPolicyV1::NotRequired, 0xab);
    let mut owner = enrolled(&f, &d);
    let request_bytes = request(&f, &mut owner);
    let requested = RequestV1::decode(&request_bytes).unwrap();
    let bytes = issuer_result(&f, &requested).encode().unwrap();
    let key = kagemusha_wallet_enrollment_result_key_v1(&request_bytes);
    // Enrollment and account admission name the winner by the same key.
    assert_eq!(
        key,
        kagemusha_wallet_provider_digest_v1("enrollment-issuer-result", &request_bytes)
    );

    // With the E5 retained and no E6 accepted yet, the winner is genuinely absent.
    let mut provider = owner.into_provider();
    let slot = provider.slots().unwrap()[0];
    let marker = provider
        .status(&slot)
        .unwrap()
        .marker()
        .expect("enrollment marker")
        .clone();
    assert!(
        provider
            .enrollment_result_at_output_marker(&slot, &marker, &request_bytes)
            .unwrap()
            .is_none()
    );
    drop(provider);

    let mut owner = enrolled(&f, &d);
    assert_eq!(owner.accept_credential(&bytes).unwrap(), bytes);
    let mut provider = owner.into_provider();
    let marker = provider
        .status(&slot)
        .unwrap()
        .marker()
        .expect("enrollment marker")
        .clone();
    let read = |provider: &NativeProviderV1<KagemushaWalletSimFsV1, FakePlatformV1>| {
        provider.enrollment_result_at_output_marker(&slot, &marker, &request_bytes)
    };

    // The exact selected original, read without publishing, signing or generating anything.
    let (signed, generated) = d.platform.with(|s| (s.sign_calls, s.generate_calls));
    let before = d.fs.steps();
    assert_eq!(read(&provider).unwrap().as_deref(), Some(bytes.as_slice()));
    let clean = d.fs.trace_since(before);
    assert!(!clean.contains(&KagemushaWalletSimStepV1::RenameNoReplace));
    assert!(!clean.contains(&KagemushaWalletSimStepV1::SyncDir));
    d.platform
        .with(|s| assert_eq!((s.sign_calls, s.generate_calls), (signed, generated)));

    // Locked protected storage is unavailable, never an absent winner.
    d.platform
        .with(|s| s.storage = Err(KagemushaWalletUnavailableV1::BeforeFirstUnlock));
    assert!(matches!(
        read(&provider),
        Err(KagemushaWalletProviderErrorV1::Unavailable(_))
    ));
    d.platform.with(|s| s.storage = Ok(()));
    // A fault at any storage step of the read is an error, or the exact winner from the
    // surviving redundant copy. It is never absence and never other bytes.
    for step in 0..u64::try_from(clean.len()).unwrap() {
        d.fs.inject(d.fs.steps() + step, KagemushaWalletSimFaultV1::Error);
        match read(&provider) {
            Ok(Some(winner)) => assert_eq!(winner.as_slice(), bytes.as_slice()),
            Ok(None) => panic!("a storage failure at step {step} read as absence"),
            Err(_) => {}
        }
        d.fs.clear_faults();
    }
    assert_eq!(read(&provider).unwrap().as_deref(), Some(bytes.as_slice()));

    // Only this slot's own retained E5 names the record, and only under its current marker.
    assert!(matches!(
        provider.enrollment_result_at_output_marker(&slot, &marker, b"request"),
        Err(KagemushaWalletProviderErrorV1::Invalid { .. })
    ));
    let other = DeviceV1::new(KagemushaWalletAnchorPolicyV1::NotRequired, 0xad);
    let mut foreign = enrolled(&f, &other).into_provider();
    let foreign_slot = foreign.slots().unwrap()[0];
    let foreign_marker = foreign
        .status(&foreign_slot)
        .unwrap()
        .marker()
        .expect("enrollment marker")
        .clone();
    assert!(matches!(
        provider.enrollment_result_at_output_marker(&slot, &foreign_marker, &request_bytes),
        Err(KagemushaWalletProviderErrorV1::Invalid { .. })
    ));

    // A winner lost from both copies reads as absent while storage stays available. The
    // caller treats that as unavailable custody data, never as permission to admit.
    provider
        .with_archive(&slot, |archive| archive.remove_record(&key))
        .unwrap();
    assert!(read(&provider).unwrap().is_none());
}

#[test]
fn durable_e6_winner_stays_readable_under_a_retained_output_only_source() {
    use crate::kagemusha_wallet_advance_v1::{
        KagemushaWalletLostCustodyV1, OutputSourceV1,
        layout::{
            KagemushaWalletCopyV1, kagemusha_wallet_completion_dir_v1,
            kagemusha_wallet_completion_name_v1,
        },
        test_support::{
            TestOwnerV1, advance_request, bootstrapped_device, next_capsule, released,
        },
    };
    const WINNER: &[u8] = b"exact selected enrollment winner";

    // A funded wallet with two released heads. Its retained E5 is the fixture's opaque
    // `request` original, and its winner is published at the enrollment key.
    let (device, f, slot, bootstrap) =
        bootstrapped_device(KagemushaWalletAnchorPolicyV1::Keychain, 0x4c);
    let mut provider = device.open();
    let current = provider.status(&slot).expect("status");
    let superseded = current.marker().expect("marker").clone();
    let next = next_capsule(&f, &bootstrap);
    released(
        provider
            .advance(&slot, &TestOwnerV1, &advance_request(&superseded, &next))
            .expect("advance"),
    );
    let key = kagemusha_wallet_enrollment_result_key_v1(b"request");
    provider
        .with_archive(&slot, |archive| archive.write_record(&key, WINNER))
        .unwrap();
    provider
        .publish_archive_checkpoint(&slot, [0; 32], b"protected test checkpoint")
        .unwrap();

    // Lose both copies of the current head's completion.
    let directory = kagemusha_wallet_completion_dir_v1(&slot);
    for copy in KagemushaWalletCopyV1::BOTH {
        let name = kagemusha_wallet_completion_name_v1(&next.body().operation_id, copy);
        device
            .fs
            .place_unsynced(&directory, name.as_str(), b"unreadable completion frame");
    }
    drop(provider);
    let mut provider = device.open();
    let OutputSourceV1::ReleasedWithoutCompletion(source) = provider.output_source(&slot).unwrap()
    else {
        panic!("completion loss cannot be an ordinary account source");
    };

    // The pinned read answers under the restricted source's own marker and changes nothing.
    let signed = device.platform.with(|state| state.sign_calls);
    let before = device.fs.steps();
    assert_eq!(
        provider
            .enrollment_result_at_output_marker(&slot, &source, b"request")
            .unwrap()
            .as_deref(),
        Some(WINNER)
    );
    let trace = device.fs.trace_since(before);
    assert!(!trace.contains(&KagemushaWalletSimStepV1::RenameNoReplace));
    assert!(!trace.contains(&KagemushaWalletSimStepV1::SyncDir));
    device
        .platform
        .with(|state| assert_eq!(state.sign_calls, signed));
    // A superseded marker or another request is not this source.
    assert!(
        provider
            .enrollment_result_at_output_marker(&slot, &superseded, b"request")
            .is_err()
    );
    assert!(
        provider
            .enrollment_result_at_output_marker(&slot, &source, b"another request")
            .is_err()
    );
    // The reconciling archive entry cannot serve this source at all, and neither that
    // refusal nor the pinned reads moved the restricted source.
    assert!(matches!(
        provider.with_archive(&slot, |archive| archive.read_record(&key, WINNER.len())),
        Err(KagemushaWalletProviderErrorV1::LostCustody(
            KagemushaWalletLostCustodyV1::CompletionLost
        ))
    ));
    assert!(matches!(
        provider.output_source(&slot).unwrap(),
        OutputSourceV1::ReleasedWithoutCompletion(after)
            if after.marker_file_digest() == source.marker_file_digest()
    ));
    assert_eq!(
        provider
            .enrollment_result_at_output_marker(&slot, &source, b"request")
            .unwrap()
            .as_deref(),
        Some(WINNER)
    );
}

/// P-256 group order `n`, big-endian.
const P256_ORDER: [u8; 32] = [
    0xff, 0xff, 0xff, 0xff, 0x00, 0x00, 0x00, 0x00, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
    0xbc, 0xe6, 0xfa, 0xad, 0xa7, 0x17, 0x9e, 0x84, 0xf3, 0xb9, 0xca, 0xc2, 0xfc, 0x63, 0x25, 0x51,
];
/// `floor(n / 2)`, big-endian: the largest low-S scalar.
const P256_HALF_ORDER: [u8; 32] = [
    0x7f, 0xff, 0xff, 0xff, 0x80, 0x00, 0x00, 0x00, 0x7f, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
    0xde, 0x73, 0x7d, 0x56, 0xd3, 0x8b, 0xcf, 0x42, 0x79, 0xdc, 0xe5, 0x61, 0x7e, 0x31, 0x92, 0xa8,
];

/// Replace a high `s` with `n - s`: the Spartan original carries a raw low-S signature.
fn low_s(mut raw: [u8; 64]) -> [u8; 64] {
    if raw[32..] > P256_HALF_ORDER[..] {
        let mut borrow = 0_u16;
        for index in (0..32).rev() {
            let subtrahend = u16::from(raw[32 + index]) + borrow;
            let minuend = u16::from(P256_ORDER[index]);
            let (difference, next) = if minuend >= subtrahend {
                (minuend - subtrahend, 0)
            } else {
                (minuend + 256 - subtrahend, 1)
            };
            raw[32 + index] = u8::try_from(difference).unwrap();
            borrow = next;
        }
    }
    raw
}

#[test]
fn low_s_issuance_helper_negates_only_a_high_scalar() {
    let mut raw = [0; 64];
    raw[32..].copy_from_slice(&P256_HALF_ORDER);
    assert_eq!(low_s(raw), raw);
    // n = 2 * floor(n / 2) + 1, so floor(n / 2) + 1 negates to floor(n / 2).
    let mut above = P256_HALF_ORDER;
    above[31] += 1;
    raw[32..].copy_from_slice(&above);
    assert_eq!(low_s(raw)[32..], P256_HALF_ORDER);
}

/// The isolated public scheme the authenticated native-device bundle was signed under:
/// engineering root `signing_key(0x11)` on the deterministic fixture genesis.
fn bundle_scheme() -> KagemushaWalletSchemeV1 {
    use iroha_data_model::sumeragi_finality::test_fixtures::NativeFinalityFixture;
    let finality = NativeFinalityFixture::start_with_explicit_parameters("portable-native-fixture");
    KagemushaWalletSchemeV1 {
        version: 1,
        network_id: *finality.network_id().as_bytes(),
        scheme_root_key: public_key(&signing_key(0x11)),
        relation_id: [0x33; 32],
        provider_contract: kagemusha_wallet_provider_contract_v1(),
    }
}

/// Genuinely install the complete real program bundle; nothing here is a test double.
fn bundle_parameters(
    scheme: &KagemushaWalletSchemeV1,
    reissue_artifact_certificate: bool,
) -> native::AuthenticatedParameters {
    use iroha_data_model::sumeragi_finality::{
        SumeragiCommitVerifierV1, test_fixtures::NativeFinalityFixture,
    };
    let directory = std::env::var_os("KAGEMUSHA_SPARTAN_DEVICE_ARTIFACTS").map_or_else(
        || {
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../experiments/kagemusha-spartan/output/full-backend-chain/native-device")
        },
        std::path::PathBuf::from,
    );
    let read = |name: &str| std::fs::read(directory.join(name)).unwrap();
    let finality = SumeragiCommitVerifierV1::new(
        &NativeFinalityFixture::new_with_explicit_parameters().verifier(),
    )
    .unwrap();
    let mut manifest = read("parameter-manifest.norito");
    let mut signature: [u8; 64] = read("parameter-signature.bin").try_into().unwrap();
    if reissue_artifact_certificate {
        // Reissue the Artifact-role certificate under the same root while preserving
        // the producer's key, programs and relation.
        let mut decoded: native::ParameterManifest = norito::decode_from_bytes(&manifest).unwrap();
        let artifact = signing_key(0x15);
        let body = KagemushaWalletSignerCertificateBodyV1 {
            version: 1,
            scheme_id: scheme.scheme_id(),
            role: KagemushaWalletSignerRoleV1::Artifact,
            key: public_key(&artifact),
            serial: 72,
        };
        let signed: P256Signature = signing_key(0x11).sign(&body.signing_message());
        decoded.signer_certificate = body.transcript();
        decoded
            .signer_certificate
            .extend_from_slice(&low_s(signed.to_bytes().into()));
        manifest = {
            let _flags = norito::core::DecodeFlagsGuard::enter(norito::core::V1_LAYOUT_FLAGS);
            norito::to_bytes(&decoded).unwrap()
        };
        let digest =
            kagemusha_wallet_poseidon_bytes_v1(u64::from_le_bytes(*b"kgwparm1"), &manifest);
        let signed: P256Signature = artifact.sign(&digest);
        signature = low_s(signed.to_bytes().into());
    }
    native::AuthenticatedParameters::install(
        &read("program-bundle.norito"),
        &manifest,
        signature,
        scheme,
        finality,
    )
    .unwrap()
}

/// The component fixture moved to the bundle's scheme and its exact Spartan generation.
fn spartan_fixture(
    parameters: &native::AuthenticatedParameters,
    scheme: KagemushaWalletSchemeV1,
) -> Fixture {
    let mut f = fixture();
    let scheme_id = scheme.scheme_id();
    f.config.enrollment_certificate = enrollment_certificate(&scheme);
    f.config.scheme = scheme;
    f.config.installation = crate::kagemusha_wallet_artifacts_v1::InstallationV1 {
        scheme_id,
        manifest_digest: *parameters.manifest_digest(),
    };
    f.config.app.scheme_id = scheme_id;
    f.config.policy.scheme_id = scheme_id;
    f.config.policy.app_policy = f.config.app.policy_digest().unwrap();
    f.challenge.scheme_id = scheme_id;
    f.challenge.app_policy = f.config.app.policy_digest().unwrap();
    f.challenge.enrollment_policy = f.config.policy.policy_digest().unwrap();
    f
}

/// The issuer's real Spartan E6 for `request`: the exact 532-byte body of the admitted
/// generation, signed low-S by the fixture Enrollment key, with the canonical certificate
/// set. `change` alters the fields before the issuer signs them.
fn spartan_result(
    f: &Fixture,
    parameters: &native::AuthenticatedParameters,
    request: &RequestV1,
    change: impl FnOnce(&mut KagemushaWalletCredentialBodyV1),
) -> ResultV1 {
    let evidence = IssuerEvidenceV1::Android {
        certificates: vec![b"original leaf".to_vec(), b"original CA".to_vec()],
    };
    let kind = KagemushaWalletEvidenceKindV1::AndroidKeyMintTee;
    let event = KagemushaWalletEvidenceV1 {
        digest: evidence.digest(&request.body, kind).unwrap(),
        time_ms: 1000,
        facts: KAGEMUSHA_WALLET_ANDROID_REQUIRED_FACTS_V1,
        os_patch_level: 202_608,
        vendor_patch_level: 202_608,
        boot_patch_level: 202_608,
    };
    let key = request.body.marker.payment_key;
    let mut fields = KagemushaWalletCredentialBodyV1 {
        version: 1,
        scheme_id: f.config.scheme.scheme_id(),
        asset_digest: f.asset.asset_digest(),
        wallet_id: request.body.marker.wallet_id,
        account_digest: f.challenge.account_digest,
        payment_key: key,
        provider_contract: f.config.scheme.provider_contract,
        evidence_kind: kind,
        enrollment_evidence: event,
        fresh_evidence: event,
        app_policy: f.challenge.app_policy,
        regulatory_policy: f.config.policy.regulatory_policy,
        enrollment_id: f.challenge.enrollment_id(&key),
        issued_at_ms: 2000,
        renewal_sequence: 0,
        lease_expires_at_ms: 0,
        issuer_certificate: f.config.enrollment_certificate.certificate_digest(),
    };
    change(&mut fields);
    let body = parameters.credential_body_for_issuance(&fields).unwrap();
    let signature: P256Signature =
        signing_key(79).sign(&native::credential_signing_message_for_body(&body));
    let mut raw = [0; 64];
    raw.copy_from_slice(&signature.to_bytes());
    let mut credential = body.to_vec();
    credential.extend_from_slice(&low_s(raw));
    ResultV1 {
        version: 1,
        credential,
        certificates: certificate_set(vec![f.config.enrollment_certificate]),
        evidence,
    }
}

#[test]
#[ignore = "requires the authenticated full-backend-chain/native-device program bundle"]
fn installed_spartan_profile_admits_only_the_exact_real_e6_and_replays_its_account_challenge() {
    let scheme = bundle_scheme();
    let parameters = Arc::new(bundle_parameters(&scheme, false));
    let f = spartan_fixture(&parameters, scheme);
    let d = DeviceV1::new(KagemushaWalletAnchorPolicyV1::NotRequired, 0xa9);
    let open = || {
        NativeProviderV1::open(
            d.fs.clone(),
            d.platform.clone(),
            f.config.scheme.scheme_id(),
            test_options(),
        )
        .unwrap()
    };
    // A profile of another generation cannot be bound to this installation.
    let mut foreign_generation = f.config.clone();
    foreign_generation.installation.manifest_digest[0] ^= 1;
    let Err((provider, _)) = EnrollmentOwnerV1::new_spartan(
        open(),
        foreign_generation,
        SpartanCredentialProfileV1::new(Arc::clone(&parameters)),
    ) else {
        panic!("foreign Spartan generation bound")
    };
    drop(provider);
    let mut owner: Owner = EnrollmentOwnerV1::new_spartan(
        open(),
        f.config.clone(),
        SpartanCredentialProfileV1::new(Arc::clone(&parameters)),
    )
    .unwrap_or_else(|(_, error)| panic!("{error}"));
    let message = begin(&f, &mut owner);
    assert!(matches!(
        owner.authorize(&sign(&f, &message)).unwrap(),
        EnrollmentProgressV1::Evidence { .. }
    ));
    let requested = RequestV1::decode(&request(&f, &mut owner)).unwrap();
    let challenge = requested.body.challenge;
    let real = spartan_result(&f, &parameters, &requested, |_| {});
    assert_eq!(real.credential.len(), SPARTAN_CREDENTIAL_BYTES_V1);
    let bytes = real.encode().unwrap();

    // Wrong fields, each genuinely signed by the selected issuer.
    let other_key = public_key(&signing_key(0x5d));
    for wrong in 0..4 {
        let altered = spartan_result(&f, &parameters, &requested, |c| match wrong {
            0 => c.account_digest[0] ^= 1,
            1 => c.app_policy[0] ^= 1,
            // Issued outside the retained attempt's dates.
            2 => c.issued_at_ms = 500_000,
            // A credential of another payment key, consistent with the same challenge.
            _ => {
                c.payment_key = other_key;
                c.enrollment_id = challenge.enrollment_id(&other_key);
                c.wallet_id = challenge.wallet_id(&other_key);
            }
        });
        assert!(
            owner.accept_credential(&altered.encode().unwrap()).is_err(),
            "wrong field {wrong}"
        );
    }
    // Changed signed bytes, a changed signature, and the Model frame of the same fields:
    // the Spartan profile never falls back to another decoder.
    for offset in [200, 300, 595] {
        let mut altered = real.clone();
        altered.credential[offset] ^= 1;
        assert!(owner.accept_credential(&altered.encode().unwrap()).is_err());
    }
    let model = issuer_result(&f, &requested);
    assert!(owner.accept_credential(&model.encode().unwrap()).is_err());
    // Substituted evidence and a certificate set without the selected issuer.
    let mut evidence = real.clone();
    if let IssuerEvidenceV1::Android { certificates } = &mut evidence.evidence {
        certificates[0][0] ^= 1;
    }
    assert!(owner.accept_credential(&evidence.encode().unwrap()).is_err());
    let foreign = foreign_enrollment_certificate(&f);
    let only_foreign = ResultV1 {
        certificates: certificate_set(vec![foreign]),
        ..real.clone()
    };
    assert!(
        owner
            .accept_credential(&only_foreign.encode().unwrap())
            .is_err()
    );
    assert!(owner.retained_result().unwrap().is_none());

    // The exact real E6 is retained once and wins every retry.
    assert_eq!(owner.accept_credential(&bytes).unwrap(), bytes);
    assert_eq!(owner.accept_credential(b"wrong retry").unwrap(), bytes);
    assert_eq!(owner.retained_result().unwrap(), Some(bytes.clone()));
    let originals = owner.open_originals().unwrap();
    assert_eq!(originals[0], real.credential);
    assert_eq!(originals[1], real.certificates);

    // Account admission from that complete E6 on the same custody and parameters.
    let account = norito::encode_canonical(&f.account).unwrap();
    let asset = norito::encode_canonical(&f.asset).unwrap();
    let issuer = f.config.enrollment_certificate;
    drop(owner);
    {
        let reissued = bundle_parameters(&f.config.scheme, true);
        assert_eq!(reissued.id(), parameters.id());
        assert_ne!(reissued.manifest_digest(), parameters.manifest_digest());
        let mut runtime = FutureRuntime::from_installed(open(), reissued, f.config.scheme, ());
        let mismatch = Err(KagemushaWalletProviderErrorV1::Invalid {
            field: "enrolled producer manifest",
        });
        assert_eq!(
            runtime.begin_account_result(&bytes, &account, &asset, &issuer),
            mismatch,
        );
        assert_eq!(
            runtime.begin_account(
                &real.credential,
                &spartan_signer_certificate_original_v1(&issuer),
                &account,
                &asset,
            ),
            mismatch,
        );
    }
    let mut runtime = FutureRuntime::from_installed(
        open(),
        Arc::clone(&parameters),
        f.config.scheme,
        (),
    );
    // Substitutions are refused and retain nothing.
    assert!(
        runtime
            .begin_account_result(&evidence.encode().unwrap(), &account, &asset, &issuer)
            .is_err()
    );
    assert!(
        runtime
            .begin_account_result(&only_foreign.encode().unwrap(), &account, &asset, &issuer)
            .is_err()
    );
    assert!(
        runtime
            .begin_account_result(&bytes, &account, &asset, &foreign)
            .is_err()
    );
    // Another genuinely signed credential of the same wallet is not the retained one.
    let reissued = spartan_result(&f, &parameters, &requested, |c| c.issued_at_ms = 2001);
    assert!(
        runtime
            .begin_account_result(&reissued.encode().unwrap(), &account, &asset, &issuer)
            .is_err()
    );
    let other_account = norito::encode_canonical(&AccountId::new(
        KeyPair::from_seed(vec![43; 32], Algorithm::Ed25519)
            .public_key()
            .clone(),
    ))
    .unwrap();
    assert!(
        runtime
            .begin_account_result(&bytes, &other_account, &asset, &issuer)
            .is_err()
    );
    let mut other_asset = asset.clone();
    *other_asset.last_mut().unwrap() ^= 1;
    assert!(
        runtime
            .begin_account_result(&bytes, &account, &other_asset, &issuer)
            .is_err()
    );
    // Pre-admission regression: the same credential and evidence with a larger certificate
    // set still holding the selected issuer verifies as an E6, but it is not the exact
    // durable winner enrollment retained. It is refused before any challenge exists and
    // retains nothing, so the exact original below still begins a fresh admission.
    let superset = ResultV1 {
        certificates: certificate_set(vec![issuer, foreign]),
        ..real.clone()
    };
    assert!(matches!(
        runtime.begin_account_result(&superset.encode().unwrap(), &account, &asset, &issuer),
        Err(KagemushaWalletProviderErrorV1::Invalid { .. })
    ));
    // The exact E6 yields a fresh challenge, and exact replay returns the retained one.
    let fresh = runtime
        .begin_account_result(&bytes, &account, &asset, &issuer)
        .unwrap();
    assert_ne!(fresh, [0; 32]);
    for _ in 0..2 {
        assert_eq!(
            runtime
                .begin_account_result(&bytes, &account, &asset, &issuer)
                .unwrap(),
            fresh
        );
    }
    // A different E6 original, another account, or the split form of the same originals is
    // not a replay of this admission; each refusal keeps the retained challenge.
    assert!(
        runtime
            .begin_account_result(&superset.encode().unwrap(), &account, &asset, &issuer)
            .is_err()
    );
    assert!(
        runtime
            .begin_account_result(&bytes, &other_account, &asset, &issuer)
            .is_err()
    );
    assert!(
        runtime
            .begin_account(
                &real.credential,
                &spartan_signer_certificate_original_v1(&issuer),
                &account,
                &asset,
            )
            .is_err()
    );
    assert_eq!(
        runtime
            .begin_account_result(&bytes, &account, &asset, &issuer)
            .unwrap(),
        fresh
    );
    // The same existing account then authorizes the same retained challenge.
    assert!(runtime.authorize_account(&[0; 64]).is_err());
    runtime.authorize_account(&sign(&f, &fresh)).unwrap();
    d.platform.with(|state| assert_eq!(state.generate_calls, 1));
}
