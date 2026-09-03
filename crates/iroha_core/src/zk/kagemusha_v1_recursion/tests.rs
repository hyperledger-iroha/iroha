//! Structural and native-primitive tests for the Kagemusha V1 recursion seam.

use std::cell::RefCell;

use ff::Field as _;
use halo2_proofs::{
    dev::MockProver,
    halo2curves::{
        group::{Curve as _, Group as _, GroupEncoding, prime::PrimeCurveAffine as _},
        pasta::{Ep, EpAffine, Eq, EqAffine, Fp, Fq},
    },
    poly::{commitment::ParamsProver as _, ipa::commitment::ParamsIPA},
};
use iroha_crypto::{Algorithm, Hash, HashOf, KeyPair};
use iroha_data_model::{
    NetworkId,
    account::AccountId,
    asset::AssetDefinitionId,
    block::BlockHeader,
    domain::DomainId,
    kagemusha::{
        KAGEMUSHA_ACCEPTANCE_TICKET_MIN_RESERVED_INBOX_BYTES_V1,
        KAGEMUSHA_CURRENT_PROOFS_MAX_BYTES_V1, KAGEMUSHA_HISTORY_ACCUMULATOR_BYTES_V1,
        KAGEMUSHA_PAIRED_PROOF_MAX_BYTES_V1, KAGEMUSHA_PARITY_PROOF_MAX_BYTES_V1,
        KAGEMUSHA_WIRE_VERSION_V1, KAGEMUSHA_XCHACHA20POLY1305_NONCE_BYTES_V1,
        KAGEMUSHA_XCHACHA20POLY1305_TAG_BYTES_V1, KagemushaAcceptanceIntentV1,
        KagemushaAcceptanceTicketV1, KagemushaArtifactBindingV1, KagemushaArtifactRoleV1,
        KagemushaCommitCertificateV1, KagemushaCommitEvidenceV1, KagemushaDevicePublicKeyV1,
        KagemushaDeviceSignatureV1, KagemushaEncryptedCreditEnvelopeV1,
        KagemushaHardwareCredentialV1, KagemushaLifecycleBindingV1, KagemushaOperationKindV1,
        KagemushaPairedProofV1, KagemushaPastaStateCommitmentV1, KagemushaPaymentOutputV1,
        KagemushaPaymentProofV1, KagemushaPaymentRequestModeV1, KagemushaPaymentRequestV1,
        KagemushaPaymentV1, KagemushaRedemptionProofV1, KagemushaSingleExactV1,
        KagemushaTrustedCommitTimeV1, kagemusha_credit_opening_canonical_len_v1,
        kagemusha_device_key_reference_v1, kagemusha_liability_pool_id_v1,
        kagemusha_payment_body_digest_v1,
    },
    nexus::AxtAssetIncarnationV1,
};
use p256::ecdsa::{Signature as P256Signature, SigningKey, signature::Signer as _};
use snark_verifier::{loader::native::NativeLoader, pcs::ipa::IpaAccumulator};

use super::*;
use crate::zk::kagemusha_v1_state::{
    ConsumedCreditInsertWitnessV1, CreditIdV1, DevicePolicyBindingV1, DigestV1, HardwareEpochV1,
    KAGEMUSHA_STATE_VERSION_V1, KagemushaStateV1,
};

fn digest(tag: u8) -> DigestV1 {
    [tag; 32]
}

fn eq_digest(tag: u64) -> DigestV1 {
    crate::zk::kagemusha_v1_poseidon::encode(Fp::from(tag))
}

fn ep_digest(tag: u64) -> DigestV1 {
    crate::zk::kagemusha_v1_poseidon::encode(Fq::from(tag))
}

fn pasta_pair(tag: u64) -> KagemushaPastaStateCommitmentV1 {
    KagemushaPastaStateCommitmentV1 {
        eq: eq_digest(tag),
        ep: ep_digest(tag + 1),
    }
}

fn network() -> NetworkId {
    NetworkId::from_genesis_hash(HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(
        b"kagemusha-v1-recursion-tests",
    )))
}

fn asset() -> AssetDefinitionId {
    AssetDefinitionId::derive_from_components(
        DomainId::try_new("wonderland", "universal").expect("domain"),
        "xor".parse().expect("asset name"),
    )
}

fn incarnation() -> AxtAssetIncarnationV1 {
    let network = network();
    let asset = asset();
    let registration =
        HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(b"kagemusha-registration"));
    AxtAssetIncarnationV1::derive(
        &network,
        &asset,
        &registration,
        &Hash::new(b"kagemusha-registration-execution"),
        1,
    )
}

fn eq_history(tag: u64) -> KagemushaEqAccumulatorV1 {
    let challenges = (0..KAGEMUSHA_RECURSION_IPA_K_V1)
        .map(|round| Fp::from(tag + u64::from(round) + 1))
        .collect();
    let point = (Eq::generator() * Fp::from(tag + 97)).to_affine();
    KagemushaEqAccumulatorV1::from_native(&IpaAccumulator::<EqAffine, NativeLoader>::new(
        challenges, point,
    ))
    .expect("canonical Eq fixture")
}

fn ep_history(tag: u64) -> KagemushaEpAccumulatorV1 {
    let challenges = (0..KAGEMUSHA_RECURSION_IPA_K_V1)
        .map(|round| Fq::from(tag + u64::from(round) + 1))
        .collect();
    let point = (Ep::generator() * Fq::from(tag + 193)).to_affine();
    KagemushaEpAccumulatorV1::from_native(&IpaAccumulator::<EpAffine, NativeLoader>::new(
        challenges, point,
    ))
    .expect("canonical Ep fixture")
}

fn artifact_binding(role: KagemushaArtifactRoleV1, tag: u8) -> KagemushaArtifactBindingV1 {
    KagemushaArtifactBindingV1 {
        role,
        sha256: digest(tag),
        byte_len: 1_024,
    }
}

fn artifacts() -> KagemushaRecursionArtifactsV1 {
    KagemushaRecursionArtifactsV1 {
        release_id: digest(0x43),
        profile_digest: digest(0x50),
        eq_protocol_digest: eq_digest(0x15),
        ep_protocol_digest: ep_digest(0x16),
        terminal_authorization_eq_protocol_digest: eq_digest(0x17),
        terminal_authorization_ep_protocol_digest: ep_digest(0x18),
        commit_wrapper_eq_protocol_digest: eq_digest(0x1D),
        commit_wrapper_ep_protocol_digest: ep_digest(0x1E),
        mint_authorization_eq_protocol_digest: eq_digest(0x19),
        mint_authorization_ep_protocol_digest: ep_digest(0x1A),
        mint_finality_eq_protocol_digest: eq_digest(0x1F),
        mint_finality_ep_protocol_digest: ep_digest(0x20),
        guard_bundle_eq_protocol_digest: eq_digest(0x1B),
        guard_bundle_ep_protocol_digest: ep_digest(0x1C),
        guard_bundle_verifying_key_eq: artifact_binding(
            KagemushaArtifactRoleV1::GuardBundleVkEq,
            0x53,
        ),
        guard_bundle_verifying_key_ep: artifact_binding(
            KagemushaArtifactRoleV1::GuardBundleVkEp,
            0x54,
        ),
        terminal_authorization_verifying_key_eq: artifact_binding(
            KagemushaArtifactRoleV1::TerminalAuthorizationVkEq,
            0x55,
        ),
        terminal_authorization_verifying_key_ep: artifact_binding(
            KagemushaArtifactRoleV1::TerminalAuthorizationVkEp,
            0x56,
        ),
        commit_wrapper_verifying_key_eq: artifact_binding(
            KagemushaArtifactRoleV1::CommitWrapperVkEq,
            0x5C,
        ),
        commit_wrapper_verifying_key_ep: artifact_binding(
            KagemushaArtifactRoleV1::CommitWrapperVkEp,
            0x5D,
        ),
        mint_finality: KagemushaMintFinalityArtifactsV1 {
            proving_key_eq: artifact_binding(KagemushaArtifactRoleV1::MintCreditPkEq, 0x57),
            verifying_key_eq: artifact_binding(KagemushaArtifactRoleV1::MintCreditVkEq, 0x58),
            proving_key_ep: artifact_binding(KagemushaArtifactRoleV1::MintCreditPkEp, 0x59),
            verifying_key_ep: artifact_binding(KagemushaArtifactRoleV1::MintCreditVkEp, 0x5A),
        },
        artifact_manifest_digest: digest(0x5B),
        canonical_empty_effect_digest: digest(0x40),
    }
}

fn account(tag: u8) -> AccountId {
    AccountId::new(
        KeyPair::from_seed(vec![tag; 32], Algorithm::Ed25519)
            .public_key()
            .clone(),
    )
}

fn p256_signing_key(seed: u8) -> SigningKey {
    SigningKey::from_bytes((&[seed; 32]).into()).expect("P-256 signing key")
}

fn device_public_key(key: &SigningKey) -> KagemushaDevicePublicKeyV1 {
    KagemushaDevicePublicKeyV1::from_sec1_bytes(
        key.verifying_key().to_encoded_point(false).as_bytes(),
    )
    .expect("device public key")
}

fn sign(key: &SigningKey, bytes: &[u8]) -> KagemushaDeviceSignatureV1 {
    let signature: P256Signature = key.sign(bytes);
    let signature = signature.normalize_s().unwrap_or(signature);
    KagemushaDeviceSignatureV1::from_raw_bytes(signature.to_bytes().as_ref())
        .expect("low-S signature")
}

fn recipient_one_time_key() -> [u8; 32] {
    let mut key = [0; 32];
    key[0] = 9;
    key
}

pub(super) struct IncomingPaymentFixtureV1 {
    pub(super) request: KagemushaPaymentRequestV1,
    pub(super) intent: KagemushaAcceptanceIntentV1,
    pub(super) ticket: KagemushaAcceptanceTicketV1,
    pub(super) payment: KagemushaPaymentV1,
}

pub(super) fn incoming_payment_fixture(
    proof_tag: u8,
    ticket_key_seed: u8,
    eq_history_tag: u64,
    ep_history_tag: u64,
    eq_body_len: usize,
    ep_body_len: usize,
) -> IncomingPaymentFixtureV1 {
    let receiver_key = p256_signing_key(7);
    let governance_key = p256_signing_key(8);
    let receiver_public_key = device_public_key(&receiver_key);
    let network_id = network();
    let asset = asset();
    let asset_incarnation = incarnation();
    let mut credential = KagemushaHardwareCredentialV1 {
        version: KAGEMUSHA_WIRE_VERSION_V1,
        credential_id: [0; 32],
        network_id,
        hardware_profile_id: digest(0x21),
        suite_id: digest(0x22),
        firmware_policy_digest: digest(0x23),
        policy_epoch: 4,
        lane_commitment: digest(0x24),
        hardware_epoch_id: digest(0x25),
        hardware_epoch_generation: 1,
        device_public_key: receiver_public_key,
        device_key_reference: kagemusha_device_key_reference_v1(&receiver_public_key),
        issued_at_ms: 1,
        expires_at_ms: 20_000,
        governance_signature: sign(&governance_key, b"governance-placeholder"),
    }
    .seal_credential_id()
    .expect("credential identity");
    credential.governance_signature = sign(
        &governance_key,
        &credential
            .canonical_signing_bytes()
            .expect("credential signing bytes"),
    );
    let mut request = KagemushaPaymentRequestV1 {
        version: KAGEMUSHA_WIRE_VERSION_V1,
        release_id: digest(0x26),
        network_id,
        asset: asset.clone(),
        asset_incarnation,
        scale: 4,
        liability_pool_id: kagemusha_liability_pool_id_v1(&network_id, &asset, asset_incarnation)
            .expect("liability pool"),
        recipient: account(0x27),
        request_mode: KagemushaPaymentRequestModeV1::SingleExact(KagemushaSingleExactV1 {
            amount: 7,
        }),
        hardware_credential: credential,
        request_id: digest(0x28),
        issued_at_ms: 100,
        expires_at_ms: 10_000,
        signature: sign(&receiver_key, b"request-placeholder"),
    };
    request.signature = sign(
        &receiver_key,
        &request
            .canonical_signing_bytes()
            .expect("request signing bytes"),
    );
    request.validate_shape().expect("valid signed request");

    let intent = KagemushaAcceptanceIntentV1 {
        version: KAGEMUSHA_WIRE_VERSION_V1,
        request_digest: request.canonical_digest().expect("request digest"),
        intent_id: digest(0x31),
        exact_amount: 7,
        sender_one_time_commitment: digest(0x32),
    };
    let mut ticket_key = recipient_one_time_key();
    ticket_key[1] = ticket_key_seed;
    let mut ticket = KagemushaAcceptanceTicketV1 {
        version: KAGEMUSHA_WIRE_VERSION_V1,
        acceptance_ticket_id: digest(0x35),
        recipient_one_time_key: ticket_key,
        reserved_inbox_bytes: KAGEMUSHA_ACCEPTANCE_TICKET_MIN_RESERVED_INBOX_BYTES_V1,
        issued_at_ms: 200,
        expires_at_ms: 9_000,
        signature: sign(&receiver_key, b"ticket-placeholder"),
    };
    ticket.signature = sign(
        &receiver_key,
        &ticket
            .canonical_signing_bytes_against(&request, &intent)
            .expect("ticket signing bytes"),
    );
    ticket
        .validate_shape_against_intent(&request, &intent)
        .expect("valid ticket");

    let output = KagemushaPaymentOutputV1 {
        version: KAGEMUSHA_WIRE_VERSION_V1,
        acceptance_intent_digest: intent
            .canonical_digest_against(&request)
            .expect("intent digest"),
        acceptance_ticket_digest: ticket
            .canonical_digest_against_intent(&request, &intent)
            .expect("ticket digest"),
        transition_nullifier: digest(0x82),
        credit_id: [0; 32],
        ciphertext_commitment: digest(0x83),
        commit_evidence: KagemushaCommitEvidenceV1::TrustedTime(KagemushaTrustedCommitTimeV1 {
            time_evidence_commitment: digest(0x81),
        }),
    }
    .seal_credit_id_against(&request, &intent)
    .expect("credit identity");
    let encrypted_credit = KagemushaEncryptedCreditEnvelopeV1 {
        version: KAGEMUSHA_WIRE_VERSION_V1,
        ephemeral_x25519_public_key: recipient_one_time_key(),
        nonce: [0x84; KAGEMUSHA_XCHACHA20POLY1305_NONCE_BYTES_V1],
        ciphertext_and_tag: vec![
            0x85;
            kagemusha_credit_opening_canonical_len_v1()
                .expect("opening length")
                + KAGEMUSHA_XCHACHA20POLY1305_TAG_BYTES_V1
        ],
    }
    .canonical_bytes_against_recipient_key(ticket.recipient_one_time_key)
    .expect("encrypted credit envelope");
    // Deliberately structural fixtures, not a qualified hardware commit or real proof.
    let certificate = KagemushaCommitCertificateV1 {
        version: KAGEMUSHA_WIRE_VERSION_V1,
        certificate_id: [0; 32],
        candidate_envelope_digest: digest(0x91),
        lifecycle_binding_digest: digest(0x92),
        transition_nullifier: output.transition_nullifier,
        outbox_reservation_commitment: digest(0x93),
        commit_evidence: output.commit_evidence,
        hardware_profile_id: digest(0x94),
        policy_epoch: 6,
        hardware_terminal_commitment: digest(0x95),
    }
    .seal_certificate_id()
    .expect("certificate identity");
    let proof = KagemushaPaymentProofV1 {
        version: KAGEMUSHA_WIRE_VERSION_V1,
        eq_protocol_digest: artifacts().commit_wrapper_eq_protocol_digest,
        ep_protocol_digest: artifacts().commit_wrapper_ep_protocol_digest,
        semantic_digest: kagemusha_payment_body_digest_v1(&output, &encrypted_credit)
            .expect("payment body"),
        candidate_envelope_digest: certificate.candidate_envelope_digest,
        commit_certificate_digest: certificate.canonical_digest().expect("certificate digest"),
        eq_deferred_audit: eq_digest(u64::from(proof_tag)),
        ep_deferred_audit: ep_digest(u64::from(proof_tag) + 1),
        eq_proof: vec![proof_tag.wrapping_add(4); eq_body_len],
        ep_proof: vec![proof_tag.wrapping_add(5); ep_body_len],
        eq_history: eq_history(eq_history_tag).as_bytes().to_vec(),
        ep_history: ep_history(ep_history_tag).as_bytes().to_vec(),
    };
    let payment = KagemushaPaymentV1 {
        version: KAGEMUSHA_WIRE_VERSION_V1,
        output,
        encrypted_credit,
        commit_certificate: certificate,
        proof,
    };
    payment
        .validate_shape_against(&request, &intent, &ticket)
        .expect("valid payment shape");
    IncomingPaymentFixtureV1 {
        request,
        intent,
        ticket,
        payment,
    }
}

#[test]
fn commit_wrapper_roles_are_distinct_and_authenticated() {
    artifacts()
        .validate()
        .expect("distinct authorization relation artifacts");

    let mut aliased_protocol = artifacts();
    aliased_protocol.commit_wrapper_eq_protocol_digest =
        aliased_protocol.terminal_authorization_eq_protocol_digest;
    assert!(aliased_protocol.validate().is_err());

    let mut wrong_role = artifacts();
    wrong_role.commit_wrapper_verifying_key_eq.role =
        KagemushaArtifactRoleV1::TerminalAuthorizationVkEq;
    assert!(wrong_role.validate().is_err());

    let mut aliased_key = artifacts();
    aliased_key.commit_wrapper_verifying_key_ep.sha256 =
        aliased_key.terminal_authorization_verifying_key_ep.sha256;
    assert!(aliased_key.validate().is_err());
}

#[test]
fn mint_finality_protocol_accessor_returns_the_compiled_identity() {
    let artifacts = artifacts();
    artifacts
        .validate()
        .expect("valid release protocol identities");
    assert_eq!(
        artifacts.mint_finality_protocol_digest(KagemushaPastaParityV1::Eq),
        Ok(artifacts.mint_finality_eq_protocol_digest)
    );
    assert_eq!(
        artifacts.mint_finality_protocol_digest(KagemushaPastaParityV1::Ep),
        Ok(artifacts.mint_finality_ep_protocol_digest)
    );

    // Compiled identities are not synthesized from the manifest or artifact-byte hashes.
    // Authentication of those separate bindings still belongs to the release/artifact loader.
    let mut changed_metadata = artifacts;
    changed_metadata.profile_digest = digest(0x80);
    changed_metadata.mint_finality.verifying_key_eq.sha256 = digest(0x81);
    changed_metadata.mint_finality.verifying_key_ep.sha256 = digest(0x82);
    for parity in [KagemushaPastaParityV1::Eq, KagemushaPastaParityV1::Ep] {
        assert_eq!(
            changed_metadata.mint_finality_protocol_digest(parity),
            artifacts.mint_finality_protocol_digest(parity)
        );
    }
}

#[test]
fn mint_finality_protocol_constructor_uses_authenticated_helper_metadata() {
    // This is a source-wiring regression, not a signed-release or cryptographic proof test.
    let source = include_str!("mod.rs");
    let constructor = source
        .split("impl KagemushaRecursionArtifactsV1 {")
        .nth(1)
        .expect("artifact implementation")
        .split("fn validate(self)")
        .next()
        .expect("artifact constructor");
    assert!(
        constructor.contains(".helper_protocol(KagemushaQualifiedHelperCircuitV1::MintCredit)")
    );
    assert!(
        constructor.contains("mint_finality_eq_protocol_digest: mint_finality.eq_protocol_digest")
    );
    assert!(
        constructor.contains("mint_finality_ep_protocol_digest: mint_finality.ep_protocol_digest")
    );
    assert!(!source.contains("fn helper_protocol_digest("));
    assert!(!source.contains("struct HelperProtocolDigestPreimageV1"));
}

#[test]
fn mint_finality_protocols_reject_noncanonical_and_aliased_identities() {
    let valid = artifacts();
    let other_protocols = [
        valid.eq_protocol_digest,
        valid.ep_protocol_digest,
        valid.terminal_authorization_eq_protocol_digest,
        valid.terminal_authorization_ep_protocol_digest,
        valid.commit_wrapper_eq_protocol_digest,
        valid.commit_wrapper_ep_protocol_digest,
        valid.mint_authorization_eq_protocol_digest,
        valid.mint_authorization_ep_protocol_digest,
        valid.guard_bundle_eq_protocol_digest,
        valid.guard_bundle_ep_protocol_digest,
    ];
    for parity in [KagemushaPastaParityV1::Eq, KagemushaPastaParityV1::Ep] {
        let opposite = match parity {
            KagemushaPastaParityV1::Eq => valid.mint_finality_ep_protocol_digest,
            KagemushaPastaParityV1::Ep => valid.mint_finality_eq_protocol_digest,
        };
        for replacement in [[0; 32], [0xFF; 32], opposite]
            .into_iter()
            .chain(other_protocols)
        {
            let mut mutated = valid;
            match parity {
                KagemushaPastaParityV1::Eq => {
                    mutated.mint_finality_eq_protocol_digest = replacement;
                }
                KagemushaPastaParityV1::Ep => {
                    mutated.mint_finality_ep_protocol_digest = replacement;
                }
            }
            assert_eq!(
                mutated.validate(),
                Err(KagemushaRecursionErrorV1::InvalidArtifacts)
            );
        }
    }
}

fn send_output() -> KagemushaRecursivePublicOutputV1 {
    let network_id = network();
    let asset = asset();
    let asset_incarnation = incarnation();
    let lifecycle = KagemushaLifecycleBindingV1 {
        version: KAGEMUSHA_WIRE_VERSION_V1,
        network_id,
        protocol_version: KAGEMUSHA_WIRE_VERSION_V1,
        suite_id: digest(0x60),
        vk_digest: digest(0x61),
        release_id: artifacts().release_id,
        asset: asset.clone(),
        asset_incarnation,
        scale: 4,
        liability_pool_id: kagemusha_liability_pool_id_v1(&network_id, &asset, asset_incarnation)
            .expect("liability pool"),
        hardware_profile_id: digest(0x62),
        policy_epoch: 1,
        operation_kind: KagemushaOperationKindV1::SendSplit,
        request_id: digest(0x63),
        acceptance_ticket_id: digest(0x64),
        credit_id: digest(0x65),
        ciphertext_digest: digest(0x66),
    };
    KagemushaRecursivePublicOutputV1::new(
        lifecycle,
        digest(0x67),
        digest(0x68),
        digest(0x69),
        digest(0x6A),
        digest(0x6B),
        digest(0x6C),
        digest(0x6D),
        75,
        digest(0x6E),
    )
    .expect("valid unlinkable send output")
}

/// Framing-only compact mint fixture; its proof bytes and histories are not cryptographic evidence.
pub(super) fn compact_mint_credit_fixture() -> KagemushaMintCreditV1 {
    let encrypted_credit = KagemushaEncryptedCreditEnvelopeV1 {
        version: KAGEMUSHA_WIRE_VERSION_V1,
        ephemeral_x25519_public_key: recipient_one_time_key(),
        nonce: [0x84; KAGEMUSHA_XCHACHA20POLY1305_NONCE_BYTES_V1],
        ciphertext_and_tag: vec![
            0x85;
            kagemusha_credit_opening_canonical_len_v1()
                .expect("opening length")
                + KAGEMUSHA_XCHACHA20POLY1305_TAG_BYTES_V1
        ],
    }
    .canonical_bytes_against_recipient_key(recipient_one_time_key())
    .expect("encrypted mint framing");
    let mut lifecycle = send_output().lifecycle;
    lifecycle.operation_kind = KagemushaOperationKindV1::MintFold;
    lifecycle.request_id = [0; 32];
    lifecycle.acceptance_ticket_id = [0; 32];
    lifecycle.credit_id = [0; 32];
    lifecycle.ciphertext_digest =
        iroha_data_model::kagemusha::kagemusha_ciphertext_digest_v1(&encrypted_credit);
    let statement = KagemushaMintCreditStatementV1 {
        version: KAGEMUSHA_WIRE_VERSION_V1,
        lifecycle,
        recipient_credential_commitment: digest(0xA1),
        authorization_context_digest: digest(0xA2),
        mint_authorization_digest: digest(0xA3),
        amount: 7,
        issuance_commitment: digest(0xA4),
        recipient: account(0xA5),
        credit_commitment: digest(0xA6),
        minted_at_ms: 40_000,
    }
    .seal_credit_id()
    .expect("sealed mint identity");
    let proof = KagemushaPairedProofV1 {
        version: KAGEMUSHA_WIRE_VERSION_V1,
        eq_protocol_digest: artifacts().mint_finality_eq_protocol_digest,
        ep_protocol_digest: artifacts().mint_finality_ep_protocol_digest,
        semantic_digest: statement.canonical_digest().expect("mint digest"),
        guard_eq_credential_audit: digest(0xA7),
        guard_ep_credential_audit: digest(0xA8),
        eq_deferred_audit: eq_digest(0xA9),
        ep_deferred_audit: ep_digest(0xAA),
        eq_proof: vec![0xAB; 32],
        ep_proof: vec![0xAC; 32],
        eq_history: eq_history(3).as_bytes().to_vec(),
        ep_history: ep_history(5).as_bytes().to_vec(),
    };
    let credit = KagemushaMintCreditV1 {
        version: KAGEMUSHA_WIRE_VERSION_V1,
        statement,
        finality_certificate_binding: proof.guard_eq_credential_audit,
        finality_authority_head: proof.guard_ep_credential_audit,
        finality_genesis_roster_id: digest(0xAD),
        finality_proof_binding_digest: digest(0xAE),
        proof,
        encrypted_credit,
        artifact_manifest_digest: artifacts().artifact_manifest_digest,
    };
    credit.validate_shape().expect("compact mint framing");
    credit
}

/// Project exact framed mint values for native-boundary tests, without verifying their proofs.
pub(super) fn compact_mint_request(
    credit: &KagemushaMintCreditV1,
) -> KagemushaMintFinalityHelperVerificationRequestV1<'_> {
    KagemushaMintFinalityHelperVerificationRequestV1 {
        eq_protocol_digest: credit.proof.eq_protocol_digest,
        ep_protocol_digest: credit.proof.ep_protocol_digest,
        statement: &credit.statement,
        semantic_digest: credit.proof.semantic_digest,
        proof: &credit.proof,
        finality_certificate_binding: credit.finality_certificate_binding,
        finality_authority_head: credit.finality_authority_head,
        finality_genesis_roster_id: credit.finality_genesis_roster_id,
        finality_proof_binding_digest: credit.finality_proof_binding_digest,
        artifact_manifest_digest: credit.artifact_manifest_digest,
    }
}

/// Test-only backend which checks dispatch bindings; it is not a cryptographic verifier.
struct MintFixtureVerifier {
    expected: KagemushaMintCreditV1,
    reject: bool,
    calls: RefCell<usize>,
}

impl KagemushaRecursiveVerifierV1 for MintFixtureVerifier {
    fn verify_state_proof_and_decide(
        &self,
        _request: &KagemushaStateProofVerificationRequestV1<'_>,
    ) -> Result<(), String> {
        Err("mint fixture has no state proof".to_owned())
    }

    fn verify_payment_and_decide(
        &self,
        _request: &KagemushaPaymentRequestV1,
        _intent: &KagemushaAcceptanceIntentV1,
        _ticket: &KagemushaAcceptanceTicketV1,
        _payment: &KagemushaPaymentV1,
    ) -> Result<(), String> {
        Err("mint fixture has no payment proof".to_owned())
    }

    fn verify_terminal_authorization_and_decide(
        &self,
        _request: &KagemushaParityVerificationRequestV1<'_>,
    ) -> Result<(), String> {
        Err("mint fixture has no terminal proof".to_owned())
    }

    fn verify_mint_finality_helper(
        &self,
        request: &KagemushaMintFinalityHelperVerificationRequestV1<'_>,
    ) -> Result<(), String> {
        *self.calls.borrow_mut() += 1;
        let expected = compact_mint_request(&self.expected);
        if self.reject
            || request.statement != expected.statement
            || request.proof != expected.proof
            || request.eq_protocol_digest != expected.eq_protocol_digest
            || request.ep_protocol_digest != expected.ep_protocol_digest
            || request.semantic_digest != expected.semantic_digest
            || request.finality_certificate_binding != expected.finality_certificate_binding
            || request.finality_authority_head != expected.finality_authority_head
            || request.finality_genesis_roster_id != expected.finality_genesis_roster_id
            || request.finality_proof_binding_digest != expected.finality_proof_binding_digest
            || request.artifact_manifest_digest != expected.artifact_manifest_digest
        {
            return Err("mint fixture backend rejected".to_owned());
        }
        Ok(())
    }
}

#[test]
fn compact_mint_inner_commitment_requires_backend_acceptance_and_exact_dispatch() {
    let credit = compact_mint_credit_fixture();
    let mut verifier = MintFixtureVerifier {
        expected: credit.clone(),
        reject: false,
        calls: RefCell::new(0),
    };
    let verified = verify_kagemusha_mint_finality_helper_v1(&verifier, artifacts(), &credit)
        .expect("explicit fixture backend accepted exact dispatch");
    assert_eq!(
        verified.proof_binding_digest(),
        credit.finality_proof_binding_digest
    );
    assert_eq!(verified.semantic_digest(), credit.proof.semantic_digest);
    assert_eq!(*verifier.calls.borrow(), 1);

    verifier.reject = true;
    assert!(matches!(
        verify_kagemusha_mint_finality_helper_v1(&verifier, artifacts(), &credit),
        Err(KagemushaRecursionErrorV1::MintFinalityProofRejected(_))
    ));
    assert_eq!(
        *verifier.calls.borrow(),
        2,
        "a nonzero commitment never skips the backend"
    );
    verifier.reject = false;
    for index in 0..7 {
        let mut spliced = credit.clone();
        match index {
            0 => spliced.finality_proof_binding_digest = digest(0xAF),
            1 => spliced.proof.eq_proof[0] ^= 1,
            2 => spliced.proof.ep_proof[0] ^= 1,
            3 => spliced.proof.eq_deferred_audit = eq_digest(0xB0),
            4 => spliced.proof.ep_deferred_audit = ep_digest(0xB1),
            5 => spliced.proof.eq_history = eq_history(7).as_bytes().to_vec(),
            _ => spliced.proof.ep_history = ep_history(9).as_bytes().to_vec(),
        }
        assert!(matches!(
            verify_kagemusha_mint_finality_helper_v1(&verifier, artifacts(), &spliced),
            Err(KagemushaRecursionErrorV1::MintFinalityProofRejected(_))
        ));
    }
    assert_eq!(*verifier.calls.borrow(), 9);
}

#[test]
fn compact_mint_release_and_malformed_history_fail_before_backend_dispatch() {
    let credit = compact_mint_credit_fixture();
    let verifier = MintFixtureVerifier {
        expected: credit.clone(),
        reject: false,
        calls: RefCell::new(0),
    };
    for index in 0..9 {
        let mut changed = credit.clone();
        match index {
            0 => {
                changed.statement.lifecycle.release_id = digest(0xB2);
                changed.statement = changed
                    .statement
                    .seal_credit_id()
                    .expect("changed release ID");
                changed.proof.semantic_digest = changed.statement.canonical_digest().unwrap();
            }
            1 => changed.artifact_manifest_digest = digest(0xB3),
            2 => changed.proof.eq_protocol_digest = eq_digest(0xB4),
            3 => changed.proof.ep_protocol_digest = ep_digest(0xB5),
            4 => changed.finality_proof_binding_digest = [0; 32],
            5 => changed.proof.eq_history[0..32].fill(0xFF),
            6 => changed.proof.ep_history[0..32].fill(0xFF),
            7 => changed.finality_certificate_binding = digest(0xB6),
            _ => changed.finality_authority_head = digest(0xB7),
        }
        assert!(
            verify_kagemusha_mint_finality_helper_v1(&verifier, artifacts(), &changed).is_err()
        );
    }
    assert_eq!(*verifier.calls.borrow(), 0);
}

fn terminal_authorization_proof(
    output: &KagemushaRecursivePublicOutputV1,
    eq_body_len: usize,
    ep_body_len: usize,
) -> KagemushaRedemptionProofV1 {
    KagemushaRedemptionProofV1 {
        version: KAGEMUSHA_WIRE_VERSION_V1,
        eq_protocol_digest: artifacts().commit_wrapper_eq_protocol_digest,
        ep_protocol_digest: artifacts().commit_wrapper_ep_protocol_digest,
        semantic_digest: output.semantic_digest,
        candidate_envelope_digest: output.candidate_envelope_digest,
        commit_certificate_digest: output.commit_certificate_digest,
        eq_deferred_audit: eq_digest(0x71),
        ep_deferred_audit: ep_digest(0x72),
        eq_proof: vec![0xA1; eq_body_len],
        ep_proof: vec![0xB2; ep_body_len],
        eq_history: eq_history(7).as_bytes().to_vec(),
        ep_history: ep_history(11).as_bytes().to_vec(),
    }
}

/// Exact test-only checker; production has no accepting non-cryptographic backend.
struct ExactFixtureVerifier {
    expected_output: KagemushaRecursivePublicOutputV1,
    expected_proof: KagemushaRedemptionProofV1,
    calls: RefCell<Vec<(KagemushaPastaParityV1, usize, usize)>>,
}

impl ExactFixtureVerifier {
    fn new(
        expected_output: KagemushaRecursivePublicOutputV1,
        expected_proof: KagemushaRedemptionProofV1,
    ) -> Self {
        Self {
            expected_output,
            expected_proof,
            calls: RefCell::new(Vec::new()),
        }
    }
}

impl KagemushaRecursiveVerifierV1 for ExactFixtureVerifier {
    fn verify_state_proof_and_decide(
        &self,
        _request: &KagemushaStateProofVerificationRequestV1<'_>,
    ) -> Result<(), String> {
        Err("fixture has no state proof".to_owned())
    }

    fn verify_payment_and_decide(
        &self,
        _request: &KagemushaPaymentRequestV1,
        _intent: &KagemushaAcceptanceIntentV1,
        _ticket: &KagemushaAcceptanceTicketV1,
        _payment: &KagemushaPaymentV1,
    ) -> Result<(), String> {
        Err("fixture has no acceptance-intent authorization".to_owned())
    }

    fn verify_mint_finality_helper(
        &self,
        _request: &KagemushaMintFinalityHelperVerificationRequestV1<'_>,
    ) -> Result<(), String> {
        Err("fixture has no mint proof".to_owned())
    }

    fn verify_terminal_authorization_and_decide(
        &self,
        request: &KagemushaParityVerificationRequestV1<'_>,
    ) -> Result<(), String> {
        let (protocol, body, history) = match request.parity {
            KagemushaPastaParityV1::Eq => (
                self.expected_proof.eq_protocol_digest,
                self.expected_proof.eq_proof.as_slice(),
                self.expected_proof.eq_history.as_slice(),
            ),
            KagemushaPastaParityV1::Ep => (
                self.expected_proof.ep_protocol_digest,
                self.expected_proof.ep_proof.as_slice(),
                self.expected_proof.ep_history.as_slice(),
            ),
        };
        if request.public_output != &self.expected_output
            || request.protocol_digest != protocol
            || request.eq_deferred_audit != self.expected_proof.eq_deferred_audit
            || request.ep_deferred_audit != self.expected_proof.ep_deferred_audit
            || request.current_proof != body
            || request.history_accumulator.as_slice() != history
        {
            return Err("fixture substitution".to_owned());
        }
        self.calls.borrow_mut().push((
            request.parity,
            request.current_proof.len(),
            request.history_accumulator.len(),
        ));
        Ok(())
    }
}

/// Test-only backend proving that state-envelope rejection happens before cryptographic dispatch.
#[derive(Default)]
struct StateFixtureVerifier {
    calls: RefCell<usize>,
}

impl KagemushaRecursiveVerifierV1 for StateFixtureVerifier {
    fn verify_state_proof_and_decide(
        &self,
        _request: &KagemushaStateProofVerificationRequestV1<'_>,
    ) -> Result<(), String> {
        *self.calls.borrow_mut() += 1;
        Ok(())
    }

    fn verify_payment_and_decide(
        &self,
        _request: &KagemushaPaymentRequestV1,
        _intent: &KagemushaAcceptanceIntentV1,
        _ticket: &KagemushaAcceptanceTicketV1,
        _payment: &KagemushaPaymentV1,
    ) -> Result<(), String> {
        Err("state fixture has no acceptance-intent authorization".to_owned())
    }

    fn verify_mint_finality_helper(
        &self,
        _request: &KagemushaMintFinalityHelperVerificationRequestV1<'_>,
    ) -> Result<(), String> {
        Err("state fixture has no mint proof".to_owned())
    }

    fn verify_terminal_authorization_and_decide(
        &self,
        _request: &KagemushaParityVerificationRequestV1<'_>,
    ) -> Result<(), String> {
        Err("state fixture has no terminal authorization".to_owned())
    }
}

fn state_verification_fixture() -> (KagemushaStateRelationPublicInputsV1, KagemushaPairedProofV1) {
    let artifacts = artifacts();
    let network_id = network();
    let asset = asset();
    let asset_incarnation = incarnation();
    let successor = KagemushaStateV1 {
        version: KAGEMUSHA_STATE_VERSION_V1,
        protocol_version: KAGEMUSHA_STATE_VERSION_V1,
        suite_id: digest(0x91),
        vk_digest: digest(0x92),
        release_id: artifacts.release_id,
        asset_incarnation,
        liability_pool_id: kagemusha_liability_pool_id_v1(&network_id, &asset, asset_incarnation)
            .expect("liability pool"),
        hardware_profile_id: digest(0x93),
        policy_epoch: 1,
        lane: KagemushaLaneIdV1 {
            network_id,
            device_lane_id: digest(0x94),
            asset,
            scale: 4,
        },
        balance: 0,
        logical_sequence: 0,
        hardware_epoch: HardwareEpochV1 {
            generation: 1,
            epoch_id: digest(0x95),
        },
        device_policy_binding: DevicePolicyBindingV1 {
            device_key_reference: digest(0x96),
            hardware_policy_id: digest(0x97),
        },
        state_nonce_commitment: digest(0x98),
        consumed_credit_root: pasta_pair(0x99),
        state_commitment_components: pasta_pair(0x9B),
        state_commitment: digest(0x9D),
    };
    let transport_semantic_digest = digest(0x9E);
    let guard_eq_credential_audit = eq_digest(0x31);
    let guard_ep_credential_audit = ep_digest(0x32);
    let eq_deferred_audit = eq_digest(0x33);
    let ep_deferred_audit = ep_digest(0x34);
    let public = KagemushaStateRelationPublicInputsV1 {
        operation: KagemushaOperationV1::Bootstrap,
        predecessor: None,
        successor,
        amount: 0,
        journal_revision_before: 0,
        journal_revision_after: 0,
        transition_effect_digest: digest(0x9F),
        mint_finality_semantic_digest: [0; 32],
        mint_finality_proof_binding_digest: [0; 32],
        peer_credit_id: [0; 32],
        peer_recipient_lane_id: [0; 32],
        receive_active_count: 0,
        receive_credit_binding_digest: [0; 32],
        lifecycle_binding_digest: digest(0xA0),
        precommit_binding_digest: [0; 32],
        suite_upgrade_authorization_digest: [0; 32],
        transport_semantic_digest,
        guard_statement_digest: digest(0xA1),
        eq_protocol_digest: artifacts.eq_protocol_digest,
        ep_protocol_digest: artifacts.ep_protocol_digest,
        guard_eq_protocol_digest: artifacts.guard_bundle_eq_protocol_digest,
        guard_ep_protocol_digest: artifacts.guard_bundle_ep_protocol_digest,
        mint_eq_protocol_digest: artifacts
            .mint_finality_protocol_digest(KagemushaPastaParityV1::Eq)
            .expect("Eq mint protocol"),
        mint_ep_protocol_digest: artifacts
            .mint_finality_protocol_digest(KagemushaPastaParityV1::Ep)
            .expect("Ep mint protocol"),
        commit_wrapper_eq_protocol_digest: artifacts.commit_wrapper_eq_protocol_digest,
        commit_wrapper_ep_protocol_digest: artifacts.commit_wrapper_ep_protocol_digest,
        guard_eq_credential_audit,
        guard_ep_credential_audit,
        eq_deferred_audit,
        ep_deferred_audit,
    };
    let proof = KagemushaPairedProofV1 {
        version: KAGEMUSHA_WIRE_VERSION_V1,
        eq_protocol_digest: artifacts.eq_protocol_digest,
        ep_protocol_digest: artifacts.ep_protocol_digest,
        semantic_digest: transport_semantic_digest,
        guard_eq_credential_audit,
        guard_ep_credential_audit,
        eq_deferred_audit,
        ep_deferred_audit,
        eq_proof: vec![0xA2],
        ep_proof: vec![0xA3],
        eq_history: eq_history(0x35).as_bytes().to_vec(),
        ep_history: ep_history(0x36).as_bytes().to_vec(),
    };
    (public, proof)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct AggregateState {
    balance: u128,
    sequence: u128,
    epoch: u128,
    replay_root: KagemushaPastaStateCommitmentV1,
}

impl AggregateState {
    fn empty(root_seed: u64) -> Self {
        Self {
            balance: 0,
            sequence: 0,
            epoch: 1,
            replay_root: pasta_pair(root_seed),
        }
    }

    fn receive(&mut self, amount: u128, next_root: u64) -> KagemushaOperationRelationWitnessV1 {
        let before = *self;
        self.balance = self.balance.checked_add(amount).expect("u128 balance");
        self.sequence = self.sequence.checked_add(1).expect("u128 sequence");
        self.replay_root = pasta_pair(next_root);
        operation_witness(
            before,
            *self,
            KagemushaOperationV1::ReceiveFold,
            amount,
        )
    }

    fn spend(
        &mut self,
        operation: KagemushaOperationV1,
        amount: u128,
    ) -> KagemushaOperationRelationWitnessV1 {
        assert!(matches!(
            operation,
            KagemushaOperationV1::SendSplit | KagemushaOperationV1::RedeemSplit
        ));
        let before = *self;
        self.balance = self
            .balance
            .checked_sub(amount)
            .expect("sufficient balance");
        self.sequence = self.sequence.checked_add(1).expect("u128 sequence");
        operation_witness(before, *self, operation, amount)
    }
}

fn operation_witness(
    before: AggregateState,
    after: AggregateState,
    operation: KagemushaOperationV1,
    amount: u128,
) -> KagemushaOperationRelationWitnessV1 {
    KagemushaOperationRelationWitnessV1 {
        operation,
        balance_before: before.balance,
        balance_after: after.balance,
        amount,
        logical_sequence_before: before.sequence,
        logical_sequence_after: after.sequence,
        hardware_epoch_before: before.epoch,
        hardware_epoch_after: after.epoch,
        replay_root_before: before.replay_root,
        replay_root_after: after.replay_root,
    }
}

fn operation_tag(operation: KagemushaOperationV1) -> u64 {
    match operation {
        KagemushaOperationV1::Bootstrap => 0,
        KagemushaOperationV1::MintFold => 1,
        KagemushaOperationV1::SendSplit => 2,
        KagemushaOperationV1::ReceiveFold => 3,
        KagemushaOperationV1::RedeemSplit => 4,
        KagemushaOperationV1::SuiteUpgrade => 5,
        KagemushaOperationV1::Rotate => 6,
    }
}

#[test]
fn suite_upgrade_authorization_binds_both_authenticated_releases() {
    let binding = canonical_suite_upgrade_authorization_digest_v1(
        digest(0x10),
        digest(0x11),
        [eq_digest(12), ep_digest(13)],
        digest(0x14),
        digest(0x15),
        digest(0x16),
        digest(0x17),
        digest(0x18),
    );
    let changed_predecessor = canonical_suite_upgrade_authorization_digest_v1(
        digest(0x19),
        digest(0x11),
        [eq_digest(12), ep_digest(13)],
        digest(0x14),
        digest(0x15),
        digest(0x16),
        digest(0x17),
        digest(0x18),
    );
    let changed_successor = canonical_suite_upgrade_authorization_digest_v1(
        digest(0x10),
        digest(0x1a),
        [eq_digest(12), ep_digest(13)],
        digest(0x14),
        digest(0x15),
        digest(0x16),
        digest(0x17),
        digest(0x18),
    );
    assert_ne!(binding, changed_predecessor);
    assert_ne!(binding, changed_successor);
    assert_ne!(changed_predecessor, changed_successor);
}

fn assert_paired_operation_relation(witness: KagemushaOperationRelationWitnessV1) {
    let tag = operation_tag(witness.operation);
    MockProver::run(
        12,
        &KagemushaOperationRelationCircuitV1::<Fp>::new(witness),
        vec![vec![Fp::from(tag)]],
    )
    .expect("Eq relation synthesizes")
    .assert_satisfied();
    MockProver::run(
        12,
        &KagemushaOperationRelationCircuitV1::<Fq>::new(witness),
        vec![vec![Fq::from(tag)]],
    )
    .expect("Ep relation synthesizes")
    .assert_satisfied();
}

#[test]
fn accumulators_are_exactly_544_bytes_and_strictly_canonical() {
    let eq = eq_history(1);
    let ep = ep_history(2);
    assert_eq!(eq.as_bytes().len(), KAGEMUSHA_HISTORY_ACCUMULATOR_BYTES_V1);
    assert_eq!(ep.as_bytes().len(), KAGEMUSHA_HISTORY_ACCUMULATOR_BYTES_V1);
    assert_eq!(
        KagemushaEqAccumulatorV1::try_from_bytes(eq.as_bytes()).unwrap(),
        eq
    );
    assert_eq!(
        KagemushaEpAccumulatorV1::try_from_bytes(ep.as_bytes()).unwrap(),
        ep
    );

    assert!(matches!(
        KagemushaEqAccumulatorV1::try_from_bytes(&eq.as_bytes()[..543]),
        Err(KagemushaRecursionErrorV1::InvalidAccumulatorLength { actual: 543, .. })
    ));
    let mut noncanonical = *eq.as_bytes();
    noncanonical[..32].fill(0xFF);
    assert!(matches!(
        KagemushaEqAccumulatorV1::try_from_bytes(&noncanonical),
        Err(KagemushaRecursionErrorV1::NonCanonicalAccumulatorScalar {
            parity: KagemushaPastaParityV1::Eq,
            round: 0,
        })
    ));
    let mut identity = *ep.as_bytes();
    identity[512..].copy_from_slice(EpAffine::identity().to_bytes().as_ref());
    assert!(matches!(
        KagemushaEpAccumulatorV1::try_from_bytes(&identity),
        Err(KagemushaRecursionErrorV1::InvalidAccumulatorPoint(
            KagemushaPastaParityV1::Ep
        ))
    ));
}

#[test]
fn fold_transcripts_have_one_fixed_shape() {
    assert_eq!(KAGEMUSHA_IPA_FOLD_PROOF_BYTES_V1, 1_280);
    assert!(KagemushaEqFoldProofV1::try_from_bytes(&vec![0; 1_280]).is_ok());
    assert!(KagemushaEpFoldProofV1::try_from_bytes(&vec![0; 1_280]).is_ok());
    assert!(KagemushaEqFoldProofV1::try_from_bytes(&vec![0; 1_279]).is_err());
    assert!(KagemushaEpFoldProofV1::try_from_bytes(&vec![0; 1_281]).is_err());
}

#[test]
fn one_thousand_receipts_collapse_into_one_unrestricted_balance() {
    let mut merchant = AggregateState::empty(10_000);
    let mut first = None;
    let mut last = None;
    for receipt in 0_u64..1_000 {
        let witness = merchant.receive(1, 10_001 + receipt);
        first.get_or_insert(witness);
        last = Some(witness);
    }
    assert_eq!((merchant.balance, merchant.sequence), (1_000, 1_000));
    assert_paired_operation_relation(first.expect("first receipt"));
    assert_paired_operation_relation(last.expect("last receipt"));

    assert_paired_operation_relation(merchant.spend(KagemushaOperationV1::SendSplit, 1_000));
    assert_eq!(merchant.balance, 0);

    let mut recipient = AggregateState::empty(20_000);
    assert_paired_operation_relation(recipient.receive(1_000, 20_001));
    assert_paired_operation_relation(recipient.spend(KagemushaOperationV1::RedeemSplit, 400));
    assert_paired_operation_relation(recipient.spend(KagemushaOperationV1::SendSplit, 600));
    assert_eq!(recipient.balance, 0);
}

#[test]
fn one_thousand_twenty_four_handoffs_keep_fixed_public_and_wire_shapes() {
    const STATE_PUBLIC_INSTANCE_COUNT: usize =
        kagemusha_state_public_instance_v1::COMMIT_WRAPPER_EP_PROTOCOL_HI + 1;
    const RECURSIVE_PUBLIC_INSTANCE_COUNT: usize =
        STATE_PUBLIC_INSTANCE_COUNT + KAGEMUSHA_HISTORY_ACCUMULATOR_BYTES_V1 / 16;
    assert_eq!(STATE_PUBLIC_INSTANCE_COUNT, 85);
    assert_eq!(RECURSIVE_PUBLIC_INSTANCE_COUNT, 119);

    let output = send_output();
    let reference = terminal_authorization_proof(&output, 256, 256);
    let reference_len = norito::encode_canonical(&reference)
        .expect("proof encoding")
        .len();
    let mut holder = AggregateState::empty(30_000);
    holder.receive(1, 30_001);

    for depth in 1_u64..=1_024 {
        let send = holder.spend(KagemushaOperationV1::SendSplit, 1);
        let mut receiver = AggregateState::empty(40_000 + depth * 2);
        let receive = receiver.receive(1, 40_001 + depth * 2);
        if matches!(depth, 8 | 64 | 1_024) {
            assert_paired_operation_relation(send);
            assert_paired_operation_relation(receive);
            let mut depth_proof = reference.clone();
            depth_proof.eq_history = eq_history(depth).as_bytes().to_vec();
            depth_proof.ep_history = ep_history(depth).as_bytes().to_vec();
            assert_eq!(
                norito::encode_canonical(&depth_proof)
                    .expect("depth proof encoding")
                    .len(),
                reference_len,
                "proof wire size changed at depth {depth}",
            );
        }
        holder = receiver;
    }
    assert_eq!(holder.balance, 1);
}

#[test]
fn state_verification_pins_the_release_m2_protocol_pair_before_backend_dispatch() {
    let (public, proof) = state_verification_fixture();
    let verifier = StateFixtureVerifier::default();
    verify_kagemusha_state_proof_v1(&verifier, artifacts(), &public, &proof)
        .expect("release-pinned state projection verifies");
    assert_eq!(*verifier.calls.borrow(), 1);

    let mut substituted_eq = public.clone();
    substituted_eq.commit_wrapper_eq_protocol_digest = eq_digest(0x71);
    assert_eq!(
        verify_kagemusha_state_proof_v1(&verifier, artifacts(), &substituted_eq, &proof),
        Err(KagemushaRecursionErrorV1::ArtifactSubstitution)
    );

    let mut substituted_ep = public.clone();
    substituted_ep.commit_wrapper_ep_protocol_digest = ep_digest(0x72);
    assert_eq!(
        verify_kagemusha_state_proof_v1(&verifier, artifacts(), &substituted_ep, &proof),
        Err(KagemushaRecursionErrorV1::ArtifactSubstitution)
    );

    let mut swapped = public;
    core::mem::swap(
        &mut swapped.commit_wrapper_eq_protocol_digest,
        &mut swapped.commit_wrapper_ep_protocol_digest,
    );
    assert_eq!(
        verify_kagemusha_state_proof_v1(&verifier, artifacts(), &swapped, &proof),
        Err(KagemushaRecursionErrorV1::ArtifactSubstitution)
    );
    assert_eq!(
        *verifier.calls.borrow(),
        1,
        "all M2 substitutions must fail before the cryptographic backend is called"
    );
}

#[test]
fn state_verification_pins_the_compiled_mint_protocol_pair_before_backend_dispatch() {
    let (public, proof) = state_verification_fixture();
    let verifier = StateFixtureVerifier::default();
    let artifacts = artifacts();
    assert_eq!(
        public.mint_eq_protocol_digest,
        artifacts.mint_finality_eq_protocol_digest
    );
    assert_eq!(
        public.mint_ep_protocol_digest,
        artifacts.mint_finality_ep_protocol_digest
    );
    verify_kagemusha_state_proof_v1(&verifier, artifacts, &public, &proof)
        .expect("compiled mint protocol identities reach the state verifier");

    let mut substituted_eq = public.clone();
    substituted_eq.mint_eq_protocol_digest = eq_digest(0x71);
    let mut substituted_ep = public.clone();
    substituted_ep.mint_ep_protocol_digest = ep_digest(0x72);
    let mut swapped = public;
    core::mem::swap(
        &mut swapped.mint_eq_protocol_digest,
        &mut swapped.mint_ep_protocol_digest,
    );
    for mutated in [substituted_eq, substituted_ep, swapped] {
        assert_eq!(
            verify_kagemusha_state_proof_v1(&verifier, artifacts, &mutated, &proof),
            Err(KagemushaRecursionErrorV1::ArtifactSubstitution)
        );
    }
    assert_eq!(
        *verifier.calls.borrow(),
        1,
        "mint substitutions must not reach the backend"
    );
}

#[test]
fn recursive_key_dependency_dag_and_ancestry_binding_are_explicit() {
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum Stage {
        SeedM2,
        State,
        TerminalAuthorization,
        FinalM2,
        StateProof,
    }

    // Key generation has only these value dependencies. Final M2 is a State proof witness,
    // never a State-key input; its value-free shape is fixed by SeedM2.
    let order = [
        Stage::SeedM2,
        Stage::State,
        Stage::TerminalAuthorization,
        Stage::FinalM2,
        Stage::StateProof,
    ];
    let key_dependencies = [
        (Stage::SeedM2, Stage::State),
        (Stage::State, Stage::TerminalAuthorization),
        (Stage::TerminalAuthorization, Stage::FinalM2),
    ];
    for (predecessor, successor) in key_dependencies {
        let predecessor_index = order
            .iter()
            .position(|stage| *stage == predecessor)
            .expect("dependency predecessor is in the key order");
        let successor_index = order
            .iter()
            .position(|stage| *stage == successor)
            .expect("dependency successor is in the key order");
        assert!(predecessor_index < successor_index);
    }
    assert!(!key_dependencies.contains(&(Stage::FinalM2, Stage::State)));
    assert_eq!(
        kagemusha_terminal_authorization_public_instance_v1::HISTORY_START,
        47
    );
    assert_eq!(TERMINAL_AUTHORIZATION_PUBLIC_INSTANCE_COUNT_V1, 81);
    assert_eq!(state_relation::PUBLIC_INSTANCE_COUNT, 85);
    assert_eq!(
        kagemusha_state_public_instance_v1::COMMIT_WRAPPER_EQ_PROTOCOL_LO,
        81
    );
    assert_eq!(
        kagemusha_state_public_instance_v1::COMMIT_WRAPPER_EP_PROTOCOL_HI,
        84
    );

    // This precise structural guard is intentionally cheap. The expensive real M2 corridor is
    // the end-to-end non-brittle proof; this catches accidental reintroduction of the old
    // State -> M2 value edge or removal of inherited M2 cells before key generation begins.
    let composite_source = include_str!("composite.rs");
    let state_builder = composite_source
        .split_once("fn build_scalar_half")
        .expect("state scalar builder exists")
        .1
        .split_once("fn assign_history_limbs")
        .expect("state scalar builder boundary exists")
        .0;
    assert!(state_builder.contains("kagemusha_protocol_structure_digest_v1("));
    assert!(state_builder.contains("load_and_constrain_parent_protocol_v1("));
    assert!(!state_builder.contains("incoming_protocol.loaded("));

    let parent_source = include_str!("deferred_parent.rs");
    let parent_binding = parent_source
        .split_once("fn constrain_parent_and_history_into_loader_v1")
        .expect("parent folding function exists")
        .1
        .split_once("/// Select one complete IPA accumulator")
        .expect("parent folding function boundary exists")
        .0;
    for required in [
        "COMMIT_WRAPPER_EQ_PROTOCOL_LO",
        "COMMIT_WRAPPER_EP_PROTOCOL_LO",
        "expected_acceptance_protocols[0]",
        "expected_acceptance_protocols[1]",
        "mul(ctx.main(), difference, enabled)",
    ] {
        assert!(
            parent_binding.contains(required),
            "missing non-Bootstrap ancestry binding: {required}"
        );
    }
}

#[test]
fn terminal_authorization_verification_is_constant_work_and_rejects_substitution() {
    let output = send_output();
    let proof = terminal_authorization_proof(&output, 1_280, 1_280);
    let verifier = ExactFixtureVerifier::new(output.clone(), proof.clone());
    let verified =
        verify_kagemusha_recursive_proof_v1(&verifier, artifacts(), output.clone(), &proof)
            .expect("exact fixture verifies");
    assert_eq!(verified.public_output(), output);
    assert_eq!(verifier.calls.borrow().len(), 2);

    let mut protocol_substitution = proof.clone();
    protocol_substitution.eq_protocol_digest = eq_digest(0x99);
    assert!(matches!(
        verify_kagemusha_recursive_proof_v1(
            &ExactFixtureVerifier::new(output.clone(), protocol_substitution.clone()),
            artifacts(),
            output.clone(),
            &protocol_substitution,
        ),
        Err(KagemushaRecursionErrorV1::ArtifactSubstitution)
    ));

    let mut body_substitution = proof.clone();
    body_substitution.eq_proof[0] ^= 1;
    assert!(matches!(
        verify_kagemusha_recursive_proof_v1(
            &verifier,
            artifacts(),
            output.clone(),
            &body_substitution,
        ),
        Err(KagemushaRecursionErrorV1::TransitionProofRejected {
            parity: KagemushaPastaParityV1::Eq,
            ..
        })
    ));

    let mut noncanonical = proof;
    noncanonical.eq_history[..32].fill(0xFF);
    assert!(matches!(
        verify_kagemusha_recursive_proof_v1(
            &ExactFixtureVerifier::new(output.clone(), noncanonical.clone()),
            artifacts(),
            output,
            &noncanonical,
        ),
        Err(KagemushaRecursionErrorV1::NonCanonicalAccumulatorScalar {
            parity: KagemushaPastaParityV1::Eq,
            round: 0,
        })
    ));
}

#[test]
fn post_commit_caps_and_incoming_binding_commit_payment_claims() {
    assert_eq!(
        KAGEMUSHA_CURRENT_PROOFS_MAX_BYTES_V1,
        2 * KAGEMUSHA_PARITY_PROOF_MAX_BYTES_V1
    );
    assert!(KAGEMUSHA_PAIRED_PROOF_MAX_BYTES_V1 > KAGEMUSHA_CURRENT_PROOFS_MAX_BYTES_V1);
    let fixture = incoming_payment_fixture(
        0x41,
        9,
        7,
        11,
        KAGEMUSHA_PARITY_PROOF_MAX_BYTES_V1,
        KAGEMUSHA_PARITY_PROOF_MAX_BYTES_V1,
    );
    assert!(
        norito::encode_canonical(&fixture.payment.proof)
            .expect("final payment proof encoding")
            .len()
            <= KAGEMUSHA_PAIRED_PROOF_MAX_BYTES_V1
    );
    let binding = kagemusha_incoming_proof_binding_digest_v1(
        &fixture.request,
        &fixture.intent,
        &fixture.ticket,
        &fixture.payment,
    )
    .expect("incoming binding");
    let claims = [
        fixture.request.canonical_digest().expect("request"),
        fixture
            .intent
            .canonical_digest_against(&fixture.request)
            .expect("intent"),
        fixture
            .ticket
            .canonical_digest_against_intent(&fixture.request, &fixture.intent)
            .expect("ticket"),
        fixture
            .payment
            .output
            .canonical_digest_against(&fixture.request, &fixture.intent, &fixture.ticket)
            .expect("output"),
        iroha_data_model::kagemusha::kagemusha_ciphertext_digest_v1(
            &fixture.payment.encrypted_credit,
        ),
        fixture.payment.proof.candidate_envelope_digest,
        fixture.payment.proof.commit_certificate_digest,
    ];
    assert_eq!(
        binding,
        canonical_incoming_payment_claims_binding_v1(claims)
    );
    for index in 0..claims.len() {
        let mut altered = claims;
        altered[index][0] ^= 1;
        assert_ne!(
            binding,
            canonical_incoming_payment_claims_binding_v1(altered),
            "incoming claim {index}"
        );
    }
    let different_ticket_key = incoming_payment_fixture(
        0x41,
        10,
        7,
        11,
        KAGEMUSHA_PARITY_PROOF_MAX_BYTES_V1,
        KAGEMUSHA_PARITY_PROOF_MAX_BYTES_V1,
    );
    assert_ne!(
        kagemusha_incoming_proof_binding_digest_v1(
            &different_ticket_key.request,
            &different_ticket_key.intent,
            &different_ticket_key.ticket,
            &different_ticket_key.payment,
        )
        .expect("different ticket-key binding"),
        binding,
    );
    let different_proof = incoming_payment_fixture(
        0x51,
        9,
        7,
        11,
        KAGEMUSHA_PARITY_PROOF_MAX_BYTES_V1,
        KAGEMUSHA_PARITY_PROOF_MAX_BYTES_V1,
    );
    // Randomized proof representation cannot change the accepted monetary claims. The actual
    // native/circuit verifier must still verify every proof and decide every carried history.
    assert_eq!(
        kagemusha_incoming_proof_binding_digest_v1(
            &different_proof.request,
            &different_proof.intent,
            &different_proof.ticket,
            &different_proof.payment,
        )
        .expect("different proof binding"),
        binding,
    );
    let different_history = incoming_payment_fixture(
        0x41,
        9,
        900,
        901,
        KAGEMUSHA_PARITY_PROOF_MAX_BYTES_V1,
        KAGEMUSHA_PARITY_PROOF_MAX_BYTES_V1,
    );
    assert_eq!(
        kagemusha_incoming_proof_binding_digest_v1(
            &different_history.request,
            &different_history.intent,
            &different_history.ticket,
            &different_history.payment,
        )
        .expect("different valid history binding"),
        binding,
    );
    let mut different_candidate = fixture.payment.clone();
    different_candidate
        .commit_certificate
        .candidate_envelope_digest = digest(0xA0);
    different_candidate.commit_certificate = different_candidate
        .commit_certificate
        .seal_certificate_id()
        .expect("resealed certificate");
    different_candidate.proof.candidate_envelope_digest = digest(0xA0);
    different_candidate.proof.commit_certificate_digest = different_candidate
        .commit_certificate
        .canonical_digest()
        .expect("changed certificate digest");
    assert_ne!(
        binding,
        kagemusha_incoming_proof_binding_digest_v1(
            &fixture.request,
            &fixture.intent,
            &fixture.ticket,
            &different_candidate,
        )
        .expect("candidate/certificate claims are bound")
    );
}

#[cfg(feature = "zk-halo2-ipa")]
#[test]
fn payment_public_projection_binds_sender_profile_ticket_and_commit_certificate() {
    let fixture = incoming_payment_fixture(0x41, 9, 7, 11, 128, 128);
    let project = |payment: &KagemushaPaymentV1| {
        native_backend::payment_terminal_public_inputs_v1(
            &fixture.request,
            &fixture.intent,
            &fixture.ticket,
            payment,
            digest(0xA1),
        )
    };
    let public = project(&fixture.payment).expect("final projection");
    assert_eq!(public.operation, KagemushaOperationV1::SendSplit);
    assert_eq!(
        public.hardware_profile_id,
        fixture.payment.commit_certificate.hardware_profile_id
    );
    assert_ne!(
        public.hardware_profile_id,
        fixture.request.hardware_credential.hardware_profile_id
    );
    assert_eq!(
        public.policy_epoch,
        fixture.payment.commit_certificate.policy_epoch
    );
    assert_eq!(
        public.semantic_digest,
        kagemusha_payment_body_digest_v1(
            &fixture.payment.output,
            &fixture.payment.encrypted_credit,
        )
        .expect("body digest")
    );
    assert_eq!(
        public.lifecycle_binding_digest,
        fixture.payment.commit_certificate.lifecycle_binding_digest
    );
    assert_eq!(
        public.candidate_envelope_digest,
        fixture.payment.proof.candidate_envelope_digest
    );
    assert_eq!(
        public.commit_certificate_digest,
        fixture.payment.proof.commit_certificate_digest
    );
    assert_eq!(
        public.terminal_output_binding,
        canonical_terminal_send_output_binding_v1(
            fixture.payment.output.credit_id,
            fixture.ticket.recipient_one_time_key,
            fixture.request.hardware_credential.lane_commitment,
            iroha_data_model::kagemusha::kagemusha_prepared_transfer_digest_v1(
                &fixture.request,
                &fixture.intent,
                &fixture.ticket,
                fixture.payment.output.transition_nullifier,
                fixture.payment.output.ciphertext_commitment,
            )
            .expect("prepared transfer"),
            fixture
                .payment
                .output
                .canonical_digest_against(&fixture.request, &fixture.intent, &fixture.ticket)
                .expect("output"),
            kagemusha_incoming_proof_binding_digest_v1(
                &fixture.request,
                &fixture.intent,
                &fixture.ticket,
                &fixture.payment,
            )
            .expect("incoming claims"),
        )
    );
    let mut wrong_body = fixture.payment.clone();
    wrong_body.proof.semantic_digest[0] ^= 1;
    assert!(project(&wrong_body).is_err());
    let mut wrong_certificate = fixture.payment.clone();
    wrong_certificate.proof.commit_certificate_digest[0] ^= 1;
    assert!(project(&wrong_certificate).is_err());
    let mut wrong_nullifier = fixture.payment.clone();
    wrong_nullifier.commit_certificate.transition_nullifier[0] ^= 1;
    assert!(project(&wrong_nullifier).is_err());
}

#[test]
fn reject_all_backend_never_grants_monetary_authority() {
    let fixture = incoming_payment_fixture(0x41, 9, 7, 11, 128, 128);
    assert!(
        RejectAllKagemushaRecursiveVerifierV1
            .verify_payment_and_decide(
                &fixture.request,
                &fixture.intent,
                &fixture.ticket,
                &fixture.payment,
            )
            .is_err()
    );
    let output = send_output();
    let proof = terminal_authorization_proof(&output, 128, 128);
    assert!(matches!(
        verify_kagemusha_recursive_proof_v1(
            &RejectAllKagemushaRecursiveVerifierV1,
            artifacts(),
            output,
            &proof,
        ),
        Err(KagemushaRecursionErrorV1::TransitionProofRejected { .. })
    ));
}

#[test]
fn replay_insert_witness_requires_a_real_root_change() {
    let state_witness = ConsumedCreditInsertWitnessV1 {
        credit_id: CreditIdV1(digest(0xA1)),
        envelope_digest: digest(0xA2),
        predecessor_root: pasta_pair(0xA3),
        successor_root: pasta_pair(0xA4),
        siblings_root_to_leaf: [pasta_pair(0xA5); KAGEMUSHA_REPLAY_PATH_DEPTH_V1],
    };
    let witness = KagemushaReplayInsertWitnessV1::from(&state_witness);
    witness.validate_shape().expect("valid replay insertion");
    assert_eq!(witness.siblings_root_to_leaf.len(), 256);
    let mut no_op = witness;
    no_op.successor_root = no_op.predecessor_root;
    assert_eq!(
        no_op.validate_shape(),
        Err(KagemushaRecursionErrorV1::InvalidReplayWitness)
    );
}

fn ipa_h_coefficients<F: ff::Field>(challenges: &[F], scalar: F) -> Vec<F> {
    let mut coefficients = vec![F::ZERO; 1 << challenges.len()];
    coefficients[0] = scalar;
    for (len, challenge) in challenges
        .iter()
        .rev()
        .enumerate()
        .map(|(index, challenge)| (1 << index, challenge))
    {
        let (left, right) = coefficients.split_at_mut(len);
        let right = &mut right[..len];
        right.copy_from_slice(left);
        for coefficient in right {
            *coefficient *= challenge;
        }
    }
    coefficients
}

fn valid_eq_accumulator(params: &ParamsIPA<EqAffine>, seed: u64) -> KagemushaEqAccumulatorV1 {
    let challenges = (0..KAGEMUSHA_RECURSION_IPA_K_V1)
        .map(|round| Fp::from(seed + u64::from(round) + 1))
        .collect::<Vec<_>>();
    let coefficients = ipa_h_coefficients(&challenges, Fp::ONE);
    let point = params
        .get_g()
        .iter()
        .zip(coefficients)
        .fold(Eq::identity(), |sum, (base, coefficient)| {
            sum + *base * coefficient
        })
        .to_affine();
    KagemushaEqAccumulatorV1::from_native(&IpaAccumulator::new(challenges, point)).unwrap()
}

fn valid_ep_accumulator(params: &ParamsIPA<EpAffine>, seed: u64) -> KagemushaEpAccumulatorV1 {
    let challenges = (0..KAGEMUSHA_RECURSION_IPA_K_V1)
        .map(|round| Fq::from(seed + u64::from(round) + 1))
        .collect::<Vec<_>>();
    let coefficients = ipa_h_coefficients(&challenges, Fq::ONE);
    let point = params
        .get_g()
        .iter()
        .zip(coefficients)
        .fold(Ep::identity(), |sum, (base, coefficient)| {
            sum + *base * coefficient
        })
        .to_affine();
    KagemushaEpAccumulatorV1::from_native(&IpaAccumulator::new(challenges, point)).unwrap()
}

#[test]
#[ignore = "expensive fixed-k native primitive qualification; run in the Kagemusha release lane"]
fn real_k16_native_folds_decide_and_reject_substitution() {
    // Fixed material belongs only to this primitive test, never hardware entropy.
    let recovery_seed = iroha_crypto::kagemusha::KagemushaRecoverySeedV1::from_unsealed([0xA7; 32])
        .expect("test-only unsealed recovery seed");
    let eq_params = ParamsIPA::<EqAffine>::new(KAGEMUSHA_RECURSION_IPA_K_V1);
    let eq_current = valid_eq_accumulator(&eq_params, 3);
    let eq_predecessor = valid_eq_accumulator(&eq_params, 29);
    let eq_fold =
        fold_kagemusha_eq_accumulators_v1(&eq_params, &eq_current, &eq_predecessor, &recovery_seed)
            .unwrap();
    assert_eq!(
        eq_fold,
        fold_kagemusha_eq_accumulators_v1(
            &eq_params,
            &eq_current,
            &eq_predecessor,
            &recovery_seed,
        )
        .unwrap(),
        "recovery must reproduce every Eq fold and successor byte"
    );
    verify_and_decide_kagemusha_eq_fold_v1(&eq_params, &eq_current, &eq_predecessor, &eq_fold)
        .unwrap();
    let mut tampered = eq_fold.proof().as_bytes().to_vec();
    tampered[0] ^= 1;
    let tampered = KagemushaEqFoldOutputV1::from_parts(
        eq_fold.successor().clone(),
        KagemushaEqFoldProofV1::try_from_bytes(&tampered).unwrap(),
    );
    assert!(
        verify_and_decide_kagemusha_eq_fold_v1(
            &eq_params,
            &eq_current,
            &eq_predecessor,
            &tampered,
        )
        .is_err()
    );

    let ep_params = ParamsIPA::<EpAffine>::new(KAGEMUSHA_RECURSION_IPA_K_V1);
    let ep_current = valid_ep_accumulator(&ep_params, 5);
    let ep_predecessor = valid_ep_accumulator(&ep_params, 31);
    let ep_fold =
        fold_kagemusha_ep_accumulators_v1(&ep_params, &ep_current, &ep_predecessor, &recovery_seed)
            .unwrap();
    assert_eq!(
        ep_fold,
        fold_kagemusha_ep_accumulators_v1(
            &ep_params,
            &ep_current,
            &ep_predecessor,
            &recovery_seed,
        )
        .unwrap(),
        "recovery must reproduce every Ep fold and successor byte"
    );
    verify_and_decide_kagemusha_ep_fold_v1(&ep_params, &ep_current, &ep_predecessor, &ep_fold)
        .unwrap();
}
