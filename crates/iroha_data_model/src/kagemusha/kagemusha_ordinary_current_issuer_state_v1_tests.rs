//! Genuine signatures over complete synthetic enrollment originals; no physical
//! attestation, installed native owner, durable Core commit or financial authority.
use super::*;
use crate::{
    kagemusha::*, testing::ordinary_app_enrollment::KagemushaOrdinaryRetailEnrollmentFixtureV1,
};
use iroha_crypto::{Algorithm, KeyPair};

// Fixture installation input, kept independent of any issuer body or returned reply.
const INSTALLED_NAMESPACE: [u8; 32] = [83; 32];

fn historical(
    f: &KagemushaOrdinaryRetailEnrollmentFixtureV1,
) -> KagemushaVerifiedHistoricalOrdinaryEnrollmentV1 {
    let c = &f.selection.preparation.challenge;
    let credential = &f.selection.issuance.credential;
    let subject = &credential.subject;
    let platform_digest = Sha256::digest(&f.proof.raw_attestation).into();
    let raw_subject = KagemushaRawAppAttestationAdmissionSubjectV1 {
        version: 1,
        enrollment_challenge_digest: c.attestation_challenge().unwrap(),
        authority_policy_digest: f.app_authority.canonical_digest().unwrap(),
        platform_class: subject.platform_class,
        security_level: subject.security_level,
        app_public_key: subject.app_public_key,
        attested_key_id: subject.attested_key_id,
        raw_platform_evidence_digest: platform_digest,
        app_signing_identity_digest: f.app_authority.app_signing_identity_digest,
        original_app_attest_counter: 0,
        issued_at_ms: c.issued_at_ms,
        expires_at_ms: c.expires_at_ms,
    };
    let app_issuer = KeyPair::from_seed(vec![61; 32], Algorithm::Ed25519);
    let raw = KagemushaRawAppAttestationAdmissionV1 {
        signature: Signature::try_new(
            app_issuer.private_key(),
            &raw_subject.canonical_signing_bytes().unwrap(),
        )
        .unwrap(),
        subject: raw_subject,
    }
    .to_transport_bytes()
    .unwrap();
    let possession = KagemushaAppEnrollmentPossessionV1 {
        challenge: KagemushaAppEnrollmentPossessionChallengeV1::from_original_enrollment(
            c,
            &subject.app_public_key,
            platform_digest,
        )
        .unwrap(),
        evidence: f.proof.app_possession.clone(),
    };
    f.ordinary_policy
        .identity_policy()
        .authenticate_historical_enrollment_originals(
            &f.selection.preparation.to_transport_bytes().unwrap(),
            c,
            &raw,
            &f.proof.raw_attestation,
            &norito::encode_canonical(&possession).unwrap(),
            &credential.canonical_bytes().unwrap(),
            &subject.app_public_key,
            300,
        )
        .unwrap()
}

fn signed(
    subject: KagemushaOrdinaryCurrentIssuerStateSubjectV1,
    seed: u8,
) -> KagemushaSignedOrdinaryCurrentIssuerStateV1 {
    let core = KeyPair::from_seed(vec![seed; 32], Algorithm::Ed25519);
    KagemushaSignedOrdinaryCurrentIssuerStateV1 {
        signature: Signature::try_new(
            core.private_key(),
            &subject.canonical_signing_bytes().unwrap(),
        )
        .unwrap(),
        subject,
    }
}

fn current_subject(
    f: &KagemushaOrdinaryRetailEnrollmentFixtureV1,
    history: &KagemushaVerifiedHistoricalOrdinaryEnrollmentV1,
    state: KagemushaOrdinaryEnrollmentStateV1,
) -> KagemushaOrdinaryCurrentIssuerStateSubjectV1 {
    KagemushaOrdinaryCurrentIssuerStateSubjectV1::for_committed_enrollment(
        f.ordinary_policy.identity_policy(),
        f.ordinary_policy.issuer_policy(),
        INSTALLED_NAMESPACE,
        history,
        [90; 32],
        state,
        7,
        if state == KagemushaOrdinaryEnrollmentStateV1::Rotated {
            [99; 32]
        } else {
            [0; 32]
        },
        300,
        400,
    )
    .unwrap()
}

fn roundtrip<T>(value: &T)
where
    T: core::fmt::Debug
        + PartialEq
        + norito::NoritoSerialize
        + norito::json::JsonSerialize
        + norito::json::JsonDeserialize,
    for<'de> T: norito::NoritoDeserialize<'de>,
{
    let original = norito::encode_canonical(value).unwrap();
    let decoded: T = norito::decode_canonical_with_limits(
        &original,
        norito::canonical_decode_limits(original.len()),
    )
    .unwrap();
    assert_eq!(&decoded, value);
    let json = norito::json::to_json(value).unwrap();
    let json_decoded: T = norito::json::from_str(&json).unwrap();
    assert_eq!(&json_decoded, value);
    assert_eq!(norito::encode_canonical(&json_decoded).unwrap(), original);
    assert!(norito::json::to_json_bounded(value, json.len() - 1).is_err());
}

#[test]
fn current_state_all_public_types_roundtrip_and_lifecycle_never_grants_native_authority() {
    for apple in [false, true] {
        let f = KagemushaOrdinaryRetailEnrollmentFixtureV1::new(apple);
        let history = historical(&f);
        let policy = f.ordinary_policy.identity_policy();
        for state in [
            KagemushaOrdinaryEnrollmentStateV1::Active,
            KagemushaOrdinaryEnrollmentStateV1::Rotated,
            KagemushaOrdinaryEnrollmentStateV1::Retired,
        ] {
            let subject = current_subject(&f, &history, state);
            roundtrip(&state);
            roundtrip(&subject);
            let reply = signed(subject, 63);
            roundtrip(&reply);
            let original = reply.canonical_bytes().unwrap();
            let decoded =
                KagemushaSignedOrdinaryCurrentIssuerStateV1::decode_canonical_exact(&original)
                    .unwrap();
            assert_eq!(decoded, reply);
            let checked = decoded
                .authenticate(
                    policy,
                    f.ordinary_policy.issuer_policy(),
                    INSTALLED_NAMESPACE,
                    &history,
                    &[90; 32],
                    7,
                    300,
                )
                .unwrap();
            assert_eq!(checked.original(), original);
            assert_eq!(checked.subject(), &subject);
            let active = checked.require_active(
                policy,
                f.ordinary_policy.issuer_policy(),
                INSTALLED_NAMESPACE,
                &history,
                &[90; 32],
                7,
                350,
            );
            assert_eq!(
                active.is_ok(),
                state == KagemushaOrdinaryEnrollmentStateV1::Active
            );
            assert!(
                checked
                    .require_active(
                        policy,
                        f.ordinary_policy.issuer_policy(),
                        INSTALLED_NAMESPACE,
                        &history,
                        &[90; 32],
                        7,
                        299
                    )
                    .is_err()
            );
            assert!(
                checked
                    .require_active(
                        policy,
                        f.ordinary_policy.issuer_policy(),
                        INSTALLED_NAMESPACE,
                        &history,
                        &[90; 32],
                        7,
                        400
                    )
                    .is_err()
            );
        }
    }
}

#[test]
fn current_state_genuine_core_signatures_cannot_substitute_original_nonce_epoch_or_signer() {
    for apple in [false, true] {
        let f = KagemushaOrdinaryRetailEnrollmentFixtureV1::new(apple);
        let history = historical(&f);
        let policy = f.ordinary_policy.identity_policy();
        let subject = current_subject(&f, &history, KagemushaOrdinaryEnrollmentStateV1::Active);
        let original = signed(subject, 63);
        original
            .authenticate(
                policy,
                f.ordinary_policy.issuer_policy(),
                INSTALLED_NAMESPACE,
                &history,
                &[90; 32],
                7,
                300,
            )
            .unwrap();
        for field in 0..16 {
            let mut changed = subject;
            match field {
                0 => changed.query_nonce[0] ^= 1,
                1 => changed.enrollment_id[0] ^= 1,
                2 => changed.account_binding[0] ^= 1,
                3 => changed.network_id[0] ^= 1,
                4 => changed.lane_id[0] ^= 1,
                5 => changed.identity_policy_id[0] ^= 1,
                6 => changed.policy_original_digest[0] ^= 1,
                7 => changed.authority_original_digest[0] ^= 1,
                8 => changed.preparation_original_digest[0] ^= 1,
                9 => changed.raw_admission_original_digest[0] ^= 1,
                10 => changed.platform_original_digest[0] ^= 1,
                11 => changed.possession_original_digest[0] ^= 1,
                12 => changed.credential_original_digest[0] ^= 1,
                13 => changed.attested_key_id[0] ^= 1,
                14 => changed.app_key_reference[0] ^= 1,
                _ => changed.financial_authority_commitment[0] ^= 1,
            }
            let changed = signed(changed, 63);
            changed
                .signature
                .verify(
                    &policy.policy().enrollment_issuer_key,
                    &changed.subject.canonical_signing_bytes().unwrap(),
                )
                .unwrap();
            assert_eq!(
                changed
                    .authenticate(
                        policy,
                        f.ordinary_policy.issuer_policy(),
                        INSTALLED_NAMESPACE,
                        &history,
                        &[90; 32],
                        7,
                        300
                    )
                    .err()
                    .unwrap(),
                "current issuer state native nonce/epoch/original join differs"
            );
        }
        assert_eq!(
            original
                .authenticate(
                    policy,
                    f.ordinary_policy.issuer_policy(),
                    INSTALLED_NAMESPACE,
                    &history,
                    &[90; 32],
                    8,
                    300
                )
                .err()
                .unwrap(),
            "current issuer state native nonce/epoch/original join differs"
        );
        assert_eq!(
            signed(subject, 64)
                .authenticate(
                    policy,
                    f.ordinary_policy.issuer_policy(),
                    INSTALLED_NAMESPACE,
                    &history,
                    &[90; 32],
                    7,
                    300
                )
                .err()
                .unwrap(),
            "current issuer state Core signature rejected"
        );
    }
}

#[test]
fn current_state_exact_archive_bounds_original_interval_and_successor_rules_survive_decode() {
    let f = KagemushaOrdinaryRetailEnrollmentFixtureV1::new(false);
    let history = historical(&f);
    let policy = f.ordinary_policy.identity_policy();
    let subject = current_subject(&f, &history, KagemushaOrdinaryEnrollmentStateV1::Active);
    let reply = signed(subject, 63);
    let original = reply.canonical_bytes().unwrap();
    let mut tail = original.clone();
    tail.push(0);
    for bytes in [
        vec![],
        original[..original.len() - 1].to_vec(),
        tail,
        vec![0; KAGEMUSHA_ORDINARY_CURRENT_ISSUER_STATE_MAX_BYTES_V1 + 1],
    ] {
        assert!(
            KagemushaSignedOrdinaryCurrentIssuerStateV1::decode_canonical_exact(&bytes).is_err()
        );
    }
    assert_eq!(
        reply
            .authenticate(
                policy,
                f.ordinary_policy.issuer_policy(),
                INSTALLED_NAMESPACE,
                &history,
                &[90; 32],
                7,
                400
            )
            .err()
            .unwrap(),
        "current issuer state original interval differs"
    );
    for state in [
        KagemushaOrdinaryEnrollmentStateV1::Active,
        KagemushaOrdinaryEnrollmentStateV1::Retired,
    ] {
        let mut changed = subject;
        changed.state = state;
        changed.successor_enrollment_id = [99; 32];
        assert!(changed.canonical_signing_bytes().is_err());
    }
    for successor in [[0; 32], subject.enrollment_id] {
        let mut changed = subject;
        changed.state = KagemushaOrdinaryEnrollmentStateV1::Rotated;
        changed.successor_enrollment_id = successor;
        assert!(changed.canonical_signing_bytes().is_err());
    }
    assert!(
        KagemushaOrdinaryCurrentIssuerStateSubjectV1::for_committed_enrollment(
            policy,
            f.ordinary_policy.issuer_policy(),
            INSTALLED_NAMESPACE,
            &history,
            [90; 32],
            KagemushaOrdinaryEnrollmentStateV1::Active,
            7,
            [0; 32],
            300,
            history.credential().subject().expires_at_ms + 1,
        )
        .is_err()
    );
}

#[test]
fn current_state_signed_issuer_cap_1000_is_inclusive_and_1001_never_admits() {
    for apple in [false, true] {
        let f = KagemushaOrdinaryRetailEnrollmentFixtureV1::new(apple);
        let history = historical(&f);
        let policy = f.ordinary_policy.identity_policy();
        let issuer = f.ordinary_policy.issuer_policy();
        assert_eq!(issuer.policy().maximum_current_state_lifetime_ms, 1000);
        let exact = KagemushaOrdinaryCurrentIssuerStateSubjectV1::for_committed_enrollment(
            policy,
            issuer,
            INSTALLED_NAMESPACE,
            &history,
            [90; 32],
            KagemushaOrdinaryEnrollmentStateV1::Active,
            7,
            [0; 32],
            300,
            1300,
        )
        .unwrap();
        assert!(
            KagemushaOrdinaryCurrentIssuerStateSubjectV1::for_committed_enrollment(
                policy,
                issuer,
                INSTALLED_NAMESPACE,
                &history,
                [90; 32],
                KagemushaOrdinaryEnrollmentStateV1::Active,
                7,
                [0; 32],
                300,
                1301,
            )
            .is_err()
        );

        let exact_reply = signed(exact, 63);
        let mut checked = exact_reply
            .authenticate(
                policy,
                issuer,
                INSTALLED_NAMESPACE,
                &history,
                &[90; 32],
                7,
                350,
            )
            .unwrap();
        checked
            .require_active(
                policy,
                issuer,
                INSTALLED_NAMESPACE,
                &history,
                &[90; 32],
                7,
                350,
            )
            .unwrap(); // Model DATA only; no native/current account or money owner exists.
        assert!(
            checked
                .require_active(
                    policy,
                    issuer,
                    INSTALLED_NAMESPACE,
                    &history,
                    &[90; 32],
                    7,
                    349,
                )
                .is_err()
        );

        // A real Core signature and the global codec limit cannot waive the signed 1000 cap.
        // All exact original joins are retained; only the otherwise valid expiry changes.
        let mut over = exact;
        over.expires_at_ms = 1301;
        let over_reply = signed(over, 63);
        over_reply
            .signature
            .verify(
                &policy.policy().enrollment_issuer_key,
                &over_reply.subject.canonical_signing_bytes().unwrap(),
            )
            .unwrap();
        assert!(
            over_reply
                .authenticate(
                    policy,
                    issuer,
                    INSTALLED_NAMESPACE,
                    &history,
                    &[90; 32],
                    7,
                    350,
                )
                .is_err()
        );

        // Private cached-original substitution control only. No public checked constructor
        // is added: completion must reauthenticate the actual original and its governed cap.
        checked.original = over_reply.canonical_bytes().unwrap();
        assert!(
            checked
                .require_active(
                    policy,
                    issuer,
                    INSTALLED_NAMESPACE,
                    &history,
                    &[90; 32],
                    7,
                    350,
                )
                .is_err()
        );
    }
}

#[test]
fn current_state_requires_the_same_complete_issuer_namespace_and_live_time_floor() {
    for apple in [false, true] {
        let f = KagemushaOrdinaryRetailEnrollmentFixtureV1::new(apple);
        let history = historical(&f);
        let policy = f.ordinary_policy.identity_policy();
        let issuer = f.ordinary_policy.issuer_policy();
        let subject = current_subject(&f, &history, KagemushaOrdinaryEnrollmentStateV1::Active);
        let reply = signed(subject, 63);
        let checked = reply
            .authenticate(
                policy,
                issuer,
                INSTALLED_NAMESPACE,
                &history,
                &[90; 32],
                7,
                350,
            )
            .unwrap();
        let foreign = KagemushaOrdinaryRetailEnrollmentFixtureV1::with_selected_scope(
            apple,
            f.selection
                .preparation
                .challenge
                .financial_authority_commitment,
            INSTALLED_NAMESPACE,
            65,
            1,
        );
        let foreign_issuer = foreign.ordinary_policy.issuer_policy();
        assert!(
            KagemushaOrdinaryCurrentIssuerStateSubjectV1::for_committed_enrollment(
                policy,
                foreign_issuer,
                INSTALLED_NAMESPACE,
                &history,
                [90; 32],
                KagemushaOrdinaryEnrollmentStateV1::Active,
                7,
                [0; 32],
                300,
                400,
            )
            .is_err()
        );
        assert!(
            reply
                .authenticate(
                    policy,
                    foreign_issuer,
                    INSTALLED_NAMESPACE,
                    &history,
                    &[90; 32],
                    7,
                    350,
                )
                .is_err()
        );
        assert!(
            checked
                .require_active(
                    policy,
                    foreign_issuer,
                    INSTALLED_NAMESPACE,
                    &history,
                    &[90; 32],
                    7,
                    350,
                )
                .is_err()
        );
        for namespace in [[0; 32], [84; 32]] {
            assert!(
                KagemushaOrdinaryCurrentIssuerStateSubjectV1::for_committed_enrollment(
                    policy,
                    issuer,
                    namespace,
                    &history,
                    [90; 32],
                    KagemushaOrdinaryEnrollmentStateV1::Active,
                    7,
                    [0; 32],
                    300,
                    400,
                )
                .is_err()
            );
            assert!(
                reply
                    .authenticate(policy, issuer, namespace, &history, &[90; 32], 7, 350,)
                    .is_err()
            );
            assert!(
                checked
                    .require_active(policy, issuer, namespace, &history, &[90; 32], 7, 350,)
                    .is_err()
            );
        }
        // A projected body with a changed cap cannot recreate the genuine issuer owner.
        let mut changed = issuer.policy().clone();
        changed.maximum_current_state_lifetime_ms = 1001;
        assert!(
            changed
                .authenticate_under_policy(policy, INSTALLED_NAMESPACE, 300)
                .is_err()
        );
        let later = issuer
            .policy()
            .authenticate_under_policy(policy, INSTALLED_NAMESPACE, 400)
            .unwrap();
        assert!(
            KagemushaOrdinaryCurrentIssuerStateSubjectV1::for_committed_enrollment(
                policy,
                &later,
                INSTALLED_NAMESPACE,
                &history,
                [90; 32],
                KagemushaOrdinaryEnrollmentStateV1::Active,
                7,
                [0; 32],
                300,
                400,
            )
            .is_err()
        );
        assert!(
            reply
                .authenticate(
                    policy,
                    &later,
                    INSTALLED_NAMESPACE,
                    &history,
                    &[90; 32],
                    7,
                    350,
                )
                .is_err()
        );
        assert!(
            checked
                .require_active(
                    policy,
                    &later,
                    INSTALLED_NAMESPACE,
                    &history,
                    &[90; 32],
                    7,
                    350,
                )
                .is_err()
        );
    }
}

#[test]
fn archived_current_cap_uses_signed_original_time_without_returning_a_live_owner() {
    for apple in [false, true] {
        let f = KagemushaOrdinaryRetailEnrollmentFixtureV1::new(apple);
        let history = historical(&f);
        let policy = f.ordinary_policy.identity_policy();
        let issuer = f.ordinary_policy.issuer_policy();
        let subject = KagemushaOrdinaryCurrentIssuerStateSubjectV1::for_committed_enrollment(
            policy,
            issuer,
            INSTALLED_NAMESPACE,
            &history,
            [90; 32],
            KagemushaOrdinaryEnrollmentStateV1::Active,
            7,
            [0; 32],
            300,
            1300,
        )
        .unwrap();
        let original = signed(subject, 63).canonical_bytes().unwrap();
        let archived = policy
            .authenticate_archived_enrollment_original_data(
                history.preparation_original(),
                &f.selection.preparation.challenge,
                history.raw_admission_original(),
                history.platform_original(),
                history.possession_original(),
                history.credential().original(),
                &history.credential().subject().app_public_key,
            )
            .unwrap();
        let later = issuer
            .policy()
            .authenticate_under_policy(policy, INSTALLED_NAMESPACE, 1500)
            .unwrap();
        assert!(
            KagemushaSignedOrdinaryCurrentIssuerStateV1::decode_canonical_exact(&original)
                .unwrap()
                .authenticate(
                    policy,
                    &later,
                    INSTALLED_NAMESPACE,
                    &history,
                    &[90; 32],
                    7,
                    1500,
                )
                .is_err()
        );
        // Exact known original remains mathematically checkable after expiry and after
        // a later genuine issuer authentication. The only return is (), not a checked grant.
        assert_eq!(
            archived
                .authenticate_current_reply_original_data(
                    &later,
                    INSTALLED_NAMESPACE,
                    &original,
                    &[90; 32],
                    7,
                )
                .unwrap(),
            ()
        );
        let mut over = subject;
        over.expires_at_ms = 1301;
        let over_original = signed(over, 63).canonical_bytes().unwrap();
        assert!(
            archived
                .authenticate_current_reply_original_data(
                    &later,
                    INSTALLED_NAMESPACE,
                    &over_original,
                    &[90; 32],
                    7,
                )
                .is_err()
        );
        assert!(archived.authenticate_current_reply_original_data(
            &later, [84; 32], &original, &[90; 32], 7,
        ).is_err());
        let foreign = KagemushaOrdinaryRetailEnrollmentFixtureV1::with_selected_scope(
            apple,
            f.selection
                .preparation
                .challenge
                .financial_authority_commitment,
            INSTALLED_NAMESPACE,
            65,
            1,
        );
        assert!(
            archived
                .authenticate_current_reply_original_data(
                    foreign.ordinary_policy.issuer_policy(),
                    INSTALLED_NAMESPACE,
                    &original,
                    &[90; 32],
                    7,
                )
                .is_err()
        );
        assert!(
            archived
                .authenticate_current_reply_original_data(
                    &later,
                    INSTALLED_NAMESPACE,
                    &signed(subject, 64).canonical_bytes().unwrap(),
                    &[90; 32],
                    7,
                )
                .is_err()
        );
    }
}
