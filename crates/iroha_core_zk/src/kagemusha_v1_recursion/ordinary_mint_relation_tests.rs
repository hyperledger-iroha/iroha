//! Mathematical originals only: genuine governed issuer/app signatures, no physical/Native owner.
use super::super::super::{
    generation::{canonical_kagemusha_ep_parameters_v1, canonical_kagemusha_eq_parameters_v1},
    ordinary_mint_public::ordinary_mint_public_column_v1,
};
use super::*;
use halo2_proofs::dev::MockProver;
use iroha_crypto::{
    KeyGenOption,
    kex::{KeyExchangeScheme as _, X25519Sha256},
};
use iroha_data_model::testing::ordinary_app_enrollment::KagemushaOrdinaryRetailEnrollmentFixtureV1 as Fixture;
use p256::ecdsa::{Signature, SigningKey, signature::Signer as _};
struct Originals {
    enrollment: Fixture,
    statement: KagemushaOrdinaryMintAuthorizationStatementV1,
    approval: KagemushaOrdinaryMintApprovalV1,
    opening: KagemushaCreditOpeningV1,
    encrypted: Vec<u8>,
    secret: DigestV1,
    table: OrdinaryIssuerTableV1,
    floor: Option<u32>,
}
fn make_context(
    apple: bool,
) -> (
    Fixture,
    KagemushaOrdinaryMintAuthorizationContextV1,
    KagemushaCreditOpeningV1,
) {
    let fixture = Fixture::with_single_member_wallet(
        apple,
        false,
        super::super::super::guard_bundle::device_authority_commitment_v1([0x41; 32]),
    );
    let verified = fixture.verify(1000).unwrap();
    let c = verified.app_credential();
    let s = c.subject();
    let (x, _) = X25519Sha256::new().keypair(KeyGenOption::UseSeed(vec![32; 32]));
    let recipient_key = x.to_bytes();
    let owner = fixture.selection.owner.clone();
    let rt = &owner.runtime;
    let operation_id = [45; 32];
    let amount = 177;
    let opening = KagemushaCreditOpeningV1 {
        version: 1,
        credit_id: [0; 32],
        amount,
        credit_commitment_opening: [50; 32],
        recipient_binding_opening: [51; 32],
        recovery_nonce: [52; 32],
    };
    let mut context = KagemushaOrdinaryMintAuthorizationContextV1 {
        version: 1,
        operation_id,
        lineage: KagemushaOrdinaryFinancialLineageV1 {
            version: 1,
            owner: owner.clone(),
            financial_epoch_id: kagemusha_ordinary_financial_epoch_id_v1(s).unwrap(),
            financial_authority_commitment: s.financial_authority_commitment,
        },
        predecessor: KagemushaOrdinaryFinancialHeadV1 {
            state_commitment: [46; 32],
            logical_sequence: (1_u128 << 101) + 7,
            state_original_sha256: [47; 32],
        },
        release_id: s.release_id,
        suite_id: s.suite_id,
        vk_digest: [48; 32],
        artifact_manifest_digest: [49; 32],
        recipient_app_credential_digest: c.digest(),
        app_credential_profile_id: s.hardware_profile_id,
        policy_epoch: s.policy_epoch,
        amount,
        recipient_credential_commitment: kagemusha_recipient_credential_commitment_v1(
            operation_id,
            c.digest(),
            opening.recipient_binding_opening,
        )
        .unwrap(),
        credit_commitment: kagemusha_mint_credit_opening_commitment_v1(
            &rt.network_id,
            &rt.asset,
            rt.asset_incarnation,
            rt.scale,
            kagemusha_liability_pool_id_v1(&rt.network_id, &rt.asset, rt.asset_incarnation)
                .unwrap(),
            amount,
            &owner.account_id,
            recipient_key,
            opening.credit_commitment_opening,
        )
        .unwrap(),
        recipient_one_time_key: recipient_key,
        clock_context: KagemushaOrdinaryCashClockContextV1 {
            version: 1,
            request_nonce: [53; 32],
            signed_observations_original_digest: [54; 32],
            lower_at_ms: 1000,
            upper_at_ms: 1010,
        },
        financial_control_original_sha256: [55; 32],
    };
    context.validate_shape().unwrap();
    context.validate_against_credential(c).unwrap();
    let mut opening = opening;
    opening.credit_id = context.credit_id().unwrap();
    context.validate_credit_opening(&opening).unwrap();
    // Ensure a stable valid context is returned, without claiming these metadata bytes were observed.
    context.version = 1;
    (fixture, context, opening)
}
fn make_statement(
    context: KagemushaOrdinaryMintAuthorizationContextV1,
) -> (KagemushaOrdinaryMintAuthorizationStatementV1, Vec<u8>) {
    let (x, _) = X25519Sha256::new().keypair(KeyGenOption::UseSeed(vec![33; 32]));
    let envelope = KagemushaEncryptedCreditEnvelopeV1 {
        version: 1,
        ephemeral_x25519_public_key: x.to_bytes(),
        nonce: [56; 24],
        ciphertext_and_tag: vec![57; kagemusha_credit_opening_canonical_len_v1().unwrap() + 16],
    };
    let raw = envelope
        .canonical_bytes_against_recipient_key(context.recipient_one_time_key)
        .unwrap();
    assert_eq!(raw.len(), KAGEMUSHA_ENCRYPTED_CREDIT_CANONICAL_BYTES_V1);
    let s = KagemushaOrdinaryMintAuthorizationStatementV1 {
        version: 1,
        issuance_commitment: context.issuance_commitment().unwrap(),
        credit_id: context.credit_id().unwrap(),
        context,
        ciphertext_digest: kagemusha_ciphertext_digest_v1(&raw),
    };
    s.validate_encrypted_credit(&raw).unwrap();
    (s, raw)
}

fn sign(
    challenge: &KagemushaOrdinaryMintApprovalChallengeV1,
    apple: bool,
    key_byte: u8,
) -> KagemushaAppOperationApprovalEvidenceV1 {
    let key = SigningKey::from_bytes((&[key_byte; 32]).into()).unwrap();
    let message = challenge.canonical_signing_bytes().unwrap();
    if !apple {
        let signature: Signature = key.sign(&message);
        return KagemushaAppOperationApprovalEvidenceV1::AndroidKeystore {
            signature_der: signature.to_der().as_bytes().to_vec(),
        };
    }
    let mut auth = [0_u8; 37];
    auth[..32].fill(2);
    auth[32] = 0x40;
    auth[33..].copy_from_slice(&17_u32.to_be_bytes());
    let mut h = Sha256::new();
    h.update(auth);
    h.update(Sha256::digest(&message));
    let signature: Signature = key.sign(&h.finalize());
    let der = signature.to_der();
    let mut raw = vec![0xa2, 0x71];
    raw.extend_from_slice(b"authenticatorData");
    raw.extend_from_slice(&[0x58, 37]);
    raw.extend(auth);
    raw.push(0x69);
    raw.extend_from_slice(b"signature");
    raw.extend_from_slice(&[0x58, der.as_bytes().len() as u8]);
    raw.extend_from_slice(der.as_bytes());
    KagemushaAppOperationApprovalEvidenceV1::AppleAppAttest { raw_assertion: raw }
}
fn originals(apple: bool) -> Originals {
    let (enrollment, context, opening) = make_context(apple);
    let table = OrdinaryIssuerTableV1::from_release(&enrollment.release).unwrap();
    let (statement, encrypted) = make_statement(context);
    let challenge = KagemushaOrdinaryMintApprovalChallengeV1 {
        version: 1,
        operation_id: statement.context.operation_id,
        nonce: [58; 32],
        credential_digest: statement.context.recipient_app_credential_digest,
        statement_digest: statement.binding_digest().unwrap(),
        clock_context_digest: statement.context.clock_context.binding_digest().unwrap(),
        financial_control_original_sha256: statement.context.financial_control_original_sha256,
        issued_at_ms: 1000,
        expires_at_ms: 1100,
    };
    let approval = KagemushaOrdinaryMintApprovalV1 {
        evidence: sign(&challenge, apple, 7),
        challenge,
    };
    let verified = enrollment.verify(1000).unwrap();
    // Mathematical witness floor16 is later than signed enrollment minimum11.
    // These fixtures create no actual Native counter owner.
    let floor = apple.then_some(16);
    approval
        .authenticate_platform_equation(verified.app_credential(), floor)
        .unwrap();
    Originals {
        enrollment,
        statement,
        approval,
        opening,
        encrypted,
        secret: [0x41; 32],
        table,
        floor,
    }
}
fn witness(f: &Originals) -> OrdinaryMintWitnessV1<'_> {
    OrdinaryMintWitnessV1 {
        statement: &f.statement,
        approval: &f.approval,
        credential: &f.enrollment.selection.issuance.credential,
        previous_app_attest_counter: f.floor,
        integrity_lease: None,
        financial_secret: &f.secret,
        credit_opening: &f.opening,
        encrypted_credit: &f.encrypted,
    }
}
fn both_satisfied(f: &Originals, public_mutations: bool) -> Result<bool, String> {
    let eq_parameters = canonical_kagemusha_eq_parameters_v1();
    let ep_parameters = canonical_kagemusha_ep_parameters_v1();
    let root = f.enrollment.release.provider_policy_root();
    let data = ordinary_mint_public_data_v1(
        &f.statement,
        &f.approval,
        &f.enrollment.selection.issuance.credential,
        None,
        root,
    )?;
    let eh = initial_kagemusha_eq_accumulator_v1(&eq_parameters).map_err(|e| e.to_string())?;
    let ph = initial_kagemusha_ep_accumulator_v1(&ep_parameters).map_err(|e| e.to_string())?;
    let eq_public = ordinary_mint_public_column_v1::<Fp>(&data, eh.as_bytes());
    let ep_public = ordinary_mint_public_column_v1::<Fq>(&data, ph.as_bytes());
    assert_eq!(eq_public.len(), 113);
    assert_eq!(ep_public.len(), 113);
    let eq = build_ordinary_mint_eq_v1(&eq_parameters, &witness(f), root, &f.table)?;
    let eq_pass = MockProver::run(KAGEMUSHA_HALO2_K_V1, &eq, vec![eq_public.clone()])
        .map_err(|e| format!("Eq Mint synthesis: {e:?}"))?
        .verify()
        .is_ok();
    if public_mutations && eq_pass {
        // Complete statement/approval/C/evidence/account/predecessor original, u128 sequence/history.
        for offset in [0, 6, 4, 8, 34, 64, 72, 79] {
            let mut e = eq_public.clone();
            e[offset] += Fp::from(1);
            assert!(
                MockProver::run(KAGEMUSHA_HALO2_K_V1, &eq, vec![e])
                    .unwrap()
                    .verify()
                    .is_err(),
                "Eq original slot {offset}"
            );
        }
    }
    drop(eq);
    halo2_proofs::release_allocator_slack();
    let ep = build_ordinary_mint_ep_v1(&ep_parameters, &witness(f), root, &f.table)?;
    let ep_pass = MockProver::run(KAGEMUSHA_HALO2_K_V1, &ep, vec![ep_public.clone()])
        .map_err(|e| format!("Ep Mint synthesis: {e:?}"))?
        .verify()
        .is_ok();
    if public_mutations && ep_pass {
        for offset in [0, 6, 4, 8, 34, 64, 72, 79] {
            let mut p = ep_public.clone();
            p[offset] += Fq::from(1);
            assert!(
                MockProver::run(KAGEMUSHA_HALO2_K_V1, &ep, vec![p])
                    .unwrap()
                    .verify()
                    .is_err(),
                "Ep original slot {offset}"
            );
        }
    }
    assert_eq!(eq_pass, ep_pass);
    Ok(eq_pass && ep_pass)
}
#[test]
fn genuine_android_and_apple_mint113_openings_and_original_mutations_fail_both_fields() {
    for apple in [false, true] {
        assert!(both_satisfied(&originals(apple), true).unwrap());
    }
}
#[test]
fn foreign_platform_key_and_wrong_financial_secret_fail_both_mint_fields() {
    let mut f = originals(false);
    f.secret[0] ^= 1;
    assert!(!both_satisfied(&f, false).unwrap());
    let mut f = originals(false);
    f.approval.evidence = sign(&f.approval.challenge, false, 8);
    assert!(!both_satisfied(&f, false).unwrap());
}
#[test]
fn full_credit_opening_substitution_is_rejected_before_proof_assignment() {
    let mut f = originals(false);
    f.opening.credit_commitment_opening[0] ^= 1;
    let p = canonical_kagemusha_eq_parameters_v1();
    assert!(
        build_ordinary_mint_eq_v1(
            &p,
            &witness(&f),
            f.enrollment.release.provider_policy_root(),
            &f.table
        )
        .is_err()
    );
    let mut f = originals(false);
    f.opening.amount += 1;
    assert!(
        build_ordinary_mint_eq_v1(
            &p,
            &witness(&f),
            f.enrollment.release.provider_policy_root(),
            &f.table
        )
        .is_err()
    );
}
#[test]
#[ignore = "actual fresh ordinary Mint Eq/Ep keygen and proof; maintained CPU/RSS runner required"]
fn genuine_ordinary_mint113_keys_and_actual_both_parity_proofs() {
    super::super::super::real_handoff_qualification_tests::real_payment_corridor::run_ordinary_zero_bootstrap_qualification_worker(
        || {
            let f = originals(false);
            let seed =
                iroha_crypto::kagemusha::KagemushaRecoverySeedV1::from_unsealed([0xa7; 32])
                    .unwrap();
            let pair=super::super::super::generation::ordinary_mint_generation::generate_ordinary_mint_pair_v1(witness(&f),f.enrollment.release.provider_policy_root(),&f.table,&seed).unwrap();
            assert_eq!(pair.eq.instances.len(), 113);
            assert_eq!(pair.ep.instances.len(), 113);
            assert!(!pair.eq.proof.is_empty());
            assert!(!pair.ep.proof.is_empty());
            // The generator already executes exact current succinct verification and terminal decisions.
            // Reusable-current proof mutations are rejected by those same actual compiled protocols.
            let mut e = pair.eq.instances.clone();
            e[0] += Fp::from(1);
            assert!(
                super::super::super::native_backend::verify_eq_succinct_protocol(
                    &canonical_kagemusha_eq_parameters_v1(),
                    &pair.eq.protocol,
                    &pair.eq.proof,
                    &e
                )
                .is_err()
            );
            let mut p = pair.ep.instances.clone();
            p[0] += Fq::from(1);
            assert!(
                super::super::super::native_backend::verify_ep_succinct_protocol(
                    &canonical_kagemusha_ep_parameters_v1(),
                    &pair.ep.protocol,
                    &pair.ep.proof,
                    &p
                )
                .is_err()
            );
            // If a same-layout processed key parses under another mode, its actual compiled
            // protocol still differs and cannot verify this proof. No loader falls back to it.
            if let Ok(wrong_vk)=super::super::super::native_backend::read_eq_ordinary_guard_vk(&pair.eq.verifying_key,pair.eq.base_params.clone(),f.enrollment.release.provider_policy_root(),&f.table) {
                let wrong=snark_verifier::system::halo2::compile(&canonical_kagemusha_eq_parameters_v1(),&wrong_vk,snark_verifier::system::halo2::Config::ipa().with_num_instance(vec![44]));
                assert!(super::super::super::native_backend::verify_eq_succinct_protocol(&canonical_kagemusha_eq_parameters_v1(),&wrong,&pair.eq.proof,&pair.eq.instances).is_err());
            }
            if let Ok(wrong_vk)=super::super::super::native_backend::read_ep_ordinary_guard_vk(&pair.ep.verifying_key,pair.ep.base_params.clone(),f.enrollment.release.provider_policy_root(),&f.table) {
                let wrong=snark_verifier::system::halo2::compile(&canonical_kagemusha_ep_parameters_v1(),&wrong_vk,snark_verifier::system::halo2::Config::ipa().with_num_instance(vec![44]));
                assert!(super::super::super::native_backend::verify_ep_succinct_protocol(&canonical_kagemusha_ep_parameters_v1(),&wrong,&pair.ep.proof,&pair.ep.instances).is_err());
            }

        },
    );
}
