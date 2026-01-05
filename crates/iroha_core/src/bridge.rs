//! Helpers for bridge finality proofs built from commit certificates.

use std::{
    collections::BTreeSet,
    num::NonZeroUsize,
    sync::{Mutex, OnceLock},
};

use iroha_crypto::HashOf;
use iroha_data_model::{
    ChainId,
    bridge::{
        BridgeAuthoritySet, BridgeCommitment, BridgeCommitmentJustification, BridgeFinalityBundle,
        BridgeFinalityProof,
    },
    consensus::VALIDATOR_SET_HASH_VERSION_V1,
    peer::PeerId,
};
use thiserror::Error;

use crate::{mmr::BlockMmr, state::StateReadOnly, sumeragi};

struct MmrCache {
    mmr: BlockMmr,
    height: u64,
}

/// Errors returned when constructing a bridge finality proof.
#[allow(variant_size_differences)]
#[derive(Debug, Error, Copy, Clone)]
pub enum BridgeFinalityError {
    /// The requested block height is zero or does not fit into the host pointer width.
    #[error("invalid block height {0}")]
    InvalidHeight(u64),
    /// The block at the requested height was not found.
    #[error("block at height {0} not found")]
    BlockNotFound(u64),
    /// No commit certificate was found for the requested height.
    #[error("commit certificate for height {0} not found")]
    CommitCertificateNotFound(u64),
    /// The commit certificate references a different block hash than the stored block.
    #[error(
        "commit certificate hash {cert_hash:?} does not match block hash {block_hash:?} at height {height}"
    )]
    CommitCertificateHashMismatch {
        /// Height being proven.
        height: u64,
        /// Hash recorded inside the commit certificate.
        cert_hash: iroha_crypto::HashOf<iroha_data_model::block::BlockHeader>,
        /// Hash of the stored block header.
        block_hash: iroha_crypto::HashOf<iroha_data_model::block::BlockHeader>,
    },
}

fn compute_block_mmr(
    state: &impl StateReadOnly,
    height: u64,
) -> Result<BlockMmr, BridgeFinalityError> {
    static BLOCK_MMR_CACHE: OnceLock<Mutex<MmrCache>> = OnceLock::new();

    let cache = BLOCK_MMR_CACHE.get_or_init(|| {
        Mutex::new(MmrCache {
            mmr: BlockMmr::default(),
            height: 0,
        })
    });

    let mut guard = cache.lock().expect("mmr cache mutex poisoned");

    if height < guard.height {
        // Rebuild from genesis to requested height to avoid rollback complexity.
        let mut fresh = BlockMmr::default();
        for h in 1..=height {
            let hash = block_hash_at(state, h)?;
            fresh.push(hash);
        }
        guard.mmr = fresh;
        guard.height = height;
    } else {
        for h in (guard.height + 1)..=height {
            let hash = block_hash_at(state, h)?;
            guard.mmr.push(hash);
            guard.height = h;
        }
    }

    Ok(guard.mmr.clone())
}

fn block_hash_at(
    state: &impl StateReadOnly,
    height: u64,
) -> Result<iroha_crypto::HashOf<iroha_data_model::block::BlockHeader>, BridgeFinalityError> {
    let h_usize: usize = height
        .try_into()
        .map_err(|_| BridgeFinalityError::InvalidHeight(height))?;
    let nonzero = NonZeroUsize::new(h_usize).ok_or(BridgeFinalityError::InvalidHeight(height))?;
    let block = state
        .kura()
        .get_block(nonzero)
        .ok_or(BridgeFinalityError::BlockNotFound(height))?;
    Ok(block.hash())
}

/// Build a self-contained finality proof for the block at `height`.
///
/// The proof bundles the block header, its hash, and the commit certificate
/// collected for that block. Verifiers recompute the block hash from the header
/// and validate the commit certificate signatures against the provided
/// validator set.
///
/// # Errors
///
/// Returns [`BridgeFinalityError`] when the height is invalid, the block or commit
/// certificate is missing, or their hashes do not match.
pub fn build_finality_proof(
    state: &impl StateReadOnly,
    height: u64,
) -> Result<BridgeFinalityProof, BridgeFinalityError> {
    let height_usize: usize = height
        .try_into()
        .map_err(|_| BridgeFinalityError::InvalidHeight(height))?;
    let nonzero_height =
        NonZeroUsize::new(height_usize).ok_or(BridgeFinalityError::InvalidHeight(height))?;

    let block = state
        .kura()
        .get_block(nonzero_height)
        .ok_or(BridgeFinalityError::BlockNotFound(height))?;
    let block_header = block.header();
    let block_hash = block.hash();

    let mut cert_candidates: Vec<_> = sumeragi::status::commit_certificate_history()
        .into_iter()
        .filter(|entry| entry.height == height)
        .collect();
    let cert = if let Some(cert) = cert_candidates
        .iter()
        .find(|candidate| candidate.subject_block_hash == block_hash)
    {
        cert.clone()
    } else if let Some(cert) = cert_candidates.pop() {
        return Err(BridgeFinalityError::CommitCertificateHashMismatch {
            height,
            cert_hash: cert.subject_block_hash,
            block_hash,
        });
    } else {
        return Err(BridgeFinalityError::CommitCertificateNotFound(height));
    };

    Ok(BridgeFinalityProof {
        height,
        chain_id: state.chain_id().clone(),
        block_header,
        block_hash,
        commit_certificate: cert,
    })
}

/// Build a commitment + justification bundle for the block at `height`.
///
/// The bundle relies on the commit certificate aggregate signature for
/// justification; the legacy signature list is left empty.
///
/// # Errors
///
/// Returns [`BridgeFinalityError`] when the underlying finality proof or block MMR
/// cannot be built for the requested height.
pub fn build_finality_bundle(
    state: &impl StateReadOnly,
    height: u64,
) -> Result<BridgeFinalityBundle, BridgeFinalityError> {
    let proof = build_finality_proof(state, height)?;
    let mmr = compute_block_mmr(state, height)?;
    let mmr_root = mmr.root();
    let authority_set = BridgeAuthoritySet {
        id: height, // simple monotonically increasing id derived from height; future revisions can carry explicit ids
        validator_set: proof.commit_certificate.validator_set.clone(),
        validator_set_hash: proof.commit_certificate.validator_set_hash,
        validator_set_hash_version: proof.commit_certificate.validator_set_hash_version,
    };
    let commitment = BridgeCommitment {
        chain_id: proof.chain_id.clone(),
        authority_set: authority_set.clone(),
        block_height: proof.height,
        block_hash: proof.block_hash,
        mmr_root,
        mmr_leaf_index: mmr.leaves().checked_sub(1),
        mmr_peaks: Some(mmr.peaks.iter().map(|p| p.hash).collect()),
        next_authority_set: None,
    };
    let justification = BridgeCommitmentJustification {
        signatures: Vec::new(),
    };
    Ok(BridgeFinalityBundle {
        commitment,
        justification,
        block_header: proof.block_header,
        commit_certificate: proof.commit_certificate,
    })
}

/// Verification errors raised when checking a [`BridgeFinalityProof`].
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum BridgeFinalityVerificationError {
    /// The proof carries a different chain id than expected.
    #[error("finality proof chain_id mismatch: expected {expected}, actual {actual}")]
    ChainIdMismatch {
        /// Chain id the verifier expects.
        expected: ChainId,
        /// Chain id carried inside the proof.
        actual: ChainId,
    },
    /// The caller expected a different height than the proof advertises.
    #[error("finality proof height mismatch: expected {expected}, proof {actual}")]
    HeightMismatch {
        /// Height the verifier expected.
        expected: u64,
        /// Height carried in the proof.
        actual: u64,
    },
    /// The block header height disagrees with the proof height.
    #[error("block header height {header_height} does not match proof height {proof_height}")]
    BlockHeaderHeightMismatch {
        /// Height in the proof.
        proof_height: u64,
        /// Height carried in the block header.
        header_height: u64,
    },
    /// The commit certificate height disagrees with the proof height.
    #[error("commit certificate height {cert_height} does not match proof height {proof_height}")]
    CommitCertificateHeightMismatch {
        /// Height in the proof.
        proof_height: u64,
        /// Height carried in the commit certificate.
        cert_height: u64,
    },
    /// Commit certificate phase is not `Commit`.
    #[error("unexpected commit certificate phase {actual:?}")]
    UnexpectedCertificatePhase {
        /// Phase carried in the commit certificate.
        actual: sumeragi::consensus::Phase,
    },
    /// Recomputed block hash does not match the proof/certificate payloads.
    #[error(
        "block hash mismatch: header {header_hash:?}, proof {proof_hash:?}, certificate {certificate_hash:?}"
    )]
    BlockHashMismatch {
        /// Hash recomputed from the block header.
        header_hash: iroha_crypto::HashOf<iroha_data_model::block::BlockHeader>,
        /// Hash carried in the proof.
        proof_hash: iroha_crypto::HashOf<iroha_data_model::block::BlockHeader>,
        /// Hash advertised inside the commit certificate.
        certificate_hash: iroha_crypto::HashOf<iroha_data_model::block::BlockHeader>,
    },
    /// Validator set hash version advertised by the certificate is unsupported.
    #[error("unsupported validator_set_hash_version {version}")]
    UnsupportedValidatorSetHashVersion {
        /// Unsupported version encountered.
        version: u16,
    },
    /// Recomputed validator set hash does not match the certificate payload.
    #[error("validator_set_hash mismatch: computed {computed:?}, advertised {advertised:?}")]
    ValidatorSetHashMismatch {
        /// Hash recomputed from the validator set.
        computed: HashOf<Vec<PeerId>>,
        /// Hash advertised in the commit certificate.
        advertised: HashOf<Vec<PeerId>>,
    },
    /// The verifier pinned a validator set hash that does not match the proof.
    #[error("trusted validator_set_hash mismatch: trusted {trusted:?}, certificate {advertised:?}")]
    TrustedValidatorSetHashMismatch {
        /// Trusted validator set hash supplied by the verifier.
        trusted: HashOf<Vec<PeerId>>,
        /// Hash recomputed from the proof validator set.
        advertised: HashOf<Vec<PeerId>>,
    },
    /// Validator set is empty, so no quorum can be reached.
    #[error("validator set is empty")]
    EmptyValidatorSet,
    /// Signer bitmap length does not match the validator set size.
    #[error("signer bitmap length mismatch: expected {expected}, got {actual}")]
    SignerBitmapLengthMismatch {
        /// Expected bitmap length in bytes.
        expected: usize,
        /// Actual bitmap length in bytes.
        actual: usize,
    },
    /// A signer index falls outside the validator set bounds.
    #[error("signer index {signer} is out of bounds for roster length {roster_len}")]
    SignerOutOfBounds {
        /// Offending signer index.
        signer: u64,
        /// Length of the validator set.
        roster_len: usize,
    },
    /// Duplicate signer index detected inside the commit certificate.
    #[error("duplicate signer index {signer} in commit certificate signatures")]
    DuplicateSigner {
        /// Signer index that appears multiple times.
        signer: u64,
    },
    /// Quorum was not met when counting unique signatures.
    #[error("insufficient signatures: collected {collected}, required {required}")]
    InsufficientSignatures {
        /// Unique signatures collected.
        collected: usize,
        /// Required quorum.
        required: usize,
    },
    /// Commit certificate carries no aggregate signature.
    #[error("commit certificate aggregate signature is missing")]
    AggregateSignatureMissing,
    /// Commit certificate aggregate signature failed verification.
    #[error("commit certificate aggregate signature is invalid")]
    AggregateSignatureInvalid,
}

/// Verification knobs for [`verify_finality_proof`].
#[derive(Debug, Clone)]
pub struct FinalityProofVerificationConfig<'a> {
    /// Chain identifier expected by the verifier.
    pub expected_chain_id: &'a ChainId,
    /// Optional expected height to bind the proof to a specific block.
    pub expected_height: Option<u64>,
    /// Optional trusted validator set hash anchor to guard against roster replays.
    pub trusted_validator_set_hash: Option<HashOf<Vec<PeerId>>>,
}

const fn min_votes_for_len(len: usize) -> usize {
    if len > 3 {
        ((len.saturating_sub(1)) / 3) * 2 + 1
    } else {
        len
    }
}

/// Verify a [`BridgeFinalityProof`] against chain/height/validator set expectations.
///
/// Callers supply the expected chain id and may optionally bind the proof to a specific
/// height and validator set hash. Verification recomputes the block hash, enforces
/// validator set hashing rules, and checks signatures for quorum and validity.
///
/// # Errors
/// Returns [`BridgeFinalityVerificationError`] when the proof fails chain/height checks,
/// validator set hashing/anchors, or signature validation.
pub fn verify_finality_proof(
    proof: &BridgeFinalityProof,
    config: &FinalityProofVerificationConfig<'_>,
) -> Result<(), BridgeFinalityVerificationError> {
    if proof.chain_id != *config.expected_chain_id {
        return Err(BridgeFinalityVerificationError::ChainIdMismatch {
            expected: config.expected_chain_id.clone(),
            actual: proof.chain_id.clone(),
        });
    }

    if let Some(expected_height) = config.expected_height {
        if proof.height != expected_height {
            return Err(BridgeFinalityVerificationError::HeightMismatch {
                expected: expected_height,
                actual: proof.height,
            });
        }
    }

    let header_height = proof.block_header.height().get();
    if header_height != proof.height {
        return Err(BridgeFinalityVerificationError::BlockHeaderHeightMismatch {
            proof_height: proof.height,
            header_height,
        });
    }

    let certificate = &proof.commit_certificate;
    if certificate.height != proof.height {
        return Err(
            BridgeFinalityVerificationError::CommitCertificateHeightMismatch {
                proof_height: proof.height,
                cert_height: certificate.height,
            },
        );
    }

    if certificate.phase != sumeragi::consensus::Phase::Commit {
        return Err(
            BridgeFinalityVerificationError::UnexpectedCertificatePhase {
                actual: certificate.phase,
            },
        );
    }

    let header_hash = proof.block_header.hash();
    if header_hash != proof.block_hash || header_hash != certificate.subject_block_hash {
        return Err(BridgeFinalityVerificationError::BlockHashMismatch {
            header_hash,
            proof_hash: proof.block_hash,
            certificate_hash: certificate.subject_block_hash,
        });
    }

    if certificate.validator_set_hash_version != VALIDATOR_SET_HASH_VERSION_V1 {
        return Err(
            BridgeFinalityVerificationError::UnsupportedValidatorSetHashVersion {
                version: certificate.validator_set_hash_version,
            },
        );
    }

    let computed_set_hash = HashOf::new(&certificate.validator_set);
    if computed_set_hash != certificate.validator_set_hash {
        return Err(BridgeFinalityVerificationError::ValidatorSetHashMismatch {
            computed: computed_set_hash,
            advertised: certificate.validator_set_hash,
        });
    }

    if let Some(trusted) = config.trusted_validator_set_hash {
        if trusted != computed_set_hash {
            return Err(
                BridgeFinalityVerificationError::TrustedValidatorSetHashMismatch {
                    trusted,
                    advertised: computed_set_hash,
                },
            );
        }
    }

    let roster_len = certificate.validator_set.len();
    if roster_len == 0 {
        return Err(BridgeFinalityVerificationError::EmptyValidatorSet);
    }
    let expected_bitmap_len = roster_len.div_ceil(8);
    if certificate.aggregate.signers_bitmap.len() != expected_bitmap_len {
        return Err(
            BridgeFinalityVerificationError::SignerBitmapLengthMismatch {
                expected: expected_bitmap_len,
                actual: certificate.aggregate.signers_bitmap.len(),
            },
        );
    }
    let required = min_votes_for_len(roster_len);
    let mut seen = BTreeSet::new();
    for (byte_idx, byte) in certificate.aggregate.signers_bitmap.iter().enumerate() {
        if *byte == 0 {
            continue;
        }
        for bit in 0..8 {
            if (byte >> bit) & 1 == 0 {
                continue;
            }
            let idx = byte_idx * 8 + bit;
            let signer = u64::try_from(idx).unwrap_or(u64::MAX);
            if idx >= roster_len {
                return Err(BridgeFinalityVerificationError::SignerOutOfBounds {
                    signer,
                    roster_len,
                });
            }
            if !seen.insert(signer) {
                return Err(BridgeFinalityVerificationError::DuplicateSigner { signer });
            }
        }
    }

    if certificate.aggregate.bls_aggregate_signature.is_empty() {
        return Err(BridgeFinalityVerificationError::AggregateSignatureMissing);
    }

    let collected = seen.len();
    if collected < required {
        return Err(BridgeFinalityVerificationError::InsufficientSignatures {
            collected,
            required,
        });
    }

    let vote = sumeragi::consensus::Vote {
        phase: certificate.phase,
        block_hash: certificate.subject_block_hash,
        height: certificate.height,
        view: certificate.view,
        epoch: certificate.epoch,
        highest_cert: None,
        signer: 0,
        bls_sig: Vec::new(),
    };
    let preimage =
        sumeragi::consensus::vote_preimage(config.expected_chain_id, &certificate.mode_tag, &vote);
    let mut public_keys: Vec<&[u8]> = Vec::with_capacity(seen.len());
    for signer in &seen {
        let idx = usize::try_from(*signer).map_err(|_| {
            BridgeFinalityVerificationError::SignerOutOfBounds {
                signer: *signer,
                roster_len,
            }
        })?;
        let Some(peer) = certificate.validator_set.get(idx) else {
            return Err(BridgeFinalityVerificationError::SignerOutOfBounds {
                signer: *signer,
                roster_len,
            });
        };
        let (_, payload) = peer.public_key().to_bytes();
        public_keys.push(payload);
    }
    if iroha_crypto::bls_normal_verify_preaggregated_same_message(
        &preimage,
        &certificate.aggregate.bls_aggregate_signature,
        &public_keys,
    )
    .is_err()
    {
        return Err(BridgeFinalityVerificationError::AggregateSignatureInvalid);
    }

    Ok(())
}
