//! Deployment-owned evidence-viewer transparency producer boundary.
//!
//! The producer accepts only a verified [`EvidenceViewerTransparencyProjectionV1`], derives a
//! payload-free public head, and advances a qualified external publisher with compare-and-publish
//! semantics. It has no scheduler, credential loader, network client, private key, or filesystem
//! fallback. Deployments own those concerns and inject the publisher at runtime.
use super::{
    EvidenceViewerRuntimeProviderQualificationErrorV1,
    EvidenceViewerRuntimeProviderQualificationV1, EvidenceViewerRuntimeProviderV1,
    EvidenceViewerSignedCheckpointAnchorV1, EvidenceViewerSignedCompactionArchiveHeadV1,
    EvidenceViewerTransparencyProjectionV1, QualifiedEvidenceViewerProviderV1, is_zero_digest,
};
use iroha_crypto::{Algorithm, PublicKey, Signature as IrohaSignature};
use norito::derive::{NoritoDeserialize, NoritoSerialize};
use std::{fmt, sync::Arc};
use thiserror::Error;
/// Canonical public transparency-head schema version.
pub const EVIDENCE_VIEWER_TRANSPARENCY_HEAD_VERSION_V1: u16 = 1;
const TRANSPARENCY_OPERATION_DOMAIN_V1: &[u8] =
    b"sorafs.evidence-viewer.transparency-publication-operation.v1";
const TRANSPARENCY_SIGNATURE_DOMAIN_V1: &[u8] =
    b"sorafs.evidence-viewer.transparency-publication-signature.v1";
const TRANSPARENCY_HEAD_DOMAIN_V1: &[u8] =
    b"sorafs.evidence-viewer.transparency-publication-head.v1";
/// Credential-free production pins for one external transparency producer.
///
/// Every field is a stable handle, public key, revision, bound, or public
/// policy digest. Credentials, private keys, bearer tokens, and vendor
/// endpoints must remain behind [`EvidenceViewerTransparencyPublisherV1`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvidenceViewerTransparencyProducerConfigV1 {
    /// Governed evidence-viewer checkpoint/receipt signer handle.
    pub receipt_signer_handle: String,
    /// Governed Ed25519 checkpoint/receipt verification key.
    pub receipt_signer_public_key: [u8; 32],
    /// Authoritative checkpoint-store handle carried by signed anchors.
    pub checkpoint_store_handle: String,
    /// Exact checkpoint-store adapter/public-policy revision.
    pub checkpoint_store_revision: u64,
    /// Exact digest of the checkpoint-store public policy.
    pub checkpoint_store_policy_digest: [u8; 32],
    /// Immutable compaction-archive handle.
    pub compaction_archive_handle: String,
    /// Exact compaction-archive adapter/public-policy qualification.
    pub compaction_archive_qualification: EvidenceViewerRuntimeProviderQualificationV1,
    /// Stable public compaction-archive namespace identity.
    pub compaction_archive_id: [u8; 32],
    /// Ed25519 key authenticating exact archive installation/readback.
    pub compaction_archive_public_key: [u8; 32],
    /// External monotonic-head publisher handle.
    pub publisher_handle: String,
    /// Exact publisher adapter/public-policy qualification.
    pub publisher_qualification: EvidenceViewerRuntimeProviderQualificationV1,
    /// Ed25519 key authenticating the externally published public head.
    pub publisher_public_key: [u8; 32],
}
impl EvidenceViewerTransparencyProducerConfigV1 {
    fn validate(&self) -> Result<(), EvidenceViewerTransparencyProducerConstructionErrorV1> {
        for handle in [
            self.receipt_signer_handle.as_str(),
            self.checkpoint_store_handle.as_str(),
            self.compaction_archive_handle.as_str(),
            self.publisher_handle.as_str(),
        ] {
            super::validate_evidence_viewer_runtime_provider_handle(handle, true)?;
        }
        if self.checkpoint_store_revision == 0
            || is_zero_digest(self.checkpoint_store_policy_digest)
            || !self.compaction_archive_qualification.is_valid()
            || is_zero_digest(self.compaction_archive_id)
            || !self.publisher_qualification.is_valid()
            || !is_ed25519_public_key(self.receipt_signer_public_key)
            || !is_ed25519_public_key(self.compaction_archive_public_key)
            || !is_ed25519_public_key(self.publisher_public_key)
        {
            return Err(
                EvidenceViewerTransparencyProducerConstructionErrorV1::InvalidPublicBinding,
            );
        }
        Ok(())
    }
}
/// Exact payload-free body installed under one monotonic public head.
#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
pub struct EvidenceViewerTransparencyHeadBodyV1 {
    /// Public-head schema version.
    pub version: u16,
    /// Monotonic external publication generation, beginning at one.
    pub generation: u64,
    /// Exact predecessor external head digest, absent only at generation one.
    pub predecessor_head_digest: Option<[u8; 32]>,
    /// Deterministic identity of this exact publication attempt.
    pub operation_id: [u8; 32],
    /// Exact signed authoritative checkpoint consumed by the producer.
    pub source_checkpoint_anchor: EvidenceViewerSignedCheckpointAnchorV1,
    /// Exact signed compaction-archive head committed by the checkpoint.
    pub source_compaction_archive_head: Option<EvidenceViewerSignedCompactionArchiveHeadV1>,
    /// Exclusive receipt predecessor supplied to the source projection.
    pub source_predecessor: Option<super::EvidenceViewerReceiptCursorV1>,
    /// Exact bounded source page limit.
    pub source_page_limit: u16,
    /// Whether the acknowledged source projection had another page.
    pub source_has_more: bool,
    /// Exact receipt cursor durably acknowledged by this public head.
    pub receipt_cursor: Option<super::EvidenceViewerReceiptCursorV1>,
    /// Digest of the complete verified source projection.
    pub source_projection_digest: [u8; 32],
    /// Stable external publisher handle.
    pub publisher_handle: String,
    /// Exact external publisher adapter/public-policy revision.
    pub publisher_revision: u64,
    /// Exact digest of the external publisher public policy.
    pub publisher_policy_digest: [u8; 32],
    /// Ed25519 public key authenticating this public head.
    pub publisher_public_key: [u8; 32],
}
/// Ed25519-authenticated public transparency head.
#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
pub struct EvidenceViewerSignedTransparencyHeadV1 {
    /// Exact payload-free public-head body.
    pub body: EvidenceViewerTransparencyHeadBodyV1,
    /// Ed25519 signature over the canonical body.
    pub signature: [u8; 64],
    /// Domain-separated digest of the canonical body and signature.
    pub head_digest: [u8; 32],
}
impl EvidenceViewerSignedTransparencyHeadV1 {
    /// Verify structure, source signatures, configured public identities, the
    /// deterministic operation id, publisher signature, and head digest.
    ///
    /// # Errors
    ///
    /// Rejects every malformed, stale-policy, substituted, forged, or noncanonical head.
    pub fn verify(
        &self,
        config: &EvidenceViewerTransparencyProducerConfigV1,
    ) -> Result<(), EvidenceViewerTransparencyProducerErrorV1> {
        let body = &self.body;
        let lineage_is_valid = match body.generation {
            1 => body.predecessor_head_digest.is_none(),
            2.. => body
                .predecessor_head_digest
                .is_some_and(|digest| !is_zero_digest(digest)),
            0 => false,
        };
        if body.version != EVIDENCE_VIEWER_TRANSPARENCY_HEAD_VERSION_V1
            || !lineage_is_valid
            || is_zero_digest(body.operation_id)
            || is_zero_digest(body.source_projection_digest)
            || body.source_page_limit == 0
            || usize::from(body.source_page_limit) > 1_024
            || body.publisher_handle != config.publisher_handle
            || body.publisher_revision != config.publisher_qualification.revision()
            || body.publisher_policy_digest != config.publisher_qualification.policy_digest()
            || body.publisher_public_key != config.publisher_public_key
        {
            return Err(EvidenceViewerTransparencyProducerErrorV1::InvalidPublishedHead);
        }
        body.source_checkpoint_anchor
            .verify(
                &config.receipt_signer_handle,
                config.receipt_signer_public_key,
            )
            .map_err(|_| EvidenceViewerTransparencyProducerErrorV1::InvalidPublishedHead)?;
        verify_checkpoint_store_binding(config, &body.source_checkpoint_anchor)?;
        verify_archive_binding(config, body.source_compaction_archive_head.as_ref())?;
        if body.source_checkpoint_anchor.compaction_archive_head_digest
            != body
                .source_compaction_archive_head
                .as_ref()
                .map(|head| head.head_digest)
            || !cursor_is_bounded_by_checkpoint(body.receipt_cursor, &body.source_checkpoint_anchor)
            || !source_cursor_is_consistent(body)
            || body.operation_id != transparency_operation_id(body)?
        {
            return Err(EvidenceViewerTransparencyProducerErrorV1::InvalidPublishedHead);
        }
        let key = PublicKey::from_bytes(Algorithm::Ed25519, &body.publisher_public_key)
            .map_err(|_| EvidenceViewerTransparencyProducerErrorV1::InvalidPublishedHead)?;
        let signature = IrohaSignature::try_from_bytes(&self.signature)
            .map_err(|_| EvidenceViewerTransparencyProducerErrorV1::InvalidPublishedHead)?;
        signature
            .verify(&key, &transparency_signature_message(body)?)
            .map_err(|_| EvidenceViewerTransparencyProducerErrorV1::InvalidPublishedHead)?;
        if self.head_digest != transparency_head_digest(body, self.signature)? {
            return Err(EvidenceViewerTransparencyProducerErrorV1::InvalidPublishedHead);
        }
        Ok(())
    }
}
/// Fixed payload-free failures returned by the deployment-owned publisher.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum EvidenceViewerTransparencyPublisherExternalErrorV1 {
    /// Publisher or a required credential is temporarily unavailable.
    #[error("evidence-viewer transparency publisher unavailable")]
    Unavailable,
    /// Publisher rejected the exact request or predecessor fence.
    #[error("evidence-viewer transparency publisher rejected request")]
    Rejected,
    /// Publisher is saturated.
    #[error("evidence-viewer transparency publisher backpressure")]
    Backpressure,
    /// Commit outcome is unknown and requires authoritative readback.
    #[error("evidence-viewer transparency publication outcome ambiguous")]
    Ambiguous,
}
/// Deployment-owned monotonic public-head authority.
///
/// `compare_and_publish` must durably install `body` only when the currently published head digest
/// equals `body.predecessor_head_digest`. An exact retry must be idempotent. `load_head` must
/// return the authoritative signed readback, never a process-local cache. Credentials, private
/// keys, endpoint discovery, and vendor diagnostics remain inside the implementation.
pub trait EvidenceViewerTransparencyPublisherV1: EvidenceViewerRuntimeProviderV1 {
    /// Return the governed Ed25519 public verification key.
    fn public_key(&self) -> [u8; 32];
    /// Load the exact authoritative public head.
    ///
    /// # Errors
    ///
    /// Returns only a fixed payload-free external failure.
    fn load_head(
        &self,
    ) -> Result<
        Option<EvidenceViewerSignedTransparencyHeadV1>,
        EvidenceViewerTransparencyPublisherExternalErrorV1,
    >;
    /// Atomically install one exact public-head body.
    ///
    /// The implementation signs the canonical body and persists the complete
    /// [`EvidenceViewerSignedTransparencyHeadV1`] before returning success.
    ///
    /// # Errors
    ///
    /// Returns `Ambiguous` whenever durability cannot be determined.
    fn compare_and_publish(
        &self,
        body: &EvidenceViewerTransparencyHeadBodyV1,
    ) -> Result<(), EvidenceViewerTransparencyPublisherExternalErrorV1>;
}
/// Startup failures for optional producer construction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum EvidenceViewerTransparencyProducerConstructionErrorV1 {
    /// Producer configuration was supplied without a publisher.
    #[error("enabled evidence-viewer transparency producer is missing its runtime publisher")]
    MissingPublisher,
    /// A publisher was injected while the producer was disabled.
    #[error("unrequested evidence-viewer transparency publisher was injected")]
    UnexpectedPublisher,
    /// A configured public binding is malformed or cryptographically invalid.
    #[error("evidence-viewer transparency producer public binding is invalid")]
    InvalidPublicBinding,
    /// Runtime publisher identity, readiness, or qualification failed closed.
    #[error(transparent)]
    PublisherQualification(#[from] EvidenceViewerRuntimeProviderQualificationErrorV1),
    /// Runtime publisher verification key differs from configuration.
    #[error("evidence-viewer transparency publisher public key does not match configuration")]
    PublisherPublicKeyMismatch,
}
/// Runtime failures for publication and reconciliation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum EvidenceViewerTransparencyProducerErrorV1 {
    /// Source projection signature, chain, cursor, or digest verification failed.
    #[error("evidence-viewer transparency source projection is invalid")]
    InvalidProjection,
    /// Source checkpoint-store identity or policy differs from configuration.
    #[error("evidence-viewer transparency checkpoint-store binding is invalid")]
    CheckpointStoreBindingMismatch,
    /// Source compaction archive identity, key, or policy differs from configuration.
    #[error("evidence-viewer transparency compaction-archive binding is invalid")]
    ArchiveBindingMismatch,
    /// Source checkpoint generation or exact receipt cursor regressed or forked.
    #[error("evidence-viewer transparency source lineage is not monotonic")]
    SourceLineageConflict,
    /// Publisher is temporarily unavailable or changed identity/policy.
    #[error("evidence-viewer transparency publisher unavailable")]
    PublisherUnavailable,
    /// Publisher is saturated.
    #[error("evidence-viewer transparency publisher backpressure")]
    PublisherBackpressure,
    /// Publisher rejected the exact predecessor fence or candidate.
    #[error("evidence-viewer transparency publisher rejected publication")]
    PublicationRejected,
    /// Ambiguous publication did not reconcile to the exact candidate.
    #[error("evidence-viewer transparency publication remains ambiguous")]
    PublicationAmbiguous,
    /// Authoritative publisher readback is missing, forged, or substituted.
    #[error("evidence-viewer transparency published head is invalid")]
    InvalidPublishedHead,
    /// A monotonic generation counter overflowed.
    #[error("evidence-viewer transparency generation exhausted")]
    GenerationExhausted,
    /// Canonical Norito encoding failed.
    #[error("evidence-viewer transparency canonical encoding failed")]
    CanonicalEncoding,
}
/// Outcome of one exact source-projection publication attempt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EvidenceViewerTransparencyProducerOutcomeV1 {
    /// A new public head was durably installed and read back.
    Published(EvidenceViewerSignedTransparencyHeadV1),
    /// The authoritative public head already acknowledges the same exact state.
    AlreadyCurrent(EvidenceViewerSignedTransparencyHeadV1),
}
struct QualifiedEvidenceViewerTransparencyPublisherV1 {
    inner: QualifiedEvidenceViewerProviderV1<dyn EvidenceViewerTransparencyPublisherV1>,
    public_key: [u8; 32],
}
impl QualifiedEvidenceViewerTransparencyPublisherV1 {
    fn try_new(
        config: &EvidenceViewerTransparencyProducerConfigV1,
        publisher: Arc<dyn EvidenceViewerTransparencyPublisherV1>,
    ) -> Result<Self, EvidenceViewerTransparencyProducerConstructionErrorV1> {
        let inner = QualifiedEvidenceViewerProviderV1::try_new(
            &config.publisher_handle,
            config.publisher_qualification,
            publisher,
        )?;
        let public_key = Self::read_qualified_public_key(&inner).map_err(
            EvidenceViewerTransparencyProducerConstructionErrorV1::PublisherQualification,
        )?;
        if public_key != config.publisher_public_key {
            return Err(
                EvidenceViewerTransparencyProducerConstructionErrorV1::PublisherPublicKeyMismatch,
            );
        }
        Ok(Self {
            inner,
            public_key: config.publisher_public_key,
        })
    }
    fn read_qualified_public_key(
        inner: &QualifiedEvidenceViewerProviderV1<dyn EvidenceViewerTransparencyPublisherV1>,
    ) -> Result<[u8; 32], EvidenceViewerRuntimeProviderQualificationErrorV1> {
        inner.revalidate()?;
        let key = inner.provider.public_key();
        inner.revalidate()?;
        Ok(key)
    }
    fn revalidate_identity(&self) -> Result<(), EvidenceViewerTransparencyProducerErrorV1> {
        let key = Self::read_qualified_public_key(&self.inner)
            .map_err(|_| EvidenceViewerTransparencyProducerErrorV1::PublisherUnavailable)?;
        if key != self.public_key {
            return Err(EvidenceViewerTransparencyProducerErrorV1::PublisherUnavailable);
        }
        Ok(())
    }
    fn load_head(
        &self,
    ) -> Result<
        Option<EvidenceViewerSignedTransparencyHeadV1>,
        EvidenceViewerTransparencyProducerErrorV1,
    > {
        self.revalidate_identity()?;
        let result = self.inner.provider.load_head();
        self.revalidate_identity()?;
        result.map_err(map_publisher_external_error)
    }
    fn compare_and_publish(
        &self,
        body: &EvidenceViewerTransparencyHeadBodyV1,
    ) -> Result<(), EvidenceViewerTransparencyPublisherExternalErrorV1> {
        self.revalidate_identity()
            .map_err(|_| EvidenceViewerTransparencyPublisherExternalErrorV1::Unavailable)?;
        let result = self.inner.provider.compare_and_publish(body);
        self.revalidate_identity()
            .map_err(|_| EvidenceViewerTransparencyPublisherExternalErrorV1::Unavailable)?;
        result
    }
}
impl fmt::Debug for QualifiedEvidenceViewerTransparencyPublisherV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("QualifiedEvidenceViewerTransparencyPublisherV1")
            .field("handle", &self.inner.handle)
            .field("qualification", &self.inner.qualification)
            .field("public_key", &self.public_key)
            .field("provider", &"<runtime-only>")
            .finish()
    }
}
/// Stateless producer coordinating one qualified authoritative publisher.
///
/// Durable cursor state lives exclusively in the external signed public head, so replica failover
/// and restart always reconcile against the same authoritative value.
pub struct EvidenceViewerTransparencyProducerV1 {
    config: EvidenceViewerTransparencyProducerConfigV1,
    publisher: QualifiedEvidenceViewerTransparencyPublisherV1,
}
impl EvidenceViewerTransparencyProducerV1 {
    /// Construct an enabled producer from public configuration and an injected
    /// deployment-owned publisher.
    ///
    /// # Errors
    ///
    /// Fails startup for malformed, unavailable, stale, test-marked,
    /// substituted, or key-mismatched publishers.
    pub fn try_new(
        config: EvidenceViewerTransparencyProducerConfigV1,
        publisher: Arc<dyn EvidenceViewerTransparencyPublisherV1>,
    ) -> Result<Self, EvidenceViewerTransparencyProducerConstructionErrorV1> {
        config.validate()?;
        let publisher =
            QualifiedEvidenceViewerTransparencyPublisherV1::try_new(&config, publisher)?;
        Ok(Self { config, publisher })
    }
    /// Resolve an optional producer without accepting partial or unrequested runtime injection.
    ///
    /// # Errors
    ///
    /// `(Some, None)` fails as missing and `(None, Some)` fails as
    /// unrequested. `(None, None)` is the only disabled form.
    pub fn try_from_optional(
        config: Option<EvidenceViewerTransparencyProducerConfigV1>,
        publisher: Option<Arc<dyn EvidenceViewerTransparencyPublisherV1>>,
    ) -> Result<Option<Self>, EvidenceViewerTransparencyProducerConstructionErrorV1> {
        match (config, publisher) {
            (None, None) => Ok(None),
            (Some(_), None) => {
                Err(EvidenceViewerTransparencyProducerConstructionErrorV1::MissingPublisher)
            }
            (None, Some(_)) => {
                Err(EvidenceViewerTransparencyProducerConstructionErrorV1::UnexpectedPublisher)
            }
            (Some(config), Some(publisher)) => Self::try_new(config, publisher).map(Some),
        }
    }
    /// Load and verify the exact authoritative public head.
    ///
    /// # Errors
    ///
    /// Fails closed for publisher drift, unavailability, malformed readback,
    /// signature failure, or configured identity substitution.
    pub fn reconcile(
        &self,
    ) -> Result<
        Option<EvidenceViewerSignedTransparencyHeadV1>,
        EvidenceViewerTransparencyProducerErrorV1,
    > {
        let current = self.publisher.load_head()?;
        if let Some(head) = current.as_ref() {
            head.verify(&self.config)?;
        }
        Ok(current)
    }
    /// Verify and durably publish one exact bounded source projection.
    ///
    /// No source state other than the signed projection is accepted. Success is
    /// returned only after authoritative signed readback equals the exact
    /// candidate. Ambiguous commits are reconciled by the same readback path.
    ///
    /// # Errors
    ///
    /// Fails for invalid source signatures, cursor/checkpoint/archive rollback, publisher drift,
    /// CAS rejection, ambiguous nonmatching readback, or malformed publication state.
    pub fn publish_projection(
        &self,
        projection: &EvidenceViewerTransparencyProjectionV1,
    ) -> Result<
        EvidenceViewerTransparencyProducerOutcomeV1,
        EvidenceViewerTransparencyProducerErrorV1,
    > {
        projection
            .verify(
                &self.config.receipt_signer_handle,
                self.config.receipt_signer_public_key,
            )
            .map_err(|_| EvidenceViewerTransparencyProducerErrorV1::InvalidProjection)?;
        verify_checkpoint_store_binding(&self.config, &projection.checkpoint_anchor)?;
        verify_archive_binding(&self.config, projection.compaction_archive_head.as_ref())?;
        let current = self.reconcile()?;
        if let Some(current) = current.as_ref() {
            if public_source_is_current(current, projection) {
                return Ok(EvidenceViewerTransparencyProducerOutcomeV1::AlreadyCurrent(
                    current.clone(),
                ));
            }
            validate_source_successor(current, projection)?;
        } else if projection.predecessor.is_some() {
            return Err(EvidenceViewerTransparencyProducerErrorV1::SourceLineageConflict);
        }
        let mut body = EvidenceViewerTransparencyHeadBodyV1 {
            version: EVIDENCE_VIEWER_TRANSPARENCY_HEAD_VERSION_V1,
            generation: match current.as_ref() {
                Some(head) => head
                    .body
                    .generation
                    .checked_add(1)
                    .ok_or(EvidenceViewerTransparencyProducerErrorV1::GenerationExhausted)?,
                None => 1,
            },
            predecessor_head_digest: current.as_ref().map(|head| head.head_digest),
            operation_id: [0; 32],
            source_checkpoint_anchor: projection.checkpoint_anchor.clone(),
            source_compaction_archive_head: projection.compaction_archive_head.clone(),
            source_predecessor: projection.predecessor,
            source_page_limit: projection.page_limit,
            source_has_more: projection.has_more,
            receipt_cursor: projection.next_cursor,
            source_projection_digest: projection.projection_digest,
            publisher_handle: self.config.publisher_handle.clone(),
            publisher_revision: self.config.publisher_qualification.revision(),
            publisher_policy_digest: self.config.publisher_qualification.policy_digest(),
            publisher_public_key: self.config.publisher_public_key,
        };
        body.operation_id = transparency_operation_id(&body)?;
        let publish_result = self.publisher.compare_and_publish(&body);
        let publish_was_rejected = matches!(
            publish_result,
            Err(EvidenceViewerTransparencyPublisherExternalErrorV1::Rejected)
        );
        match publish_result {
            Ok(())
            | Err(
                EvidenceViewerTransparencyPublisherExternalErrorV1::Rejected
                | EvidenceViewerTransparencyPublisherExternalErrorV1::Ambiguous,
            ) => {}
            Err(error) => return Err(map_publisher_external_error(error)),
        }
        let readback = self.publisher.load_head()?.ok_or(if publish_was_rejected {
            EvidenceViewerTransparencyProducerErrorV1::PublicationRejected
        } else {
            EvidenceViewerTransparencyProducerErrorV1::PublicationAmbiguous
        })?;
        readback.verify(&self.config)?;
        if readback.body != body {
            return Err(if publish_was_rejected {
                EvidenceViewerTransparencyProducerErrorV1::PublicationRejected
            } else {
                EvidenceViewerTransparencyProducerErrorV1::PublicationAmbiguous
            });
        }
        Ok(EvidenceViewerTransparencyProducerOutcomeV1::Published(
            readback,
        ))
    }
}
impl fmt::Debug for EvidenceViewerTransparencyProducerV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("EvidenceViewerTransparencyProducerV1")
            .field("config", &self.config)
            .field("publisher", &self.publisher)
            .finish()
    }
}
fn validate_source_successor(
    current: &EvidenceViewerSignedTransparencyHeadV1,
    projection: &EvidenceViewerTransparencyProjectionV1,
) -> Result<(), EvidenceViewerTransparencyProducerErrorV1> {
    let previous = &current.body.source_checkpoint_anchor;
    let candidate = &projection.checkpoint_anchor;
    if projection.predecessor != current.body.receipt_cursor
        || candidate.checkpoint_generation < previous.checkpoint_generation
        || (candidate.checkpoint_generation == previous.checkpoint_generation
            && candidate != previous)
        || (candidate.checkpoint_generation == previous.checkpoint_generation.saturating_add(1)
            && candidate.predecessor_checkpoint_digest != Some(previous.checkpoint_digest))
        || candidate.receipt_count < previous.receipt_count
        || (candidate.receipt_count == previous.receipt_count
            && candidate.chain_head != previous.chain_head)
    {
        return Err(EvidenceViewerTransparencyProducerErrorV1::SourceLineageConflict);
    }
    validate_archive_successor(
        current.body.source_compaction_archive_head.as_ref(),
        projection.compaction_archive_head.as_ref(),
    )
}
fn validate_archive_successor(
    previous: Option<&EvidenceViewerSignedCompactionArchiveHeadV1>,
    candidate: Option<&EvidenceViewerSignedCompactionArchiveHeadV1>,
) -> Result<(), EvidenceViewerTransparencyProducerErrorV1> {
    match (previous, candidate) {
        (None, None) => Ok(()),
        (Some(_), None) => Err(EvidenceViewerTransparencyProducerErrorV1::SourceLineageConflict),
        (None, Some(_)) => Ok(()),
        (Some(previous), Some(candidate)) if candidate == previous => Ok(()),
        (Some(previous), Some(candidate)) => {
            if candidate.generation <= previous.generation
                || (candidate.generation == previous.generation.saturating_add(1)
                    && (candidate.predecessor_head_digest != Some(previous.head_digest)
                        || candidate.predecessor_operation_id != Some(previous.operation_id)))
            {
                return Err(EvidenceViewerTransparencyProducerErrorV1::SourceLineageConflict);
            }
            Ok(())
        }
    }
}
fn public_source_is_current(
    current: &EvidenceViewerSignedTransparencyHeadV1,
    projection: &EvidenceViewerTransparencyProjectionV1,
) -> bool {
    current.body.source_checkpoint_anchor == projection.checkpoint_anchor
        && current.body.source_compaction_archive_head == projection.compaction_archive_head
        && current.body.source_predecessor == projection.predecessor
        && current.body.source_page_limit == projection.page_limit
        && current.body.source_has_more == projection.has_more
        && current.body.receipt_cursor == projection.next_cursor
        && current.body.source_projection_digest == projection.projection_digest
}
fn verify_checkpoint_store_binding(
    config: &EvidenceViewerTransparencyProducerConfigV1,
    anchor: &EvidenceViewerSignedCheckpointAnchorV1,
) -> Result<(), EvidenceViewerTransparencyProducerErrorV1> {
    if anchor.checkpoint_store_handle != config.checkpoint_store_handle
        || anchor.checkpoint_store_revision != config.checkpoint_store_revision
        || anchor.checkpoint_store_policy_digest != config.checkpoint_store_policy_digest
    {
        return Err(EvidenceViewerTransparencyProducerErrorV1::CheckpointStoreBindingMismatch);
    }
    Ok(())
}
fn verify_archive_binding(
    config: &EvidenceViewerTransparencyProducerConfigV1,
    head: Option<&EvidenceViewerSignedCompactionArchiveHeadV1>,
) -> Result<(), EvidenceViewerTransparencyProducerErrorV1> {
    let Some(head) = head else {
        return Ok(());
    };
    head.verify(
        &config.receipt_signer_handle,
        config.receipt_signer_public_key,
    )
    .map_err(|_| EvidenceViewerTransparencyProducerErrorV1::ArchiveBindingMismatch)?;
    verify_checkpoint_store_binding(config, &head.source_checkpoint_anchor)
        .map_err(|_| EvidenceViewerTransparencyProducerErrorV1::ArchiveBindingMismatch)?;
    if head.archive_handle != config.compaction_archive_handle
        || head.archive_revision != config.compaction_archive_qualification.revision()
        || head.archive_policy_digest != config.compaction_archive_qualification.policy_digest()
        || head.archive_id != config.compaction_archive_id
        || head.archive_public_key != config.compaction_archive_public_key
    {
        return Err(EvidenceViewerTransparencyProducerErrorV1::ArchiveBindingMismatch);
    }
    Ok(())
}
fn cursor_is_bounded_by_checkpoint(
    cursor: Option<super::EvidenceViewerReceiptCursorV1>,
    anchor: &EvidenceViewerSignedCheckpointAnchorV1,
) -> bool {
    match (cursor, anchor.chain_head) {
        (None, None) => anchor.receipt_count == 0,
        (Some(cursor), Some(head)) => {
            cursor.sequence != 0
                && !is_zero_digest(cursor.receipt_digest)
                && cursor.sequence <= head.sequence
                && (cursor.sequence != head.sequence || cursor == head)
        }
        (None, Some(_)) | (Some(_), None) => false,
    }
}
fn source_cursor_is_consistent(body: &EvidenceViewerTransparencyHeadBodyV1) -> bool {
    let continuation_is_valid = match (
        body.source_has_more,
        body.receipt_cursor,
        body.source_checkpoint_anchor.chain_head,
    ) {
        (false, cursor, head) => cursor == head,
        (true, Some(cursor), Some(head)) => cursor.sequence < head.sequence,
        _ => false,
    };
    let predecessor_is_valid = match (body.source_predecessor, body.receipt_cursor) {
        (None, _) => true,
        (Some(predecessor), Some(cursor)) => {
            predecessor.sequence != 0
                && !is_zero_digest(predecessor.receipt_digest)
                && (predecessor.sequence < cursor.sequence || predecessor == cursor)
        }
        (Some(_), None) => false,
    };
    continuation_is_valid && predecessor_is_valid
}
fn transparency_operation_id(
    body: &EvidenceViewerTransparencyHeadBodyV1,
) -> Result<[u8; 32], EvidenceViewerTransparencyProducerErrorV1> {
    let mut canonical = body.clone();
    canonical.operation_id = [0; 32];
    let bytes = norito::to_bytes(&canonical)
        .map_err(|_| EvidenceViewerTransparencyProducerErrorV1::CanonicalEncoding)?;
    let mut hasher = blake3::Hasher::new();
    hasher.update(TRANSPARENCY_OPERATION_DOMAIN_V1);
    hasher.update(&bytes);
    Ok(*hasher.finalize().as_bytes())
}
fn transparency_signature_message(
    body: &EvidenceViewerTransparencyHeadBodyV1,
) -> Result<Vec<u8>, EvidenceViewerTransparencyProducerErrorV1> {
    let bytes = norito::to_bytes(body)
        .map_err(|_| EvidenceViewerTransparencyProducerErrorV1::CanonicalEncoding)?;
    let mut message = Vec::with_capacity(TRANSPARENCY_SIGNATURE_DOMAIN_V1.len() + bytes.len());
    message.extend_from_slice(TRANSPARENCY_SIGNATURE_DOMAIN_V1);
    message.extend_from_slice(&bytes);
    Ok(message)
}
fn transparency_head_digest(
    body: &EvidenceViewerTransparencyHeadBodyV1,
    signature: [u8; 64],
) -> Result<[u8; 32], EvidenceViewerTransparencyProducerErrorV1> {
    let bytes = norito::to_bytes(body)
        .map_err(|_| EvidenceViewerTransparencyProducerErrorV1::CanonicalEncoding)?;
    let mut hasher = blake3::Hasher::new();
    hasher.update(TRANSPARENCY_HEAD_DOMAIN_V1);
    hasher.update(&bytes);
    hasher.update(&signature);
    Ok(*hasher.finalize().as_bytes())
}
fn is_ed25519_public_key(bytes: [u8; 32]) -> bool {
    !is_zero_digest(bytes) && PublicKey::from_bytes(Algorithm::Ed25519, &bytes).is_ok()
}
fn map_publisher_external_error(
    error: EvidenceViewerTransparencyPublisherExternalErrorV1,
) -> EvidenceViewerTransparencyProducerErrorV1 {
    match error {
        EvidenceViewerTransparencyPublisherExternalErrorV1::Unavailable => {
            EvidenceViewerTransparencyProducerErrorV1::PublisherUnavailable
        }
        EvidenceViewerTransparencyPublisherExternalErrorV1::Backpressure => {
            EvidenceViewerTransparencyProducerErrorV1::PublisherBackpressure
        }
        EvidenceViewerTransparencyPublisherExternalErrorV1::Rejected => {
            EvidenceViewerTransparencyProducerErrorV1::PublicationRejected
        }
        EvidenceViewerTransparencyPublisherExternalErrorV1::Ambiguous => {
            EvidenceViewerTransparencyProducerErrorV1::PublicationAmbiguous
        }
    }
}
#[cfg(test)]
mod tests {
    use super::super::{
        EVIDENCE_VIEWER_CHECKPOINT_VERSION_V1, EVIDENCE_VIEWER_COMPACTION_ARCHIVE_VERSION_V1,
        EVIDENCE_VIEWER_RECEIPT_VERSION_V1, EVIDENCE_VIEWER_TRANSPARENCY_PROJECTION_VERSION_V1,
        EvidenceViewerReceiptBodyV1, EvidenceViewerReceiptKindV1,
        EvidenceViewerRuntimeProviderReadinessErrorV1, EvidenceViewerSignedReceiptV1,
        checkpoint_anchor_signature_message, compaction_archive_head_digest,
        compaction_archive_operation_id, compaction_archive_receipt_message,
        compaction_archive_signature_message, receipt_body_digest, receipt_signature_message,
        transparency_projection_digest,
    };
    use super::*;
    use ed25519_dalek::{Signer as _, SigningKey};
    use std::sync::{
        Mutex,
        atomic::{AtomicBool, Ordering},
    };
    const RECEIPT_SIGNER_HANDLE: &str = "provider:prod-evidence-receipts";
    const CHECKPOINT_STORE_HANDLE: &str = "sealed:prod-evidence-checkpoints";
    const ARCHIVE_HANDLE: &str = "object-lock:prod-evidence-archive";
    const PUBLISHER_HANDLE: &str = "provider:prod-evidence-transparency";
    const CHECKPOINT_STORE_REVISION: u64 = 4;
    const CHECKPOINT_STORE_POLICY_DIGEST: [u8; 32] = [0x41; 32];
    const ARCHIVE_QUALIFICATION: EvidenceViewerRuntimeProviderQualificationV1 =
        EvidenceViewerRuntimeProviderQualificationV1::new(5, [0x51; 32]);
    const ARCHIVE_ID: [u8; 32] = [0x52; 32];
    const PUBLISHER_QUALIFICATION: EvidenceViewerRuntimeProviderQualificationV1 =
        EvidenceViewerRuntimeProviderQualificationV1::new(6, [0x61; 32]);
    const RECEIPT_SIGNING_SEED: [u8; 32] = [0x71; 32];
    const ARCHIVE_SIGNING_SEED: [u8; 32] = [0x72; 32];
    const PUBLISHER_SIGNING_SEED: [u8; 32] = [0x73; 32];
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum PublishMode {
        Definite,
        CommitThenAmbiguous,
    }
    struct FakePublisher {
        handle: String,
        qualification: EvidenceViewerRuntimeProviderQualificationV1,
        signing_key: SigningKey,
        stale: AtomicBool,
        substituted_key: AtomicBool,
        mode: Mutex<PublishMode>,
        head: Mutex<Option<EvidenceViewerSignedTransparencyHeadV1>>,
    }
    impl fmt::Debug for FakePublisher {
        fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            formatter
                .debug_struct("FakePublisher")
                .field("handle", &self.handle)
                .field("qualification", &self.qualification)
                .field("signing_key", &"<runtime-only>")
                .finish()
        }
    }
    impl FakePublisher {
        fn new(handle: &str) -> Self {
            Self {
                handle: handle.to_owned(),
                qualification: PUBLISHER_QUALIFICATION,
                signing_key: SigningKey::from_bytes(&PUBLISHER_SIGNING_SEED),
                stale: AtomicBool::new(false),
                substituted_key: AtomicBool::new(false),
                mode: Mutex::new(PublishMode::Definite),
                head: Mutex::new(None),
            }
        }
        fn set_stale(&self, stale: bool) {
            self.stale.store(stale, Ordering::SeqCst);
        }
        fn set_substituted_key(&self, substituted: bool) {
            self.substituted_key.store(substituted, Ordering::SeqCst);
        }
        fn set_mode(&self, mode: PublishMode) {
            *self.mode.lock().expect("publisher mode lock") = mode;
        }
        fn sign_head(
            &self,
            body: EvidenceViewerTransparencyHeadBodyV1,
        ) -> EvidenceViewerSignedTransparencyHeadV1 {
            let signature = self
                .signing_key
                .sign(&transparency_signature_message(&body).expect("signature message"))
                .to_bytes();
            let head_digest =
                transparency_head_digest(&body, signature).expect("public head digest");
            EvidenceViewerSignedTransparencyHeadV1 {
                body,
                signature,
                head_digest,
            }
        }
    }
    impl EvidenceViewerRuntimeProviderV1 for FakePublisher {
        fn handle(&self) -> &str {
            &self.handle
        }
        fn qualification(
            &self,
        ) -> Result<
            EvidenceViewerRuntimeProviderQualificationV1,
            EvidenceViewerRuntimeProviderReadinessErrorV1,
        > {
            if self.stale.load(Ordering::SeqCst) {
                Err(EvidenceViewerRuntimeProviderReadinessErrorV1::Rejected)
            } else {
                Ok(self.qualification)
            }
        }
    }
    impl EvidenceViewerTransparencyPublisherV1 for FakePublisher {
        fn public_key(&self) -> [u8; 32] {
            if self.substituted_key.load(Ordering::SeqCst) {
                SigningKey::from_bytes(&[0x7F; 32])
                    .verifying_key()
                    .to_bytes()
            } else {
                self.signing_key.verifying_key().to_bytes()
            }
        }
        fn load_head(
            &self,
        ) -> Result<
            Option<EvidenceViewerSignedTransparencyHeadV1>,
            EvidenceViewerTransparencyPublisherExternalErrorV1,
        > {
            self.head
                .lock()
                .map(|head| head.clone())
                .map_err(|_| EvidenceViewerTransparencyPublisherExternalErrorV1::Unavailable)
        }
        fn compare_and_publish(
            &self,
            body: &EvidenceViewerTransparencyHeadBodyV1,
        ) -> Result<(), EvidenceViewerTransparencyPublisherExternalErrorV1> {
            let mut head = self
                .head
                .lock()
                .map_err(|_| EvidenceViewerTransparencyPublisherExternalErrorV1::Unavailable)?;
            if head.as_ref().is_some_and(|current| current.body == *body) {
                return Ok(());
            }
            if head.as_ref().map(|current| current.head_digest) != body.predecessor_head_digest {
                return Err(EvidenceViewerTransparencyPublisherExternalErrorV1::Rejected);
            }
            *head = Some(self.sign_head(body.clone()));
            let mut mode = self
                .mode
                .lock()
                .map_err(|_| EvidenceViewerTransparencyPublisherExternalErrorV1::Unavailable)?;
            if *mode == PublishMode::CommitThenAmbiguous {
                *mode = PublishMode::Definite;
                return Err(EvidenceViewerTransparencyPublisherExternalErrorV1::Ambiguous);
            }
            Ok(())
        }
    }
    fn receipt_signing_key() -> SigningKey {
        SigningKey::from_bytes(&RECEIPT_SIGNING_SEED)
    }
    fn archive_signing_key() -> SigningKey {
        SigningKey::from_bytes(&ARCHIVE_SIGNING_SEED)
    }
    fn config() -> EvidenceViewerTransparencyProducerConfigV1 {
        EvidenceViewerTransparencyProducerConfigV1 {
            receipt_signer_handle: RECEIPT_SIGNER_HANDLE.to_owned(),
            receipt_signer_public_key: receipt_signing_key().verifying_key().to_bytes(),
            checkpoint_store_handle: CHECKPOINT_STORE_HANDLE.to_owned(),
            checkpoint_store_revision: CHECKPOINT_STORE_REVISION,
            checkpoint_store_policy_digest: CHECKPOINT_STORE_POLICY_DIGEST,
            compaction_archive_handle: ARCHIVE_HANDLE.to_owned(),
            compaction_archive_qualification: ARCHIVE_QUALIFICATION,
            compaction_archive_id: ARCHIVE_ID,
            compaction_archive_public_key: SigningKey::from_bytes(&ARCHIVE_SIGNING_SEED)
                .verifying_key()
                .to_bytes(),
            publisher_handle: PUBLISHER_HANDLE.to_owned(),
            publisher_qualification: PUBLISHER_QUALIFICATION,
            publisher_public_key: SigningKey::from_bytes(&PUBLISHER_SIGNING_SEED)
                .verifying_key()
                .to_bytes(),
        }
    }
    fn signed_anchor(
        checkpoint_generation: u64,
        predecessor_checkpoint_digest: Option<[u8; 32]>,
        checkpoint_digest: [u8; 32],
        chain_head: Option<super::super::EvidenceViewerReceiptCursorV1>,
    ) -> EvidenceViewerSignedCheckpointAnchorV1 {
        let signing_key = receipt_signing_key();
        let mut anchor = EvidenceViewerSignedCheckpointAnchorV1 {
            version: EVIDENCE_VIEWER_CHECKPOINT_VERSION_V1,
            checkpoint_generation,
            predecessor_checkpoint_revision: predecessor_checkpoint_digest.map(|digest| {
                let mut hasher = blake3::Hasher::new();
                hasher.update(b"test-checkpoint-revision");
                hasher.update(&digest);
                *hasher.finalize().as_bytes()
            }),
            predecessor_checkpoint_digest,
            checkpoint_digest,
            receipt_count: chain_head.map_or(0, |cursor| cursor.sequence),
            chain_head,
            compaction_archive_head_digest: None,
            checkpoint_store_handle: CHECKPOINT_STORE_HANDLE.to_owned(),
            checkpoint_store_revision: CHECKPOINT_STORE_REVISION,
            checkpoint_store_policy_digest: CHECKPOINT_STORE_POLICY_DIGEST,
            signer_handle: RECEIPT_SIGNER_HANDLE.to_owned(),
            signer_public_key: signing_key.verifying_key().to_bytes(),
            signature: [0; 64],
        };
        anchor.signature = signing_key
            .sign(&checkpoint_anchor_signature_message(&anchor))
            .to_bytes();
        anchor
    }
    fn signed_archive_head(
        source_anchor: EvidenceViewerSignedCheckpointAnchorV1,
    ) -> EvidenceViewerSignedCompactionArchiveHeadV1 {
        let receipt_signing_key = receipt_signing_key();
        let archive_signing_key = archive_signing_key();
        let mut head = EvidenceViewerSignedCompactionArchiveHeadV1 {
            version: EVIDENCE_VIEWER_COMPACTION_ARCHIVE_VERSION_V1,
            generation: 1,
            predecessor_head_digest: None,
            predecessor_operation_id: None,
            operation_id: [0; 32],
            source_checkpoint_generation: source_anchor.checkpoint_generation,
            source_checkpoint_revision: [0x53; 32],
            source_checkpoint_anchor: source_anchor,
            compacted_through_unix_ms: 1_900_000_000_000,
            maximum_records: 1,
            challenge_count: 1,
            session_count: 0,
            compacted_payload_digest: [0x54; 32],
            archive_handle: ARCHIVE_HANDLE.to_owned(),
            archive_revision: ARCHIVE_QUALIFICATION.revision(),
            archive_policy_digest: ARCHIVE_QUALIFICATION.policy_digest(),
            archive_id: ARCHIVE_ID,
            archive_public_key: archive_signing_key.verifying_key().to_bytes(),
            signer_handle: RECEIPT_SIGNER_HANDLE.to_owned(),
            signer_public_key: receipt_signing_key.verifying_key().to_bytes(),
            signature: [0; 64],
            head_digest: [0; 32],
            archive_signature: [0; 64],
        };
        head.operation_id = compaction_archive_operation_id(&head).expect("archive operation id");
        head.signature = receipt_signing_key
            .sign(&compaction_archive_signature_message(&head).expect("archive signature message"))
            .to_bytes();
        head.head_digest = compaction_archive_head_digest(&head).expect("archive head digest");
        head.archive_signature = archive_signing_key
            .sign(&compaction_archive_receipt_message(&head))
            .to_bytes();
        head.verify(
            RECEIPT_SIGNER_HANDLE,
            receipt_signing_key.verifying_key().to_bytes(),
        )
        .expect("signed archive head");
        head
    }
    fn bind_archive_head(
        mut anchor: EvidenceViewerSignedCheckpointAnchorV1,
        head: &EvidenceViewerSignedCompactionArchiveHeadV1,
    ) -> EvidenceViewerSignedCheckpointAnchorV1 {
        anchor.compaction_archive_head_digest = Some(head.head_digest);
        anchor.signature = receipt_signing_key()
            .sign(&checkpoint_anchor_signature_message(&anchor))
            .to_bytes();
        anchor
    }
    fn signed_receipt() -> EvidenceViewerSignedReceiptV1 {
        let signing_key = receipt_signing_key();
        let body = EvidenceViewerReceiptBodyV1 {
            version: EVIDENCE_VIEWER_RECEIPT_VERSION_V1,
            sequence: 1,
            kind: EvidenceViewerReceiptKindV1::ManifestAccessed,
            session_id: Some([0x11; 16]),
            case_id: Some("case-prod-1".to_owned()),
            round_id: Some("round-prod-1".to_owned()),
            quarantine_id: [0x12; 16],
            object_id: [0x13; 16],
            evidence_digest: [0x14; 32],
            actor_account_digest: [0x15; 32],
            idempotency_key_digest: [0x16; 32],
            request_digest: [0x17; 32],
            range_start: None,
            range_end: None,
            issued_at_unix_ms: 1_900_000_000_000,
            previous_receipt_digest: [0; 32],
        };
        let receipt_digest = receipt_body_digest(&body).expect("receipt digest");
        EvidenceViewerSignedReceiptV1 {
            body,
            receipt_digest,
            signer_handle: RECEIPT_SIGNER_HANDLE.to_owned(),
            signer_public_key: signing_key.verifying_key().to_bytes(),
            signature: signing_key
                .sign(&receipt_signature_message(receipt_digest))
                .to_bytes(),
        }
    }
    fn projection(
        anchor: EvidenceViewerSignedCheckpointAnchorV1,
        predecessor: Option<super::super::EvidenceViewerReceiptCursorV1>,
        receipts: Vec<EvidenceViewerSignedReceiptV1>,
    ) -> EvidenceViewerTransparencyProjectionV1 {
        projection_with_archive(anchor, None, predecessor, receipts)
    }
    fn projection_with_archive(
        anchor: EvidenceViewerSignedCheckpointAnchorV1,
        compaction_archive_head: Option<EvidenceViewerSignedCompactionArchiveHeadV1>,
        predecessor: Option<super::super::EvidenceViewerReceiptCursorV1>,
        receipts: Vec<EvidenceViewerSignedReceiptV1>,
    ) -> EvidenceViewerTransparencyProjectionV1 {
        let next_cursor = receipts
            .last()
            .map(|receipt| super::super::EvidenceViewerReceiptCursorV1 {
                sequence: receipt.body.sequence,
                receipt_digest: receipt.receipt_digest,
            })
            .or(predecessor);
        let page_limit = 16;
        let projection_digest = transparency_projection_digest(
            &anchor,
            compaction_archive_head.as_ref(),
            predecessor,
            page_limit,
            &receipts,
            next_cursor,
            false,
        )
        .expect("projection digest");
        EvidenceViewerTransparencyProjectionV1 {
            version: EVIDENCE_VIEWER_TRANSPARENCY_PROJECTION_VERSION_V1,
            checkpoint_anchor: anchor,
            compaction_archive_head,
            predecessor,
            page_limit,
            receipts,
            next_cursor,
            has_more: false,
            projection_digest,
        }
    }
    #[test]
    fn optional_construction_rejects_partial_unrequested_and_test_marked_bindings() {
        assert!(
            EvidenceViewerTransparencyProducerV1::try_from_optional(None, None)
                .expect("disabled producer")
                .is_none()
        );
        assert!(matches!(
            EvidenceViewerTransparencyProducerV1::try_from_optional(Some(config()), None),
            Err(EvidenceViewerTransparencyProducerConstructionErrorV1::MissingPublisher)
        ));
        let unrequested: Arc<dyn EvidenceViewerTransparencyPublisherV1> =
            Arc::new(FakePublisher::new(PUBLISHER_HANDLE));
        assert!(matches!(
            EvidenceViewerTransparencyProducerV1::try_from_optional(None, Some(unrequested)),
            Err(EvidenceViewerTransparencyProducerConstructionErrorV1::UnexpectedPublisher)
        ));
        let mut test_marked = config();
        test_marked.publisher_handle = "provider:test-evidence-transparency".to_owned();
        let test_provider: Arc<dyn EvidenceViewerTransparencyPublisherV1> =
            Arc::new(FakePublisher::new("provider:test-evidence-transparency"));
        assert!(matches!(
            EvidenceViewerTransparencyProducerV1::try_new(test_marked, test_provider),
            Err(
                EvidenceViewerTransparencyProducerConstructionErrorV1::PublisherQualification(
                    EvidenceViewerRuntimeProviderQualificationErrorV1::TestMarkedConfiguredHandle
                )
            )
        ));
        let substituted: Arc<dyn EvidenceViewerTransparencyPublisherV1> =
            Arc::new(FakePublisher::new("provider:prod-substituted-transparency"));
        assert!(matches!(
            EvidenceViewerTransparencyProducerV1::try_new(config(), substituted),
            Err(
                EvidenceViewerTransparencyProducerConstructionErrorV1::PublisherQualification(
                    EvidenceViewerRuntimeProviderQualificationErrorV1::SubstitutedProvider
                )
            )
        ));
    }
    #[test]
    fn publishes_exact_cursor_and_reconciles_ambiguous_commit() {
        let publisher = Arc::new(FakePublisher::new(PUBLISHER_HANDLE));
        let producer = EvidenceViewerTransparencyProducerV1::try_new(config(), publisher.clone())
            .expect("qualified producer");
        let first = projection(signed_anchor(1, None, [0x81; 32], None), None, Vec::new());
        let first_head = match producer
            .publish_projection(&first)
            .expect("publish first public head")
        {
            EvidenceViewerTransparencyProducerOutcomeV1::Published(head) => head,
            EvidenceViewerTransparencyProducerOutcomeV1::AlreadyCurrent(_) => {
                panic!("first publication must install a head")
            }
        };
        assert_eq!(first_head.body.generation, 1);
        assert_eq!(first_head.body.receipt_cursor, None);
        assert!(matches!(
            producer
                .publish_projection(&first)
                .expect("idempotent exact retry"),
            EvidenceViewerTransparencyProducerOutcomeV1::AlreadyCurrent(ref head)
                if head == &first_head
        ));
        let receipt = signed_receipt();
        let cursor = super::super::EvidenceViewerReceiptCursorV1 {
            sequence: receipt.body.sequence,
            receipt_digest: receipt.receipt_digest,
        };
        let second = projection(
            signed_anchor(2, Some([0x81; 32]), [0x82; 32], Some(cursor)),
            None,
            vec![receipt],
        );
        publisher.set_mode(PublishMode::CommitThenAmbiguous);
        let second_head = match producer
            .publish_projection(&second)
            .expect("ambiguous commit must reconcile by exact readback")
        {
            EvidenceViewerTransparencyProducerOutcomeV1::Published(head) => head,
            EvidenceViewerTransparencyProducerOutcomeV1::AlreadyCurrent(_) => {
                panic!("new receipt cursor must advance")
            }
        };
        assert_eq!(second_head.body.generation, 2);
        assert_eq!(
            second_head.body.predecessor_head_digest,
            Some(first_head.head_digest)
        );
        assert_eq!(second_head.body.receipt_cursor, Some(cursor));
        assert_eq!(
            producer.reconcile().expect("authoritative reconciliation"),
            Some(second_head.clone())
        );
        assert!(matches!(
            producer
                .publish_projection(&second)
                .expect("exact cursor-advancing retry must be idempotent"),
            EvidenceViewerTransparencyProducerOutcomeV1::AlreadyCurrent(ref head)
                if head == &second_head
        ));
    }
    #[test]
    fn publishes_and_pins_signed_compaction_archive_head() {
        let source_anchor = signed_anchor(1, None, [0xA1; 32], None);
        let archive_head = signed_archive_head(source_anchor);
        let current_anchor = bind_archive_head(
            signed_anchor(2, Some([0xA1; 32]), [0xA2; 32], None),
            &archive_head,
        );
        let projection =
            projection_with_archive(current_anchor, Some(archive_head.clone()), None, Vec::new());
        let publisher = Arc::new(FakePublisher::new(PUBLISHER_HANDLE));
        let producer = EvidenceViewerTransparencyProducerV1::try_new(config(), publisher)
            .expect("qualified producer");
        let published = producer
            .publish_projection(&projection)
            .expect("publish signed archive head");
        assert!(matches!(
            published,
            EvidenceViewerTransparencyProducerOutcomeV1::Published(ref head)
                if head.body.source_compaction_archive_head.as_ref() == Some(&archive_head)
        ));
        let mut substituted_config = config();
        substituted_config.compaction_archive_id = [0xFF; 32];
        let substituted = EvidenceViewerTransparencyProducerV1::try_new(
            substituted_config,
            Arc::new(FakePublisher::new(PUBLISHER_HANDLE)),
        )
        .expect("publicly well-formed substituted archive binding");
        assert_eq!(
            substituted
                .publish_projection(&projection)
                .expect_err("substituted archive identity must fail closed"),
            EvidenceViewerTransparencyProducerErrorV1::ArchiveBindingMismatch
        );
    }
    #[test]
    fn source_forks_and_live_provider_drift_fail_closed() {
        let publisher = Arc::new(FakePublisher::new(PUBLISHER_HANDLE));
        let producer = EvidenceViewerTransparencyProducerV1::try_new(config(), publisher.clone())
            .expect("qualified producer");
        let first = projection(signed_anchor(1, None, [0x91; 32], None), None, Vec::new());
        producer
            .publish_projection(&first)
            .expect("publish first public head");
        let same_generation_substitution =
            projection(signed_anchor(1, None, [0x92; 32], None), None, Vec::new());
        assert_eq!(
            producer
                .publish_projection(&same_generation_substitution)
                .expect_err("same-generation substitution must fail"),
            EvidenceViewerTransparencyProducerErrorV1::SourceLineageConflict
        );
        let wrong_predecessor = projection(
            signed_anchor(2, Some([0xFE; 32]), [0x93; 32], None),
            None,
            Vec::new(),
        );
        assert_eq!(
            producer
                .publish_projection(&wrong_predecessor)
                .expect_err("direct successor with wrong predecessor must fail"),
            EvidenceViewerTransparencyProducerErrorV1::SourceLineageConflict
        );
        publisher.set_stale(true);
        assert_eq!(
            producer
                .reconcile()
                .expect_err("stale runtime qualification must fail every operation"),
            EvidenceViewerTransparencyProducerErrorV1::PublisherUnavailable
        );
        publisher.set_stale(false);
        publisher.set_substituted_key(true);
        assert_eq!(
            producer
                .reconcile()
                .expect_err("live public-key substitution must fail every operation"),
            EvidenceViewerTransparencyProducerErrorV1::PublisherUnavailable
        );
    }
}
