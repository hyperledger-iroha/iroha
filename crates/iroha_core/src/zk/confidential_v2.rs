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
        create_proof, keygen_pk, Circuit, ConstraintSystem, Error as PlonkError, Selector,
    },
    poly::{
        Rotation,
        ipa::{commitment::IPACommitmentScheme, multiopen::ProverIPA},
    },
    transcript::{Blake2bWrite, Challenge255, TranscriptWriterBuffer as _},
};
#[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
use iroha_data_model::{
    ChainId,
    proof::{ProofBox, VerifyingKeyBox},
    zk::{BackendTag, OpenVerifyEnvelope, StarkFriOpenProofV1},
};
#[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
use rand_core_06::OsRng;

pub const CONFIDENTIAL_TRANSFER_V2_CIRCUIT_ID: &str =
    "halo2/pasta/ipa/anon-transfer-2x2-merkle16-poseidon";
pub const CONFIDENTIAL_UNSHIELD_V2_CIRCUIT_ID: &str =
    "halo2/pasta/ipa/anon-unshield-merkle16-poseidon";
pub const CONFIDENTIAL_TREE_DEPTH_V2: usize = 16;
pub const CONFIDENTIAL_TREE_CAPACITY_V2: usize = 1 << CONFIDENTIAL_TREE_DEPTH_V2;
pub const CONFIDENTIAL_TRANSFER_V2_PUBLIC_INPUTS_SCHEMA_V1: &[u8] = br#"{"schema":"confidential_transfer_v2","public_inputs":["input_commitment_0","input_commitment_1","nullifier_0","nullifier_1","output_commitment_0","output_commitment_1","root","asset_tag","chain_tag"]}"#;
pub const CONFIDENTIAL_UNSHIELD_V2_PUBLIC_INPUTS_SCHEMA_V1: &[u8] = br#"{"schema":"confidential_unshield_v2","public_inputs":["input_commitment_0","input_commitment_1","nullifier_0","nullifier_1","root","public_amount","asset_tag","chain_tag"]}"#;

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
    pub leaf_index: usize,
}

#[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
#[derive(Debug, Clone)]
pub struct ConfidentialUnshieldProofV2 {
    pub nullifiers: Vec<[u8; 32]>,
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

pub fn parse_transfer_public_inputs(
    proof_bytes: &[u8],
) -> Result<([[u8; 32]; 2], [[u8; 32]; 2], [[u8; 32]; 2], [u8; 32], [u8; 32], [u8; 32]), String>
{
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
) -> Result<([[u8; 32]; 2], [[u8; 32]; 2], [u8; 32], [u8; 32], [u8; 32], [u8; 32]), String> {
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

fn extract_confidential_public_columns(proof_bytes: &[u8]) -> Option<Vec<Vec<[u8; 32]>>> {
    if let Ok(envelope) = norito::decode_from_bytes::<OpenVerifyEnvelope>(proof_bytes) {
        return match envelope.backend {
            BackendTag::Halo2IpaPasta => {
                super::extract_pasta_instance_columns_bytes(&envelope.proof_bytes)
            }
            BackendTag::Stark => norito::decode_from_bytes::<StarkFriOpenProofV1>(&envelope.proof_bytes)
                .ok()
                .map(|proof| proof.public_inputs),
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
    poseidon_pair(amount, poseidon_pair(rho, poseidon_pair(owner_tag, asset_tag)))
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
    let spend_scalar = hash_to_scalar(b"iroha.confidential.v2.spend_scalar", &[spend_key]);
    scalar_to_repr_bytes(poseidon_pair(spend_scalar, Scalar::ONE))
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
    let owner_tag_scalar =
        scalar_from_repr(owner_tag).ok_or_else(|| "owner_tag must be a canonical Pasta scalar".to_owned())?;
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
        let direction = if current_index.is_multiple_of(2) { 0 } else { 1 };
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
    output_0_owner_tag: [u8; 32],
    output_1_owner_tag: [u8; 32],
    input_0_path: ConfidentialMerklePathV2,
    input_1_path: ConfidentialMerklePathV2,
}

#[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
#[derive(Clone, Default)]
struct ConfidentialTransferCircuitV2<const DEPTH: usize> {
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
        halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>, // output_0_owner_tag
        halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>, // output_1_owner_tag
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
        let output_0_owner_tag = meta.advice_column();
        let output_1_owner_tag = meta.advice_column();
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
            let out0_owner = meta.query_advice(output_0_owner_tag, Rotation::cur());
            let out1_owner = meta.query_advice(output_1_owner_tag, Rotation::cur());
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
            let note_commit_expr =
                |amount: halo2_proofs::plonk::Expression<Scalar>,
                 rho: halo2_proofs::plonk::Expression<Scalar>,
                 owner_tag: halo2_proofs::plonk::Expression<Scalar>| {
                    poseidon_pair_expr(
                        amount,
                        poseidon_pair_expr(rho, poseidon_pair_expr(owner_tag, asset_tag.clone())),
                    )
                };
            let nullifier_expr =
                |rho: halo2_proofs::plonk::Expression<Scalar>| {
                    poseidon_pair_expr(
                        sk.clone(),
                        poseidon_pair_expr(
                            rho,
                            poseidon_pair_expr(asset_tag.clone(), chain_tag.clone()),
                        ),
                    )
                };
            let owner_tag = poseidon_pair_expr(sk.clone(), one.clone());
            let in0_commit_expr = note_commit_expr(in0_amt.clone(), in0_rho.clone(), owner_tag.clone());
            let in1_commit_raw = note_commit_expr(in1_amt.clone(), in1_rho.clone(), owner_tag);
            let out0_commit_expr = note_commit_expr(out0_amt.clone(), out0_rho.clone(), out0_owner);
            let out1_commit_raw = note_commit_expr(out1_amt.clone(), out1_rho.clone(), out1_owner);
            let nf0_expr = nullifier_expr(in0_rho.clone());
            let nf1_raw = nullifier_expr(in1_rho.clone());
            let mut constraints = vec![
                enabled.clone() * in1_present.clone() * (in1_present.clone() - one.clone()),
                enabled.clone() * out1_present.clone() * (out1_present.clone() - one.clone()),
                enabled.clone()
                    * (in0_amt.clone() + in1_present.clone() * in1_amt.clone()
                        - (out0_amt.clone() + out1_present.clone() * out1_amt.clone())),
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
                constraints.push(
                    enabled.clone() * direction.clone() * (direction.clone() - one.clone()),
                );
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
                constraints.push(
                    enabled.clone() * direction.clone() * (direction.clone() - one.clone()),
                );
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
            output_0_owner_tag,
            output_1_owner_tag,
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
    fn synthesize(&self, cfg: Self::Config, mut layouter: impl Layouter<Scalar>) -> Result<(), PlonkError> {
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
            output_0_owner_tag,
            output_1_owner_tag,
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
                    value.and_then(scalar_from_repr).map_or(Value::unknown(), Value::known)
                };
                let amount_or_unknown = |value: Option<u128>| {
                    value.map(scalar_from_u128).map_or(Value::unknown(), Value::known)
                };
                let bool_or_unknown = |value: Option<bool>| {
                    value.map(|flag| if flag { Scalar::ONE } else { Scalar::ZERO })
                        .map_or(Value::unknown(), Value::known)
                };
                super::assign_advice_compat(&mut region, || "include_input_1", include_input_1, 0, || {
                    bool_or_unknown(witness.as_ref().map(|value| value.include_input_1))
                })?;
                super::assign_advice_compat(&mut region, || "include_output_1", include_output_1, 0, || {
                    bool_or_unknown(witness.as_ref().map(|value| value.include_output_1))
                })?;
                super::assign_advice_compat(&mut region, || "input_0_amount", input_0_amount, 0, || {
                    amount_or_unknown(witness.as_ref().map(|value| value.input_0_amount))
                })?;
                super::assign_advice_compat(&mut region, || "input_1_amount", input_1_amount, 0, || {
                    amount_or_unknown(witness.as_ref().map(|value| value.input_1_amount))
                })?;
                super::assign_advice_compat(&mut region, || "output_0_amount", output_0_amount, 0, || {
                    amount_or_unknown(witness.as_ref().map(|value| value.output_0_amount))
                })?;
                super::assign_advice_compat(&mut region, || "output_1_amount", output_1_amount, 0, || {
                    amount_or_unknown(witness.as_ref().map(|value| value.output_1_amount))
                })?;
                super::assign_advice_compat(&mut region, || "input_0_rho", input_0_rho, 0, || {
                    scalar_or_unknown(witness.as_ref().map(|value| scalar_to_repr_bytes(hash_to_scalar(
                        b"iroha.confidential.v2.note_rho",
                        &[&value.input_0_rho],
                    ))))
                })?;
                super::assign_advice_compat(&mut region, || "input_1_rho", input_1_rho, 0, || {
                    scalar_or_unknown(witness.as_ref().map(|value| scalar_to_repr_bytes(hash_to_scalar(
                        b"iroha.confidential.v2.note_rho",
                        &[&value.input_1_rho],
                    ))))
                })?;
                super::assign_advice_compat(&mut region, || "output_0_rho", output_0_rho, 0, || {
                    scalar_or_unknown(witness.as_ref().map(|value| scalar_to_repr_bytes(hash_to_scalar(
                        b"iroha.confidential.v2.note_rho",
                        &[&value.output_0_rho],
                    ))))
                })?;
                super::assign_advice_compat(&mut region, || "output_1_rho", output_1_rho, 0, || {
                    scalar_or_unknown(witness.as_ref().map(|value| scalar_to_repr_bytes(hash_to_scalar(
                        b"iroha.confidential.v2.note_rho",
                        &[&value.output_1_rho],
                    ))))
                })?;
                super::assign_advice_compat(&mut region, || "spend_scalar", spend_scalar, 0, || {
                    scalar_or_unknown(witness.as_ref().map(|value| value.spend_scalar))
                })?;
                super::assign_advice_compat(&mut region, || "output_0_owner_tag", output_0_owner_tag, 0, || {
                    scalar_or_unknown(witness.as_ref().map(|value| value.output_0_owner_tag))
                })?;
                super::assign_advice_compat(&mut region, || "output_1_owner_tag", output_1_owner_tag, 0, || {
                    scalar_or_unknown(witness.as_ref().map(|value| value.output_1_owner_tag))
                })?;
                for index in 0..DEPTH {
                    super::assign_advice_compat(&mut region, || format!("input_0_sibling_{index}"), input_0_siblings[index], 0, || {
                        scalar_or_unknown(witness.as_ref().and_then(|value| value.input_0_path.siblings.get(index).copied()))
                    })?;
                    super::assign_advice_compat(&mut region, || format!("input_0_direction_{index}"), input_0_directions[index], 0, || {
                        witness.as_ref()
                            .and_then(|value| value.input_0_path.directions.get(index).copied())
                            .map(|flag| if flag == 0 { Scalar::ZERO } else { Scalar::ONE })
                            .map_or(Value::unknown(), Value::known)
                    })?;
                    super::assign_advice_compat(&mut region, || format!("input_0_witness_{index}"), input_0_witness_nodes[index], 0, || {
                        scalar_or_unknown(witness.as_ref().and_then(|value| value.input_0_path.witness_nodes.get(index).copied()))
                    })?;
                    super::assign_advice_compat(&mut region, || format!("input_1_sibling_{index}"), input_1_siblings[index], 0, || {
                        scalar_or_unknown(witness.as_ref().and_then(|value| value.input_1_path.siblings.get(index).copied()))
                    })?;
                    super::assign_advice_compat(&mut region, || format!("input_1_direction_{index}"), input_1_directions[index], 0, || {
                        witness.as_ref()
                            .and_then(|value| value.input_1_path.directions.get(index).copied())
                            .map(|flag| if flag == 0 { Scalar::ZERO } else { Scalar::ONE })
                            .map_or(Value::unknown(), Value::known)
                    })?;
                    super::assign_advice_compat(&mut region, || format!("input_1_witness_{index}"), input_1_witness_nodes[index], 0, || {
                        scalar_or_unknown(witness.as_ref().and_then(|value| value.input_1_path.witness_nodes.get(index).copied()))
                    })?;
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
    input_0_path: ConfidentialMerklePathV2,
    input_1_path: ConfidentialMerklePathV2,
}

#[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
#[derive(Clone, Default)]
struct ConfidentialUnshieldCircuitV2<const DEPTH: usize> {
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
            let owner_tag = poseidon_pair_expr(sk.clone(), one.clone());
            let note_commit_expr =
                |amount: halo2_proofs::plonk::Expression<Scalar>,
                 rho: halo2_proofs::plonk::Expression<Scalar>| {
                    poseidon_pair_expr(
                        amount,
                        poseidon_pair_expr(rho, poseidon_pair_expr(owner_tag.clone(), asset_tag.clone())),
                    )
                };
            let nullifier_expr =
                |rho: halo2_proofs::plonk::Expression<Scalar>| {
                    poseidon_pair_expr(
                        sk.clone(),
                        poseidon_pair_expr(
                            rho,
                            poseidon_pair_expr(asset_tag.clone(), chain_tag.clone()),
                        ),
                    )
                };
            let in0_commit_expr = note_commit_expr(in0_amt.clone(), in0_rho.clone());
            let in1_commit_raw = note_commit_expr(in1_amt.clone(), in1_rho.clone());
            let nf0_expr = nullifier_expr(in0_rho.clone());
            let nf1_raw = nullifier_expr(in1_rho.clone());
            let mut constraints = vec![
                enabled.clone() * in1_present.clone() * (in1_present.clone() - one.clone()),
                enabled.clone()
                    * (in0_amt.clone() + in1_present.clone() * in1_amt.clone() - public_amount.clone()),
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
                constraints.push(
                    enabled.clone() * direction.clone() * (direction.clone() - one.clone()),
                );
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
                constraints.push(
                    enabled.clone() * direction.clone() * (direction.clone() - one.clone()),
                );
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
    fn synthesize(&self, cfg: Self::Config, mut layouter: impl Layouter<Scalar>) -> Result<(), PlonkError> {
        let (
            include_input_1,
            input_0_amount,
            input_1_amount,
            input_0_rho,
            input_1_rho,
            spend_scalar,
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
                    value.and_then(scalar_from_repr).map_or(Value::unknown(), Value::known)
                };
                let amount_or_unknown = |value: Option<u128>| {
                    value.map(scalar_from_u128).map_or(Value::unknown(), Value::known)
                };
                super::assign_advice_compat(&mut region, || "include_input_1", include_input_1, 0, || {
                    witness
                        .as_ref()
                        .map(|value| if value.include_input_1 { Scalar::ONE } else { Scalar::ZERO })
                        .map_or(Value::unknown(), Value::known)
                })?;
                super::assign_advice_compat(&mut region, || "input_0_amount", input_0_amount, 0, || {
                    amount_or_unknown(witness.as_ref().map(|value| value.input_0_amount))
                })?;
                super::assign_advice_compat(&mut region, || "input_1_amount", input_1_amount, 0, || {
                    amount_or_unknown(witness.as_ref().map(|value| value.input_1_amount))
                })?;
                super::assign_advice_compat(&mut region, || "input_0_rho", input_0_rho, 0, || {
                    scalar_or_unknown(witness.as_ref().map(|value| scalar_to_repr_bytes(hash_to_scalar(
                        b"iroha.confidential.v2.note_rho",
                        &[&value.input_0_rho],
                    ))))
                })?;
                super::assign_advice_compat(&mut region, || "input_1_rho", input_1_rho, 0, || {
                    scalar_or_unknown(witness.as_ref().map(|value| scalar_to_repr_bytes(hash_to_scalar(
                        b"iroha.confidential.v2.note_rho",
                        &[&value.input_1_rho],
                    ))))
                })?;
                super::assign_advice_compat(&mut region, || "spend_scalar", spend_scalar, 0, || {
                    scalar_or_unknown(witness.as_ref().map(|value| value.spend_scalar))
                })?;
                for index in 0..DEPTH {
                    super::assign_advice_compat(&mut region, || format!("input_0_sibling_{index}"), input_0_siblings[index], 0, || {
                        scalar_or_unknown(witness.as_ref().and_then(|value| value.input_0_path.siblings.get(index).copied()))
                    })?;
                    super::assign_advice_compat(&mut region, || format!("input_0_direction_{index}"), input_0_directions[index], 0, || {
                        witness.as_ref()
                            .and_then(|value| value.input_0_path.directions.get(index).copied())
                            .map(|flag| if flag == 0 { Scalar::ZERO } else { Scalar::ONE })
                            .map_or(Value::unknown(), Value::known)
                    })?;
                    super::assign_advice_compat(&mut region, || format!("input_0_witness_{index}"), input_0_witness_nodes[index], 0, || {
                        scalar_or_unknown(witness.as_ref().and_then(|value| value.input_0_path.witness_nodes.get(index).copied()))
                    })?;
                    super::assign_advice_compat(&mut region, || format!("input_1_sibling_{index}"), input_1_siblings[index], 0, || {
                        scalar_or_unknown(witness.as_ref().and_then(|value| value.input_1_path.siblings.get(index).copied()))
                    })?;
                    super::assign_advice_compat(&mut region, || format!("input_1_direction_{index}"), input_1_directions[index], 0, || {
                        witness.as_ref()
                            .and_then(|value| value.input_1_path.directions.get(index).copied())
                            .map(|flag| if flag == 0 { Scalar::ZERO } else { Scalar::ONE })
                            .map_or(Value::unknown(), Value::known)
                    })?;
                    super::assign_advice_compat(&mut region, || format!("input_1_witness_{index}"), input_1_witness_nodes[index], 0, || {
                        scalar_or_unknown(witness.as_ref().and_then(|value| value.input_1_path.witness_nodes.get(index).copied()))
                    })?;
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
    let parsed =
        super::zkparse::vk_from_bytes::<ConfidentialTransferCircuitV2<CONFIDENTIAL_TREE_DEPTH_V2>>(
            vk_box.bytes.as_slice(),
            &params,
        )
        .ok_or_else(|| "missing/invalid H2VK payload for confidential transfer verifying key".to_owned())?;
    Ok((params, parsed))
}

#[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
fn parse_vk_for_unshield(
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
    let parsed =
        super::zkparse::vk_from_bytes::<ConfidentialUnshieldCircuitV2<CONFIDENTIAL_TREE_DEPTH_V2>>(
            vk_box.bytes.as_slice(),
            &params,
        )
        .ok_or_else(|| "missing/invalid H2VK payload for confidential unshield verifying key".to_owned())?;
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
    Ok(ProofBox::new(super::ZK_BACKEND_HALO2_IPA.to_owned(), encoded))
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
    let owner_tag = derive_confidential_owner_tag_v2(spend_key);
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
    let input_0_commitment = derive_confidential_note_v2(
        asset_definition_id,
        input_0.amount,
        input_0.rho,
        owner_tag,
    )?;
    let input_1_commitment = if let Some(note) = input_1.as_ref() {
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
    let nullifier_0 =
        derive_confidential_nullifier_v2(chain_id.as_str(), asset_definition_id, spend_key, input_0.rho);
    let nullifier_1 = input_1.as_ref().map_or([0u8; 32], |note| {
        derive_confidential_nullifier_v2(chain_id.as_str(), asset_definition_id, spend_key, note.rho)
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
        output_0_owner_tag: output_0.owner_tag,
        output_1_owner_tag: output_1.as_ref().map_or([0u8; 32], |note| note.owner_tag),
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
        vec![scalar_from_repr(root_hint).ok_or_else(|| "root_hint must be a canonical Pasta scalar".to_owned())?],
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
    let proof = encode_halo2_envelope(
        CONFIDENTIAL_TRANSFER_V2_CIRCUIT_ID,
        vk_box,
        CONFIDENTIAL_TRANSFER_V2_PUBLIC_INPUTS_SCHEMA_V1.to_vec(),
        &instance_columns,
        transcript.finalize(),
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
    if inputs.is_empty() || inputs.len() > 2 {
        return Err("confidential unshield v2 supports one or two inputs".to_owned());
    }
    let computed_root = compute_confidential_root_v2(tree_commitments)?;
    if computed_root != root_hint {
        return Err("tree commitments do not match the supplied root_hint".to_owned());
    }
    let (params, parsed_vk) = parse_vk_for_unshield(circuit_id, vk_box)?;
    let owner_tag = derive_confidential_owner_tag_v2(spend_key);
    let spend_scalar = hash_to_scalar(b"iroha.confidential.v2.spend_scalar", &[spend_key]);
    let spend_scalar_bytes = scalar_to_repr_bytes(spend_scalar);
    let asset_tag = derive_confidential_asset_tag_v2(asset_definition_id);
    let chain_tag = derive_confidential_chain_tag_v2(chain_id.as_str());
    let input_0 = inputs
        .first()
        .cloned()
        .ok_or_else(|| "missing unshield input".to_owned())?;
    let input_1 = inputs.get(1).cloned();
    let input_0_commitment = derive_confidential_note_v2(
        asset_definition_id,
        input_0.amount,
        input_0.rho,
        owner_tag,
    )?;
    let input_1_commitment = if let Some(note) = input_1.as_ref() {
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
    let nullifier_0 =
        derive_confidential_nullifier_v2(chain_id.as_str(), asset_definition_id, spend_key, input_0.rho);
    let nullifier_1 = input_1.as_ref().map_or([0u8; 32], |note| {
        derive_confidential_nullifier_v2(chain_id.as_str(), asset_definition_id, spend_key, note.rho)
    });
    let witness = ConfidentialUnshieldWitnessV2 {
        include_input_1: input_1.is_some(),
        input_0_amount: input_0.amount,
        input_1_amount: input_1.as_ref().map_or(0, |note| note.amount),
        input_0_rho: input_0.rho,
        input_1_rho: input_1.as_ref().map_or([0u8; 32], |note| note.rho),
        spend_scalar: spend_scalar_bytes,
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
        vec![scalar_from_repr(root_hint).ok_or_else(|| "root_hint must be a canonical Pasta scalar".to_owned())?],
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
