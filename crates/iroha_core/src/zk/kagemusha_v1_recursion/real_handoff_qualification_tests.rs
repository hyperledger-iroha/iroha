//! Real paired-Pasta recursive-history qualification for KAGEMUSHA V1.
//!
//! Payment qualification uses the production recursive state, GuardBundle, mint authority,
//! credential, fold, and native terminal-verification code. Only a positive-value SendSplit
//! followed by ReceiveFold counts as a payment handoff. Inactive parser padding is never
//! accepted as monetary authority or reported as a real payment proof.

#[path = "real_payment_corridor.rs"]
pub(super) mod real_payment_corridor;
#[cfg(all(test, unix))]
pub(crate) use real_payment_corridor::DiagnosticMintStageProofV1;

#[cfg(feature = "kagemusha-real-proof-harness")]
pub(super) fn run_guarded_real_mint_authority_proof_v1() {
    real_payment_corridor::run_guarded_real_mint_authority_proof_v1();
}

use std::io::Cursor;

use halo2_base::gates::circuit::BaseCircuitParams;
use halo2_proofs::{
    SerdeFormat,
    halo2curves::{
        CurveAffine,
        group::{GroupEncoding as _, prime::PrimeCurveAffine as _},
        pasta::{EpAffine, EqAffine, Fp, Fq},
    },
    plonk::{Circuit, ProvingKey, VerifyingKey, create_proof, keygen_vk},
    poly::ipa::{
        commitment::{IPACommitmentScheme, ParamsIPA},
        multiopen::ProverIPA,
    },
};
use iroha_crypto::{Hash, HashOf, kagemusha::KagemushaRecoverySeedV1};
use iroha_data_model::{
    NetworkId,
    asset::AssetDefinitionId,
    block::BlockHeader,
    kagemusha::{
        KAGEMUSHA_HARDWARE_REQUIRED_CAPABILITIES_V1, KAGEMUSHA_PAIRED_PROOF_MAX_BYTES_V1,
        KAGEMUSHA_PAYMENT_MAX_BYTES_V1, KAGEMUSHA_WIRE_VERSION_V1, KagemushaDevicePublicKeyV1,
        KagemushaDeviceSignatureV1, KagemushaEnabledProfileV1, KagemushaEvidenceFileV1,
        KagemushaHardwarePlatformClassV1, KagemushaHardwareProfileV1, KagemushaPairedProofV1,
        KagemushaPastaStateCommitmentV1, KagemushaPaymentRequestV1, KagemushaPaymentV1,
        KagemushaProviderPolicyEntryV1, kagemusha_asset_identity_digest_v1,
        kagemusha_device_key_reference_v1, kagemusha_liability_pool_id_v1,
        kagemusha_pasta_state_commitment_v1, kagemusha_provider_policy_path_v1,
        kagemusha_provider_policy_root_v1, kagemusha_provider_policy_signing_bytes_v1,
        kagemusha_suite_commitment_v1,
    },
    nexus::AxtAssetIncarnationV1,
};
use iroha_model_base::domain::DomainId;
use p256::ecdsa::{Signature, SigningKey, signature::Signer as _};
use rand_core_06::OsRng;
use sha2::{Digest as _, Sha256};
use snark_verifier::{
    loader::native::NativeLoader,
    system::halo2::{
        compile,
        transcript::halo2::{ChallengeScalar, PoseidonTranscript},
    },
    verifier::plonk::PlonkProtocol,
};

use super::generation::{
    canonical_kagemusha_ep_parameters_v1, canonical_kagemusha_eq_parameters_v1,
    keygen_pk_with_helper_resource_preflight_consuming_v1,
};
use super::{
    KAGEMUSHA_HISTORY_ACCUMULATOR_BYTES_V1, KAGEMUSHA_IPA_FOLD_PROOF_BYTES_V1,
    KAGEMUSHA_IPA_POSEIDON_FULL_ROUNDS_V1, KAGEMUSHA_IPA_POSEIDON_PARTIAL_ROUNDS_V1,
    KAGEMUSHA_IPA_POSEIDON_RATE_V1, KAGEMUSHA_IPA_POSEIDON_SECURE_MDS_V1,
    KAGEMUSHA_IPA_POSEIDON_WIDTH_V1, KAGEMUSHA_RECURSION_IPA_K_V1, KagemushaEpAccumulatorV1,
    KagemushaEpFoldProofV1, KagemushaEqAccumulatorV1, KagemushaEqFoldProofV1,
    KagemushaGeneratedRecursiveStateProofV1, KagemushaGuardBundleRelationWitnessV1,
    KagemushaLoadedEpRecursiveStateArtifactsV1, KagemushaLoadedEqRecursiveStateArtifactsV1,
    KagemushaNormalizedGuardStatementV1, KagemushaOperationV1, KagemushaPastaParityV1,
    KagemushaPlatformCredentialRelationWitnessV1, KagemushaPlatformCredentialStatementV1,
    KagemushaRecursionArtifactsV1, KagemushaRecursiveVerifierV1,
    KagemushaStateRelationPublicInputsV1, KagemushaStateRelationWitnessV1,
    composite::{
        KagemushaRecursiveStateEpCircuitV1, KagemushaRecursiveStateEqCircuitV1, ep_succinct_vk,
        eq_succinct_vk,
    },
    decide_kagemusha_ep_accumulator_v1, decide_kagemusha_eq_accumulator_v1,
    deferred_parent::{
        accumulator_limb_count, native_parent_protocol_digest_v1, ordinary_ipa_proof_profile_v1,
    },
    fold_kagemusha_ep_accumulators_v1, fold_kagemusha_eq_accumulators_v1,
    generation::{
        KagemushaGeneratedMintHashClaimV1, KagemushaLoadedEpMintHashArtifactsV1,
        KagemushaLoadedEqMintHashArtifactsV1, KagemushaRawHalo2IpaProofV1,
        augment_halo2_ipa_proof_v1, preflight_kagemusha_platform_credential_key_configuration_v1,
        prove_kagemusha_platform_credential_hash_claim_v1,
    },
    guard_bundle::{
        GUARD_RECURSIVE_PUBLIC_INSTANCE_COUNT_V1,
        KAGEMUSHA_PLATFORM_CREDENTIAL_PUBLIC_INSTANCE_COUNT_V1,
        KagemushaGuardBundleRecursiveWitnessV1, KagemushaPlatformCredentialHashClaimPairWitnessV1,
        KagemushaPlatformCredentialHashClaimParityWitnessV1, build_kagemusha_guard_bundle_pair_v1,
        build_kagemusha_platform_credential_ep_v1, build_kagemusha_platform_credential_eq_v1,
        device_authority_commitment_v1, discover_kagemusha_platform_credential_audits_v1,
    },
    initial_kagemusha_ep_accumulator_v1, initial_kagemusha_eq_accumulator_v1,
    mint_authority::KAGEMUSHA_MINT_AUTHORITY_PUBLIC_INSTANCE_COUNT_V1,
    native_backend::{verify_ep_succinct_protocol, verify_eq_succinct_protocol},
    state_relation::{PUBLIC_INSTANCE_COUNT, public_instance},
    transport_decider::{
        KagemushaTransportDeciderEpCircuitV1, KagemushaTransportDeciderEqCircuitV1,
    },
};
use crate::zk::{
    kagemusha_v1_poseidon::{
        KAGEMUSHA_STATE_DOMAIN_V1, KagemushaPoseidonFieldV1, decode as decode_pasta, digest_limbs,
        empty_replay_root, encode as encode_pasta, from_u128, hash as pasta_hash,
    },
    kagemusha_v1_state::{
        DevicePolicyBindingV1, DigestV1, HardwareEpochV1, KAGEMUSHA_STATE_VERSION_V1,
        KagemushaHandoffEvidenceV1, KagemushaHandoffSequenceVerificationV1, KagemushaLaneIdV1,
        KagemushaStateV1, ReceiveFoldCreditV1, verify_kagemusha_handoff_evidence_sequence_v1,
    },
};

const PROVIDER_AUTHORITY_DOMAIN: &[u8] = b"iroha:kagemusha:v1:provider-proof-authority";
const POLICY_LEAF_DOMAIN: &[u8] = b"iroha:kagemusha:v1:hardware-policy-leaf";
const POLICY_NODE_DOMAIN: &[u8] = b"iroha:kagemusha:v1:hardware-policy-node";
const RECURSIVE_PUBLIC_INSTANCE_COUNT: usize =
    PUBLIC_INSTANCE_COUNT + KAGEMUSHA_HISTORY_ACCUMULATOR_BYTES_V1 / 16;

/// Compare generated structured-key bytes directly, without another artifact-sized allocation.
struct GeneratedStructuredKeyBytesV1<'a> {
    expected: &'a [u8],
    offset: usize,
}

impl std::io::Write for GeneratedStructuredKeyBytesV1<'_> {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        let end = self.offset.checked_add(bytes.len()).ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "structured key byte count overflow",
            )
        })?;
        if self.expected.get(self.offset..end) != Some(bytes) {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "structured key differs from exact generated bytes",
            ));
        }
        self.offset = end;
        Ok(bytes.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

/// Check the generated structured frame without claiming threshold release authentication.
fn assert_generated_structured_proving_key_bytes_v1<C>(key: &ProvingKey<C>, expected: &[u8])
where
    C: CurveAffine + halo2_proofs::SerdeCurveAffine,
    C::Scalar: halo2_proofs::SerdePrimeField + ff::FromUniformBytes<64>,
{
    let mut canonical = GeneratedStructuredKeyBytesV1 {
        expected,
        offset: 0,
    };
    key.write_structured_v1(&mut canonical)
        .expect("canonical structured PK equals exact generated bytes");
    assert_eq!(
        canonical.offset,
        expected.len(),
        "complete structured PK frame"
    );
}

#[test]
fn generated_structured_key_comparison_checks_chunks_content_and_exact_length() {
    use std::io::Write as _;

    let expected = b"exact structured key bytes";
    let mut exact = GeneratedStructuredKeyBytesV1 {
        expected,
        offset: 0,
    };
    exact.write_all(&expected[..7]).unwrap();
    exact.write_all(&expected[7..]).unwrap();
    assert_eq!(exact.offset, expected.len());
    assert_eq!(exact.write(&[]).unwrap(), 0);
    assert_eq!(
        exact.write(b"extra").unwrap_err().kind(),
        std::io::ErrorKind::InvalidData
    );
    assert_eq!(exact.offset, expected.len());

    let mut wrong = GeneratedStructuredKeyBytesV1 {
        expected,
        offset: 0,
    };
    assert_eq!(
        wrong.write(b"wrong").unwrap_err().kind(),
        std::io::ErrorKind::InvalidData
    );
    assert_eq!(wrong.offset, 0);

    let mut short = GeneratedStructuredKeyBytesV1 {
        expected,
        offset: 0,
    };
    short.write_all(&expected[..expected.len() - 1]).unwrap();
    assert_ne!(short.offset, expected.len());

    let mut overflow = GeneratedStructuredKeyBytesV1 {
        expected,
        offset: usize::MAX,
    };
    assert_eq!(
        overflow.write(b"x").unwrap_err().kind(),
        std::io::ErrorKind::InvalidData
    );
}

/// Owned output required from the real handoff generator before evidence qualification.
///
/// This record deliberately has no constructor that fabricates proofs. The real corridor must
/// populate it from generated SendSplit, post-commit payment, and ReceiveFold artifacts.
struct GeneratedHandoffEvidenceV1 {
    sender_public_inputs: KagemushaStateRelationPublicInputsV1,
    sender_state_proof: KagemushaPairedProofV1,
    payment_request: KagemushaPaymentRequestV1,
    payment: KagemushaPaymentV1,
    receive_credit: ReceiveFoldCreditV1,
    receiver_public_inputs: KagemushaStateRelationPublicInputsV1,
    receiver_state_proof: KagemushaPairedProofV1,
}

impl GeneratedHandoffEvidenceV1 {
    fn evidence(&self) -> KagemushaHandoffEvidenceV1<'_> {
        KagemushaHandoffEvidenceV1 {
            sender_public_inputs: &self.sender_public_inputs,
            sender_state_proof: &self.sender_state_proof,
            payment_request: &self.payment_request,
            payment: &self.payment,
            receive_credit: self.receive_credit,
            receiver_public_inputs: &self.receiver_public_inputs,
            receiver_state_proof: &self.receiver_state_proof,
        }
    }
}

/// Apply the production fail-closed verifier to every generated proof in the qualification run.
///
/// This is the only acceptance path for the pending real 1,024-handoff generator. It prevents a
/// rotation-only loop, a relation-model loop, or unchecked proof-size samples from being reported
/// as payment handoffs.
fn verify_real_handoff_qualification_v1<V: KagemushaRecursiveVerifierV1>(
    verifier: &V,
    artifacts: KagemushaRecursionArtifactsV1,
    generated: &[GeneratedHandoffEvidenceV1],
) -> KagemushaHandoffSequenceVerificationV1 {
    assert!(
        generated.len() >= 1_024,
        "real payment qualification requires at least 1,024 generated handoffs"
    );
    let evidence = generated
        .iter()
        .map(GeneratedHandoffEvidenceV1::evidence)
        .collect::<Vec<_>>();
    let verified = verify_kagemusha_handoff_evidence_sequence_v1(verifier, artifacts, &evidence)
        .expect("every real generated handoff must pass terminal evidence verification");
    assert_eq!(verified.verified_handoffs, generated.len());
    assert!(
        verified.constant_sizes.sender_state_proof_bytes <= KAGEMUSHA_PAIRED_PROOF_MAX_BYTES_V1,
        "sender state proof exceeded the compact paired-proof envelope"
    );
    assert!(
        verified.constant_sizes.receiver_state_proof_bytes <= KAGEMUSHA_PAIRED_PROOF_MAX_BYTES_V1,
        "receiver state proof exceeded the compact paired-proof envelope"
    );
    assert!(
        verified.constant_sizes.payment_bytes <= KAGEMUSHA_PAYMENT_MAX_BYTES_V1,
        "committed payment exceeded the compact transport envelope"
    );
    verified
}

/// Predictable entropy for fixture replay only, never a qualified provider seed.
fn test_only_recovery_seed() -> KagemushaRecoverySeedV1 {
    KagemushaRecoverySeedV1::from_unsealed([0xA7; 32]).expect("test-only unsealed recovery seed")
}

fn digest(label: &[u8], index: u64) -> DigestV1 {
    let mut hasher = Sha256::new();
    hasher.update(b"iroha:kagemusha:v1:real-recursion-qualification");
    hasher.update([0]);
    hasher.update(label);
    hasher.update(index.to_le_bytes());
    hasher.finalize().into()
}

fn network() -> NetworkId {
    NetworkId::from_genesis_hash(HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(
        b"kagemusha-v1-real-recursion-qualification",
    )))
}

fn asset() -> AssetDefinitionId {
    AssetDefinitionId::derive_from_components(
        DomainId::try_new("qualification", "universal").expect("qualification domain"),
        "cash".parse().expect("qualification asset name"),
    )
}

fn incarnation() -> AxtAssetIncarnationV1 {
    let network = network();
    let asset = asset();
    let registration = HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(
        b"kagemusha-v1-real-recursion-registration",
    ));
    AxtAssetIncarnationV1::derive(
        &network,
        &asset,
        &registration,
        &Hash::new(b"kagemusha-v1-real-recursion-execution"),
        1,
    )
}

fn lane() -> KagemushaLaneIdV1 {
    KagemushaLaneIdV1 {
        network_id: network(),
        device_lane_id: digest(b"lane", 0),
        asset: asset(),
        scale: 6,
    }
}

fn provider_authority_commitment(secret: DigestV1) -> DigestV1 {
    let mut hasher = Sha256::new();
    hasher.update(PROVIDER_AUTHORITY_DOMAIN);
    hasher.update([0]);
    hasher.update(secret);
    hasher.finalize().into()
}

fn policy_leaf(statement: &KagemushaPlatformCredentialStatementV1) -> DigestV1 {
    let mut hasher = Sha256::new();
    hasher.update(POLICY_LEAF_DOMAIN);
    hasher.update([0]);
    hasher.update(statement.hardware_profile_id);
    hasher.update([statement.platform_class]);
    hasher.update(statement.capability_mask.to_le_bytes());
    hasher.update(statement.provider_authority_commitment);
    hasher.finalize().into()
}

fn policy_node(left: DigestV1, right: DigestV1) -> DigestV1 {
    let mut hasher = Sha256::new();
    hasher.update(POLICY_NODE_DOMAIN);
    hasher.update([0]);
    hasher.update(left);
    hasher.update(right);
    hasher.finalize().into()
}

fn deterministic_signing_key(index: u64) -> SigningKey {
    for attempt in 0_u64.. {
        let candidate = digest(b"p256-device-key", index.wrapping_add(attempt));
        if let Ok(key) = SigningKey::from_bytes((&candidate).into()) {
            return key;
        }
    }
    unreachable!("P-256 scalar search is effectively bounded")
}

/// Public setup material for cryptographic diagnostics; it grants no release or OEM authority.
struct DiagnosticProviderPolicy {
    hardware_profile: KagemushaHardwareProfileV1,
    root: DigestV1,
    siblings: [DigestV1; 16],
    provider_profile_index: u16,
    provider_secret: DigestV1,
}

impl DiagnosticProviderPolicy {
    fn new(hardware_profile: KagemushaHardwareProfileV1) -> Self {
        let provider_secret = digest(b"provider-secret", 0);
        let provider_profile_index = 0x5a31;
        // These bounded records exercise the production tree derivation only. Their placeholder
        // report/VK bindings are not authenticated evidence and never create a runtime capability.
        let profile = KagemushaEnabledProfileV1 {
            hardware_profile,
            hardware_profile_id: hardware_profile.hardware_profile_id,
            suite_id: digest(b"suite", 0),
            vk_digest: digest(b"diagnostic-policy-vk", 0),
            qualification_digest: digest(b"diagnostic-policy-qualification", 0),
            policy_epoch: hardware_profile.policy_epoch,
            qualification_report: KagemushaEvidenceFileV1 {
                sha256: hardware_profile.qualification_report_digest,
                byte_len: 1,
            },
        };
        let provider_authority_commitment = provider_authority_commitment(provider_secret);
        let issuer = deterministic_signing_key(0x7000);
        assert_eq!(
            hardware_profile.governance_credential_public_key,
            KagemushaDevicePublicKeyV1::from_sec1_bytes(
                issuer.verifying_key().to_encoded_point(false).as_bytes()
            )
            .expect("canonical diagnostic provider issuer"),
            "diagnostic policy rows must be signed by the profile's exact issuer"
        );
        let authorization = kagemusha_provider_policy_signing_bytes_v1(
            hardware_profile.hardware_profile_id,
            provider_profile_index,
            provider_authority_commitment,
        )
        .expect("canonical diagnostic provider-policy authorization");
        let issuer_signature: Signature = issuer.sign(&authorization);
        let issuer_signature = issuer_signature.normalize_s().unwrap_or(issuer_signature);
        let entry = KagemushaProviderPolicyEntryV1 {
            hardware_profile_id: hardware_profile.hardware_profile_id,
            provider_authority_commitment,
            provider_profile_index,
            issuer_signature: KagemushaDeviceSignatureV1::from_raw_bytes(
                issuer_signature.to_bytes().as_ref(),
            )
            .expect("canonical low-S diagnostic provider-policy issuer signature"),
        };
        let root = kagemusha_provider_policy_root_v1(&[profile], &[entry])
            .expect("derive diagnostic policy root from independent setup inventory");
        let siblings = kagemusha_provider_policy_path_v1(
            &[profile],
            &[entry],
            hardware_profile.hardware_profile_id,
        )
        .expect("derive canonical diagnostic provider inclusion path");
        Self {
            hardware_profile,
            root,
            siblings,
            provider_profile_index,
            provider_secret,
        }
    }
}

fn diagnostic_hardware_profile() -> KagemushaHardwareProfileV1 {
    let issuer = deterministic_signing_key(0x7000);
    let public_key = KagemushaDevicePublicKeyV1::from_sec1_bytes(
        issuer.verifying_key().to_encoded_point(false).as_bytes(),
    )
    .expect("canonical diagnostic issuer key");
    KagemushaHardwareProfileV1 {
        version: KAGEMUSHA_WIRE_VERSION_V1,
        protocol_version: KAGEMUSHA_WIRE_VERSION_V1,
        hardware_profile_id: [0; 32],
        provider_id: digest(b"mint-provider", 0),
        platform_class: KagemushaHardwarePlatformClassV1::DedicatedSecureElement,
        product_class_digest: digest(b"mint-product", 0),
        firmware_policy_digest: digest(b"mint-firmware", 0),
        enrollment_attestation_verifier_digest: digest(b"mint-enrollment-verifier", 0),
        attestation_trust_roots_digest: digest(b"mint-attestation-root", 0),
        allowed_suite_commitment: kagemusha_suite_commitment_v1(digest(b"suite", 0)),
        policy_epoch: 1,
        governance_credential_public_key: public_key,
        capability_mask: KAGEMUSHA_HARDWARE_REQUIRED_CAPABILITIES_V1,
        qualification_report_digest: digest(b"mint-qualification-report", 0),
        valid_from_ms: 1,
        expires_at_ms: 1_000_000,
    }
    .seal_hardware_profile_id()
    .expect("canonical diagnostic hardware profile")
}

pub(super) fn credential_witness(
    index: u64,
    release_id: DigestV1,
    empty_effect: DigestV1,
) -> (KagemushaPlatformCredentialRelationWitnessV1, DigestV1) {
    let policy = DiagnosticProviderPolicy::new(diagnostic_hardware_profile());
    credential_witness_with_policy(index, release_id, empty_effect, &policy)
}

fn credential_witness_with_policy(
    index: u64,
    release_id: DigestV1,
    empty_effect: DigestV1,
    policy: &DiagnosticProviderPolicy,
) -> (KagemushaPlatformCredentialRelationWitnessV1, DigestV1) {
    let provider_secret = policy.provider_secret;
    let device_secret = digest(b"device-secret", index);
    let signing_key = deterministic_signing_key(index);
    let device_public_key = KagemushaDevicePublicKeyV1::from_sec1_bytes(
        signing_key
            .verifying_key()
            .to_encoded_point(false)
            .as_bytes(),
    )
    .expect("canonical qualification P-256 key");
    let policy_siblings = policy.siblings;
    let lane = lane();
    let asset_incarnation = incarnation();
    let statement = KagemushaPlatformCredentialStatementV1 {
        version: KAGEMUSHA_WIRE_VERSION_V1,
        protocol_version: KAGEMUSHA_WIRE_VERSION_V1,
        suite_id: digest(b"suite", 0),
        release_id,
        network_id: *lane.network_id.as_bytes(),
        asset_id: kagemusha_asset_identity_digest_v1(&lane.asset)
            .expect("canonical qualification asset"),
        asset_incarnation,
        asset_scale: lane.scale,
        liability_pool_id: kagemusha_liability_pool_id_v1(
            &lane.network_id,
            &lane.asset,
            asset_incarnation,
        )
        .expect("qualification liability pool"),
        lane_id: lane.device_lane_id,
        hardware_epoch_generation: u128::from(index) + 1,
        hardware_epoch_id: digest(b"hardware-epoch", index),
        key_reference: kagemusha_device_key_reference_v1(&device_public_key),
        device_public_key,
        hardware_policy_id: policy.root,
        device_authority_commitment: device_authority_commitment_v1(device_secret),
        hardware_profile_id: policy.hardware_profile.hardware_profile_id,
        policy_epoch: policy.hardware_profile.policy_epoch,
        platform_class: policy.hardware_profile.platform_class as u8,
        capability_mask: KAGEMUSHA_HARDWARE_REQUIRED_CAPABILITIES_V1,
        provider_authority_commitment: provider_authority_commitment(provider_secret),
        platform_attestation_digest: digest(b"platform-attestation", index),
        credential_issuance_digest: digest(b"credential-issuance", index),
        canonical_empty_effect_digest: empty_effect,
        provider_profile_index: policy.provider_profile_index,
    };
    let mut root = policy_leaf(&statement);
    for (depth, sibling) in policy_siblings.iter().copied().enumerate() {
        root = if (statement.provider_profile_index >> depth) & 1 == 0 {
            policy_node(root, sibling)
        } else {
            policy_node(sibling, root)
        };
    }
    assert_eq!(
        root, policy.root,
        "credential must use the independent setup root"
    );
    let witness = KagemushaPlatformCredentialRelationWitnessV1 {
        statement,
        provider_authority_secret: provider_secret,
        policy_siblings,
    };
    witness.validate().expect("valid qualification credential");
    (witness, device_secret)
}

fn empty_replay_root_pair() -> KagemushaPastaStateCommitmentV1 {
    KagemushaPastaStateCommitmentV1 {
        eq: encode_pasta(empty_replay_root::<Fp>()),
        ep: encode_pasta(empty_replay_root::<Fq>()),
    }
}

fn state_component<F: KagemushaPoseidonFieldV1>(
    state: &KagemushaStateV1,
    replay_root: DigestV1,
) -> DigestV1 {
    let replay_root = decode_pasta::<F>(replay_root).expect("canonical replay root");
    let mut inputs = Vec::with_capacity(34);
    inputs.push(F::from(u64::from(state.version)));
    inputs.push(F::from(u64::from(state.protocol_version)));
    inputs.extend(digest_limbs::<F>(state.suite_id));
    inputs.extend(digest_limbs::<F>(state.vk_digest));
    inputs.extend(digest_limbs::<F>(state.release_id));
    inputs.extend(digest_limbs::<F>(*state.asset_incarnation.as_bytes()));
    inputs.extend(digest_limbs::<F>(state.liability_pool_id));
    inputs.extend(digest_limbs::<F>(state.hardware_profile_id));
    inputs.push(F::from(state.policy_epoch));
    inputs.extend(digest_limbs::<F>(*state.lane.network_id.as_bytes()));
    inputs.extend(digest_limbs::<F>(
        kagemusha_asset_identity_digest_v1(&state.lane.asset).expect("canonical state asset"),
    ));
    inputs.push(F::from(u64::from(state.lane.scale)));
    inputs.extend(digest_limbs::<F>(state.lane.device_lane_id));
    inputs.push(from_u128(state.balance));
    inputs.push(from_u128(state.logical_sequence));
    inputs.push(from_u128(state.hardware_epoch.generation));
    inputs.extend(digest_limbs::<F>(state.hardware_epoch.epoch_id));
    inputs.extend(digest_limbs::<F>(
        state.device_policy_binding.device_key_reference,
    ));
    inputs.extend(digest_limbs::<F>(
        state.device_policy_binding.hardware_policy_id,
    ));
    inputs.extend(digest_limbs::<F>(state.state_nonce_commitment));
    inputs.push(replay_root);
    encode_pasta(pasta_hash(KAGEMUSHA_STATE_DOMAIN_V1, &inputs))
}

fn aggregate_state(
    release_id: DigestV1,
    credential: &KagemushaPlatformCredentialStatementV1,
    nonce: DigestV1,
) -> KagemushaStateV1 {
    aggregate_state_with_balance(release_id, credential, nonce, 0, 0)
}

pub(super) fn aggregate_state_with_balance(
    release_id: DigestV1,
    credential: &KagemushaPlatformCredentialStatementV1,
    nonce: DigestV1,
    balance: u128,
    logical_sequence: u128,
) -> KagemushaStateV1 {
    let lane = lane();
    let asset_incarnation = credential.asset_incarnation;
    let liability_pool_id =
        kagemusha_liability_pool_id_v1(&lane.network_id, &lane.asset, asset_incarnation)
            .expect("qualification liability pool");
    let consumed_credit_root = empty_replay_root_pair();
    let mut state = KagemushaStateV1 {
        version: KAGEMUSHA_STATE_VERSION_V1,
        protocol_version: credential.protocol_version,
        suite_id: credential.suite_id,
        vk_digest: digest(b"vk-set", 0),
        release_id,
        asset_incarnation,
        liability_pool_id,
        hardware_profile_id: credential.hardware_profile_id,
        policy_epoch: credential.policy_epoch,
        lane,
        balance,
        logical_sequence,
        hardware_epoch: HardwareEpochV1 {
            generation: credential.hardware_epoch_generation,
            epoch_id: credential.hardware_epoch_id,
        },
        device_policy_binding: DevicePolicyBindingV1 {
            device_key_reference: credential.key_reference,
            hardware_policy_id: credential.hardware_policy_id,
        },
        state_nonce_commitment: nonce,
        consumed_credit_root,
        state_commitment_components: KagemushaPastaStateCommitmentV1::ZERO,
        state_commitment: [0; 32],
    };
    state.state_commitment_components = KagemushaPastaStateCommitmentV1 {
        eq: state_component::<Fp>(&state, consumed_credit_root.eq),
        ep: state_component::<Fq>(&state, consumed_credit_root.ep),
    };
    state.state_commitment = kagemusha_pasta_state_commitment_v1(state.state_commitment_components);
    state.validate().expect("valid qualification state");
    state
}

type EqTranscript<S> = PoseidonTranscript<
    EqAffine,
    NativeLoader,
    S,
    KAGEMUSHA_IPA_POSEIDON_WIDTH_V1,
    KAGEMUSHA_IPA_POSEIDON_RATE_V1,
    KAGEMUSHA_IPA_POSEIDON_FULL_ROUNDS_V1,
    KAGEMUSHA_IPA_POSEIDON_PARTIAL_ROUNDS_V1,
>;
type EpTranscript<S> = PoseidonTranscript<
    EpAffine,
    NativeLoader,
    S,
    KAGEMUSHA_IPA_POSEIDON_WIDTH_V1,
    KAGEMUSHA_IPA_POSEIDON_RATE_V1,
    KAGEMUSHA_IPA_POSEIDON_FULL_ROUNDS_V1,
    KAGEMUSHA_IPA_POSEIDON_PARTIAL_ROUNDS_V1,
>;

pub(super) fn create_eq_proof<C: Circuit<Fp>>(
    params: &ParamsIPA<EqAffine>,
    proving_key: &ProvingKey<EqAffine>,
    circuit: C,
    instances: &[Fp],
) -> Vec<u8> {
    try_create_eq_proof(params, proving_key, circuit, instances)
        .expect("create and augment genuine EQ Halo2 proof")
}

fn try_create_eq_proof<C: Circuit<Fp>>(
    params: &ParamsIPA<EqAffine>,
    proving_key: &ProvingKey<EqAffine>,
    circuit: C,
    instances: &[Fp],
) -> Result<Vec<u8>, String> {
    let columns: [&[Fp]; 1] = [instances];
    let proofs_instances: [&[&[Fp]]; 1] = [&columns];
    let mut transcript =
        EqTranscript::new::<KAGEMUSHA_IPA_POSEIDON_SECURE_MDS_V1>(Vec::<u8>::new());
    create_proof::<
        IPACommitmentScheme<EqAffine>,
        ProverIPA<'_, EqAffine>,
        ChallengeScalar<EqAffine>,
        _,
        _,
        _,
    >(
        params,
        proving_key,
        &[circuit],
        &proofs_instances,
        OsRng,
        &mut transcript,
    )
    .map_err(|error| format!("Halo2 proof creation rejected: {error}"))?;
    let raw = transcript.finalize();
    let proof = augment_halo2_ipa_proof_v1(
        params,
        proving_key.get_vk(),
        KagemushaRawHalo2IpaProofV1::new(raw),
        instances,
    )
    .map_err(|error| format!("Halo2 proof augmentation rejected: {error}"))?;
    assert!(!proof.is_empty());
    let protocol = compile(
        params,
        proving_key.get_vk(),
        snark_verifier::system::halo2::Config::ipa().with_num_instance(vec![instances.len()]),
    );
    let expected = ordinary_ipa_proof_profile_v1(&protocol)
        .expect("valid Eq proof profile")
        .byte_len;
    assert_eq!(proof.len(), expected, "non-canonical real Eq proof length");
    Ok(proof)
}

pub(super) fn create_ep_proof<C: Circuit<Fq>>(
    params: &ParamsIPA<EpAffine>,
    proving_key: &ProvingKey<EpAffine>,
    circuit: C,
    instances: &[Fq],
) -> Vec<u8> {
    try_create_ep_proof(params, proving_key, circuit, instances)
        .expect("create and augment genuine EP Halo2 proof")
}

fn try_create_ep_proof<C: Circuit<Fq>>(
    params: &ParamsIPA<EpAffine>,
    proving_key: &ProvingKey<EpAffine>,
    circuit: C,
    instances: &[Fq],
) -> Result<Vec<u8>, String> {
    let columns: [&[Fq]; 1] = [instances];
    let proofs_instances: [&[&[Fq]]; 1] = [&columns];
    let mut transcript =
        EpTranscript::new::<KAGEMUSHA_IPA_POSEIDON_SECURE_MDS_V1>(Vec::<u8>::new());
    create_proof::<
        IPACommitmentScheme<EpAffine>,
        ProverIPA<'_, EpAffine>,
        ChallengeScalar<EpAffine>,
        _,
        _,
        _,
    >(
        params,
        proving_key,
        &[circuit],
        &proofs_instances,
        OsRng,
        &mut transcript,
    )
    .map_err(|error| format!("Halo2 proof creation rejected: {error}"))?;
    let raw = transcript.finalize();
    let proof = augment_halo2_ipa_proof_v1(
        params,
        proving_key.get_vk(),
        KagemushaRawHalo2IpaProofV1::new(raw),
        instances,
    )
    .map_err(|error| format!("Halo2 proof augmentation rejected: {error}"))?;
    assert!(!proof.is_empty());
    let protocol = compile(
        params,
        proving_key.get_vk(),
        snark_verifier::system::halo2::Config::ipa().with_num_instance(vec![instances.len()]),
    );
    let expected = ordinary_ipa_proof_profile_v1(&protocol)
        .expect("valid Ep proof profile")
        .byte_len;
    assert_eq!(proof.len(), expected, "non-canonical real Ep proof length");
    Ok(proof)
}

fn dummy_ordinary_proof<C: CurveAffine>(protocol: &PlonkProtocol<C>, point: C) -> Vec<u8> {
    let profile = ordinary_ipa_proof_profile_v1(protocol).expect("valid dummy proof profile");
    let point = point.to_bytes();
    let scalar = [0_u8; 32];
    let mut proof = Vec::with_capacity(profile.byte_len);
    for _ in 0..profile.witness_commitments {
        proof.extend_from_slice(point.as_ref());
    }
    for _ in 0..profile.quotient_commitments {
        proof.extend_from_slice(point.as_ref());
    }
    for _ in 0..profile.evaluations {
        proof.extend_from_slice(&scalar);
    }
    proof.extend_from_slice(point.as_ref());
    for _ in 0..profile.bgh19_rotation_sets {
        proof.extend_from_slice(&scalar);
    }
    proof.extend_from_slice(point.as_ref());
    for _ in 0..(2 * KAGEMUSHA_RECURSION_IPA_K_V1 as usize) {
        proof.extend_from_slice(point.as_ref());
    }
    proof.extend_from_slice(&scalar);
    proof.extend_from_slice(&scalar);
    proof.extend_from_slice(point.as_ref());
    assert_eq!(proof.len(), profile.byte_len);
    proof
}

fn dummy_fold_bytes<C: CurveAffine>(point: C) -> Vec<u8> {
    let point = point.to_bytes();
    let scalar = [0_u8; 32];
    let mut proof = Vec::with_capacity(KAGEMUSHA_IPA_FOLD_PROOF_BYTES_V1);
    proof.extend_from_slice(&scalar);
    proof.extend_from_slice(&scalar);
    proof.extend_from_slice(point.as_ref());
    proof.extend_from_slice(&scalar);
    proof.extend_from_slice(point.as_ref());
    proof.extend_from_slice(&scalar);
    for _ in 0..(2 * KAGEMUSHA_RECURSION_IPA_K_V1 as usize) {
        proof.extend_from_slice(point.as_ref());
    }
    proof.extend_from_slice(point.as_ref());
    proof.extend_from_slice(&scalar);
    assert_eq!(proof.len(), KAGEMUSHA_IPA_FOLD_PROOF_BYTES_V1);
    proof
}

fn dummy_eq_fold() -> KagemushaEqFoldProofV1 {
    KagemushaEqFoldProofV1::try_from_bytes(&dummy_fold_bytes(EqAffine::generator()))
        .expect("fixed-shape Eq fold parser witness")
}

fn dummy_ep_fold() -> KagemushaEpFoldProofV1 {
    KagemushaEpFoldProofV1::try_from_bytes(&dummy_fold_bytes(EpAffine::generator()))
        .expect("fixed-shape Ep fold parser witness")
}

fn history_instances<F: KagemushaPoseidonFieldV1>(
    prefix_len: usize,
    history: &[u8; KAGEMUSHA_HISTORY_ACCUMULATOR_BYTES_V1],
) -> Vec<Vec<F>> {
    let mut column = vec![F::ZERO; prefix_len + accumulator_limb_count()];
    for (destination, chunk) in column[prefix_len..]
        .iter_mut()
        .zip(history.chunks_exact(16))
    {
        *destination = from_u128(u128::from_le_bytes(
            chunk.try_into().expect("history limb width"),
        ));
    }
    vec![column]
}

struct CredentialKeys {
    provider_policy_root: DigestV1,
    eq_proving_key: ProvingKey<EqAffine>,
    ep_proving_key: ProvingKey<EpAffine>,
    eq_protocol: PlonkProtocol<EqAffine>,
    ep_protocol: PlonkProtocol<EpAffine>,
    eq_protocol_digest: DigestV1,
    ep_protocol_digest: DigestV1,
    eq_circuit_params: BaseCircuitParams,
    ep_circuit_params: BaseCircuitParams,
    hash_claim: KagemushaGeneratedMintHashClaimV1,
    hash_claim_relation: KagemushaPlatformCredentialRelationWitnessV1,
}

struct CredentialProof {
    relation: KagemushaPlatformCredentialRelationWitnessV1,
    eq_instances: Vec<Vec<Fp>>,
    ep_instances: Vec<Vec<Fq>>,
    eq_proof: Vec<u8>,
    ep_proof: Vec<u8>,
    eq_claim_history: KagemushaEqAccumulatorV1,
    ep_claim_history: KagemushaEpAccumulatorV1,
    eq_current: KagemushaEqAccumulatorV1,
    ep_current: KagemushaEpAccumulatorV1,
}

fn with_claim_backed_credential_witness<R>(
    eq_hash: &KagemushaLoadedEqMintHashArtifactsV1,
    ep_hash: &KagemushaLoadedEpMintHashArtifactsV1,
    relation: &KagemushaPlatformCredentialRelationWitnessV1,
    claim: &KagemushaGeneratedMintHashClaimV1,
    build: impl FnOnce(&KagemushaPlatformCredentialHashClaimPairWitnessV1<'_>) -> R,
) -> R {
    let eq_claim_instances = claim.eq_inner_instances.clone();
    let ep_claim_instances = claim.ep_inner_instances.clone();
    let eq_claim_history = claim
        .eq_history
        .to_native()
        .expect("decode Eq PlatformCredential claim history");
    let ep_claim_history = claim
        .ep_history
        .to_native()
        .expect("decode Ep PlatformCredential claim history");
    let witness = KagemushaPlatformCredentialHashClaimPairWitnessV1 {
        relation: relation.clone(),
        eq_claim_protocol_digest: eq_hash.claim_protocol_digest,
        ep_claim_protocol_digest: ep_hash.claim_protocol_digest,
        eq_shard_protocol_digest: eq_hash.shard_protocol_digest,
        ep_shard_protocol_digest: ep_hash.shard_protocol_digest,
        eq: KagemushaPlatformCredentialHashClaimParityWitnessV1 {
            claim_protocol: &eq_hash.claim_protocol,
            claim_instances: &eq_claim_instances,
            claim_proof: &claim.eq_proof,
            claim_history: &eq_claim_history,
            claim_history_fold_proof: claim.eq_history_fold_proof.as_bytes(),
            successor_history: claim.eq_complete_history.as_bytes(),
        },
        ep: KagemushaPlatformCredentialHashClaimParityWitnessV1 {
            claim_protocol: &ep_hash.claim_protocol,
            claim_instances: &ep_claim_instances,
            claim_proof: &claim.ep_proof,
            claim_history: &ep_claim_history,
            claim_history_fold_proof: claim.ep_history_fold_proof.as_bytes(),
            successor_history: claim.ep_complete_history.as_bytes(),
        },
    };
    build(&witness)
}

impl CredentialKeys {
    fn generate(
        eq_params: &ParamsIPA<EqAffine>,
        ep_params: &ParamsIPA<EpAffine>,
        eq_hash: &KagemushaLoadedEqMintHashArtifactsV1,
        ep_hash: &KagemushaLoadedEpMintHashArtifactsV1,
        witness: &KagemushaPlatformCredentialRelationWitnessV1,
        provider_policy_root: DigestV1,
    ) -> Self {
        assert_ne!(provider_policy_root, [0; 32]);
        assert_eq!(witness.statement.hardware_policy_id, provider_policy_root);
        preflight_kagemusha_platform_credential_key_configuration_v1(provider_policy_root).expect(
            "PlatformCredential fixed auxiliary geometry must fit immutable helper-key limits",
        );
        let hash_claim = prove_kagemusha_platform_credential_hash_claim_v1(
            eq_hash,
            ep_hash,
            witness,
            &test_only_recovery_seed(),
        )
        .expect("prove reusable PlatformCredential typed SHA claim");
        let (
            eq_proving_key,
            ep_proving_key,
            eq_protocol,
            ep_protocol,
            eq_protocol_digest,
            ep_protocol_digest,
            eq_circuit_params,
            ep_circuit_params,
        ) = with_claim_backed_credential_witness(
            eq_hash,
            ep_hash,
            witness,
            &hash_claim,
            |witness| {
                let discovery = discover_kagemusha_platform_credential_audits_v1(
                    eq_params,
                    ep_params,
                    witness,
                    provider_policy_root,
                )
                .expect("discover PlatformCredential reciprocal audits");
                let eq_circuit = build_kagemusha_platform_credential_eq_v1(
                    eq_params,
                    witness,
                    &discovery,
                    provider_policy_root,
                )
                .expect("build exact Eq PlatformCredential circuit");
                let eq_circuit_params = eq_circuit.params().base;
                let eq_proving_key = keygen_pk_with_helper_resource_preflight_consuming_v1(
                    eq_params,
                    eq_circuit,
                    KagemushaPastaParityV1::Eq,
                    "PlatformCredential",
                    "PlatformCredential proving key",
                )
                .expect("Eq credential PK");
                halo2_proofs::release_allocator_slack();

                let ep_circuit = build_kagemusha_platform_credential_ep_v1(
                    ep_params,
                    witness,
                    &discovery,
                    provider_policy_root,
                )
                .expect("build exact Ep PlatformCredential circuit");
                let ep_circuit_params = ep_circuit.params().base;
                let ep_proving_key = keygen_pk_with_helper_resource_preflight_consuming_v1(
                    ep_params,
                    ep_circuit,
                    KagemushaPastaParityV1::Ep,
                    "PlatformCredential",
                    "PlatformCredential proving key",
                )
                .expect("Ep credential PK");
                halo2_proofs::release_allocator_slack();

                let eq_protocol = compile(
                    eq_params,
                    eq_proving_key.get_vk(),
                    snark_verifier::system::halo2::Config::ipa().with_num_instance(vec![
                        KAGEMUSHA_PLATFORM_CREDENTIAL_PUBLIC_INSTANCE_COUNT_V1,
                    ]),
                );
                let ep_protocol = compile(
                    ep_params,
                    ep_proving_key.get_vk(),
                    snark_verifier::system::halo2::Config::ipa().with_num_instance(vec![
                        KAGEMUSHA_PLATFORM_CREDENTIAL_PUBLIC_INSTANCE_COUNT_V1,
                    ]),
                );
                let eq_protocol_digest =
                    native_parent_protocol_digest_v1(&eq_protocol, KagemushaPastaParityV1::Eq)
                        .expect("Eq PlatformCredential protocol digest");
                let ep_protocol_digest =
                    native_parent_protocol_digest_v1(&ep_protocol, KagemushaPastaParityV1::Ep)
                        .expect("Ep PlatformCredential protocol digest");
                (
                    eq_proving_key,
                    ep_proving_key,
                    eq_protocol,
                    ep_protocol,
                    eq_protocol_digest,
                    ep_protocol_digest,
                    eq_circuit_params,
                    ep_circuit_params,
                )
            },
        );
        Self {
            provider_policy_root,
            eq_proving_key,
            ep_proving_key,
            eq_protocol,
            ep_protocol,
            eq_protocol_digest,
            ep_protocol_digest,
            eq_circuit_params,
            ep_circuit_params,
            hash_claim,
            hash_claim_relation: witness.clone(),
        }
    }

    fn prove(
        &self,
        eq_params: &ParamsIPA<EqAffine>,
        ep_params: &ParamsIPA<EpAffine>,
        eq_hash: &KagemushaLoadedEqMintHashArtifactsV1,
        ep_hash: &KagemushaLoadedEpMintHashArtifactsV1,
        relation: KagemushaPlatformCredentialRelationWitnessV1,
    ) -> CredentialProof {
        let provider_policy_root = self.provider_policy_root;
        assert_eq!(
            relation.statement.hardware_policy_id, provider_policy_root,
            "reused credential keys retain their independently selected setup root"
        );
        // The keys are reusable; a typed SHA claim authenticates only its exact relation.
        // A new device/epoch therefore needs its own genuine claim under the same hash keys.
        let generated_claim = (relation != self.hash_claim_relation).then(|| {
            prove_kagemusha_platform_credential_hash_claim_v1(
                eq_hash,
                ep_hash,
                &relation,
                &test_only_recovery_seed(),
            )
            .expect("prove the exact successor PlatformCredential typed SHA claim")
        });
        let hash_claim = generated_claim.as_ref().unwrap_or(&self.hash_claim);
        let (
            eq_instances,
            ep_instances,
            eq_proof,
            ep_proof,
            eq_claim_history,
            ep_claim_history,
            eq_current,
            ep_current,
        ) = with_claim_backed_credential_witness(
            eq_hash,
            ep_hash,
            &relation,
            hash_claim,
            |witness| {
                let discovery = discover_kagemusha_platform_credential_audits_v1(
                    eq_params,
                    ep_params,
                    witness,
                    provider_policy_root,
                )
                .expect("discover PlatformCredential proof audits");
                let eq_circuit = build_kagemusha_platform_credential_eq_v1(
                    eq_params,
                    witness,
                    &discovery,
                    provider_policy_root,
                )
                .expect("build exact Eq PlatformCredential proof circuit");
                assert_base_circuit_params_eq(&eq_circuit.params().base, &self.eq_circuit_params);
                let eq_column = eq_circuit
                    .public_instances()
                    .expect("Eq PlatformCredential instances");
                assert_eq!(
                    eq_column.len(),
                    KAGEMUSHA_PLATFORM_CREDENTIAL_PUBLIC_INSTANCE_COUNT_V1
                );
                let eq_proof =
                    create_eq_proof(eq_params, &self.eq_proving_key, eq_circuit, &eq_column);
                let eq_instances = vec![eq_column];
                let eq_current = KagemushaEqAccumulatorV1::from_native(
                    &verify_eq_succinct_protocol(
                        eq_params,
                        &self.eq_protocol,
                        &eq_proof,
                        &eq_instances[0],
                    )
                    .expect("verify real Eq PlatformCredential proof"),
                )
                .expect("encode Eq PlatformCredential accumulator");
                halo2_proofs::release_allocator_slack();

                let ep_circuit = build_kagemusha_platform_credential_ep_v1(
                    ep_params,
                    witness,
                    &discovery,
                    provider_policy_root,
                )
                .expect("build exact Ep PlatformCredential proof circuit");
                assert_base_circuit_params_eq(&ep_circuit.params().base, &self.ep_circuit_params);
                let ep_column = ep_circuit
                    .public_instances()
                    .expect("Ep PlatformCredential instances");
                assert_eq!(
                    ep_column.len(),
                    KAGEMUSHA_PLATFORM_CREDENTIAL_PUBLIC_INSTANCE_COUNT_V1
                );
                let ep_proof =
                    create_ep_proof(ep_params, &self.ep_proving_key, ep_circuit, &ep_column);
                let ep_instances = vec![ep_column];
                let ep_current = KagemushaEpAccumulatorV1::from_native(
                    &verify_ep_succinct_protocol(
                        ep_params,
                        &self.ep_protocol,
                        &ep_proof,
                        &ep_instances[0],
                    )
                    .expect("verify real Ep PlatformCredential proof"),
                )
                .expect("encode Ep PlatformCredential accumulator");
                halo2_proofs::release_allocator_slack();

                (
                    eq_instances,
                    ep_instances,
                    eq_proof,
                    ep_proof,
                    hash_claim.eq_complete_history.clone(),
                    hash_claim.ep_complete_history.clone(),
                    eq_current,
                    ep_current,
                )
            },
        );
        CredentialProof {
            relation,
            eq_instances,
            ep_instances,
            eq_proof,
            ep_proof,
            eq_claim_history,
            ep_claim_history,
            eq_current,
            ep_current,
        }
    }

    /// Attack the retained setup keys with a complete, self-consistent alternative policy.
    /// This is cryptographic diagnostic evidence, never provider or hardware qualification.
    fn assert_substituted_policy_rejected(
        &self,
        eq_params: &ParamsIPA<EqAffine>,
        ep_params: &ParamsIPA<EpAffine>,
        eq_hash: &KagemushaLoadedEqMintHashArtifactsV1,
        ep_hash: &KagemushaLoadedEpMintHashArtifactsV1,
        credential: &CredentialProof,
    ) {
        assert_eq!(
            credential.relation.statement.hardware_policy_id,
            self.provider_policy_root
        );
        let mut substituted = credential.relation.clone();
        substituted.provider_authority_secret = digest(b"unapproved-provider-secret", 0);
        substituted.statement.provider_authority_commitment =
            provider_authority_commitment(substituted.provider_authority_secret);
        let mut substituted_root = policy_leaf(&substituted.statement);
        for (depth, sibling) in substituted.policy_siblings.iter().copied().enumerate() {
            substituted_root = if (substituted.statement.provider_profile_index >> depth) & 1 == 0 {
                policy_node(substituted_root, sibling)
            } else {
                policy_node(sibling, substituted_root)
            };
        }
        substituted.statement.hardware_policy_id = substituted_root;
        substituted
            .validate()
            .expect("self-consistent adversarial policy witness");
        assert_ne!(substituted_root, self.provider_policy_root);

        // Replaying a real proof with only the public statement's policy root replaced fails.
        let mut rebound = credential.relation.statement;
        rebound.hardware_policy_id = substituted_root;
        let mut eq_rebound = credential.eq_instances[0].clone();
        eq_rebound[..2].copy_from_slice(&digest_limbs::<Fp>(rebound.canonical_digest()));
        let mut ep_rebound = credential.ep_instances[0].clone();
        ep_rebound[..2].copy_from_slice(&digest_limbs::<Fq>(rebound.canonical_digest()));
        let accepts_eq = |proof: &[u8], instances: &[Fp]| {
            verify_eq_succinct_protocol(eq_params, &self.eq_protocol, proof, instances)
                .ok()
                .and_then(|claim| KagemushaEqAccumulatorV1::from_native(&claim).ok())
                .is_some_and(|claim| decide_kagemusha_eq_accumulator_v1(eq_params, &claim).is_ok())
        };
        let accepts_ep = |proof: &[u8], instances: &[Fq]| {
            verify_ep_succinct_protocol(ep_params, &self.ep_protocol, proof, instances)
                .ok()
                .and_then(|claim| KagemushaEpAccumulatorV1::from_native(&claim).ok())
                .is_some_and(|claim| decide_kagemusha_ep_accumulator_v1(ep_params, &claim).is_ok())
        };
        assert!(accepts_eq(
            &credential.eq_proof,
            &credential.eq_instances[0]
        ));
        assert!(accepts_ep(
            &credential.ep_proof,
            &credential.ep_instances[0]
        ));
        assert!(!accepts_eq(&credential.eq_proof, &eq_rebound));
        assert!(!accepts_ep(&credential.ep_proof, &ep_rebound));

        // Give the attacker a genuine SHA claim for its own root. Rejection must not depend on a
        // malformed or stale claim, nor only on the honest prover's host preflight.
        let claim = prove_kagemusha_platform_credential_hash_claim_v1(
            eq_hash,
            ep_hash,
            &substituted,
            &test_only_recovery_seed(),
        )
        .expect("prove the exact adversarial policy SHA claim");
        with_claim_backed_credential_witness(eq_hash, ep_hash, &substituted, &claim, |witness| {
            assert!(
                discover_kagemusha_platform_credential_audits_v1(
                    eq_params,
                    ep_params,
                    witness,
                    self.provider_policy_root,
                )
                .is_err(),
                "the retained setup root rejects a self-consistent replacement policy"
            );

            let discovery = discover_kagemusha_platform_credential_audits_v1(
                eq_params,
                ep_params,
                witness,
                substituted_root,
            )
            .expect("build an adversarial circuit under its independently substituted setup root");
            assert!(
                build_kagemusha_platform_credential_eq_v1(
                    eq_params,
                    witness,
                    &discovery,
                    self.provider_policy_root,
                )
                .is_err()
            );
            assert!(
                build_kagemusha_platform_credential_ep_v1(
                    ep_params,
                    witness,
                    &discovery,
                    self.provider_policy_root,
                )
                .is_err()
            );

            // Deliberately bypass the honest retained-root selection and attempt to prove the
            // replacement-root circuit with the original PK. Only cryptographic rejection counts.
            let eq_circuit = build_kagemusha_platform_credential_eq_v1(
                eq_params,
                witness,
                &discovery,
                substituted_root,
            )
            .expect("construct exact adversarial Eq circuit");
            assert_base_circuit_params_eq(&eq_circuit.params().base, &self.eq_circuit_params);
            assert_eq!(eq_circuit.params().provider_policy_root, substituted_root);
            let eq_instances = eq_circuit
                .public_instances()
                .expect("adversarial Eq instances");
            if let Ok(proof) =
                try_create_eq_proof(eq_params, &self.eq_proving_key, eq_circuit, &eq_instances)
            {
                assert!(
                    !accepts_eq(&proof, &eq_instances),
                    "original Eq key must reject a different fixed provider root"
                );
            }
            halo2_proofs::release_allocator_slack();
            let ep_circuit = build_kagemusha_platform_credential_ep_v1(
                ep_params,
                witness,
                &discovery,
                substituted_root,
            )
            .expect("construct exact adversarial Ep circuit");
            assert_base_circuit_params_eq(&ep_circuit.params().base, &self.ep_circuit_params);
            assert_eq!(ep_circuit.params().provider_policy_root, substituted_root);
            let ep_instances = ep_circuit
                .public_instances()
                .expect("adversarial Ep instances");
            if let Ok(proof) =
                try_create_ep_proof(ep_params, &self.ep_proving_key, ep_circuit, &ep_instances)
            {
                assert!(
                    !accepts_ep(&proof, &ep_instances),
                    "original Ep key must reject a different fixed provider root"
                );
            }
            halo2_proofs::release_allocator_slack();
        });
    }
}

fn assert_augmented_credential_proof_rejections(
    eq_params: &ParamsIPA<EqAffine>,
    ep_params: &ParamsIPA<EpAffine>,
    keys: &CredentialKeys,
    credential: &CredentialProof,
) {
    let eq_instances = &credential.eq_instances[0];
    let ep_instances = &credential.ep_instances[0];

    let mut eq_truncated = credential.eq_proof.clone();
    eq_truncated.pop();
    assert!(
        verify_eq_succinct_protocol(eq_params, &keys.eq_protocol, &eq_truncated, eq_instances)
            .is_err(),
        "truncated Eq folded-generator encoding must fail closed",
    );
    let mut eq_padded = credential.eq_proof.clone();
    eq_padded.push(0);
    assert!(
        verify_eq_succinct_protocol(eq_params, &keys.eq_protocol, &eq_padded, eq_instances)
            .is_err(),
        "padded Eq augmented proof must fail closed",
    );
    let eq_replacement = EqAffine::generator().to_bytes();
    assert_ne!(
        &credential.eq_proof[credential.eq_proof.len() - 32..],
        eq_replacement.as_ref(),
        "test replacement must differ from the derived Eq folded generator",
    );
    let mut eq_mutated = credential.eq_proof.clone();
    let eq_point_offset = eq_mutated.len() - 32;
    eq_mutated[eq_point_offset..].copy_from_slice(eq_replacement.as_ref());
    assert!(
        verify_eq_succinct_protocol(eq_params, &keys.eq_protocol, &eq_mutated, eq_instances)
            .is_err(),
        "substituted Eq folded generator must fail the succinct equation",
    );
    decide_kagemusha_eq_accumulator_v1(eq_params, &credential.eq_current)
        .expect("valid Eq folded generator must pass the terminal SRS decision");
    let mut eq_forged_accumulator = *credential.eq_current.as_bytes();
    let eq_accumulator_point_offset = eq_forged_accumulator.len() - 32;
    eq_forged_accumulator[eq_accumulator_point_offset..].copy_from_slice(eq_replacement.as_ref());
    let eq_forged_accumulator = KagemushaEqAccumulatorV1::try_from_bytes(&eq_forged_accumulator)
        .expect("canonical forged Eq accumulator fixture");
    assert!(
        decide_kagemusha_eq_accumulator_v1(eq_params, &eq_forged_accumulator).is_err(),
        "substituted Eq folded generator must fail the terminal SRS decision",
    );

    let mut ep_truncated = credential.ep_proof.clone();
    ep_truncated.pop();
    assert!(
        verify_ep_succinct_protocol(ep_params, &keys.ep_protocol, &ep_truncated, ep_instances)
            .is_err(),
        "truncated Ep folded-generator encoding must fail closed",
    );
    let mut ep_padded = credential.ep_proof.clone();
    ep_padded.push(0);
    assert!(
        verify_ep_succinct_protocol(ep_params, &keys.ep_protocol, &ep_padded, ep_instances)
            .is_err(),
        "padded Ep augmented proof must fail closed",
    );
    let ep_replacement = EpAffine::generator().to_bytes();
    assert_ne!(
        &credential.ep_proof[credential.ep_proof.len() - 32..],
        ep_replacement.as_ref(),
        "test replacement must differ from the derived Ep folded generator",
    );
    let mut ep_mutated = credential.ep_proof.clone();
    let ep_point_offset = ep_mutated.len() - 32;
    ep_mutated[ep_point_offset..].copy_from_slice(ep_replacement.as_ref());
    assert!(
        verify_ep_succinct_protocol(ep_params, &keys.ep_protocol, &ep_mutated, ep_instances)
            .is_err(),
        "substituted Ep folded generator must fail the succinct equation",
    );
    decide_kagemusha_ep_accumulator_v1(ep_params, &credential.ep_current)
        .expect("valid Ep folded generator must pass the terminal SRS decision");
    let mut ep_forged_accumulator = *credential.ep_current.as_bytes();
    let ep_accumulator_point_offset = ep_forged_accumulator.len() - 32;
    ep_forged_accumulator[ep_accumulator_point_offset..].copy_from_slice(ep_replacement.as_ref());
    let ep_forged_accumulator = KagemushaEpAccumulatorV1::try_from_bytes(&ep_forged_accumulator)
        .expect("canonical forged Ep accumulator fixture");
    assert!(
        decide_kagemusha_ep_accumulator_v1(ep_params, &ep_forged_accumulator).is_err(),
        "substituted Ep folded generator must fail the terminal SRS decision",
    );
}

fn bootstrap_guard_relation(
    state: &KagemushaStateV1,
    credential: &KagemushaPlatformCredentialStatementV1,
    device_secret: DigestV1,
    empty_effect: DigestV1,
) -> KagemushaGuardBundleRelationWitnessV1 {
    let statement = KagemushaNormalizedGuardStatementV1 {
        version: KAGEMUSHA_WIRE_VERSION_V1,
        protocol_version: KAGEMUSHA_WIRE_VERSION_V1,
        predecessor_suite_id: [0; 32],
        predecessor_vk_digest: [0; 32],
        successor_suite_id: state.suite_id,
        successor_vk_digest: state.vk_digest,
        operation: KagemushaOperationV1::Bootstrap,
        amount: 0,
        peer_credit_id: [0; 32],
        recipient_encryption_key_binding: [0; 32],
        mint_finality_proof_binding_digest: [0; 32],
        predecessor_release_id: [0; 32],
        release_id: state.release_id,
        network_id: *state.lane.network_id.as_bytes(),
        asset_id: kagemusha_asset_identity_digest_v1(&state.lane.asset)
            .expect("bootstrap asset identity"),
        asset_incarnation: state.asset_incarnation,
        asset_scale: state.lane.scale,
        liability_pool_id: state.liability_pool_id,
        hardware_profile_id: state.hardware_profile_id,
        policy_epoch: state.policy_epoch,
        lane_id: state.lane.device_lane_id,
        predecessor_state_commitment: [0; 32],
        successor_state_commitment: state.state_commitment,
        predecessor_state_nonce_commitment: [0; 32],
        successor_state_nonce_commitment: state.state_nonce_commitment,
        predecessor_logical_sequence: 0,
        successor_logical_sequence: 0,
        predecessor_hardware_epoch_generation: 0,
        successor_hardware_epoch_generation: state.hardware_epoch.generation,
        predecessor_hardware_epoch_id: [0; 32],
        successor_hardware_epoch_id: state.hardware_epoch.epoch_id,
        predecessor_key_reference: [0; 32],
        successor_key_reference: state.device_policy_binding.device_key_reference,
        predecessor_hardware_policy_id: [0; 32],
        successor_hardware_policy_id: state.device_policy_binding.hardware_policy_id,
        journal_revision_before: 0,
        journal_revision_after: 0,
        lifecycle_binding_digest: digest(b"bootstrap-lifecycle", 0),
        prepared_transition_binding_digest: [0; 32],
        terminal_commit_binding_digest: [0; 32],
        sender_one_time_authorization_digest: [0; 32],
        receive_credit_binding_digest: [0; 32],
        transition_intent_digest: digest(b"bootstrap-intent", 0),
        transition_effect_digest: digest(b"bootstrap-effect", 0),
        recovery_record_digest: digest(b"bootstrap-recovery", 0),
        durable_inbox_effect_digest: empty_effect,
        durable_outbox_effect_digest: empty_effect,
    };
    let relation = KagemushaGuardBundleRelationWitnessV1 {
        statement,
        canonical_empty_effect_digest: empty_effect,
        predecessor_credential: *credential,
        successor_credential: *credential,
        predecessor_device_authority_secret: device_secret,
        successor_device_authority_secret: device_secret,
    };
    relation.validate().expect("valid bootstrap GuardBundle");
    relation
}

pub(super) fn guard_public_instances<F: KagemushaPoseidonFieldV1>(
    relation: &KagemushaGuardBundleRelationWitnessV1,
    eq_audit: DigestV1,
    ep_audit: DigestV1,
    history: &[u8; KAGEMUSHA_HISTORY_ACCUMULATOR_BYTES_V1],
) -> Vec<F> {
    let mut instances = digest_limbs::<F>(relation.statement_digest()).to_vec();
    instances.extend(digest_limbs::<F>(eq_audit));
    instances.extend(digest_limbs::<F>(ep_audit));
    instances.extend(
        relation
            .credential_digests()
            .into_iter()
            .flat_map(digest_limbs::<F>),
    );
    instances.extend(history.chunks_exact(16).map(|chunk| {
        from_u128::<F>(u128::from_le_bytes(
            chunk.try_into().expect("guard history limb width"),
        ))
    }));
    assert_eq!(instances.len(), GUARD_RECURSIVE_PUBLIC_INSTANCE_COUNT_V1);
    instances
}

struct GuardKeys {
    provider_policy_root: DigestV1,
    eq_proving_key: ProvingKey<EqAffine>,
    ep_proving_key: ProvingKey<EpAffine>,
    eq_verifying_key: VerifyingKey<EqAffine>,
    ep_verifying_key: VerifyingKey<EpAffine>,
    eq_protocol: PlonkProtocol<EqAffine>,
    ep_protocol: PlonkProtocol<EpAffine>,
    eq_circuit_params: BaseCircuitParams,
    ep_circuit_params: BaseCircuitParams,
    eq_protocol_digest: DigestV1,
    ep_protocol_digest: DigestV1,
}

fn assert_base_circuit_params_eq(actual: &BaseCircuitParams, expected: &BaseCircuitParams) {
    assert_eq!(actual.k, expected.k);
    assert_eq!(actual.num_advice_per_phase, expected.num_advice_per_phase);
    assert_eq!(actual.num_fixed, expected.num_fixed);
    assert_eq!(
        actual.num_lookup_advice_per_phase,
        expected.num_lookup_advice_per_phase
    );
    assert_eq!(actual.lookup_bits, expected.lookup_bits);
    assert_eq!(actual.num_instance_columns, expected.num_instance_columns);
}

struct GuardProof {
    relation: KagemushaGuardBundleRelationWitnessV1,
    eq_credential_audit: DigestV1,
    ep_credential_audit: DigestV1,
    eq_proof: Vec<u8>,
    ep_proof: Vec<u8>,
    eq_history: KagemushaEqAccumulatorV1,
    ep_history: KagemushaEpAccumulatorV1,
    eq_current: KagemushaEqAccumulatorV1,
    ep_current: KagemushaEpAccumulatorV1,
}

fn guard_recursive_witness<'a>(
    relation: KagemushaGuardBundleRelationWitnessV1,
    credential_keys: &'a CredentialKeys,
    predecessor: &'a CredentialProof,
    successor: &'a CredentialProof,
    eq_completed: &'a [super::KagemushaEqFoldOutputV1; 2],
    ep_completed: &'a [super::KagemushaEpFoldOutputV1; 2],
    eq_fold: &'a super::KagemushaEqFoldOutputV1,
    ep_fold: &'a super::KagemushaEpFoldOutputV1,
    eq_audit: DigestV1,
    ep_audit: DigestV1,
) -> KagemushaGuardBundleRecursiveWitnessV1<'a> {
    KagemushaGuardBundleRecursiveWitnessV1 {
        relation,
        eq_credential_protocol: &credential_keys.eq_protocol,
        ep_credential_protocol: &credential_keys.ep_protocol,
        eq_credential_instances: [&predecessor.eq_instances[0], &successor.eq_instances[0]],
        ep_credential_instances: [&predecessor.ep_instances[0], &successor.ep_instances[0]],
        eq_credential_claim_histories: [&predecessor.eq_claim_history, &successor.eq_claim_history],
        ep_credential_claim_histories: [&predecessor.ep_claim_history, &successor.ep_claim_history],
        eq_credential_history_fold_proofs: [eq_completed[0].proof(), eq_completed[1].proof()],
        ep_credential_history_fold_proofs: [ep_completed[0].proof(), ep_completed[1].proof()],
        eq_predecessor_credential_proof: &predecessor.eq_proof,
        eq_successor_credential_proof: &successor.eq_proof,
        eq_credential_fold_proof: eq_fold.proof(),
        eq_credential_history: eq_fold.successor(),
        ep_predecessor_credential_proof: &predecessor.ep_proof,
        ep_successor_credential_proof: &successor.ep_proof,
        ep_credential_fold_proof: ep_fold.proof(),
        ep_credential_history: ep_fold.successor(),
        eq_credential_audit: eq_audit,
        ep_credential_audit: ep_audit,
    }
}

fn prove_guard(
    eq_params: &ParamsIPA<EqAffine>,
    ep_params: &ParamsIPA<EpAffine>,
    credential_keys: &CredentialKeys,
    guard_keys: &mut Option<GuardKeys>,
    relation: KagemushaGuardBundleRelationWitnessV1,
    predecessor: &CredentialProof,
    successor: &CredentialProof,
) -> GuardProof {
    let provider_policy_root = credential_keys.provider_policy_root;
    assert_eq!(
        predecessor.relation.statement.hardware_policy_id,
        provider_policy_root
    );
    assert_eq!(
        successor.relation.statement.hardware_policy_id,
        provider_policy_root
    );
    if let Some(keys) = guard_keys.as_ref() {
        assert_eq!(
            keys.provider_policy_root, provider_policy_root,
            "reused Guard keys retain the credential setup root"
        );
    }
    let recovery_seed = test_only_recovery_seed();
    let eq_completed = [predecessor, successor].map(|credential| {
        fold_kagemusha_eq_accumulators_v1(
            eq_params,
            &credential.eq_current,
            &credential.eq_claim_history,
            &recovery_seed,
        )
        .expect("complete Eq credential proof and transported SHA history")
    });
    let ep_completed = [predecessor, successor].map(|credential| {
        fold_kagemusha_ep_accumulators_v1(
            ep_params,
            &credential.ep_current,
            &credential.ep_claim_history,
            &recovery_seed,
        )
        .expect("complete Ep credential proof and transported SHA history")
    });
    let eq_credential_fold = fold_kagemusha_eq_accumulators_v1(
        eq_params,
        eq_completed[0].successor(),
        eq_completed[1].successor(),
        &recovery_seed,
    )
    .expect("merge complete Eq credential histories");
    let ep_credential_fold = fold_kagemusha_ep_accumulators_v1(
        ep_params,
        ep_completed[0].successor(),
        ep_completed[1].successor(),
        &recovery_seed,
    )
    .expect("merge complete Ep credential histories");
    let (_, _, eq_audit, ep_audit) = build_kagemusha_guard_bundle_pair_v1(
        &eq_succinct_vk(eq_params),
        &ep_succinct_vk(ep_params),
        guard_recursive_witness(
            relation.clone(),
            credential_keys,
            predecessor,
            successor,
            &eq_completed,
            &ep_completed,
            &eq_credential_fold,
            &ep_credential_fold,
            [1; 32],
            [2; 32],
        ),
        provider_policy_root,
    )
    .expect("derive GuardBundle audits");
    let build_proof_pair = || {
        build_kagemusha_guard_bundle_pair_v1(
            &eq_succinct_vk(eq_params),
            &ep_succinct_vk(ep_params),
            guard_recursive_witness(
                relation.clone(),
                credential_keys,
                predecessor,
                successor,
                &eq_completed,
                &ep_completed,
                &eq_credential_fold,
                &ep_credential_fold,
                eq_audit,
                ep_audit,
            ),
            provider_policy_root,
        )
        .expect("build GuardBundle proof pair")
    };
    let (mut eq_circuit, mut ep_circuit, rebuilt_eq_audit, rebuilt_ep_audit) = build_proof_pair();
    assert_eq!(rebuilt_eq_audit, eq_audit);
    assert_eq!(rebuilt_ep_audit, ep_audit);

    if guard_keys.is_none() {
        let eq_circuit_params = eq_circuit.params().base;
        let ep_circuit_params = ep_circuit.params().base;
        let eq_proving_key = keygen_pk_with_helper_resource_preflight_consuming_v1(
            eq_params,
            eq_circuit,
            KagemushaPastaParityV1::Eq,
            "GuardBundle",
            "GuardBundle proving key",
        )
        .expect("Eq GuardBundle PK");
        halo2_proofs::release_allocator_slack();
        let eq_verifying_key = eq_proving_key.get_vk().clone();
        let ep_proving_key = keygen_pk_with_helper_resource_preflight_consuming_v1(
            ep_params,
            ep_circuit,
            KagemushaPastaParityV1::Ep,
            "GuardBundle",
            "GuardBundle proving key",
        )
        .expect("Ep GuardBundle PK");
        halo2_proofs::release_allocator_slack();
        let ep_verifying_key = ep_proving_key.get_vk().clone();
        let eq_protocol = compile(
            eq_params,
            &eq_verifying_key,
            snark_verifier::system::halo2::Config::ipa()
                .with_num_instance(vec![GUARD_RECURSIVE_PUBLIC_INSTANCE_COUNT_V1]),
        );
        let ep_protocol = compile(
            ep_params,
            &ep_verifying_key,
            snark_verifier::system::halo2::Config::ipa()
                .with_num_instance(vec![GUARD_RECURSIVE_PUBLIC_INSTANCE_COUNT_V1]),
        );
        let eq_protocol_digest =
            native_parent_protocol_digest_v1(&eq_protocol, KagemushaPastaParityV1::Eq)
                .expect("Eq GuardBundle protocol digest");
        let ep_protocol_digest =
            native_parent_protocol_digest_v1(&ep_protocol, KagemushaPastaParityV1::Ep)
                .expect("Ep GuardBundle protocol digest");
        *guard_keys = Some(GuardKeys {
            provider_policy_root,
            eq_proving_key,
            ep_proving_key,
            eq_verifying_key,
            ep_verifying_key,
            eq_protocol,
            ep_protocol,
            eq_circuit_params,
            ep_circuit_params,
            eq_protocol_digest,
            ep_protocol_digest,
        });

        // Consuming key generation intentionally releases the large witness/configuration
        // graphs before expanding each proving key. Rebuild fresh proving circuits instead of
        // cloning and retaining those graphs across key generation.
        let (rebuilt_eq_circuit, rebuilt_ep_circuit, proof_eq_audit, proof_ep_audit) =
            build_proof_pair();
        assert_eq!(proof_eq_audit, eq_audit);
        assert_eq!(proof_ep_audit, ep_audit);
        eq_circuit = rebuilt_eq_circuit;
        ep_circuit = rebuilt_ep_circuit;
    }
    let keys = guard_keys.as_ref().expect("GuardBundle keys installed");
    assert_base_circuit_params_eq(&eq_circuit.params().base, &keys.eq_circuit_params);
    assert_base_circuit_params_eq(&ep_circuit.params().base, &keys.ep_circuit_params);
    let eq_instances = guard_public_instances::<Fp>(
        &relation,
        eq_audit,
        ep_audit,
        eq_credential_fold.successor().as_bytes(),
    );
    let ep_instances = guard_public_instances::<Fq>(
        &relation,
        eq_audit,
        ep_audit,
        ep_credential_fold.successor().as_bytes(),
    );
    let eq_proof = create_eq_proof(eq_params, &keys.eq_proving_key, eq_circuit, &eq_instances);
    let ep_proof = create_ep_proof(ep_params, &keys.ep_proving_key, ep_circuit, &ep_instances);
    let eq_current = KagemushaEqAccumulatorV1::from_native(
        &verify_eq_succinct_protocol(eq_params, &keys.eq_protocol, &eq_proof, &eq_instances)
            .expect("verify real Eq GuardBundle proof"),
    )
    .expect("encode Eq GuardBundle accumulator");
    let ep_current = KagemushaEpAccumulatorV1::from_native(
        &verify_ep_succinct_protocol(ep_params, &keys.ep_protocol, &ep_proof, &ep_instances)
            .expect("verify real Ep GuardBundle proof"),
    )
    .expect("encode Ep GuardBundle accumulator");
    GuardProof {
        relation,
        eq_credential_audit: eq_audit,
        ep_credential_audit: ep_audit,
        eq_proof,
        ep_proof,
        eq_history: eq_credential_fold.successor().clone(),
        ep_history: ep_credential_fold.successor().clone(),
        eq_current,
        ep_current,
    }
}

struct ParentProof {
    eq_instances: Vec<Vec<Fp>>,
    ep_instances: Vec<Vec<Fq>>,
    eq_proof: Vec<u8>,
    ep_proof: Vec<u8>,
    eq_history: KagemushaEqAccumulatorV1,
    ep_history: KagemushaEpAccumulatorV1,
    eq_current: Option<KagemushaEqAccumulatorV1>,
    ep_current: Option<KagemushaEpAccumulatorV1>,
}

fn dummy_parent(
    eq_protocol: &PlonkProtocol<EqAffine>,
    ep_protocol: &PlonkProtocol<EpAffine>,
    eq_history: KagemushaEqAccumulatorV1,
    ep_history: KagemushaEpAccumulatorV1,
) -> ParentProof {
    ParentProof {
        eq_instances: history_instances(PUBLIC_INSTANCE_COUNT, eq_history.as_bytes()),
        ep_instances: history_instances(PUBLIC_INSTANCE_COUNT, ep_history.as_bytes()),
        eq_proof: dummy_ordinary_proof(eq_protocol, EqAffine::generator()),
        ep_proof: dummy_ordinary_proof(ep_protocol, EpAffine::generator()),
        eq_history,
        ep_history,
        eq_current: None,
        ep_current: None,
    }
}

fn parent_from_generated(proof: KagemushaGeneratedRecursiveStateProofV1) -> ParentProof {
    let KagemushaGeneratedRecursiveStateProofV1 {
        eq_public_instances,
        ep_public_instances,
        eq_transport_public_instances: _,
        ep_transport_public_instances: _,
        eq_inner_proof,
        ep_inner_proof,
        proof: _,
        eq_current_accumulator,
        ep_current_accumulator,
        eq_history,
        ep_history,
    } = proof;
    ParentProof {
        eq_instances: vec![eq_public_instances],
        ep_instances: vec![ep_public_instances],
        eq_proof: eq_inner_proof,
        ep_proof: ep_inner_proof,
        eq_history,
        ep_history,
        eq_current: Some(eq_current_accumulator),
        ep_current: Some(ep_current_accumulator),
    }
}

struct StateKeys {
    eq: KagemushaLoadedEqRecursiveStateArtifactsV1,
    ep: KagemushaLoadedEpRecursiveStateArtifactsV1,
    eq_transport_protocol: PlonkProtocol<EqAffine>,
    ep_transport_protocol: PlonkProtocol<EpAffine>,
    eq_protocol: PlonkProtocol<EqAffine>,
    ep_protocol: PlonkProtocol<EpAffine>,
    eq_protocol_digest: DigestV1,
    ep_protocol_digest: DigestV1,
}

fn decode_state_keys(
    eq_params: &ParamsIPA<EqAffine>,
    ep_params: &ParamsIPA<EpAffine>,
    state: &KagemushaStateV1,
    generated: super::KagemushaGeneratedRecursiveStateArtifactsV1,
) -> StateKeys {
    eprintln!(
        "KAGEMUSHA generated State transport capacity: Eq {:?}; Ep {:?}",
        generated.eq_transport_capacity, generated.ep_transport_capacity,
    );
    let mut eq_transport_vk_cursor = Cursor::new(generated.eq.verifying_key.as_ref());
    let eq_transport_verifying_key = VerifyingKey::read::<_, KagemushaTransportDeciderEqCircuitV1>(
        &mut eq_transport_vk_cursor,
        SerdeFormat::Processed,
        generated.eq_circuit_params.clone(),
    )
    .expect("decode generated Eq transport VK");
    assert_eq!(
        usize::try_from(eq_transport_vk_cursor.position()).expect("Eq transport VK cursor"),
        generated.eq.verifying_key.len()
    );
    let mut eq_transport_pk_cursor = Cursor::new(generated.eq.proving_key.as_ref());
    let eq_transport_proving_key =
        ProvingKey::read_structured_v1_checked::<_, KagemushaTransportDeciderEqCircuitV1>(
            &mut eq_transport_pk_cursor,
            KAGEMUSHA_RECURSION_IPA_K_V1,
            u64::try_from(generated.eq.proving_key.len()).expect("Eq transport PK length fits u64"),
            generated.eq_circuit_params.clone(),
        )
        .expect("decode generated Eq transport PK");
    assert_eq!(
        usize::try_from(eq_transport_pk_cursor.position()).expect("Eq transport PK cursor"),
        generated.eq.proving_key.len()
    );
    assert_generated_structured_proving_key_bytes_v1(
        &eq_transport_proving_key,
        generated.eq.proving_key.as_ref(),
    );
    assert_eq!(
        eq_transport_proving_key
            .get_vk()
            .to_bytes(SerdeFormat::Processed)
            .as_slice(),
        generated.eq.verifying_key.as_ref(),
        "Eq transport embedded VK equals exact generated standalone VK",
    );
    let mut ep_transport_vk_cursor = Cursor::new(generated.ep.verifying_key.as_ref());
    let ep_transport_verifying_key = VerifyingKey::read::<_, KagemushaTransportDeciderEpCircuitV1>(
        &mut ep_transport_vk_cursor,
        SerdeFormat::Processed,
        generated.ep_circuit_params.clone(),
    )
    .expect("decode generated Ep transport VK");
    assert_eq!(
        usize::try_from(ep_transport_vk_cursor.position()).expect("Ep transport VK cursor"),
        generated.ep.verifying_key.len()
    );
    let mut ep_transport_pk_cursor = Cursor::new(generated.ep.proving_key.as_ref());
    let ep_transport_proving_key =
        ProvingKey::read_structured_v1_checked::<_, KagemushaTransportDeciderEpCircuitV1>(
            &mut ep_transport_pk_cursor,
            KAGEMUSHA_RECURSION_IPA_K_V1,
            u64::try_from(generated.ep.proving_key.len()).expect("Ep transport PK length fits u64"),
            generated.ep_circuit_params.clone(),
        )
        .expect("decode generated Ep transport PK");
    assert_eq!(
        usize::try_from(ep_transport_pk_cursor.position()).expect("Ep transport PK cursor"),
        generated.ep.proving_key.len()
    );
    assert_generated_structured_proving_key_bytes_v1(
        &ep_transport_proving_key,
        generated.ep.proving_key.as_ref(),
    );
    assert_eq!(
        ep_transport_proving_key
            .get_vk()
            .to_bytes(SerdeFormat::Processed)
            .as_slice(),
        generated.ep.verifying_key.as_ref(),
        "Ep transport embedded VK equals exact generated standalone VK",
    );

    let mut eq_vk_cursor = Cursor::new(generated.inner_eq.verifying_key.as_ref());
    let eq_verifying_key = VerifyingKey::read::<_, KagemushaRecursiveStateEqCircuitV1>(
        &mut eq_vk_cursor,
        SerdeFormat::Processed,
        generated.inner_eq_circuit_params.clone(),
    )
    .expect("decode generated Eq state VK");
    assert_eq!(
        usize::try_from(eq_vk_cursor.position()).expect("Eq VK cursor"),
        generated.inner_eq.verifying_key.len()
    );
    let mut eq_pk_cursor = Cursor::new(generated.inner_eq.proving_key.as_ref());
    let eq_proving_key =
        ProvingKey::read_structured_v1_checked::<_, KagemushaRecursiveStateEqCircuitV1>(
            &mut eq_pk_cursor,
            KAGEMUSHA_RECURSION_IPA_K_V1,
            u64::try_from(generated.inner_eq.proving_key.len())
                .expect("Eq state PK length fits u64"),
            generated.inner_eq_circuit_params.clone(),
        )
        .expect("decode generated Eq state PK");
    assert_eq!(
        usize::try_from(eq_pk_cursor.position()).expect("Eq PK cursor"),
        generated.inner_eq.proving_key.len()
    );
    assert_generated_structured_proving_key_bytes_v1(
        &eq_proving_key,
        generated.inner_eq.proving_key.as_ref(),
    );
    assert_eq!(
        eq_proving_key
            .get_vk()
            .to_bytes(SerdeFormat::Processed)
            .as_slice(),
        generated.inner_eq.verifying_key.as_ref(),
        "Eq state embedded VK equals exact generated standalone VK",
    );
    let mut ep_vk_cursor = Cursor::new(generated.inner_ep.verifying_key.as_ref());
    let ep_verifying_key = VerifyingKey::read::<_, KagemushaRecursiveStateEpCircuitV1>(
        &mut ep_vk_cursor,
        SerdeFormat::Processed,
        generated.inner_ep_circuit_params.clone(),
    )
    .expect("decode generated Ep state VK");
    assert_eq!(
        usize::try_from(ep_vk_cursor.position()).expect("Ep VK cursor"),
        generated.inner_ep.verifying_key.len()
    );
    let mut ep_pk_cursor = Cursor::new(generated.inner_ep.proving_key.as_ref());
    let ep_proving_key =
        ProvingKey::read_structured_v1_checked::<_, KagemushaRecursiveStateEpCircuitV1>(
            &mut ep_pk_cursor,
            KAGEMUSHA_RECURSION_IPA_K_V1,
            u64::try_from(generated.inner_ep.proving_key.len())
                .expect("Ep state PK length fits u64"),
            generated.inner_ep_circuit_params.clone(),
        )
        .expect("decode generated Ep state PK");
    assert_eq!(
        usize::try_from(ep_pk_cursor.position()).expect("Ep PK cursor"),
        generated.inner_ep.proving_key.len()
    );
    assert_generated_structured_proving_key_bytes_v1(
        &ep_proving_key,
        generated.inner_ep.proving_key.as_ref(),
    );
    assert_eq!(
        ep_proving_key
            .get_vk()
            .to_bytes(SerdeFormat::Processed)
            .as_slice(),
        generated.inner_ep.verifying_key.as_ref(),
        "Ep state embedded VK equals exact generated standalone VK",
    );
    let eq_protocol = compile(
        eq_params,
        &eq_verifying_key,
        snark_verifier::system::halo2::Config::ipa()
            .with_num_instance(vec![RECURSIVE_PUBLIC_INSTANCE_COUNT]),
    );
    let ep_protocol = compile(
        ep_params,
        &ep_verifying_key,
        snark_verifier::system::halo2::Config::ipa()
            .with_num_instance(vec![RECURSIVE_PUBLIC_INSTANCE_COUNT]),
    );
    let eq_transport_protocol = compile(
        eq_params,
        &eq_transport_verifying_key,
        snark_verifier::system::halo2::Config::ipa()
            .with_num_instance(vec![RECURSIVE_PUBLIC_INSTANCE_COUNT]),
    );
    let ep_transport_protocol = compile(
        ep_params,
        &ep_transport_verifying_key,
        snark_verifier::system::halo2::Config::ipa()
            .with_num_instance(vec![RECURSIVE_PUBLIC_INSTANCE_COUNT]),
    );
    let eq_protocol_digest =
        native_parent_protocol_digest_v1(&eq_transport_protocol, KagemushaPastaParityV1::Eq)
            .expect("Eq transport protocol digest");
    let ep_protocol_digest =
        native_parent_protocol_digest_v1(&ep_transport_protocol, KagemushaPastaParityV1::Ep)
            .expect("Ep transport protocol digest");
    StateKeys {
        eq: KagemushaLoadedEqRecursiveStateArtifactsV1 {
            release_id: state.release_id,
            suite_id: state.suite_id,
            vk_digest: state.vk_digest,
            parameters: eq_params.clone(),
            proving_key: eq_transport_proving_key,
            verifying_key: eq_transport_verifying_key,
            circuit_params: generated.eq_circuit_params,
            inner_proving_key: eq_proving_key,
            inner_verifying_key: eq_verifying_key,
            inner_circuit_params: generated.inner_eq_circuit_params,
        },
        ep: KagemushaLoadedEpRecursiveStateArtifactsV1 {
            release_id: state.release_id,
            suite_id: state.suite_id,
            vk_digest: state.vk_digest,
            parameters: ep_params.clone(),
            proving_key: ep_transport_proving_key,
            verifying_key: ep_transport_verifying_key,
            circuit_params: generated.ep_circuit_params,
            inner_proving_key: ep_proving_key,
            inner_verifying_key: ep_verifying_key,
            inner_circuit_params: generated.inner_ep_circuit_params,
        },
        eq_transport_protocol,
        ep_transport_protocol,
        eq_protocol,
        ep_protocol,
        eq_protocol_digest,
        ep_protocol_digest,
    }
}

fn terminally_verify_state_proof(
    state_keys: &StateKeys,
    proof: &KagemushaGeneratedRecursiveStateProofV1,
) {
    let eq_current = KagemushaEqAccumulatorV1::from_native(
        &verify_eq_succinct_protocol(
            &state_keys.eq.parameters,
            &state_keys.eq_protocol,
            &proof.eq_inner_proof,
            &proof.eq_public_instances,
        )
        .expect("reverify real Eq state proof"),
    )
    .expect("encode reverified Eq state accumulator");
    assert_eq!(eq_current, proof.eq_current_accumulator);
    let eq_transport_current = KagemushaEqAccumulatorV1::from_native(
        &verify_eq_succinct_protocol(
            &state_keys.eq.parameters,
            &state_keys.eq_transport_protocol,
            &proof.proof.eq_proof,
            &proof.eq_transport_public_instances,
        )
        .expect("reverify real Eq transport proof"),
    )
    .expect("encode reverified Eq transport accumulator");
    decide_kagemusha_eq_accumulator_v1(&state_keys.eq.parameters, &eq_transport_current)
        .expect("terminally decide Eq transport proof");
    let eq_terminal = KagemushaEqAccumulatorV1::try_from_bytes(&proof.proof.eq_history)
        .expect("decode exact Eq transported history");
    let eq_terminal_instances =
        history_instances::<Fp>(PUBLIC_INSTANCE_COUNT, eq_terminal.as_bytes());
    assert_eq!(
        &proof.eq_transport_public_instances[PUBLIC_INSTANCE_COUNT..],
        &eq_terminal_instances[0][PUBLIC_INSTANCE_COUNT..],
        "Eq wire history must be the history bound by the outer proof",
    );
    decide_kagemusha_eq_accumulator_v1(&state_keys.eq.parameters, &eq_terminal)
        .expect("terminally decide exact Eq transported history");

    let ep_current = KagemushaEpAccumulatorV1::from_native(
        &verify_ep_succinct_protocol(
            &state_keys.ep.parameters,
            &state_keys.ep_protocol,
            &proof.ep_inner_proof,
            &proof.ep_public_instances,
        )
        .expect("reverify real Ep state proof"),
    )
    .expect("encode reverified Ep state accumulator");
    assert_eq!(ep_current, proof.ep_current_accumulator);
    let ep_transport_current = KagemushaEpAccumulatorV1::from_native(
        &verify_ep_succinct_protocol(
            &state_keys.ep.parameters,
            &state_keys.ep_transport_protocol,
            &proof.proof.ep_proof,
            &proof.ep_transport_public_instances,
        )
        .expect("reverify real Ep transport proof"),
    )
    .expect("encode reverified Ep transport accumulator");
    decide_kagemusha_ep_accumulator_v1(&state_keys.ep.parameters, &ep_transport_current)
        .expect("terminally decide Ep transport proof");
    let ep_terminal = KagemushaEpAccumulatorV1::try_from_bytes(&proof.proof.ep_history)
        .expect("decode exact Ep transported history");
    let ep_terminal_instances =
        history_instances::<Fq>(PUBLIC_INSTANCE_COUNT, ep_terminal.as_bytes());
    assert_eq!(
        &proof.ep_transport_public_instances[PUBLIC_INSTANCE_COUNT..],
        &ep_terminal_instances[0][PUBLIC_INSTANCE_COUNT..],
        "Ep wire history must be the history bound by the outer proof",
    );
    decide_kagemusha_ep_accumulator_v1(&state_keys.ep.parameters, &ep_terminal)
        .expect("terminally decide exact Ep transported history");
}

fn eq_transport_boundary_accepts(
    state_keys: &StateKeys,
    proof: &[u8],
    semantic_instances: &[Fp],
    history: &[u8],
) -> bool {
    if semantic_instances.len() != RECURSIVE_PUBLIC_INSTANCE_COUNT {
        return false;
    }
    let Ok(history) = KagemushaEqAccumulatorV1::try_from_bytes(history) else {
        return false;
    };
    let mut instances = semantic_instances[..PUBLIC_INSTANCE_COUNT].to_vec();
    instances.extend(history_instances::<Fp>(0, history.as_bytes()).remove(0));
    // Succinct verification only returns a deferred IPA equation. Monetary acceptance requires
    // deciding both that equation and the separately transported history, exactly as production.
    let Ok(current) = verify_eq_succinct_protocol(
        &state_keys.eq.parameters,
        &state_keys.eq_transport_protocol,
        proof,
        &instances,
    ) else {
        return false;
    };
    let Ok(current) = KagemushaEqAccumulatorV1::from_native(&current) else {
        return false;
    };
    decide_kagemusha_eq_accumulator_v1(&state_keys.eq.parameters, &current).is_ok()
        && decide_kagemusha_eq_accumulator_v1(&state_keys.eq.parameters, &history).is_ok()
}

fn ep_transport_boundary_accepts(
    state_keys: &StateKeys,
    proof: &[u8],
    semantic_instances: &[Fq],
    history: &[u8],
) -> bool {
    if semantic_instances.len() != RECURSIVE_PUBLIC_INSTANCE_COUNT {
        return false;
    }
    let Ok(history) = KagemushaEpAccumulatorV1::try_from_bytes(history) else {
        return false;
    };
    let mut instances = semantic_instances[..PUBLIC_INSTANCE_COUNT].to_vec();
    instances.extend(history_instances::<Fq>(0, history.as_bytes()).remove(0));
    // Do not mistake successful parsing/accumulation for proof acceptance; decision is authoritative.
    let Ok(current) = verify_ep_succinct_protocol(
        &state_keys.ep.parameters,
        &state_keys.ep_transport_protocol,
        proof,
        &instances,
    ) else {
        return false;
    };
    let Ok(current) = KagemushaEpAccumulatorV1::from_native(&current) else {
        return false;
    };
    decide_kagemusha_ep_accumulator_v1(&state_keys.ep.parameters, &current).is_ok()
        && decide_kagemusha_ep_accumulator_v1(&state_keys.ep.parameters, &history).is_ok()
}

fn paired_transport_boundary_accepts(
    state_keys: &StateKeys,
    proof: &KagemushaPairedProofV1,
    expected_semantic_digest: [u8; 32],
    eq_semantic_instances: &[Fp],
    ep_semantic_instances: &[Fq],
) -> bool {
    if proof.eq_protocol_digest != state_keys.eq_protocol_digest
        || proof.ep_protocol_digest != state_keys.ep_protocol_digest
        || proof
            .validate_shape_for_semantic_digest(expected_semantic_digest)
            .is_err()
    {
        return false;
    }
    let eq_transport = digest_limbs::<Fp>(proof.semantic_digest);
    let ep_transport = digest_limbs::<Fq>(proof.semantic_digest);
    if eq_semantic_instances.get(public_instance::TRANSPORT_LO..=public_instance::TRANSPORT_HI)
        != Some(eq_transport.as_slice())
        || ep_semantic_instances.get(public_instance::TRANSPORT_LO..=public_instance::TRANSPORT_HI)
            != Some(ep_transport.as_slice())
    {
        return false;
    }
    eq_transport_boundary_accepts(
        state_keys,
        &proof.eq_proof,
        eq_semantic_instances,
        &proof.eq_history,
    ) && ep_transport_boundary_accepts(
        state_keys,
        &proof.ep_proof,
        ep_semantic_instances,
        &proof.ep_history,
    )
}

fn assert_transport_public_substitutions_rejected(
    state_keys: &StateKeys,
    proof: &KagemushaGeneratedRecursiveStateProofV1,
) {
    assert!(paired_transport_boundary_accepts(
        state_keys,
        &proof.proof,
        proof.proof.semantic_digest,
        &proof.eq_transport_public_instances,
        &proof.ep_transport_public_instances,
    ));
    assert_ne!(
        proof.proof.eq_history.as_slice(),
        &proof.eq_history.as_bytes()[..],
        "Eq transported history must differ from the pre-fold private history",
    );
    assert_ne!(
        proof.proof.ep_history.as_slice(),
        &proof.ep_history.as_bytes()[..],
        "Ep transported history must differ from the pre-fold private history",
    );

    let mut old_history = proof.proof.clone();
    old_history.eq_history = proof.eq_history.as_bytes().to_vec();
    assert!(
        !paired_transport_boundary_accepts(
            state_keys,
            &old_history,
            proof.proof.semantic_digest,
            &proof.eq_transport_public_instances,
            &proof.ep_transport_public_instances,
        ),
        "the outer boundary must reject substitution of the Eq pre-fold private history",
    );
    old_history = proof.proof.clone();
    old_history.ep_history = proof.ep_history.as_bytes().to_vec();
    assert!(
        !paired_transport_boundary_accepts(
            state_keys,
            &old_history,
            proof.proof.semantic_digest,
            &proof.eq_transport_public_instances,
            &proof.ep_transport_public_instances,
        ),
        "the outer boundary must reject substitution of the Ep pre-fold private history",
    );

    let mut eq_semantic = proof.eq_transport_public_instances.clone();
    let mut ep_semantic = proof.ep_transport_public_instances.clone();
    eq_semantic[public_instance::AMOUNT] += Fp::from(1);
    ep_semantic[public_instance::AMOUNT] += Fq::from(1);
    assert!(
        !paired_transport_boundary_accepts(
            state_keys,
            &proof.proof,
            proof.proof.semantic_digest,
            &eq_semantic,
            &ep_semantic,
        ),
        "the outer boundary must reject substituted semantic/public outputs",
    );
}

#[test]
fn diagnostic_policy_setup_is_independent_of_device_release_and_empty_effect() {
    let setup = DiagnosticProviderPolicy::new(diagnostic_hardware_profile());
    let (first, _) = credential_witness_with_policy(
        0,
        digest(b"policy-test-release", 0),
        digest(b"policy-test-empty", 0),
        &setup,
    );
    let (second, _) = credential_witness_with_policy(
        1,
        digest(b"policy-test-release", 1),
        digest(b"policy-test-empty", 1),
        &setup,
    );
    assert_eq!(first.statement.hardware_policy_id, setup.root);
    assert_eq!(second.statement.hardware_policy_id, setup.root);
    assert_eq!(first.policy_siblings, setup.siblings);
    assert_eq!(second.policy_siblings, setup.siblings);
    assert_ne!(
        first.statement.canonical_digest(),
        second.statement.canonical_digest()
    );
    assert_eq!(
        first.statement.platform_class,
        KagemushaHardwarePlatformClassV1::DedicatedSecureElement as u8
    );
    let mut substituted = second;
    substituted.statement.hardware_policy_id = digest(b"caller-selected-policy", 0);
    assert!(substituted.validate().is_err());
    assert_eq!(
        setup.root, first.statement.hardware_policy_id,
        "changing a caller witness cannot replace the retained setup policy"
    );
}

#[test]
fn bootstrap_guard_shape_preflight_needs_no_halo2_proof() {
    let release_id = digest(b"release", 0);
    let empty_effect = digest(b"empty-durable-effect", 0);
    let (credential, device_secret) = credential_witness(0, release_id, empty_effect);
    let state = aggregate_state(release_id, &credential.statement, digest(b"state-nonce", 0));
    let relation =
        bootstrap_guard_relation(&state, &credential.statement, device_secret, empty_effect);
    relation.validate().expect("valid bootstrap GuardBundle");
}

#[test]
#[ignore = "blocked until the real funded SendSplit and ReceiveFold generator is complete"]
fn real_1024_payment_handoffs_must_pass_the_fail_closed_evidence_corridor() {
    // TODO: Replace this explicit failure with the real positive-balance alternating-device
    // generator once MintFold exposes its funded recursive state output to SendSplit. Keeping the
    // gate fail-closed prevents a model loop or Rotate proof from satisfying this qualification.
    let generated: Result<Vec<GeneratedHandoffEvidenceV1>, &str> = Err(
        "real positive-value MintFold -> SendSplit -> PaymentV1 -> ReceiveFold generation is not yet wired",
    );
    let generated = generated.expect("real 1,024-handoff generator must be installed");
    let verifier = super::RejectAllKagemushaRecursiveVerifierV1;
    let artifacts = super::tests::artifacts();
    let verified = verify_real_handoff_qualification_v1(&verifier, artifacts, &generated);
    assert_eq!(verified.verified_handoffs, 1_024);
}
