//! Real-proof payment-corridor fixtures, separate from transition-model qualification.
//!
//! The mint fixture creates a genuine four-validator finality certificate and recursively
//! proves Bootstrap -> FinalizedMint with one reusable carrier key pair. Certificate preflight
//! tests are not proof evidence; the explicitly ignored real-proof gate is expensive.
//!
//! TODO: connect these funded inputs to the revised terminal/payment-proof corridor once its
//! post-commit proof shape is fixed. No rotation or fabricated positive bootstrap substitutes
//! for the pending SendSplit -> transported authorization -> ReceiveFold qualification.

use std::fs::{File, OpenOptions, TryLockError};

use super::*;
use crate::zk::kagemusha_v1_recursion::{
    KagemushaGeneratedMintAuthorityArtifactsV1, KagemushaGeneratedMintAuthorityProofV1,
    KagemushaGeneratedMintAuthorizationArtifactsV1, KagemushaGeneratedMintAuthorizationProofV1,
    KagemushaLoadedEpMintAuthorityArtifactsV1, KagemushaLoadedEpMintAuthorizationArtifactsV1,
    KagemushaLoadedEpMintHashArtifactsV1, KagemushaLoadedEqMintAuthorityArtifactsV1,
    KagemushaLoadedEqMintAuthorizationArtifactsV1, KagemushaLoadedEqMintHashArtifactsV1,
    KagemushaMintAuthorityGenerationWitnessV1, KagemushaMintAuthorityStepV1,
    KagemushaMintAuthorizationGenerationWitnessV1, KagemushaMintAuthorizationRelationWitnessV1,
    KagemushaMintCertificateWitnessV1, KagemushaMintFinalitySignerV1, KagemushaMintFinalityTreeV1,
    KagemushaMintHashArtifactGenerationWitnessV1, KagemushaMintHashClaimGenerationWitnessV1,
    KagemushaReceiveFoldCreditV1, KagemushaRecursiveIncomingEpGenerationWitnessV1,
    KagemushaRecursiveIncomingEqGenerationWitnessV1, KagemushaRecursiveStateGenerationWitnessV1,
    KagemushaReplayInsertWitnessV1,
    accumulation::{
        fold_kagemusha_ep_accumulators_with_rng_v1, fold_kagemusha_eq_accumulators_with_rng_v1,
    },
    deferred_parent::kagemusha_protocol_structure_digest_v1,
    generate_kagemusha_mint_authority_artifacts_v1,
    generate_kagemusha_mint_authorization_artifacts_v1,
    generate_kagemusha_mint_hash_artifacts_for_guarded_test_v1,
    generate_kagemusha_recursive_state_artifacts_v1,
    guard_bundle::KAGEMUSHA_ENABLED_HARDWARE_PROFILE_SLOTS_V1,
    kagemusha_mint_finality_empty_root_v1,
    mint_authority::{
        KagemushaMintAuthorityEpCircuitV1, KagemushaMintAuthorityEqCircuitV1,
        public_instance as mint_instance,
    },
    mint_authorization::{
        KagemushaMintAuthorizationEpCircuitV1, KagemushaMintAuthorizationEqCircuitV1,
        MINT_AUTHORIZATION_PUBLIC_INSTANCE_COUNT_V1,
    },
    mint_hash_claim_fold::{
        KAGEMUSHA_MINT_HASH_CLAIM_CARRIER_INSTANCE_COUNT_V1,
        KAGEMUSHA_MINT_HASH_CLAIM_INNER_SEMANTIC_INSTANCE_COUNT_V1,
    },
    mint_transport_decider::{
        KagemushaMintAuthorityTransportEpCircuitV1, KagemushaMintAuthorityTransportEqCircuitV1,
        KagemushaMintAuthorizationTransportEpCircuitV1,
        KagemushaMintAuthorizationTransportEqCircuitV1,
    },
    prove_kagemusha_mint_authority_v1, prove_kagemusha_mint_authorization_hash_claim_v1,
    prove_kagemusha_mint_authorization_v1, prove_kagemusha_mint_hash_claim_v1,
    prove_kagemusha_recursive_state_hash_claim_v1, prove_kagemusha_recursive_state_v1,
    terminal_authorization::TERMINAL_AUTHORIZATION_PUBLIC_INSTANCE_COUNT_V1,
};
use halo2_proofs::{
    halo2curves::{ff::Field as _, group::Group as _},
    poly::commitment::Params as _,
};
use iroha_crypto::{Algorithm, KeyPair};
use iroha_data_model::{
    account::AccountId,
    block::consensus_v2::{HeightContextId, ValidatorPower},
    isi::kagemusha_v1::{
        KAGEMUSHA_CHAIN_VERSION_V1, KAGEMUSHA_MINT_FINALITY_TREE_DEPTH_V1,
        KagemushaMintFinalityEpochRosterV1, KagemushaMintFinalitySealBundleV1,
        KagemushaMintFinalitySealMessageV1, KagemushaPastaSchnorrSignatureV1, KagemushaTopUpLeafV1,
        KagemushaTopUpMembershipWitnessV1, kagemusha_mint_finality_root_v1,
    },
    kagemusha::{
        KAGEMUSHA_HARDWARE_REQUIRED_CAPABILITIES_V1, KAGEMUSHA_XCHACHA20POLY1305_NONCE_BYTES_V1,
        KAGEMUSHA_XCHACHA20POLY1305_TAG_BYTES_V1, KagemushaCreditOpeningV1,
        KagemushaDevicePublicKeyV1, KagemushaDeviceSignatureV1, KagemushaEncryptedCreditEnvelopeV1,
        KagemushaHardwareCredentialV1, KagemushaHardwarePlatformClassV1,
        KagemushaHardwareProfileV1, KagemushaLifecycleBindingV1,
        KagemushaMintAuthorizationContextV1, KagemushaMintAuthorizationStatementV1,
        KagemushaMintAuthorizationV1, KagemushaMintCreditStatementV1, KagemushaOperationKindV1,
        kagemusha_ciphertext_digest_v1, kagemusha_credit_opening_canonical_len_v1,
        kagemusha_mint_credit_opening_commitment_v1, kagemusha_recipient_credential_commitment_v1,
    },
    peer::PeerId,
};
use p256::ecdsa::{Signature, signature::Signer as _};
use zeroize::Zeroizing;

const SUITE_COMMITMENT_DOMAIN_V1: &[u8] = b"iroha:kagemusha:v1:suite-commitment";
const REAL_PROOF_TEST_STACK_BYTES: usize = 64 * 1024 * 1024;

/// Minimal three-column protocol used only to bootstrap recursive claim-key shape convergence.
///
/// The generated hash suite replaces this protocol after the first bounded convergence round. A
/// dedicated seed avoids the former circular dependency on the much larger PlatformCredential
/// proving key and keeps this setup path's peak allocation small.
#[derive(Clone)]
struct MintHashClaimProtocolSeedCircuit<F: ff::PrimeField>(halo2_proofs::circuit::Value<F>);

impl<F: ff::PrimeField> Circuit<F> for MintHashClaimProtocolSeedCircuit<F> {
    type Config = (
        halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>,
        [halo2_proofs::plonk::Column<halo2_proofs::plonk::Instance>; 3],
    );
    type FloorPlanner = halo2_proofs::circuit::SimpleFloorPlanner;
    type Params = ();

    fn without_witnesses(&self) -> Self {
        Self(halo2_proofs::circuit::Value::unknown())
    }

    fn configure(meta: &mut halo2_proofs::plonk::ConstraintSystem<F>) -> Self::Config {
        let advice = meta.advice_column();
        let instances = [
            meta.instance_column(),
            meta.instance_column(),
            meta.instance_column(),
        ];
        meta.enable_equality(advice);
        for instance in instances {
            meta.enable_equality(instance);
        }
        (advice, instances)
    }

    fn synthesize(
        &self,
        (advice, instances): Self::Config,
        mut layouter: impl halo2_proofs::circuit::Layouter<F>,
    ) -> Result<(), halo2_proofs::plonk::Error> {
        let cells = layouter.assign_region(
            || "mint-hash claim protocol seed",
            |mut region| {
                Ok([
                    region.assign_advice(advice, 0, self.0).cell(),
                    region.assign_advice(advice, 1, self.0).cell(),
                    region.assign_advice(advice, 2, self.0).cell(),
                ])
            },
        )?;
        for (cell, instance) in cells.into_iter().zip(instances) {
            layouter.constrain_instance(cell, instance, 0);
        }
        Ok(())
    }
}

fn mint_hash_claim_protocol_seeds(
    eq: &ParamsIPA<EqAffine>,
    ep: &ParamsIPA<EpAffine>,
) -> (PlonkProtocol<EqAffine>, PlonkProtocol<EpAffine>) {
    let eq_circuit =
        MintHashClaimProtocolSeedCircuit::<Fp>(halo2_proofs::circuit::Value::known(Fp::ZERO));
    let eq_vk = keygen_vk(eq, &eq_circuit).expect("Eq mint-hash claim seed VK");
    let eq_protocol = compile(
        eq,
        &eq_vk,
        snark_verifier::system::halo2::Config::ipa().with_num_instance(vec![
            KAGEMUSHA_MINT_HASH_CLAIM_INNER_SEMANTIC_INSTANCE_COUNT_V1,
            KAGEMUSHA_MINT_HASH_CLAIM_CARRIER_INSTANCE_COUNT_V1,
            KAGEMUSHA_MINT_HASH_CLAIM_CARRIER_INSTANCE_COUNT_V1,
        ]),
    );
    drop(eq_vk);
    halo2_proofs::release_allocator_slack();

    let ep_circuit =
        MintHashClaimProtocolSeedCircuit::<Fq>(halo2_proofs::circuit::Value::known(Fq::ZERO));
    let ep_vk = keygen_vk(ep, &ep_circuit).expect("Ep mint-hash claim seed VK");
    let ep_protocol = compile(
        ep,
        &ep_vk,
        snark_verifier::system::halo2::Config::ipa().with_num_instance(vec![
            KAGEMUSHA_MINT_HASH_CLAIM_INNER_SEMANTIC_INSTANCE_COUNT_V1,
            KAGEMUSHA_MINT_HASH_CLAIM_CARRIER_INSTANCE_COUNT_V1,
            KAGEMUSHA_MINT_HASH_CLAIM_CARRIER_INSTANCE_COUNT_V1,
        ]),
    );
    drop(ep_vk);
    halo2_proofs::release_allocator_slack();
    (eq_protocol, ep_protocol)
}

/// Keep the explicitly ignored, machine-scale proof fixtures mutually exclusive across Cargo
/// invocations. The lock lives outside every target directory because separate target roots must
/// not allow two copies of the same memory-heavy proof to run concurrently.
fn exclusive_real_proof_test_lock() -> File {
    let path = std::env::temp_dir().join("iroha-kagemusha-real-proof-v1.lock");
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(&path)
        .unwrap_or_else(|error| panic!("open real-proof test lock {}: {error}", path.display()));
    match file.try_lock() {
        Ok(()) => file,
        Err(TryLockError::WouldBlock) => panic!(
            "another Kagemusha real-proof fixture is already running; refusing duplicate memory allocation"
        ),
        Err(TryLockError::Error(error)) => {
            panic!("lock real-proof fixture {}: {error}", path.display())
        }
    }
}

fn digest_bytes(label: &[u8], bytes: &[u8]) -> DigestV1 {
    let mut hasher = Sha256::new();
    hasher.update(label);
    hasher.update([0]);
    hasher.update(
        u64::try_from(bytes.len())
            .expect("qualification input length fits u64")
            .to_le_bytes(),
    );
    hasher.update(bytes);
    hasher.finalize().into()
}

fn device_public_key(signing_key: &SigningKey) -> KagemushaDevicePublicKeyV1 {
    KagemushaDevicePublicKeyV1::from_sec1_bytes(
        signing_key
            .verifying_key()
            .to_encoded_point(false)
            .as_bytes(),
    )
    .expect("canonical corridor P-256 key")
}

fn device_signature(signing_key: &SigningKey, message: &[u8]) -> KagemushaDeviceSignatureV1 {
    let signature: Signature = signing_key.sign(message);
    let signature = signature.normalize_s().unwrap_or(signature);
    KagemushaDeviceSignatureV1::from_raw_bytes(signature.to_bytes().as_ref())
        .expect("canonical low-S corridor signature")
}

/// Exact recipient authorization material retained into the real MintFold witness.
struct MintRecipientMaterial {
    platform_credential: KagemushaPlatformCredentialRelationWitnessV1,
    hardware_profile: KagemushaHardwareProfileV1,
    hardware_credential: KagemushaHardwareCredentialV1,
    authorization_relation: KagemushaMintAuthorizationRelationWitnessV1,
    recipient: AccountId,
}

impl MintRecipientMaterial {
    fn finalized_statement(
        &self,
        authorization: &KagemushaMintAuthorizationV1,
    ) -> KagemushaMintCreditStatementV1 {
        assert_eq!(
            authorization.statement, self.authorization_relation.statement,
            "finalized mint must use this recipient's exact authorization statement",
        );
        self.finalized_statement_with_authorization_digest(
            authorization
                .canonical_digest()
                .expect("mint authorization digest"),
        )
    }

    fn finalized_statement_with_authorization_digest(
        &self,
        mint_authorization_digest: DigestV1,
    ) -> KagemushaMintCreditStatementV1 {
        assert_ne!(
            mint_authorization_digest, [0; 32],
            "finalized mint authorization digest must be nonzero",
        );
        let authorization = &self.authorization_relation.statement;
        let context = &authorization.context;
        let statement = KagemushaMintCreditStatementV1 {
            version: KAGEMUSHA_WIRE_VERSION_V1,
            lifecycle: KagemushaLifecycleBindingV1 {
                version: KAGEMUSHA_WIRE_VERSION_V1,
                network_id: context.network_id,
                protocol_version: KAGEMUSHA_WIRE_VERSION_V1,
                suite_id: context.suite_id,
                vk_digest: context.vk_digest,
                release_id: context.release_id,
                asset: context.asset.clone(),
                asset_incarnation: context.asset_incarnation,
                scale: context.scale,
                liability_pool_id: context.liability_pool_id,
                hardware_profile_id: context.hardware_profile_id,
                policy_epoch: context.policy_epoch,
                operation_kind: KagemushaOperationKindV1::MintFold,
                request_id: [0; 32],
                receiver_lane_commitment: [0; 32],
                credit_id: authorization.credit_id,
                ciphertext_digest: authorization.ciphertext_digest,
            },
            recipient_credential_commitment: context.recipient_credential_commitment,
            authorization_context_digest: context
                .canonical_digest()
                .expect("mint authorization context digest"),
            mint_authorization_digest,
            amount: context.amount,
            issuance_commitment: authorization.issuance_commitment,
            recipient: context.recipient.clone(),
            credit_commitment: context.credit_commitment,
            minted_at_ms: 100,
        };
        statement
            .validate_shape()
            .expect("authorization-bound finalized mint statement");
        statement
    }
}

fn mint_recipient_material(
    release_id: DigestV1,
    vk_digest: DigestV1,
    artifact_manifest_digest: DigestV1,
    amount: u128,
) -> MintRecipientMaterial {
    let suite_id = digest(b"suite", 0);
    let issuer = deterministic_signing_key(0x7000);
    let issuer_public_key = device_public_key(&issuer);
    let hardware_profile = KagemushaHardwareProfileV1 {
        version: KAGEMUSHA_WIRE_VERSION_V1,
        protocol_version: KAGEMUSHA_WIRE_VERSION_V1,
        hardware_profile_id: [0; 32],
        provider_id: digest(b"mint-provider", 0),
        platform_class: KagemushaHardwarePlatformClassV1::DedicatedSecureElement,
        product_class_digest: digest(b"mint-product", 0),
        firmware_policy_digest: digest(b"mint-firmware", 0),
        enrollment_attestation_verifier_digest: digest(b"mint-enrollment-verifier", 0),
        attestation_trust_roots_digest: digest(b"mint-attestation-root", 0),
        allowed_suite_commitment: digest_bytes(SUITE_COMMITMENT_DOMAIN_V1, &suite_id),
        policy_epoch: 1,
        governance_credential_public_key: issuer_public_key,
        capability_mask: KAGEMUSHA_HARDWARE_REQUIRED_CAPABILITIES_V1,
        qualification_report_digest: digest(b"mint-qualification-report", 0),
        valid_from_ms: 1,
        expires_at_ms: 1_000_000,
    }
    .seal_hardware_profile_id()
    .expect("canonical corridor hardware profile");

    let empty_effect = digest(b"empty-durable-effect", 0);
    let (mut platform_credential, device_authority_secret) =
        credential_witness(0, release_id, empty_effect);
    platform_credential.statement.hardware_profile_id = hardware_profile.hardware_profile_id;
    let mut hardware_policy_id = policy_leaf(&platform_credential.statement);
    for (depth, sibling) in platform_credential
        .policy_siblings
        .iter()
        .copied()
        .enumerate()
    {
        hardware_policy_id =
            if (platform_credential.statement.provider_profile_index >> depth) & 1 == 0 {
                policy_node(hardware_policy_id, sibling)
            } else {
                policy_node(sibling, hardware_policy_id)
            };
    }
    platform_credential.statement.hardware_policy_id = hardware_policy_id;

    let mut hardware_credential = KagemushaHardwareCredentialV1 {
        version: KAGEMUSHA_WIRE_VERSION_V1,
        credential_id: [0; 32],
        network_id: lane().network_id,
        hardware_profile_id: hardware_profile.hardware_profile_id,
        suite_id,
        firmware_policy_digest: hardware_profile.firmware_policy_digest,
        policy_epoch: hardware_profile.policy_epoch,
        lane_commitment: platform_credential.statement.lane_id,
        hardware_epoch_id: platform_credential.statement.hardware_epoch_id,
        hardware_epoch_generation: u64::try_from(
            platform_credential.statement.hardware_epoch_generation,
        )
        .expect("corridor hardware generation fits u64"),
        device_public_key: platform_credential.statement.device_public_key,
        device_key_reference: platform_credential.statement.key_reference,
        issued_at_ms: 10,
        expires_at_ms: 900_000,
        governance_signature: device_signature(&issuer, b"unsealed-corridor-credential"),
    }
    .seal_credential_id()
    .expect("canonical corridor credential ID");
    hardware_credential.governance_signature = device_signature(
        &issuer,
        &hardware_credential
            .canonical_signing_bytes()
            .expect("corridor credential signing bytes"),
    );
    hardware_credential
        .validate_against_profile(&hardware_profile)
        .expect("release-enabled corridor credential");
    platform_credential.statement.credential_issuance_digest = hardware_credential.credential_id;
    platform_credential
        .validate()
        .expect("credential relation remains valid after canonical recipient binding");

    let payer = AccountId::new(
        KeyPair::from_seed(vec![90; 32], Algorithm::Ed25519)
            .public_key()
            .clone(),
    );
    let recipient = AccountId::new(
        KeyPair::from_seed(vec![91; 32], Algorithm::Ed25519)
            .public_key()
            .clone(),
    );
    let operation_id = digest(b"funding-operation", 0);
    let recipient_one_time_key = digest(b"mint-recipient-x25519", 0);
    let credit_opening_secret = digest(b"mint-credit-opening", 0);
    let recipient_binding_opening = digest(b"mint-recipient-opening", 0);
    let encrypted_credit = KagemushaEncryptedCreditEnvelopeV1 {
        version: KAGEMUSHA_WIRE_VERSION_V1,
        ephemeral_x25519_public_key: digest(b"mint-ephemeral-x25519", 0),
        nonce: [0x33; KAGEMUSHA_XCHACHA20POLY1305_NONCE_BYTES_V1],
        ciphertext_and_tag: vec![
            0x44;
            kagemusha_credit_opening_canonical_len_v1()
                .expect("credit opening width")
                + KAGEMUSHA_XCHACHA20POLY1305_TAG_BYTES_V1
        ],
    }
    .canonical_bytes_against_recipient_key(recipient_one_time_key)
    .expect("canonical corridor encrypted credit");
    let recipient_credential_commitment = kagemusha_recipient_credential_commitment_v1(
        operation_id,
        hardware_credential.credential_id,
        recipient_binding_opening,
    )
    .expect("recipient credential commitment");
    let credit_commitment = kagemusha_mint_credit_opening_commitment_v1(
        &lane().network_id,
        &lane().asset,
        incarnation(),
        lane().scale,
        kagemusha_liability_pool_id_v1(&lane().network_id, &lane().asset, incarnation())
            .expect("corridor liability pool"),
        amount,
        &recipient,
        recipient_one_time_key,
        credit_opening_secret,
    )
    .expect("mint credit opening commitment");
    let context = KagemushaMintAuthorizationContextV1 {
        version: KAGEMUSHA_WIRE_VERSION_V1,
        operation_id,
        release_id,
        suite_id,
        vk_digest,
        artifact_manifest_digest,
        network_id: lane().network_id,
        asset: lane().asset,
        asset_incarnation: incarnation(),
        scale: lane().scale,
        liability_pool_id: kagemusha_liability_pool_id_v1(
            &lane().network_id,
            &lane().asset,
            incarnation(),
        )
        .expect("corridor liability pool"),
        amount,
        payer,
        recipient: recipient.clone(),
        hardware_credential_id: hardware_credential.credential_id,
        hardware_profile_id: hardware_profile.hardware_profile_id,
        policy_epoch: hardware_profile.policy_epoch,
        recipient_credential_commitment,
        credit_commitment,
        recipient_one_time_key,
    };
    context
        .validate_shape()
        .expect("valid mint authorization context");

    // The credit identity excludes the full authorization digest, avoiding a self-reference.
    let provisional_credit_statement = KagemushaMintCreditStatementV1 {
        version: KAGEMUSHA_WIRE_VERSION_V1,
        lifecycle: KagemushaLifecycleBindingV1 {
            version: KAGEMUSHA_WIRE_VERSION_V1,
            network_id: context.network_id,
            protocol_version: KAGEMUSHA_WIRE_VERSION_V1,
            suite_id: context.suite_id,
            vk_digest: context.vk_digest,
            release_id: context.release_id,
            asset: context.asset.clone(),
            asset_incarnation: context.asset_incarnation,
            scale: context.scale,
            liability_pool_id: context.liability_pool_id,
            hardware_profile_id: context.hardware_profile_id,
            policy_epoch: context.policy_epoch,
            operation_kind: KagemushaOperationKindV1::MintFold,
            request_id: [0; 32],
            receiver_lane_commitment: [0; 32],
            credit_id: [0; 32],
            ciphertext_digest: kagemusha_ciphertext_digest_v1(&encrypted_credit),
        },
        recipient_credential_commitment,
        authorization_context_digest: context
            .canonical_digest()
            .expect("mint authorization context digest"),
        mint_authorization_digest: digest(b"provisional-authorization", 0),
        amount,
        issuance_commitment: digest(b"funding-issuance", 0),
        recipient: recipient.clone(),
        credit_commitment,
        minted_at_ms: 100,
    }
    .seal_credit_id()
    .expect("canonical mint credit identity");
    let credit_opening = KagemushaCreditOpeningV1 {
        version: KAGEMUSHA_WIRE_VERSION_V1,
        credit_id: provisional_credit_statement.lifecycle.credit_id,
        amount,
        credit_commitment_opening: credit_opening_secret,
        recipient_binding_opening,
        recovery_nonce: digest(b"mint-opening-recovery", 0),
    };
    let authorization_relation = KagemushaMintAuthorizationRelationWitnessV1 {
        statement: KagemushaMintAuthorizationStatementV1 {
            version: KAGEMUSHA_WIRE_VERSION_V1,
            context,
            issuance_commitment: provisional_credit_statement.issuance_commitment,
            credit_id: provisional_credit_statement.lifecycle.credit_id,
            ciphertext_digest: provisional_credit_statement.lifecycle.ciphertext_digest,
        },
        hardware_profile,
        hardware_credential: hardware_credential.clone(),
        platform_credential: platform_credential.statement,
        device_authority_secret,
        credit_opening,
        recipient_key_handle_opening: digest(b"mint-key-handle", 0),
        hardware_authorization_nonce: digest(b"mint-authorization-nonce", 0),
        encrypted_credit,
    };
    authorization_relation
        .validate_shape()
        .expect("valid exact mint recipient authorization relation");
    MintRecipientMaterial {
        platform_credential,
        hardware_profile,
        hardware_credential,
        authorization_relation,
        recipient,
    }
}

/// Reusable real MintAuthorization keys for the funded-state corridor.
struct MintAuthorizationKeys {
    eq: KagemushaLoadedEqMintAuthorizationArtifactsV1,
    ep: KagemushaLoadedEpMintAuthorizationArtifactsV1,
    eq_protocol: PlonkProtocol<EqAffine>,
    ep_protocol: PlonkProtocol<EpAffine>,
    recursive_inputs: PreparedMintAuthorizationRecursiveInputs,
}

/// Compact authorization verifier state retained after the authorization proof is complete.
struct MintAuthorizationProtocols {
    eq_protocol: PlonkProtocol<EqAffine>,
    ep_protocol: PlonkProtocol<EpAffine>,
    eq_digest: DigestV1,
    ep_digest: DigestV1,
}

/// One proved recipient authorization plus the recursive histories it transports.
struct ProvenMintAuthorization {
    authorization: KagemushaMintAuthorizationV1,
    generated: KagemushaGeneratedMintAuthorizationProofV1,
    eq_history: KagemushaEqAccumulatorV1,
    ep_history: KagemushaEpAccumulatorV1,
}

struct PreparedMintAuthorizationRecursiveInputs {
    hash_claim: KagemushaGeneratedMintHashClaimV1,
    eq_credential_history_fold_proof: KagemushaEqFoldProofV1,
    ep_credential_history_fold_proof: KagemushaEpFoldProofV1,
    eq_hash_claim_merge_fold_proof: KagemushaEqFoldProofV1,
    eq_successor_history: KagemushaEqAccumulatorV1,
    ep_hash_claim_merge_fold_proof: KagemushaEpFoldProofV1,
    ep_successor_history: KagemushaEpAccumulatorV1,
}

impl PreparedMintAuthorizationRecursiveInputs {
    fn new(
        material: &MintRecipientMaterial,
        eq_params: &ParamsIPA<EqAffine>,
        ep_params: &ParamsIPA<EpAffine>,
        eq_hash: &KagemushaLoadedEqMintHashArtifactsV1,
        ep_hash: &KagemushaLoadedEpMintHashArtifactsV1,
        enabled_hardware_profiles: &[[u8; 32]; KAGEMUSHA_ENABLED_HARDWARE_PROFILE_SLOTS_V1],
        credential: &CredentialProof,
    ) -> Self {
        assert_eq!(
            credential.relation, material.platform_credential,
            "MintAuthorization must consume the exact proved PlatformCredential relation",
        );
        let eq_credential_fold = fold_kagemusha_eq_accumulators_v1(
            eq_params,
            &credential.eq_current,
            &credential.eq_claim_history,
            &test_only_recovery_seed(),
        )
        .expect("fold Eq PlatformCredential proof into its typed-SHA history");
        let ep_credential_fold = fold_kagemusha_ep_accumulators_v1(
            ep_params,
            &credential.ep_current,
            &credential.ep_claim_history,
            &test_only_recovery_seed(),
        )
        .expect("fold Ep PlatformCredential proof into its typed-SHA history");
        let hash_claim = prove_kagemusha_mint_authorization_hash_claim_v1(
            eq_hash,
            ep_hash,
            &material.authorization_relation,
            enabled_hardware_profiles,
            &test_only_recovery_seed(),
        )
        .expect("prove complete MintAuthorization typed-SHA claim");
        let eq_hash_claim_merge = fold_kagemusha_eq_accumulators_v1(
            eq_params,
            eq_credential_fold.successor(),
            &hash_claim.eq_complete_history,
            &test_only_recovery_seed(),
        )
        .expect("merge Eq credential and MintAuthorization typed-SHA histories");
        let ep_hash_claim_merge = fold_kagemusha_ep_accumulators_v1(
            ep_params,
            ep_credential_fold.successor(),
            &hash_claim.ep_complete_history,
            &test_only_recovery_seed(),
        )
        .expect("merge Ep credential and MintAuthorization typed-SHA histories");
        Self {
            hash_claim,
            eq_credential_history_fold_proof: eq_credential_fold.proof().clone(),
            ep_credential_history_fold_proof: ep_credential_fold.proof().clone(),
            eq_hash_claim_merge_fold_proof: eq_hash_claim_merge.proof().clone(),
            eq_successor_history: eq_hash_claim_merge.successor().clone(),
            ep_hash_claim_merge_fold_proof: ep_hash_claim_merge.proof().clone(),
            ep_successor_history: ep_hash_claim_merge.successor().clone(),
        }
    }

    fn witness<'a>(
        &'a self,
        material: &MintRecipientMaterial,
        enabled_hardware_profiles: [[u8; 32]; KAGEMUSHA_ENABLED_HARDWARE_PROFILE_SLOTS_V1],
        eq_hash: &'a KagemushaLoadedEqMintHashArtifactsV1,
        ep_hash: &'a KagemushaLoadedEpMintHashArtifactsV1,
        credential_keys: &'a CredentialKeys,
        credential: &'a CredentialProof,
    ) -> KagemushaMintAuthorizationGenerationWitnessV1<'a> {
        KagemushaMintAuthorizationGenerationWitnessV1 {
            relation: material.authorization_relation.clone(),
            enabled_hardware_profiles,
            eq_hash_claim_protocol_digest: eq_hash.claim_protocol_digest,
            ep_hash_claim_protocol_digest: ep_hash.claim_protocol_digest,
            eq_hash_shard_protocol_digest: eq_hash.shard_protocol_digest,
            ep_hash_shard_protocol_digest: ep_hash.shard_protocol_digest,
            eq_credential_protocol_digest: credential_keys.eq_protocol_digest,
            eq_credential_protocol: &credential_keys.eq_protocol,
            eq_credential_instances: &credential.eq_instances,
            eq_credential_proof: &credential.eq_proof,
            eq_credential_claim_history: &credential.eq_claim_history,
            eq_credential_history_fold_proof: &self.eq_credential_history_fold_proof,
            eq_hash_claim_protocol: &eq_hash.claim_protocol,
            eq_hash_claim_instances: &self.hash_claim.eq_inner_instances,
            eq_hash_claim_proof: &self.hash_claim.eq_proof,
            eq_hash_claim_history: &self.hash_claim.eq_history,
            eq_hash_claim_history_fold_proof: &self.hash_claim.eq_history_fold_proof,
            eq_hash_claim_merge_fold_proof: &self.eq_hash_claim_merge_fold_proof,
            eq_successor_history: &self.eq_successor_history,
            ep_credential_protocol_digest: credential_keys.ep_protocol_digest,
            ep_credential_protocol: &credential_keys.ep_protocol,
            ep_credential_instances: &credential.ep_instances,
            ep_credential_proof: &credential.ep_proof,
            ep_credential_claim_history: &credential.ep_claim_history,
            ep_credential_history_fold_proof: &self.ep_credential_history_fold_proof,
            ep_hash_claim_protocol: &ep_hash.claim_protocol,
            ep_hash_claim_instances: &self.hash_claim.ep_inner_instances,
            ep_hash_claim_proof: &self.hash_claim.ep_proof,
            ep_hash_claim_history: &self.hash_claim.ep_history,
            ep_hash_claim_history_fold_proof: &self.hash_claim.ep_history_fold_proof,
            ep_hash_claim_merge_fold_proof: &self.ep_hash_claim_merge_fold_proof,
            ep_successor_history: &self.ep_successor_history,
        }
    }
}

impl MintAuthorizationKeys {
    fn decode(
        generated: KagemushaGeneratedMintAuthorizationArtifactsV1,
        relation: &KagemushaMintAuthorizationRelationWitnessV1,
        recursive_inputs: PreparedMintAuthorizationRecursiveInputs,
    ) -> Self {
        macro_rules! decode {
            ($bytes:expr, $key:ident, $circuit:ty, $params:expr) => {{
                let mut cursor = Cursor::new($bytes.as_ref());
                let key = $key::read_checked::<_, $circuit>(
                    &mut cursor,
                    SerdeFormat::Processed,
                    u32::try_from($params.k).expect("generated authorization circuit degree"),
                    $params.clone(),
                )
                .expect("decode generated mint-authorization key");
                assert_eq!(
                    usize::try_from(cursor.position()).expect("key cursor"),
                    $bytes.len(),
                    "mint-authorization key must have no trailing bytes",
                );
                key
            }};
        }

        let context = &relation.statement.context;
        let profile_digest = digest(b"funding-test-profile-identity", 0);
        let eq_parameters = ParamsIPA::read(&mut Cursor::new(generated.eq_parameters.as_ref()))
            .expect("Eq mint-authorization parameters");
        let ep_parameters = ParamsIPA::read(&mut Cursor::new(generated.ep_parameters.as_ref()))
            .expect("Ep mint-authorization parameters");
        let eq = KagemushaLoadedEqMintAuthorizationArtifactsV1 {
            parameters: eq_parameters,
            proving_key: decode!(
                generated.eq_proving_key,
                ProvingKey,
                KagemushaMintAuthorizationTransportEqCircuitV1,
                generated.eq_circuit_params
            ),
            verifying_key: decode!(
                generated.eq_verifying_key,
                VerifyingKey,
                KagemushaMintAuthorizationTransportEqCircuitV1,
                generated.eq_circuit_params
            ),
            circuit_params: generated.eq_circuit_params.clone(),
            protocol_digest: generated.eq_protocol_digest,
            inner_proving_key: decode!(
                generated.inner_eq_proving_key,
                ProvingKey,
                KagemushaMintAuthorizationEqCircuitV1,
                generated.inner_eq_circuit_params
            ),
            inner_verifying_key: decode!(
                generated.inner_eq_verifying_key,
                VerifyingKey,
                KagemushaMintAuthorizationEqCircuitV1,
                generated.inner_eq_circuit_params
            ),
            inner_circuit_params: generated.inner_eq_circuit_params.clone(),
            release_id: context.release_id,
            profile_digest,
            artifact_manifest_digest: context.artifact_manifest_digest,
            suite_id: context.suite_id,
            vk_digest: context.vk_digest,
            enabled_hardware_profiles: generated.enabled_hardware_profiles,
        };
        let ep = KagemushaLoadedEpMintAuthorizationArtifactsV1 {
            parameters: ep_parameters,
            proving_key: decode!(
                generated.ep_proving_key,
                ProvingKey,
                KagemushaMintAuthorizationTransportEpCircuitV1,
                generated.ep_circuit_params
            ),
            verifying_key: decode!(
                generated.ep_verifying_key,
                VerifyingKey,
                KagemushaMintAuthorizationTransportEpCircuitV1,
                generated.ep_circuit_params
            ),
            circuit_params: generated.ep_circuit_params.clone(),
            protocol_digest: generated.ep_protocol_digest,
            inner_proving_key: decode!(
                generated.inner_ep_proving_key,
                ProvingKey,
                KagemushaMintAuthorizationEpCircuitV1,
                generated.inner_ep_circuit_params
            ),
            inner_verifying_key: decode!(
                generated.inner_ep_verifying_key,
                VerifyingKey,
                KagemushaMintAuthorizationEpCircuitV1,
                generated.inner_ep_circuit_params
            ),
            inner_circuit_params: generated.inner_ep_circuit_params.clone(),
            release_id: context.release_id,
            profile_digest,
            artifact_manifest_digest: context.artifact_manifest_digest,
            suite_id: context.suite_id,
            vk_digest: context.vk_digest,
            enabled_hardware_profiles: generated.enabled_hardware_profiles,
        };
        let eq_protocol = compile(
            &eq.parameters,
            &eq.verifying_key,
            snark_verifier::system::halo2::Config::ipa()
                .with_num_instance(vec![MINT_AUTHORIZATION_PUBLIC_INSTANCE_COUNT_V1]),
        );
        let ep_protocol = compile(
            &ep.parameters,
            &ep.verifying_key,
            snark_verifier::system::halo2::Config::ipa()
                .with_num_instance(vec![MINT_AUTHORIZATION_PUBLIC_INSTANCE_COUNT_V1]),
        );
        assert_eq!(
            native_parent_protocol_digest_v1(&eq_protocol, KagemushaPastaParityV1::Eq)
                .expect("Eq mint-authorization identity"),
            eq.protocol_digest,
        );
        assert_eq!(
            native_parent_protocol_digest_v1(&ep_protocol, KagemushaPastaParityV1::Ep)
                .expect("Ep mint-authorization identity"),
            ep.protocol_digest,
        );
        drop(generated);
        halo2_proofs::release_allocator_slack();
        Self {
            eq,
            ep,
            eq_protocol,
            ep_protocol,
            recursive_inputs,
        }
    }

    fn generate(
        material: &MintRecipientMaterial,
        eq_params: &ParamsIPA<EqAffine>,
        ep_params: &ParamsIPA<EpAffine>,
        eq_hash: &KagemushaLoadedEqMintHashArtifactsV1,
        ep_hash: &KagemushaLoadedEpMintHashArtifactsV1,
        credential_keys: &CredentialKeys,
        credential: &CredentialProof,
    ) -> Self {
        let mut enabled_hardware_profiles = [[0; 32]; KAGEMUSHA_ENABLED_HARDWARE_PROFILE_SLOTS_V1];
        enabled_hardware_profiles[0] = material.hardware_profile.hardware_profile_id;
        let recursive_inputs = PreparedMintAuthorizationRecursiveInputs::new(
            material,
            eq_params,
            ep_params,
            eq_hash,
            ep_hash,
            &enabled_hardware_profiles,
            credential,
        );
        let witness = recursive_inputs.witness(
            material,
            enabled_hardware_profiles,
            eq_hash,
            ep_hash,
            credential_keys,
            credential,
        );
        let generated =
            generate_kagemusha_mint_authorization_artifacts_v1(witness, &test_only_recovery_seed())
                .expect("generate real MintAuthorization artifacts");
        Self::decode(
            generated,
            &material.authorization_relation,
            recursive_inputs,
        )
    }

    fn prove(
        &self,
        material: &MintRecipientMaterial,
        eq_hash: &KagemushaLoadedEqMintHashArtifactsV1,
        ep_hash: &KagemushaLoadedEpMintHashArtifactsV1,
        credential_keys: &CredentialKeys,
        credential: &CredentialProof,
    ) -> ProvenMintAuthorization {
        let generated = prove_kagemusha_mint_authorization_v1(
            &self.eq,
            &self.ep,
            self.recursive_inputs.witness(
                material,
                self.eq.enabled_hardware_profiles,
                eq_hash,
                ep_hash,
                credential_keys,
                credential,
            ),
            &test_only_recovery_seed(),
        )
        .expect("prove real MintAuthorization");
        let authorization = KagemushaMintAuthorizationV1 {
            version: KAGEMUSHA_WIRE_VERSION_V1,
            statement: material.authorization_relation.statement.clone(),
            proof: generated.proof.clone(),
        };
        authorization
            .validate_shape()
            .expect("real MintAuthorization transport shape");
        let eq_history = KagemushaEqAccumulatorV1::try_from_bytes(&generated.proof.eq_history)
            .expect("decode MintAuthorization Eq history");
        let ep_history = KagemushaEpAccumulatorV1::try_from_bytes(&generated.proof.ep_history)
            .expect("decode MintAuthorization Ep history");
        ProvenMintAuthorization {
            authorization,
            generated,
            eq_history,
            ep_history,
        }
    }

    fn into_protocols(self) -> MintAuthorizationProtocols {
        let Self {
            eq,
            ep,
            eq_protocol,
            ep_protocol,
            recursive_inputs: _,
        } = self;
        let protocols = MintAuthorizationProtocols {
            eq_protocol,
            ep_protocol,
            eq_digest: eq.protocol_digest,
            ep_digest: ep.protocol_digest,
        };
        drop(eq);
        drop(ep);
        halo2_proofs::release_allocator_slack();
        protocols
    }
}

/// Reusable genuinely signed funding material, not an invented nonzero state.
struct FundingCertificate {
    bootstrap: KagemushaMintCertificateWitnessV1,
    finalized: KagemushaMintCertificateWitnessV1,
    genesis_roster_id: DigestV1,
}

impl FundingCertificate {
    fn new(state: &KagemushaStateV1, recipient: AccountId, amount: u128) -> Self {
        assert!(
            amount > 0,
            "a payment corridor must begin with positive funding"
        );
        let statement = KagemushaMintCreditStatementV1 {
            version: KAGEMUSHA_WIRE_VERSION_V1,
            lifecycle: KagemushaLifecycleBindingV1 {
                version: KAGEMUSHA_WIRE_VERSION_V1,
                network_id: state.lane.network_id,
                protocol_version: KAGEMUSHA_WIRE_VERSION_V1,
                suite_id: state.suite_id,
                vk_digest: state.vk_digest,
                release_id: state.release_id,
                asset: state.lane.asset.clone(),
                asset_incarnation: state.asset_incarnation,
                scale: state.lane.scale,
                liability_pool_id: state.liability_pool_id,
                hardware_profile_id: state.hardware_profile_id,
                policy_epoch: state.policy_epoch,
                operation_kind: KagemushaOperationKindV1::MintFold,
                request_id: [0; 32],
                receiver_lane_commitment: [0; 32],
                credit_id: [0; 32],
                ciphertext_digest: digest(b"funding-ciphertext", 0),
            },
            recipient_credential_commitment: digest(b"funding-recipient-credential", 0),
            authorization_context_digest: digest(b"funding-authorization-context", 0),
            mint_authorization_digest: digest(b"funding-authorization", 0),
            amount,
            issuance_commitment: digest(b"funding-issuance", 0),
            recipient,
            credit_commitment: digest(b"funding-credit", 0),
            minted_at_ms: 100,
        }
        .seal_credit_id()
        .expect("seal positive-value funding statement");
        Self::from_statement(statement)
    }

    fn from_statement(statement: KagemushaMintCreditStatementV1) -> Self {
        statement
            .validate_shape()
            .expect("positive authorization-bound funding statement");
        let amount = statement.amount;
        let mut validators = (0_u8..4)
            .map(|index| {
                let key = KeyPair::from_seed(vec![index + 1; 32], Algorithm::Ed25519);
                ValidatorPower {
                    validator: PeerId::new(key.public_key().clone()),
                    power: 1,
                }
            })
            .collect::<Vec<_>>();
        validators.sort_by(|left, right| left.validator.cmp(&right.validator));
        let roster = crate::kagemusha_v1_test_fixtures::mint_finality_roster(
            statement.lifecycle.network_id,
            0,
            &validators,
        );
        let genesis_roster_id = roster.finality_epoch_id().expect("finality roster ID");
        let leaf = KagemushaTopUpLeafV1 {
            version: KAGEMUSHA_CHAIN_VERSION_V1,
            operation_id: digest(b"funding-operation", 0),
            reserve_receipt_digest: digest(b"funding-reserve-receipt", 0),
            statement_digest: statement
                .canonical_digest()
                .expect("funding statement digest"),
            amount,
        };
        let tree = KagemushaMintFinalityTreeV1::new(vec![leaf.clone()])
            .expect("genuine sparse top-up membership tree");
        let message = Self::message(
            &roster,
            genesis_roster_id,
            tree.execution_root(),
            tree.leaf_count(),
            None,
        );
        let seals = (0_u32..3)
            .map(|index| {
                let signer = KagemushaMintFinalitySignerV1::from_seed(
                    Zeroizing::new([0xA0 + u8::try_from(index).expect("small signer index"); 32]),
                    index,
                    &roster,
                )
                .expect("real signer admitted by its paired-Pasta roster keys");
                signer
                    .sign(&message)
                    .expect("real paired-Pasta finality seal")
            })
            .collect();
        let finalized = KagemushaMintCertificateWitnessV1 {
            statement: statement.clone(),
            membership: tree
                .witness(leaf.operation_id)
                .expect("funding membership path"),
            seal_bundle: KagemushaMintFinalitySealBundleV1 { message, seals },
            epoch_roster: roster.clone(),
        };
        finalized
            .validate_shape()
            .expect("valid positive-value funding certificate");
        let empty_root = kagemusha_mint_finality_empty_root_v1().expect("empty finality root");
        let bootstrap = KagemushaMintCertificateWitnessV1 {
            statement,
            membership: KagemushaTopUpMembershipWitnessV1 {
                leaf,
                leaf_index: 0,
                root: empty_root,
                siblings: vec![
                    KagemushaPastaStateCommitmentV1::ZERO;
                    KAGEMUSHA_MINT_FINALITY_TREE_DEPTH_V1
                ],
            },
            seal_bundle: KagemushaMintFinalitySealBundleV1 {
                message: Self::message(
                    &roster,
                    genesis_roster_id,
                    kagemusha_mint_finality_root_v1(empty_root),
                    0,
                    Some(genesis_roster_id),
                ),
                seals: Vec::new(),
            },
            epoch_roster: roster,
        };
        bootstrap
            .validate_for_step(KagemushaMintAuthorityStepV1::Bootstrap)
            .expect("zero-authority bootstrap certificate shape");
        Self {
            bootstrap,
            finalized,
            genesis_roster_id,
        }
    }

    fn message(
        roster: &KagemushaMintFinalityEpochRosterV1,
        epoch_id: DigestV1,
        root: Hash,
        count: u32,
        next: Option<DigestV1>,
    ) -> KagemushaMintFinalitySealMessageV1 {
        KagemushaMintFinalitySealMessageV1 {
            version: KAGEMUSHA_CHAIN_VERSION_V1,
            finality_epoch_id: epoch_id,
            validator_count: u32::try_from(roster.validators.len()).expect("four validators"),
            network_id: roster.network_id,
            block_height: if count == 0 { 1 } else { 2 },
            height_context_id: HeightContextId(HashOf::from_untyped_unchecked(Hash::new(digest(
                b"funding-height",
                u64::from(count),
            )))),
            subject_digest: digest(b"funding-subject", u64::from(count)),
            execution_commitment_digest: digest(b"funding-execution", u64::from(count)),
            kagemusha_top_up_root: root,
            kagemusha_top_up_count: count,
            next_finality_epoch_id: next,
        }
    }
}

fn generate_mint_hash_suite(
    material: &MintRecipientMaterial,
    eq: &ParamsIPA<EqAffine>,
    ep: &ParamsIPA<EpAffine>,
) -> (
    KagemushaLoadedEqMintHashArtifactsV1,
    KagemushaLoadedEpMintHashArtifactsV1,
) {
    let seed_funding =
        FundingCertificate::from_statement(material.finalized_statement_with_authorization_digest(
            digest(b"mint-hash-key-shape-seed-authorization", 0),
        ));
    let (eq_claim_seed, ep_claim_seed) = mint_hash_claim_protocol_seeds(eq, ep);
    let generated = generate_kagemusha_mint_hash_artifacts_for_guarded_test_v1(
        KagemushaMintHashArtifactGenerationWitnessV1 {
            release_id: material.authorization_relation.statement.context.release_id,
            certificate: &seed_funding.finalized,
            step: KagemushaMintAuthorityStepV1::FinalizedMint,
            eq_claim_protocol_seed: &eq_claim_seed,
            ep_claim_protocol_seed: &ep_claim_seed,
            recovery_seed: &test_only_recovery_seed(),
        },
    )
    .expect("generate reusable typed-SHA shard/claim artifacts");
    drop(eq_claim_seed);
    drop(ep_claim_seed);
    drop(seed_funding);
    halo2_proofs::release_allocator_slack();

    let key_sizes = [
        generated.eq_shard_proving_key.len(),
        generated.eq_shard_verifying_key.len(),
        generated.ep_shard_proving_key.len(),
        generated.ep_shard_verifying_key.len(),
        generated.eq_claim_proving_key.len(),
        generated.eq_claim_verifying_key.len(),
        generated.ep_claim_proving_key.len(),
        generated.ep_claim_verifying_key.len(),
    ];
    eprintln!(
        "KAGEMUSHA reusable typed-SHA artifact bytes Eq shard PK/VK={} / {}, Ep shard PK/VK={} / {}, Eq claim PK/VK={} / {}, Ep claim PK/VK={} / {}; total={}",
        key_sizes[0],
        key_sizes[1],
        key_sizes[2],
        key_sizes[3],
        key_sizes[4],
        key_sizes[5],
        key_sizes[6],
        key_sizes[7],
        key_sizes.iter().sum::<usize>(),
    );
    let context = &material.authorization_relation.statement.context;
    let loaded = generated
        .into_loaded_for_testing(
            digest(b"funding-test-profile-identity", 0),
            context.artifact_manifest_digest,
            context.suite_id,
            context.vk_digest,
        )
        .expect("decode reusable typed-SHA shard/claim artifacts");
    halo2_proofs::release_allocator_slack();
    loaded
}

/// Loaded keys retained across the bootstrap and every funded mint proof.
struct MintKeys {
    eq: KagemushaLoadedEqMintAuthorityArtifactsV1,
    ep: KagemushaLoadedEpMintAuthorityArtifactsV1,
    hash_eq: KagemushaLoadedEqMintHashArtifactsV1,
    hash_ep: KagemushaLoadedEpMintHashArtifactsV1,
    eq_protocol: PlonkProtocol<EqAffine>,
    ep_protocol: PlonkProtocol<EpAffine>,
    eq_digest: DigestV1,
    ep_digest: DigestV1,
    bootstrap_hash_claim: KagemushaGeneratedMintHashClaimV1,
    finalized_hash_claim: KagemushaGeneratedMintHashClaimV1,
}

/// Compact mint verifier state retained after both authority proofs have been decided.
struct MintProtocols {
    eq_protocol: PlonkProtocol<EqAffine>,
    ep_protocol: PlonkProtocol<EpAffine>,
    eq_digest: DigestV1,
    ep_digest: DigestV1,
}

impl MintKeys {
    fn decode(
        generated: KagemushaGeneratedMintAuthorityArtifactsV1,
        hash_eq: KagemushaLoadedEqMintHashArtifactsV1,
        hash_ep: KagemushaLoadedEpMintHashArtifactsV1,
        bootstrap_hash_claim: KagemushaGeneratedMintHashClaimV1,
        finalized_hash_claim: KagemushaGeneratedMintHashClaimV1,
    ) -> Self {
        macro_rules! decode {
            ($bytes:expr, $key:ident, $circuit:ty, $params:expr) => {{
                let mut cursor = Cursor::new($bytes.as_ref());
                let key = $key::read_checked::<_, $circuit>(
                    &mut cursor,
                    SerdeFormat::Processed,
                    u32::try_from($params.k).expect("generated mint circuit degree"),
                    $params.clone(),
                )
                .expect("decode generated mint-authority key");
                assert_eq!(
                    usize::try_from(cursor.position()).expect("key cursor"),
                    $bytes.len()
                );
                key
            }};
        }
        // This direct-decoding fixture has no authenticated release profile or manifest.
        // These shared labels are test-only metadata, never release-authentication evidence;
        // the production loader obtains both identities from its authenticated artifact set.
        assert_eq!(hash_eq.profile_digest, hash_ep.profile_digest);
        assert_eq!(
            hash_eq.artifact_manifest_digest,
            hash_ep.artifact_manifest_digest
        );
        let test_only_profile_digest = hash_eq.profile_digest;
        let test_only_manifest_digest = hash_eq.artifact_manifest_digest;
        let eq = KagemushaLoadedEqMintAuthorityArtifactsV1 {
            parameters: ParamsIPA::read(&mut Cursor::new(generated.eq_parameters.as_ref()))
                .expect("Eq mint parameters"),
            proving_key: decode!(
                generated.eq_proving_key,
                ProvingKey,
                KagemushaMintAuthorityTransportEqCircuitV1,
                generated.eq_circuit_params
            ),
            verifying_key: decode!(
                generated.eq_verifying_key,
                VerifyingKey,
                KagemushaMintAuthorityTransportEqCircuitV1,
                generated.eq_circuit_params
            ),
            circuit_params: generated.eq_circuit_params.clone(),
            inner_proving_key: decode!(
                generated.inner_eq_proving_key,
                ProvingKey,
                KagemushaMintAuthorityEqCircuitV1,
                generated.inner_eq_circuit_params
            ),
            inner_verifying_key: decode!(
                generated.inner_eq_verifying_key,
                VerifyingKey,
                KagemushaMintAuthorityEqCircuitV1,
                generated.inner_eq_circuit_params
            ),
            inner_circuit_params: generated.inner_eq_circuit_params.clone(),
            protocol_digest: generated.eq_protocol_digest,
            release_id: generated.release_id,
            profile_digest: test_only_profile_digest,
            artifact_manifest_digest: test_only_manifest_digest,
            genesis_roster_id: generated.genesis_roster_id,
        };
        let ep = KagemushaLoadedEpMintAuthorityArtifactsV1 {
            parameters: ParamsIPA::read(&mut Cursor::new(generated.ep_parameters.as_ref()))
                .expect("Ep mint parameters"),
            proving_key: decode!(
                generated.ep_proving_key,
                ProvingKey,
                KagemushaMintAuthorityTransportEpCircuitV1,
                generated.ep_circuit_params
            ),
            verifying_key: decode!(
                generated.ep_verifying_key,
                VerifyingKey,
                KagemushaMintAuthorityTransportEpCircuitV1,
                generated.ep_circuit_params
            ),
            circuit_params: generated.ep_circuit_params.clone(),
            inner_proving_key: decode!(
                generated.inner_ep_proving_key,
                ProvingKey,
                KagemushaMintAuthorityEpCircuitV1,
                generated.inner_ep_circuit_params
            ),
            inner_verifying_key: decode!(
                generated.inner_ep_verifying_key,
                VerifyingKey,
                KagemushaMintAuthorityEpCircuitV1,
                generated.inner_ep_circuit_params
            ),
            inner_circuit_params: generated.inner_ep_circuit_params.clone(),
            protocol_digest: generated.ep_protocol_digest,
            release_id: generated.release_id,
            profile_digest: test_only_profile_digest,
            artifact_manifest_digest: test_only_manifest_digest,
            genesis_roster_id: generated.genesis_roster_id,
        };
        let eq_protocol = compile(
            &eq.parameters,
            &eq.verifying_key,
            snark_verifier::system::halo2::Config::ipa()
                .with_num_instance(vec![KAGEMUSHA_MINT_AUTHORITY_PUBLIC_INSTANCE_COUNT_V1]),
        );
        let ep_protocol = compile(
            &ep.parameters,
            &ep.verifying_key,
            snark_verifier::system::halo2::Config::ipa()
                .with_num_instance(vec![KAGEMUSHA_MINT_AUTHORITY_PUBLIC_INSTANCE_COUNT_V1]),
        );
        assert_eq!(
            native_parent_protocol_digest_v1(&eq_protocol, KagemushaPastaParityV1::Eq)
                .expect("Eq mint identity"),
            generated.eq_protocol_digest
        );
        assert_eq!(
            native_parent_protocol_digest_v1(&ep_protocol, KagemushaPastaParityV1::Ep)
                .expect("Ep mint identity"),
            generated.ep_protocol_digest
        );
        let eq_digest = generated.eq_protocol_digest;
        let ep_digest = generated.ep_protocol_digest;
        drop(generated);
        halo2_proofs::release_allocator_slack();
        Self {
            eq,
            ep,
            hash_eq,
            hash_ep,
            eq_protocol,
            ep_protocol,
            eq_digest,
            ep_digest,
            bootstrap_hash_claim,
            finalized_hash_claim,
        }
    }

    fn generate(
        eq_params: &ParamsIPA<EqAffine>,
        ep_params: &ParamsIPA<EpAffine>,
        eq_seed: PlonkProtocol<EqAffine>,
        ep_seed: PlonkProtocol<EpAffine>,
        hash_eq: KagemushaLoadedEqMintHashArtifactsV1,
        hash_ep: KagemushaLoadedEpMintHashArtifactsV1,
        funding: &FundingCertificate,
    ) -> Self {
        let eq_history =
            initial_kagemusha_eq_accumulator_v1(eq_params).expect("Eq mint seed history");
        let ep_history =
            initial_kagemusha_ep_accumulator_v1(ep_params).expect("Ep mint seed history");
        // The generator owns inner/outer key-shape convergence and returns stable outer keys.
        let padding = MintPadding::new(&eq_seed, &ep_seed, &eq_history, &ep_history);
        let started = std::time::Instant::now();
        let bootstrap_claim = prove_kagemusha_mint_hash_claim_v1(
            &hash_eq,
            &hash_ep,
            &funding.bootstrap,
            KagemushaMintAuthorityStepV1::Bootstrap,
            &test_only_recovery_seed(),
        )
        .expect("prove complete bootstrap mint-hash claim");
        let finalized_hash_claim = prove_kagemusha_mint_hash_claim_v1(
            &hash_eq,
            &hash_ep,
            &funding.finalized,
            KagemushaMintAuthorityStepV1::FinalizedMint,
            &test_only_recovery_seed(),
        )
        .expect("prove reusable finalized-mint hash claim");
        let eq_claim_merge = fold_kagemusha_eq_accumulators_v1(
            eq_params,
            &eq_history,
            &bootstrap_claim.eq_complete_history,
            &test_only_recovery_seed(),
        )
        .expect("merge bootstrap Eq hash claim into authority history");
        let ep_claim_merge = fold_kagemusha_ep_accumulators_v1(
            ep_params,
            &ep_history,
            &bootstrap_claim.ep_complete_history,
            &test_only_recovery_seed(),
        )
        .expect("merge bootstrap Ep hash claim into authority history");
        let claim_witness = bootstrap_claim
            .mint_authority_witness(
                &hash_eq,
                &hash_ep,
                eq_claim_merge.proof(),
                ep_claim_merge.proof(),
            )
            .expect("bind bootstrap hash claim to MintAuthority");
        let keys = Self::decode(
            generate_kagemusha_mint_authority_artifacts_v1(padding.witness(
                funding,
                &eq_seed,
                &ep_seed,
                &eq_history,
                &ep_history,
                claim_witness,
                eq_claim_merge.successor(),
                ep_claim_merge.successor(),
            ))
            .expect("generate real mint-authority keys"),
            hash_eq,
            hash_ep,
            bootstrap_claim,
            finalized_hash_claim,
        );
        eprintln!(
            "KAGEMUSHA converged mint-authority key generation: {:?}",
            started.elapsed()
        );
        keys
    }

    fn prove_funding(&self, funding: &FundingCertificate) -> ProvenFunding {
        let eq_initial = initial_kagemusha_eq_accumulator_v1(&self.eq.parameters)
            .expect("Eq mint initial history");
        let ep_initial = initial_kagemusha_ep_accumulator_v1(&self.ep.parameters)
            .expect("Ep mint initial history");
        let padding = MintPadding::new(
            &self.eq_protocol,
            &self.ep_protocol,
            &eq_initial,
            &ep_initial,
        );
        let bootstrap_claim = &self.bootstrap_hash_claim;
        let eq_bootstrap_claim_merge = fold_kagemusha_eq_accumulators_v1(
            &self.eq.parameters,
            &eq_initial,
            &bootstrap_claim.eq_complete_history,
            &test_only_recovery_seed(),
        )
        .expect("merge bootstrap Eq hash claim");
        let ep_bootstrap_claim_merge = fold_kagemusha_ep_accumulators_v1(
            &self.ep.parameters,
            &ep_initial,
            &bootstrap_claim.ep_complete_history,
            &test_only_recovery_seed(),
        )
        .expect("merge bootstrap Ep hash claim");
        let bootstrap_claim_witness = bootstrap_claim
            .mint_authority_witness(
                &self.hash_eq,
                &self.hash_ep,
                eq_bootstrap_claim_merge.proof(),
                ep_bootstrap_claim_merge.proof(),
            )
            .expect("bind bootstrap hash claim");
        let started = std::time::Instant::now();
        let bootstrap = prove_kagemusha_mint_authority_v1(
            &self.eq,
            &self.ep,
            padding.witness(
                funding,
                &self.eq_protocol,
                &self.ep_protocol,
                &eq_initial,
                &ep_initial,
                bootstrap_claim_witness,
                eq_bootstrap_claim_merge.successor(),
                ep_bootstrap_claim_merge.successor(),
            ),
        )
        .expect("real bootstrap mint-authority proof");
        // The compact proof already includes its private carrier in its history.
        // The next authority step must extend that transported history, not the empty seed.
        let eq_bootstrap_history =
            KagemushaEqAccumulatorV1::try_from_bytes(&bootstrap.proof.eq_history)
                .expect("decode actual Eq bootstrap history");
        let ep_bootstrap_history =
            KagemushaEpAccumulatorV1::try_from_bytes(&bootstrap.proof.ep_history)
                .expect("decode actual Ep bootstrap history");
        self.decide(&bootstrap, &eq_bootstrap_history, &ep_bootstrap_history);
        eprintln!(
            "KAGEMUSHA genuine mint-authority bootstrap proof: {:?}",
            started.elapsed()
        );
        let eq_fold = fold_kagemusha_eq_accumulators_with_rng_v1(
            &self.eq.parameters,
            &bootstrap.eq_current_accumulator,
            &eq_bootstrap_history,
            OsRng,
        )
        .expect("fold actual Eq bootstrap proof");
        let ep_fold = fold_kagemusha_ep_accumulators_with_rng_v1(
            &self.ep.parameters,
            &bootstrap.ep_current_accumulator,
            &ep_bootstrap_history,
            OsRng,
        )
        .expect("fold actual Ep bootstrap proof");
        let eq_instances = vec![bootstrap.eq_public_instances.clone()];
        let ep_instances = vec![bootstrap.ep_public_instances.clone()];
        let finalized_claim = &self.finalized_hash_claim;
        let eq_finalized_claim_merge = fold_kagemusha_eq_accumulators_v1(
            &self.eq.parameters,
            eq_fold.successor(),
            &finalized_claim.eq_complete_history,
            &test_only_recovery_seed(),
        )
        .expect("merge finalized Eq hash claim");
        let ep_finalized_claim_merge = fold_kagemusha_ep_accumulators_v1(
            &self.ep.parameters,
            ep_fold.successor(),
            &finalized_claim.ep_complete_history,
            &test_only_recovery_seed(),
        )
        .expect("merge finalized Ep hash claim");
        let finalized_claim_witness = finalized_claim
            .mint_authority_witness(
                &self.hash_eq,
                &self.hash_ep,
                eq_finalized_claim_merge.proof(),
                ep_finalized_claim_merge.proof(),
            )
            .expect("bind finalized mint hash claim");
        let started = std::time::Instant::now();
        let finalized = prove_kagemusha_mint_authority_v1(
            &self.eq,
            &self.ep,
            KagemushaMintAuthorityGenerationWitnessV1 {
                step: KagemushaMintAuthorityStepV1::FinalizedMint,
                release_id: funding.finalized.statement.lifecycle.release_id,
                genesis_roster_id: funding.genesis_roster_id,
                eq_protocol_digest: self.eq_digest,
                ep_protocol_digest: self.ep_digest,
                eq_deferred_audit: [1; 32],
                ep_deferred_audit: [2; 32],
                certificate: funding.finalized.clone(),
                mint_hash_claim: Some(finalized_claim_witness),
                eq_parent_protocol: &self.eq_protocol,
                ep_parent_protocol: &self.ep_protocol,
                eq_parent_instances: &eq_instances,
                ep_parent_instances: &ep_instances,
                eq_parent_proof: &bootstrap.proof.eq_proof,
                ep_parent_proof: &bootstrap.proof.ep_proof,
                eq_parent_history: &eq_bootstrap_history,
                ep_parent_history: &ep_bootstrap_history,
                eq_parent_fold_proof: eq_fold.proof(),
                ep_parent_fold_proof: ep_fold.proof(),
                eq_successor_history: eq_finalized_claim_merge.successor(),
                ep_successor_history: ep_finalized_claim_merge.successor(),
            },
        )
        .expect("real quorum-backed finalized-mint proof");
        let eq_finalized_history =
            KagemushaEqAccumulatorV1::try_from_bytes(&finalized.proof.eq_history)
                .expect("decode actual Eq finalized-mint history");
        let ep_finalized_history =
            KagemushaEpAccumulatorV1::try_from_bytes(&finalized.proof.ep_history)
                .expect("decode actual Ep finalized-mint history");
        self.decide(&finalized, &eq_finalized_history, &ep_finalized_history);
        eprintln!(
            "KAGEMUSHA genuine finalized-mint proof: {:?}; Eq={} Ep={} bytes",
            started.elapsed(),
            finalized.proof.eq_proof.len(),
            finalized.proof.ep_proof.len()
        );
        ProvenFunding {
            proof: finalized,
            eq_history: eq_finalized_history,
            ep_history: ep_finalized_history,
        }
    }

    fn decide(
        &self,
        proof: &KagemushaGeneratedMintAuthorityProofV1,
        eq_history: &KagemushaEqAccumulatorV1,
        ep_history: &KagemushaEpAccumulatorV1,
    ) {
        assert_eq!(
            eq_history.as_bytes().as_slice(),
            proof.proof.eq_history.as_slice(),
            "the decided Eq ancestry must be the transported proof history"
        );
        assert_eq!(
            ep_history.as_bytes().as_slice(),
            proof.proof.ep_history.as_slice(),
            "the decided Ep ancestry must be the transported proof history"
        );
        let eq_current = KagemushaEqAccumulatorV1::from_native(
            &verify_eq_succinct_protocol(
                &self.eq.parameters,
                &self.eq_protocol,
                &proof.proof.eq_proof,
                &proof.eq_public_instances,
            )
            .expect("verify actual Eq mint proof"),
        )
        .expect("Eq mint accumulator");
        let ep_current = KagemushaEpAccumulatorV1::from_native(
            &verify_ep_succinct_protocol(
                &self.ep.parameters,
                &self.ep_protocol,
                &proof.proof.ep_proof,
                &proof.ep_public_instances,
            )
            .expect("verify actual Ep mint proof"),
        )
        .expect("Ep mint accumulator");
        assert_eq!(eq_current, proof.eq_current_accumulator);
        assert_eq!(ep_current, proof.ep_current_accumulator);
        decide_kagemusha_eq_accumulator_v1(&self.eq.parameters, &eq_current)
            .expect("decide actual Eq mint equation");
        decide_kagemusha_ep_accumulator_v1(&self.ep.parameters, &ep_current)
            .expect("decide actual Ep mint equation");
        decide_kagemusha_eq_accumulator_v1(&self.eq.parameters, eq_history)
            .expect("decide actual Eq mint ancestry");
        decide_kagemusha_ep_accumulator_v1(&self.ep.parameters, ep_history)
            .expect("decide actual Ep mint ancestry");
        assert!(
            proof.proof.eq_proof.len() + proof.proof.ep_proof.len()
                <= KAGEMUSHA_PAIRED_PROOF_MAX_BYTES_V1
        );
    }

    fn into_protocols(
        self,
    ) -> (
        MintProtocols,
        KagemushaLoadedEqMintHashArtifactsV1,
        KagemushaLoadedEpMintHashArtifactsV1,
    ) {
        let Self {
            eq,
            ep,
            hash_eq,
            hash_ep,
            eq_protocol,
            ep_protocol,
            eq_digest,
            ep_digest,
            bootstrap_hash_claim,
            finalized_hash_claim,
        } = self;
        let protocols = MintProtocols {
            eq_protocol,
            ep_protocol,
            eq_digest,
            ep_digest,
        };
        drop(eq);
        drop(ep);
        drop(bootstrap_hash_claim);
        drop(finalized_hash_claim);
        halo2_proofs::release_allocator_slack();
        (protocols, hash_eq, hash_ep)
    }
}

struct ProvenFunding {
    proof: KagemushaGeneratedMintAuthorityProofV1,
    eq_history: KagemushaEqAccumulatorV1,
    ep_history: KagemushaEpAccumulatorV1,
}

impl ProvenFunding {
    fn mint_credit(
        &self,
        material: &MintRecipientMaterial,
        authorization: &KagemushaMintAuthorizationV1,
    ) -> iroha_data_model::kagemusha::KagemushaMintCreditV1 {
        let credit = iroha_data_model::kagemusha::KagemushaMintCreditV1 {
            version: KAGEMUSHA_WIRE_VERSION_V1,
            statement: material.finalized_statement(authorization),
            proof: self.proof.proof.clone(),
            finality_certificate_binding: self.proof.certificate_binding,
            finality_authority_head: self.proof.authority_head,
            finality_genesis_roster_id: self.proof.genesis_roster_id,
            finality_proof_binding_digest: self.proof.proof_binding_digest,
            encrypted_credit: material.authorization_relation.encrypted_credit.clone(),
            artifact_manifest_digest: authorization.statement.context.artifact_manifest_digest,
        };
        credit
            .validate_shape_against_authorization(authorization)
            .expect("genuine finalized mint credit and recipient authorization must bind");
        credit
    }
}

/// Inactive bootstrap parsing material; explicitly not an accepted predecessor proof.
struct MintPadding {
    eq_instances: Vec<Vec<Fp>>,
    ep_instances: Vec<Vec<Fq>>,
    eq_proof: Vec<u8>,
    ep_proof: Vec<u8>,
    eq_fold: KagemushaEqFoldProofV1,
    ep_fold: KagemushaEpFoldProofV1,
}

impl MintPadding {
    fn new(
        eq: &PlonkProtocol<EqAffine>,
        ep: &PlonkProtocol<EpAffine>,
        eq_history: &KagemushaEqAccumulatorV1,
        ep_history: &KagemushaEpAccumulatorV1,
    ) -> Self {
        assert_eq!(
            eq.num_instance,
            vec![KAGEMUSHA_MINT_AUTHORITY_PUBLIC_INSTANCE_COUNT_V1]
        );
        assert_eq!(
            ep.num_instance,
            vec![KAGEMUSHA_MINT_AUTHORITY_PUBLIC_INSTANCE_COUNT_V1]
        );
        Self {
            eq_instances: history_instances(mint_instance::HISTORY_START, eq_history.as_bytes()),
            ep_instances: history_instances(mint_instance::HISTORY_START, ep_history.as_bytes()),
            eq_proof: dummy_ordinary_proof(eq, EqAffine::generator()),
            ep_proof: dummy_ordinary_proof(ep, EpAffine::generator()),
            eq_fold: dummy_eq_fold(),
            ep_fold: dummy_ep_fold(),
        }
    }

    fn witness<'a>(
        &'a self,
        funding: &FundingCertificate,
        eq: &'a PlonkProtocol<EqAffine>,
        ep: &'a PlonkProtocol<EpAffine>,
        eq_history: &'a KagemushaEqAccumulatorV1,
        ep_history: &'a KagemushaEpAccumulatorV1,
        mint_hash_claim: KagemushaMintHashClaimGenerationWitnessV1<'a>,
        eq_successor_history: &'a KagemushaEqAccumulatorV1,
        ep_successor_history: &'a KagemushaEpAccumulatorV1,
    ) -> KagemushaMintAuthorityGenerationWitnessV1<'a> {
        KagemushaMintAuthorityGenerationWitnessV1 {
            step: KagemushaMintAuthorityStepV1::Bootstrap,
            release_id: funding.bootstrap.statement.lifecycle.release_id,
            genesis_roster_id: funding.genesis_roster_id,
            eq_protocol_digest: native_parent_protocol_digest_v1(eq, KagemushaPastaParityV1::Eq)
                .expect("Eq mint parent identity"),
            ep_protocol_digest: native_parent_protocol_digest_v1(ep, KagemushaPastaParityV1::Ep)
                .expect("Ep mint parent identity"),
            eq_deferred_audit: [1; 32],
            ep_deferred_audit: [2; 32],
            certificate: funding.bootstrap.clone(),
            mint_hash_claim: Some(mint_hash_claim),
            eq_parent_protocol: eq,
            ep_parent_protocol: ep,
            eq_parent_instances: &self.eq_instances,
            ep_parent_instances: &self.ep_instances,
            eq_parent_proof: &self.eq_proof,
            ep_parent_proof: &self.ep_proof,
            eq_parent_history: eq_history,
            ep_parent_history: ep_history,
            eq_parent_fold_proof: &self.eq_fold,
            ep_parent_fold_proof: &self.ep_fold,
            eq_successor_history,
            ep_successor_history,
        }
    }
}

fn funding_fixture() -> (
    KagemushaPlatformCredentialRelationWitnessV1,
    KagemushaStateV1,
    FundingCertificate,
) {
    let release = digest(b"payment-corridor-release", 0);
    let (credential, _) = credential_witness(0, release, digest(b"empty-durable-effect", 0));
    let state = aggregate_state(
        release,
        &credential.statement,
        digest(b"payment-state-nonce", 0),
    );
    let recipient = AccountId::new(
        KeyPair::from_seed(vec![91; 32], Algorithm::Ed25519)
            .public_key()
            .clone(),
    );
    let funding = FundingCertificate::new(&state, recipient, 1_000);
    (credential, state, funding)
}

// Independently check the actual Schnorr equations for fixture preflight. This test-only
// check does not replace the production mint circuit or authenticate a consensus epoch.
fn signature_equation<C: CurveAffine>(
    parity: u8,
    validator_index: u32,
    signing_digest: DigestV1,
    public_key: DigestV1,
    signature: &KagemushaPastaSchnorrSignatureV1,
) -> bool
where
    C::ScalarExt: KagemushaPoseidonFieldV1,
{
    let point = |bytes: &DigestV1| {
        let mut repr = C::Repr::default();
        repr.as_mut().copy_from_slice(bytes);
        Option::<C>::from(C::from_bytes(&repr)).filter(|value| !bool::from(value.is_identity()))
    };
    let Some(public) = point(&public_key) else {
        return false;
    };
    let Some(nonce) = point(&signature.nonce_commitment) else {
        return false;
    };
    let Some(response) = decode_pasta::<C::ScalarExt>(signature.response) else {
        return false;
    };
    if bool::from(response.is_zero()) {
        return false;
    }
    let mut hash = Sha256::new();
    hash.update(b"iroha:kagemusha:v1:mint-finality:challenge");
    hash.update([0, parity]);
    hash.update(validator_index.to_le_bytes());
    hash.update(signing_digest);
    hash.update(signature.nonce_commitment);
    hash.update(public_key);
    let digest: DigestV1 = hash.finalize().into();
    let challenge = from_u128::<C::ScalarExt>(u128::from_le_bytes(
        digest[..16].try_into().expect("challenge half"),
    ));
    C::CurveExt::generator() * response
        == C::CurveExt::from(nonce) + C::CurveExt::from(public) * challenge
}

fn certificate_signature_equations(certificate: &KagemushaMintCertificateWitnessV1) -> bool {
    let digest = certificate
        .seal_bundle
        .message
        .signing_digest()
        .expect("fixture signing digest");
    certificate.seal_bundle.seals.iter().all(|seal| {
        let keys = &certificate.epoch_roster.validators
            [usize::try_from(seal.validator_index).expect("fixture signer")];
        signature_equation::<EpAffine>(
            0,
            seal.validator_index,
            digest,
            keys.eq_proof_public_key,
            &seal.eq_proof_signature,
        ) && signature_equation::<EqAffine>(
            1,
            seal.validator_index,
            digest,
            keys.ep_proof_public_key,
            &seal.ep_proof_signature,
        )
    })
}

/// Genuine recipient ownership, finalized reserve credit, and compact verifier protocols.
///
/// The placeholder identities accepted by this helper are witness values, not release authority.
/// The full handoff qualification rebuilds the same deterministic recipient against its sealed
/// 50-role test release before any production verifier accepts the resulting payment.
struct RealFundedPrerequisite {
    eq: ParamsIPA<EqAffine>,
    ep: ParamsIPA<EpAffine>,
    hash_eq: KagemushaLoadedEqMintHashArtifactsV1,
    hash_ep: KagemushaLoadedEpMintHashArtifactsV1,
    authorization_protocols: MintAuthorizationProtocols,
    authorization: ProvenMintAuthorization,
    genesis_roster_id: DigestV1,
    mint_protocols: MintProtocols,
    funded: ProvenFunding,
    mint_credit: iroha_data_model::kagemusha::KagemushaMintCreditV1,
}

fn prove_funded_prerequisite(
    release_id: DigestV1,
    vk_digest: DigestV1,
    artifact_manifest_digest: DigestV1,
    amount: u128,
) -> RealFundedPrerequisite {
    let eq = canonical_kagemusha_eq_parameters_v1();
    let ep = canonical_kagemusha_ep_parameters_v1();
    let material = mint_recipient_material(release_id, vk_digest, artifact_manifest_digest, amount);
    let (hash_eq, hash_ep) = generate_mint_hash_suite(&material, &eq, &ep);
    let credential_keys =
        CredentialKeys::generate(&eq, &ep, &hash_eq, &hash_ep, &material.platform_credential);
    let credential = credential_keys.prove(
        &eq,
        &ep,
        &hash_eq,
        &hash_ep,
        material.platform_credential.clone(),
        material.authorization_relation.device_authority_secret,
    );
    let authorization_keys = MintAuthorizationKeys::generate(
        &material,
        &eq,
        &ep,
        &hash_eq,
        &hash_ep,
        &credential_keys,
        &credential,
    );
    let authorization =
        authorization_keys.prove(&material, &hash_eq, &hash_ep, &credential_keys, &credential);
    let funding = FundingCertificate::from_statement(
        material.finalized_statement(&authorization.authorization),
    );
    let eq_seed = compile(
        &eq,
        credential_keys.eq_proving_key.get_vk(),
        snark_verifier::system::halo2::Config::ipa()
            .with_num_instance(vec![KAGEMUSHA_MINT_AUTHORITY_PUBLIC_INSTANCE_COUNT_V1]),
    );
    let ep_seed = compile(
        &ep,
        credential_keys.ep_proving_key.get_vk(),
        snark_verifier::system::halo2::Config::ipa()
            .with_num_instance(vec![KAGEMUSHA_MINT_AUTHORITY_PUBLIC_INSTANCE_COUNT_V1]),
    );
    let authorization_protocols = authorization_keys.into_protocols();
    drop(credential_keys);
    drop(credential);
    halo2_proofs::release_allocator_slack();
    let mint_keys = MintKeys::generate(&eq, &ep, eq_seed, ep_seed, hash_eq, hash_ep, &funding);
    let funded = mint_keys.prove_funding(&funding);
    mint_keys.decide(&funded.proof, &funded.eq_history, &funded.ep_history);
    let mint_credit = funded.mint_credit(&material, &authorization.authorization);
    let (mint_protocols, hash_eq, hash_ep) = mint_keys.into_protocols();
    let genesis_roster_id = funding.genesis_roster_id;
    drop(material);
    drop(funding);
    halo2_proofs::release_allocator_slack();
    RealFundedPrerequisite {
        eq,
        ep,
        hash_eq,
        hash_ep,
        authorization_protocols,
        authorization,
        genesis_roster_id,
        mint_protocols,
        funded,
        mint_credit,
    }
}

/// Fixed-shape CommitWrapper slot used by every state operation.
///
/// Before the real post-commit keys exist, key generation uses a parseable 81-instance carrier
/// with no monetary authority. The final `ReceiveFold` replaces every field with the genuine
/// CommitWrapper proof and its recursively authenticated history.
struct IncomingStateProofMaterial {
    eq_protocol: PlonkProtocol<EqAffine>,
    ep_protocol: PlonkProtocol<EpAffine>,
    eq_instances: Vec<Vec<Fp>>,
    ep_instances: Vec<Vec<Fq>>,
    eq_proof: Vec<u8>,
    ep_proof: Vec<u8>,
    eq_history: KagemushaEqAccumulatorV1,
    ep_history: KagemushaEpAccumulatorV1,
    eq_current: Option<KagemushaEqAccumulatorV1>,
    ep_current: Option<KagemushaEpAccumulatorV1>,
}

impl IncomingStateProofMaterial {
    fn padding(
        eq_params: &ParamsIPA<EqAffine>,
        ep_params: &ParamsIPA<EpAffine>,
        guard_keys: &GuardKeys,
    ) -> Self {
        let eq_protocol = compile(
            eq_params,
            &guard_keys.eq_verifying_key,
            snark_verifier::system::halo2::Config::ipa()
                .with_num_instance(vec![TERMINAL_AUTHORIZATION_PUBLIC_INSTANCE_COUNT_V1]),
        );
        let ep_protocol = compile(
            ep_params,
            &guard_keys.ep_verifying_key,
            snark_verifier::system::halo2::Config::ipa()
                .with_num_instance(vec![TERMINAL_AUTHORIZATION_PUBLIC_INSTANCE_COUNT_V1]),
        );
        let eq_history = initial_kagemusha_eq_accumulator_v1(eq_params)
            .expect("Eq inactive CommitWrapper history");
        let ep_history = initial_kagemusha_ep_accumulator_v1(ep_params)
            .expect("Ep inactive CommitWrapper history");
        let prefix = TERMINAL_AUTHORIZATION_PUBLIC_INSTANCE_COUNT_V1
            .checked_sub(accumulator_limb_count())
            .expect("CommitWrapper public history prefix");
        Self {
            eq_instances: history_instances(prefix, eq_history.as_bytes()),
            ep_instances: history_instances(prefix, ep_history.as_bytes()),
            eq_proof: dummy_ordinary_proof(&eq_protocol, EqAffine::generator()),
            ep_proof: dummy_ordinary_proof(&ep_protocol, EpAffine::generator()),
            eq_protocol,
            ep_protocol,
            eq_history,
            ep_history,
            eq_current: None,
            ep_current: None,
        }
    }
}

/// All fixed folds consumed by one recursive state proof.
struct RecursiveStateHistoryPlan {
    eq_parent_fold: KagemushaEqFoldProofV1,
    ep_parent_fold: KagemushaEpFoldProofV1,
    eq_incoming_history_fold: KagemushaEqFoldProofV1,
    ep_incoming_history_fold: KagemushaEpFoldProofV1,
    eq_incoming_merge_fold: KagemushaEqFoldProofV1,
    ep_incoming_merge_fold: KagemushaEpFoldProofV1,
    eq_guard_history_fold: KagemushaEqFoldProofV1,
    ep_guard_history_fold: KagemushaEpFoldProofV1,
    eq_guard_merge_fold: KagemushaEqFoldProofV1,
    ep_guard_merge_fold: KagemushaEpFoldProofV1,
    eq_mint_authorization_history_fold: KagemushaEqFoldProofV1,
    ep_mint_authorization_history_fold: KagemushaEpFoldProofV1,
    eq_mint_authorization_merge_fold: KagemushaEqFoldProofV1,
    ep_mint_authorization_merge_fold: KagemushaEpFoldProofV1,
    eq_mint_history_fold: KagemushaEqFoldProofV1,
    ep_mint_history_fold: KagemushaEpFoldProofV1,
    eq_mint_merge_fold: KagemushaEqFoldProofV1,
    ep_mint_merge_fold: KagemushaEpFoldProofV1,
    eq_successor_history: KagemushaEqAccumulatorV1,
    ep_successor_history: KagemushaEpAccumulatorV1,
}

fn recursive_state_history_plan(
    operation: KagemushaOperationV1,
    funded: &RealFundedPrerequisite,
    parent: &ParentProof,
    incoming: &IncomingStateProofMaterial,
    guard: &GuardProof,
) -> RecursiveStateHistoryPlan {
    let seed = test_only_recovery_seed();
    let (eq_parent_fold, mut eq_history) = if let Some(current) = &parent.eq_current {
        let fold =
            fold_kagemusha_eq_accumulators_v1(&funded.eq, current, &parent.eq_history, &seed)
                .expect("fold Eq predecessor state");
        (fold.proof().clone(), fold.successor().clone())
    } else {
        (dummy_eq_fold(), parent.eq_history.clone())
    };
    let (ep_parent_fold, mut ep_history) = if let Some(current) = &parent.ep_current {
        let fold =
            fold_kagemusha_ep_accumulators_v1(&funded.ep, current, &parent.ep_history, &seed)
                .expect("fold Ep predecessor state");
        (fold.proof().clone(), fold.successor().clone())
    } else {
        (dummy_ep_fold(), parent.ep_history.clone())
    };

    let mut eq_incoming_history_fold = dummy_eq_fold();
    let mut ep_incoming_history_fold = dummy_ep_fold();
    let mut eq_incoming_merge_fold = dummy_eq_fold();
    let mut ep_incoming_merge_fold = dummy_ep_fold();
    if operation == KagemushaOperationV1::ReceiveFold {
        let eq_current = incoming
            .eq_current
            .as_ref()
            .expect("ReceiveFold requires the genuine Eq CommitWrapper current claim");
        let ep_current = incoming
            .ep_current
            .as_ref()
            .expect("ReceiveFold requires the genuine Ep CommitWrapper current claim");
        let eq_complete =
            fold_kagemusha_eq_accumulators_v1(&funded.eq, eq_current, &incoming.eq_history, &seed)
                .expect("complete Eq incoming payment history");
        let ep_complete =
            fold_kagemusha_ep_accumulators_v1(&funded.ep, ep_current, &incoming.ep_history, &seed)
                .expect("complete Ep incoming payment history");
        let eq_merge = fold_kagemusha_eq_accumulators_v1(
            &funded.eq,
            &eq_history,
            eq_complete.successor(),
            &seed,
        )
        .expect("merge Eq incoming payment history");
        let ep_merge = fold_kagemusha_ep_accumulators_v1(
            &funded.ep,
            &ep_history,
            ep_complete.successor(),
            &seed,
        )
        .expect("merge Ep incoming payment history");
        eq_incoming_history_fold = eq_complete.proof().clone();
        ep_incoming_history_fold = ep_complete.proof().clone();
        eq_incoming_merge_fold = eq_merge.proof().clone();
        ep_incoming_merge_fold = ep_merge.proof().clone();
        eq_history = eq_merge.successor().clone();
        ep_history = ep_merge.successor().clone();
    }

    let eq_guard_complete =
        fold_kagemusha_eq_accumulators_v1(&funded.eq, &guard.eq_current, &guard.eq_history, &seed)
            .expect("complete Eq GuardBundle history");
    let ep_guard_complete =
        fold_kagemusha_ep_accumulators_v1(&funded.ep, &guard.ep_current, &guard.ep_history, &seed)
            .expect("complete Ep GuardBundle history");
    let eq_guard_merge = fold_kagemusha_eq_accumulators_v1(
        &funded.eq,
        &eq_history,
        eq_guard_complete.successor(),
        &seed,
    )
    .expect("merge Eq GuardBundle history");
    let ep_guard_merge = fold_kagemusha_ep_accumulators_v1(
        &funded.ep,
        &ep_history,
        ep_guard_complete.successor(),
        &seed,
    )
    .expect("merge Ep GuardBundle history");
    eq_history = eq_guard_merge.successor().clone();
    ep_history = ep_guard_merge.successor().clone();

    let mut eq_mint_authorization_history_fold = dummy_eq_fold();
    let mut ep_mint_authorization_history_fold = dummy_ep_fold();
    let mut eq_mint_authorization_merge_fold = dummy_eq_fold();
    let mut ep_mint_authorization_merge_fold = dummy_ep_fold();
    let mut eq_mint_history_fold = dummy_eq_fold();
    let mut ep_mint_history_fold = dummy_ep_fold();
    let mut eq_mint_merge_fold = dummy_eq_fold();
    let mut ep_mint_merge_fold = dummy_ep_fold();
    if operation == KagemushaOperationV1::MintFold {
        let eq_authorization_complete = fold_kagemusha_eq_accumulators_v1(
            &funded.eq,
            &funded.authorization.generated.eq_current_accumulator,
            &funded.authorization.eq_history,
            &seed,
        )
        .expect("complete Eq MintAuthorization history");
        let ep_authorization_complete = fold_kagemusha_ep_accumulators_v1(
            &funded.ep,
            &funded.authorization.generated.ep_current_accumulator,
            &funded.authorization.ep_history,
            &seed,
        )
        .expect("complete Ep MintAuthorization history");
        let eq_authorization_merge = fold_kagemusha_eq_accumulators_v1(
            &funded.eq,
            &eq_history,
            eq_authorization_complete.successor(),
            &seed,
        )
        .expect("merge Eq MintAuthorization history");
        let ep_authorization_merge = fold_kagemusha_ep_accumulators_v1(
            &funded.ep,
            &ep_history,
            ep_authorization_complete.successor(),
            &seed,
        )
        .expect("merge Ep MintAuthorization history");
        eq_mint_authorization_history_fold = eq_authorization_complete.proof().clone();
        ep_mint_authorization_history_fold = ep_authorization_complete.proof().clone();
        eq_mint_authorization_merge_fold = eq_authorization_merge.proof().clone();
        ep_mint_authorization_merge_fold = ep_authorization_merge.proof().clone();
        eq_history = eq_authorization_merge.successor().clone();
        ep_history = ep_authorization_merge.successor().clone();

        let eq_mint_complete = fold_kagemusha_eq_accumulators_v1(
            &funded.eq,
            &funded.funded.proof.eq_current_accumulator,
            &funded.funded.eq_history,
            &seed,
        )
        .expect("complete Eq MintAuthority history");
        let ep_mint_complete = fold_kagemusha_ep_accumulators_v1(
            &funded.ep,
            &funded.funded.proof.ep_current_accumulator,
            &funded.funded.ep_history,
            &seed,
        )
        .expect("complete Ep MintAuthority history");
        let eq_mint_merge = fold_kagemusha_eq_accumulators_v1(
            &funded.eq,
            &eq_history,
            eq_mint_complete.successor(),
            &seed,
        )
        .expect("merge Eq MintAuthority history");
        let ep_mint_merge = fold_kagemusha_ep_accumulators_v1(
            &funded.ep,
            &ep_history,
            ep_mint_complete.successor(),
            &seed,
        )
        .expect("merge Ep MintAuthority history");
        eq_mint_history_fold = eq_mint_complete.proof().clone();
        ep_mint_history_fold = ep_mint_complete.proof().clone();
        eq_mint_merge_fold = eq_mint_merge.proof().clone();
        ep_mint_merge_fold = ep_mint_merge.proof().clone();
        eq_history = eq_mint_merge.successor().clone();
        ep_history = ep_mint_merge.successor().clone();
    }

    RecursiveStateHistoryPlan {
        eq_parent_fold,
        ep_parent_fold,
        eq_incoming_history_fold,
        ep_incoming_history_fold,
        eq_incoming_merge_fold,
        ep_incoming_merge_fold,
        eq_guard_history_fold: eq_guard_complete.proof().clone(),
        ep_guard_history_fold: ep_guard_complete.proof().clone(),
        eq_guard_merge_fold: eq_guard_merge.proof().clone(),
        ep_guard_merge_fold: ep_guard_merge.proof().clone(),
        eq_mint_authorization_history_fold,
        ep_mint_authorization_history_fold,
        eq_mint_authorization_merge_fold,
        ep_mint_authorization_merge_fold,
        eq_mint_history_fold,
        ep_mint_history_fold,
        eq_mint_merge_fold,
        ep_mint_merge_fold,
        eq_successor_history: eq_history,
        ep_successor_history: ep_history,
    }
}

fn merge_recursive_state_hash_history(
    funded: &RealFundedPrerequisite,
    plan: &mut RecursiveStateHistoryPlan,
    claim: &KagemushaGeneratedMintHashClaimV1,
) -> (KagemushaEqFoldProofV1, KagemushaEpFoldProofV1) {
    let seed = test_only_recovery_seed();
    let eq_merge = fold_kagemusha_eq_accumulators_v1(
        &funded.eq,
        &plan.eq_successor_history,
        &claim.eq_complete_history,
        &seed,
    )
    .expect("merge complete Eq state SHA history");
    let ep_merge = fold_kagemusha_ep_accumulators_v1(
        &funded.ep,
        &plan.ep_successor_history,
        &claim.ep_complete_history,
        &seed,
    )
    .expect("merge complete Ep state SHA history");
    plan.eq_successor_history = eq_merge.successor().clone();
    plan.ep_successor_history = ep_merge.successor().clone();
    (eq_merge.proof().clone(), ep_merge.proof().clone())
}

#[allow(clippy::too_many_arguments)]
fn recursive_state_generation_witness<'a>(
    state: KagemushaStateRelationWitnessV1,
    mint_fold_opening: Option<
        crate::zk::kagemusha_v1_state::KagemushaMintFoldOpeningCapabilityV1<'a>,
    >,
    funded: &'a RealFundedPrerequisite,
    guard: &'a GuardProof,
    guard_keys: &'a GuardKeys,
    eq_parent_protocol: &'a PlonkProtocol<EqAffine>,
    ep_parent_protocol: &'a PlonkProtocol<EpAffine>,
    parent: &'a ParentProof,
    incoming: &'a IncomingStateProofMaterial,
    plan: &'a RecursiveStateHistoryPlan,
) -> KagemushaRecursiveStateGenerationWitnessV1<'a> {
    KagemushaRecursiveStateGenerationWitnessV1 {
        hash_claim: None,
        state,
        mint_fold_opening,
        mint_authorization: &funded.authorization.authorization,
        mint_credit: &funded.mint_credit,
        guard_relation: guard.relation.clone(),
        eq_parent_protocol,
        ep_parent_protocol,
        eq_parent_instances: &parent.eq_instances,
        ep_parent_instances: &parent.ep_instances,
        eq_parent_proof: &parent.eq_proof,
        ep_parent_proof: &parent.ep_proof,
        eq_predecessor_history: &parent.eq_history,
        ep_predecessor_history: &parent.ep_history,
        eq_parent_fold_proof: &plan.eq_parent_fold,
        ep_parent_fold_proof: &plan.ep_parent_fold,
        eq_incoming_protocol: &incoming.eq_protocol,
        ep_incoming_protocol: &incoming.ep_protocol,
        eq_incoming_credits: [KagemushaRecursiveIncomingEqGenerationWitnessV1 {
            instances: &incoming.eq_instances,
            proof: &incoming.eq_proof,
            history: &incoming.eq_history,
            history_fold_proof: &plan.eq_incoming_history_fold,
            merge_fold_proof: &plan.eq_incoming_merge_fold,
        }],
        ep_incoming_credits: [KagemushaRecursiveIncomingEpGenerationWitnessV1 {
            instances: &incoming.ep_instances,
            proof: &incoming.ep_proof,
            history: &incoming.ep_history,
            history_fold_proof: &plan.ep_incoming_history_fold,
            merge_fold_proof: &plan.ep_incoming_merge_fold,
        }],
        eq_successor_history: &plan.eq_successor_history,
        ep_successor_history: &plan.ep_successor_history,
        eq_guard_protocol: &guard_keys.eq_protocol,
        ep_guard_protocol: &guard_keys.ep_protocol,
        eq_guard_proof: &guard.eq_proof,
        ep_guard_proof: &guard.ep_proof,
        eq_guard_history: &guard.eq_history,
        ep_guard_history: &guard.ep_history,
        eq_guard_history_fold_proof: &plan.eq_guard_history_fold,
        ep_guard_history_fold_proof: &plan.ep_guard_history_fold,
        eq_guard_merge_fold_proof: &plan.eq_guard_merge_fold,
        ep_guard_merge_fold_proof: &plan.ep_guard_merge_fold,
        eq_mint_authorization_protocol: &funded.authorization_protocols.eq_protocol,
        ep_mint_authorization_protocol: &funded.authorization_protocols.ep_protocol,
        eq_mint_authorization_instances: std::slice::from_ref(
            &funded.authorization.generated.eq_public_instances,
        ),
        ep_mint_authorization_instances: std::slice::from_ref(
            &funded.authorization.generated.ep_public_instances,
        ),
        eq_mint_authorization_proof: &funded.authorization.generated.proof.eq_proof,
        ep_mint_authorization_proof: &funded.authorization.generated.proof.ep_proof,
        eq_mint_authorization_history: &funded.authorization.eq_history,
        ep_mint_authorization_history: &funded.authorization.ep_history,
        eq_mint_authorization_history_fold_proof: &plan.eq_mint_authorization_history_fold,
        ep_mint_authorization_history_fold_proof: &plan.ep_mint_authorization_history_fold,
        eq_mint_authorization_merge_fold_proof: &plan.eq_mint_authorization_merge_fold,
        ep_mint_authorization_merge_fold_proof: &plan.ep_mint_authorization_merge_fold,
        eq_mint_protocol: &funded.mint_protocols.eq_protocol,
        ep_mint_protocol: &funded.mint_protocols.ep_protocol,
        eq_mint_instances: std::slice::from_ref(&funded.funded.proof.eq_public_instances),
        ep_mint_instances: std::slice::from_ref(&funded.funded.proof.ep_public_instances),
        eq_mint_proof: &funded.funded.proof.proof.eq_proof,
        ep_mint_proof: &funded.funded.proof.proof.ep_proof,
        eq_mint_history: &funded.funded.eq_history,
        ep_mint_history: &funded.funded.ep_history,
        eq_mint_history_fold_proof: &plan.eq_mint_history_fold,
        ep_mint_history_fold_proof: &plan.ep_mint_history_fold,
        eq_mint_merge_fold_proof: &plan.eq_mint_merge_fold,
        ep_mint_merge_fold_proof: &plan.ep_mint_merge_fold,
    }
}

#[derive(Clone, Copy)]
struct RecursiveStateProtocolBindings {
    eq_state: DigestV1,
    ep_state: DigestV1,
    eq_guard: DigestV1,
    ep_guard: DigestV1,
    eq_mint_authorization: DigestV1,
    ep_mint_authorization: DigestV1,
    eq_mint: DigestV1,
    ep_mint: DigestV1,
    eq_commit_wrapper: DigestV1,
    ep_commit_wrapper: DigestV1,
}

impl RecursiveStateProtocolBindings {
    fn new(
        eq_state: DigestV1,
        ep_state: DigestV1,
        guard_keys: &GuardKeys,
        funded: &RealFundedPrerequisite,
        incoming: &IncomingStateProofMaterial,
    ) -> Self {
        Self {
            eq_state,
            ep_state,
            eq_guard: guard_keys.eq_protocol_digest,
            ep_guard: guard_keys.ep_protocol_digest,
            eq_mint_authorization: funded.authorization_protocols.eq_digest,
            ep_mint_authorization: funded.authorization_protocols.ep_digest,
            eq_mint: funded.mint_protocols.eq_digest,
            ep_mint: funded.mint_protocols.ep_digest,
            eq_commit_wrapper: native_parent_protocol_digest_v1(
                &incoming.eq_protocol,
                KagemushaPastaParityV1::Eq,
            )
            .expect("Eq CommitWrapper protocol identity"),
            ep_commit_wrapper: native_parent_protocol_digest_v1(
                &incoming.ep_protocol,
                KagemushaPastaParityV1::Ep,
            )
            .expect("Ep CommitWrapper protocol identity"),
        }
    }
}

fn bootstrap_relation_for_corridor(
    successor: KagemushaStateV1,
    guard: &GuardProof,
    transport_semantic_digest: DigestV1,
    protocols: RecursiveStateProtocolBindings,
) -> KagemushaStateRelationWitnessV1 {
    let relation = KagemushaStateRelationWitnessV1 {
        operation: KagemushaOperationV1::Bootstrap,
        predecessor: None,
        successor,
        amount: 0,
        journal_revision_before: 0,
        journal_revision_after: 0,
        transition_effect_digest: guard.relation.statement.transition_effect_digest,
        mint_finality_semantic_digest: [0; 32],
        mint_finality_proof_binding_digest: [0; 32],
        peer_credit_id: [0; 32],
        recipient_encryption_key_binding: [0; 32],
        receive_credit: None,
        receive_credit_binding_digest: [0; 32],
        lifecycle_binding_digest: guard.relation.statement.lifecycle_binding_digest,
        prepared_transition_binding_digest: [0; 32],
        transport_semantic_digest,
        guard_statement_digest: guard.relation.statement_digest(),
        eq_protocol_digest: protocols.eq_state,
        ep_protocol_digest: protocols.ep_state,
        guard_eq_protocol_digest: protocols.eq_guard,
        guard_ep_protocol_digest: protocols.ep_guard,
        mint_eq_protocol_digest: protocols.eq_mint,
        mint_ep_protocol_digest: protocols.ep_mint,
        mint_authorization_eq_protocol_digest: protocols.eq_mint_authorization,
        mint_authorization_ep_protocol_digest: protocols.ep_mint_authorization,
        commit_wrapper_eq_protocol_digest: protocols.eq_commit_wrapper,
        commit_wrapper_ep_protocol_digest: protocols.ep_commit_wrapper,
        guard_eq_credential_audit: guard.eq_credential_audit,
        guard_ep_credential_audit: guard.ep_credential_audit,
        eq_deferred_audit: [1; 32],
        ep_deferred_audit: [2; 32],
        replay_insert: None,
    };
    relation
        .validate()
        .expect("valid genuine corridor Bootstrap relation");
    relation
}

fn transition_relation_for_corridor(
    predecessor: KagemushaStateV1,
    preview: &crate::zk::kagemusha_v1_state::TransitionPreviewV1,
    guard: &GuardProof,
    protocols: RecursiveStateProtocolBindings,
    replay_insert: Option<KagemushaReplayInsertWitnessV1>,
    receive_credit: Option<KagemushaReceiveFoldCreditV1>,
) -> KagemushaStateRelationWitnessV1 {
    let statement = &preview.proof_statement;
    let relation = KagemushaStateRelationWitnessV1 {
        operation: statement.kind.into(),
        predecessor: Some(predecessor),
        successor: preview.successor.clone(),
        amount: statement.amount,
        journal_revision_before: statement.journal_revision_before,
        journal_revision_after: statement.journal_revision_after,
        transition_effect_digest: statement.effect_digest,
        mint_finality_semantic_digest: statement.mint_finality_semantic_digest,
        mint_finality_proof_binding_digest: statement.mint_finality_proof_binding_digest,
        peer_credit_id: statement.peer_credit_id,
        recipient_encryption_key_binding: statement.recipient_encryption_key_binding,
        receive_credit,
        receive_credit_binding_digest: statement.receive_credit_binding_digest,
        lifecycle_binding_digest: statement.lifecycle_binding_digest,
        prepared_transition_binding_digest: statement.prepared_transition_binding_digest,
        transport_semantic_digest: preview.transport_semantic_digest,
        guard_statement_digest: guard.relation.statement_digest(),
        eq_protocol_digest: protocols.eq_state,
        ep_protocol_digest: protocols.ep_state,
        guard_eq_protocol_digest: protocols.eq_guard,
        guard_ep_protocol_digest: protocols.ep_guard,
        mint_eq_protocol_digest: protocols.eq_mint,
        mint_ep_protocol_digest: protocols.ep_mint,
        mint_authorization_eq_protocol_digest: protocols.eq_mint_authorization,
        mint_authorization_ep_protocol_digest: protocols.ep_mint_authorization,
        commit_wrapper_eq_protocol_digest: protocols.eq_commit_wrapper,
        commit_wrapper_ep_protocol_digest: protocols.ep_commit_wrapper,
        guard_eq_credential_audit: guard.eq_credential_audit,
        guard_ep_credential_audit: guard.ep_credential_audit,
        eq_deferred_audit: [1; 32],
        ep_deferred_audit: [2; 32],
        replay_insert,
    };
    relation
        .validate()
        .expect("valid genuine corridor state transition relation");
    relation
}

fn generate_recursive_state_keys_for_corridor(
    funded: &RealFundedPrerequisite,
    state: &KagemushaStateV1,
    guard: &GuardProof,
    guard_keys: &GuardKeys,
    incoming: &IncomingStateProofMaterial,
) -> StateKeys {
    let eq_parent_protocol = compile(
        &funded.eq,
        &guard_keys.eq_verifying_key,
        snark_verifier::system::halo2::Config::ipa()
            .with_num_instance(vec![RECURSIVE_PUBLIC_INSTANCE_COUNT]),
    );
    let ep_parent_protocol = compile(
        &funded.ep,
        &guard_keys.ep_verifying_key,
        snark_verifier::system::halo2::Config::ipa()
            .with_num_instance(vec![RECURSIVE_PUBLIC_INSTANCE_COUNT]),
    );
    let eq_initial = initial_kagemusha_eq_accumulator_v1(&funded.eq)
        .expect("Eq recursive state initial history");
    let ep_initial = initial_kagemusha_ep_accumulator_v1(&funded.ep)
        .expect("Ep recursive state initial history");

    let parent = dummy_parent(
        &eq_parent_protocol,
        &ep_parent_protocol,
        eq_initial.clone(),
        ep_initial.clone(),
    );
    let mut plan = recursive_state_history_plan(
        KagemushaOperationV1::Bootstrap,
        funded,
        &parent,
        incoming,
        guard,
    );
    let eq_seed_digest =
        native_parent_protocol_digest_v1(&eq_parent_protocol, KagemushaPastaParityV1::Eq)
            .expect("Eq recursive state seed protocol identity");
    let ep_seed_digest =
        native_parent_protocol_digest_v1(&ep_parent_protocol, KagemushaPastaParityV1::Ep)
            .expect("Ep recursive state seed protocol identity");
    let protocols = RecursiveStateProtocolBindings::new(
        eq_seed_digest,
        ep_seed_digest,
        guard_keys,
        funded,
        incoming,
    );
    let relation = bootstrap_relation_for_corridor(
        state.clone(),
        guard,
        digest(b"state-keygen-transport", 0),
        protocols,
    );
    let hash_claim = prove_kagemusha_recursive_state_hash_claim_v1(
        &funded.hash_eq,
        &funded.hash_ep,
        recursive_state_generation_witness(
            relation.clone(),
            None,
            funded,
            guard,
            guard_keys,
            &eq_parent_protocol,
            &ep_parent_protocol,
            &parent,
            incoming,
            &plan,
        ),
        &test_only_recovery_seed(),
    )
    .expect("prove keygen state's complete exact SHA queue");
    let (eq_hash_merge, ep_hash_merge) =
        merge_recursive_state_hash_history(funded, &mut plan, &hash_claim);
    let mut witness = recursive_state_generation_witness(
        relation,
        None,
        funded,
        guard,
        guard_keys,
        &eq_parent_protocol,
        &ep_parent_protocol,
        &parent,
        incoming,
        &plan,
    );
    witness.hash_claim = Some(
        hash_claim
            .mint_authority_witness(
                &funded.hash_eq,
                &funded.hash_ep,
                &eq_hash_merge,
                &ep_hash_merge,
            )
            .expect("borrow exact state hash claim"),
    );
    let generated =
        generate_kagemusha_recursive_state_artifacts_v1(witness, &test_only_recovery_seed())
            .expect("generate genuine recursive-state artifacts");
    decode_state_keys(&funded.eq, &funded.ep, state, generated)
}

#[allow(clippy::too_many_arguments)]
fn prove_recursive_state_step(
    funded: &RealFundedPrerequisite,
    state_keys: &StateKeys,
    guard: &GuardProof,
    guard_keys: &GuardKeys,
    parent: &ParentProof,
    incoming: &IncomingStateProofMaterial,
    relation: KagemushaStateRelationWitnessV1,
    mint_fold_opening: Option<
        crate::zk::kagemusha_v1_state::KagemushaMintFoldOpeningCapabilityV1<'_>,
    >,
) -> KagemushaGeneratedRecursiveStateProofV1 {
    let mut plan =
        recursive_state_history_plan(relation.operation, funded, parent, incoming, guard);
    let hash_claim = prove_kagemusha_recursive_state_hash_claim_v1(
        &funded.hash_eq,
        &funded.hash_ep,
        recursive_state_generation_witness(
            relation.clone(),
            mint_fold_opening,
            funded,
            guard,
            guard_keys,
            &state_keys.eq_protocol,
            &state_keys.ep_protocol,
            parent,
            incoming,
            &plan,
        ),
        &test_only_recovery_seed(),
    )
    .expect("prove recursive state's complete exact SHA queue");
    let (eq_hash_merge, ep_hash_merge) =
        merge_recursive_state_hash_history(funded, &mut plan, &hash_claim);
    let mut witness = recursive_state_generation_witness(
        relation,
        mint_fold_opening,
        funded,
        guard,
        guard_keys,
        &state_keys.eq_protocol,
        &state_keys.ep_protocol,
        parent,
        incoming,
        &plan,
    );
    witness.hash_claim = Some(
        hash_claim
            .mint_authority_witness(
                &funded.hash_eq,
                &funded.hash_ep,
                &eq_hash_merge,
                &ep_hash_merge,
            )
            .expect("borrow exact recursive-state hash claim"),
    );
    prove_kagemusha_recursive_state_v1(
        &state_keys.eq,
        &state_keys.ep,
        witness,
        &test_only_recovery_seed(),
    )
    .expect("prove genuine recursive state transition")
}

#[test]
fn funding_certificate_preflight_has_exact_real_quorum_and_positive_membership() {
    let (_, state, funding) = funding_fixture();
    assert_eq!(
        state.balance, 0,
        "funding must not invent a positive bootstrap balance"
    );
    assert_eq!(funding.finalized.statement.amount, 1_000);
    assert_eq!(funding.finalized.epoch_roster.validators.len(), 4);
    assert_eq!(funding.finalized.seal_bundle.seals.len(), 3);
    assert!(
        certificate_signature_equations(&funding.finalized),
        "genuine paired-Pasta quorum signatures"
    );
    let mut substituted = funding.finalized.clone();
    substituted.membership.leaf.amount += 1;
    assert!(substituted.validate_shape().is_err());
    let mut substituted = funding.finalized.clone();
    substituted.seal_bundle.message.subject_digest = digest(b"substituted-finality-subject", 0);
    assert!(!certificate_signature_equations(&substituted));
}

#[test]
fn mint_authorization_sha_queue_has_exact_job_and_block_profile() {
    let material = mint_recipient_material(
        digest(b"payment-corridor-release", 0),
        digest(b"vk-set", 0),
        digest(b"payment-corridor-artifact-manifest", 0),
        1_000,
    );
    let mut enabled_profiles = [[0_u8; 32]; KAGEMUSHA_ENABLED_HARDWARE_PROFILE_SLOTS_V1];
    enabled_profiles[0] = material.hardware_profile.hardware_profile_id;
    let hardware_authorization = material
        .authorization_relation
        .hardware_authorization_digest()
        .expect("canonical hardware authorization");
    let eq = super::super::mint_authorization::mint_authorization_sha_messages_v1::<Fp>(
        &material.authorization_relation,
        &enabled_profiles,
        hardware_authorization,
    )
    .expect("Eq mint-authorization SHA queue");
    let ep = super::super::mint_authorization::mint_authorization_sha_messages_v1::<Fq>(
        &material.authorization_relation,
        &enabled_profiles,
        hardware_authorization,
    )
    .expect("Ep mint-authorization SHA queue");
    let expected_lengths = vec![663, 422, 76, 426, 198, 363, 365, 200, 74, 367];

    assert_eq!(
        eq.iter().map(Vec::len).collect::<Vec<_>>(),
        expected_lengths
    );
    assert_eq!(
        ep.iter().map(Vec::len).collect::<Vec<_>>(),
        expected_lengths
    );
    let blocks = |messages: &[Vec<u8>]| {
        messages
            .iter()
            .map(|message| (message.len() + 9).div_ceil(64))
            .sum::<usize>()
    };
    assert_eq!((eq.len(), blocks(&eq)), (10, 55));
    assert_eq!((ep.len(), blocks(&ep)), (10, 55));
}

#[test]
#[ignore = "expensive genuine recipient MintAuthorization prerequisite"]
fn real_recipient_mint_authorization_uses_hardware_credential_and_paired_proofs() {
    let _exclusive_proof = exclusive_real_proof_test_lock();
    std::thread::Builder::new()
        .name("kagemusha-real-mint-authorization".to_owned())
        .stack_size(REAL_PROOF_TEST_STACK_BYTES)
        .spawn(|| {
            let release_id = digest(b"payment-corridor-release", 0);
            let material = mint_recipient_material(
                release_id,
                digest(b"vk-set", 0),
                digest(b"payment-corridor-artifact-manifest", 0),
                1_000,
            );
            let eq = canonical_kagemusha_eq_parameters_v1();
            let ep = canonical_kagemusha_ep_parameters_v1();
            let (hash_eq, hash_ep) = generate_mint_hash_suite(&material, &eq, &ep);
            let credential_keys = CredentialKeys::generate(
                &eq,
                &ep,
                &hash_eq,
                &hash_ep,
                &material.platform_credential,
            );
            let credential = credential_keys.prove(
                &eq,
                &ep,
                &hash_eq,
                &hash_ep,
                material.platform_credential.clone(),
                material.authorization_relation.device_authority_secret,
            );
            let keys = MintAuthorizationKeys::generate(
                &material,
                &eq,
                &ep,
                &hash_eq,
                &hash_ep,
                &credential_keys,
                &credential,
            );
            let proved = keys.prove(&material, &hash_eq, &hash_ep, &credential_keys, &credential);
            proved
                .authorization
                .validate_shape()
                .expect("genuine paired recipient authorization");
            let mint_statement = material.finalized_statement(&proved.authorization);
            assert_eq!(
                mint_statement.lifecycle.credit_id, proved.authorization.statement.credit_id,
                "finalized top-up must retain the hardware-authorized credit identity",
            );
            let funding = FundingCertificate::from_statement(mint_statement);
            assert!(
                certificate_signature_equations(&funding.finalized),
                "authorization-bound top-up needs a genuine finality quorum",
            );
            assert_eq!(
                proved.generated.proof.semantic_digest,
                material
                    .authorization_relation
                    .statement
                    .canonical_digest()
                    .expect("authorization statement digest"),
            );
            decide_kagemusha_eq_accumulator_v1(&keys.eq.parameters, &proved.eq_history)
                .expect("decide genuine Eq authorization history");
            decide_kagemusha_ep_accumulator_v1(&keys.ep.parameters, &proved.ep_history)
                .expect("decide genuine Ep authorization history");
            assert_eq!(
                native_parent_protocol_digest_v1(&keys.eq_protocol, KagemushaPastaParityV1::Eq,)
                    .expect("Eq authorization identity"),
                proved.generated.proof.eq_protocol_digest,
            );
            assert_eq!(
                native_parent_protocol_digest_v1(&keys.ep_protocol, KagemushaPastaParityV1::Ep,)
                    .expect("Ep authorization identity"),
                proved.generated.proof.ep_protocol_digest,
            );
            assert_eq!(
                material.hardware_credential.credential_id,
                material
                    .authorization_relation
                    .statement
                    .context
                    .hardware_credential_id,
            );
            assert_eq!(
                material.recipient,
                material.authorization_relation.statement.context.recipient,
            );
        })
        .expect("start explicitly sized MintAuthorization stack")
        .join()
        .expect("real MintAuthorization proof thread");
}

#[test]
#[ignore = "expensive real mint-authority prerequisite; not payment-corridor qualification"]
fn real_mint_authority_bootstrap_and_positive_finalized_mint_use_reusable_keys() {
    run_guarded_real_mint_authority_proof_v1();
}

pub(super) fn run_guarded_real_mint_authority_proof_v1() {
    let _exclusive_proof = exclusive_real_proof_test_lock();
    std::thread::Builder::new()
        .name("kagemusha-real-mint".to_owned())
        .stack_size(REAL_PROOF_TEST_STACK_BYTES)
        .spawn(|| {
            let started = std::time::Instant::now();
            let prerequisite = prove_funded_prerequisite(
                digest(b"payment-corridor-release", 0),
                digest(b"vk-set", 0),
                digest(b"funding-test-manifest-identity", 0),
                1_000,
            );
            prerequisite
                .mint_credit
                .validate_shape_against_authorization(&prerequisite.authorization.authorization)
                .expect("real issued mint credit binds the proved recipient authorization");
            assert_eq!(
                prerequisite.funded.proof.authority_head,
                prerequisite.genesis_roster_id
            );
            eprintln!(
                "KAGEMUSHA real positive funding prerequisite wall time: {:?}",
                started.elapsed()
            );
        })
        .expect("start explicitly sized real-proof stack")
        .join()
        .expect("real funding proof thread");
}
