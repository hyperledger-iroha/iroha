#![allow(missing_docs, missing_copy_implementations)]

use blake3::Hasher as Blake3Hasher;

#[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
use halo2_proofs::{
    circuit::{Layouter, SimpleFloorPlanner, Value},
    halo2curves::{
        ff::{Field as _, PrimeField as _},
        pasta::{EqAffine as Curve, Fp as Scalar},
    },
    plonk::{
        Circuit, ConstraintSystem, Error as PlonkError, Selector, create_proof, keygen_pk,
        keygen_vk, verify_proof,
    },
    poly::{
        Rotation, VerificationStrategy as _,
        ipa::{
            commitment::IPACommitmentScheme, multiopen::ProverIPA,
            strategy::SingleStrategy as SingleVerifier,
        },
    },
    transcript::{
        Blake2bRead, Blake2bWrite, Challenge255, TranscriptReadBuffer as _,
        TranscriptWriterBuffer as _,
    },
};
#[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
use iroha_crypto::Hash as CryptoHash;
#[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
use iroha_data_model::{
    ChainId,
    confidential::ConfidentialStatus,
    proof::{ProofBox, VerifyingKeyBox, VerifyingKeyRecord},
    zk::{BackendTag, OpenVerifyEnvelope, StarkFriOpenProofV1},
};
#[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
use rand_core_06::OsRng;

pub const CONFIDENTIAL_TRANSFER_V2_CIRCUIT_ID: &str =
    "halo2/pasta/ipa/anon-transfer-2x2-merkle16-poseidon-diversified";
pub const CONFIDENTIAL_UNSHIELD_V2_CIRCUIT_ID: &str =
    "halo2/pasta/ipa/anon-unshield-merkle16-poseidon-diversified";
pub const CONFIDENTIAL_UNSHIELD_V3_CIRCUIT_ID: &str =
    "halo2/pasta/ipa/anon-unshield-2in-1change-merkle16-poseidon-diversified";
pub const CONFIDENTIAL_TRANSFER_V2_IPA_K: u32 = 7;
pub const CONFIDENTIAL_UNSHIELD_V2_IPA_K: u32 = 7;
pub const CONFIDENTIAL_TREE_DEPTH_V2: usize = 16;
pub const CONFIDENTIAL_TREE_CAPACITY_V2: usize = 1 << CONFIDENTIAL_TREE_DEPTH_V2;
pub const CONFIDENTIAL_TRANSFER_V2_PUBLIC_INPUTS_SCHEMA_V1: &[u8] = br#"{"schema":"confidential_transfer_v2","public_inputs":["input_commitment_0","input_commitment_1","nullifier_0","nullifier_1","output_commitment_0","output_commitment_1","root","asset_tag","chain_tag"]}"#;
pub const CONFIDENTIAL_UNSHIELD_V2_PUBLIC_INPUTS_SCHEMA_V1: &[u8] = br#"{"schema":"confidential_unshield_v2","public_inputs":["input_commitment_0","input_commitment_1","nullifier_0","nullifier_1","root","public_amount","asset_tag","chain_tag"]}"#;
pub const CONFIDENTIAL_UNSHIELD_V3_PUBLIC_INPUTS_SCHEMA_V1: &[u8] = br#"{"schema":"confidential_unshield_v3","public_inputs":["input_commitment_0","input_commitment_1","nullifier_0","nullifier_1","change_commitment_0","root","public_amount","asset_tag","chain_tag"]}"#;
const CONFIDENTIAL_V2_MAX_PROOF_BYTES: u32 = 192 * 1024;

#[derive(Debug, Clone)]
pub struct ConfidentialMerklePathV2 {
    pub siblings: Vec<[u8; 32]>,
    pub directions: Vec<u8>,
    pub witness_nodes: Vec<[u8; 32]>,
    pub root: [u8; 32],
}

#[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
#[derive(Debug, Clone)]
pub struct ConfidentialTransferInputV2 {
    pub amount: u128,
    pub rho: [u8; 32],
    pub diversifier: [u8; 32],
    pub leaf_index: usize,
}

#[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
#[derive(Debug, Clone)]
pub struct ConfidentialTransferOutputV2 {
    pub amount: u128,
    pub rho: [u8; 32],
    pub owner_tag: [u8; 32],
}

#[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
#[derive(Debug, Clone)]
pub struct ConfidentialTransferProofV2 {
    pub nullifiers: Vec<[u8; 32]>,
    pub output_commitments: Vec<[u8; 32]>,
    pub root: [u8; 32],
    pub proof: ProofBox,
}

#[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
#[derive(Debug, Clone)]
pub struct ConfidentialUnshieldInputV2 {
    pub amount: u128,
    pub rho: [u8; 32],
    pub diversifier: [u8; 32],
    pub leaf_index: usize,
}

#[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
#[derive(Debug, Clone)]
pub struct ConfidentialUnshieldProofV2 {
    pub nullifiers: Vec<[u8; 32]>,
    pub root: [u8; 32],
    pub proof: ProofBox,
}

#[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
#[derive(Debug, Clone)]
pub struct ConfidentialUnshieldOutputV3 {
    pub amount: u128,
    pub rho: [u8; 32],
}

#[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
#[derive(Debug, Clone)]
pub struct ConfidentialUnshieldProofV3 {
    pub nullifiers: Vec<[u8; 32]>,
    pub output_commitments: Vec<[u8; 32]>,
    pub root: [u8; 32],
    pub proof: ProofBox,
}

pub fn normalize_confidential_circuit_id(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    if let Some(rest) = trimmed.strip_prefix("halo2/pasta/ipa/") {
        return format!("halo2/pasta/ipa/{rest}");
    }
    if let Some(rest) = trimmed.strip_prefix("halo2/pasta/") {
        return format!("halo2/pasta/ipa/{rest}");
    }
    format!("halo2/pasta/ipa/{trimmed}")
}

pub fn is_confidential_transfer_v2_circuit_id(raw: &str) -> bool {
    normalize_confidential_circuit_id(raw) == CONFIDENTIAL_TRANSFER_V2_CIRCUIT_ID
}

pub fn is_confidential_unshield_v2_circuit_id(raw: &str) -> bool {
    normalize_confidential_circuit_id(raw) == CONFIDENTIAL_UNSHIELD_V2_CIRCUIT_ID
}

#[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
fn build_confidential_v2_vk_box<C>(k: u32, circuit: &C) -> Result<VerifyingKeyBox, String>
where
    C: Circuit<Scalar>,
{
    super::ensure_halo2_max_degree(1024);
    let params = super::pasta_params_new(k);
    let vk = keygen_vk(&params, circuit)
        .map_err(|err| format!("failed to generate confidential v2 verifying key: {err}"))?;
    let mut bytes = super::zk1::wrap_start();
    super::zk1::wrap_append_ipa_k(&mut bytes, k);
    super::zk1::wrap_append_vk_pasta(&mut bytes, &vk);
    Ok(VerifyingKeyBox::new(
        super::ZK_BACKEND_HALO2_IPA.to_owned(),
        bytes,
    ))
}

#[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
pub fn confidential_transfer_v2_vk_box() -> Result<VerifyingKeyBox, String> {
    build_confidential_v2_vk_box(
        CONFIDENTIAL_TRANSFER_V2_IPA_K,
        &ConfidentialTransferCircuitV2::<CONFIDENTIAL_TREE_DEPTH_V2>::default(),
    )
}

#[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
pub fn confidential_unshield_v2_vk_box() -> Result<VerifyingKeyBox, String> {
    build_confidential_v2_vk_box(
        CONFIDENTIAL_UNSHIELD_V2_IPA_K,
        &ConfidentialUnshieldCircuitV2::<CONFIDENTIAL_TREE_DEPTH_V2>::default(),
    )
}

#[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
fn confidential_v2_vk_record(
    name: &str,
    version: u32,
    circuit_id: &str,
    public_inputs_schema: &[u8],
    vk_box: VerifyingKeyBox,
) -> Result<VerifyingKeyRecord, String> {
    let mut record = VerifyingKeyRecord::new(
        version,
        circuit_id,
        BackendTag::Halo2IpaPasta,
        "pallas",
        CryptoHash::new(public_inputs_schema).into(),
        super::hash_vk(&vk_box),
    );
    record.vk_len = u32::try_from(vk_box.bytes.len())
        .map_err(|_| "confidential v2 verifying key length overflowed u32".to_owned())?;
    record.max_proof_bytes = CONFIDENTIAL_V2_MAX_PROOF_BYTES;
    record.gas_schedule_id = Some("halo2_default".to_owned());
    record.key = Some(vk_box);
    record.status = ConfidentialStatus::Active;
    record.namespace = name.to_owned();
    Ok(record)
}

#[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
pub fn confidential_transfer_v2_vk_record(
    name: &str,
    version: u32,
) -> Result<VerifyingKeyRecord, String> {
    confidential_v2_vk_record(
        name,
        version,
        CONFIDENTIAL_TRANSFER_V2_CIRCUIT_ID,
        CONFIDENTIAL_TRANSFER_V2_PUBLIC_INPUTS_SCHEMA_V1,
        confidential_transfer_v2_vk_box()?,
    )
}

#[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
pub fn confidential_unshield_v2_vk_record(
    name: &str,
    version: u32,
) -> Result<VerifyingKeyRecord, String> {
    confidential_v2_vk_record(
        name,
        version,
        CONFIDENTIAL_UNSHIELD_V2_CIRCUIT_ID,
        CONFIDENTIAL_UNSHIELD_V2_PUBLIC_INPUTS_SCHEMA_V1,
        confidential_unshield_v2_vk_box()?,
    )
}

pub fn is_confidential_unshield_v3_circuit_id(raw: &str) -> bool {
    normalize_confidential_circuit_id(raw) == CONFIDENTIAL_UNSHIELD_V3_CIRCUIT_ID
}

pub fn parse_transfer_public_inputs(
    proof_bytes: &[u8],
) -> Result<
    (
        [[u8; 32]; 2],
        [[u8; 32]; 2],
        [[u8; 32]; 2],
        [u8; 32],
        [u8; 32],
        [u8; 32],
    ),
    String,
> {
    let columns = extract_confidential_public_columns(proof_bytes)
        .ok_or_else(|| "failed to decode transfer proof public inputs".to_owned())?;
    if columns.len() < 9 || columns.iter().take(9).any(|column| column.len() != 1) {
        return Err("transfer proof must expose 9 single-row instance columns".to_owned());
    }
    Ok((
        [columns[0][0], columns[1][0]],
        [columns[2][0], columns[3][0]],
        [columns[4][0], columns[5][0]],
        columns[6][0],
        columns[7][0],
        columns[8][0],
    ))
}

pub fn parse_unshield_public_inputs(
    proof_bytes: &[u8],
) -> Result<
    (
        [[u8; 32]; 2],
        [[u8; 32]; 2],
        [u8; 32],
        [u8; 32],
        [u8; 32],
        [u8; 32],
    ),
    String,
> {
    let columns = extract_confidential_public_columns(proof_bytes)
        .ok_or_else(|| "failed to decode unshield proof public inputs".to_owned())?;
    if columns.len() < 8 || columns.iter().take(8).any(|column| column.len() != 1) {
        return Err("unshield proof must expose 8 single-row instance columns".to_owned());
    }
    Ok((
        [columns[0][0], columns[1][0]],
        [columns[2][0], columns[3][0]],
        columns[4][0],
        columns[5][0],
        columns[6][0],
        columns[7][0],
    ))
}

pub fn parse_unshield_public_inputs_v3(
    proof_bytes: &[u8],
) -> Result<
    (
        [[u8; 32]; 2],
        [[u8; 32]; 2],
        [u8; 32],
        [u8; 32],
        [u8; 32],
        [u8; 32],
        [u8; 32],
    ),
    String,
> {
    let columns = extract_confidential_public_columns(proof_bytes)
        .ok_or_else(|| "failed to decode unshield proof public inputs".to_owned())?;
    if columns.len() < 9 || columns.iter().take(9).any(|column| column.len() != 1) {
        return Err("unshield proof must expose 9 single-row instance columns".to_owned());
    }
    Ok((
        [columns[0][0], columns[1][0]],
        [columns[2][0], columns[3][0]],
        columns[4][0],
        columns[5][0],
        columns[6][0],
        columns[7][0],
        columns[8][0],
    ))
}

fn extract_confidential_public_columns(proof_bytes: &[u8]) -> Option<Vec<Vec<[u8; 32]>>> {
    if let Ok(envelope) = norito::decode_from_bytes::<OpenVerifyEnvelope>(proof_bytes) {
        return match envelope.backend {
            BackendTag::Halo2IpaPasta => {
                super::extract_pasta_instance_columns_bytes(&envelope.proof_bytes)
            }
            BackendTag::Stark => {
                norito::decode_from_bytes::<StarkFriOpenProofV1>(&envelope.proof_bytes)
                    .ok()
                    .map(|proof| proof.public_inputs)
            }
            _ => None,
        };
    }
    super::extract_pasta_instance_columns_bytes(proof_bytes)
}

#[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
fn scalar_from_repr(bytes: [u8; 32]) -> Option<Scalar> {
    let mut repr = <Scalar as halo2_proofs::halo2curves::ff::PrimeField>::Repr::default();
    repr.as_mut().copy_from_slice(&bytes);
    Option::from(Scalar::from_repr(repr))
}

#[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
fn scalar_to_repr_bytes(value: Scalar) -> [u8; 32] {
    let mut out = [0u8; 32];
    out.copy_from_slice(value.to_repr().as_ref());
    out
}

#[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
fn hash_to_scalar(label: &[u8], parts: &[&[u8]]) -> Scalar {
    let mut counter = 0u64;
    loop {
        let mut hasher = Blake3Hasher::new();
        hasher.update(label);
        hasher.update(&counter.to_le_bytes());
        for part in parts {
            hasher.update(&u64::try_from(part.len()).unwrap_or(u64::MAX).to_le_bytes());
            hasher.update(part);
        }
        let digest = hasher.finalize();
        let mut candidate = [0u8; 32];
        candidate.copy_from_slice(digest.as_bytes());
        if let Some(value) = scalar_from_repr(candidate) {
            return value;
        }
        counter = counter.wrapping_add(1);
    }
}

#[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
fn scalar_from_u128(amount: u128) -> Scalar {
    let mut repr = <Scalar as halo2_proofs::halo2curves::ff::PrimeField>::Repr::default();
    repr.as_mut()[..16].copy_from_slice(&amount.to_le_bytes());
    Scalar::from_repr(repr).expect("u128 always fits inside Pasta Fp")
}

#[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
fn poseidon_pair(lhs: Scalar, rhs: Scalar) -> Scalar {
    let lhs = lhs + Scalar::from(7u64);
    let rhs = rhs + Scalar::from(13u64);
    let lhs_sq = lhs * lhs;
    let lhs_fourth = lhs_sq * lhs_sq;
    let rhs_sq = rhs * rhs;
    let rhs_fourth = rhs_sq * rhs_sq;
    Scalar::from(2u64) * (lhs_fourth * lhs) + Scalar::from(3u64) * (rhs_fourth * rhs)
}

#[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
fn note_commitment_scalar(
    amount: Scalar,
    rho: Scalar,
    owner_tag: Scalar,
    asset_tag: Scalar,
) -> Scalar {
    poseidon_pair(
        amount,
        poseidon_pair(rho, poseidon_pair(owner_tag, asset_tag)),
    )
}

#[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
fn nullifier_scalar(sk: Scalar, rho: Scalar, asset_tag: Scalar, chain_tag: Scalar) -> Scalar {
    poseidon_pair(sk, poseidon_pair(rho, poseidon_pair(asset_tag, chain_tag)))
}

#[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
fn leaf_scalar_from_commitment(commitment: [u8; 32]) -> Scalar {
    scalar_from_repr(commitment)
        .unwrap_or_else(|| hash_to_scalar(b"iroha.confidential.v2.legacy_leaf", &[&commitment]))
}

#[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
pub fn derive_confidential_owner_tag_v2(spend_key: &[u8]) -> [u8; 32] {
    derive_confidential_owner_tag_v2_with_diversifier(
        spend_key,
        default_confidential_diversifier_v2(),
    )
    .expect("default confidential diversifier is canonical")
}

#[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
pub fn default_confidential_diversifier_v2() -> [u8; 32] {
    scalar_to_repr_bytes(Scalar::ONE)
}

#[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
pub fn derive_confidential_diversifier_v2(seed: &[u8]) -> [u8; 32] {
    scalar_to_repr_bytes(hash_to_scalar(
        b"iroha.confidential.v2.diversifier",
        &[seed],
    ))
}

#[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
pub fn derive_confidential_owner_tag_v2_with_diversifier(
    spend_key: &[u8],
    diversifier: [u8; 32],
) -> Result<[u8; 32], String> {
    let spend_scalar = hash_to_scalar(b"iroha.confidential.v2.spend_scalar", &[spend_key]);
    let diversifier_scalar = scalar_from_repr(diversifier)
        .ok_or_else(|| "diversifier must be a canonical Pasta scalar".to_owned())?;
    Ok(scalar_to_repr_bytes(poseidon_pair(
        spend_scalar,
        diversifier_scalar,
    )))
}

#[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
pub fn derive_confidential_asset_tag_v2(asset_definition_id: &str) -> [u8; 32] {
    scalar_to_repr_bytes(hash_to_scalar(
        b"iroha.confidential.v2.asset_tag",
        &[asset_definition_id.trim().as_bytes()],
    ))
}

#[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
pub fn derive_confidential_chain_tag_v2(chain_id: &str) -> [u8; 32] {
    scalar_to_repr_bytes(hash_to_scalar(
        b"iroha.confidential.v2.chain_tag",
        &[chain_id.trim().as_bytes()],
    ))
}

#[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
pub fn derive_confidential_note_v2(
    asset_definition_id: &str,
    amount: u128,
    rho: [u8; 32],
    owner_tag: [u8; 32],
) -> Result<[u8; 32], String> {
    let owner_tag_scalar = scalar_from_repr(owner_tag)
        .ok_or_else(|| "owner_tag must be a canonical Pasta scalar".to_owned())?;
    let asset_tag_scalar =
        scalar_from_repr(derive_confidential_asset_tag_v2(asset_definition_id)).expect("asset tag");
    let rho_scalar = hash_to_scalar(b"iroha.confidential.v2.note_rho", &[&rho]);
    Ok(scalar_to_repr_bytes(note_commitment_scalar(
        scalar_from_u128(amount),
        rho_scalar,
        owner_tag_scalar,
        asset_tag_scalar,
    )))
}

#[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
pub fn derive_confidential_nullifier_v2(
    chain_id: &str,
    asset_definition_id: &str,
    spend_key: &[u8],
    rho: [u8; 32],
) -> [u8; 32] {
    let spend_scalar = hash_to_scalar(b"iroha.confidential.v2.spend_scalar", &[spend_key]);
    let asset_tag_scalar =
        scalar_from_repr(derive_confidential_asset_tag_v2(asset_definition_id)).expect("asset tag");
    let chain_tag_scalar =
        scalar_from_repr(derive_confidential_chain_tag_v2(chain_id)).expect("chain tag");
    let rho_scalar = hash_to_scalar(b"iroha.confidential.v2.note_rho", &[&rho]);
    scalar_to_repr_bytes(nullifier_scalar(
        spend_scalar,
        rho_scalar,
        asset_tag_scalar,
        chain_tag_scalar,
    ))
}

#[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
pub fn encode_confidential_amount_v2(amount: u128) -> [u8; 32] {
    scalar_to_repr_bytes(scalar_from_u128(amount))
}

#[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
pub fn poseidon_empty_root_v2() -> [u8; 32] {
    let mut node = Scalar::ZERO;
    for _ in 0..CONFIDENTIAL_TREE_DEPTH_V2 {
        node = poseidon_pair(node, node);
    }
    scalar_to_repr_bytes(node)
}

#[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
fn build_padded_leaf_layer(commitments: &[[u8; 32]]) -> Result<Vec<Scalar>, String> {
    if commitments.len() > CONFIDENTIAL_TREE_CAPACITY_V2 {
        return Err(format!(
            "confidential v2 tree supports at most {} leaves",
            CONFIDENTIAL_TREE_CAPACITY_V2
        ));
    }
    let mut layer = Vec::with_capacity(CONFIDENTIAL_TREE_CAPACITY_V2);
    for commitment in commitments {
        layer.push(leaf_scalar_from_commitment(*commitment));
    }
    while layer.len() < CONFIDENTIAL_TREE_CAPACITY_V2 {
        layer.push(Scalar::ZERO);
    }
    Ok(layer)
}

#[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
pub fn compute_confidential_root_v2(commitments: &[[u8; 32]]) -> Result<[u8; 32], String> {
    let mut layer = build_padded_leaf_layer(commitments)?;
    for _ in 0..CONFIDENTIAL_TREE_DEPTH_V2 {
        layer = layer
            .chunks_exact(2)
            .map(|pair| poseidon_pair(pair[0], pair[1]))
            .collect();
    }
    Ok(scalar_to_repr_bytes(layer[0]))
}

#[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
pub fn compute_confidential_merkle_path_v2(
    commitments: &[[u8; 32]],
    leaf_index: usize,
) -> Result<ConfidentialMerklePathV2, String> {
    if leaf_index >= CONFIDENTIAL_TREE_CAPACITY_V2 {
        return Err(format!(
            "leaf_index must be < {} for confidential v2 proofs",
            CONFIDENTIAL_TREE_CAPACITY_V2
        ));
    }
    let mut current_index = leaf_index;
    let mut layer = build_padded_leaf_layer(commitments)?;
    let mut siblings = Vec::with_capacity(CONFIDENTIAL_TREE_DEPTH_V2);
    let mut directions = Vec::with_capacity(CONFIDENTIAL_TREE_DEPTH_V2);
    let mut witness_nodes = Vec::with_capacity(CONFIDENTIAL_TREE_DEPTH_V2);
    for _ in 0..CONFIDENTIAL_TREE_DEPTH_V2 {
        let sibling_index = if current_index.is_multiple_of(2) {
            current_index + 1
        } else {
            current_index - 1
        };
        let direction = if current_index.is_multiple_of(2) {
            0
        } else {
            1
        };
        let lhs = if direction == 0 {
            layer[current_index]
        } else {
            layer[sibling_index]
        };
        let rhs = if direction == 0 {
            layer[sibling_index]
        } else {
            layer[current_index]
        };
        siblings.push(scalar_to_repr_bytes(layer[sibling_index]));
        directions.push(direction);
        witness_nodes.push(scalar_to_repr_bytes(poseidon_pair(lhs, rhs)));
        current_index /= 2;
        layer = layer
            .chunks_exact(2)
            .map(|pair| poseidon_pair(pair[0], pair[1]))
            .collect();
    }
    Ok(ConfidentialMerklePathV2 {
        siblings,
        directions,
        witness_nodes,
        root: scalar_to_repr_bytes(layer[0]),
    })
}

#[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
#[derive(Clone, Debug)]
struct ConfidentialTransferWitnessV2 {
    include_input_1: bool,
    include_output_1: bool,
    input_0_amount: u128,
    input_1_amount: u128,
    output_0_amount: u128,
    output_1_amount: u128,
    input_0_rho: [u8; 32],
    input_1_rho: [u8; 32],
    output_0_rho: [u8; 32],
    output_1_rho: [u8; 32],
    spend_scalar: [u8; 32],
    input_0_diversifier: [u8; 32],
    input_1_diversifier: [u8; 32],
    output_0_owner_tag: [u8; 32],
    output_1_owner_tag: [u8; 32],
    asset_tag: [u8; 32],
    chain_tag: [u8; 32],
    input_0_path: ConfidentialMerklePathV2,
    input_1_path: ConfidentialMerklePathV2,
}

#[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
#[derive(Clone, Default)]
pub(super) struct ConfidentialTransferCircuitV2<const DEPTH: usize> {
    witness: Option<ConfidentialTransferWitnessV2>,
}

#[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
impl<const DEPTH: usize> Circuit<Scalar> for ConfidentialTransferCircuitV2<DEPTH> {
    type Config = (
        halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>, // include_input_1
        halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>, // include_output_1
        halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>, // input_0_amount
        halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>, // input_1_amount
        halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>, // output_0_amount
        halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>, // output_1_amount
        halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>, // input_0_rho
        halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>, // input_1_rho
        halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>, // output_0_rho
        halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>, // output_1_rho
        halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>, // spend_scalar
        halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>, // input_0_diversifier
        halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>, // input_1_diversifier
        halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>, // output_0_owner_tag
        halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>, // output_1_owner_tag
        halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>, // input_0_derived_owner_tag
        halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>, // input_1_derived_owner_tag
        [halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>; 4], // note_owner_asset
        [halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>; 4], // note_rho_owner_asset
        halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>, // nullifier_asset_chain
        halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>, // nullifier_0_rho_asset_chain
        halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>, // nullifier_1_rho_asset_chain
        [halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>; DEPTH],
        [halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>; DEPTH],
        [halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>; DEPTH],
        [halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>; DEPTH],
        [halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>; DEPTH],
        [halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>; DEPTH],
        [halo2_proofs::plonk::Column<halo2_proofs::plonk::Instance>; 9],
        Selector,
    );
    type FloorPlanner = SimpleFloorPlanner;
    type Params = ();

    fn without_witnesses(&self) -> Self {
        Self::default()
    }

    fn configure(meta: &mut ConstraintSystem<Scalar>) -> Self::Config {
        let include_input_1 = meta.advice_column();
        let include_output_1 = meta.advice_column();
        let input_0_amount = meta.advice_column();
        let input_1_amount = meta.advice_column();
        let output_0_amount = meta.advice_column();
        let output_1_amount = meta.advice_column();
        let input_0_rho = meta.advice_column();
        let input_1_rho = meta.advice_column();
        let output_0_rho = meta.advice_column();
        let output_1_rho = meta.advice_column();
        let spend_scalar = meta.advice_column();
        let input_0_diversifier = meta.advice_column();
        let input_1_diversifier = meta.advice_column();
        let output_0_owner_tag = meta.advice_column();
        let output_1_owner_tag = meta.advice_column();
        let input_0_derived_owner_tag = meta.advice_column();
        let input_1_derived_owner_tag = meta.advice_column();
        let note_owner_asset = std::array::from_fn(|_| meta.advice_column());
        let note_rho_owner_asset = std::array::from_fn(|_| meta.advice_column());
        let nullifier_asset_chain = meta.advice_column();
        let nullifier_0_rho_asset_chain = meta.advice_column();
        let nullifier_1_rho_asset_chain = meta.advice_column();
        let input_0_siblings = std::array::from_fn(|_| meta.advice_column());
        let input_0_directions = std::array::from_fn(|_| meta.advice_column());
        let input_0_witness_nodes = std::array::from_fn(|_| meta.advice_column());
        let input_1_siblings = std::array::from_fn(|_| meta.advice_column());
        let input_1_directions = std::array::from_fn(|_| meta.advice_column());
        let input_1_witness_nodes = std::array::from_fn(|_| meta.advice_column());
        let instances = std::array::from_fn(|_| meta.instance_column());
        let selector = meta.selector();
        meta.create_gate("confidential_transfer_v2", |meta| {
            let enabled = meta.query_selector(selector);
            let in1_present = meta.query_advice(include_input_1, Rotation::cur());
            let out1_present = meta.query_advice(include_output_1, Rotation::cur());
            let in0_amt = meta.query_advice(input_0_amount, Rotation::cur());
            let in1_amt = meta.query_advice(input_1_amount, Rotation::cur());
            let out0_amt = meta.query_advice(output_0_amount, Rotation::cur());
            let out1_amt = meta.query_advice(output_1_amount, Rotation::cur());
            let in0_rho = meta.query_advice(input_0_rho, Rotation::cur());
            let in1_rho = meta.query_advice(input_1_rho, Rotation::cur());
            let out0_rho = meta.query_advice(output_0_rho, Rotation::cur());
            let out1_rho = meta.query_advice(output_1_rho, Rotation::cur());
            let sk = meta.query_advice(spend_scalar, Rotation::cur());
            let in0_diversifier = meta.query_advice(input_0_diversifier, Rotation::cur());
            let in1_diversifier = meta.query_advice(input_1_diversifier, Rotation::cur());
            let out0_owner = meta.query_advice(output_0_owner_tag, Rotation::cur());
            let out1_owner = meta.query_advice(output_1_owner_tag, Rotation::cur());
            let in0_owner = meta.query_advice(input_0_derived_owner_tag, Rotation::cur());
            let in1_owner = meta.query_advice(input_1_derived_owner_tag, Rotation::cur());
            let note_owner_asset_exprs = note_owner_asset
                .iter()
                .map(|column| meta.query_advice(*column, Rotation::cur()))
                .collect::<Vec<_>>();
            let note_rho_owner_asset_exprs = note_rho_owner_asset
                .iter()
                .map(|column| meta.query_advice(*column, Rotation::cur()))
                .collect::<Vec<_>>();
            let asset_chain_expr = meta.query_advice(nullifier_asset_chain, Rotation::cur());
            let nf0_rho_asset_chain =
                meta.query_advice(nullifier_0_rho_asset_chain, Rotation::cur());
            let nf1_rho_asset_chain =
                meta.query_advice(nullifier_1_rho_asset_chain, Rotation::cur());
            let cm_in0 = meta.query_instance(instances[0], Rotation::cur());
            let cm_in1 = meta.query_instance(instances[1], Rotation::cur());
            let nf0 = meta.query_instance(instances[2], Rotation::cur());
            let nf1 = meta.query_instance(instances[3], Rotation::cur());
            let cm_out0 = meta.query_instance(instances[4], Rotation::cur());
            let cm_out1 = meta.query_instance(instances[5], Rotation::cur());
            let root = meta.query_instance(instances[6], Rotation::cur());
            let asset_tag = meta.query_instance(instances[7], Rotation::cur());
            let chain_tag = meta.query_instance(instances[8], Rotation::cur());
            let one = halo2_proofs::plonk::Expression::Constant(Scalar::ONE);
            let zero = halo2_proofs::plonk::Expression::Constant(Scalar::ZERO);
            let poseidon_pair_expr =
                |lhs: halo2_proofs::plonk::Expression<Scalar>,
                 rhs: halo2_proofs::plonk::Expression<Scalar>| {
                    let lhs = lhs + halo2_proofs::plonk::Expression::Constant(Scalar::from(7u64));
                    let rhs = rhs + halo2_proofs::plonk::Expression::Constant(Scalar::from(13u64));
                    let lhs_sq = lhs.clone() * lhs.clone();
                    let lhs_fourth = lhs_sq.clone() * lhs_sq;
                    let rhs_sq = rhs.clone() * rhs.clone();
                    let rhs_fourth = rhs_sq.clone() * rhs_sq;
                    halo2_proofs::plonk::Expression::Constant(Scalar::from(2u64))
                        * (lhs_fourth * lhs)
                        + halo2_proofs::plonk::Expression::Constant(Scalar::from(3u64))
                            * (rhs_fourth * rhs)
                };
            let in0_commit_expr =
                poseidon_pair_expr(in0_amt.clone(), note_rho_owner_asset_exprs[0].clone());
            let in1_commit_raw =
                poseidon_pair_expr(in1_amt.clone(), note_rho_owner_asset_exprs[1].clone());
            let out0_commit_expr =
                poseidon_pair_expr(out0_amt.clone(), note_rho_owner_asset_exprs[2].clone());
            let out1_commit_raw =
                poseidon_pair_expr(out1_amt.clone(), note_rho_owner_asset_exprs[3].clone());
            let nf0_expr = poseidon_pair_expr(sk.clone(), nf0_rho_asset_chain.clone());
            let nf1_raw = poseidon_pair_expr(sk.clone(), nf1_rho_asset_chain.clone());
            let mut constraints = vec![
                enabled.clone() * in1_present.clone() * (in1_present.clone() - one.clone()),
                enabled.clone() * out1_present.clone() * (out1_present.clone() - one.clone()),
                enabled.clone()
                    * (in0_amt.clone() + in1_present.clone() * in1_amt.clone()
                        - (out0_amt.clone() + out1_present.clone() * out1_amt.clone())),
                enabled.clone()
                    * (in0_owner.clone() - poseidon_pair_expr(sk.clone(), in0_diversifier)),
                enabled.clone()
                    * (in1_owner.clone() - poseidon_pair_expr(sk.clone(), in1_diversifier)),
                enabled.clone()
                    * (note_owner_asset_exprs[0].clone()
                        - poseidon_pair_expr(in0_owner.clone(), asset_tag.clone())),
                enabled.clone()
                    * (note_owner_asset_exprs[1].clone()
                        - poseidon_pair_expr(in1_owner.clone(), asset_tag.clone())),
                enabled.clone()
                    * (note_owner_asset_exprs[2].clone()
                        - poseidon_pair_expr(out0_owner, asset_tag.clone())),
                enabled.clone()
                    * (note_owner_asset_exprs[3].clone()
                        - poseidon_pair_expr(out1_owner, asset_tag.clone())),
                enabled.clone()
                    * (note_rho_owner_asset_exprs[0].clone()
                        - poseidon_pair_expr(in0_rho.clone(), note_owner_asset_exprs[0].clone())),
                enabled.clone()
                    * (note_rho_owner_asset_exprs[1].clone()
                        - poseidon_pair_expr(in1_rho.clone(), note_owner_asset_exprs[1].clone())),
                enabled.clone()
                    * (note_rho_owner_asset_exprs[2].clone()
                        - poseidon_pair_expr(out0_rho, note_owner_asset_exprs[2].clone())),
                enabled.clone()
                    * (note_rho_owner_asset_exprs[3].clone()
                        - poseidon_pair_expr(out1_rho, note_owner_asset_exprs[3].clone())),
                enabled.clone()
                    * (asset_chain_expr.clone()
                        - poseidon_pair_expr(asset_tag.clone(), chain_tag.clone())),
                enabled.clone()
                    * (nf0_rho_asset_chain.clone()
                        - poseidon_pair_expr(in0_rho.clone(), asset_chain_expr.clone())),
                enabled.clone()
                    * (nf1_rho_asset_chain.clone()
                        - poseidon_pair_expr(in1_rho.clone(), asset_chain_expr)),
                enabled.clone() * (in0_commit_expr.clone() - cm_in0.clone()),
                enabled.clone() * (cm_in1.clone() - in1_present.clone() * in1_commit_raw),
                enabled.clone() * (out0_commit_expr - cm_out0.clone()),
                enabled.clone() * (cm_out1.clone() - out1_present.clone() * out1_commit_raw),
                enabled.clone() * (nf0_expr - nf0.clone()),
                enabled.clone() * (nf1.clone() - in1_present.clone() * nf1_raw),
            ];
            let mut input_0_prev = cm_in0;
            for i in 0..DEPTH {
                let sibling = meta.query_advice(input_0_siblings[i], Rotation::cur());
                let direction = meta.query_advice(input_0_directions[i], Rotation::cur());
                let witness = meta.query_advice(input_0_witness_nodes[i], Rotation::cur());
                constraints
                    .push(enabled.clone() * direction.clone() * (direction.clone() - one.clone()));
                let forward = poseidon_pair_expr(input_0_prev.clone(), sibling.clone());
                let reverse = poseidon_pair_expr(sibling, input_0_prev.clone());
                constraints.push(
                    enabled.clone()
                        * (witness.clone()
                            - ((one.clone() - direction.clone()) * forward + direction * reverse)),
                );
                input_0_prev = witness;
            }
            constraints.push(enabled.clone() * (input_0_prev - root.clone()));
            let mut input_1_prev = cm_in1;
            for i in 0..DEPTH {
                let sibling = meta.query_advice(input_1_siblings[i], Rotation::cur());
                let direction = meta.query_advice(input_1_directions[i], Rotation::cur());
                let witness = meta.query_advice(input_1_witness_nodes[i], Rotation::cur());
                constraints
                    .push(enabled.clone() * direction.clone() * (direction.clone() - one.clone()));
                let forward = poseidon_pair_expr(input_1_prev.clone(), sibling.clone());
                let reverse = poseidon_pair_expr(sibling, input_1_prev.clone());
                constraints.push(
                    enabled.clone()
                        * (witness.clone()
                            - ((one.clone() - direction.clone()) * forward + direction * reverse)),
                );
                input_1_prev = witness;
            }
            constraints.push(enabled * (input_1_prev - root));
            constraints.push(zero);
            constraints
        });
        (
            include_input_1,
            include_output_1,
            input_0_amount,
            input_1_amount,
            output_0_amount,
            output_1_amount,
            input_0_rho,
            input_1_rho,
            output_0_rho,
            output_1_rho,
            spend_scalar,
            input_0_diversifier,
            input_1_diversifier,
            output_0_owner_tag,
            output_1_owner_tag,
            input_0_derived_owner_tag,
            input_1_derived_owner_tag,
            note_owner_asset,
            note_rho_owner_asset,
            nullifier_asset_chain,
            nullifier_0_rho_asset_chain,
            nullifier_1_rho_asset_chain,
            input_0_siblings,
            input_0_directions,
            input_0_witness_nodes,
            input_1_siblings,
            input_1_directions,
            input_1_witness_nodes,
            instances,
            selector,
        )
    }

    #[allow(clippy::too_many_lines)]
    fn synthesize(
        &self,
        cfg: Self::Config,
        mut layouter: impl Layouter<Scalar>,
    ) -> Result<(), PlonkError> {
        let (
            include_input_1,
            include_output_1,
            input_0_amount,
            input_1_amount,
            output_0_amount,
            output_1_amount,
            input_0_rho,
            input_1_rho,
            output_0_rho,
            output_1_rho,
            spend_scalar,
            input_0_diversifier,
            input_1_diversifier,
            output_0_owner_tag,
            output_1_owner_tag,
            input_0_derived_owner_tag,
            input_1_derived_owner_tag,
            note_owner_asset,
            note_rho_owner_asset,
            nullifier_asset_chain,
            nullifier_0_rho_asset_chain,
            nullifier_1_rho_asset_chain,
            input_0_siblings,
            input_0_directions,
            input_0_witness_nodes,
            input_1_siblings,
            input_1_directions,
            input_1_witness_nodes,
            _instances,
            selector,
        ) = cfg;
        let witness = self.witness.clone();
        layouter.assign_region(
            || "confidential_transfer_v2",
            |mut region| {
                selector.enable(&mut region, 0)?;
                let scalar_or_unknown = |value: Option<[u8; 32]>| {
                    value
                        .and_then(scalar_from_repr)
                        .map_or(Value::unknown(), Value::known)
                };
                let amount_or_unknown = |value: Option<u128>| {
                    value
                        .map(scalar_from_u128)
                        .map_or(Value::unknown(), Value::known)
                };
                let bool_or_unknown = |value: Option<bool>| {
                    value
                        .map(|flag| if flag { Scalar::ONE } else { Scalar::ZERO })
                        .map_or(Value::unknown(), Value::known)
                };
                let poseidon_bytes = |lhs: [u8; 32], rhs: [u8; 32]| {
                    let lhs = scalar_from_repr(lhs)?;
                    let rhs = scalar_from_repr(rhs)?;
                    Some(scalar_to_repr_bytes(poseidon_pair(lhs, rhs)))
                };
                let rho_scalar_bytes = |rho: [u8; 32]| {
                    scalar_to_repr_bytes(hash_to_scalar(b"iroha.confidential.v2.note_rho", &[&rho]))
                };
                let derived_owner_tag =
                    |value: &ConfidentialTransferWitnessV2, diversifier: [u8; 32]| {
                        poseidon_bytes(value.spend_scalar, diversifier)
                    };
                let owner_tag_for_note =
                    |value: &ConfidentialTransferWitnessV2, index: usize| match index {
                        0 => derived_owner_tag(value, value.input_0_diversifier),
                        1 => derived_owner_tag(value, value.input_1_diversifier),
                        2 => Some(value.output_0_owner_tag),
                        3 => Some(value.output_1_owner_tag),
                        _ => None,
                    };
                let rho_for_note =
                    |value: &ConfidentialTransferWitnessV2, index: usize| -> Option<[u8; 32]> {
                        match index {
                            0 => Some(value.input_0_rho),
                            1 => Some(value.input_1_rho),
                            2 => Some(value.output_0_rho),
                            3 => Some(value.output_1_rho),
                            _ => None,
                        }
                    };
                let note_owner_asset_value =
                    |value: &ConfidentialTransferWitnessV2, index: usize| {
                        poseidon_bytes(owner_tag_for_note(value, index)?, value.asset_tag)
                    };
                let note_rho_owner_asset_value =
                    |value: &ConfidentialTransferWitnessV2, index: usize| {
                        poseidon_bytes(
                            rho_scalar_bytes(rho_for_note(value, index)?),
                            note_owner_asset_value(value, index)?,
                        )
                    };
                let nullifier_asset_chain_value = |value: &ConfidentialTransferWitnessV2| {
                    poseidon_bytes(value.asset_tag, value.chain_tag)
                };
                super::assign_advice_compat(
                    &mut region,
                    || "include_input_1",
                    include_input_1,
                    0,
                    || bool_or_unknown(witness.as_ref().map(|value| value.include_input_1)),
                )?;
                super::assign_advice_compat(
                    &mut region,
                    || "include_output_1",
                    include_output_1,
                    0,
                    || bool_or_unknown(witness.as_ref().map(|value| value.include_output_1)),
                )?;
                super::assign_advice_compat(
                    &mut region,
                    || "input_0_amount",
                    input_0_amount,
                    0,
                    || amount_or_unknown(witness.as_ref().map(|value| value.input_0_amount)),
                )?;
                super::assign_advice_compat(
                    &mut region,
                    || "input_1_amount",
                    input_1_amount,
                    0,
                    || amount_or_unknown(witness.as_ref().map(|value| value.input_1_amount)),
                )?;
                super::assign_advice_compat(
                    &mut region,
                    || "output_0_amount",
                    output_0_amount,
                    0,
                    || amount_or_unknown(witness.as_ref().map(|value| value.output_0_amount)),
                )?;
                super::assign_advice_compat(
                    &mut region,
                    || "output_1_amount",
                    output_1_amount,
                    0,
                    || amount_or_unknown(witness.as_ref().map(|value| value.output_1_amount)),
                )?;
                super::assign_advice_compat(
                    &mut region,
                    || "input_0_rho",
                    input_0_rho,
                    0,
                    || {
                        scalar_or_unknown(witness.as_ref().map(|value| {
                            scalar_to_repr_bytes(hash_to_scalar(
                                b"iroha.confidential.v2.note_rho",
                                &[&value.input_0_rho],
                            ))
                        }))
                    },
                )?;
                super::assign_advice_compat(
                    &mut region,
                    || "input_1_rho",
                    input_1_rho,
                    0,
                    || {
                        scalar_or_unknown(witness.as_ref().map(|value| {
                            scalar_to_repr_bytes(hash_to_scalar(
                                b"iroha.confidential.v2.note_rho",
                                &[&value.input_1_rho],
                            ))
                        }))
                    },
                )?;
                super::assign_advice_compat(
                    &mut region,
                    || "output_0_rho",
                    output_0_rho,
                    0,
                    || {
                        scalar_or_unknown(witness.as_ref().map(|value| {
                            scalar_to_repr_bytes(hash_to_scalar(
                                b"iroha.confidential.v2.note_rho",
                                &[&value.output_0_rho],
                            ))
                        }))
                    },
                )?;
                super::assign_advice_compat(
                    &mut region,
                    || "output_1_rho",
                    output_1_rho,
                    0,
                    || {
                        scalar_or_unknown(witness.as_ref().map(|value| {
                            scalar_to_repr_bytes(hash_to_scalar(
                                b"iroha.confidential.v2.note_rho",
                                &[&value.output_1_rho],
                            ))
                        }))
                    },
                )?;
                super::assign_advice_compat(
                    &mut region,
                    || "spend_scalar",
                    spend_scalar,
                    0,
                    || scalar_or_unknown(witness.as_ref().map(|value| value.spend_scalar)),
                )?;
                super::assign_advice_compat(
                    &mut region,
                    || "input_0_diversifier",
                    input_0_diversifier,
                    0,
                    || scalar_or_unknown(witness.as_ref().map(|value| value.input_0_diversifier)),
                )?;
                super::assign_advice_compat(
                    &mut region,
                    || "input_1_diversifier",
                    input_1_diversifier,
                    0,
                    || scalar_or_unknown(witness.as_ref().map(|value| value.input_1_diversifier)),
                )?;
                super::assign_advice_compat(
                    &mut region,
                    || "output_0_owner_tag",
                    output_0_owner_tag,
                    0,
                    || scalar_or_unknown(witness.as_ref().map(|value| value.output_0_owner_tag)),
                )?;
                super::assign_advice_compat(
                    &mut region,
                    || "output_1_owner_tag",
                    output_1_owner_tag,
                    0,
                    || scalar_or_unknown(witness.as_ref().map(|value| value.output_1_owner_tag)),
                )?;
                super::assign_advice_compat(
                    &mut region,
                    || "input_0_derived_owner_tag",
                    input_0_derived_owner_tag,
                    0,
                    || {
                        scalar_or_unknown(
                            witness.as_ref().and_then(|value| {
                                derived_owner_tag(value, value.input_0_diversifier)
                            }),
                        )
                    },
                )?;
                super::assign_advice_compat(
                    &mut region,
                    || "input_1_derived_owner_tag",
                    input_1_derived_owner_tag,
                    0,
                    || {
                        scalar_or_unknown(
                            witness.as_ref().and_then(|value| {
                                derived_owner_tag(value, value.input_1_diversifier)
                            }),
                        )
                    },
                )?;
                for note_index in 0..4 {
                    super::assign_advice_compat(
                        &mut region,
                        || format!("note_owner_asset_{note_index}"),
                        note_owner_asset[note_index],
                        0,
                        || {
                            scalar_or_unknown(
                                witness
                                    .as_ref()
                                    .and_then(|value| note_owner_asset_value(value, note_index)),
                            )
                        },
                    )?;
                    super::assign_advice_compat(
                        &mut region,
                        || format!("note_rho_owner_asset_{note_index}"),
                        note_rho_owner_asset[note_index],
                        0,
                        || {
                            scalar_or_unknown(
                                witness.as_ref().and_then(|value| {
                                    note_rho_owner_asset_value(value, note_index)
                                }),
                            )
                        },
                    )?;
                }
                super::assign_advice_compat(
                    &mut region,
                    || "nullifier_asset_chain",
                    nullifier_asset_chain,
                    0,
                    || scalar_or_unknown(witness.as_ref().and_then(nullifier_asset_chain_value)),
                )?;
                super::assign_advice_compat(
                    &mut region,
                    || "nullifier_0_rho_asset_chain",
                    nullifier_0_rho_asset_chain,
                    0,
                    || {
                        scalar_or_unknown(witness.as_ref().and_then(|value| {
                            poseidon_bytes(
                                rho_scalar_bytes(value.input_0_rho),
                                nullifier_asset_chain_value(value)?,
                            )
                        }))
                    },
                )?;
                super::assign_advice_compat(
                    &mut region,
                    || "nullifier_1_rho_asset_chain",
                    nullifier_1_rho_asset_chain,
                    0,
                    || {
                        scalar_or_unknown(witness.as_ref().and_then(|value| {
                            poseidon_bytes(
                                rho_scalar_bytes(value.input_1_rho),
                                nullifier_asset_chain_value(value)?,
                            )
                        }))
                    },
                )?;
                for index in 0..DEPTH {
                    super::assign_advice_compat(
                        &mut region,
                        || format!("input_0_sibling_{index}"),
                        input_0_siblings[index],
                        0,
                        || {
                            scalar_or_unknown(
                                witness.as_ref().and_then(|value| {
                                    value.input_0_path.siblings.get(index).copied()
                                }),
                            )
                        },
                    )?;
                    super::assign_advice_compat(
                        &mut region,
                        || format!("input_0_direction_{index}"),
                        input_0_directions[index],
                        0,
                        || {
                            witness
                                .as_ref()
                                .and_then(|value| value.input_0_path.directions.get(index).copied())
                                .map(|flag| if flag == 0 { Scalar::ZERO } else { Scalar::ONE })
                                .map_or(Value::unknown(), Value::known)
                        },
                    )?;
                    super::assign_advice_compat(
                        &mut region,
                        || format!("input_0_witness_{index}"),
                        input_0_witness_nodes[index],
                        0,
                        || {
                            scalar_or_unknown(witness.as_ref().and_then(|value| {
                                value.input_0_path.witness_nodes.get(index).copied()
                            }))
                        },
                    )?;
                    super::assign_advice_compat(
                        &mut region,
                        || format!("input_1_sibling_{index}"),
                        input_1_siblings[index],
                        0,
                        || {
                            scalar_or_unknown(
                                witness.as_ref().and_then(|value| {
                                    value.input_1_path.siblings.get(index).copied()
                                }),
                            )
                        },
                    )?;
                    super::assign_advice_compat(
                        &mut region,
                        || format!("input_1_direction_{index}"),
                        input_1_directions[index],
                        0,
                        || {
                            witness
                                .as_ref()
                                .and_then(|value| value.input_1_path.directions.get(index).copied())
                                .map(|flag| if flag == 0 { Scalar::ZERO } else { Scalar::ONE })
                                .map_or(Value::unknown(), Value::known)
                        },
                    )?;
                    super::assign_advice_compat(
                        &mut region,
                        || format!("input_1_witness_{index}"),
                        input_1_witness_nodes[index],
                        0,
                        || {
                            scalar_or_unknown(witness.as_ref().and_then(|value| {
                                value.input_1_path.witness_nodes.get(index).copied()
                            }))
                        },
                    )?;
                }
                Ok(())
            },
        )
    }
}

#[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
#[derive(Clone, Debug)]
struct ConfidentialUnshieldWitnessV2 {
    include_input_1: bool,
    input_0_amount: u128,
    input_1_amount: u128,
    input_0_rho: [u8; 32],
    input_1_rho: [u8; 32],
    spend_scalar: [u8; 32],
    input_0_diversifier: [u8; 32],
    input_1_diversifier: [u8; 32],
    input_0_path: ConfidentialMerklePathV2,
    input_1_path: ConfidentialMerklePathV2,
}

#[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
#[derive(Clone, Default)]
pub(super) struct ConfidentialUnshieldCircuitV2<const DEPTH: usize> {
    witness: Option<ConfidentialUnshieldWitnessV2>,
}

#[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
impl<const DEPTH: usize> Circuit<Scalar> for ConfidentialUnshieldCircuitV2<DEPTH> {
    type Config = (
        halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>,
        halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>,
        halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>,
        halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>,
        halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>,
        halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>,
        halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>,
        halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>,
        [halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>; DEPTH],
        [halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>; DEPTH],
        [halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>; DEPTH],
        [halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>; DEPTH],
        [halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>; DEPTH],
        [halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>; DEPTH],
        [halo2_proofs::plonk::Column<halo2_proofs::plonk::Instance>; 8],
        Selector,
    );
    type FloorPlanner = SimpleFloorPlanner;
    type Params = ();

    fn without_witnesses(&self) -> Self {
        Self::default()
    }

    fn configure(meta: &mut ConstraintSystem<Scalar>) -> Self::Config {
        let include_input_1 = meta.advice_column();
        let input_0_amount = meta.advice_column();
        let input_1_amount = meta.advice_column();
        let input_0_rho = meta.advice_column();
        let input_1_rho = meta.advice_column();
        let spend_scalar = meta.advice_column();
        let input_0_diversifier = meta.advice_column();
        let input_1_diversifier = meta.advice_column();
        let input_0_siblings = std::array::from_fn(|_| meta.advice_column());
        let input_0_directions = std::array::from_fn(|_| meta.advice_column());
        let input_0_witness_nodes = std::array::from_fn(|_| meta.advice_column());
        let input_1_siblings = std::array::from_fn(|_| meta.advice_column());
        let input_1_directions = std::array::from_fn(|_| meta.advice_column());
        let input_1_witness_nodes = std::array::from_fn(|_| meta.advice_column());
        let instances = std::array::from_fn(|_| meta.instance_column());
        let selector = meta.selector();
        meta.create_gate("confidential_unshield_v2", |meta| {
            let enabled = meta.query_selector(selector);
            let in1_present = meta.query_advice(include_input_1, Rotation::cur());
            let in0_amt = meta.query_advice(input_0_amount, Rotation::cur());
            let in1_amt = meta.query_advice(input_1_amount, Rotation::cur());
            let in0_rho = meta.query_advice(input_0_rho, Rotation::cur());
            let in1_rho = meta.query_advice(input_1_rho, Rotation::cur());
            let sk = meta.query_advice(spend_scalar, Rotation::cur());
            let in0_diversifier = meta.query_advice(input_0_diversifier, Rotation::cur());
            let in1_diversifier = meta.query_advice(input_1_diversifier, Rotation::cur());
            let cm_in0 = meta.query_instance(instances[0], Rotation::cur());
            let cm_in1 = meta.query_instance(instances[1], Rotation::cur());
            let nf0 = meta.query_instance(instances[2], Rotation::cur());
            let nf1 = meta.query_instance(instances[3], Rotation::cur());
            let root = meta.query_instance(instances[4], Rotation::cur());
            let public_amount = meta.query_instance(instances[5], Rotation::cur());
            let asset_tag = meta.query_instance(instances[6], Rotation::cur());
            let chain_tag = meta.query_instance(instances[7], Rotation::cur());
            let one = halo2_proofs::plonk::Expression::Constant(Scalar::ONE);
            let poseidon_pair_expr =
                |lhs: halo2_proofs::plonk::Expression<Scalar>,
                 rhs: halo2_proofs::plonk::Expression<Scalar>| {
                    let lhs = lhs + halo2_proofs::plonk::Expression::Constant(Scalar::from(7u64));
                    let rhs = rhs + halo2_proofs::plonk::Expression::Constant(Scalar::from(13u64));
                    let lhs_sq = lhs.clone() * lhs.clone();
                    let lhs_fourth = lhs_sq.clone() * lhs_sq;
                    let rhs_sq = rhs.clone() * rhs.clone();
                    let rhs_fourth = rhs_sq.clone() * rhs_sq;
                    halo2_proofs::plonk::Expression::Constant(Scalar::from(2u64))
                        * (lhs_fourth * lhs)
                        + halo2_proofs::plonk::Expression::Constant(Scalar::from(3u64))
                            * (rhs_fourth * rhs)
                };
            let note_commit_expr =
                |amount: halo2_proofs::plonk::Expression<Scalar>,
                 rho: halo2_proofs::plonk::Expression<Scalar>,
                 owner_tag: halo2_proofs::plonk::Expression<Scalar>| {
                    poseidon_pair_expr(
                        amount,
                        poseidon_pair_expr(rho, poseidon_pair_expr(owner_tag, asset_tag.clone())),
                    )
                };
            let nullifier_expr = |rho: halo2_proofs::plonk::Expression<Scalar>| {
                poseidon_pair_expr(
                    sk.clone(),
                    poseidon_pair_expr(
                        rho,
                        poseidon_pair_expr(asset_tag.clone(), chain_tag.clone()),
                    ),
                )
            };
            let input_0_owner_tag = poseidon_pair_expr(sk.clone(), in0_diversifier);
            let input_1_owner_tag = poseidon_pair_expr(sk.clone(), in1_diversifier);
            let in0_commit_expr =
                note_commit_expr(in0_amt.clone(), in0_rho.clone(), input_0_owner_tag);
            let in1_commit_raw =
                note_commit_expr(in1_amt.clone(), in1_rho.clone(), input_1_owner_tag);
            let nf0_expr = nullifier_expr(in0_rho.clone());
            let nf1_raw = nullifier_expr(in1_rho.clone());
            let mut constraints = vec![
                enabled.clone() * in1_present.clone() * (in1_present.clone() - one.clone()),
                enabled.clone()
                    * (in0_amt.clone() + in1_present.clone() * in1_amt.clone()
                        - public_amount.clone()),
                enabled.clone() * (in0_commit_expr.clone() - cm_in0.clone()),
                enabled.clone() * (cm_in1.clone() - in1_present.clone() * in1_commit_raw),
                enabled.clone() * (nf0_expr - nf0.clone()),
                enabled.clone() * (nf1.clone() - in1_present.clone() * nf1_raw),
            ];
            let mut input_0_prev = cm_in0;
            for i in 0..DEPTH {
                let sibling = meta.query_advice(input_0_siblings[i], Rotation::cur());
                let direction = meta.query_advice(input_0_directions[i], Rotation::cur());
                let witness = meta.query_advice(input_0_witness_nodes[i], Rotation::cur());
                constraints
                    .push(enabled.clone() * direction.clone() * (direction.clone() - one.clone()));
                let forward = poseidon_pair_expr(input_0_prev.clone(), sibling.clone());
                let reverse = poseidon_pair_expr(sibling, input_0_prev.clone());
                constraints.push(
                    enabled.clone()
                        * (witness.clone()
                            - ((one.clone() - direction.clone()) * forward + direction * reverse)),
                );
                input_0_prev = witness;
            }
            constraints.push(enabled.clone() * (input_0_prev - root.clone()));
            let mut input_1_prev = cm_in1;
            for i in 0..DEPTH {
                let sibling = meta.query_advice(input_1_siblings[i], Rotation::cur());
                let direction = meta.query_advice(input_1_directions[i], Rotation::cur());
                let witness = meta.query_advice(input_1_witness_nodes[i], Rotation::cur());
                constraints
                    .push(enabled.clone() * direction.clone() * (direction.clone() - one.clone()));
                let forward = poseidon_pair_expr(input_1_prev.clone(), sibling.clone());
                let reverse = poseidon_pair_expr(sibling, input_1_prev.clone());
                constraints.push(
                    enabled.clone()
                        * (witness.clone()
                            - ((one.clone() - direction.clone()) * forward + direction * reverse)),
                );
                input_1_prev = witness;
            }
            constraints.push(enabled * (input_1_prev - root));
            constraints
        });
        (
            include_input_1,
            input_0_amount,
            input_1_amount,
            input_0_rho,
            input_1_rho,
            spend_scalar,
            input_0_diversifier,
            input_1_diversifier,
            input_0_siblings,
            input_0_directions,
            input_0_witness_nodes,
            input_1_siblings,
            input_1_directions,
            input_1_witness_nodes,
            instances,
            selector,
        )
    }

    #[allow(clippy::too_many_lines)]
    fn synthesize(
        &self,
        cfg: Self::Config,
        mut layouter: impl Layouter<Scalar>,
    ) -> Result<(), PlonkError> {
        let (
            include_input_1,
            input_0_amount,
            input_1_amount,
            input_0_rho,
            input_1_rho,
            spend_scalar,
            input_0_diversifier,
            input_1_diversifier,
            input_0_siblings,
            input_0_directions,
            input_0_witness_nodes,
            input_1_siblings,
            input_1_directions,
            input_1_witness_nodes,
            _instances,
            selector,
        ) = cfg;
        let witness = self.witness.clone();
        layouter.assign_region(
            || "confidential_unshield_v2",
            |mut region| {
                selector.enable(&mut region, 0)?;
                let scalar_or_unknown = |value: Option<[u8; 32]>| {
                    value
                        .and_then(scalar_from_repr)
                        .map_or(Value::unknown(), Value::known)
                };
                let amount_or_unknown = |value: Option<u128>| {
                    value
                        .map(scalar_from_u128)
                        .map_or(Value::unknown(), Value::known)
                };
                super::assign_advice_compat(
                    &mut region,
                    || "include_input_1",
                    include_input_1,
                    0,
                    || {
                        witness
                            .as_ref()
                            .map(|value| {
                                if value.include_input_1 {
                                    Scalar::ONE
                                } else {
                                    Scalar::ZERO
                                }
                            })
                            .map_or(Value::unknown(), Value::known)
                    },
                )?;
                super::assign_advice_compat(
                    &mut region,
                    || "input_0_amount",
                    input_0_amount,
                    0,
                    || amount_or_unknown(witness.as_ref().map(|value| value.input_0_amount)),
                )?;
                super::assign_advice_compat(
                    &mut region,
                    || "input_1_amount",
                    input_1_amount,
                    0,
                    || amount_or_unknown(witness.as_ref().map(|value| value.input_1_amount)),
                )?;
                super::assign_advice_compat(
                    &mut region,
                    || "input_0_rho",
                    input_0_rho,
                    0,
                    || {
                        scalar_or_unknown(witness.as_ref().map(|value| {
                            scalar_to_repr_bytes(hash_to_scalar(
                                b"iroha.confidential.v2.note_rho",
                                &[&value.input_0_rho],
                            ))
                        }))
                    },
                )?;
                super::assign_advice_compat(
                    &mut region,
                    || "input_1_rho",
                    input_1_rho,
                    0,
                    || {
                        scalar_or_unknown(witness.as_ref().map(|value| {
                            scalar_to_repr_bytes(hash_to_scalar(
                                b"iroha.confidential.v2.note_rho",
                                &[&value.input_1_rho],
                            ))
                        }))
                    },
                )?;
                super::assign_advice_compat(
                    &mut region,
                    || "spend_scalar",
                    spend_scalar,
                    0,
                    || scalar_or_unknown(witness.as_ref().map(|value| value.spend_scalar)),
                )?;
                super::assign_advice_compat(
                    &mut region,
                    || "input_0_diversifier",
                    input_0_diversifier,
                    0,
                    || scalar_or_unknown(witness.as_ref().map(|value| value.input_0_diversifier)),
                )?;
                super::assign_advice_compat(
                    &mut region,
                    || "input_1_diversifier",
                    input_1_diversifier,
                    0,
                    || scalar_or_unknown(witness.as_ref().map(|value| value.input_1_diversifier)),
                )?;
                for index in 0..DEPTH {
                    super::assign_advice_compat(
                        &mut region,
                        || format!("input_0_sibling_{index}"),
                        input_0_siblings[index],
                        0,
                        || {
                            scalar_or_unknown(
                                witness.as_ref().and_then(|value| {
                                    value.input_0_path.siblings.get(index).copied()
                                }),
                            )
                        },
                    )?;
                    super::assign_advice_compat(
                        &mut region,
                        || format!("input_0_direction_{index}"),
                        input_0_directions[index],
                        0,
                        || {
                            witness
                                .as_ref()
                                .and_then(|value| value.input_0_path.directions.get(index).copied())
                                .map(|flag| if flag == 0 { Scalar::ZERO } else { Scalar::ONE })
                                .map_or(Value::unknown(), Value::known)
                        },
                    )?;
                    super::assign_advice_compat(
                        &mut region,
                        || format!("input_0_witness_{index}"),
                        input_0_witness_nodes[index],
                        0,
                        || {
                            scalar_or_unknown(witness.as_ref().and_then(|value| {
                                value.input_0_path.witness_nodes.get(index).copied()
                            }))
                        },
                    )?;
                    super::assign_advice_compat(
                        &mut region,
                        || format!("input_1_sibling_{index}"),
                        input_1_siblings[index],
                        0,
                        || {
                            scalar_or_unknown(
                                witness.as_ref().and_then(|value| {
                                    value.input_1_path.siblings.get(index).copied()
                                }),
                            )
                        },
                    )?;
                    super::assign_advice_compat(
                        &mut region,
                        || format!("input_1_direction_{index}"),
                        input_1_directions[index],
                        0,
                        || {
                            witness
                                .as_ref()
                                .and_then(|value| value.input_1_path.directions.get(index).copied())
                                .map(|flag| if flag == 0 { Scalar::ZERO } else { Scalar::ONE })
                                .map_or(Value::unknown(), Value::known)
                        },
                    )?;
                    super::assign_advice_compat(
                        &mut region,
                        || format!("input_1_witness_{index}"),
                        input_1_witness_nodes[index],
                        0,
                        || {
                            scalar_or_unknown(witness.as_ref().and_then(|value| {
                                value.input_1_path.witness_nodes.get(index).copied()
                            }))
                        },
                    )?;
                }
                Ok(())
            },
        )
    }
}

#[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
#[derive(Clone, Debug)]
struct ConfidentialUnshieldWitnessV3 {
    include_input_1: bool,
    include_output_0: bool,
    input_0_amount: u128,
    input_1_amount: u128,
    output_0_amount: u128,
    input_0_rho: [u8; 32],
    input_1_rho: [u8; 32],
    output_0_rho: [u8; 32],
    spend_scalar: [u8; 32],
    input_0_diversifier: [u8; 32],
    input_1_diversifier: [u8; 32],
    input_0_path: ConfidentialMerklePathV2,
    input_1_path: ConfidentialMerklePathV2,
}

#[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
#[derive(Clone, Default)]
struct ConfidentialUnshieldCircuitV3<const DEPTH: usize> {
    witness: Option<ConfidentialUnshieldWitnessV3>,
}

#[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
impl<const DEPTH: usize> Circuit<Scalar> for ConfidentialUnshieldCircuitV3<DEPTH> {
    type Config = (
        halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>, // include_input_1
        halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>, // include_output_0
        halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>, // input_0_amount
        halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>, // input_1_amount
        halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>, // output_0_amount
        halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>, // input_0_rho
        halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>, // input_1_rho
        halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>, // output_0_rho
        halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>, // spend_scalar
        halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>, // input_0_diversifier
        halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>, // input_1_diversifier
        [halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>; DEPTH],
        [halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>; DEPTH],
        [halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>; DEPTH],
        [halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>; DEPTH],
        [halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>; DEPTH],
        [halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>; DEPTH],
        [halo2_proofs::plonk::Column<halo2_proofs::plonk::Instance>; 9],
        Selector,
    );
    type FloorPlanner = SimpleFloorPlanner;
    type Params = ();

    fn without_witnesses(&self) -> Self {
        Self::default()
    }

    fn configure(meta: &mut ConstraintSystem<Scalar>) -> Self::Config {
        let include_input_1 = meta.advice_column();
        let include_output_0 = meta.advice_column();
        let input_0_amount = meta.advice_column();
        let input_1_amount = meta.advice_column();
        let output_0_amount = meta.advice_column();
        let input_0_rho = meta.advice_column();
        let input_1_rho = meta.advice_column();
        let output_0_rho = meta.advice_column();
        let spend_scalar = meta.advice_column();
        let input_0_diversifier = meta.advice_column();
        let input_1_diversifier = meta.advice_column();
        let input_0_siblings = std::array::from_fn(|_| meta.advice_column());
        let input_0_directions = std::array::from_fn(|_| meta.advice_column());
        let input_0_witness_nodes = std::array::from_fn(|_| meta.advice_column());
        let input_1_siblings = std::array::from_fn(|_| meta.advice_column());
        let input_1_directions = std::array::from_fn(|_| meta.advice_column());
        let input_1_witness_nodes = std::array::from_fn(|_| meta.advice_column());
        let instances = std::array::from_fn(|_| meta.instance_column());
        let selector = meta.selector();
        meta.create_gate("confidential_unshield_v3", |meta| {
            let enabled = meta.query_selector(selector);
            let in1_present = meta.query_advice(include_input_1, Rotation::cur());
            let out0_present = meta.query_advice(include_output_0, Rotation::cur());
            let in0_amt = meta.query_advice(input_0_amount, Rotation::cur());
            let in1_amt = meta.query_advice(input_1_amount, Rotation::cur());
            let out0_amt = meta.query_advice(output_0_amount, Rotation::cur());
            let in0_rho = meta.query_advice(input_0_rho, Rotation::cur());
            let in1_rho = meta.query_advice(input_1_rho, Rotation::cur());
            let out0_rho = meta.query_advice(output_0_rho, Rotation::cur());
            let sk = meta.query_advice(spend_scalar, Rotation::cur());
            let in0_diversifier = meta.query_advice(input_0_diversifier, Rotation::cur());
            let in1_diversifier = meta.query_advice(input_1_diversifier, Rotation::cur());
            let cm_in0 = meta.query_instance(instances[0], Rotation::cur());
            let cm_in1 = meta.query_instance(instances[1], Rotation::cur());
            let nf0 = meta.query_instance(instances[2], Rotation::cur());
            let nf1 = meta.query_instance(instances[3], Rotation::cur());
            let cm_out0 = meta.query_instance(instances[4], Rotation::cur());
            let root = meta.query_instance(instances[5], Rotation::cur());
            let public_amount = meta.query_instance(instances[6], Rotation::cur());
            let asset_tag = meta.query_instance(instances[7], Rotation::cur());
            let chain_tag = meta.query_instance(instances[8], Rotation::cur());
            let one = halo2_proofs::plonk::Expression::Constant(Scalar::ONE);
            let poseidon_pair_expr =
                |lhs: halo2_proofs::plonk::Expression<Scalar>,
                 rhs: halo2_proofs::plonk::Expression<Scalar>| {
                    let lhs = lhs + halo2_proofs::plonk::Expression::Constant(Scalar::from(7u64));
                    let rhs = rhs + halo2_proofs::plonk::Expression::Constant(Scalar::from(13u64));
                    let lhs_sq = lhs.clone() * lhs.clone();
                    let lhs_fourth = lhs_sq.clone() * lhs_sq;
                    let rhs_sq = rhs.clone() * rhs.clone();
                    let rhs_fourth = rhs_sq.clone() * rhs_sq;
                    halo2_proofs::plonk::Expression::Constant(Scalar::from(2u64))
                        * (lhs_fourth * lhs)
                        + halo2_proofs::plonk::Expression::Constant(Scalar::from(3u64))
                            * (rhs_fourth * rhs)
                };
            let change_owner_tag = poseidon_pair_expr(sk.clone(), one.clone());
            let note_commit_expr =
                |amount: halo2_proofs::plonk::Expression<Scalar>,
                 rho: halo2_proofs::plonk::Expression<Scalar>,
                 owner_tag: halo2_proofs::plonk::Expression<Scalar>| {
                    poseidon_pair_expr(
                        amount,
                        poseidon_pair_expr(rho, poseidon_pair_expr(owner_tag, asset_tag.clone())),
                    )
                };
            let nullifier_expr = |rho: halo2_proofs::plonk::Expression<Scalar>| {
                poseidon_pair_expr(
                    sk.clone(),
                    poseidon_pair_expr(
                        rho,
                        poseidon_pair_expr(asset_tag.clone(), chain_tag.clone()),
                    ),
                )
            };
            let input_0_owner_tag = poseidon_pair_expr(sk.clone(), in0_diversifier);
            let input_1_owner_tag = poseidon_pair_expr(sk.clone(), in1_diversifier);
            let in0_commit_expr =
                note_commit_expr(in0_amt.clone(), in0_rho.clone(), input_0_owner_tag);
            let in1_commit_raw =
                note_commit_expr(in1_amt.clone(), in1_rho.clone(), input_1_owner_tag);
            let out0_commit_expr =
                note_commit_expr(out0_amt.clone(), out0_rho.clone(), change_owner_tag);
            let nf0_expr = nullifier_expr(in0_rho.clone());
            let nf1_raw = nullifier_expr(in1_rho.clone());
            let mut constraints = vec![
                enabled.clone() * in1_present.clone() * (in1_present.clone() - one.clone()),
                enabled.clone() * out0_present.clone() * (out0_present.clone() - one.clone()),
                enabled.clone()
                    * (in0_amt.clone() + in1_present.clone() * in1_amt.clone()
                        - (public_amount.clone() + out0_present.clone() * out0_amt.clone())),
                enabled.clone() * (in0_commit_expr.clone() - cm_in0.clone()),
                enabled.clone() * (cm_in1.clone() - in1_present.clone() * in1_commit_raw),
                enabled.clone() * (cm_out0.clone() - out0_present.clone() * out0_commit_expr),
                enabled.clone() * (nf0_expr - nf0.clone()),
                enabled.clone() * (nf1.clone() - in1_present.clone() * nf1_raw),
            ];
            let mut input_0_prev = cm_in0;
            for i in 0..DEPTH {
                let sibling = meta.query_advice(input_0_siblings[i], Rotation::cur());
                let direction = meta.query_advice(input_0_directions[i], Rotation::cur());
                let witness = meta.query_advice(input_0_witness_nodes[i], Rotation::cur());
                constraints
                    .push(enabled.clone() * direction.clone() * (direction.clone() - one.clone()));
                let forward = poseidon_pair_expr(input_0_prev.clone(), sibling.clone());
                let reverse = poseidon_pair_expr(sibling, input_0_prev.clone());
                constraints.push(
                    enabled.clone()
                        * (witness.clone()
                            - ((one.clone() - direction.clone()) * forward + direction * reverse)),
                );
                input_0_prev = witness;
            }
            constraints.push(enabled.clone() * (input_0_prev - root.clone()));
            let mut input_1_prev = cm_in1;
            for i in 0..DEPTH {
                let sibling = meta.query_advice(input_1_siblings[i], Rotation::cur());
                let direction = meta.query_advice(input_1_directions[i], Rotation::cur());
                let witness = meta.query_advice(input_1_witness_nodes[i], Rotation::cur());
                constraints
                    .push(enabled.clone() * direction.clone() * (direction.clone() - one.clone()));
                let forward = poseidon_pair_expr(input_1_prev.clone(), sibling.clone());
                let reverse = poseidon_pair_expr(sibling, input_1_prev.clone());
                constraints.push(
                    enabled.clone()
                        * (witness.clone()
                            - ((one.clone() - direction.clone()) * forward + direction * reverse)),
                );
                input_1_prev = witness;
            }
            constraints.push(enabled * (input_1_prev - root));
            constraints
        });
        (
            include_input_1,
            include_output_0,
            input_0_amount,
            input_1_amount,
            output_0_amount,
            input_0_rho,
            input_1_rho,
            output_0_rho,
            spend_scalar,
            input_0_diversifier,
            input_1_diversifier,
            input_0_siblings,
            input_0_directions,
            input_0_witness_nodes,
            input_1_siblings,
            input_1_directions,
            input_1_witness_nodes,
            instances,
            selector,
        )
    }

    #[allow(clippy::too_many_lines)]
    fn synthesize(
        &self,
        cfg: Self::Config,
        mut layouter: impl Layouter<Scalar>,
    ) -> Result<(), PlonkError> {
        let (
            include_input_1,
            include_output_0,
            input_0_amount,
            input_1_amount,
            output_0_amount,
            input_0_rho,
            input_1_rho,
            output_0_rho,
            spend_scalar,
            input_0_diversifier,
            input_1_diversifier,
            input_0_siblings,
            input_0_directions,
            input_0_witness_nodes,
            input_1_siblings,
            input_1_directions,
            input_1_witness_nodes,
            _instances,
            selector,
        ) = cfg;
        let witness = self.witness.clone();
        layouter.assign_region(
            || "confidential_unshield_v3",
            |mut region| {
                selector.enable(&mut region, 0)?;
                let scalar_or_unknown = |value: Option<[u8; 32]>| {
                    value
                        .and_then(scalar_from_repr)
                        .map_or(Value::unknown(), Value::known)
                };
                let amount_or_unknown = |value: Option<u128>| {
                    value
                        .map(scalar_from_u128)
                        .map_or(Value::unknown(), Value::known)
                };
                super::assign_advice_compat(
                    &mut region,
                    || "include_input_1",
                    include_input_1,
                    0,
                    || {
                        witness
                            .as_ref()
                            .map(|value| {
                                if value.include_input_1 {
                                    Scalar::ONE
                                } else {
                                    Scalar::ZERO
                                }
                            })
                            .map_or(Value::unknown(), Value::known)
                    },
                )?;
                super::assign_advice_compat(
                    &mut region,
                    || "include_output_0",
                    include_output_0,
                    0,
                    || {
                        witness
                            .as_ref()
                            .map(|value| {
                                if value.include_output_0 {
                                    Scalar::ONE
                                } else {
                                    Scalar::ZERO
                                }
                            })
                            .map_or(Value::unknown(), Value::known)
                    },
                )?;
                super::assign_advice_compat(
                    &mut region,
                    || "input_0_amount",
                    input_0_amount,
                    0,
                    || amount_or_unknown(witness.as_ref().map(|value| value.input_0_amount)),
                )?;
                super::assign_advice_compat(
                    &mut region,
                    || "input_1_amount",
                    input_1_amount,
                    0,
                    || amount_or_unknown(witness.as_ref().map(|value| value.input_1_amount)),
                )?;
                super::assign_advice_compat(
                    &mut region,
                    || "output_0_amount",
                    output_0_amount,
                    0,
                    || amount_or_unknown(witness.as_ref().map(|value| value.output_0_amount)),
                )?;
                super::assign_advice_compat(
                    &mut region,
                    || "input_0_rho",
                    input_0_rho,
                    0,
                    || {
                        scalar_or_unknown(witness.as_ref().map(|value| {
                            scalar_to_repr_bytes(hash_to_scalar(
                                b"iroha.confidential.v2.note_rho",
                                &[&value.input_0_rho],
                            ))
                        }))
                    },
                )?;
                super::assign_advice_compat(
                    &mut region,
                    || "input_1_rho",
                    input_1_rho,
                    0,
                    || {
                        scalar_or_unknown(witness.as_ref().map(|value| {
                            scalar_to_repr_bytes(hash_to_scalar(
                                b"iroha.confidential.v2.note_rho",
                                &[&value.input_1_rho],
                            ))
                        }))
                    },
                )?;
                super::assign_advice_compat(
                    &mut region,
                    || "output_0_rho",
                    output_0_rho,
                    0,
                    || {
                        scalar_or_unknown(witness.as_ref().map(|value| {
                            scalar_to_repr_bytes(hash_to_scalar(
                                b"iroha.confidential.v2.note_rho",
                                &[&value.output_0_rho],
                            ))
                        }))
                    },
                )?;
                super::assign_advice_compat(
                    &mut region,
                    || "spend_scalar",
                    spend_scalar,
                    0,
                    || scalar_or_unknown(witness.as_ref().map(|value| value.spend_scalar)),
                )?;
                super::assign_advice_compat(
                    &mut region,
                    || "input_0_diversifier",
                    input_0_diversifier,
                    0,
                    || scalar_or_unknown(witness.as_ref().map(|value| value.input_0_diversifier)),
                )?;
                super::assign_advice_compat(
                    &mut region,
                    || "input_1_diversifier",
                    input_1_diversifier,
                    0,
                    || scalar_or_unknown(witness.as_ref().map(|value| value.input_1_diversifier)),
                )?;
                for index in 0..DEPTH {
                    super::assign_advice_compat(
                        &mut region,
                        || format!("input_0_sibling_{index}"),
                        input_0_siblings[index],
                        0,
                        || {
                            scalar_or_unknown(
                                witness.as_ref().and_then(|value| {
                                    value.input_0_path.siblings.get(index).copied()
                                }),
                            )
                        },
                    )?;
                    super::assign_advice_compat(
                        &mut region,
                        || format!("input_0_direction_{index}"),
                        input_0_directions[index],
                        0,
                        || {
                            witness
                                .as_ref()
                                .and_then(|value| value.input_0_path.directions.get(index).copied())
                                .map(|flag| if flag == 0 { Scalar::ZERO } else { Scalar::ONE })
                                .map_or(Value::unknown(), Value::known)
                        },
                    )?;
                    super::assign_advice_compat(
                        &mut region,
                        || format!("input_0_witness_{index}"),
                        input_0_witness_nodes[index],
                        0,
                        || {
                            scalar_or_unknown(witness.as_ref().and_then(|value| {
                                value.input_0_path.witness_nodes.get(index).copied()
                            }))
                        },
                    )?;
                    super::assign_advice_compat(
                        &mut region,
                        || format!("input_1_sibling_{index}"),
                        input_1_siblings[index],
                        0,
                        || {
                            scalar_or_unknown(
                                witness.as_ref().and_then(|value| {
                                    value.input_1_path.siblings.get(index).copied()
                                }),
                            )
                        },
                    )?;
                    super::assign_advice_compat(
                        &mut region,
                        || format!("input_1_direction_{index}"),
                        input_1_directions[index],
                        0,
                        || {
                            witness
                                .as_ref()
                                .and_then(|value| value.input_1_path.directions.get(index).copied())
                                .map(|flag| if flag == 0 { Scalar::ZERO } else { Scalar::ONE })
                                .map_or(Value::unknown(), Value::known)
                        },
                    )?;
                    super::assign_advice_compat(
                        &mut region,
                        || format!("input_1_witness_{index}"),
                        input_1_witness_nodes[index],
                        0,
                        || {
                            scalar_or_unknown(witness.as_ref().and_then(|value| {
                                value.input_1_path.witness_nodes.get(index).copied()
                            }))
                        },
                    )?;
                }
                Ok(())
            },
        )
    }
}

#[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
fn parse_vk_for_transfer(
    circuit_id: &str,
    vk_box: &VerifyingKeyBox,
) -> Result<(super::PastaParams, halo2_proofs::plonk::VerifyingKey<Curve>), String> {
    if vk_box.backend.as_str() != super::ZK_BACKEND_HALO2_IPA {
        return Err("confidential v2 proving requires a halo2/ipa verifying key".to_owned());
    }
    if !is_confidential_transfer_v2_circuit_id(circuit_id) {
        return Err(format!(
            "unsupported confidential transfer verifier circuit `{circuit_id}`"
        ));
    }
    let params = super::zkparse::params_any(vk_box.bytes.as_slice())
        .ok_or_else(|| "missing/invalid IPAK parameters in verifying key envelope".to_owned())?;
    let parsed = super::zkparse::vk_from_bytes::<
        ConfidentialTransferCircuitV2<CONFIDENTIAL_TREE_DEPTH_V2>,
    >(vk_box.bytes.as_slice(), &params)
    .ok_or_else(|| {
        "missing/invalid H2VK payload for confidential transfer verifying key".to_owned()
    })?;
    Ok((params, parsed))
}

#[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
fn parse_vk_for_unshield_v2(
    circuit_id: &str,
    vk_box: &VerifyingKeyBox,
) -> Result<(super::PastaParams, halo2_proofs::plonk::VerifyingKey<Curve>), String> {
    if vk_box.backend.as_str() != super::ZK_BACKEND_HALO2_IPA {
        return Err("confidential v2 proving requires a halo2/ipa verifying key".to_owned());
    }
    if !is_confidential_unshield_v2_circuit_id(circuit_id) {
        return Err(format!(
            "unsupported confidential unshield verifier circuit `{circuit_id}`"
        ));
    }
    let params = super::zkparse::params_any(vk_box.bytes.as_slice())
        .ok_or_else(|| "missing/invalid IPAK parameters in verifying key envelope".to_owned())?;
    let parsed = super::zkparse::vk_from_bytes::<
        ConfidentialUnshieldCircuitV2<CONFIDENTIAL_TREE_DEPTH_V2>,
    >(vk_box.bytes.as_slice(), &params)
    .ok_or_else(|| {
        "missing/invalid H2VK payload for confidential unshield verifying key".to_owned()
    })?;
    Ok((params, parsed))
}

#[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
fn parse_vk_for_unshield_v3(
    circuit_id: &str,
    vk_box: &VerifyingKeyBox,
) -> Result<(super::PastaParams, halo2_proofs::plonk::VerifyingKey<Curve>), String> {
    if vk_box.backend.as_str() != super::ZK_BACKEND_HALO2_IPA {
        return Err("confidential v3 proving requires a halo2/ipa verifying key".to_owned());
    }
    if !is_confidential_unshield_v3_circuit_id(circuit_id) {
        return Err(format!(
            "unsupported confidential unshield verifier circuit `{circuit_id}`"
        ));
    }
    let params = super::zkparse::params_any(vk_box.bytes.as_slice())
        .ok_or_else(|| "missing/invalid IPAK parameters in verifying key envelope".to_owned())?;
    let parsed = super::zkparse::vk_from_bytes::<
        ConfidentialUnshieldCircuitV3<CONFIDENTIAL_TREE_DEPTH_V2>,
    >(vk_box.bytes.as_slice(), &params)
    .ok_or_else(|| {
        "missing/invalid H2VK payload for confidential unshield verifying key".to_owned()
    })?;
    Ok((params, parsed))
}

#[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
fn encode_halo2_envelope(
    circuit_id: &str,
    vk_box: &VerifyingKeyBox,
    schema_descriptor: Vec<u8>,
    instance_columns: &[Vec<Scalar>],
    proof_raw: Vec<u8>,
) -> Result<ProofBox, String> {
    let mut proof_payload = super::zk1::wrap_start();
    super::zk1::wrap_append_proof(&mut proof_payload, &proof_raw);
    let instance_refs: Vec<&[Scalar]> = instance_columns.iter().map(Vec::as_slice).collect();
    super::zk1::wrap_append_instances_pasta_fp_cols(instance_refs.as_slice(), &mut proof_payload);
    let envelope = OpenVerifyEnvelope {
        backend: BackendTag::Halo2IpaPasta,
        circuit_id: circuit_id.to_owned(),
        vk_hash: super::hash_vk(vk_box),
        public_inputs: schema_descriptor,
        proof_bytes: proof_payload,
        aux: Vec::new(),
    };
    let encoded = norito::to_bytes(&envelope)
        .map_err(|err| format!("failed to encode confidential proof envelope: {err}"))?;
    Ok(ProofBox::new(
        super::ZK_BACKEND_HALO2_IPA.to_owned(),
        encoded,
    ))
}

#[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
pub fn build_confidential_transfer_proof_v2(
    chain_id: &ChainId,
    asset_definition_id: &str,
    spend_key: &[u8],
    tree_commitments: &[[u8; 32]],
    inputs: &[ConfidentialTransferInputV2],
    outputs: &[ConfidentialTransferOutputV2],
    root_hint: [u8; 32],
    circuit_id: &str,
    vk_box: &VerifyingKeyBox,
) -> Result<ConfidentialTransferProofV2, String> {
    super::ensure_halo2_max_degree(1024);
    if inputs.is_empty() || inputs.len() > 2 {
        return Err("confidential transfer v2 supports one or two inputs".to_owned());
    }
    if outputs.is_empty() || outputs.len() > 2 {
        return Err("confidential transfer v2 supports one or two outputs".to_owned());
    }
    let computed_root = compute_confidential_root_v2(tree_commitments)?;
    if computed_root != root_hint {
        return Err("tree commitments do not match the supplied root_hint".to_owned());
    }
    let (params, parsed_vk) = parse_vk_for_transfer(circuit_id, vk_box)?;
    let spend_scalar = hash_to_scalar(b"iroha.confidential.v2.spend_scalar", &[spend_key]);
    let spend_scalar_bytes = scalar_to_repr_bytes(spend_scalar);
    let asset_tag = derive_confidential_asset_tag_v2(asset_definition_id);
    let chain_tag = derive_confidential_chain_tag_v2(chain_id.as_str());
    let input_0 = inputs
        .first()
        .cloned()
        .ok_or_else(|| "missing transfer input".to_owned())?;
    let input_1 = inputs.get(1).cloned();
    let output_0 = outputs
        .first()
        .cloned()
        .ok_or_else(|| "missing transfer output".to_owned())?;
    let output_1 = outputs.get(1).cloned();
    let input_0_owner_tag =
        derive_confidential_owner_tag_v2_with_diversifier(spend_key, input_0.diversifier)?;
    let input_0_commitment = derive_confidential_note_v2(
        asset_definition_id,
        input_0.amount,
        input_0.rho,
        input_0_owner_tag,
    )?;
    let input_1_commitment = if let Some(note) = input_1.as_ref() {
        let owner_tag =
            derive_confidential_owner_tag_v2_with_diversifier(spend_key, note.diversifier)?;
        derive_confidential_note_v2(asset_definition_id, note.amount, note.rho, owner_tag)?
    } else {
        [0u8; 32]
    };
    if tree_commitments
        .get(input_0.leaf_index)
        .copied()
        .unwrap_or_default()
        != input_0_commitment
    {
        return Err("transfer input 0 does not match the current confidential tree".to_owned());
    }
    if let Some(note) = input_1.as_ref()
        && tree_commitments
            .get(note.leaf_index)
            .copied()
            .unwrap_or_default()
            != input_1_commitment
    {
        return Err("transfer input 1 does not match the current confidential tree".to_owned());
    }
    let input_0_path = compute_confidential_merkle_path_v2(tree_commitments, input_0.leaf_index)?;
    let input_1_path = compute_confidential_merkle_path_v2(
        tree_commitments,
        input_1
            .as_ref()
            .map_or(tree_commitments.len(), |note| note.leaf_index),
    )?;
    if input_0_path.root != root_hint || input_1_path.root != root_hint {
        return Err("computed confidential Merkle path does not match root_hint".to_owned());
    }
    let output_0_commitment = derive_confidential_note_v2(
        asset_definition_id,
        output_0.amount,
        output_0.rho,
        output_0.owner_tag,
    )?;
    let output_1_commitment = if let Some(note) = output_1.as_ref() {
        derive_confidential_note_v2(asset_definition_id, note.amount, note.rho, note.owner_tag)?
    } else {
        [0u8; 32]
    };
    let nullifier_0 = derive_confidential_nullifier_v2(
        chain_id.as_str(),
        asset_definition_id,
        spend_key,
        input_0.rho,
    );
    let nullifier_1 = input_1.as_ref().map_or([0u8; 32], |note| {
        derive_confidential_nullifier_v2(
            chain_id.as_str(),
            asset_definition_id,
            spend_key,
            note.rho,
        )
    });
    let witness = ConfidentialTransferWitnessV2 {
        include_input_1: input_1.is_some(),
        include_output_1: output_1.is_some(),
        input_0_amount: input_0.amount,
        input_1_amount: input_1.as_ref().map_or(0, |note| note.amount),
        output_0_amount: output_0.amount,
        output_1_amount: output_1.as_ref().map_or(0, |note| note.amount),
        input_0_rho: input_0.rho,
        input_1_rho: input_1.as_ref().map_or([0u8; 32], |note| note.rho),
        output_0_rho: output_0.rho,
        output_1_rho: output_1.as_ref().map_or([0u8; 32], |note| note.rho),
        spend_scalar: spend_scalar_bytes,
        input_0_diversifier: input_0.diversifier,
        input_1_diversifier: input_1
            .as_ref()
            .map_or_else(default_confidential_diversifier_v2, |note| note.diversifier),
        output_0_owner_tag: output_0.owner_tag,
        output_1_owner_tag: output_1.as_ref().map_or([0u8; 32], |note| note.owner_tag),
        asset_tag,
        chain_tag,
        input_0_path,
        input_1_path,
    };
    let circuit = ConfidentialTransferCircuitV2::<CONFIDENTIAL_TREE_DEPTH_V2> {
        witness: Some(witness),
    };
    let instance_columns = vec![
        vec![scalar_from_repr(input_0_commitment).expect("v2 commitment scalar")],
        vec![scalar_from_repr(input_1_commitment).unwrap_or(Scalar::ZERO)],
        vec![scalar_from_repr(nullifier_0).expect("nullifier scalar")],
        vec![scalar_from_repr(nullifier_1).unwrap_or(Scalar::ZERO)],
        vec![scalar_from_repr(output_0_commitment).expect("v2 commitment scalar")],
        vec![scalar_from_repr(output_1_commitment).unwrap_or(Scalar::ZERO)],
        vec![
            scalar_from_repr(root_hint)
                .ok_or_else(|| "root_hint must be a canonical Pasta scalar".to_owned())?,
        ],
        vec![scalar_from_repr(asset_tag).expect("asset tag scalar")],
        vec![scalar_from_repr(chain_tag).expect("chain tag scalar")],
    ];
    let instance_refs: Vec<&[Scalar]> = instance_columns.iter().map(Vec::as_slice).collect();
    let instance_wrapper = vec![instance_refs.as_slice()];
    let proving_key = keygen_pk(
        &params,
        parsed_vk.clone(),
        &ConfidentialTransferCircuitV2::<CONFIDENTIAL_TREE_DEPTH_V2>::default(),
    )
    .map_err(|err| format!("failed to derive confidential transfer proving key: {err}"))?;
    let mut transcript = Blake2bWrite::<_, Curve, Challenge255<Curve>>::init(vec![]);
    create_proof::<IPACommitmentScheme<Curve>, ProverIPA<'_, Curve>, Challenge255<Curve>, _, _, _>(
        &params,
        &proving_key,
        &[circuit],
        &instance_wrapper,
        OsRng,
        &mut transcript,
    )
    .map_err(|err| format!("failed to create confidential transfer proof: {err}"))?;
    let proof_raw = transcript.finalize();
    {
        let mut verify_transcript = Blake2bRead::<_, Curve, Challenge255<Curve>>::init(
            std::io::Cursor::new(proof_raw.as_slice()),
        );
        let strategy = SingleVerifier::new(&params);
        let proofs_instances = [&instance_refs[..]];
        verify_proof(
            &params,
            &parsed_vk,
            strategy,
            &proofs_instances,
            &mut verify_transcript,
        )
        .map_err(|err| {
            format!("generated confidential transfer proof failed local self-verification: {err}")
        })?;
    }
    let proof = encode_halo2_envelope(
        CONFIDENTIAL_TRANSFER_V2_CIRCUIT_ID,
        vk_box,
        CONFIDENTIAL_TRANSFER_V2_PUBLIC_INPUTS_SCHEMA_V1.to_vec(),
        &instance_columns,
        proof_raw,
    )?;
    Ok(ConfidentialTransferProofV2 {
        nullifiers: if input_1.is_some() {
            vec![nullifier_0, nullifier_1]
        } else {
            vec![nullifier_0]
        },
        output_commitments: if output_1.is_some() {
            vec![output_0_commitment, output_1_commitment]
        } else {
            vec![output_0_commitment]
        },
        root: root_hint,
        proof,
    })
}

#[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
pub fn build_confidential_unshield_proof_v2(
    chain_id: &ChainId,
    asset_definition_id: &str,
    spend_key: &[u8],
    tree_commitments: &[[u8; 32]],
    inputs: &[ConfidentialUnshieldInputV2],
    public_amount: u128,
    root_hint: [u8; 32],
    circuit_id: &str,
    vk_box: &VerifyingKeyBox,
) -> Result<ConfidentialUnshieldProofV2, String> {
    super::ensure_halo2_max_degree(1024);
    if inputs.is_empty() || inputs.len() > 2 {
        return Err("confidential unshield v2 supports one or two inputs".to_owned());
    }
    let computed_root = compute_confidential_root_v2(tree_commitments)?;
    if computed_root != root_hint {
        return Err("tree commitments do not match the supplied root_hint".to_owned());
    }
    let (params, parsed_vk) = parse_vk_for_unshield_v2(circuit_id, vk_box)?;
    let spend_scalar = hash_to_scalar(b"iroha.confidential.v2.spend_scalar", &[spend_key]);
    let spend_scalar_bytes = scalar_to_repr_bytes(spend_scalar);
    let asset_tag = derive_confidential_asset_tag_v2(asset_definition_id);
    let chain_tag = derive_confidential_chain_tag_v2(chain_id.as_str());
    let input_0 = inputs
        .first()
        .cloned()
        .ok_or_else(|| "missing unshield input".to_owned())?;
    let input_1 = inputs.get(1).cloned();
    let input_0_owner_tag =
        derive_confidential_owner_tag_v2_with_diversifier(spend_key, input_0.diversifier)?;
    let input_0_commitment = derive_confidential_note_v2(
        asset_definition_id,
        input_0.amount,
        input_0.rho,
        input_0_owner_tag,
    )?;
    let input_1_commitment = if let Some(note) = input_1.as_ref() {
        let owner_tag =
            derive_confidential_owner_tag_v2_with_diversifier(spend_key, note.diversifier)?;
        derive_confidential_note_v2(asset_definition_id, note.amount, note.rho, owner_tag)?
    } else {
        [0u8; 32]
    };
    if tree_commitments
        .get(input_0.leaf_index)
        .copied()
        .unwrap_or_default()
        != input_0_commitment
    {
        return Err("unshield input 0 does not match the current confidential tree".to_owned());
    }
    if let Some(note) = input_1.as_ref()
        && tree_commitments
            .get(note.leaf_index)
            .copied()
            .unwrap_or_default()
            != input_1_commitment
    {
        return Err("unshield input 1 does not match the current confidential tree".to_owned());
    }
    let input_0_path = compute_confidential_merkle_path_v2(tree_commitments, input_0.leaf_index)?;
    let input_1_path = compute_confidential_merkle_path_v2(
        tree_commitments,
        input_1
            .as_ref()
            .map_or(tree_commitments.len(), |note| note.leaf_index),
    )?;
    if input_0_path.root != root_hint || input_1_path.root != root_hint {
        return Err("computed confidential Merkle path does not match root_hint".to_owned());
    }
    let nullifier_0 = derive_confidential_nullifier_v2(
        chain_id.as_str(),
        asset_definition_id,
        spend_key,
        input_0.rho,
    );
    let nullifier_1 = input_1.as_ref().map_or([0u8; 32], |note| {
        derive_confidential_nullifier_v2(
            chain_id.as_str(),
            asset_definition_id,
            spend_key,
            note.rho,
        )
    });
    let witness = ConfidentialUnshieldWitnessV2 {
        include_input_1: input_1.is_some(),
        input_0_amount: input_0.amount,
        input_1_amount: input_1.as_ref().map_or(0, |note| note.amount),
        input_0_rho: input_0.rho,
        input_1_rho: input_1.as_ref().map_or([0u8; 32], |note| note.rho),
        spend_scalar: spend_scalar_bytes,
        input_0_diversifier: input_0.diversifier,
        input_1_diversifier: input_1
            .as_ref()
            .map_or_else(default_confidential_diversifier_v2, |note| note.diversifier),
        input_0_path,
        input_1_path,
    };
    let circuit = ConfidentialUnshieldCircuitV2::<CONFIDENTIAL_TREE_DEPTH_V2> {
        witness: Some(witness),
    };
    let instance_columns = vec![
        vec![scalar_from_repr(input_0_commitment).expect("v2 commitment scalar")],
        vec![scalar_from_repr(input_1_commitment).unwrap_or(Scalar::ZERO)],
        vec![scalar_from_repr(nullifier_0).expect("nullifier scalar")],
        vec![scalar_from_repr(nullifier_1).unwrap_or(Scalar::ZERO)],
        vec![
            scalar_from_repr(root_hint)
                .ok_or_else(|| "root_hint must be a canonical Pasta scalar".to_owned())?,
        ],
        vec![scalar_from_u128(public_amount)],
        vec![scalar_from_repr(asset_tag).expect("asset tag scalar")],
        vec![scalar_from_repr(chain_tag).expect("chain tag scalar")],
    ];
    let instance_refs: Vec<&[Scalar]> = instance_columns.iter().map(Vec::as_slice).collect();
    let instance_wrapper = vec![instance_refs.as_slice()];
    let proving_key = keygen_pk(
        &params,
        parsed_vk.clone(),
        &ConfidentialUnshieldCircuitV2::<CONFIDENTIAL_TREE_DEPTH_V2>::default(),
    )
    .map_err(|err| format!("failed to derive confidential unshield proving key: {err}"))?;
    let mut transcript = Blake2bWrite::<_, Curve, Challenge255<Curve>>::init(vec![]);
    create_proof::<IPACommitmentScheme<Curve>, ProverIPA<'_, Curve>, Challenge255<Curve>, _, _, _>(
        &params,
        &proving_key,
        &[circuit],
        &instance_wrapper,
        OsRng,
        &mut transcript,
    )
    .map_err(|err| format!("failed to create confidential unshield proof: {err}"))?;
    let proof = encode_halo2_envelope(
        CONFIDENTIAL_UNSHIELD_V2_CIRCUIT_ID,
        vk_box,
        CONFIDENTIAL_UNSHIELD_V2_PUBLIC_INPUTS_SCHEMA_V1.to_vec(),
        &instance_columns,
        transcript.finalize(),
    )?;
    Ok(ConfidentialUnshieldProofV2 {
        nullifiers: if input_1.is_some() {
            vec![nullifier_0, nullifier_1]
        } else {
            vec![nullifier_0]
        },
        root: root_hint,
        proof,
    })
}

#[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
pub fn build_confidential_unshield_proof_v3(
    chain_id: &ChainId,
    asset_definition_id: &str,
    spend_key: &[u8],
    tree_commitments: &[[u8; 32]],
    inputs: &[ConfidentialUnshieldInputV2],
    outputs: &[ConfidentialUnshieldOutputV3],
    public_amount: u128,
    root_hint: [u8; 32],
    circuit_id: &str,
    vk_box: &VerifyingKeyBox,
) -> Result<ConfidentialUnshieldProofV3, String> {
    super::ensure_halo2_max_degree(1024);
    if inputs.is_empty() || inputs.len() > 2 {
        return Err("confidential unshield v3 supports one or two inputs".to_owned());
    }
    if outputs.len() > 1 {
        return Err(
            "confidential unshield v3 supports at most one private change output".to_owned(),
        );
    }
    let computed_root = compute_confidential_root_v2(tree_commitments)?;
    if computed_root != root_hint {
        return Err("tree commitments do not match the supplied root_hint".to_owned());
    }
    let (params, parsed_vk) = parse_vk_for_unshield_v3(circuit_id, vk_box)?;
    let change_owner_tag = derive_confidential_owner_tag_v2(spend_key);
    let spend_scalar = hash_to_scalar(b"iroha.confidential.v2.spend_scalar", &[spend_key]);
    let spend_scalar_bytes = scalar_to_repr_bytes(spend_scalar);
    let asset_tag = derive_confidential_asset_tag_v2(asset_definition_id);
    let chain_tag = derive_confidential_chain_tag_v2(chain_id.as_str());
    let input_0 = inputs
        .first()
        .cloned()
        .ok_or_else(|| "missing unshield input".to_owned())?;
    let input_1 = inputs.get(1).cloned();
    let output_0 = outputs.first().cloned();
    let input_0_owner_tag =
        derive_confidential_owner_tag_v2_with_diversifier(spend_key, input_0.diversifier)?;
    let input_0_commitment = derive_confidential_note_v2(
        asset_definition_id,
        input_0.amount,
        input_0.rho,
        input_0_owner_tag,
    )?;
    let input_1_commitment = if let Some(note) = input_1.as_ref() {
        let owner_tag =
            derive_confidential_owner_tag_v2_with_diversifier(spend_key, note.diversifier)?;
        derive_confidential_note_v2(asset_definition_id, note.amount, note.rho, owner_tag)?
    } else {
        [0u8; 32]
    };
    if tree_commitments
        .get(input_0.leaf_index)
        .copied()
        .unwrap_or_default()
        != input_0_commitment
    {
        return Err("unshield input 0 does not match the current confidential tree".to_owned());
    }
    if let Some(note) = input_1.as_ref()
        && tree_commitments
            .get(note.leaf_index)
            .copied()
            .unwrap_or_default()
            != input_1_commitment
    {
        return Err("unshield input 1 does not match the current confidential tree".to_owned());
    }
    let input_0_path = compute_confidential_merkle_path_v2(tree_commitments, input_0.leaf_index)?;
    let input_1_path = compute_confidential_merkle_path_v2(
        tree_commitments,
        input_1
            .as_ref()
            .map_or(tree_commitments.len(), |note| note.leaf_index),
    )?;
    if input_0_path.root != root_hint || input_1_path.root != root_hint {
        return Err("computed confidential Merkle path does not match root_hint".to_owned());
    }
    let total_input_amount = input_0.amount + input_1.as_ref().map_or(0, |note| note.amount);
    let expected_change_amount = total_input_amount
        .checked_sub(public_amount)
        .ok_or_else(|| "public amount exceeds the available confidential inputs".to_owned())?;
    let output_0_commitment = if let Some(note) = output_0.as_ref() {
        if note.amount != expected_change_amount {
            return Err("confidential unshield v3 change note amount mismatch".to_owned());
        }
        derive_confidential_note_v2(asset_definition_id, note.amount, note.rho, change_owner_tag)?
    } else if expected_change_amount == 0 {
        [0u8; 32]
    } else {
        return Err("confidential unshield v3 requires a private change output".to_owned());
    };
    let nullifier_0 = derive_confidential_nullifier_v2(
        chain_id.as_str(),
        asset_definition_id,
        spend_key,
        input_0.rho,
    );
    let nullifier_1 = input_1.as_ref().map_or([0u8; 32], |note| {
        derive_confidential_nullifier_v2(
            chain_id.as_str(),
            asset_definition_id,
            spend_key,
            note.rho,
        )
    });
    let witness = ConfidentialUnshieldWitnessV3 {
        include_input_1: input_1.is_some(),
        include_output_0: output_0.is_some(),
        input_0_amount: input_0.amount,
        input_1_amount: input_1.as_ref().map_or(0, |note| note.amount),
        output_0_amount: output_0.as_ref().map_or(0, |note| note.amount),
        input_0_rho: input_0.rho,
        input_1_rho: input_1.as_ref().map_or([0u8; 32], |note| note.rho),
        output_0_rho: output_0.as_ref().map_or([0u8; 32], |note| note.rho),
        spend_scalar: spend_scalar_bytes,
        input_0_diversifier: input_0.diversifier,
        input_1_diversifier: input_1
            .as_ref()
            .map_or_else(default_confidential_diversifier_v2, |note| note.diversifier),
        input_0_path,
        input_1_path,
    };
    let circuit = ConfidentialUnshieldCircuitV3::<CONFIDENTIAL_TREE_DEPTH_V2> {
        witness: Some(witness),
    };
    let instance_columns = vec![
        vec![scalar_from_repr(input_0_commitment).expect("v2 commitment scalar")],
        vec![scalar_from_repr(input_1_commitment).unwrap_or(Scalar::ZERO)],
        vec![scalar_from_repr(nullifier_0).expect("nullifier scalar")],
        vec![scalar_from_repr(nullifier_1).unwrap_or(Scalar::ZERO)],
        vec![scalar_from_repr(output_0_commitment).unwrap_or(Scalar::ZERO)],
        vec![
            scalar_from_repr(root_hint)
                .ok_or_else(|| "root_hint must be a canonical Pasta scalar".to_owned())?,
        ],
        vec![scalar_from_u128(public_amount)],
        vec![scalar_from_repr(asset_tag).expect("asset tag scalar")],
        vec![scalar_from_repr(chain_tag).expect("chain tag scalar")],
    ];
    let instance_refs: Vec<&[Scalar]> = instance_columns.iter().map(Vec::as_slice).collect();
    let instance_wrapper = vec![instance_refs.as_slice()];
    let proving_key = keygen_pk(
        &params,
        parsed_vk.clone(),
        &ConfidentialUnshieldCircuitV3::<CONFIDENTIAL_TREE_DEPTH_V2>::default(),
    )
    .map_err(|err| format!("failed to derive confidential unshield proving key: {err}"))?;
    let mut transcript = Blake2bWrite::<_, Curve, Challenge255<Curve>>::init(vec![]);
    create_proof::<IPACommitmentScheme<Curve>, ProverIPA<'_, Curve>, Challenge255<Curve>, _, _, _>(
        &params,
        &proving_key,
        &[circuit],
        &instance_wrapper,
        OsRng,
        &mut transcript,
    )
    .map_err(|err| format!("failed to create confidential unshield proof: {err}"))?;
    let proof = encode_halo2_envelope(
        CONFIDENTIAL_UNSHIELD_V3_CIRCUIT_ID,
        vk_box,
        CONFIDENTIAL_UNSHIELD_V3_PUBLIC_INPUTS_SCHEMA_V1.to_vec(),
        &instance_columns,
        transcript.finalize(),
    )?;
    Ok(ConfidentialUnshieldProofV3 {
        nullifiers: if input_1.is_some() {
            vec![nullifier_0, nullifier_1]
        } else {
            vec![nullifier_0]
        },
        output_commitments: if output_0.is_some() {
            vec![output_0_commitment]
        } else {
            Vec::new()
        },
        root: root_hint,
        proof,
    })
}

#[cfg(test)]
mod tests {
    #[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
    #[test]
    fn generated_confidential_v2_vk_records_parse_as_matching_circuits() {
        let transfer = super::confidential_transfer_v2_vk_record("vk_transfer", 3)
            .expect("transfer vk record");
        let unshield = super::confidential_unshield_v2_vk_record("vk_unshield", 4)
            .expect("unshield vk record");

        assert_eq!(
            transfer.circuit_id,
            super::CONFIDENTIAL_TRANSFER_V2_CIRCUIT_ID
        );
        assert_eq!(
            unshield.circuit_id,
            super::CONFIDENTIAL_UNSHIELD_V2_CIRCUIT_ID
        );
        assert!(transfer.is_active());
        assert!(unshield.is_active());
        assert!(transfer.max_proof_bytes > 0);
        assert!(unshield.max_proof_bytes > 0);

        let transfer_key = transfer.key.as_ref().expect("transfer key");
        let unshield_key = unshield.key.as_ref().expect("unshield key");
        super::parse_vk_for_transfer(&transfer.circuit_id, transfer_key)
            .expect("transfer key must parse as confidential transfer v2");
        super::parse_vk_for_unshield_v2(&unshield.circuit_id, unshield_key)
            .expect("unshield key must parse as confidential unshield v2");
    }

    #[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
    #[test]
    fn generated_confidential_transfer_v2_one_input_one_output_verifies_against_generated_vk() {
        use halo2_proofs::halo2curves::{ff::Field as _, pasta::Fp};
        use iroha_data_model::ChainId;

        let chain_id: ChainId = "809574f5-fee7-5e69-bfcf-52451e42d50f"
            .parse()
            .expect("valid chain id");
        let asset_definition_id = "xor#universal";
        let spend_key = [0x11_u8; 32];
        let input_rho = [0x22_u8; 32];
        let input_diversifier = super::derive_confidential_diversifier_v2(b"input");
        let input_owner_tag =
            super::derive_confidential_owner_tag_v2_with_diversifier(&spend_key, input_diversifier)
                .expect("input owner tag");
        let input_commitment =
            super::derive_confidential_note_v2(asset_definition_id, 7, input_rho, input_owner_tag)
                .expect("input commitment");
        let tree_commitments = vec![input_commitment];
        let root = super::compute_confidential_root_v2(&tree_commitments).expect("root");

        let recipient_key = [0x33_u8; 32];
        let output_rho = [0x44_u8; 32];
        let output_diversifier = super::derive_confidential_diversifier_v2(b"recipient");
        let output_owner_tag = super::derive_confidential_owner_tag_v2_with_diversifier(
            &recipient_key,
            output_diversifier,
        )
        .expect("output owner tag");
        let transfer_vk =
            super::confidential_transfer_v2_vk_record("vk_transfer", 3).expect("transfer vk");
        let transfer_key = transfer_vk.key.as_ref().expect("inline transfer vk");
        let input_path =
            super::compute_confidential_merkle_path_v2(&tree_commitments, 0).expect("input path");
        let empty_path =
            super::compute_confidential_merkle_path_v2(&tree_commitments, tree_commitments.len())
                .expect("empty input path");
        let output_commitment = super::derive_confidential_note_v2(
            asset_definition_id,
            7,
            output_rho,
            output_owner_tag,
        )
        .expect("output commitment");
        let asset_tag = super::derive_confidential_asset_tag_v2(asset_definition_id);
        let chain_tag = super::derive_confidential_chain_tag_v2(chain_id.as_str());
        let nullifier = super::derive_confidential_nullifier_v2(
            chain_id.as_str(),
            asset_definition_id,
            &spend_key,
            input_rho,
        );
        let witness = super::ConfidentialTransferWitnessV2 {
            include_input_1: false,
            include_output_1: false,
            input_0_amount: 7,
            input_1_amount: 0,
            output_0_amount: 7,
            output_1_amount: 0,
            input_0_rho: input_rho,
            input_1_rho: [0u8; 32],
            output_0_rho: output_rho,
            output_1_rho: [0u8; 32],
            spend_scalar: super::scalar_to_repr_bytes(super::hash_to_scalar(
                b"iroha.confidential.v2.spend_scalar",
                &[&spend_key],
            )),
            input_0_diversifier: input_diversifier,
            input_1_diversifier: super::default_confidential_diversifier_v2(),
            output_0_owner_tag: output_owner_tag,
            output_1_owner_tag: [0u8; 32],
            asset_tag,
            chain_tag,
            input_0_path: input_path,
            input_1_path: empty_path,
        };
        let circuit = super::ConfidentialTransferCircuitV2::<{ super::CONFIDENTIAL_TREE_DEPTH_V2 }> {
            witness: Some(witness),
        };
        let instance_columns = vec![
            vec![super::scalar_from_repr(input_commitment).expect("input commitment")],
            vec![Fp::ZERO],
            vec![super::scalar_from_repr(nullifier).expect("nullifier")],
            vec![Fp::ZERO],
            vec![super::scalar_from_repr(output_commitment).expect("output commitment")],
            vec![Fp::ZERO],
            vec![super::scalar_from_repr(root).expect("root")],
            vec![super::scalar_from_repr(asset_tag).expect("asset tag")],
            vec![super::scalar_from_repr(chain_tag).expect("chain tag")],
        ];
        halo2_proofs::dev::MockProver::run(
            super::CONFIDENTIAL_TRANSFER_V2_IPA_K,
            &circuit,
            instance_columns,
        )
        .expect("mock prover")
        .assert_satisfied();

        let proof = super::build_confidential_transfer_proof_v2(
            &chain_id,
            asset_definition_id,
            &spend_key,
            &tree_commitments,
            &[super::ConfidentialTransferInputV2 {
                amount: 7,
                rho: input_rho,
                diversifier: input_diversifier,
                leaf_index: 0,
            }],
            &[super::ConfidentialTransferOutputV2 {
                amount: 7,
                rho: output_rho,
                owner_tag: output_owner_tag,
            }],
            root,
            &transfer_vk.circuit_id,
            transfer_key,
        )
        .expect("transfer proof");

        assert!(
            crate::zk::verify_backend(
                crate::zk::ZK_BACKEND_HALO2_IPA,
                &proof.proof,
                Some(transfer_key),
            ),
            "generated one-input one-output confidential transfer v2 proof should verify against the generated VK"
        );
    }

    #[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
    #[test]
    fn generated_confidential_transfer_v2_proof_verifies_against_generated_vk() {
        let chain_id = iroha_data_model::ChainId::from("confidential-transfer-v2-test");
        let asset_definition_id = "zcoin#wonderland";
        let spend_key = [0x11_u8; 32];
        let input_0_rho = [0x21_u8; 32];
        let input_1_rho = [0x22_u8; 32];
        let output_0_rho = [0x31_u8; 32];
        let output_1_rho = [0x32_u8; 32];
        let input_0_diversifier = super::default_confidential_diversifier_v2();
        let input_1_diversifier = super::derive_confidential_diversifier_v2(b"input-1");
        let output_0_owner_tag = super::derive_confidential_owner_tag_v2_with_diversifier(
            &spend_key,
            input_0_diversifier,
        )
        .expect("owner tag");
        let recipient_diversifier = super::derive_confidential_diversifier_v2(b"recipient");
        let output_1_owner_tag = super::derive_confidential_owner_tag_v2_with_diversifier(
            &[0x44_u8; 32],
            recipient_diversifier,
        )
        .expect("recipient owner tag");

        let input_0_commitment = super::derive_confidential_note_v2(
            asset_definition_id,
            7,
            input_0_rho,
            output_0_owner_tag,
        )
        .expect("input 0 commitment");
        let input_1_owner_tag = super::derive_confidential_owner_tag_v2_with_diversifier(
            &spend_key,
            input_1_diversifier,
        )
        .expect("input 1 owner tag");
        let input_1_commitment = super::derive_confidential_note_v2(
            asset_definition_id,
            5,
            input_1_rho,
            input_1_owner_tag,
        )
        .expect("input 1 commitment");

        let mut tree_commitments = Vec::new();
        tree_commitments.push(input_0_commitment);
        tree_commitments.push([0x99_u8; 32]);
        tree_commitments.push(input_1_commitment);
        let root_hint =
            super::compute_confidential_root_v2(&tree_commitments).expect("confidential root");

        let vk_record =
            super::confidential_transfer_v2_vk_record("vk_transfer", 3).expect("transfer vk");
        let vk_box = vk_record.key.clone().expect("inline transfer vk");
        let proof = super::build_confidential_transfer_proof_v2(
            &chain_id,
            asset_definition_id,
            &spend_key,
            &tree_commitments,
            &[
                super::ConfidentialTransferInputV2 {
                    amount: 7,
                    rho: input_0_rho,
                    diversifier: input_0_diversifier,
                    leaf_index: 0,
                },
                super::ConfidentialTransferInputV2 {
                    amount: 5,
                    rho: input_1_rho,
                    diversifier: input_1_diversifier,
                    leaf_index: 2,
                },
            ],
            &[
                super::ConfidentialTransferOutputV2 {
                    amount: 8,
                    rho: output_0_rho,
                    owner_tag: output_0_owner_tag,
                },
                super::ConfidentialTransferOutputV2 {
                    amount: 4,
                    rho: output_1_rho,
                    owner_tag: output_1_owner_tag,
                },
            ],
            root_hint,
            &vk_record.circuit_id,
            &vk_box,
        )
        .expect("build transfer proof");

        assert!(
            crate::zk::verify_backend(crate::zk::ZK_BACKEND_HALO2_IPA, &proof.proof, Some(&vk_box)),
            "generated confidential transfer v2 proof should verify against the generated VK"
        );
    }

    #[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
    #[test]
    fn generated_confidential_transfer_v2_one_input_two_outputs_verifies_against_generated_vk() {
        let chain_id = iroha_data_model::ChainId::from("confidential-transfer-v2-one-input-test");
        let asset_definition_id = "zcoin#wonderland";
        let spend_key = [0x61_u8; 32];
        let input_rho = [0x71_u8; 32];
        let recipient_output_rho = [0x81_u8; 32];
        let change_output_rho = [0x82_u8; 32];
        let input_diversifier = super::default_confidential_diversifier_v2();
        let sender_owner_tag =
            super::derive_confidential_owner_tag_v2_with_diversifier(&spend_key, input_diversifier)
                .expect("sender owner tag");
        let recipient_diversifier = super::derive_confidential_diversifier_v2(b"recipient");
        let recipient_owner_tag = super::derive_confidential_owner_tag_v2_with_diversifier(
            &[0x72_u8; 32],
            recipient_diversifier,
        )
        .expect("recipient owner tag");
        let input_commitment =
            super::derive_confidential_note_v2(asset_definition_id, 2, input_rho, sender_owner_tag)
                .expect("input commitment");
        let tree_commitments = vec![input_commitment];
        let root_hint =
            super::compute_confidential_root_v2(&tree_commitments).expect("confidential root");
        let vk_record =
            super::confidential_transfer_v2_vk_record("vk_transfer", 3).expect("transfer vk");
        let vk_box = vk_record.key.clone().expect("inline transfer vk");
        let proof = super::build_confidential_transfer_proof_v2(
            &chain_id,
            asset_definition_id,
            &spend_key,
            &tree_commitments,
            &[super::ConfidentialTransferInputV2 {
                amount: 2,
                rho: input_rho,
                diversifier: input_diversifier,
                leaf_index: 0,
            }],
            &[
                super::ConfidentialTransferOutputV2 {
                    amount: 1,
                    rho: recipient_output_rho,
                    owner_tag: recipient_owner_tag,
                },
                super::ConfidentialTransferOutputV2 {
                    amount: 1,
                    rho: change_output_rho,
                    owner_tag: sender_owner_tag,
                },
            ],
            root_hint,
            &vk_record.circuit_id,
            &vk_box,
        )
        .expect("build transfer proof");

        assert!(
            crate::zk::verify_backend(crate::zk::ZK_BACKEND_HALO2_IPA, &proof.proof, Some(&vk_box)),
            "generated one-input confidential transfer v2 proof should verify against the generated VK"
        );
    }
}
