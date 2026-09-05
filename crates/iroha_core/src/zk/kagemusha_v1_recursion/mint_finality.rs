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
    block::consensus_v2::{
        ExecutionCommitment, GlobalPhase, HeightContext, QuorumCertificate, Vote,
    },
    isi::kagemusha_v1::{
        KAGEMUSHA_CHAIN_VERSION_V1, KAGEMUSHA_MINT_FINALITY_TREE_DEPTH_V1,
        KagemushaMintFinalityEpochRosterV1, KagemushaMintFinalityGenesisParametersV1,
        KagemushaMintFinalitySealBundleV1, KagemushaMintFinalitySealMessageV1,
        KagemushaMintFinalitySealShareV1, KagemushaMintFinalityValidatorKeysV1,
        KagemushaMintFinalityValidatorSealV1, KagemushaOperationKindV1,
        KagemushaPastaSchnorrSignatureV1, KagemushaReserveReceiptV1, KagemushaTopUpLeafV1,
        KagemushaTopUpMembershipWitnessV1, kagemusha_mint_finality_root_v1,
    },
    kagemusha::KagemushaPastaStateCommitmentV1,
};
use norito::codec::{DecodeAll as _, Encode};
use sha2::{Digest as _, Sha256, Sha512};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};
use thiserror::Error;
use zeroize::Zeroizing;

use crate::zk::kagemusha_v1_poseidon::{
    KagemushaPoseidonFieldV1, decode, digest_limbs, encode, from_u128, hash,
};

pub(super) const MINT_LEAF_DOMAIN_V1: u64 = u64::from_le_bytes(*b"kgmmntl1");
const MINT_EMPTY_DOMAIN_V1: u64 = u64::from_le_bytes(*b"kgminte1");
pub(super) const MINT_NODE_DOMAIN_V1: u64 = u64::from_le_bytes(*b"kgmmntn1");
const SUBJECT_DIGEST_DOMAIN_V1: &[u8] = b"iroha:kagemusha:v1:mint-finality:subject";
const EXECUTION_DIGEST_DOMAIN_V1: &[u8] = b"iroha:kagemusha:v1:mint-finality:execution-commitment";
const KEY_DERIVATION_DOMAIN_V1: &[u8] = b"iroha:kagemusha:v1:mint-finality:key";
const NONCE_DERIVATION_DOMAIN_V1: &[u8] = b"iroha:kagemusha:v1:mint-finality:nonce";
const CHALLENGE_DOMAIN_V1: &[u8] = b"iroha:kagemusha:v1:mint-finality:challenge";
const EQ_PARITY_TAG: u8 = 0;
const EP_PARITY_TAG: u8 = 1;

/// Native mint-finality construction or verification failure.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum KagemushaMintFinalityErrorV1 {
    /// The separately provisioned epoch roster is malformed or mismatches consensus.
    #[error("invalid Kagemusha mint-finality epoch roster: {0}")]
    InvalidEpochRoster(String),
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
    /// Canonical auxiliary payload decoding failed.
    #[error("invalid Kagemusha mint-finality auxiliary payload: {0}")]
    InvalidAuxiliaryPayload(String),
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
/// Boundary Commit votes use this value when no top-up occurs so the old epoch can still
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

/// Strictly validate separately provisioned Pasta keys against a frozen context.
///
/// # Errors
///
/// Returns an error unless network, epoch, count, identity order, and every
/// canonical non-identity curve point match.
pub fn validate_kagemusha_mint_finality_epoch_v1(
    epoch: &KagemushaMintFinalityEpochRosterV1,
    context: &HeightContext,
) -> Result<(), KagemushaMintFinalityErrorV1> {
    epoch
        .validate()
        .map_err(|error| KagemushaMintFinalityErrorV1::InvalidEpochRoster(error.to_string()))?;
    context
        .validate()
        .map_err(|error| KagemushaMintFinalityErrorV1::InvalidEpochRoster(error.to_string()))?;
    let finality_epoch_id = epoch
        .finality_epoch_id()
        .map_err(|error| KagemushaMintFinalityErrorV1::InvalidEpochRoster(error.to_string()))?;
    if finality_epoch_id != context.kagemusha_mint_finality_epoch_id
        || epoch.network_id != context.network_id
        || epoch.epoch != context.epoch
        || epoch.validators.len() != context.roster.len()
        || epoch
            .validators
            .iter()
            .zip(&context.roster)
            .any(|(keys, validator)| keys.validator != validator.validator)
    {
        return Err(KagemushaMintFinalityErrorV1::InvalidEpochRoster(
            "epoch authority or its committed identifier does not exactly match the frozen context"
                .to_owned(),
        ));
    }
    validate_kagemusha_mint_finality_roster_keys_v1(epoch)
}

/// Decode every paired-Pasta public key in a structurally valid epoch roster.
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
    epoch: &KagemushaMintFinalityEpochRosterV1,
) -> Result<(), KagemushaMintFinalityErrorV1> {
    epoch
        .validate()
        .map_err(|error| KagemushaMintFinalityErrorV1::InvalidEpochRoster(error.to_string()))?;
    validate_kagemusha_mint_finality_validator_keys_v1(&epoch.validators)
}

/// Decode every paired-Pasta public key in network-independent genesis authority parameters.
///
/// Operator tooling calls this before a genesis network identity exists, so malformed compressed
/// points fail closed before a manifest can be generated or signed.
///
/// # Errors
///
/// Returns an error unless both roster templates are structurally valid and every Pallas and
/// Vesta encoding is canonical and non-identity.
pub fn validate_kagemusha_mint_finality_genesis_parameter_keys_v1(
    parameters: &KagemushaMintFinalityGenesisParametersV1,
) -> Result<(), KagemushaMintFinalityErrorV1> {
    parameters
        .validate()
        .map_err(|error| KagemushaMintFinalityErrorV1::InvalidEpochRoster(error.to_string()))?;
    validate_kagemusha_mint_finality_validator_keys_v1(&parameters.epoch_roster.validators)?;
    if let Some(next_epoch_roster) = &parameters.next_epoch_roster {
        validate_kagemusha_mint_finality_validator_keys_v1(&next_epoch_roster.validators)?;
    }
    Ok(())
}

fn validate_kagemusha_mint_finality_validator_keys_v1(
    validators: &[KagemushaMintFinalityValidatorKeysV1],
) -> Result<(), KagemushaMintFinalityErrorV1> {
    for keys in validators {
        decode_nonidentity_point::<EpAffine>(keys.eq_proof_public_key).ok_or_else(|| {
            KagemushaMintFinalityErrorV1::InvalidEpochRoster(
                "Eq/Fp helper key is not a canonical non-identity Pallas point".to_owned(),
            )
        })?;
        decode_nonidentity_point::<EqAffine>(keys.ep_proof_public_key).ok_or_else(|| {
            KagemushaMintFinalityErrorV1::InvalidEpochRoster(
                "Ep/Fq helper key is not a canonical non-identity Vesta point".to_owned(),
            )
        })?;
    }
    Ok(())
}

/// Derive one validator's epoch-scoped public keys from separately provisioned seed material.
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
    epoch: u64,
    validator: iroha_data_model::peer::PeerId,
) -> Result<KagemushaMintFinalityValidatorKeysV1, KagemushaMintFinalityErrorV1> {
    let validator_bytes = validator.encode();
    let eq_secret = derive_nonzero_key_scalar::<Fq>(EQ_PARITY_TAG, seed, epoch, &validator_bytes)?;
    let ep_secret = derive_nonzero_key_scalar::<Fp>(EP_PARITY_TAG, seed, epoch, &validator_bytes)?;
    Ok(KagemushaMintFinalityValidatorKeysV1 {
        validator,
        eq_proof_public_key: encode_point::<EpAffine>(
            (<EpAffine as CurveAffine>::CurveExt::generator() * eq_secret).to_affine(),
        ),
        ep_proof_public_key: encode_point::<EqAffine>(
            (<EqAffine as CurveAffine>::CurveExt::generator() * ep_secret).to_affine(),
        ),
    })
}

/// Validator-local holder for separately provisioned mint-finality seed material.
pub struct KagemushaMintFinalitySignerV1 {
    seed: Zeroizing<[u8; 32]>,
    validator_index: u32,
    network_id: iroha_data_model::NetworkId,
    epoch: u64,
    finality_epoch_id: [u8; 32],
    validator: iroha_data_model::peer::PeerId,
}

/// Runtime authority for one node's exact epoch roster and local signing seed.
///
/// Sumeragi may share this object through `Arc`; no key material is cloned or
/// exposed by the recursive relation.
pub struct KagemushaMintFinalityLocalAuthorityV1 {
    epoch: Arc<KagemushaMintFinalityEpochRosterV1>,
    signer: KagemushaMintFinalitySignerV1,
}

impl core::fmt::Debug for KagemushaMintFinalityLocalAuthorityV1 {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("KagemushaMintFinalityLocalAuthorityV1")
            .field("epoch", &self.epoch)
            .field("signer", &self.signer)
            .finish()
    }
}

impl KagemushaMintFinalityLocalAuthorityV1 {
    /// Bind separately provisioned seed material to one authenticated epoch roster.
    ///
    /// # Errors
    ///
    /// Returns an error unless the seed-derived keys exactly equal the local
    /// validator's roster entry.
    pub fn new(
        epoch: Arc<KagemushaMintFinalityEpochRosterV1>,
        seed: Zeroizing<[u8; 32]>,
        validator_index: u32,
    ) -> Result<Self, KagemushaMintFinalityErrorV1> {
        let signer =
            KagemushaMintFinalitySignerV1::from_seed(seed, validator_index, epoch.as_ref())?;
        Ok(Self { epoch, signer })
    }

    /// Borrow the mandatory epoch authority.
    #[must_use]
    pub fn epoch(&self) -> &KagemushaMintFinalityEpochRosterV1 {
        self.epoch.as_ref()
    }

    /// Borrow the non-exportable local signer.
    #[must_use]
    pub const fn signer(&self) -> &KagemushaMintFinalitySignerV1 {
        &self.signer
    }
}

impl core::fmt::Debug for KagemushaMintFinalitySignerV1 {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("KagemushaMintFinalitySignerV1")
            .field("validator_index", &self.validator_index)
            .field("network_id", &self.network_id)
            .field("epoch", &self.epoch)
            .field("finality_epoch_id", &self.finality_epoch_id)
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
        epoch: &KagemushaMintFinalityEpochRosterV1,
    ) -> Result<Self, KagemushaMintFinalityErrorV1> {
        epoch
            .validate()
            .map_err(|error| KagemushaMintFinalityErrorV1::InvalidSigner(error.to_string()))?;
        let index = usize::try_from(validator_index).map_err(|_| {
            KagemushaMintFinalityErrorV1::InvalidSigner(
                "validator index does not fit usize".to_owned(),
            )
        })?;
        let expected = epoch.validators.get(index).ok_or_else(|| {
            KagemushaMintFinalityErrorV1::InvalidSigner(
                "validator index is outside the epoch roster".to_owned(),
            )
        })?;
        let derived = derive_kagemusha_mint_finality_validator_keys_v1(
            &seed,
            epoch.epoch,
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
            network_id: epoch.network_id,
            epoch: epoch.epoch,
            finality_epoch_id: epoch
                .finality_epoch_id()
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
    /// Returns an error when the statement names another epoch/network/roster
    /// or deterministic nonce derivation fails.
    pub fn sign(
        &self,
        message: &KagemushaMintFinalitySealMessageV1,
    ) -> Result<KagemushaMintFinalityValidatorSealV1, KagemushaMintFinalityErrorV1> {
        message
            .validate()
            .map_err(|error| KagemushaMintFinalityErrorV1::InvalidStatement(error.to_string()))?;
        if message.network_id != self.network_id
            || message.finality_epoch_id != self.finality_epoch_id
            || self.validator_index >= message.validator_count
        {
            return Err(KagemushaMintFinalityErrorV1::InvalidSigner(
                "message does not belong to the signer's admitted epoch".to_owned(),
            ));
        }
        let signing_digest = message
            .signing_digest()
            .map_err(|error| KagemushaMintFinalityErrorV1::InvalidStatement(error.to_string()))?;
        let validator_bytes = self.validator.encode();
        let eq_secret = derive_nonzero_key_scalar::<Fq>(
            EQ_PARITY_TAG,
            &self.seed,
            self.epoch,
            &validator_bytes,
        )?;
        let ep_secret = derive_nonzero_key_scalar::<Fp>(
            EP_PARITY_TAG,
            &self.seed,
            self.epoch,
            &validator_bytes,
        )?;
        Ok(KagemushaMintFinalityValidatorSealV1 {
            validator_index: self.validator_index,
            eq_proof_signature: schnorr_sign::<EpAffine>(
                &self.seed,
                &self.network_id,
                self.epoch,
                &validator_bytes,
                self.validator_index,
                EQ_PARITY_TAG,
                eq_secret,
                signing_digest,
            )?,
            ep_proof_signature: schnorr_sign::<EqAffine>(
                &self.seed,
                &self.network_id,
                self.epoch,
                &validator_bytes,
                self.validator_index,
                EP_PARITY_TAG,
                ep_secret,
                signing_digest,
            )?,
        })
    }
}

/// Build the sole block-level mint-finality statement for an unsigned Commit vote.
///
/// Returns `Ok(None)` only for non-boundary blocks without top-ups. A boundary Commit vote always
/// returns a statement so the old epoch authorizes the complete next Pasta roster even when no
/// mint occurs. A top-up commitment on any non-Commit vote is rejected rather than silently left
/// unsealed.
///
/// # Errors
///
/// Returns an error unless the epoch authority and unsigned vote match the
/// frozen context exactly.
pub fn build_kagemusha_mint_finality_seal_message_v1(
    epoch: &KagemushaMintFinalityEpochRosterV1,
    context: &HeightContext,
    vote: &Vote,
) -> Result<Option<KagemushaMintFinalitySealMessageV1>, KagemushaMintFinalityErrorV1> {
    validate_kagemusha_mint_finality_epoch_v1(epoch, context)?;
    validate_unsigned_vote(context, vote)?;
    let next_finality_epoch_id = context
        .next_epoch_snapshot
        .as_ref()
        .map(|snapshot| snapshot.kagemusha_mint_finality_epoch_id);
    match (
        vote.execution_commitment.kagemusha_top_up_count,
        vote.execution_commitment.kagemusha_top_up_root,
    ) {
        (0, None) if next_finality_epoch_id.is_none() => Ok(None),
        (0, None) => {
            if vote.phase != GlobalPhase::Commit {
                return Ok(None);
            }
            let root = kagemusha_mint_finality_root_v1(kagemusha_mint_finality_empty_root_v1()?);
            seal_message_from_parts(
                epoch,
                context,
                vote.subject,
                vote.execution_commitment,
                root,
                0,
                next_finality_epoch_id,
            )
            .map(Some)
        }
        (0, Some(_)) | (_, None) => Err(KagemushaMintFinalityErrorV1::InvalidStatement(
            "top-up root/count pair is inconsistent".to_owned(),
        )),
        (count, Some(root)) => {
            if vote.phase != GlobalPhase::Commit {
                return Err(KagemushaMintFinalityErrorV1::InvalidStatement(
                    "only Commit votes may carry a top-up root".to_owned(),
                ));
            }
            let message = seal_message_from_parts(
                epoch,
                context,
                vote.subject,
                vote.execution_commitment,
                root,
                count,
                next_finality_epoch_id,
            )?;
            Ok(Some(message))
        }
    }
}

/// Sign one block-level statement with a previously admitted validator signer.
///
/// # Errors
///
/// Delegates all validation and nonce errors to [`KagemushaMintFinalitySignerV1::sign`].
pub fn sign_kagemusha_mint_finality_seal_v1(
    signer: &KagemushaMintFinalitySignerV1,
    message: &KagemushaMintFinalitySealMessageV1,
) -> Result<KagemushaMintFinalityValidatorSealV1, KagemushaMintFinalityErrorV1> {
    signer.sign(message)
}

/// Verify one canonical share against its enclosing unsigned Commit vote.
///
/// # Errors
///
/// Returns an error for any context/message/signer/key/signature substitution.
pub fn verify_kagemusha_mint_finality_seal_share_v1(
    epoch: &KagemushaMintFinalityEpochRosterV1,
    context: &HeightContext,
    vote: &Vote,
    share: &KagemushaMintFinalitySealShareV1,
) -> Result<(), KagemushaMintFinalityErrorV1> {
    share
        .validate()
        .map_err(|error| KagemushaMintFinalityErrorV1::InvalidStatement(error.to_string()))?;
    let expected = build_kagemusha_mint_finality_seal_message_v1(epoch, context, vote)?
        .ok_or_else(|| {
            KagemushaMintFinalityErrorV1::InvalidStatement(
                "a mint-finality share was attached to a block without top-ups".to_owned(),
            )
        })?;
    if share.message != expected || share.seal.validator_index != vote.signer {
        return Err(KagemushaMintFinalityErrorV1::InvalidStatement(
            "share message or signer differs from the enclosing vote".to_owned(),
        ));
    }
    verify_validator_seal(epoch, &share.message, &share.seal)
}

/// Verify one exact-quorum seal bundle against its enclosing CommitQC.
///
/// # Errors
///
/// Returns an error for any context/QC/message/quorum/key/signature substitution.
pub fn verify_kagemusha_mint_finality_seal_bundle_v1(
    epoch: &KagemushaMintFinalityEpochRosterV1,
    context: &HeightContext,
    certificate: &QuorumCertificate,
    bundle: &KagemushaMintFinalitySealBundleV1,
) -> Result<(), KagemushaMintFinalityErrorV1> {
    validate_kagemusha_mint_finality_epoch_v1(epoch, context)?;
    validate_commit_qc_shape(context, certificate)?;
    bundle
        .validate()
        .map_err(|error| KagemushaMintFinalityErrorV1::InvalidStatement(error.to_string()))?;
    let next_finality_epoch_id = context
        .next_epoch_snapshot
        .as_ref()
        .map(|snapshot| snapshot.kagemusha_mint_finality_epoch_id);
    let root = match (
        certificate.execution_commitment.kagemusha_top_up_count,
        certificate.execution_commitment.kagemusha_top_up_root,
    ) {
        (0, None) if next_finality_epoch_id.is_some() => {
            kagemusha_mint_finality_root_v1(kagemusha_mint_finality_empty_root_v1()?)
        }
        (count, Some(root)) if count > 0 => root,
        _ => {
            return Err(KagemushaMintFinalityErrorV1::InvalidStatement(
                "mint-finality bundle requires a top-up or epoch-boundary commitment".to_owned(),
            ));
        }
    };
    let expected = seal_message_from_parts(
        epoch,
        context,
        certificate.subject,
        certificate.execution_commitment,
        root,
        certificate.execution_commitment.kagemusha_top_up_count,
        next_finality_epoch_id,
    )?;
    if bundle.message != expected
        || bundle
            .seals
            .iter()
            .map(|seal| seal.validator_index)
            .ne(certificate.signers.iter().copied())
    {
        return Err(KagemushaMintFinalityErrorV1::InvalidStatement(
            "bundle statement/signers differ from the enclosing CommitQC".to_owned(),
        ));
    }
    for seal in &bundle.seals {
        verify_validator_seal(epoch, &bundle.message, seal)?;
    }
    Ok(())
}

/// Decode one canonical Commit-vote auxiliary share.
///
/// # Errors
///
/// Returns an error for trailing bytes, non-canonical encoding, or an invalid share.
pub fn decode_kagemusha_mint_finality_seal_share_v1(
    bytes: &[u8],
) -> Result<KagemushaMintFinalitySealShareV1, KagemushaMintFinalityErrorV1> {
    let mut cursor = bytes;
    let decoded = KagemushaMintFinalitySealShareV1::decode_all(&mut cursor).map_err(|error| {
        KagemushaMintFinalityErrorV1::InvalidAuxiliaryPayload(error.to_string())
    })?;
    if decoded.encode() != bytes {
        return Err(KagemushaMintFinalityErrorV1::InvalidAuxiliaryPayload(
            "share encoding is not canonical".to_owned(),
        ));
    }
    decoded.validate().map_err(|error| {
        KagemushaMintFinalityErrorV1::InvalidAuxiliaryPayload(error.to_string())
    })?;
    Ok(decoded)
}

/// Decode one canonical CommitQC auxiliary bundle.
///
/// # Errors
///
/// Returns an error for trailing bytes, non-canonical encoding, or an invalid bundle.
pub fn decode_kagemusha_mint_finality_seal_bundle_v1(
    bytes: &[u8],
) -> Result<KagemushaMintFinalitySealBundleV1, KagemushaMintFinalityErrorV1> {
    let mut cursor = bytes;
    let decoded = KagemushaMintFinalitySealBundleV1::decode_all(&mut cursor).map_err(|error| {
        KagemushaMintFinalityErrorV1::InvalidAuxiliaryPayload(error.to_string())
    })?;
    if decoded.encode() != bytes {
        return Err(KagemushaMintFinalityErrorV1::InvalidAuxiliaryPayload(
            "bundle encoding is not canonical".to_owned(),
        ));
    }
    decoded.validate().map_err(|error| {
        KagemushaMintFinalityErrorV1::InvalidAuxiliaryPayload(error.to_string())
    })?;
    Ok(decoded)
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

fn validate_unsigned_vote(
    context: &HeightContext,
    vote: &Vote,
) -> Result<(), KagemushaMintFinalityErrorV1> {
    context
        .validate()
        .map_err(|error| KagemushaMintFinalityErrorV1::InvalidStatement(error.to_string()))?;
    vote.execution_commitment
        .validate()
        .map_err(|error| KagemushaMintFinalityErrorV1::InvalidStatement(error.to_string()))?;
    if vote.round.context_id != context.id()
        || vote.round.height != context.height
        || vote.proposal_round != vote.round
        || usize::try_from(vote.signer)
            .ok()
            .is_none_or(|index| index >= context.roster.len())
    {
        return Err(KagemushaMintFinalityErrorV1::InvalidStatement(
            "unsigned vote does not match the frozen context".to_owned(),
        ));
    }
    Ok(())
}

fn validate_commit_qc_shape(
    context: &HeightContext,
    certificate: &QuorumCertificate,
) -> Result<(), KagemushaMintFinalityErrorV1> {
    context
        .validate()
        .map_err(|error| KagemushaMintFinalityErrorV1::InvalidStatement(error.to_string()))?;
    certificate
        .execution_commitment
        .validate()
        .map_err(|error| KagemushaMintFinalityErrorV1::InvalidStatement(error.to_string()))?;
    if certificate.phase != GlobalPhase::Commit
        || certificate.round.context_id != context.id()
        || certificate.round.height != context.height
        || certificate.proposal_round != certificate.round
        || certificate.signers.len()
            != usize::try_from(context.quorum.min_signers).expect("u32 fits usize")
        || certificate
            .signers
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
        || certificate.signers.iter().any(|signer| {
            usize::try_from(*signer)
                .ok()
                .is_none_or(|index| index >= context.roster.len())
        })
    {
        return Err(KagemushaMintFinalityErrorV1::InvalidStatement(
            "CommitQC does not match the frozen equal-vote context".to_owned(),
        ));
    }
    Ok(())
}

fn seal_message_from_parts(
    epoch: &KagemushaMintFinalityEpochRosterV1,
    context: &HeightContext,
    subject: iroha_data_model::block::consensus_v2::BlockSubject,
    execution_commitment: ExecutionCommitment,
    root: iroha_crypto::Hash,
    count: u32,
    next_finality_epoch_id: Option<[u8; 32]>,
) -> Result<KagemushaMintFinalitySealMessageV1, KagemushaMintFinalityErrorV1> {
    let message = KagemushaMintFinalitySealMessageV1 {
        version: KAGEMUSHA_CHAIN_VERSION_V1,
        finality_epoch_id: epoch
            .finality_epoch_id()
            .map_err(|error| KagemushaMintFinalityErrorV1::InvalidEpochRoster(error.to_string()))?,
        validator_count: u32::try_from(epoch.validators.len()).map_err(|_| {
            KagemushaMintFinalityErrorV1::InvalidEpochRoster(
                "validator count does not fit u32".to_owned(),
            )
        })?,
        network_id: context.network_id,
        block_height: context.height,
        height_context_id: context.id(),
        subject_digest: canonical_sha256(SUBJECT_DIGEST_DOMAIN_V1, &subject.encode()),
        execution_commitment_digest: canonical_sha256(
            EXECUTION_DIGEST_DOMAIN_V1,
            &execution_commitment.encode(),
        ),
        kagemusha_top_up_root: root,
        kagemusha_top_up_count: count,
        next_finality_epoch_id,
    };
    message
        .validate()
        .map_err(|error| KagemushaMintFinalityErrorV1::InvalidStatement(error.to_string()))?;
    Ok(message)
}

fn verify_validator_seal(
    epoch: &KagemushaMintFinalityEpochRosterV1,
    message: &KagemushaMintFinalitySealMessageV1,
    seal: &KagemushaMintFinalityValidatorSealV1,
) -> Result<(), KagemushaMintFinalityErrorV1> {
    let index = usize::try_from(seal.validator_index).map_err(|_| {
        KagemushaMintFinalityErrorV1::InvalidSignature(
            "validator index does not fit usize".to_owned(),
        )
    })?;
    let keys = epoch.validators.get(index).ok_or_else(|| {
        KagemushaMintFinalityErrorV1::InvalidSignature(
            "validator index is outside the authenticated epoch".to_owned(),
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
    epoch: u64,
    validator_bytes: &[u8],
    validator_index: u32,
    parity: u8,
    secret: C::ScalarExt,
    signing_digest: [u8; 32],
) -> Result<KagemushaPastaSchnorrSignatureV1, KagemushaMintFinalityErrorV1>
where
    C: CurveAffine,
    C::ScalarExt: FromUniformBytes<64> + PrimeField,
{
    let public = (C::CurveExt::generator() * secret).to_affine();
    let public_bytes = encode_point(public);
    for counter in 0..u32::MAX {
        let nonce = derive_nonzero_nonce_scalar::<C::ScalarExt>(
            parity,
            seed,
            network_id,
            epoch,
            validator_bytes,
            &[&signing_digest[..], &counter.to_le_bytes()].concat(),
        )?;
        let nonce_point = (C::CurveExt::generator() * nonce).to_affine();
        let nonce_commitment = encode_point(nonce_point);
        let challenge = schnorr_challenge::<C::ScalarExt>(
            parity,
            validator_index,
            signing_digest,
            nonce_commitment,
            public_bytes,
        );
        let response = nonce + challenge * secret;
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

fn derive_nonzero_key_scalar<F>(
    parity: u8,
    seed: &[u8; 32],
    epoch: u64,
    validator_bytes: &[u8],
) -> Result<F, KagemushaMintFinalityErrorV1>
where
    F: Field + FromUniformBytes<64>,
{
    for counter in 0..u32::MAX {
        let mut hasher = Sha512::new();
        hasher.update(KEY_DERIVATION_DOMAIN_V1);
        hasher.update([0, parity]);
        hasher.update(seed);
        hasher.update(epoch.to_le_bytes());
        hasher.update(
            u32::try_from(validator_bytes.len())
                .expect("bounded PeerId encoding length fits u32")
                .to_le_bytes(),
        );
        hasher.update(validator_bytes);
        hasher.update(counter.to_le_bytes());
        let uniform: [u8; 64] = hasher.finalize().into();
        let scalar = F::from_uniform_bytes(&uniform);
        if !bool::from(scalar.is_zero()) {
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
    epoch: u64,
    validator_bytes: &[u8],
    extra: &[u8],
) -> Result<F, KagemushaMintFinalityErrorV1>
where
    F: Field + FromUniformBytes<64>,
{
    for counter in 0..u32::MAX {
        let mut hasher = Sha512::new();
        hasher.update(NONCE_DERIVATION_DOMAIN_V1);
        hasher.update([0, parity]);
        hasher.update(seed);
        hasher.update(network_id.as_bytes());
        hasher.update(epoch.to_le_bytes());
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
        let uniform: [u8; 64] = hasher.finalize().into();
        let scalar = F::from_uniform_bytes(&uniform);
        if !bool::from(scalar.is_zero()) {
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

fn canonical_sha256(domain: &[u8], bytes: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(domain);
    hasher.update([0]);
    hasher.update(bytes);
    hasher.finalize().into()
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
        isi::kagemusha_v1::{KAGEMUSHA_CHAIN_VERSION_V1, KagemushaMintFinalityEpochRosterV1},
        peer::PeerId,
    };

    fn peer(seed: u8) -> PeerId {
        let key_pair = KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519)
            .expect("derive deterministic mint-finality test peer");
        PeerId::new(key_pair.public_key().clone())
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
                .expect("derive in another epoch")
        );
        assert_ne!(
            baseline,
            derive_kagemusha_mint_finality_validator_keys_v1(&seed, 7, peer(2))
                .expect("derive for another validator")
        );
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
        let mut roster = KagemushaMintFinalityEpochRosterV1 {
            version: KAGEMUSHA_CHAIN_VERSION_V1,
            network_id,
            epoch: 0,
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
            KagemushaMintFinalityEpochRosterTemplateV1, KagemushaMintFinalityGenesisParametersV1,
        };

        let mut validators = (1_u8..=4).map(peer).collect::<Vec<_>>();
        validators.sort();
        let derive_keys = |epoch, seed_base: u8| {
            validators
                .iter()
                .cloned()
                .enumerate()
                .map(|(index, validator)| {
                    derive_kagemusha_mint_finality_validator_keys_v1(
                        &[seed_base.wrapping_add(u8::try_from(index).expect("small roster")); 32],
                        epoch,
                        validator,
                    )
                    .expect("derive canonical fixture keys")
                })
                .collect::<Vec<_>>()
        };
        let epoch_zero_keys = derive_keys(0, 0xC0);
        let epoch_one_keys = derive_keys(1, 0xD0);
        let mut parameters = KagemushaMintFinalityGenesisParametersV1 {
            epoch_roster: KagemushaMintFinalityEpochRosterTemplateV1 {
                version: KAGEMUSHA_CHAIN_VERSION_V1,
                epoch: 0,
                validators: epoch_zero_keys.clone(),
            },
            next_epoch_roster: Some(KagemushaMintFinalityEpochRosterTemplateV1 {
                version: KAGEMUSHA_CHAIN_VERSION_V1,
                epoch: 1,
                validators: epoch_one_keys,
            }),
        };
        validate_kagemusha_mint_finality_genesis_parameter_keys_v1(&parameters)
            .expect("derived genesis keys are canonical points");
        parameters.epoch_roster.validators[0].eq_proof_public_key = [0xFF; 32];
        let error = validate_kagemusha_mint_finality_genesis_parameter_keys_v1(&parameters)
            .expect_err("non-canonical Pallas point must fail closed");
        assert!(error.to_string().contains("Pallas point"));

        parameters.epoch_roster.validators = epoch_zero_keys;
        parameters
            .next_epoch_roster
            .as_mut()
            .expect("epoch-one fixture exists")
            .validators[0]
            .ep_proof_public_key = [0xFF; 32];
        let error = validate_kagemusha_mint_finality_genesis_parameter_keys_v1(&parameters)
            .expect_err("non-canonical Vesta point must fail closed");
        assert!(error.to_string().contains("Vesta point"));
    }
}
