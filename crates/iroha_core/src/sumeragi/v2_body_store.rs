//! Crash-safe storage for exact Sumeragi v2 proposal bodies.
//!
//! Consensus votes authenticate a [`wire::BlockSubject`], including the hash of the
//! exact canonical [`SignedBlock`] wire bytes.  This store is the durability
//! boundary between reconstruction and the reducer's `BodyStored` input: a
//! receipt can only be obtained after the bytes, their metadata, and the
//! directory entry have been synchronised.

use std::{
    collections::BTreeMap,
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    mem::size_of,
    path::{Path, PathBuf},
};

use iroha_crypto::{Hash, HashOf, PublicKey};
use iroha_data_model::block::{
    CertifiedMergeLedgerReference, SignedBlock, consensus_v2 as wire, decode_framed_signed_block,
};
use norito::codec::{Decode, DecodeAll as _, Encode};
use thiserror::Error;

use super::v2_effects::{BodyStoreTask, BodyValidationTask, EffectWorkId};
use crate::kura::KuraV2CommitReceipt;

const STORE_MAGIC: &[u8; 8] = b"SUM2BODY";
const VALIDATED_MAGIC: &[u8; 8] = b"SUM2VALD";
const STORE_VERSION: u16 = 1;
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

/// Non-forgeable acknowledgement that deterministic validation succeeded for
/// the exact durable body.
#[derive(Clone, Debug, PartialEq, Eq)]
#[must_use]
pub(crate) struct ValidatedBodyReceipt {
    durable: DurableBodyReceipt,
}

/// Completion minted only after an exact body task reaches durable storage.
#[derive(Clone, Debug, PartialEq, Eq)]
#[must_use]
pub(crate) struct BodyStoreCompletion {
    work_id: EffectWorkId,
    tag: iroha_sumeragi_core::EventTag,
    manifest: wire::PayloadManifest,
    receipt: DurableBodyReceipt,
}

impl BodyStoreCompletion {
    /// Stable asynchronous work identifier.
    pub(crate) const fn work_id(&self) -> EffectWorkId {
        self.work_id
    }

    /// Original reducer event tag.
    pub(crate) const fn tag(&self) -> iroha_sumeragi_core::EventTag {
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
        /// Original reducer event tag.
        tag: iroha_sumeragi_core::EventTag,
        /// Non-forgeable validation receipt.
        receipt: ValidatedBodyReceipt,
    },
    /// Deterministic semantic validation rejected the exact body.
    Rejected {
        /// Stable asynchronous work identifier.
        work_id: EffectWorkId,
        /// Original reducer event tag.
        tag: iroha_sumeragi_core::EventTag,
        /// Deterministic validator diagnostic.
        reason: String,
    },
    /// Validation is sound but cannot finish until the exact certified merge
    /// sidecar referenced by the durable body is fetched and authenticated.
    DeferredMergeSidecar {
        /// Stable asynchronous work identifier retained for the exact retry.
        work_id: EffectWorkId,
        /// Original reducer event tag.
        tag: iroha_sumeragi_core::EventTag,
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

    /// Original reducer event tag.
    pub(crate) const fn tag(&self) -> iroha_sumeragi_core::EventTag {
        match self {
            Self::Validated { tag, .. }
            | Self::Rejected { tag, .. }
            | Self::DeferredMergeSidecar { tag, .. } => *tag,
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
    /// Require the rotating leader selected for the block's immutable origin
    /// view. A later proposal leader authenticates the re-proposal separately.
    RotatingLeader,
    /// Require signature index zero and the configured genesis public key.
    GenesisAuthority(PublicKey),
}

impl ValidatedBodyReceipt {
    /// Durable body receipt whose exact bytes passed validation.
    pub(crate) const fn durable(&self) -> &DurableBodyReceipt {
        &self.durable
    }

    #[cfg(test)]
    pub(crate) fn for_test(durable: DurableBodyReceipt) -> Self {
        Self { durable }
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
pub(crate) struct V2BodyStore {
    context: wire::HeightContext,
    signature_policy: BlockSignaturePolicy,
    directory: PathBuf,
    entries: BTreeMap<(wire::ConsensusRound, wire::BlockSubject), DurableBodyReceipt>,
    manifests: BTreeMap<(wire::ConsensusRound, wire::BlockSubject), wire::PayloadManifest>,
    validated: BTreeMap<(wire::ConsensusRound, wire::BlockSubject), ValidatedBodyReceipt>,
}

impl V2BodyStore {
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
            if store
                .validated
                .insert(key, ValidatedBodyReceipt { durable: receipt })
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

    /// Snapshot validation receipts reconstructed from durable marker files.
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

    /// Execute deterministic validation against the exact durable task body.
    ///
    /// Filesystem loading, canonical decoding, and the validator callback all
    /// run in the caller's storage/validation service context, never on the
    /// serialized reducer owner. Production callers use
    /// `ValidBlock::validate_sumeragi_v2_candidate_keep_voting_block` so the
    /// store-verified immutable-origin signature is not incorrectly rechecked
    /// as the current proposal leader while all transaction/state checks remain.
    pub(crate) fn execute_validation_task<F, E>(
        &mut self,
        task: &BodyValidationTask,
        validator: F,
    ) -> Result<BodyValidationCompletion, V2BodyStoreError>
    where
        F: FnOnce(&SignedBlock) -> Result<(), E>,
        E: BodyValidationError,
    {
        if task.round() != task.durable_receipt().round()
            || task.subject() != task.durable_receipt().subject()
        {
            return Err(V2BodyStoreError::ReceiptMismatch);
        }
        let key = (task.round(), task.subject());
        if let Some(validated) = self.validated.get(&key) {
            if validated.durable() != task.durable_receipt() {
                return Err(V2BodyStoreError::ReceiptMismatch);
            }
            return Ok(BodyValidationCompletion::Validated {
                work_id: task.id(),
                tag: task.tag(),
                receipt: validated.clone(),
            });
        }
        let block = self.load(task.durable_receipt())?;
        match validator(&block) {
            Ok(()) => Ok(BodyValidationCompletion::Validated {
                work_id: task.id(),
                tag: task.tag(),
                receipt: self.persist_validated_receipt(task.durable_receipt())?,
            }),
            Err(error) => {
                if let Some(reference) = error.missing_certified_merge_sidecar() {
                    return Ok(BodyValidationCompletion::DeferredMergeSidecar {
                        work_id: task.id(),
                        tag: task.tag(),
                        reference: reference.clone(),
                    });
                }
                Ok(BodyValidationCompletion::Rejected {
                    work_id: task.id(),
                    tag: task.tag(),
                    reason: error.to_string(),
                })
            }
        }
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
    /// Certified fetch responses and safe locked-subject reproposals must
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
    /// A later certified leader may only re-propose its lock's exact body. The
    /// BTreeMap order makes the selected source deterministic across restart,
    /// while the returned receipt still has to pass the normal frame checks
    /// before bytes can be loaded.
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
        F: FnOnce(&SignedBlock) -> Result<(), E>,
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
        validator(&block)
            .map_err(|error| V2BodyStoreError::DeterministicValidation(error.to_string()))?;
        self.persist_validated_receipt(receipt)
    }

    fn persist_validated_receipt(
        &mut self,
        receipt: &DurableBodyReceipt,
    ) -> Result<ValidatedBodyReceipt, V2BodyStoreError> {
        let key = (receipt.round, receipt.subject);
        if let Some(validated) = self.validated.get(&key) {
            if validated.durable() != receipt {
                return Err(V2BodyStoreError::ReceiptMismatch);
            }
            return Ok(validated.clone());
        }
        let validated = ValidatedBodyReceipt {
            durable: receipt.clone(),
        };
        let marker = ValidatedBodyMarker {
            version: STORE_VERSION,
            context_id: receipt.context_id,
            round: receipt.round,
            subject: receipt.subject,
            manifest_hash: receipt.manifest_hash,
            body_frame_hash: receipt.frame_hash,
        };
        write_validated_marker(
            &self.validated_path_for(receipt.round, receipt.subject),
            &marker,
        )?;
        self.validated.insert(key, validated.clone());
        Ok(validated)
    }

    /// Retire every losing and decided candidate after Kura durably finalizes
    /// the owning height context.
    ///
    /// Once a height has one immutable CommitQC artifact, no candidate body at
    /// that height can be voted on again. Context/height matching is therefore
    /// the authorization boundary for deleting the complete directory; only
    /// the decided candidate additionally matches the receipt's block hash.
    pub(crate) fn retire_height(
        self,
        kura_receipt: &KuraV2CommitReceipt,
    ) -> Result<(), V2BodyStoreError> {
        if kura_receipt.context_id() != self.context.id()
            || kura_receipt.height() != self.context.height
        {
            return Err(V2BodyStoreError::KuraReceiptMismatch);
        }
        let parent = self.directory.parent().map(Path::to_path_buf);
        fs::remove_dir_all(&self.directory).map_err(|source| V2BodyStoreError::Io {
            path: self.directory.clone(),
            source,
        })?;
        if let Some(parent) = parent {
            sync_directory(&parent)?;
        }
        Ok(())
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
        let reencoded = block
            .encode_wire()
            .map_err(|error| V2BodyStoreError::BlockEncode(error.to_string()))?;
        if reencoded != envelope.canonical_wire {
            return Err(V2BodyStoreError::NonCanonicalBlockWire);
        }
        let header = block.header();
        let body_origin_view = header.view_change_index();
        let view_matches = match &self.signature_policy {
            // A later certified leader may re-propose the exact body protected
            // by a lock. Its separately authenticated Proposal uses the new
            // round, while the immutable block retains its creation view.
            BlockSignaturePolicy::RotatingLeader => body_origin_view <= envelope.round.view,
            BlockSignaturePolicy::GenesisAuthority(_) => header.view_change_index() == 0,
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
            .map(|certificate| certificate.subject.block_hash);
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
    /// Validation marker is not bound to the matching exact body frame.
    #[error("Sumeragi v2 validation marker differs from its durable body")]
    ValidationMarkerMismatch,
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
    use std::{cell::Cell, fs, num::NonZeroU64};

    use iroha_crypto::{Algorithm, Hash, HashOf, KeyPair, SignatureOf};
    use iroha_data_model::{
        block::{
            BlockHeader, BlockSignature, CertifiedMergeLedgerReference, SignedBlock,
            consensus_v2 as wire,
        },
        merge::MergeQuorumCertificate,
        peer::PeerId,
    };
    use iroha_sumeragi_core::{EventTag, Generation};
    use tempfile::TempDir;

    use super::{
        BlockSignaturePolicy, BodyValidationCompletion, BodyValidationError, STORE_VERSION,
        V2BodyStore, V2BodyStoreError, ValidatedBodyMarker, ValidatedBodyReceipt,
        write_validated_marker,
    };
    use crate::sumeragi::v2_effects::BodyValidationTask;

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
            mode: wire::ConsensusMode::Permissioned,
            parent_commit_qc: None,
            quorum: wire::DualQuorum::from_roster(&roster).expect("fixture quorum"),
            roster,
            nexus_amx_context_hash: Hash::new(b"test nexus amx context"),
            da_layout: wire::DataAvailabilityLayout {
                encoding: wire::PayloadEncoding::Plain,
                chunk_size_bytes: 1_048_576,
                data_shards: 0,
                parity_shards: 0,
                max_payload_size_bytes: 1_048_576,
                max_chunk_count: 1,
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
        let manifest = wire::PayloadManifest::derive(
            context,
            round,
            subject,
            u64::try_from(canonical_wire.len()).expect("fixture body length"),
            std::slice::from_ref(&canonical_wire),
        )
        .expect("fixture manifest");
        (canonical_wire, manifest)
    }

    #[test]
    fn durable_body_roundtrips_and_reopens_idempotently() {
        let directory = TempDir::new().expect("temporary directory");
        let (context, keys) = context_and_keys();
        let (body, manifest) = body_and_manifest(&context, &keys, None);
        let mut store = V2BodyStore::open(directory.path(), context.clone()).expect("open store");
        let receipt = store
            .store(manifest.clone(), body.clone())
            .expect("store exact body");
        let validated = store
            .validate(&receipt, |block| {
                (block.hash() == receipt.subject().block_hash)
                    .then_some(())
                    .ok_or("wrong block")
            })
            .expect("validate exact durable body");
        assert_eq!(validated.durable(), &receipt);
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
        assert_eq!(
            reopened
                .validated_recovery_catalog()
                .get(&(receipt.round(), receipt.subject()))
                .map(ValidatedBodyReceipt::durable),
            Some(&receipt),
        );
        let callback_ran = Cell::new(false);
        let _validated = reopened
            .validate(&receipt, |_| {
                callback_ran.set(true);
                Err("persisted validation must bypass changed post-apply state")
            })
            .expect("durable validation marker resumes without revalidation");
        assert!(!callback_ran.get());
    }

    #[test]
    fn typed_validation_deferral_and_rejection_never_mint_success_receipts() {
        let directory = TempDir::new().expect("temporary directory");
        let (context, keys) = context_and_keys();
        let (body, manifest) = body_and_manifest(&context, &keys, None);
        let mut store = V2BodyStore::open(directory.path(), context).expect("open store");
        let receipt = store.store(manifest, body).expect("store exact body");
        let task = BodyValidationTask::for_test(
            41,
            EventTag::new(1, 0, Generation::new(1)),
            receipt.clone(),
        );
        let reference = missing_merge_reference(&receipt);

        let deferred = store
            .execute_validation_task(&task, |_| {
                Err::<(), _>(FixtureValidationError::MissingMergeSidecar(
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
                Err::<(), _>(FixtureValidationError::Invalid("invalid candidate"))
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
            .execute_validation_task(&task, |_| Ok::<(), FixtureValidationError>(()))
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
    fn corrupted_or_orphaned_validation_marker_fails_closed() {
        let directory = TempDir::new().expect("temporary directory");
        let (context, keys) = context_and_keys();
        let (body, manifest) = body_and_manifest(&context, &keys, None);
        let mut store = V2BodyStore::open(directory.path(), context.clone()).expect("open store");
        let receipt = store.store(manifest, body).expect("store exact body");
        let _validated = store
            .validate(&receipt, |_| Ok::<_, &'static str>(()))
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
    fn locked_body_keeps_its_origin_signature_when_reproposed_in_a_later_view() {
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

        let _receipt = store
            .store(manifest, body)
            .expect("later proposal retains the locked body's origin signature");
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
