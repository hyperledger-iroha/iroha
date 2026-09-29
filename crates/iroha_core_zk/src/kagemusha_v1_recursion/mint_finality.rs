//! Circuit-facing block finality for Kagemusha V1 mint credits.
//!
//! Ordinary Sumeragi finality remains BLS12-381.  A block containing reserve
//! top-ups additionally carries an exact quorum of signatures made with
//! separately provisioned Pasta keys. Those signatures authorize a sparse
//! depth-32 paired-Poseidon top-up root which the mint helper can verify
//! recursively without trusting a host-side finality boolean.

use ff::{Field, FromUniformBytes, PrimeField};
use halo2_proofs::halo2curves::{
    CurveAffine,
    group::{Curve as _, Group as _},
    pasta::{EpAffine, EqAffine, Fp, Fq},
};
use iroha_data_model::{
    isi::kagemusha_v1::{
        KAGEMUSHA_CHAIN_VERSION_V1, KAGEMUSHA_MINT_FINALITY_TREE_DEPTH_V1,
        KagemushaMintFinalityAuthorityGenerationV1, KagemushaMintFinalityGenesisParametersV1,
        KagemushaMintFinalityPairedPossessionProofV1, KagemushaMintFinalitySealMessageV1,
        KagemushaMintFinalitySeatReadinessContextV1, KagemushaMintFinalityValidatorKeysV1,
        KagemushaMintFinalityValidatorSealV1, KagemushaOperationKindV1,
        KagemushaPastaSchnorrSignatureV1, KagemushaReserveReceiptV1, KagemushaTopUpLeafV1,
        KagemushaTopUpMembershipWitnessV1, kagemusha_mint_finality_candidate_possession_digest_v1,
        kagemusha_mint_finality_root_v1,
    },
    kagemusha::KagemushaPastaStateCommitmentV1,
};
use norito::codec::Encode;
use sha2::{Digest as _, Sha256, Sha512};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};
use thiserror::Error;
use zeroize::{DefaultIsZeroes, Zeroizing};

use crate::kagemusha_v1_poseidon::{
    KagemushaPoseidonFieldV1, decode, digest_limbs, encode, from_u128, hash,
};

pub(super) const MINT_LEAF_DOMAIN_V1: u64 = u64::from_le_bytes(*b"kgmmntl1");
const MINT_EMPTY_DOMAIN_V1: u64 = u64::from_le_bytes(*b"kgminte1");
pub(super) const MINT_NODE_DOMAIN_V1: u64 = u64::from_le_bytes(*b"kgmmntn1");
const KEY_DERIVATION_DOMAIN_V1: &[u8] = b"iroha:kagemusha:v1:mint-finality:key";
const NONCE_DERIVATION_DOMAIN_V1: &[u8] = b"iroha:kagemusha:v1:mint-finality:nonce";
const CHALLENGE_DOMAIN_V1: &[u8] = b"iroha:kagemusha:v1:mint-finality:challenge";
const EQ_PARITY_TAG: u8 = 0;
const EP_PARITY_TAG: u8 = 1;

/// Native mint-finality construction or verification failure.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum KagemushaMintFinalityErrorV1 {
    /// The separately provisioned generation roster is malformed or mismatches consensus.
    #[error("invalid Kagemusha mint-finality generation roster: {0}")]
    InvalidAuthorityGeneration(String),
    /// A top-up leaf or fixed-depth path is malformed.
    #[error("invalid Kagemusha mint-finality top-up tree: {0}")]
    InvalidTopUpTree(String),
    /// The block-level seal statement does not match the consensus object.
    #[error("invalid Kagemusha mint-finality statement: {0}")]
    InvalidStatement(String),
    /// A separately provisioned private seed does not match its public roster entry.
    #[error("invalid Kagemusha mint-finality signer: {0}")]
    InvalidSigner(String),
    /// One Eq or Ep Schnorr equation failed.
    #[error("invalid Kagemusha mint-finality signature: {0}")]
    InvalidSignature(String),
}

/// Complete native fixed-depth tree used to commit one block's top-up receipts.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KagemushaMintFinalityTreeV1 {
    leaves: Vec<KagemushaTopUpLeafV1>,
    levels: Vec<BTreeMap<u32, KagemushaPastaStateCommitmentV1>>,
    empty_roots: Vec<KagemushaPastaStateCommitmentV1>,
}

impl KagemushaMintFinalityTreeV1 {
    /// Construct the unique canonical tree for one non-empty block-local top-up set.
    ///
    /// Input order is irrelevant.  Leaves are sorted by operation identifier,
    /// duplicates are rejected, and all unused positions use the protocol-fixed
    /// empty leaf.
    ///
    /// # Errors
    ///
    /// Returns an error for an empty/non-representable set, invalid leaf, or duplicate
    /// operation identifier.
    pub fn new(
        mut leaves: Vec<KagemushaTopUpLeafV1>,
    ) -> Result<Self, KagemushaMintFinalityErrorV1> {
        if leaves.is_empty() || u32::try_from(leaves.len()).is_err() {
            return Err(KagemushaMintFinalityErrorV1::InvalidTopUpTree(
                "leaf count must be non-zero and fit the 32-bit sparse index space".to_owned(),
            ));
        }
        for leaf in &leaves {
            leaf.validate().map_err(|error| {
                KagemushaMintFinalityErrorV1::InvalidTopUpTree(error.to_string())
            })?;
        }
        leaves.sort_by_key(|leaf| leaf.operation_id);
        if leaves
            .windows(2)
            .any(|pair| pair[0].operation_id == pair[1].operation_id)
        {
            return Err(KagemushaMintFinalityErrorV1::InvalidTopUpTree(
                "operation identifiers must be unique".to_owned(),
            ));
        }

        let first = leaves
            .iter()
            .enumerate()
            .map(|(index, leaf)| {
                (
                    u32::try_from(index).expect("validated sparse leaf index fits u32"),
                    top_up_leaf_commitment_v1(leaf),
                )
            })
            .collect::<BTreeMap<_, _>>();
        let mut empty_roots = Vec::with_capacity(KAGEMUSHA_MINT_FINALITY_TREE_DEPTH_V1 + 1);
        empty_roots.push(empty_top_up_leaf_commitment_v1());
        for level in 0..KAGEMUSHA_MINT_FINALITY_TREE_DEPTH_V1 {
            empty_roots.push(top_up_node_commitment_v1(
                empty_roots[level],
                empty_roots[level],
            )?);
        }
        let mut levels = vec![first];
        for level in 0..KAGEMUSHA_MINT_FINALITY_TREE_DEPTH_V1 {
            let children = levels.last().expect("the sparse child level is present");
            let parent_indices = children
                .keys()
                .map(|index| index >> 1)
                .collect::<BTreeSet<_>>();
            let mut parent = BTreeMap::new();
            for parent_index in parent_indices {
                let left_index = parent_index
                    .checked_mul(2)
                    .expect("a sparse parent index has representable children");
                let right_index = left_index | 1;
                let left = children
                    .get(&left_index)
                    .copied()
                    .unwrap_or(empty_roots[level]);
                let right = children
                    .get(&right_index)
                    .copied()
                    .unwrap_or(empty_roots[level]);
                parent.insert(parent_index, top_up_node_commitment_v1(left, right)?);
            }
            levels.push(parent);
        }
        Ok(Self {
            leaves,
            levels,
            empty_roots,
        })
    }

    /// Return the number of real, non-padding leaves.
    #[must_use]
    pub fn leaf_count(&self) -> u32 {
        u32::try_from(self.leaves.len()).expect("validated sparse tree count fits u32")
    }

    /// Borrow the canonical operation-id-sorted real leaves.
    #[must_use]
    pub fn leaves(&self) -> &[KagemushaTopUpLeafV1] {
        &self.leaves
    }

    /// Return the paired field-native root.
    #[must_use]
    pub fn root(&self) -> KagemushaPastaStateCommitmentV1 {
        self.levels[KAGEMUSHA_MINT_FINALITY_TREE_DEPTH_V1]
            .get(&0)
            .copied()
            .unwrap_or(self.empty_roots[KAGEMUSHA_MINT_FINALITY_TREE_DEPTH_V1])
    }

    /// Return the marked consensus hash stored in `ExecutionCommitment`.
    #[must_use]
    pub fn execution_root(&self) -> iroha_crypto::Hash {
        kagemusha_mint_finality_root_v1(self.root())
    }

    /// Build the exact 32-sibling witness for one operation.
    ///
    /// # Errors
    ///
    /// Returns an error if the operation is absent.
    pub fn witness(
        &self,
        operation_id: [u8; 32],
    ) -> Result<KagemushaTopUpMembershipWitnessV1, KagemushaMintFinalityErrorV1> {
        let leaf_position = self
            .leaves
            .binary_search_by_key(&operation_id, |leaf| leaf.operation_id)
            .map_err(|_| {
                KagemushaMintFinalityErrorV1::InvalidTopUpTree(
                    "operation is absent from the canonical tree".to_owned(),
                )
            })?;
        let leaf_index = u32::try_from(leaf_position).expect("validated sparse index fits u32");
        let mut index = leaf_index;
        let mut siblings = Vec::with_capacity(KAGEMUSHA_MINT_FINALITY_TREE_DEPTH_V1);
        for level in 0..KAGEMUSHA_MINT_FINALITY_TREE_DEPTH_V1 {
            siblings.push(
                self.levels[level]
                    .get(&(index ^ 1))
                    .copied()
                    .unwrap_or(self.empty_roots[level]),
            );
            index >>= 1;
        }
        Ok(KagemushaTopUpMembershipWitnessV1 {
            leaf: self.leaves[leaf_position].clone(),
            leaf_index,
            root: self.root(),
            siblings,
        })
    }
}

/// Return the protocol-fixed empty sparse depth-32 top-up root.
///
/// Boundary Commit votes use this value when no top-up occurs so the old generation can still
/// authenticate the next Pasta roster without inventing an execution-commitment leaf.
///
/// # Errors
///
/// Returns an error only if an internal canonical Pasta encoding cannot be decoded.
pub fn kagemusha_mint_finality_empty_root_v1()
-> Result<KagemushaPastaStateCommitmentV1, KagemushaMintFinalityErrorV1> {
    let mut root = empty_top_up_leaf_commitment_v1();
    for _ in 0..KAGEMUSHA_MINT_FINALITY_TREE_DEPTH_V1 {
        root = top_up_node_commitment_v1(root, root)?;
    }
    Ok(root)
}

/// Convert one consensus reserve receipt into the exact top-up tree leaf.
///
/// # Errors
///
/// Returns an error unless the receipt is a valid top-up and carries the
/// non-zero mint-statement digest fixed at commit time.
pub fn kagemusha_top_up_leaf_from_receipt_v1(
    receipt: &KagemushaReserveReceiptV1,
) -> Result<KagemushaTopUpLeafV1, KagemushaMintFinalityErrorV1> {
    receipt
        .validate()
        .map_err(|error| KagemushaMintFinalityErrorV1::InvalidTopUpTree(error.to_string()))?;
    if receipt.kind != KagemushaOperationKindV1::TopUp {
        return Err(KagemushaMintFinalityErrorV1::InvalidTopUpTree(
            "redemption receipts cannot enter the mint tree".to_owned(),
        ));
    }
    let reserve_receipt_digest = receipt
        .canonical_digest()
        .map_err(|error| KagemushaMintFinalityErrorV1::InvalidTopUpTree(error.to_string()))?;
    Ok(KagemushaTopUpLeafV1 {
        version: KAGEMUSHA_CHAIN_VERSION_V1,
        operation_id: receipt.operation_id,
        reserve_receipt_digest,
        statement_digest: receipt.mint_statement_digest,
        amount: receipt.amount,
    })
}

/// Recompute both parity roots of a private membership witness.
///
/// # Errors
///
/// Returns an error for malformed field encodings, path shape, or a root mismatch.
pub fn verify_kagemusha_top_up_membership_v1(
    witness: &KagemushaTopUpMembershipWitnessV1,
    top_up_count: u32,
) -> Result<(), KagemushaMintFinalityErrorV1> {
    witness
        .validate(top_up_count)
        .map_err(|error| KagemushaMintFinalityErrorV1::InvalidTopUpTree(error.to_string()))?;
    let mut current = top_up_leaf_commitment_v1(&witness.leaf);
    let mut index = witness.leaf_index;
    for sibling in &witness.siblings {
        current = if index & 1 == 0 {
            top_up_node_commitment_v1(current, *sibling)?
        } else {
            top_up_node_commitment_v1(*sibling, current)?
        };
        index >>= 1;
    }
    if current != witness.root {
        return Err(KagemushaMintFinalityErrorV1::InvalidTopUpTree(
            "membership path does not reconstruct the paired root".to_owned(),
        ));
    }
    Ok(())
}

/// Decode every paired-Pasta public key in a structurally valid generation roster.
///
/// Genesis freeze calls this immediately after binding a networkless signed
/// template to the final network. Runtime verification calls it again at the
/// point of use, so malformed compressed points fail closed before any share
/// can be accepted.
///
/// # Errors
///
/// Returns an error unless the roster is structurally valid and every Pallas
/// and Vesta encoding is canonical and non-identity.
pub fn validate_kagemusha_mint_finality_roster_keys_v1(
    generation: &KagemushaMintFinalityAuthorityGenerationV1,
) -> Result<(), KagemushaMintFinalityErrorV1> {
    generation.validate().map_err(|error| {
        KagemushaMintFinalityErrorV1::InvalidAuthorityGeneration(error.to_string())
    })?;
    validate_kagemusha_mint_finality_validator_keys_v1(&generation.validators)
}

/// Decode every paired-Pasta public key in network-independent genesis authority parameters.
///
/// Operator tooling calls this before a genesis network identity exists, so malformed compressed
/// points fail closed before a manifest can be generated or signed.
///
/// # Errors
///
/// Returns an error unless the single generation-zero authority template is structurally valid
/// and every Pallas and Vesta encoding is canonical and non-identity.
pub fn validate_kagemusha_mint_finality_genesis_parameter_keys_v1(
    parameters: &KagemushaMintFinalityGenesisParametersV1,
) -> Result<(), KagemushaMintFinalityErrorV1> {
    parameters.validate().map_err(|error| {
        KagemushaMintFinalityErrorV1::InvalidAuthorityGeneration(error.to_string())
    })?;
    validate_kagemusha_mint_finality_validator_keys_v1(&parameters.authority_generation.validators)
}

fn validate_kagemusha_mint_finality_validator_keys_v1(
    validators: &[KagemushaMintFinalityValidatorKeysV1],
) -> Result<(), KagemushaMintFinalityErrorV1> {
    for keys in validators {
        iroha_zkp_poseidon::pasta_keys::validate_paired_public_keys(
            &keys.eq_proof_public_key,
            &keys.ep_proof_public_key,
        )
        .map_err(|error| KagemushaMintFinalityErrorV1::InvalidAuthorityGeneration(error.into()))?;
    }
    Ok(())
}

/// Derive one validator's generation-scoped public keys from separately provisioned seed material.
///
/// This helper is for provisioning only.  It never accepts a consensus private
/// key and there is no BLS-to-Pasta fallback. Deployments must provision an
/// independent seed per network; the final network identity remains bound by
/// the runtime roster identifier, signature statement, and deterministic nonce.
///
/// # Errors
///
/// Returns an error only if deterministic non-zero scalar derivation exhausts
/// its counter space.
pub fn derive_kagemusha_mint_finality_validator_keys_v1(
    seed: &[u8; 32],
    generation: u64,
    validator: iroha_model_base::peer::PeerId,
) -> Result<KagemushaMintFinalityValidatorKeysV1, KagemushaMintFinalityErrorV1> {
    let validator_bytes = validator.encode();
    let eq_secret =
        derive_nonzero_key_scalar::<Fq>(EQ_PARITY_TAG, seed, generation, &validator_bytes)?;
    let ep_secret =
        derive_nonzero_key_scalar::<Fp>(EP_PARITY_TAG, seed, generation, &validator_bytes)?;
    Ok(KagemushaMintFinalityValidatorKeysV1 {
        validator,
        eq_proof_public_key: encode_point::<EpAffine>(
            (<EpAffine as CurveAffine>::CurveExt::generator() * eq_secret.value).to_affine(),
        ),
        ep_proof_public_key: encode_point::<EqAffine>(
            (<EqAffine as CurveAffine>::CurveExt::generator() * ep_secret.value).to_affine(),
        ),
    })
}

/// Prove possession of candidate Pasta keys before a committee or beacon is selected.
///
/// # Errors
/// Rejects malformed public keys, an incorrect independent seed, or invalid challenge identity.
pub fn prove_kagemusha_mint_finality_candidate_possession_v1(
    seed: &[u8; 32],
    network_id: iroha_data_model::NetworkId,
    generation: u64,
    keys: &KagemushaMintFinalityValidatorKeysV1,
) -> Result<KagemushaMintFinalityPairedPossessionProofV1, KagemushaMintFinalityErrorV1> {
    let digest =
        kagemusha_mint_finality_candidate_possession_digest_v1(network_id, generation, keys)
            .map_err(|error| KagemushaMintFinalityErrorV1::InvalidStatement(error.to_string()))?;
    prove_paired_possession(seed, network_id, generation, keys, digest)
}

/// Verify candidate consent under the exact network, generation, identity, and public keys.
///
/// # Errors
/// Rejects invalid curves or signatures and any replay under a different challenge.
pub fn verify_kagemusha_mint_finality_candidate_possession_v1(
    network_id: iroha_data_model::NetworkId,
    generation: u64,
    keys: &KagemushaMintFinalityValidatorKeysV1,
    proof: &KagemushaMintFinalityPairedPossessionProofV1,
) -> Result<(), KagemushaMintFinalityErrorV1> {
    let digest =
        kagemusha_mint_finality_candidate_possession_digest_v1(network_id, generation, keys)
            .map_err(|error| KagemushaMintFinalityErrorV1::InvalidStatement(error.to_string()))?;
    verify_paired_possession(keys, proof, digest)
}

/// Prove local possession of both Pasta keys for one exact prepared target seat.
///
/// This statement includes the finalized preparation attempt, target epoch and interval, exact
/// ordered committee commitment, and beacon transcript. It does not attest to beacon share custody.
///
/// # Errors
/// Rejects a mismatched authority, seat, challenge, or independent signing seed.
pub fn prove_kagemusha_mint_finality_seat_readiness_v1(
    seed: &[u8; 32],
    authority: &KagemushaMintFinalityAuthorityGenerationV1,
    context: &KagemushaMintFinalitySeatReadinessContextV1,
) -> Result<KagemushaMintFinalityPairedPossessionProofV1, KagemushaMintFinalityErrorV1> {
    let keys = readiness_keys(authority, context)?;
    let digest = context
        .signing_digest(keys)
        .map_err(|error| KagemushaMintFinalityErrorV1::InvalidStatement(error.to_string()))?;
    prove_paired_possession(
        seed,
        authority.network_id,
        authority.generation,
        keys,
        digest,
    )
}

/// Verify paired-key readiness for one exact seat of an authenticated prepared authority.
///
/// # Errors
/// Rejects missing seats, mismatched authority commitments, malformed keys or signatures, and
/// replay across target epochs, attempts, intervals, networks, committees, or beacon transcripts.
pub fn verify_kagemusha_mint_finality_seat_readiness_v1(
    authority: &KagemushaMintFinalityAuthorityGenerationV1,
    context: &KagemushaMintFinalitySeatReadinessContextV1,
    proof: &KagemushaMintFinalityPairedPossessionProofV1,
) -> Result<(), KagemushaMintFinalityErrorV1> {
    let keys = readiness_keys(authority, context)?;
    let digest = context
        .signing_digest(keys)
        .map_err(|error| KagemushaMintFinalityErrorV1::InvalidStatement(error.to_string()))?;
    verify_paired_possession(keys, proof, digest)
}

fn readiness_keys<'a>(
    authority: &'a KagemushaMintFinalityAuthorityGenerationV1,
    context: &KagemushaMintFinalitySeatReadinessContextV1,
) -> Result<&'a KagemushaMintFinalityValidatorKeysV1, KagemushaMintFinalityErrorV1> {
    context
        .validate()
        .map_err(|error| KagemushaMintFinalityErrorV1::InvalidStatement(error.to_string()))?;
    let authority_id = authority.authority_id().map_err(|error| {
        KagemushaMintFinalityErrorV1::InvalidAuthorityGeneration(error.to_string())
    })?;
    if context.network_id != authority.network_id
        || context.authority_generation != authority.generation
        || context.authority_id != authority_id
    {
        return Err(KagemushaMintFinalityErrorV1::InvalidAuthorityGeneration(
            "readiness challenge does not name the exact target authority".into(),
        ));
    }
    usize::try_from(context.validator_index)
        .ok()
        .and_then(|index| authority.validators.get(index))
        .ok_or_else(|| {
            KagemushaMintFinalityErrorV1::InvalidSigner(
                "readiness index is outside the exact target committee".into(),
            )
        })
}

fn prove_paired_possession(
    seed: &[u8; 32],
    network_id: iroha_data_model::NetworkId,
    generation: u64,
    keys: &KagemushaMintFinalityValidatorKeysV1,
    digest: [u8; 32],
) -> Result<KagemushaMintFinalityPairedPossessionProofV1, KagemushaMintFinalityErrorV1> {
    let derived =
        derive_kagemusha_mint_finality_validator_keys_v1(seed, generation, keys.validator.clone())?;
    if &derived != keys {
        return Err(KagemushaMintFinalityErrorV1::InvalidSigner(
            "independent seed does not possess the challenged paired public keys".into(),
        ));
    }
    let validator_bytes = keys.validator.encode();
    let eq_secret =
        derive_nonzero_key_scalar::<Fq>(EQ_PARITY_TAG, seed, generation, &validator_bytes)?;
    let ep_secret =
        derive_nonzero_key_scalar::<Fp>(EP_PARITY_TAG, seed, generation, &validator_bytes)?;
    Ok(KagemushaMintFinalityPairedPossessionProofV1 {
        eq_proof_signature: schnorr_sign::<EpAffine>(
            seed,
            &network_id,
            generation,
            &validator_bytes,
            0,
            EQ_PARITY_TAG,
            &eq_secret,
            digest,
        )?,
        ep_proof_signature: schnorr_sign::<EqAffine>(
            seed,
            &network_id,
            generation,
            &validator_bytes,
            0,
            EP_PARITY_TAG,
            &ep_secret,
            digest,
        )?,
    })
}

fn verify_paired_possession(
    keys: &KagemushaMintFinalityValidatorKeysV1,
    proof: &KagemushaMintFinalityPairedPossessionProofV1,
    digest: [u8; 32],
) -> Result<(), KagemushaMintFinalityErrorV1> {
    proof
        .validate()
        .map_err(|error| KagemushaMintFinalityErrorV1::InvalidSignature(error.to_string()))?;
    validate_kagemusha_mint_finality_validator_keys_v1(std::slice::from_ref(keys))?;
    schnorr_verify::<EpAffine>(
        EQ_PARITY_TAG,
        0,
        keys.eq_proof_public_key,
        &proof.eq_proof_signature,
        digest,
    )?;
    schnorr_verify::<EqAffine>(
        EP_PARITY_TAG,
        0,
        keys.ep_proof_public_key,
        &proof.ep_proof_signature,
        digest,
    )
}

/// Validator-local holder for separately provisioned mint-finality seed material.
pub struct KagemushaMintFinalitySignerV1 {
    seed: Zeroizing<[u8; 32]>,
    validator_index: u32,
    network_id: iroha_data_model::NetworkId,
    generation: u64,
    authority_id: [u8; 32],
    validator: iroha_model_base::peer::PeerId,
}

/// Runtime authority for one node's exact key generation and local signing seed.
///
/// Sumeragi may share this object through `Arc`; no key material is cloned or
/// exposed by the recursive relation.
pub struct KagemushaMintFinalityLocalAuthorityV1 {
    binding: LocalMintFinalityBindingV1,
}

enum LocalMintFinalityBindingV1 {
    Seated {
        generation: Arc<KagemushaMintFinalityAuthorityGenerationV1>,
        signer: KagemushaMintFinalitySignerV1,
    },
    Unseated {
        network_id: iroha_data_model::NetworkId,
        validator: iroha_model_base::peer::PeerId,
        seed: Zeroizing<[u8; 32]>,
        genesis_generation: u64,
    },
}

impl core::fmt::Debug for KagemushaMintFinalityLocalAuthorityV1 {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("KagemushaMintFinalityLocalAuthorityV1")
            .field("network_id", &self.network_id())
            .field("validator", &self.validator())
            .field("seated_at_genesis", &self.authority().is_some())
            .finish()
    }
}

impl KagemushaMintFinalityLocalAuthorityV1 {
    /// Bind separately provisioned seed material to one authenticated key generation.
    ///
    /// # Errors
    ///
    /// Returns an error unless the seed-derived keys exactly equal the local
    /// validator's roster entry.
    pub fn new(
        generation: Arc<KagemushaMintFinalityAuthorityGenerationV1>,
        seed: Zeroizing<[u8; 32]>,
        validator_index: u32,
    ) -> Result<Self, KagemushaMintFinalityErrorV1> {
        let signer =
            KagemushaMintFinalitySignerV1::from_seed(seed, validator_index, generation.as_ref())?;
        Ok(Self {
            binding: LocalMintFinalityBindingV1::Seated { generation, signer },
        })
    }

    /// Retain one private seed for a future candidate absent from signed genesis.
    ///
    /// This holder has no genesis vote authority. The seed can be used for candidate
    /// possession and prepared-seat readiness, while consensus derives a signer only
    /// from a later authenticated height context containing this exact peer and keys.
    ///
    /// # Errors
    ///
    /// Rejects a malformed genesis authority, a non-genesis generation, or a peer
    /// already seated in that authority.
    pub fn new_unseated(
        genesis: &KagemushaMintFinalityAuthorityGenerationV1,
        validator: iroha_model_base::peer::PeerId,
        seed: Zeroizing<[u8; 32]>,
    ) -> Result<Self, KagemushaMintFinalityErrorV1> {
        genesis.validate().map_err(|error| {
            KagemushaMintFinalityErrorV1::InvalidAuthorityGeneration(error.to_string())
        })?;
        if genesis.generation != 0
            || genesis
                .validators
                .iter()
                .any(|entry| entry.validator == validator)
        {
            return Err(KagemushaMintFinalityErrorV1::InvalidSigner(
                "unseated seed owner must be absent from signed genesis".to_owned(),
            ));
        }
        Ok(Self {
            binding: LocalMintFinalityBindingV1::Unseated {
                network_id: genesis.network_id,
                validator,
                seed,
                genesis_generation: genesis.generation,
            },
        })
    }

    fn seed(&self) -> &[u8; 32] {
        match &self.binding {
            LocalMintFinalityBindingV1::Seated { signer, .. } => &signer.seed,
            LocalMintFinalityBindingV1::Unseated { seed, .. } => seed,
        }
    }

    fn network_id(&self) -> iroha_data_model::NetworkId {
        match &self.binding {
            LocalMintFinalityBindingV1::Seated { signer, .. } => signer.network_id,
            LocalMintFinalityBindingV1::Unseated { network_id, .. } => *network_id,
        }
    }

    fn validator(&self) -> &iroha_model_base::peer::PeerId {
        match &self.binding {
            LocalMintFinalityBindingV1::Seated { signer, .. } => &signer.validator,
            LocalMintFinalityBindingV1::Unseated { validator, .. } => validator,
        }
    }

    /// Bind the held private seed to this validator in an authenticated key generation.
    ///
    /// The caller must obtain `generation` from its verified height context. Network identity and
    /// local validator identity remain fixed; every generation's published keys must exactly match
    /// private derivation. Historical contexts remain usable for authenticated recovery.
    ///
    /// # Errors
    ///
    /// Rejects another network, an absent local validator, malformed roster, or changed keys.
    #[doc(hidden)]
    pub fn signer_for_authority(
        &self,
        generation: &KagemushaMintFinalityAuthorityGenerationV1,
    ) -> Result<KagemushaMintFinalitySignerV1, KagemushaMintFinalityErrorV1> {
        if generation.network_id != self.network_id() {
            return Err(KagemushaMintFinalityErrorV1::InvalidSigner(
                "generation does not belong to the signer's admitted network".to_owned(),
            ));
        }
        if matches!(
            &self.binding,
            LocalMintFinalityBindingV1::Unseated {
                genesis_generation,
                ..
            } if generation.generation <= *genesis_generation
        ) {
            return Err(KagemushaMintFinalityErrorV1::InvalidSigner(
                "unseated genesis candidate cannot sign its original generation".to_owned(),
            ));
        }
        let index = generation
            .validators
            .iter()
            .position(|entry| &entry.validator == self.validator())
            .and_then(|index| u32::try_from(index).ok())
            .ok_or_else(|| {
                KagemushaMintFinalityErrorV1::InvalidSigner(
                    "local validator is absent from the authenticated generation roster".to_owned(),
                )
            })?;
        KagemushaMintFinalitySignerV1::from_seed(Zeroizing::new(*self.seed()), index, generation)
    }

    /// Derive and prove consent to this node's public keys for a proposed generation.
    ///
    /// Private seed material remains inside this runtime owner. Candidate publication does not
    /// authenticate a committee, transition or activation.
    ///
    /// # Errors
    /// Returns an error for invalid derivation or an invalid candidate possession statement.
    pub fn candidate_possession(
        &self,
        generation: u64,
    ) -> Result<
        (
            KagemushaMintFinalityValidatorKeysV1,
            KagemushaMintFinalityPairedPossessionProofV1,
        ),
        KagemushaMintFinalityErrorV1,
    > {
        let keys = derive_kagemusha_mint_finality_validator_keys_v1(
            self.seed(),
            generation,
            self.validator().clone(),
        )?;
        let proof = prove_kagemusha_mint_finality_candidate_possession_v1(
            self.seed(),
            self.network_id(),
            generation,
            &keys,
        )?;
        Ok((keys, proof))
    }

    #[cfg(test)]
    /// Prove readiness of this local peer in an authenticated prepared authority.
    ///
    /// This proves both Pasta keys while retaining the private seed in its custody owner. The
    /// caller must separately obtain the exact beacon-share proof and incumbent authorization.
    ///
    /// # Errors
    /// Rejects a different network or peer, changed published keys, or malformed context.
    pub fn prove_seat_readiness(
        &self,
        authority: &KagemushaMintFinalityAuthorityGenerationV1,
        context: &KagemushaMintFinalitySeatReadinessContextV1,
    ) -> Result<KagemushaMintFinalityPairedPossessionProofV1, KagemushaMintFinalityErrorV1> {
        let keys = readiness_keys(authority, context)?;
        if authority.network_id != self.network_id() || &keys.validator != self.validator() {
            return Err(KagemushaMintFinalityErrorV1::InvalidSigner(
                "prepared seat does not belong to the local authority".to_owned(),
            ));
        }
        prove_kagemusha_mint_finality_seat_readiness_v1(self.seed(), authority, context)
    }

    /// Borrow the immutable key generation admitted at startup.
    #[must_use]
    pub fn authority(&self) -> Option<&KagemushaMintFinalityAuthorityGenerationV1> {
        match &self.binding {
            LocalMintFinalityBindingV1::Seated { generation, .. } => Some(generation.as_ref()),
            LocalMintFinalityBindingV1::Unseated { .. } => None,
        }
    }

    /// Borrow the local signer without exposing its private seed.
    #[must_use]
    pub fn signer(&self) -> Option<&KagemushaMintFinalitySignerV1> {
        match &self.binding {
            LocalMintFinalityBindingV1::Seated { signer, .. } => Some(signer),
            LocalMintFinalityBindingV1::Unseated { .. } => None,
        }
    }
}

impl core::fmt::Debug for KagemushaMintFinalitySignerV1 {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("KagemushaMintFinalitySignerV1")
            .field("validator_index", &self.validator_index)
            .field("network_id", &self.network_id)
            .field("generation", &self.generation)
            .field("authority_id", &self.authority_id)
            .finish_non_exhaustive()
    }
}

impl KagemushaMintFinalitySignerV1 {
    /// Admit seed material only when its derived keys equal the authenticated roster entry.
    ///
    /// # Errors
    ///
    /// Returns an error for a malformed roster/index or a key mismatch.
    pub fn from_seed(
        seed: Zeroizing<[u8; 32]>,
        validator_index: u32,
        generation: &KagemushaMintFinalityAuthorityGenerationV1,
    ) -> Result<Self, KagemushaMintFinalityErrorV1> {
        generation
            .validate()
            .map_err(|error| KagemushaMintFinalityErrorV1::InvalidSigner(error.to_string()))?;
        let index = usize::try_from(validator_index).map_err(|_| {
            KagemushaMintFinalityErrorV1::InvalidSigner(
                "validator index does not fit usize".to_owned(),
            )
        })?;
        let expected = generation.validators.get(index).ok_or_else(|| {
            KagemushaMintFinalityErrorV1::InvalidSigner(
                "validator index is outside the generation roster".to_owned(),
            )
        })?;
        let derived = derive_kagemusha_mint_finality_validator_keys_v1(
            &seed,
            generation.generation,
            expected.validator.clone(),
        )?;
        if &derived != expected {
            return Err(KagemushaMintFinalityErrorV1::InvalidSigner(
                "seed-derived keys do not match the authenticated roster".to_owned(),
            ));
        }
        Ok(Self {
            seed,
            validator_index,
            network_id: generation.network_id,
            generation: generation.generation,
            authority_id: generation
                .authority_id()
                .map_err(|error| KagemushaMintFinalityErrorV1::InvalidSigner(error.to_string()))?,
            validator: expected.validator.clone(),
        })
    }

    /// Return the exact frozen-roster position owned by this signer.
    #[must_use]
    pub const fn validator_index(&self) -> u32 {
        self.validator_index
    }

    /// Sign one already validated block-level statement with both Pasta keys.
    ///
    /// # Errors
    ///
    /// Returns an error when the statement names another generation/network/roster
    /// or deterministic nonce derivation fails.
    pub fn sign(
        &self,
        message: &KagemushaMintFinalitySealMessageV1,
    ) -> Result<KagemushaMintFinalityValidatorSealV1, KagemushaMintFinalityErrorV1> {
        message
            .validate()
            .map_err(|error| KagemushaMintFinalityErrorV1::InvalidStatement(error.to_string()))?;
        if message.network_id != self.network_id
            || message.epoch_authorization.authority_id != self.authority_id
            || message.epoch_authorization.authority_generation != self.generation
            || self.validator_index >= message.validator_count
        {
            return Err(KagemushaMintFinalityErrorV1::InvalidSigner(
                "message does not belong to the signer's admitted generation".to_owned(),
            ));
        }
        let signing_digest = message
            .signing_digest()
            .map_err(|error| KagemushaMintFinalityErrorV1::InvalidStatement(error.to_string()))?;
        let validator_bytes = self.validator.encode();
        let eq_secret = derive_nonzero_key_scalar::<Fq>(
            EQ_PARITY_TAG,
            &self.seed,
            self.generation,
            &validator_bytes,
        )?;
        let ep_secret = derive_nonzero_key_scalar::<Fp>(
            EP_PARITY_TAG,
            &self.seed,
            self.generation,
            &validator_bytes,
        )?;
        Ok(KagemushaMintFinalityValidatorSealV1 {
            validator_index: self.validator_index,
            eq_proof_signature: schnorr_sign::<EpAffine>(
                &self.seed,
                &self.network_id,
                self.generation,
                &validator_bytes,
                self.validator_index,
                EQ_PARITY_TAG,
                &eq_secret,
                signing_digest,
            )?,
            ep_proof_signature: schnorr_sign::<EqAffine>(
                &self.seed,
                &self.network_id,
                self.generation,
                &validator_bytes,
                self.validator_index,
                EP_PARITY_TAG,
                &ep_secret,
                signing_digest,
            )?,
        })
    }
}

fn top_up_leaf_component<F: KagemushaPoseidonFieldV1>(leaf: &KagemushaTopUpLeafV1) -> F {
    let operation = digest_limbs::<F>(leaf.operation_id);
    let receipt = digest_limbs::<F>(leaf.reserve_receipt_digest);
    let statement = digest_limbs::<F>(leaf.statement_digest);
    hash(
        MINT_LEAF_DOMAIN_V1,
        &[
            operation[0],
            operation[1],
            receipt[0],
            receipt[1],
            statement[0],
            statement[1],
            from_u128(leaf.amount),
        ],
    )
}

fn top_up_leaf_commitment_v1(leaf: &KagemushaTopUpLeafV1) -> KagemushaPastaStateCommitmentV1 {
    KagemushaPastaStateCommitmentV1 {
        eq: encode(top_up_leaf_component::<Fp>(leaf)),
        ep: encode(top_up_leaf_component::<Fq>(leaf)),
    }
}

fn empty_top_up_leaf_commitment_v1() -> KagemushaPastaStateCommitmentV1 {
    KagemushaPastaStateCommitmentV1 {
        eq: encode(hash::<Fp>(MINT_EMPTY_DOMAIN_V1, &[])),
        ep: encode(hash::<Fq>(MINT_EMPTY_DOMAIN_V1, &[])),
    }
}

fn top_up_node_commitment_v1(
    left: KagemushaPastaStateCommitmentV1,
    right: KagemushaPastaStateCommitmentV1,
) -> Result<KagemushaPastaStateCommitmentV1, KagemushaMintFinalityErrorV1> {
    let left_eq = decode::<Fp>(left.eq).ok_or_else(|| {
        KagemushaMintFinalityErrorV1::InvalidTopUpTree("left Eq root is not canonical".to_owned())
    })?;
    let right_eq = decode::<Fp>(right.eq).ok_or_else(|| {
        KagemushaMintFinalityErrorV1::InvalidTopUpTree("right Eq root is not canonical".to_owned())
    })?;
    let left_ep = decode::<Fq>(left.ep).ok_or_else(|| {
        KagemushaMintFinalityErrorV1::InvalidTopUpTree("left Ep root is not canonical".to_owned())
    })?;
    let right_ep = decode::<Fq>(right.ep).ok_or_else(|| {
        KagemushaMintFinalityErrorV1::InvalidTopUpTree("right Ep root is not canonical".to_owned())
    })?;
    Ok(KagemushaPastaStateCommitmentV1 {
        eq: encode(hash(MINT_NODE_DOMAIN_V1, &[left_eq, right_eq])),
        ep: encode(hash(MINT_NODE_DOMAIN_V1, &[left_ep, right_ep])),
    })
}

/// Verify one genuine paired-Pasta seal against its exact immutable generation and epoch.
/// The native consensus caller additionally binds the message to its source-complete signed R.
///
/// # Errors
/// Rejects substituted generation, network, authorization, count, signer or either signature.
pub fn verify_kagemusha_mint_finality_validator_seal_v1(
    generation: &KagemushaMintFinalityAuthorityGenerationV1,
    message: &KagemushaMintFinalitySealMessageV1,
    seal: &KagemushaMintFinalityValidatorSealV1,
) -> Result<(), KagemushaMintFinalityErrorV1> {
    validate_kagemusha_mint_finality_roster_keys_v1(generation)?;
    message
        .epoch_authorization
        .validate_against_authority(generation)
        .map_err(|error| {
            KagemushaMintFinalityErrorV1::InvalidAuthorityGeneration(error.to_string())
        })?;
    if message.network_id != generation.network_id
        || usize::try_from(message.validator_count).ok() != Some(generation.validators.len())
    {
        return Err(KagemushaMintFinalityErrorV1::InvalidAuthorityGeneration(
            "seal message differs from its complete immutable authority".into(),
        ));
    }
    let index = usize::try_from(seal.validator_index).map_err(|_| {
        KagemushaMintFinalityErrorV1::InvalidSignature(
            "validator index does not fit usize".to_owned(),
        )
    })?;
    let keys = generation.validators.get(index).ok_or_else(|| {
        KagemushaMintFinalityErrorV1::InvalidSignature(
            "validator index is outside the authenticated generation".to_owned(),
        )
    })?;
    let digest = message
        .signing_digest()
        .map_err(|error| KagemushaMintFinalityErrorV1::InvalidStatement(error.to_string()))?;
    schnorr_verify::<EpAffine>(
        EQ_PARITY_TAG,
        seal.validator_index,
        keys.eq_proof_public_key,
        &seal.eq_proof_signature,
        digest,
    )?;
    schnorr_verify::<EqAffine>(
        EP_PARITY_TAG,
        seal.validator_index,
        keys.ep_proof_public_key,
        &seal.ep_proof_signature,
        digest,
    )
}

#[allow(clippy::too_many_arguments)]
fn schnorr_sign<C>(
    seed: &[u8; 32],
    network_id: &iroha_data_model::NetworkId,
    generation: u64,
    validator_bytes: &[u8],
    validator_index: u32,
    parity: u8,
    secret: &ScalarToZeroize<C::ScalarExt>,
    signing_digest: [u8; 32],
) -> Result<KagemushaPastaSchnorrSignatureV1, KagemushaMintFinalityErrorV1>
where
    C: CurveAffine,
    C::ScalarExt: FromUniformBytes<64> + PrimeField,
{
    let public = (C::CurveExt::generator() * secret.value).to_affine();
    let public_bytes = encode_point(public);
    for counter in 0..u32::MAX {
        let nonce = derive_nonzero_nonce_scalar::<C::ScalarExt>(
            parity,
            seed,
            network_id,
            generation,
            validator_bytes,
            &[&signing_digest[..], &counter.to_le_bytes()].concat(),
        )?;
        let nonce_point = (C::CurveExt::generator() * nonce.value).to_affine();
        let nonce_commitment = encode_point(nonce_point);
        let challenge = schnorr_challenge::<C::ScalarExt>(
            parity,
            validator_index,
            signing_digest,
            nonce_commitment,
            public_bytes,
        );
        let response = nonce.value + challenge * secret.value;
        if !bool::from(response.is_zero()) {
            return Ok(KagemushaPastaSchnorrSignatureV1 {
                nonce_commitment,
                response: encode_scalar(response),
            });
        }
    }
    Err(KagemushaMintFinalityErrorV1::InvalidSigner(
        "deterministic nonce counter exhausted".to_owned(),
    ))
}

fn schnorr_verify<C>(
    parity: u8,
    validator_index: u32,
    public_key: [u8; 32],
    signature: &KagemushaPastaSchnorrSignatureV1,
    signing_digest: [u8; 32],
) -> Result<(), KagemushaMintFinalityErrorV1>
where
    C: CurveAffine,
    C::ScalarExt: PrimeField,
{
    let public = decode_nonidentity_point::<C>(public_key).ok_or_else(|| {
        KagemushaMintFinalityErrorV1::InvalidSignature(
            "public key is not a canonical non-identity point".to_owned(),
        )
    })?;
    let nonce = decode_nonidentity_point::<C>(signature.nonce_commitment).ok_or_else(|| {
        KagemushaMintFinalityErrorV1::InvalidSignature(
            "nonce commitment is not a canonical non-identity point".to_owned(),
        )
    })?;
    let response = decode_scalar::<C::ScalarExt>(signature.response)
        .filter(|value| !bool::from(value.is_zero()))
        .ok_or_else(|| {
            KagemushaMintFinalityErrorV1::InvalidSignature(
                "response is not a canonical non-zero scalar".to_owned(),
            )
        })?;
    let challenge = schnorr_challenge::<C::ScalarExt>(
        parity,
        validator_index,
        signing_digest,
        signature.nonce_commitment,
        public_key,
    );
    let lhs = C::CurveExt::generator() * response;
    let rhs = C::CurveExt::from(nonce) + C::CurveExt::from(public) * challenge;
    if lhs != rhs {
        return Err(KagemushaMintFinalityErrorV1::InvalidSignature(
            if parity == EQ_PARITY_TAG {
                "Eq/Fp helper Schnorr equation failed"
            } else {
                "Ep/Fq helper Schnorr equation failed"
            }
            .to_owned(),
        ));
    }
    Ok(())
}

/// Field storage whose zeroization result is the additive identity.
///
/// Keep secret values in `Zeroizing<ScalarToZeroize<_>>`: the field types are
/// `Copy`, so this wrapper alone does not erase copies or wipe on drop. The
/// retained owner is cleared using `zeroize`'s volatile write and fence. This
/// does not erase SHA-512 internals, arithmetic temporaries, or copies made
/// when Rust moves a value between storage locations.
#[derive(Clone, Copy)]
struct ScalarToZeroize<F: Field> {
    value: F,
}

impl<F: Field> Default for ScalarToZeroize<F> {
    fn default() -> Self {
        Self { value: F::ZERO }
    }
}

impl<F: Field> DefaultIsZeroes for ScalarToZeroize<F> {}

fn derive_nonzero_key_scalar<F>(
    parity: u8,
    seed: &[u8; 32],
    generation: u64,
    validator_bytes: &[u8],
) -> Result<Zeroizing<ScalarToZeroize<F>>, KagemushaMintFinalityErrorV1>
where
    F: Field + FromUniformBytes<64>,
{
    for counter in 0..u32::MAX {
        let mut hasher = Sha512::new();
        hasher.update(KEY_DERIVATION_DOMAIN_V1);
        hasher.update([0, parity]);
        hasher.update(seed);
        hasher.update(generation.to_le_bytes());
        hasher.update(
            u32::try_from(validator_bytes.len())
                .expect("bounded PeerId encoding length fits u32")
                .to_le_bytes(),
        );
        hasher.update(validator_bytes);
        hasher.update(counter.to_le_bytes());
        let mut uniform = Zeroizing::new([0_u8; 64]);
        hasher.finalize_into(sha2::digest::Output::<Sha512>::from_mut_slice(
            &mut uniform[..],
        ));
        let scalar = Zeroizing::new(ScalarToZeroize {
            value: F::from_uniform_bytes(&uniform),
        });
        if !bool::from(scalar.value.is_zero()) {
            return Ok(scalar);
        }
    }
    Err(KagemushaMintFinalityErrorV1::InvalidSigner(
        "non-zero scalar derivation counter exhausted".to_owned(),
    ))
}

#[allow(clippy::too_many_arguments)]
fn derive_nonzero_nonce_scalar<F>(
    parity: u8,
    seed: &[u8; 32],
    network_id: &iroha_data_model::NetworkId,
    generation: u64,
    validator_bytes: &[u8],
    extra: &[u8],
) -> Result<Zeroizing<ScalarToZeroize<F>>, KagemushaMintFinalityErrorV1>
where
    F: Field + FromUniformBytes<64>,
{
    for counter in 0..u32::MAX {
        let mut hasher = Sha512::new();
        hasher.update(NONCE_DERIVATION_DOMAIN_V1);
        hasher.update([0, parity]);
        hasher.update(seed);
        hasher.update(network_id.as_bytes());
        hasher.update(generation.to_le_bytes());
        hasher.update(
            u32::try_from(validator_bytes.len())
                .expect("bounded PeerId encoding length fits u32")
                .to_le_bytes(),
        );
        hasher.update(validator_bytes);
        hasher.update(
            u32::try_from(extra.len())
                .expect("bounded mint-finality derivation context fits u32")
                .to_le_bytes(),
        );
        hasher.update(extra);
        hasher.update(counter.to_le_bytes());
        let mut uniform = Zeroizing::new([0_u8; 64]);
        hasher.finalize_into(sha2::digest::Output::<Sha512>::from_mut_slice(
            &mut uniform[..],
        ));
        let scalar = Zeroizing::new(ScalarToZeroize {
            value: F::from_uniform_bytes(&uniform),
        });
        if !bool::from(scalar.value.is_zero()) {
            return Ok(scalar);
        }
    }
    Err(KagemushaMintFinalityErrorV1::InvalidSigner(
        "non-zero nonce derivation counter exhausted".to_owned(),
    ))
}

fn schnorr_challenge<F: PrimeField>(
    parity: u8,
    validator_index: u32,
    signing_digest: [u8; 32],
    nonce_commitment: [u8; 32],
    public_key: [u8; 32],
) -> F {
    let mut hasher = Sha256::new();
    hasher.update(CHALLENGE_DOMAIN_V1);
    hasher.update([0, parity]);
    hasher.update(validator_index.to_le_bytes());
    hasher.update(signing_digest);
    hasher.update(nonce_commitment);
    hasher.update(public_key);
    let digest: [u8; 32] = hasher.finalize().into();
    from_u128(u128::from_le_bytes(
        digest[..16].try_into().expect("fixed challenge half"),
    ))
}

fn encode_point<C: CurveAffine>(point: C) -> [u8; 32] {
    point
        .to_bytes()
        .as_ref()
        .try_into()
        .expect("Pasta compressed points are exactly 32 bytes")
}

fn decode_nonidentity_point<C: CurveAffine>(bytes: [u8; 32]) -> Option<C> {
    let mut repr = <C as halo2_proofs::halo2curves::group::GroupEncoding>::Repr::default();
    repr.as_mut().copy_from_slice(&bytes);
    Option::<C>::from(C::from_bytes(&repr)).filter(|point| !bool::from(point.is_identity()))
}

fn encode_scalar<F: PrimeField>(scalar: F) -> [u8; 32] {
    scalar
        .to_repr()
        .as_ref()
        .try_into()
        .expect("Pasta scalar representations are exactly 32 bytes")
}

fn decode_scalar<F: PrimeField>(bytes: [u8; 32]) -> Option<F> {
    let mut repr = F::Repr::default();
    repr.as_mut().copy_from_slice(&bytes);
    Option::from(F::from_repr(repr))
}

#[cfg(test)]
mod tests {
    use super::*;
    use iroha_crypto::{Algorithm, Hash, HashOf, KeyPair};
    use iroha_data_model::{
        NetworkId,
        block::BlockHeader,
        isi::kagemusha_v1::{
            KAGEMUSHA_CHAIN_VERSION_V1, KagemushaMintFinalityAuthorityGenerationV1,
        },
    };
    use iroha_model_base::peer::PeerId;

    fn peer(seed: u8) -> PeerId {
        let key_pair = KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519)
            .expect("derive deterministic mint-finality test peer");
        PeerId::new(key_pair.public_key().clone())
    }

    // These fixtures use a synthetic seed/context and an independent integer
    // model of the V1 SHA transcripts, Pasta moduli, generator, and point codec.
    fn assert_secret_hygiene_vector<C>(
        parity: u8,
        expected_key: [u8; 32],
        expected_nonce: [u8; 32],
        expected_public: [u8; 32],
        expected_commitment: [u8; 32],
        expected_response: [u8; 32],
    ) where
        C: CurveAffine,
        C::ScalarExt: FromUniformBytes<64> + PrimeField,
    {
        let seed = [0xA5; 32];
        let network_id = NetworkId::from_genesis_hash(
            HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([0xA7; 32])),
        );
        let validator_bytes = b"mint-secret-hygiene-v1";
        let digest = [0xA9; 32];
        let key = derive_nonzero_key_scalar::<C::ScalarExt>(parity, &seed, 7, validator_bytes)
            .expect("derive fixed key vector");
        assert_eq!(encode_scalar(key.value), expected_key);
        assert_eq!(
            encode_point::<C>((C::CurveExt::generator() * key.value).to_affine()),
            expected_public,
        );
        let mut nonce_context = [0; 36];
        nonce_context[..32].copy_from_slice(&digest);
        let nonce = derive_nonzero_nonce_scalar::<C::ScalarExt>(
            parity,
            &seed,
            &network_id,
            7,
            validator_bytes,
            &nonce_context,
        )
        .expect("derive fixed first nonce vector");
        assert_eq!(encode_scalar(nonce.value), expected_nonce);
        let signature = schnorr_sign::<C>(
            &seed,
            &network_id,
            7,
            validator_bytes,
            3,
            parity,
            &key,
            digest,
        )
        .expect("sign fixed vector with borrowed protected key");
        assert_eq!(signature.nonce_commitment, expected_commitment);
        assert_eq!(signature.response, expected_response);
        schnorr_verify::<C>(parity, 3, expected_public, &signature, digest)
            .expect("verify unchanged V1 signature vector");
        assert!(schnorr_verify::<C>(parity, 3, expected_public, &signature, [0xAA; 32]).is_err());
    }

    #[test]
    fn retained_secret_scalars_zeroize_to_pasta_zero() {
        use zeroize::{Zeroize as _, ZeroizeOnDrop};

        fn assert_drop_owner<T: ZeroizeOnDrop>() {}
        assert_drop_owner::<Zeroizing<ScalarToZeroize<Fp>>>();
        assert_drop_owner::<Zeroizing<ScalarToZeroize<Fq>>>();
        let mut fp = derive_nonzero_key_scalar::<Fp>(1, &[0xA5; 32], 7, b"wipe-test")
            .expect("derive nonzero Fp owner");
        let mut fq = derive_nonzero_key_scalar::<Fq>(0, &[0xA5; 32], 7, b"wipe-test")
            .expect("derive nonzero Fq owner");
        assert!(!bool::from(fp.value.is_zero()));
        assert!(!bool::from(fq.value.is_zero()));
        fp.zeroize();
        fq.zeroize();
        assert_eq!(fp.value, Fp::ZERO);
        assert_eq!(fq.value, Fq::ZERO);
    }

    #[test]
    fn protected_fq_key_nonce_and_signature_match_v1_vector() {
        assert_secret_hygiene_vector::<EpAffine>(
            0,
            [
                0xc6, 0x06, 0x42, 0x05, 0xbd, 0x16, 0xfb, 0xc0, 0x4d, 0x2c, 0x5c, 0x24, 0xe0, 0x63,
                0x5a, 0x8e, 0xda, 0x94, 0x69, 0x41, 0x19, 0xc0, 0x6b, 0x05, 0x00, 0xf5, 0xb9, 0xf4,
                0x9d, 0x4a, 0x9e, 0x1f,
            ],
            [
                0xff, 0x41, 0x16, 0x55, 0x14, 0xe5, 0x67, 0x8b, 0x53, 0x75, 0x81, 0xb0, 0xdc, 0x1a,
                0x61, 0x60, 0xbd, 0xf4, 0xff, 0xfe, 0x09, 0x1e, 0x9f, 0xbb, 0x54, 0xed, 0x29, 0xe1,
                0x1e, 0xa5, 0x6b, 0x1e,
            ],
            [
                0xf2, 0xa7, 0x46, 0x3a, 0x87, 0x6d, 0x0d, 0x73, 0xed, 0xaf, 0xe5, 0x9d, 0xa1, 0x57,
                0x2f, 0xf1, 0x53, 0x64, 0x5d, 0x45, 0x7e, 0x41, 0xcb, 0x4c, 0x9f, 0xde, 0x55, 0x0f,
                0x19, 0x11, 0xb8, 0x86,
            ],
            [
                0xb3, 0x90, 0x6e, 0x6b, 0xdf, 0xba, 0x3a, 0xc3, 0x19, 0x81, 0xfb, 0x41, 0xc4, 0x7d,
                0xdf, 0x61, 0x9c, 0xdc, 0x77, 0xdb, 0x16, 0x0f, 0x29, 0x8e, 0xe5, 0xe1, 0xf8, 0x98,
                0x9c, 0x54, 0xa6, 0xbd,
            ],
            [
                0xa4, 0xe6, 0xa1, 0x7a, 0x0b, 0x80, 0x16, 0x60, 0xba, 0xeb, 0xa1, 0xbf, 0x06, 0xe2,
                0x2f, 0x11, 0x3b, 0xaa, 0x47, 0xee, 0x9a, 0x21, 0xeb, 0x97, 0xfb, 0x95, 0xd7, 0x45,
                0x61, 0x95, 0x34, 0x1c,
            ],
        );
    }

    #[test]
    fn protected_fp_key_nonce_and_signature_match_v1_vector() {
        assert_secret_hygiene_vector::<EqAffine>(
            1,
            [
                0x18, 0x7b, 0x15, 0x7e, 0x29, 0x76, 0x0c, 0xc5, 0x9f, 0x2a, 0xdd, 0x01, 0x04, 0x7f,
                0xaa, 0xac, 0xab, 0x36, 0xd4, 0x37, 0xd1, 0x1a, 0x71, 0x89, 0x13, 0x84, 0xd9, 0xa5,
                0xb2, 0x6b, 0xc3, 0x04,
            ],
            [
                0x52, 0xc7, 0xf9, 0xec, 0x2d, 0xb0, 0x0d, 0x95, 0xce, 0x57, 0xcb, 0x3c, 0x81, 0x83,
                0x82, 0x99, 0x2c, 0x34, 0xa8, 0xd9, 0xc5, 0xd1, 0x3c, 0xc9, 0x89, 0xef, 0x0e, 0x10,
                0x3f, 0x2b, 0x7f, 0x1a,
            ],
            [
                0x36, 0x89, 0x33, 0xef, 0xbd, 0xa9, 0x85, 0x48, 0x22, 0x45, 0x94, 0x1a, 0x7f, 0x4b,
                0x0a, 0xb8, 0x51, 0x00, 0xd8, 0x0c, 0xc0, 0xe6, 0x82, 0xee, 0x5f, 0x2d, 0x29, 0x4d,
                0xb0, 0xa7, 0x44, 0x2f,
            ],
            [
                0x1a, 0x1c, 0xca, 0xe2, 0xc8, 0x8b, 0xea, 0xc7, 0x39, 0x1d, 0x99, 0x40, 0x25, 0x50,
                0x43, 0xab, 0xf0, 0x01, 0xd4, 0xb5, 0xc7, 0xb1, 0xd0, 0x7f, 0x76, 0x18, 0xf0, 0x38,
                0x42, 0xa0, 0x76, 0x9d,
            ],
            [
                0xa5, 0xa7, 0xc0, 0x1b, 0x52, 0x1c, 0xf5, 0x0f, 0x17, 0x4b, 0x53, 0x9a, 0xf5, 0xdc,
                0x36, 0xd7, 0x79, 0x89, 0x39, 0x56, 0x8a, 0x9b, 0xa3, 0xef, 0xd1, 0x84, 0xfc, 0x42,
                0xcf, 0xc3, 0xdc, 0x07,
            ],
        );
    }

    #[test]
    fn validator_key_derivation_is_deterministic_and_context_separated() {
        let seed = [0xA5; 32];
        let validator = peer(1);
        let baseline =
            derive_kagemusha_mint_finality_validator_keys_v1(&seed, 7, validator.clone())
                .expect("derive baseline keys");
        assert_eq!(
            baseline,
            derive_kagemusha_mint_finality_validator_keys_v1(&seed, 7, validator.clone())
                .expect("repeat deterministic derivation")
        );
        assert_ne!(
            baseline,
            derive_kagemusha_mint_finality_validator_keys_v1(&[0xA6; 32], 7, validator.clone())
                .expect("derive with another seed")
        );
        assert_ne!(
            baseline,
            derive_kagemusha_mint_finality_validator_keys_v1(&seed, 8, validator)
                .expect("derive in another generation")
        );
        assert_ne!(
            baseline,
            derive_kagemusha_mint_finality_validator_keys_v1(&seed, 7, peer(2))
                .expect("derive for another validator")
        );
    }

    fn runtime_generation_fixture(generation: u64) -> KagemushaMintFinalityAuthorityGenerationV1 {
        let mut validators = (1_u8..=4).map(peer).collect::<Vec<_>>();
        validators.sort();
        KagemushaMintFinalityAuthorityGenerationV1 {
            version: KAGEMUSHA_CHAIN_VERSION_V1,
            network_id: NetworkId::from_genesis_hash(
                HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(
                    b"runtime generation fixture",
                )),
            ),
            generation,
            validators: validators
                .into_iter()
                .enumerate()
                .map(|(index, validator)| {
                    derive_kagemusha_mint_finality_validator_keys_v1(
                        &[0xB0 + u8::try_from(index).expect("four validators"); 32],
                        generation,
                        validator,
                    )
                    .expect("derive exact generation fixture")
                })
                .collect(),
        }
    }

    #[test]
    fn runtime_authority_rebinds_private_seed_to_each_exact_authority_generation() {
        let generation_zero = runtime_generation_fixture(0);
        let authority = KagemushaMintFinalityLocalAuthorityV1::new(
            Arc::new(generation_zero.clone()),
            Zeroizing::new([0xB1; 32]),
            1,
        )
        .expect("bind epoch zero");
        let generation_one = runtime_generation_fixture(1);
        let signer = authority
            .signer_for_authority(&generation_one)
            .expect("bind authenticated next epoch");
        assert_eq!(signer.validator_index(), 1);
        assert_eq!(signer.validator, generation_zero.validators[1].validator);
        assert_eq!(signer.generation, 1);
        assert_eq!(
            signer.authority_id,
            generation_one.authority_id().expect("epoch id")
        );
        assert!(
            authority.signer_for_authority(&generation_zero).is_ok(),
            "exact recovery context remains valid"
        );
    }

    #[test]
    fn unseated_seed_waits_for_exact_authenticated_seven_seat_generation() {
        let genesis = runtime_generation_fixture(0);
        let candidate = peer(7);
        let seed = [0xC7; 32];
        let holder = KagemushaMintFinalityLocalAuthorityV1::new_unseated(
            &genesis,
            candidate.clone(),
            Zeroizing::new(seed),
        )
        .expect("retain an unseated candidate seed");
        assert!(holder.authority().is_none());
        assert!(holder.signer().is_none());
        assert!(holder.signer_for_authority(&genesis).is_err());
        assert!(
            KagemushaMintFinalityLocalAuthorityV1::new_unseated(
                &genesis,
                genesis.validators[0].validator.clone(),
                Zeroizing::new([0xB0; 32]),
            )
            .is_err()
        );

        let mut peers = (1_u8..=7).map(peer).collect::<Vec<_>>();
        peers.sort();
        let next = KagemushaMintFinalityAuthorityGenerationV1 {
            version: KAGEMUSHA_CHAIN_VERSION_V1,
            network_id: genesis.network_id,
            generation: 1,
            validators: peers
                .into_iter()
                .map(|validator| {
                    let original = genesis
                        .validators
                        .iter()
                        .position(|entry| entry.validator == validator);
                    let seed = original
                        .map(|index| 0xB0 + u8::try_from(index).expect("four seats"))
                        .unwrap_or_else(|| {
                            if validator == peer(5) {
                                0xC5
                            } else if validator == peer(6) {
                                0xC6
                            } else {
                                0xC7
                            }
                        });
                    derive_kagemusha_mint_finality_validator_keys_v1(&[seed; 32], 1, validator)
                        .expect("derive exact candidate keys")
                })
                .collect(),
        };
        next.validate().expect("seven-seat authority");
        let signer = holder
            .signer_for_authority(&next)
            .expect("authenticated activation seats this seed");
        assert_eq!(signer.validator, candidate);
        assert_eq!(signer.generation, 1);
        let resumed = KagemushaMintFinalityLocalAuthorityV1::new_unseated(
            &genesis,
            candidate.clone(),
            Zeroizing::new(seed),
        )
        .expect("restart retains the same seed");
        assert_eq!(
            resumed
                .signer_for_authority(&next)
                .expect("restart rebinds exact active generation")
                .authority_id,
            signer.authority_id
        );
        let wrong_seed = KagemushaMintFinalityLocalAuthorityV1::new_unseated(
            &genesis,
            candidate.clone(),
            Zeroizing::new([0xEE; 32]),
        )
        .expect("unseated seed is retained until authorization exists");
        assert!(wrong_seed.signer_for_authority(&next).is_err());
        let wrong_peer = KagemushaMintFinalityLocalAuthorityV1::new_unseated(
            &genesis,
            peer(8),
            Zeroizing::new(seed),
        )
        .expect("another unseated peer");
        assert!(wrong_peer.signer_for_authority(&next).is_err());
        let mut forged = next.clone();
        forged
            .validators
            .iter_mut()
            .find(|entry| entry.validator == candidate)
            .expect("candidate seat")
            .eq_proof_public_key[0] ^= 1;
        assert!(holder.signer_for_authority(&forged).is_err());
        let mut original_generation = next;
        original_generation.generation = 0;
        assert!(holder.signer_for_authority(&original_generation).is_err());
    }

    #[test]
    fn candidate_and_prepared_seat_possession_are_distinct_and_replay_bound() {
        use iroha_data_model::isi::kagemusha_v1::BeaconEpochBindingV1;
        let authority = runtime_generation_fixture(1);
        let keys = &authority.validators[1];
        let seed = [0xB1; 32];
        let candidate = prove_kagemusha_mint_finality_candidate_possession_v1(
            &seed,
            authority.network_id,
            1,
            keys,
        )
        .unwrap();
        verify_kagemusha_mint_finality_candidate_possession_v1(
            authority.network_id,
            1,
            keys,
            &candidate,
        )
        .unwrap();
        assert!(
            prove_kagemusha_mint_finality_candidate_possession_v1(
                &[0xCC; 32],
                authority.network_id,
                1,
                keys
            )
            .is_err()
        );
        assert!(
            verify_kagemusha_mint_finality_candidate_possession_v1(
                authority.network_id,
                2,
                keys,
                &candidate
            )
            .is_err()
        );
        let context = KagemushaMintFinalitySeatReadinessContextV1 {
            version: KAGEMUSHA_CHAIN_VERSION_V1,
            network_id: authority.network_id,
            transition_id: [0x21; 32],
            target_epoch: 2,
            authority_generation: 1,
            authority_id: authority.authority_id().unwrap(),
            first_height: 11,
            last_height: 20,
            validator_index: 1,
            beacon: BeaconEpochBindingV1::Installed(
                iroha_data_model::isi::kagemusha_v1::InstalledBeaconEpochBindingV1 {
                    session_id: [0x22; 32],
                    transcript_hash: [0x23; 32],
                },
            ),
        };
        let readiness =
            prove_kagemusha_mint_finality_seat_readiness_v1(&seed, &authority, &context).unwrap();
        let owner = KagemushaMintFinalityLocalAuthorityV1::new(
            Arc::new(runtime_generation_fixture(0)),
            Zeroizing::new(seed),
            1,
        )
        .unwrap();
        let (owned_keys, owned_candidate) = owner.candidate_possession(1).unwrap();
        assert_eq!(&owned_keys, keys);
        assert_eq!(owned_candidate, candidate);
        assert_eq!(
            owner.prove_seat_readiness(&authority, &context).unwrap(),
            readiness
        );
        let mut other_seat = context;
        other_seat.validator_index = 2;
        assert!(owner.prove_seat_readiness(&authority, &other_seat).is_err());
        verify_kagemusha_mint_finality_seat_readiness_v1(&authority, &context, &readiness).unwrap();
        assert!(
            verify_kagemusha_mint_finality_seat_readiness_v1(&authority, &context, &candidate)
                .is_err()
        );
        assert!(
            verify_kagemusha_mint_finality_candidate_possession_v1(
                authority.network_id,
                1,
                keys,
                &readiness
            )
            .is_err()
        );
        for coordinate in 0..9 {
            let mut changed = context;
            match coordinate {
                0 => changed.transition_id[0] ^= 1,
                1 => changed.target_epoch += 1,
                2 => changed.first_height += 1,
                3 => changed.last_height += 1,
                4 => changed.authority_generation += 1,
                5 => changed.authority_id[0] ^= 1,
                6 => changed.validator_index = 2,
                7 => {
                    changed.beacon = BeaconEpochBindingV1::Installed(
                        iroha_data_model::isi::kagemusha_v1::InstalledBeaconEpochBindingV1 {
                            session_id: [0x22; 32],
                            transcript_hash: [0x24; 32],
                        },
                    )
                }
                _ => {
                    changed.network_id =
                        NetworkId::from_genesis_hash(HashOf::<BlockHeader>::from_untyped_unchecked(
                            Hash::new(b"foreign readiness network"),
                        ))
                }
            }
            assert!(
                verify_kagemusha_mint_finality_seat_readiness_v1(&authority, &changed, &readiness)
                    .is_err(),
                "coordinate {coordinate}"
            );
        }
        let mut malformed = readiness;
        malformed.ep_proof_signature.response = [0; 32];
        assert!(
            verify_kagemusha_mint_finality_seat_readiness_v1(&authority, &context, &malformed)
                .is_err()
        );
        let mut uninstalled = context;
        uninstalled.beacon = BeaconEpochBindingV1::Bootstrap;
        assert!(
            prove_kagemusha_mint_finality_seat_readiness_v1(&seed, &authority, &uninstalled)
                .is_err()
        );
    }

    #[test]
    fn retained_generation_signs_new_epoch_with_distinct_nonce_and_rejects_replay() {
        use iroha_data_model::isi::kagemusha_v1::KagemushaMintFinalityEpochDecisionV1;
        let authority = runtime_generation_fixture(0);
        let signer =
            KagemushaMintFinalitySignerV1::from_seed(Zeroizing::new([0xB1; 32]), 1, &authority)
                .unwrap();
        let initial =
            crate::kagemusha_v1_test_fixtures::mint_finality_genesis_for_authority(&authority, 10);
        let retained = crate::kagemusha_v1_test_fixtures::mint_finality_successor_authorization(
            &initial,
            &authority,
            20,
            crate::kagemusha_v1_test_fixtures::fixture_installed_beacon(),
            KagemushaMintFinalityEpochDecisionV1::Retain,
            [0; 32],
        );
        let message = KagemushaMintFinalitySealMessageV1 {
            version: KAGEMUSHA_CHAIN_VERSION_V1,
            epoch_authorization: initial,
            validator_count: 4,
            network_id: authority.network_id,
            block_height: 2,
            native_instance: [0x30; 32],
            native_epoch_context: [0x31; 32],
            native_block_hash: [0x32; 32],
            native_result: [0x33; 32],
            kagemusha_top_up_root: Hash::new(b"generation retained top-up root"),
            kagemusha_top_up_count: 1,
            next_epoch_authorization: None,
        };
        let initial_seal = signer.sign(&message).unwrap();
        let changes: [fn(&mut KagemushaMintFinalitySealMessageV1); 4] = [
            |value| value.native_instance[0] ^= 1,
            |value| value.native_epoch_context[31] ^= 1,
            |value| value.native_block_hash[0] ^= 1,
            |value| value.native_result[0] ^= 1,
        ];
        for change in changes {
            let mut changed = message;
            change(&mut changed);
            assert!(
                verify_kagemusha_mint_finality_validator_seal_v1(
                    &authority,
                    &changed,
                    &initial_seal,
                )
                .is_err(),
                "original paired signatures must reject every changed native coordinate"
            );
            let resigned = signer.sign(&changed).unwrap();
            verify_kagemusha_mint_finality_validator_seal_v1(&authority, &changed, &resigned)
                .unwrap();
        }

        let retained_message = KagemushaMintFinalitySealMessageV1 {
            epoch_authorization: retained,
            block_height: 11,
            ..message
        };
        let retained_seal = signer.sign(&retained_message).unwrap();
        assert_ne!(
            initial_seal.eq_proof_signature.nonce_commitment,
            retained_seal.eq_proof_signature.nonce_commitment
        );
        assert_ne!(
            initial_seal.ep_proof_signature.nonce_commitment,
            retained_seal.ep_proof_signature.nonce_commitment
        );
        verify_kagemusha_mint_finality_validator_seal_v1(
            &authority,
            &retained_message,
            &retained_seal,
        )
        .unwrap();
        assert!(
            verify_kagemusha_mint_finality_validator_seal_v1(
                &authority,
                &retained_message,
                &initial_seal
            )
            .is_err()
        );
        let mut wrong_generation = retained_message;
        wrong_generation.epoch_authorization.authority_generation = 1;
        assert!(signer.sign(&wrong_generation).is_err());
    }

    #[test]
    fn runtime_epoch_rebinding_rejects_network_keys_epoch_and_missing_validator() {
        let epoch_zero = runtime_generation_fixture(0);
        let authority = KagemushaMintFinalityLocalAuthorityV1::new(
            Arc::new(epoch_zero.clone()),
            Zeroizing::new([0xB1; 32]),
            1,
        )
        .expect("bind epoch zero");
        let mut foreign = runtime_generation_fixture(1);
        foreign.network_id = NetworkId::from_genesis_hash(
            HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(b"foreign runtime generation")),
        );
        assert!(authority.signer_for_authority(&foreign).is_err());
        let mut wrong_epoch = epoch_zero.clone();
        wrong_epoch.generation = 1;
        assert!(authority.signer_for_authority(&wrong_epoch).is_err());
        let mut wrong_key = runtime_generation_fixture(1);
        wrong_key.validators[1] = derive_kagemusha_mint_finality_validator_keys_v1(
            &[0xDD; 32],
            1,
            wrong_key.validators[1].validator.clone(),
        )
        .expect("wrong seed keys");
        assert!(authority.signer_for_authority(&wrong_key).is_err());
        let mut absent = runtime_generation_fixture(1);
        absent.validators[1] =
            derive_kagemusha_mint_finality_validator_keys_v1(&[0xDD; 32], 1, peer(99))
                .expect("replacement validator keys");
        absent
            .validators
            .sort_by(|left, right| left.validator.cmp(&right.validator));
        absent
            .validate()
            .expect("valid roster without local validator");
        assert!(authority.signer_for_authority(&absent).is_err());
    }

    #[test]
    fn mint_merkle_domain_tags_match_the_circuit_contract() {
        assert_eq!(MINT_LEAF_DOMAIN_V1.to_le_bytes(), *b"kgmmntl1");
        assert_eq!(MINT_NODE_DOMAIN_V1.to_le_bytes(), *b"kgmmntn1");
    }

    #[test]
    fn roster_key_validation_rejects_a_noncanonical_curve_point() {
        let mut validators = (1_u8..=4).map(peer).collect::<Vec<_>>();
        validators.sort();
        let keys = validators
            .into_iter()
            .enumerate()
            .map(|(index, validator)| {
                derive_kagemusha_mint_finality_validator_keys_v1(
                    &[0xB0_u8.wrapping_add(u8::try_from(index).expect("small roster")); 32],
                    0,
                    validator,
                )
                .expect("derive canonical fixture keys")
            })
            .collect::<Vec<_>>();
        let network_id = NetworkId::from_genesis_hash(
            HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(b"roster key validation")),
        );
        let mut roster = KagemushaMintFinalityAuthorityGenerationV1 {
            version: KAGEMUSHA_CHAIN_VERSION_V1,
            network_id,
            generation: 0,
            validators: keys,
        };
        validate_kagemusha_mint_finality_roster_keys_v1(&roster)
            .expect("derived keys are canonical points");
        roster.validators[0].eq_proof_public_key = [0xFF; 32];
        assert!(validate_kagemusha_mint_finality_roster_keys_v1(&roster).is_err());
    }

    #[test]
    fn genesis_parameter_key_validation_rejects_a_noncanonical_curve_point() {
        use iroha_data_model::isi::kagemusha_v1::{
            KagemushaMintFinalityAuthorityGenerationTemplateV1,
            KagemushaMintFinalityGenesisParametersV1,
        };

        let mut validators = (1_u8..=4).map(peer).collect::<Vec<_>>();
        validators.sort();
        let derive_keys = |generation, seed_base: u8| {
            validators
                .iter()
                .cloned()
                .enumerate()
                .map(|(index, validator)| {
                    derive_kagemusha_mint_finality_validator_keys_v1(
                        &[seed_base.wrapping_add(u8::try_from(index).expect("small roster")); 32],
                        generation,
                        validator,
                    )
                    .expect("derive canonical fixture keys")
                })
                .collect::<Vec<_>>()
        };
        let epoch_zero_keys = derive_keys(0, 0xC0);
        let mut parameters = KagemushaMintFinalityGenesisParametersV1 {
            authority_generation: KagemushaMintFinalityAuthorityGenerationTemplateV1 {
                version: KAGEMUSHA_CHAIN_VERSION_V1,
                generation: 0,
                validators: epoch_zero_keys.clone(),
            },
        };
        validate_kagemusha_mint_finality_genesis_parameter_keys_v1(&parameters)
            .expect("derived genesis keys are canonical points");
        parameters.authority_generation.validators[0].eq_proof_public_key = [0xFF; 32];
        let error = validate_kagemusha_mint_finality_genesis_parameter_keys_v1(&parameters)
            .expect_err("non-canonical Pallas point must fail closed");
        assert!(error.to_string().contains("Pallas point"));

        parameters.authority_generation.validators = epoch_zero_keys;
        parameters.authority_generation.validators[0].ep_proof_public_key = [0xFF; 32];
        let error = validate_kagemusha_mint_finality_genesis_parameter_keys_v1(&parameters)
            .expect_err("non-canonical Vesta point must fail closed");
        assert!(error.to_string().contains("Vesta point"));
    }
}
