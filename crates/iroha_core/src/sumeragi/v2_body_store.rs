//! Crash-safe storage for exact Sumeragi v2 proposal bodies.
//!
//! Consensus votes authenticate a [`wire::BlockSubject`], including the hash of the
//! exact canonical [`SignedBlock`] wire bytes.  This store is the durability
//! boundary between reconstruction and the reducer's `BodyStored` input: a
//! receipt can only be obtained after the bytes, their metadata, and the
//! directory entry have been synchronised.

use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    mem::size_of,
    path::{Path, PathBuf},
};

use super::v2_core::EventTag;
use iroha_crypto::{Hash, HashOf, PublicKey};
use iroha_data_model::block::{
    CertifiedMergeLedgerReference, SignedBlock, consensus_v2 as wire, decode_framed_signed_block,
};
use norito::codec::{Decode, DecodeAll as _, Encode};
use thiserror::Error;

use super::{
    v2::RecoveredValidationAuthority,
    v2_apply::VerifiedRecoveredFinalitySubject,
    v2_effects::{BodyStoreTask, BodyValidationTask, EffectWorkId},
    v2_transport::AuthenticatedCertifiedBodyResponse,
};
use crate::kura::KuraV2CommitReceipt;

const STORE_MAGIC: &[u8; 8] = b"SUM2BODY";
const VALIDATED_MAGIC: &[u8; 8] = b"SUM2VALD";
const STORE_VERSION: u16 = 4;
const FRAME_HEADER_LEN: usize = STORE_MAGIC.len() + size_of::<u16>() + size_of::<u64>();
const CHECKSUM_LEN: usize = 32;

/// Metadata and exact canonical bytes persisted in one final body file.
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode)]
struct StoredBodyEnvelope {
    version: u16,
    context_id: wire::HeightContextId,
    round: wire::ConsensusRound,
    subject: wire::BlockSubject,
    manifest: wire::PayloadManifest,
    canonical_wire: Vec<u8>,
}

/// Durable proof that deterministic validation completed for one exact body
/// frame before the reducer persisted a Prepare intent.
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode)]
struct ValidatedBodyMarker {
    version: u16,
    context_id: wire::HeightContextId,
    round: wire::ConsensusRound,
    subject: wire::BlockSubject,
    manifest_hash: HashOf<wire::PayloadManifest>,
    body_frame_hash: Hash,
    execution_commitment: wire::ExecutionCommitment,
}

/// Non-forgeable acknowledgement that one exact body is durable locally.
#[derive(Clone, Debug, PartialEq, Eq)]
#[must_use]
pub(crate) struct DurableBodyReceipt {
    context_id: wire::HeightContextId,
    round: wire::ConsensusRound,
    subject: wire::BlockSubject,
    manifest_hash: HashOf<wire::PayloadManifest>,
    frame_hash: Hash,
}

/// Durable proof that one fully authenticated certified-Fetch response's exact
/// canonical body has crossed the local file-and-directory sync boundary.
///
/// This receipt deliberately retains the transport occurrence hashes without
/// serializing them into the body store. The nested body receipt remains the
/// sole authority for reloading the canonical bytes after restart.
#[derive(Clone, Debug, PartialEq, Eq)]
#[must_use]
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) struct DurableCertifiedFetchBodyReceipt {
    request_hash: HashOf<wire::CertifiedBodyRequest>,
    response_hash: HashOf<wire::CertifiedBodyResponse>,
    durable_body: DurableBodyReceipt,
}

#[cfg_attr(not(test), allow(dead_code))]
impl DurableCertifiedFetchBodyReceipt {
    /// Hash of the exact authenticated request family served by the response.
    pub(crate) const fn request_hash(&self) -> HashOf<wire::CertifiedBodyRequest> {
        self.request_hash
    }

    /// Hash of the complete authenticated response, including its responder.
    pub(crate) const fn response_hash(&self) -> HashOf<wire::CertifiedBodyResponse> {
        self.response_hash
    }

    /// Durable receipt for the response's exact canonical body-store frame.
    pub(crate) const fn durable_body(&self) -> &DurableBodyReceipt {
        &self.durable_body
    }
}

/// Non-forgeable acknowledgement that deterministic validation succeeded for
/// the exact durable body.
#[derive(Clone, Debug, PartialEq, Eq)]
#[must_use]
pub(crate) struct ValidatedBodyReceipt {
    durable: DurableBodyReceipt,
    execution_commitment: wire::ExecutionCommitment,
}

// DURABLE_BODY_VALIDATION_SURFACE_BEGIN
/// Canonical volatile identity of a deterministic body rejection.
///
/// All non-sidecar validation failures currently drive the same
/// `ValidationCompleted { valid: false }` reducer transition. Keeping one
/// closed code prevents diagnostic formatting or unstable internal error
/// variants from becoming physical lifecycle identity.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum BodyValidationRejectionIdentity {
    /// Deterministic validation rejected the exact durable body.
    Rejected,
}

impl BodyValidationRejectionIdentity {
    /// Return the domain-local bounded code used by volatile lifecycle digests.
    pub(crate) const fn canonical_code(&self) -> u8 {
        match self {
            Self::Rejected => 0,
        }
    }
}

/// Scheduler-free result of validating one exact durable body.
///
/// The private payload keeps construction inside this storage boundary. In
/// particular, every non-success result retains the same durable receipt that
/// was checked before the validator ran, so a caller cannot accidentally
/// rebind a diagnostic or merge-sidecar dependency to another proposal.
#[derive(Debug, PartialEq, Eq)]
#[must_use]
pub(crate) struct DurableBodyValidationOutcome(DurableBodyValidationOutcomeBody);

#[derive(Debug, PartialEq, Eq)]
#[allow(variant_size_differences, clippy::large_enum_variant)]
enum DurableBodyValidationOutcomeBody {
    Validated(ValidatedBodyReceipt),
    Rejected {
        durable: DurableBodyReceipt,
        identity: BodyValidationRejectionIdentity,
        reason: String,
    },
    DeferredMergeSidecar {
        durable: DurableBodyReceipt,
        reference: CertifiedMergeLedgerReference,
    },
}

#[cfg_attr(not(test), allow(dead_code))]
impl DurableBodyValidationOutcome {
    /// Exact durable body bound to this result.
    pub(crate) const fn durable_body(&self) -> &DurableBodyReceipt {
        match &self.0 {
            DurableBodyValidationOutcomeBody::Validated(receipt) => receipt.durable(),
            DurableBodyValidationOutcomeBody::Rejected { durable, .. }
            | DurableBodyValidationOutcomeBody::DeferredMergeSidecar { durable, .. } => durable,
        }
    }

    /// Durable success receipt, when deterministic validation succeeded.
    pub(crate) const fn validated_receipt(&self) -> Option<&ValidatedBodyReceipt> {
        match &self.0 {
            DurableBodyValidationOutcomeBody::Validated(receipt) => Some(receipt),
            DurableBodyValidationOutcomeBody::Rejected { .. }
            | DurableBodyValidationOutcomeBody::DeferredMergeSidecar { .. } => None,
        }
    }

    /// Deterministic rejection diagnostic, when validation rejected the body.
    pub(crate) fn rejection_reason(&self) -> Option<&str> {
        match &self.0 {
            DurableBodyValidationOutcomeBody::Rejected { reason, .. } => Some(reason),
            DurableBodyValidationOutcomeBody::Validated(_)
            | DurableBodyValidationOutcomeBody::DeferredMergeSidecar { .. } => None,
        }
    }

    /// Canonical volatile identity of a deterministic rejection.
    pub(crate) const fn rejection_identity(&self) -> Option<&BodyValidationRejectionIdentity> {
        match &self.0 {
            DurableBodyValidationOutcomeBody::Rejected { identity, .. } => Some(identity),
            DurableBodyValidationOutcomeBody::Validated(_)
            | DurableBodyValidationOutcomeBody::DeferredMergeSidecar { .. } => None,
        }
    }

    /// Exact certified merge reference whose absence deferred validation.
    pub(crate) const fn missing_merge_sidecar(&self) -> Option<&CertifiedMergeLedgerReference> {
        match &self.0 {
            DurableBodyValidationOutcomeBody::DeferredMergeSidecar { reference, .. } => {
                Some(reference)
            }
            DurableBodyValidationOutcomeBody::Validated(_)
            | DurableBodyValidationOutcomeBody::Rejected { .. } => None,
        }
    }

    /// Consume this closed result only when deterministic validation succeeded.
    ///
    /// A rejection or sidecar deferral is returned intact so the future typed
    /// lifecycle transaction cannot accidentally discard or relabel it while
    /// attempting the success-only Validate completion path.
    pub(crate) fn into_validated_receipt(self) -> Result<ValidatedBodyReceipt, Self> {
        match self.0 {
            DurableBodyValidationOutcomeBody::Validated(receipt) => Ok(receipt),
            body => Err(Self(body)),
        }
    }

    fn into_body(self) -> DurableBodyValidationOutcomeBody {
        self.0
    }
}
// DURABLE_BODY_VALIDATION_SURFACE_END

/// Completion minted only after an exact body task reaches durable storage.
#[derive(Clone, Debug, PartialEq, Eq)]
#[must_use]
pub(crate) struct BodyStoreCompletion {
    work_id: EffectWorkId,
    tag: EventTag,
    manifest: wire::PayloadManifest,
    receipt: DurableBodyReceipt,
}

impl BodyStoreCompletion {
    /// Stable asynchronous work identifier.
    pub(crate) const fn work_id(&self) -> EffectWorkId {
        self.work_id
    }

    /// Original reducer event tag.
    pub(crate) const fn tag(&self) -> EventTag {
        self.tag
    }

    /// Non-forgeable receipt for the exact durable bytes.
    pub(crate) const fn receipt(&self) -> &DurableBodyReceipt {
        &self.receipt
    }

    /// Exact manifest stored beside the durable bytes.
    pub(crate) const fn manifest(&self) -> &wire::PayloadManifest {
        &self.manifest
    }
}

/// Result minted by the body-store service after reloading and validating the
/// exact durable body represented by a validation task.
#[derive(Clone, Debug, PartialEq, Eq)]
#[must_use]
pub(crate) enum BodyValidationCompletion {
    /// Deterministic semantic validation succeeded.
    Validated {
        /// Stable asynchronous work identifier.
        work_id: EffectWorkId,
        /// Non-forgeable validation receipt.
        receipt: ValidatedBodyReceipt,
    },
    /// Deterministic semantic validation rejected the exact body.
    Rejected {
        /// Stable asynchronous work identifier.
        work_id: EffectWorkId,
        /// Deterministic validator diagnostic.
        reason: String,
    },
    /// Validation is sound but cannot finish until the exact certified merge
    /// sidecar referenced by the durable body is fetched and authenticated.
    DeferredMergeSidecar {
        /// Stable asynchronous work identifier retained for the exact retry.
        work_id: EffectWorkId,
        /// Complete compact reference needed by the bounded sidecar transport.
        reference: CertifiedMergeLedgerReference,
    },
}

impl BodyValidationCompletion {
    /// Stable asynchronous work identifier.
    pub(crate) const fn work_id(&self) -> EffectWorkId {
        match self {
            Self::Validated { work_id, .. }
            | Self::Rejected { work_id, .. }
            | Self::DeferredMergeSidecar { work_id, .. } => *work_id,
        }
    }

    /// Non-forgeable success receipt, when validation succeeded.
    pub(crate) const fn validated_receipt(&self) -> Option<&ValidatedBodyReceipt> {
        match self {
            Self::Validated { receipt, .. } => Some(receipt),
            Self::Rejected { .. } | Self::DeferredMergeSidecar { .. } => None,
        }
    }

    /// Deterministic rejection diagnostic, when validation failed.
    pub(crate) fn rejection_reason(&self) -> Option<&str> {
        match self {
            Self::Rejected { reason, .. } => Some(reason),
            Self::Validated { .. } | Self::DeferredMergeSidecar { .. } => None,
        }
    }

    /// Compact certified merge reference whose absence deferred validation.
    pub(crate) const fn missing_merge_sidecar(&self) -> Option<&CertifiedMergeLedgerReference> {
        match self {
            Self::DeferredMergeSidecar { reference, .. } => Some(reference),
            Self::Validated { .. } | Self::Rejected { .. } => None,
        }
    }
}

/// Typed classification supplied by deterministic body validators.
///
/// Only a missing, compact-reference-bound merge sidecar is recoverable. Every
/// other semantic error remains a terminal rejection of the exact body.
pub(crate) trait BodyValidationError: std::fmt::Display {
    /// Return the canonical reducer-level identity of a terminal rejection.
    ///
    /// Every current non-sidecar failure has identical `valid: false`
    /// semantics, so the safe default is the one closed rejection identity.
    fn rejection_identity(&self) -> BodyValidationRejectionIdentity {
        BodyValidationRejectionIdentity::Rejected
    }

    /// Return the exact missing sidecar reference when validation should defer.
    fn missing_certified_merge_sidecar(&self) -> Option<&CertifiedMergeLedgerReference> {
        None
    }
}

impl BodyValidationError for String {}

/// Authority whose single block signature must cover an exact proposal body.
///
/// Height-one genesis is signed by the configured genesis authority rather
/// than by the rotating consensus leader.  Keeping that distinction explicit
/// avoids either rejecting a valid genesis body or silently weakening the
/// signature check for ordinary blocks.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum BlockSignaturePolicy {
    /// Require the rotating leader selected by the immutable body-header view.
    RotatingLeader,
    /// Require signature index zero and the configured genesis public key.
    GenesisAuthority(PublicKey),
}

impl ValidatedBodyReceipt {
    /// Durable body receipt whose exact bytes passed validation.
    pub(crate) const fn durable(&self) -> &DurableBodyReceipt {
        &self.durable
    }

    /// Exact deterministic execution result fsynced with the validation marker.
    pub(crate) const fn execution_commitment(&self) -> wire::ExecutionCommitment {
        self.execution_commitment
    }

    #[cfg(test)]
    pub(crate) fn for_test(durable: DurableBodyReceipt) -> Self {
        let empty = Hash::new([]);
        let bind_frame = |domain: &[u8]| {
            let mut preimage = Vec::with_capacity(domain.len() + 1 + Hash::LENGTH);
            preimage.extend_from_slice(domain);
            preimage.push(0);
            preimage.extend_from_slice(durable.frame_hash.as_ref());
            Hash::new(preimage)
        };
        Self {
            execution_commitment: wire::ExecutionCommitment::new_without_merge_carrier(
                bind_frame(b"iroha:sumeragi:v2:test-parent-state-root:v1"),
                bind_frame(b"iroha:sumeragi:v2:test-post-state-root:v1"),
                empty,
                None,
                0,
                1,
                bind_frame(b"iroha:sumeragi:v2:test-executed-block-wire:v1"),
            )
            .expect("test execution commitment is canonical"),
            durable,
        }
    }

    #[cfg(test)]
    pub(crate) const fn for_test_with_commitment(
        durable: DurableBodyReceipt,
        execution_commitment: wire::ExecutionCommitment,
    ) -> Self {
        Self {
            durable,
            execution_commitment,
        }
    }
}

impl DurableBodyReceipt {
    /// Frozen context which owns the body.
    pub(crate) const fn context_id(&self) -> wire::HeightContextId {
        self.context_id
    }

    /// Proposal round which owns the body.
    pub(crate) const fn round(&self) -> wire::ConsensusRound {
        self.round
    }

    /// Exact certified subject represented by the bytes.
    pub(crate) const fn subject(&self) -> wire::BlockSubject {
        self.subject
    }

    /// Hash of the canonical manifest stored beside the body.
    pub(crate) const fn manifest_hash(&self) -> HashOf<wire::PayloadManifest> {
        self.manifest_hash
    }

    /// Hash of the complete checksummed body-store frame.
    ///
    /// This is the lossless 256-bit identity of the bytes acknowledged by the
    /// receipt. Comparisons mediated by this value rely on the repository's
    /// reviewed collision-resistance contract for [`Hash`].
    pub(crate) const fn frame_hash(&self) -> Hash {
        self.frame_hash
    }

    #[cfg(test)]
    pub(crate) fn for_test(
        context_id: wire::HeightContextId,
        round: wire::ConsensusRound,
        subject: wire::BlockSubject,
        manifest_hash: HashOf<wire::PayloadManifest>,
    ) -> Self {
        Self {
            context_id,
            round,
            subject,
            manifest_hash,
            frame_hash: Hash::new(b"test-only durable body receipt"),
        }
    }
}

/// Persistent exact-body store for one immutable height context.
///
/// The validation snapshot is part of that height ownership contract: callers
/// must not change world state or context-bound validation inputs while this
/// store is active. A committed-parent change retires the store and creates a
/// new height context. Validation receipts remain bound to one exact proposal
/// round and are never promoted across views.
pub(crate) struct V2BodyStore {
    context: wire::HeightContext,
    signature_policy: BlockSignaturePolicy,
    directory: PathBuf,
    entries: BTreeMap<(wire::ConsensusRound, wire::BlockSubject), DurableBodyReceipt>,
    manifests: BTreeMap<(wire::ConsensusRound, wire::BlockSubject), wire::PayloadManifest>,
    /// Structurally authenticated restart markers which have not yet crossed
    /// deterministic candidate validation in this process.
    ///
    /// A checksum only detects accidental corruption; it is not authority to
    /// vote. Production preflight must promote every entry through
    /// [`Self::revalidate_recovered_markers`] before constructing the runtime.
    pending_revalidation:
        BTreeMap<(wire::ConsensusRound, wire::BlockSubject), ValidatedBodyReceipt>,
    validated: BTreeMap<(wire::ConsensusRound, wire::BlockSubject), ValidatedBodyReceipt>,
}

/// Immutable post-finality deletion authority for one exact body directory.
///
/// Construction consumes the height-local store only after a matching Kura
/// receipt is available. Executing the plan may block on the filesystem and
/// therefore belongs exclusively to the runner's bounded cleanup worker.
pub(crate) struct V2BodyRetirementJob {
    directory: PathBuf,
    parent: Option<PathBuf>,
}

impl V2BodyRetirementJob {
    /// Delete the finalized height's durable candidate bodies and fsync the
    /// containing directory.
    pub(crate) fn execute(self) -> Result<(), V2BodyStoreError> {
        match fs::remove_dir_all(&self.directory) {
            Ok(()) => {}
            Err(source) if source.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(source) => {
                return Err(V2BodyStoreError::Io {
                    path: self.directory,
                    source,
                });
            }
        }
        if let Some(parent) = self.parent {
            sync_directory(&parent)?;
        }
        Ok(())
    }
}

impl V2BodyStore {
    /// Return whether this already-open store belongs to the exact context.
    ///
    /// Production startup uses this when a durable recovery catalog must be
    /// inspected before the serialized runtime is constructed.
    pub(crate) fn matches_context(&self, context: &wire::HeightContext) -> bool {
        &self.context == context
    }

    /// Open the active context directory and fail closed on every malformed
    /// final body file. Incomplete `.tmp` files are unacknowledged writes and
    /// are deliberately ignored.
    #[cfg(test)]
    pub(crate) fn open(
        root: impl AsRef<Path>,
        context: wire::HeightContext,
    ) -> Result<Self, V2BodyStoreError> {
        Self::open_with_policy(root, context, BlockSignaturePolicy::RotatingLeader)
    }

    /// Open a context directory with an explicit block-signature policy.
    ///
    /// The genesis-authority policy is valid only for the first height with no
    /// parent certificate. All successor heights use rotating-leader signing.
    pub(crate) fn open_with_policy(
        root: impl AsRef<Path>,
        context: wire::HeightContext,
        signature_policy: BlockSignaturePolicy,
    ) -> Result<Self, V2BodyStoreError> {
        context.validate()?;
        if matches!(signature_policy, BlockSignaturePolicy::GenesisAuthority(_))
            && (context.height != 1 || context.parent_commit_qc.is_some())
        {
            return Err(V2BodyStoreError::InvalidSignaturePolicy);
        }
        let directory = root.as_ref().join(hex::encode(context.id().0.as_ref()));
        fs::create_dir_all(&directory).map_err(|source| V2BodyStoreError::Io {
            path: directory.clone(),
            source,
        })?;
        sync_directory(&directory)?;
        if let Some(parent) = directory.parent() {
            sync_directory(parent)?;
        }

        let mut store = Self {
            context,
            signature_policy,
            directory,
            entries: BTreeMap::new(),
            manifests: BTreeMap::new(),
            pending_revalidation: BTreeMap::new(),
            validated: BTreeMap::new(),
        };
        let mut paths = fs::read_dir(&store.directory)
            .map_err(|source| V2BodyStoreError::Io {
                path: store.directory.clone(),
                source,
            })?
            .map(|entry| {
                entry
                    .map(|entry| entry.path())
                    .map_err(|source| V2BodyStoreError::Io {
                        path: store.directory.clone(),
                        source,
                    })
            })
            .collect::<Result<Vec<_>, _>>()?;
        paths.sort();
        for path in paths
            .iter()
            .filter(|path| path.extension().and_then(|value| value.to_str()) == Some("norito"))
        {
            let (envelope, frame_hash) = read_envelope(&path)?;
            store.validate_envelope(&envelope)?;
            let receipt = receipt_for(&envelope, frame_hash);
            let key = (envelope.round, envelope.subject);
            if store.entries.insert(key, receipt).is_some()
                || store.manifests.insert(key, envelope.manifest).is_some()
            {
                return Err(V2BodyStoreError::DuplicateBodyKey);
            }
        }
        for path in paths
            .iter()
            .filter(|path| path.extension().and_then(|value| value.to_str()) == Some("validated"))
        {
            let marker = read_validated_marker(path)?;
            let key = (marker.round, marker.subject);
            let receipt = store
                .entries
                .get(&key)
                .cloned()
                .ok_or(V2BodyStoreError::OrphanedValidationMarker)?;
            store.validate_marker(&marker, &receipt)?;
            store.ensure_execution_commitment_consistent(&receipt, marker.execution_commitment)?;
            if store
                .pending_revalidation
                .insert(
                    key,
                    ValidatedBodyReceipt {
                        durable: receipt,
                        execution_commitment: marker.execution_commitment,
                    },
                )
                .is_some()
            {
                return Err(V2BodyStoreError::DuplicateValidationMarker);
            }
        }
        for path in &paths {
            let name = path
                .file_name()
                .and_then(|value| value.to_str())
                .ok_or_else(|| V2BodyStoreError::UnexpectedEntry(path.clone()))?;
            let extension = path.extension().and_then(|value| value.to_str());
            if name.ends_with(".tmp") || matches!(extension, Some("norito" | "validated")) {
                continue;
            }
            return Err(V2BodyStoreError::UnexpectedEntry(path.clone()));
        }
        Ok(store)
    }

    /// Recover the durable receipt indexed by an exact round and subject.
    ///
    /// Receipts are reconstructed and fully revalidated while opening the
    /// store, so returning one here permits crash recovery without a needless
    /// network fetch.
    #[cfg(test)]
    pub(crate) fn receipt(
        &self,
        round: wire::ConsensusRound,
        subject: wire::BlockSubject,
    ) -> Option<DurableBodyReceipt> {
        self.entries.get(&(round, subject)).cloned()
    }

    /// Reload a recovered body's canonical manifest together with its durable
    /// receipt.
    ///
    /// The manifest is part of the reducer's subject registry. Returning it at
    /// the recovery boundary is essential when a replayed CommitQC requests a
    /// body without having first received the original proposal.
    pub(crate) fn recovered(
        &self,
        round: wire::ConsensusRound,
        subject: wire::BlockSubject,
    ) -> Result<Option<(wire::PayloadManifest, DurableBodyReceipt)>, V2BodyStoreError> {
        let key = (round, subject);
        let Some(receipt) = self.entries.get(&key).cloned() else {
            return Ok(None);
        };
        let manifest = self
            .manifests
            .get(&key)
            .cloned()
            .ok_or(V2BodyStoreError::ReceiptMismatch)?;
        Ok(Some((manifest, receipt)))
    }

    /// Snapshot the in-memory recovery index reconstructed while opening.
    ///
    /// This performs no filesystem I/O and lets the serialized reducer owner
    /// recover local durability while the store itself moves to an asynchronous
    /// storage service.
    pub(crate) fn recovery_catalog(
        &self,
    ) -> Result<
        BTreeMap<
            (wire::ConsensusRound, wire::BlockSubject),
            (wire::PayloadManifest, DurableBodyReceipt),
        >,
        V2BodyStoreError,
    > {
        let mut catalog = BTreeMap::new();
        for (key, receipt) in &self.entries {
            let manifest = self
                .manifests
                .get(key)
                .cloned()
                .ok_or(V2BodyStoreError::ReceiptMismatch)?;
            catalog.insert(*key, (manifest, receipt.clone()));
        }
        Ok(catalog)
    }

    /// Re-run deterministic validation for every marker recovered from disk.
    ///
    /// Structurally valid marker bytes are deliberately quarantined while the
    /// store opens. This method promotes them atomically only after the exact
    /// durable bodies reproduce the persisted execution commitments. A typed
    /// missing-certified-sidecar result retires the affected marker authority
    /// without promoting it; the exact durable body remains available to the
    /// ordinary bounded validation and sidecar-fetch pipeline. Every other
    /// validation failure remains terminal. Bodies shared by several proposal
    /// rounds are executed once because validation consumes the signed body and
    /// immutable height context, not the manifest round; every round-local
    /// marker remains checked against that result.
    pub(crate) fn revalidate_recovered_markers<F, E>(
        &mut self,
        mut validator: F,
    ) -> Result<(), V2BodyStoreError>
    where
        F: FnMut(&SignedBlock) -> Result<wire::ExecutionCommitment, E>,
        E: BodyValidationError,
    {
        if self.pending_revalidation.is_empty() {
            return Ok(());
        }

        let mut commitments = BTreeMap::<wire::BlockSubject, wire::ExecutionCommitment>::new();
        let mut retired_missing_sidecar_subjects = BTreeSet::new();
        let mut promoted = BTreeMap::new();
        for (key, recovered) in &self.pending_revalidation {
            let receipt = self
                .entries
                .get(key)
                .ok_or(V2BodyStoreError::OrphanedValidationMarker)?;
            if recovered.durable() != receipt {
                return Err(V2BodyStoreError::ValidationMarkerMismatch);
            }
            if retired_missing_sidecar_subjects.contains(&key.1) {
                continue;
            }
            let execution_commitment = if let Some(commitment) = commitments.get(&key.1) {
                *commitment
            } else {
                let body = self.load(receipt)?;
                let commitment = match validator(&body) {
                    Ok(commitment) => commitment,
                    Err(error) if error.missing_certified_merge_sidecar().is_some() => {
                        retired_missing_sidecar_subjects.insert(key.1);
                        continue;
                    }
                    Err(error) => {
                        return Err(V2BodyStoreError::RecoveredValidationRejected(
                            error.to_string(),
                        ));
                    }
                };
                commitment.validate()?;
                commitments.insert(key.1, commitment);
                commitment
            };
            if execution_commitment != recovered.execution_commitment() {
                return Err(V2BodyStoreError::RecoveredValidationCommitmentMismatch);
            }
            promoted.insert(*key, recovered.clone());
        }

        self.validated.extend(promoted);
        self.pending_revalidation.clear();
        Ok(())
    }

    /// Retire restart vote authority for bodies other than a verified decision.
    ///
    /// Once Kura has a cryptographically verified finality artifact, losing
    /// candidates cannot be re-executed against the now-advanced world state
    /// and must never recover height-local vote authority. Their durable body
    /// bytes remain available for bounded cleanup; their quarantined and
    /// already-promoted marker capabilities are dropped from the in-memory
    /// recovery catalogs.
    pub(crate) fn retain_recovered_markers_for_subject(
        &mut self,
        decision: VerifiedRecoveredFinalitySubject,
    ) -> Result<(), V2BodyStoreError> {
        if !decision.authorizes_context(&self.context) {
            return Err(V2BodyStoreError::RecoveredFinalityContextMismatch);
        }
        let subject = decision.subject();
        self.pending_revalidation
            .retain(|(_, candidate), _| *candidate == subject);
        self.validated
            .retain(|(_, candidate), _| *candidate == subject);
        Ok(())
    }

    /// Retain only marker authority named by authenticated WAL replay.
    ///
    /// Superseded view-local markers remain on disk as checksummed diagnostics,
    /// and their exact bodies remain available for certified serving or later
    /// bounded validation. They are excluded from synchronous semantic replay
    /// and cannot restore vote authority unless the live runtime validates the
    /// body again.
    pub(crate) fn retain_recovered_markers_for_authority(
        &mut self,
        authority: RecoveredValidationAuthority,
    ) -> Result<(), V2BodyStoreError> {
        if !authority.authorizes_context(&self.context) {
            return Err(V2BodyStoreError::RecoveredValidationAuthorityContextMismatch);
        }
        self.pending_revalidation
            .retain(|(round, subject), _| authority.authorizes(*round, *subject));
        self.validated
            .retain(|(round, subject), _| authority.authorizes(*round, *subject));
        Ok(())
    }

    /// Require all restart markers to have crossed semantic revalidation.
    ///
    /// The serialized runtime calls this before restoring vote authority so a
    /// caller cannot accidentally treat a checksummed local file as a trusted
    /// validation receipt.
    pub(crate) fn ensure_recovered_markers_revalidated(&self) -> Result<(), V2BodyStoreError> {
        if self.pending_revalidation.is_empty() {
            Ok(())
        } else {
            Err(V2BodyStoreError::UnrevalidatedValidationMarkers)
        }
    }

    /// Snapshot semantically revalidated recovery receipts.
    pub(crate) fn validated_recovery_catalog(
        &self,
    ) -> BTreeMap<(wire::ConsensusRound, wire::BlockSubject), ValidatedBodyReceipt> {
        self.validated.clone()
    }

    /// Execute one exact-body persistence task on a storage-service thread.
    ///
    /// Only this method can mint [`BodyStoreCompletion`], and it does so only
    /// after [`Self::store`] has completed file and directory synchronisation.
    pub(crate) fn execute_store_task(
        &mut self,
        task: &BodyStoreTask,
    ) -> Result<BodyStoreCompletion, V2BodyStoreError> {
        let receipt = self.store(task.manifest().clone(), task.canonical_wire().to_vec())?;
        Ok(BodyStoreCompletion {
            work_id: task.id(),
            tag: task.tag(),
            manifest: task.manifest().clone(),
            receipt,
        })
    }

    // DURABLE_BODY_VALIDATION_API_BEGIN
    /// Execute deterministic validation against one exact durable body.
    ///
    /// The independently supplied manifest hash is checked against both the
    /// receipt and the store's canonical manifest before the callback can run.
    /// The complete checksummed frame is then reloaded, structurally validated,
    /// and decoded.  A success result is minted only after its validation
    /// marker has crossed the file-and-directory durability boundary.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn execute_durable_validation<F, E>(
        &mut self,
        durable: DurableBodyReceipt,
        expected_manifest_hash: HashOf<wire::PayloadManifest>,
        validator: F,
    ) -> Result<DurableBodyValidationOutcome, V2BodyStoreError>
    where
        F: FnOnce(&SignedBlock) -> Result<wire::ExecutionCommitment, E>,
        E: BodyValidationError,
    {
        self.verify_receipt(&durable)?;
        let key = (durable.round(), durable.subject());
        let stored_manifest = self
            .manifests
            .get(&key)
            .ok_or(V2BodyStoreError::ReceiptMismatch)?;
        if durable.context_id() != self.context.id()
            || durable.round().context_id != durable.context_id()
            || durable.round().height != self.context.height
            || stored_manifest.round != durable.round()
            || stored_manifest.subject != durable.subject()
            || durable.manifest_hash() != expected_manifest_hash
            || HashOf::new(stored_manifest) != expected_manifest_hash
        {
            return Err(V2BodyStoreError::ReceiptMismatch);
        }

        let envelope = self.load_envelope(&durable)?;
        if envelope.context_id != durable.context_id()
            || envelope.round != durable.round()
            || envelope.subject != durable.subject()
            || &envelope.manifest != stored_manifest
            || HashOf::new(&envelope.manifest) != expected_manifest_hash
        {
            return Err(V2BodyStoreError::ReceiptMismatch);
        }
        let block = decode_framed_signed_block(&envelope.canonical_wire)
            .map_err(|error| V2BodyStoreError::BlockDecode(error.to_string()))?;

        if let Some(validated) = self.validated.get(&key) {
            if validated.durable() != &durable {
                return Err(V2BodyStoreError::ReceiptMismatch);
            }
            return Ok(DurableBodyValidationOutcome(
                DurableBodyValidationOutcomeBody::Validated(validated.clone()),
            ));
        }

        match validator(&block) {
            Ok(execution_commitment) => {
                let validated = self.persist_validated_receipt(&durable, execution_commitment)?;
                Ok(DurableBodyValidationOutcome(
                    DurableBodyValidationOutcomeBody::Validated(validated),
                ))
            }
            Err(error) => {
                if let Some(reference) = error.missing_certified_merge_sidecar() {
                    return Ok(DurableBodyValidationOutcome(
                        DurableBodyValidationOutcomeBody::DeferredMergeSidecar {
                            durable,
                            reference: reference.clone(),
                        },
                    ));
                }
                Ok(DurableBodyValidationOutcome(
                    DurableBodyValidationOutcomeBody::Rejected {
                        durable,
                        identity: error.rejection_identity(),
                        reason: error.to_string(),
                    },
                ))
            }
        }
    }
    // DURABLE_BODY_VALIDATION_API_END

    /// Execute deterministic validation against the exact durable task body.
    ///
    /// Filesystem loading, canonical decoding, and the validator callback all
    /// run in the caller's storage/validation service context, never on the
    /// serialized reducer owner. Production callers use
    /// `ValidBlock::validate_sumeragi_v2_candidate_keep_voting_block` so the
    /// store-verified proposal-round signature is not redundantly rechecked
    /// while all transaction/state checks remain. Validation success is bound
    /// to the exact durable proposal round; a marker from another view cannot
    /// authorize this task. Final application re-executes and checks the
    /// commitment.
    pub(crate) fn execute_validation_task<F, E>(
        &mut self,
        task: &BodyValidationTask,
        validator: F,
    ) -> Result<BodyValidationCompletion, V2BodyStoreError>
    where
        F: FnOnce(&SignedBlock) -> Result<wire::ExecutionCommitment, E>,
        E: BodyValidationError,
    {
        if task.round() != task.durable_receipt().round()
            || task.subject() != task.durable_receipt().subject()
        {
            return Err(V2BodyStoreError::ReceiptMismatch);
        }
        let work_id = task.id();
        let outcome = self.execute_durable_validation(
            task.durable_receipt().clone(),
            task.durable_receipt().manifest_hash(),
            validator,
        )?;
        match outcome.into_body() {
            DurableBodyValidationOutcomeBody::Validated(receipt) => {
                Ok(BodyValidationCompletion::Validated { work_id, receipt })
            }
            DurableBodyValidationOutcomeBody::Rejected { reason, .. } => {
                Ok(BodyValidationCompletion::Rejected { work_id, reason })
            }
            DurableBodyValidationOutcomeBody::DeferredMergeSidecar { reference, .. } => {
                Ok(BodyValidationCompletion::DeferredMergeSidecar { work_id, reference })
            }
        }
    }

    /// Durably persist the exact body carried by an authenticated certified
    /// Fetch response and bind its complete transport occurrence.
    ///
    /// The canonical [`Self::store`] path validates the response manifest and
    /// body against this store's immutable context and returns only after a new
    /// file and its directory entry have been synchronised. Exact repeats reuse
    /// that already durable frame; a different body for the same round and
    /// subject fails closed under the existing store contract.
    // TODO: Move the blocking I/O to the bounded storage worker and consume
    // this sealed receipt in the future composite Fetch-to-Store transaction.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn persist_authenticated_certified_fetch_response(
        &mut self,
        authenticated: &AuthenticatedCertifiedBodyResponse,
    ) -> Result<DurableCertifiedFetchBodyReceipt, V2BodyStoreError> {
        let response = authenticated.response();
        let request_hash = response.request_hash;
        let response_hash = HashOf::new(response);
        let round = response.manifest.round;
        let subject = response.manifest.subject;
        let context_id = round.context_id;
        let manifest_hash = HashOf::new(&response.manifest);
        let durable_body = self.store(response.manifest.clone(), response.body.clone())?;

        if durable_body.context_id() != context_id
            || durable_body.round() != round
            || durable_body.subject() != subject
            || durable_body.manifest_hash() != manifest_hash
        {
            return Err(V2BodyStoreError::ReceiptMismatch);
        }

        Ok(DurableCertifiedFetchBodyReceipt {
            request_hash,
            response_hash,
            durable_body,
        })
    }

    /// Validate and durably store canonical `SignedBlockWire` bytes.
    ///
    /// An identical body is idempotent. A different final file for the same
    /// round and subject fails closed instead of being replaced.
    pub(crate) fn store(
        &mut self,
        manifest: wire::PayloadManifest,
        canonical_wire: Vec<u8>,
    ) -> Result<DurableBodyReceipt, V2BodyStoreError> {
        let envelope = StoredBodyEnvelope {
            version: STORE_VERSION,
            context_id: self.context.id(),
            round: manifest.round,
            subject: manifest.subject,
            manifest,
            canonical_wire,
        };
        self.validate_envelope(&envelope)?;

        let key = (envelope.round, envelope.subject);
        if let Some(existing) = self.entries.get(&key) {
            let existing_envelope = self.load_envelope(existing)?;
            if existing_envelope == envelope {
                return Ok(existing.clone());
            }
            return Err(V2BodyStoreError::ConflictingBody);
        }

        let encoded = envelope.encode();
        let framed = frame_payload(&encoded)?;
        let frame_hash = Hash::new(&framed);
        let path = self.path_for(envelope.round, envelope.subject);
        write_atomic_synced(&path, &framed)?;
        let receipt = receipt_for(&envelope, frame_hash);
        self.entries.insert(key, receipt.clone());
        self.manifests.insert(key, envelope.manifest);
        Ok(receipt)
    }

    /// Load and revalidate the exact block represented by a durable receipt.
    pub(crate) fn load(
        &self,
        receipt: &DurableBodyReceipt,
    ) -> Result<SignedBlock, V2BodyStoreError> {
        let envelope = self.load_envelope(receipt)?;
        decode_framed_signed_block(&envelope.canonical_wire)
            .map_err(|error| V2BodyStoreError::BlockDecode(error.to_string()))
    }

    /// Load the exact canonical `SignedBlockWire` bytes represented by a
    /// durable receipt.
    ///
    /// Certified fetch responses and exact locked-subject recovery must
    /// preserve byte identity. Re-encoding a decoded block at those boundaries
    /// would be an unnecessary second source of truth, so callers receive the
    /// checksummed bytes from the final store frame itself.
    pub(crate) fn load_canonical_wire(
        &self,
        receipt: &DurableBodyReceipt,
    ) -> Result<Vec<u8>, V2BodyStoreError> {
        Ok(self.load_envelope(receipt)?.canonical_wire)
    }

    /// Find the newest durable proposal round retaining one exact subject.
    ///
    /// Locked-candidate recovery uses this lookup to find the retained body for
    /// the exact subject. The BTreeMap order makes selection deterministic
    /// across restart, while the returned receipt still has to pass the normal
    /// frame checks before bytes can be loaded.
    pub(crate) fn latest_for_subject(
        &self,
        subject: wire::BlockSubject,
    ) -> Result<Option<(wire::PayloadManifest, DurableBodyReceipt)>, V2BodyStoreError> {
        let selected = self
            .entries
            .iter()
            .rev()
            .find(|((_, stored_subject), _)| *stored_subject == subject)
            .map(|(key, receipt)| (*key, receipt.clone()));
        let Some((key, receipt)) = selected else {
            return Ok(None);
        };
        self.verify_receipt(&receipt)?;
        let manifest = self
            .manifests
            .get(&key)
            .cloned()
            .ok_or(V2BodyStoreError::ReceiptMismatch)?;
        Ok(Some((manifest, receipt)))
    }

    /// Test helper that runs a validator synchronously over the exact durable
    /// body and persists the same marker used by the production task API.
    #[cfg(test)]
    pub(crate) fn validate<F, E>(
        &mut self,
        receipt: &DurableBodyReceipt,
        validator: F,
    ) -> Result<ValidatedBodyReceipt, V2BodyStoreError>
    where
        F: FnOnce(&SignedBlock) -> Result<wire::ExecutionCommitment, E>,
        E: std::fmt::Display,
    {
        let key = (receipt.round, receipt.subject);
        if let Some(validated) = self.validated.get(&key) {
            if validated.durable() != receipt {
                return Err(V2BodyStoreError::ReceiptMismatch);
            }
            return Ok(validated.clone());
        }
        let block = self.load(receipt)?;
        let execution_commitment = validator(&block)
            .map_err(|error| V2BodyStoreError::DeterministicValidation(error.to_string()))?;
        self.persist_validated_receipt(receipt, execution_commitment)
    }

    fn persist_validated_receipt(
        &mut self,
        receipt: &DurableBodyReceipt,
        execution_commitment: wire::ExecutionCommitment,
    ) -> Result<ValidatedBodyReceipt, V2BodyStoreError> {
        execution_commitment.validate()?;
        self.ensure_execution_commitment_consistent(receipt, execution_commitment)?;
        let key = (receipt.round, receipt.subject);
        if let Some(validated) = self.validated.get(&key) {
            if validated.durable() != receipt
                || validated.execution_commitment() != execution_commitment
            {
                return Err(V2BodyStoreError::ReceiptMismatch);
            }
            return Ok(validated.clone());
        }
        if let Some(recovered) = self.pending_revalidation.get(&key).cloned() {
            if recovered.durable() != receipt {
                return Err(V2BodyStoreError::ReceiptMismatch);
            }
            if recovered.execution_commitment() != execution_commitment {
                return Err(V2BodyStoreError::RecoveredValidationCommitmentMismatch);
            }
            self.pending_revalidation.remove(&key);
            self.validated.insert(key, recovered.clone());
            return Ok(recovered);
        }
        let validated = ValidatedBodyReceipt {
            durable: receipt.clone(),
            execution_commitment,
        };
        let marker = ValidatedBodyMarker {
            version: STORE_VERSION,
            context_id: receipt.context_id,
            round: receipt.round,
            subject: receipt.subject,
            manifest_hash: receipt.manifest_hash,
            body_frame_hash: receipt.frame_hash,
            execution_commitment,
        };
        write_validated_marker(
            &self.validated_path_for(receipt.round, receipt.subject),
            &marker,
        )?;
        self.validated.insert(key, validated.clone());
        Ok(validated)
    }

    fn ensure_execution_commitment_consistent(
        &self,
        receipt: &DurableBodyReceipt,
        execution_commitment: wire::ExecutionCommitment,
    ) -> Result<(), V2BodyStoreError> {
        let conflicts = self.validated.iter().any(|((round, subject), validated)| {
            *subject == receipt.subject
                && round.context_id == receipt.round.context_id
                && round.height == receipt.round.height
                && validated.execution_commitment() != execution_commitment
        }) || self.pending_revalidation.iter().any(
            |((round, subject), validated)| {
                *subject == receipt.subject
                    && round.context_id == receipt.round.context_id
                    && round.height == receipt.round.height
                    && validated.execution_commitment() != execution_commitment
            },
        );
        if conflicts {
            return Err(V2BodyStoreError::ConflictingValidationCommitment);
        }
        Ok(())
    }

    /// Retire every losing and decided candidate after Kura durably finalizes
    /// the owning height context.
    ///
    /// Once a height has one immutable CommitQC artifact, no candidate body at
    /// that height can be voted on again. Context/height matching is therefore
    /// the authorization boundary for deleting the complete directory; only
    /// the decided candidate additionally matches the receipt's block hash.
    pub(crate) fn into_retirement_job(
        self,
        kura_receipt: &KuraV2CommitReceipt,
    ) -> Result<V2BodyRetirementJob, V2BodyStoreError> {
        if kura_receipt.context_id() != self.context.id()
            || kura_receipt.height() != self.context.height
        {
            return Err(V2BodyStoreError::KuraReceiptMismatch);
        }
        Ok(V2BodyRetirementJob {
            parent: self.directory.parent().map(Path::to_path_buf),
            directory: self.directory,
        })
    }

    fn load_envelope(
        &self,
        receipt: &DurableBodyReceipt,
    ) -> Result<StoredBodyEnvelope, V2BodyStoreError> {
        self.verify_receipt(receipt)?;
        let path = self.path_for(receipt.round, receipt.subject);
        let (envelope, frame_hash) = read_envelope(&path)?;
        if frame_hash != receipt.frame_hash {
            return Err(V2BodyStoreError::ReceiptMismatch);
        }
        self.validate_envelope(&envelope)?;
        if receipt_for(&envelope, frame_hash) != *receipt {
            return Err(V2BodyStoreError::ReceiptMismatch);
        }
        Ok(envelope)
    }

    fn verify_receipt(&self, receipt: &DurableBodyReceipt) -> Result<(), V2BodyStoreError> {
        if receipt.context_id != self.context.id()
            || receipt.round.context_id != receipt.context_id
            || receipt.round.height != self.context.height
            || self
                .entries
                .get(&(receipt.round, receipt.subject))
                .is_none_or(|known| known != receipt)
        {
            return Err(V2BodyStoreError::ReceiptMismatch);
        }
        Ok(())
    }

    fn validate_envelope(&self, envelope: &StoredBodyEnvelope) -> Result<(), V2BodyStoreError> {
        if envelope.version != STORE_VERSION {
            return Err(V2BodyStoreError::UnsupportedVersion(envelope.version));
        }
        if envelope.context_id != self.context.id()
            || envelope.round.context_id != envelope.context_id
            || envelope.round.height != self.context.height
            || envelope.manifest.round != envelope.round
            || envelope.manifest.subject != envelope.subject
        {
            return Err(V2BodyStoreError::ContextMismatch);
        }
        envelope.manifest.validate(&self.context)?;
        let body_len = u64::try_from(envelope.canonical_wire.len())
            .map_err(|_| V2BodyStoreError::BodyTooLarge)?;
        if body_len != envelope.manifest.payload_size_bytes {
            return Err(V2BodyStoreError::BodyLengthMismatch);
        }
        if Hash::new(&envelope.canonical_wire) != envelope.subject.payload_hash {
            return Err(V2BodyStoreError::PayloadHashMismatch);
        }

        let block = decode_framed_signed_block(&envelope.canonical_wire)
            .map_err(|error| V2BodyStoreError::BlockDecode(error.to_string()))?;
        if !block.is_resultless_proposal() {
            return Err(V2BodyStoreError::ResultBearingProposal);
        }
        let reencoded = block
            .encode_wire()
            .map_err(|error| V2BodyStoreError::BlockEncode(error.to_string()))?;
        if reencoded != envelope.canonical_wire {
            return Err(V2BodyStoreError::NonCanonicalBlockWire);
        }
        let header = block.header();
        let body_origin_view = header.view_change_index();
        let view_matches = match &self.signature_policy {
            // An unchanged locked body keeps its original header and signature
            // when a later leader re-proposes it. Authenticate that original
            // leader and forbid only bodies originating in a future view.
            BlockSignaturePolicy::RotatingLeader => body_origin_view <= envelope.round.view,
            // Genesis is the sole exception: height one retains the fixed
            // authority-signed view-zero body across certified view changes.
            // `open_with_policy` separately rejects this policy outside an
            // unparented height-one context.
            BlockSignaturePolicy::GenesisAuthority(_) => {
                header.height().get() == 1 && body_origin_view == 0
            }
        };
        if header.height().get() != envelope.round.height
            || !view_matches
            || block.hash() != envelope.subject.block_hash
            || header.prev_block_hash() != envelope.subject.parent_block_hash
        {
            return Err(V2BodyStoreError::BlockSubjectMismatch);
        }
        let expected_parent = self
            .context
            .parent_commit_qc
            .as_ref()
            .map(|certificate| certificate.subject.block_hash)
            .or_else(|| {
                self.context
                    .snapshot_bootstrap
                    .as_ref()
                    .map(|anchor| anchor.snapshot_block_hash)
            });
        if header.prev_block_hash() != expected_parent {
            return Err(V2BodyStoreError::ParentMismatch);
        }

        let (expected_index, expected_key) = match &self.signature_policy {
            BlockSignaturePolicy::RotatingLeader => {
                let leader = self.context.leader(body_origin_view);
                let leader_index =
                    usize::try_from(leader).map_err(|_| V2BodyStoreError::LeaderOutOfRange)?;
                let leader_key = self
                    .context
                    .roster
                    .get(leader_index)
                    .ok_or(V2BodyStoreError::LeaderOutOfRange)?
                    .validator
                    .public_key();
                (u64::from(leader), leader_key)
            }
            BlockSignaturePolicy::GenesisAuthority(public_key) => (0, public_key),
        };
        let mut signatures = block.signatures();
        let signature = signatures
            .next()
            .ok_or(V2BodyStoreError::MissingExpectedSignature)?;
        if signatures.next().is_some() || signature.index() != expected_index {
            return Err(V2BodyStoreError::InvalidExpectedSignatureSet);
        }
        signature
            .signature()
            .verify_hash(expected_key, block.hash())
            .map_err(|_| V2BodyStoreError::InvalidExpectedSignature)?;
        Ok(())
    }

    fn validate_marker(
        &self,
        marker: &ValidatedBodyMarker,
        receipt: &DurableBodyReceipt,
    ) -> Result<(), V2BodyStoreError> {
        if marker.version != STORE_VERSION {
            return Err(V2BodyStoreError::UnsupportedVersion(marker.version));
        }
        if marker.context_id != self.context.id()
            || marker.round.context_id != marker.context_id
            || marker.round.height != self.context.height
            || marker.context_id != receipt.context_id
            || marker.round != receipt.round
            || marker.subject != receipt.subject
            || marker.manifest_hash != receipt.manifest_hash
            || marker.body_frame_hash != receipt.frame_hash
        {
            return Err(V2BodyStoreError::ValidationMarkerMismatch);
        }
        marker.execution_commitment.validate()?;
        Ok(())
    }

    fn path_for(&self, round: wire::ConsensusRound, subject: wire::BlockSubject) -> PathBuf {
        let key_hash = Hash::new((round, subject).encode());
        self.directory.join(format!(
            "{:020}-{:020}-{}.norito",
            round.height,
            round.view,
            hex::encode(key_hash.as_ref())
        ))
    }

    fn validated_path_for(
        &self,
        round: wire::ConsensusRound,
        subject: wire::BlockSubject,
    ) -> PathBuf {
        self.path_for(round, subject).with_extension("validated")
    }
}

fn receipt_for(envelope: &StoredBodyEnvelope, frame_hash: Hash) -> DurableBodyReceipt {
    DurableBodyReceipt {
        context_id: envelope.context_id,
        round: envelope.round,
        subject: envelope.subject,
        manifest_hash: HashOf::new(&envelope.manifest),
        frame_hash,
    }
}

fn frame_payload(payload: &[u8]) -> Result<Vec<u8>, V2BodyStoreError> {
    frame_payload_with_magic(STORE_MAGIC, payload)
}

fn frame_payload_with_magic(magic: &[u8; 8], payload: &[u8]) -> Result<Vec<u8>, V2BodyStoreError> {
    let payload_len = u64::try_from(payload.len()).map_err(|_| V2BodyStoreError::BodyTooLarge)?;
    let capacity = FRAME_HEADER_LEN
        .checked_add(payload.len())
        .and_then(|length| length.checked_add(CHECKSUM_LEN))
        .ok_or(V2BodyStoreError::BodyTooLarge)?;
    let mut frame = Vec::with_capacity(capacity);
    frame.extend_from_slice(magic);
    frame.extend_from_slice(&STORE_VERSION.to_le_bytes());
    frame.extend_from_slice(&payload_len.to_le_bytes());
    frame.extend_from_slice(payload);
    frame.extend_from_slice(Hash::new(payload).as_ref());
    Ok(frame)
}

fn read_envelope(path: &Path) -> Result<(StoredBodyEnvelope, Hash), V2BodyStoreError> {
    let (payload, frame_hash) = read_frame_payload_with_hash(path, STORE_MAGIC)?;
    let mut cursor = payload.as_slice();
    let envelope = StoredBodyEnvelope::decode_all(&mut cursor)
        .map_err(|error| V2BodyStoreError::EnvelopeDecode(error.to_string()))?;
    Ok((envelope, frame_hash))
}

fn read_frame_payload(path: &Path, magic: &[u8; 8]) -> Result<Vec<u8>, V2BodyStoreError> {
    read_frame_payload_with_hash(path, magic).map(|(payload, _)| payload)
}

fn read_frame_payload_with_hash(
    path: &Path,
    magic: &[u8; 8],
) -> Result<(Vec<u8>, Hash), V2BodyStoreError> {
    let mut file = File::open(path).map_err(|source| V2BodyStoreError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    let mut frame = Vec::new();
    file.read_to_end(&mut frame)
        .map_err(|source| V2BodyStoreError::Io {
            path: path.to_path_buf(),
            source,
        })?;
    if frame.len() < FRAME_HEADER_LEN + CHECKSUM_LEN || &frame[..magic.len()] != magic {
        return Err(V2BodyStoreError::CorruptFrame);
    }
    let version_offset = magic.len();
    let version = u16::from_le_bytes(
        frame[version_offset..version_offset + size_of::<u16>()]
            .try_into()
            .map_err(|_| V2BodyStoreError::CorruptFrame)?,
    );
    if version != STORE_VERSION {
        return Err(V2BodyStoreError::UnsupportedVersion(version));
    }
    let length_offset = version_offset + size_of::<u16>();
    let payload_len = u64::from_le_bytes(
        frame[length_offset..length_offset + size_of::<u64>()]
            .try_into()
            .map_err(|_| V2BodyStoreError::CorruptFrame)?,
    );
    let payload_len = usize::try_from(payload_len).map_err(|_| V2BodyStoreError::BodyTooLarge)?;
    let expected_len = FRAME_HEADER_LEN
        .checked_add(payload_len)
        .and_then(|length| length.checked_add(CHECKSUM_LEN))
        .ok_or(V2BodyStoreError::BodyTooLarge)?;
    if frame.len() != expected_len {
        return Err(V2BodyStoreError::CorruptFrame);
    }
    let payload = frame[FRAME_HEADER_LEN..FRAME_HEADER_LEN + payload_len].to_vec();
    let checksum = &frame[FRAME_HEADER_LEN + payload_len..];
    if Hash::new(&payload).as_ref().as_slice() != checksum {
        return Err(V2BodyStoreError::ChecksumMismatch);
    }
    Ok((payload, Hash::new(&frame)))
}

fn write_validated_marker(
    path: &Path,
    marker: &ValidatedBodyMarker,
) -> Result<(), V2BodyStoreError> {
    let payload = marker.encode();
    let frame = frame_payload_with_magic(VALIDATED_MAGIC, &payload)?;
    write_atomic_synced(path, &frame)
}

fn read_validated_marker(path: &Path) -> Result<ValidatedBodyMarker, V2BodyStoreError> {
    let payload = read_frame_payload(path, VALIDATED_MAGIC)?;
    let mut cursor = payload.as_slice();
    ValidatedBodyMarker::decode_all(&mut cursor)
        .map_err(|error| V2BodyStoreError::ValidationMarkerDecode(error.to_string()))
}

fn write_atomic_synced(path: &Path, bytes: &[u8]) -> Result<(), V2BodyStoreError> {
    let tmp_path = path.with_extension(
        path.extension()
            .and_then(|extension| extension.to_str())
            .map_or_else(|| "tmp".to_owned(), |extension| format!("{extension}.tmp")),
    );
    let mut file = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(&tmp_path)
        .map_err(|source| V2BodyStoreError::Io {
            path: tmp_path.clone(),
            source,
        })?;
    file.write_all(bytes)
        .and_then(|()| file.flush())
        .and_then(|()| file.sync_all())
        .map_err(|source| V2BodyStoreError::Io {
            path: tmp_path.clone(),
            source,
        })?;
    fs::rename(&tmp_path, path).map_err(|source| V2BodyStoreError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    let parent = path.parent().ok_or(V2BodyStoreError::MissingParent)?;
    sync_directory(parent)
}

fn sync_directory(path: &Path) -> Result<(), V2BodyStoreError> {
    File::open(path)
        .and_then(|directory| directory.sync_all())
        .map_err(|source| V2BodyStoreError::Io {
            path: path.to_path_buf(),
            source,
        })
}

/// Exact-body persistence or validation failure.
#[derive(Debug, Error)]
pub(crate) enum V2BodyStoreError {
    /// Filesystem operation failed.
    #[error("Sumeragi v2 body-store I/O failed at {path}: {source}")]
    Io {
        /// Affected path.
        path: PathBuf,
        /// Underlying error.
        #[source]
        source: std::io::Error,
    },
    /// Frozen height context is structurally invalid.
    #[error("invalid Sumeragi v2 body-store height context: {0}")]
    Context(#[from] wire::ValidationError),
    /// Stored file uses an unsupported framing or envelope version.
    #[error("unsupported Sumeragi v2 body-store version {0}")]
    UnsupportedVersion(u16),
    /// Stored file framing, length, or trailing bytes are invalid.
    #[error("corrupt Sumeragi v2 body-store frame")]
    CorruptFrame,
    /// Stored frame checksum does not match its payload.
    #[error("Sumeragi v2 body-store checksum mismatch")]
    ChecksumMismatch,
    /// Norito envelope could not be decoded exactly.
    #[error("invalid Sumeragi v2 body-store envelope: {0}")]
    EnvelopeDecode(String),
    /// Durable validation marker could not be decoded exactly.
    #[error("invalid Sumeragi v2 validation marker: {0}")]
    ValidationMarkerDecode(String),
    /// Stored metadata does not belong to the active context.
    #[error("Sumeragi v2 body-store context, round, subject, or manifest mismatch")]
    ContextMismatch,
    /// Canonical body size cannot be represented safely.
    #[error("Sumeragi v2 body is too large")]
    BodyTooLarge,
    /// Manifest size differs from the exact wire byte length.
    #[error("Sumeragi v2 body length differs from its manifest")]
    BodyLengthMismatch,
    /// Exact wire byte hash differs from the proposal subject.
    #[error("Sumeragi v2 body payload hash mismatch")]
    PayloadHashMismatch,
    /// Exact body could not be decoded as `SignedBlockWire`.
    #[error("invalid canonical Sumeragi v2 block body: {0}")]
    BlockDecode(String),
    /// Decoded body could not be re-encoded canonically.
    #[error("failed to encode canonical Sumeragi v2 block body: {0}")]
    BlockEncode(String),
    /// Accepted bytes were not the unique canonical encoding.
    #[error("non-canonical Sumeragi v2 block wire bytes")]
    NonCanonicalBlockWire,
    /// Proposal ingress contained execution results or a result-root commitment.
    #[error("Sumeragi v2 proposal body must be resultless")]
    ResultBearingProposal,
    /// Header height/view/hash/parent differs from the proposal subject.
    #[error("Sumeragi v2 block header differs from its proposal subject")]
    BlockSubjectMismatch,
    /// Header parent differs from the frozen parent CommitQC.
    #[error("Sumeragi v2 block parent differs from the frozen height context")]
    ParentMismatch,
    /// Frozen leader index is not representable in the roster.
    #[error("Sumeragi v2 leader is outside the frozen roster")]
    LeaderOutOfRange,
    /// Selected signature policy is incompatible with the frozen height.
    #[error("Sumeragi v2 genesis signature policy is valid only at height one without a parent")]
    InvalidSignaturePolicy,
    /// Body contains no expected block signature.
    #[error("Sumeragi v2 body is missing its expected block signature")]
    MissingExpectedSignature,
    /// Body contains the wrong signature index or more than one block signature.
    #[error("Sumeragi v2 body must contain exactly its expected block signature")]
    InvalidExpectedSignatureSet,
    /// Expected block signature is cryptographically invalid.
    #[error("invalid Sumeragi v2 block signature")]
    InvalidExpectedSignature,
    /// Test-only synchronous deterministic validation rejected the body.
    #[cfg(test)]
    #[error("deterministic Sumeragi v2 body validation failed: {0}")]
    DeterministicValidation(String),
    /// Two final files map to one semantic round/subject key.
    #[error("duplicate Sumeragi v2 durable body key")]
    DuplicateBodyKey,
    /// More than one durable validation marker maps to a semantic body key.
    #[error("duplicate Sumeragi v2 durable validation marker")]
    DuplicateValidationMarker,
    /// Validation marker has no matching exact durable body.
    #[error("orphaned Sumeragi v2 validation marker")]
    OrphanedValidationMarker,
    /// Byte-identical bodies in one height context produced different execution commitments.
    #[error("conflicting Sumeragi v2 execution commitments for one exact body")]
    ConflictingValidationCommitment,
    /// Validation marker is not bound to the matching exact body frame.
    #[error("Sumeragi v2 validation marker differs from its durable body")]
    ValidationMarkerMismatch,
    /// Recovered marker was not accepted by current deterministic validation.
    #[error("recovered Sumeragi v2 validation marker failed semantic replay: {0}")]
    RecoveredValidationRejected(String),
    /// Recovered marker commitment differs from deterministic replay.
    #[error("recovered Sumeragi v2 validation commitment differs from semantic replay")]
    RecoveredValidationCommitmentMismatch,
    /// Verified finality capability belongs to a different height context.
    #[error("verified Sumeragi v2 recovery finality belongs to a different height context")]
    RecoveredFinalityContextMismatch,
    /// WAL replay authority belongs to a different immutable height context.
    #[error("recovered Sumeragi v2 validation authority belongs to a different height context")]
    RecoveredValidationAuthorityContextMismatch,
    /// Runtime construction attempted to restore unvalidated local markers.
    #[error("recovered Sumeragi v2 validation markers require semantic replay")]
    UnrevalidatedValidationMarkers,
    /// Context directory contains an unrecognized final entry.
    #[error("unexpected Sumeragi v2 body-store entry: {}", .0.display())]
    UnexpectedEntry(PathBuf),
    /// Caller attempted to replace a durable body for the same subject.
    #[error("conflicting Sumeragi v2 durable body")]
    ConflictingBody,
    /// Receipt was not minted by this active store or no longer exists.
    #[error("Sumeragi v2 durable body receipt mismatch")]
    ReceiptMismatch,
    /// Kura receipt does not durably cover this body.
    #[error("Kura finality receipt does not match the pending Sumeragi v2 body")]
    KuraReceiptMismatch,
    /// Atomic destination has no parent directory.
    #[error("Sumeragi v2 body-store path has no parent directory")]
    MissingParent,
}

#[cfg(test)]
mod tests {
    use std::{cell::Cell, fs, num::NonZeroU64, path::Path};

    use iroha_crypto::{Algorithm, Hash, HashOf, KeyPair, SignatureOf};
    use iroha_data_model::{
        block::{
            BlockHeader, BlockSignature, CertifiedMergeLedgerReference, SignedBlock,
            consensus_v2 as wire, decode_framed_signed_block,
        },
        merge::MergeQuorumCertificate,
        peer::PeerId,
    };
    use tempfile::TempDir;

    use super::{
        BlockSignaturePolicy, BodyValidationCompletion, BodyValidationError,
        BodyValidationRejectionIdentity, STORE_MAGIC, STORE_VERSION, V2BodyStore, V2BodyStoreError,
        VALIDATED_MAGIC, ValidatedBodyMarker, ValidatedBodyReceipt, write_validated_marker,
    };

    use crate::sumeragi::{
        v2::RecoveredValidationAuthority, v2_apply::VerifiedRecoveredFinalitySubject,
        v2_chunks::encode_payload, v2_effects::BodyValidationTask,
    };

    #[derive(Debug)]
    enum FixtureValidationError {
        MissingMergeSidecar(CertifiedMergeLedgerReference),
        Invalid(&'static str),
    }

    impl std::fmt::Display for FixtureValidationError {
        fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            match self {
                Self::MissingMergeSidecar(reference) => {
                    write!(formatter, "missing merge sidecar {}", reference.entry_hash)
                }
                Self::Invalid(reason) => formatter.write_str(reason),
            }
        }
    }

    impl BodyValidationError for FixtureValidationError {
        fn missing_certified_merge_sidecar(&self) -> Option<&CertifiedMergeLedgerReference> {
            match self {
                Self::MissingMergeSidecar(reference) => Some(reference),
                Self::Invalid(_) => None,
            }
        }
    }

    fn context_and_keys() -> (wire::HeightContext, Vec<KeyPair>) {
        let mut keys = (1_u8..=4)
            .map(|seed| {
                KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519)
                    .expect("deterministic key")
            })
            .collect::<Vec<_>>();
        keys.sort_by(|left, right| left.public_key().cmp(right.public_key()));
        let roster = keys
            .iter()
            .map(|key| wire::ValidatorPower {
                validator: PeerId::new(key.public_key().clone()),
                power: 1,
            })
            .collect::<Vec<_>>();
        let context = wire::HeightContext {
            chain_id: "sumeragi-v2-body-store-test".into(),
            protocol_version: wire::PROTOCOL_VERSION,
            height: 1,
            epoch: 0,
            epoch_end_height: 100,
            next_epoch_snapshot: None,
            mode: wire::ConsensusMode::Permissioned,
            parent_commit_qc: None,
            snapshot_bootstrap: None,
            quorum: wire::DualQuorum::from_roster(&roster).expect("fixture quorum"),
            roster,
            nexus_amx_context_hash: Hash::new(b"test nexus amx context"),
            execution_policy_hash: iroha_crypto::Hash::new(b"test execution policy"),
            da_layout: wire::DataAvailabilityLayout {
                encoding: wire::PayloadEncoding::ReedSolomon16,
                chunk_size_bytes: 1_048_576,
                data_shards: 1,
                parity_shards: 1,
                max_payload_size_bytes: 1_048_576,
                max_chunk_count: 2,
            },
            leader_seed: [0x42; 32],
        };
        (context, keys)
    }

    fn missing_merge_reference(
        receipt: &super::DurableBodyReceipt,
    ) -> CertifiedMergeLedgerReference {
        let parent_hash =
            HashOf::from_untyped_unchecked(Hash::new(b"body-store validation parent"));
        CertifiedMergeLedgerReference {
            version: 1,
            entry_hash: HashOf::from_untyped_unchecked(Hash::new(
                b"body-store missing merge sidecar",
            )),
            encoded_len: 512,
            epoch_id: 7,
            execution_batch_hash: None,
            entrypoint_count: None,
            entrypoint_merkle_root: None,
            result_merkle_root: None,
            base_state_height: None,
            base_state_hash: None,
            merge_qc: MergeQuorumCertificate::new(
                receipt.round().view,
                7,
                receipt.round().height,
                parent_hash,
                Hash::new(b"body-store validation chain"),
                1,
                HashOf::new(&Vec::<PeerId>::new()),
                Vec::new(),
                Vec::new(),
                Vec::new(),
                Vec::new(),
                Hash::new(b"body-store validation certificate"),
            ),
        }
    }

    fn body_and_manifest(
        context: &wire::HeightContext,
        keys: &[KeyPair],
        signing_key_index: Option<usize>,
    ) -> (Vec<u8>, wire::PayloadManifest) {
        let round = wire::ConsensusRound {
            context_id: context.id(),
            height: context.height,
            view: 0,
        };
        let leader = context.leader(round.view);
        let leader_index = usize::try_from(leader).expect("leader index");
        body_and_manifest_with_signature(
            context,
            &keys[signing_key_index.unwrap_or(leader_index)],
            u64::from(leader),
        )
    }

    fn body_and_manifest_with_signature(
        context: &wire::HeightContext,
        signing_key: &KeyPair,
        signature_index: u64,
    ) -> (Vec<u8>, wire::PayloadManifest) {
        body_and_manifest_with_signature_and_views(context, signing_key, signature_index, 0, 0)
    }

    fn body_and_manifest_with_signature_and_views(
        context: &wire::HeightContext,
        signing_key: &KeyPair,
        signature_index: u64,
        proposal_view: u64,
        header_view: u64,
    ) -> (Vec<u8>, wire::PayloadManifest) {
        let round = wire::ConsensusRound {
            context_id: context.id(),
            height: context.height,
            view: proposal_view,
        };
        let header = BlockHeader::new(
            NonZeroU64::new(round.height).expect("non-zero height"),
            None,
            None,
            None,
            1_000,
            header_view,
        );
        let signature = SignatureOf::try_from_hash(signing_key.private_key(), header.hash())
            .expect("sign block header");
        let block = SignedBlock::presigned(
            BlockSignature::new(signature_index, signature),
            header,
            Vec::new(),
        );
        let canonical_wire = block.encode_wire().expect("canonical block wire");
        let subject = wire::BlockSubject {
            parent_block_hash: None,
            block_hash: block.hash(),
            payload_hash: Hash::new(&canonical_wire),
        };
        let manifest = encode_payload(context, round, subject, &canonical_wire)
            .expect("encode canonical fixture payload")
            .manifest()
            .clone();
        (canonical_wire, manifest)
    }

    fn durable_files_snapshot(root: &Path) -> Vec<(std::path::PathBuf, Vec<u8>)> {
        fn visit(root: &Path, directory: &Path, files: &mut Vec<(std::path::PathBuf, Vec<u8>)>) {
            let mut entries = fs::read_dir(directory)
                .expect("read body-store snapshot directory")
                .map(|entry| entry.expect("read body-store snapshot entry").path())
                .collect::<Vec<_>>();
            entries.sort();
            for path in entries {
                if path.is_dir() {
                    visit(root, &path, files);
                } else {
                    files.push((
                        path.strip_prefix(root)
                            .expect("snapshot entry belongs to root")
                            .to_path_buf(),
                        fs::read(&path).expect("read body-store snapshot file"),
                    ));
                }
            }
        }

        let mut files = Vec::new();
        visit(root, root, &mut files);
        files
    }

    #[test]
    fn durable_body_roundtrips_and_reopens_idempotently() {
        let directory = TempDir::new().expect("temporary directory");
        let (context, keys) = context_and_keys();
        let (body, manifest) = body_and_manifest(&context, &keys, None);
        let mut store = V2BodyStore::open(directory.path(), context.clone()).expect("open store");
        assert!(store.matches_context(&context));
        let mut foreign_context = context.clone();
        foreign_context.height = foreign_context.height.saturating_add(1);
        assert!(!store.matches_context(&foreign_context));
        let receipt = store
            .store(manifest.clone(), body.clone())
            .expect("store exact body");
        let execution_commitment =
            ValidatedBodyReceipt::for_test(receipt.clone()).execution_commitment();
        let validated = store
            .validate(&receipt, |block| {
                (block.hash() == receipt.subject().block_hash)
                    .then_some(execution_commitment)
                    .ok_or("wrong block")
            })
            .expect("validate exact durable body");
        assert_eq!(validated.durable(), &receipt);
        assert_eq!(validated.execution_commitment(), execution_commitment);
        assert_eq!(
            store
                .load(&receipt)
                .expect("load exact body")
                .encode_wire()
                .unwrap(),
            body
        );
        assert_eq!(
            store
                .store(manifest.clone(), body)
                .expect("idempotent store returns same receipt"),
            receipt
        );

        drop(store);
        let mut reopened = V2BodyStore::open(directory.path(), context).expect("replay store");
        assert_eq!(
            reopened.receipt(manifest.round, manifest.subject),
            Some(receipt.clone())
        );
        assert_eq!(
            reopened
                .recovered(manifest.round, manifest.subject)
                .expect("reload recovered manifest"),
            Some((manifest, receipt.clone()))
        );
        assert_eq!(
            reopened
                .load(&receipt)
                .expect("receipt remains valid after replay")
                .hash(),
            receipt.subject().block_hash
        );
        assert!(reopened.validated_recovery_catalog().is_empty());
        assert!(matches!(
            reopened.ensure_recovered_markers_revalidated(),
            Err(V2BodyStoreError::UnrevalidatedValidationMarkers)
        ));
        let callback_ran = Cell::new(false);
        let _validated = reopened
            .validate(&receipt, |_| {
                callback_ran.set(true);
                Ok::<wire::ExecutionCommitment, &str>(execution_commitment)
            })
            .expect("durable validation marker resumes after semantic replay");
        assert!(callback_ran.get());
        reopened
            .ensure_recovered_markers_revalidated()
            .expect("recovered marker crossed semantic replay");
        assert_eq!(
            reopened
                .validated_recovery_catalog()
                .get(&(receipt.round(), receipt.subject()))
                .map(ValidatedBodyReceipt::execution_commitment),
            Some(execution_commitment),
        );
    }

    #[test]
    fn recovered_marker_cannot_restore_vote_authority_without_semantic_replay() {
        let directory = TempDir::new().expect("temporary directory");
        let (context, keys) = context_and_keys();
        let (body, manifest) = body_and_manifest(&context, &keys, None);
        let mut store = V2BodyStore::open(directory.path(), context.clone()).expect("open store");
        let receipt = store.store(manifest, body).expect("store exact body");
        let expected = ValidatedBodyReceipt::for_test(receipt.clone()).execution_commitment();
        let _validated = store
            .persist_validated_receipt(&receipt, expected)
            .expect("persist legitimate validation marker");

        let forged = wire::ExecutionCommitment::without_topups_or_merge_carrier(
            Hash::new(b"forged parent root"),
            Hash::new(b"forged post root"),
            Hash::new(b"forged ordinary writes"),
            1,
            Hash::new(b"forged executed block"),
        );
        assert_ne!(expected, forged);
        let marker = ValidatedBodyMarker {
            version: STORE_VERSION,
            context_id: receipt.context_id,
            round: receipt.round,
            subject: receipt.subject,
            manifest_hash: receipt.manifest_hash,
            body_frame_hash: receipt.frame_hash,
            execution_commitment: forged,
        };
        write_validated_marker(
            &store.validated_path_for(receipt.round(), receipt.subject()),
            &marker,
        )
        .expect("substitute a checksum-valid local marker");
        drop(store);

        let mut reopened = V2BodyStore::open(directory.path(), context)
            .expect("structurally read substituted marker");
        assert!(reopened.validated_recovery_catalog().is_empty());
        assert!(matches!(
            reopened.revalidate_recovered_markers(|_| {
                Ok::<wire::ExecutionCommitment, String>(expected)
            }),
            Err(V2BodyStoreError::RecoveredValidationCommitmentMismatch)
        ));
        assert!(reopened.validated_recovery_catalog().is_empty());
        assert!(matches!(
            reopened.ensure_recovered_markers_revalidated(),
            Err(V2BodyStoreError::UnrevalidatedValidationMarkers)
        ));
    }

    #[test]
    fn recovered_marker_missing_sidecar_retires_authority_without_losing_body() {
        let directory = TempDir::new().expect("temporary directory");
        let (context, keys) = context_and_keys();
        let (body, manifest) = body_and_manifest(&context, &keys, None);
        let mut store = V2BodyStore::open(directory.path(), context.clone()).expect("open store");
        let receipt = store
            .store(manifest.clone(), body)
            .expect("store exact body");
        let execution_commitment =
            ValidatedBodyReceipt::for_test(receipt.clone()).execution_commitment();
        let _validated = store
            .persist_validated_receipt(&receipt, execution_commitment)
            .expect("persist validation marker");
        drop(store);

        let mut reopened = V2BodyStore::open(directory.path(), context).expect("reopen store");
        assert!(matches!(
            reopened.revalidate_recovered_markers(|_| {
                Err::<wire::ExecutionCommitment, _>(FixtureValidationError::Invalid(
                    "terminal recovered validation failure",
                ))
            }),
            Err(V2BodyStoreError::RecoveredValidationRejected(reason))
                if reason == "terminal recovered validation failure"
        ));
        assert!(matches!(
            reopened.ensure_recovered_markers_revalidated(),
            Err(V2BodyStoreError::UnrevalidatedValidationMarkers)
        ));

        let reference = missing_merge_reference(&receipt);
        reopened
            .revalidate_recovered_markers(|_| {
                Err::<wire::ExecutionCommitment, _>(FixtureValidationError::MissingMergeSidecar(
                    reference.clone(),
                ))
            })
            .expect("missing sidecar retires marker authority without failing startup");
        reopened
            .ensure_recovered_markers_revalidated()
            .expect("no untrusted marker authority survives startup");
        assert!(reopened.validated_recovery_catalog().is_empty());
        assert_eq!(
            reopened
                .recovered(manifest.round, manifest.subject)
                .expect("inspect retained exact body"),
            Some((manifest, receipt.clone()))
        );

        let task = BodyValidationTask::for_test(43, receipt.clone());
        let deferred = reopened
            .execute_validation_task(&task, |_| {
                Err::<wire::ExecutionCommitment, _>(FixtureValidationError::MissingMergeSidecar(
                    reference.clone(),
                ))
            })
            .expect("ordinary validation defers on the exact missing sidecar");
        assert!(matches!(
            deferred,
            BodyValidationCompletion::DeferredMergeSidecar {
                reference: deferred_reference,
                ..
            } if deferred_reference == reference
        ));
        assert!(reopened.validated_recovery_catalog().is_empty());

        let validated = reopened
            .execute_validation_task(&task, |_| {
                Ok::<_, FixtureValidationError>(execution_commitment)
            })
            .expect("ordinary bounded retry validates after sidecar recovery");
        assert_eq!(
            validated
                .validated_receipt()
                .map(ValidatedBodyReceipt::execution_commitment),
            Some(execution_commitment)
        );
    }

    #[test]
    fn wal_frontier_bounds_many_view_restart_validation_work() {
        let directory = TempDir::new().expect("temporary directory");
        let (context, keys) = context_and_keys();
        let mut store = V2BodyStore::open(directory.path(), context.clone()).expect("open store");
        let mut receipts = Vec::new();
        for view in 0_u64..32 {
            let leader = context.leader(view);
            let leader_index = usize::try_from(leader).expect("leader index");
            let (body, manifest) = body_and_manifest_with_signature_and_views(
                &context,
                &keys[leader_index],
                u64::from(leader),
                view,
                view,
            );
            let receipt = store.store(manifest, body).expect("store view candidate");
            let commitment = ValidatedBodyReceipt::for_test(receipt.clone()).execution_commitment();
            let _validated = store
                .persist_validated_receipt(&receipt, commitment)
                .expect("persist view validation marker");
            receipts.push((receipt, commitment));
        }
        drop(store);

        let mut reopened =
            V2BodyStore::open(directory.path(), context.clone()).expect("reopen view catalog");
        let selected = [receipts[7].0.clone(), receipts[31].0.clone()];
        let authority = RecoveredValidationAuthority::for_test(
            &context,
            selected
                .iter()
                .map(|receipt| (receipt.round(), receipt.subject())),
        );
        assert_eq!(authority.len(), 2);
        reopened
            .retain_recovered_markers_for_authority(authority)
            .expect("WAL frontier belongs to the exact body context");

        let callback_count = Cell::new(0_usize);
        reopened
            .revalidate_recovered_markers(|block| {
                callback_count.set(callback_count.get().saturating_add(1));
                receipts
                    .iter()
                    .find_map(|(receipt, commitment)| {
                        (receipt.subject().block_hash == block.hash()).then_some(*commitment)
                    })
                    .ok_or_else(|| "replayed an unauthorized body".to_owned())
            })
            .expect("revalidate only the authenticated WAL frontier");
        assert_eq!(callback_count.get(), 2);
        assert_eq!(reopened.validated_recovery_catalog().len(), 2);
        assert_eq!(
            reopened
                .recovery_catalog()
                .expect("retained body catalog")
                .len(),
            32,
            "superseded markers lose authority without deleting DA body evidence"
        );
    }

    #[test]
    fn wal_frontier_capability_cannot_cross_height_contexts() {
        let directory = TempDir::new().expect("temporary directory");
        let (context, keys) = context_and_keys();
        let (body, manifest) = body_and_manifest(&context, &keys, None);
        let mut store = V2BodyStore::open(directory.path(), context.clone()).expect("open store");
        let receipt = store.store(manifest, body).expect("store exact body");
        let commitment = ValidatedBodyReceipt::for_test(receipt.clone()).execution_commitment();
        let _validated = store
            .persist_validated_receipt(&receipt, commitment)
            .expect("persist validation marker");
        drop(store);

        let mut reopened =
            V2BodyStore::open(directory.path(), context.clone()).expect("reopen store");
        let pending_before = reopened.pending_revalidation.clone();
        let validated_before = reopened.validated.clone();
        let mut foreign_context = context;
        foreign_context.leader_seed[0] ^= 0x40;
        let foreign_round = wire::ConsensusRound {
            context_id: foreign_context.id(),
            height: foreign_context.height,
            view: receipt.round().view,
        };
        let authority = RecoveredValidationAuthority::for_test(
            &foreign_context,
            [(foreign_round, receipt.subject())],
        );

        assert!(matches!(
            reopened.retain_recovered_markers_for_authority(authority),
            Err(V2BodyStoreError::RecoveredValidationAuthorityContextMismatch)
        ));
        assert_eq!(reopened.pending_revalidation, pending_before);
        assert_eq!(reopened.validated, validated_before);
    }

    #[test]
    fn verified_decision_retires_losing_restart_marker_authority() {
        let directory = TempDir::new().expect("temporary directory");
        let (context, keys) = context_and_keys();
        let mut store = V2BodyStore::open(directory.path(), context.clone()).expect("open store");
        let mut receipts = Vec::new();
        for view in [0_u64, 1] {
            let leader = context.leader(view);
            let leader_index = usize::try_from(leader).expect("leader index");
            let (body, manifest) = body_and_manifest_with_signature_and_views(
                &context,
                &keys[leader_index],
                u64::from(leader),
                view,
                view,
            );
            let receipt = store.store(manifest, body).expect("store candidate body");
            let commitment = ValidatedBodyReceipt::for_test(receipt.clone()).execution_commitment();
            let _validated = store
                .persist_validated_receipt(&receipt, commitment)
                .expect("persist candidate validation marker");
            receipts.push((receipt, commitment));
        }
        assert_ne!(receipts[0].0.subject(), receipts[1].0.subject());
        drop(store);

        let mut reopened =
            V2BodyStore::open(directory.path(), context.clone()).expect("reopen store");
        reopened
            .retain_recovered_markers_for_subject(VerifiedRecoveredFinalitySubject::for_test(
                &context,
                receipts[0].0.subject(),
            ))
            .expect("verified decision belongs to the recovered context");
        reopened
            .revalidate_recovered_markers(|_| {
                Ok::<wire::ExecutionCommitment, String>(receipts[0].1)
            })
            .expect("revalidate only the verified decision");
        reopened
            .ensure_recovered_markers_revalidated()
            .expect("losing marker authority was retired");
        let catalog = reopened.validated_recovery_catalog();
        assert!(catalog.contains_key(&(receipts[0].0.round(), receipts[0].0.subject())));
        assert!(!catalog.contains_key(&(receipts[1].0.round(), receipts[1].0.subject())));
    }

    #[test]
    fn verified_decision_capability_cannot_cross_height_contexts() {
        let directory = TempDir::new().expect("temporary directory");
        let (context, keys) = context_and_keys();
        let (body, manifest) = body_and_manifest(&context, &keys, None);
        let mut store = V2BodyStore::open(directory.path(), context.clone()).expect("open store");
        let receipt = store.store(manifest, body).expect("store candidate body");
        let commitment = ValidatedBodyReceipt::for_test(receipt.clone()).execution_commitment();
        let _validated = store
            .persist_validated_receipt(&receipt, commitment)
            .expect("persist candidate validation marker");
        drop(store);

        let mut reopened =
            V2BodyStore::open(directory.path(), context.clone()).expect("reopen store");
        let pending_before = reopened.pending_revalidation.clone();
        let validated_before = reopened.validated.clone();
        let mut foreign_context = context.clone();
        foreign_context.leader_seed[0] ^= 0x80;
        assert_ne!(foreign_context.id(), context.id());

        let error = reopened
            .retain_recovered_markers_for_subject(VerifiedRecoveredFinalitySubject::for_test(
                &foreign_context,
                receipt.subject(),
            ))
            .expect_err("foreign finality capability must fail closed");
        assert!(matches!(
            error,
            V2BodyStoreError::RecoveredFinalityContextMismatch
        ));
        assert_eq!(reopened.pending_revalidation, pending_before);
        assert_eq!(reopened.validated, validated_before);
    }

    #[test]
    fn verified_decision_retires_already_promoted_losing_marker_authority() {
        let directory = TempDir::new().expect("temporary directory");
        let (context, keys) = context_and_keys();
        let mut store = V2BodyStore::open(directory.path(), context.clone()).expect("open store");
        let mut receipts = Vec::new();
        for view in [0_u64, 1] {
            let leader = context.leader(view);
            let leader_index = usize::try_from(leader).expect("leader index");
            let (body, manifest) = body_and_manifest_with_signature_and_views(
                &context,
                &keys[leader_index],
                u64::from(leader),
                view,
                view,
            );
            let receipt = store.store(manifest, body).expect("store candidate body");
            let commitment = ValidatedBodyReceipt::for_test(receipt.clone()).execution_commitment();
            let _validated = store
                .persist_validated_receipt(&receipt, commitment)
                .expect("persist candidate validation marker");
            receipts.push((receipt, commitment));
        }
        assert_ne!(receipts[0].0.subject(), receipts[1].0.subject());
        drop(store);

        let mut reopened =
            V2BodyStore::open(directory.path(), context.clone()).expect("reopen store");
        let _validated = reopened
            .validate(&receipts[1].0, |_| {
                Ok::<wire::ExecutionCommitment, &str>(receipts[1].1)
            })
            .expect("promote the losing recovered marker before finality filtering");
        assert!(
            reopened
                .validated_recovery_catalog()
                .contains_key(&(receipts[1].0.round(), receipts[1].0.subject()))
        );

        reopened
            .retain_recovered_markers_for_subject(VerifiedRecoveredFinalitySubject::for_test(
                &context,
                receipts[0].0.subject(),
            ))
            .expect("verified decision belongs to the recovered context");
        assert!(reopened.validated_recovery_catalog().is_empty());
        reopened
            .revalidate_recovered_markers(|_| {
                Ok::<wire::ExecutionCommitment, String>(receipts[0].1)
            })
            .expect("revalidate only the verified decision");
        reopened
            .ensure_recovered_markers_revalidated()
            .expect("all losing marker authority was retired");
        let catalog = reopened.validated_recovery_catalog();
        assert!(catalog.contains_key(&(receipts[0].0.round(), receipts[0].0.subject())));
        assert!(!catalog.contains_key(&(receipts[1].0.round(), receipts[1].0.subject())));
    }

    #[test]
    fn legacy_v3_body_and_validation_frames_are_rejected() {
        const LEGACY_VERSION: u16 = 3;

        let body_directory = TempDir::new().expect("temporary body directory");
        let (body_context, body_keys) = context_and_keys();
        let (body, manifest) = body_and_manifest(&body_context, &body_keys, None);
        let mut body_store =
            V2BodyStore::open(body_directory.path(), body_context.clone()).expect("open store");
        let body_receipt = body_store.store(manifest, body).expect("store body");
        let body_path = body_store.path_for(body_receipt.round(), body_receipt.subject());
        drop(body_store);
        let mut body_frame = fs::read(&body_path).expect("read body frame");
        body_frame[STORE_MAGIC.len()..STORE_MAGIC.len() + size_of::<u16>()]
            .copy_from_slice(&LEGACY_VERSION.to_le_bytes());
        fs::write(&body_path, body_frame).expect("write legacy body frame");
        assert!(matches!(
            V2BodyStore::open(body_directory.path(), body_context),
            Err(V2BodyStoreError::UnsupportedVersion(LEGACY_VERSION))
        ));

        let marker_directory = TempDir::new().expect("temporary marker directory");
        let (marker_context, marker_keys) = context_and_keys();
        let (body, manifest) = body_and_manifest(&marker_context, &marker_keys, None);
        let mut marker_store = V2BodyStore::open(marker_directory.path(), marker_context.clone())
            .expect("open marker store");
        let marker_receipt = marker_store
            .store(manifest, body)
            .expect("store marker body");
        let commitment =
            ValidatedBodyReceipt::for_test(marker_receipt.clone()).execution_commitment();
        let _validated_receipt = marker_store
            .validate(&marker_receipt, |_| Ok::<_, &'static str>(commitment))
            .expect("persist validation marker");
        let marker_path =
            marker_store.validated_path_for(marker_receipt.round(), marker_receipt.subject());
        drop(marker_store);
        let mut marker_frame = fs::read(&marker_path).expect("read validation frame");
        marker_frame[VALIDATED_MAGIC.len()..VALIDATED_MAGIC.len() + size_of::<u16>()]
            .copy_from_slice(&LEGACY_VERSION.to_le_bytes());
        fs::write(&marker_path, marker_frame).expect("write legacy validation frame");
        assert!(matches!(
            V2BodyStore::open(marker_directory.path(), marker_context),
            Err(V2BodyStoreError::UnsupportedVersion(LEGACY_VERSION))
        ));
    }

    #[test]
    fn rotating_leader_locked_body_reproposal_is_stored_and_revalidated_per_round() {
        let directory = TempDir::new().expect("temporary directory");
        let (context, keys) = context_and_keys();
        let (body, origin_manifest) = body_and_manifest(&context, &keys, None);
        let mut store =
            V2BodyStore::open(directory.path(), context.clone()).expect("open body store");
        let origin_receipt = store
            .store(origin_manifest.clone(), body.clone())
            .expect("store the origin-view body");
        let execution_commitment =
            ValidatedBodyReceipt::for_test(origin_receipt.clone()).execution_commitment();
        let origin_task = BodyValidationTask::for_test(51, origin_receipt.clone());
        let first_callback_ran = Cell::new(false);
        let origin_validation = store
            .execute_validation_task(&origin_task, |_| {
                first_callback_ran.set(true);
                Ok::<_, FixtureValidationError>(execution_commitment)
            })
            .expect("validate the exact body once");
        assert!(first_callback_ran.get());
        assert_eq!(
            origin_validation
                .validated_receipt()
                .map(ValidatedBodyReceipt::execution_commitment),
            Some(execution_commitment)
        );

        drop(store);
        let mut store = V2BodyStore::open(directory.path(), context.clone())
            .expect("recover the exact origin-view validation marker");
        store
            .revalidate_recovered_markers(|_| {
                Ok::<wire::ExecutionCommitment, String>(execution_commitment)
            })
            .expect("semantically replay the recovered origin marker");

        let later_round = wire::ConsensusRound {
            context_id: context.id(),
            height: context.height,
            view: 7,
        };
        let later_manifest = encode_payload(&context, later_round, origin_manifest.subject, &body)
            .expect("encode the exact body for its later view")
            .manifest()
            .clone();
        let later_receipt = store
            .store(later_manifest, body)
            .expect("the original leader signature authenticates an unchanged reproposal body");
        let callback_ran = Cell::new(false);
        let later_validation = store
            .execute_validation_task(&BodyValidationTask::for_test(52, later_receipt), |_| {
                callback_ran.set(true);
                Ok::<_, FixtureValidationError>(execution_commitment)
            })
            .expect("revalidate the unchanged body under its new proposal round");
        assert!(
            callback_ran.get(),
            "validation markers never promote across rounds"
        );
        assert_eq!(
            later_validation
                .validated_receipt()
                .map(ValidatedBodyReceipt::execution_commitment),
            Some(execution_commitment)
        );
        assert!(
            store
                .validated_path_for(later_round, origin_manifest.subject)
                .exists()
        );
        assert_eq!(
            store
                .validated_recovery_catalog()
                .get(&(origin_manifest.round, origin_manifest.subject))
                .map(ValidatedBodyReceipt::durable),
            Some(&origin_receipt)
        );
    }

    #[test]
    fn genesis_cross_view_validation_is_reexecuted_and_conflicts_fail_closed() {
        let directory = TempDir::new().expect("temporary directory");
        let (context, _keys) = context_and_keys();
        let genesis = KeyPair::try_from_seed(vec![0xC4; 32], Algorithm::Ed25519)
            .expect("deterministic genesis key");
        let (body, origin_manifest) =
            body_and_manifest_with_signature_and_views(&context, &genesis, 0, 0, 0);
        let mut store = V2BodyStore::open_with_policy(
            directory.path(),
            context.clone(),
            BlockSignaturePolicy::GenesisAuthority(genesis.public_key().clone()),
        )
        .expect("open genesis body store");
        let origin_receipt = store
            .store(origin_manifest.clone(), body.clone())
            .expect("store the origin-view body");
        let origin_commitment =
            ValidatedBodyReceipt::for_test(origin_receipt.clone()).execution_commitment();
        let _ = store
            .persist_validated_receipt(&origin_receipt, origin_commitment)
            .expect("persist the origin-view validation witness");

        let later_round = wire::ConsensusRound {
            context_id: context.id(),
            height: context.height,
            view: 7,
        };
        let later_manifest = encode_payload(&context, later_round, origin_manifest.subject, &body)
            .expect("encode the exact body for its later view")
            .manifest()
            .clone();
        let later_receipt = store
            .store(later_manifest, body)
            .expect("durably bind the exact body to the later round");
        let conflicting_commitment =
            ValidatedBodyReceipt::for_test(later_receipt.clone()).execution_commitment();
        assert_ne!(origin_commitment, conflicting_commitment);

        let callback_ran = Cell::new(false);
        let later_task = BodyValidationTask::for_test(52, later_receipt.clone());
        let error = store
            .execute_validation_task(&later_task, |_| {
                callback_ran.set(true);
                Ok::<_, FixtureValidationError>(conflicting_commitment)
            })
            .expect_err("a prior-view marker must not bypass exact-round validation");
        assert!(
            callback_ran.get(),
            "the later proposal round must be revalidated"
        );
        assert!(matches!(
            error,
            V2BodyStoreError::ConflictingValidationCommitment
        ));
        let marker_path = store.validated_path_for(later_round, origin_manifest.subject);
        assert!(!marker_path.exists());

        let conflicting_marker = ValidatedBodyMarker {
            version: STORE_VERSION,
            context_id: later_receipt.context_id,
            round: later_receipt.round,
            subject: later_receipt.subject,
            manifest_hash: later_receipt.manifest_hash,
            body_frame_hash: later_receipt.frame_hash,
            execution_commitment: conflicting_commitment,
        };
        write_validated_marker(&marker_path, &conflicting_marker)
            .expect("write a syntactically valid conflicting marker");
        drop(store);

        let error = match V2BodyStore::open_with_policy(
            directory.path(),
            context,
            BlockSignaturePolicy::GenesisAuthority(genesis.public_key().clone()),
        ) {
            Ok(_) => panic!("recovery must reject conflicting exact-body commitments"),
            Err(error) => error,
        };
        assert!(matches!(
            error,
            V2BodyStoreError::ConflictingValidationCommitment
        ));
    }

    #[test]
    fn result_bearing_proposal_is_rejected_before_durable_admission() {
        let directory = TempDir::new().expect("temporary directory");
        let (context, keys) = context_and_keys();
        let (body, manifest) = body_and_manifest(&context, &keys, None);
        let mut result_bearing = decode_framed_signed_block(&body).expect("decode fixture body");
        result_bearing
            .set_transaction_results(Vec::new(), &[], Vec::new())
            .expect("attach empty deterministic execution result");
        assert!(!result_bearing.is_resultless_proposal());
        let result_bearing_wire = result_bearing
            .encode_wire()
            .expect("encode result-bearing body");
        let subject = wire::BlockSubject {
            parent_block_hash: result_bearing.header().prev_block_hash(),
            block_hash: result_bearing.hash(),
            payload_hash: Hash::new(&result_bearing_wire),
        };
        let result_bearing_manifest =
            encode_payload(&context, manifest.round, subject, &result_bearing_wire)
                .expect("encode result-bearing fixture payload")
                .manifest()
                .clone();
        let mut store = V2BodyStore::open(directory.path(), context).expect("open store");
        assert!(matches!(
            store.store(result_bearing_manifest, result_bearing_wire),
            Err(V2BodyStoreError::ResultBearingProposal)
        ));
    }

    #[test]
    fn typed_validation_deferral_and_rejection_never_mint_success_receipts() {
        let directory = TempDir::new().expect("temporary directory");
        let (context, keys) = context_and_keys();
        let (body, manifest) = body_and_manifest(&context, &keys, None);
        let mut store = V2BodyStore::open(directory.path(), context).expect("open store");
        let receipt = store.store(manifest, body).expect("store exact body");
        let task = BodyValidationTask::for_test(41, receipt.clone());
        let reference = missing_merge_reference(&receipt);
        let execution_commitment =
            ValidatedBodyReceipt::for_test(receipt.clone()).execution_commitment();

        let deferred = store
            .execute_validation_task(&task, |_| {
                Err::<wire::ExecutionCommitment, _>(FixtureValidationError::MissingMergeSidecar(
                    reference.clone(),
                ))
            })
            .expect("classify exact missing sidecar as deferred");
        assert!(matches!(
            deferred,
            BodyValidationCompletion::DeferredMergeSidecar {
                reference: deferred_reference,
                ..
            } if deferred_reference == reference
        ));
        assert!(store.validated_recovery_catalog().is_empty());
        assert!(
            !store
                .validated_path_for(receipt.round(), receipt.subject())
                .exists()
        );

        let rejected = store
            .execute_validation_task(&task, |_| {
                Err::<wire::ExecutionCommitment, _>(FixtureValidationError::Invalid(
                    "invalid candidate",
                ))
            })
            .expect("return terminal deterministic rejection");
        assert_eq!(rejected.rejection_reason(), Some("invalid candidate"));
        assert!(store.validated_recovery_catalog().is_empty());
        assert!(
            !store
                .validated_path_for(receipt.round(), receipt.subject())
                .exists()
        );

        let validated = store
            .execute_validation_task(&task, |_| {
                Ok::<_, FixtureValidationError>(execution_commitment)
            })
            .expect("persist validation only after success");
        assert_eq!(
            validated
                .validated_receipt()
                .map(ValidatedBodyReceipt::durable),
            Some(&receipt)
        );
        assert!(
            store
                .validated_path_for(receipt.round(), receipt.subject())
                .exists()
        );
    }

    #[test]
    fn durable_validation_persists_success_and_repeats_idempotently() {
        let directory = TempDir::new().expect("temporary directory");
        let (context, keys) = context_and_keys();
        let (body, manifest) = body_and_manifest(&context, &keys, None);
        let mut store = V2BodyStore::open(directory.path(), context).expect("open store");
        let receipt = store
            .store(manifest, body)
            .expect("persist exact candidate body");
        let expected_manifest_hash = receipt.manifest_hash();
        let execution_commitment =
            ValidatedBodyReceipt::for_test(receipt.clone()).execution_commitment();

        let validated = store
            .execute_durable_validation(receipt.clone(), expected_manifest_hash, |_| {
                Ok::<_, FixtureValidationError>(execution_commitment)
            })
            .expect("durable validation succeeds");
        assert_eq!(validated.durable_body(), &receipt);
        assert_eq!(
            validated
                .validated_receipt()
                .map(ValidatedBodyReceipt::execution_commitment),
            Some(execution_commitment)
        );
        let marker_path = store.validated_path_for(receipt.round(), receipt.subject());
        let marker_before_repeat = fs::read(&marker_path)
            .expect("success marker is durable before the outcome is returned");
        let files_before_repeat = durable_files_snapshot(directory.path());

        let validator_called = Cell::new(false);
        let repeated = store
            .execute_durable_validation(receipt.clone(), expected_manifest_hash, |_| {
                validator_called.set(true);
                Err::<wire::ExecutionCommitment, _>(FixtureValidationError::Invalid(
                    "idempotent validation must not rerun the callback",
                ))
            })
            .expect("repeat reuses the exact durable success");
        assert!(!validator_called.get());
        assert_eq!(repeated.durable_body(), &receipt);
        assert_eq!(repeated.validated_receipt(), validated.validated_receipt());
        assert_eq!(
            fs::read(marker_path).expect("read repeated success marker"),
            marker_before_repeat
        );
        assert_eq!(
            durable_files_snapshot(directory.path()),
            files_before_repeat
        );
        assert_eq!(
            repeated
                .into_validated_receipt()
                .expect("success-only extraction accepts the durable validation")
                .execution_commitment(),
            execution_commitment
        );
    }

    #[test]
    fn durable_validation_binds_rejection_and_typed_deferral_to_the_exact_body() {
        let directory = TempDir::new().expect("temporary directory");
        let (context, keys) = context_and_keys();
        let (body, manifest) = body_and_manifest(&context, &keys, None);
        let mut store = V2BodyStore::open(directory.path(), context).expect("open store");
        let receipt = store
            .store(manifest, body)
            .expect("persist exact candidate body");
        let expected_manifest_hash = receipt.manifest_hash();
        let marker_path = store.validated_path_for(receipt.round(), receipt.subject());
        let files_before = durable_files_snapshot(directory.path());

        let rejected = store
            .execute_durable_validation(receipt.clone(), expected_manifest_hash, |_| {
                Err::<wire::ExecutionCommitment, _>(FixtureValidationError::Invalid(
                    "candidate is invalid",
                ))
            })
            .expect("return a closed rejection outcome");
        assert_eq!(rejected.durable_body(), &receipt);
        assert_eq!(rejected.rejection_reason(), Some("candidate is invalid"));
        assert_eq!(
            rejected.rejection_identity(),
            Some(&BodyValidationRejectionIdentity::Rejected)
        );
        assert!(rejected.validated_receipt().is_none());
        assert!(rejected.missing_merge_sidecar().is_none());
        assert!(!marker_path.exists());
        assert_eq!(durable_files_snapshot(directory.path()), files_before);
        let rejected = rejected
            .into_validated_receipt()
            .expect_err("rejection must remain intact on the success-only path");
        assert_eq!(rejected.durable_body(), &receipt);
        assert_eq!(rejected.rejection_reason(), Some("candidate is invalid"));
        assert_eq!(
            rejected.rejection_identity(),
            Some(&BodyValidationRejectionIdentity::Rejected)
        );

        let reference = missing_merge_reference(&receipt);
        let deferred = store
            .execute_durable_validation(receipt.clone(), expected_manifest_hash, |_| {
                Err::<wire::ExecutionCommitment, _>(FixtureValidationError::MissingMergeSidecar(
                    reference.clone(),
                ))
            })
            .expect("return a reference-bound deferral outcome");
        assert_eq!(deferred.durable_body(), &receipt);
        assert_eq!(deferred.missing_merge_sidecar(), Some(&reference));
        assert!(deferred.validated_receipt().is_none());
        assert!(deferred.rejection_reason().is_none());
        assert!(deferred.rejection_identity().is_none());
        assert!(!marker_path.exists());
        assert!(store.validated_recovery_catalog().is_empty());
        assert_eq!(durable_files_snapshot(directory.path()), files_before);
    }

    #[test]
    fn durable_validation_preflight_errors_preserve_store_state_byte_for_byte() {
        let directory = TempDir::new().expect("temporary directory");
        let (context, keys) = context_and_keys();
        let (body, manifest) = body_and_manifest(&context, &keys, None);
        let mut store =
            V2BodyStore::open(directory.path(), context.clone()).expect("open exact store");
        let receipt = store
            .store(manifest, body)
            .expect("persist exact candidate body");

        let entries_before = store.entries.clone();
        let manifests_before = store.manifests.clone();
        let pending_before = store.pending_revalidation.clone();
        let validated_before = store.validated.clone();
        let files_before = durable_files_snapshot(directory.path());
        let callback_called = Cell::new(false);
        let wrong_expected = HashOf::<wire::PayloadManifest>::from_untyped_unchecked(Hash::new(
            b"independently wrong expected manifest",
        ));
        let wrong_manifest = store.execute_durable_validation(
            receipt.clone(),
            wrong_expected,
            |_| -> Result<wire::ExecutionCommitment, FixtureValidationError> {
                callback_called.set(true);
                unreachable!("manifest mismatch must precede the validator")
            },
        );
        assert!(matches!(
            wrong_manifest,
            Err(V2BodyStoreError::ReceiptMismatch)
        ));
        assert!(!callback_called.get());
        assert_eq!(store.entries, entries_before);
        assert_eq!(store.manifests, manifests_before);
        assert_eq!(store.pending_revalidation, pending_before);
        assert_eq!(store.validated, validated_before);
        assert_eq!(durable_files_snapshot(directory.path()), files_before);

        let foreign_directory = TempDir::new().expect("foreign temporary directory");
        let mut foreign_context = context;
        foreign_context.chain_id = "foreign-sumeragi-v2-body-store".into();
        let (foreign_body, foreign_manifest) = body_and_manifest(&foreign_context, &keys, None);
        let mut foreign_store = V2BodyStore::open(foreign_directory.path(), foreign_context)
            .expect("open foreign store");
        let foreign_receipt = foreign_store
            .store(foreign_manifest, foreign_body)
            .expect("persist foreign body");
        let foreign_result = store.execute_durable_validation(
            foreign_receipt.clone(),
            foreign_receipt.manifest_hash(),
            |_| -> Result<wire::ExecutionCommitment, FixtureValidationError> {
                callback_called.set(true);
                unreachable!("foreign receipt must precede the validator")
            },
        );
        assert!(matches!(
            foreign_result,
            Err(V2BodyStoreError::ReceiptMismatch)
        ));
        assert!(!callback_called.get());
        assert_eq!(store.entries, entries_before);
        assert_eq!(store.manifests, manifests_before);
        assert_eq!(store.pending_revalidation, pending_before);
        assert_eq!(store.validated, validated_before);
        assert_eq!(durable_files_snapshot(directory.path()), files_before);
    }

    #[test]
    fn durable_validation_surface_has_no_scheduler_identity_or_ordinal() {
        let source = include_str!("v2_body_store.rs");
        let section = |begin: &str, end: &str| {
            source
                .split_once(begin)
                .expect("durable validation section begins")
                .1
                .split_once(end)
                .expect("durable validation section ends")
                .0
        };
        let surface = section(
            "// DURABLE_BODY_VALIDATION_SURFACE_BEGIN",
            "// DURABLE_BODY_VALIDATION_SURFACE_END",
        );
        let api = section(
            "// DURABLE_BODY_VALIDATION_API_BEGIN",
            "// DURABLE_BODY_VALIDATION_API_END",
        );
        let error_classification = source
            .split("pub(crate) trait BodyValidationError")
            .nth(1)
            .expect("body validation error classification exists")
            .split("pub(crate) enum BlockSignaturePolicy")
            .next()
            .expect("signature policy follows validation error classification");

        for forbidden in [
            "EffectWorkId",
            "BodyValidationTask",
            "RuntimeEffectOwnership",
            "lifecycle_ordinal",
            "work_id",
        ] {
            assert!(
                !surface.contains(forbidden) && !api.contains(forbidden),
                "new durable validation surface must not contain {forbidden}"
            );
        }
        assert!(!surface.contains("Clone"));
        assert!(
            surface
                .contains("struct DurableBodyValidationOutcome(DurableBodyValidationOutcomeBody);")
        );
        assert!(surface.contains("enum DurableBodyValidationOutcomeBody"));
        assert!(surface.contains("enum BodyValidationRejectionIdentity"));
        assert!(surface.contains("identity: BodyValidationRejectionIdentity"));
        assert!(surface.contains("pub(crate) const fn rejection_identity"));
        assert!(api.contains("identity: error.rejection_identity()"));
        assert!(error_classification.contains("fn rejection_identity(&self)"));
        assert!(error_classification.contains("BodyValidationRejectionIdentity::Rejected"));
    }

    #[test]
    fn corrupted_or_orphaned_validation_marker_fails_closed() {
        let directory = TempDir::new().expect("temporary directory");
        let (context, keys) = context_and_keys();
        let (body, manifest) = body_and_manifest(&context, &keys, None);
        let mut store = V2BodyStore::open(directory.path(), context.clone()).expect("open store");
        let receipt = store.store(manifest, body).expect("store exact body");
        let execution_commitment =
            ValidatedBodyReceipt::for_test(receipt.clone()).execution_commitment();
        let _validated = store
            .validate(&receipt, |_| Ok::<_, &'static str>(execution_commitment))
            .expect("persist validation marker");
        let marker_path = store.validated_path_for(receipt.round(), receipt.subject());
        let mut marker_bytes = fs::read(&marker_path).expect("read marker");
        *marker_bytes.last_mut().expect("nonempty marker") ^= 0x80;
        fs::write(&marker_path, marker_bytes).expect("corrupt marker");
        drop(store);
        assert!(matches!(
            V2BodyStore::open(directory.path(), context.clone()),
            Err(V2BodyStoreError::ChecksumMismatch)
        ));

        fs::remove_file(&marker_path).expect("remove corrupt marker");
        let reopened = V2BodyStore::open(directory.path(), context.clone()).expect("reopen body");
        let marker = ValidatedBodyMarker {
            version: STORE_VERSION,
            context_id: receipt.context_id(),
            round: wire::ConsensusRound {
                view: receipt.round().view.saturating_add(1),
                ..receipt.round()
            },
            subject: receipt.subject(),
            manifest_hash: receipt.manifest_hash(),
            body_frame_hash: receipt.frame_hash,
            execution_commitment,
        };
        let orphan_path = reopened.validated_path_for(marker.round, marker.subject);
        write_validated_marker(&orphan_path, &marker).expect("write orphan marker");
        drop(reopened);
        assert!(matches!(
            V2BodyStore::open(directory.path(), context),
            Err(V2BodyStoreError::OrphanedValidationMarker)
        ));
    }

    #[test]
    fn final_file_corruption_fails_closed_but_incomplete_temp_is_ignored() {
        let directory = TempDir::new().expect("temporary directory");
        let (context, keys) = context_and_keys();
        let (body, manifest) = body_and_manifest(&context, &keys, None);
        let mut store = V2BodyStore::open(directory.path(), context.clone()).expect("open store");
        let _receipt = store.store(manifest, body).expect("store body");
        let context_directory = directory.path().join(hex::encode(context.id().0.as_ref()));
        fs::write(context_directory.join("interrupted.norito.tmp"), b"partial")
            .expect("write incomplete temp file");
        V2BodyStore::open(directory.path(), context.clone())
            .expect("incomplete temp is unacknowledged");

        let final_path = fs::read_dir(&context_directory)
            .expect("list context directory")
            .map(|entry| entry.expect("directory entry").path())
            .find(|path| path.extension().and_then(|value| value.to_str()) == Some("norito"))
            .expect("durable final body");
        let mut bytes = fs::read(&final_path).expect("read final body");
        let last = bytes.last_mut().expect("non-empty frame");
        *last ^= 0x80;
        fs::write(&final_path, bytes).expect("corrupt final body");
        assert!(matches!(
            V2BodyStore::open(directory.path(), context),
            Err(V2BodyStoreError::ChecksumMismatch)
        ));
    }

    #[test]
    fn wrong_leader_signature_is_rejected_before_durability() {
        let directory = TempDir::new().expect("temporary directory");
        let (context, keys) = context_and_keys();
        let leader = usize::try_from(context.leader(0)).expect("leader index");
        let wrong = (leader + 1) % keys.len();
        let (body, manifest) = body_and_manifest(&context, &keys, Some(wrong));
        let mut store = V2BodyStore::open(directory.path(), context).expect("open store");
        assert!(matches!(
            store.store(manifest, body),
            Err(V2BodyStoreError::InvalidExpectedSignature)
        ));
    }

    #[test]
    fn height_one_can_require_the_distinct_genesis_authority() {
        let directory = TempDir::new().expect("temporary directory");
        let (context, _keys) = context_and_keys();
        let genesis = KeyPair::try_from_seed(vec![0xA5; 32], Algorithm::Ed25519)
            .expect("deterministic genesis key");
        let impostor = KeyPair::try_from_seed(vec![0x5A; 32], Algorithm::Ed25519)
            .expect("deterministic impostor key");
        let (body, manifest) = body_and_manifest_with_signature(&context, &genesis, 0);
        let mut store = V2BodyStore::open_with_policy(
            directory.path(),
            context.clone(),
            BlockSignaturePolicy::GenesisAuthority(genesis.public_key().clone()),
        )
        .expect("open genesis body store");
        let _receipt = store
            .store(manifest, body)
            .expect("configured genesis signature is accepted");

        let other_directory = TempDir::new().expect("other temporary directory");
        let (body, manifest) = body_and_manifest_with_signature(&context, &impostor, 0);
        let mut store = V2BodyStore::open_with_policy(
            other_directory.path(),
            context,
            BlockSignaturePolicy::GenesisAuthority(genesis.public_key().clone()),
        )
        .expect("open genesis body store");
        assert!(matches!(
            store.store(manifest, body),
            Err(V2BodyStoreError::InvalidExpectedSignature)
        ));
    }

    #[test]
    fn fixed_genesis_body_can_be_reproposed_after_a_certified_view_change() {
        let directory = TempDir::new().expect("temporary directory");
        let (context, _keys) = context_and_keys();
        let genesis = KeyPair::try_from_seed(vec![0xC3; 32], Algorithm::Ed25519)
            .expect("deterministic genesis key");
        let (body, manifest) =
            body_and_manifest_with_signature_and_views(&context, &genesis, 0, 3, 0);
        let mut store = V2BodyStore::open_with_policy(
            directory.path(),
            context,
            BlockSignaturePolicy::GenesisAuthority(genesis.public_key().clone()),
        )
        .expect("open genesis body store");

        let _receipt = store
            .store(manifest, body)
            .expect("fixed signed genesis body is valid in a later proposal view");
    }

    #[test]
    fn rotating_leader_reproposal_authenticates_the_immutable_header_leader() {
        let directory = TempDir::new().expect("temporary directory");
        let (context, keys) = context_and_keys();
        let origin_view = 1;
        let later_view = 4;
        let origin_leader = usize::try_from(context.leader(origin_view)).expect("leader index");
        let (body, manifest) = body_and_manifest_with_signature_and_views(
            &context,
            &keys[origin_leader],
            u64::try_from(origin_leader).expect("leader index fits u64"),
            later_view,
            origin_view,
        );
        let mut store = V2BodyStore::open(directory.path(), context).expect("open body store");

        let _ = store
            .store(manifest, body)
            .expect("a later reproposal retains the original header leader signature");
    }

    #[test]
    fn body_from_a_future_view_is_rejected() {
        let directory = TempDir::new().expect("temporary directory");
        let (context, keys) = context_and_keys();
        let proposal_view = 2;
        let future_origin_view = 3;
        let future_leader =
            usize::try_from(context.leader(future_origin_view)).expect("leader index");
        let (body, manifest) = body_and_manifest_with_signature_and_views(
            &context,
            &keys[future_leader],
            u64::try_from(future_leader).expect("leader index fits u64"),
            proposal_view,
            future_origin_view,
        );
        let mut store = V2BodyStore::open(directory.path(), context).expect("open body store");

        assert!(matches!(
            store.store(manifest, body),
            Err(V2BodyStoreError::BlockSubjectMismatch)
        ));
    }
}
