//! Genuine ordinary Mint113 keys, both-field proofs and complete-original mutations.
//!
//! The issuer/app keys and measurement metadata are known-public mathematical fixture data.
//! No phone, threshold release, FI control, Node debit/finality or Native owner is admitted.
//! This test qualifies the complete production Mint113 relation, not a funded State or Receive.

use super::*;
use crate::kagemusha_v1_recursion::native_backend::{
    verify_ep_succinct_protocol, verify_eq_succinct_protocol,
};
use crate::kagemusha_v1_recursion::{
    ordinary_mint_circuit::{
        ORDINARY_MINT_PUBLIC_INSTANCE_COUNT_V1, OrdinaryMintWitnessV1, build_ordinary_mint_ep_v1,
        build_ordinary_mint_eq_v1,
    },
    ordinary_mint_public::{ordinary_mint_public_column_v1, ordinary_mint_public_data_v1},
};
use crate::kagemusha_v1_state::DigestV1;
use halo2_proofs::dev::MockProver;
use iroha_crypto::{
    Algorithm, KeyGenOption, KeyPair,
    kex::{KeyExchangeScheme as _, X25519Sha256},
};
use iroha_data_model::{account::AccountId, kagemusha::*};
use p256::ecdsa::{Signature as P256Signature, SigningKey, signature::Signer as _};

#[path = "ordinary_zero_bootstrap_fixture.rs"]
mod originals;

pub(super) struct MintOriginals {
    pub(super) enrollment: originals::Fixture,
    pub(super) statement: KagemushaOrdinaryMintAuthorizationStatementV1,
    pub(super) approval: KagemushaOrdinaryMintApprovalV1,
    pub(super) opening: KagemushaCreditOpeningV1,
    pub(super) encrypted_credit: Vec<u8>,
}

fn sign_message(message: &[u8], apple: bool) -> KagemushaAppOperationApprovalEvidenceV1 {
    sign_message_with_counter(message, apple, 17)
}

pub(super) fn sign_message_with_counter(
    message: &[u8],
    apple: bool,
    counter: u32,
) -> KagemushaAppOperationApprovalEvidenceV1 {
    let key = SigningKey::from_bytes((&[7; 32]).into()).unwrap();
    if !apple {
        let signature: P256Signature = key.sign(message);
        let signature = signature.normalize_s().unwrap_or(signature);
        return KagemushaAppOperationApprovalEvidenceV1::AndroidKeystore {
            signature_der: signature.to_der().as_bytes().to_vec(),
        };
    }
    let mut auth = [0; 37];
    auth[..32].fill(2);
    auth[32] = 0x40;
    auth[33..].copy_from_slice(&counter.to_be_bytes());
    let mut nonce = Sha256::new();
    nonce.update(auth);
    nonce.update(Sha256::digest(message));
    let signature: P256Signature = key.sign(&nonce.finalize());
    let signature = signature.normalize_s().unwrap_or(signature);
    let der = signature.to_der();
    let mut raw = vec![0xa2, 0x71];
    raw.extend_from_slice(b"authenticatorData");
    raw.extend_from_slice(&[0x58, 37]);
    raw.extend_from_slice(&auth);
    raw.push(0x69);
    raw.extend_from_slice(b"signature");
    raw.extend_from_slice(&[0x58, u8::try_from(der.as_bytes().len()).unwrap()]);
    raw.extend_from_slice(der.as_bytes());
    KagemushaAppOperationApprovalEvidenceV1::AppleAppAttest { raw_assertion: raw }
}

fn fixture(apple: bool) -> MintOriginals {
    fixture_for_active_state(apple, [41; 32], [40; 32], [42; 32], [49; 32], [47; 32])
}

/// Known-public mathematical originals derived by the same maintained sole encoders/signers.
/// This is test data, never a Native enrollment, clock, FI decision or finalized source grant.
pub(super) fn fixture_for_active_state(
    apple: bool,
    release_id: DigestV1,
    suite_id: DigestV1,
    vk_digest: DigestV1,
    manifest_digest: DigestV1,
    predecessor_public_sha256: DigestV1,
) -> MintOriginals {
    let account = super::ordinary_qualification_wallet_account_v1(62);
    let enrollment =
        originals::fixture_for_account(apple, release_id, suite_id, vk_digest, &account);
    let c = &enrollment.credential;
    let s = &c.subject;
    let (recipient, _) = X25519Sha256::new().keypair(KeyGenOption::UseSeed(vec![32; 32]));
    let mut opening = KagemushaCreditOpeningV1 {
        version: 1,
        credit_id: [0; 32],
        amount: 177,
        credit_commitment_opening: [50; 32],
        recipient_binding_opening: [51; 32],
        recovery_nonce: [52; 32],
    };
    let operation_id = [45; 32];
    let owner = KagemushaRetailEnrollmentOwnerV1 {
        account_id: account,
        runtime: KagemushaRetailEnrollmentRuntimeV1 {
            fi_id: "mathematical-fi".parse().unwrap(),
            ledger_dataspace_id: iroha_model_base::topology::DataSpaceId::new(1),
            authentication_namespace: "mathematical-fi".parse().unwrap(),
            network_id: enrollment.state.lane.network_id,
            asset: enrollment.state.lane.asset.clone(),
            asset_incarnation: enrollment.state.asset_incarnation,
            scale: enrollment.state.lane.scale,
        },
        lane_id: enrollment.state.lane.device_lane_id,
    };
    let context = KagemushaOrdinaryMintAuthorizationContextV1 {
        version: 1,
        operation_id,
        lineage: KagemushaOrdinaryFinancialLineageV1 {
            version: 1,
            owner: owner.clone(),
            financial_epoch_id: kagemusha_ordinary_financial_epoch_id_v1(s).unwrap(),
            financial_authority_commitment: s.financial_authority_commitment,
        },
        predecessor: KagemushaOrdinaryFinancialHeadV1 {
            state_commitment: enrollment.state.state_commitment,
            logical_sequence: enrollment.state.logical_sequence,
            state_original_sha256: predecessor_public_sha256,
        },
        release_id: s.release_id,
        suite_id: s.suite_id,
        vk_digest: enrollment.state.vk_digest,
        artifact_manifest_digest: manifest_digest,
        recipient_app_credential_digest: c.canonical_digest().unwrap(),
        app_credential_profile_id: s.hardware_profile_id,
        policy_epoch: s.policy_epoch,
        amount: opening.amount,
        recipient_credential_commitment: kagemusha_recipient_credential_commitment_v1(
            operation_id,
            c.canonical_digest().unwrap(),
            opening.recipient_binding_opening,
        )
        .unwrap(),
        credit_commitment: kagemusha_mint_credit_opening_commitment_v1(
            &owner.runtime.network_id,
            &owner.runtime.asset,
            owner.runtime.asset_incarnation,
            owner.runtime.scale,
            enrollment.state.liability_pool_id,
            opening.amount,
            &owner.account_id,
            recipient.to_bytes(),
            opening.credit_commitment_opening,
        )
        .unwrap(),
        recipient_one_time_key: recipient.to_bytes(),
        clock_context: KagemushaOrdinaryCashClockContextV1 {
            version: 1,
            request_nonce: [53; 32],
            signed_observations_original_digest: [54; 32],
            lower_at_ms: 1000,
            upper_at_ms: 1010,
        },
        financial_control_original_sha256: [55; 32],
    };
    opening.credit_id = context.credit_id().unwrap();
    context.validate_shape().unwrap();
    context.validate_credit_opening(&opening).unwrap();
    let (ephemeral, _) = X25519Sha256::new().keypair(KeyGenOption::UseSeed(vec![33; 32]));
    // Exact neutral envelope data. AEAD admission is a separate Native relation; this
    // mathematical Mint113 test neither decrypts this synthetic ciphertext nor grants funds.
    let envelope = KagemushaEncryptedCreditEnvelopeV1 {
        version: 1,
        ephemeral_x25519_public_key: ephemeral.to_bytes(),
        nonce: [56; 24],
        ciphertext_and_tag: vec![57; kagemusha_credit_opening_canonical_len_v1().unwrap() + 16],
    };
    let encrypted_credit = envelope
        .canonical_bytes_against_recipient_key(recipient.to_bytes())
        .unwrap();
    assert_eq!(
        encrypted_credit.len(),
        KAGEMUSHA_ENCRYPTED_CREDIT_CANONICAL_BYTES_V1
    );
    let statement = KagemushaOrdinaryMintAuthorizationStatementV1 {
        version: 1,
        issuance_commitment: context.issuance_commitment().unwrap(),
        credit_id: opening.credit_id,
        context,
        ciphertext_digest: kagemusha_ciphertext_digest_v1(&encrypted_credit),
    };
    let challenge = KagemushaOrdinaryMintApprovalChallengeV1 {
        version: 1,
        operation_id,
        nonce: [58; 32],
        credential_digest: statement.context.recipient_app_credential_digest,
        statement_digest: statement.binding_digest().unwrap(),
        clock_context_digest: statement.context.clock_context.binding_digest().unwrap(),
        financial_control_original_sha256: statement.context.financial_control_original_sha256,
        issued_at_ms: 1000,
        expires_at_ms: 1100,
    };
    let approval = KagemushaOrdinaryMintApprovalV1 {
        evidence: sign_message(&challenge.canonical_signing_bytes().unwrap(), apple),
        challenge,
    };
    verify_platform(&approval, c, enrollment.previous_counter);
    MintOriginals {
        enrollment,
        statement,
        approval,
        opening,
        encrypted_credit,
    }
}

#[test]
#[ignore = "genuine ordinary Mint113 PK/VK plus both-field proofs; exclusive maintained worker and external direct-libtest CPU/RSS guard required"]
fn ordinary_mint113_generated_keys_both_parities_reject_complete_original_and_secret_substitution()
{
    crate::kagemusha_v1_recursion::real_handoff_qualification_tests::real_payment_corridor::run_ordinary_zero_bootstrap_qualification_worker(qualify);
}

fn qualify() {
    let eq = canonical_kagemusha_eq_parameters_v1();
    let ep = canonical_kagemusha_ep_parameters_v1();
    let seed = KagemushaRecoverySeedV1::from_unsealed([44; 32]).unwrap();
    let mut first_protocols = None;
    for apple in [false, true] {
        let f = fixture(apple);
        let secret = [0x41; 32];
        let witness = OrdinaryMintWitnessV1 {
            statement: &f.statement,
            approval: &f.approval,
            credential: &f.enrollment.credential,
            previous_app_attest_counter: f.enrollment.previous_counter,
            integrity_lease: None,
            financial_secret: &secret,
            credit_opening: &f.opening,
            encrypted_credit: &f.encrypted_credit,
        };
        let provider = f.enrollment.state.device_policy_binding.hardware_policy_id;
        let generated = super::ordinary_mint_generation::generate_ordinary_mint_pair_v1(
            witness, provider, &f.enrollment.issuer_table, &seed,
        ).expect("complete production Mint113 resource-preflight key generation and genuine Eq/Ep proofs");
        let identities = (generated.eq.protocol_digest, generated.ep.protocol_digest);
        if let Some(expected) = first_protocols {
            assert_eq!(
                identities, expected,
                "fixed platform union must retain the same Mint113 protocol family"
            );
        } else {
            first_protocols = Some(identities);
        }
        let data = ordinary_mint_public_data_v1(
            &f.statement,
            &f.approval,
            &f.enrollment.credential,
            None,
            provider,
        )
        .unwrap();
        let eq_column =
            ordinary_mint_public_column_v1::<Fp>(&data, generated.eq.history.as_bytes());
        let ep_column =
            ordinary_mint_public_column_v1::<Fq>(&data, generated.ep.history.as_bytes());
        assert_eq!(eq_column.len(), ORDINARY_MINT_PUBLIC_INSTANCE_COUNT_V1);
        assert_eq!(eq_column, generated.eq.instances);
        assert_eq!(ep_column, generated.ep.instances);
        assert!(decide_eq(
            &eq,
            &generated.eq.protocol,
            &generated.eq.proof,
            &eq_column
        ));
        assert!(decide_ep(
            &ep,
            &generated.ep.protocol,
            &generated.ep.proof,
            &ep_column
        ));
        for slot in [
            0, 1, 2, 3, 4, 6, 7, 10, 18, 19, 21, 22, 23, 24, 25, 26, 27, 28, 31, 32, 33,
        ] {
            let mut changed = data;
            changed.digests[slot][0] ^= 1;
            let eq_column =
                ordinary_mint_public_column_v1::<Fp>(&changed, generated.eq.history.as_bytes());
            let ep_column =
                ordinary_mint_public_column_v1::<Fq>(&changed, generated.ep.history.as_bytes());
            assert!(
                !decide_eq(&eq, &generated.eq.protocol, &generated.eq.proof, &eq_column),
                "actual Eq original selector {slot}, Apple={apple}"
            );
            assert!(
                !decide_ep(&ep, &generated.ep.protocol, &generated.ep.proof, &ep_column),
                "actual Ep original selector {slot}, Apple={apple}"
            );
        }
        // A well-signed, shape-valid foreign approval is a changed COMPLETE original,
        // not merely an offered instance-vector mutation.
        let mut foreign = f.approval.clone();
        foreign.challenge.nonce[0] ^= 1;
        foreign.evidence =
            sign_message(&foreign.challenge.canonical_signing_bytes().unwrap(), apple);
        verify_platform(
            &foreign,
            &f.enrollment.credential,
            f.enrollment.previous_counter,
        );
        let foreign_data = ordinary_mint_public_data_v1(
            &f.statement,
            &foreign,
            &f.enrollment.credential,
            None,
            provider,
        )
        .unwrap();
        assert!(!decide_eq(
            &eq,
            &generated.eq.protocol,
            &generated.eq.proof,
            &ordinary_mint_public_column_v1::<Fp>(&foreign_data, generated.eq.history.as_bytes())
        ));
        assert!(!decide_ep(
            &ep,
            &generated.ep.protocol,
            &generated.ep.proof,
            &ordinary_mint_public_column_v1::<Fq>(&foreign_data, generated.ep.history.as_bytes())
        ));
        for mutation in 0..4 {
            let mut opening = f.opening;
            let mut secret = secret;
            match mutation {
                0 => secret[0] ^= 1,
                1 => opening.credit_commitment_opening[0] ^= 1,
                2 => opening.recipient_binding_opening[0] ^= 1,
                3 => opening.recovery_nonce = [0; 32],
                _ => unreachable!(),
            }
            let bad = OrdinaryMintWitnessV1 {
                statement: &f.statement,
                approval: &f.approval,
                credential: &f.enrollment.credential,
                previous_app_attest_counter: f.enrollment.previous_counter,
                integrity_lease: None,
                financial_secret: &secret,
                credit_opening: &opening,
                encrypted_credit: &f.encrypted_credit,
            };
            let bad_eq = build_ordinary_mint_eq_v1(&eq, &bad, provider, &f.enrollment.issuer_table);
            if let Ok(circuit) = bad_eq {
                assert!(
                    MockProver::run(16, &circuit, vec![eq_column.clone()])
                        .unwrap()
                        .verify()
                        .is_err(),
                    "Eq secret opening {mutation}, Apple={apple}"
                );
            }
            halo2_proofs::release_allocator_slack();
            let bad_ep = build_ordinary_mint_ep_v1(&ep, &bad, provider, &f.enrollment.issuer_table);
            if let Ok(circuit) = bad_ep {
                assert!(
                    MockProver::run(16, &circuit, vec![ep_column.clone()])
                        .unwrap()
                        .verify()
                        .is_err(),
                    "Ep secret opening {mutation}, Apple={apple}"
                );
            }
            halo2_proofs::release_allocator_slack();
        }
        let mut history = *generated.eq.history.as_bytes();
        history[0] ^= 1;
        assert!(!decide_eq(
            &eq,
            &generated.eq.protocol,
            &generated.eq.proof,
            &ordinary_mint_public_column_v1::<Fp>(&data, &history)
        ));
        let mut history = *generated.ep.history.as_bytes();
        history[0] ^= 1;
        assert!(!decide_ep(
            &ep,
            &generated.ep.protocol,
            &generated.ep.proof,
            &ordinary_mint_public_column_v1::<Fq>(&data, &history)
        ));
        assert!(!decide_eq(
            &eq,
            &generated.eq.protocol,
            &generated.ep.proof,
            &eq_column
        ));
        assert!(!decide_ep(
            &ep,
            &generated.ep.protocol,
            &generated.eq.proof,
            &ep_column
        ));
        assert!(!decide_eq(
            &eq,
            &generated.eq.protocol,
            &generated.eq.proof,
            &eq_column[..84]
        ));
        assert!(!decide_ep(
            &ep,
            &generated.ep.protocol,
            &generated.ep.proof,
            &ep_column[..84]
        ));
        #[cfg(unix)]
        super::ordinary_mint_public_artifact_tests::export_from_environment(
            &generated,
            provider,
            &f.enrollment.issuer_table,
            apple,
        )
        .expect("export and replay exact public mathematical Mint artifacts");
        drop(generated);
        halo2_proofs::release_allocator_slack();
    }
}

fn decide_eq(
    params: &ParamsIPA<EqAffine>,
    protocol: &PlonkProtocol<EqAffine>,
    proof: &[u8],
    column: &[Fp],
) -> bool {
    verify_eq_succinct_protocol(params, protocol, proof, column)
        .ok()
        .and_then(|a| KagemushaEqAccumulatorV1::from_native(&a).ok())
        .is_some_and(|a| decide_kagemusha_eq_accumulator_v1(params, &a).is_ok())
}
fn decide_ep(
    params: &ParamsIPA<EpAffine>,
    protocol: &PlonkProtocol<EpAffine>,
    proof: &[u8],
    column: &[Fq],
) -> bool {
    verify_ep_succinct_protocol(params, protocol, proof, column)
        .ok()
        .and_then(|a| KagemushaEpAccumulatorV1::from_native(&a).ok())
        .is_some_and(|a| decide_kagemusha_ep_accumulator_v1(params, &a).is_ok())
}

fn verify_platform(
    approval: &KagemushaOrdinaryMintApprovalV1,
    c: &KagemushaOrdinaryAppCredentialV1,
    floor: Option<u32>,
) {
    approval
        .evidence
        .authenticate_signature(
            c.subject.platform_class,
            &c.subject.app_public_key,
            c.subject.app_signing_identity_digest,
            c.subject.app_release_digest,
            floor,
            &approval.challenge.canonical_signing_bytes().unwrap(),
        )
        .expect(
            "actual P256/Apple complete-original signature equation over public mathematical C",
        );
}

#[path = "ordinary_active_state_qualification_tests.rs"]
mod active_state;

#[cfg(unix)]
#[path = "ordinary_active_mint_state_tests.rs"]
pub(super) mod active_mint_state;
