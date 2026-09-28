//! The certified-chain reader: committed Sumeragi blocks as their Kura frames certify them
//! (`specs/sumeragi.md` §12.7).
//!
//! Kura holds one frame per committed height: the result-bearing iroha block and its
//! [`CommitCertificate`] — the canonical core [`BlockHeader`], the `CommitQC` and the preimage of
//! the certified result `R` ([`ExecutionResultCommitment`]). Genesis carries a result-only
//! certificate (no header, no `CommitQC`); it is the chain's trust root, authenticated by its
//! signature and by the network id every transaction binds (the genesis hash).
//!
//! The reader offers two reads with different trust models:
//!
//! - [`committed_block`]: **consensus-visible data only**. The frame must be the block this State
//!   view committed at the height (header hash and height against the view's block-hash
//!   journal), its core header must certify the block's payload (§3 rule 2), and its result
//!   preimage must commit the exact result-bearing block wire. The receipt ([`CommittedBlock`])
//!   is derived from the header and the result preimage alone: the `CommitQC` bytes are never
//!   decoded. A block's `CommitQC` is **per node** — headers bind `parent_hash` and
//!   `parent_result`, never the parent's certificate, so honest nodes may store different valid
//!   certificates (any `q` signers) for one block — while the header and the result preimage are
//!   identical on every honest node. This is the only read deterministic code (instruction
//!   execution, anything that feeds `R`) may use. It needs the frame of the height it reads:
//!   every validator keeps full blocks in the first release (a snapshot-bootstrapped node must
//!   retain the frames its instructions may name, goal S7).
//! - [`CertifiedChain::certified`]: the committed read **plus the local `CommitQC`**. The
//!   certificate must certify exactly this header and result (kind, height, block hash, result,
//!   attestation flag, instance) and verify under the committee of its height (see below). The
//!   verification is pluggable: by default it is the commit-only check of F8's rule H6 (b)
//!   (`verify_qc_signatures`: `q` Commit signatures prove the commit); a caller that relays the
//!   certificate as an attestation bundle passes an [`AttestationVerifier`] for the full check
//!   (a). Off-chain consumers (Torii, signers, provers, bridges) use this read.
//!
//! **Committees.** The committee of height `x` is `C_x` of the lag-2 schedule (§10.1). `R_{x-2}`
//! commits `committee_digest(C_x)` (`next_committee_digest`), so the reader authenticates a
//! candidate committee against the digest in the result preimage stored at `x - 2`, bound to the
//! certified header of `x` through the header of `x - 1` (its hash is the certified header's
//! `parent_hash`, and it binds `R_{x-2}` as `parent_result`); `C_{g+1}` is the committee the
//! signed genesis registers. The candidates are the committees of the World
//! schedule window and the genesis committee, each with its members' proofs of possession
//! (aggregate verification only admits `PoP`-verified keys). A historical committee that is
//! neither — the chain has since rotated validators — is not reconstructible from the retained
//! state: the reader then reports [`QcVerification::CommittedLocally`], relying on the
//! verification the driver performed before it stored the block (the driver appends only blocks
//! whose `CommitQC` verified under `C_x`, §12.2). That is the trust boundary: a node trusts its
//! own Kura for such heights as it trusts it for its whole state; a caller that needs an
//! independent proof requires [`QcVerification::Verified`].
//!
//! **Chains.** [`CertifiedChain::walk`] reads consecutive heights and additionally checks that
//! each block extends the previous one: its core header binds the parent's block hash and `R`,
//! and its iroha header the parent's iroha hash.

use std::{collections::BTreeMap, num::NonZeroUsize, sync::Arc};

use iroha_crypto::{Hash, HashOf, PublicKey as IrohaPublicKey};
use iroha_data_model::{
    block::{
        BlockHeader as IrohaHeader, CommitCertificate, SignedBlock,
        consensus_v2::HeightContextId,
        proofs::{
            TrustedBlockProofAnchor, TrustedBlockProofAnchorError, TrustedExecutionOutputAnchor,
        },
    },
    transaction::TransactionEntrypoint,
};
use iroha_sumeragi::{
    crypto::{AttestationVerifier, CertError, verify_qc, verify_qc_signatures},
    message::{BlockHeader, Qc, VoteKind},
    preimage::{committee_digest_preimage, payload_hash},
    types::{Committee, Hash32},
};

use super::{
    block_store::{decode_certificate, derive_payload},
    commitment::{ExecutionResultCommitment, chain_hash, result_of_preimage},
    crypto::BlsCrypto,
    node::global_instance,
    schedule,
    startup::{GENESIS_HEIGHT, core_hash_of},
};
use crate::state::{StateReadOnly, WorldReadOnly};

/// Domain tag of a certified block id: `H(tag ‖ block_hash ‖ R)`.
pub const CERTIFIED_BLOCK_ID_TAG: &[u8] = b"iroha/sumeragi/certified-block/v1";

/// Why a committed height could not be read or verified. Every variant is local: a missing
/// height, a view/Kura disagreement, local corruption or a certificate that does not verify.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum ChainReadError {
    /// The height is zero or above this view's committed height.
    #[error("height {height} is not committed in this view")]
    NotCommitted {
        /// Requested height.
        height: u64,
    },
    /// Kura does not hold the block this view committed at the height.
    #[error("Kura does not hold the block this view committed at {height}")]
    NotInView {
        /// Requested height.
        height: u64,
    },
    /// The block carries no commit certificate.
    #[error("the block at {height} carries no commit certificate")]
    MissingCertificate {
        /// Requested height.
        height: u64,
    },
    /// A certificate part is not one canonical frame, or has the wrong shape.
    #[error("the commit certificate at {height} is malformed: {reason}")]
    Malformed {
        /// Requested height.
        height: u64,
        /// What is wrong.
        reason: String,
    },
    /// The certified header or `CommitQC` does not belong to the stored block at this height.
    #[error("the certified header does not match the block at {height}")]
    HeaderMismatch {
        /// Requested height.
        height: u64,
    },
    /// The result preimage does not hash to the certified result.
    #[error("the result preimage at {height} does not hash to the certified result")]
    ResultMismatch {
        /// Requested height.
        height: u64,
    },
    /// The certified result does not commit the stored result-bearing block.
    #[error("the certified result at {height} does not commit the stored block")]
    ExecutionMismatch {
        /// Requested height.
        height: u64,
    },
    /// The certificate names another chain instance.
    #[error("the block at {height} is certified for another chain instance")]
    WrongInstance {
        /// Requested height.
        height: u64,
    },
    /// The block does not extend the block before it.
    #[error("the block at {height} does not extend its parent")]
    Discontinuous {
        /// Requested height.
        height: u64,
    },
    /// Kura's genesis is not this view's network genesis.
    #[error("Kura's genesis is not this view's network genesis")]
    ForeignGenesis,
    /// The committee of the height cannot be formed.
    #[error("no committee for height {height}: {reason}")]
    Committee {
        /// Requested height.
        height: u64,
        /// What is wrong.
        reason: String,
    },
    /// The `CommitQC` does not verify under the committee of its height.
    #[error("the commit certificate at {height} does not verify: {error:?}")]
    Certificate {
        /// Requested height.
        height: u64,
        /// The failed check.
        error: CertError,
    },
}

/// The consensus-visible receipt of a committed height: identical on every honest node, derived
/// from the stored block, its core header and its result preimage (never from the `CommitQC`).
#[derive(Clone, Debug)]
pub struct CommittedBlock {
    height: u64,
    block: Arc<SignedBlock>,
    header: Option<BlockHeader>,
    core_hash: Hash32,
    result: Hash32,
    commitment: ExecutionResultCommitment,
}

impl CommittedBlock {
    /// The one-based height.
    #[must_use]
    pub fn height(&self) -> u64 {
        self.height
    }

    /// The committed result-bearing iroha block as Kura stores it. Its
    /// [`commit_certificate`](SignedBlock::commit_certificate) is node-local: deterministic code
    /// must not read it.
    #[must_use]
    pub fn block(&self) -> &Arc<SignedBlock> {
        &self.block
    }

    /// The iroha block hash.
    #[must_use]
    pub fn block_hash(&self) -> HashOf<IrohaHeader> {
        self.block.hash()
    }

    /// The certified core header (`None` for genesis).
    #[must_use]
    pub fn header(&self) -> Option<&BlockHeader> {
        self.header.as_ref()
    }

    /// The core block hash: the hash of the core header, or the iroha hash bytes for genesis.
    #[must_use]
    pub fn core_hash(&self) -> Hash32 {
        self.core_hash
    }

    /// The certified execution result `R`.
    #[must_use]
    pub fn result(&self) -> Hash32 {
        self.result
    }

    /// The decoded preimage of `R`.
    #[must_use]
    pub fn commitment(&self) -> &ExecutionResultCommitment {
        &self.commitment
    }

    /// The certified block id `H(CERTIFIED_BLOCK_ID_TAG ‖ core_hash ‖ R)`: what a `CommitQC` of
    /// this height certifies, as one identifier. Signer floors and proofs pin it in their
    /// `context_id` fields (the data-model type keeps its v2 name until the wire cleanup).
    #[must_use]
    pub fn id(&self) -> HeightContextId {
        certified_block_id(&self.core_hash, &self.result)
    }

    /// The block time in milliseconds.
    #[must_use]
    pub fn block_time_ms(&self) -> u64 {
        u64::try_from(self.block.header().creation_time().as_millis()).unwrap_or(u64::MAX)
    }

    /// Whether `self` directly extends `parent`: consecutive heights, the core header binds the
    /// parent's block hash and `R`, and the iroha header the parent's iroha hash.
    #[must_use]
    pub fn extends(&self, parent: &Self) -> bool {
        parent.height.checked_add(1) == Some(self.height)
            && self.block.header().prev_block_hash() == Some(parent.block_hash())
            && self.header.as_ref().is_some_and(|header| {
                header.parent_hash == parent.core_hash && header.parent_result == parent.result
            })
    }

    /// The inclusion anchor of the network entrypoint `entry_hash`, bound to the executed wire
    /// that `R` commits.
    ///
    /// # Errors
    /// The entry is absent or the block's Merkle material is inconsistent.
    pub fn entry_anchor(
        &self,
        entry_hash: &HashOf<TransactionEntrypoint>,
    ) -> Result<TrustedBlockProofAnchor, TrustedBlockProofAnchorError> {
        let execution = &self.commitment.execution;
        TrustedBlockProofAnchor::from_committed_execution(
            &self.block,
            execution.executed_block_wire_len,
            execution.executed_block_wire_hash,
            entry_hash,
        )
    }

    /// The anchor of the typed output at `output_index`, bound to the executed wire that `R`
    /// commits.
    ///
    /// # Errors
    /// The output is absent or the block's Merkle material is inconsistent.
    pub fn output_anchor(
        &self,
        output_index: u32,
    ) -> Result<TrustedExecutionOutputAnchor, TrustedBlockProofAnchorError> {
        let execution = &self.commitment.execution;
        TrustedExecutionOutputAnchor::from_committed_execution(
            &self.block,
            execution.executed_block_wire_len,
            execution.executed_block_wire_hash,
            output_index,
        )
    }
}

/// `H(CERTIFIED_BLOCK_ID_TAG ‖ block_hash ‖ R)` as the data model's id type.
#[must_use]
pub fn certified_block_id(block_hash: &Hash32, result: &Hash32) -> HeightContextId {
    let mut bytes = Vec::with_capacity(CERTIFIED_BLOCK_ID_TAG.len() + 64);
    bytes.extend_from_slice(CERTIFIED_BLOCK_ID_TAG);
    bytes.extend_from_slice(&block_hash.0);
    bytes.extend_from_slice(&result.0);
    HeightContextId(HashOf::from_untyped_unchecked(Hash::new(&bytes)))
}

/// Read the consensus-visible receipt of the block `view` committed at `height` (see the module
/// documentation). Deterministic: every honest node with the same committed history reads the
/// same receipt, whatever `CommitQC` it stored.
///
/// # Errors
/// The height is not committed, Kura does not hold the view's block, or the frame's header or
/// result preimage does not certify it.
pub fn committed_block(
    view: &(impl StateReadOnly + ?Sized),
    height: u64,
) -> Result<CommittedBlock, ChainReadError> {
    let index = usize::try_from(height)
        .ok()
        .and_then(NonZeroUsize::new)
        .filter(|index| index.get() <= view.block_hashes().len())
        .ok_or(ChainReadError::NotCommitted { height })?;
    let block = view
        .canonical_block_by_height(index)
        .map_err(|_| ChainReadError::NotInView { height })?;
    read_frame(block, height)
}

/// The receipt of a Kura frame claimed to be the committed block at `height`.
fn read_frame(block: Arc<SignedBlock>, height: u64) -> Result<CommittedBlock, ChainReadError> {
    let malformed = |reason: String| ChainReadError::Malformed { height, reason };
    // Hashing only: the chain hash `H` needs no admitted key.
    let hasher = BlsCrypto::new();
    let certificate = block
        .commit_certificate()
        .ok_or(ChainReadError::MissingCertificate { height })?;
    let genesis = height == GENESIS_HEIGHT;
    let (header, core_hash) = if genesis {
        if !certificate.consensus_header.is_empty() || !certificate.commit_qc.is_empty() {
            return Err(malformed(
                "genesis carries a result-only certificate".into(),
            ));
        }
        (None, core_hash_of(&block))
    } else {
        let header: BlockHeader = norito::decode_canonical(&certificate.consensus_header)
            .map_err(|error| malformed(error.to_string()))?;
        let payload = derive_payload(&block, header.payload_len)
            .map_err(|error| malformed(error.to_string()))?;
        if header.height != height
            || u32::try_from(payload.len()).ok() != Some(header.payload_len)
            || payload_hash(&hasher, &payload) != header.payload_hash
        {
            return Err(ChainReadError::HeaderMismatch { height });
        }
        let core_hash = header.hash(&hasher);
        (Some(header), core_hash)
    };
    let result = result_of_preimage(&certificate.result_preimage);
    let commitment = ExecutionResultCommitment::decode(&certificate.result_preimage)
        .map_err(|error| malformed(error.to_string()))?;
    let (wire_len, wire_hash) = block
        .executed_block_wire_identity()
        .map_err(|error| malformed(error.to_string()))?;
    if block.header().height().get() != height
        || commitment.execution.executed_block_wire_len != wire_len
        || commitment.execution.executed_block_wire_hash != wire_hash
    {
        return Err(ChainReadError::ExecutionMismatch { height });
    }
    Ok(CommittedBlock {
        height,
        block,
        header,
        core_hash,
        result,
        commitment,
    })
}

/// How the local `CommitQC` of a height was checked.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QcVerification {
    /// Genesis: the signed trust root of the chain, without a `CommitQC`.
    Genesis,
    /// The `CommitQC` verified under the committee of its height.
    Verified,
    /// The committee of the height is not reconstructible from the retained chain; the driver
    /// verified the `CommitQC` before storing the block (see the module documentation).
    CommittedLocally,
}

/// A committed height with its local `CommitQC` checked.
#[derive(Clone, Debug)]
pub struct CertifiedBlock {
    committed: CommittedBlock,
    commit_qc: Option<Qc>,
    verification: QcVerification,
    certificate_len: usize,
}

impl CertifiedBlock {
    /// The consensus-visible receipt.
    #[must_use]
    pub fn committed(&self) -> &CommittedBlock {
        &self.committed
    }

    /// Take the consensus-visible receipt.
    #[must_use]
    pub fn into_committed(self) -> CommittedBlock {
        self.committed
    }

    /// This node's `CommitQC` of the block (`None` for genesis).
    #[must_use]
    pub fn commit_qc(&self) -> Option<&Qc> {
        self.commit_qc.as_ref()
    }

    /// How the `CommitQC` was checked.
    #[must_use]
    pub fn verification(&self) -> QcVerification {
        self.verification
    }

    /// The node-local commit certificate (header, `CommitQC`, result preimage); always present
    /// on a certified read.
    #[must_use]
    pub fn certificate(&self) -> Option<&CommitCertificate> {
        self.committed.block.commit_certificate()
    }

    /// Canonical frame bytes of the certificate, for callers that bound what they read.
    #[must_use]
    pub fn certificate_len(&self) -> usize {
        self.certificate_len
    }
}

impl core::ops::Deref for CertifiedBlock {
    type Target = CommittedBlock;

    fn deref(&self) -> &CommittedBlock {
        &self.committed
    }
}

/// A committee candidate: the core committee and its members with their proofs of possession.
struct Candidate {
    committee: Committee,
    members: Vec<(IrohaPublicKey, Vec<u8>)>,
}

/// The candidate committees of a reader (see the module documentation).
struct Candidates {
    /// Candidates by `committee_digest` under the chain hash.
    by_digest: BTreeMap<[u8; 32], Candidate>,
    /// `digest(C_{g+1})`: the genesis committee.
    genesis: [u8; 32],
}

/// Keys whose proof of possession verified, for aggregate verification. A key stays admitted:
/// possession is a property of the key, and certificates of old heights still name it.
static ADMITTED: std::sync::LazyLock<BlsCrypto> = std::sync::LazyLock::new(BlsCrypto::new);

/// The certified-chain reader over one State view (see the module documentation).
pub struct CertifiedChain<'v, V: StateReadOnly + ?Sized> {
    view: &'v V,
    genesis: Arc<SignedBlock>,
    instance: Hash32,
    attestations: Option<&'v dyn AttestationVerifier>,
    candidates: std::sync::OnceLock<Result<Candidates, ChainReadError>>,
}

impl<V: StateReadOnly + ?Sized> core::fmt::Debug for CertifiedChain<'_, V> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("CertifiedChain")
            .field("instance", &self.instance)
            .finish_non_exhaustive()
    }
}

impl<'v, V: StateReadOnly + ?Sized> CertifiedChain<'v, V> {
    /// A reader over `view` with the commit-only `CommitQC` check (F8 rule H6 (b)).
    ///
    /// # Errors
    /// The view has no genesis, or Kura's genesis is not the view's network genesis.
    pub fn new(view: &'v V) -> Result<Self, ChainReadError> {
        if view.block_hashes().is_empty() {
            return Err(ChainReadError::NotCommitted {
                height: GENESIS_HEIGHT,
            });
        }
        let genesis = view
            .canonical_block_by_height(NonZeroUsize::MIN)
            .map_err(|_| ChainReadError::NotInView {
                height: GENESIS_HEIGHT,
            })?;
        if genesis.hash().as_ref() != view.network_id().as_bytes() || !genesis.header().is_genesis()
        {
            return Err(ChainReadError::ForeignGenesis);
        }
        let instance = global_instance(&genesis, &view.chain_id().to_string());
        Ok(Self {
            view,
            genesis,
            instance,
            attestations: None,
            candidates: std::sync::OnceLock::new(),
        })
    }

    /// Verify `CommitQC`s fully with `verifier`, including the attestations of flagged blocks
    /// (F8 rule H6 (a)).
    #[must_use]
    pub fn with_attestation_verifier(mut self, verifier: &'v dyn AttestationVerifier) -> Self {
        self.attestations = Some(verifier);
        self
    }

    /// The chain instance `I` every certificate must name.
    #[must_use]
    pub fn instance(&self) -> Hash32 {
        self.instance
    }

    /// The signed genesis block (the chain's trust root).
    #[must_use]
    pub fn genesis(&self) -> &Arc<SignedBlock> {
        &self.genesis
    }

    /// The consensus-visible receipt of `height` ([`committed_block`]).
    ///
    /// # Errors
    /// See [`committed_block`].
    pub fn committed(&self, height: u64) -> Result<CommittedBlock, ChainReadError> {
        committed_block(self.view, height)
    }

    /// The committed block at `height` with its local `CommitQC` checked (see the module
    /// documentation).
    ///
    /// # Errors
    /// The committed read fails, the certificate does not certify the stored header and result,
    /// names another instance, or does not verify under the committee of its height.
    pub fn certified(&self, height: u64) -> Result<CertifiedBlock, ChainReadError> {
        self.check_certificate(self.committed(height)?)
    }

    /// The exact authenticated committee and its proofs of possession for a portable proof.
    ///
    /// Genesis uses the registrations in its signed block; other heights require the
    /// lag-2 committed committee digest to match a retained candidate. This never returns
    /// an unverified current roster for a historical height.
    ///
    /// # Errors
    /// The committee is not independently reconstructible from the retained chain.
    pub fn proof_committee(
        &self,
        height: u64,
    ) -> Result<Vec<(IrohaPublicKey, Vec<u8>)>, ChainReadError> {
        let candidates = self.candidates()?;
        let selected = if height == GENESIS_HEIGHT {
            candidates.genesis
        } else {
            let (header, _) = self.certificate_parts(height)?;
            let header = header.ok_or(ChainReadError::NotCommitted { height })?;
            let committee =
                self.committee_of(&header)?
                    .ok_or_else(|| ChainReadError::Committee {
                        height,
                        reason: "historical committee is not independently reconstructible"
                            .to_owned(),
                    })?;
            digest(committee)
        };
        let members = &candidates
            .by_digest
            .get(&selected)
            .ok_or_else(|| ChainReadError::Committee {
                height,
                reason: "authenticated committee candidate is missing".to_owned(),
            })?
            .members;
        let mut ordered = members
            .iter()
            .map(|(key, pop)| {
                super::crypto::core_key(key)
                    .map(|core| (core, (key.clone(), pop.clone())))
                    .map_err(|error| ChainReadError::Committee {
                        height,
                        reason: error.to_string(),
                    })
            })
            .collect::<Result<Vec<_>, _>>()?;
        ordered.sort_by(|left, right| left.0.cmp(&right.0));
        Ok(ordered.into_iter().map(|(_, member)| member).collect())
    }

    /// The certified blocks `from..=to`, oldest first, each checked to extend the previous one.
    /// The iterator stops after the first error.
    pub fn walk(
        &self,
        from: u64,
        to: u64,
    ) -> impl Iterator<Item = Result<CertifiedBlock, ChainReadError>> + '_ {
        let mut parent: Option<CommittedBlock> = None;
        let mut failed = false;
        (from..=to).map_while(move |height| {
            if failed {
                return None;
            }
            let read = self.certified(height).and_then(|block| match &parent {
                Some(parent) if !block.extends(parent) => {
                    Err(ChainReadError::Discontinuous { height })
                }
                _ => Ok(block),
            });
            match &read {
                Ok(block) => parent = Some(block.committed.clone()),
                Err(_) => failed = true,
            }
            Some(read)
        })
    }

    /// The candidate committees: the genesis committee and the World schedule window's.
    fn candidates(&self) -> Result<&Candidates, ChainReadError> {
        self.candidates
            .get_or_init(|| {
                let committee_error = |reason: String| ChainReadError::Committee {
                    height: GENESIS_HEIGHT.saturating_add(1),
                    reason,
                };
                let registrations = schedule::genesis_registrations(&self.genesis)
                    .map_err(|error| committee_error(error.to_string()))?;
                let genesis = candidate(
                    registrations
                        .into_iter()
                        .map(|(peer, pop)| (peer.public_key().clone(), pop)),
                )
                .map_err(committee_error)?;
                let genesis_digest = digest(&genesis.committee);
                let mut by_digest = BTreeMap::new();
                by_digest.insert(genesis_digest, genesis);
                let world = self.view.world();
                for config in world.consensus_schedule().entries() {
                    let members = schedule::committee_pops(world, config)
                        .into_iter()
                        .map(|(peer, pop)| (peer.public_key().clone(), pop));
                    // A window entry whose members lack live proofs of possession cannot verify
                    // an aggregate; leave it out (its heights report `CommittedLocally`).
                    if let Ok(entry) = candidate(members)
                        && config
                            .height_config()
                            .is_ok_and(|core| core.committee == entry.committee)
                    {
                        by_digest.entry(digest(&entry.committee)).or_insert(entry);
                    }
                }
                Ok(Candidates {
                    by_digest,
                    genesis: genesis_digest,
                })
            })
            .as_ref()
            .map_err(Clone::clone)
    }

    /// `C_height` of the stored core `header` of `height`, authenticated against the digest
    /// `R_{height-2}` commits (`C_{g+1}`: the genesis committee), with its keys admitted; `None`
    /// if no candidate matches.
    fn committee_of(&self, header: &BlockHeader) -> Result<Option<&Committee>, ChainReadError> {
        let height = header.height;
        let candidates = self.candidates()?;
        let first = GENESIS_HEIGHT.saturating_add(1);
        let expected = if height == first {
            candidates.genesis
        } else {
            let scheduler = height
                .checked_sub(schedule::LAG)
                .filter(|scheduler| *scheduler >= GENESIS_HEIGHT)
                .ok_or(ChainReadError::NotCommitted { height })?;
            // `R_{height-2}`'s preimage, authenticated through the headers the `CommitQC` of
            // `height` certifies: `header` binds its parent's hash, and that parent (the header of
            // `height - 1`) binds `R_{height-2}` as `parent_result`.
            let (_, preimage) = self.certificate_parts(scheduler)?;
            let (parent, _) = self.certificate_parts(scheduler.saturating_add(1))?;
            let parent = parent
                .filter(|parent| parent.hash(&BlsCrypto::new()) == header.parent_hash)
                .ok_or(ChainReadError::Discontinuous { height })?;
            if parent.parent_result != result_of_preimage(&preimage) {
                return Err(ChainReadError::ResultMismatch { height: scheduler });
            }
            ExecutionResultCommitment::decode(&preimage)
                .map_err(|error| ChainReadError::Malformed {
                    height: scheduler,
                    reason: error.to_string(),
                })?
                .next_committee_digest
        };
        let Some(candidate) = candidates.by_digest.get(&expected) else {
            return Ok(None);
        };
        for (key, pop) in &candidate.members {
            let admitted =
                super::crypto::core_key(key).is_ok_and(|core| ADMITTED.is_admitted(&core));
            if !admitted {
                ADMITTED
                    .admit(key, pop)
                    .map_err(|error| ChainReadError::Committee {
                        height,
                        reason: error.to_string(),
                    })?;
            }
        }
        Ok(Some(&candidate.committee))
    }

    /// The core header (`None` for genesis) and result preimage stored with the block this view
    /// committed at `height`, without re-deriving the block's wire.
    fn certificate_parts(
        &self,
        height: u64,
    ) -> Result<(Option<BlockHeader>, Vec<u8>), ChainReadError> {
        let index = usize::try_from(height)
            .ok()
            .and_then(NonZeroUsize::new)
            .filter(|index| index.get() <= self.view.block_hashes().len())
            .ok_or(ChainReadError::NotCommitted { height })?;
        let block = self
            .view
            .canonical_block_by_height(index)
            .map_err(|_| ChainReadError::NotInView { height })?;
        let certificate = block
            .commit_certificate()
            .ok_or(ChainReadError::MissingCertificate { height })?;
        let header = if height == GENESIS_HEIGHT {
            None
        } else {
            let header: BlockHeader = norito::decode_canonical(&certificate.consensus_header)
                .map_err(|error| ChainReadError::Malformed {
                    height,
                    reason: error.to_string(),
                })?;
            Some(header)
        };
        Ok((header, certificate.result_preimage.clone()))
    }

    /// Check a committed block's local `CommitQC` (the second half of [`Self::certified`]).
    fn check_certificate(
        &self,
        committed: CommittedBlock,
    ) -> Result<CertifiedBlock, ChainReadError> {
        let height = committed.height;
        let malformed = |reason: String| ChainReadError::Malformed { height, reason };
        let certificate = committed
            .block
            .commit_certificate()
            .ok_or(ChainReadError::MissingCertificate { height })?;
        let certificate_len = norito::canonical_frame_len(certificate)
            .map_err(|error| malformed(error.to_string()))?;
        let Some(header) = committed.header.as_ref() else {
            return Ok(CertifiedBlock {
                committed,
                commit_qc: None,
                verification: QcVerification::Genesis,
                certificate_len,
            });
        };
        let (_, commit_qc) =
            decode_certificate(certificate).map_err(|error| malformed(error.to_string()))?;
        if commit_qc.kind != VoteKind::Commit
            || commit_qc.height != height
            || commit_qc.block_hash != committed.core_hash
            || commit_qc.attest != header.attest
        {
            return Err(ChainReadError::HeaderMismatch { height });
        }
        if commit_qc.result != committed.result {
            return Err(ChainReadError::ResultMismatch { height });
        }
        if header.instance != self.instance || commit_qc.instance != self.instance {
            return Err(ChainReadError::WrongInstance { height });
        }
        let verification = match self.committee_of(header)? {
            Some(committee) => {
                let checked = match self.attestations {
                    Some(verifier) => {
                        verify_qc(&*ADMITTED, verifier, &self.instance, committee, &commit_qc)
                    }
                    // TODO(F8.4): readers that relay a flagged block's certificate pass the
                    // historical KAGEMUSHA attestation verifier (F8 rule H6 (a)); F8.4 adds it
                    // with a flagged-certificate test for this reader.
                    None => verify_qc_signatures(&*ADMITTED, &self.instance, committee, &commit_qc),
                };
                checked.map_err(|error| ChainReadError::Certificate { height, error })?;
                QcVerification::Verified
            }
            None => QcVerification::CommittedLocally,
        };
        Ok(CertifiedBlock {
            committed,
            commit_qc: Some(commit_qc),
            verification,
            certificate_len,
        })
    }
}

/// `committee_digest(committee)` under the chain hash, as `R` commits it.
fn digest(committee: &Committee) -> [u8; 32] {
    chain_hash(&committee_digest_preimage(committee)).0
}

/// A candidate committee of `members` (their keys in any order).
fn candidate(
    members: impl IntoIterator<Item = (IrohaPublicKey, Vec<u8>)>,
) -> Result<Candidate, String> {
    let members: Vec<_> = members.into_iter().collect();
    let keys = members
        .iter()
        .map(|(key, _)| super::crypto::core_key(key).map_err(|error| error.to_string()))
        .collect::<Result<Vec<_>, _>>()?;
    let committee = Committee::new(keys).map_err(|error| format!("{error:?}"))?;
    Ok(Candidate { committee, members })
}

#[cfg(test)]
mod tests;
