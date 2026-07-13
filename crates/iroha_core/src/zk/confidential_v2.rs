use blake3::Hasher as Blake3Hasher;
use iroha_data_model::proof::VerifyingKeyBox;

#[cfg(all(any(feature = "zk-halo2", feature = "zk-halo2-ipa"), test))]
use halo2_proofs::{
    circuit::{Layouter, SimpleFloorPlanner, Value},
    plonk::{ConstraintSystem, Error as PlonkError, Selector},
    poly::Rotation,
};
#[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
use halo2_proofs::{
    halo2curves::{
        ff::{Field as _, PrimeField as _},
        pasta::Fp as Scalar,
    },
    plonk::Circuit,
};
#[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
use iroha_crypto::Hash as CryptoHash;
#[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
use iroha_data_model::{
    ChainId,
    confidential::ConfidentialStatus,
    proof::{ProofBox, VerifyingKeyRecord},
    zk::{BackendTag, OpenVerifyEnvelope, StarkFriOpenProofV1},
};
#[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
use zeroize::{Zeroize, Zeroizing};
/// Canonical circuit identifier for two-input/two-output confidential transfers.
pub const CONFIDENTIAL_TRANSFER_V2_CIRCUIT_ID: &str =
    "halo2/pasta/ipa/confidential-transfer-2x2-merkle16-axiom-poseidon-v3";
/// Canonical circuit identifier for full confidential unshielding.
pub const CONFIDENTIAL_UNSHIELD_V2_CIRCUIT_ID: &str =
    "halo2/pasta/ipa/confidential-unshield-full-merkle16-axiom-poseidon-v3";
/// Canonical circuit identifier for unshielding with one change output.
pub const CONFIDENTIAL_UNSHIELD_V3_CIRCUIT_ID: &str =
    "halo2/pasta/ipa/confidential-unshield-change-merkle16-axiom-poseidon-v4";
/// Canonical circuit identifier for Kagemusha top-up shielding.
pub const KAGEMUSHA_TOPUP_SHIELD_V2_CIRCUIT_ID: &str =
    "halo2/pasta/ipa/kagemusha-topup-shield-merkle16-axiom-poseidon-v3";
/// Canonical circuit identifier for asset-hidden transfers.
pub const ASSET_HIDDEN_TRANSFER_V1_CIRCUIT_ID: &str =
    "halo2/pasta/ipa/asset-hidden-transfer-public-v1";
/// IPA domain exponent for confidential transfer V2.
pub const CONFIDENTIAL_TRANSFER_V2_IPA_K: u32 = 13;
/// IPA domain exponent for confidential unshield V2.
pub const CONFIDENTIAL_UNSHIELD_V2_IPA_K: u32 = 13;
/// IPA domain exponent for confidential unshield V3.
pub const CONFIDENTIAL_UNSHIELD_V3_IPA_K: u32 = 13;
/// IPA domain exponent for Kagemusha top-up shielding.
pub const KAGEMUSHA_TOPUP_SHIELD_V2_IPA_K: u32 = 13;
/// IPA domain exponent for asset-hidden transfers.
pub const ASSET_HIDDEN_TRANSFER_V1_IPA_K: u32 = 6;
/// Fixed depth of the confidential commitment tree.
pub const CONFIDENTIAL_TREE_DEPTH_V2: usize = 16;
/// Maximum number of leaves in the confidential commitment tree.
pub const CONFIDENTIAL_TREE_CAPACITY_V2: usize = 1 << CONFIDENTIAL_TREE_DEPTH_V2;
/// Canonical public-input schema for confidential transfer V2.
pub const CONFIDENTIAL_TRANSFER_V2_PUBLIC_INPUTS_SCHEMA_V1: &[u8] = br#"{"schema":"confidential_transfer_v3","hash":"axiom_poseidon_t3_r2_rf8_rp57_mds0","merkle_leaf_domain":"cfleaf03","merkle_node_domain":"cfnode03","public_inputs":["input_commitment_0","input_commitment_1","nullifier_0","nullifier_1","output_commitment_0","output_commitment_1","root","asset_tag","chain_tag"]}"#;
/// Canonical public-input schema for confidential unshield V2.
pub const CONFIDENTIAL_UNSHIELD_V2_PUBLIC_INPUTS_SCHEMA_V1: &[u8] = br#"{"schema":"confidential_unshield_full_v3","hash":"axiom_poseidon_t3_r2_rf8_rp57_mds0","merkle_leaf_domain":"cfleaf03","merkle_node_domain":"cfnode03","public_inputs":["input_commitment_0","input_commitment_1","nullifier_0","nullifier_1","root","public_amount","asset_tag","chain_tag"]}"#;
/// Canonical public-input schema for confidential unshield V3.
pub const CONFIDENTIAL_UNSHIELD_V3_PUBLIC_INPUTS_SCHEMA_V1: &[u8] = br#"{"schema":"confidential_unshield_change_v4","hash":"axiom_poseidon_t3_r2_rf8_rp57_mds0","merkle_leaf_domain":"cfleaf03","merkle_node_domain":"cfnode03","public_inputs":["input_commitment_0","input_commitment_1","nullifier_0","nullifier_1","change_commitment_0","root","public_amount","asset_tag","chain_tag"]}"#;
/// Canonical public-input schema for Kagemusha top-up shielding.
pub const KAGEMUSHA_TOPUP_SHIELD_V2_PUBLIC_INPUTS_SCHEMA_V1: &[u8] = br#"{"schema":"kagemusha_topup_shield_v3","hash":"axiom_poseidon_t3_r2_rf8_rp57_mds0","merkle_leaf_domain":"cfleaf03","merkle_node_domain":"cfnode03","public_inputs":["output_commitment","spend_nullifier","initial_root","finalized_root","atomic_amount","asset_scale","leaf_index","asset_tag","chain_tag","payer_tag","operation_tag"]}"#;
/// Compatibility name for the second Kagemusha top-up schema contract.
///
/// The secure-relation rollout changed the authenticated schema contents while
/// retaining the same public columns. Both names therefore identify the exact
/// same canonical bytes during the first-release migration.
pub const KAGEMUSHA_TOPUP_SHIELD_V2_PUBLIC_INPUTS_SCHEMA_V2: &[u8] =
    KAGEMUSHA_TOPUP_SHIELD_V2_PUBLIC_INPUTS_SCHEMA_V1;
/// Canonical public-input schema for asset-hidden transfers.
pub const ASSET_HIDDEN_TRANSFER_V1_PUBLIC_INPUTS_SCHEMA_V1: &[u8] = br#"{"schema":"asset_hidden_transfer_v1","public_inputs":["pool_id","asset_set_root","input_commitment_0","input_commitment_1","nullifier_0","nullifier_1","output_commitment_0","output_commitment_1","root","chain_tag"]}"#;
/// Maximum accepted encoded confidential proof size.
pub const CONFIDENTIAL_V2_MAX_PROOF_BYTES: u32 = 192 * 1024;
/// Width of the pinned Axiom Poseidon secure permutation.
pub const CONFIDENTIAL_POSEIDON_T_V3: usize = 3;
/// Sponge rate of the pinned Axiom Poseidon secure permutation.
pub const CONFIDENTIAL_POSEIDON_RATE_V3: usize = 2;
/// Number of full rounds in the pinned Axiom Poseidon specification.
pub const CONFIDENTIAL_POSEIDON_FULL_ROUNDS_V3: usize = 8;
/// Number of partial rounds in the pinned Axiom Poseidon specification.
pub const CONFIDENTIAL_POSEIDON_PARTIAL_ROUNDS_V3: usize = 57;
/// Secure-MDS search index in the pinned Axiom Poseidon specification.
pub const CONFIDENTIAL_POSEIDON_SECURE_MDS_V3: usize = 0;
/// Domain word for owner-tag derivation.
pub const CONFIDENTIAL_POSEIDON_OWNER_DOMAIN_V3: u64 = u64::from_le_bytes(*b"cfownr03");
/// Domain word for note commitments.
pub const CONFIDENTIAL_POSEIDON_NOTE_DOMAIN_V3: u64 = u64::from_le_bytes(*b"cfnote03");
/// Domain word for spend nullifiers.
pub const CONFIDENTIAL_POSEIDON_NULLIFIER_DOMAIN_V3: u64 = u64::from_le_bytes(*b"cfnull03");
/// Domain word for commitment-tree leaves.
pub const CONFIDENTIAL_POSEIDON_MERKLE_LEAF_DOMAIN_V3: u64 = u64::from_le_bytes(*b"cfleaf03");
/// Domain word for commitment-tree internal nodes.
pub const CONFIDENTIAL_POSEIDON_MERKLE_NODE_DOMAIN_V3: u64 = u64::from_le_bytes(*b"cfnode03");
/// Domain word for asset tags.
pub const CONFIDENTIAL_POSEIDON_ASSET_DOMAIN_V3: u64 = u64::from_le_bytes(*b"cfasst03");
/// Domain word for chain tags.
pub const CONFIDENTIAL_POSEIDON_CHAIN_DOMAIN_V3: u64 = u64::from_le_bytes(*b"cfchn_03");
/// Domain word for Kagemusha payer tags.
pub const CONFIDENTIAL_POSEIDON_PAYER_DOMAIN_V3: u64 = u64::from_le_bytes(*b"cfpayr03");
/// Domain word for Kagemusha operation tags.
pub const CONFIDENTIAL_POSEIDON_OPERATION_DOMAIN_V3: u64 = u64::from_le_bytes(*b"cfoper03");

/// Canonical Merkle authentication path used by confidential circuits.
#[derive(Debug, Clone)]
pub struct ConfidentialMerklePathV2 {
    /// Sibling node at each tree level, from leaf to root.
    pub siblings: Vec<[u8; 32]>,
    /// Canonical left/right direction bit at each level.
    pub directions: Vec<u8>,
    /// Recomputed parent node at each level.
    pub witness_nodes: Vec<[u8; 32]>,
    /// Root authenticated by the complete path.
    pub root: [u8; 32],
}

impl ConfidentialMerklePathV2 {
    /// Consume the path without bypassing its zeroizing `Drop` implementation.
    ///
    /// Moving individual fields out of a type that implements `Drop` is not
    /// permitted.  Callers that need to translate the path into another typed
    /// wire representation use this method, which replaces every retained
    /// field with its zero/empty value before the original allocation drops.
    #[must_use]
    pub fn into_parts(mut self) -> (Vec<[u8; 32]>, Vec<u8>, Vec<[u8; 32]>, [u8; 32]) {
        (
            std::mem::take(&mut self.siblings),
            std::mem::take(&mut self.directions),
            std::mem::take(&mut self.witness_nodes),
            std::mem::take(&mut self.root),
        )
    }
}

/// Secret opening and tree position for one transfer input.
#[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
#[derive(Debug, Clone)]
pub struct ConfidentialTransferInputV2 {
    /// Exact atomic amount opened by the note.
    pub amount: u128,
    /// Secret note nonce.
    pub rho: [u8; 32],
    /// Owner-key diversifier.
    pub diversifier: [u8; 32],
    /// Commitment-tree leaf index.
    pub leaf_index: usize,
}

/// Secret opening and owner binding for one transfer output.
#[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
#[derive(Debug, Clone)]
pub struct ConfidentialTransferOutputV2 {
    /// Exact atomic output amount.
    pub amount: u128,
    /// Secret note nonce.
    pub rho: [u8; 32],
    /// Recipient owner tag.
    pub owner_tag: [u8; 32],
}

/// Generated confidential transfer evidence and its public outputs.
#[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
#[derive(Debug, Clone)]
pub struct ConfidentialTransferProofV2 {
    /// Nullifiers consumed by the transfer.
    pub nullifiers: Vec<[u8; 32]>,
    /// Commitments created by the transfer.
    pub output_commitments: Vec<[u8; 32]>,
    /// Authenticated input commitment-tree root.
    pub root: [u8; 32],
    /// Encoded Halo2 proof envelope.
    pub proof: ProofBox,
}

/// Generated Kagemusha top-up shield evidence and public state.
#[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
#[derive(Debug, Clone)]
pub struct KagemushaTopUpShieldProofV2 {
    /// Newly inserted confidential note commitment.
    pub output_commitment: [u8; 32],
    /// Nullifier derived for the inserted note.
    pub spend_nullifier: [u8; 32],
    /// Root before insertion.
    pub initial_root: [u8; 32],
    /// Root after insertion.
    pub finalized_root: [u8; 32],
    /// Inserted leaf index.
    pub leaf_index: u32,
    /// Encoded Halo2 proof envelope.
    pub proof: ProofBox,
}

/// Parsed public inputs for one Kagemusha top-up shield proof.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KagemushaTopUpShieldPublicInputsV2 {
    /// Newly inserted confidential note commitment.
    pub output_commitment: [u8; 32],
    /// Nullifier derived for the inserted note.
    pub spend_nullifier: [u8; 32],
    /// Root before insertion.
    pub initial_root: [u8; 32],
    /// Root after insertion.
    pub finalized_root: [u8; 32],
    /// Canonically encoded atomic amount.
    pub atomic_amount: [u8; 32],
    /// Canonically encoded asset scale.
    pub asset_scale: [u8; 32],
    /// Canonically encoded leaf index.
    pub leaf_index: [u8; 32],
    /// Asset-domain tag.
    pub asset_tag: [u8; 32],
    /// Chain-domain tag.
    pub chain_tag: [u8; 32],
    /// Payer identity tag.
    pub payer_tag: [u8; 32],
    /// Top-up operation tag.
    pub operation_tag: [u8; 32],
}

/// Secret opening and tree position for one unshield input.
#[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
#[derive(Debug, Clone)]
pub struct ConfidentialUnshieldInputV2 {
    /// Exact atomic amount opened by the note.
    pub amount: u128,
    /// Secret note nonce.
    pub rho: [u8; 32],
    /// Owner-key diversifier.
    pub diversifier: [u8; 32],
    /// Commitment-tree leaf index.
    pub leaf_index: usize,
}

/// Generated full-unshield evidence and public state.
#[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
#[derive(Debug, Clone)]
pub struct ConfidentialUnshieldProofV2 {
    /// Nullifiers consumed by the unshield.
    pub nullifiers: Vec<[u8; 32]>,
    /// Authenticated input commitment-tree root.
    pub root: [u8; 32],
    /// Encoded Halo2 proof envelope.
    pub proof: ProofBox,
}

/// Secret opening for the optional unshield-change output.
#[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
#[derive(Debug, Clone)]
pub struct ConfidentialUnshieldOutputV3 {
    /// Exact atomic change amount.
    pub amount: u128,
    /// Secret change-note nonce.
    pub rho: [u8; 32],
}

/// Generated change-preserving unshield evidence and public state.
#[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
#[derive(Debug, Clone)]
pub struct ConfidentialUnshieldProofV3 {
    /// Nullifiers consumed by the unshield.
    pub nullifiers: Vec<[u8; 32]>,
    /// Confidential change commitments, empty for a full redemption.
    pub output_commitments: Vec<[u8; 32]>,
    /// Authenticated input commitment-tree root.
    pub root: [u8; 32],
    /// Encoded Halo2 proof envelope.
    pub proof: ProofBox,
}

/// Generated asset-hidden transfer evidence and public state.
#[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
#[derive(Debug, Clone)]
pub struct AssetHiddenTransferProofV1 {
    /// Confidential input commitments.
    pub input_commitments: Vec<[u8; 32]>,
    /// Nullifiers consumed by the transfer.
    pub nullifiers: Vec<[u8; 32]>,
    /// Confidential output commitments.
    pub output_commitments: Vec<[u8; 32]>,
    /// Authenticated input commitment-tree root.
    pub root: [u8; 32],
    /// Encoded proof envelope.
    pub proof: ProofBox,
}

#[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
impl Zeroize for ConfidentialMerklePathV2 {
    fn zeroize(&mut self) {
        self.siblings.zeroize();
        self.directions.zeroize();
        self.witness_nodes.zeroize();
        self.root.zeroize();
    }
}

#[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
impl Drop for ConfidentialMerklePathV2 {
    fn drop(&mut self) {
        self.zeroize();
    }
}

#[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
impl Zeroize for ConfidentialTransferInputV2 {
    fn zeroize(&mut self) {
        self.amount.zeroize();
        self.rho.zeroize();
        self.diversifier.zeroize();
        self.leaf_index.zeroize();
    }
}

#[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
impl Drop for ConfidentialTransferInputV2 {
    fn drop(&mut self) {
        self.zeroize();
    }
}

#[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
impl Zeroize for ConfidentialTransferOutputV2 {
    fn zeroize(&mut self) {
        self.amount.zeroize();
        self.rho.zeroize();
        self.owner_tag.zeroize();
    }
}

#[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
impl Drop for ConfidentialTransferOutputV2 {
    fn drop(&mut self) {
        self.zeroize();
    }
}

#[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
impl Zeroize for ConfidentialUnshieldInputV2 {
    fn zeroize(&mut self) {
        self.amount.zeroize();
        self.rho.zeroize();
        self.diversifier.zeroize();
        self.leaf_index.zeroize();
    }
}

#[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
impl Drop for ConfidentialUnshieldInputV2 {
    fn drop(&mut self) {
        self.zeroize();
    }
}

#[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
impl Zeroize for ConfidentialUnshieldOutputV3 {
    fn zeroize(&mut self) {
        self.amount.zeroize();
        self.rho.zeroize();
    }
}

#[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
impl Drop for ConfidentialUnshieldOutputV3 {
    fn drop(&mut self) {
        self.zeroize();
    }
}

/// Return whether an identifier exactly selects the production confidential-transfer circuit.
pub fn is_confidential_transfer_v2_circuit_id(raw: &str) -> bool {
    raw == CONFIDENTIAL_TRANSFER_V2_CIRCUIT_ID
}

/// Return whether an identifier exactly selects the production Kagemusha top-up circuit.
pub fn is_kagemusha_topup_shield_v2_circuit_id(raw: &str) -> bool {
    raw == KAGEMUSHA_TOPUP_SHIELD_V2_CIRCUIT_ID
}

/// Return whether an identifier exactly selects the production full-unshield circuit.
pub fn is_confidential_unshield_v2_circuit_id(raw: &str) -> bool {
    raw == CONFIDENTIAL_UNSHIELD_V2_CIRCUIT_ID
}

#[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
type ConfidentialV2ProvingKey = super::halo2_backend::ProvingKey;

#[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
type ConfidentialV2VerifyingKey = super::halo2_backend::VerifyingKey;

#[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
fn build_confidential_v2_vk_box<C>(
    k: u32,
    circuit_id: &str,
    circuit: &C,
) -> Result<VerifyingKeyBox, String>
where
    C: Circuit<Scalar>,
{
    let params = super::pasta_params_new(k);
    let vk = super::halo2_backend::keygen_vk(&params, circuit)
        .map_err(|err| format!("failed to generate confidential v2 verifying key: {err}"))?;
    let mut bytes = super::zk1::wrap_start();
    super::zk1::wrap_append_ipa_k(&mut bytes, k);
    super::zk1::wrap_append_circuit_id(&mut bytes, circuit_id);
    super::zk1::wrap_append_vk_pasta(&mut bytes, &vk);
    Ok(VerifyingKeyBox::new(
        super::ZK_BACKEND_HALO2_IPA.to_owned(),
        bytes,
    ))
}

#[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
fn ensure_confidential_v2_vk_box_shape(
    vk_box: &VerifyingKeyBox,
    circuit_id: &str,
    ipa_k: u32,
    label: &str,
) -> Result<(), String> {
    let actual_ipa_k =
        super::zk1::ensure_halo2_ipa_vk_envelope_shape_any_k(&vk_box.bytes, circuit_id)
            .map_err(|err| format!("{label} verifier key {err}"))?;
    if actual_ipa_k != ipa_k {
        return Err(format!(
            "{label} verifier key IPAK `{actual_ipa_k}` is not `{ipa_k}`"
        ));
    }
    let h2vk = super::zk1::h2vk_payload(vk_box.bytes.as_slice())
        .map_err(|err| format!("{label} verifier key {err}"))?;
    let (h2vk_k, _compress_selectors, _fixed_columns) = super::zk1::halo2_pasta_vk_header(h2vk)
        .map_err(|err| format!("{label} verifier key {err}"))?;
    if h2vk_k != actual_ipa_k {
        return Err(format!(
            "{label} verifier key IPAK `{actual_ipa_k}` does not match H2VK domain `{h2vk_k}`"
        ));
    }
    Ok(())
}

#[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
/// Return the process-cached canonical confidential-transfer verifying key.
pub fn confidential_transfer_v2_vk_box() -> Result<VerifyingKeyBox, String> {
    static CACHE: std::sync::OnceLock<Result<VerifyingKeyBox, String>> = std::sync::OnceLock::new();

    CACHE
        .get_or_init(|| {
            build_confidential_v2_vk_box(
                CONFIDENTIAL_TRANSFER_V2_IPA_K,
                CONFIDENTIAL_TRANSFER_V2_CIRCUIT_ID,
                &secure_relation_v3::ConfidentialTransferCircuitV3::<
                    CONFIDENTIAL_TREE_DEPTH_V2,
                >::default(),
            )
        })
        .clone()
}

#[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
/// Return the process-cached canonical Kagemusha top-up verifying key.
pub fn kagemusha_topup_shield_v2_vk_box() -> Result<VerifyingKeyBox, String> {
    static CACHE: std::sync::OnceLock<Result<VerifyingKeyBox, String>> = std::sync::OnceLock::new();

    CACHE
        .get_or_init(|| {
            build_confidential_v2_vk_box(
                KAGEMUSHA_TOPUP_SHIELD_V2_IPA_K,
                KAGEMUSHA_TOPUP_SHIELD_V2_CIRCUIT_ID,
                &secure_relation_v3::KagemushaTopUpShieldCircuitV3::<
                    CONFIDENTIAL_TREE_DEPTH_V2,
                >::default(),
            )
        })
        .clone()
}

/// Require an exact canonical Kagemusha top-up verifying key.
pub fn ensure_kagemusha_topup_shield_v2_canonical_vk_box(
    vk_box: &VerifyingKeyBox,
) -> Result<(), String> {
    if vk_box.backend.as_str() != super::ZK_BACKEND_HALO2_IPA {
        return Err(format!(
            "Kagemusha top-up shield v2 verifier key backend `{}` is not `{}`",
            vk_box.backend,
            super::ZK_BACKEND_HALO2_IPA
        ));
    }
    if vk_box.bytes.is_empty() {
        return Err("Kagemusha top-up shield v2 verifier key must be non-empty".to_owned());
    }
    #[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
    {
        ensure_confidential_v2_vk_box_shape(
            vk_box,
            KAGEMUSHA_TOPUP_SHIELD_V2_CIRCUIT_ID,
            KAGEMUSHA_TOPUP_SHIELD_V2_IPA_K,
            "Kagemusha top-up shield v2",
        )?;
        let canonical = kagemusha_topup_shield_v2_vk_box()?;
        if super::hash_vk(vk_box) != super::hash_vk(&canonical) || vk_box.bytes != canonical.bytes {
            return Err(
                "Kagemusha top-up shield v2 verifier key must match the canonical issuance circuit key"
                    .to_owned(),
            );
        }
    }
    Ok(())
}

#[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
/// Return the process-cached canonical asset-hidden transfer verifying key.
pub fn asset_hidden_transfer_v1_vk_box() -> Result<VerifyingKeyBox, String> {
    static CACHE: std::sync::OnceLock<Result<VerifyingKeyBox, String>> = std::sync::OnceLock::new();

    CACHE
        .get_or_init(|| {
            build_confidential_v2_vk_box(
                ASSET_HIDDEN_TRANSFER_V1_IPA_K,
                ASSET_HIDDEN_TRANSFER_V1_CIRCUIT_ID,
                &super::pasta_tiny::AssetHiddenTransferPublic::default(),
            )
        })
        .clone()
}

/// Require an exact canonical confidential-transfer verifying key.
pub fn ensure_confidential_transfer_v2_canonical_vk_box(
    vk_box: &VerifyingKeyBox,
) -> Result<(), String> {
    if vk_box.backend.as_str() != super::ZK_BACKEND_HALO2_IPA {
        return Err(format!(
            "Confidential transfer v2 verifier key backend `{}` is not `{}`",
            vk_box.backend,
            super::ZK_BACKEND_HALO2_IPA
        ));
    }
    if vk_box.bytes.is_empty() {
        return Err("Confidential transfer v2 verifier key must be non-empty".to_owned());
    }
    #[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
    {
        ensure_confidential_v2_vk_box_shape(
            vk_box,
            CONFIDENTIAL_TRANSFER_V2_CIRCUIT_ID,
            CONFIDENTIAL_TRANSFER_V2_IPA_K,
            "Confidential transfer v2",
        )?;
        let canonical = confidential_transfer_v2_vk_box()?;
        if super::hash_vk(vk_box) != super::hash_vk(&canonical) || vk_box.bytes != canonical.bytes {
            return Err(
                "Confidential transfer v2 verifier key must match the canonical semantic circuit key"
                    .to_owned(),
            );
        }
    }
    Ok(())
}

/// Require an exact canonical asset-hidden transfer verifying key.
pub fn ensure_asset_hidden_transfer_v1_canonical_vk_box(
    vk_box: &VerifyingKeyBox,
) -> Result<(), String> {
    if vk_box.backend.as_str() != super::ZK_BACKEND_HALO2_IPA {
        return Err(format!(
            "Asset-hidden transfer v1 verifier key backend `{}` is not `{}`",
            vk_box.backend,
            super::ZK_BACKEND_HALO2_IPA
        ));
    }
    if vk_box.bytes.is_empty() {
        return Err("Asset-hidden transfer v1 verifier key must be non-empty".to_owned());
    }
    #[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
    {
        ensure_confidential_v2_vk_box_shape(
            vk_box,
            ASSET_HIDDEN_TRANSFER_V1_CIRCUIT_ID,
            ASSET_HIDDEN_TRANSFER_V1_IPA_K,
            "Asset-hidden transfer v1",
        )?;
        let canonical = asset_hidden_transfer_v1_vk_box()?;
        if super::hash_vk(vk_box) != super::hash_vk(&canonical) || vk_box.bytes != canonical.bytes {
            return Err(
                "Asset-hidden transfer v1 verifier key must match the canonical public-input circuit key"
                    .to_owned(),
            );
        }
    }
    Ok(())
}

#[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
/// Return the process-cached canonical full-unshield verifying key.
pub fn confidential_unshield_v2_vk_box() -> Result<VerifyingKeyBox, String> {
    static CACHE: std::sync::OnceLock<Result<VerifyingKeyBox, String>> = std::sync::OnceLock::new();

    CACHE
        .get_or_init(|| {
            build_confidential_v2_vk_box(
                CONFIDENTIAL_UNSHIELD_V2_IPA_K,
                CONFIDENTIAL_UNSHIELD_V2_CIRCUIT_ID,
                &secure_relation_v3::ConfidentialUnshieldFullCircuitV3::<
                    CONFIDENTIAL_TREE_DEPTH_V2,
                >::default(),
            )
        })
        .clone()
}

/// Require an exact canonical full-unshield verifying key.
pub fn ensure_confidential_unshield_v2_canonical_vk_box(
    vk_box: &VerifyingKeyBox,
) -> Result<(), String> {
    if vk_box.backend.as_str() != super::ZK_BACKEND_HALO2_IPA {
        return Err(format!(
            "Confidential unshield v2 verifier key backend `{}` is not `{}`",
            vk_box.backend,
            super::ZK_BACKEND_HALO2_IPA
        ));
    }
    if vk_box.bytes.is_empty() {
        return Err("Confidential unshield v2 verifier key must be non-empty".to_owned());
    }
    #[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
    {
        ensure_confidential_v2_vk_box_shape(
            vk_box,
            CONFIDENTIAL_UNSHIELD_V2_CIRCUIT_ID,
            CONFIDENTIAL_UNSHIELD_V2_IPA_K,
            "Confidential unshield v2",
        )?;
        let canonical = confidential_unshield_v2_vk_box()?;
        if super::hash_vk(vk_box) != super::hash_vk(&canonical) || vk_box.bytes != canonical.bytes {
            return Err(
                "Confidential unshield v2 verifier key must match the canonical semantic circuit key"
                    .to_owned(),
            );
        }
    }
    Ok(())
}

#[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
/// Return the process-cached canonical change-unshield verifying key.
pub fn confidential_unshield_v3_vk_box() -> Result<VerifyingKeyBox, String> {
    static CACHE: std::sync::OnceLock<Result<VerifyingKeyBox, String>> = std::sync::OnceLock::new();

    CACHE
        .get_or_init(|| {
            build_confidential_v2_vk_box(
                CONFIDENTIAL_UNSHIELD_V3_IPA_K,
                CONFIDENTIAL_UNSHIELD_V3_CIRCUIT_ID,
                &secure_relation_v3::ConfidentialUnshieldChangeCircuitV4::<
                    CONFIDENTIAL_TREE_DEPTH_V2,
                >::default(),
            )
        })
        .clone()
}

/// Require an exact canonical change-unshield verifying key.
pub fn ensure_confidential_unshield_v3_canonical_vk_box(
    vk_box: &VerifyingKeyBox,
) -> Result<(), String> {
    if vk_box.backend.as_str() != super::ZK_BACKEND_HALO2_IPA {
        return Err(format!(
            "Confidential unshield v3 verifier key backend `{}` is not `{}`",
            vk_box.backend,
            super::ZK_BACKEND_HALO2_IPA
        ));
    }
    if vk_box.bytes.is_empty() {
        return Err("Confidential unshield v3 verifier key must be non-empty".to_owned());
    }
    #[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
    {
        ensure_confidential_v2_vk_box_shape(
            vk_box,
            CONFIDENTIAL_UNSHIELD_V3_CIRCUIT_ID,
            CONFIDENTIAL_UNSHIELD_V3_IPA_K,
            "Confidential unshield v3",
        )?;
        let canonical = confidential_unshield_v3_vk_box()?;
        if super::hash_vk(vk_box) != super::hash_vk(&canonical) || vk_box.bytes != canonical.bytes {
            return Err(
                "Confidential unshield v3 verifier key must match the canonical semantic circuit key"
                    .to_owned(),
            );
        }
    }
    Ok(())
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
/// Build an active verifier record for confidential transfer V2.
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
/// Build an active verifier record for Kagemusha top-up shielding V2.
pub fn kagemusha_topup_shield_v2_vk_record(
    name: &str,
    version: u32,
) -> Result<VerifyingKeyRecord, String> {
    confidential_v2_vk_record(
        name,
        version,
        KAGEMUSHA_TOPUP_SHIELD_V2_CIRCUIT_ID,
        KAGEMUSHA_TOPUP_SHIELD_V2_PUBLIC_INPUTS_SCHEMA_V2,
        kagemusha_topup_shield_v2_vk_box()?,
    )
}

#[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
/// Build an active verifier record for confidential unshield V2.
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

#[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
/// Build an active verifier record for confidential unshield V3.
pub fn confidential_unshield_v3_vk_record(
    name: &str,
    version: u32,
) -> Result<VerifyingKeyRecord, String> {
    confidential_v2_vk_record(
        name,
        version,
        CONFIDENTIAL_UNSHIELD_V3_CIRCUIT_ID,
        CONFIDENTIAL_UNSHIELD_V3_PUBLIC_INPUTS_SCHEMA_V1,
        confidential_unshield_v3_vk_box()?,
    )
}

/// Return whether an identifier exactly selects the production change-unshield circuit.
pub fn is_confidential_unshield_v3_circuit_id(raw: &str) -> bool {
    raw == CONFIDENTIAL_UNSHIELD_V3_CIRCUIT_ID
}

/// Return whether an identifier exactly selects asset-hidden transfer V1.
pub fn is_asset_hidden_transfer_v1_circuit_id(raw: &str) -> bool {
    raw == ASSET_HIDDEN_TRANSFER_V1_CIRCUIT_ID
}

/// Parse the exact public columns from a confidential-transfer proof envelope.
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

/// Parse the exact public columns from a Kagemusha top-up proof envelope.
pub fn parse_kagemusha_topup_shield_public_inputs_v2(
    proof_bytes: &[u8],
) -> Result<KagemushaTopUpShieldPublicInputsV2, String> {
    let columns = extract_confidential_public_columns(proof_bytes)
        .ok_or_else(|| "failed to decode Kagemusha top-up shield public inputs".to_owned())?;
    if columns.len() != 11 || columns.iter().any(|column| column.len() != 1) {
        return Err(
            "Kagemusha top-up shield proof must expose exactly 11 single-row instance columns"
                .to_owned(),
        );
    }
    Ok(KagemushaTopUpShieldPublicInputsV2 {
        output_commitment: columns[0][0],
        spend_nullifier: columns[1][0],
        initial_root: columns[2][0],
        finalized_root: columns[3][0],
        atomic_amount: columns[4][0],
        asset_scale: columns[5][0],
        leaf_index: columns[6][0],
        asset_tag: columns[7][0],
        chain_tag: columns[8][0],
        payer_tag: columns[9][0],
        operation_tag: columns[10][0],
    })
}

/// Parse the exact public columns from a full-unshield proof envelope.
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

/// Parse the exact public columns from a change-unshield proof envelope.
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// Parsed public inputs for an asset-hidden transfer proof.
pub struct AssetHiddenTransferPublicInputsV1 {
    /// Pool identity tag.
    pub pool_id_tag: [u8; 32],
    /// Root of the permitted asset set.
    pub asset_set_root: [u8; 32],
    /// Confidential input commitments.
    pub input_commitments: [[u8; 32]; 2],
    /// Consumed nullifiers.
    pub nullifiers: [[u8; 32]; 2],
    /// Confidential output commitments.
    pub outputs: [[u8; 32]; 2],
    /// Authenticated commitment-tree root.
    pub root: [u8; 32],
    /// Chain-domain tag.
    pub chain_tag: [u8; 32],
}

/// Parse the exact public columns from an asset-hidden transfer proof envelope.
pub fn parse_asset_hidden_transfer_public_inputs(
    proof_bytes: &[u8],
) -> Result<AssetHiddenTransferPublicInputsV1, String> {
    let columns = extract_confidential_public_columns(proof_bytes)
        .ok_or_else(|| "failed to decode asset-hidden transfer proof public inputs".to_owned())?;
    if columns.len() < 10 || columns.iter().take(10).any(|column| column.len() != 1) {
        return Err(
            "asset-hidden transfer proof must expose 10 single-row instance columns".to_owned(),
        );
    }
    Ok(AssetHiddenTransferPublicInputsV1 {
        pool_id_tag: columns[0][0],
        asset_set_root: columns[1][0],
        input_commitments: [columns[2][0], columns[3][0]],
        nullifiers: [columns[4][0], columns[5][0]],
        outputs: [columns[6][0], columns[7][0]],
        root: columns[8][0],
        chain_tag: columns[9][0],
    })
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

#[cfg(all(any(feature = "zk-halo2", feature = "zk-halo2-ipa"), test))]
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
fn confidential_poseidon_hash_v3<F>(domain: u64, inputs: &[F]) -> F
where
    F: snark_verifier::util::arithmetic::FieldExt,
{
    use snark_verifier::{loader::native::LOADER, util::hash::Poseidon};

    let mut preimage = Vec::with_capacity(inputs.len() + 2);
    preimage.push(F::from(domain));
    preimage.push(F::from_u128(inputs.len() as u128));
    preimage.extend_from_slice(inputs);
    let mut hasher =
        Poseidon::<F, F, CONFIDENTIAL_POSEIDON_T_V3, CONFIDENTIAL_POSEIDON_RATE_V3>::new::<
            CONFIDENTIAL_POSEIDON_FULL_ROUNDS_V3,
            CONFIDENTIAL_POSEIDON_PARTIAL_ROUNDS_V3,
            CONFIDENTIAL_POSEIDON_SECURE_MDS_V3,
        >(&LOADER);
    hasher.update(&preimage);
    hasher.squeeze()
}

/// Shared confidential relation expressions used by standalone proofs and
/// Kagemusha's recursive Eq step. Keeping this module as the single source of
/// the note, nullifier, and Merkle formulas prevents the recursive circuit from
/// drifting away from the public confidential proof system.
#[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
pub(super) mod confidential_relation_gadget {
    use halo2_base::{
        AssignedValue, Context,
        gates::{RangeChip, RangeInstructions},
        poseidon::hasher::{PoseidonHasher, spec::OptimizedPoseidonSpec},
        utils::BigPrimeField,
    };
    #[cfg(test)]
    use halo2_proofs::{
        circuit::{Region, Value},
        halo2curves::ff::Field,
        plonk::{Advice, Column, ConstraintSystem, Error, Expression, Selector},
        poly::Rotation,
    };

    #[cfg(test)]
    const U128_RANGE_ROWS: usize = 8;
    #[cfg(test)]
    const U128_RANGE_BITS_PER_ROW: usize = 16;

    /// Shared secure Poseidon gadget for confidential and recursive relations.
    pub(in crate::zk) struct ConfidentialPoseidonChipV3<F: BigPrimeField> {
        hasher: PoseidonHasher<
            F,
            { super::CONFIDENTIAL_POSEIDON_T_V3 },
            { super::CONFIDENTIAL_POSEIDON_RATE_V3 },
        >,
    }

    impl<F: BigPrimeField> ConfidentialPoseidonChipV3<F> {
        /// Initialize the pinned Axiom specification and reusable constants.
        pub(super) fn new(ctx: &mut Context<F>, range: &RangeChip<F>) -> Self {
            let spec = OptimizedPoseidonSpec::new::<
                { super::CONFIDENTIAL_POSEIDON_FULL_ROUNDS_V3 },
                { super::CONFIDENTIAL_POSEIDON_PARTIAL_ROUNDS_V3 },
                { super::CONFIDENTIAL_POSEIDON_SECURE_MDS_V3 },
            >();
            let mut hasher = PoseidonHasher::new(spec);
            hasher.initialize_consts(ctx, range.gate());
            Self { hasher }
        }

        /// Hash a fixed-arity list with an explicit use-domain and arity word.
        pub(super) fn hash(
            &self,
            ctx: &mut Context<F>,
            range: &RangeChip<F>,
            domain: u64,
            inputs: &[AssignedValue<F>],
        ) -> AssignedValue<F> {
            let mut preimage = Vec::with_capacity(inputs.len() + 2);
            preimage.push(ctx.load_constant(F::from(domain)));
            preimage.push(ctx.load_constant(F::from(inputs.len() as u64)));
            preimage.extend_from_slice(inputs);
            self.hasher.hash_fix_len_array(ctx, range.gate(), &preimage)
        }
    }

    #[derive(Clone, Debug)]
    /// Reusable bit-decomposition gadget constraining one exact `u128` value.
    #[cfg(test)]
    pub(in crate::zk) struct U128RangeConfig {
        value: Column<Advice>,
        bits: [Column<Advice>; U128_RANGE_BITS_PER_ROW],
        selector: Selector,
    }

    #[cfg(test)]
    impl U128RangeConfig {
        /// Allocate the fixed 128-bit decomposition columns and constraints.
        pub(super) fn configure<F>(meta: &mut ConstraintSystem<F>) -> Self
        where
            F: Field + From<u64>,
        {
            let value = meta.advice_column();
            let bits = std::array::from_fn(|_| meta.advice_column());
            let selector = meta.selector();
            meta.create_gate("confidential_u128_range", |meta| {
                let enabled = meta.query_selector(selector);
                let value_expression = meta.query_advice(value, Rotation::cur());
                let one = Expression::Constant(F::ONE);
                let mut coefficient = F::ONE;
                let mut reconstructed = Expression::Constant(F::ZERO);
                let mut constraints = Vec::with_capacity(129);
                for row in 0..U128_RANGE_ROWS {
                    for bit_column in bits {
                        let bit = meta.query_advice(
                            bit_column,
                            Rotation(i32::try_from(row).expect("u128 range rotation fits i32")),
                        );
                        constraints
                            .push(enabled.clone() * bit.clone() * (bit.clone() - one.clone()));
                        reconstructed = reconstructed + Expression::Constant(coefficient) * bit;
                        coefficient = coefficient + coefficient;
                    }
                }
                constraints.push(enabled * (value_expression - reconstructed));
                constraints
            });
            Self {
                value,
                bits,
                selector,
            }
        }

        /// Query the packed range-checked value at a fixed region offset.
        pub(super) fn query_value_at<F>(
            &self,
            meta: &mut halo2_proofs::plonk::VirtualCells<'_, F>,
            offset: usize,
        ) -> Expression<F>
        where
            F: Field,
        {
            meta.query_advice(
                self.value,
                Rotation(i32::try_from(offset).expect("u128 range offset fits i32")),
            )
        }

        /// Assign one optional witness and all 128 canonical bits.
        pub(super) fn assign<F>(
            &self,
            region: &mut Region<'_, F>,
            offset: usize,
            value: Option<u128>,
        ) -> Result<(), Error>
        where
            F: Field + From<u64>,
        {
            self.selector.enable(region, offset)?;
            let field_value = value.map(|value| {
                let low = F::from(value as u64);
                let high = F::from((value >> 64) as u64);
                let mut two_pow_64 = F::ONE;
                for _ in 0..64 {
                    two_pow_64 = two_pow_64 + two_pow_64;
                }
                low + high * two_pow_64
            });
            super::super::assign_advice_compat(
                region,
                || "u128_range_value",
                self.value,
                offset,
                || field_value.map_or(Value::unknown(), Value::known),
            )?;
            for row in 0..U128_RANGE_ROWS {
                for (column_index, column) in self.bits.iter().copied().enumerate() {
                    let bit_index = row * U128_RANGE_BITS_PER_ROW + column_index;
                    let bit = value.map(|value| F::from(((value >> bit_index) & 1) as u64));
                    super::super::assign_advice_compat(
                        region,
                        || "u128_range_bit",
                        column,
                        offset + row,
                        || bit.map_or(Value::unknown(), Value::known),
                    )?;
                }
            }
            Ok(())
        }
    }

    /// Return the retired two-input polynomial expression.
    ///
    /// This remains only while call sites migrate atomically to the pinned
    /// secure Poseidon chip; it must not back a production release.
    #[cfg(test)]
    pub(super) fn poseidon_pair_expression<F>(
        lhs: Expression<F>,
        rhs: Expression<F>,
    ) -> Expression<F>
    where
        F: Field + From<u64>,
    {
        let lhs = lhs + Expression::Constant(F::from(7u64));
        let rhs = rhs + Expression::Constant(F::from(13u64));
        let lhs_sq = lhs.clone() * lhs.clone();
        let lhs_fourth = lhs_sq.clone() * lhs_sq;
        let rhs_sq = rhs.clone() * rhs.clone();
        let rhs_fourth = rhs_sq.clone() * rhs_sq;
        Expression::Constant(F::from(2u64)) * (lhs_fourth * lhs)
            + Expression::Constant(F::from(3u64)) * (rhs_fourth * rhs)
    }

    /// Return the retired note-commitment expression during atomic migration.
    #[cfg(test)]
    pub(super) fn note_commitment_expression<F>(
        amount: Expression<F>,
        rho: Expression<F>,
        owner_tag: Expression<F>,
        asset_tag: Expression<F>,
    ) -> Expression<F>
    where
        F: Field + From<u64>,
    {
        poseidon_pair_expression(
            amount,
            poseidon_pair_expression(rho, poseidon_pair_expression(owner_tag, asset_tag)),
        )
    }

    /// Return the retired nullifier expression during atomic migration.
    #[cfg(test)]
    pub(super) fn nullifier_expression<F>(
        spend_scalar: Expression<F>,
        rho: Expression<F>,
        asset_tag: Expression<F>,
        chain_tag: Expression<F>,
    ) -> Expression<F>
    where
        F: Field + From<u64>,
    {
        poseidon_pair_expression(
            spend_scalar,
            poseidon_pair_expression(rho, poseidon_pair_expression(asset_tag, chain_tag)),
        )
    }

    /// Return the retired direction-selected Merkle-parent expression.
    #[cfg(test)]
    pub(super) fn merkle_parent_expression<F>(
        node: Expression<F>,
        sibling: Expression<F>,
        direction: Expression<F>,
    ) -> Expression<F>
    where
        F: Field + From<u64>,
    {
        let one = Expression::Constant(F::ONE);
        let forward = poseidon_pair_expression(node.clone(), sibling.clone());
        let reverse = poseidon_pair_expression(sibling, node);
        (one - direction.clone()) * forward + direction * reverse
    }
}

/// Secure-permutation confidential relations built entirely in one constrained
/// `halo2-base` execution trace.
///
/// The legacy manual circuits below are intentionally not reused here: mixing
/// their advice cells with a virtual-region hash gadget would leave the bridge
/// between the two regions unconstrained.  Every value consumed by this
/// relation, including public instances, range checks, presence flags, note
/// openings, nullifiers, and Merkle paths, is therefore an `AssignedValue` in
/// the same copy-constraint graph.
#[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
pub(in crate::zk) mod secure_relation_v3 {
    use halo2_base::{
        AssignedValue, QuantumCell,
        gates::{
            GateInstructions, RangeInstructions,
            circuit::{BaseCircuitParams, BaseConfig, builder::BaseCircuitBuilder},
        },
        halo2_proofs::plonk::Assigned,
    };
    use halo2_proofs::{
        circuit::Layouter,
        halo2curves::ff::Field as _,
        plonk::{Circuit, ConstraintSystem, Error as PlonkError},
    };
    use zeroize::Zeroize as _;

    use super::{
        CONFIDENTIAL_POSEIDON_MERKLE_LEAF_DOMAIN_V3, CONFIDENTIAL_POSEIDON_MERKLE_NODE_DOMAIN_V3,
        CONFIDENTIAL_POSEIDON_NOTE_DOMAIN_V3, CONFIDENTIAL_POSEIDON_NULLIFIER_DOMAIN_V3,
        CONFIDENTIAL_POSEIDON_OWNER_DOMAIN_V3, ConfidentialMerklePathV2,
        ConfidentialTransferWitnessV2, ConfidentialUnshieldWitnessV2,
        ConfidentialUnshieldWitnessV3, KagemushaTopUpShieldWitnessV2, Scalar,
        confidential_relation_gadget, scalar_from_repr, scalar_from_u128,
    };

    const MINIMUM_UNUSABLE_ROWS: usize = 9;

    fn canonical_scalar(bytes: [u8; 32], label: &str) -> Result<Scalar, String> {
        scalar_from_repr(bytes).ok_or_else(|| format!("{label} must be a canonical Pasta scalar"))
    }

    fn canonical_nonzero_scalar(bytes: [u8; 32], label: &str) -> Result<Scalar, String> {
        canonical_scalar(bytes, label).and_then(|value| {
            if value == Scalar::ZERO {
                Err(format!("{label} must be non-zero"))
            } else {
                Ok(value)
            }
        })
    }

    fn validate_path<const DEPTH: usize>(
        path: &ConfidentialMerklePathV2,
        label: &str,
    ) -> Result<(), String> {
        if path.siblings.len() != DEPTH
            || path.directions.len() != DEPTH
            || path.witness_nodes.len() != DEPTH
        {
            return Err(format!(
                "{label} must contain exactly {DEPTH} siblings, directions, and witness nodes"
            ));
        }
        for (level, sibling) in path.siblings.iter().copied().enumerate() {
            canonical_scalar(sibling, &format!("{label} sibling[{level}]"))?;
        }
        for (level, direction) in path.directions.iter().copied().enumerate() {
            if direction > 1 {
                return Err(format!("{label} direction[{level}] must be zero or one"));
            }
        }
        for (level, node) in path.witness_nodes.iter().copied().enumerate() {
            canonical_scalar(node, &format!("{label} witness_node[{level}]"))?;
        }
        canonical_scalar(path.root, &format!("{label} root"))?;
        Ok(())
    }

    fn validate_transfer_witness<const DEPTH: usize>(
        witness: &ConfidentialTransferWitnessV2,
    ) -> Result<(), String> {
        if witness.input_0_amount == 0 || witness.output_0_amount == 0 {
            return Err("mandatory transfer amounts must be non-zero".to_owned());
        }
        if witness.input_0_rho == [0; 32] || witness.output_0_rho == [0; 32] {
            return Err("mandatory transfer rho values must be non-zero".to_owned());
        }
        canonical_nonzero_scalar(witness.spend_scalar, "transfer spend scalar")?;
        canonical_nonzero_scalar(witness.input_0_diversifier, "transfer input 0 diversifier")?;
        canonical_nonzero_scalar(witness.output_0_owner_tag, "transfer output 0 owner tag")?;
        canonical_nonzero_scalar(witness.asset_tag, "transfer asset tag")?;
        canonical_nonzero_scalar(witness.chain_tag, "transfer chain tag")?;
        validate_path::<DEPTH>(&witness.input_0_path, "transfer input 0 path")?;
        validate_path::<DEPTH>(&witness.input_1_path, "transfer input 1 path")?;

        if witness.include_input_1 {
            if witness.input_1_amount == 0 || witness.input_1_rho == [0; 32] {
                return Err("present transfer input 1 must have non-zero amount and rho".to_owned());
            }
            canonical_nonzero_scalar(witness.input_1_diversifier, "transfer input 1 diversifier")?;
        } else if witness.input_1_amount != 0
            || witness.input_1_rho != [0; 32]
            || witness.input_1_diversifier != [0; 32]
        {
            return Err(
                "absent transfer input 1 opening must use the canonical all-zero form".to_owned(),
            );
        }

        if witness.include_output_1 {
            if witness.output_1_amount == 0 || witness.output_1_rho == [0; 32] {
                return Err(
                    "present transfer output 1 must have non-zero amount and rho".to_owned(),
                );
            }
            canonical_nonzero_scalar(witness.output_1_owner_tag, "transfer output 1 owner tag")?;
        } else if witness.output_1_amount != 0
            || witness.output_1_rho != [0; 32]
            || witness.output_1_owner_tag != [0; 32]
        {
            return Err(
                "absent transfer output 1 opening must use the canonical all-zero form".to_owned(),
            );
        }
        Ok(())
    }

    fn validate_topup_witness<const DEPTH: usize>(
        witness: &KagemushaTopUpShieldWitnessV2,
    ) -> Result<(), String> {
        if witness.amount == 0 {
            return Err("Kagemusha top-up amount must be non-zero".to_owned());
        }
        if witness.rho == [0; 32] {
            return Err("Kagemusha top-up rho must be non-zero".to_owned());
        }
        if DEPTH >= u32::BITS as usize || u64::from(witness.leaf_index) >= (1u64 << DEPTH) {
            return Err(format!(
                "Kagemusha top-up leaf index must fit the {DEPTH}-bit tree"
            ));
        }
        for (bytes, label) in [
            (witness.spend_scalar, "Kagemusha top-up spend scalar"),
            (witness.diversifier, "Kagemusha top-up diversifier"),
            (witness.asset_tag, "Kagemusha top-up asset tag"),
            (witness.chain_tag, "Kagemusha top-up chain tag"),
            (witness.payer_tag, "Kagemusha top-up payer tag"),
            (witness.operation_tag, "Kagemusha top-up operation tag"),
        ] {
            canonical_nonzero_scalar(bytes, label)?;
        }
        validate_path::<DEPTH>(&witness.zero_path, "Kagemusha top-up empty-leaf path")?;
        if witness.output_nodes.len() != DEPTH {
            return Err(format!(
                "Kagemusha top-up must carry exactly {DEPTH} output path nodes"
            ));
        }
        for (level, node) in witness.output_nodes.iter().copied().enumerate() {
            canonical_scalar(node, &format!("Kagemusha top-up output_node[{level}]"))?;
        }
        Ok(())
    }

    fn validate_unshield_inputs<const DEPTH: usize>(
        include_input_1: bool,
        input_amounts: [u128; 2],
        input_rhos: [[u8; 32]; 2],
        spend_scalar: [u8; 32],
        diversifiers: [[u8; 32]; 2],
        asset_tag: [u8; 32],
        chain_tag: [u8; 32],
        paths: [&ConfidentialMerklePathV2; 2],
    ) -> Result<(), String> {
        if input_amounts[0] == 0 || input_rhos[0] == [0; 32] {
            return Err("mandatory unshield input must have non-zero amount and rho".to_owned());
        }
        canonical_nonzero_scalar(spend_scalar, "unshield spend scalar")?;
        canonical_nonzero_scalar(diversifiers[0], "unshield input 0 diversifier")?;
        canonical_nonzero_scalar(asset_tag, "unshield asset tag")?;
        canonical_nonzero_scalar(chain_tag, "unshield chain tag")?;
        validate_path::<DEPTH>(paths[0], "unshield input 0 path")?;
        validate_path::<DEPTH>(paths[1], "unshield input 1 path")?;
        if include_input_1 {
            if input_amounts[1] == 0 || input_rhos[1] == [0; 32] {
                return Err("present unshield input 1 must have non-zero amount and rho".to_owned());
            }
            canonical_nonzero_scalar(diversifiers[1], "unshield input 1 diversifier")?;
        } else if input_amounts[1] != 0 || input_rhos[1] != [0; 32] || diversifiers[1] != [0; 32] {
            return Err(
                "absent unshield input 1 opening must use the canonical all-zero form".to_owned(),
            );
        }
        Ok(())
    }

    fn validate_unshield_v2_witness<const DEPTH: usize>(
        witness: &ConfidentialUnshieldWitnessV2,
    ) -> Result<(), String> {
        validate_unshield_inputs::<DEPTH>(
            witness.include_input_1,
            [witness.input_0_amount, witness.input_1_amount],
            [witness.input_0_rho, witness.input_1_rho],
            witness.spend_scalar,
            [witness.input_0_diversifier, witness.input_1_diversifier],
            witness.asset_tag,
            witness.chain_tag,
            [&witness.input_0_path, &witness.input_1_path],
        )
    }

    fn validate_unshield_v3_witness<const DEPTH: usize>(
        witness: &ConfidentialUnshieldWitnessV3,
    ) -> Result<(), String> {
        validate_unshield_inputs::<DEPTH>(
            witness.include_input_1,
            [witness.input_0_amount, witness.input_1_amount],
            [witness.input_0_rho, witness.input_1_rho],
            witness.spend_scalar,
            [witness.input_0_diversifier, witness.input_1_diversifier],
            witness.asset_tag,
            witness.chain_tag,
            [&witness.input_0_path, &witness.input_1_path],
        )?;
        if !witness.include_output_0 {
            return Err("change-unshield relation requires one private change output".to_owned());
        }
        if witness.output_0_amount == 0 || witness.output_0_rho == [0; 32] {
            return Err("change-unshield output must have non-zero amount and rho".to_owned());
        }
        Ok(())
    }

    fn assert_equal(
        ctx: &mut halo2_base::Context<Scalar>,
        range: &halo2_base::gates::RangeChip<Scalar>,
        lhs: AssignedValue<Scalar>,
        rhs: impl Into<QuantumCell<Scalar>>,
    ) {
        let difference = range.gate().sub(ctx, lhs, rhs);
        range
            .gate()
            .assert_is_const(ctx, &difference, &Scalar::ZERO);
    }

    fn assert_nonzero(
        ctx: &mut halo2_base::Context<Scalar>,
        range: &halo2_base::gates::RangeChip<Scalar>,
        value: AssignedValue<Scalar>,
    ) {
        let is_zero = range.gate().is_zero(ctx, value);
        range.gate().assert_is_const(ctx, &is_zero, &Scalar::ZERO);
    }

    fn constrain_optional_nonzero(
        ctx: &mut halo2_base::Context<Scalar>,
        range: &halo2_base::gates::RangeChip<Scalar>,
        value: AssignedValue<Scalar>,
        present: AssignedValue<Scalar>,
    ) {
        let is_zero = range.gate().is_zero(ctx, value);
        let absent = range.gate().not(ctx, present);
        assert_equal(ctx, range, is_zero, absent);
    }

    fn note_hash(
        ctx: &mut halo2_base::Context<Scalar>,
        range: &halo2_base::gates::RangeChip<Scalar>,
        poseidon: &confidential_relation_gadget::ConfidentialPoseidonChipV3<Scalar>,
        amount: AssignedValue<Scalar>,
        rho: AssignedValue<Scalar>,
        owner: AssignedValue<Scalar>,
        asset: AssignedValue<Scalar>,
    ) -> AssignedValue<Scalar> {
        poseidon.hash(
            ctx,
            range,
            CONFIDENTIAL_POSEIDON_NOTE_DOMAIN_V3,
            &[amount, rho, owner, asset],
        )
    }

    fn nullifier_hash(
        ctx: &mut halo2_base::Context<Scalar>,
        range: &halo2_base::gates::RangeChip<Scalar>,
        poseidon: &confidential_relation_gadget::ConfidentialPoseidonChipV3<Scalar>,
        spend: AssignedValue<Scalar>,
        rho: AssignedValue<Scalar>,
        asset: AssignedValue<Scalar>,
        chain: AssignedValue<Scalar>,
    ) -> AssignedValue<Scalar> {
        poseidon.hash(
            ctx,
            range,
            CONFIDENTIAL_POSEIDON_NULLIFIER_DOMAIN_V3,
            &[spend, rho, asset, chain],
        )
    }

    fn merkle_root<const DEPTH: usize>(
        ctx: &mut halo2_base::Context<Scalar>,
        range: &halo2_base::gates::RangeChip<Scalar>,
        poseidon: &confidential_relation_gadget::ConfidentialPoseidonChipV3<Scalar>,
        commitment: AssignedValue<Scalar>,
        path: Option<&ConfidentialMerklePathV2>,
    ) -> AssignedValue<Scalar> {
        let mut node = poseidon.hash(
            ctx,
            range,
            CONFIDENTIAL_POSEIDON_MERKLE_LEAF_DOMAIN_V3,
            &[commitment],
        );
        for level in 0..DEPTH {
            let sibling = ctx.load_witness(
                path.and_then(|value| value.siblings.get(level).copied())
                    .map_or(Scalar::ZERO, |bytes| {
                        canonical_scalar(bytes, "validated Merkle sibling")
                            .expect("validated Merkle sibling")
                    }),
            );
            let direction = ctx.load_witness(Scalar::from(u64::from(
                path.and_then(|value| value.directions.get(level).copied())
                    .unwrap_or(0),
            )));
            range.gate().assert_bit(ctx, direction);
            let left = range.gate().select(ctx, sibling, node, direction);
            let right = range.gate().select(ctx, node, sibling, direction);
            node = poseidon.hash(
                ctx,
                range,
                CONFIDENTIAL_POSEIDON_MERKLE_NODE_DOMAIN_V3,
                &[left, right],
            );
            let carried_node = ctx.load_witness(
                path.and_then(|value| value.witness_nodes.get(level).copied())
                    .map_or(Scalar::ZERO, |bytes| {
                        canonical_scalar(bytes, "validated Merkle witness node")
                            .expect("validated Merkle witness node")
                    }),
            );
            assert_equal(ctx, range, node, carried_node);
        }
        let carried_root = ctx.load_witness(path.map_or(Scalar::ZERO, |value| {
            canonical_scalar(value.root, "validated Merkle root").expect("validated Merkle root")
        }));
        assert_equal(ctx, range, node, carried_root);
        node
    }

    fn wipe_builder(builder: &mut BaseCircuitBuilder<Scalar>) {
        for phase in &mut builder.core_mut().phase_manager {
            for context in &mut phase.threads {
                for value in &mut context.advice {
                    *value = Assigned::Trivial(Scalar::ZERO);
                }
            }
        }
        for column in &mut builder.assigned_instances {
            for value in column {
                value.value = Assigned::Trivial(Scalar::ZERO);
            }
        }
        builder.clear();
    }

    fn transfer_builder<const DEPTH: usize>(
        witness: Option<&ConfidentialTransferWitnessV2>,
        k: usize,
    ) -> Result<BaseCircuitBuilder<Scalar>, String> {
        if let Some(witness) = witness {
            validate_transfer_witness::<DEPTH>(witness)?;
        }
        let mut builder = BaseCircuitBuilder::new(false)
            .use_k(k)
            .use_lookup_bits(k - 1)
            .use_instance_columns(9);
        let range = builder.range_chip();
        let ctx = builder.main(0);
        let gate = range.gate();

        let present_input_1 =
            ctx.load_witness(if witness.is_some_and(|value| value.include_input_1) {
                Scalar::ONE
            } else {
                Scalar::ZERO
            });
        let present_output_1 =
            ctx.load_witness(if witness.is_some_and(|value| value.include_output_1) {
                Scalar::ONE
            } else {
                Scalar::ZERO
            });
        gate.assert_bit(ctx, present_input_1);
        gate.assert_bit(ctx, present_output_1);

        let amounts = [
            witness.map_or(0, |value| value.input_0_amount),
            witness.map_or(0, |value| value.input_1_amount),
            witness.map_or(0, |value| value.output_0_amount),
            witness.map_or(0, |value| value.output_1_amount),
        ]
        .map(|amount| ctx.load_witness(scalar_from_u128(amount)));
        for amount in amounts {
            range.range_check(ctx, amount, 128);
        }
        assert_nonzero(ctx, &range, amounts[0]);
        constrain_optional_nonzero(ctx, &range, amounts[1], present_input_1);
        assert_nonzero(ctx, &range, amounts[2]);
        constrain_optional_nonzero(ctx, &range, amounts[3], present_output_1);
        let input_sum = gate.add(ctx, amounts[0], amounts[1]);
        let output_sum = gate.add(ctx, amounts[2], amounts[3]);
        assert_equal(ctx, &range, input_sum, output_sum);

        let rho_bytes = [
            witness.map_or([0; 32], |value| value.input_0_rho),
            witness.map_or([0; 32], |value| value.input_1_rho),
            witness.map_or([0; 32], |value| value.output_0_rho),
            witness.map_or([0; 32], |value| value.output_1_rho),
        ];
        let rho_present = [
            witness.is_some(),
            witness.is_some_and(|value| value.include_input_1),
            witness.is_some(),
            witness.is_some_and(|value| value.include_output_1),
        ];
        let rho: [AssignedValue<Scalar>; 4] = std::array::from_fn(|index| {
            ctx.load_witness(if rho_present[index] {
                super::hash_to_scalar(b"iroha.confidential.v3.note_rho", &[&rho_bytes[index]])
            } else {
                Scalar::ZERO
            })
        });
        for (rho, present) in rho.iter().copied().zip(rho_present) {
            if present {
                assert_nonzero(ctx, &range, rho);
            }
        }
        let spend = ctx.load_witness(match witness {
            Some(value) => canonical_nonzero_scalar(value.spend_scalar, "transfer spend scalar")
                .expect("validated transfer spend scalar"),
            None => Scalar::ZERO,
        });
        let diversifiers = [
            witness.map_or([0; 32], |value| value.input_0_diversifier),
            witness.map_or([0; 32], |value| value.input_1_diversifier),
        ]
        .map(|value| {
            ctx.load_witness(
                canonical_scalar(value, "validated transfer diversifier")
                    .expect("validated transfer diversifier"),
            )
        });
        let output_owners = [
            witness.map_or([0; 32], |value| value.output_0_owner_tag),
            witness.map_or([0; 32], |value| value.output_1_owner_tag),
        ]
        .map(|value| {
            ctx.load_witness(
                canonical_scalar(value, "validated transfer owner tag")
                    .expect("validated transfer owner tag"),
            )
        });
        let asset = ctx.load_witness(match witness {
            Some(value) => canonical_nonzero_scalar(value.asset_tag, "transfer asset tag")
                .expect("validated transfer asset tag"),
            None => Scalar::ZERO,
        });
        let chain = ctx.load_witness(match witness {
            Some(value) => canonical_nonzero_scalar(value.chain_tag, "transfer chain tag")
                .expect("validated transfer chain tag"),
            None => Scalar::ZERO,
        });
        if witness.is_some() {
            for value in [spend, diversifiers[0], output_owners[0], asset, chain] {
                assert_nonzero(ctx, &range, value);
            }
        }

        let poseidon = confidential_relation_gadget::ConfidentialPoseidonChipV3::new(ctx, &range);
        let input_owners = diversifiers.map(|diversifier| {
            poseidon.hash(
                ctx,
                &range,
                CONFIDENTIAL_POSEIDON_OWNER_DOMAIN_V3,
                &[spend, diversifier],
            )
        });
        let commitments = [
            note_hash(
                ctx,
                &range,
                &poseidon,
                amounts[0],
                rho[0],
                input_owners[0],
                asset,
            ),
            note_hash(
                ctx,
                &range,
                &poseidon,
                amounts[1],
                rho[1],
                input_owners[1],
                asset,
            ),
            note_hash(
                ctx,
                &range,
                &poseidon,
                amounts[2],
                rho[2],
                output_owners[0],
                asset,
            ),
            note_hash(
                ctx,
                &range,
                &poseidon,
                amounts[3],
                rho[3],
                output_owners[1],
                asset,
            ),
        ];
        let nullifiers = [
            nullifier_hash(ctx, &range, &poseidon, spend, rho[0], asset, chain),
            nullifier_hash(ctx, &range, &poseidon, spend, rho[1], asset, chain),
        ];
        for value in [commitments[0], commitments[2], nullifiers[0]] {
            assert_nonzero(ctx, &range, value);
        }

        let duplicate_nullifier = gate.is_equal(ctx, nullifiers[0], nullifiers[1]);
        let selected_duplicate = gate.mul(ctx, present_input_1, duplicate_nullifier);
        gate.assert_is_const(ctx, &selected_duplicate, &Scalar::ZERO);
        let duplicate_output = gate.is_equal(ctx, commitments[2], commitments[3]);
        let selected_duplicate = gate.mul(ctx, present_output_1, duplicate_output);
        gate.assert_is_const(ctx, &selected_duplicate, &Scalar::ZERO);

        let public_input_1 = gate.mul(ctx, present_input_1, commitments[1]);
        let root_0 = merkle_root::<DEPTH>(
            ctx,
            &range,
            &poseidon,
            commitments[0],
            witness.map(|value| &value.input_0_path),
        );
        let root_1 = merkle_root::<DEPTH>(
            ctx,
            &range,
            &poseidon,
            public_input_1,
            witness.map(|value| &value.input_1_path),
        );
        assert_equal(ctx, &range, root_0, root_1);

        let public_nullifier_1 = gate.mul(ctx, present_input_1, nullifiers[1]);
        let public_output_1 = gate.mul(ctx, present_output_1, commitments[3]);
        builder.assigned_instances = vec![
            vec![commitments[0]],
            vec![public_input_1],
            vec![nullifiers[0]],
            vec![public_nullifier_1],
            vec![commitments[2]],
            vec![public_output_1],
            vec![root_0],
            vec![asset],
            vec![chain],
        ];
        builder.calculate_params(Some(MINIMUM_UNUSABLE_ROWS));
        Ok(builder)
    }

    fn topup_builder<const DEPTH: usize>(
        witness: Option<&KagemushaTopUpShieldWitnessV2>,
        k: usize,
    ) -> Result<BaseCircuitBuilder<Scalar>, String> {
        if let Some(witness) = witness {
            validate_topup_witness::<DEPTH>(witness)?;
        }
        let mut builder = BaseCircuitBuilder::new(false)
            .use_k(k)
            .use_lookup_bits(k - 1)
            .use_instance_columns(11);
        let range = builder.range_chip();
        let ctx = builder.main(0);
        let gate = range.gate();

        let amount = ctx.load_witness(scalar_from_u128(witness.map_or(0, |value| value.amount)));
        range.range_check(ctx, amount, 128);
        assert_nonzero(ctx, &range, amount);
        let scale = ctx.load_witness(Scalar::from(u64::from(
            witness.map_or(0, |value| value.asset_scale),
        )));
        range.range_check(ctx, scale, 32);
        let leaf_index = ctx.load_witness(Scalar::from(u64::from(
            witness.map_or(0, |value| value.leaf_index),
        )));
        range.range_check(ctx, leaf_index, DEPTH);
        let index_bits = gate.num_to_bits(ctx, leaf_index, DEPTH);

        let rho = ctx.load_witness(if let Some(witness) = witness {
            super::hash_to_scalar(b"iroha.confidential.v3.note_rho", &[&witness.rho])
        } else {
            Scalar::ZERO
        });
        let decode = |bytes, label| match witness {
            Some(_) => canonical_nonzero_scalar(bytes, label).expect("validated top-up scalar"),
            None => Scalar::ZERO,
        };
        let spend = ctx.load_witness(decode(
            witness.map_or([0; 32], |value| value.spend_scalar),
            "Kagemusha top-up spend scalar",
        ));
        let diversifier = ctx.load_witness(decode(
            witness.map_or([0; 32], |value| value.diversifier),
            "Kagemusha top-up diversifier",
        ));
        let asset = ctx.load_witness(decode(
            witness.map_or([0; 32], |value| value.asset_tag),
            "Kagemusha top-up asset tag",
        ));
        let chain = ctx.load_witness(decode(
            witness.map_or([0; 32], |value| value.chain_tag),
            "Kagemusha top-up chain tag",
        ));
        let payer = ctx.load_witness(decode(
            witness.map_or([0; 32], |value| value.payer_tag),
            "Kagemusha top-up payer tag",
        ));
        let operation = ctx.load_witness(decode(
            witness.map_or([0; 32], |value| value.operation_tag),
            "Kagemusha top-up operation tag",
        ));
        if witness.is_some() {
            for value in [rho, spend, diversifier, asset, chain, payer, operation] {
                assert_nonzero(ctx, &range, value);
            }
        }

        let poseidon = confidential_relation_gadget::ConfidentialPoseidonChipV3::new(ctx, &range);
        let owner = poseidon.hash(
            ctx,
            &range,
            CONFIDENTIAL_POSEIDON_OWNER_DOMAIN_V3,
            &[spend, diversifier],
        );
        let output_commitment = note_hash(ctx, &range, &poseidon, amount, rho, owner, asset);
        let spend_nullifier = nullifier_hash(ctx, &range, &poseidon, spend, rho, asset, chain);
        for value in [output_commitment, spend_nullifier] {
            assert_nonzero(ctx, &range, value);
        }
        let note_nullifier_equal = gate.is_equal(ctx, output_commitment, spend_nullifier);
        gate.assert_is_const(ctx, &note_nullifier_equal, &Scalar::ZERO);

        let zero = ctx.load_zero();
        let mut initial_node = poseidon.hash(
            ctx,
            &range,
            CONFIDENTIAL_POSEIDON_MERKLE_LEAF_DOMAIN_V3,
            &[zero],
        );
        let mut final_node = poseidon.hash(
            ctx,
            &range,
            CONFIDENTIAL_POSEIDON_MERKLE_LEAF_DOMAIN_V3,
            &[output_commitment],
        );
        for (level, expected_direction) in index_bits.into_iter().enumerate() {
            let sibling = ctx.load_witness(
                witness
                    .and_then(|value| value.zero_path.siblings.get(level).copied())
                    .map_or(Scalar::ZERO, |bytes| {
                        canonical_scalar(bytes, "validated Kagemusha top-up sibling")
                            .expect("validated Kagemusha top-up sibling")
                    }),
            );
            let direction = ctx.load_witness(Scalar::from(u64::from(
                witness
                    .and_then(|value| value.zero_path.directions.get(level).copied())
                    .unwrap_or(0),
            )));
            gate.assert_bit(ctx, direction);
            assert_equal(ctx, &range, direction, expected_direction);

            let initial_left = gate.select(ctx, sibling, initial_node, direction);
            let initial_right = gate.select(ctx, initial_node, sibling, direction);
            initial_node = poseidon.hash(
                ctx,
                &range,
                CONFIDENTIAL_POSEIDON_MERKLE_NODE_DOMAIN_V3,
                &[initial_left, initial_right],
            );
            let carried_initial = ctx.load_witness(
                witness
                    .and_then(|value| value.zero_path.witness_nodes.get(level).copied())
                    .map_or(Scalar::ZERO, |bytes| {
                        canonical_scalar(bytes, "validated Kagemusha top-up empty node")
                            .expect("validated Kagemusha top-up empty node")
                    }),
            );
            assert_equal(ctx, &range, initial_node, carried_initial);

            let final_left = gate.select(ctx, sibling, final_node, direction);
            let final_right = gate.select(ctx, final_node, sibling, direction);
            final_node = poseidon.hash(
                ctx,
                &range,
                CONFIDENTIAL_POSEIDON_MERKLE_NODE_DOMAIN_V3,
                &[final_left, final_right],
            );
            let carried_final = ctx.load_witness(
                witness
                    .and_then(|value| value.output_nodes.get(level).copied())
                    .map_or(Scalar::ZERO, |bytes| {
                        canonical_scalar(bytes, "validated Kagemusha top-up output node")
                            .expect("validated Kagemusha top-up output node")
                    }),
            );
            assert_equal(ctx, &range, final_node, carried_final);
        }
        let carried_initial_root = ctx.load_witness(witness.map_or(Scalar::ZERO, |value| {
            canonical_scalar(
                value.zero_path.root,
                "validated Kagemusha top-up initial root",
            )
            .expect("validated Kagemusha top-up initial root")
        }));
        assert_equal(ctx, &range, initial_node, carried_initial_root);
        for root in [initial_node, final_node] {
            assert_nonzero(ctx, &range, root);
        }
        let roots_equal = gate.is_equal(ctx, initial_node, final_node);
        gate.assert_is_const(ctx, &roots_equal, &Scalar::ZERO);

        builder.assigned_instances = vec![
            vec![output_commitment],
            vec![spend_nullifier],
            vec![initial_node],
            vec![final_node],
            vec![amount],
            vec![scale],
            vec![leaf_index],
            vec![asset],
            vec![chain],
            vec![payer],
            vec![operation],
        ];
        builder.calculate_params(Some(MINIMUM_UNUSABLE_ROWS));
        Ok(builder)
    }

    #[derive(Clone, Copy)]
    enum UnshieldWitnessRef<'a> {
        Full(Option<&'a ConfidentialUnshieldWitnessV2>),
        Change(Option<&'a ConfidentialUnshieldWitnessV3>),
    }

    fn unshield_builder<const DEPTH: usize>(
        witness: UnshieldWitnessRef<'_>,
        k: usize,
    ) -> Result<BaseCircuitBuilder<Scalar>, String> {
        match witness {
            UnshieldWitnessRef::Full(Some(value)) => {
                validate_unshield_v2_witness::<DEPTH>(value)?;
            }
            UnshieldWitnessRef::Change(Some(value)) => {
                validate_unshield_v3_witness::<DEPTH>(value)?;
            }
            UnshieldWitnessRef::Full(None) | UnshieldWitnessRef::Change(None) => {}
        }
        let is_change = matches!(witness, UnshieldWitnessRef::Change(_));
        let instance_count = if is_change { 9 } else { 8 };
        let mut builder = BaseCircuitBuilder::new(false)
            .use_k(k)
            .use_lookup_bits(k - 1)
            .use_instance_columns(instance_count);
        let range = builder.range_chip();
        let ctx = builder.main(0);
        let gate = range.gate();

        let include_input_1 = match witness {
            UnshieldWitnessRef::Full(Some(value)) => value.include_input_1,
            UnshieldWitnessRef::Change(Some(value)) => value.include_input_1,
            UnshieldWitnessRef::Full(None) | UnshieldWitnessRef::Change(None) => false,
        };
        let present_input_1 = ctx.load_witness(if include_input_1 {
            Scalar::ONE
        } else {
            Scalar::ZERO
        });
        gate.assert_bit(ctx, present_input_1);
        let input_amounts_u128 = match witness {
            UnshieldWitnessRef::Full(Some(value)) => [value.input_0_amount, value.input_1_amount],
            UnshieldWitnessRef::Change(Some(value)) => [value.input_0_amount, value.input_1_amount],
            UnshieldWitnessRef::Full(None) | UnshieldWitnessRef::Change(None) => [0; 2],
        };
        let input_amounts =
            input_amounts_u128.map(|amount| ctx.load_witness(scalar_from_u128(amount)));
        for amount in input_amounts {
            range.range_check(ctx, amount, 128);
        }
        assert_nonzero(ctx, &range, input_amounts[0]);
        constrain_optional_nonzero(ctx, &range, input_amounts[1], present_input_1);

        let input_rho_bytes = match witness {
            UnshieldWitnessRef::Full(Some(value)) => [value.input_0_rho, value.input_1_rho],
            UnshieldWitnessRef::Change(Some(value)) => [value.input_0_rho, value.input_1_rho],
            UnshieldWitnessRef::Full(None) | UnshieldWitnessRef::Change(None) => [[0; 32]; 2],
        };
        let input_rho: [AssignedValue<Scalar>; 2] = std::array::from_fn(|index| {
            ctx.load_witness(if index == 0 || include_input_1 {
                super::hash_to_scalar(
                    b"iroha.confidential.v3.note_rho",
                    &[&input_rho_bytes[index]],
                )
            } else {
                Scalar::ZERO
            })
        });
        if !matches!(
            witness,
            UnshieldWitnessRef::Full(None) | UnshieldWitnessRef::Change(None)
        ) {
            assert_nonzero(ctx, &range, input_rho[0]);
            if include_input_1 {
                assert_nonzero(ctx, &range, input_rho[1]);
            }
        }

        let (spend_bytes, diversifier_bytes, asset_bytes, chain_bytes) = match witness {
            UnshieldWitnessRef::Full(Some(value)) => (
                value.spend_scalar,
                [value.input_0_diversifier, value.input_1_diversifier],
                value.asset_tag,
                value.chain_tag,
            ),
            UnshieldWitnessRef::Change(Some(value)) => (
                value.spend_scalar,
                [value.input_0_diversifier, value.input_1_diversifier],
                value.asset_tag,
                value.chain_tag,
            ),
            UnshieldWitnessRef::Full(None) | UnshieldWitnessRef::Change(None) => {
                ([0; 32], [[0; 32]; 2], [0; 32], [0; 32])
            }
        };
        let decode = |bytes, label| {
            if matches!(
                witness,
                UnshieldWitnessRef::Full(None) | UnshieldWitnessRef::Change(None)
            ) {
                Scalar::ZERO
            } else {
                canonical_scalar(bytes, label).expect("validated unshield scalar")
            }
        };
        let spend = ctx.load_witness(decode(spend_bytes, "validated unshield spend scalar"));
        let diversifiers = diversifier_bytes
            .map(|bytes| ctx.load_witness(decode(bytes, "validated unshield diversifier")));
        let asset = ctx.load_witness(decode(asset_bytes, "validated unshield asset tag"));
        let chain = ctx.load_witness(decode(chain_bytes, "validated unshield chain tag"));
        if !matches!(
            witness,
            UnshieldWitnessRef::Full(None) | UnshieldWitnessRef::Change(None)
        ) {
            for value in [spend, diversifiers[0], asset, chain] {
                assert_nonzero(ctx, &range, value);
            }
        }

        let poseidon = confidential_relation_gadget::ConfidentialPoseidonChipV3::new(ctx, &range);
        let input_owners = diversifiers.map(|diversifier| {
            poseidon.hash(
                ctx,
                &range,
                CONFIDENTIAL_POSEIDON_OWNER_DOMAIN_V3,
                &[spend, diversifier],
            )
        });
        let input_commitments = [
            note_hash(
                ctx,
                &range,
                &poseidon,
                input_amounts[0],
                input_rho[0],
                input_owners[0],
                asset,
            ),
            note_hash(
                ctx,
                &range,
                &poseidon,
                input_amounts[1],
                input_rho[1],
                input_owners[1],
                asset,
            ),
        ];
        let nullifiers = [
            nullifier_hash(ctx, &range, &poseidon, spend, input_rho[0], asset, chain),
            nullifier_hash(ctx, &range, &poseidon, spend, input_rho[1], asset, chain),
        ];
        for value in [input_commitments[0], nullifiers[0]] {
            assert_nonzero(ctx, &range, value);
        }
        let duplicate_nullifier = gate.is_equal(ctx, nullifiers[0], nullifiers[1]);
        let selected_duplicate = gate.mul(ctx, present_input_1, duplicate_nullifier);
        gate.assert_is_const(ctx, &selected_duplicate, &Scalar::ZERO);

        let public_input_1 = gate.mul(ctx, present_input_1, input_commitments[1]);
        let paths = match witness {
            UnshieldWitnessRef::Full(Some(value)) => {
                [Some(&value.input_0_path), Some(&value.input_1_path)]
            }
            UnshieldWitnessRef::Change(Some(value)) => {
                [Some(&value.input_0_path), Some(&value.input_1_path)]
            }
            UnshieldWitnessRef::Full(None) | UnshieldWitnessRef::Change(None) => [None, None],
        };
        let root_0 = merkle_root::<DEPTH>(ctx, &range, &poseidon, input_commitments[0], paths[0]);
        let root_1 = merkle_root::<DEPTH>(ctx, &range, &poseidon, public_input_1, paths[1]);
        assert_equal(ctx, &range, root_0, root_1);
        let public_nullifier_1 = gate.mul(ctx, present_input_1, nullifiers[1]);
        let input_sum = gate.add(ctx, input_amounts[0], input_amounts[1]);

        let mut public = vec![
            vec![input_commitments[0]],
            vec![public_input_1],
            vec![nullifiers[0]],
            vec![public_nullifier_1],
        ];
        if let UnshieldWitnessRef::Change(change_witness) = witness {
            let output_amount_u128 = change_witness.map_or(0, |value| value.output_0_amount);
            let output_amount = ctx.load_witness(scalar_from_u128(output_amount_u128));
            range.range_check(ctx, output_amount, 128);
            assert_nonzero(ctx, &range, output_amount);
            let output_rho_bytes = change_witness.map_or([0; 32], |value| value.output_0_rho);
            let output_rho = ctx.load_witness(if change_witness.is_some() {
                super::hash_to_scalar(b"iroha.confidential.v3.note_rho", &[&output_rho_bytes])
            } else {
                Scalar::ZERO
            });
            if change_witness.is_some() {
                assert_nonzero(ctx, &range, output_rho);
            }
            let one = ctx.load_constant(Scalar::ONE);
            let output_owner = poseidon.hash(
                ctx,
                &range,
                CONFIDENTIAL_POSEIDON_OWNER_DOMAIN_V3,
                &[spend, one],
            );
            let change_commitment = note_hash(
                ctx,
                &range,
                &poseidon,
                output_amount,
                output_rho,
                output_owner,
                asset,
            );
            assert_nonzero(ctx, &range, change_commitment);
            for input in input_commitments {
                let equal = gate.is_equal(ctx, change_commitment, input);
                gate.assert_is_const(ctx, &equal, &Scalar::ZERO);
            }
            let public_amount = gate.sub(ctx, input_sum, output_amount);
            range.range_check(ctx, public_amount, 128);
            assert_nonzero(ctx, &range, public_amount);
            public.push(vec![change_commitment]);
            public.push(vec![root_0]);
            public.push(vec![public_amount]);
            public.push(vec![asset]);
            public.push(vec![chain]);
        } else {
            range.range_check(ctx, input_sum, 128);
            assert_nonzero(ctx, &range, input_sum);
            public.push(vec![root_0]);
            public.push(vec![input_sum]);
            public.push(vec![asset]);
            public.push(vec![chain]);
        }
        builder.assigned_instances = public;
        builder.calculate_params(Some(MINIMUM_UNUSABLE_ROWS));
        Ok(builder)
    }

    /// Fixed-shape transfer relation using the full secure permutation.
    #[derive(Clone, Default)]
    pub(in crate::zk) struct ConfidentialTransferCircuitV3<const DEPTH: usize> {
        pub(super) witness: Option<ConfidentialTransferWitnessV2>,
    }

    impl<const DEPTH: usize> zeroize::Zeroize for ConfidentialTransferCircuitV3<DEPTH> {
        fn zeroize(&mut self) {
            if let Some(witness) = &mut self.witness {
                witness.zeroize();
            }
            self.witness = None;
        }
    }

    impl<const DEPTH: usize> Drop for ConfidentialTransferCircuitV3<DEPTH> {
        fn drop(&mut self) {
            self.zeroize();
        }
    }

    impl<const DEPTH: usize> Circuit<Scalar> for ConfidentialTransferCircuitV3<DEPTH> {
        type Config = BaseConfig<Scalar>;
        type FloorPlanner = halo2_proofs::circuit::SimpleFloorPlanner;
        type Params = ();

        fn without_witnesses(&self) -> Self {
            Self::default()
        }

        fn configure(meta: &mut ConstraintSystem<Scalar>) -> Self::Config {
            let params: BaseCircuitParams =
                transfer_builder::<DEPTH>(None, super::CONFIDENTIAL_TRANSFER_V2_IPA_K as usize)
                    .expect("witness-free transfer relation must have a valid fixed shape")
                    .config_params;
            BaseConfig::configure(meta, params)
        }

        fn synthesize(
            &self,
            config: Self::Config,
            layouter: impl Layouter<Scalar>,
        ) -> Result<(), PlonkError> {
            let mut builder = match transfer_builder::<DEPTH>(
                self.witness.as_ref(),
                super::CONFIDENTIAL_TRANSFER_V2_IPA_K as usize,
            ) {
                Ok(builder) => builder,
                Err(_) => return Err(PlonkError::Synthesis),
            };
            let result = <BaseCircuitBuilder<Scalar> as Circuit<Scalar>>::synthesize(
                &builder, config, layouter,
            );
            wipe_builder(&mut builder);
            result
        }
    }

    /// Fixed-shape Kagemusha top-up relation using the full secure permutation.
    #[derive(Clone, Default)]
    pub(in crate::zk) struct KagemushaTopUpShieldCircuitV3<const DEPTH: usize> {
        pub(super) witness: Option<KagemushaTopUpShieldWitnessV2>,
    }

    impl<const DEPTH: usize> zeroize::Zeroize for KagemushaTopUpShieldCircuitV3<DEPTH> {
        fn zeroize(&mut self) {
            if let Some(witness) = &mut self.witness {
                witness.zeroize();
            }
            self.witness = None;
        }
    }

    impl<const DEPTH: usize> Drop for KagemushaTopUpShieldCircuitV3<DEPTH> {
        fn drop(&mut self) {
            self.zeroize();
        }
    }

    impl<const DEPTH: usize> Circuit<Scalar> for KagemushaTopUpShieldCircuitV3<DEPTH> {
        type Config = BaseConfig<Scalar>;
        type FloorPlanner = halo2_proofs::circuit::SimpleFloorPlanner;
        type Params = ();

        fn without_witnesses(&self) -> Self {
            Self::default()
        }

        fn configure(meta: &mut ConstraintSystem<Scalar>) -> Self::Config {
            let params =
                topup_builder::<DEPTH>(None, super::KAGEMUSHA_TOPUP_SHIELD_V2_IPA_K as usize)
                    .expect("witness-free top-up relation must have a valid fixed shape")
                    .config_params;
            BaseConfig::configure(meta, params)
        }

        fn synthesize(
            &self,
            config: Self::Config,
            layouter: impl Layouter<Scalar>,
        ) -> Result<(), PlonkError> {
            let mut builder = match topup_builder::<DEPTH>(
                self.witness.as_ref(),
                super::KAGEMUSHA_TOPUP_SHIELD_V2_IPA_K as usize,
            ) {
                Ok(builder) => builder,
                Err(_) => return Err(PlonkError::Synthesis),
            };
            let result = <BaseCircuitBuilder<Scalar> as Circuit<Scalar>>::synthesize(
                &builder, config, layouter,
            );
            wipe_builder(&mut builder);
            result
        }
    }

    /// Fixed-shape complete-unshield relation using the full secure permutation.
    #[derive(Clone, Default)]
    pub(in crate::zk) struct ConfidentialUnshieldFullCircuitV3<const DEPTH: usize> {
        pub(super) witness: Option<ConfidentialUnshieldWitnessV2>,
    }

    impl<const DEPTH: usize> zeroize::Zeroize for ConfidentialUnshieldFullCircuitV3<DEPTH> {
        fn zeroize(&mut self) {
            if let Some(witness) = &mut self.witness {
                witness.zeroize();
            }
            self.witness = None;
        }
    }

    impl<const DEPTH: usize> Drop for ConfidentialUnshieldFullCircuitV3<DEPTH> {
        fn drop(&mut self) {
            self.zeroize();
        }
    }

    impl<const DEPTH: usize> Circuit<Scalar> for ConfidentialUnshieldFullCircuitV3<DEPTH> {
        type Config = BaseConfig<Scalar>;
        type FloorPlanner = halo2_proofs::circuit::SimpleFloorPlanner;
        type Params = ();

        fn without_witnesses(&self) -> Self {
            Self::default()
        }

        fn configure(meta: &mut ConstraintSystem<Scalar>) -> Self::Config {
            let params = unshield_builder::<DEPTH>(
                UnshieldWitnessRef::Full(None),
                super::CONFIDENTIAL_UNSHIELD_V2_IPA_K as usize,
            )
            .expect("witness-free full-unshield relation must have a valid fixed shape")
            .config_params;
            BaseConfig::configure(meta, params)
        }

        fn synthesize(
            &self,
            config: Self::Config,
            layouter: impl Layouter<Scalar>,
        ) -> Result<(), PlonkError> {
            let mut builder = match unshield_builder::<DEPTH>(
                UnshieldWitnessRef::Full(self.witness.as_ref()),
                super::CONFIDENTIAL_UNSHIELD_V2_IPA_K as usize,
            ) {
                Ok(builder) => builder,
                Err(_) => return Err(PlonkError::Synthesis),
            };
            let result = <BaseCircuitBuilder<Scalar> as Circuit<Scalar>>::synthesize(
                &builder, config, layouter,
            );
            wipe_builder(&mut builder);
            result
        }
    }

    /// Fixed-shape change-unshield relation using the full secure permutation.
    #[derive(Clone, Default)]
    pub(in crate::zk) struct ConfidentialUnshieldChangeCircuitV4<const DEPTH: usize> {
        pub(super) witness: Option<ConfidentialUnshieldWitnessV3>,
    }

    impl<const DEPTH: usize> zeroize::Zeroize for ConfidentialUnshieldChangeCircuitV4<DEPTH> {
        fn zeroize(&mut self) {
            if let Some(witness) = &mut self.witness {
                witness.zeroize();
            }
            self.witness = None;
        }
    }

    impl<const DEPTH: usize> Drop for ConfidentialUnshieldChangeCircuitV4<DEPTH> {
        fn drop(&mut self) {
            self.zeroize();
        }
    }

    impl<const DEPTH: usize> Circuit<Scalar> for ConfidentialUnshieldChangeCircuitV4<DEPTH> {
        type Config = BaseConfig<Scalar>;
        type FloorPlanner = halo2_proofs::circuit::SimpleFloorPlanner;
        type Params = ();

        fn without_witnesses(&self) -> Self {
            Self::default()
        }

        fn configure(meta: &mut ConstraintSystem<Scalar>) -> Self::Config {
            let params = unshield_builder::<DEPTH>(
                UnshieldWitnessRef::Change(None),
                super::CONFIDENTIAL_UNSHIELD_V3_IPA_K as usize,
            )
            .expect("witness-free change-unshield relation must have a valid fixed shape")
            .config_params;
            BaseConfig::configure(meta, params)
        }

        fn synthesize(
            &self,
            config: Self::Config,
            layouter: impl Layouter<Scalar>,
        ) -> Result<(), PlonkError> {
            let mut builder = match unshield_builder::<DEPTH>(
                UnshieldWitnessRef::Change(self.witness.as_ref()),
                super::CONFIDENTIAL_UNSHIELD_V3_IPA_K as usize,
            ) {
                Ok(builder) => builder,
                Err(_) => return Err(PlonkError::Synthesis),
            };
            let result = <BaseCircuitBuilder<Scalar> as Circuit<Scalar>>::synthesize(
                &builder, config, layouter,
            );
            wipe_builder(&mut builder);
            result
        }
    }

    #[cfg(test)]
    mod tests {
        use halo2_proofs::dev::MockProver;

        use super::*;
        use crate::zk::confidential_v2::{confidential_poseidon_hash_v3, scalar_to_repr_bytes};

        fn native_hash(domain: u64, inputs: &[Scalar]) -> Scalar {
            confidential_poseidon_hash_v3(domain, inputs)
        }

        fn sample_witness_shape(
            include_input_1: bool,
            include_output_1: bool,
        ) -> ConfidentialTransferWitnessV2 {
            let spend = Scalar::from(41);
            let diversifiers = [
                Scalar::from(43),
                if include_input_1 {
                    Scalar::from(47)
                } else {
                    Scalar::ZERO
                },
            ];
            let input_rho_bytes = [
                [0x11; 32],
                if include_input_1 { [0x22; 32] } else { [0; 32] },
            ];
            let input_rho = [
                super::super::hash_to_scalar(
                    b"iroha.confidential.v3.note_rho",
                    &[&input_rho_bytes[0]],
                ),
                if include_input_1 {
                    super::super::hash_to_scalar(
                        b"iroha.confidential.v3.note_rho",
                        &[&input_rho_bytes[1]],
                    )
                } else {
                    Scalar::ZERO
                },
            ];
            let asset = Scalar::from(53);
            let input_owner = diversifiers.map(|diversifier| {
                native_hash(CONFIDENTIAL_POSEIDON_OWNER_DOMAIN_V3, &[spend, diversifier])
            });
            let input_commitments = [
                native_hash(
                    CONFIDENTIAL_POSEIDON_NOTE_DOMAIN_V3,
                    &[
                        Scalar::from(if include_input_1 { 5u64 } else { 12u64 }),
                        input_rho[0],
                        input_owner[0],
                        asset,
                    ],
                ),
                if include_input_1 {
                    native_hash(
                        CONFIDENTIAL_POSEIDON_NOTE_DOMAIN_V3,
                        &[Scalar::from(7), input_rho[1], input_owner[1], asset],
                    )
                } else {
                    Scalar::ZERO
                },
            ];
            let empty_leaf =
                native_hash(CONFIDENTIAL_POSEIDON_MERKLE_LEAF_DOMAIN_V3, &[Scalar::ZERO]);
            let leaves = [
                native_hash(
                    CONFIDENTIAL_POSEIDON_MERKLE_LEAF_DOMAIN_V3,
                    &[input_commitments[0]],
                ),
                if include_input_1 {
                    native_hash(
                        CONFIDENTIAL_POSEIDON_MERKLE_LEAF_DOMAIN_V3,
                        &[input_commitments[1]],
                    )
                } else {
                    empty_leaf
                },
            ];
            let input_pair = native_hash(
                CONFIDENTIAL_POSEIDON_MERKLE_NODE_DOMAIN_V3,
                &[leaves[0], leaves[1]],
            );
            let empty_pair = native_hash(
                CONFIDENTIAL_POSEIDON_MERKLE_NODE_DOMAIN_V3,
                &[empty_leaf, empty_leaf],
            );
            let root = native_hash(
                CONFIDENTIAL_POSEIDON_MERKLE_NODE_DOMAIN_V3,
                &[input_pair, empty_pair],
            );
            let path_0 = ConfidentialMerklePathV2 {
                siblings: [leaves[1], empty_pair].map(scalar_to_repr_bytes).to_vec(),
                directions: vec![0, 0],
                witness_nodes: [input_pair, root].map(scalar_to_repr_bytes).to_vec(),
                root: scalar_to_repr_bytes(root),
            };
            let path_1 = ConfidentialMerklePathV2 {
                siblings: [leaves[0], empty_pair].map(scalar_to_repr_bytes).to_vec(),
                directions: vec![1, 0],
                witness_nodes: [input_pair, root].map(scalar_to_repr_bytes).to_vec(),
                root: scalar_to_repr_bytes(root),
            };
            ConfidentialTransferWitnessV2 {
                include_input_1,
                include_output_1,
                input_0_amount: if include_input_1 { 5 } else { 12 },
                input_1_amount: if include_input_1 { 7 } else { 0 },
                output_0_amount: if include_output_1 { 8 } else { 12 },
                output_1_amount: if include_output_1 { 4 } else { 0 },
                input_0_rho: input_rho_bytes[0],
                input_1_rho: input_rho_bytes[1],
                output_0_rho: [0x33; 32],
                output_1_rho: if include_output_1 {
                    [0x44; 32]
                } else {
                    [0; 32]
                },
                spend_scalar: scalar_to_repr_bytes(spend),
                input_0_diversifier: scalar_to_repr_bytes(diversifiers[0]),
                input_1_diversifier: scalar_to_repr_bytes(diversifiers[1]),
                output_0_owner_tag: scalar_to_repr_bytes(Scalar::from(59)),
                output_1_owner_tag: scalar_to_repr_bytes(if include_output_1 {
                    Scalar::from(67)
                } else {
                    Scalar::ZERO
                }),
                asset_tag: scalar_to_repr_bytes(asset),
                chain_tag: scalar_to_repr_bytes(Scalar::from(61)),
                input_0_path: path_0,
                input_1_path: path_1,
            }
        }

        fn sample_witness() -> ConfidentialTransferWitnessV2 {
            sample_witness_shape(true, false)
        }

        fn instances(builder: &BaseCircuitBuilder<Scalar>) -> Vec<Vec<Scalar>> {
            builder
                .assigned_instances
                .iter()
                .map(|column| column.iter().map(|value| *value.value()).collect())
                .collect()
        }

        fn expected_instances(witness: &ConfidentialTransferWitnessV2) -> Vec<Vec<Scalar>> {
            let spend = scalar_from_repr(witness.spend_scalar).expect("canonical spend scalar");
            let asset = scalar_from_repr(witness.asset_tag).expect("canonical asset tag");
            let chain = scalar_from_repr(witness.chain_tag).expect("canonical chain tag");
            let amounts = [
                witness.input_0_amount,
                witness.input_1_amount,
                witness.output_0_amount,
                witness.output_1_amount,
            ]
            .map(scalar_from_u128);
            let rho_bytes = [
                witness.input_0_rho,
                witness.input_1_rho,
                witness.output_0_rho,
                witness.output_1_rho,
            ];
            let rho = rho_bytes.map(|rho| {
                super::super::hash_to_scalar(b"iroha.confidential.v3.note_rho", &[&rho])
            });
            let input_owners =
                [witness.input_0_diversifier, witness.input_1_diversifier].map(|bytes| {
                    native_hash(
                        CONFIDENTIAL_POSEIDON_OWNER_DOMAIN_V3,
                        &[
                            spend,
                            scalar_from_repr(bytes).expect("canonical diversifier"),
                        ],
                    )
                });
            let output_owners = [
                scalar_from_repr(witness.output_0_owner_tag).expect("canonical owner tag"),
                scalar_from_repr(witness.output_1_owner_tag).expect("canonical owner tag"),
            ];
            let commitments = [
                native_hash(
                    CONFIDENTIAL_POSEIDON_NOTE_DOMAIN_V3,
                    &[amounts[0], rho[0], input_owners[0], asset],
                ),
                native_hash(
                    CONFIDENTIAL_POSEIDON_NOTE_DOMAIN_V3,
                    &[amounts[1], rho[1], input_owners[1], asset],
                ),
                native_hash(
                    CONFIDENTIAL_POSEIDON_NOTE_DOMAIN_V3,
                    &[amounts[2], rho[2], output_owners[0], asset],
                ),
                native_hash(
                    CONFIDENTIAL_POSEIDON_NOTE_DOMAIN_V3,
                    &[amounts[3], rho[3], output_owners[1], asset],
                ),
            ];
            let nullifiers = [rho[0], rho[1]].map(|rho| {
                native_hash(
                    CONFIDENTIAL_POSEIDON_NULLIFIER_DOMAIN_V3,
                    &[spend, rho, asset, chain],
                )
            });
            vec![
                vec![commitments[0]],
                vec![if witness.include_input_1 {
                    commitments[1]
                } else {
                    Scalar::ZERO
                }],
                vec![nullifiers[0]],
                vec![if witness.include_input_1 {
                    nullifiers[1]
                } else {
                    Scalar::ZERO
                }],
                vec![commitments[2]],
                vec![if witness.include_output_1 {
                    commitments[3]
                } else {
                    Scalar::ZERO
                }],
                vec![scalar_from_repr(witness.input_0_path.root).expect("canonical root")],
                vec![asset],
                vec![chain],
            ]
        }

        #[test]
        fn secure_transfer_relation_accepts_valid_witness_and_rejects_public_mutation() {
            const K: usize = super::super::CONFIDENTIAL_TRANSFER_V2_IPA_K as usize;
            let witness = sample_witness();
            let builder = transfer_builder::<2>(Some(&witness), K).expect("valid witness");
            let public = expected_instances(&witness);
            assert_eq!(instances(&builder), public);
            MockProver::run(K as u32, &builder, public.clone())
                .expect("secure transfer relation")
                .assert_satisfied();

            for column in 0..public.len() {
                let mut mutated = public.clone();
                mutated[column][0] += Scalar::ONE;
                assert!(
                    MockProver::run(K as u32, &builder, mutated)
                        .expect("mutated secure transfer relation")
                        .verify()
                        .is_err(),
                    "substitution in public column {column} must not satisfy the relation"
                );
            }
        }

        #[test]
        fn secure_transfer_relation_rejects_bad_path_direction_and_unbalanced_amounts() {
            const K: usize = super::super::CONFIDENTIAL_TRANSFER_V2_IPA_K as usize;

            let mut bad_direction = sample_witness();
            bad_direction.input_0_path.directions[0] = 1;
            let builder =
                transfer_builder::<2>(Some(&bad_direction), K).expect("canonical witness");
            assert!(
                MockProver::run(K as u32, &builder, instances(&builder))
                    .expect("bad-direction secure transfer relation")
                    .verify()
                    .is_err()
            );

            let mut unbalanced = sample_witness();
            unbalanced.output_0_amount += 1;
            let builder = transfer_builder::<2>(Some(&unbalanced), K).expect("canonical witness");
            assert!(
                MockProver::run(K as u32, &builder, instances(&builder))
                    .expect("unbalanced secure transfer relation")
                    .verify()
                    .is_err()
            );
        }

        #[test]
        fn secure_transfer_relation_accepts_all_supported_presence_shapes() {
            const K: usize = super::super::CONFIDENTIAL_TRANSFER_V2_IPA_K as usize;
            for include_input_1 in [false, true] {
                for include_output_1 in [false, true] {
                    let witness = sample_witness_shape(include_input_1, include_output_1);
                    let builder = transfer_builder::<2>(Some(&witness), K)
                        .expect("canonical presence-shape witness");
                    let public = expected_instances(&witness);
                    assert_eq!(instances(&builder), public);
                    MockProver::run(K as u32, &builder, public)
                        .expect("presence-shape secure transfer relation")
                        .assert_satisfied();
                }
            }
        }

        #[test]
        fn secure_transfer_builder_rejects_noncanonical_and_nonexact_witnesses() {
            const K: usize = super::super::CONFIDENTIAL_TRANSFER_V2_IPA_K as usize;
            let mut witness = sample_witness();
            witness.spend_scalar = [0xff; 32];
            assert!(transfer_builder::<2>(Some(&witness), K).is_err());

            let mut witness = sample_witness();
            witness.input_0_path.siblings.push([0; 32]);
            assert!(transfer_builder::<2>(Some(&witness), K).is_err());

            let mut witness = sample_witness();
            witness.input_0_path.witness_nodes.pop();
            assert!(transfer_builder::<2>(Some(&witness), K).is_err());

            let mut witness = sample_witness();
            witness.input_0_path.directions[0] = 2;
            assert!(transfer_builder::<2>(Some(&witness), K).is_err());

            let mut witness = sample_witness_shape(false, false);
            witness.input_1_rho = [9; 32];
            assert!(transfer_builder::<2>(Some(&witness), K).is_err());

            let mut witness = sample_witness_shape(false, false);
            witness.output_1_owner_tag = scalar_to_repr_bytes(Scalar::ONE);
            assert!(transfer_builder::<2>(Some(&witness), K).is_err());
        }

        #[test]
        fn secure_transfer_relation_rejects_each_private_witness_and_path_substitution() {
            const K: usize = super::super::CONFIDENTIAL_TRANSFER_V2_IPA_K as usize;
            let original = sample_witness_shape(true, true);
            let public = expected_instances(&original);
            let bump = |bytes: [u8; 32]| {
                scalar_to_repr_bytes(
                    scalar_from_repr(bytes).expect("canonical mutation source") + Scalar::ONE,
                )
            };
            let rejects =
                |label: &str, witness: ConfidentialTransferWitnessV2| match transfer_builder::<2>(
                    Some(&witness),
                    K,
                ) {
                    Err(_) => {}
                    Ok(builder) => assert!(
                        MockProver::run(K as u32, &builder, public.clone())
                            .expect("private-witness mutation prover")
                            .verify()
                            .is_err(),
                        "private substitution `{label}` must fail"
                    ),
                };

            let mut witness = original.clone();
            witness.input_0_amount += 1;
            witness.output_0_amount += 1;
            rejects("amounts", witness);
            for (label, mutate) in [
                ("input_0_rho", 0usize),
                ("input_1_rho", 1),
                ("output_0_rho", 2),
                ("output_1_rho", 3),
            ] {
                let mut witness = original.clone();
                match mutate {
                    0 => witness.input_0_rho[0] ^= 1,
                    1 => witness.input_1_rho[0] ^= 1,
                    2 => witness.output_0_rho[0] ^= 1,
                    3 => witness.output_1_rho[0] ^= 1,
                    _ => unreachable!(),
                }
                rejects(label, witness);
            }
            for (label, mutate) in [
                ("spend_scalar", 0usize),
                ("input_0_diversifier", 1),
                ("input_1_diversifier", 2),
                ("output_0_owner", 3),
                ("output_1_owner", 4),
                ("asset_tag", 5),
                ("chain_tag", 6),
            ] {
                let mut witness = original.clone();
                match mutate {
                    0 => witness.spend_scalar = bump(witness.spend_scalar),
                    1 => {
                        witness.input_0_diversifier = bump(witness.input_0_diversifier);
                    }
                    2 => {
                        witness.input_1_diversifier = bump(witness.input_1_diversifier);
                    }
                    3 => witness.output_0_owner_tag = bump(witness.output_0_owner_tag),
                    4 => witness.output_1_owner_tag = bump(witness.output_1_owner_tag),
                    5 => witness.asset_tag = bump(witness.asset_tag),
                    6 => witness.chain_tag = bump(witness.chain_tag),
                    _ => unreachable!(),
                }
                rejects(label, witness);
            }
            for path_index in 0..2 {
                for level in 0..2 {
                    let mut sibling = original.clone();
                    let path = if path_index == 0 {
                        &mut sibling.input_0_path
                    } else {
                        &mut sibling.input_1_path
                    };
                    path.siblings[level] = bump(path.siblings[level]);
                    rejects("path_sibling", sibling);

                    let mut direction = original.clone();
                    let path = if path_index == 0 {
                        &mut direction.input_0_path
                    } else {
                        &mut direction.input_1_path
                    };
                    path.directions[level] ^= 1;
                    rejects("path_direction", direction);

                    let mut node = original.clone();
                    let path = if path_index == 0 {
                        &mut node.input_0_path
                    } else {
                        &mut node.input_1_path
                    };
                    path.witness_nodes[level] = bump(path.witness_nodes[level]);
                    rejects("path_witness_node", node);
                }
                let mut root = original.clone();
                let path = if path_index == 0 {
                    &mut root.input_0_path
                } else {
                    &mut root.input_1_path
                };
                path.root = bump(path.root);
                rejects("path_root", root);
            }

            let mut presence = original.clone();
            presence.include_input_1 = false;
            rejects("input_presence", presence);
            let mut presence = original;
            presence.include_output_1 = false;
            rejects("output_presence", presence);
        }

        fn sample_topup_witness() -> KagemushaTopUpShieldWitnessV2 {
            let amount = 10u128;
            let rho_bytes = [0x71; 32];
            let rho =
                super::super::hash_to_scalar(b"iroha.confidential.v3.note_rho", &[&rho_bytes]);
            let spend = Scalar::from(73);
            let diversifier = Scalar::from(79);
            let asset = Scalar::from(83);
            let chain = Scalar::from(89);
            let owner = native_hash(CONFIDENTIAL_POSEIDON_OWNER_DOMAIN_V3, &[spend, diversifier]);
            let commitment = native_hash(
                CONFIDENTIAL_POSEIDON_NOTE_DOMAIN_V3,
                &[scalar_from_u128(amount), rho, owner, asset],
            );
            let empty_leaf =
                native_hash(CONFIDENTIAL_POSEIDON_MERKLE_LEAF_DOMAIN_V3, &[Scalar::ZERO]);
            let output_leaf =
                native_hash(CONFIDENTIAL_POSEIDON_MERKLE_LEAF_DOMAIN_V3, &[commitment]);
            let empty_pair = native_hash(
                CONFIDENTIAL_POSEIDON_MERKLE_NODE_DOMAIN_V3,
                &[empty_leaf, empty_leaf],
            );
            let initial_root = native_hash(
                CONFIDENTIAL_POSEIDON_MERKLE_NODE_DOMAIN_V3,
                &[empty_pair, empty_pair],
            );
            let output_pair = native_hash(
                CONFIDENTIAL_POSEIDON_MERKLE_NODE_DOMAIN_V3,
                &[empty_leaf, output_leaf],
            );
            let final_root = native_hash(
                CONFIDENTIAL_POSEIDON_MERKLE_NODE_DOMAIN_V3,
                &[output_pair, empty_pair],
            );
            KagemushaTopUpShieldWitnessV2 {
                amount,
                asset_scale: 18,
                leaf_index: 1,
                rho: rho_bytes,
                spend_scalar: scalar_to_repr_bytes(spend),
                diversifier: scalar_to_repr_bytes(diversifier),
                asset_tag: scalar_to_repr_bytes(asset),
                chain_tag: scalar_to_repr_bytes(chain),
                payer_tag: scalar_to_repr_bytes(Scalar::from(97)),
                operation_tag: scalar_to_repr_bytes(Scalar::from(101)),
                zero_path: ConfidentialMerklePathV2 {
                    siblings: [empty_leaf, empty_pair].map(scalar_to_repr_bytes).to_vec(),
                    directions: vec![1, 0],
                    witness_nodes: [empty_pair, initial_root]
                        .map(scalar_to_repr_bytes)
                        .to_vec(),
                    root: scalar_to_repr_bytes(initial_root),
                },
                output_nodes: [output_pair, final_root].map(scalar_to_repr_bytes).to_vec(),
            }
        }

        fn expected_topup_instances(witness: &KagemushaTopUpShieldWitnessV2) -> Vec<Vec<Scalar>> {
            let amount = scalar_from_u128(witness.amount);
            let rho =
                super::super::hash_to_scalar(b"iroha.confidential.v3.note_rho", &[&witness.rho]);
            let spend = scalar_from_repr(witness.spend_scalar).expect("canonical spend");
            let diversifier = scalar_from_repr(witness.diversifier).expect("canonical diversifier");
            let asset = scalar_from_repr(witness.asset_tag).expect("canonical asset");
            let chain = scalar_from_repr(witness.chain_tag).expect("canonical chain");
            let owner = native_hash(CONFIDENTIAL_POSEIDON_OWNER_DOMAIN_V3, &[spend, diversifier]);
            let commitment = native_hash(
                CONFIDENTIAL_POSEIDON_NOTE_DOMAIN_V3,
                &[amount, rho, owner, asset],
            );
            let nullifier = native_hash(
                CONFIDENTIAL_POSEIDON_NULLIFIER_DOMAIN_V3,
                &[spend, rho, asset, chain],
            );
            vec![
                vec![commitment],
                vec![nullifier],
                vec![scalar_from_repr(witness.zero_path.root).expect("canonical initial root")],
                vec![
                    scalar_from_repr(*witness.output_nodes.last().expect("final root"))
                        .expect("canonical final root"),
                ],
                vec![amount],
                vec![Scalar::from(u64::from(witness.asset_scale))],
                vec![Scalar::from(u64::from(witness.leaf_index))],
                vec![asset],
                vec![chain],
                vec![scalar_from_repr(witness.payer_tag).expect("canonical payer")],
                vec![scalar_from_repr(witness.operation_tag).expect("canonical operation")],
            ]
        }

        #[test]
        fn secure_topup_relation_binds_every_public_column() {
            const K: usize = super::super::KAGEMUSHA_TOPUP_SHIELD_V2_IPA_K as usize;
            let witness = sample_topup_witness();
            let builder = topup_builder::<2>(Some(&witness), K).expect("canonical top-up");
            let public = expected_topup_instances(&witness);
            assert_eq!(instances(&builder), public);
            MockProver::run(K as u32, &builder, public.clone())
                .expect("secure top-up relation")
                .assert_satisfied();
            for column in 0..public.len() {
                let mut mutated = public.clone();
                mutated[column][0] += Scalar::ONE;
                assert!(
                    MockProver::run(K as u32, &builder, mutated)
                        .expect("mutated secure top-up relation")
                        .verify()
                        .is_err(),
                    "substitution in top-up public column {column} must fail"
                );
            }
        }

        #[test]
        fn secure_topup_rejects_malformed_or_contradictory_paths() {
            const K: usize = super::super::KAGEMUSHA_TOPUP_SHIELD_V2_IPA_K as usize;
            let mut witness = sample_topup_witness();
            witness.output_nodes.push([0; 32]);
            assert!(topup_builder::<2>(Some(&witness), K).is_err());

            let mut witness = sample_topup_witness();
            witness.zero_path.siblings[0] = [0xff; 32];
            assert!(topup_builder::<2>(Some(&witness), K).is_err());

            let mut witness = sample_topup_witness();
            witness.leaf_index = 0;
            let builder = topup_builder::<2>(Some(&witness), K).expect("canonical fields");
            assert!(
                MockProver::run(K as u32, &builder, instances(&builder))
                    .expect("direction/index mismatch top-up")
                    .verify()
                    .is_err()
            );
        }

        #[test]
        fn secure_topup_rejects_each_private_witness_and_path_substitution() {
            const K: usize = super::super::KAGEMUSHA_TOPUP_SHIELD_V2_IPA_K as usize;
            let original = sample_topup_witness();
            let public = expected_topup_instances(&original);
            let bump = |bytes: [u8; 32]| {
                scalar_to_repr_bytes(
                    scalar_from_repr(bytes).expect("canonical mutation source") + Scalar::ONE,
                )
            };
            let rejects =
                |label: &str, witness: KagemushaTopUpShieldWitnessV2| match topup_builder::<2>(
                    Some(&witness),
                    K,
                ) {
                    Err(_) => {}
                    Ok(builder) => assert!(
                        MockProver::run(K as u32, &builder, public.clone())
                            .expect("top-up private-witness mutation prover")
                            .verify()
                            .is_err(),
                        "top-up private substitution `{label}` must fail"
                    ),
                };

            let mut witness = original.clone();
            witness.amount += 1;
            rejects("amount", witness);
            let mut witness = original.clone();
            witness.asset_scale += 1;
            rejects("asset_scale", witness);
            let mut witness = original.clone();
            witness.leaf_index = 0;
            rejects("leaf_index", witness);
            let mut witness = original.clone();
            witness.rho[0] ^= 1;
            rejects("rho", witness);
            for (label, mutate) in [
                ("spend", 0usize),
                ("diversifier", 1),
                ("asset", 2),
                ("chain", 3),
                ("payer", 4),
                ("operation", 5),
            ] {
                let mut witness = original.clone();
                match mutate {
                    0 => witness.spend_scalar = bump(witness.spend_scalar),
                    1 => witness.diversifier = bump(witness.diversifier),
                    2 => witness.asset_tag = bump(witness.asset_tag),
                    3 => witness.chain_tag = bump(witness.chain_tag),
                    4 => witness.payer_tag = bump(witness.payer_tag),
                    5 => witness.operation_tag = bump(witness.operation_tag),
                    _ => unreachable!(),
                }
                rejects(label, witness);
            }
            for level in 0..2 {
                let mut witness = original.clone();
                witness.zero_path.siblings[level] = bump(witness.zero_path.siblings[level]);
                rejects("sibling", witness);
                let mut witness = original.clone();
                witness.zero_path.directions[level] ^= 1;
                rejects("direction", witness);
                let mut witness = original.clone();
                witness.zero_path.witness_nodes[level] =
                    bump(witness.zero_path.witness_nodes[level]);
                rejects("empty_witness_node", witness);
                let mut witness = original.clone();
                witness.output_nodes[level] = bump(witness.output_nodes[level]);
                rejects("output_witness_node", witness);
            }
            let mut witness = original;
            witness.zero_path.root = bump(witness.zero_path.root);
            rejects("initial_root", witness);
        }

        fn sample_full_unshield_witness() -> ConfidentialUnshieldWitnessV2 {
            let transfer = sample_witness_shape(true, false);
            ConfidentialUnshieldWitnessV2 {
                include_input_1: true,
                input_0_amount: transfer.input_0_amount,
                input_1_amount: transfer.input_1_amount,
                input_0_rho: transfer.input_0_rho,
                input_1_rho: transfer.input_1_rho,
                spend_scalar: transfer.spend_scalar,
                input_0_diversifier: transfer.input_0_diversifier,
                input_1_diversifier: transfer.input_1_diversifier,
                asset_tag: transfer.asset_tag,
                chain_tag: transfer.chain_tag,
                input_0_path: transfer.input_0_path.clone(),
                input_1_path: transfer.input_1_path.clone(),
            }
        }

        fn sample_change_unshield_witness() -> ConfidentialUnshieldWitnessV3 {
            let full = sample_full_unshield_witness();
            ConfidentialUnshieldWitnessV3 {
                include_input_1: full.include_input_1,
                include_output_0: true,
                input_0_amount: full.input_0_amount,
                input_1_amount: full.input_1_amount,
                output_0_amount: 4,
                input_0_rho: full.input_0_rho,
                input_1_rho: full.input_1_rho,
                output_0_rho: [0x75; 32],
                spend_scalar: full.spend_scalar,
                input_0_diversifier: full.input_0_diversifier,
                input_1_diversifier: full.input_1_diversifier,
                asset_tag: full.asset_tag,
                chain_tag: full.chain_tag,
                input_0_path: full.input_0_path.clone(),
                input_1_path: full.input_1_path.clone(),
            }
        }

        fn expected_full_unshield_instances(
            witness: &ConfidentialUnshieldWitnessV2,
        ) -> Vec<Vec<Scalar>> {
            let transfer = ConfidentialTransferWitnessV2 {
                include_input_1: witness.include_input_1,
                include_output_1: false,
                input_0_amount: witness.input_0_amount,
                input_1_amount: witness.input_1_amount,
                output_0_amount: witness.input_0_amount + witness.input_1_amount,
                output_1_amount: 0,
                input_0_rho: witness.input_0_rho,
                input_1_rho: witness.input_1_rho,
                output_0_rho: [1; 32],
                output_1_rho: [0; 32],
                spend_scalar: witness.spend_scalar,
                input_0_diversifier: witness.input_0_diversifier,
                input_1_diversifier: witness.input_1_diversifier,
                output_0_owner_tag: scalar_to_repr_bytes(Scalar::ONE),
                output_1_owner_tag: [0; 32],
                asset_tag: witness.asset_tag,
                chain_tag: witness.chain_tag,
                input_0_path: witness.input_0_path.clone(),
                input_1_path: witness.input_1_path.clone(),
            };
            let transfer_public = expected_instances(&transfer);
            vec![
                transfer_public[0].clone(),
                transfer_public[1].clone(),
                transfer_public[2].clone(),
                transfer_public[3].clone(),
                transfer_public[6].clone(),
                vec![scalar_from_u128(
                    witness.input_0_amount + witness.input_1_amount,
                )],
                transfer_public[7].clone(),
                transfer_public[8].clone(),
            ]
        }

        fn expected_change_unshield_instances(
            witness: &ConfidentialUnshieldWitnessV3,
        ) -> Vec<Vec<Scalar>> {
            let full = ConfidentialUnshieldWitnessV2 {
                include_input_1: witness.include_input_1,
                input_0_amount: witness.input_0_amount,
                input_1_amount: witness.input_1_amount,
                input_0_rho: witness.input_0_rho,
                input_1_rho: witness.input_1_rho,
                spend_scalar: witness.spend_scalar,
                input_0_diversifier: witness.input_0_diversifier,
                input_1_diversifier: witness.input_1_diversifier,
                asset_tag: witness.asset_tag,
                chain_tag: witness.chain_tag,
                input_0_path: witness.input_0_path.clone(),
                input_1_path: witness.input_1_path.clone(),
            };
            let full_public = expected_full_unshield_instances(&full);
            let spend = scalar_from_repr(witness.spend_scalar).expect("canonical spend");
            let asset = scalar_from_repr(witness.asset_tag).expect("canonical asset");
            let output_rho = super::super::hash_to_scalar(
                b"iroha.confidential.v3.note_rho",
                &[&witness.output_0_rho],
            );
            let output_owner =
                native_hash(CONFIDENTIAL_POSEIDON_OWNER_DOMAIN_V3, &[spend, Scalar::ONE]);
            let change = native_hash(
                CONFIDENTIAL_POSEIDON_NOTE_DOMAIN_V3,
                &[
                    scalar_from_u128(witness.output_0_amount),
                    output_rho,
                    output_owner,
                    asset,
                ],
            );
            vec![
                full_public[0].clone(),
                full_public[1].clone(),
                full_public[2].clone(),
                full_public[3].clone(),
                vec![change],
                full_public[4].clone(),
                vec![scalar_from_u128(
                    witness.input_0_amount + witness.input_1_amount - witness.output_0_amount,
                )],
                full_public[6].clone(),
                full_public[7].clone(),
            ]
        }

        #[test]
        fn secure_full_unshield_relation_binds_every_public_column() {
            const K: usize = super::super::CONFIDENTIAL_UNSHIELD_V2_IPA_K as usize;
            let witness = sample_full_unshield_witness();
            let builder = unshield_builder::<2>(UnshieldWitnessRef::Full(Some(&witness)), K)
                .expect("canonical full unshield");
            let public = expected_full_unshield_instances(&witness);
            assert_eq!(instances(&builder), public);
            MockProver::run(K as u32, &builder, public.clone())
                .expect("secure full-unshield relation")
                .assert_satisfied();
            for column in 0..public.len() {
                let mut mutated = public.clone();
                mutated[column][0] += Scalar::ONE;
                assert!(
                    MockProver::run(K as u32, &builder, mutated)
                        .expect("mutated full-unshield relation")
                        .verify()
                        .is_err(),
                    "substitution in full-unshield public column {column} must fail"
                );
            }
        }

        #[test]
        fn secure_change_unshield_relation_binds_change_and_public_amount() {
            const K: usize = super::super::CONFIDENTIAL_UNSHIELD_V3_IPA_K as usize;
            let witness = sample_change_unshield_witness();
            let builder = unshield_builder::<2>(UnshieldWitnessRef::Change(Some(&witness)), K)
                .expect("canonical change unshield");
            let public = expected_change_unshield_instances(&witness);
            assert_eq!(instances(&builder), public);
            MockProver::run(K as u32, &builder, public.clone())
                .expect("secure change-unshield relation")
                .assert_satisfied();
            for column in 0..public.len() {
                let mut mutated = public.clone();
                mutated[column][0] += Scalar::ONE;
                assert!(
                    MockProver::run(K as u32, &builder, mutated)
                        .expect("mutated change-unshield relation")
                        .verify()
                        .is_err(),
                    "substitution in change-unshield public column {column} must fail"
                );
            }

            let mut malformed = sample_change_unshield_witness();
            malformed.include_output_0 = false;
            assert!(
                unshield_builder::<2>(UnshieldWitnessRef::Change(Some(&malformed)), K).is_err()
            );
        }

        #[test]
        #[ignore = "explicit production-depth release resource measurement"]
        fn report_production_depth_secure_relation_shapes() {
            fn report(label: &str, builder: &BaseCircuitBuilder<Scalar>) {
                let stats = builder.statistics();
                eprintln!(
                    "{label}: k={} advice_cells={:?} advice_columns={:?} fixed_columns={} lookup_cells={:?} lookup_columns={:?} instance_columns={}",
                    builder.config_params.k,
                    stats.gate.total_advice_per_phase,
                    builder.config_params.num_advice_per_phase,
                    builder.config_params.num_fixed,
                    stats.total_lookup_advice_per_phase,
                    builder.config_params.num_lookup_advice_per_phase,
                    builder.config_params.num_instance_columns,
                );
            }

            report(
                "transfer",
                &transfer_builder::<{ super::super::CONFIDENTIAL_TREE_DEPTH_V2 }>(
                    None,
                    super::super::CONFIDENTIAL_TRANSFER_V2_IPA_K as usize,
                )
                .expect("transfer shape"),
            );
            report(
                "topup",
                &topup_builder::<{ super::super::CONFIDENTIAL_TREE_DEPTH_V2 }>(
                    None,
                    super::super::KAGEMUSHA_TOPUP_SHIELD_V2_IPA_K as usize,
                )
                .expect("top-up shape"),
            );
            report(
                "full-unshield",
                &unshield_builder::<{ super::super::CONFIDENTIAL_TREE_DEPTH_V2 }>(
                    UnshieldWitnessRef::Full(None),
                    super::super::CONFIDENTIAL_UNSHIELD_V2_IPA_K as usize,
                )
                .expect("full-unshield shape"),
            );
            report(
                "change-unshield",
                &unshield_builder::<{ super::super::CONFIDENTIAL_TREE_DEPTH_V2 }>(
                    UnshieldWitnessRef::Change(None),
                    super::super::CONFIDENTIAL_UNSHIELD_V3_IPA_K as usize,
                )
                .expect("change-unshield shape"),
            );
        }
    }
}

#[cfg(all(any(feature = "zk-halo2", feature = "zk-halo2-ipa"), test))]
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

#[cfg(all(any(feature = "zk-halo2", feature = "zk-halo2-ipa"), test))]
fn nullifier_scalar(sk: Scalar, rho: Scalar, asset_tag: Scalar, chain_tag: Scalar) -> Scalar {
    poseidon_pair(sk, poseidon_pair(rho, poseidon_pair(asset_tag, chain_tag)))
}

#[cfg(all(any(feature = "zk-halo2", feature = "zk-halo2-ipa"), test))]
fn leaf_scalar_from_commitment(commitment: [u8; 32]) -> Scalar {
    scalar_from_repr(commitment)
        .unwrap_or_else(|| hash_to_scalar(b"iroha.confidential.v2.legacy_leaf", &[&commitment]))
}

#[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
/// Derive the default-diversifier owner tag for a confidential spend key.
pub fn derive_confidential_owner_tag_v2(spend_key: &[u8]) -> Result<[u8; 32], String> {
    derive_confidential_owner_tag_v2_with_diversifier(
        spend_key,
        default_confidential_diversifier_v2(),
    )
}

#[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
/// Return the canonical default owner diversifier.
pub fn default_confidential_diversifier_v2() -> [u8; 32] {
    scalar_to_repr_bytes(Scalar::ONE)
}

#[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
/// Derive a canonical owner diversifier from arbitrary seed bytes.
pub fn derive_confidential_diversifier_v2(seed: &[u8]) -> [u8; 32] {
    scalar_to_repr_bytes(hash_to_scalar(
        b"iroha.confidential.v3.diversifier",
        &[seed],
    ))
}

#[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
/// Derive an owner tag for a spend key and explicit diversifier.
pub fn derive_confidential_owner_tag_v2_with_diversifier(
    spend_key: &[u8],
    diversifier: [u8; 32],
) -> Result<[u8; 32], String> {
    derive_confidential_owner_tag_v3_with_diversifier(spend_key, diversifier)
}

#[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
/// Derive the field tag for an asset-definition identifier.
pub fn derive_confidential_asset_tag_v2(asset_definition_id: &str) -> [u8; 32] {
    derive_confidential_asset_tag_v3(asset_definition_id)
        .expect("validated asset identifiers derive non-zero V3 tags")
}

#[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
/// Derive the field tag for a chain identifier.
pub fn derive_confidential_chain_tag_v2(chain_id: &str) -> [u8; 32] {
    derive_confidential_chain_tag_v3(chain_id)
        .expect("validated chain identifiers derive non-zero V3 tags")
}

#[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
/// Derive the field tag for a Kagemusha top-up payer.
pub fn derive_kagemusha_topup_payer_tag_v2(payer: &str) -> [u8; 32] {
    derive_kagemusha_topup_payer_tag_v3(payer)
        .expect("validated payer identifiers derive non-zero V3 tags")
}

#[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
/// Derive the field tag for a Kagemusha top-up operation.
pub fn derive_kagemusha_topup_operation_tag_v2(operation_id: &[u8; 32]) -> [u8; 32] {
    derive_kagemusha_topup_operation_tag_v3(operation_id)
        .expect("validated non-zero operation IDs derive non-zero V3 tags")
}

#[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
/// Encode a `u32` as one canonical Pasta scalar.
pub fn encode_kagemusha_topup_u32_v2(value: u32) -> [u8; 32] {
    scalar_to_repr_bytes(Scalar::from(u64::from(value)))
}

#[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
/// Derive the field tag for an asset-hidden pool identifier.
pub fn derive_asset_hidden_pool_id_tag_v1(pool_id: &str) -> [u8; 32] {
    scalar_to_repr_bytes(hash_to_scalar(
        b"iroha.confidential.asset_hidden.v1.pool_id",
        &[pool_id.trim().as_bytes()],
    ))
}

#[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
/// Derive a confidential note commitment from its opening and context.
pub fn derive_confidential_note_v2(
    asset_definition_id: &str,
    amount: u128,
    rho: [u8; 32],
    owner_tag: [u8; 32],
) -> Result<[u8; 32], String> {
    derive_confidential_note_v3(
        derive_confidential_asset_tag_v3(asset_definition_id)?,
        amount,
        rho,
        owner_tag,
    )
}

#[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
/// Derive a confidential spend nullifier from its opening and context.
pub fn derive_confidential_nullifier_v2(
    chain_id: &str,
    asset_definition_id: &str,
    spend_key: &[u8],
    rho: [u8; 32],
) -> [u8; 32] {
    derive_confidential_nullifier_v3(
        spend_key,
        rho,
        derive_confidential_asset_tag_v3(asset_definition_id).expect("validated asset identifier"),
        derive_confidential_chain_tag_v3(chain_id).expect("validated chain identifier"),
    )
    .expect("validated confidential nullifier inputs")
}

#[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
/// Encode an exact `u128` amount as one canonical Pasta scalar.
pub fn encode_confidential_amount_v2(amount: u128) -> [u8; 32] {
    scalar_to_repr_bytes(scalar_from_u128(amount))
}

#[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
/// Return the canonical empty root of the fixed confidential tree.
pub fn poseidon_empty_root_v2() -> [u8; 32] {
    let mut node =
        confidential_poseidon_hash_v3(CONFIDENTIAL_POSEIDON_MERKLE_LEAF_DOMAIN_V3, &[Scalar::ZERO]);
    for _ in 0..CONFIDENTIAL_TREE_DEPTH_V2 {
        node = merkle_parent_v3(node, node);
    }
    scalar_to_repr_bytes(node)
}

#[cfg(all(any(feature = "zk-halo2", feature = "zk-halo2-ipa"), test))]
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
/// Compute the fixed-tree root for canonical commitment leaves.
pub fn compute_confidential_root_v2(commitments: &[[u8; 32]]) -> Result<[u8; 32], String> {
    compute_confidential_root_v3(commitments)
}

#[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
/// Compute a canonical fixed-tree authentication path for one leaf index.
pub fn compute_confidential_merkle_path_v2(
    commitments: &[[u8; 32]],
    leaf_index: usize,
) -> Result<ConfidentialMerklePathV2, String> {
    compute_confidential_merkle_path_v3(commitments, leaf_index)
}

#[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
fn poseidon_tag_v3(domain: u64, label: &[u8], bytes: &[u8]) -> Result<Scalar, String> {
    let preimage = hash_to_scalar(label, &[bytes]);
    let tag = confidential_poseidon_hash_v3(domain, &[preimage]);
    if tag == Scalar::ZERO {
        Err("V3 domain-separated tag must not be zero".to_owned())
    } else {
        Ok(tag)
    }
}

#[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
fn strict_v3_identifier<'a>(value: &'a str, label: &str) -> Result<&'a str, String> {
    if value.is_empty() || value.trim() != value {
        Err(format!(
            "V3 {label} must be non-empty and contain no surrounding whitespace"
        ))
    } else {
        Ok(value)
    }
}

#[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
/// Derive a domain-separated V3 owner tag from a spend key and diversifier.
pub fn derive_confidential_owner_tag_v3_with_diversifier(
    spend_key: &[u8],
    diversifier: [u8; 32],
) -> Result<[u8; 32], String> {
    if spend_key.len() != 32 || spend_key.iter().all(|byte| *byte == 0) {
        return Err("V3 spend key must be exactly 32 non-zero bytes".to_owned());
    }
    let spend = hash_to_scalar(b"iroha.confidential.v3.spend_scalar", &[spend_key]);
    let diversifier = scalar_from_repr(diversifier)
        .filter(|value| *value != Scalar::ZERO)
        .ok_or_else(|| "V3 diversifier must be a non-zero canonical Pasta scalar".to_owned())?;
    let owner =
        confidential_poseidon_hash_v3(CONFIDENTIAL_POSEIDON_OWNER_DOMAIN_V3, &[spend, diversifier]);
    if owner == Scalar::ZERO {
        return Err("V3 owner tag must not be zero".to_owned());
    }
    Ok(scalar_to_repr_bytes(owner))
}

#[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
/// Derive the domain-separated V3 asset tag.
pub fn derive_confidential_asset_tag_v3(asset_definition_id: &str) -> Result<[u8; 32], String> {
    let canonical = strict_v3_identifier(asset_definition_id, "asset definition identifier")?;
    Ok(scalar_to_repr_bytes(poseidon_tag_v3(
        CONFIDENTIAL_POSEIDON_ASSET_DOMAIN_V3,
        b"iroha.confidential.v3.asset_preimage",
        canonical.as_bytes(),
    )?))
}

#[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
/// Derive the domain-separated V3 chain tag.
pub fn derive_confidential_chain_tag_v3(chain_id: &str) -> Result<[u8; 32], String> {
    let canonical = strict_v3_identifier(chain_id, "chain identifier")?;
    Ok(scalar_to_repr_bytes(poseidon_tag_v3(
        CONFIDENTIAL_POSEIDON_CHAIN_DOMAIN_V3,
        b"iroha.confidential.v3.chain_preimage",
        canonical.as_bytes(),
    )?))
}

#[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
/// Derive the domain-separated V3 Kagemusha payer tag.
pub fn derive_kagemusha_topup_payer_tag_v3(payer: &str) -> Result<[u8; 32], String> {
    let canonical = strict_v3_identifier(payer, "Kagemusha payer")?;
    Ok(scalar_to_repr_bytes(poseidon_tag_v3(
        CONFIDENTIAL_POSEIDON_PAYER_DOMAIN_V3,
        b"iroha.kagemusha.topup.payer.preimage.v3",
        canonical.as_bytes(),
    )?))
}

#[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
/// Derive the domain-separated V3 Kagemusha operation tag.
pub fn derive_kagemusha_topup_operation_tag_v3(
    operation_id: &[u8; 32],
) -> Result<[u8; 32], String> {
    if *operation_id == [0; 32] {
        return Err("V3 Kagemusha operation ID must be non-zero".to_owned());
    }
    Ok(scalar_to_repr_bytes(poseidon_tag_v3(
        CONFIDENTIAL_POSEIDON_OPERATION_DOMAIN_V3,
        b"iroha.kagemusha.topup.operation.preimage.v3",
        operation_id,
    )?))
}

#[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
/// Derive a V3 note commitment with the full secure permutation.
pub fn derive_confidential_note_v3(
    asset_tag: [u8; 32],
    amount: u128,
    rho: [u8; 32],
    owner_tag: [u8; 32],
) -> Result<[u8; 32], String> {
    if amount == 0 || rho == [0; 32] {
        return Err("V3 note amount and rho must be non-zero".to_owned());
    }
    let asset = scalar_from_repr(asset_tag)
        .filter(|value| *value != Scalar::ZERO)
        .ok_or_else(|| "V3 asset tag must be a non-zero canonical Pasta scalar".to_owned())?;
    let owner = scalar_from_repr(owner_tag)
        .filter(|value| *value != Scalar::ZERO)
        .ok_or_else(|| "V3 owner tag must be a non-zero canonical Pasta scalar".to_owned())?;
    let rho = hash_to_scalar(b"iroha.confidential.v3.note_rho", &[&rho]);
    let commitment = confidential_poseidon_hash_v3(
        CONFIDENTIAL_POSEIDON_NOTE_DOMAIN_V3,
        &[scalar_from_u128(amount), rho, owner, asset],
    );
    if commitment == Scalar::ZERO {
        return Err("V3 note commitment must not be zero".to_owned());
    }
    Ok(scalar_to_repr_bytes(commitment))
}

#[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
/// Derive a V3 spend nullifier with the full secure permutation.
pub fn derive_confidential_nullifier_v3(
    spend_key: &[u8],
    rho: [u8; 32],
    asset_tag: [u8; 32],
    chain_tag: [u8; 32],
) -> Result<[u8; 32], String> {
    if spend_key.len() != 32 || spend_key.iter().all(|byte| *byte == 0) || rho == [0; 32] {
        return Err("V3 nullifier spend key and rho must be non-zero".to_owned());
    }
    let spend = hash_to_scalar(b"iroha.confidential.v3.spend_scalar", &[spend_key]);
    let rho = hash_to_scalar(b"iroha.confidential.v3.note_rho", &[&rho]);
    let asset = scalar_from_repr(asset_tag)
        .filter(|value| *value != Scalar::ZERO)
        .ok_or_else(|| "V3 asset tag must be a non-zero canonical Pasta scalar".to_owned())?;
    let chain = scalar_from_repr(chain_tag)
        .filter(|value| *value != Scalar::ZERO)
        .ok_or_else(|| "V3 chain tag must be a non-zero canonical Pasta scalar".to_owned())?;
    let nullifier = confidential_poseidon_hash_v3(
        CONFIDENTIAL_POSEIDON_NULLIFIER_DOMAIN_V3,
        &[spend, rho, asset, chain],
    );
    if nullifier == Scalar::ZERO {
        return Err("V3 nullifier must not be zero".to_owned());
    }
    Ok(scalar_to_repr_bytes(nullifier))
}

#[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
fn build_padded_leaf_layer_v3(commitments: &[[u8; 32]]) -> Result<Vec<Scalar>, String> {
    if commitments.len() > CONFIDENTIAL_TREE_CAPACITY_V2 {
        return Err(format!(
            "confidential V3 tree supports at most {} leaves",
            CONFIDENTIAL_TREE_CAPACITY_V2
        ));
    }
    let empty_leaf =
        confidential_poseidon_hash_v3(CONFIDENTIAL_POSEIDON_MERKLE_LEAF_DOMAIN_V3, &[Scalar::ZERO]);
    let mut layer = Vec::with_capacity(CONFIDENTIAL_TREE_CAPACITY_V2);
    for (index, commitment) in commitments.iter().copied().enumerate() {
        let commitment = scalar_from_repr(commitment)
            .filter(|value| *value != Scalar::ZERO)
            .ok_or_else(|| {
                format!("confidential V3 commitment[{index}] must be non-zero and canonical")
            })?;
        layer.push(confidential_poseidon_hash_v3(
            CONFIDENTIAL_POSEIDON_MERKLE_LEAF_DOMAIN_V3,
            &[commitment],
        ));
    }
    layer.resize(CONFIDENTIAL_TREE_CAPACITY_V2, empty_leaf);
    Ok(layer)
}

#[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
fn merkle_parent_v3(left: Scalar, right: Scalar) -> Scalar {
    confidential_poseidon_hash_v3(CONFIDENTIAL_POSEIDON_MERKLE_NODE_DOMAIN_V3, &[left, right])
}

#[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
/// Compute the fixed-tree root using V3 leaf and internal-node domains.
pub fn compute_confidential_root_v3(commitments: &[[u8; 32]]) -> Result<[u8; 32], String> {
    let mut layer = build_padded_leaf_layer_v3(commitments)?;
    for _ in 0..CONFIDENTIAL_TREE_DEPTH_V2 {
        layer = layer
            .chunks_exact(2)
            .map(|pair| merkle_parent_v3(pair[0], pair[1]))
            .collect();
    }
    Ok(scalar_to_repr_bytes(layer[0]))
}

#[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
/// Compute an exact V3 authentication path, including redundant checked nodes.
pub fn compute_confidential_merkle_path_v3(
    commitments: &[[u8; 32]],
    leaf_index: usize,
) -> Result<ConfidentialMerklePathV2, String> {
    if leaf_index >= CONFIDENTIAL_TREE_CAPACITY_V2 {
        return Err(format!(
            "leaf_index must be < {} for confidential V3 proofs",
            CONFIDENTIAL_TREE_CAPACITY_V2
        ));
    }
    let mut current_index = leaf_index;
    let mut layer = build_padded_leaf_layer_v3(commitments)?;
    let mut siblings = Vec::with_capacity(CONFIDENTIAL_TREE_DEPTH_V2);
    let mut directions = Vec::with_capacity(CONFIDENTIAL_TREE_DEPTH_V2);
    let mut witness_nodes = Vec::with_capacity(CONFIDENTIAL_TREE_DEPTH_V2);
    for _ in 0..CONFIDENTIAL_TREE_DEPTH_V2 {
        let sibling_index = if current_index.is_multiple_of(2) {
            current_index + 1
        } else {
            current_index - 1
        };
        let direction = u8::from(!current_index.is_multiple_of(2));
        let (left, right) = if direction == 0 {
            (layer[current_index], layer[sibling_index])
        } else {
            (layer[sibling_index], layer[current_index])
        };
        siblings.push(scalar_to_repr_bytes(layer[sibling_index]));
        directions.push(direction);
        witness_nodes.push(scalar_to_repr_bytes(merkle_parent_v3(left, right)));
        current_index /= 2;
        layer = layer
            .chunks_exact(2)
            .map(|pair| merkle_parent_v3(pair[0], pair[1]))
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
/// Derive the next empty-leaf path from a supplied current root and path.
pub fn derive_confidential_next_zero_path_v2(
    previous_leaf_commitment: [u8; 32],
    previous_leaf_index: usize,
    previous_path: &ConfidentialMerklePathV2,
    root_hint: [u8; 32],
) -> Result<ConfidentialMerklePathV2, String> {
    let next_leaf_index = previous_leaf_index
        .checked_add(1)
        .ok_or_else(|| "next zero leaf_index overflowed usize".to_owned())?;
    if next_leaf_index >= CONFIDENTIAL_TREE_CAPACITY_V2 {
        return Err(format!(
            "next zero leaf_index must be < {CONFIDENTIAL_TREE_CAPACITY_V2}"
        ));
    }
    let previous_path = normalize_supplied_confidential_merkle_path_v2(
        previous_leaf_commitment,
        Some(previous_leaf_index),
        previous_path,
        root_hint,
        "previous latest confidential path",
    )?;
    let mut zero_subtrees = Vec::with_capacity(CONFIDENTIAL_TREE_DEPTH_V2);
    let mut zero_node =
        confidential_poseidon_hash_v3(CONFIDENTIAL_POSEIDON_MERKLE_LEAF_DOMAIN_V3, &[Scalar::ZERO]);
    for _ in 0..CONFIDENTIAL_TREE_DEPTH_V2 {
        zero_subtrees.push(zero_node);
        zero_node = merkle_parent_v3(zero_node, zero_node);
    }

    let mut node =
        confidential_poseidon_hash_v3(CONFIDENTIAL_POSEIDON_MERKLE_LEAF_DOMAIN_V3, &[Scalar::ZERO]);
    let previous_commitment = scalar_from_repr(previous_leaf_commitment)
        .filter(|value| *value != Scalar::ZERO)
        .ok_or_else(|| "previous commitment must be non-zero and canonical".to_owned())?;
    let mut previous_node = confidential_poseidon_hash_v3(
        CONFIDENTIAL_POSEIDON_MERKLE_LEAF_DOMAIN_V3,
        &[previous_commitment],
    );
    let mut siblings = Vec::with_capacity(CONFIDENTIAL_TREE_DEPTH_V2);
    let mut directions = Vec::with_capacity(CONFIDENTIAL_TREE_DEPTH_V2);
    let mut witness_nodes = Vec::with_capacity(CONFIDENTIAL_TREE_DEPTH_V2);

    for level in 0..CONFIDENTIAL_TREE_DEPTH_V2 {
        let previous_subtree_index = previous_leaf_index >> level;
        let next_subtree_index = next_leaf_index >> level;
        let direction = if next_subtree_index.is_multiple_of(2) {
            0
        } else {
            1
        };
        let sibling = if next_subtree_index == previous_subtree_index {
            scalar_from_repr(previous_path.siblings[level]).ok_or_else(|| {
                format!(
                    "previous latest confidential path sibling[{level}] must be a canonical Pasta scalar"
                )
            })?
        } else if direction == 1 && next_subtree_index == previous_subtree_index + 1 {
            previous_node
        } else {
            zero_subtrees[level]
        };
        node = if direction == 0 {
            merkle_parent_v3(node, sibling)
        } else {
            merkle_parent_v3(sibling, node)
        };
        siblings.push(scalar_to_repr_bytes(sibling));
        directions.push(direction);
        witness_nodes.push(scalar_to_repr_bytes(node));
        previous_node = scalar_from_repr(previous_path.witness_nodes[level]).ok_or_else(|| {
            format!(
                "previous latest confidential path witness_nodes[{level}] must be a canonical Pasta scalar"
            )
        })?;
    }
    let computed_root = scalar_to_repr_bytes(node);
    if computed_root != root_hint {
        return Err("derived next zero confidential path does not prove root_hint".to_owned());
    }
    Ok(ConfidentialMerklePathV2 {
        siblings,
        directions,
        witness_nodes,
        root: computed_root,
    })
}

#[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
pub(super) fn normalize_supplied_confidential_merkle_path_v2(
    leaf_commitment: [u8; 32],
    leaf_index: Option<usize>,
    path: &ConfidentialMerklePathV2,
    root_hint: [u8; 32],
    context: &str,
) -> Result<ConfidentialMerklePathV2, String> {
    if let Some(index) = leaf_index
        && index >= CONFIDENTIAL_TREE_CAPACITY_V2
    {
        return Err(format!(
            "{context} leaf_index must be < {CONFIDENTIAL_TREE_CAPACITY_V2}"
        ));
    }
    if path.siblings.len() != CONFIDENTIAL_TREE_DEPTH_V2 {
        return Err(format!(
            "{context} must contain exactly {CONFIDENTIAL_TREE_DEPTH_V2} siblings"
        ));
    }
    if path.directions.len() != CONFIDENTIAL_TREE_DEPTH_V2 {
        return Err(format!(
            "{context} must contain exactly {CONFIDENTIAL_TREE_DEPTH_V2} directions"
        ));
    }
    if path.witness_nodes.len() != CONFIDENTIAL_TREE_DEPTH_V2 {
        return Err(format!(
            "{context} witness_nodes must contain exactly {CONFIDENTIAL_TREE_DEPTH_V2} nodes"
        ));
    }
    if path.root != root_hint {
        return Err(format!("{context} root does not match root_hint"));
    }

    let mut current_index = leaf_index;
    let commitment = scalar_from_repr(leaf_commitment)
        .ok_or_else(|| format!("{context} leaf commitment must be a canonical Pasta scalar"))?;
    let mut node =
        confidential_poseidon_hash_v3(CONFIDENTIAL_POSEIDON_MERKLE_LEAF_DOMAIN_V3, &[commitment]);
    let mut witness_nodes = Vec::with_capacity(CONFIDENTIAL_TREE_DEPTH_V2);
    for level in 0..CONFIDENTIAL_TREE_DEPTH_V2 {
        let direction = path.directions[level];
        if direction > 1 {
            return Err(format!("{context} direction[{level}] must be 0 or 1"));
        }
        if let Some(index) = current_index.as_mut() {
            let expected = if index.is_multiple_of(2) { 0 } else { 1 };
            if direction != expected {
                return Err(format!(
                    "{context} direction[{level}] does not match leaf_index"
                ));
            }
            *index /= 2;
        }
        let sibling = scalar_from_repr(path.siblings[level]).ok_or_else(|| {
            format!("{context} sibling[{level}] must be a canonical Pasta scalar")
        })?;
        node = if direction == 0 {
            merkle_parent_v3(node, sibling)
        } else {
            merkle_parent_v3(sibling, node)
        };
        witness_nodes.push(scalar_to_repr_bytes(node));
    }
    let computed_root = scalar_to_repr_bytes(node);
    if computed_root != root_hint {
        return Err(format!("{context} does not prove the supplied root_hint"));
    }
    if path.witness_nodes != witness_nodes {
        return Err(format!(
            "{context} witness_nodes do not match the recomputed path"
        ));
    }
    Ok(ConfidentialMerklePathV2 {
        siblings: path.siblings.clone(),
        directions: path.directions.clone(),
        witness_nodes,
        root: computed_root,
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
impl Zeroize for ConfidentialTransferWitnessV2 {
    fn zeroize(&mut self) {
        self.include_input_1.zeroize();
        self.include_output_1.zeroize();
        self.input_0_amount.zeroize();
        self.input_1_amount.zeroize();
        self.output_0_amount.zeroize();
        self.output_1_amount.zeroize();
        self.input_0_rho.zeroize();
        self.input_1_rho.zeroize();
        self.output_0_rho.zeroize();
        self.output_1_rho.zeroize();
        self.spend_scalar.zeroize();
        self.input_0_diversifier.zeroize();
        self.input_1_diversifier.zeroize();
        self.output_0_owner_tag.zeroize();
        self.output_1_owner_tag.zeroize();
        self.asset_tag.zeroize();
        self.chain_tag.zeroize();
        self.input_0_path.zeroize();
        self.input_1_path.zeroize();
    }
}

#[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
impl Drop for ConfidentialTransferWitnessV2 {
    fn drop(&mut self) {
        self.zeroize();
    }
}

#[cfg(all(any(feature = "zk-halo2", feature = "zk-halo2-ipa"), test))]
#[derive(Clone, Default)]
/// Confidential transfer circuit shared by standalone proving and Kagemusha.
pub(super) struct ConfidentialTransferCircuitV2<const DEPTH: usize> {
    witness: Option<ConfidentialTransferWitnessV2>,
}

#[cfg(all(any(feature = "zk-halo2", feature = "zk-halo2-ipa"), test))]
impl<const DEPTH: usize> Zeroize for ConfidentialTransferCircuitV2<DEPTH> {
    fn zeroize(&mut self) {
        if let Some(witness) = &mut self.witness {
            witness.zeroize();
        }
        self.witness = None;
    }
}

#[cfg(all(any(feature = "zk-halo2", feature = "zk-halo2-ipa"), test))]
impl<const DEPTH: usize> Drop for ConfidentialTransferCircuitV2<DEPTH> {
    fn drop(&mut self) {
        self.zeroize();
    }
}

#[cfg(all(any(feature = "zk-halo2", feature = "zk-halo2-ipa"), test))]
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
        confidential_relation_gadget::U128RangeConfig,
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
        let amount_range = confidential_relation_gadget::U128RangeConfig::configure(meta);
        let selector = meta.selector();
        let amount_range_gate = amount_range.clone();
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
                |lhs, rhs| confidential_relation_gadget::poseidon_pair_expression(lhs, rhs);
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
                enabled.clone() * (in0_amt.clone() - amount_range_gate.query_value_at(meta, 0)),
                enabled.clone() * (in1_amt.clone() - amount_range_gate.query_value_at(meta, 8)),
                enabled.clone() * (out0_amt.clone() - amount_range_gate.query_value_at(meta, 16)),
                enabled.clone() * (out1_amt.clone() - amount_range_gate.query_value_at(meta, 24)),
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
                constraints.push(
                    enabled.clone()
                        * (witness.clone()
                            - confidential_relation_gadget::merkle_parent_expression(
                                input_0_prev.clone(),
                                sibling,
                                direction,
                            )),
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
                constraints.push(
                    enabled.clone()
                        * (witness.clone()
                            - confidential_relation_gadget::merkle_parent_expression(
                                input_1_prev.clone(),
                                sibling,
                                direction,
                            )),
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
            amount_range,
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
            amount_range,
            selector,
        ) = cfg;
        let witness = self.witness.clone();
        layouter.assign_region(
            || "confidential_transfer_v2",
            |mut region| {
                selector.enable(&mut region, 0)?;
                amount_range.assign(
                    &mut region,
                    0,
                    witness.as_ref().map(|value| value.input_0_amount),
                )?;
                amount_range.assign(
                    &mut region,
                    8,
                    witness.as_ref().map(|value| value.input_1_amount),
                )?;
                amount_range.assign(
                    &mut region,
                    16,
                    witness.as_ref().map(|value| value.output_0_amount),
                )?;
                amount_range.assign(
                    &mut region,
                    24,
                    witness.as_ref().map(|value| value.output_1_amount),
                )?;
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
struct KagemushaTopUpShieldWitnessV2 {
    amount: u128,
    asset_scale: u32,
    leaf_index: u32,
    rho: [u8; 32],
    spend_scalar: [u8; 32],
    diversifier: [u8; 32],
    asset_tag: [u8; 32],
    chain_tag: [u8; 32],
    payer_tag: [u8; 32],
    operation_tag: [u8; 32],
    zero_path: ConfidentialMerklePathV2,
    output_nodes: Vec<[u8; 32]>,
}

#[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
impl Zeroize for KagemushaTopUpShieldWitnessV2 {
    fn zeroize(&mut self) {
        self.amount.zeroize();
        self.asset_scale.zeroize();
        self.leaf_index.zeroize();
        self.rho.zeroize();
        self.spend_scalar.zeroize();
        self.diversifier.zeroize();
        self.asset_tag.zeroize();
        self.chain_tag.zeroize();
        self.payer_tag.zeroize();
        self.operation_tag.zeroize();
        self.zero_path.zeroize();
        self.output_nodes.zeroize();
    }
}

#[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
impl Drop for KagemushaTopUpShieldWitnessV2 {
    fn drop(&mut self) {
        self.zeroize();
    }
}

#[cfg(all(any(feature = "zk-halo2", feature = "zk-halo2-ipa"), test))]
#[derive(Clone, Default)]
struct KagemushaTopUpShieldCircuitV2<const DEPTH: usize> {
    witness: Option<KagemushaTopUpShieldWitnessV2>,
}

#[cfg(all(any(feature = "zk-halo2", feature = "zk-halo2-ipa"), test))]
impl<const DEPTH: usize> Zeroize for KagemushaTopUpShieldCircuitV2<DEPTH> {
    fn zeroize(&mut self) {
        if let Some(witness) = &mut self.witness {
            witness.zeroize();
        }
        self.witness = None;
    }
}

#[cfg(all(any(feature = "zk-halo2", feature = "zk-halo2-ipa"), test))]
impl<const DEPTH: usize> Drop for KagemushaTopUpShieldCircuitV2<DEPTH> {
    fn drop(&mut self) {
        self.zeroize();
    }
}

#[cfg(all(any(feature = "zk-halo2", feature = "zk-halo2-ipa"), test))]
#[derive(Clone)]
struct KagemushaTopUpShieldConfigV2 {
    amount_range: confidential_relation_gadget::U128RangeConfig,
    amount: halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>,
    amount_inverse: halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>,
    output_commitment_inverse: halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>,
    spend_nullifier_inverse: halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>,
    note_field_difference_inverse: halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>,
    initial_root_inverse: halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>,
    finalized_root_inverse: halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>,
    root_difference_inverse: halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>,
    asset_scale: halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>,
    leaf_index: halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>,
    rho: halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>,
    spend_scalar: halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>,
    diversifier: halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>,
    owner_tag: halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>,
    owner_asset: halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>,
    rho_owner_asset: halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>,
    asset_chain: halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>,
    rho_asset_chain: halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>,
    payer_tag: halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>,
    operation_tag: halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>,
    siblings: Vec<halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>>,
    directions: Vec<halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>>,
    zero_nodes: Vec<halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>>,
    output_nodes: Vec<halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>>,
    index_quotients: Vec<halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>>,
    selector: Selector,
}

#[cfg(all(any(feature = "zk-halo2", feature = "zk-halo2-ipa"), test))]
impl<const DEPTH: usize> Circuit<Scalar> for KagemushaTopUpShieldCircuitV2<DEPTH> {
    type Config = KagemushaTopUpShieldConfigV2;
    type FloorPlanner = SimpleFloorPlanner;
    type Params = ();

    fn without_witnesses(&self) -> Self {
        Self::default()
    }

    fn configure(meta: &mut ConstraintSystem<Scalar>) -> Self::Config {
        let amount_range = confidential_relation_gadget::U128RangeConfig::configure(meta);
        let amount = meta.advice_column();
        let amount_inverse = meta.advice_column();
        let output_commitment_inverse = meta.advice_column();
        let spend_nullifier_inverse = meta.advice_column();
        let note_field_difference_inverse = meta.advice_column();
        let initial_root_inverse = meta.advice_column();
        let finalized_root_inverse = meta.advice_column();
        let root_difference_inverse = meta.advice_column();
        let asset_scale = meta.advice_column();
        let leaf_index = meta.advice_column();
        let rho = meta.advice_column();
        let spend_scalar = meta.advice_column();
        let diversifier = meta.advice_column();
        let owner_tag = meta.advice_column();
        let owner_asset = meta.advice_column();
        let rho_owner_asset = meta.advice_column();
        let asset_chain = meta.advice_column();
        let rho_asset_chain = meta.advice_column();
        let payer_tag = meta.advice_column();
        let operation_tag = meta.advice_column();
        let siblings = (0..DEPTH).map(|_| meta.advice_column()).collect::<Vec<_>>();
        let directions = (0..DEPTH).map(|_| meta.advice_column()).collect::<Vec<_>>();
        let zero_nodes = (0..DEPTH).map(|_| meta.advice_column()).collect::<Vec<_>>();
        let output_nodes = (0..DEPTH).map(|_| meta.advice_column()).collect::<Vec<_>>();
        let index_quotients = (0..=DEPTH)
            .map(|_| meta.advice_column())
            .collect::<Vec<_>>();
        let instances: [halo2_proofs::plonk::Column<halo2_proofs::plonk::Instance>; 11] =
            std::array::from_fn(|_| meta.instance_column());
        let selector = meta.selector();

        let gate_siblings = siblings.clone();
        let gate_directions = directions.clone();
        let gate_zero_nodes = zero_nodes.clone();
        let gate_output_nodes = output_nodes.clone();
        let gate_index_quotients = index_quotients.clone();
        let gate_amount_range = amount_range.clone();
        meta.create_gate("kagemusha_topup_shield_v2", move |meta| {
            let enabled = meta.query_selector(selector);
            let amount_value = meta.query_advice(amount, Rotation::cur());
            let amount_inverse_value = meta.query_advice(amount_inverse, Rotation::cur());
            let output_commitment_inverse_value =
                meta.query_advice(output_commitment_inverse, Rotation::cur());
            let spend_nullifier_inverse_value =
                meta.query_advice(spend_nullifier_inverse, Rotation::cur());
            let note_field_difference_inverse_value =
                meta.query_advice(note_field_difference_inverse, Rotation::cur());
            let initial_root_inverse_value =
                meta.query_advice(initial_root_inverse, Rotation::cur());
            let finalized_root_inverse_value =
                meta.query_advice(finalized_root_inverse, Rotation::cur());
            let root_difference_inverse_value =
                meta.query_advice(root_difference_inverse, Rotation::cur());
            let scale_value = meta.query_advice(asset_scale, Rotation::cur());
            let leaf_index_value = meta.query_advice(leaf_index, Rotation::cur());
            let rho_value = meta.query_advice(rho, Rotation::cur());
            let spend_value = meta.query_advice(spend_scalar, Rotation::cur());
            let diversifier_value = meta.query_advice(diversifier, Rotation::cur());
            let owner_value = meta.query_advice(owner_tag, Rotation::cur());
            let owner_asset_value = meta.query_advice(owner_asset, Rotation::cur());
            let rho_owner_asset_value = meta.query_advice(rho_owner_asset, Rotation::cur());
            let asset_chain_value = meta.query_advice(asset_chain, Rotation::cur());
            let rho_asset_chain_value = meta.query_advice(rho_asset_chain, Rotation::cur());
            let payer_value = meta.query_advice(payer_tag, Rotation::cur());
            let operation_value = meta.query_advice(operation_tag, Rotation::cur());
            let public = instances.map(|column| meta.query_instance(column, Rotation::cur()));
            let one = halo2_proofs::plonk::Expression::Constant(Scalar::ONE);
            let two = halo2_proofs::plonk::Expression::Constant(Scalar::from(2u64));
            let poseidon =
                |lhs, rhs| confidential_relation_gadget::poseidon_pair_expression(lhs, rhs);

            let mut constraints = vec![
                enabled.clone()
                    * (amount_value.clone() - gate_amount_range.query_value_at(meta, 0)),
                enabled.clone() * (amount_value.clone() * amount_inverse_value - one.clone()),
                enabled.clone()
                    * (public[0].clone() * output_commitment_inverse_value - one.clone()),
                enabled.clone() * (public[1].clone() * spend_nullifier_inverse_value - one.clone()),
                enabled.clone()
                    * ((public[0].clone() - public[1].clone())
                        * note_field_difference_inverse_value
                        - one.clone()),
                enabled.clone() * (public[2].clone() * initial_root_inverse_value - one.clone()),
                enabled.clone() * (public[3].clone() * finalized_root_inverse_value - one.clone()),
                enabled.clone()
                    * ((public[3].clone() - public[2].clone()) * root_difference_inverse_value
                        - one.clone()),
                enabled.clone() * (amount_value.clone() - public[4].clone()),
                enabled.clone() * (scale_value - public[5].clone()),
                enabled.clone() * (leaf_index_value - public[6].clone()),
                enabled.clone() * (payer_value - public[9].clone()),
                enabled.clone() * (operation_value - public[10].clone()),
                enabled.clone()
                    * (owner_value.clone() - poseidon(spend_value.clone(), diversifier_value)),
                enabled.clone()
                    * (owner_asset_value.clone() - poseidon(owner_value, public[7].clone())),
                enabled.clone()
                    * (rho_owner_asset_value.clone()
                        - poseidon(rho_value.clone(), owner_asset_value)),
                enabled.clone()
                    * (public[0].clone() - poseidon(amount_value, rho_owner_asset_value)),
                enabled.clone()
                    * (asset_chain_value.clone() - poseidon(public[7].clone(), public[8].clone())),
                enabled.clone()
                    * (rho_asset_chain_value.clone() - poseidon(rho_value, asset_chain_value)),
                enabled.clone()
                    * (public[1].clone() - poseidon(spend_value, rho_asset_chain_value)),
                enabled.clone()
                    * (meta.query_advice(gate_index_quotients[0], Rotation::cur())
                        - public[6].clone()),
            ];

            let mut zero_previous = halo2_proofs::plonk::Expression::Constant(Scalar::ZERO);
            let mut output_previous = public[0].clone();
            for level in 0..DEPTH {
                let sibling = meta.query_advice(gate_siblings[level], Rotation::cur());
                let direction = meta.query_advice(gate_directions[level], Rotation::cur());
                let zero_node = meta.query_advice(gate_zero_nodes[level], Rotation::cur());
                let output_node = meta.query_advice(gate_output_nodes[level], Rotation::cur());
                let index_current = meta.query_advice(gate_index_quotients[level], Rotation::cur());
                let index_next =
                    meta.query_advice(gate_index_quotients[level + 1], Rotation::cur());
                constraints
                    .push(enabled.clone() * direction.clone() * (direction.clone() - one.clone()));
                constraints.push(
                    enabled.clone()
                        * (index_current - direction.clone() - two.clone() * index_next),
                );
                constraints.push(
                    enabled.clone()
                        * (zero_node.clone()
                            - confidential_relation_gadget::merkle_parent_expression(
                                zero_previous.clone(),
                                sibling.clone(),
                                direction.clone(),
                            )),
                );
                constraints.push(
                    enabled.clone()
                        * (output_node.clone()
                            - confidential_relation_gadget::merkle_parent_expression(
                                output_previous.clone(),
                                sibling,
                                direction,
                            )),
                );
                zero_previous = zero_node;
                output_previous = output_node;
            }
            constraints.push(
                enabled.clone() * meta.query_advice(gate_index_quotients[DEPTH], Rotation::cur()),
            );
            constraints.push(enabled.clone() * (zero_previous - public[2].clone()));
            constraints.push(enabled * (output_previous - public[3].clone()));
            constraints
        });

        KagemushaTopUpShieldConfigV2 {
            amount_range,
            amount,
            amount_inverse,
            output_commitment_inverse,
            spend_nullifier_inverse,
            note_field_difference_inverse,
            initial_root_inverse,
            finalized_root_inverse,
            root_difference_inverse,
            asset_scale,
            leaf_index,
            rho,
            spend_scalar,
            diversifier,
            owner_tag,
            owner_asset,
            rho_owner_asset,
            asset_chain,
            rho_asset_chain,
            payer_tag,
            operation_tag,
            siblings,
            directions,
            zero_nodes,
            output_nodes,
            index_quotients,
            selector,
        }
    }

    fn synthesize(
        &self,
        config: Self::Config,
        mut layouter: impl Layouter<Scalar>,
    ) -> Result<(), PlonkError> {
        let witness = self.witness.clone();
        layouter.assign_region(
            || "kagemusha_topup_shield_v2",
            |mut region| {
                config.selector.enable(&mut region, 0)?;
                config.amount_range.assign(
                    &mut region,
                    0,
                    witness.as_ref().map(|value| value.amount),
                )?;
                let scalar_value = |value: Option<[u8; 32]>| {
                    value
                        .and_then(scalar_from_repr)
                        .map_or(Value::unknown(), Value::known)
                };
                let amount_value = witness.as_ref().map(|value| scalar_from_u128(value.amount));
                let rho_value = witness
                    .as_ref()
                    .map(|value| hash_to_scalar(b"iroha.confidential.v2.note_rho", &[&value.rho]));
                let owner_value = witness.as_ref().and_then(|value| {
                    Some(poseidon_pair(
                        scalar_from_repr(value.spend_scalar)?,
                        scalar_from_repr(value.diversifier)?,
                    ))
                });
                let owner_asset_value = witness.as_ref().and_then(|value| {
                    Some(poseidon_pair(
                        owner_value?,
                        scalar_from_repr(value.asset_tag)?,
                    ))
                });
                let rho_owner_asset_value = witness
                    .as_ref()
                    .and_then(|_| Some(poseidon_pair(rho_value?, owner_asset_value?)));
                let asset_chain_value = witness.as_ref().and_then(|value| {
                    Some(poseidon_pair(
                        scalar_from_repr(value.asset_tag)?,
                        scalar_from_repr(value.chain_tag)?,
                    ))
                });
                let rho_asset_chain_value = witness
                    .as_ref()
                    .and_then(|_| Some(poseidon_pair(rho_value?, asset_chain_value?)));
                let output_commitment_value = witness
                    .as_ref()
                    .and_then(|_| Some(poseidon_pair(amount_value?, rho_owner_asset_value?)));
                let spend_nullifier_value = witness.as_ref().and_then(|value| {
                    Some(poseidon_pair(
                        scalar_from_repr(value.spend_scalar)?,
                        rho_asset_chain_value?,
                    ))
                });
                let note_field_difference_value = witness
                    .as_ref()
                    .and_then(|_| Some(output_commitment_value? - spend_nullifier_value?));
                let initial_root_value = witness
                    .as_ref()
                    .and_then(|value| scalar_from_repr(value.zero_path.root));
                let finalized_root_value = witness.as_ref().and_then(|value| {
                    value
                        .output_nodes
                        .last()
                        .copied()
                        .and_then(scalar_from_repr)
                });
                let root_difference_value = witness
                    .as_ref()
                    .and_then(|_| Some(finalized_root_value? - initial_root_value?));
                let inverse =
                    |value: Option<Scalar>| value.and_then(|value| Option::from(value.invert()));
                let assign_scalar = |region: &mut halo2_proofs::circuit::Region<'_, Scalar>,
                                     label: &'static str,
                                     column,
                                     value: Option<Scalar>|
                 -> Result<(), PlonkError> {
                    super::assign_advice_compat(
                        region,
                        || label,
                        column,
                        0,
                        || value.map_or(Value::unknown(), Value::known),
                    )
                    .map(|_| ())
                };
                assign_scalar(&mut region, "amount", config.amount, amount_value)?;
                assign_scalar(
                    &mut region,
                    "amount_inverse",
                    config.amount_inverse,
                    inverse(amount_value),
                )?;
                assign_scalar(
                    &mut region,
                    "output_commitment_inverse",
                    config.output_commitment_inverse,
                    inverse(output_commitment_value),
                )?;
                assign_scalar(
                    &mut region,
                    "spend_nullifier_inverse",
                    config.spend_nullifier_inverse,
                    inverse(spend_nullifier_value),
                )?;
                assign_scalar(
                    &mut region,
                    "note_field_difference_inverse",
                    config.note_field_difference_inverse,
                    inverse(note_field_difference_value),
                )?;
                assign_scalar(
                    &mut region,
                    "initial_root_inverse",
                    config.initial_root_inverse,
                    inverse(initial_root_value),
                )?;
                assign_scalar(
                    &mut region,
                    "finalized_root_inverse",
                    config.finalized_root_inverse,
                    inverse(finalized_root_value),
                )?;
                assign_scalar(
                    &mut region,
                    "root_difference_inverse",
                    config.root_difference_inverse,
                    inverse(root_difference_value),
                )?;
                assign_scalar(
                    &mut region,
                    "asset_scale",
                    config.asset_scale,
                    witness
                        .as_ref()
                        .map(|value| Scalar::from(u64::from(value.asset_scale))),
                )?;
                assign_scalar(
                    &mut region,
                    "leaf_index",
                    config.leaf_index,
                    witness
                        .as_ref()
                        .map(|value| Scalar::from(u64::from(value.leaf_index))),
                )?;
                assign_scalar(&mut region, "rho", config.rho, rho_value)?;
                super::assign_advice_compat(
                    &mut region,
                    || "spend_scalar",
                    config.spend_scalar,
                    0,
                    || scalar_value(witness.as_ref().map(|value| value.spend_scalar)),
                )?;
                super::assign_advice_compat(
                    &mut region,
                    || "diversifier",
                    config.diversifier,
                    0,
                    || scalar_value(witness.as_ref().map(|value| value.diversifier)),
                )?;
                assign_scalar(&mut region, "owner_tag", config.owner_tag, owner_value)?;
                assign_scalar(
                    &mut region,
                    "owner_asset",
                    config.owner_asset,
                    owner_asset_value,
                )?;
                assign_scalar(
                    &mut region,
                    "rho_owner_asset",
                    config.rho_owner_asset,
                    rho_owner_asset_value,
                )?;
                assign_scalar(
                    &mut region,
                    "asset_chain",
                    config.asset_chain,
                    asset_chain_value,
                )?;
                assign_scalar(
                    &mut region,
                    "rho_asset_chain",
                    config.rho_asset_chain,
                    rho_asset_chain_value,
                )?;
                super::assign_advice_compat(
                    &mut region,
                    || "payer_tag",
                    config.payer_tag,
                    0,
                    || scalar_value(witness.as_ref().map(|value| value.payer_tag)),
                )?;
                super::assign_advice_compat(
                    &mut region,
                    || "operation_tag",
                    config.operation_tag,
                    0,
                    || scalar_value(witness.as_ref().map(|value| value.operation_tag)),
                )?;

                for level in 0..DEPTH {
                    super::assign_advice_compat(
                        &mut region,
                        || format!("sibling_{level}"),
                        config.siblings[level],
                        0,
                        || {
                            scalar_value(
                                witness
                                    .as_ref()
                                    .and_then(|value| value.zero_path.siblings.get(level).copied()),
                            )
                        },
                    )?;
                    assign_scalar(
                        &mut region,
                        "direction",
                        config.directions[level],
                        witness.as_ref().and_then(|value| {
                            value
                                .zero_path
                                .directions
                                .get(level)
                                .map(|direction| Scalar::from(u64::from(*direction)))
                        }),
                    )?;
                    super::assign_advice_compat(
                        &mut region,
                        || format!("zero_node_{level}"),
                        config.zero_nodes[level],
                        0,
                        || {
                            scalar_value(witness.as_ref().and_then(|value| {
                                value.zero_path.witness_nodes.get(level).copied()
                            }))
                        },
                    )?;
                    super::assign_advice_compat(
                        &mut region,
                        || format!("output_node_{level}"),
                        config.output_nodes[level],
                        0,
                        || {
                            scalar_value(
                                witness
                                    .as_ref()
                                    .and_then(|value| value.output_nodes.get(level).copied()),
                            )
                        },
                    )?;
                }
                for level in 0..=DEPTH {
                    assign_scalar(
                        &mut region,
                        "index_quotient",
                        config.index_quotients[level],
                        witness
                            .as_ref()
                            .map(|value| Scalar::from(u64::from(value.leaf_index >> level))),
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
    asset_tag: [u8; 32],
    chain_tag: [u8; 32],
    input_0_path: ConfidentialMerklePathV2,
    input_1_path: ConfidentialMerklePathV2,
}

#[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
impl Zeroize for ConfidentialUnshieldWitnessV2 {
    fn zeroize(&mut self) {
        self.include_input_1.zeroize();
        self.input_0_amount.zeroize();
        self.input_1_amount.zeroize();
        self.input_0_rho.zeroize();
        self.input_1_rho.zeroize();
        self.spend_scalar.zeroize();
        self.input_0_diversifier.zeroize();
        self.input_1_diversifier.zeroize();
        self.asset_tag.zeroize();
        self.chain_tag.zeroize();
        self.input_0_path.zeroize();
        self.input_1_path.zeroize();
    }
}

#[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
impl Drop for ConfidentialUnshieldWitnessV2 {
    fn drop(&mut self) {
        self.zeroize();
    }
}

#[cfg(all(any(feature = "zk-halo2", feature = "zk-halo2-ipa"), test))]
#[derive(Clone, Default)]
/// Full-unshield circuit shared by standalone proving and Kagemusha.
pub(super) struct ConfidentialUnshieldCircuitV2<const DEPTH: usize> {
    witness: Option<ConfidentialUnshieldWitnessV2>,
}

#[cfg(all(any(feature = "zk-halo2", feature = "zk-halo2-ipa"), test))]
impl<const DEPTH: usize> Zeroize for ConfidentialUnshieldCircuitV2<DEPTH> {
    fn zeroize(&mut self) {
        if let Some(witness) = &mut self.witness {
            witness.zeroize();
        }
        self.witness = None;
    }
}

#[cfg(all(any(feature = "zk-halo2", feature = "zk-halo2-ipa"), test))]
impl<const DEPTH: usize> Drop for ConfidentialUnshieldCircuitV2<DEPTH> {
    fn drop(&mut self) {
        self.zeroize();
    }
}

#[cfg(all(any(feature = "zk-halo2", feature = "zk-halo2-ipa"), test))]
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
                |lhs, rhs| confidential_relation_gadget::poseidon_pair_expression(lhs, rhs);
            let note_commit_expr =
                |amount: halo2_proofs::plonk::Expression<Scalar>,
                 rho: halo2_proofs::plonk::Expression<Scalar>,
                 owner_tag: halo2_proofs::plonk::Expression<Scalar>| {
                    confidential_relation_gadget::note_commitment_expression(
                        amount,
                        rho,
                        owner_tag,
                        asset_tag.clone(),
                    )
                };
            let nullifier_expr = |rho: halo2_proofs::plonk::Expression<Scalar>| {
                confidential_relation_gadget::nullifier_expression(
                    sk.clone(),
                    rho,
                    asset_tag.clone(),
                    chain_tag.clone(),
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
                constraints.push(
                    enabled.clone()
                        * (witness.clone()
                            - confidential_relation_gadget::merkle_parent_expression(
                                input_0_prev.clone(),
                                sibling,
                                direction,
                            )),
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
                constraints.push(
                    enabled.clone()
                        * (witness.clone()
                            - confidential_relation_gadget::merkle_parent_expression(
                                input_1_prev.clone(),
                                sibling,
                                direction,
                            )),
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
    asset_tag: [u8; 32],
    chain_tag: [u8; 32],
    input_0_path: ConfidentialMerklePathV2,
    input_1_path: ConfidentialMerklePathV2,
}

#[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
impl Zeroize for ConfidentialUnshieldWitnessV3 {
    fn zeroize(&mut self) {
        self.include_input_1.zeroize();
        self.include_output_0.zeroize();
        self.input_0_amount.zeroize();
        self.input_1_amount.zeroize();
        self.output_0_amount.zeroize();
        self.input_0_rho.zeroize();
        self.input_1_rho.zeroize();
        self.output_0_rho.zeroize();
        self.spend_scalar.zeroize();
        self.input_0_diversifier.zeroize();
        self.input_1_diversifier.zeroize();
        self.asset_tag.zeroize();
        self.chain_tag.zeroize();
        self.input_0_path.zeroize();
        self.input_1_path.zeroize();
    }
}

#[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
impl Drop for ConfidentialUnshieldWitnessV3 {
    fn drop(&mut self) {
        self.zeroize();
    }
}

#[cfg(all(any(feature = "zk-halo2", feature = "zk-halo2-ipa"), test))]
#[derive(Clone, Default)]
/// Change-preserving unshield circuit shared by standalone proving and Kagemusha.
pub(super) struct ConfidentialUnshieldCircuitV3<const DEPTH: usize> {
    witness: Option<ConfidentialUnshieldWitnessV3>,
}

#[cfg(all(any(feature = "zk-halo2", feature = "zk-halo2-ipa"), test))]
impl<const DEPTH: usize> Zeroize for ConfidentialUnshieldCircuitV3<DEPTH> {
    fn zeroize(&mut self) {
        if let Some(witness) = &mut self.witness {
            witness.zeroize();
        }
        self.witness = None;
    }
}

#[cfg(all(any(feature = "zk-halo2", feature = "zk-halo2-ipa"), test))]
impl<const DEPTH: usize> Drop for ConfidentialUnshieldCircuitV3<DEPTH> {
    fn drop(&mut self) {
        self.zeroize();
    }
}

#[cfg(all(any(feature = "zk-halo2", feature = "zk-halo2-ipa"), test))]
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
                |lhs, rhs| confidential_relation_gadget::poseidon_pair_expression(lhs, rhs);
            let change_owner_tag = poseidon_pair_expr(sk.clone(), one.clone());
            let note_commit_expr =
                |amount: halo2_proofs::plonk::Expression<Scalar>,
                 rho: halo2_proofs::plonk::Expression<Scalar>,
                 owner_tag: halo2_proofs::plonk::Expression<Scalar>| {
                    confidential_relation_gadget::note_commitment_expression(
                        amount,
                        rho,
                        owner_tag,
                        asset_tag.clone(),
                    )
                };
            let nullifier_expr = |rho: halo2_proofs::plonk::Expression<Scalar>| {
                confidential_relation_gadget::nullifier_expression(
                    sk.clone(),
                    rho,
                    asset_tag.clone(),
                    chain_tag.clone(),
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
                constraints.push(
                    enabled.clone()
                        * (witness.clone()
                            - confidential_relation_gadget::merkle_parent_expression(
                                input_0_prev.clone(),
                                sibling,
                                direction,
                            )),
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
                constraints.push(
                    enabled.clone()
                        * (witness.clone()
                            - confidential_relation_gadget::merkle_parent_expression(
                                input_1_prev.clone(),
                                sibling,
                                direction,
                            )),
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
) -> Result<(super::PastaParams, ConfidentialV2VerifyingKey), String> {
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
        secure_relation_v3::ConfidentialTransferCircuitV3<CONFIDENTIAL_TREE_DEPTH_V2>,
    >(vk_box.bytes.as_slice(), &params)
    .ok_or_else(|| {
        "missing/invalid H2VK payload for confidential transfer verifying key".to_owned()
    })?;
    Ok((params, parsed))
}

#[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
fn parse_vk_for_kagemusha_topup_shield_v2(
    circuit_id: &str,
    vk_box: &VerifyingKeyBox,
) -> Result<(super::PastaParams, ConfidentialV2VerifyingKey), String> {
    if vk_box.backend.as_str() != super::ZK_BACKEND_HALO2_IPA {
        return Err(
            "Kagemusha top-up shield v2 proving requires a halo2/ipa verifying key".to_owned(),
        );
    }
    if !is_kagemusha_topup_shield_v2_circuit_id(circuit_id) {
        return Err(format!(
            "unsupported Kagemusha top-up shield verifier circuit `{circuit_id}`"
        ));
    }
    let params = super::zkparse::params_any(vk_box.bytes.as_slice())
        .ok_or_else(|| "missing/invalid IPAK parameters in verifying key envelope".to_owned())?;
    let parsed = super::zkparse::vk_from_bytes::<
        secure_relation_v3::KagemushaTopUpShieldCircuitV3<CONFIDENTIAL_TREE_DEPTH_V2>,
    >(vk_box.bytes.as_slice(), &params)
    .ok_or_else(|| {
        "missing/invalid H2VK payload for Kagemusha top-up shield verifying key".to_owned()
    })?;
    Ok((params, parsed))
}

#[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
fn parse_vk_for_unshield_v2(
    circuit_id: &str,
    vk_box: &VerifyingKeyBox,
) -> Result<(super::PastaParams, ConfidentialV2VerifyingKey), String> {
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
        secure_relation_v3::ConfidentialUnshieldFullCircuitV3<CONFIDENTIAL_TREE_DEPTH_V2>,
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
) -> Result<(super::PastaParams, ConfidentialV2VerifyingKey), String> {
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
        secure_relation_v3::ConfidentialUnshieldChangeCircuitV4<CONFIDENTIAL_TREE_DEPTH_V2>,
    >(vk_box.bytes.as_slice(), &params)
    .ok_or_else(|| {
        "missing/invalid H2VK payload for confidential unshield verifying key".to_owned()
    })?;
    Ok((params, parsed))
}

#[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
fn parse_vk_for_asset_hidden_transfer_v1(
    circuit_id: &str,
    vk_box: &VerifyingKeyBox,
) -> Result<(super::PastaParams, ConfidentialV2VerifyingKey), String> {
    if vk_box.backend.as_str() != super::ZK_BACKEND_HALO2_IPA {
        return Err(
            "asset-hidden transfer v1 proving requires a halo2/ipa verifying key".to_owned(),
        );
    }
    if !is_asset_hidden_transfer_v1_circuit_id(circuit_id) {
        return Err(format!(
            "unsupported asset-hidden transfer verifier circuit `{circuit_id}`"
        ));
    }
    let params = super::zkparse::params_any(vk_box.bytes.as_slice())
        .ok_or_else(|| "missing/invalid IPAK parameters in verifying key envelope".to_owned())?;
    let parsed = super::zkparse::vk_from_bytes::<super::pasta_tiny::AssetHiddenTransferPublic>(
        vk_box.bytes.as_slice(),
        &params,
    )
    .ok_or_else(|| {
        "missing/invalid H2VK payload for asset-hidden transfer verifying key".to_owned()
    })?;
    Ok((params, parsed))
}

#[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
fn derive_confidential_v2_proving_key<C>(
    params: &super::PastaParams,
    parsed_vk: ConfidentialV2VerifyingKey,
    empty_circuit: &C,
    context: &str,
) -> Result<ConfidentialV2ProvingKey, String>
where
    C: Circuit<Scalar>,
{
    super::halo2_backend::keygen_pk(params, parsed_vk, empty_circuit)
        .map_err(|err| format!("failed to derive confidential {context} proving key: {err}"))
}

#[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
fn cached_confidential_transfer_v2_proving_key() -> Result<&'static ConfidentialV2ProvingKey, String>
{
    static CACHE: std::sync::OnceLock<Result<ConfidentialV2ProvingKey, String>> =
        std::sync::OnceLock::new();

    match CACHE.get_or_init(|| {
        let vk_box = confidential_transfer_v2_vk_box()?;
        let (params, parsed_vk) =
            parse_vk_for_transfer(CONFIDENTIAL_TRANSFER_V2_CIRCUIT_ID, &vk_box)?;
        derive_confidential_v2_proving_key(
            &params,
            parsed_vk,
            &secure_relation_v3::ConfidentialTransferCircuitV3::<
                CONFIDENTIAL_TREE_DEPTH_V2,
            >::default(),
            "transfer",
        )
    }) {
        Ok(proving_key) => Ok(proving_key),
        Err(err) => Err(err.clone()),
    }
}

#[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
fn cached_kagemusha_topup_shield_v2_proving_key()
-> Result<&'static ConfidentialV2ProvingKey, String> {
    static CACHE: std::sync::OnceLock<Result<ConfidentialV2ProvingKey, String>> =
        std::sync::OnceLock::new();

    match CACHE.get_or_init(|| {
        let vk_box = kagemusha_topup_shield_v2_vk_box()?;
        let (params, parsed_vk) =
            parse_vk_for_kagemusha_topup_shield_v2(KAGEMUSHA_TOPUP_SHIELD_V2_CIRCUIT_ID, &vk_box)?;
        derive_confidential_v2_proving_key(
            &params,
            parsed_vk,
            &secure_relation_v3::KagemushaTopUpShieldCircuitV3::<
                CONFIDENTIAL_TREE_DEPTH_V2,
            >::default(),
            "Kagemusha top-up shield v2",
        )
    }) {
        Ok(proving_key) => Ok(proving_key),
        Err(err) => Err(err.clone()),
    }
}

#[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
fn cached_confidential_unshield_v2_proving_key() -> Result<&'static ConfidentialV2ProvingKey, String>
{
    static CACHE: std::sync::OnceLock<Result<ConfidentialV2ProvingKey, String>> =
        std::sync::OnceLock::new();

    match CACHE.get_or_init(|| {
        let vk_box = confidential_unshield_v2_vk_box()?;
        let (params, parsed_vk) =
            parse_vk_for_unshield_v2(CONFIDENTIAL_UNSHIELD_V2_CIRCUIT_ID, &vk_box)?;
        derive_confidential_v2_proving_key(
            &params,
            parsed_vk,
            &secure_relation_v3::ConfidentialUnshieldFullCircuitV3::<
                CONFIDENTIAL_TREE_DEPTH_V2,
            >::default(),
            "unshield",
        )
    }) {
        Ok(proving_key) => Ok(proving_key),
        Err(err) => Err(err.clone()),
    }
}

#[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
fn cached_confidential_unshield_v3_proving_key() -> Result<&'static ConfidentialV2ProvingKey, String>
{
    static CACHE: std::sync::OnceLock<Result<ConfidentialV2ProvingKey, String>> =
        std::sync::OnceLock::new();

    match CACHE.get_or_init(|| {
        let vk_box = confidential_unshield_v3_vk_box()?;
        let (params, parsed_vk) =
            parse_vk_for_unshield_v3(CONFIDENTIAL_UNSHIELD_V3_CIRCUIT_ID, &vk_box)?;
        derive_confidential_v2_proving_key(
            &params,
            parsed_vk,
            &secure_relation_v3::ConfidentialUnshieldChangeCircuitV4::<
                CONFIDENTIAL_TREE_DEPTH_V2,
            >::default(),
            "unshield",
        )
    }) {
        Ok(proving_key) => Ok(proving_key),
        Err(err) => Err(err.clone()),
    }
}

#[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
fn cached_asset_hidden_transfer_v1_proving_key() -> Result<&'static ConfidentialV2ProvingKey, String>
{
    static CACHE: std::sync::OnceLock<Result<ConfidentialV2ProvingKey, String>> =
        std::sync::OnceLock::new();

    match CACHE.get_or_init(|| {
        let vk_box = asset_hidden_transfer_v1_vk_box()?;
        let (params, parsed_vk) =
            parse_vk_for_asset_hidden_transfer_v1(ASSET_HIDDEN_TRANSFER_V1_CIRCUIT_ID, &vk_box)?;
        derive_confidential_v2_proving_key(
            &params,
            parsed_vk,
            &super::pasta_tiny::AssetHiddenTransferPublic::default(),
            "asset-hidden transfer",
        )
    }) {
        Ok(proving_key) => Ok(proving_key),
        Err(err) => Err(err.clone()),
    }
}

#[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
fn create_confidential_v2_proof<C>(
    params: &super::PastaParams,
    proving_key: &ConfidentialV2ProvingKey,
    circuit: C,
    instance_wrapper: &[&[&[Scalar]]],
    context: &str,
) -> Result<Vec<u8>, String>
where
    C: Circuit<Scalar>,
{
    let proof_raw =
        super::halo2_backend::create_ipa_proof(params, proving_key, &[circuit], instance_wrapper)
            .map_err(|err| format!("failed to create confidential {context} proof: {err}"))?;
    Ok(proof_raw)
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
fn kagemusha_topup_output_path_nodes_v2(
    output_commitment: [u8; 32],
    path: &ConfidentialMerklePathV2,
) -> Result<Vec<[u8; 32]>, String> {
    let commitment = scalar_from_repr(output_commitment)
        .ok_or_else(|| "output commitment must be a canonical Pasta scalar".to_owned())?;
    let mut node =
        confidential_poseidon_hash_v3(CONFIDENTIAL_POSEIDON_MERKLE_LEAF_DOMAIN_V3, &[commitment]);
    let mut nodes = Vec::with_capacity(CONFIDENTIAL_TREE_DEPTH_V2);
    for level in 0..CONFIDENTIAL_TREE_DEPTH_V2 {
        let sibling = scalar_from_repr(path.siblings[level]).ok_or_else(|| {
            format!("top-up zero path sibling[{level}] must be a canonical Pasta scalar")
        })?;
        node = match path.directions[level] {
            0 => merkle_parent_v3(node, sibling),
            1 => merkle_parent_v3(sibling, node),
            _ => {
                return Err(format!(
                    "top-up zero path direction[{level}] must be 0 or 1"
                ));
            }
        };
        nodes.push(scalar_to_repr_bytes(node));
    }
    Ok(nodes)
}

#[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
fn validate_kagemusha_topup_shield_statement_v2(
    output_commitment: [u8; 32],
    spend_nullifier: [u8; 32],
    initial_root: [u8; 32],
    finalized_root: [u8; 32],
) -> Result<(), String> {
    if output_commitment == [0; 32] {
        return Err("Kagemusha top-up output commitment must be non-zero".to_owned());
    }
    if spend_nullifier == [0; 32] {
        return Err("Kagemusha top-up spend nullifier must be non-zero".to_owned());
    }
    if output_commitment == spend_nullifier {
        return Err(
            "Kagemusha top-up output commitment and spend nullifier must be distinct".to_owned(),
        );
    }
    if initial_root == [0; 32] {
        return Err("Kagemusha top-up initial root must be non-zero".to_owned());
    }
    if finalized_root == [0; 32] {
        return Err("Kagemusha top-up finalized root must be non-zero".to_owned());
    }
    if initial_root == finalized_root {
        return Err("Kagemusha top-up output must change the confidential root".to_owned());
    }
    Ok(())
}

/// Build a Kagemusha top-up shield proof from one exact note opening.
#[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
#[allow(clippy::too_many_arguments)]
pub fn build_kagemusha_topup_shield_proof_v2(
    chain_id: &ChainId,
    asset_definition_id: &str,
    payer: &str,
    operation_id: [u8; 32],
    atomic_amount: u128,
    asset_scale: u32,
    spend_key: &[u8],
    rho: [u8; 32],
    diversifier: [u8; 32],
    leaf_index: u32,
    zero_path: &ConfidentialMerklePathV2,
    circuit_id: &str,
    vk_box: &VerifyingKeyBox,
) -> Result<KagemushaTopUpShieldProofV2, String> {
    if asset_definition_id.is_empty() || asset_definition_id.trim() != asset_definition_id {
        return Err("Kagemusha top-up asset definition must be exact and non-empty".to_owned());
    }
    if payer.is_empty() || payer.trim() != payer {
        return Err("Kagemusha top-up payer must be exact and non-empty".to_owned());
    }
    if operation_id == [0; 32] {
        return Err("Kagemusha top-up operation_id must be non-zero".to_owned());
    }
    if atomic_amount == 0 {
        return Err("Kagemusha top-up atomic amount must be positive".to_owned());
    }
    if asset_scale > iroha_data_model::offline::KAGEMUSHA_SCALED_AMOUNT_MAX_SCALE_V2 {
        return Err("Kagemusha top-up asset scale exceeds the protocol maximum".to_owned());
    }
    if spend_key.len() != 32 || spend_key.iter().all(|byte| *byte == 0) {
        return Err("Kagemusha top-up spend key must be exactly 32 non-zero bytes".to_owned());
    }
    if rho == [0; 32] {
        return Err("Kagemusha top-up rho must be non-zero".to_owned());
    }
    let diversifier_scalar = scalar_from_repr(diversifier)
        .filter(|value| *value != Scalar::ZERO)
        .ok_or_else(|| "Kagemusha top-up diversifier must be a non-zero Pasta scalar".to_owned())?;
    let leaf_index_usize = usize::try_from(leaf_index)
        .map_err(|_| "Kagemusha top-up leaf_index does not fit usize".to_owned())?;
    if leaf_index_usize >= CONFIDENTIAL_TREE_CAPACITY_V2 {
        return Err(format!(
            "Kagemusha top-up leaf_index must be < {CONFIDENTIAL_TREE_CAPACITY_V2}"
        ));
    }

    ensure_kagemusha_topup_shield_v2_canonical_vk_box(vk_box)?;
    let (params, parsed_vk) = parse_vk_for_kagemusha_topup_shield_v2(circuit_id, vk_box)?;
    let initial_root = zero_path.root;
    let normalized_zero_path = normalize_supplied_confidential_merkle_path_v2(
        [0; 32],
        Some(leaf_index_usize),
        zero_path,
        initial_root,
        "Kagemusha top-up zero path",
    )?;
    let spend_scalar = hash_to_scalar(b"iroha.confidential.v3.spend_scalar", &[spend_key]);
    let owner_tag = confidential_poseidon_hash_v3(
        CONFIDENTIAL_POSEIDON_OWNER_DOMAIN_V3,
        &[spend_scalar, diversifier_scalar],
    );
    let asset_tag = derive_confidential_asset_tag_v3(asset_definition_id)?;
    let chain_tag = derive_confidential_chain_tag_v3(chain_id.as_str())?;
    let rho_scalar = hash_to_scalar(b"iroha.confidential.v3.note_rho", &[&rho]);
    let asset_scalar = scalar_from_repr(asset_tag).expect("derived asset tag is canonical");
    let chain_scalar = scalar_from_repr(chain_tag).expect("derived chain tag is canonical");
    let output_commitment = scalar_to_repr_bytes(confidential_poseidon_hash_v3(
        CONFIDENTIAL_POSEIDON_NOTE_DOMAIN_V3,
        &[
            scalar_from_u128(atomic_amount),
            rho_scalar,
            owner_tag,
            asset_scalar,
        ],
    ));
    let spend_nullifier = scalar_to_repr_bytes(confidential_poseidon_hash_v3(
        CONFIDENTIAL_POSEIDON_NULLIFIER_DOMAIN_V3,
        &[spend_scalar, rho_scalar, asset_scalar, chain_scalar],
    ));
    let output_nodes =
        kagemusha_topup_output_path_nodes_v2(output_commitment, &normalized_zero_path)?;
    let finalized_root = output_nodes
        .last()
        .copied()
        .ok_or_else(|| "Kagemusha top-up path must not be empty".to_owned())?;
    validate_kagemusha_topup_shield_statement_v2(
        output_commitment,
        spend_nullifier,
        initial_root,
        finalized_root,
    )?;
    let payer_tag = derive_kagemusha_topup_payer_tag_v3(payer)?;
    let operation_tag = derive_kagemusha_topup_operation_tag_v3(&operation_id)?;
    let witness = KagemushaTopUpShieldWitnessV2 {
        amount: atomic_amount,
        asset_scale,
        leaf_index,
        rho,
        spend_scalar: scalar_to_repr_bytes(spend_scalar),
        diversifier,
        asset_tag,
        chain_tag,
        payer_tag,
        operation_tag,
        zero_path: normalized_zero_path,
        output_nodes,
    };
    let circuit = secure_relation_v3::KagemushaTopUpShieldCircuitV3::<CONFIDENTIAL_TREE_DEPTH_V2> {
        witness: Some(witness),
    };
    let instance_columns = vec![
        vec![scalar_from_repr(output_commitment).expect("output commitment is canonical")],
        vec![scalar_from_repr(spend_nullifier).expect("spend nullifier is canonical")],
        vec![
            scalar_from_repr(initial_root)
                .ok_or_else(|| "initial root must be a canonical Pasta scalar".to_owned())?,
        ],
        vec![scalar_from_repr(finalized_root).expect("finalized root is canonical")],
        vec![scalar_from_u128(atomic_amount)],
        vec![Scalar::from(u64::from(asset_scale))],
        vec![Scalar::from(u64::from(leaf_index))],
        vec![scalar_from_repr(asset_tag).expect("asset tag is canonical")],
        vec![scalar_from_repr(chain_tag).expect("chain tag is canonical")],
        vec![scalar_from_repr(payer_tag).expect("payer tag is canonical")],
        vec![scalar_from_repr(operation_tag).expect("operation tag is canonical")],
    ];
    let instance_refs = instance_columns
        .iter()
        .map(Vec::as_slice)
        .collect::<Vec<_>>();
    let instance_wrapper = vec![instance_refs.as_slice()];
    let proof_raw = create_confidential_v2_proof(
        &params,
        cached_kagemusha_topup_shield_v2_proving_key()?,
        circuit,
        &instance_wrapper,
        "Kagemusha top-up shield v2",
    )?;
    {
        let proofs_instances = [&instance_refs[..]];
        super::halo2_backend::verify_ipa_proof(
            &params,
            &parsed_vk,
            proof_raw.as_slice(),
            &proofs_instances,
        )
        .map_err(|error| {
            format!(
                "generated Kagemusha top-up shield proof failed local self-verification: {error}"
            )
        })?;
    }
    let proof = encode_halo2_envelope(
        KAGEMUSHA_TOPUP_SHIELD_V2_CIRCUIT_ID,
        vk_box,
        KAGEMUSHA_TOPUP_SHIELD_V2_PUBLIC_INPUTS_SCHEMA_V2.to_vec(),
        &instance_columns,
        proof_raw,
    )?;
    Ok(KagemushaTopUpShieldProofV2 {
        output_commitment,
        spend_nullifier,
        initial_root,
        finalized_root,
        leaf_index,
        proof,
    })
}

/// Build an asset-hidden transfer proof for canonical public commitments.
#[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
pub fn build_asset_hidden_transfer_proof_v1(
    chain_id: &ChainId,
    pool_id: &str,
    asset_set_root: [u8; 32],
    input_commitments: &[[u8; 32]],
    nullifiers: &[[u8; 32]],
    output_commitments: &[[u8; 32]],
    root_hint: [u8; 32],
    circuit_id: &str,
    vk_box: &VerifyingKeyBox,
) -> Result<AssetHiddenTransferProofV1, String> {
    if pool_id.trim().is_empty() {
        return Err("asset-hidden transfer v1 pool_id must be non-empty".to_owned());
    }
    if asset_set_root == [0u8; 32] {
        return Err("asset-hidden transfer v1 asset_set_root must be nonzero".to_owned());
    }
    if input_commitments.is_empty() || input_commitments.len() > 2 {
        return Err("asset-hidden transfer v1 supports one or two input commitments".to_owned());
    }
    if nullifiers.is_empty() || nullifiers.len() > 2 {
        return Err("asset-hidden transfer v1 supports one or two nullifiers".to_owned());
    }
    if output_commitments.is_empty() || output_commitments.len() > 2 {
        return Err("asset-hidden transfer v1 supports one or two output commitments".to_owned());
    }
    if input_commitments.len() != nullifiers.len() {
        return Err(
            "asset-hidden transfer v1 input commitments must match nullifier count".to_owned(),
        );
    }
    for (index, nullifier) in nullifiers.iter().enumerate() {
        if *nullifier == [0u8; 32] {
            return Err(format!(
                "asset-hidden transfer v1 nullifier {index} must be nonzero"
            ));
        }
        if nullifiers[..index].iter().any(|seen| seen == nullifier) {
            return Err("asset-hidden transfer v1 duplicate nullifier".to_owned());
        }
    }
    for (index, commitment) in output_commitments.iter().enumerate() {
        if *commitment == [0u8; 32] {
            return Err(format!(
                "asset-hidden transfer v1 output commitment {index} must be nonzero"
            ));
        }
        if output_commitments[..index]
            .iter()
            .any(|seen| seen == commitment)
        {
            return Err("asset-hidden transfer v1 duplicate output commitment".to_owned());
        }
    }

    let (params, parsed_vk) = parse_vk_for_asset_hidden_transfer_v1(circuit_id, vk_box)?;
    let zero = [0u8; 32];
    let public_words = [
        derive_asset_hidden_pool_id_tag_v1(pool_id),
        asset_set_root,
        input_commitments.first().copied().unwrap_or(zero),
        input_commitments.get(1).copied().unwrap_or(zero),
        nullifiers.first().copied().unwrap_or(zero),
        nullifiers.get(1).copied().unwrap_or(zero),
        output_commitments.first().copied().unwrap_or(zero),
        output_commitments.get(1).copied().unwrap_or(zero),
        root_hint,
        derive_confidential_chain_tag_v3(chain_id.as_str())?,
    ];
    let values = public_words
        .into_iter()
        .enumerate()
        .map(|(index, word)| {
            scalar_from_repr(word).ok_or_else(|| {
                format!(
                    "asset-hidden transfer v1 public input column {index} must be a canonical Pasta scalar"
                )
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    let values: [Scalar; 10] = values
        .try_into()
        .map_err(|_| "asset-hidden transfer v1 public input shape mismatch".to_owned())?;
    let circuit = super::pasta_tiny::AssetHiddenTransferPublic { values };
    let instance_columns: Vec<Vec<Scalar>> = values.iter().map(|value| vec![*value]).collect();
    let instance_refs: Vec<&[Scalar]> = instance_columns.iter().map(Vec::as_slice).collect();
    let instance_wrapper = vec![instance_refs.as_slice()];
    let proof_raw = if ensure_asset_hidden_transfer_v1_canonical_vk_box(vk_box).is_ok() {
        create_confidential_v2_proof(
            &params,
            cached_asset_hidden_transfer_v1_proving_key()?,
            circuit,
            &instance_wrapper,
            "asset-hidden transfer",
        )?
    } else {
        let proving_key = derive_confidential_v2_proving_key(
            &params,
            parsed_vk.clone(),
            &super::pasta_tiny::AssetHiddenTransferPublic::default(),
            "asset-hidden transfer",
        )?;
        create_confidential_v2_proof(
            &params,
            &proving_key,
            circuit,
            &instance_wrapper,
            "asset-hidden transfer",
        )?
    };
    {
        let proofs_instances = [&instance_refs[..]];
        super::halo2_backend::verify_ipa_proof(
            &params,
            &parsed_vk,
            proof_raw.as_slice(),
            &proofs_instances,
        )
        .map_err(|err| {
            format!("generated asset-hidden transfer proof failed local self-verification: {err}")
        })?;
    }
    let proof = encode_halo2_envelope(
        ASSET_HIDDEN_TRANSFER_V1_CIRCUIT_ID,
        vk_box,
        ASSET_HIDDEN_TRANSFER_V1_PUBLIC_INPUTS_SCHEMA_V1.to_vec(),
        &instance_columns,
        proof_raw,
    )?;
    Ok(AssetHiddenTransferProofV1 {
        input_commitments: input_commitments.to_vec(),
        nullifiers: nullifiers.to_vec(),
        output_commitments: output_commitments.to_vec(),
        root: root_hint,
        proof,
    })
}

#[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
#[allow(clippy::too_many_arguments)]
fn build_confidential_transfer_proof_v2_resolved_paths(
    chain_id: &ChainId,
    asset_definition_id: &str,
    spend_key: &[u8],
    inputs: &[ConfidentialTransferInputV2],
    outputs: &[ConfidentialTransferOutputV2],
    root_hint: [u8; 32],
    circuit_id: &str,
    vk_box: &VerifyingKeyBox,
    resolve_input_paths: impl FnOnce(
        &ConfidentialTransferInputV2,
        Option<&ConfidentialTransferInputV2>,
        [u8; 32],
        [u8; 32],
    ) -> Result<
        (ConfidentialMerklePathV2, ConfidentialMerklePathV2),
        String,
    >,
) -> Result<ConfidentialTransferProofV2, String> {
    if inputs.is_empty() || inputs.len() > 2 {
        return Err("confidential transfer v2 supports one or two inputs".to_owned());
    }
    if outputs.is_empty() || outputs.len() > 2 {
        return Err("confidential transfer v2 supports one or two outputs".to_owned());
    }
    let input_total = inputs.iter().try_fold(0u128, |sum, input| {
        sum.checked_add(input.amount)
            .ok_or_else(|| "confidential transfer input sum overflows u128".to_owned())
    })?;
    let output_total = outputs.iter().try_fold(0u128, |sum, output| {
        sum.checked_add(output.amount)
            .ok_or_else(|| "confidential transfer output sum overflows u128".to_owned())
    })?;
    if input_total != output_total {
        return Err("confidential transfer input and output sums must match exactly".to_owned());
    }
    let (params, parsed_vk) = parse_vk_for_transfer(circuit_id, vk_box)?;
    let spend_scalar = hash_to_scalar(b"iroha.confidential.v3.spend_scalar", &[spend_key]);
    let spend_scalar_bytes = Zeroizing::new(scalar_to_repr_bytes(spend_scalar));
    let asset_tag = derive_confidential_asset_tag_v3(asset_definition_id)?;
    let chain_tag = derive_confidential_chain_tag_v3(chain_id.as_str())?;
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
    let (input_0_path, input_1_path) = resolve_input_paths(
        &input_0,
        input_1.as_ref(),
        input_0_commitment,
        input_1_commitment,
    )?;
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
        derive_confidential_nullifier_v3(spend_key, input_0.rho, asset_tag, chain_tag)?;
    let nullifier_1 = if let Some(note) = input_1.as_ref() {
        derive_confidential_nullifier_v3(spend_key, note.rho, asset_tag, chain_tag)?
    } else {
        [0u8; 32]
    };
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
        spend_scalar: *spend_scalar_bytes,
        input_0_diversifier: input_0.diversifier,
        input_1_diversifier: input_1.as_ref().map_or([0; 32], |note| note.diversifier),
        output_0_owner_tag: output_0.owner_tag,
        output_1_owner_tag: output_1.as_ref().map_or([0u8; 32], |note| note.owner_tag),
        asset_tag,
        chain_tag,
        input_0_path,
        input_1_path,
    };
    let circuit = secure_relation_v3::ConfidentialTransferCircuitV3::<CONFIDENTIAL_TREE_DEPTH_V2> {
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
    let proof_raw = if ensure_confidential_transfer_v2_canonical_vk_box(vk_box).is_ok() {
        create_confidential_v2_proof(
            &params,
            cached_confidential_transfer_v2_proving_key()?,
            circuit,
            &instance_wrapper,
            "transfer",
        )?
    } else {
        let proving_key = derive_confidential_v2_proving_key(
            &params,
            parsed_vk.clone(),
            &secure_relation_v3::ConfidentialTransferCircuitV3::<
                CONFIDENTIAL_TREE_DEPTH_V2,
            >::default(),
            "transfer",
        )?;
        create_confidential_v2_proof(
            &params,
            &proving_key,
            circuit,
            &instance_wrapper,
            "transfer",
        )?
    };
    {
        let proofs_instances = [&instance_refs[..]];
        super::halo2_backend::verify_ipa_proof(
            &params,
            &parsed_vk,
            proof_raw.as_slice(),
            &proofs_instances,
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

/// Build a confidential transfer proof, deriving input paths from the tree.
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
    let computed_root = compute_confidential_root_v2(tree_commitments)?;
    if computed_root != root_hint {
        return Err("tree commitments do not match the supplied root_hint".to_owned());
    }
    build_confidential_transfer_proof_v2_resolved_paths(
        chain_id,
        asset_definition_id,
        spend_key,
        inputs,
        outputs,
        root_hint,
        circuit_id,
        vk_box,
        |input_0, input_1, input_0_commitment, input_1_commitment| {
            if tree_commitments
                .get(input_0.leaf_index)
                .copied()
                .unwrap_or_default()
                != input_0_commitment
            {
                return Err(
                    "transfer input 0 does not match the current confidential tree".to_owned(),
                );
            }
            if let Some(note) = input_1
                && tree_commitments
                    .get(note.leaf_index)
                    .copied()
                    .unwrap_or_default()
                    != input_1_commitment
            {
                return Err(
                    "transfer input 1 does not match the current confidential tree".to_owned(),
                );
            }
            let input_0_path =
                compute_confidential_merkle_path_v2(tree_commitments, input_0.leaf_index)?;
            let input_1_path = compute_confidential_merkle_path_v2(
                tree_commitments,
                input_1
                    .as_ref()
                    .map_or(tree_commitments.len(), |note| note.leaf_index),
            )?;
            if input_0_path.root != root_hint || input_1_path.root != root_hint {
                return Err("computed confidential Merkle path does not match root_hint".to_owned());
            }
            Ok((input_0_path, input_1_path))
        },
    )
}

/// Build a confidential transfer proof using explicitly supplied input paths.
#[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
pub fn build_confidential_transfer_proof_v2_with_paths(
    chain_id: &ChainId,
    asset_definition_id: &str,
    spend_key: &[u8],
    input_paths: &[ConfidentialMerklePathV2],
    inputs: &[ConfidentialTransferInputV2],
    outputs: &[ConfidentialTransferOutputV2],
    root_hint: [u8; 32],
    circuit_id: &str,
    vk_box: &VerifyingKeyBox,
) -> Result<ConfidentialTransferProofV2, String> {
    build_confidential_transfer_proof_v2_resolved_paths(
        chain_id,
        asset_definition_id,
        spend_key,
        inputs,
        outputs,
        root_hint,
        circuit_id,
        vk_box,
        |input_0, input_1, input_0_commitment, input_1_commitment| {
            let expected_paths = 2;
            if input_paths.len() != expected_paths {
                return Err(format!(
                    "confidential transfer v2 path mode requires exactly {expected_paths} input paths"
                ));
            }
            let input_0_path = normalize_supplied_confidential_merkle_path_v2(
                input_0_commitment,
                Some(input_0.leaf_index),
                &input_paths[0],
                root_hint,
                "transfer input 0 path",
            )?;
            let input_1_path = if let Some(note) = input_1 {
                normalize_supplied_confidential_merkle_path_v2(
                    input_1_commitment,
                    Some(note.leaf_index),
                    &input_paths[1],
                    root_hint,
                    "transfer input 1 path",
                )?
            } else {
                normalize_supplied_confidential_merkle_path_v2(
                    [0u8; 32],
                    None,
                    &input_paths[1],
                    root_hint,
                    "transfer dummy input 1 path",
                )?
            };
            Ok((input_0_path, input_1_path))
        },
    )
}

/// Build a full confidential unshield proof.
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
    let expected_public_amount = inputs.iter().try_fold(0u128, |sum, input| {
        sum.checked_add(input.amount)
            .ok_or_else(|| "full-unshield input sum overflows u128".to_owned())
    })?;
    if public_amount == 0 || public_amount != expected_public_amount {
        return Err(
            "full-unshield public amount must equal the exact non-zero input sum".to_owned(),
        );
    }
    let computed_root = compute_confidential_root_v2(tree_commitments)?;
    if computed_root != root_hint {
        return Err("tree commitments do not match the supplied root_hint".to_owned());
    }
    let (params, parsed_vk) = parse_vk_for_unshield_v2(circuit_id, vk_box)?;
    let spend_scalar = hash_to_scalar(b"iroha.confidential.v3.spend_scalar", &[spend_key]);
    let spend_scalar_bytes = Zeroizing::new(scalar_to_repr_bytes(spend_scalar));
    let asset_tag = derive_confidential_asset_tag_v3(asset_definition_id)?;
    let chain_tag = derive_confidential_chain_tag_v3(chain_id.as_str())?;
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
    let nullifier_0 =
        derive_confidential_nullifier_v3(spend_key, input_0.rho, asset_tag, chain_tag)?;
    let nullifier_1 = if let Some(note) = input_1.as_ref() {
        derive_confidential_nullifier_v3(spend_key, note.rho, asset_tag, chain_tag)?
    } else {
        [0u8; 32]
    };
    let witness = ConfidentialUnshieldWitnessV2 {
        include_input_1: input_1.is_some(),
        input_0_amount: input_0.amount,
        input_1_amount: input_1.as_ref().map_or(0, |note| note.amount),
        input_0_rho: input_0.rho,
        input_1_rho: input_1.as_ref().map_or([0u8; 32], |note| note.rho),
        spend_scalar: *spend_scalar_bytes,
        input_0_diversifier: input_0.diversifier,
        input_1_diversifier: input_1.as_ref().map_or([0; 32], |note| note.diversifier),
        asset_tag,
        chain_tag,
        input_0_path,
        input_1_path,
    };
    let circuit =
        secure_relation_v3::ConfidentialUnshieldFullCircuitV3::<CONFIDENTIAL_TREE_DEPTH_V2> {
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
    let proof_raw = if ensure_confidential_unshield_v2_canonical_vk_box(vk_box).is_ok() {
        create_confidential_v2_proof(
            &params,
            cached_confidential_unshield_v2_proving_key()?,
            circuit,
            &instance_wrapper,
            "unshield",
        )?
    } else {
        let proving_key = derive_confidential_v2_proving_key(
            &params,
            parsed_vk.clone(),
            &secure_relation_v3::ConfidentialUnshieldFullCircuitV3::<
                CONFIDENTIAL_TREE_DEPTH_V2,
            >::default(),
            "unshield",
        )?;
        create_confidential_v2_proof(
            &params,
            &proving_key,
            circuit,
            &instance_wrapper,
            "unshield",
        )?
    };
    let proof = encode_halo2_envelope(
        CONFIDENTIAL_UNSHIELD_V2_CIRCUIT_ID,
        vk_box,
        CONFIDENTIAL_UNSHIELD_V2_PUBLIC_INPUTS_SCHEMA_V1.to_vec(),
        &instance_columns,
        proof_raw,
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
#[allow(clippy::too_many_arguments)]
fn build_confidential_unshield_proof_v3_resolved_paths(
    chain_id: &ChainId,
    asset_definition_id: &str,
    spend_key: &[u8],
    inputs: &[ConfidentialUnshieldInputV2],
    outputs: &[ConfidentialUnshieldOutputV3],
    public_amount: u128,
    root_hint: [u8; 32],
    circuit_id: &str,
    vk_box: &VerifyingKeyBox,
    resolve_input_paths: impl FnOnce(
        &ConfidentialUnshieldInputV2,
        Option<&ConfidentialUnshieldInputV2>,
        [u8; 32],
        [u8; 32],
    ) -> Result<
        (ConfidentialMerklePathV2, ConfidentialMerklePathV2),
        String,
    >,
) -> Result<ConfidentialUnshieldProofV3, String> {
    if inputs.is_empty() || inputs.len() > 2 {
        return Err("confidential unshield v3 supports one or two inputs".to_owned());
    }
    if outputs.len() != 1 {
        return Err("change-unshield requires exactly one private change output".to_owned());
    }
    let (params, parsed_vk) = parse_vk_for_unshield_v3(circuit_id, vk_box)?;
    let change_owner_tag = derive_confidential_owner_tag_v2(spend_key)?;
    let spend_scalar = hash_to_scalar(b"iroha.confidential.v3.spend_scalar", &[spend_key]);
    let spend_scalar_bytes = Zeroizing::new(scalar_to_repr_bytes(spend_scalar));
    let asset_tag = derive_confidential_asset_tag_v3(asset_definition_id)?;
    let chain_tag = derive_confidential_chain_tag_v3(chain_id.as_str())?;
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
    let (input_0_path, input_1_path) = resolve_input_paths(
        &input_0,
        input_1.as_ref(),
        input_0_commitment,
        input_1_commitment,
    )?;
    let total_input_amount = input_0
        .amount
        .checked_add(input_1.as_ref().map_or(0, |note| note.amount))
        .ok_or_else(|| "confidential unshield v3 input amount sum overflows u128".to_owned())?;
    let expected_change_amount = total_input_amount
        .checked_sub(public_amount)
        .ok_or_else(|| "public amount exceeds the available confidential inputs".to_owned())?;
    let output_note = output_0
        .as_ref()
        .expect("exactly one change output was validated");
    if output_note.amount != expected_change_amount || expected_change_amount == 0 {
        return Err(
            "change-unshield output amount must be the exact non-zero remainder".to_owned(),
        );
    }
    let output_0_commitment = derive_confidential_note_v2(
        asset_definition_id,
        output_note.amount,
        output_note.rho,
        change_owner_tag,
    )?;
    let nullifier_0 =
        derive_confidential_nullifier_v3(spend_key, input_0.rho, asset_tag, chain_tag)?;
    let nullifier_1 = if let Some(note) = input_1.as_ref() {
        derive_confidential_nullifier_v3(spend_key, note.rho, asset_tag, chain_tag)?
    } else {
        [0u8; 32]
    };
    let witness = ConfidentialUnshieldWitnessV3 {
        include_input_1: input_1.is_some(),
        include_output_0: output_0.is_some(),
        input_0_amount: input_0.amount,
        input_1_amount: input_1.as_ref().map_or(0, |note| note.amount),
        output_0_amount: output_0.as_ref().map_or(0, |note| note.amount),
        input_0_rho: input_0.rho,
        input_1_rho: input_1.as_ref().map_or([0u8; 32], |note| note.rho),
        output_0_rho: output_0.as_ref().map_or([0u8; 32], |note| note.rho),
        spend_scalar: *spend_scalar_bytes,
        input_0_diversifier: input_0.diversifier,
        input_1_diversifier: input_1.as_ref().map_or([0; 32], |note| note.diversifier),
        asset_tag,
        chain_tag,
        input_0_path,
        input_1_path,
    };
    let circuit =
        secure_relation_v3::ConfidentialUnshieldChangeCircuitV4::<CONFIDENTIAL_TREE_DEPTH_V2> {
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
    let proof_raw = if ensure_confidential_unshield_v3_canonical_vk_box(vk_box).is_ok() {
        create_confidential_v2_proof(
            &params,
            cached_confidential_unshield_v3_proving_key()?,
            circuit,
            &instance_wrapper,
            "unshield",
        )?
    } else {
        let proving_key =
            derive_confidential_v2_proving_key(
                &params,
                parsed_vk.clone(),
                &secure_relation_v3::ConfidentialUnshieldChangeCircuitV4::<
                    CONFIDENTIAL_TREE_DEPTH_V2,
                >::default(),
                "unshield",
            )?;
        create_confidential_v2_proof(
            &params,
            &proving_key,
            circuit,
            &instance_wrapper,
            "unshield",
        )?
    };
    let proof = encode_halo2_envelope(
        CONFIDENTIAL_UNSHIELD_V3_CIRCUIT_ID,
        vk_box,
        CONFIDENTIAL_UNSHIELD_V3_PUBLIC_INPUTS_SCHEMA_V1.to_vec(),
        &instance_columns,
        proof_raw,
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

/// Build a change-preserving confidential unshield proof, deriving input paths.
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
    let computed_root = compute_confidential_root_v2(tree_commitments)?;
    if computed_root != root_hint {
        return Err("tree commitments do not match the supplied root_hint".to_owned());
    }
    build_confidential_unshield_proof_v3_resolved_paths(
        chain_id,
        asset_definition_id,
        spend_key,
        inputs,
        outputs,
        public_amount,
        root_hint,
        circuit_id,
        vk_box,
        |input_0, input_1, input_0_commitment, input_1_commitment| {
            if tree_commitments
                .get(input_0.leaf_index)
                .copied()
                .unwrap_or_default()
                != input_0_commitment
            {
                return Err(
                    "unshield input 0 does not match the current confidential tree".to_owned(),
                );
            }
            if let Some(note) = input_1
                && tree_commitments
                    .get(note.leaf_index)
                    .copied()
                    .unwrap_or_default()
                    != input_1_commitment
            {
                return Err(
                    "unshield input 1 does not match the current confidential tree".to_owned(),
                );
            }
            let input_0_path =
                compute_confidential_merkle_path_v2(tree_commitments, input_0.leaf_index)?;
            let input_1_path = compute_confidential_merkle_path_v2(
                tree_commitments,
                input_1
                    .as_ref()
                    .map_or(tree_commitments.len(), |note| note.leaf_index),
            )?;
            if input_0_path.root != root_hint || input_1_path.root != root_hint {
                return Err("computed confidential Merkle path does not match root_hint".to_owned());
            }
            Ok((input_0_path, input_1_path))
        },
    )
}

/// Build a change-preserving unshield proof using explicitly supplied paths.
#[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
pub fn build_confidential_unshield_proof_v3_with_paths(
    chain_id: &ChainId,
    asset_definition_id: &str,
    spend_key: &[u8],
    input_paths: &[ConfidentialMerklePathV2],
    inputs: &[ConfidentialUnshieldInputV2],
    outputs: &[ConfidentialUnshieldOutputV3],
    public_amount: u128,
    root_hint: [u8; 32],
    circuit_id: &str,
    vk_box: &VerifyingKeyBox,
) -> Result<ConfidentialUnshieldProofV3, String> {
    build_confidential_unshield_proof_v3_resolved_paths(
        chain_id,
        asset_definition_id,
        spend_key,
        inputs,
        outputs,
        public_amount,
        root_hint,
        circuit_id,
        vk_box,
        |input_0, input_1, input_0_commitment, input_1_commitment| {
            let expected_paths = 2;
            if input_paths.len() != expected_paths {
                return Err(format!(
                    "confidential unshield v3 path mode requires exactly {expected_paths} input paths"
                ));
            }
            let input_0_path = normalize_supplied_confidential_merkle_path_v2(
                input_0_commitment,
                Some(input_0.leaf_index),
                &input_paths[0],
                root_hint,
                "unshield input 0 path",
            )?;
            let input_1_path = if let Some(note) = input_1 {
                normalize_supplied_confidential_merkle_path_v2(
                    input_1_commitment,
                    Some(note.leaf_index),
                    &input_paths[1],
                    root_hint,
                    "unshield input 1 path",
                )?
            } else {
                normalize_supplied_confidential_merkle_path_v2(
                    [0u8; 32],
                    None,
                    &input_paths[1],
                    root_hint,
                    "unshield dummy input 1 path",
                )?
            };
            Ok((input_0_path, input_1_path))
        },
    )
}

#[cfg(test)]
mod tests {
    #[test]
    fn production_circuit_selectors_reject_noncanonical_aliases() {
        let selectors: [(&str, fn(&str) -> bool); 5] = [
            (
                super::CONFIDENTIAL_TRANSFER_V2_CIRCUIT_ID,
                super::is_confidential_transfer_v2_circuit_id,
            ),
            (
                super::KAGEMUSHA_TOPUP_SHIELD_V2_CIRCUIT_ID,
                super::is_kagemusha_topup_shield_v2_circuit_id,
            ),
            (
                super::CONFIDENTIAL_UNSHIELD_V2_CIRCUIT_ID,
                super::is_confidential_unshield_v2_circuit_id,
            ),
            (
                super::CONFIDENTIAL_UNSHIELD_V3_CIRCUIT_ID,
                super::is_confidential_unshield_v3_circuit_id,
            ),
            (
                super::ASSET_HIDDEN_TRANSFER_V1_CIRCUIT_ID,
                super::is_asset_hidden_transfer_v1_circuit_id,
            ),
        ];
        for (canonical, accepts) in selectors {
            assert!(accepts(canonical));
            assert!(!accepts(&format!(" {canonical}")));
            assert!(!accepts(&format!("{canonical} ")));
            let bare = canonical
                .strip_prefix("halo2/pasta/ipa/")
                .expect("production circuit IDs use the canonical IPA prefix");
            assert!(!accepts(bare));
            assert!(!accepts(&format!("halo2/pasta/{bare}")));
        }
    }

    #[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
    #[test]
    fn retired_single_expression_poseidon_pair_has_constructive_collisions() {
        use halo2_proofs::halo2curves::{
            ff::Field,
            pasta::{Fp, Fq},
        };

        fn fifth_power<F: Field>(value: F) -> F {
            let square = value.square();
            square.square() * value
        }

        fn broken_pair<F>(lhs: F, rhs: F) -> F
        where
            F: Field + From<u64>,
        {
            F::from(2) * fifth_power(lhs + F::from(7)) + F::from(3) * fifth_power(rhs + F::from(13))
        }

        fn assert_constructive_collision<F>(inverse_five: [u64; 4])
        where
            F: Field + From<u64> + PartialEq + core::fmt::Debug,
        {
            let lhs = F::from(5);
            let rhs = F::from(9);
            let replacement_shifted_rhs = F::from(31);
            let shifted_lhs = lhs + F::from(7);
            let shifted_rhs = rhs + F::from(13);
            let half = F::from(2).invert().unwrap();
            let replacement_shifted_lhs_fifth = fifth_power(shifted_lhs)
                + F::from(3)
                    * half
                    * (fifth_power(shifted_rhs) - fifth_power(replacement_shifted_rhs));
            let replacement_shifted_lhs = replacement_shifted_lhs_fifth.pow_vartime(inverse_five);
            let replacement = (
                replacement_shifted_lhs - F::from(7),
                replacement_shifted_rhs - F::from(13),
            );
            assert_ne!((lhs, rhs), replacement);
            assert_eq!(
                broken_pair(lhs, rhs),
                broken_pair(replacement.0, replacement.1)
            );
        }

        assert_constructive_collision::<Fp>([
            0xe0f0_f3f0_cccc_cccd,
            0x4e9e_e0c9_a10a_60e2,
            0x3333_3333_3333_3333,
            0x3333_3333_3333_3333,
        ]);
        assert_constructive_collision::<Fq>([
            0xd69f_2280_cccc_cccd,
            0x4e9e_e0c9_a143_ba4a,
            0x3333_3333_3333_3333,
            0x3333_3333_3333_3333,
        ]);
    }

    #[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
    #[test]
    fn secure_confidential_poseidon_host_and_chip_match_all_domains_on_both_pasta_fields() {
        use halo2_base::{gates::circuit::builder::BaseCircuitBuilder, utils::BigPrimeField};
        use halo2_proofs::{
            dev::MockProver,
            halo2curves::{
                ff::PrimeField,
                pasta::{Fp, Fq},
            },
        };
        use snark_verifier::util::arithmetic::FieldExt;

        fn check<F>()
        where
            F: BigPrimeField + FieldExt,
        {
            const K: usize = 11;
            let uses = [
                (super::CONFIDENTIAL_POSEIDON_OWNER_DOMAIN_V3, &[3, 5][..]),
                (super::CONFIDENTIAL_POSEIDON_NOTE_DOMAIN_V3, &[3, 5, 8, 13]),
                (
                    super::CONFIDENTIAL_POSEIDON_NULLIFIER_DOMAIN_V3,
                    &[3, 5, 8, 13],
                ),
                (super::CONFIDENTIAL_POSEIDON_MERKLE_LEAF_DOMAIN_V3, &[3]),
                (super::CONFIDENTIAL_POSEIDON_MERKLE_NODE_DOMAIN_V3, &[3, 5]),
                (super::CONFIDENTIAL_POSEIDON_ASSET_DOMAIN_V3, &[3]),
                (super::CONFIDENTIAL_POSEIDON_CHAIN_DOMAIN_V3, &[3]),
                (super::CONFIDENTIAL_POSEIDON_PAYER_DOMAIN_V3, &[3]),
                (super::CONFIDENTIAL_POSEIDON_OPERATION_DOMAIN_V3, &[3]),
            ];
            let expected = uses
                .iter()
                .map(|(domain, inputs)| {
                    let inputs = inputs.iter().copied().map(F::from).collect::<Vec<_>>();
                    super::confidential_poseidon_hash_v3(*domain, &inputs)
                })
                .collect::<Vec<_>>();
            assert!(
                expected
                    .iter()
                    .all(|value| { value.to_repr().as_ref().iter().any(|byte| *byte != 0) })
            );

            let mut builder = BaseCircuitBuilder::new(false)
                .use_k(K)
                .use_lookup_bits(K - 1);
            let range = builder.range_chip();
            let outputs = {
                let ctx = builder.main(0);
                let chip = super::confidential_relation_gadget::ConfidentialPoseidonChipV3::new(
                    ctx, &range,
                );
                uses.iter()
                    .map(|(domain, inputs)| {
                        let assigned = ctx.assign_witnesses(inputs.iter().copied().map(F::from));
                        chip.hash(ctx, &range, *domain, &assigned)
                    })
                    .collect::<Vec<_>>()
            };
            builder.assigned_instances = vec![outputs];
            builder.calculate_params(Some(9));
            MockProver::run(K as u32, &builder, vec![expected])
                .expect("secure Poseidon mock prover")
                .assert_satisfied();
        }

        check::<Fp>();
        check::<Fq>();
    }

    #[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
    #[test]
    fn secure_confidential_poseidon_kats_pin_both_pasta_fields_and_domains() {
        use halo2_proofs::halo2curves::{
            ff::PrimeField,
            pasta::{Fp, Fq},
        };

        fn repr<F>(domain: u64) -> [u8; 32]
        where
            F: snark_verifier::util::arithmetic::FieldExt,
        {
            repr_inputs::<F>(domain, &[3, 5, 8, 13])
        }

        fn repr_inputs<F>(domain: u64, inputs: &[u64]) -> [u8; 32]
        where
            F: snark_verifier::util::arithmetic::FieldExt,
        {
            let inputs = inputs.iter().copied().map(F::from).collect::<Vec<_>>();
            let value = super::confidential_poseidon_hash_v3(domain, &inputs);
            value
                .to_repr()
                .as_ref()
                .try_into()
                .expect("32-byte Pasta repr")
        }

        fn hex32(value: &str) -> [u8; 32] {
            assert_eq!(value.len(), 64);
            std::array::from_fn(|index| {
                u8::from_str_radix(&value[index * 2..index * 2 + 2], 16)
                    .expect("valid KAT hex byte")
            })
        }

        let vectors = [
            (
                super::CONFIDENTIAL_POSEIDON_OWNER_DOMAIN_V3,
                [
                    0xce, 0x9c, 0x57, 0xdb, 0x56, 0x29, 0x51, 0xd1, 0xdd, 0x72, 0xe8, 0x34, 0xbf,
                    0xac, 0xcc, 0x74, 0xa9, 0xe2, 0x5f, 0x5c, 0xa2, 0xc1, 0xcd, 0x7d, 0xa1, 0xec,
                    0x5c, 0x3c, 0xaf, 0x45, 0x45, 0x3d,
                ],
                [
                    0x83, 0x82, 0xed, 0x00, 0xbb, 0x4b, 0xcb, 0xf7, 0x7d, 0x0c, 0x9b, 0xcc, 0x8e,
                    0xf1, 0x22, 0xac, 0x6f, 0x67, 0xa8, 0x8f, 0x68, 0xce, 0x46, 0x51, 0xce, 0x23,
                    0x7b, 0x67, 0x33, 0x4a, 0x65, 0x30,
                ],
            ),
            (
                super::CONFIDENTIAL_POSEIDON_NOTE_DOMAIN_V3,
                [
                    0xcd, 0xb8, 0x44, 0xf8, 0xa4, 0x78, 0xeb, 0xf3, 0x14, 0x54, 0x6c, 0xc9, 0xa8,
                    0x14, 0x5b, 0xbc, 0xa0, 0x5b, 0x42, 0x21, 0xa3, 0x1a, 0x9c, 0xee, 0x2a, 0x34,
                    0xa6, 0xb2, 0xd8, 0x98, 0x86, 0x2c,
                ],
                [
                    0x22, 0x2f, 0xe8, 0xdf, 0xb1, 0x1b, 0x68, 0xb9, 0x38, 0x47, 0xd2, 0x86, 0x94,
                    0xdb, 0x28, 0xc5, 0x63, 0x6c, 0x5b, 0xbf, 0x78, 0xa7, 0xb7, 0xdb, 0x73, 0xc6,
                    0x2b, 0x3e, 0x38, 0x9a, 0xc0, 0x2d,
                ],
            ),
            (
                super::CONFIDENTIAL_POSEIDON_NULLIFIER_DOMAIN_V3,
                [
                    0x00, 0x76, 0x08, 0x32, 0xfe, 0x2d, 0x8d, 0x60, 0x37, 0x3d, 0x15, 0xeb, 0x76,
                    0x43, 0x6a, 0x21, 0x6d, 0xec, 0x7d, 0xef, 0xaa, 0xf1, 0xda, 0x69, 0xd5, 0x23,
                    0x3c, 0xce, 0x5c, 0x98, 0xab, 0x06,
                ],
                [
                    0xb4, 0x6a, 0x51, 0x8a, 0x68, 0x0c, 0xdf, 0x75, 0x06, 0x9e, 0x35, 0x78, 0x4d,
                    0x7f, 0xd5, 0x80, 0x3c, 0x8d, 0xbf, 0xc1, 0xa3, 0xb8, 0x66, 0xc1, 0xff, 0xd0,
                    0x3a, 0x2b, 0x35, 0xdf, 0x0d, 0x00,
                ],
            ),
            (
                super::CONFIDENTIAL_POSEIDON_MERKLE_LEAF_DOMAIN_V3,
                [
                    0x66, 0x12, 0x9a, 0x24, 0xba, 0x49, 0x66, 0xae, 0xd5, 0xe6, 0xf5, 0x69, 0x56,
                    0xe8, 0x09, 0x16, 0xd5, 0x07, 0xcf, 0x6a, 0x68, 0xa6, 0xe2, 0x61, 0xb9, 0x2d,
                    0x0a, 0x9f, 0x9d, 0x13, 0x9c, 0x33,
                ],
                [
                    0x34, 0x22, 0xab, 0xe3, 0x43, 0x31, 0x71, 0x93, 0x0e, 0xb6, 0x7c, 0xa9, 0xb4,
                    0xe0, 0x5a, 0xdf, 0x27, 0xf8, 0x23, 0x62, 0xed, 0xe7, 0x8c, 0x8a, 0x65, 0x5e,
                    0x2e, 0x79, 0x85, 0xc0, 0xc5, 0x38,
                ],
            ),
            (
                super::CONFIDENTIAL_POSEIDON_MERKLE_NODE_DOMAIN_V3,
                [
                    0xe6, 0x44, 0x99, 0x62, 0xdd, 0xc1, 0xd2, 0x3d, 0x9d, 0x62, 0x94, 0x57, 0x72,
                    0xb9, 0x68, 0x8c, 0xea, 0x4e, 0x03, 0x82, 0x4f, 0x3c, 0xaf, 0x77, 0x3f, 0x3a,
                    0x74, 0x10, 0x4d, 0x4b, 0xb2, 0x34,
                ],
                [
                    0x1e, 0x00, 0xc2, 0xeb, 0xab, 0x3d, 0x5c, 0x05, 0x74, 0xcb, 0xc7, 0xf6, 0x47,
                    0xb5, 0xfe, 0xb4, 0xc4, 0xff, 0x27, 0x1b, 0xd8, 0x4f, 0xb7, 0x7b, 0xbb, 0x0c,
                    0xc0, 0xf3, 0xda, 0x60, 0x70, 0x39,
                ],
            ),
        ];
        for (domain, fp, fq) in vectors {
            assert_eq!(repr::<Fp>(domain), fp);
            assert_eq!(repr::<Fq>(domain), fq);
        }
        for (domain, inputs, fp, fq) in [
            (
                super::CONFIDENTIAL_POSEIDON_OWNER_DOMAIN_V3,
                &[3, 5][..],
                "612ad09a40970302036fef4c16385a98a7b337143c086d7ec4c0f9fc4792610d",
                "da41767db79387f7bfb20625144da612661c38f7ea94dc3a62f330e9ddbbef10",
            ),
            (
                super::CONFIDENTIAL_POSEIDON_NOTE_DOMAIN_V3,
                &[3, 5, 8, 13],
                "cdb844f8a478ebf314546cc9a8145bbca05b4221a31a9cee2a34a6b2d898862c",
                "222fe8dfb11b68b93847d28694db28c5636c5bbf78a7b7db73c62b3e389ac02d",
            ),
            (
                super::CONFIDENTIAL_POSEIDON_NULLIFIER_DOMAIN_V3,
                &[3, 5, 8, 13],
                "00760832fe2d8d60373d15eb76436a216dec7defaaf1da69d5233cce5c98ab06",
                "b46a518a680cdf75069e35784d7fd5803c8dbfc1a3b866c1ffd03a2b35df0d00",
            ),
            (
                super::CONFIDENTIAL_POSEIDON_MERKLE_LEAF_DOMAIN_V3,
                &[3],
                "75b309c05d81f516d4ceadaca9640d240c24f365453f476db07b4d8e3c943713",
                "a447fb1114387ca98a59cdc3bbc721bdcf6a74b0cfe9ad7ae45125f07538a532",
            ),
            (
                super::CONFIDENTIAL_POSEIDON_MERKLE_NODE_DOMAIN_V3,
                &[3, 5],
                "22a66785c01757e9f8b6c401f5e1f08f6649cc52a0083bb452af4378d15b2228",
                "3f39495312f7cdfe4af7346fc00f674709cca1fce1686e2881c708ff5034842a",
            ),
            (
                super::CONFIDENTIAL_POSEIDON_ASSET_DOMAIN_V3,
                &[3],
                "e12530abfe9e4f7c1f95d510191b65c89546e4d9b8e9ed79d3e3521772f02930",
                "45591fdcac6208fef59f1955ef819d2296dab0aeba1023a3813ccf2d4e52eb03",
            ),
            (
                super::CONFIDENTIAL_POSEIDON_CHAIN_DOMAIN_V3,
                &[3],
                "fca84dd79474290906d03758d1c9dd2ab58a8a97117c2265eb9dccca8652801f",
                "870b2059b229ac2c6039448efe1fb1ee2b84eab4a3a471f71c87d4c221f4902b",
            ),
            (
                super::CONFIDENTIAL_POSEIDON_PAYER_DOMAIN_V3,
                &[3],
                "9cbed996e9fa7df2defec498c6a0b03c230ac514bb36b02cdf6c0566dee6f120",
                "d4b100e87bdadbe867edd65ad713c0021856edfd117637be7b520392bb654a3a",
            ),
            (
                super::CONFIDENTIAL_POSEIDON_OPERATION_DOMAIN_V3,
                &[3],
                "de6686c1d1e59eecf8b522355c36624ea6d2ceeec8cd8607dadbbcc13ac08812",
                "b1697fa1593829176a2b72416bebdec305cf036b621480bc2d5ba74d1d339a03",
            ),
        ] {
            assert_eq!(repr_inputs::<Fp>(domain, inputs), hex32(fp));
            assert_eq!(repr_inputs::<Fq>(domain, inputs), hex32(fq));
        }
    }

    #[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
    #[test]
    fn confidential_v3_native_derivations_are_domain_separated_and_fail_closed() {
        use std::collections::BTreeSet;

        let asset =
            super::derive_confidential_asset_tag_v3("rose#wonderland").expect("V3 asset tag");
        let chain = super::derive_confidential_chain_tag_v3("00000000-0000-0000-0000-000000000001")
            .expect("V3 chain tag");
        let payer = super::derive_kagemusha_topup_payer_tag_v3("alice").expect("V3 payer tag");
        let operation =
            super::derive_kagemusha_topup_operation_tag_v3(&[7; 32]).expect("V3 operation tag");
        assert_eq!(
            BTreeSet::from([asset, chain, payer, operation]).len(),
            4,
            "distinct use domains must not alias the same preimage"
        );

        let spend_key = [11; 32];
        let diversifier = super::scalar_to_repr_bytes(super::Scalar::from(13));
        let owner =
            super::derive_confidential_owner_tag_v3_with_diversifier(&spend_key, diversifier)
                .expect("V3 owner");
        let rho = [17; 32];
        let note = super::derive_confidential_note_v3(asset, 19, rho, owner).expect("V3 note");
        let nullifier = super::derive_confidential_nullifier_v3(&spend_key, rho, asset, chain)
            .expect("V3 nullifier");
        assert_ne!(note, nullifier);
        assert!(
            super::derive_confidential_owner_tag_v3_with_diversifier(&[0; 32], diversifier)
                .is_err()
        );
        assert!(
            super::derive_confidential_owner_tag_v3_with_diversifier(&spend_key, [0xff; 32])
                .is_err()
        );
        assert!(super::derive_confidential_asset_tag_v3("  ").is_err());
        assert!(super::derive_confidential_asset_tag_v3(" rose#wonderland").is_err());
        assert!(
            super::derive_confidential_chain_tag_v3("00000000-0000-0000-0000-000000000001 ")
                .is_err()
        );
        assert!(super::derive_kagemusha_topup_payer_tag_v3("alice ").is_err());
        assert!(super::derive_kagemusha_topup_operation_tag_v3(&[0; 32]).is_err());
        assert!(super::derive_confidential_note_v3(asset, 0, rho, owner).is_err());
        assert!(
            super::derive_confidential_nullifier_v3(&spend_key, [0; 32], asset, chain).is_err()
        );
    }

    #[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
    #[test]
    fn generated_confidential_v2_vk_records_parse_as_matching_circuits() {
        let transfer = super::confidential_transfer_v2_vk_record("vk_transfer", 3)
            .expect("transfer vk record");
        let unshield = super::confidential_unshield_v2_vk_record("vk_unshield", 4)
            .expect("unshield vk record");
        let unshield_v3 = super::confidential_unshield_v3_vk_record("vk_unshield_v3", 5)
            .expect("unshield v3 vk record");

        assert_eq!(
            transfer.circuit_id,
            super::CONFIDENTIAL_TRANSFER_V2_CIRCUIT_ID
        );
        assert_eq!(
            unshield.circuit_id,
            super::CONFIDENTIAL_UNSHIELD_V2_CIRCUIT_ID
        );
        assert_eq!(
            unshield_v3.circuit_id,
            super::CONFIDENTIAL_UNSHIELD_V3_CIRCUIT_ID
        );
        assert!(transfer.is_active());
        assert!(unshield.is_active());
        assert!(unshield_v3.is_active());
        assert!(transfer.max_proof_bytes > 0);
        assert!(unshield.max_proof_bytes > 0);
        assert!(unshield_v3.max_proof_bytes > 0);

        let transfer_key = transfer.key.as_ref().expect("transfer key");
        let unshield_key = unshield.key.as_ref().expect("unshield key");
        let unshield_v3_key = unshield_v3.key.as_ref().expect("unshield v3 key");
        super::parse_vk_for_transfer(&transfer.circuit_id, transfer_key)
            .expect("transfer key must parse as confidential transfer v2");
        super::parse_vk_for_unshield_v2(&unshield.circuit_id, unshield_key)
            .expect("unshield key must parse as confidential unshield v2");
        super::parse_vk_for_unshield_v3(&unshield_v3.circuit_id, unshield_v3_key)
            .expect("unshield v3 key must parse as confidential unshield v3");
    }

    #[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
    #[test]
    fn supplied_confidential_merkle_path_recomputes_witness_nodes() {
        let commitments = vec![[0x11; 32], [0x22; 32], [0x33; 32]];
        let path =
            super::compute_confidential_merkle_path_v2(&commitments, 2).expect("computed path");
        let mut supplied = path.clone();
        supplied.witness_nodes.clear();

        let normalized = super::normalize_supplied_confidential_merkle_path_v2(
            [0x33; 32],
            Some(2),
            &supplied,
            path.root,
            "test path",
        )
        .expect("supplied path should validate");

        assert_eq!(normalized.root, path.root);
        assert_eq!(normalized.witness_nodes, path.witness_nodes);

        let mut tampered = supplied;
        tampered.directions[0] ^= 1;
        assert!(
            super::normalize_supplied_confidential_merkle_path_v2(
                [0x33; 32],
                Some(2),
                &tampered,
                path.root,
                "test path",
            )
            .is_err()
        );
    }

    #[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
    #[test]
    fn next_zero_confidential_path_matches_padded_tree_path() {
        for len in 1usize..12 {
            let commitments: Vec<[u8; 32]> = (0..len)
                .map(|index| {
                    let mut commitment = [0u8; 32];
                    commitment[0] = 0x40;
                    commitment[31] = u8::try_from(index + 1).expect("fixture index fits in u8");
                    commitment
                })
                .collect();
            let previous_index = commitments.len() - 1;
            let previous_path =
                super::compute_confidential_merkle_path_v2(&commitments, previous_index)
                    .expect("previous path");
            let expected_next_zero =
                super::compute_confidential_merkle_path_v2(&commitments, commitments.len())
                    .expect("expected zero path");
            let derived = super::derive_confidential_next_zero_path_v2(
                commitments[previous_index],
                previous_index,
                &previous_path,
                previous_path.root,
            )
            .expect("derived next zero path");

            assert_eq!(derived.root, expected_next_zero.root, "len={len}");
            assert_eq!(
                derived.siblings, expected_next_zero.siblings,
                "siblings len={len}"
            );
            assert_eq!(
                derived.directions, expected_next_zero.directions,
                "directions len={len}"
            );
            assert_eq!(
                derived.witness_nodes, expected_next_zero.witness_nodes,
                "witness nodes len={len}"
            );
        }
    }

    #[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
    #[test]
    fn confidential_transfer_v2_canonical_vk_guard_rejects_self_consistent_key_substitution() {
        use iroha_data_model::proof::VerifyingKeyBox;

        let canonical = super::confidential_transfer_v2_vk_box().expect("canonical transfer vk");
        let cached = super::confidential_transfer_v2_vk_box().expect("cached transfer vk");
        assert_eq!(
            canonical, cached,
            "confidential transfer v2 verifier key generation should be cached and deterministic"
        );
        super::ensure_confidential_transfer_v2_canonical_vk_box(&canonical)
            .expect("canonical transfer verifier key should pass");
        let proving_key = super::cached_confidential_transfer_v2_proving_key()
            .expect("canonical transfer proving key");
        let cached_proving_key = super::cached_confidential_transfer_v2_proving_key()
            .expect("cached transfer proving key");
        assert!(
            std::ptr::eq(proving_key, cached_proving_key),
            "confidential transfer v2 proving key generation should be cached"
        );

        let mut mutated = canonical.clone();
        let last = mutated
            .bytes
            .last_mut()
            .expect("canonical transfer verifier key bytes");
        *last ^= 0x01;
        let err = super::ensure_confidential_transfer_v2_canonical_vk_box(&mutated)
            .expect_err("mutated self-consistent verifier key must reject");
        assert!(
            err.contains("canonical semantic circuit key"),
            "unexpected mutated-key error: {err}"
        );

        let wrong_backend =
            VerifyingKeyBox::new("halo2/ipa:kzg".to_owned(), canonical.bytes.clone());
        let err = super::ensure_confidential_transfer_v2_canonical_vk_box(&wrong_backend)
            .expect_err("wrong backend must reject before canonical bytes are considered");
        assert!(err.contains("backend"), "unexpected backend error: {err}");

        let empty = VerifyingKeyBox::new(crate::zk::ZK_BACKEND_HALO2_IPA.to_owned(), Vec::new());
        let err = super::ensure_confidential_transfer_v2_canonical_vk_box(&empty)
            .expect_err("empty verifier key must reject");
        assert!(
            err.contains("non-empty"),
            "unexpected empty-key error: {err}"
        );
    }

    #[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
    #[test]
    fn confidential_transfer_v2_canonical_vk_guard_rejects_malformed_key_preflight() {
        use iroha_data_model::proof::VerifyingKeyBox;

        let malformed =
            VerifyingKeyBox::new(crate::zk::ZK_BACKEND_HALO2_IPA.to_owned(), vec![0xC9; 32]);
        let err = super::ensure_confidential_transfer_v2_canonical_vk_box(&malformed)
            .expect_err("malformed verifier key must reject before canonical key generation");
        assert!(
            err.contains("invalid CID1/Halo2 IPA verifier-key envelope"),
            "unexpected malformed-key error: {err}"
        );
    }

    #[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
    #[test]
    fn confidential_unshield_v2_v3_canonical_caches_reject_key_substitution() {
        use iroha_data_model::proof::VerifyingKeyBox;

        let v2 = super::confidential_unshield_v2_vk_box().expect("canonical unshield v2 vk");
        let v2_cached = super::confidential_unshield_v2_vk_box().expect("cached unshield v2 vk");
        assert_eq!(v2, v2_cached);
        super::ensure_confidential_unshield_v2_canonical_vk_box(&v2)
            .expect("canonical unshield v2 verifier key should pass");

        let v3 = super::confidential_unshield_v3_vk_box().expect("canonical unshield v3 vk");
        let v3_cached = super::confidential_unshield_v3_vk_box().expect("cached unshield v3 vk");
        assert_eq!(v3, v3_cached);
        super::ensure_confidential_unshield_v3_canonical_vk_box(&v3)
            .expect("canonical unshield v3 verifier key should pass");

        let v2_pk = super::cached_confidential_unshield_v2_proving_key()
            .expect("canonical unshield v2 proving key");
        let v2_pk_cached = super::cached_confidential_unshield_v2_proving_key()
            .expect("cached unshield v2 proving key");
        assert!(
            std::ptr::eq(v2_pk, v2_pk_cached),
            "unshield v2 proving key should come from a process-local cache"
        );

        let v3_pk = super::cached_confidential_unshield_v3_proving_key()
            .expect("canonical unshield v3 proving key");
        let v3_pk_cached = super::cached_confidential_unshield_v3_proving_key()
            .expect("cached unshield v3 proving key");
        assert!(
            std::ptr::eq(v3_pk, v3_pk_cached),
            "unshield v3 proving key should come from a process-local cache"
        );

        fn assert_rejects_key_substitution(
            label: &str,
            canonical: &VerifyingKeyBox,
            ensure: fn(&VerifyingKeyBox) -> Result<(), String>,
        ) {
            let mut mutated = canonical.clone();
            *mutated
                .bytes
                .last_mut()
                .expect("canonical unshield verifier key bytes") ^= 0x01;
            let err = match ensure(&mutated) {
                Ok(()) => panic!("{label} mutated verifier key must reject"),
                Err(err) => err,
            };
            assert!(
                err.contains("canonical semantic circuit key"),
                "unexpected {label} mutated-key error: {err}"
            );

            let wrong_backend =
                VerifyingKeyBox::new("halo2/ipa:kzg".to_owned(), canonical.bytes.clone());
            let err = match ensure(&wrong_backend) {
                Ok(()) => panic!("{label} wrong backend must reject"),
                Err(err) => err,
            };
            assert!(
                err.contains("backend"),
                "unexpected {label} backend error: {err}"
            );
        }
        assert_rejects_key_substitution(
            "unshield v2",
            &v2,
            super::ensure_confidential_unshield_v2_canonical_vk_box,
        );
        assert_rejects_key_substitution(
            "unshield v3",
            &v3,
            super::ensure_confidential_unshield_v3_canonical_vk_box,
        );

        let err = super::ensure_confidential_unshield_v3_canonical_vk_box(&v2)
            .expect_err("unshield v2 key must not satisfy unshield v3 canonical guard");
        assert!(
            err.contains("CID1"),
            "unexpected v2-as-v3 canonical-guard error: {err}"
        );
        let err = super::ensure_confidential_unshield_v2_canonical_vk_box(&v3)
            .expect_err("unshield v3 key must not satisfy unshield v2 canonical guard");
        assert!(
            err.contains("CID1"),
            "unexpected v3-as-v2 canonical-guard error: {err}"
        );
    }

    #[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
    #[test]
    fn generated_confidential_transfer_v2_one_input_one_output_verifies_against_generated_vk() {
        use halo2_proofs::halo2curves::{ff::Field as _, pasta::Fp};
        use iroha_data_model::ChainId;

        let chain_id: ChainId = "fc56984b-2be7-431d-840e-21514d1883f0"
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

        let wrong_cid_key = super::build_confidential_v2_vk_box(
            super::CONFIDENTIAL_TRANSFER_V2_IPA_K,
            super::CONFIDENTIAL_UNSHIELD_V2_CIRCUIT_ID,
            &super::ConfidentialTransferCircuitV2::<{ super::CONFIDENTIAL_TREE_DEPTH_V2 }>::default(
            ),
        )
        .expect("transfer-shaped verifier key with wrong CID1");
        assert_ne!(
            crate::zk::hash_vk(transfer_key),
            crate::zk::hash_vk(&wrong_cid_key)
        );
        let wrong_cid_proof = super::build_confidential_transfer_proof_v2(
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
            &wrong_cid_key,
        )
        .expect("transfer proof with wrong-CID verifier key");
        assert!(
            !crate::zk::verify_backend(
                crate::zk::ZK_BACKEND_HALO2_IPA,
                &wrong_cid_proof.proof,
                Some(&wrong_cid_key),
            ),
            "verifier must reject a cryptographically valid proof whose VK CID1 names another circuit"
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

    #[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
    #[test]
    fn generated_asset_hidden_transfer_v1_proof_verifies_against_cached_canonical_vk() {
        let chain_id = iroha_data_model::ChainId::from("asset-hidden-transfer-v1-test");
        let pool_id = "boi-private-is-pool";
        let asset_set_root = super::scalar_to_repr_bytes(super::scalar_from_u128(0xA0));
        let input_commitment = super::scalar_to_repr_bytes(super::scalar_from_u128(0xA1));
        let nullifier = super::scalar_to_repr_bytes(super::scalar_from_u128(0xB1));
        let output_commitment = super::scalar_to_repr_bytes(super::scalar_from_u128(0xC1));
        let root_hint = super::scalar_to_repr_bytes(super::scalar_from_u128(0xD1));
        let vk_box = super::asset_hidden_transfer_v1_vk_box().expect("asset hidden vk");

        let proof = super::build_asset_hidden_transfer_proof_v1(
            &chain_id,
            pool_id,
            asset_set_root,
            &[input_commitment],
            &[nullifier],
            &[output_commitment],
            root_hint,
            super::ASSET_HIDDEN_TRANSFER_V1_CIRCUIT_ID,
            &vk_box,
        )
        .expect("build asset-hidden transfer proof");

        assert_eq!(proof.input_commitments, vec![input_commitment]);
        assert_eq!(proof.nullifiers, vec![nullifier]);
        assert_eq!(proof.output_commitments, vec![output_commitment]);
        assert_eq!(proof.root, root_hint);
        assert!(
            crate::zk::verify_backend(crate::zk::ZK_BACKEND_HALO2_IPA, &proof.proof, Some(&vk_box)),
            "generated asset-hidden transfer v1 proof should verify against the cached canonical VK"
        );

        let public_inputs = super::parse_asset_hidden_transfer_public_inputs(&proof.proof.bytes)
            .expect("asset-hidden public inputs");
        assert_eq!(
            public_inputs.pool_id_tag,
            super::derive_asset_hidden_pool_id_tag_v1(pool_id)
        );
        assert_eq!(public_inputs.asset_set_root, asset_set_root);
        assert_eq!(
            public_inputs.input_commitments,
            [input_commitment, [0u8; 32]]
        );
        assert_eq!(public_inputs.nullifiers, [nullifier, [0u8; 32]]);
        assert_eq!(public_inputs.outputs, [output_commitment, [0u8; 32]]);
        assert_eq!(public_inputs.root, root_hint);
        assert_eq!(
            public_inputs.chain_tag,
            super::derive_confidential_chain_tag_v2(chain_id.as_str())
        );

        let duplicate_nullifier = super::build_asset_hidden_transfer_proof_v1(
            &chain_id,
            pool_id,
            asset_set_root,
            &[
                input_commitment,
                super::scalar_to_repr_bytes(super::scalar_from_u128(0xA2)),
            ],
            &[nullifier, nullifier],
            &[output_commitment],
            root_hint,
            super::ASSET_HIDDEN_TRANSFER_V1_CIRCUIT_ID,
            &vk_box,
        )
        .expect_err("duplicate nullifiers must reject before proving");
        assert!(
            duplicate_nullifier.contains("duplicate nullifier"),
            "unexpected duplicate-nullifier error: {duplicate_nullifier}"
        );
    }

    #[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
    #[test]
    fn kagemusha_topup_shield_v2_binds_every_public_field_and_rejects_substitution() {
        let chain_id = iroha_data_model::ChainId::from("kagemusha-topup-shield-test");
        let asset_definition_id = "pkr#sbp";
        let payer = "ed0120AABBCC@sbp";
        let operation_id = [0x41_u8; 32];
        let spend_key = [0x42_u8; 32];
        let rho = [0x43_u8; 32];
        let diversifier = super::derive_confidential_diversifier_v2(b"topup-owner");
        let atomic_amount = 10_750_000_000_u128;
        let asset_scale = 9;
        let tree_commitments = vec![
            super::scalar_to_repr_bytes(super::scalar_from_u128(0x51)),
            super::scalar_to_repr_bytes(super::scalar_from_u128(0x52)),
        ];
        let leaf_index = u32::try_from(tree_commitments.len()).expect("fixture index");
        let zero_path =
            super::compute_confidential_merkle_path_v2(&tree_commitments, leaf_index as usize)
                .expect("next-zero path");
        let vk_box = super::kagemusha_topup_shield_v2_vk_box().expect("canonical shield vk");
        let result = super::build_kagemusha_topup_shield_proof_v2(
            &chain_id,
            asset_definition_id,
            payer,
            operation_id,
            atomic_amount,
            asset_scale,
            &spend_key,
            rho,
            diversifier,
            leaf_index,
            &zero_path,
            super::KAGEMUSHA_TOPUP_SHIELD_V2_CIRCUIT_ID,
            &vk_box,
        )
        .expect("build Kagemusha top-up shield proof");
        assert!(crate::zk::verify_backend(
            crate::zk::ZK_BACKEND_HALO2_IPA,
            &result.proof,
            Some(&vk_box)
        ));
        let public = super::parse_kagemusha_topup_shield_public_inputs_v2(&result.proof.bytes)
            .expect("parse shield public inputs");
        assert_eq!(public.output_commitment, result.output_commitment);
        assert_eq!(public.spend_nullifier, result.spend_nullifier);
        assert_eq!(public.initial_root, result.initial_root);
        assert_eq!(public.finalized_root, result.finalized_root);
        assert_eq!(
            public.atomic_amount,
            super::encode_confidential_amount_v2(atomic_amount)
        );
        assert_eq!(
            public.asset_scale,
            super::encode_kagemusha_topup_u32_v2(asset_scale)
        );
        assert_eq!(
            public.leaf_index,
            super::encode_kagemusha_topup_u32_v2(leaf_index)
        );
        assert_eq!(
            public.asset_tag,
            super::derive_confidential_asset_tag_v2(asset_definition_id)
        );
        assert_eq!(
            public.chain_tag,
            super::derive_confidential_chain_tag_v2(chain_id.as_str())
        );
        assert_eq!(
            public.payer_tag,
            super::derive_kagemusha_topup_payer_tag_v2(payer)
        );
        assert_eq!(
            public.operation_tag,
            super::derive_kagemusha_topup_operation_tag_v2(&operation_id)
        );

        let envelope: iroha_data_model::zk::OpenVerifyEnvelope =
            norito::decode_from_bytes(&result.proof.bytes).expect("outer proof envelope");
        let (transcript, columns) =
            crate::zk::zkparse::strict_proof_and_instances(&envelope.proof_bytes)
                .expect("inner proof and instances");
        assert_eq!(columns.len(), 11);
        let spend_scalar =
            super::hash_to_scalar(b"iroha.confidential.v2.spend_scalar", &[&spend_key]);
        let output_nodes =
            super::kagemusha_topup_output_path_nodes_v2(result.output_commitment, &zero_path)
                .expect("output path nodes");
        let circuit = super::KagemushaTopUpShieldCircuitV2::<{ super::CONFIDENTIAL_TREE_DEPTH_V2 }> {
            witness: Some(super::KagemushaTopUpShieldWitnessV2 {
                amount: atomic_amount,
                asset_scale,
                leaf_index,
                rho,
                spend_scalar: super::scalar_to_repr_bytes(spend_scalar),
                diversifier,
                asset_tag: super::derive_confidential_asset_tag_v2(asset_definition_id),
                chain_tag: super::derive_confidential_chain_tag_v2(chain_id.as_str()),
                payer_tag: super::derive_kagemusha_topup_payer_tag_v2(payer),
                operation_tag: super::derive_kagemusha_topup_operation_tag_v2(&operation_id),
                zero_path: zero_path.clone(),
                output_nodes,
            }),
        };
        halo2_proofs::dev::MockProver::run(
            super::KAGEMUSHA_TOPUP_SHIELD_V2_IPA_K,
            &circuit,
            columns.clone(),
        )
        .expect("canonical Kagemusha top-up shield mock prover")
        .assert_satisfied();
        for substituted_column in 0..columns.len() {
            let mut substituted = columns.clone();
            substituted[substituted_column][0] += super::Scalar::from(1_u64);
            let substituted_mock = halo2_proofs::dev::MockProver::run(
                super::KAGEMUSHA_TOPUP_SHIELD_V2_IPA_K,
                &circuit,
                substituted.clone(),
            )
            .expect("substituted Kagemusha top-up shield mock prover");
            assert!(
                substituted_mock.verify().is_err(),
                "fixed witness must reject substituted public input column {substituted_column}"
            );
            let mut inner = crate::zk::zk1::wrap_start();
            crate::zk::zk1::wrap_append_proof(&mut inner, &transcript);
            let refs: Vec<&[super::Scalar]> = substituted.iter().map(Vec::as_slice).collect();
            crate::zk::zk1::wrap_append_instances_pasta_fp_cols(&refs, &mut inner);
            let mut substituted_envelope = envelope.clone();
            substituted_envelope.proof_bytes = inner;
            let substituted_proof = iroha_data_model::proof::ProofBox::new(
                crate::zk::ZK_BACKEND_HALO2_IPA.to_owned(),
                norito::to_bytes(&substituted_envelope).expect("encode substituted envelope"),
            );
            assert!(
                !crate::zk::verify_backend(
                    crate::zk::ZK_BACKEND_HALO2_IPA,
                    &substituted_proof,
                    Some(&vk_box),
                ),
                "substituting public input column {substituted_column} must invalidate the proof"
            );
        }
    }

    #[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
    #[test]
    fn kagemusha_topup_shield_v2_statement_rejects_zero_and_colliding_fields() {
        let commitment = super::scalar_to_repr_bytes(super::scalar_from_u128(1));
        let nullifier = super::scalar_to_repr_bytes(super::scalar_from_u128(2));
        let initial_root = super::scalar_to_repr_bytes(super::scalar_from_u128(3));
        let finalized_root = super::scalar_to_repr_bytes(super::scalar_from_u128(4));

        for (output, nullifier, initial, finalized, expected) in [
            (
                [0; 32],
                nullifier,
                initial_root,
                finalized_root,
                "output commitment must be non-zero",
            ),
            (
                commitment,
                [0; 32],
                initial_root,
                finalized_root,
                "spend nullifier must be non-zero",
            ),
            (
                commitment,
                commitment,
                initial_root,
                finalized_root,
                "must be distinct",
            ),
            (
                commitment,
                nullifier,
                [0; 32],
                finalized_root,
                "initial root must be non-zero",
            ),
            (
                commitment,
                nullifier,
                initial_root,
                [0; 32],
                "finalized root must be non-zero",
            ),
            (
                commitment,
                nullifier,
                initial_root,
                initial_root,
                "must change the confidential root",
            ),
        ] {
            let error = super::validate_kagemusha_topup_shield_statement_v2(
                output, nullifier, initial, finalized,
            )
            .expect_err("invalid Kagemusha top-up statement must reject");
            assert!(
                error.contains(expected),
                "unexpected statement validation error `{error}`; expected `{expected}`"
            );
        }
    }

    #[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
    #[test]
    fn kagemusha_topup_shield_v2_builder_rejects_zero_amount_bad_path_and_key_substitution() {
        let chain_id = iroha_data_model::ChainId::from("kagemusha-topup-negative-test");
        let commitments = vec![super::scalar_to_repr_bytes(super::scalar_from_u128(0x61))];
        let zero_path =
            super::compute_confidential_merkle_path_v2(&commitments, 1).expect("next-zero path");
        let vk_box = super::kagemusha_topup_shield_v2_vk_box().expect("canonical shield vk");
        let build = |amount, operation_id, path: &super::ConfidentialMerklePathV2, vk| {
            super::build_kagemusha_topup_shield_proof_v2(
                &chain_id,
                "pkr#sbp",
                "payer@sbp",
                operation_id,
                amount,
                9,
                &[0x62; 32],
                [0x63; 32],
                super::derive_confidential_diversifier_v2(b"negative-topup"),
                1,
                path,
                super::KAGEMUSHA_TOPUP_SHIELD_V2_CIRCUIT_ID,
                vk,
            )
        };
        assert!(
            build(0, [0x64; 32], &zero_path, &vk_box)
                .expect_err("zero amount")
                .contains("must be positive")
        );
        assert!(
            build(1, [0; 32], &zero_path, &vk_box)
                .expect_err("zero operation")
                .contains("operation_id must be non-zero")
        );

        let mut bad_direction = zero_path.clone();
        bad_direction.directions[0] ^= 1;
        assert!(
            build(1, [0x64; 32], &bad_direction, &vk_box)
                .expect_err("path/index substitution")
                .contains("direction[0] does not match leaf_index")
        );
        let mut bad_root = zero_path.clone();
        bad_root.root[0] ^= 1;
        assert!(
            build(1, [0x64; 32], &bad_root, &vk_box)
                .expect_err("root substitution")
                .contains("does not prove the supplied root_hint")
        );

        let transfer_vk = super::confidential_transfer_v2_vk_box().expect("transfer vk");
        let key_error = build(1, [0x64; 32], &zero_path, &transfer_vk)
            .expect_err("cross-circuit verifier substitution");
        assert!(
            key_error.contains("Kagemusha top-up shield v2 verifier key"),
            "unexpected key-substitution error: {key_error}"
        );
    }

    #[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
    #[test]
    fn generated_confidential_unshield_v2_proof_verifies_against_cached_canonical_vk() {
        let chain_id = iroha_data_model::ChainId::from("confidential-unshield-v2-test");
        let asset_definition_id = "zcoin#wonderland";
        let spend_key = [0x91_u8; 32];
        let input_rho = [0x92_u8; 32];
        let input_diversifier = super::derive_confidential_diversifier_v2(b"unshield-v2-input");
        let input_owner_tag =
            super::derive_confidential_owner_tag_v2_with_diversifier(&spend_key, input_diversifier)
                .expect("input owner tag");
        let input_commitment =
            super::derive_confidential_note_v2(asset_definition_id, 9, input_rho, input_owner_tag)
                .expect("input commitment");
        let tree_commitments = vec![input_commitment];
        let root_hint =
            super::compute_confidential_root_v2(&tree_commitments).expect("confidential root");
        let vk_record =
            super::confidential_unshield_v2_vk_record("vk_unshield", 4).expect("unshield vk");
        let vk_box = vk_record.key.clone().expect("inline unshield vk");

        let proof = super::build_confidential_unshield_proof_v2(
            &chain_id,
            asset_definition_id,
            &spend_key,
            &tree_commitments,
            &[super::ConfidentialUnshieldInputV2 {
                amount: 9,
                rho: input_rho,
                diversifier: input_diversifier,
                leaf_index: 0,
            }],
            9,
            root_hint,
            &vk_record.circuit_id,
            &vk_box,
        )
        .expect("build unshield v2 proof");

        assert_eq!(proof.nullifiers.len(), 1);
        assert_eq!(proof.root, root_hint);
        assert!(
            crate::zk::verify_backend(crate::zk::ZK_BACKEND_HALO2_IPA, &proof.proof, Some(&vk_box)),
            "generated confidential unshield v2 proof should verify against the cached canonical VK"
        );

        let mut tampered = proof.proof.clone();
        let mut envelope: iroha_data_model::zk::OpenVerifyEnvelope =
            norito::decode_from_bytes(&tampered.bytes).expect("OpenVerifyEnvelope");
        envelope.vk_hash[0] ^= 0x80;
        tampered.bytes = norito::to_bytes(&envelope).expect("OpenVerifyEnvelope encode");
        assert!(
            !crate::zk::verify_backend(crate::zk::ZK_BACKEND_HALO2_IPA, &tampered, Some(&vk_box)),
            "unshield v2 proof must reject verifier-key hash substitution"
        );
    }

    #[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
    #[test]
    fn generated_confidential_unshield_v3_proof_verifies_and_rejects_bad_change() {
        let chain_id = iroha_data_model::ChainId::from("confidential-unshield-v3-test");
        let asset_definition_id = "zcoin#wonderland";
        let spend_key = [0xA1_u8; 32];
        let input_rho = [0xA2_u8; 32];
        let change_rho = [0xA3_u8; 32];
        let input_diversifier = super::derive_confidential_diversifier_v2(b"unshield-v3-input");
        let input_owner_tag =
            super::derive_confidential_owner_tag_v2_with_diversifier(&spend_key, input_diversifier)
                .expect("input owner tag");
        let input_commitment =
            super::derive_confidential_note_v2(asset_definition_id, 9, input_rho, input_owner_tag)
                .expect("input commitment");
        let tree_commitments = vec![input_commitment];
        let root_hint =
            super::compute_confidential_root_v2(&tree_commitments).expect("confidential root");
        let vk_record =
            super::confidential_unshield_v3_vk_record("vk_unshield_v3", 5).expect("unshield v3 vk");
        let vk_box = vk_record.key.clone().expect("inline unshield v3 vk");

        let missing_change = super::build_confidential_unshield_proof_v3(
            &chain_id,
            asset_definition_id,
            &spend_key,
            &tree_commitments,
            &[super::ConfidentialUnshieldInputV2 {
                amount: 9,
                rho: input_rho,
                diversifier: input_diversifier,
                leaf_index: 0,
            }],
            &[],
            5,
            root_hint,
            &vk_record.circuit_id,
            &vk_box,
        )
        .expect_err("nonzero change must require a private change note");
        assert!(
            missing_change.contains("requires a private change output"),
            "unexpected missing-change error: {missing_change}"
        );

        let bad_change = super::build_confidential_unshield_proof_v3(
            &chain_id,
            asset_definition_id,
            &spend_key,
            &tree_commitments,
            &[super::ConfidentialUnshieldInputV2 {
                amount: 9,
                rho: input_rho,
                diversifier: input_diversifier,
                leaf_index: 0,
            }],
            &[super::ConfidentialUnshieldOutputV3 {
                amount: 3,
                rho: change_rho,
            }],
            5,
            root_hint,
            &vk_record.circuit_id,
            &vk_box,
        )
        .expect_err("incorrect private change amount must reject");
        assert!(
            bad_change.contains("change note amount mismatch"),
            "unexpected bad-change error: {bad_change}"
        );

        let overflow_input_0_rho = [0xB1_u8; 32];
        let overflow_input_1_rho = [0xB2_u8; 32];
        let overflow_input_0_diversifier =
            super::derive_confidential_diversifier_v2(b"unshield-v3-overflow-input-0");
        let overflow_input_1_diversifier =
            super::derive_confidential_diversifier_v2(b"unshield-v3-overflow-input-1");
        let overflow_input_0_owner_tag = super::derive_confidential_owner_tag_v2_with_diversifier(
            &spend_key,
            overflow_input_0_diversifier,
        )
        .expect("overflow input 0 owner tag");
        let overflow_input_1_owner_tag = super::derive_confidential_owner_tag_v2_with_diversifier(
            &spend_key,
            overflow_input_1_diversifier,
        )
        .expect("overflow input 1 owner tag");
        let overflow_tree_commitments = vec![
            super::derive_confidential_note_v2(
                asset_definition_id,
                u128::MAX,
                overflow_input_0_rho,
                overflow_input_0_owner_tag,
            )
            .expect("overflow input 0 commitment"),
            super::derive_confidential_note_v2(
                asset_definition_id,
                1,
                overflow_input_1_rho,
                overflow_input_1_owner_tag,
            )
            .expect("overflow input 1 commitment"),
        ];
        let overflow_root_hint = super::compute_confidential_root_v2(&overflow_tree_commitments)
            .expect("overflow confidential root");
        let overflow = super::build_confidential_unshield_proof_v3(
            &chain_id,
            asset_definition_id,
            &spend_key,
            &overflow_tree_commitments,
            &[
                super::ConfidentialUnshieldInputV2 {
                    amount: u128::MAX,
                    rho: overflow_input_0_rho,
                    diversifier: overflow_input_0_diversifier,
                    leaf_index: 0,
                },
                super::ConfidentialUnshieldInputV2 {
                    amount: 1,
                    rho: overflow_input_1_rho,
                    diversifier: overflow_input_1_diversifier,
                    leaf_index: 1,
                },
            ],
            &[],
            0,
            overflow_root_hint,
            &vk_record.circuit_id,
            &vk_box,
        )
        .expect_err("overflowing private input sum must reject");
        assert!(
            overflow.contains("input amount sum overflows u128"),
            "unexpected overflow error: {overflow}"
        );

        let proof = super::build_confidential_unshield_proof_v3(
            &chain_id,
            asset_definition_id,
            &spend_key,
            &tree_commitments,
            &[super::ConfidentialUnshieldInputV2 {
                amount: 9,
                rho: input_rho,
                diversifier: input_diversifier,
                leaf_index: 0,
            }],
            &[super::ConfidentialUnshieldOutputV3 {
                amount: 4,
                rho: change_rho,
            }],
            5,
            root_hint,
            &vk_record.circuit_id,
            &vk_box,
        )
        .expect("build unshield v3 proof");

        let expected_change_owner_tag =
            super::derive_confidential_owner_tag_v2(&spend_key).expect("valid default owner tag");
        let expected_change_commitment = super::derive_confidential_note_v2(
            asset_definition_id,
            4,
            change_rho,
            expected_change_owner_tag,
        )
        .expect("expected change commitment");
        assert_eq!(proof.output_commitments, vec![expected_change_commitment]);
        assert_eq!(proof.nullifiers.len(), 1);
        assert_eq!(proof.root, root_hint);
        assert!(
            crate::zk::verify_backend(crate::zk::ZK_BACKEND_HALO2_IPA, &proof.proof, Some(&vk_box)),
            "generated confidential unshield v3 proof should verify against the cached canonical VK"
        );
    }
}
