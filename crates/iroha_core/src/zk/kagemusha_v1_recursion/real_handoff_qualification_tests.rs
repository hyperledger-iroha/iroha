//! Real paired-Pasta recursive-history qualification for KAGEMUSHA V1.
//!
//! Payment qualification uses the production recursive state, GuardBundle, mint authority,
//! credential, fold, and native terminal-verification code. Only a positive-value SendSplit
//! followed by ReceiveFold counts as a payment handoff. Inactive parser padding is never
//! accepted as monetary authority or reported as a real payment proof.

#[path = "real_payment_corridor.rs"]
mod real_payment_corridor;

use std::io::Cursor;

use halo2_base::gates::circuit::BaseCircuitParams;
use halo2_proofs::{
    SerdeFormat,
    halo2curves::{
        CurveAffine,
        group::{GroupEncoding as _, prime::PrimeCurveAffine as _},
        pasta::{EpAffine, EqAffine, Fp, Fq},
    },
    plonk::{Circuit, ProvingKey, VerifyingKey, create_proof, keygen_pk, keygen_vk},
    poly::{
        commitment::ParamsProver as _,
        ipa::{
            commitment::{IPACommitmentScheme, ParamsIPA},
            multiopen::ProverIPA,
        },
    },
};
use iroha_crypto::{Hash, HashOf, kagemusha::KagemushaRecoverySeedV1};
use iroha_data_model::{
    NetworkId,
    asset::AssetDefinitionId,
    block::BlockHeader,
    domain::DomainId,
    kagemusha::{
        KAGEMUSHA_HALO2_K_V1, KAGEMUSHA_HARDWARE_REQUIRED_CAPABILITIES_V1,
        KAGEMUSHA_PAIRED_PROOF_MAX_BYTES_V1, KAGEMUSHA_WIRE_VERSION_V1, KagemushaDevicePublicKeyV1,
        KagemushaPairedProofV1, KagemushaPastaStateCommitmentV1,
        kagemusha_asset_identity_digest_v1, kagemusha_device_key_reference_v1,
        kagemusha_liability_pool_id_v1, kagemusha_pasta_state_commitment_v1,
    },
    nexus::AxtAssetIncarnationV1,
};
use p256::ecdsa::SigningKey;
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

use super::{
    KAGEMUSHA_HISTORY_ACCUMULATOR_BYTES_V1, KAGEMUSHA_IPA_FOLD_PROOF_BYTES_V1,
    KAGEMUSHA_IPA_POSEIDON_FULL_ROUNDS_V1, KAGEMUSHA_IPA_POSEIDON_PARTIAL_ROUNDS_V1,
    KAGEMUSHA_IPA_POSEIDON_RATE_V1, KAGEMUSHA_IPA_POSEIDON_SECURE_MDS_V1,
    KAGEMUSHA_IPA_POSEIDON_WIDTH_V1, KAGEMUSHA_RECURSION_IPA_K_V1, KagemushaEpAccumulatorV1,
    KagemushaEpFoldProofV1, KagemushaEqAccumulatorV1, KagemushaEqFoldProofV1,
    KagemushaGeneratedRecursiveStateProofV1, KagemushaGuardBundleRelationWitnessV1,
    KagemushaLoadedEpRecursiveStateArtifactsV1, KagemushaLoadedEqRecursiveStateArtifactsV1,
    KagemushaNormalizedGuardStatementV1, KagemushaOperationV1, KagemushaPastaParityV1,
    KagemushaPlatformCredentialRelationCircuitV1, KagemushaPlatformCredentialRelationWitnessV1,
    KagemushaPlatformCredentialStatementV1, KagemushaStateRelationWitnessV1,
    composite::{
        KagemushaRecursiveStateEpCircuitV1, KagemushaRecursiveStateEqCircuitV1, ep_succinct_vk,
        eq_succinct_vk,
    },
    decide_kagemusha_ep_accumulator_v1, decide_kagemusha_eq_accumulator_v1,
    deferred_parent::{
        accumulator_limb_count, native_parent_protocol_digest_v1, ordinary_ipa_proof_profile_v1,
    },
    fold_kagemusha_ep_accumulators_v1, fold_kagemusha_eq_accumulators_v1,
    generation::{KagemushaRawHalo2IpaProofV1, augment_halo2_ipa_proof_v1},
    guard_bundle::{
        GUARD_RECURSIVE_PUBLIC_INSTANCE_COUNT_V1, KagemushaGuardBundleRecursiveWitnessV1,
        build_kagemusha_guard_bundle_pair_v1, device_authority_commitment_v1,
    },
    initial_kagemusha_ep_accumulator_v1, initial_kagemusha_eq_accumulator_v1,
    mint_authority::KAGEMUSHA_MINT_AUTHORITY_PUBLIC_INSTANCE_COUNT_V1,
    native_backend::{verify_ep_succinct_protocol, verify_eq_succinct_protocol},
    state_relation::{PUBLIC_INSTANCE_COUNT, public_instance},
    transport_decider::{
        KagemushaTransportDeciderCapacityProfileV1, KagemushaTransportDeciderEpCircuitV1,
        KagemushaTransportDeciderEqCircuitV1, KagemushaTransportDeciderParityWitnessV1,
        KagemushaTransportDeciderWitnessV1, build_kagemusha_transport_decider_pair_v1,
    },
};
use crate::zk::{
    kagemusha_v1_poseidon::{
        KAGEMUSHA_STATE_DOMAIN_V1, KagemushaPoseidonFieldV1, decode as decode_pasta, digest_limbs,
        empty_replay_root, encode as encode_pasta, from_u128, hash as pasta_hash,
    },
    kagemusha_v1_state::{
        DevicePolicyBindingV1, DigestV1, HardwareEpochV1, KAGEMUSHA_STATE_VERSION_V1,
        KagemushaLaneIdV1, KagemushaStateV1,
    },
};

const PROVIDER_AUTHORITY_DOMAIN: &[u8] = b"iroha:kagemusha:v1:provider-proof-authority";
const POLICY_LEAF_DOMAIN: &[u8] = b"iroha:kagemusha:v1:hardware-policy-leaf";
const POLICY_NODE_DOMAIN: &[u8] = b"iroha:kagemusha:v1:hardware-policy-node";
const RECURSIVE_PUBLIC_INSTANCE_COUNT: usize =
    PUBLIC_INSTANCE_COUNT + KAGEMUSHA_HISTORY_ACCUMULATOR_BYTES_V1 / 16;

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
    hasher.update(statement.canonical_empty_effect_digest);
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

pub(super) fn credential_witness(
    index: u64,
    release_id: DigestV1,
    empty_effect: DigestV1,
) -> (KagemushaPlatformCredentialRelationWitnessV1, DigestV1) {
    let provider_secret = digest(b"provider-secret", 0);
    let device_secret = digest(b"device-secret", index);
    let signing_key = deterministic_signing_key(index);
    let device_public_key = KagemushaDevicePublicKeyV1::from_sec1_bytes(
        signing_key
            .verifying_key()
            .to_encoded_point(false)
            .as_bytes(),
    )
    .expect("canonical qualification P-256 key");
    let policy_siblings = core::array::from_fn(|depth| {
        digest(
            b"policy-sibling",
            u64::try_from(depth).expect("policy depth fits u64"),
        )
    });
    let lane = lane();
    let asset_incarnation = incarnation();
    let mut statement = KagemushaPlatformCredentialStatementV1 {
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
        hardware_policy_id: digest(b"temporary-policy", 0),
        device_authority_commitment: device_authority_commitment_v1(device_secret),
        hardware_profile_id: digest(b"hardware-profile", 0),
        policy_epoch: 1,
        platform_class: 1,
        capability_mask: KAGEMUSHA_HARDWARE_REQUIRED_CAPABILITIES_V1,
        provider_authority_commitment: provider_authority_commitment(provider_secret),
        platform_attestation_digest: digest(b"platform-attestation", index),
        credential_issuance_digest: digest(b"credential-issuance", index),
        canonical_empty_effect_digest: empty_effect,
        provider_profile_index: 0x5a31,
    };
    let mut root = policy_leaf(&statement);
    for (depth, sibling) in policy_siblings.iter().copied().enumerate() {
        root = if (statement.provider_profile_index >> depth) & 1 == 0 {
            policy_node(root, sibling)
        } else {
            policy_node(sibling, root)
        };
    }
    statement.hardware_policy_id = root;
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
    .expect("real Eq Halo2 proof");
    let raw = transcript.finalize();
    let proof = augment_halo2_ipa_proof_v1(
        params,
        proving_key.get_vk(),
        KagemushaRawHalo2IpaProofV1::new(raw),
        instances,
    )
    .expect("augment real Eq Halo2 proof");
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
    proof
}

pub(super) fn create_ep_proof<C: Circuit<Fq>>(
    params: &ParamsIPA<EpAffine>,
    proving_key: &ProvingKey<EpAffine>,
    circuit: C,
    instances: &[Fq],
) -> Vec<u8> {
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
    .expect("real Ep Halo2 proof");
    let raw = transcript.finalize();
    let proof = augment_halo2_ipa_proof_v1(
        params,
        proving_key.get_vk(),
        KagemushaRawHalo2IpaProofV1::new(raw),
        instances,
    )
    .expect("augment real Ep Halo2 proof");
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
    proof
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
    eq_proving_key: ProvingKey<EqAffine>,
    ep_proving_key: ProvingKey<EpAffine>,
    eq_protocol: PlonkProtocol<EqAffine>,
    ep_protocol: PlonkProtocol<EpAffine>,
}

struct CredentialProof {
    relation: KagemushaPlatformCredentialRelationWitnessV1,
    device_secret: DigestV1,
    eq_proof: Vec<u8>,
    ep_proof: Vec<u8>,
    eq_current: KagemushaEqAccumulatorV1,
    ep_current: KagemushaEpAccumulatorV1,
}

impl CredentialKeys {
    fn generate(
        eq_params: &ParamsIPA<EqAffine>,
        ep_params: &ParamsIPA<EpAffine>,
        witness: &KagemushaPlatformCredentialRelationWitnessV1,
    ) -> Self {
        let eq_circuit = KagemushaPlatformCredentialRelationCircuitV1::<Fp>::new(witness.clone())
            .expect("Eq credential circuit");
        let ep_circuit = KagemushaPlatformCredentialRelationCircuitV1::<Fq>::new(witness.clone())
            .expect("Ep credential circuit");
        let eq_verifying_key = keygen_vk(eq_params, &eq_circuit).expect("Eq credential VK");
        let eq_proving_key =
            keygen_pk(eq_params, eq_verifying_key.clone(), &eq_circuit).expect("Eq credential PK");
        let ep_verifying_key = keygen_vk(ep_params, &ep_circuit).expect("Ep credential VK");
        let ep_proving_key =
            keygen_pk(ep_params, ep_verifying_key.clone(), &ep_circuit).expect("Ep credential PK");
        let eq_protocol = compile(
            eq_params,
            &eq_verifying_key,
            snark_verifier::system::halo2::Config::ipa().with_num_instance(vec![2]),
        );
        let ep_protocol = compile(
            ep_params,
            &ep_verifying_key,
            snark_verifier::system::halo2::Config::ipa().with_num_instance(vec![2]),
        );
        Self {
            eq_proving_key,
            ep_proving_key,
            eq_protocol,
            ep_protocol,
        }
    }

    fn prove(
        &self,
        eq_params: &ParamsIPA<EqAffine>,
        ep_params: &ParamsIPA<EpAffine>,
        relation: KagemushaPlatformCredentialRelationWitnessV1,
        device_secret: DigestV1,
    ) -> CredentialProof {
        let eq_circuit = KagemushaPlatformCredentialRelationCircuitV1::<Fp>::new(relation.clone())
            .expect("Eq credential circuit");
        let ep_circuit = KagemushaPlatformCredentialRelationCircuitV1::<Fq>::new(relation.clone())
            .expect("Ep credential circuit");
        let eq_instances = eq_circuit
            .public_instances()
            .expect("Eq credential instances");
        let ep_instances = ep_circuit
            .public_instances()
            .expect("Ep credential instances");
        let eq_proof = create_eq_proof(eq_params, &self.eq_proving_key, eq_circuit, &eq_instances);
        let ep_proof = create_ep_proof(ep_params, &self.ep_proving_key, ep_circuit, &ep_instances);
        let eq_current = KagemushaEqAccumulatorV1::from_native(
            &verify_eq_succinct_protocol(eq_params, &self.eq_protocol, &eq_proof, &eq_instances)
                .expect("verify real Eq credential proof"),
        )
        .expect("encode Eq credential accumulator");
        let ep_current = KagemushaEpAccumulatorV1::from_native(
            &verify_ep_succinct_protocol(ep_params, &self.ep_protocol, &ep_proof, &ep_instances)
                .expect("verify real Ep credential proof"),
        )
        .expect("encode Ep credential accumulator");
        CredentialProof {
            relation,
            device_secret,
            eq_proof,
            ep_proof,
            eq_current,
            ep_current,
        }
    }
}

fn assert_augmented_credential_proof_rejections(
    eq_params: &ParamsIPA<EqAffine>,
    ep_params: &ParamsIPA<EpAffine>,
    keys: &CredentialKeys,
    credential: &CredentialProof,
) {
    let eq_instances =
        KagemushaPlatformCredentialRelationCircuitV1::<Fp>::new(credential.relation.clone())
            .expect("Eq credential circuit for augmented-proof rejection")
            .public_instances()
            .expect("Eq credential instances for augmented-proof rejection");
    let ep_instances =
        KagemushaPlatformCredentialRelationCircuitV1::<Fq>::new(credential.relation.clone())
            .expect("Ep credential circuit for augmented-proof rejection")
            .public_instances()
            .expect("Ep credential instances for augmented-proof rejection");

    let mut eq_truncated = credential.eq_proof.clone();
    eq_truncated.pop();
    assert!(
        verify_eq_succinct_protocol(eq_params, &keys.eq_protocol, &eq_truncated, &eq_instances)
            .is_err(),
        "truncated Eq folded-generator encoding must fail closed",
    );
    let mut eq_padded = credential.eq_proof.clone();
    eq_padded.push(0);
    assert!(
        verify_eq_succinct_protocol(eq_params, &keys.eq_protocol, &eq_padded, &eq_instances)
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
        verify_eq_succinct_protocol(eq_params, &keys.eq_protocol, &eq_mutated, &eq_instances)
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
        verify_ep_succinct_protocol(ep_params, &keys.ep_protocol, &ep_truncated, &ep_instances)
            .is_err(),
        "truncated Ep folded-generator encoding must fail closed",
    );
    let mut ep_padded = credential.ep_proof.clone();
    ep_padded.push(0);
    assert!(
        verify_ep_succinct_protocol(ep_params, &keys.ep_protocol, &ep_padded, &ep_instances)
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
        verify_ep_succinct_protocol(ep_params, &keys.ep_protocol, &ep_mutated, &ep_instances)
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
        peer_recipient_lane_id: [0; 32],
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
        precommit_binding_digest: [0; 32],
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
    instances.extend(history.chunks_exact(16).map(|chunk| {
        from_u128::<F>(u128::from_le_bytes(
            chunk.try_into().expect("guard history limb width"),
        ))
    }));
    assert_eq!(instances.len(), GUARD_RECURSIVE_PUBLIC_INSTANCE_COUNT_V1);
    instances
}

struct GuardKeys {
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
    eq_fold: &'a super::KagemushaEqFoldOutputV1,
    ep_fold: &'a super::KagemushaEpFoldOutputV1,
    eq_audit: DigestV1,
    ep_audit: DigestV1,
) -> KagemushaGuardBundleRecursiveWitnessV1<'a> {
    KagemushaGuardBundleRecursiveWitnessV1 {
        relation,
        eq_credential_protocol: &credential_keys.eq_protocol,
        ep_credential_protocol: &credential_keys.ep_protocol,
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
    let recovery_seed = test_only_recovery_seed();
    let eq_credential_fold = fold_kagemusha_eq_accumulators_v1(
        eq_params,
        &predecessor.eq_current,
        &successor.eq_current,
        &recovery_seed,
    )
    .expect("fold Eq credential proofs");
    let ep_credential_fold = fold_kagemusha_ep_accumulators_v1(
        ep_params,
        &predecessor.ep_current,
        &successor.ep_current,
        &recovery_seed,
    )
    .expect("fold Ep credential proofs");
    let (_, _, eq_audit, ep_audit) = build_kagemusha_guard_bundle_pair_v1(
        &eq_succinct_vk(eq_params),
        &ep_succinct_vk(ep_params),
        guard_recursive_witness(
            relation.clone(),
            credential_keys,
            predecessor,
            successor,
            &eq_credential_fold,
            &ep_credential_fold,
            [1; 32],
            [2; 32],
        ),
    )
    .expect("derive GuardBundle audits");
    let (eq_circuit, ep_circuit, rebuilt_eq_audit, rebuilt_ep_audit) =
        build_kagemusha_guard_bundle_pair_v1(
            &eq_succinct_vk(eq_params),
            &ep_succinct_vk(ep_params),
            guard_recursive_witness(
                relation.clone(),
                credential_keys,
                predecessor,
                successor,
                &eq_credential_fold,
                &ep_credential_fold,
                eq_audit,
                ep_audit,
            ),
        )
        .expect("build GuardBundle proof pair");
    assert_eq!(rebuilt_eq_audit, eq_audit);
    assert_eq!(rebuilt_ep_audit, ep_audit);

    if guard_keys.is_none() {
        let eq_circuit_params = eq_circuit.params();
        let ep_circuit_params = ep_circuit.params();
        let eq_verifying_key = keygen_vk(eq_params, &eq_circuit).expect("Eq GuardBundle VK");
        let eq_proving_key =
            keygen_pk(eq_params, eq_verifying_key.clone(), &eq_circuit).expect("Eq GuardBundle PK");
        let ep_verifying_key = keygen_vk(ep_params, &ep_circuit).expect("Ep GuardBundle VK");
        let ep_proving_key =
            keygen_pk(ep_params, ep_verifying_key.clone(), &ep_circuit).expect("Ep GuardBundle PK");
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
    }
    let keys = guard_keys.as_ref().expect("GuardBundle keys installed");
    assert_base_circuit_params_eq(&eq_circuit.params(), &keys.eq_circuit_params);
    assert_base_circuit_params_eq(&ep_circuit.params(), &keys.ep_circuit_params);
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

struct DisabledMint {
    eq_protocol: PlonkProtocol<EqAffine>,
    ep_protocol: PlonkProtocol<EpAffine>,
    eq_protocol_digest: DigestV1,
    ep_protocol_digest: DigestV1,
    eq_instances: Vec<Vec<Fp>>,
    ep_instances: Vec<Vec<Fq>>,
    eq_proof: Vec<u8>,
    ep_proof: Vec<u8>,
    eq_history: KagemushaEqAccumulatorV1,
    ep_history: KagemushaEpAccumulatorV1,
    eq_fold: KagemushaEqFoldProofV1,
    ep_fold: KagemushaEpFoldProofV1,
}

impl DisabledMint {
    fn new(
        eq_params: &ParamsIPA<EqAffine>,
        ep_params: &ParamsIPA<EpAffine>,
        guard_keys: &GuardKeys,
    ) -> Self {
        let eq_protocol = compile(
            eq_params,
            &guard_keys.eq_verifying_key,
            snark_verifier::system::halo2::Config::ipa()
                .with_num_instance(vec![KAGEMUSHA_MINT_AUTHORITY_PUBLIC_INSTANCE_COUNT_V1]),
        );
        let ep_protocol = compile(
            ep_params,
            &guard_keys.ep_verifying_key,
            snark_verifier::system::halo2::Config::ipa()
                .with_num_instance(vec![KAGEMUSHA_MINT_AUTHORITY_PUBLIC_INSTANCE_COUNT_V1]),
        );
        let eq_protocol_digest =
            native_parent_protocol_digest_v1(&eq_protocol, KagemushaPastaParityV1::Eq)
                .expect("Eq disabled-mint protocol digest");
        let ep_protocol_digest =
            native_parent_protocol_digest_v1(&ep_protocol, KagemushaPastaParityV1::Ep)
                .expect("Ep disabled-mint protocol digest");
        let eq_history =
            initial_kagemusha_eq_accumulator_v1(eq_params).expect("Eq disabled-mint seed history");
        let ep_history =
            initial_kagemusha_ep_accumulator_v1(ep_params).expect("Ep disabled-mint seed history");
        let prefix = KAGEMUSHA_MINT_AUTHORITY_PUBLIC_INSTANCE_COUNT_V1
            .checked_sub(accumulator_limb_count())
            .expect("mint public prefix");
        Self {
            eq_instances: history_instances(prefix, eq_history.as_bytes()),
            ep_instances: history_instances(prefix, ep_history.as_bytes()),
            eq_proof: dummy_ordinary_proof(&eq_protocol, EqAffine::generator()),
            ep_proof: dummy_ordinary_proof(&ep_protocol, EpAffine::generator()),
            eq_protocol,
            ep_protocol,
            eq_protocol_digest,
            ep_protocol_digest,
            eq_history,
            ep_history,
            eq_fold: dummy_eq_fold(),
            ep_fold: dummy_ep_fold(),
        }
    }
}

fn bootstrap_state_relation(
    successor: KagemushaStateV1,
    guard: &GuardProof,
    eq_protocol_digest: DigestV1,
    ep_protocol_digest: DigestV1,
    incoming_eq_protocol_digest: DigestV1,
    incoming_ep_protocol_digest: DigestV1,
    guard_keys: &GuardKeys,
    mint: &DisabledMint,
) -> KagemushaStateRelationWitnessV1 {
    let witness = KagemushaStateRelationWitnessV1 {
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
        peer_recipient_lane_id: [0; 32],
        receive_credit: None,
        receive_credit_binding_digest: [0; 32],
        lifecycle_binding_digest: guard.relation.statement.lifecycle_binding_digest,
        precommit_binding_digest: [0; 32],
        transport_semantic_digest: digest(b"bootstrap-transport", 0),
        guard_statement_digest: guard.relation.statement_digest(),
        eq_protocol_digest,
        ep_protocol_digest,
        guard_eq_protocol_digest: guard_keys.eq_protocol_digest,
        guard_ep_protocol_digest: guard_keys.ep_protocol_digest,
        mint_eq_protocol_digest: mint.eq_protocol_digest,
        mint_ep_protocol_digest: mint.ep_protocol_digest,
        commit_wrapper_eq_protocol_digest: incoming_eq_protocol_digest,
        commit_wrapper_ep_protocol_digest: incoming_ep_protocol_digest,
        guard_eq_credential_audit: guard.eq_credential_audit,
        guard_ep_credential_audit: guard.ep_credential_audit,
        eq_deferred_audit: [1; 32],
        ep_deferred_audit: [2; 32],
        replay_insert: None,
    };
    witness.validate().expect("valid bootstrap state relation");
    witness
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

struct StateHistoryFolds {
    eq_parent_fold: KagemushaEqFoldProofV1,
    ep_parent_fold: KagemushaEpFoldProofV1,
    eq_guard_history_fold: KagemushaEqFoldProofV1,
    ep_guard_history_fold: KagemushaEpFoldProofV1,
    eq_guard_merge_fold: KagemushaEqFoldProofV1,
    ep_guard_merge_fold: KagemushaEpFoldProofV1,
    eq_successor_history: KagemushaEqAccumulatorV1,
    ep_successor_history: KagemushaEpAccumulatorV1,
}

fn prepare_state_histories(
    eq_params: &ParamsIPA<EqAffine>,
    ep_params: &ParamsIPA<EpAffine>,
    parent: &ParentProof,
    guard: &GuardProof,
) -> StateHistoryFolds {
    let recovery_seed = test_only_recovery_seed();
    let (eq_parent_fold, eq_base_history) = if let Some(current) = &parent.eq_current {
        let fold = fold_kagemusha_eq_accumulators_v1(
            eq_params,
            current,
            &parent.eq_history,
            &recovery_seed,
        )
        .expect("fold Eq state predecessor");
        (fold.proof().clone(), fold.successor().clone())
    } else {
        (dummy_eq_fold(), parent.eq_history.clone())
    };
    let (ep_parent_fold, ep_base_history) = if let Some(current) = &parent.ep_current {
        let fold = fold_kagemusha_ep_accumulators_v1(
            ep_params,
            current,
            &parent.ep_history,
            &recovery_seed,
        )
        .expect("fold Ep state predecessor");
        (fold.proof().clone(), fold.successor().clone())
    } else {
        (dummy_ep_fold(), parent.ep_history.clone())
    };
    let eq_guard_history = fold_kagemusha_eq_accumulators_v1(
        eq_params,
        &guard.eq_current,
        &guard.eq_history,
        &recovery_seed,
    )
    .expect("complete Eq GuardBundle history");
    let ep_guard_history = fold_kagemusha_ep_accumulators_v1(
        ep_params,
        &guard.ep_current,
        &guard.ep_history,
        &recovery_seed,
    )
    .expect("complete Ep GuardBundle history");
    let eq_guard_merge = fold_kagemusha_eq_accumulators_v1(
        eq_params,
        &eq_base_history,
        eq_guard_history.successor(),
        &recovery_seed,
    )
    .expect("merge Eq GuardBundle history");
    let ep_guard_merge = fold_kagemusha_ep_accumulators_v1(
        ep_params,
        &ep_base_history,
        ep_guard_history.successor(),
        &recovery_seed,
    )
    .expect("merge Ep GuardBundle history");
    StateHistoryFolds {
        eq_parent_fold,
        ep_parent_fold,
        eq_guard_history_fold: eq_guard_history.proof().clone(),
        ep_guard_history_fold: ep_guard_history.proof().clone(),
        eq_guard_merge_fold: eq_guard_merge.proof().clone(),
        ep_guard_merge_fold: ep_guard_merge.proof().clone(),
        eq_successor_history: eq_guard_merge.successor().clone(),
        ep_successor_history: ep_guard_merge.successor().clone(),
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
    eq_transport_capacity: KagemushaTransportDeciderCapacityProfileV1,
    ep_transport_capacity: KagemushaTransportDeciderCapacityProfileV1,
}

fn decode_state_keys(
    eq_params: &ParamsIPA<EqAffine>,
    ep_params: &ParamsIPA<EpAffine>,
    state: &KagemushaStateV1,
    generated: super::KagemushaGeneratedRecursiveStateArtifactsV1,
) -> StateKeys {
    let eq_transport_capacity = generated.eq_transport_capacity.clone();
    let ep_transport_capacity = generated.ep_transport_capacity.clone();
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
    let eq_transport_proving_key = ProvingKey::read::<_, KagemushaTransportDeciderEqCircuitV1>(
        &mut eq_transport_pk_cursor,
        SerdeFormat::Processed,
        generated.eq_circuit_params.clone(),
    )
    .expect("decode generated Eq transport PK");
    assert_eq!(
        usize::try_from(eq_transport_pk_cursor.position()).expect("Eq transport PK cursor"),
        generated.eq.proving_key.len()
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
    let ep_transport_proving_key = ProvingKey::read::<_, KagemushaTransportDeciderEpCircuitV1>(
        &mut ep_transport_pk_cursor,
        SerdeFormat::Processed,
        generated.ep_circuit_params.clone(),
    )
    .expect("decode generated Ep transport PK");
    assert_eq!(
        usize::try_from(ep_transport_pk_cursor.position()).expect("Ep transport PK cursor"),
        generated.ep.proving_key.len()
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
    let eq_proving_key = ProvingKey::read::<_, KagemushaRecursiveStateEqCircuitV1>(
        &mut eq_pk_cursor,
        SerdeFormat::Processed,
        generated.inner_eq_circuit_params.clone(),
    )
    .expect("decode generated Eq state PK");
    assert_eq!(
        usize::try_from(eq_pk_cursor.position()).expect("Eq PK cursor"),
        generated.inner_eq.proving_key.len()
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
    let ep_proving_key = ProvingKey::read::<_, KagemushaRecursiveStateEpCircuitV1>(
        &mut ep_pk_cursor,
        SerdeFormat::Processed,
        generated.inner_ep_circuit_params.clone(),
    )
    .expect("decode generated Ep state PK");
    assert_eq!(
        usize::try_from(ep_pk_cursor.position()).expect("Ep PK cursor"),
        generated.inner_ep.proving_key.len()
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
        eq_transport_capacity,
        ep_transport_capacity,
    }
}

fn paired_state_proof(
    state_relation: &KagemushaStateRelationWitnessV1,
    guard: &GuardProof,
    state_keys: &StateKeys,
    proof: &KagemushaGeneratedRecursiveStateProofV1,
) -> KagemushaPairedProofV1 {
    let paired = proof.proof.clone();
    assert_eq!(paired.eq_protocol_digest, state_keys.eq_protocol_digest);
    assert_eq!(paired.ep_protocol_digest, state_keys.ep_protocol_digest);
    assert_eq!(
        paired.semantic_digest,
        state_relation.transport_semantic_digest
    );
    assert_eq!(paired.guard_eq_credential_audit, guard.eq_credential_audit);
    assert_eq!(paired.guard_ep_credential_audit, guard.ep_credential_audit);
    paired
        .validate_shape_for_semantic_digest(state_relation.transport_semantic_digest)
        .expect("valid constant-size paired state proof");
    assert!(
        norito::encode_canonical(&paired)
            .expect("paired proof encoding")
            .len()
            <= KAGEMUSHA_PAIRED_PROOF_MAX_BYTES_V1
    );
    paired
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

fn assert_altered_private_fold_proof_rejected(
    state_keys: &StateKeys,
    proof: &KagemushaGeneratedRecursiveStateProofV1,
) {
    let recovery_seed = test_only_recovery_seed();
    let eq_history = proof
        .eq_history
        .to_native()
        .expect("decode Eq private history for negative transport witness");
    let ep_history = proof
        .ep_history
        .to_native()
        .expect("decode Ep private history for negative transport witness");
    let eq_fold = fold_kagemusha_eq_accumulators_v1(
        &state_keys.eq.parameters,
        &proof.eq_current_accumulator,
        &proof.eq_history,
        &recovery_seed,
    )
    .expect("construct Eq fold control for negative transport witness");
    let ep_fold = fold_kagemusha_ep_accumulators_v1(
        &state_keys.ep.parameters,
        &proof.ep_current_accumulator,
        &proof.ep_history,
        &recovery_seed,
    )
    .expect("construct Ep fold control for negative transport witness");
    let mut altered_eq_fold = eq_fold.proof().as_bytes().to_vec();
    altered_eq_fold[64..96].fill(0xff);
    let Err(error) = build_kagemusha_transport_decider_pair_v1(
        &state_keys.eq.parameters,
        &state_keys.ep.parameters,
        KagemushaTransportDeciderWitnessV1 {
            eq: KagemushaTransportDeciderParityWitnessV1 {
                inner_protocol: &state_keys.eq_protocol,
                inner_instances: &proof.eq_public_instances,
                inner_proof: &proof.eq_inner_proof,
                inner_history: &eq_history,
                inner_history_fold_proof: &altered_eq_fold,
                outer_instances: &proof.eq_transport_public_instances,
            },
            ep: KagemushaTransportDeciderParityWitnessV1 {
                inner_protocol: &state_keys.ep_protocol,
                inner_instances: &proof.ep_public_instances,
                inner_proof: &proof.ep_inner_proof,
                inner_history: &ep_history,
                inner_history_fold_proof: ep_fold.proof().as_bytes(),
                outer_instances: &proof.ep_transport_public_instances,
            },
        },
    ) else {
        panic!("the outer circuit builder accepted an altered Eq history-fold proof");
    };
    assert!(
        error.contains("failed to fold current carrier into history"),
        "unexpected altered-fold rejection: {error}",
    );

    let mut altered_ep_fold = ep_fold.proof().as_bytes().to_vec();
    altered_ep_fold[64..96].fill(0xff);
    let Err(error) = build_kagemusha_transport_decider_pair_v1(
        &state_keys.eq.parameters,
        &state_keys.ep.parameters,
        KagemushaTransportDeciderWitnessV1 {
            eq: KagemushaTransportDeciderParityWitnessV1 {
                inner_protocol: &state_keys.eq_protocol,
                inner_instances: &proof.eq_public_instances,
                inner_proof: &proof.eq_inner_proof,
                inner_history: &eq_history,
                inner_history_fold_proof: eq_fold.proof().as_bytes(),
                outer_instances: &proof.eq_transport_public_instances,
            },
            ep: KagemushaTransportDeciderParityWitnessV1 {
                inner_protocol: &state_keys.ep_protocol,
                inner_instances: &proof.ep_public_instances,
                inner_proof: &proof.ep_inner_proof,
                inner_history: &ep_history,
                inner_history_fold_proof: &altered_ep_fold,
                outer_instances: &proof.ep_transport_public_instances,
            },
        },
    ) else {
        panic!("the outer circuit builder accepted an altered Ep history-fold proof");
    };
    assert!(
        error.contains("failed to fold current carrier into history"),
        "unexpected altered-fold rejection: {error}",
    );
}

fn assert_mixed_transport_pairs_rejected(
    state_keys: &StateKeys,
    first: &KagemushaGeneratedRecursiveStateProofV1,
    second: &KagemushaGeneratedRecursiveStateProofV1,
) {
    assert!(paired_transport_boundary_accepts(
        state_keys,
        &second.proof,
        second.proof.semantic_digest,
        &second.eq_transport_public_instances,
        &second.ep_transport_public_instances,
    ));

    let mut mixed = second.proof.clone();
    mixed.eq_proof.clone_from(&first.proof.eq_proof);
    mixed.eq_history.clone_from(&first.proof.eq_history);
    assert!(
        !paired_transport_boundary_accepts(
            state_keys,
            &mixed,
            second.proof.semantic_digest,
            &second.eq_transport_public_instances,
            &second.ep_transport_public_instances,
        ),
        "the outer boundary must reject an old Eq proof/history mixed with a new Ep half",
    );

    mixed = second.proof.clone();
    mixed.eq_proof.clone_from(&first.proof.eq_proof);
    assert!(
        !paired_transport_boundary_accepts(
            state_keys,
            &mixed,
            second.proof.semantic_digest,
            &second.eq_transport_public_instances,
            &second.ep_transport_public_instances,
        ),
        "the outer boundary must reject an old Eq proof paired with current histories",
    );

    mixed = second.proof.clone();
    mixed.ep_proof.clone_from(&first.proof.ep_proof);
    mixed.ep_history.clone_from(&first.proof.ep_history);
    assert!(
        !paired_transport_boundary_accepts(
            state_keys,
            &mixed,
            second.proof.semantic_digest,
            &second.eq_transport_public_instances,
            &second.ep_transport_public_instances,
        ),
        "the outer boundary must reject a new Eq half mixed with an old Ep proof/history",
    );

    mixed = second.proof.clone();
    mixed.ep_proof.clone_from(&first.proof.ep_proof);
    assert!(
        !paired_transport_boundary_accepts(
            state_keys,
            &mixed,
            second.proof.semantic_digest,
            &second.eq_transport_public_instances,
            &second.ep_transport_public_instances,
        ),
        "the outer boundary must reject an old Ep proof paired with current histories",
    );

    mixed = second.proof.clone();
    mixed.eq_history.clone_from(&first.proof.eq_history);
    assert!(
        !paired_transport_boundary_accepts(
            state_keys,
            &mixed,
            second.proof.semantic_digest,
            &second.eq_transport_public_instances,
            &second.ep_transport_public_instances,
        ),
        "the outer boundary must reject a current Eq proof with an old Eq history",
    );

    mixed = second.proof.clone();
    mixed.ep_history.clone_from(&first.proof.ep_history);
    assert!(
        !paired_transport_boundary_accepts(
            state_keys,
            &mixed,
            second.proof.semantic_digest,
            &second.eq_transport_public_instances,
            &second.ep_transport_public_instances,
        ),
        "the outer boundary must reject a current Ep proof with an old Ep history",
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
