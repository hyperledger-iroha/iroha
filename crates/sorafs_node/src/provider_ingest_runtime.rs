//! Supervised finalized-ledger runtime for provider replication ingest.
//!
//! The runtime deliberately owns no authoritative order, completion, or
//! provider-registration state. Every scan reads one immutable finalized view,
//! drives the durable [`ProviderIngestOutbox`], and reconciles semantic ledger
//! completion before considering transaction-level delivery state.

use std::{
    cell::Cell,
    collections::{BTreeMap, BTreeSet},
    fmt,
    future::Future,
    io::{self, Read},
    pin::Pin,
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use iroha_config::parameters::{
    defaults::sorafs::storage::provider_ingest_runtime::outbox as provider_ingest_outbox_defaults,
    is_production_runtime_handle,
};
use iroha_crypto::{Algorithm, PublicKey};
use iroha_data_model::{
    ChainId,
    account::AccountId,
    musubi::{
        ArchiveId, MUSUBI_REGISTRY_VERSION_V1, MusubiArchiveCommitmentV1,
        MusubiArtifactDescriptorV1, MusubiContentDigestV1,
        MusubiProviderBundleVerificationBindingV1, MusubiProviderBundleVerificationPayloadV1,
        MusubiReplicationOrderArchiveBindingV1, MusubiSemanticReleaseDigestV1,
        MusubiVerificationLockDigestV1,
    },
    sorafs::{
        capacity::ProviderId,
        pin_registry::{
            PinManifestFinalizedRecordV1, PinManifestRecord, PinStatus,
            ProviderIngestCompletionAuthorityV1, ProviderIngestCompletionSignerPolicyV1,
            ReplicationOrderCompletionRecord, ReplicationOrderRecord, ReplicationOrderStatus,
        },
    },
    transaction::{SignedTransaction, TransactionPayload},
};
use norito::{
    codec::Encode as _,
    core::DecodeLimits,
    decode_from_bytes_with_limits,
    derive::{NoritoDeserialize, NoritoSerialize},
};
use sorafs_car::{
    CarBuildPlan, compute_chunk_plan_digest_sha3,
    musubi::{MusubiBundleVerifierV1, VerifiedMusubiBundleV1},
};
use sorafs_manifest::capacity::{
    MAX_CAPACITY_METADATA_VALUE_BYTES, MAX_REPLICATION_ORDER_ASSIGNMENTS, ReplicationOrderV1,
};
use thiserror::Error;
use tokio::sync::watch;

use crate::provider_ingest_outbox::{
    FinalizedProviderIngestAuthorizationV1, FinalizedProviderIngestMusubiContextV1,
    PROVIDER_INGEST_STATUS_PAGE_MAX_V1, ProviderIngestCancellationReasonV1,
    ProviderIngestClaimOwnerV1, ProviderIngestCompletionSigningContextV1,
    ProviderIngestCompletionStateV1, ProviderIngestDeadLetterReasonV1,
    ProviderIngestDeliveryStateV1, ProviderIngestExposedCompletionExpiryV1,
    ProviderIngestFailureClassV1, ProviderIngestFinalizedCancellationV1,
    ProviderIngestFinalizedCompletionV1, ProviderIngestFinalizedCursorV1, ProviderIngestOutbox,
    ProviderIngestOutboxError, ProviderIngestSignerPolicyObservationV1,
    ProviderIngestSourceClaimV1,
};
use crate::store::AdmittedPayloadReadLeaseV1;

const REPLICATION_ORDER_MAX_CANONICAL_BYTES_V1: usize = 256 * 1024;
const PROVIDER_INGEST_SOURCE_QUALIFICATION_VERSION_V1: u8 = 1;
const PROVIDER_INGEST_COMPLETION_SIGNER_QUALIFICATION_VERSION_V1: u8 = 1;
const PROVIDER_INGEST_MUSUBI_COMPLETION_CLAIM_DIGEST_DOMAIN_V1: &[u8] =
    b"iroha.sorafs.provider-ingest.musubi-completion-claim.v1\0";
const MUSUBI_ARTIFACT_DESCRIPTOR_DIGEST_DOMAIN_V1: &[u8] = b"musubi-artifact-descriptor-v1\0";
/// Maximum canonical bytes for one persisted pre-completion Musubi verification receipt.
pub const PROVIDER_INGEST_VERIFIED_MUSUBI_RECEIPT_MAX_CANONICAL_BYTES_V1: usize = 8 * 1024;
const REPLICATION_ORDER_DECODE_LIMITS_V1: DecodeLimits = DecodeLimits::new(
    MAX_CAPACITY_METADATA_VALUE_BYTES,
    REPLICATION_ORDER_MAX_CANONICAL_BYTES_V1,
    131_072,
    REPLICATION_ORDER_MAX_CANONICAL_BYTES_V1 * 4,
    32,
);

/// Boxed asynchronous operation used by provider-ingest integration traits.
pub type ProviderIngestFutureV1<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// Resource and timeout policy for one provider-ingest runtime.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProviderIngestRuntimePolicyV1 {
    /// Maximum finalized assignment rows requested in one page.
    pub max_page_rows: usize,
    /// Maximum finalized pages reconciled in one tick.
    pub max_pages_per_tick: usize,
    /// Maximum source jobs performed in one tick.
    pub max_source_jobs_per_tick: usize,
    /// Maximum governed source provider identities passed to one fetch.
    pub max_source_providers: usize,
    /// Delay between supervised scans.
    pub scan_interval_ms: u64,
    /// Timeout for source verification and fetch, and soft deadline for a
    /// mutating storage operation that must finish under its durable lease.
    pub source_operation_timeout_ms: u64,
    /// Interval used to durably renew a source lease during slow I/O.
    pub source_lease_renew_interval_ms: u64,
    /// Timeout for payload construction, signer resolution, and signing.
    pub signer_timeout_ms: u64,
    /// Timeout for queue preflight, exposure, and transaction observation.
    pub ingress_timeout_ms: u64,
}

impl Default for ProviderIngestRuntimePolicyV1 {
    fn default() -> Self {
        Self {
            max_page_rows: 256,
            max_pages_per_tick: 16,
            max_source_jobs_per_tick: 16,
            max_source_providers: MAX_REPLICATION_ORDER_ASSIGNMENTS,
            scan_interval_ms: 1_000,
            source_operation_timeout_ms: 5 * 60_000,
            source_lease_renew_interval_ms: 15_000,
            signer_timeout_ms: 30_000,
            ingress_timeout_ms: 30_000,
        }
    }
}

impl ProviderIngestRuntimePolicyV1 {
    fn validate(self, outbox: &ProviderIngestOutbox) -> Result<(), ProviderIngestRuntimeErrorV1> {
        if self.max_page_rows == 0
            || self.max_page_rows > PROVIDER_INGEST_STATUS_PAGE_MAX_V1
            || self.max_pages_per_tick == 0
            || self.max_source_jobs_per_tick == 0
            || self.max_source_providers == 0
            || self.max_source_providers > MAX_REPLICATION_ORDER_ASSIGNMENTS
            || self.scan_interval_ms == 0
            || self.source_operation_timeout_ms == 0
            || self.source_lease_renew_interval_ms == 0
            || self.source_lease_renew_interval_ms >= outbox.policy().source_lease_ttl_ms
            || self.signer_timeout_ms == 0
            || self.ingress_timeout_ms == 0
            || self
                .max_page_rows
                .checked_mul(self.max_pages_per_tick)
                .is_none()
        {
            return Err(ProviderIngestRuntimeErrorV1::InvalidPolicy);
        }
        Ok(())
    }
}

/// One assignment row read from a single immutable finalized state view.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderIngestFinalizedAssignmentV1 {
    /// Chain-authoritative pin manifest and its finalized cursor.
    pub pin: PinManifestFinalizedRecordV1,
    /// Chain-authoritative replication order.
    pub order: ReplicationOrderRecord,
    /// Reader-authenticated Musubi archive claim, absent for generic
    /// non-Musubi replication orders.
    pub musubi_archive: Option<ProviderIngestFinalizedMusubiArchiveClaimV1>,
    /// Reader-authenticated Musubi archive and this provider's exact finalized
    /// completion, absent until the local provider completion is committed.
    pub completed_musubi_archive: Option<ProviderIngestFinalizedMusubiCompletionClaimV1>,
    /// Current registered owner of this runtime's provider identity.
    pub provider_owner: Option<AccountId>,
    /// Exact current chain-authoritative completion owner and signer policy.
    pub completion_authority: Option<ProviderIngestCompletionAuthorityV1>,
    /// Current authoritative epoch to use for a new completion transaction.
    pub completion_epoch: Option<u64>,
    /// Exact committed transaction hash, when the finalized reader exposes it.
    pub committed_transaction_hash: Option<[u8; 32]>,
}

/// Opaque consensus-authenticated Musubi archive binding emitted only by a
/// finalized-ledger reader.
///
/// The private representation deliberately prevents publisher request DTOs
/// from being reinterpreted as finalized evidence. A reader can create this
/// value only while servicing a runtime-issued
/// [`ProviderIngestFinalizedClaimFactoryV1`].
///
/// This pre-completion claim binds the configured chain/genesis identity and
/// the exact finalized archive cursor. The source and storage path may use it
/// only to fetch and semantically verify the bundle before replication
/// completion. It contains no post-completion finalized row and therefore
/// cannot authorize a Musubi provider attestation.
///
/// ```compile_fail
/// use sorafs_node::ProviderIngestFinalizedMusubiArchiveClaimV1;
///
/// let _forged = ProviderIngestFinalizedMusubiArchiveClaimV1 { binding: todo!() };
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderIngestFinalizedMusubiArchiveClaimV1 {
    chain_id: ChainId,
    genesis_block_hash: [u8; 32],
    provider_id: [u8; 32],
    observed_finalized_cursor: ProviderIngestFinalizedCursorV1,
    binding: MusubiReplicationOrderArchiveBindingV1,
}

impl ProviderIngestFinalizedMusubiArchiveClaimV1 {
    /// Exact configured chain identity authenticated by the runtime boundary.
    #[must_use]
    pub const fn chain_id(&self) -> &ChainId {
        &self.chain_id
    }

    /// Exact configured genesis block hash authenticated by the runtime boundary.
    #[must_use]
    pub const fn genesis_block_hash(&self) -> [u8; 32] {
        self.genesis_block_hash
    }

    /// Local provider identity for which the finalized assignment was read.
    #[must_use]
    pub const fn provider_id(&self) -> [u8; 32] {
        self.provider_id
    }

    /// Exact finalized archive cursor at which this binding was observed.
    #[must_use]
    pub const fn observed_finalized_cursor(&self) -> ProviderIngestFinalizedCursorV1 {
        self.observed_finalized_cursor
    }

    /// Exact replication order authenticated by the finalized reader.
    #[must_use]
    pub const fn replication_order(&self) -> [u8; 32] {
        *self.binding.replication_order.as_bytes()
    }

    /// Exact derived archive identity authenticated by the finalized reader.
    #[must_use]
    pub const fn archive_id(&self) -> ArchiveId {
        self.binding.archive_id
    }

    /// Complete immutable archive commitment authenticated by the finalized
    /// reader.
    #[must_use]
    pub const fn commitment(&self) -> &MusubiArchiveCommitmentV1 {
        &self.binding.commitment
    }

    /// Check every field shared with one finalized local-ingest authorization.
    #[must_use]
    pub fn matches_authorization(
        &self,
        authorization: &FinalizedProviderIngestAuthorizationV1,
    ) -> bool {
        let commitment = self.commitment();
        authorization.validate().is_ok()
            && !self.chain_id.as_str().is_empty()
            && self.chain_id.as_str().len()
                <= provider_ingest_outbox_defaults::COMPLETION_CHAIN_ID_MAX_BYTES_V1
            && self.genesis_block_hash != [0; 32]
            && self.provider_id == authorization.provider_id()
            && authorization_musubi_context_matches(
                authorization,
                &self.chain_id,
                self.genesis_block_hash,
                self.archive_id(),
            )
            && finalized_cursor_is_same_or_later(
                self.observed_finalized_cursor,
                authorization.admission_finalized_cursor(),
            )
            && self.replication_order() == authorization.order_id()
            && self.archive_id() == commitment.archive_id()
            && commitment.validate().is_ok()
            && commitment.root_cid.as_bytes() == authorization.manifest_cid()
            && commitment.chunker.to_handle() == authorization.chunker_handle()
            && commitment.chunk_plan_digest.as_bytes() == &authorization.chunk_digest_sha3_256()
            && commitment.por_root.as_bytes() == &authorization.por_root()
            && commitment.content_length == authorization.content_length()
    }
}

/// Opaque consensus-authenticated Musubi claim sealed only from this provider's
/// finalized completion row.
///
/// This value is intentionally distinct from
/// [`ProviderIngestFinalizedMusubiArchiveClaimV1`]. The pre-completion claim can
/// authorize fetching and semantic verification before storage completion, but
/// only this completed-row capability may enter the later provider-attestation
/// path. It has no public constructor or serialization implementation.
///
/// ```compile_fail
/// use sorafs_node::provider_ingest_runtime::ProviderIngestFinalizedMusubiCompletionClaimV1;
///
/// let _forged = ProviderIngestFinalizedMusubiCompletionClaimV1 {
///     chain_id: todo!(),
///     genesis_block_hash: [0; 32],
///     provider_id: [0; 32],
///     observed_finalized_cursor: todo!(),
///     binding: todo!(),
///     completion: todo!(),
/// };
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderIngestFinalizedMusubiCompletionClaimV1 {
    chain_id: ChainId,
    genesis_block_hash: [u8; 32],
    provider_id: [u8; 32],
    observed_finalized_cursor: ProviderIngestFinalizedCursorV1,
    binding: MusubiReplicationOrderArchiveBindingV1,
    completion: ReplicationOrderCompletionRecord,
}

impl ProviderIngestFinalizedMusubiCompletionClaimV1 {
    /// Exact configured chain identity authenticated by the finalized reader.
    #[must_use]
    pub const fn chain_id(&self) -> &ChainId {
        &self.chain_id
    }

    /// Exact configured genesis hash authenticated by the finalized reader.
    #[must_use]
    pub const fn genesis_block_hash(&self) -> [u8; 32] {
        self.genesis_block_hash
    }

    /// Local provider whose finalized completion was observed.
    #[must_use]
    pub const fn provider_id(&self) -> [u8; 32] {
        self.provider_id
    }

    /// Exact finalized archive cursor at which the completion row was observed.
    #[must_use]
    pub const fn observed_finalized_cursor(&self) -> ProviderIngestFinalizedCursorV1 {
        self.observed_finalized_cursor
    }

    /// Exact replication order authenticated by the finalized reader.
    #[must_use]
    pub const fn replication_order(&self) -> [u8; 32] {
        *self.binding.replication_order.as_bytes()
    }

    /// Exact derived archive identity authenticated by the finalized reader.
    #[must_use]
    pub const fn archive_id(&self) -> ArchiveId {
        self.binding.archive_id
    }

    /// Complete immutable archive commitment authenticated by the finalized reader.
    #[must_use]
    pub const fn commitment(&self) -> &MusubiArchiveCommitmentV1 {
        &self.binding.commitment
    }

    /// Exact provider-scoped completion copied from the finalized order row.
    #[must_use]
    pub const fn completion(&self) -> &ReplicationOrderCompletionRecord {
        &self.completion
    }

    /// Check every finalized identity, cursor, completion, and commitment field against the
    /// retained local-ingest authorization.
    ///
    /// The completed row may first be observed after both its original admission cursor and its
    /// own finalized anchor. When either earlier point has the same height as the observed cursor,
    /// its block hash must also match. Conflicting admission and completion hashes at one height
    /// are always rejected, including when the completed row is observed at a later height.
    #[must_use]
    pub fn matches_authorization(
        &self,
        authorization: &FinalizedProviderIngestAuthorizationV1,
    ) -> bool {
        let Some(context) = authorization.musubi_context() else {
            return false;
        };
        let admission_cursor = authorization.admission_finalized_cursor();
        let completion_anchor = self.completion.finalized_anchor;
        authorization.validate().is_ok()
            && self.binding.validate().is_ok()
            && context.chain_id() == &self.chain_id
            && context.genesis_block_hash() == self.genesis_block_hash
            && context.archive_id() == self.archive_id()
            && self.provider_id == authorization.provider_id()
            && self.replication_order() == authorization.order_id()
            && self.archive_id() == self.commitment().archive_id()
            && self.observed_finalized_cursor.height != 0
            && self.observed_finalized_cursor.block_hash != [0; 32]
            && self.completion.provider_id == ProviderId::new(self.provider_id)
            && self.completion.completed_by == self.completion.completion_authority.provider_owner
            && self.completion.completion_authority.is_valid()
            && self.completion.assignment_revision != 0
            && self.completion.completion_epoch != 0
            && completion_anchor.is_valid()
            && (admission_cursor.height != completion_anchor.height
                || admission_cursor.block_hash == completion_anchor.block_hash)
            && finalized_cursor_is_same_or_later(self.observed_finalized_cursor, admission_cursor)
            && finalized_cursor_is_same_or_later(
                self.observed_finalized_cursor,
                ProviderIngestFinalizedCursorV1 {
                    height: completion_anchor.height,
                    block_hash: completion_anchor.block_hash,
                },
            )
            && self.commitment().root_cid.as_bytes() == authorization.manifest_cid()
            && self.commitment().chunker.to_handle() == authorization.chunker_handle()
            && self.commitment().chunk_plan_digest.as_bytes()
                == &authorization.chunk_digest_sha3_256()
            && self.commitment().por_root.as_bytes() == &authorization.por_root()
            && self.commitment().content_length == authorization.content_length()
    }
}

/// Closed failure returned while deriving a provider-attestation approval request.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum ProviderIngestMusubiAttestationApprovalRequestErrorV1 {
    /// The completed claim or canonical verifier evidence was inconsistent.
    #[error("provider-ingest Musubi attestation evidence was rejected")]
    Rejected,
}

/// Opaque, unsigned request to approve one exact Musubi provider attestation payload.
///
/// This value is deliberately nonserializable and has no arbitrary constructor. It can be
/// derived only by combining a finalized post-completion claim with the canonical bundle
/// verifier's opaque output. Construction performs no signing and activates no runtime path.
///
/// Neither pre-completion receipt nor verifier evidence can bypass the storage lifecycle lease.
/// The raw evidence constructor is crate-private, so downstream code must use
/// [`AdmittedPayloadReadLeaseV1::verify_completed_musubi_bundle`]:
///
/// ```compile_fail
/// use sorafs_node::{
///     ProviderIngestFinalizedMusubiCompletionClaimV1,
///     ProviderIngestMusubiAttestationApprovalRequestV1,
/// };
/// use sorafs_car::musubi::VerifiedMusubiBundleV1;
///
/// fn cannot_reuse_precompletion_verifier_evidence(
///     claim: &ProviderIngestFinalizedMusubiCompletionClaimV1,
///     verified: &VerifiedMusubiBundleV1,
/// ) {
///     let _ = ProviderIngestMusubiAttestationApprovalRequestV1::from_verified_completion(
///         claim, verified,
///     );
/// }
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderIngestMusubiAttestationApprovalRequestV1 {
    payload: MusubiProviderBundleVerificationPayloadV1,
    completion_claim_digest: [u8; 32],
    observed_finalized_cursor: ProviderIngestFinalizedCursorV1,
    signer_policy: ProviderIngestCompletionSignerPolicyV1,
}

impl ProviderIngestMusubiAttestationApprovalRequestV1 {
    /// Derive an unsigned approval request from exact finalized completion and verifier evidence.
    ///
    /// # Errors
    ///
    /// Rejects noncanonical or substituted claim fields, completion authority, archive
    /// commitment, CAR statistics, descriptor, semantic release, or verification lock evidence.
    pub(crate) fn from_verified_completion(
        claim: &ProviderIngestFinalizedMusubiCompletionClaimV1,
        verified: &VerifiedMusubiBundleV1,
    ) -> Result<Self, ProviderIngestMusubiAttestationApprovalRequestErrorV1> {
        let rejected = || ProviderIngestMusubiAttestationApprovalRequestErrorV1::Rejected;
        let commitment = claim.commitment();
        let completion = claim.completion();
        let descriptor = verified.descriptor();
        let car_stats = verified.car_stats();
        let semantic_release_manifest_digest = verified.semantic_release().semantic_digest();
        let verification_lock_digest = verified.verification_lock().digest();
        let descriptor_digest =
            musubi_artifact_descriptor_digest_v1(descriptor).ok_or_else(rejected)?;

        claim.binding.validate().map_err(|_| rejected())?;
        descriptor.validate().map_err(|_| rejected())?;
        if claim.chain_id.as_str().is_empty()
            || claim.chain_id.as_str().len()
                > provider_ingest_outbox_defaults::COMPLETION_CHAIN_ID_MAX_BYTES_V1
            || claim.genesis_block_hash == [0; 32]
            || claim.provider_id == [0; 32]
            || claim.observed_finalized_cursor.height == 0
            || claim.observed_finalized_cursor.block_hash == [0; 32]
            || verified.archive_id() != claim.archive_id()
            || claim.archive_id() != commitment.archive_id()
            || completion.provider_id != ProviderId::new(claim.provider_id)
            || completion.completed_by != completion.completion_authority.provider_owner
            || !completion.completion_authority.is_valid()
            || completion.assignment_revision == 0
            || completion.completion_epoch == 0
            || !completion.finalized_anchor.is_valid()
            || completion.finalized_anchor.height > claim.observed_finalized_cursor.height
            || completion.finalized_anchor.height == claim.observed_finalized_cursor.height
                && completion.finalized_anchor.block_hash
                    != claim.observed_finalized_cursor.block_hash
            || descriptor.semantic_release_manifest_digest != semantic_release_manifest_digest
            || descriptor.verification_lock_digest != verification_lock_digest
            || descriptor.source_tree_digest != commitment.source_tree_digest
            || descriptor_digest != commitment.descriptor_digest
            || descriptor.source_file_count != verified.source_file_count()
            || descriptor.source_bytes != verified.source_bytes()
            || commitment.file_count != verified.source_file_count()
            || car_stats.payload_bytes != commitment.content_length
            || car_stats.car_size != commitment.car_size
            || car_stats.car_archive_digest.as_bytes() != commitment.car_digest.as_bytes()
            || car_stats.chunk_count
                != usize::try_from(commitment.chunk_count).unwrap_or(usize::MAX)
            || car_stats.root_cids.len() != 1
            || car_stats.root_cids[0].as_slice() != commitment.root_cid.as_bytes()
        {
            return Err(rejected());
        }

        let payload = MusubiProviderBundleVerificationPayloadV1 {
            version: MUSUBI_REGISTRY_VERSION_V1,
            binding: MusubiProviderBundleVerificationBindingV1 {
                chain_id: claim.chain_id.clone(),
                genesis_block_hash: claim.genesis_block_hash,
                provider_id: ProviderId::new(claim.provider_id),
                completed_by: completion.completed_by.clone(),
                completion_authority: completion.completion_authority.clone(),
                replication_order: claim.binding.replication_order,
                assignment_revision: completion.assignment_revision,
                completion_epoch: completion.completion_epoch,
                finalized_anchor: completion.finalized_anchor,
                archive_id: claim.archive_id(),
                bundle_digest: commitment.bundle_digest,
                descriptor_digest,
                semantic_release_manifest_digest,
                verification_lock_digest,
                source_tree_digest: descriptor.source_tree_digest,
            },
        };
        payload.validate().map_err(|_| rejected())?;
        let completion_claim_digest =
            provider_ingest_musubi_completion_claim_digest_v1(claim).ok_or_else(rejected)?;
        Ok(Self {
            payload,
            completion_claim_digest,
            observed_finalized_cursor: claim.observed_finalized_cursor,
            signer_policy: completion.completion_authority.signer_policy,
        })
    }

    /// Construct a structurally valid opaque request for crate-local unit tests.
    ///
    /// Production code cannot call this helper, and the helper deliberately
    /// preserves the same completed-owner, signer-policy, cursor, and non-zero
    /// claim-digest invariants exposed through the request accessors.
    #[cfg(test)]
    pub(crate) fn test_fixture(
        payload: MusubiProviderBundleVerificationPayloadV1,
        completion_claim_digest: [u8; 32],
        observed_finalized_cursor: ProviderIngestFinalizedCursorV1,
        signer_policy: ProviderIngestCompletionSignerPolicyV1,
    ) -> Result<Self, ProviderIngestMusubiAttestationApprovalRequestErrorV1> {
        let anchor = payload.binding.finalized_anchor;
        if payload.validate().is_err()
            || payload.binding.chain_id.as_str().is_empty()
            || payload.binding.chain_id.as_str().len()
                > provider_ingest_outbox_defaults::COMPLETION_CHAIN_ID_MAX_BYTES_V1
            || completion_claim_digest == [0; 32]
            || observed_finalized_cursor.height == 0
            || observed_finalized_cursor.block_hash == [0; 32]
            || !signer_policy.is_valid()
            || payload.binding.completion_authority.signer_policy != signer_policy
            || payload.binding.completed_by != payload.binding.completion_authority.provider_owner
            || anchor.height > observed_finalized_cursor.height
            || anchor.height == observed_finalized_cursor.height
                && anchor.block_hash != observed_finalized_cursor.block_hash
        {
            return Err(ProviderIngestMusubiAttestationApprovalRequestErrorV1::Rejected);
        }
        Ok(Self {
            payload,
            completion_claim_digest,
            observed_finalized_cursor,
            signer_policy,
        })
    }

    /// Return the exact unsigned provider-attestation payload.
    #[must_use]
    pub const fn payload(&self) -> &MusubiProviderBundleVerificationPayloadV1 {
        &self.payload
    }

    /// Return the stable domain-separated digest of the completed-row evidence.
    ///
    /// The observation cursor is retained separately and deliberately excluded
    /// from this digest so an identical completed row can be reverified at a
    /// later finalized head after restart.
    #[must_use]
    pub const fn completion_claim_digest(&self) -> [u8; 32] {
        self.completion_claim_digest
    }

    /// Return the finalized cursor at which the completed row was observed.
    #[must_use]
    pub const fn observed_finalized_cursor(&self) -> ProviderIngestFinalizedCursorV1 {
        self.observed_finalized_cursor
    }

    /// Return the exact governed signer policy accepted by the finalized completion.
    #[must_use]
    pub const fn signer_policy(&self) -> ProviderIngestCompletionSignerPolicyV1 {
        self.signer_policy
    }
}

impl AdmittedPayloadReadLeaseV1<'_> {
    /// Reverify one completed Musubi bundle and mint its opaque unsigned approval request.
    ///
    /// This is the only public request-minting boundary. It binds the retained finalized
    /// authorization and sealed completed-row claim to this exact storage-admitted manifest,
    /// checks the supplied reconstruction plan against the admitted payload, and opens all three
    /// byte-zero readers itself while the storage lifecycle lease remains held. The canonical
    /// Musubi V1 verifier applies its fixed consensus bounds; no caller-controlled limit or
    /// previously retained verifier result is accepted.
    ///
    /// # Errors
    ///
    /// Returns [`ProviderIngestLocalStorageErrorV1::Permanent`] for an identity, cursor, plan,
    /// commitment, payload, or semantic-integrity mismatch. Transient admitted-storage reader
    /// failures return [`ProviderIngestLocalStorageErrorV1::Retryable`].
    pub fn verify_completed_musubi_bundle(
        &self,
        plan: &CarBuildPlan,
        authorization: &FinalizedProviderIngestAuthorizationV1,
        completed_claim: &ProviderIngestFinalizedMusubiCompletionClaimV1,
    ) -> Result<ProviderIngestMusubiAttestationApprovalRequestV1, ProviderIngestLocalStorageErrorV1>
    {
        if !completed_claim.matches_authorization(authorization)
            || self.manifest_digest() != &authorization.manifest_digest()
            || self.content_length() != authorization.content_length()
            || self.payload_digest() != plan.payload_digest.as_bytes()
            || plan.content_length != authorization.content_length()
            || compute_chunk_plan_digest_sha3(&plan.chunks) != authorization.chunk_digest_sha3_256()
        {
            return Err(ProviderIngestLocalStorageErrorV1::Permanent);
        }

        let first_read_error = Cell::new(None);
        let verified = MusubiBundleVerifierV1::verify_payload_with_factory(
            plan,
            completed_claim.commitment(),
            || {
                self.open_reader()
                    .inspect_err(|error| {
                        if first_read_error.get().is_none() {
                            first_read_error.set(Some(error.kind()));
                        }
                    })
                    .map(|inner| ProviderIngestObservedAdmittedPayloadReaderV1 {
                        inner,
                        first_error_kind: &first_read_error,
                    })
            },
        );
        match verified {
            Ok(verified) => {
                ProviderIngestMusubiAttestationApprovalRequestV1::from_verified_completion(
                    completed_claim,
                    &verified,
                )
                .map_err(|_| ProviderIngestLocalStorageErrorV1::Permanent)
            }
            Err(_)
                if first_read_error.get().is_some_and(|kind| {
                    provider_ingest_admitted_payload_read_error_is_retryable(kind)
                }) =>
            {
                Err(ProviderIngestLocalStorageErrorV1::Retryable)
            }
            Err(_) => Err(ProviderIngestLocalStorageErrorV1::Permanent),
        }
    }
}

struct ProviderIngestObservedAdmittedPayloadReaderV1<'observation, R> {
    inner: R,
    first_error_kind: &'observation Cell<Option<io::ErrorKind>>,
}

impl<R: Read> Read for ProviderIngestObservedAdmittedPayloadReaderV1<'_, R> {
    fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
        self.inner.read(output).inspect_err(|error| {
            if self.first_error_kind.get().is_none() {
                self.first_error_kind.set(Some(error.kind()));
            }
        })
    }
}

const fn provider_ingest_admitted_payload_read_error_is_retryable(kind: io::ErrorKind) -> bool {
    matches!(
        kind,
        io::ErrorKind::Interrupted
            | io::ErrorKind::WouldBlock
            | io::ErrorKind::TimedOut
            | io::ErrorKind::NotFound
            | io::ErrorKind::Other
    )
}

#[derive(NoritoSerialize)]
struct ProviderIngestMusubiCompletionClaimDigestPreimageV1 {
    chain_id: ChainId,
    genesis_block_hash: [u8; 32],
    provider_id: [u8; 32],
    binding: MusubiReplicationOrderArchiveBindingV1,
    completion: ReplicationOrderCompletionRecord,
}

fn provider_ingest_musubi_completion_claim_digest_v1(
    claim: &ProviderIngestFinalizedMusubiCompletionClaimV1,
) -> Option<[u8; 32]> {
    let preimage = ProviderIngestMusubiCompletionClaimDigestPreimageV1 {
        chain_id: claim.chain_id.clone(),
        genesis_block_hash: claim.genesis_block_hash,
        provider_id: claim.provider_id,
        binding: claim.binding.clone(),
        completion: claim.completion.clone(),
    };
    let canonical = norito::to_bytes(&preimage).ok()?;
    let canonical_len = u64::try_from(canonical.len()).ok()?;
    let mut hasher = blake3::Hasher::new();
    hasher.update(PROVIDER_INGEST_MUSUBI_COMPLETION_CLAIM_DIGEST_DOMAIN_V1);
    hasher.update(&canonical_len.to_be_bytes());
    hasher.update(&canonical);
    Some(*hasher.finalize().as_bytes())
}

fn musubi_artifact_descriptor_digest_v1(
    descriptor: &MusubiArtifactDescriptorV1,
) -> Option<MusubiContentDigestV1> {
    let descriptor_bytes = descriptor.encode();
    let domain_len = u64::try_from(MUSUBI_ARTIFACT_DESCRIPTOR_DIGEST_DOMAIN_V1.len()).ok()?;
    let descriptor_len = u64::try_from(descriptor_bytes.len()).ok()?;
    let material_len = 8_u64
        .checked_add(domain_len)?
        .checked_add(8)?
        .checked_add(descriptor_len)?;
    let mut hasher = blake3::Hasher::new();
    hasher.update(MUSUBI_ARTIFACT_DESCRIPTOR_DIGEST_DOMAIN_V1);
    hasher.update(&material_len.to_be_bytes());
    hasher.update(&domain_len.to_be_bytes());
    hasher.update(MUSUBI_ARTIFACT_DESCRIPTOR_DIGEST_DOMAIN_V1);
    hasher.update(&descriptor_len.to_be_bytes());
    hasher.update(&descriptor_bytes);
    Some(MusubiContentDigestV1::new(*hasher.finalize().as_bytes()))
}

/// Runtime-owned capability for constructing opaque claims inside one
/// finalized-ledger read.
///
/// This type has no public constructor. The provider runtime creates it for a
/// ledger call, and only that ledger implementation can turn a projected
/// consensus binding into an opaque claim.
///
/// The capability authenticates the configured finalized-ledger
/// implementation boundary, not arbitrary bytes. That trusted implementation
/// receives ownership and can retain the capability; production wiring must
/// therefore install only the qualified archive-backed reader.
#[derive(Debug)]
pub struct ProviderIngestFinalizedClaimFactoryV1 {
    chain_id: ChainId,
    genesis_block_hash: [u8; 32],
    provider_id: [u8; 32],
}

impl ProviderIngestFinalizedClaimFactoryV1 {
    fn new(chain_id: ChainId, genesis_block_hash: [u8; 32], provider_id: [u8; 32]) -> Self {
        Self {
            chain_id,
            genesis_block_hash,
            provider_id,
        }
    }

    /// Validate and seal one exact projected Musubi archive binding.
    ///
    /// # Errors
    ///
    /// Rejects a noncanonical binding or one substituted from another
    /// replication order or pin commitment.
    pub fn seal_musubi_archive(
        &self,
        observed_chain_id: &ChainId,
        observed_finalized_cursor: ProviderIngestFinalizedCursorV1,
        expected_order: [u8; 32],
        expected_pin: &PinManifestRecord,
        binding: MusubiReplicationOrderArchiveBindingV1,
    ) -> Result<ProviderIngestFinalizedMusubiArchiveClaimV1, ProviderIngestFinalizedLedgerErrorV1>
    {
        binding
            .validate()
            .map_err(|_| ProviderIngestFinalizedLedgerErrorV1::Rejected)?;
        let commitment = &binding.commitment;
        if observed_chain_id != &self.chain_id
            || observed_finalized_cursor.height == 0
            || observed_finalized_cursor.block_hash == [0; 32]
            || binding.replication_order.as_bytes() != &expected_order
            || commitment.root_cid != expected_pin.root_cid
            || commitment.chunker != expected_pin.chunker
            || commitment.chunk_plan_digest.as_bytes() != &expected_pin.chunk_digest_sha3_256
            || commitment.por_root.as_bytes() != &expected_pin.por_root
            || commitment.content_length != expected_pin.content_length
        {
            return Err(ProviderIngestFinalizedLedgerErrorV1::Rejected);
        }
        Ok(ProviderIngestFinalizedMusubiArchiveClaimV1 {
            chain_id: self.chain_id.clone(),
            genesis_block_hash: self.genesis_block_hash,
            provider_id: self.provider_id,
            observed_finalized_cursor,
            binding,
        })
    }

    /// Seal this provider's exact completed Musubi row as a post-completion capability.
    ///
    /// The method accepts the complete finalized order record and locates the
    /// configured provider's completion itself. A caller therefore cannot
    /// substitute a detached completion record that was not part of the row.
    ///
    /// # Errors
    ///
    /// Rejects a pending local provider, a noncanonical completion, or any
    /// substituted chain, cursor, order, pin, archive, or commitment field.
    pub fn seal_completed_musubi_archive(
        &self,
        observed_chain_id: &ChainId,
        observed_finalized_cursor: ProviderIngestFinalizedCursorV1,
        expected_provider_id: ProviderId,
        expected_order: &ReplicationOrderRecord,
        expected_pin: &PinManifestRecord,
        binding: MusubiReplicationOrderArchiveBindingV1,
    ) -> Result<ProviderIngestFinalizedMusubiCompletionClaimV1, ProviderIngestFinalizedLedgerErrorV1>
    {
        binding
            .validate()
            .map_err(|_| ProviderIngestFinalizedLedgerErrorV1::Rejected)?;
        let commitment = &binding.commitment;
        let provider_id = ProviderId::new(self.provider_id);
        let canonical_order = decode_from_bytes_with_limits::<ReplicationOrderV1>(
            &expected_order.canonical_order,
            REPLICATION_ORDER_DECODE_LIMITS_V1,
        )
        .map_err(|_| ProviderIngestFinalizedLedgerErrorV1::Rejected)?;
        canonical_order
            .validate()
            .map_err(|_| ProviderIngestFinalizedLedgerErrorV1::Rejected)?;
        let canonical_bytes = norito::to_bytes(&canonical_order)
            .map_err(|_| ProviderIngestFinalizedLedgerErrorV1::Rejected)?;
        let mut provider_completions = expected_order
            .provider_completions
            .iter()
            .filter(|completion| completion.provider_id == provider_id);
        let completion = provider_completions
            .next()
            .ok_or(ProviderIngestFinalizedLedgerErrorV1::Rejected)?;
        let completion_anchor = completion.finalized_anchor;
        if observed_chain_id != &self.chain_id
            || observed_finalized_cursor.height == 0
            || observed_finalized_cursor.block_hash == [0; 32]
            || provider_id != expected_provider_id
            || canonical_bytes != expected_order.canonical_order
            || canonical_order.order_id != *expected_order.order_id.as_bytes()
            || canonical_order.manifest_digest != *expected_order.manifest_digest.as_bytes()
            || canonical_order.manifest_cid.as_slice()
                != expected_order.manifest_root_cid.as_bytes()
            || !canonical_order
                .assignments
                .iter()
                .any(|assignment| assignment.provider_id == self.provider_id)
            || provider_completions.next().is_some()
            || binding.replication_order != expected_order.order_id
            || expected_order.musubi_archive != Some(binding.archive_id)
            || expected_order.manifest_digest != expected_pin.digest
            || expected_order.manifest_root_cid != expected_pin.root_cid
            || commitment.root_cid != expected_pin.root_cid
            || commitment.chunker != expected_pin.chunker
            || commitment.chunk_plan_digest.as_bytes() != &expected_pin.chunk_digest_sha3_256
            || commitment.por_root.as_bytes() != &expected_pin.por_root
            || commitment.content_length != expected_pin.content_length
            || completion.provider_id != provider_id
            || completion.completed_by != completion.completion_authority.provider_owner
            || !completion.completion_authority.is_valid()
            || completion.assignment_revision == 0
            || completion.assignment_revision != expected_order.assignment_revision
            || completion.completion_epoch < expected_order.issued_epoch
            || completion.completion_epoch > expected_order.deadline_epoch
            || !completion_anchor.is_valid()
            || completion_anchor.height > observed_finalized_cursor.height
            || completion_anchor.height == observed_finalized_cursor.height
                && completion_anchor.block_hash != observed_finalized_cursor.block_hash
        {
            return Err(ProviderIngestFinalizedLedgerErrorV1::Rejected);
        }
        Ok(ProviderIngestFinalizedMusubiCompletionClaimV1 {
            chain_id: self.chain_id.clone(),
            genesis_block_hash: self.genesis_block_hash,
            provider_id: self.provider_id,
            observed_finalized_cursor,
            binding,
            completion: completion.clone(),
        })
    }
}

/// Bounded stable page of provider assignments from one finalized state view.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderIngestFinalizedAssignmentPageV1 {
    /// Immutable finalized cursor shared by every row.
    pub finalized_cursor: ProviderIngestFinalizedCursorV1,
    /// Finalized block creation time used for transaction-TTL proofs.
    pub finalized_block_time_ms: u64,
    /// Rows in strictly increasing replication-order identity.
    pub rows: Vec<ProviderIngestFinalizedAssignmentV1>,
    /// Last returned order identity when another page exists.
    pub next_after_order_id: Option<[u8; 32]>,
}

/// Finalized-ledger paging failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderIngestFinalizedLedgerErrorV1 {
    /// The finalized query service is temporarily unavailable.
    Unavailable,
    /// The finalized query service rejected the bounded request.
    Rejected,
}

/// Reader for chain-authoritative assignments and provider completions.
pub trait ProviderIngestFinalizedLedgerV1: Send + Sync + 'static {
    /// Read one stable page after `after_order_id`.
    ///
    /// `at_finalized_cursor` is `None` only for the first page of a scan. Every
    /// continuation supplies the exact immutable cursor returned by that first
    /// page, including continuations resumed in a later runtime tick.
    fn read_assignment_page<'a>(
        &'a self,
        claim_factory: ProviderIngestFinalizedClaimFactoryV1,
        at_finalized_cursor: Option<ProviderIngestFinalizedCursorV1>,
        after_order_id: Option<[u8; 32]>,
        limit: usize,
    ) -> ProviderIngestFutureV1<
        'a,
        Result<ProviderIngestFinalizedAssignmentPageV1, ProviderIngestFinalizedLedgerErrorV1>,
    >;
}

/// One unsealed projected assignment supplied to the capture scanner.
///
/// This value is deliberately opaque and has no wire codec. Constructing it
/// does not confer finalized authority: the scanner validates the complete
/// page, then uses its private claim factory to seal the raw archive binding.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderIngestCompletedMusubiCaptureSourceRowV1 {
    pin: PinManifestFinalizedRecordV1,
    order: ReplicationOrderRecord,
    musubi_archive: Option<MusubiReplicationOrderArchiveBindingV1>,
    provider_owner: Option<AccountId>,
    completion_authority: Option<ProviderIngestCompletionAuthorityV1>,
    completion_epoch: Option<u64>,
    committed_transaction_hash: Option<[u8; 32]>,
}

impl ProviderIngestCompletedMusubiCaptureSourceRowV1 {
    /// Package one untrusted archive projection for scanner-side validation.
    ///
    /// No field is validated here. This constructor intentionally returns no
    /// opaque claim and cannot be used as finalized evidence on its own.
    #[must_use]
    pub fn from_projected_fields(
        pin: PinManifestFinalizedRecordV1,
        order: ReplicationOrderRecord,
        musubi_archive: Option<MusubiReplicationOrderArchiveBindingV1>,
        provider_owner: Option<AccountId>,
        completion_authority: Option<ProviderIngestCompletionAuthorityV1>,
        completion_epoch: Option<u64>,
        committed_transaction_hash: Option<[u8; 32]>,
    ) -> Self {
        Self {
            pin,
            order,
            musubi_archive,
            provider_owner,
            completion_authority,
            completion_epoch,
            committed_transaction_hash,
        }
    }
}

/// One unsealed, replayable page supplied to the capture scanner.
///
/// The private fields and lack of a codec keep this projection distinct from
/// opaque finalized evidence. The scanner commits no cursor progress until it
/// has checked the page identity, bounds, ordering, every row, and every claim
/// that it seals from the raw bindings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderIngestCompletedMusubiCaptureSourcePageV1 {
    chain_id: ChainId,
    provider_id: [u8; 32],
    finalized_cursor: ProviderIngestFinalizedCursorV1,
    finalized_block_time_ms: u64,
    rows: Vec<ProviderIngestCompletedMusubiCaptureSourceRowV1>,
    next_after_order_id: Option<[u8; 32]>,
}

impl ProviderIngestCompletedMusubiCaptureSourcePageV1 {
    /// Package one untrusted archive page for scanner-side validation.
    ///
    /// No field is validated here. In particular, this constructor cannot
    /// mint or carry a completed-row claim.
    #[must_use]
    pub fn from_projected_fields(
        chain_id: ChainId,
        provider_id: [u8; 32],
        finalized_cursor: ProviderIngestFinalizedCursorV1,
        finalized_block_time_ms: u64,
        rows: Vec<ProviderIngestCompletedMusubiCaptureSourceRowV1>,
        next_after_order_id: Option<[u8; 32]>,
    ) -> Self {
        Self {
            chain_id,
            provider_id,
            finalized_cursor,
            finalized_block_time_ms,
            rows,
            next_after_order_id,
        }
    }

    /// Return the projected finalized cursor without conferring authority.
    #[must_use]
    pub const fn finalized_cursor(&self) -> ProviderIngestFinalizedCursorV1 {
        self.finalized_cursor
    }

    /// Return the exclusive continuation boundary, when another page exists.
    #[must_use]
    pub const fn next_after_order_id(&self) -> Option<[u8; 32]> {
        self.next_after_order_id
    }
}

/// Replayable raw finalized-ledger boundary used only by the capture scanner.
///
/// Unlike the ordinary provider-ingest reader, this trait never receives the
/// private [`ProviderIngestFinalizedClaimFactoryV1`] and cannot return opaque
/// claims. It supplies only unsealed projections. The scanner creates and
/// retains the factory, validates the complete page, and seals claims itself.
///
/// This boundary must also be stateless with respect to page continuation.
/// Repeating an exact request must reconstruct the same immutable raw page
/// without relying on an earlier read having advanced adapter-local state.
/// That keeps both post-read validation failures and task cancellation
/// retry-safe.
pub trait ProviderIngestCompletedMusubiCaptureLedgerV1: Send + Sync + 'static {
    /// Reconstruct one exact bounded unsealed page without consuming it.
    fn read_completed_musubi_capture_page<'a>(
        &'a self,
        at_finalized_cursor: Option<ProviderIngestFinalizedCursorV1>,
        after_order_id: Option<[u8; 32]>,
        limit: usize,
    ) -> ProviderIngestFutureV1<
        'a,
        Result<
            ProviderIngestCompletedMusubiCaptureSourcePageV1,
            ProviderIngestFinalizedLedgerErrorV1,
        >,
    >;
}

/// One validated completed-Musubi candidate emitted by the capture scanner.
///
/// The value has no public constructor or wire codec. Its authorization and
/// opaque completed-row claim were derived together from one validated row of
/// a single finalized page.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderIngestCompletedMusubiCaptureCandidateV1 {
    authorization: FinalizedProviderIngestAuthorizationV1,
    completed_claim: ProviderIngestFinalizedMusubiCompletionClaimV1,
}

impl ProviderIngestCompletedMusubiCaptureCandidateV1 {
    /// Borrow the exact finalized provider-ingest authorization.
    #[must_use]
    pub const fn authorization(&self) -> &FinalizedProviderIngestAuthorizationV1 {
        &self.authorization
    }

    /// Borrow the opaque local-provider completed-row claim.
    #[must_use]
    pub const fn completed_claim(&self) -> &ProviderIngestFinalizedMusubiCompletionClaimV1 {
        &self.completed_claim
    }
}

/// One bounded page emitted by the completed-Musubi capture scanner.
///
/// `scan_complete` means this page exhausted the exact finalized snapshot.
/// The next scanner call then starts a new snapshot and may observe a later
/// finalized head. When that probe still resolves to the last completely
/// scanned head, every bounded row is revalidated but candidates are
/// suppressed and an empty terminal page is returned.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderIngestCompletedMusubiCapturePageV1 {
    finalized_cursor: ProviderIngestFinalizedCursorV1,
    candidates: Vec<ProviderIngestCompletedMusubiCaptureCandidateV1>,
    scan_complete: bool,
}

impl ProviderIngestCompletedMusubiCapturePageV1 {
    /// Return the exact finalized cursor shared by every validated source row.
    #[must_use]
    pub const fn finalized_cursor(&self) -> ProviderIngestFinalizedCursorV1 {
        self.finalized_cursor
    }

    /// Borrow the completed-Musubi candidates selected from this page.
    #[must_use]
    pub fn candidates(&self) -> &[ProviderIngestCompletedMusubiCaptureCandidateV1] {
        &self.candidates
    }

    /// Return whether this page exhausted the pinned finalized snapshot.
    #[must_use]
    pub const fn scan_complete(&self) -> bool {
        self.scan_complete
    }
}

/// Result of verifying and durably enqueuing one bounded capture page.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ProviderIngestCompletedMusubiReconcileOutcomeV1 {
    /// Exact finalized cursor shared by the reconciled source rows.
    pub finalized_cursor: ProviderIngestFinalizedCursorV1,
    /// Number of completed-Musubi candidates reverified under storage leases.
    pub candidates: usize,
    /// Number of newly inserted approval intents.
    pub inserted: usize,
    /// Number of exact approval intents already retained by the journal.
    pub existing: usize,
    /// Whether this page exhausted its immutable finalized snapshot.
    pub scan_complete: bool,
}

/// Path-free failure while reconciling one completed-Musubi capture page.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub(crate) enum ProviderIngestCompletedMusubiReconcileErrorV1 {
    /// The replay-safe scanner or its projected page failed validation.
    #[error("completed-Musubi finalized capture failed")]
    Capture,
    /// The admitted manifest or its exact stored CAR plan was unavailable.
    #[error("completed-Musubi admitted payload plan is unavailable")]
    AdmittedPlanUnavailable,
    /// Lifecycle-leased bundle verification rejected or could not read the payload.
    #[error("completed-Musubi admitted payload verification failed")]
    VerificationFailed,
    /// The durable approval-intent journal rejected or could not persist the request.
    #[error("completed-Musubi approval intent could not be enqueued")]
    JournalUnavailable,
}

/// Opaque bounded scanner for finalized local-provider Musubi completions.
///
/// Only [`crate::NodeHandle`] can construct this scanner. The raw ledger never
/// receives a claim factory or opaque claim. After it returns a page, the
/// scanner validates the complete unsealed projection, privately creates the
/// factory, seals and revalidates every assignment, owns its continuation
/// cursor, and exposes only the authorization plus opaque completed claim
/// needed by a later capture-only verifier. It performs no storage, signing,
/// journal, inventory, or registry mutation. After a complete scan it performs
/// only one bounded validating probe at an unchanged finalized head and
/// resumes ordinary paging once the head advances.
pub struct ProviderIngestCompletedMusubiCaptureScannerV1<Ledger>
where
    Ledger: ProviderIngestCompletedMusubiCaptureLedgerV1,
{
    provider_id: [u8; 32],
    chain_id: ChainId,
    genesis_block_hash: [u8; 32],
    max_page_rows: usize,
    ledger: Arc<Ledger>,
    scan_cursor: Option<ProviderIngestFinalizedCursorV1>,
    scan_after_order_id: Option<[u8; 32]>,
    last_finalized_cursor: Option<ProviderIngestFinalizedCursorV1>,
    last_completed_cursor: Option<ProviderIngestFinalizedCursorV1>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ProviderIngestCompletedMusubiCaptureProgressV1 {
    scan_cursor: Option<ProviderIngestFinalizedCursorV1>,
    scan_after_order_id: Option<[u8; 32]>,
    last_finalized_cursor: Option<ProviderIngestFinalizedCursorV1>,
    last_completed_cursor: Option<ProviderIngestFinalizedCursorV1>,
}

impl<Ledger> fmt::Debug for ProviderIngestCompletedMusubiCaptureScannerV1<Ledger>
where
    Ledger: ProviderIngestCompletedMusubiCaptureLedgerV1,
{
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ProviderIngestCompletedMusubiCaptureScannerV1")
            .field("provider_id", &self.provider_id)
            .field("chain_id", &self.chain_id)
            .field("max_page_rows", &self.max_page_rows)
            .field("scan_cursor", &self.scan_cursor)
            .field("scan_after_order_id", &self.scan_after_order_id)
            .field("last_finalized_cursor", &self.last_finalized_cursor)
            .field("last_completed_cursor", &self.last_completed_cursor)
            .finish_non_exhaustive()
    }
}

impl<Ledger> ProviderIngestCompletedMusubiCaptureScannerV1<Ledger>
where
    Ledger: ProviderIngestCompletedMusubiCaptureLedgerV1,
{
    pub(crate) fn new(
        provider_id: [u8; 32],
        chain_id: ChainId,
        genesis_block_hash: [u8; 32],
        max_page_rows: usize,
        ledger: Arc<Ledger>,
    ) -> Result<Self, ProviderIngestRuntimeErrorV1> {
        if provider_id == [0; 32] {
            return Err(ProviderIngestRuntimeErrorV1::InvalidProviderId);
        }
        if chain_id.as_str().is_empty()
            || chain_id.as_str().len()
                > provider_ingest_outbox_defaults::COMPLETION_CHAIN_ID_MAX_BYTES_V1
        {
            return Err(ProviderIngestRuntimeErrorV1::InvalidChainId);
        }
        if genesis_block_hash == [0; 32] {
            return Err(ProviderIngestRuntimeErrorV1::InvalidGenesisBlockHash);
        }
        if max_page_rows == 0 || max_page_rows > PROVIDER_INGEST_STATUS_PAGE_MAX_V1 {
            return Err(ProviderIngestRuntimeErrorV1::InvalidPolicy);
        }
        Ok(Self {
            provider_id,
            chain_id,
            genesis_block_hash,
            max_page_rows,
            ledger,
            scan_cursor: None,
            scan_after_order_id: None,
            last_finalized_cursor: None,
            last_completed_cursor: None,
        })
    }

    pub(crate) const fn progress(&self) -> ProviderIngestCompletedMusubiCaptureProgressV1 {
        ProviderIngestCompletedMusubiCaptureProgressV1 {
            scan_cursor: self.scan_cursor,
            scan_after_order_id: self.scan_after_order_id,
            last_finalized_cursor: self.last_finalized_cursor,
            last_completed_cursor: self.last_completed_cursor,
        }
    }

    pub(crate) fn restore_progress(
        &mut self,
        progress: ProviderIngestCompletedMusubiCaptureProgressV1,
    ) {
        self.scan_cursor = progress.scan_cursor;
        self.scan_after_order_id = progress.scan_after_order_id;
        self.last_finalized_cursor = progress.last_finalized_cursor;
        self.last_completed_cursor = progress.last_completed_cursor;
    }

    /// Read and validate the next bounded page from the pinned finalized scan.
    ///
    /// A terminal page resets only the private continuation state. The
    /// monotonic finalized high-water remains retained, so the next fresh scan
    /// may stay at the same head or advance but can never regress or switch an
    /// equal-height hash.
    ///
    /// # Errors
    ///
    /// Returns an error when the finalized reader is unavailable or rejects
    /// the private capability, or when page bounds, cursor lineage, an
    /// assignment, or a sealed claim is malformed or substituted.
    pub async fn next_page(
        &mut self,
    ) -> Result<ProviderIngestCompletedMusubiCapturePageV1, ProviderIngestRuntimeErrorV1> {
        let expected_cursor = self.scan_cursor;
        let after_order_id = self.scan_after_order_id;
        let source_page = self
            .ledger
            .read_completed_musubi_capture_page(expected_cursor, after_order_id, self.max_page_rows)
            .await
            .map_err(map_capture_ledger_error)?;
        let validation = (|| {
            validate_completed_musubi_capture_source_page(
                &source_page,
                after_order_id,
                expected_cursor,
                self.max_page_rows,
                &self.chain_id,
                self.provider_id,
            )?;
            let page = seal_completed_musubi_capture_source_page(
                source_page,
                &ProviderIngestFinalizedClaimFactoryV1::new(
                    self.chain_id.clone(),
                    self.genesis_block_hash,
                    self.provider_id,
                ),
                &self.chain_id,
                self.provider_id,
            )?;
            if after_order_id.is_some()
                && expected_cursor.is_some_and(|cursor| cursor != page.finalized_cursor)
            {
                return Err(ProviderIngestRuntimeErrorV1::InvalidFinalizedPage);
            }
            let finalized_cursor = expected_cursor.unwrap_or(page.finalized_cursor);
            validate_page(&page, after_order_id, finalized_cursor, self.max_page_rows)?;
            validate_monotonic_finalized_cursor(self.last_finalized_cursor, finalized_cursor)?;
            let suppress_unchanged_head =
                expected_cursor.is_none() && self.last_completed_cursor == Some(finalized_cursor);

            let mut candidates = Vec::new();
            candidates
                .try_reserve(page.rows.len())
                .map_err(|_| ProviderIngestRuntimeErrorV1::InvalidFinalizedPage)?;
            for row in &page.rows {
                let validated = validate_assignment_with_source_bound(
                    row,
                    finalized_cursor,
                    self.provider_id,
                    &self.chain_id,
                    self.genesis_block_hash,
                    MAX_REPLICATION_ORDER_ASSIGNMENTS,
                )?;
                if let Some(completed_claim) = row.completed_musubi_archive.as_ref() {
                    if !completed_claim.matches_authorization(&validated.authorization) {
                        return Err(ProviderIngestRuntimeErrorV1::InvalidFinalizedBinding);
                    }
                    candidates.push(ProviderIngestCompletedMusubiCaptureCandidateV1 {
                        authorization: validated.authorization,
                        completed_claim: completed_claim.clone(),
                    });
                }
            }
            Ok((
                finalized_cursor,
                (!suppress_unchanged_head).then_some(candidates),
                page.next_after_order_id,
            ))
        })();
        let (finalized_cursor, candidates, next_after_order_id) = match validation {
            Ok((finalized_cursor, Some(candidates), next_after_order_id)) => {
                (finalized_cursor, candidates, next_after_order_id)
            }
            Ok((finalized_cursor, None, _)) => {
                return Ok(ProviderIngestCompletedMusubiCapturePageV1 {
                    finalized_cursor,
                    candidates: Vec::new(),
                    scan_complete: true,
                });
            }
            Err(validation_error) => return Err(validation_error),
        };

        let scan_complete = next_after_order_id.is_none();
        self.last_finalized_cursor = Some(finalized_cursor);
        if scan_complete {
            self.scan_cursor = None;
            self.scan_after_order_id = None;
            self.last_completed_cursor = Some(finalized_cursor);
        } else {
            self.scan_cursor = Some(finalized_cursor);
            self.scan_after_order_id = next_after_order_id;
        }
        Ok(ProviderIngestCompletedMusubiCapturePageV1 {
            finalized_cursor,
            candidates,
            scan_complete,
        })
    }
}

fn validate_completed_musubi_capture_source_page(
    page: &ProviderIngestCompletedMusubiCaptureSourcePageV1,
    after_order_id: Option<[u8; 32]>,
    expected_cursor: Option<ProviderIngestFinalizedCursorV1>,
    limit: usize,
    expected_chain_id: &ChainId,
    expected_provider_id: [u8; 32],
) -> Result<(), ProviderIngestRuntimeErrorV1> {
    if &page.chain_id != expected_chain_id
        || page.provider_id != expected_provider_id
        || page.finalized_cursor.height == 0
        || page.finalized_cursor.block_hash == [0; 32]
        || page.finalized_block_time_ms == 0
        || expected_cursor.is_some_and(|cursor| cursor != page.finalized_cursor)
        || page.rows.len() > limit
        || page.next_after_order_id.is_some() && page.rows.is_empty()
        || page.next_after_order_id.is_some() && page.rows.len() != limit
    {
        return Err(ProviderIngestRuntimeErrorV1::InvalidFinalizedPage);
    }
    let mut previous = after_order_id;
    for row in &page.rows {
        let order_id = *row.order.order_id.as_bytes();
        if previous.is_some_and(|previous| previous >= order_id)
            || row.pin.finalized_cursor.height != page.finalized_cursor.height
            || row.pin.finalized_cursor.block_hash != page.finalized_cursor.block_hash
            || row.order.assignment_revision == 0
            || row.order.canonical_order.is_empty()
            || row.order.canonical_order.len() > REPLICATION_ORDER_MAX_CANONICAL_BYTES_V1
            || row
                .committed_transaction_hash
                .is_some_and(|hash| hash == [0; 32])
            || row.completion_authority.as_ref().is_some_and(|authority| {
                !authority.is_valid()
                    || row.provider_owner.as_ref() != Some(&authority.provider_owner)
            })
            || row.provider_owner.is_none() && row.completion_authority.is_some()
        {
            return Err(ProviderIngestRuntimeErrorV1::InvalidFinalizedBinding);
        }
        let canonical_order = decode_from_bytes_with_limits::<ReplicationOrderV1>(
            &row.order.canonical_order,
            REPLICATION_ORDER_DECODE_LIMITS_V1,
        )
        .map_err(|_| ProviderIngestRuntimeErrorV1::InvalidFinalizedBinding)?;
        canonical_order
            .validate()
            .map_err(|_| ProviderIngestRuntimeErrorV1::InvalidFinalizedBinding)?;
        let canonical_bytes = norito::to_bytes(&canonical_order)
            .map_err(|_| ProviderIngestRuntimeErrorV1::InvalidFinalizedBinding)?;
        if canonical_bytes != row.order.canonical_order
            || canonical_order.order_id != order_id
            || canonical_order.manifest_digest != *row.order.manifest_digest.as_bytes()
            || canonical_order.manifest_cid.as_slice() != row.order.manifest_root_cid.as_bytes()
            || row.pin.manifest.digest != row.order.manifest_digest
            || row.pin.manifest.root_cid != row.order.manifest_root_cid
            || row.pin.manifest.chunker.to_handle() != canonical_order.chunking_profile
            || !canonical_order
                .assignments
                .iter()
                .any(|assignment| assignment.provider_id == expected_provider_id)
        {
            return Err(ProviderIngestRuntimeErrorV1::InvalidFinalizedBinding);
        }
        match (row.order.musubi_archive, row.musubi_archive.as_ref()) {
            (None, None) => {}
            (Some(archive_id), Some(binding)) => {
                let commitment = &binding.commitment;
                if binding.validate().is_err()
                    || binding.replication_order.as_bytes() != &order_id
                    || binding.archive_id != archive_id
                    || commitment.archive_id() != archive_id
                    || commitment.root_cid != row.pin.manifest.root_cid
                    || commitment.chunker != row.pin.manifest.chunker
                    || commitment.chunk_plan_digest.as_bytes()
                        != &row.pin.manifest.chunk_digest_sha3_256
                    || commitment.por_root.as_bytes() != &row.pin.manifest.por_root
                    || commitment.content_length != row.pin.manifest.content_length
                {
                    return Err(ProviderIngestRuntimeErrorV1::InvalidFinalizedBinding);
                }
            }
            (None, Some(_)) | (Some(_), None) => {
                return Err(ProviderIngestRuntimeErrorV1::InvalidFinalizedBinding);
            }
        }
        previous = Some(order_id);
    }
    if page.next_after_order_id.is_some() && page.next_after_order_id != previous {
        return Err(ProviderIngestRuntimeErrorV1::InvalidFinalizedPage);
    }
    Ok(())
}

fn seal_completed_musubi_capture_source_page(
    source: ProviderIngestCompletedMusubiCaptureSourcePageV1,
    claim_factory: &ProviderIngestFinalizedClaimFactoryV1,
    expected_chain_id: &ChainId,
    expected_provider_id: [u8; 32],
) -> Result<ProviderIngestFinalizedAssignmentPageV1, ProviderIngestRuntimeErrorV1> {
    if &source.chain_id != expected_chain_id || source.provider_id != expected_provider_id {
        return Err(ProviderIngestRuntimeErrorV1::InvalidFinalizedPage);
    }
    let mut rows = Vec::new();
    rows.try_reserve(source.rows.len())
        .map_err(|_| ProviderIngestRuntimeErrorV1::InvalidFinalizedPage)?;
    for source_row in source.rows {
        let order_id = *source_row.order.order_id.as_bytes();
        let musubi_archive = source_row
            .musubi_archive
            .as_ref()
            .map(|binding| {
                claim_factory.seal_musubi_archive(
                    &source.chain_id,
                    source.finalized_cursor,
                    order_id,
                    &source_row.pin.manifest,
                    binding.clone(),
                )
            })
            .transpose()
            .map_err(map_capture_ledger_error)?;
        let completed_musubi_archive = match source_row.musubi_archive {
            Some(binding)
                if source_row
                    .order
                    .provider_completion(ProviderId::new(expected_provider_id))
                    .is_some() =>
            {
                Some(
                    claim_factory
                        .seal_completed_musubi_archive(
                            &source.chain_id,
                            source.finalized_cursor,
                            ProviderId::new(expected_provider_id),
                            &source_row.order,
                            &source_row.pin.manifest,
                            binding,
                        )
                        .map_err(map_capture_ledger_error)?,
                )
            }
            Some(_) | None => None,
        };
        rows.push(ProviderIngestFinalizedAssignmentV1 {
            pin: source_row.pin,
            order: source_row.order,
            musubi_archive,
            completed_musubi_archive,
            provider_owner: source_row.provider_owner,
            completion_authority: source_row.completion_authority,
            completion_epoch: source_row.completion_epoch,
            committed_transaction_hash: source_row.committed_transaction_hash,
        });
    }
    Ok(ProviderIngestFinalizedAssignmentPageV1 {
        finalized_cursor: source.finalized_cursor,
        finalized_block_time_ms: source.finalized_block_time_ms,
        rows,
        next_after_order_id: source.next_after_order_id,
    })
}

fn map_capture_ledger_error(
    error: ProviderIngestFinalizedLedgerErrorV1,
) -> ProviderIngestRuntimeErrorV1 {
    match error {
        ProviderIngestFinalizedLedgerErrorV1::Unavailable => {
            ProviderIngestRuntimeErrorV1::FinalizedLedgerUnavailable
        }
        ProviderIngestFinalizedLedgerErrorV1::Rejected => {
            ProviderIngestRuntimeErrorV1::InvalidFinalizedPage
        }
    }
}

/// Authenticated source-fetch outcome.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderIngestSourceFetchErrorV1 {
    /// No currently admitted authenticated source was reachable.
    Unavailable,
    /// A source returned malformed, noncanonical, or authorization-mismatched material.
    ContentRejected,
    /// A source identity, public policy, or qualification binding was rejected.
    Rejected,
}

/// Exact Musubi commitment forwarded only across the private authenticated source boundary.
///
/// This value is not finalized evidence and cannot authorize storage completion or provider
/// attestation. The runtime derives it from an opaque finalized claim, and the private broker
/// revalidates every field against the ordinary finalized ingest authorization before transport.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderIngestMusubiArchiveFetchBindingV1 {
    chain_id: ChainId,
    genesis_block_hash: [u8; 32],
    provider_id: [u8; 32],
    observed_finalized_cursor: ProviderIngestFinalizedCursorV1,
    binding: MusubiReplicationOrderArchiveBindingV1,
}

impl ProviderIngestMusubiArchiveFetchBindingV1 {
    /// Construct and validate one private-broker Musubi fetch binding.
    ///
    /// This constructor does not confer finality. It exists so an authenticated private broker
    /// can reconstruct the checked informational binding on its server side.
    ///
    /// # Errors
    ///
    /// Returns a fixed rejection when an identity or commitment is inert or noncanonical.
    pub fn new(
        chain_id: ChainId,
        genesis_block_hash: [u8; 32],
        provider_id: [u8; 32],
        observed_finalized_cursor: ProviderIngestFinalizedCursorV1,
        binding: MusubiReplicationOrderArchiveBindingV1,
    ) -> Result<Self, ProviderIngestSourceFetchErrorV1> {
        if chain_id.as_str().is_empty()
            || chain_id.as_str().len()
                > provider_ingest_outbox_defaults::COMPLETION_CHAIN_ID_MAX_BYTES_V1
            || genesis_block_hash == [0; 32]
            || provider_id == [0; 32]
            || observed_finalized_cursor.height == 0
            || observed_finalized_cursor.block_hash == [0; 32]
            || binding.validate().is_err()
        {
            return Err(ProviderIngestSourceFetchErrorV1::Rejected);
        }
        Ok(Self {
            chain_id,
            genesis_block_hash,
            provider_id,
            observed_finalized_cursor,
            binding,
        })
    }

    fn from_finalized_claim(claim: &ProviderIngestFinalizedMusubiArchiveClaimV1) -> Self {
        Self {
            chain_id: claim.chain_id.clone(),
            genesis_block_hash: claim.genesis_block_hash,
            provider_id: claim.provider_id,
            observed_finalized_cursor: claim.observed_finalized_cursor,
            binding: claim.binding.clone(),
        }
    }

    /// Exact configured chain identity.
    #[must_use]
    pub const fn chain_id(&self) -> &ChainId {
        &self.chain_id
    }

    /// Exact configured genesis block hash.
    #[must_use]
    pub const fn genesis_block_hash(&self) -> [u8; 32] {
        self.genesis_block_hash
    }

    /// Provider for which the finalized assignment was read.
    #[must_use]
    pub const fn provider_id(&self) -> [u8; 32] {
        self.provider_id
    }

    /// Finalized archive cursor from which the runtime derived this binding.
    #[must_use]
    pub const fn observed_finalized_cursor(&self) -> ProviderIngestFinalizedCursorV1 {
        self.observed_finalized_cursor
    }

    /// Complete order/archive commitment forwarded to the authenticated source.
    #[must_use]
    pub const fn binding(&self) -> &MusubiReplicationOrderArchiveBindingV1 {
        &self.binding
    }

    /// Check this informational binding against the exact finalized ingest authorization.
    #[must_use]
    pub fn matches_authorization(
        &self,
        authorization: &FinalizedProviderIngestAuthorizationV1,
    ) -> bool {
        let commitment = &self.binding.commitment;
        authorization.validate().is_ok()
            && !self.chain_id.as_str().is_empty()
            && self.chain_id.as_str().len()
                <= provider_ingest_outbox_defaults::COMPLETION_CHAIN_ID_MAX_BYTES_V1
            && self.genesis_block_hash != [0; 32]
            && self.provider_id == authorization.provider_id()
            && authorization_musubi_context_matches(
                authorization,
                &self.chain_id,
                self.genesis_block_hash,
                self.binding.archive_id,
            )
            && finalized_cursor_is_same_or_later(
                self.observed_finalized_cursor,
                authorization.admission_finalized_cursor(),
            )
            && self.binding.validate().is_ok()
            && self.binding.replication_order.as_bytes() == &authorization.order_id()
            && commitment.archive_id() == self.binding.archive_id
            && commitment.root_cid.as_bytes() == authorization.manifest_cid()
            && commitment.chunker.to_handle() == authorization.chunker_handle()
            && commitment.chunk_plan_digest.as_bytes() == &authorization.chunk_digest_sha3_256()
            && commitment.por_root.as_bytes() == &authorization.por_root()
            && commitment.content_length == authorization.content_length()
    }
}

/// Exact fetch request containing no source credentials or lease material.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderIngestSourceRequestV1 {
    authorization: FinalizedProviderIngestAuthorizationV1,
    source_provider_ids: Vec<[u8; 32]>,
    musubi_archive: Option<ProviderIngestMusubiArchiveFetchBindingV1>,
}

impl ProviderIngestSourceRequestV1 {
    /// Construct one canonical request for an authenticated provider source.
    ///
    /// The optional Musubi value is informational transport data, not
    /// finalized evidence. It must agree exactly with the ordinary finalized
    /// ingest authorization.
    ///
    /// # Errors
    ///
    /// Returns a fixed rejection for an invalid authorization, an empty,
    /// oversized, unsorted, duplicate, self-referential, or zero source list,
    /// or a substituted Musubi binding.
    pub fn new(
        authorization: FinalizedProviderIngestAuthorizationV1,
        source_provider_ids: Vec<[u8; 32]>,
        musubi_archive: Option<ProviderIngestMusubiArchiveFetchBindingV1>,
    ) -> Result<Self, ProviderIngestSourceFetchErrorV1> {
        if authorization.validate().is_err()
            || source_provider_ids.is_empty()
            || source_provider_ids.len() > MAX_REPLICATION_ORDER_ASSIGNMENTS
            || source_provider_ids.iter().any(|provider_id| {
                *provider_id == [0; 32] || *provider_id == authorization.provider_id()
            })
            || source_provider_ids
                .windows(2)
                .any(|pair| pair[0] >= pair[1])
            || match (authorization.musubi_context(), musubi_archive.as_ref()) {
                (None, None) => false,
                (Some(_), Some(binding)) => !binding.matches_authorization(&authorization),
                (None, Some(_)) | (Some(_), None) => true,
            }
        {
            return Err(ProviderIngestSourceFetchErrorV1::Rejected);
        }
        Ok(Self {
            authorization,
            source_provider_ids,
            musubi_archive,
        })
    }

    /// Immutable finalized provider/order/manifest authorization.
    #[must_use]
    pub const fn authorization(&self) -> &FinalizedProviderIngestAuthorizationV1 {
        &self.authorization
    }

    /// Canonically ordered governed source provider identities.
    #[must_use]
    pub fn source_provider_ids(&self) -> &[[u8; 32]] {
        &self.source_provider_ids
    }

    /// Informational Musubi commitment derived from the opaque finalized claim.
    #[must_use]
    pub const fn musubi_archive(&self) -> Option<&ProviderIngestMusubiArchiveFetchBindingV1> {
        self.musubi_archive.as_ref()
    }

    /// Consume the request into its checked transport components.
    #[must_use]
    pub fn into_parts(
        self,
    ) -> (
        FinalizedProviderIngestAuthorizationV1,
        Vec<[u8; 32]>,
        Option<ProviderIngestMusubiArchiveFetchBindingV1>,
    ) {
        (
            self.authorization,
            self.source_provider_ids,
            self.musubi_archive,
        )
    }
}

/// Public, non-secret qualification for a top-level provider-ingest adapter.
///
/// The source-pool and governed signer-resolver roles each expose an
/// independently configured value. Credentials, endpoint material, grants,
/// private keys, and payload data are never represented by this type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProviderIngestRuntimeProviderQualificationV1 {
    /// Non-zero adapter and public-policy revision.
    pub revision: u64,
    /// Non-zero digest of the exact public adapter policy.
    pub policy_digest: [u8; 32],
}

impl ProviderIngestRuntimeProviderQualificationV1 {
    /// Construct one first-release public adapter qualification.
    #[must_use]
    pub const fn new(revision: u64, policy_digest: [u8; 32]) -> Self {
        Self {
            revision,
            policy_digest,
        }
    }

    /// Return whether both required public qualification pins are non-zero.
    #[must_use]
    pub fn is_valid(self) -> bool {
        self.revision != 0 && self.policy_digest != [0; 32]
    }
}

/// Payload-free public qualification of one authenticated provider source.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProviderIngestSourceQualificationV1 {
    /// Qualification schema version.
    pub version: u8,
    /// Non-zero adapter and public-policy revision.
    pub revision: u64,
    /// Non-zero digest of the exact public source policy.
    pub policy_digest: [u8; 32],
}

impl ProviderIngestSourceQualificationV1 {
    /// Construct a first-release source qualification.
    #[must_use]
    pub const fn new(revision: u64, policy_digest: [u8; 32]) -> Self {
        Self {
            version: PROVIDER_INGEST_SOURCE_QUALIFICATION_VERSION_V1,
            revision,
            policy_digest,
        }
    }

    /// Validate the qualification schema and non-zero binding.
    ///
    /// # Errors
    ///
    /// Returns an error for an unsupported schema or zero revision/digest.
    pub fn validate(self) -> Result<(), ProviderIngestAuthenticatedSourcePoolErrorV1> {
        if self.version != PROVIDER_INGEST_SOURCE_QUALIFICATION_VERSION_V1
            || self.revision == 0
            || self.policy_digest == [0; 32]
        {
            return Err(ProviderIngestAuthenticatedSourcePoolErrorV1::InvalidSourceQualification);
        }
        Ok(())
    }
}

/// Independently configured public binding for one authenticated provider source.
///
/// The binding contains no endpoint credentials, grants, tokens, private keys,
/// or payload material.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderIngestAuthenticatedSourceBindingV1 {
    /// Exact governed provider identity served by the source.
    pub provider_id: [u8; 32],
    /// Stable opaque identity of the provider-specific transport.
    pub runtime_handle: String,
    /// Exact non-zero adapter and public-policy revision.
    pub revision: u64,
    /// Exact non-zero digest of the public source policy.
    pub policy_digest: [u8; 32],
}

impl ProviderIngestAuthenticatedSourceBindingV1 {
    /// Return the exact qualification required from the injected source.
    #[must_use]
    pub const fn qualification(&self) -> ProviderIngestSourceQualificationV1 {
        ProviderIngestSourceQualificationV1::new(self.revision, self.policy_digest)
    }

    fn validate(
        &self,
        pool_runtime_handle: &str,
    ) -> Result<(), ProviderIngestAuthenticatedSourcePoolErrorV1> {
        if self.provider_id == [0; 32] {
            return Err(ProviderIngestAuthenticatedSourcePoolErrorV1::InvalidProviderId);
        }
        if !is_production_runtime_handle(&self.runtime_handle)
            || self.runtime_handle == pool_runtime_handle
        {
            return Err(ProviderIngestAuthenticatedSourcePoolErrorV1::InvalidSourceHandle);
        }
        self.qualification().validate()
    }
}

/// Authenticated source fetch boundary.
///
/// Production implementations must resolve only current governance-admitted
/// signed adverts, authenticate a bounded stream grant, require HTTPS with
/// pinned trust and DNS-rebinding defenses, reject redirects and implicit
/// decompression, and enforce the exact manifest, chunk-plan, payload-length,
/// chunk digest, and PoR-root binding in the request. Implementations try only
/// a bounded canonical source list and never persist tokens, URLs, or payload
/// bytes in the outbox.
pub trait ProviderIngestAuthenticatedSourceFetchV1: Send + Sync + 'static {
    /// Verified material passed directly to local storage.
    type Fetched: Send + 'static;

    /// Fetch and verify exact material from an authenticated governed source.
    fn fetch<'a>(
        &'a self,
        request: ProviderIngestSourceRequestV1,
    ) -> ProviderIngestFutureV1<'a, Result<Self::Fetched, ProviderIngestSourceFetchErrorV1>>;
}

/// One independently authenticated provider source used by the production pool.
///
/// Implementations own their runtime-only endpoint, grant, credential, and
/// pinned-trust material. The stable handle and provider identity are public
/// policy identifiers only. A fetch must use a hard transport deadline, reject
/// redirects and implicit decompression, and verify the exact finalized
/// authorization before returning.
pub trait ProviderIngestAuthenticatedProviderSourceV1: Send + Sync + 'static {
    /// Verified material returned by this source.
    type Fetched: Send + 'static;

    /// Exact governed provider identity served by this source.
    fn provider_id(&self) -> [u8; 32];

    /// Stable public identity of this provider-specific transport.
    fn runtime_handle(&self) -> &str;

    /// Current payload-free adapter and public-policy qualification.
    ///
    /// # Errors
    ///
    /// Returns a fixed payload-free source failure when the qualification
    /// cannot be authenticated.
    fn qualification(
        &self,
    ) -> Result<ProviderIngestSourceQualificationV1, ProviderIngestSourceFetchErrorV1>;

    /// Non-mutating authenticated readiness check.
    ///
    /// # Errors
    ///
    /// Returns a fixed payload-free source failure when the source is not
    /// currently ready or its public policy is rejected.
    fn check_readiness(&self) -> Result<(), ProviderIngestSourceFetchErrorV1>;

    /// Fetch exact material for one finalized authorization.
    fn fetch_provider<'a>(
        &'a self,
        authorization: FinalizedProviderIngestAuthorizationV1,
        musubi_archive: Option<ProviderIngestMusubiArchiveFetchBindingV1>,
    ) -> ProviderIngestFutureV1<'a, Result<Self::Fetched, ProviderIngestSourceFetchErrorV1>>;
}

/// Invalid construction of an authenticated multi-provider source pool.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum ProviderIngestAuthenticatedSourcePoolErrorV1 {
    /// The pool's public runtime identity is not production-safe.
    #[error("provider-ingest source-pool handle is invalid")]
    InvalidPoolHandle,
    /// The pool's top-level public qualification is zero or otherwise invalid.
    #[error("provider-ingest source-pool qualification is invalid")]
    InvalidPoolQualification,
    /// The per-request source bound is outside the V1 protocol range.
    #[error("provider-ingest source-pool request bound is invalid")]
    InvalidSourceLimit,
    /// A production pool must contain at least two and at most the V1 maximum sources.
    #[error("provider-ingest source-pool size is invalid")]
    InvalidSourceCount,
    /// A source exposed a zero provider identity.
    #[error("provider-ingest source-pool provider identity is invalid")]
    InvalidProviderId,
    /// A source's public runtime identity is invalid or aliases the pool.
    #[error("provider-ingest provider-source handle is invalid")]
    InvalidSourceHandle,
    /// A configured source qualification is unsupported or contains a zero pin.
    #[error("provider-ingest provider-source qualification is invalid")]
    InvalidSourceQualification,
    /// An injected source does not match its independent public binding.
    #[error("provider-ingest provider-source identity or qualification does not match")]
    SourceBindingMismatch,
    /// Two sources claimed the same governed provider.
    #[error("provider-ingest source-pool contains a duplicate provider")]
    DuplicateProvider,
    /// Two independently administered sources reused one runtime identity.
    #[error("provider-ingest source-pool contains a duplicate source handle")]
    DuplicateSourceHandle,
}

/// One independently bound authenticated provider source.
pub struct ProviderIngestAuthenticatedSourceRegistrationV1<Fetched: Send + 'static> {
    binding: ProviderIngestAuthenticatedSourceBindingV1,
    source: Arc<dyn ProviderIngestAuthenticatedProviderSourceV1<Fetched = Fetched>>,
}

impl<Fetched: Send + 'static> fmt::Debug
    for ProviderIngestAuthenticatedSourceRegistrationV1<Fetched>
{
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ProviderIngestAuthenticatedSourceRegistrationV1")
            .field("binding", &self.binding)
            .finish_non_exhaustive()
    }
}

impl<Fetched: Send + 'static> ProviderIngestAuthenticatedSourceRegistrationV1<Fetched> {
    /// Pair an independently configured public binding with one injected source.
    #[must_use]
    pub fn new(
        binding: ProviderIngestAuthenticatedSourceBindingV1,
        source: Arc<dyn ProviderIngestAuthenticatedProviderSourceV1<Fetched = Fetched>>,
    ) -> Self {
        Self { binding, source }
    }
}

struct PinnedProviderIngestSourceV1<Fetched: Send + 'static> {
    binding: ProviderIngestAuthenticatedSourceBindingV1,
    source: Arc<dyn ProviderIngestAuthenticatedProviderSourceV1<Fetched = Fetched>>,
}

/// Bounded, identity-pinned authenticated multi-provider source coordinator.
///
/// The pool freezes a canonical public provider inventory at construction,
/// retains an independent top-level revision/policy-digest qualification,
/// rejects missing or substituted sources before contacting any transport, and
/// tries only the exact canonical source list carried by finalized assignment
/// state. Each selected source is rechecked before and after its fetch. Source
/// locations, grants, credentials, and payload bytes remain inside the child
/// adapters and are never copied into pool metadata or durable state.
pub struct ProviderIngestAuthenticatedSourcePoolV1<Fetched: Send + 'static> {
    runtime_handle: String,
    qualification: ProviderIngestRuntimeProviderQualificationV1,
    max_sources_per_fetch: usize,
    provider_ids: Vec<[u8; 32]>,
    sources: BTreeMap<[u8; 32], PinnedProviderIngestSourceV1<Fetched>>,
}

impl<Fetched: Send + 'static> fmt::Debug for ProviderIngestAuthenticatedSourcePoolV1<Fetched> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ProviderIngestAuthenticatedSourcePoolV1")
            .field("runtime_handle", &self.runtime_handle)
            .field("qualification", &self.qualification)
            .field("max_sources_per_fetch", &self.max_sources_per_fetch)
            .field("provider_ids", &self.provider_ids)
            .finish_non_exhaustive()
    }
}

impl<Fetched: Send + 'static> ProviderIngestAuthenticatedSourcePoolV1<Fetched> {
    /// Construct a production pool from independently configured source bindings.
    ///
    /// # Errors
    ///
    /// Returns an error when the pool bounds or any independently configured
    /// source identity, handle, qualification, or injected adapter is invalid.
    pub fn new(
        runtime_handle: impl Into<String>,
        qualification: ProviderIngestRuntimeProviderQualificationV1,
        max_sources_per_fetch: usize,
        sources: Vec<ProviderIngestAuthenticatedSourceRegistrationV1<Fetched>>,
    ) -> Result<Self, ProviderIngestAuthenticatedSourcePoolErrorV1> {
        let runtime_handle = runtime_handle.into();
        if !is_production_runtime_handle(&runtime_handle) {
            return Err(ProviderIngestAuthenticatedSourcePoolErrorV1::InvalidPoolHandle);
        }
        if !qualification.is_valid() {
            return Err(ProviderIngestAuthenticatedSourcePoolErrorV1::InvalidPoolQualification);
        }
        if max_sources_per_fetch == 0 || max_sources_per_fetch > MAX_REPLICATION_ORDER_ASSIGNMENTS {
            return Err(ProviderIngestAuthenticatedSourcePoolErrorV1::InvalidSourceLimit);
        }
        if sources.len() < 2 || sources.len() > MAX_REPLICATION_ORDER_ASSIGNMENTS {
            return Err(ProviderIngestAuthenticatedSourcePoolErrorV1::InvalidSourceCount);
        }

        let mut pinned_handles = BTreeSet::new();
        let mut pinned_sources = BTreeMap::new();
        for registration in sources {
            let binding = registration.binding;
            binding.validate(&runtime_handle)?;
            if !pinned_handles.insert(binding.runtime_handle.clone()) {
                return Err(ProviderIngestAuthenticatedSourcePoolErrorV1::DuplicateSourceHandle);
            }
            let source = registration.source;
            let qualification = source
                .qualification()
                .map_err(|_| ProviderIngestAuthenticatedSourcePoolErrorV1::SourceBindingMismatch)?;
            if source.provider_id() != binding.provider_id
                || source.runtime_handle() != binding.runtime_handle.as_str()
                || qualification != binding.qualification()
            {
                return Err(ProviderIngestAuthenticatedSourcePoolErrorV1::SourceBindingMismatch);
            }
            let provider_id = binding.provider_id;
            let pinned = PinnedProviderIngestSourceV1 { binding, source };
            if pinned_sources.insert(provider_id, pinned).is_some() {
                return Err(ProviderIngestAuthenticatedSourcePoolErrorV1::DuplicateProvider);
            }
        }
        let provider_ids = pinned_sources.keys().copied().collect();
        Ok(Self {
            runtime_handle,
            qualification,
            max_sources_per_fetch,
            provider_ids,
            sources: pinned_sources,
        })
    }

    /// Stable public identity of the complete source pool.
    #[must_use]
    pub fn runtime_handle(&self) -> &str {
        &self.runtime_handle
    }

    /// Return the pool's exact top-level adapter qualification.
    #[must_use]
    pub const fn qualification(&self) -> ProviderIngestRuntimeProviderQualificationV1 {
        self.qualification
    }

    /// Canonical identity-pinned provider inventory.
    #[must_use]
    pub fn source_provider_ids(&self) -> &[[u8; 32]] {
        &self.provider_ids
    }

    /// Maximum sources admitted in one finalized fetch request.
    #[must_use]
    pub const fn max_sources_per_fetch(&self) -> usize {
        self.max_sources_per_fetch
    }

    /// Revalidate every pinned source without exposing request material.
    ///
    /// # Errors
    ///
    /// Returns [`ProviderIngestSourceFetchErrorV1::Unavailable`] when no
    /// independently qualified source is currently ready, or
    /// [`ProviderIngestSourceFetchErrorV1::Rejected`] when any pinned source is
    /// substituted, stale, or policy-rejected.
    pub fn check_readiness(&self) -> Result<(), ProviderIngestSourceFetchErrorV1> {
        if !is_production_runtime_handle(&self.runtime_handle)
            || !self.qualification.is_valid()
            || self.provider_ids.len() < 2
            || self.provider_ids.len() != self.sources.len()
            || self.max_sources_per_fetch == 0
            || self.max_sources_per_fetch > MAX_REPLICATION_ORDER_ASSIGNMENTS
        {
            return Err(ProviderIngestSourceFetchErrorV1::Rejected);
        }
        let mut ready = false;
        for provider_id in &self.provider_ids {
            let source = self
                .sources
                .get(provider_id)
                .ok_or(ProviderIngestSourceFetchErrorV1::Rejected)?;
            if self.source_is_ready(source)? {
                ready = true;
            }
        }
        if !ready {
            return Err(ProviderIngestSourceFetchErrorV1::Unavailable);
        }
        Ok(())
    }

    fn source_is_ready(
        &self,
        source: &PinnedProviderIngestSourceV1<Fetched>,
    ) -> Result<bool, ProviderIngestSourceFetchErrorV1> {
        let before_ready = match self.validate_source(source) {
            Ok(()) => true,
            Err(ProviderIngestSourceFetchErrorV1::Unavailable) => false,
            Err(
                ProviderIngestSourceFetchErrorV1::ContentRejected
                | ProviderIngestSourceFetchErrorV1::Rejected,
            ) => return Err(ProviderIngestSourceFetchErrorV1::Rejected),
        };
        let readiness = source.source.check_readiness();
        let after = self.validate_source(source);
        if matches!(
            readiness,
            Err(ProviderIngestSourceFetchErrorV1::ContentRejected
                | ProviderIngestSourceFetchErrorV1::Rejected)
        ) || matches!(
            after,
            Err(ProviderIngestSourceFetchErrorV1::ContentRejected
                | ProviderIngestSourceFetchErrorV1::Rejected)
        ) {
            return Err(ProviderIngestSourceFetchErrorV1::Rejected);
        }
        Ok(before_ready && readiness.is_ok() && after.is_ok())
    }

    fn validate_source(
        &self,
        source: &PinnedProviderIngestSourceV1<Fetched>,
    ) -> Result<(), ProviderIngestSourceFetchErrorV1> {
        let actual_handle = source.source.runtime_handle();
        if source.binding.validate(&self.runtime_handle).is_err()
            || source.source.provider_id() != source.binding.provider_id
            || !is_production_runtime_handle(actual_handle)
            || actual_handle != source.binding.runtime_handle.as_str()
        {
            return Err(ProviderIngestSourceFetchErrorV1::Rejected);
        }
        let actual_qualification = match source.source.qualification() {
            Ok(qualification) => qualification,
            Err(ProviderIngestSourceFetchErrorV1::Unavailable) => {
                return Err(ProviderIngestSourceFetchErrorV1::Unavailable);
            }
            Err(
                ProviderIngestSourceFetchErrorV1::ContentRejected
                | ProviderIngestSourceFetchErrorV1::Rejected,
            ) => return Err(ProviderIngestSourceFetchErrorV1::Rejected),
        };
        if actual_qualification != source.binding.qualification() {
            return Err(ProviderIngestSourceFetchErrorV1::Rejected);
        }
        Ok(())
    }

    fn validate_request(
        &self,
        request: &ProviderIngestSourceRequestV1,
    ) -> Result<(), ProviderIngestSourceFetchErrorV1> {
        if request.source_provider_ids().len() > self.max_sources_per_fetch
            || request.source_provider_ids().iter().any(|provider_id| {
                *provider_id == [0; 32]
                    || *provider_id == request.authorization().provider_id()
                    || !self.sources.contains_key(provider_id)
            })
            || request
                .source_provider_ids()
                .windows(2)
                .any(|pair| pair[0] >= pair[1])
            || request
                .musubi_archive()
                .is_some_and(|binding| !binding.matches_authorization(request.authorization()))
        {
            return Err(ProviderIngestSourceFetchErrorV1::Rejected);
        }
        Ok(())
    }
}

impl<Fetched: Send + 'static> ProviderIngestAuthenticatedSourceFetchV1
    for ProviderIngestAuthenticatedSourcePoolV1<Fetched>
{
    type Fetched = Fetched;

    fn fetch<'a>(
        &'a self,
        request: ProviderIngestSourceRequestV1,
    ) -> ProviderIngestFutureV1<'a, Result<Self::Fetched, ProviderIngestSourceFetchErrorV1>> {
        Box::pin(async move {
            self.validate_request(&request)?;
            let mut content_rejected = false;
            for provider_id in request.source_provider_ids() {
                let source = self
                    .sources
                    .get(provider_id)
                    .ok_or(ProviderIngestSourceFetchErrorV1::Rejected)?;
                if !self.source_is_ready(source)? {
                    continue;
                }
                let result = source
                    .source
                    .fetch_provider(
                        request.authorization().clone(),
                        request.musubi_archive().cloned(),
                    )
                    .await;
                let post_fetch_ready = self.source_is_ready(source)?;
                match result {
                    Ok(fetched) if post_fetch_ready => return Ok(fetched),
                    Ok(_) => {}
                    Err(ProviderIngestSourceFetchErrorV1::Unavailable) => {}
                    Err(ProviderIngestSourceFetchErrorV1::ContentRejected) => {
                        content_rejected = true;
                    }
                    Err(ProviderIngestSourceFetchErrorV1::Rejected) => {
                        return Err(ProviderIngestSourceFetchErrorV1::Rejected);
                    }
                }
            }
            Err(if content_rejected {
                ProviderIngestSourceFetchErrorV1::ContentRejected
            } else {
                ProviderIngestSourceFetchErrorV1::Unavailable
            })
        })
    }
}

/// Local storage verification/persistence failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderIngestLocalStorageErrorV1 {
    /// Local storage failed in a retryable manner.
    Retryable,
    /// Local storage permanently rejected exact verified material.
    Permanent,
}

/// Closed evidence that one immutable local payload passed the complete Musubi verifier.
///
/// The value can be constructed only from [`VerifiedMusubiBundleV1`], whose representation is
/// private to the canonical `sorafs_car` verifier. It is deliberately not Norito-decodable:
/// durable checkpoints use a crate-private representation so downstream callers cannot fabricate
/// verifier evidence from public field-shaped bytes. It is a pre-completion receipt, not a
/// provider attestation, and intentionally contains no finalized completion row or signature.
///
/// ```compile_fail
/// use sorafs_node::ProviderIngestVerifiedMusubiBundleReceiptV1;
///
/// let encoded = [];
/// let _: ProviderIngestVerifiedMusubiBundleReceiptV1 =
///     norito::decode_from_bytes(&encoded).expect("public receipts are not decodable");
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderIngestVerifiedMusubiBundleReceiptV1 {
    chain_id: ChainId,
    genesis_block_hash: [u8; 32],
    provider_id: [u8; 32],
    observed_finalized_cursor: ProviderIngestFinalizedCursorV1,
    replication_order: [u8; 32],
    manifest_digest: [u8; 32],
    archive_id: ArchiveId,
    commitment: MusubiArchiveCommitmentV1,
    semantic_release_manifest_digest: MusubiSemanticReleaseDigestV1,
    verification_lock_digest: MusubiVerificationLockDigestV1,
}

/// Crate-private canonical checkpoint representation of a verified Musubi receipt.
///
/// Keeping codec implementations on this non-exported type preserves durable checkpointing
/// without turning public receipt bytes into a construction capability.
#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
pub(crate) struct StoredProviderIngestVerifiedMusubiBundleReceiptV1 {
    chain_id: ChainId,
    genesis_block_hash: [u8; 32],
    provider_id: [u8; 32],
    observed_finalized_cursor: ProviderIngestFinalizedCursorV1,
    replication_order: [u8; 32],
    manifest_digest: [u8; 32],
    archive_id: ArchiveId,
    commitment: MusubiArchiveCommitmentV1,
    semantic_release_manifest_digest: MusubiSemanticReleaseDigestV1,
    verification_lock_digest: MusubiVerificationLockDigestV1,
}

impl ProviderIngestVerifiedMusubiBundleReceiptV1 {
    /// Bind canonical verifier output to the exact opaque finalized precursor.
    ///
    /// # Errors
    ///
    /// Returns a permanent failure if the parsed descriptor does not agree with the verified
    /// semantic release, verification lock, or finalized archive commitment.
    pub fn from_verified_bundle(
        claim: &ProviderIngestFinalizedMusubiArchiveClaimV1,
        authorization: &FinalizedProviderIngestAuthorizationV1,
        verified: &VerifiedMusubiBundleV1,
    ) -> Result<Self, ProviderIngestLocalStorageErrorV1> {
        let descriptor = verified.descriptor();
        let semantic_release_manifest_digest = verified.semantic_release().semantic_digest();
        let verification_lock_digest = verified.verification_lock().digest();
        if verified.archive_id() != claim.archive_id()
            || !claim.matches_authorization(authorization)
            || descriptor.semantic_release_manifest_digest != semantic_release_manifest_digest
            || descriptor.verification_lock_digest != verification_lock_digest
            || descriptor.source_tree_digest != claim.commitment().source_tree_digest
            || claim.archive_id() != claim.commitment().archive_id()
            || claim.provider_id() != authorization.provider_id()
            || claim.replication_order() != authorization.order_id()
        {
            return Err(ProviderIngestLocalStorageErrorV1::Permanent);
        }
        let receipt = Self {
            chain_id: claim.chain_id.clone(),
            genesis_block_hash: claim.genesis_block_hash,
            provider_id: claim.provider_id,
            observed_finalized_cursor: claim.observed_finalized_cursor,
            replication_order: claim.replication_order(),
            manifest_digest: authorization.manifest_digest(),
            archive_id: claim.archive_id(),
            commitment: claim.commitment().clone(),
            semantic_release_manifest_digest,
            verification_lock_digest,
        };
        if !receipt.validate_stored(authorization) {
            return Err(ProviderIngestLocalStorageErrorV1::Permanent);
        }
        Ok(receipt)
    }

    /// Return whether this receipt covers the exact finalized precursor and local authorization.
    #[must_use]
    pub fn matches(
        &self,
        claim: &ProviderIngestFinalizedMusubiArchiveClaimV1,
        authorization: &FinalizedProviderIngestAuthorizationV1,
    ) -> bool {
        self.validate_stored(authorization)
            && claim.matches_authorization(authorization)
            && self.chain_id == claim.chain_id
            && self.genesis_block_hash == claim.genesis_block_hash
            && self.provider_id == claim.provider_id
            && self.provider_id == authorization.provider_id()
            && finalized_cursor_is_same_or_later(
                self.observed_finalized_cursor,
                authorization.admission_finalized_cursor(),
            )
            && finalized_cursor_is_same_or_later(
                claim.observed_finalized_cursor,
                self.observed_finalized_cursor,
            )
            && self.replication_order == claim.replication_order()
            && self.replication_order == authorization.order_id()
            && self.manifest_digest == authorization.manifest_digest()
            && self.archive_id == claim.archive_id()
            && self.commitment == *claim.commitment()
            && !self.semantic_release_manifest_digest.is_zero()
            && !self.verification_lock_digest.is_zero()
    }

    pub(crate) fn validate_stored(
        &self,
        authorization: &FinalizedProviderIngestAuthorizationV1,
    ) -> bool {
        let commitment = &self.commitment;
        authorization.validate().is_ok()
            && !self.chain_id.as_str().is_empty()
            && self.chain_id.as_str().len()
                <= provider_ingest_outbox_defaults::COMPLETION_CHAIN_ID_MAX_BYTES_V1
            && self.genesis_block_hash != [0; 32]
            && self.provider_id == authorization.provider_id()
            && authorization_musubi_context_matches(
                authorization,
                &self.chain_id,
                self.genesis_block_hash,
                self.archive_id,
            )
            && self.replication_order == authorization.order_id()
            && self.manifest_digest == authorization.manifest_digest()
            && finalized_cursor_is_same_or_later(
                self.observed_finalized_cursor,
                authorization.admission_finalized_cursor(),
            )
            && self.archive_id == commitment.archive_id()
            && commitment.validate().is_ok()
            && commitment.root_cid.as_bytes() == authorization.manifest_cid()
            && commitment.chunker.to_handle() == authorization.chunker_handle()
            && commitment.chunk_plan_digest.as_bytes() == &authorization.chunk_digest_sha3_256()
            && commitment.por_root.as_bytes() == &authorization.por_root()
            && commitment.content_length == authorization.content_length()
            && !self.semantic_release_manifest_digest.is_zero()
            && !self.verification_lock_digest.is_zero()
            && norito::to_bytes(&self.to_stored()).is_ok_and(|encoded| {
                encoded.len() <= PROVIDER_INGEST_VERIFIED_MUSUBI_RECEIPT_MAX_CANONICAL_BYTES_V1
            })
    }

    /// Domain-separated semantic release-manifest digest parsed from the bundle.
    #[must_use]
    pub const fn semantic_release_manifest_digest(&self) -> MusubiSemanticReleaseDigestV1 {
        self.semantic_release_manifest_digest
    }

    /// Normalized verification-lock digest parsed from the bundle.
    #[must_use]
    pub const fn verification_lock_digest(&self) -> MusubiVerificationLockDigestV1 {
        self.verification_lock_digest
    }

    pub(crate) fn to_stored(&self) -> StoredProviderIngestVerifiedMusubiBundleReceiptV1 {
        StoredProviderIngestVerifiedMusubiBundleReceiptV1 {
            chain_id: self.chain_id.clone(),
            genesis_block_hash: self.genesis_block_hash,
            provider_id: self.provider_id,
            observed_finalized_cursor: self.observed_finalized_cursor,
            replication_order: self.replication_order,
            manifest_digest: self.manifest_digest,
            archive_id: self.archive_id,
            commitment: self.commitment.clone(),
            semantic_release_manifest_digest: self.semantic_release_manifest_digest,
            verification_lock_digest: self.verification_lock_digest,
        }
    }

    #[cfg(test)]
    pub(crate) fn new_for_test(
        authorization: &FinalizedProviderIngestAuthorizationV1,
        commitment: MusubiArchiveCommitmentV1,
        semantic_release_manifest_digest: MusubiSemanticReleaseDigestV1,
        verification_lock_digest: MusubiVerificationLockDigestV1,
    ) -> Self {
        let context = authorization
            .musubi_context()
            .expect("test receipt requires Musubi authorization context");
        Self {
            chain_id: context.chain_id().clone(),
            genesis_block_hash: context.genesis_block_hash(),
            provider_id: authorization.provider_id(),
            observed_finalized_cursor: authorization.admission_finalized_cursor(),
            replication_order: authorization.order_id(),
            manifest_digest: authorization.manifest_digest(),
            archive_id: context.archive_id(),
            commitment,
            semantic_release_manifest_digest,
            verification_lock_digest,
        }
    }
}

impl StoredProviderIngestVerifiedMusubiBundleReceiptV1 {
    pub(crate) const fn observed_finalized_cursor(&self) -> ProviderIngestFinalizedCursorV1 {
        self.observed_finalized_cursor
    }

    #[cfg(test)]
    pub(crate) const fn set_observed_finalized_cursor_for_test(
        &mut self,
        cursor: ProviderIngestFinalizedCursorV1,
    ) {
        self.observed_finalized_cursor = cursor;
    }

    pub(crate) fn into_receipt(self) -> ProviderIngestVerifiedMusubiBundleReceiptV1 {
        ProviderIngestVerifiedMusubiBundleReceiptV1 {
            chain_id: self.chain_id,
            genesis_block_hash: self.genesis_block_hash,
            provider_id: self.provider_id,
            observed_finalized_cursor: self.observed_finalized_cursor,
            replication_order: self.replication_order,
            manifest_digest: self.manifest_digest,
            archive_id: self.archive_id,
            commitment: self.commitment,
            semantic_release_manifest_digest: self.semantic_release_manifest_digest,
            verification_lock_digest: self.verification_lock_digest,
        }
    }

    pub(crate) fn to_receipt(&self) -> ProviderIngestVerifiedMusubiBundleReceiptV1 {
        self.clone().into_receipt()
    }

    pub(crate) fn validate_stored(
        &self,
        authorization: &FinalizedProviderIngestAuthorizationV1,
    ) -> bool {
        self.to_receipt().validate_stored(authorization)
    }
}

fn finalized_cursor_is_same_or_later(
    candidate: ProviderIngestFinalizedCursorV1,
    baseline: ProviderIngestFinalizedCursorV1,
) -> bool {
    candidate.height > baseline.height
        || candidate.height == baseline.height && candidate.block_hash == baseline.block_hash
}

fn authorization_musubi_context_matches(
    authorization: &FinalizedProviderIngestAuthorizationV1,
    chain_id: &ChainId,
    genesis_block_hash: [u8; 32],
    archive_id: ArchiveId,
) -> bool {
    authorization.musubi_context().is_some_and(|context| {
        context.chain_id() == chain_id
            && context.genesis_block_hash() == genesis_block_hash
            && context.archive_id() == archive_id
    })
}

/// Exact local-storage result accepted before a completion transaction may be prepared.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderIngestLocalStoredV1 {
    manifest_id: String,
    musubi_bundle: Option<ProviderIngestVerifiedMusubiBundleReceiptV1>,
}

impl ProviderIngestLocalStoredV1 {
    /// Construct a result for an ordinary non-Musubi replication order.
    #[must_use]
    pub fn generic(manifest_id: String) -> Self {
        Self {
            manifest_id,
            musubi_bundle: None,
        }
    }

    /// Construct a result whose local payload passed the complete Musubi verifier.
    #[must_use]
    pub fn musubi(
        manifest_id: String,
        receipt: ProviderIngestVerifiedMusubiBundleReceiptV1,
    ) -> Self {
        Self {
            manifest_id,
            musubi_bundle: Some(receipt),
        }
    }

    /// Canonical local manifest identity.
    #[must_use]
    pub fn manifest_id(&self) -> &str {
        &self.manifest_id
    }

    /// Verified Musubi receipt, absent only for a generic replication order.
    #[must_use]
    pub const fn musubi_bundle(&self) -> Option<&ProviderIngestVerifiedMusubiBundleReceiptV1> {
        self.musubi_bundle.as_ref()
    }
}

/// Exact local storage boundary.
pub trait ProviderIngestLocalStorageV1<Fetched>: Send + Sync + 'static {
    /// Verify whether exact authorized material is already durable locally.
    ///
    /// A Musubi claim requires the complete semantic bundle verifier and a matching receipt.
    fn verify_existing<'a>(
        &'a self,
        authorization: FinalizedProviderIngestAuthorizationV1,
        musubi_archive: Option<ProviderIngestFinalizedMusubiArchiveClaimV1>,
    ) -> ProviderIngestFutureV1<
        'a,
        Result<Option<ProviderIngestLocalStoredV1>, ProviderIngestLocalStorageErrorV1>,
    >;

    /// Atomically store exact material, then verify any Musubi bundle from admitted storage.
    fn store<'a>(
        &'a self,
        authorization: FinalizedProviderIngestAuthorizationV1,
        musubi_archive: Option<ProviderIngestFinalizedMusubiArchiveClaimV1>,
        fetched: Fetched,
    ) -> ProviderIngestFutureV1<
        'a,
        Result<ProviderIngestLocalStoredV1, ProviderIngestLocalStorageErrorV1>,
    >;
}

/// Request for one exact fee-quoted provider completion payload.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderIngestCompletionPayloadRequestV1 {
    /// Immutable provider/order/manifest authorization.
    pub authorization: FinalizedProviderIngestAuthorizationV1,
    /// Current finalized provider owner.
    pub provider_owner: AccountId,
    /// Exact finalized completion authority to compare again at commit.
    pub expected_authority: ProviderIngestCompletionAuthorityV1,
    /// Exact order-scoped assignment revision to compare again at commit.
    pub expected_assignment_revision: u64,
    /// Exact configured production chain identity.
    pub chain_id: ChainId,
    /// Authoritative completion epoch.
    pub completion_epoch: u64,
    /// Finalized baseline preceding signing.
    pub finalized_cursor: ProviderIngestFinalizedCursorV1,
}

/// Completion payload construction failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderIngestCompletionPayloadErrorV1 {
    /// Fee quoting or payload construction is temporarily unavailable.
    Unavailable,
    /// Current policy rejects completion payload construction.
    Rejected,
}

/// Builds the exact bounded, fee-quoted transaction payload to sign.
pub trait ProviderIngestCompletionPayloadBuilderV1: Send + Sync + 'static {
    /// Build one exact completion payload.
    fn build_payload<'a>(
        &'a self,
        request: ProviderIngestCompletionPayloadRequestV1,
    ) -> ProviderIngestFutureV1<
        'a,
        Result<TransactionPayload, ProviderIngestCompletionPayloadErrorV1>,
    >;
}

/// Isolated signer failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderIngestCompletionSignerErrorV1 {
    /// HSM/KMS signing is temporarily unavailable.
    Unavailable,
    /// The signer rejected an otherwise exact prepared operation.
    Rejected,
}

/// Exact finalized authorization independently pinned while resolving a signer.
///
/// The context is carried separately from the transaction payload so an
/// isolated signer can reject payloads whose owner, signer policy, assignment
/// revision, or finalized anchor was substituted after resolution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderIngestCompletionSignerResolutionContextV1 {
    /// Exact finalized provider owner authorized to sign.
    pub provider_owner: AccountId,
    /// Exact finalized signer-policy identity and digest lineage.
    pub signer_policy: ProviderIngestCompletionSignerPolicyV1,
    /// Exact non-zero order-scoped assignment revision.
    pub expected_assignment_revision: u64,
    /// Exact finalized baseline whose anchor must appear in the completion.
    pub finalized_cursor: ProviderIngestFinalizedCursorV1,
}

impl ProviderIngestCompletionSignerResolutionContextV1 {
    /// Construct one exact signer-resolution context.
    #[must_use]
    pub fn new(
        provider_owner: AccountId,
        signer_policy: ProviderIngestCompletionSignerPolicyV1,
        expected_assignment_revision: u64,
        finalized_cursor: ProviderIngestFinalizedCursorV1,
    ) -> Self {
        Self {
            provider_owner,
            signer_policy,
            expected_assignment_revision,
            finalized_cursor,
        }
    }

    /// Return whether every independently pinned field is production-shaped.
    #[must_use]
    pub fn is_valid(&self) -> bool {
        self.signer_policy.is_valid()
            && self.expected_assignment_revision != 0
            && self.finalized_cursor.height != 0
            && self.finalized_cursor.block_hash != [0; 32]
    }
}

/// Payload-free public qualification of one provider-ingest completion signer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderIngestCompletionSignerQualificationV1 {
    /// Qualification schema version.
    pub version: u8,
    /// Non-zero signer adapter and public-policy revision.
    pub adapter_revision: u64,
    /// Exact chain-authoritative signer policy identity and digest lineage.
    pub signer_policy: ProviderIngestCompletionSignerPolicyV1,
    /// Exact admitted signature algorithm.
    pub algorithm: Algorithm,
    /// Exact public key controlled by the external signer.
    pub public_key: PublicKey,
}

impl ProviderIngestCompletionSignerQualificationV1 {
    /// Construct a first-release completion-signer qualification.
    #[must_use]
    pub fn new(
        adapter_revision: u64,
        signer_policy: ProviderIngestCompletionSignerPolicyV1,
        algorithm: Algorithm,
        public_key: PublicKey,
    ) -> Self {
        Self {
            version: PROVIDER_INGEST_COMPLETION_SIGNER_QUALIFICATION_VERSION_V1,
            adapter_revision,
            signer_policy,
            algorithm,
            public_key,
        }
    }

    /// Validate the schema, revision, policy lineage, algorithm, and public key.
    ///
    /// # Errors
    ///
    /// Returns an error for an unsupported schema, zero revision, invalid
    /// signer policy, unapproved algorithm, or algorithm/key mismatch.
    pub fn validate(&self) -> Result<(), ProviderIngestCompletionSignerBindingErrorV1> {
        let public_key = self.public_key.try_to_bytes().ok();
        if self.version != PROVIDER_INGEST_COMPLETION_SIGNER_QUALIFICATION_VERSION_V1
            || self.adapter_revision == 0
            || !self.signer_policy.is_valid()
            || !matches!(self.algorithm, Algorithm::Ed25519 | Algorithm::MlDsa)
            || public_key.is_none_or(|(algorithm, bytes)| {
                algorithm != self.algorithm
                    || bytes.is_empty()
                    || bytes.iter().all(|byte| *byte == 0)
            })
        {
            return Err(ProviderIngestCompletionSignerBindingErrorV1::InvalidSignerQualification);
        }
        Ok(())
    }

    /// Return whether the qualified public key is the exact single-key authority.
    #[must_use]
    pub fn matches_authority(&self, authority: &AccountId) -> bool {
        authority.try_signatory() == Some(&self.public_key)
    }
}

/// Independently configured public binding for one completion signer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderIngestCompletionSignerBindingV1 {
    /// Stable opaque HSM/KMS signer or key handle.
    pub runtime_handle: String,
    /// Exact payload-free signer qualification.
    pub qualification: ProviderIngestCompletionSignerQualificationV1,
}

impl ProviderIngestCompletionSignerBindingV1 {
    /// Pair a stable public handle with the required signer qualification.
    #[must_use]
    pub fn new(
        runtime_handle: impl Into<String>,
        qualification: ProviderIngestCompletionSignerQualificationV1,
    ) -> Self {
        Self {
            runtime_handle: runtime_handle.into(),
            qualification,
        }
    }

    /// Validate the complete configured signer binding.
    ///
    /// # Errors
    ///
    /// Returns an error for a non-production handle or invalid qualification.
    pub fn validate(&self) -> Result<(), ProviderIngestCompletionSignerBindingErrorV1> {
        if !is_production_runtime_handle(&self.runtime_handle) {
            return Err(ProviderIngestCompletionSignerBindingErrorV1::InvalidSignerHandle);
        }
        self.qualification.validate()
    }
}

/// Invalid public binding for a provider-ingest completion signer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum ProviderIngestCompletionSignerBindingErrorV1 {
    /// The signer handle is missing, test-marked, or otherwise not production-safe.
    #[error("provider-ingest completion-signer handle is invalid")]
    InvalidSignerHandle,
    /// The signer qualification is unsupported, zero, malformed, or inconsistent.
    #[error("provider-ingest completion-signer qualification is invalid")]
    InvalidSignerQualification,
}

/// Isolated runtime signer that has no queue or outbox access.
pub trait ProviderIngestCompletionSignerV1: Send + Sync + 'static {
    /// Stable public HSM/KMS signer or key handle.
    fn runtime_handle(&self) -> &str;

    /// Account controlled by this signer.
    fn authority(&self) -> &AccountId;

    /// Current payload-free signer qualification.
    ///
    /// # Errors
    ///
    /// Returns a fixed payload-free signer failure when the signer
    /// qualification cannot be authenticated.
    fn qualification(
        &self,
    ) -> Result<ProviderIngestCompletionSignerQualificationV1, ProviderIngestCompletionSignerErrorV1>;

    /// Exact governed policy identity under which this signer is currently
    /// eligible. Implementations must change this value on key rotation and
    /// reject signing atomically when the policy is revoked or superseded.
    fn signer_policy(&self) -> ProviderIngestCompletionSignerPolicyV1;

    /// Revalidate the live owner/key/policy authority represented by this
    /// signer and return its exact current policy identity.
    ///
    /// This method runs on the async worker thread and therefore must be a
    /// bounded, non-blocking read of a locally maintained eligibility snapshot;
    /// it must never perform HSM/KMS, network, filesystem, or other blocking
    /// I/O. Implementations must update that snapshot on revocation/rotation
    /// and fail closed when it is stale or unavailable. The timed [`Self::sign`]
    /// operation remains responsible for the HSM/KMS-side atomic check.
    fn current_eligibility(
        &self,
    ) -> Result<ProviderIngestCompletionSignerPolicyV1, ProviderIngestCompletionSignerErrorV1>;

    /// Sign exactly the supplied payload without rewriting any field.
    fn sign<'a>(
        &'a self,
        payload: TransactionPayload,
    ) -> ProviderIngestFutureV1<'a, Result<SignedTransaction, ProviderIngestCompletionSignerErrorV1>>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CurrentSignerPolicyErrorV1 {
    Unavailable,
    Ineligible,
    ProtocolViolation,
}

fn exact_current_signer_policy<Signer: ProviderIngestCompletionSignerV1>(
    signer: &Signer,
    expected_owner: &AccountId,
) -> Result<ProviderIngestCompletionSignerPolicyV1, CurrentSignerPolicyErrorV1> {
    if signer.authority() != expected_owner
        || !is_production_runtime_handle(signer.runtime_handle())
    {
        return Err(CurrentSignerPolicyErrorV1::ProtocolViolation);
    }
    let qualification = signer.qualification().map_err(|error| match error {
        ProviderIngestCompletionSignerErrorV1::Unavailable => {
            CurrentSignerPolicyErrorV1::Unavailable
        }
        ProviderIngestCompletionSignerErrorV1::Rejected => CurrentSignerPolicyErrorV1::Ineligible,
    })?;
    let policy = signer.current_eligibility().map_err(|error| match error {
        ProviderIngestCompletionSignerErrorV1::Unavailable => {
            CurrentSignerPolicyErrorV1::Unavailable
        }
        ProviderIngestCompletionSignerErrorV1::Rejected => CurrentSignerPolicyErrorV1::Ineligible,
    })?;
    if qualification.validate().is_err()
        || !qualification.matches_authority(expected_owner)
        || !policy.is_valid()
    {
        return Err(CurrentSignerPolicyErrorV1::ProtocolViolation);
    }
    if signer.signer_policy() != policy || qualification.signer_policy != policy {
        return Err(CurrentSignerPolicyErrorV1::Ineligible);
    }
    Ok(policy)
}

/// Signer resolution failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderIngestCompletionSignerResolverErrorV1 {
    /// Signer discovery is temporarily unavailable.
    Unavailable,
    /// The requested finalized owner is revoked or disallowed.
    Rejected,
}

/// Resolves the signer for one exact finalized completion authorization.
///
/// Implementations must authenticate every field in the resolution context
/// independently of the transaction payload and bind the returned signer to
/// that context for the lifetime of the signing operation.
pub trait ProviderIngestCompletionSignerResolverV1: Send + Sync + 'static {
    /// Isolated signer implementation.
    type Signer: ProviderIngestCompletionSignerV1;

    /// Resolve an eligible signer for one exact finalized authorization.
    fn resolve<'a>(
        &'a self,
        context: ProviderIngestCompletionSignerResolutionContextV1,
    ) -> ProviderIngestFutureV1<
        'a,
        Result<Option<Self::Signer>, ProviderIngestCompletionSignerResolverErrorV1>,
    >;
}

/// Queue preflight failure that occurs before transaction exposure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderIngestIngressPrepareErrorV1 {
    /// Queue preflight is temporarily unavailable.
    Unavailable,
    /// The exact transaction was terminally rejected before exposure.
    Rejected,
}

/// Outcome after an exact transaction may have been exposed to ingress.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderIngestIngressDispositionV1 {
    /// The exact transaction is pending or applied.
    Submitted,
    /// The adapter proves exposure did not reach the queue.
    DefinitelyNotSubmitted,
    /// The exact transaction was terminally rejected.
    Rejected,
    /// Exposure may have happened and requires reconciliation.
    Ambiguous,
}

/// Observation of one exact retained transaction hash.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderIngestTransactionObservationV1 {
    /// The exact transaction committed and execution succeeded.
    ///
    /// This proves only the transaction-level outcome. The finalized
    /// replication-order projection remains the sole semantic completion
    /// authority.
    CommittedSuccess,
    /// The exact transaction committed but execution was rejected.
    CommittedRejected,
    /// The exact transaction remains pending or applied but unfinalized.
    Pending,
    /// The exact transaction is absent from the observed finalized/pipeline view.
    Unknown,
    /// The observation service is temporarily unavailable.
    Unavailable,
}

/// Transaction ingress split into preflight and post-durable exposure phases.
pub trait ProviderIngestTransactionIngressV1: Send + Sync + 'static {
    /// Opaque prepared queue operation that has not exposed transaction bytes.
    type Prepared: Send + 'static;

    /// Validate and prepare queue admission without exposing transaction bytes.
    fn prepare<'a>(
        &'a self,
        transaction: SignedTransaction,
    ) -> ProviderIngestFutureV1<'a, Result<Self::Prepared, ProviderIngestIngressPrepareErrorV1>>;

    /// Expose the exact transaction only after the durable ambiguous transition.
    fn expose<'a>(
        &'a self,
        prepared: Self::Prepared,
        transaction: SignedTransaction,
    ) -> ProviderIngestFutureV1<'a, ProviderIngestIngressDispositionV1>;

    /// Observe one exact retained transaction hash without mutating ingress.
    ///
    /// A committed observation must include the execution result; block/hash
    /// membership alone is not a successful semantic completion signal.
    fn observe<'a>(
        &'a self,
        transaction_hash: [u8; 32],
    ) -> ProviderIngestFutureV1<'a, ProviderIngestTransactionObservationV1>;
}

/// Runtime clock used only for leases, backoff, and timeouts.
pub trait ProviderIngestClockV1: Send + Sync + 'static {
    /// Current runtime time in milliseconds.
    fn now_ms(&self) -> u64;
}

/// Wall-clock implementation for production runtime use.
#[derive(Debug, Clone, Copy, Default)]
pub struct ProviderIngestSystemClockV1;

impl ProviderIngestClockV1 for ProviderIngestSystemClockV1 {
    fn now_ms(&self) -> u64 {
        u64::try_from(
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis(),
        )
        .unwrap_or(u64::MAX)
    }
}

/// Payload-free counters for one bounded runtime tick.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ProviderIngestTickOutcomeV1 {
    /// Finalized rows validated.
    pub rows_scanned: usize,
    /// New durable jobs admitted.
    pub jobs_inserted: usize,
    /// Jobs reconciled to semantic finalized completion.
    pub jobs_finalized: usize,
    /// Jobs cancelled from finalized state.
    pub jobs_cancelled: usize,
    /// Source jobs claimed in this tick.
    pub source_jobs_claimed: usize,
    /// Exact manifests confirmed or stored locally.
    pub manifests_stored: usize,
    /// Completion transactions durably signed.
    pub completions_signed: usize,
    /// Completion transaction exposure attempts.
    pub completion_submissions: usize,
}

/// Supervised provider-ingest runtime.
pub struct ProviderIngestRuntimeV1<Ledger, Fetch, Storage, Builder, Resolver, Ingress, Clock>
where
    Ledger: ProviderIngestFinalizedLedgerV1,
    Fetch: ProviderIngestAuthenticatedSourceFetchV1,
    Storage: ProviderIngestLocalStorageV1<Fetch::Fetched>,
    Builder: ProviderIngestCompletionPayloadBuilderV1,
    Resolver: ProviderIngestCompletionSignerResolverV1,
    Ingress: ProviderIngestTransactionIngressV1,
    Clock: ProviderIngestClockV1,
{
    provider_id: [u8; 32],
    chain_id: ChainId,
    genesis_block_hash: [u8; 32],
    claim_owner: ProviderIngestClaimOwnerV1,
    policy: ProviderIngestRuntimePolicyV1,
    outbox: ProviderIngestOutbox,
    ledger: Arc<Ledger>,
    fetch: Arc<Fetch>,
    storage: Arc<Storage>,
    payload_builder: Arc<Builder>,
    signer_resolver: Arc<Resolver>,
    ingress: Arc<Ingress>,
    clock: Arc<Clock>,
    last_finalized_cursor: Option<ProviderIngestFinalizedCursorV1>,
    scan_cursor: Option<ProviderIngestFinalizedCursorV1>,
    scan_after_order_id: Option<[u8; 32]>,
}

impl<Ledger, Fetch, Storage, Builder, Resolver, Ingress, Clock>
    ProviderIngestRuntimeV1<Ledger, Fetch, Storage, Builder, Resolver, Ingress, Clock>
where
    Ledger: ProviderIngestFinalizedLedgerV1,
    Fetch: ProviderIngestAuthenticatedSourceFetchV1,
    Storage: ProviderIngestLocalStorageV1<Fetch::Fetched>,
    Builder: ProviderIngestCompletionPayloadBuilderV1,
    Resolver: ProviderIngestCompletionSignerResolverV1,
    Ingress: ProviderIngestTransactionIngressV1,
    Clock: ProviderIngestClockV1,
{
    /// Construct a bounded runtime from explicit production dependencies.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        provider_id: [u8; 32],
        chain_id: ChainId,
        genesis_block_hash: [u8; 32],
        claim_owner: ProviderIngestClaimOwnerV1,
        policy: ProviderIngestRuntimePolicyV1,
        outbox: ProviderIngestOutbox,
        ledger: Arc<Ledger>,
        fetch: Arc<Fetch>,
        storage: Arc<Storage>,
        payload_builder: Arc<Builder>,
        signer_resolver: Arc<Resolver>,
        ingress: Arc<Ingress>,
        clock: Arc<Clock>,
    ) -> Result<Self, ProviderIngestRuntimeErrorV1> {
        if provider_id == [0; 32] {
            return Err(ProviderIngestRuntimeErrorV1::InvalidProviderId);
        }
        if chain_id.as_str().is_empty()
            || chain_id.as_str().len()
                > provider_ingest_outbox_defaults::COMPLETION_CHAIN_ID_MAX_BYTES_V1
        {
            return Err(ProviderIngestRuntimeErrorV1::InvalidChainId);
        }
        if genesis_block_hash == [0; 32] {
            return Err(ProviderIngestRuntimeErrorV1::InvalidGenesisBlockHash);
        }
        policy.validate(&outbox)?;
        let last_finalized_cursor = outbox.finalized_cursor_high_water()?;
        Ok(Self {
            provider_id,
            chain_id,
            genesis_block_hash,
            claim_owner,
            policy,
            outbox,
            ledger,
            fetch,
            storage,
            payload_builder,
            signer_resolver,
            ingress,
            clock,
            last_finalized_cursor,
            scan_cursor: None,
            scan_after_order_id: None,
        })
    }

    /// Run until shutdown or a fatal supervised-runtime error.
    pub async fn run(
        mut self,
        mut shutdown: watch::Receiver<bool>,
    ) -> Result<(), ProviderIngestRuntimeErrorV1> {
        if *shutdown.borrow() {
            return Ok(());
        }
        let mut interval =
            tokio::time::interval(Duration::from_millis(self.policy.scan_interval_ms));
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        loop {
            tokio::select! {
                changed = shutdown.changed() => {
                    if changed.is_err() || *shutdown.borrow() {
                        return Ok(());
                    }
                }
                _ = interval.tick() => {
                    let shutdown_requested = std::sync::atomic::AtomicBool::new(false);
                    let tick = self.tick_with_shutdown(&shutdown_requested);
                    tokio::pin!(tick);
                    let mut stop_after_tick = false;
                    loop {
                        tokio::select! {
                            result = &mut tick => {
                                result?;
                                break;
                            }
                            changed = shutdown.changed(), if !stop_after_tick => {
                                if changed.is_err() || *shutdown.borrow() {
                                    shutdown_requested.store(
                                        true,
                                        std::sync::atomic::Ordering::Release,
                                    );
                                    stop_after_tick = true;
                                }
                            }
                        }
                    }
                    if stop_after_tick {
                        return Ok(());
                    }
                }
            }
        }
    }

    /// Execute one bounded finalized scan and delivery pass.
    pub async fn tick(
        &mut self,
    ) -> Result<ProviderIngestTickOutcomeV1, ProviderIngestRuntimeErrorV1> {
        self.tick_inner(None).await
    }

    /// Execute one bounded scan while honoring a cooperative shutdown request.
    ///
    /// The current row always runs to a durable boundary. A request observed
    /// between rows or pages prevents additional work from starting, so callers
    /// may keep polling this same future after selecting their shutdown signal
    /// without detaching a source claim or an in-flight storage mutation.
    pub async fn tick_with_shutdown(
        &mut self,
        shutdown_requested: &std::sync::atomic::AtomicBool,
    ) -> Result<ProviderIngestTickOutcomeV1, ProviderIngestRuntimeErrorV1> {
        self.tick_inner(Some(shutdown_requested)).await
    }

    async fn tick_inner(
        &mut self,
        shutdown_requested: Option<&std::sync::atomic::AtomicBool>,
    ) -> Result<ProviderIngestTickOutcomeV1, ProviderIngestRuntimeErrorV1> {
        let mut outcome = ProviderIngestTickOutcomeV1::default();
        let mut source_budget = self.policy.max_source_jobs_per_tick;
        let mut after = self.scan_after_order_id;
        let mut expected_cursor = self.scan_cursor;
        let mut recovered_interrupted_signing = false;

        for _ in 0..self.policy.max_pages_per_tick {
            if shutdown_requested
                .is_some_and(|requested| requested.load(std::sync::atomic::Ordering::Acquire))
            {
                return Ok(outcome);
            }
            let page = self
                .ledger
                .read_assignment_page(
                    ProviderIngestFinalizedClaimFactoryV1::new(
                        self.chain_id.clone(),
                        self.genesis_block_hash,
                        self.provider_id,
                    ),
                    expected_cursor,
                    after,
                    self.policy.max_page_rows,
                )
                .await
                .map_err(|_| ProviderIngestRuntimeErrorV1::FinalizedLedgerUnavailable)?;
            if after.is_some()
                && expected_cursor.is_some_and(|cursor| cursor != page.finalized_cursor)
            {
                return Err(ProviderIngestRuntimeErrorV1::InvalidFinalizedPage);
            }
            let cursor = expected_cursor.unwrap_or(page.finalized_cursor);
            validate_page(&page, after, cursor, self.policy.max_page_rows)?;
            validate_monotonic_finalized_cursor(self.last_finalized_cursor, cursor)?;
            self.outbox
                .observe_finalized_snapshot(cursor, page.finalized_block_time_ms)?;
            self.last_finalized_cursor = Some(cursor);
            expected_cursor = Some(cursor);
            if !recovered_interrupted_signing {
                self.outbox
                    .recover_expired_completion_signing(self.clock.now_ms(), cursor)?;
                recovered_interrupted_signing = true;
            }

            let finalized_block_time_ms = page.finalized_block_time_ms;
            for row in page.rows {
                if shutdown_requested
                    .is_some_and(|requested| requested.load(std::sync::atomic::Ordering::Acquire))
                {
                    return Ok(outcome);
                }
                outcome.rows_scanned = outcome.rows_scanned.saturating_add(1);
                self.process_row(
                    row,
                    cursor,
                    finalized_block_time_ms,
                    &mut source_budget,
                    &mut outcome,
                )
                .await?;
            }

            after = page.next_after_order_id;
            if after.is_none() {
                self.scan_after_order_id = None;
                self.scan_cursor = None;
                return Ok(outcome);
            }
        }

        self.scan_after_order_id = after;
        self.scan_cursor = expected_cursor;
        Ok(outcome)
    }

    #[allow(clippy::too_many_lines)]
    async fn process_row(
        &self,
        row: ProviderIngestFinalizedAssignmentV1,
        cursor: ProviderIngestFinalizedCursorV1,
        finalized_block_time_ms: u64,
        source_budget: &mut usize,
        outcome: &mut ProviderIngestTickOutcomeV1,
    ) -> Result<(), ProviderIngestRuntimeErrorV1> {
        let ValidatedAssignmentV1 {
            authorization,
            source_provider_ids,
            musubi_archive,
        } = validate_assignment(
            &row,
            cursor,
            self.provider_id,
            &self.chain_id,
            self.genesis_block_hash,
            self.policy,
        )?;
        let job_id = authorization.job_id();
        let provider_id = ProviderId::new(self.provider_id);

        if let Some(completion) = row.order.provider_completion(provider_id) {
            self.outbox.reconcile_finalized_completion(
                authorization,
                ProviderIngestFinalizedCompletionV1 {
                    finalized_cursor: cursor,
                    provider_id: self.provider_id,
                    order_id: *row.order.order_id.as_bytes(),
                    manifest_digest: *row.order.manifest_digest.as_bytes(),
                    completion_epoch: completion.completion_epoch,
                    completed_by: completion.completed_by.clone(),
                    committed_transaction_hash: row.committed_transaction_hash,
                },
            )?;
            outcome.jobs_finalized = outcome.jobs_finalized.saturating_add(1);
            return Ok(());
        }

        let cancellation_reason = match (&row.pin.manifest.status, &row.order.status) {
            (PinStatus::Retired(_), _) => Some(ProviderIngestCancellationReasonV1::ManifestRetired),
            (_, ReplicationOrderStatus::Expired(_)) => {
                Some(ProviderIngestCancellationReasonV1::OrderExpired)
            }
            (_, ReplicationOrderStatus::Completed(_)) => {
                Some(ProviderIngestCancellationReasonV1::OrderCompletedByOther)
            }
            _ => None,
        };
        if let Some(reason) = cancellation_reason {
            self.outbox.reconcile_finalized_cancellation(
                authorization,
                ProviderIngestFinalizedCancellationV1 {
                    finalized_cursor: cursor,
                    provider_id: self.provider_id,
                    order_id: *row.order.order_id.as_bytes(),
                    manifest_digest: *row.order.manifest_digest.as_bytes(),
                    reason,
                },
            )?;
            outcome.jobs_cancelled = outcome.jobs_cancelled.saturating_add(1);
            return Ok(());
        }
        if !matches!(row.pin.manifest.status, PinStatus::Approved(_))
            || !matches!(row.order.status, ReplicationOrderStatus::Pending)
        {
            return Ok(());
        }

        let enqueue = self.outbox.enqueue(authorization.clone())?;
        if matches!(
            enqueue,
            crate::provider_ingest_outbox::ProviderIngestEnqueueResultV1::Inserted { .. }
        ) {
            outcome.jobs_inserted = outcome.jobs_inserted.saturating_add(1);
        }
        // The job identity deliberately excludes the finalized cursor so an unchanged
        // assignment can resume after the head advances. Every source/storage transition must
        // therefore use the exact authorization retained by the outbox, while the freshly
        // sealed Musubi claim proves that binding still exists at the newer scan cursor.
        let authorization = self.outbox.authorization(job_id)?;
        let status = self.outbox.status(job_id)?;
        match status.state {
            ProviderIngestDeliveryStateV1::PendingSource { .. }
            | ProviderIngestDeliveryStateV1::RetryScheduled { .. }
            | ProviderIngestDeliveryStateV1::SourceClaimed { .. }
                if *source_budget != 0 =>
            {
                if self
                    .process_source(
                        authorization.clone(),
                        source_provider_ids,
                        musubi_archive.clone(),
                        cursor,
                        outcome,
                    )
                    .await?
                {
                    *source_budget -= 1;
                    if let Ok(status) = self.outbox.status(job_id)
                        && let ProviderIngestDeliveryStateV1::LocalStored {
                            musubi_bundle,
                            completion,
                            ..
                        } = status.state
                    {
                        if !persisted_receipt_matches(
                            &authorization,
                            musubi_archive.as_ref(),
                            musubi_bundle.as_deref(),
                        ) {
                            return Err(ProviderIngestRuntimeErrorV1::StorageProtocolViolation);
                        }
                        self.process_completion(
                            &row,
                            status.job_id,
                            completion,
                            cursor,
                            finalized_block_time_ms,
                            outcome,
                        )
                        .await?;
                    }
                }
            }
            ProviderIngestDeliveryStateV1::LocalStored {
                musubi_bundle,
                completion,
                ..
            } => {
                if !persisted_receipt_matches(
                    &authorization,
                    musubi_archive.as_ref(),
                    musubi_bundle.as_deref(),
                ) {
                    return Err(ProviderIngestRuntimeErrorV1::StorageProtocolViolation);
                }
                self.process_completion(
                    &row,
                    status.job_id,
                    completion,
                    cursor,
                    finalized_block_time_ms,
                    outcome,
                )
                .await?;
            }
            ProviderIngestDeliveryStateV1::PendingSource { .. }
            | ProviderIngestDeliveryStateV1::RetryScheduled { .. }
            | ProviderIngestDeliveryStateV1::SourceClaimed { .. }
            | ProviderIngestDeliveryStateV1::FinalizedCompleted { .. }
            | ProviderIngestDeliveryStateV1::Cancelled { .. }
            | ProviderIngestDeliveryStateV1::DeadLetter { .. } => {}
        }
        Ok(())
    }

    #[allow(clippy::too_many_lines)]
    async fn process_source(
        &self,
        authorization: FinalizedProviderIngestAuthorizationV1,
        source_provider_ids: Vec<[u8; 32]>,
        musubi_archive: Option<ProviderIngestFinalizedMusubiArchiveClaimV1>,
        cursor: ProviderIngestFinalizedCursorV1,
        outcome: &mut ProviderIngestTickOutcomeV1,
    ) -> Result<bool, ProviderIngestRuntimeErrorV1> {
        // TODO: After completion is finalized, seal a separate completed-row
        // claim, rerun the verifier, and durably prepare an approval-only
        // provider attestation. This pre-completion claim and receipt never
        // authorize an attestation by themselves.
        let claim = match self.outbox.claim_source(
            authorization.job_id(),
            self.claim_owner,
            self.clock.now_ms(),
            cursor,
        ) {
            Ok(claim) => claim,
            Err(
                ProviderIngestOutboxError::RetryNotDue
                | ProviderIngestOutboxError::LeaseAlreadyHeld
                | ProviderIngestOutboxError::InvalidTransition
                | ProviderIngestOutboxError::RetryExhausted,
            ) => return Ok(false),
            Err(error) => return Err(error.into()),
        };
        outcome.source_jobs_claimed = outcome.source_jobs_claimed.saturating_add(1);

        let verify = self
            .storage
            .verify_existing(authorization.clone(), musubi_archive.clone());
        let (claim, existing) = self.await_with_lease(claim, cursor, verify).await?;
        match existing {
            LeaseOperationOutcomeV1::Completed(Ok(Some(stored))) => {
                if !local_stored_matches(&stored, &authorization, musubi_archive.as_ref()) {
                    self.outbox.dead_letter_source(
                        &claim,
                        self.clock.now_ms(),
                        cursor,
                        ProviderIngestDeadLetterReasonV1::StorageRejected,
                        ProviderIngestFailureClassV1::BindingMismatch,
                    )?;
                    return Err(ProviderIngestRuntimeErrorV1::StorageProtocolViolation);
                }
                if let Err(error) = self.outbox.mark_local_stored_verified(
                    &claim,
                    self.clock.now_ms(),
                    stored.manifest_id().to_owned(),
                    stored.musubi_bundle().cloned(),
                ) {
                    if error == ProviderIngestOutboxError::InvalidManifestId {
                        self.outbox.dead_letter_source(
                            &claim,
                            self.clock.now_ms(),
                            cursor,
                            ProviderIngestDeadLetterReasonV1::StorageRejected,
                            ProviderIngestFailureClassV1::StorageRejected,
                        )?;
                        return Err(ProviderIngestRuntimeErrorV1::StorageProtocolViolation);
                    }
                    return Err(error.into());
                }
                outcome.manifests_stored = outcome.manifests_stored.saturating_add(1);
                return Ok(true);
            }
            LeaseOperationOutcomeV1::Completed(Ok(None)) => {}
            LeaseOperationOutcomeV1::Completed(Err(error)) => {
                self.handle_storage_failure(claim, cursor, error)?;
                return Ok(true);
            }
            LeaseOperationOutcomeV1::TimedOut => {
                self.outbox.schedule_source_retry(
                    &claim,
                    self.clock.now_ms(),
                    cursor,
                    ProviderIngestFailureClassV1::StorageRejected,
                )?;
                return Ok(true);
            }
        }

        if source_provider_ids.is_empty() {
            self.outbox.schedule_source_retry(
                &claim,
                self.clock.now_ms(),
                cursor,
                ProviderIngestFailureClassV1::SourceUnavailable,
            )?;
            return Ok(true);
        }
        let request = match ProviderIngestSourceRequestV1::new(
            authorization.clone(),
            source_provider_ids,
            musubi_archive
                .as_ref()
                .map(ProviderIngestMusubiArchiveFetchBindingV1::from_finalized_claim),
        ) {
            Ok(request) => request,
            Err(_) => {
                self.outbox.schedule_source_retry(
                    &claim,
                    self.clock.now_ms(),
                    cursor,
                    ProviderIngestFailureClassV1::SourceRejected,
                )?;
                return Err(ProviderIngestRuntimeErrorV1::SourceProtocolViolation);
            }
        };
        let fetch = self.fetch.fetch(request);
        let (claim, fetched) = self.await_with_lease(claim, cursor, fetch).await?;
        let fetched = match fetched {
            LeaseOperationOutcomeV1::Completed(Ok(fetched)) => fetched,
            LeaseOperationOutcomeV1::Completed(Err(
                ProviderIngestSourceFetchErrorV1::Unavailable,
            ))
            | LeaseOperationOutcomeV1::TimedOut => {
                self.outbox.schedule_source_retry(
                    &claim,
                    self.clock.now_ms(),
                    cursor,
                    ProviderIngestFailureClassV1::SourceUnavailable,
                )?;
                return Ok(true);
            }
            LeaseOperationOutcomeV1::Completed(Err(
                ProviderIngestSourceFetchErrorV1::ContentRejected,
            )) => {
                self.outbox.schedule_source_retry(
                    &claim,
                    self.clock.now_ms(),
                    cursor,
                    ProviderIngestFailureClassV1::SourceRejected,
                )?;
                return Ok(true);
            }
            LeaseOperationOutcomeV1::Completed(Err(ProviderIngestSourceFetchErrorV1::Rejected)) => {
                self.outbox.schedule_source_retry(
                    &claim,
                    self.clock.now_ms(),
                    cursor,
                    ProviderIngestFailureClassV1::SourceRejected,
                )?;
                return Err(ProviderIngestRuntimeErrorV1::SourceProtocolViolation);
            }
        };

        let store = self
            .storage
            .store(authorization.clone(), musubi_archive.clone(), fetched);
        let (claim, stored) = self
            .await_mutating_storage_with_lease(claim, cursor, store)
            .await?;
        let stored = match stored {
            MutatingStorageOutcomeV1::Completed(output)
            | MutatingStorageOutcomeV1::CompletedAfterSoftTimeout(output) => output,
        };
        match stored {
            Ok(stored) => {
                if !local_stored_matches(&stored, &authorization, musubi_archive.as_ref()) {
                    self.outbox.dead_letter_source(
                        &claim,
                        self.clock.now_ms(),
                        cursor,
                        ProviderIngestDeadLetterReasonV1::StorageRejected,
                        ProviderIngestFailureClassV1::BindingMismatch,
                    )?;
                    return Err(ProviderIngestRuntimeErrorV1::StorageProtocolViolation);
                }
                if let Err(error) = self.outbox.mark_local_stored_verified(
                    &claim,
                    self.clock.now_ms(),
                    stored.manifest_id().to_owned(),
                    stored.musubi_bundle().cloned(),
                ) {
                    if error == ProviderIngestOutboxError::InvalidManifestId {
                        self.outbox.dead_letter_source(
                            &claim,
                            self.clock.now_ms(),
                            cursor,
                            ProviderIngestDeadLetterReasonV1::StorageRejected,
                            ProviderIngestFailureClassV1::StorageRejected,
                        )?;
                        return Err(ProviderIngestRuntimeErrorV1::StorageProtocolViolation);
                    }
                    return Err(error.into());
                }
                outcome.manifests_stored = outcome.manifests_stored.saturating_add(1);
            }
            Err(error) => {
                self.handle_storage_failure(claim, cursor, error)?;
            }
        }
        Ok(true)
    }

    fn handle_storage_failure(
        &self,
        claim: ProviderIngestSourceClaimV1,
        cursor: ProviderIngestFinalizedCursorV1,
        error: ProviderIngestLocalStorageErrorV1,
    ) -> Result<(), ProviderIngestRuntimeErrorV1> {
        match error {
            ProviderIngestLocalStorageErrorV1::Retryable => {
                self.outbox.schedule_source_retry(
                    &claim,
                    self.clock.now_ms(),
                    cursor,
                    ProviderIngestFailureClassV1::StorageRejected,
                )?;
            }
            ProviderIngestLocalStorageErrorV1::Permanent => {
                self.outbox.dead_letter_source(
                    &claim,
                    self.clock.now_ms(),
                    cursor,
                    ProviderIngestDeadLetterReasonV1::StorageRejected,
                    ProviderIngestFailureClassV1::StorageRejected,
                )?;
            }
        }
        Ok(())
    }

    async fn await_with_lease<T, Fut>(
        &self,
        mut claim: ProviderIngestSourceClaimV1,
        cursor: ProviderIngestFinalizedCursorV1,
        future: Fut,
    ) -> Result<
        (ProviderIngestSourceClaimV1, LeaseOperationOutcomeV1<T>),
        ProviderIngestRuntimeErrorV1,
    >
    where
        Fut: Future<Output = T> + Send,
    {
        let future = future;
        tokio::pin!(future);
        let timeout = tokio::time::sleep(Duration::from_millis(
            self.policy.source_operation_timeout_ms,
        ));
        tokio::pin!(timeout);
        let renew_period = Duration::from_millis(self.policy.source_lease_renew_interval_ms);
        let mut renewal =
            tokio::time::interval_at(tokio::time::Instant::now() + renew_period, renew_period);
        renewal.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        loop {
            tokio::select! {
                biased;
                _ = renewal.tick() => {
                    claim = self.outbox.renew_source_claim(
                        &claim,
                        self.clock.now_ms(),
                        cursor,
                    )?;
                }
                output = &mut future => {
                    return Ok((claim, LeaseOperationOutcomeV1::Completed(output)));
                }
                _ = &mut timeout => {
                    return Ok((claim, LeaseOperationOutcomeV1::TimedOut));
                }
            }
        }
    }

    /// Await an in-flight atomic storage mutation without ever detaching it.
    ///
    /// The configured operation timeout is a soft diagnostic boundary for
    /// mutating storage. Once storage may be writing, the runtime keeps the
    /// durable claim renewed and waits for the exact operation to finish before
    /// it can persist success or schedule a retry. This prevents a timed-out
    /// blocking writer from racing a replacement attempt.
    async fn await_mutating_storage_with_lease<T, Fut>(
        &self,
        mut claim: ProviderIngestSourceClaimV1,
        cursor: ProviderIngestFinalizedCursorV1,
        future: Fut,
    ) -> Result<
        (ProviderIngestSourceClaimV1, MutatingStorageOutcomeV1<T>),
        ProviderIngestRuntimeErrorV1,
    >
    where
        Fut: Future<Output = T> + Send,
    {
        let future = future;
        tokio::pin!(future);
        let soft_timeout = tokio::time::sleep(Duration::from_millis(
            self.policy.source_operation_timeout_ms,
        ));
        tokio::pin!(soft_timeout);
        let renew_period = Duration::from_millis(self.policy.source_lease_renew_interval_ms);
        let mut renewal =
            tokio::time::interval_at(tokio::time::Instant::now() + renew_period, renew_period);
        renewal.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        let mut exceeded_soft_timeout = false;
        loop {
            tokio::select! {
                biased;
                _ = renewal.tick() => {
                    claim = self.outbox.renew_source_claim(
                        &claim,
                        self.clock.now_ms(),
                        cursor,
                    )?;
                }
                output = &mut future => {
                    let outcome = if exceeded_soft_timeout {
                        MutatingStorageOutcomeV1::CompletedAfterSoftTimeout(output)
                    } else {
                        MutatingStorageOutcomeV1::Completed(output)
                    };
                    return Ok((claim, outcome));
                }
                _ = &mut soft_timeout, if !exceeded_soft_timeout => {
                    exceeded_soft_timeout = true;
                }
            }
        }
    }

    #[allow(clippy::too_many_lines)]
    async fn process_completion(
        &self,
        row: &ProviderIngestFinalizedAssignmentV1,
        job_id: [u8; 32],
        completion: ProviderIngestCompletionStateV1,
        cursor: ProviderIngestFinalizedCursorV1,
        finalized_block_time_ms: u64,
        outcome: &mut ProviderIngestTickOutcomeV1,
    ) -> Result<(), ProviderIngestRuntimeErrorV1> {
        let mut completion = completion;
        let mut exposed_absent_transaction = None;
        let completion_authority = row.completion_authority.as_ref().filter(|binding| {
            binding.is_valid() && row.provider_owner.as_ref() == Some(&binding.provider_owner)
        });

        // Bytes that may already have crossed the queue boundary are always
        // reconciled by exact hash before any signer/HSM dependency is queried.
        match &completion {
            ProviderIngestCompletionStateV1::Ambiguous {
                baseline_finalized_cursor,
                transaction_hash,
                ..
            } => {
                self.reconcile_transaction(
                    job_id,
                    *transaction_hash,
                    *baseline_finalized_cursor,
                    cursor,
                    true,
                )
                .await?;
                return Ok(());
            }
            ProviderIngestCompletionStateV1::Submitted {
                baseline_finalized_cursor,
                transaction_hash,
                ..
            } => {
                self.reconcile_transaction(
                    job_id,
                    *transaction_hash,
                    *baseline_finalized_cursor,
                    cursor,
                    false,
                )
                .await?;
                return Ok(());
            }
            ProviderIngestCompletionStateV1::Ready { .. }
            | ProviderIngestCompletionStateV1::Signing { .. }
            | ProviderIngestCompletionStateV1::Signed { .. } => {}
        }

        if let ProviderIngestCompletionStateV1::Signed {
            baseline_finalized_cursor,
            transaction_hash,
            ever_exposed: true,
            ..
        } = &completion
        {
            let observation = tokio::time::timeout(
                Duration::from_millis(self.policy.ingress_timeout_ms),
                self.ingress.observe(*transaction_hash),
            )
            .await
            .unwrap_or(ProviderIngestTransactionObservationV1::Unavailable);
            match observation {
                ProviderIngestTransactionObservationV1::CommittedSuccess
                | ProviderIngestTransactionObservationV1::Pending => {
                    self.outbox
                        .mark_exposed_completion_observed(job_id, *transaction_hash)?;
                    return Ok(());
                }
                ProviderIngestTransactionObservationV1::CommittedRejected => {
                    self.outbox.mark_completion_transaction_rejected(
                        job_id,
                        *transaction_hash,
                        self.clock.now_ms(),
                        cursor,
                    )?;
                    return Ok(());
                }
                ProviderIngestTransactionObservationV1::Unknown => {
                    if cursor.height > baseline_finalized_cursor.height {
                        exposed_absent_transaction = Some(*transaction_hash);
                    }
                }
                ProviderIngestTransactionObservationV1::Unavailable => return Ok(()),
            }
        }

        let mut submission_authority = None;
        let mut checked_signer_policy = None;
        if matches!(
            &completion,
            ProviderIngestCompletionStateV1::Signing { .. }
                | ProviderIngestCompletionStateV1::Signed { .. }
        ) {
            if completion_authority.is_none() {
                self.outbox.invalidate_stale_completion_authority(
                    job_id,
                    row.provider_owner.as_ref(),
                    ProviderIngestSignerPolicyObservationV1::Missing,
                    self.clock.now_ms(),
                    cursor,
                )?;
                return Ok(());
            }
            self.outbox.invalidate_stale_completion_authority(
                job_id,
                row.provider_owner.as_ref(),
                ProviderIngestSignerPolicyObservationV1::NotChecked,
                self.clock.now_ms(),
                cursor,
            )?;
            completion = match self.outbox.status(job_id)?.state {
                ProviderIngestDeliveryStateV1::LocalStored { completion, .. } => completion,
                ProviderIngestDeliveryStateV1::PendingSource { .. }
                | ProviderIngestDeliveryStateV1::SourceClaimed { .. }
                | ProviderIngestDeliveryStateV1::RetryScheduled { .. }
                | ProviderIngestDeliveryStateV1::FinalizedCompleted { .. }
                | ProviderIngestDeliveryStateV1::Cancelled { .. }
                | ProviderIngestDeliveryStateV1::DeadLetter { .. } => return Ok(()),
            };
        }
        if matches!(
            &completion,
            ProviderIngestCompletionStateV1::Signing { .. }
                | ProviderIngestCompletionStateV1::Signed { .. }
        ) {
            if let Some(provider_owner) = row.provider_owner.clone() {
                let Some(expected_policy) =
                    completion_authority.map(|binding| binding.signer_policy)
                else {
                    return Ok(());
                };
                let signer_policy_observation = match tokio::time::timeout(
                    Duration::from_millis(self.policy.signer_timeout_ms),
                    self.signer_resolver.resolve(
                        ProviderIngestCompletionSignerResolutionContextV1::new(
                            provider_owner.clone(),
                            expected_policy,
                            row.order.assignment_revision,
                            cursor,
                        ),
                    ),
                )
                .await
                {
                    Ok(Ok(Some(signer))) => {
                        match exact_current_signer_policy(&signer, &provider_owner) {
                            Ok(signer_policy) => {
                                if signer_policy == expected_policy {
                                    submission_authority =
                                        Some((provider_owner.clone(), signer_policy));
                                }
                                ProviderIngestSignerPolicyObservationV1::Active(signer_policy)
                            }
                            Err(CurrentSignerPolicyErrorV1::Ineligible) => {
                                ProviderIngestSignerPolicyObservationV1::Missing
                            }
                            Err(CurrentSignerPolicyErrorV1::Unavailable) => return Ok(()),
                            Err(CurrentSignerPolicyErrorV1::ProtocolViolation) => {
                                return Err(ProviderIngestRuntimeErrorV1::SignerProtocolViolation);
                            }
                        }
                    }
                    Ok(Ok(None))
                    | Ok(Err(ProviderIngestCompletionSignerResolverErrorV1::Rejected)) => {
                        ProviderIngestSignerPolicyObservationV1::Missing
                    }
                    Ok(Err(ProviderIngestCompletionSignerResolverErrorV1::Unavailable))
                    | Err(_) => {
                        return Ok(());
                    }
                };
                checked_signer_policy = Some(signer_policy_observation);
                self.outbox.invalidate_stale_completion_authority(
                    job_id,
                    Some(&provider_owner),
                    signer_policy_observation,
                    self.clock.now_ms(),
                    cursor,
                )?;
                completion = match self.outbox.status(job_id)?.state {
                    ProviderIngestDeliveryStateV1::LocalStored { completion, .. } => completion,
                    ProviderIngestDeliveryStateV1::PendingSource { .. }
                    | ProviderIngestDeliveryStateV1::SourceClaimed { .. }
                    | ProviderIngestDeliveryStateV1::RetryScheduled { .. }
                    | ProviderIngestDeliveryStateV1::FinalizedCompleted { .. }
                    | ProviderIngestDeliveryStateV1::Cancelled { .. }
                    | ProviderIngestDeliveryStateV1::DeadLetter { .. } => return Ok(()),
                };
            } else {
                checked_signer_policy = Some(ProviderIngestSignerPolicyObservationV1::NotChecked);
            }
        }
        if let Some(transaction_hash) = exposed_absent_transaction
            && self
                .outbox
                .expire_absent_exposed_completion(ProviderIngestExposedCompletionExpiryV1 {
                    job_id,
                    expected_transaction_hash: transaction_hash,
                    current_provider_owner: row.provider_owner.as_ref(),
                    current_signer_policy: checked_signer_policy
                        .unwrap_or(ProviderIngestSignerPolicyObservationV1::NotChecked),
                    runtime_now_ms: self.clock.now_ms(),
                    finalized_block_time_ms,
                    observed_finalized_cursor: cursor,
                })?
                .is_some()
        {
            return Ok(());
        }
        match completion {
            ProviderIngestCompletionStateV1::Ready {
                next_attempt_at_ms, ..
            } => {
                let (Some(completion_authority), Some(completion_epoch)) =
                    (completion_authority.cloned(), row.completion_epoch)
                else {
                    return Ok(());
                };
                let provider_owner = completion_authority.provider_owner.clone();
                if completion_epoch < row.order.issued_epoch
                    || completion_epoch > row.order.deadline_epoch
                {
                    return Ok(());
                }
                let owner_changed = self.outbox.reconcile_ready_completion_owner(
                    job_id,
                    &provider_owner,
                    cursor,
                )?;
                if !owner_changed && self.clock.now_ms() < next_attempt_at_ms {
                    return Ok(());
                }
                let signer = match tokio::time::timeout(
                    Duration::from_millis(self.policy.signer_timeout_ms),
                    self.signer_resolver.resolve(
                        ProviderIngestCompletionSignerResolutionContextV1::new(
                            provider_owner.clone(),
                            completion_authority.signer_policy,
                            row.order.assignment_revision,
                            cursor,
                        ),
                    ),
                )
                .await
                {
                    Ok(Ok(Some(signer))) => signer,
                    Ok(Ok(None))
                    | Ok(Err(ProviderIngestCompletionSignerResolverErrorV1::Rejected)) => {
                        self.outbox.record_completion_signer_policy_missing(
                            job_id,
                            &provider_owner,
                            self.clock.now_ms(),
                            cursor,
                        )?;
                        return Ok(());
                    }
                    Ok(Err(ProviderIngestCompletionSignerResolverErrorV1::Unavailable))
                    | Err(_) => {
                        self.outbox.record_completion_signer_resolution_failure(
                            job_id,
                            self.clock.now_ms(),
                            cursor,
                        )?;
                        return Ok(());
                    }
                };
                let signer_policy = match exact_current_signer_policy(&signer, &provider_owner) {
                    Ok(policy) => policy,
                    Err(CurrentSignerPolicyErrorV1::Unavailable) => {
                        self.outbox.record_completion_signer_resolution_failure(
                            job_id,
                            self.clock.now_ms(),
                            cursor,
                        )?;
                        return Ok(());
                    }
                    Err(CurrentSignerPolicyErrorV1::Ineligible) => {
                        self.outbox.record_completion_signer_policy_missing(
                            job_id,
                            &provider_owner,
                            self.clock.now_ms(),
                            cursor,
                        )?;
                        return Ok(());
                    }
                    Err(CurrentSignerPolicyErrorV1::ProtocolViolation) => {
                        self.outbox.record_completion_signer_resolution_failure(
                            job_id,
                            self.clock.now_ms(),
                            cursor,
                        )?;
                        return Err(ProviderIngestRuntimeErrorV1::SignerProtocolViolation);
                    }
                };
                if signer_policy != completion_authority.signer_policy {
                    self.outbox.record_completion_signer_policy_missing(
                        job_id,
                        &provider_owner,
                        self.clock.now_ms(),
                        cursor,
                    )?;
                    return Ok(());
                }
                if let Err(error) = self.outbox.validate_ready_completion_signer_policy(
                    job_id,
                    &provider_owner,
                    signer_policy,
                    cursor,
                ) {
                    self.outbox.record_completion_signer_resolution_failure(
                        job_id,
                        self.clock.now_ms(),
                        cursor,
                    )?;
                    return Err(error.into());
                }
                let status = self.outbox.status(job_id)?;
                let request = ProviderIngestCompletionPayloadRequestV1 {
                    authorization: authorization_from_status_and_row(&status, row, cursor)?,
                    provider_owner: provider_owner.clone(),
                    expected_authority: completion_authority,
                    expected_assignment_revision: row.order.assignment_revision,
                    chain_id: self.chain_id.clone(),
                    completion_epoch,
                    finalized_cursor: cursor,
                };
                let payload = match tokio::time::timeout(
                    Duration::from_millis(self.policy.signer_timeout_ms),
                    self.payload_builder.build_payload(request),
                )
                .await
                {
                    Ok(Ok(payload)) => payload,
                    Ok(Err(
                        ProviderIngestCompletionPayloadErrorV1::Unavailable
                        | ProviderIngestCompletionPayloadErrorV1::Rejected,
                    ))
                    | Err(_) => {
                        self.outbox.record_completion_preparation_failure(
                            job_id,
                            self.clock.now_ms(),
                            cursor,
                        )?;
                        return Ok(());
                    }
                };
                let Some(network_id) = payload.network_id().copied() else {
                    self.outbox.record_completion_preparation_failure(
                        job_id,
                        self.clock.now_ms(),
                        cursor,
                    )?;
                    return Ok(());
                };
                let context = ProviderIngestCompletionSigningContextV1 {
                    baseline_finalized_cursor: cursor,
                    chain_id: self.chain_id.clone(),
                    network_id,
                    provider_owner: provider_owner.clone(),
                    signer_policy,
                    assignment_revision: row.order.assignment_revision,
                    completion_epoch,
                    expected_payload: payload,
                };
                let claim =
                    match self
                        .outbox
                        .claim_completion_signing(job_id, context, self.clock.now_ms())
                    {
                        Ok(claim) => claim,
                        Err(ProviderIngestOutboxError::RetryNotDue) => return Ok(()),
                        Err(error) => return Err(error.into()),
                    };
                match exact_current_signer_policy(&signer, &provider_owner) {
                    Ok(policy) if policy == claim.context().signer_policy => {}
                    Err(CurrentSignerPolicyErrorV1::ProtocolViolation) => {
                        self.outbox.release_completion_signing(
                            &claim,
                            self.clock.now_ms(),
                            cursor,
                        )?;
                        return Err(ProviderIngestRuntimeErrorV1::SignerProtocolViolation);
                    }
                    Ok(_)
                    | Err(
                        CurrentSignerPolicyErrorV1::Unavailable
                        | CurrentSignerPolicyErrorV1::Ineligible,
                    ) => {
                        self.outbox.release_completion_signing(
                            &claim,
                            self.clock.now_ms(),
                            cursor,
                        )?;
                        return Ok(());
                    }
                }
                let transaction = match tokio::time::timeout(
                    Duration::from_millis(self.policy.signer_timeout_ms),
                    signer.sign(claim.context().expected_payload.clone()),
                )
                .await
                {
                    Ok(Ok(transaction)) => transaction,
                    Ok(Err(ProviderIngestCompletionSignerErrorV1::Unavailable)) | Err(_) => {
                        self.outbox.release_completion_signing(
                            &claim,
                            self.clock.now_ms(),
                            cursor,
                        )?;
                        return Ok(());
                    }
                    Ok(Err(ProviderIngestCompletionSignerErrorV1::Rejected)) => {
                        self.outbox.release_completion_signing(
                            &claim,
                            self.clock.now_ms(),
                            cursor,
                        )?;
                        return Err(ProviderIngestRuntimeErrorV1::SignerProtocolViolation);
                    }
                };
                match exact_current_signer_policy(&signer, &provider_owner) {
                    Ok(policy) if policy == claim.context().signer_policy => {}
                    Err(CurrentSignerPolicyErrorV1::ProtocolViolation) => {
                        self.outbox.release_completion_signing(
                            &claim,
                            self.clock.now_ms(),
                            cursor,
                        )?;
                        return Err(ProviderIngestRuntimeErrorV1::SignerProtocolViolation);
                    }
                    Ok(_)
                    | Err(
                        CurrentSignerPolicyErrorV1::Unavailable
                        | CurrentSignerPolicyErrorV1::Ineligible,
                    ) => {
                        self.outbox.release_completion_signing(
                            &claim,
                            self.clock.now_ms(),
                            cursor,
                        )?;
                        return Ok(());
                    }
                }
                match self
                    .outbox
                    .store_completion_transaction(&claim, transaction)
                {
                    Ok(_) => {}
                    Err(
                        ProviderIngestOutboxError::InvalidSignedTransaction
                        | ProviderIngestOutboxError::InvalidSigningClaim,
                    ) => {
                        self.outbox.release_completion_signing(
                            &claim,
                            self.clock.now_ms(),
                            cursor,
                        )?;
                        return Err(ProviderIngestRuntimeErrorV1::SignerProtocolViolation);
                    }
                    Err(error) => return Err(error.into()),
                }
                outcome.completions_signed = outcome.completions_signed.saturating_add(1);
                let status = self.outbox.status(job_id)?;
                if let ProviderIngestDeliveryStateV1::LocalStored {
                    completion: ProviderIngestCompletionStateV1::Signed { .. },
                    ..
                } = status.state
                {
                    self.submit_signed(
                        job_id,
                        &provider_owner,
                        signer_policy,
                        row.order.assignment_revision,
                        cursor,
                        outcome,
                    )
                    .await?;
                }
            }
            ProviderIngestCompletionStateV1::Signing { .. } => {}
            ProviderIngestCompletionStateV1::Signed {
                next_attempt_at_ms, ..
            } => {
                if self.clock.now_ms() >= next_attempt_at_ms {
                    let Some((provider_owner, signer_policy)) = submission_authority else {
                        return Ok(());
                    };
                    self.submit_signed(
                        job_id,
                        &provider_owner,
                        signer_policy,
                        row.order.assignment_revision,
                        cursor,
                        outcome,
                    )
                    .await?;
                }
            }
            ProviderIngestCompletionStateV1::Ambiguous {
                baseline_finalized_cursor,
                transaction_hash,
                ..
            } => {
                self.reconcile_transaction(
                    job_id,
                    transaction_hash,
                    baseline_finalized_cursor,
                    cursor,
                    true,
                )
                .await?;
            }
            ProviderIngestCompletionStateV1::Submitted {
                baseline_finalized_cursor,
                transaction_hash,
                ..
            } => {
                self.reconcile_transaction(
                    job_id,
                    transaction_hash,
                    baseline_finalized_cursor,
                    cursor,
                    false,
                )
                .await?;
            }
        }
        Ok(())
    }

    #[allow(clippy::too_many_lines)]
    async fn submit_signed(
        &self,
        job_id: [u8; 32],
        provider_owner: &AccountId,
        expected_signer_policy: ProviderIngestCompletionSignerPolicyV1,
        expected_assignment_revision: u64,
        cursor: ProviderIngestFinalizedCursorV1,
        outcome: &mut ProviderIngestTickOutcomeV1,
    ) -> Result<(), ProviderIngestRuntimeErrorV1> {
        let exact = match self.outbox.completion_transaction_for_authorized_preflight(
            job_id,
            provider_owner,
            expected_signer_policy,
            cursor,
            self.clock.now_ms(),
        ) {
            Ok(exact) => exact,
            Err(
                ProviderIngestOutboxError::RetryNotDue
                | ProviderIngestOutboxError::InvalidTransition
                | ProviderIngestOutboxError::InvalidSigningContext
                | ProviderIngestOutboxError::SignerPolicyRollback
                | ProviderIngestOutboxError::StaleFinalizedCursor,
            ) => return Ok(()),
            Err(error) => return Err(error.into()),
        };
        let prepared = match tokio::time::timeout(
            Duration::from_millis(self.policy.ingress_timeout_ms),
            self.ingress.prepare(exact.signed_transaction.clone()),
        )
        .await
        {
            Ok(Ok(prepared)) => prepared,
            Ok(Err(ProviderIngestIngressPrepareErrorV1::Unavailable)) | Err(_) => {
                self.outbox.mark_completion_preflight_unavailable(
                    job_id,
                    exact.transaction_hash,
                    self.clock.now_ms(),
                    cursor,
                )?;
                return Ok(());
            }
            Ok(Err(ProviderIngestIngressPrepareErrorV1::Rejected)) => {
                self.outbox.mark_completion_preflight_rejected(
                    job_id,
                    exact.transaction_hash,
                    self.clock.now_ms(),
                    cursor,
                )?;
                return Ok(());
            }
        };
        let signer = match tokio::time::timeout(
            Duration::from_millis(self.policy.signer_timeout_ms),
            self.signer_resolver
                .resolve(ProviderIngestCompletionSignerResolutionContextV1::new(
                    provider_owner.clone(),
                    expected_signer_policy,
                    expected_assignment_revision,
                    cursor,
                )),
        )
        .await
        {
            Ok(Ok(Some(signer))) => signer,
            Ok(Ok(None)) | Ok(Err(ProviderIngestCompletionSignerResolverErrorV1::Rejected)) => {
                self.outbox.invalidate_stale_completion_authority(
                    job_id,
                    Some(provider_owner),
                    ProviderIngestSignerPolicyObservationV1::Missing,
                    self.clock.now_ms(),
                    cursor,
                )?;
                return Ok(());
            }
            Ok(Err(ProviderIngestCompletionSignerResolverErrorV1::Unavailable)) | Err(_) => {
                return Ok(());
            }
        };
        let signer_policy = match exact_current_signer_policy(&signer, provider_owner) {
            Ok(policy) => policy,
            Err(CurrentSignerPolicyErrorV1::Ineligible) => {
                self.outbox.invalidate_stale_completion_authority(
                    job_id,
                    Some(provider_owner),
                    ProviderIngestSignerPolicyObservationV1::Missing,
                    self.clock.now_ms(),
                    cursor,
                )?;
                return Ok(());
            }
            Err(CurrentSignerPolicyErrorV1::Unavailable) => return Ok(()),
            Err(CurrentSignerPolicyErrorV1::ProtocolViolation) => {
                return Err(ProviderIngestRuntimeErrorV1::SignerProtocolViolation);
            }
        };
        if signer_policy != expected_signer_policy {
            self.outbox.invalidate_stale_completion_authority(
                job_id,
                Some(provider_owner),
                ProviderIngestSignerPolicyObservationV1::Active(signer_policy),
                self.clock.now_ms(),
                cursor,
            )?;
            return Ok(());
        }
        self.outbox.invalidate_stale_completion_authority(
            job_id,
            Some(provider_owner),
            ProviderIngestSignerPolicyObservationV1::Active(signer_policy),
            self.clock.now_ms(),
            cursor,
        )?;
        match exact_current_signer_policy(&signer, provider_owner) {
            Ok(current_policy) if current_policy == signer_policy => {}
            Ok(_)
            | Err(
                CurrentSignerPolicyErrorV1::Ineligible | CurrentSignerPolicyErrorV1::Unavailable,
            ) => return Ok(()),
            Err(CurrentSignerPolicyErrorV1::ProtocolViolation) => {
                return Err(ProviderIngestRuntimeErrorV1::SignerProtocolViolation);
            }
        }
        let submission = match self.outbox.authorize_and_begin_completion_submission(
            job_id,
            exact.transaction_hash,
            provider_owner,
            signer_policy,
            cursor,
            self.clock.now_ms(),
        ) {
            Ok(submission) => submission,
            Err(
                ProviderIngestOutboxError::RetryNotDue
                | ProviderIngestOutboxError::InvalidTransition
                | ProviderIngestOutboxError::InvalidSigningContext
                | ProviderIngestOutboxError::SignerPolicyRollback
                | ProviderIngestOutboxError::StaleFinalizedCursor,
            ) => return Ok(()),
            Err(error) => return Err(error.into()),
        };
        if submission.signed_transaction != exact.signed_transaction {
            return Err(ProviderIngestRuntimeErrorV1::IngressProtocolViolation);
        }
        match exact_current_signer_policy(&signer, provider_owner) {
            Ok(current_policy) if current_policy == signer_policy => {}
            result => {
                self.outbox.mark_completion_not_submitted(
                    job_id,
                    exact.transaction_hash,
                    self.clock.now_ms(),
                    cursor,
                )?;
                if result == Err(CurrentSignerPolicyErrorV1::ProtocolViolation) {
                    return Err(ProviderIngestRuntimeErrorV1::SignerProtocolViolation);
                }
                return Ok(());
            }
        }
        outcome.completion_submissions = outcome.completion_submissions.saturating_add(1);
        let disposition = match tokio::time::timeout(
            Duration::from_millis(self.policy.ingress_timeout_ms),
            self.ingress.expose(prepared, submission.signed_transaction),
        )
        .await
        {
            Ok(disposition) => disposition,
            Err(_) => ProviderIngestIngressDispositionV1::Ambiguous,
        };
        match disposition {
            ProviderIngestIngressDispositionV1::Submitted => self
                .outbox
                .mark_completion_submitted(job_id, exact.transaction_hash)?,
            ProviderIngestIngressDispositionV1::DefinitelyNotSubmitted => {
                self.outbox.mark_completion_not_submitted(
                    job_id,
                    exact.transaction_hash,
                    self.clock.now_ms(),
                    cursor,
                )?;
            }
            ProviderIngestIngressDispositionV1::Rejected => {
                self.outbox.mark_completion_transaction_rejected(
                    job_id,
                    exact.transaction_hash,
                    self.clock.now_ms(),
                    cursor,
                )?;
            }
            ProviderIngestIngressDispositionV1::Ambiguous => {}
        }
        Ok(())
    }

    async fn reconcile_transaction(
        &self,
        job_id: [u8; 32],
        transaction_hash: [u8; 32],
        baseline: ProviderIngestFinalizedCursorV1,
        cursor: ProviderIngestFinalizedCursorV1,
        ambiguous: bool,
    ) -> Result<(), ProviderIngestRuntimeErrorV1> {
        let observation = tokio::time::timeout(
            Duration::from_millis(self.policy.ingress_timeout_ms),
            self.ingress.observe(transaction_hash),
        )
        .await
        .unwrap_or(ProviderIngestTransactionObservationV1::Unavailable);
        match observation {
            ProviderIngestTransactionObservationV1::CommittedSuccess
            | ProviderIngestTransactionObservationV1::Pending => {
                if ambiguous {
                    self.outbox
                        .mark_completion_submitted(job_id, transaction_hash)?;
                }
            }
            ProviderIngestTransactionObservationV1::CommittedRejected => {
                self.outbox.mark_completion_transaction_rejected(
                    job_id,
                    transaction_hash,
                    self.clock.now_ms(),
                    cursor,
                )?;
            }
            ProviderIngestTransactionObservationV1::Unknown if cursor.height > baseline.height => {
                self.outbox.mark_completion_finalized_absent(
                    job_id,
                    transaction_hash,
                    self.clock.now_ms(),
                    cursor,
                )?;
            }
            ProviderIngestTransactionObservationV1::Unknown
            | ProviderIngestTransactionObservationV1::Unavailable => {}
        }
        Ok(())
    }
}

struct ValidatedAssignmentV1 {
    authorization: FinalizedProviderIngestAuthorizationV1,
    source_provider_ids: Vec<[u8; 32]>,
    musubi_archive: Option<ProviderIngestFinalizedMusubiArchiveClaimV1>,
}

fn local_stored_matches(
    stored: &ProviderIngestLocalStoredV1,
    authorization: &FinalizedProviderIngestAuthorizationV1,
    claim: Option<&ProviderIngestFinalizedMusubiArchiveClaimV1>,
) -> bool {
    if stored.manifest_id().is_empty() {
        return false;
    }
    match (claim, stored.musubi_bundle()) {
        (None, None) => authorization.musubi_context().is_none(),
        (Some(claim), Some(receipt)) => receipt.matches(claim, authorization),
        (None, Some(_)) | (Some(_), None) => false,
    }
}

fn persisted_receipt_matches(
    authorization: &FinalizedProviderIngestAuthorizationV1,
    claim: Option<&ProviderIngestFinalizedMusubiArchiveClaimV1>,
    receipt: Option<&ProviderIngestVerifiedMusubiBundleReceiptV1>,
) -> bool {
    match (claim, receipt) {
        (None, None) => authorization.musubi_context().is_none(),
        (Some(claim), Some(receipt)) => receipt.matches(claim, authorization),
        (None, Some(_)) | (Some(_), None) => false,
    }
}

fn validate_monotonic_finalized_cursor(
    previous: Option<ProviderIngestFinalizedCursorV1>,
    candidate: ProviderIngestFinalizedCursorV1,
) -> Result<(), ProviderIngestRuntimeErrorV1> {
    if previous.is_some_and(|previous| {
        candidate.height < previous.height
            || (candidate.height == previous.height && candidate.block_hash != previous.block_hash)
    }) {
        return Err(ProviderIngestRuntimeErrorV1::InvalidFinalizedPage);
    }
    Ok(())
}

fn validate_page(
    page: &ProviderIngestFinalizedAssignmentPageV1,
    after_order_id: Option<[u8; 32]>,
    expected_cursor: ProviderIngestFinalizedCursorV1,
    limit: usize,
) -> Result<(), ProviderIngestRuntimeErrorV1> {
    if page.finalized_cursor != expected_cursor
        || page.finalized_cursor.height == 0
        || page.finalized_cursor.block_hash == [0; 32]
        || page.finalized_block_time_ms == 0
        || page.rows.len() > limit
        || page.next_after_order_id.is_some() && page.rows.is_empty()
        || page.next_after_order_id.is_some() && page.rows.len() != limit
    {
        return Err(ProviderIngestRuntimeErrorV1::InvalidFinalizedPage);
    }
    let mut previous = after_order_id;
    for row in &page.rows {
        let order_id = *row.order.order_id.as_bytes();
        if previous.is_some_and(|previous| previous >= order_id) {
            return Err(ProviderIngestRuntimeErrorV1::InvalidFinalizedPage);
        }
        previous = Some(order_id);
    }
    if let Some(next) = page.next_after_order_id
        && Some(next) != previous
    {
        return Err(ProviderIngestRuntimeErrorV1::InvalidFinalizedPage);
    }
    Ok(())
}

fn validate_assignment(
    row: &ProviderIngestFinalizedAssignmentV1,
    cursor: ProviderIngestFinalizedCursorV1,
    provider_id: [u8; 32],
    chain_id: &ChainId,
    genesis_block_hash: [u8; 32],
    policy: ProviderIngestRuntimePolicyV1,
) -> Result<ValidatedAssignmentV1, ProviderIngestRuntimeErrorV1> {
    validate_assignment_with_source_bound(
        row,
        cursor,
        provider_id,
        chain_id,
        genesis_block_hash,
        policy.max_source_providers,
    )
}

fn validate_assignment_with_source_bound(
    row: &ProviderIngestFinalizedAssignmentV1,
    cursor: ProviderIngestFinalizedCursorV1,
    provider_id: [u8; 32],
    chain_id: &ChainId,
    genesis_block_hash: [u8; 32],
    max_source_providers: usize,
) -> Result<ValidatedAssignmentV1, ProviderIngestRuntimeErrorV1> {
    if row.pin.finalized_cursor.height != cursor.height
        || row.pin.finalized_cursor.block_hash != cursor.block_hash
        || row.order.deadline_epoch <= row.order.issued_epoch
        || row
            .committed_transaction_hash
            .is_some_and(|hash| hash == [0; 32])
        || row.order.assignment_revision == 0
        || row.order.canonical_order.is_empty()
        || row.order.canonical_order.len() > REPLICATION_ORDER_MAX_CANONICAL_BYTES_V1
        || row.completion_authority.as_ref().is_some_and(|authority| {
            !authority.is_valid() || row.provider_owner.as_ref() != Some(&authority.provider_owner)
        })
        || row.provider_owner.is_none() && row.completion_authority.is_some()
    {
        return Err(ProviderIngestRuntimeErrorV1::InvalidFinalizedBinding);
    }
    let order = decode_from_bytes_with_limits::<ReplicationOrderV1>(
        &row.order.canonical_order,
        REPLICATION_ORDER_DECODE_LIMITS_V1,
    )
    .map_err(|_| ProviderIngestRuntimeErrorV1::InvalidFinalizedBinding)?;
    order
        .validate()
        .map_err(|_| ProviderIngestRuntimeErrorV1::InvalidFinalizedBinding)?;
    let canonical = norito::to_bytes(&order)
        .map_err(|_| ProviderIngestRuntimeErrorV1::InvalidFinalizedBinding)?;
    if canonical != row.order.canonical_order
        || order.order_id != *row.order.order_id.as_bytes()
        || order.manifest_digest != *row.order.manifest_digest.as_bytes()
        || order.manifest_cid.as_slice() != row.order.manifest_root_cid.as_bytes()
        || row.pin.manifest.digest != row.order.manifest_digest
        || row.pin.manifest.root_cid != row.order.manifest_root_cid
        || row.pin.manifest.chunker.to_handle() != order.chunking_profile
    {
        return Err(ProviderIngestRuntimeErrorV1::InvalidFinalizedBinding);
    }
    if !order
        .assignments
        .iter()
        .any(|assignment| assignment.provider_id == provider_id)
    {
        return Err(ProviderIngestRuntimeErrorV1::InvalidFinalizedBinding);
    }
    match (row.order.musubi_archive, &row.musubi_archive) {
        (None, None) => {}
        (Some(archive_id), Some(claim)) => {
            let commitment = claim.commitment();
            if claim.chain_id() != chain_id
                || claim.genesis_block_hash() != genesis_block_hash
                || claim.provider_id() != provider_id
                || claim.observed_finalized_cursor() != cursor
                || claim.replication_order() != *row.order.order_id.as_bytes()
                || claim.archive_id() != archive_id
                || claim.archive_id() != commitment.archive_id()
                || commitment.validate().is_err()
                || commitment.root_cid != row.pin.manifest.root_cid
                || commitment.chunker != row.pin.manifest.chunker
                || commitment.chunk_plan_digest.as_bytes()
                    != &row.pin.manifest.chunk_digest_sha3_256
                || commitment.por_root.as_bytes() != &row.pin.manifest.por_root
                || commitment.content_length != row.pin.manifest.content_length
            {
                return Err(ProviderIngestRuntimeErrorV1::InvalidFinalizedBinding);
            }
        }
        (None, Some(_)) | (Some(_), None) => {
            return Err(ProviderIngestRuntimeErrorV1::InvalidFinalizedBinding);
        }
    }
    let local_completion = row.order.provider_completion(ProviderId::new(provider_id));
    match (
        row.order.musubi_archive,
        local_completion,
        &row.completed_musubi_archive,
    ) {
        (None, _, None) | (Some(_), None, None) => {}
        (Some(archive_id), Some(completion), Some(claim)) => {
            let commitment = claim.commitment();
            let Some(pre_completion_claim) = row.musubi_archive.as_ref() else {
                return Err(ProviderIngestRuntimeErrorV1::InvalidFinalizedBinding);
            };
            if claim.chain_id() != chain_id
                || claim.genesis_block_hash() != genesis_block_hash
                || claim.provider_id() != provider_id
                || claim.observed_finalized_cursor() != cursor
                || claim.replication_order() != *row.order.order_id.as_bytes()
                || claim.archive_id() != archive_id
                || claim.archive_id() != commitment.archive_id()
                || commitment != pre_completion_claim.commitment()
                || claim.completion() != completion
                || commitment.validate().is_err()
                || commitment.root_cid != row.pin.manifest.root_cid
                || commitment.chunker != row.pin.manifest.chunker
                || commitment.chunk_plan_digest.as_bytes()
                    != &row.pin.manifest.chunk_digest_sha3_256
                || commitment.por_root.as_bytes() != &row.pin.manifest.por_root
                || commitment.content_length != row.pin.manifest.content_length
            {
                return Err(ProviderIngestRuntimeErrorV1::InvalidFinalizedBinding);
            }
        }
        (None, _, Some(_)) | (Some(_), None, Some(_)) | (Some(_), Some(_), None) => {
            return Err(ProviderIngestRuntimeErrorV1::InvalidFinalizedBinding);
        }
    }
    let source_provider_ids = order
        .assignments
        .iter()
        .filter_map(|assignment| {
            (assignment.provider_id != provider_id).then_some(assignment.provider_id)
        })
        .collect::<Vec<_>>();
    if source_provider_ids.len() > max_source_providers {
        return Err(ProviderIngestRuntimeErrorV1::InvalidFinalizedBinding);
    }
    let target_replicas = usize::from(order.target_replicas);
    if row.order.provider_completions.len() > target_replicas {
        return Err(ProviderIngestRuntimeErrorV1::InvalidFinalizedBinding);
    }
    let mut completions = BTreeSet::new();
    for completion in &row.order.provider_completions {
        if !order
            .assignments
            .iter()
            .any(|assignment| assignment.provider_id == *completion.provider_id.as_bytes())
            || completion.completion_epoch < row.order.issued_epoch
            || completion.completion_epoch > row.order.deadline_epoch
            || completion.assignment_revision != row.order.assignment_revision
            || !completion.completion_authority.is_valid()
            || completion.completion_authority.provider_owner != completion.completed_by
            || !completion.finalized_anchor.is_valid()
            || completion.finalized_anchor.height > cursor.height
            || completion.finalized_anchor.height == cursor.height
                && completion.finalized_anchor.block_hash != cursor.block_hash
            || !completions.insert(*completion.provider_id.as_bytes())
        {
            return Err(ProviderIngestRuntimeErrorV1::InvalidFinalizedBinding);
        }
    }
    match row.order.status {
        ReplicationOrderStatus::Pending | ReplicationOrderStatus::Expired(_)
            if row.order.provider_completions.len() < target_replicas => {}
        ReplicationOrderStatus::Completed(epoch)
            if row.order.provider_completions.len() == target_replicas
                && row
                    .order
                    .provider_completions
                    .last()
                    .is_some_and(|completion| completion.completion_epoch == epoch) => {}
        ReplicationOrderStatus::Pending
        | ReplicationOrderStatus::Completed(_)
        | ReplicationOrderStatus::Expired(_) => {
            return Err(ProviderIngestRuntimeErrorV1::InvalidFinalizedBinding);
        }
    }
    let authorization = if let Some(claim) = row.musubi_archive.as_ref() {
        FinalizedProviderIngestAuthorizationV1::from_finalized_musubi_state(
            cursor.height,
            cursor.block_hash,
            provider_id,
            *row.order.order_id.as_bytes(),
            *row.pin.manifest.digest.as_bytes(),
            order.manifest_cid,
            order.chunking_profile,
            row.pin.manifest.chunk_digest_sha3_256,
            row.pin.manifest.por_root,
            row.pin.manifest.content_length,
            FinalizedProviderIngestMusubiContextV1::new(
                chain_id.clone(),
                genesis_block_hash,
                claim.archive_id(),
            )?,
        )?
    } else {
        FinalizedProviderIngestAuthorizationV1::from_finalized_state(
            cursor.height,
            cursor.block_hash,
            provider_id,
            *row.order.order_id.as_bytes(),
            *row.pin.manifest.digest.as_bytes(),
            order.manifest_cid,
            order.chunking_profile,
            row.pin.manifest.chunk_digest_sha3_256,
            row.pin.manifest.por_root,
            row.pin.manifest.content_length,
        )?
    };
    Ok(ValidatedAssignmentV1 {
        authorization,
        source_provider_ids,
        musubi_archive: row.musubi_archive.clone(),
    })
}

fn authorization_from_status_and_row(
    status: &crate::provider_ingest_outbox::ProviderIngestStatusV1,
    row: &ProviderIngestFinalizedAssignmentV1,
    cursor: ProviderIngestFinalizedCursorV1,
) -> Result<FinalizedProviderIngestAuthorizationV1, ProviderIngestRuntimeErrorV1> {
    let order = decode_from_bytes_with_limits::<ReplicationOrderV1>(
        &row.order.canonical_order,
        REPLICATION_ORDER_DECODE_LIMITS_V1,
    )
    .map_err(|_| ProviderIngestRuntimeErrorV1::InvalidFinalizedBinding)?;
    let authorization = if let Some(claim) = row.musubi_archive.as_ref() {
        FinalizedProviderIngestAuthorizationV1::from_finalized_musubi_state(
            cursor.height,
            cursor.block_hash,
            status.provider_id,
            status.order_id,
            status.manifest_digest,
            order.manifest_cid,
            order.chunking_profile,
            row.pin.manifest.chunk_digest_sha3_256,
            row.pin.manifest.por_root,
            row.pin.manifest.content_length,
            FinalizedProviderIngestMusubiContextV1::new(
                claim.chain_id().clone(),
                claim.genesis_block_hash(),
                claim.archive_id(),
            )?,
        )?
    } else {
        FinalizedProviderIngestAuthorizationV1::from_finalized_state(
            cursor.height,
            cursor.block_hash,
            status.provider_id,
            status.order_id,
            status.manifest_digest,
            order.manifest_cid,
            order.chunking_profile,
            row.pin.manifest.chunk_digest_sha3_256,
            row.pin.manifest.por_root,
            row.pin.manifest.content_length,
        )?
    };
    if authorization.job_id() != status.job_id {
        return Err(ProviderIngestRuntimeErrorV1::InvalidFinalizedBinding);
    }
    Ok(authorization)
}

enum LeaseOperationOutcomeV1<T> {
    Completed(T),
    TimedOut,
}

enum MutatingStorageOutcomeV1<T> {
    Completed(T),
    CompletedAfterSoftTimeout(T),
}

/// Fatal supervised-runtime failure.
#[allow(clippy::large_enum_variant, variant_size_differences)]
#[derive(Debug, Error)]
pub enum ProviderIngestRuntimeErrorV1 {
    /// Runtime bounds or timeout policy is invalid.
    #[error("provider-ingest runtime policy is invalid")]
    InvalidPolicy,
    /// Configured provider identity is zero.
    #[error("provider-ingest runtime provider identity is invalid")]
    InvalidProviderId,
    /// Configured chain identity is empty or over the V1 canonical bound.
    #[error("provider-ingest runtime chain identity is invalid")]
    InvalidChainId,
    /// Configured genesis trust anchor is zero.
    #[error("provider-ingest runtime genesis block hash is invalid")]
    InvalidGenesisBlockHash,
    /// Finalized ledger paging is unavailable.
    #[error("provider-ingest finalized ledger is unavailable")]
    FinalizedLedgerUnavailable,
    /// Finalized page cursor, bounds, or ordering is invalid.
    #[error("provider-ingest finalized page is invalid")]
    InvalidFinalizedPage,
    /// Finalized pin/order/provider material is noncanonical or inconsistent.
    #[error("provider-ingest finalized binding is invalid")]
    InvalidFinalizedBinding,
    /// Local storage returned a manifest identity that violates its exact contract.
    #[error("provider-ingest local storage violated the exact binding")]
    StorageProtocolViolation,
    /// Authenticated source identity, policy, or qualification drifted.
    #[error("provider-ingest authenticated source violated its exact binding")]
    SourceProtocolViolation,
    /// Resolved signer or signed transaction violated the prepared context.
    #[error("provider-ingest signer violated the prepared operation")]
    SignerProtocolViolation,
    /// Queue preflight/exposure violated the exact transaction contract.
    #[error("provider-ingest ingress violated the prepared operation")]
    IngressProtocolViolation,
    /// Durable outbox transition failed.
    #[error(transparent)]
    Outbox(#[from] ProviderIngestOutboxError),
}

#[cfg(test)]
#[allow(clippy::too_many_lines)]
mod tests {
    use std::{
        io,
        sync::{
            Mutex,
            atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering},
        },
        time::Instant,
    };

    use iroha_crypto::{Algorithm, KeyPair};
    use iroha_data_model::{
        isi::{InstructionBox, sorafs::CompleteReplicationOrder},
        metadata::Metadata,
        musubi::{
            MusubiAbiBindingV1, MusubiKotodamaEditionV1, MusubiPackageIdV1, MusubiPackageScopeV1,
            MusubiReleaseIdV1, MusubiReleaseMetadataV1, MusubiSemanticReleaseManifestV1,
            MusubiVerificationLockV1,
        },
        nexus::DataSpaceId,
        sorafs::pin_registry::{
            ChunkerProfileHandle, ManifestDigest, ManifestRootCid, PinManifestFinalizedCursorV1,
            PinManifestRecord, PinPolicy, ProviderIngestFinalizedAnchorV1,
            ReplicationOrderCompletionRecord, ReplicationOrderId,
        },
        transaction::{FeePaymentIntent, TransactionBuilder},
    };
    use sorafs_car::{
        CarBuildPlan, CarWriter, FileEntry, compute_chunk_plan_digest_sha3, compute_por_root,
        musubi::{
            MUSUBI_BUNDLE_ARTIFACT_DESCRIPTOR_PATH_V1, MUSUBI_BUNDLE_SEMANTIC_RELEASE_PATH_V1,
            MUSUBI_BUNDLE_VERIFICATION_LOCK_PATH_V1, MusubiBundleVerifierV1,
        },
    };
    use sorafs_manifest::{
        BLAKE3_256_MULTIHASH_CODE, DagCodecId, ManifestBuilder, ManifestV1,
        capacity::{REPLICATION_ORDER_VERSION_V1, ReplicationAssignmentV1, ReplicationOrderSlaV1},
    };

    use super::*;
    use crate::provider_ingest_outbox::{
        ProviderIngestCompletionStateV1, ProviderIngestDeliveryStateV1,
        ProviderIngestOutboxPolicyV1,
    };
    use crate::{
        NodeHandle,
        config::StorageConfig,
        provider_attestation_journal::{
            MusubiProviderAttestationJournalCasOutcomeV1, MusubiProviderAttestationJournalPolicyV1,
            MusubiProviderAttestationJournalStoreErrorV1,
            MusubiProviderAttestationJournalStoreSnapshotV1,
            MusubiProviderAttestationJournalStoreV1, MusubiProviderAttestationJournalV1,
            musubi_provider_attestation_journal_checkpoint_revision_v1,
        },
        scheduler::{StorageSchedulerConfig, StorageSchedulersRuntime},
        store::StorageBackend,
    };

    const LOCAL_PROVIDER: [u8; 32] = [0x11; 32];
    const SOURCE_PROVIDER: [u8; 32] = [0x22; 32];
    const TEST_GENESIS_BLOCK_HASH: [u8; 32] = [0xA7; 32];

    fn test_chain_id() -> ChainId {
        ChainId::from("provider-ingest-runtime-test")
    }

    fn validate_assignment(
        row: &ProviderIngestFinalizedAssignmentV1,
        cursor: ProviderIngestFinalizedCursorV1,
        provider_id: [u8; 32],
        policy: ProviderIngestRuntimePolicyV1,
    ) -> Result<ValidatedAssignmentV1, ProviderIngestRuntimeErrorV1> {
        super::validate_assignment(
            row,
            cursor,
            provider_id,
            &test_chain_id(),
            TEST_GENESIS_BLOCK_HASH,
            policy,
        )
    }

    fn account(seed: u8) -> AccountId {
        let key = KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519).expect("key");
        AccountId::new(key.public_key().clone())
    }

    fn cursor(height: u64) -> ProviderIngestFinalizedCursorV1 {
        ProviderIngestFinalizedCursorV1 {
            height,
            block_hash: [u8::try_from(height).unwrap_or(0xFE); 32],
        }
    }

    fn completion_signer_policy(revision: u64) -> ProviderIngestCompletionSignerPolicyV1 {
        let digest_byte = u8::try_from(revision).unwrap_or(0xFE);
        ProviderIngestCompletionSignerPolicyV1 {
            policy_id: [0xA1; 32],
            revision,
            predecessor_digest: (revision > 1).then(|| [digest_byte.saturating_sub(1); 32]),
            policy_digest: [digest_byte; 32],
        }
    }

    #[test]
    fn completion_signer_binding_rejects_test_handles_stale_revisions_and_key_mismatch() {
        let key = KeyPair::try_from_seed(vec![0x73; 32], Algorithm::Ed25519)
            .expect("completion signer key");
        let authority = AccountId::new(key.public_key().clone());
        let qualification = ProviderIngestCompletionSignerQualificationV1::new(
            1,
            completion_signer_policy(1),
            Algorithm::Ed25519,
            key.public_key().clone(),
        );
        assert!(qualification.matches_authority(&authority));
        assert_eq!(qualification.validate(), Ok(()));
        assert_eq!(
            ProviderIngestCompletionSignerBindingV1::new(
                "pkcs11:sorafs-provider-ingest-primary",
                qualification.clone(),
            )
            .validate(),
            Ok(())
        );
        assert_eq!(
            ProviderIngestCompletionSignerBindingV1::new(
                "pkcs11:sorafs-provider-ingest-test",
                qualification.clone(),
            )
            .validate(),
            Err(ProviderIngestCompletionSignerBindingErrorV1::InvalidSignerHandle)
        );

        let mut stale = qualification.clone();
        stale.adapter_revision = 0;
        assert_eq!(
            stale.validate(),
            Err(ProviderIngestCompletionSignerBindingErrorV1::InvalidSignerQualification)
        );
        let mut mismatched_algorithm = qualification;
        mismatched_algorithm.algorithm = Algorithm::MlDsa;
        assert_eq!(
            mismatched_algorithm.validate(),
            Err(ProviderIngestCompletionSignerBindingErrorV1::InvalidSignerQualification)
        );
    }

    fn completion_record(
        provider_id: ProviderId,
        completed_by: AccountId,
        completion_epoch: u64,
    ) -> ReplicationOrderCompletionRecord {
        ReplicationOrderCompletionRecord {
            provider_id,
            completed_by: completed_by.clone(),
            completion_epoch,
            assignment_revision: 1,
            completion_authority: ProviderIngestCompletionAuthorityV1::new(
                completed_by,
                completion_signer_policy(1),
            ),
            finalized_anchor: ProviderIngestFinalizedAnchorV1 {
                height: completion_epoch,
                block_hash: cursor(completion_epoch).block_hash,
            },
        }
    }

    fn fixture_row(order_seed: u8) -> ProviderIngestFinalizedAssignmentV1 {
        let digest = ManifestDigest::new([order_seed.wrapping_add(0x40); 32]);
        let root =
            ManifestRootCid::from_blake3_digest([order_seed.wrapping_add(0x50); 32]).unwrap();
        let chunker = ChunkerProfileHandle {
            profile_id: 1,
            namespace: "sorafs".to_owned(),
            name: "sf1".to_owned(),
            semver: "1.0.0".to_owned(),
            multihash_code: 0x1f,
        };
        let mut manifest = PinManifestRecord::new(
            digest,
            root.clone(),
            chunker,
            [order_seed.wrapping_add(0x60); 32],
            [order_seed.wrapping_add(0x70); 32],
            4_096,
            PinPolicy::default(),
            account(1),
            7,
            None,
            None,
            Metadata::default(),
        );
        manifest.status = PinStatus::Approved(7);
        let order_id = [order_seed; 32];
        let order_body = ReplicationOrderV1 {
            version: REPLICATION_ORDER_VERSION_V1,
            order_id,
            manifest_cid: root.as_bytes().to_vec(),
            manifest_digest: *digest.as_bytes(),
            chunking_profile: "sorafs.sf1@1.0.0".to_owned(),
            target_replicas: 2,
            assignments: vec![
                ReplicationAssignmentV1 {
                    provider_id: LOCAL_PROVIDER,
                    slice_gib: 1,
                    lane: None,
                },
                ReplicationAssignmentV1 {
                    provider_id: SOURCE_PROVIDER,
                    slice_gib: 1,
                    lane: None,
                },
            ],
            issued_at: 100,
            deadline_at: 200,
            sla: ReplicationOrderSlaV1 {
                ingest_deadline_secs: 10,
                min_availability_percent_milli: 99_000,
                min_por_success_percent_milli: 99_000,
            },
            metadata: Vec::new(),
        };
        order_body.validate().expect("valid order");
        ProviderIngestFinalizedAssignmentV1 {
            pin: PinManifestFinalizedRecordV1 {
                finalized_cursor: PinManifestFinalizedCursorV1 {
                    height: 8,
                    block_hash: cursor(8).block_hash,
                },
                manifest,
            },
            order: ReplicationOrderRecord {
                order_id: ReplicationOrderId::new(order_id),
                manifest_digest: digest,
                manifest_root_cid: root,
                musubi_archive: None,
                issued_by: account(1),
                issued_epoch: 7,
                deadline_epoch: 20,
                canonical_order: norito::to_bytes(&order_body).expect("order bytes"),
                assignment_revision: 1,
                provider_completions: Vec::new(),
                status: ReplicationOrderStatus::Pending,
            },
            musubi_archive: None,
            completed_musubi_archive: None,
            provider_owner: Some(account(8)),
            completion_authority: Some(ProviderIngestCompletionAuthorityV1::new(
                account(8),
                completion_signer_policy(1),
            )),
            completion_epoch: Some(8),
            committed_transaction_hash: None,
        }
    }

    fn fixture_page(
        row: ProviderIngestFinalizedAssignmentV1,
    ) -> ProviderIngestFinalizedAssignmentPageV1 {
        let finalized_cursor = ProviderIngestFinalizedCursorV1 {
            height: row.pin.finalized_cursor.height,
            block_hash: row.pin.finalized_cursor.block_hash,
        };
        ProviderIngestFinalizedAssignmentPageV1 {
            finalized_cursor,
            finalized_block_time_ms: finalized_cursor.height.saturating_mul(1_000),
            rows: vec![row],
            next_after_order_id: None,
        }
    }

    fn musubi_binding_for_row(
        row: &ProviderIngestFinalizedAssignmentV1,
        seed: u8,
    ) -> MusubiReplicationOrderArchiveBindingV1 {
        let commitment = MusubiArchiveCommitmentV1 {
            root_cid: row.pin.manifest.root_cid.clone(),
            chunker: row.pin.manifest.chunker.clone(),
            chunk_plan_digest: iroha_data_model::musubi::MusubiContentDigestV1::new(
                row.pin.manifest.chunk_digest_sha3_256,
            ),
            por_root: iroha_data_model::musubi::MusubiContentDigestV1::new(
                row.pin.manifest.por_root,
            ),
            content_length: row.pin.manifest.content_length,
            car_digest: iroha_data_model::musubi::MusubiContentDigestV1::new([seed; 32]),
            car_size: row.pin.manifest.content_length.saturating_add(1_024),
            bundle_digest: iroha_data_model::musubi::MusubiContentDigestV1::new(
                [seed.wrapping_add(1); 32],
            ),
            source_tree_digest: iroha_data_model::musubi::MusubiContentDigestV1::new(
                [seed.wrapping_add(2); 32],
            ),
            descriptor_digest: iroha_data_model::musubi::MusubiContentDigestV1::new(
                [seed.wrapping_add(3); 32],
            ),
            file_count: 1,
            chunk_count: 1,
        };
        MusubiReplicationOrderArchiveBindingV1::new(
            row.order.order_id,
            commitment.archive_id(),
            commitment,
        )
    }

    fn fixture_musubi_row(
        order_seed: u8,
        commitment_seed: u8,
    ) -> ProviderIngestFinalizedAssignmentV1 {
        let mut row = fixture_row(order_seed);
        let binding = musubi_binding_for_row(&row, commitment_seed);
        let claim = ProviderIngestFinalizedClaimFactoryV1::new(
            test_chain_id(),
            TEST_GENESIS_BLOCK_HASH,
            LOCAL_PROVIDER,
        )
        .seal_musubi_archive(
            &test_chain_id(),
            cursor(8),
            *row.order.order_id.as_bytes(),
            &row.pin.manifest,
            binding.clone(),
        )
        .expect("seal Musubi fixture claim");
        row.order.musubi_archive = Some(binding.archive_id);
        row.musubi_archive = Some(claim);
        row
    }

    fn test_verified_musubi_receipt(
        claim: &ProviderIngestFinalizedMusubiArchiveClaimV1,
        authorization: &FinalizedProviderIngestAuthorizationV1,
    ) -> ProviderIngestVerifiedMusubiBundleReceiptV1 {
        ProviderIngestVerifiedMusubiBundleReceiptV1 {
            chain_id: claim.chain_id().clone(),
            genesis_block_hash: claim.genesis_block_hash(),
            provider_id: claim.provider_id(),
            observed_finalized_cursor: claim.observed_finalized_cursor(),
            replication_order: claim.replication_order(),
            manifest_digest: authorization.manifest_digest(),
            archive_id: claim.archive_id(),
            commitment: claim.commitment().clone(),
            semantic_release_manifest_digest: MusubiSemanticReleaseDigestV1::new([0xC1; 32]),
            verification_lock_digest: MusubiVerificationLockDigestV1::new([0xC2; 32]),
        }
    }

    fn append_attestation_fixture_frame(output: &mut Vec<u8>, bytes: &[u8]) {
        output.extend_from_slice(
            &u64::try_from(bytes.len())
                .expect("fixture frame length")
                .to_be_bytes(),
        );
        output.extend_from_slice(bytes);
    }

    fn attestation_fixture_domain_digest(domain: &[u8], material: &[u8]) -> MusubiContentDigestV1 {
        let mut hasher = blake3::Hasher::new();
        hasher.update(domain);
        hasher.update(
            &u64::try_from(material.len())
                .expect("fixture transcript length")
                .to_be_bytes(),
        );
        hasher.update(material);
        MusubiContentDigestV1::new(*hasher.finalize().as_bytes())
    }

    struct VerifiedAttestationBundleFixtureV1 {
        verified: VerifiedMusubiBundleV1,
        commitment: MusubiArchiveCommitmentV1,
        plan: CarBuildPlan,
        payload: Vec<u8>,
    }

    fn verified_attestation_bundle_fixture(source_seed: u8) -> VerifiedAttestationBundleFixtureV1 {
        const SOURCE_TREE_DOMAIN: &[u8] = b"musubi-source-tree-v1\0";
        const BUNDLE_DOMAIN: &[u8] = b"musubi-bundle-v1\0";

        let release = MusubiReleaseIdV1::new(
            MusubiPackageIdV1::new(
                DataSpaceId::new(7),
                MusubiPackageScopeV1::DataspaceRoot,
                "attestation-fixture".parse().expect("fixture package name"),
            ),
            "1.0.0".parse().expect("fixture version"),
        );
        let verification_lock = MusubiVerificationLockV1 {
            schema: MusubiVerificationLockV1::SCHEMA.to_owned(),
            version: MUSUBI_REGISTRY_VERSION_V1,
            root: release.clone(),
            root_dependencies: Vec::new(),
            nodes: Vec::new(),
        };
        let semantic_release = MusubiSemanticReleaseManifestV1 {
            release,
            edition: MusubiKotodamaEditionV1::V1,
            abi: MusubiAbiBindingV1::new([0xA8; 32]).expect("fixture ABI"),
            dependencies: Vec::new(),
            exports: Vec::new(),
            interface_digest: MusubiContentDigestV1::new([0xA9; 32]),
            metadata: MusubiReleaseMetadataV1::default(),
            verification_lock_digest: verification_lock.digest(),
        };
        let source_path = "src/lib.ko";
        let source = vec![source_seed; 37];
        let mut source_material = Vec::new();
        append_attestation_fixture_frame(&mut source_material, SOURCE_TREE_DOMAIN);
        source_material.extend_from_slice(&1_u32.to_be_bytes());
        append_attestation_fixture_frame(&mut source_material, source_path.as_bytes());
        source_material.extend_from_slice(
            &u64::try_from(source.len())
                .expect("fixture source length")
                .to_be_bytes(),
        );
        source_material.extend_from_slice(blake3::hash(&source).as_bytes());
        let source_tree_digest =
            attestation_fixture_domain_digest(SOURCE_TREE_DOMAIN, &source_material);
        let descriptor = MusubiArtifactDescriptorV1::new(
            semantic_release.semantic_digest(),
            source_tree_digest,
            verification_lock.digest(),
            u64::try_from(source.len()).expect("fixture source length"),
            1,
        )
        .expect("fixture descriptor");
        let semantic_release_bytes = semantic_release.encode();
        let descriptor_bytes = descriptor.encode();
        let verification_lock_bytes = verification_lock.encode();
        let mut descriptor_material = Vec::new();
        append_attestation_fixture_frame(
            &mut descriptor_material,
            MUSUBI_ARTIFACT_DESCRIPTOR_DIGEST_DOMAIN_V1,
        );
        append_attestation_fixture_frame(&mut descriptor_material, &descriptor_bytes);
        let descriptor_digest = attestation_fixture_domain_digest(
            MUSUBI_ARTIFACT_DESCRIPTOR_DIGEST_DOMAIN_V1,
            &descriptor_material,
        );
        let mut bundle_material = Vec::new();
        for transcript in [
            BUNDLE_DOMAIN,
            semantic_release_bytes.as_slice(),
            descriptor_material.as_slice(),
            source_material.as_slice(),
            verification_lock_bytes.as_slice(),
        ] {
            append_attestation_fixture_frame(&mut bundle_material, transcript);
        }
        let bundle_digest = attestation_fixture_domain_digest(BUNDLE_DOMAIN, &bundle_material);
        let entries = vec![
            FileEntry {
                path: source_path.split('/').map(str::to_owned).collect(),
                data: source,
            },
            FileEntry {
                path: MUSUBI_BUNDLE_SEMANTIC_RELEASE_PATH_V1
                    .split('/')
                    .map(str::to_owned)
                    .collect(),
                data: semantic_release_bytes,
            },
            FileEntry {
                path: MUSUBI_BUNDLE_ARTIFACT_DESCRIPTOR_PATH_V1
                    .split('/')
                    .map(str::to_owned)
                    .collect(),
                data: descriptor_bytes,
            },
            FileEntry {
                path: MUSUBI_BUNDLE_VERIFICATION_LOCK_PATH_V1
                    .split('/')
                    .map(str::to_owned)
                    .collect(),
                data: verification_lock_bytes,
            },
        ];
        let (plan, payload) = CarBuildPlan::from_files(entries).expect("fixture bundle plan");
        let mut car = Vec::new();
        let stats = CarWriter::new(&plan, &payload)
            .expect("fixture CAR writer")
            .write_to(&mut car)
            .expect("fixture canonical CAR");
        let chunker = sorafs_car::chunker_registry::default_descriptor();
        let commitment = MusubiArchiveCommitmentV1 {
            root_cid: ManifestRootCid::try_from(stats.root_cids[0].clone())
                .expect("fixture root CID"),
            chunker: ChunkerProfileHandle {
                profile_id: chunker.id.0,
                namespace: chunker.namespace.to_owned(),
                name: chunker.name.to_owned(),
                semver: chunker.semver.to_owned(),
                multihash_code: chunker.multihash_code,
            },
            chunk_plan_digest: MusubiContentDigestV1::new(compute_chunk_plan_digest_sha3(
                &plan.chunks,
            )),
            por_root: MusubiContentDigestV1::new(
                compute_por_root(&payload, &plan).expect("fixture PoR"),
            ),
            content_length: plan.content_length,
            car_digest: MusubiContentDigestV1::new(*stats.car_archive_digest.as_bytes()),
            car_size: stats.car_size,
            bundle_digest,
            source_tree_digest,
            descriptor_digest,
            file_count: 1,
            chunk_count: u32::try_from(plan.chunks.len()).expect("fixture chunk count"),
        };
        let verified = MusubiBundleVerifierV1::verify(&plan, &car, &commitment)
            .expect("fixture canonical bundle verification");
        VerifiedAttestationBundleFixtureV1 {
            verified,
            commitment,
            plan,
            payload,
        }
    }

    fn completed_attestation_claim(
        commitment: MusubiArchiveCommitmentV1,
    ) -> ProviderIngestFinalizedMusubiCompletionClaimV1 {
        ProviderIngestFinalizedMusubiCompletionClaimV1 {
            chain_id: test_chain_id(),
            genesis_block_hash: TEST_GENESIS_BLOCK_HASH,
            provider_id: LOCAL_PROVIDER,
            observed_finalized_cursor: cursor(8),
            binding: MusubiReplicationOrderArchiveBindingV1::new(
                ReplicationOrderId::new([0xAC; 32]),
                commitment.archive_id(),
                commitment,
            ),
            completion: completion_record(ProviderId::new(LOCAL_PROVIDER), account(8), 8),
        }
    }

    fn completed_attestation_authorization(
        claim: &ProviderIngestFinalizedMusubiCompletionClaimV1,
        manifest_digest: [u8; 32],
    ) -> FinalizedProviderIngestAuthorizationV1 {
        FinalizedProviderIngestAuthorizationV1::from_finalized_musubi_state(
            claim.observed_finalized_cursor().height,
            claim.observed_finalized_cursor().block_hash,
            claim.provider_id(),
            claim.replication_order(),
            manifest_digest,
            claim.commitment().root_cid.as_bytes().to_vec(),
            claim.commitment().chunker.to_handle(),
            *claim.commitment().chunk_plan_digest.as_bytes(),
            *claim.commitment().por_root.as_bytes(),
            claim.commitment().content_length,
            FinalizedProviderIngestMusubiContextV1::new(
                claim.chain_id().clone(),
                claim.genesis_block_hash(),
                claim.archive_id(),
            )
            .expect("completed-claim Musubi context"),
        )
        .expect("completed-claim retained authorization")
    }

    fn completed_attestation_manifest(fixture: &VerifiedAttestationBundleFixtureV1) -> ManifestV1 {
        let car_stats = CarWriter::new(&fixture.plan, &fixture.payload)
            .expect("prepare completed-attestation fixture CAR")
            .write_to(io::sink())
            .expect("compute completed-attestation fixture CAR");
        ManifestBuilder::new()
            .root_cid(
                car_stats
                    .root_cids
                    .first()
                    .cloned()
                    .expect("completed-attestation fixture root"),
            )
            .dag_codec(DagCodecId(car_stats.dag_codec))
            .chunking_from_profile(fixture.plan.chunk_profile, BLAKE3_256_MULTIHASH_CODE)
            .chunk_digest_sha3_256(compute_chunk_plan_digest_sha3(&fixture.plan.chunks))
            .por_root(
                compute_por_root(&fixture.payload, &fixture.plan)
                    .expect("completed-attestation fixture PoR root"),
            )
            .content_length(fixture.plan.content_length)
            .car_digest(*car_stats.car_archive_digest.as_bytes())
            .car_size(car_stats.car_size)
            .pin_policy(PinPolicy::default())
            .build()
            .expect("completed-attestation fixture manifest")
    }

    fn completed_attestation_capture_source_row(
        fixture: &VerifiedAttestationBundleFixtureV1,
        manifest: &ManifestV1,
    ) -> ProviderIngestCompletedMusubiCaptureSourceRowV1 {
        let claim = completed_attestation_claim(fixture.commitment.clone());
        let manifest_digest =
            ManifestDigest::from_manifest(manifest).expect("completed-attestation manifest digest");
        let manifest_root = ManifestRootCid::try_from_slice(&manifest.root_cid)
            .expect("completed-attestation manifest root");
        let chunker = ChunkerProfileHandle {
            profile_id: manifest.chunking.profile_id.0,
            namespace: manifest.chunking.namespace.clone(),
            name: manifest.chunking.name.clone(),
            semver: manifest.chunking.semver.clone(),
            multihash_code: manifest.chunking.multihash_code,
        };
        let mut pin = PinManifestRecord::new(
            manifest_digest,
            manifest_root.clone(),
            chunker.clone(),
            manifest.chunk_digest_sha3_256,
            manifest.por_root,
            manifest.content_length,
            PinPolicy::default(),
            account(8),
            8,
            None,
            None,
            Metadata::default(),
        );
        pin.status = PinStatus::Approved(8);
        let order_id = claim.replication_order();
        let order_body = ReplicationOrderV1 {
            version: REPLICATION_ORDER_VERSION_V1,
            order_id,
            manifest_cid: manifest.root_cid.clone(),
            manifest_digest: *manifest_digest.as_bytes(),
            chunking_profile: chunker.to_handle(),
            target_replicas: 1,
            assignments: vec![ReplicationAssignmentV1 {
                provider_id: LOCAL_PROVIDER,
                slice_gib: 1,
                lane: None,
            }],
            issued_at: 1,
            deadline_at: 20,
            sla: ReplicationOrderSlaV1 {
                ingest_deadline_secs: 10,
                min_availability_percent_milli: 99_000,
                min_por_success_percent_milli: 99_000,
            },
            metadata: Vec::new(),
        };
        order_body
            .validate()
            .expect("completed-attestation replication order");
        ProviderIngestCompletedMusubiCaptureSourceRowV1::from_projected_fields(
            PinManifestFinalizedRecordV1 {
                finalized_cursor: PinManifestFinalizedCursorV1 {
                    height: cursor(8).height,
                    block_hash: cursor(8).block_hash,
                },
                manifest: pin,
            },
            ReplicationOrderRecord {
                order_id: ReplicationOrderId::new(order_id),
                manifest_digest,
                manifest_root_cid: manifest_root,
                musubi_archive: Some(claim.archive_id()),
                issued_by: account(1),
                issued_epoch: 1,
                deadline_epoch: 20,
                canonical_order: norito::to_bytes(&order_body)
                    .expect("encode completed-attestation replication order"),
                assignment_revision: 1,
                provider_completions: vec![claim.completion().clone()],
                status: ReplicationOrderStatus::Completed(8),
            },
            Some(claim.binding.clone()),
            Some(account(8)),
            Some(claim.completion().completion_authority.clone()),
            Some(8),
            None,
        )
    }

    #[derive(Default)]
    struct CaptureJournalMemoryStore {
        checkpoint: Mutex<Option<Vec<u8>>>,
    }

    impl MusubiProviderAttestationJournalStoreV1 for CaptureJournalMemoryStore {
        fn load<'a>(
            &'a self,
        ) -> ProviderIngestFutureV1<
            'a,
            Result<
                MusubiProviderAttestationJournalStoreSnapshotV1,
                MusubiProviderAttestationJournalStoreErrorV1,
            >,
        > {
            Box::pin(async move {
                let checkpoint = self
                    .checkpoint
                    .lock()
                    .map_err(|_| MusubiProviderAttestationJournalStoreErrorV1::Unavailable)?;
                checkpoint.as_ref().map_or_else(
                    || Ok(MusubiProviderAttestationJournalStoreSnapshotV1::empty()),
                    |bytes| {
                        MusubiProviderAttestationJournalStoreSnapshotV1::from_checkpoint_bytes(
                            bytes.clone(),
                        )
                    },
                )
            })
        }

        fn compare_and_swap<'a>(
            &'a self,
            expected_revision: Option<[u8; 32]>,
            replacement_checkpoint_bytes: Vec<u8>,
        ) -> ProviderIngestFutureV1<
            'a,
            Result<
                MusubiProviderAttestationJournalCasOutcomeV1,
                MusubiProviderAttestationJournalStoreErrorV1,
            >,
        > {
            Box::pin(async move {
                let replacement =
                    MusubiProviderAttestationJournalStoreSnapshotV1::from_checkpoint_bytes(
                        replacement_checkpoint_bytes,
                    )?;
                let replacement_revision = replacement
                    .revision()
                    .ok_or(MusubiProviderAttestationJournalStoreErrorV1::Rejected)?;
                let replacement_bytes = replacement
                    .checkpoint_bytes()
                    .ok_or(MusubiProviderAttestationJournalStoreErrorV1::Rejected)?;
                let mut checkpoint = self
                    .checkpoint
                    .lock()
                    .map_err(|_| MusubiProviderAttestationJournalStoreErrorV1::Unavailable)?;
                if checkpoint.as_deref() == Some(replacement_bytes) {
                    return Ok(MusubiProviderAttestationJournalCasOutcomeV1::Stored {
                        revision: replacement_revision,
                    });
                }
                let retained_revision = checkpoint
                    .as_ref()
                    .map(|bytes| musubi_provider_attestation_journal_checkpoint_revision_v1(bytes));
                if retained_revision != expected_revision {
                    return Ok(MusubiProviderAttestationJournalCasOutcomeV1::Conflict);
                }
                *checkpoint = Some(replacement_bytes.to_vec());
                Ok(MusubiProviderAttestationJournalCasOutcomeV1::Stored {
                    revision: replacement_revision,
                })
            })
        }
    }

    fn outbox_policy() -> ProviderIngestOutboxPolicyV1 {
        ProviderIngestOutboxPolicyV1 {
            max_active_entries: 32,
            max_terminal_entries: 32,
            max_attempts: 4,
            checkpoint_max_bytes: 16 * 1024 * 1024,
            checkpoint_operation_timeout_ms: 250,
            source_lease_ttl_ms: 20,
            retry_base_delay_ms: 10_000,
            retry_max_delay_ms: 100_000,
            terminal_retention_blocks: 100,
            max_signed_transaction_bytes: 128 * 1024,
            max_status_page_size: 32,
        }
    }

    fn runtime_policy() -> ProviderIngestRuntimePolicyV1 {
        ProviderIngestRuntimePolicyV1 {
            max_page_rows: 16,
            max_pages_per_tick: 2,
            max_source_jobs_per_tick: 4,
            max_source_providers: 4,
            scan_interval_ms: 10,
            source_operation_timeout_ms: 250,
            source_lease_renew_interval_ms: 5,
            signer_timeout_ms: 100,
            ingress_timeout_ms: 100,
        }
    }

    struct TestLedger {
        page: Mutex<ProviderIngestFinalizedAssignmentPageV1>,
    }

    impl ProviderIngestFinalizedLedgerV1 for TestLedger {
        fn read_assignment_page<'a>(
            &'a self,
            _claim_factory: ProviderIngestFinalizedClaimFactoryV1,
            at_finalized_cursor: Option<ProviderIngestFinalizedCursorV1>,
            after_order_id: Option<[u8; 32]>,
            _limit: usize,
        ) -> ProviderIngestFutureV1<
            'a,
            Result<ProviderIngestFinalizedAssignmentPageV1, ProviderIngestFinalizedLedgerErrorV1>,
        > {
            let page = self.page.lock().unwrap().clone();
            Box::pin(async move {
                if at_finalized_cursor.is_some_and(|cursor| cursor != page.finalized_cursor) {
                    return Err(ProviderIngestFinalizedLedgerErrorV1::Rejected);
                }
                if after_order_id.is_some() {
                    Ok(ProviderIngestFinalizedAssignmentPageV1 {
                        finalized_cursor: page.finalized_cursor,
                        finalized_block_time_ms: page.finalized_block_time_ms,
                        rows: Vec::new(),
                        next_after_order_id: None,
                    })
                } else {
                    Ok(page)
                }
            })
        }
    }

    fn fixture_completed_musubi_capture_row(
        order_seed: u8,
        commitment_seed: u8,
    ) -> ProviderIngestCompletedMusubiCaptureSourceRowV1 {
        let mut row = fixture_musubi_row(order_seed, commitment_seed);
        row.order.provider_completions.push(completion_record(
            ProviderId::new(LOCAL_PROVIDER),
            account(8),
            8,
        ));
        ProviderIngestCompletedMusubiCaptureSourceRowV1::from_projected_fields(
            row.pin,
            row.order,
            row.musubi_archive.map(|claim| claim.binding),
            row.provider_owner,
            row.completion_authority,
            row.completion_epoch,
            row.committed_transaction_hash,
        )
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum CaptureScannerLedgerFaultV1 {
        None,
        MalformedRow,
        SubstitutedArchiveBinding,
    }

    struct CaptureScannerLedgerV1 {
        rows: Vec<ProviderIngestCompletedMusubiCaptureSourceRowV1>,
        finalized_height: AtomicU64,
        fault: Mutex<CaptureScannerLedgerFaultV1>,
        requested_limits: Mutex<Vec<usize>>,
    }

    impl CaptureScannerLedgerV1 {
        fn new(
            rows: Vec<ProviderIngestCompletedMusubiCaptureSourceRowV1>,
            finalized_height: u64,
            fault: CaptureScannerLedgerFaultV1,
        ) -> Self {
            Self {
                rows,
                finalized_height: AtomicU64::new(finalized_height),
                fault: Mutex::new(fault),
                requested_limits: Mutex::new(Vec::new()),
            }
        }

        fn set_finalized_height(&self, height: u64) {
            self.finalized_height.store(height, Ordering::SeqCst);
        }

        fn set_fault(&self, fault: CaptureScannerLedgerFaultV1) {
            *self.fault.lock().unwrap() = fault;
        }

        fn requested_limits(&self) -> Vec<usize> {
            self.requested_limits.lock().unwrap().clone()
        }
    }

    impl ProviderIngestCompletedMusubiCaptureLedgerV1 for CaptureScannerLedgerV1 {
        fn read_completed_musubi_capture_page<'a>(
            &'a self,
            at_finalized_cursor: Option<ProviderIngestFinalizedCursorV1>,
            after_order_id: Option<[u8; 32]>,
            limit: usize,
        ) -> ProviderIngestFutureV1<
            'a,
            Result<
                ProviderIngestCompletedMusubiCaptureSourcePageV1,
                ProviderIngestFinalizedLedgerErrorV1,
            >,
        > {
            Box::pin(async move {
                self.requested_limits.lock().unwrap().push(limit);
                let finalized_cursor = at_finalized_cursor
                    .unwrap_or_else(|| cursor(self.finalized_height.load(Ordering::SeqCst)));
                let mut rows = self
                    .rows
                    .iter()
                    .filter(|row| {
                        after_order_id.is_none_or(|after| *row.order.order_id.as_bytes() > after)
                    })
                    .take(limit.saturating_add(1))
                    .cloned()
                    .collect::<Vec<_>>();
                let has_more = rows.len() > limit;
                rows.truncate(limit);
                for row in &mut rows {
                    row.pin.finalized_cursor = PinManifestFinalizedCursorV1 {
                        height: finalized_cursor.height,
                        block_hash: finalized_cursor.block_hash,
                    };
                }
                match *self.fault.lock().unwrap() {
                    CaptureScannerLedgerFaultV1::None => {}
                    CaptureScannerLedgerFaultV1::MalformedRow => {
                        if let Some(row) = rows.first_mut() {
                            row.pin.finalized_cursor.block_hash = [0xE1; 32];
                        }
                    }
                    CaptureScannerLedgerFaultV1::SubstitutedArchiveBinding => {
                        if let Some(binding) =
                            rows.first_mut().and_then(|row| row.musubi_archive.as_mut())
                        {
                            binding.replication_order = ReplicationOrderId::new([0xE2; 32]);
                        }
                    }
                }
                let next_after_order_id = has_more.then(|| {
                    *rows
                        .last()
                        .expect("a continued capture page has one row")
                        .order
                        .order_id
                        .as_bytes()
                });
                Ok(ProviderIngestCompletedMusubiCaptureSourcePageV1 {
                    chain_id: test_chain_id(),
                    provider_id: LOCAL_PROVIDER,
                    finalized_cursor,
                    finalized_block_time_ms: finalized_cursor.height.saturating_mul(1_000),
                    rows,
                    next_after_order_id,
                })
            })
        }
    }

    #[tokio::test]
    async fn completed_musubi_capture_scanner_pages_and_restarts_at_a_later_head() {
        let ledger = Arc::new(CaptureScannerLedgerV1::new(
            vec![
                fixture_completed_musubi_capture_row(0x39, 0x86),
                fixture_completed_musubi_capture_row(0x3A, 0x87),
            ],
            8,
            CaptureScannerLedgerFaultV1::None,
        ));
        let mut scanner = ProviderIngestCompletedMusubiCaptureScannerV1::new(
            LOCAL_PROVIDER,
            test_chain_id(),
            TEST_GENESIS_BLOCK_HASH,
            1,
            Arc::clone(&ledger),
        )
        .expect("construct bounded capture scanner");

        let first = scanner.next_page().await.expect("first capture page");
        assert_eq!(first.finalized_cursor(), cursor(8));
        assert_eq!(first.candidates().len(), 1);
        assert!(!first.scan_complete());
        assert!(
            first.candidates()[0]
                .completed_claim()
                .matches_authorization(first.candidates()[0].authorization())
        );

        let second = scanner.next_page().await.expect("second capture page");
        assert_eq!(second.finalized_cursor(), cursor(8));
        assert_eq!(second.candidates().len(), 1);
        assert!(second.scan_complete());
        assert_ne!(
            first.candidates()[0].completed_claim().replication_order(),
            second.candidates()[0].completed_claim().replication_order()
        );

        let unchanged = scanner
            .next_page()
            .await
            .expect("unchanged finalized head probe");
        assert_eq!(unchanged.finalized_cursor(), cursor(8));
        assert!(unchanged.candidates().is_empty());
        assert!(unchanged.scan_complete());

        ledger.set_fault(CaptureScannerLedgerFaultV1::SubstitutedArchiveBinding);
        assert!(matches!(
            scanner.next_page().await,
            Err(ProviderIngestRuntimeErrorV1::InvalidFinalizedBinding)
        ));
        ledger.set_fault(CaptureScannerLedgerFaultV1::None);

        ledger.set_finalized_height(9);
        let later = scanner
            .next_page()
            .await
            .expect("fresh capture scan at later head");
        assert_eq!(later.finalized_cursor(), cursor(9));
        assert_eq!(later.candidates().len(), 1);
        assert!(!later.scan_complete());
        assert_eq!(
            later.candidates()[0]
                .completed_claim()
                .observed_finalized_cursor(),
            cursor(9)
        );
        let later_terminal = scanner
            .next_page()
            .await
            .expect("terminal page at later head");
        assert_eq!(later_terminal.finalized_cursor(), cursor(9));
        assert_eq!(later_terminal.candidates().len(), 1);
        assert!(later_terminal.scan_complete());
        let later_unchanged = scanner
            .next_page()
            .await
            .expect("unchanged later-head probe");
        assert_eq!(later_unchanged.finalized_cursor(), cursor(9));
        assert!(later_unchanged.candidates().is_empty());
        assert!(later_unchanged.scan_complete());
        assert_eq!(ledger.requested_limits(), vec![1; 7]);
    }

    #[tokio::test]
    async fn completed_musubi_capture_scanner_rejects_malformed_and_substituted_raw_pages() {
        for fault in [
            CaptureScannerLedgerFaultV1::MalformedRow,
            CaptureScannerLedgerFaultV1::SubstitutedArchiveBinding,
        ] {
            let ledger = Arc::new(CaptureScannerLedgerV1::new(
                vec![fixture_completed_musubi_capture_row(0x3B, 0x88)],
                8,
                fault,
            ));
            let mut scanner = ProviderIngestCompletedMusubiCaptureScannerV1::new(
                LOCAL_PROVIDER,
                test_chain_id(),
                TEST_GENESIS_BLOCK_HASH,
                1,
                ledger,
            )
            .expect("construct capture scanner");
            assert!(matches!(
                scanner.next_page().await,
                Err(ProviderIngestRuntimeErrorV1::InvalidFinalizedBinding)
            ));
        }
    }

    #[tokio::test]
    async fn completed_musubi_capture_scanner_retries_a_repaired_page_without_cursor_drift() {
        let ledger = Arc::new(CaptureScannerLedgerV1::new(
            vec![fixture_completed_musubi_capture_row(0x3C, 0x89)],
            8,
            CaptureScannerLedgerFaultV1::MalformedRow,
        ));
        let mut scanner = ProviderIngestCompletedMusubiCaptureScannerV1::new(
            LOCAL_PROVIDER,
            test_chain_id(),
            TEST_GENESIS_BLOCK_HASH,
            1,
            Arc::clone(&ledger),
        )
        .expect("construct capture scanner");

        assert!(matches!(
            scanner.next_page().await,
            Err(ProviderIngestRuntimeErrorV1::InvalidFinalizedBinding)
        ));
        ledger.set_fault(CaptureScannerLedgerFaultV1::None);
        let repaired = scanner
            .next_page()
            .await
            .expect("retry exact repaired page");
        assert_eq!(repaired.finalized_cursor(), cursor(8));
        assert_eq!(repaired.candidates().len(), 1);
        assert!(repaired.scan_complete());
        assert_eq!(ledger.requested_limits(), vec![1, 1]);
    }

    #[tokio::test]
    async fn completed_musubi_capture_reconciliation_retries_without_skipping_and_enqueues_once() {
        let fixture = verified_attestation_bundle_fixture(0xEC);
        let manifest = completed_attestation_manifest(&fixture);
        let source_row = completed_attestation_capture_source_row(&fixture, &manifest);
        let ledger = Arc::new(CaptureScannerLedgerV1::new(
            vec![source_row],
            8,
            CaptureScannerLedgerFaultV1::None,
        ));
        let mut scanner = ProviderIngestCompletedMusubiCaptureScannerV1::new(
            LOCAL_PROVIDER,
            test_chain_id(),
            TEST_GENESIS_BLOCK_HASH,
            1,
            Arc::clone(&ledger),
        )
        .expect("construct reconciliation scanner");
        let initial_progress = scanner.progress();
        let temp_dir = tempfile::tempdir().expect("capture reconciliation tempdir");
        let handle = NodeHandle::try_new(
            StorageConfig::builder()
                .enabled(true)
                .data_dir(temp_dir.path().join("storage"))
                .build(),
        )
        .expect("open capture reconciliation storage");
        let journal = MusubiProviderAttestationJournalV1::new(
            Arc::new(CaptureJournalMemoryStore::default()),
            MusubiProviderAttestationJournalPolicyV1::default(),
        )
        .expect("open capture reconciliation journal");

        assert_eq!(
            handle
                .reconcile_provider_ingest_completed_musubi_capture_page(&mut scanner, &journal,)
                .await,
            Err(ProviderIngestCompletedMusubiReconcileErrorV1::AdmittedPlanUnavailable)
        );
        assert_eq!(
            scanner.progress(),
            initial_progress,
            "a failed page must restore its exact scanner continuation"
        );

        let mut payload = fixture.payload.as_slice();
        handle
            .ingest_manifest(&manifest, &fixture.plan, &mut payload)
            .expect("admit completed Musubi payload");
        let inserted = handle
            .reconcile_provider_ingest_completed_musubi_capture_page(&mut scanner, &journal)
            .await
            .expect("verify and enqueue completed Musubi page");
        assert_eq!(inserted.finalized_cursor, cursor(8));
        assert_eq!(inserted.candidates, 1);
        assert_eq!(inserted.inserted, 1);
        assert_eq!(inserted.existing, 0);
        assert!(inserted.scan_complete);

        scanner.restore_progress(initial_progress);
        let replayed = handle
            .reconcile_provider_ingest_completed_musubi_capture_page(&mut scanner, &journal)
            .await
            .expect("idempotently replay exact completed Musubi page");
        assert_eq!(replayed.candidates, 1);
        assert_eq!(replayed.inserted, 0);
        assert_eq!(replayed.existing, 1);
        assert!(replayed.scan_complete);
        assert_eq!(ledger.requested_limits(), vec![1, 1, 1]);
    }

    #[test]
    fn completed_musubi_capture_ledger_never_receives_claim_minting_capabilities() {
        let source = include_str!("provider_ingest_runtime.rs");
        let trait_start = source
            .find("pub trait ProviderIngestCompletedMusubiCaptureLedgerV1")
            .expect("capture ledger trait");
        let trait_tail = &source[trait_start..];
        let trait_end = trait_tail
            .find("\n}\n")
            .expect("capture ledger trait terminator");
        let trait_source = &trait_tail[..trait_end];
        assert!(trait_source.contains("ProviderIngestCompletedMusubiCaptureSourcePageV1"));
        assert!(!trait_source.contains("ProviderIngestFinalizedClaimFactoryV1"));
        assert!(!trait_source.contains("ProviderIngestFinalizedMusubiCompletionClaimV1"));
        assert!(!trait_source.contains("ProviderIngestFinalizedAssignmentPageV1"));
    }

    #[test]
    fn completed_musubi_capture_scanner_enforces_identity_and_page_bounds() {
        let ledger = Arc::new(CaptureScannerLedgerV1::new(
            Vec::new(),
            8,
            CaptureScannerLedgerFaultV1::None,
        ));
        for (provider_id, chain_id, genesis_block_hash, max_page_rows, expected) in [
            (
                [0; 32],
                test_chain_id(),
                TEST_GENESIS_BLOCK_HASH,
                1,
                "provider",
            ),
            (
                LOCAL_PROVIDER,
                ChainId::from(""),
                TEST_GENESIS_BLOCK_HASH,
                1,
                "chain",
            ),
            (LOCAL_PROVIDER, test_chain_id(), [0; 32], 1, "genesis"),
            (
                LOCAL_PROVIDER,
                test_chain_id(),
                TEST_GENESIS_BLOCK_HASH,
                0,
                "policy",
            ),
            (
                LOCAL_PROVIDER,
                test_chain_id(),
                TEST_GENESIS_BLOCK_HASH,
                PROVIDER_INGEST_STATUS_PAGE_MAX_V1 + 1,
                "policy",
            ),
        ] {
            let result = ProviderIngestCompletedMusubiCaptureScannerV1::new(
                provider_id,
                chain_id,
                genesis_block_hash,
                max_page_rows,
                Arc::clone(&ledger),
            );
            match expected {
                "provider" => assert!(matches!(
                    result,
                    Err(ProviderIngestRuntimeErrorV1::InvalidProviderId)
                )),
                "chain" => assert!(matches!(
                    result,
                    Err(ProviderIngestRuntimeErrorV1::InvalidChainId)
                )),
                "genesis" => assert!(matches!(
                    result,
                    Err(ProviderIngestRuntimeErrorV1::InvalidGenesisBlockHash)
                )),
                "policy" => assert!(matches!(
                    result,
                    Err(ProviderIngestRuntimeErrorV1::InvalidPolicy)
                )),
                _ => unreachable!("known expected capture-scanner error"),
            }
        }
    }

    struct TestFetch {
        result: Mutex<Result<Vec<u8>, ProviderIngestSourceFetchErrorV1>>,
        delay_ms: u64,
        calls: AtomicUsize,
    }

    impl ProviderIngestAuthenticatedSourceFetchV1 for TestFetch {
        type Fetched = Vec<u8>;

        fn fetch<'a>(
            &'a self,
            request: ProviderIngestSourceRequestV1,
        ) -> ProviderIngestFutureV1<'a, Result<Self::Fetched, ProviderIngestSourceFetchErrorV1>>
        {
            self.calls.fetch_add(1, Ordering::SeqCst);
            assert_eq!(request.source_provider_ids(), [SOURCE_PROVIDER]);
            let result = self.result.lock().unwrap().clone();
            let delay_ms = self.delay_ms;
            Box::pin(async move {
                if delay_ms != 0 {
                    tokio::time::sleep(Duration::from_millis(delay_ms)).await;
                }
                result
            })
        }
    }

    struct TestProviderSource {
        provider_id: [u8; 32],
        runtime_handle: &'static str,
        drifted_runtime_handle: &'static str,
        drift_after_fetch: bool,
        drifted: AtomicBool,
        qualification: Mutex<ProviderIngestSourceQualificationV1>,
        qualification_after_fetch: Mutex<Option<ProviderIngestSourceQualificationV1>>,
        readiness: Mutex<Result<(), ProviderIngestSourceFetchErrorV1>>,
        result: Mutex<Result<Vec<u8>, ProviderIngestSourceFetchErrorV1>>,
        calls: Arc<Mutex<Vec<[u8; 32]>>>,
        musubi_calls: Mutex<Vec<Option<ProviderIngestMusubiArchiveFetchBindingV1>>>,
    }

    impl ProviderIngestAuthenticatedProviderSourceV1 for TestProviderSource {
        type Fetched = Vec<u8>;

        fn provider_id(&self) -> [u8; 32] {
            self.provider_id
        }

        fn runtime_handle(&self) -> &str {
            if self.drifted.load(Ordering::SeqCst) {
                self.drifted_runtime_handle
            } else {
                self.runtime_handle
            }
        }

        fn qualification(
            &self,
        ) -> Result<ProviderIngestSourceQualificationV1, ProviderIngestSourceFetchErrorV1> {
            Ok(*self.qualification.lock().unwrap())
        }

        fn check_readiness(&self) -> Result<(), ProviderIngestSourceFetchErrorV1> {
            *self.readiness.lock().unwrap()
        }

        fn fetch_provider<'a>(
            &'a self,
            authorization: FinalizedProviderIngestAuthorizationV1,
            musubi_archive: Option<ProviderIngestMusubiArchiveFetchBindingV1>,
        ) -> ProviderIngestFutureV1<'a, Result<Self::Fetched, ProviderIngestSourceFetchErrorV1>>
        {
            assert_eq!(authorization.provider_id(), LOCAL_PROVIDER);
            self.calls.lock().unwrap().push(self.provider_id);
            self.musubi_calls.lock().unwrap().push(musubi_archive);
            let result = self.result.lock().unwrap().clone();
            let qualification_after_fetch = self.qualification_after_fetch.lock().unwrap().take();
            if let Some(qualification) = qualification_after_fetch {
                *self.qualification.lock().unwrap() = qualification;
            }
            if self.drift_after_fetch {
                self.drifted.store(true, Ordering::SeqCst);
            }
            Box::pin(async move { result })
        }
    }

    fn test_provider_source(
        provider_id: [u8; 32],
        runtime_handle: &'static str,
        readiness: Result<(), ProviderIngestSourceFetchErrorV1>,
        result: Result<Vec<u8>, ProviderIngestSourceFetchErrorV1>,
        drift_after_fetch: bool,
        calls: Arc<Mutex<Vec<[u8; 32]>>>,
    ) -> Arc<TestProviderSource> {
        Arc::new(TestProviderSource {
            provider_id,
            runtime_handle,
            drifted_runtime_handle: "https-pinned:provider-substituted",
            drift_after_fetch,
            drifted: AtomicBool::new(false),
            qualification: Mutex::new(ProviderIngestSourceQualificationV1::new(1, provider_id)),
            qualification_after_fetch: Mutex::new(None),
            readiness: Mutex::new(readiness),
            result: Mutex::new(result),
            calls,
            musubi_calls: Mutex::new(Vec::new()),
        })
    }

    fn test_source_binding(
        provider_id: [u8; 32],
        runtime_handle: impl Into<String>,
    ) -> ProviderIngestAuthenticatedSourceBindingV1 {
        ProviderIngestAuthenticatedSourceBindingV1 {
            provider_id,
            runtime_handle: runtime_handle.into(),
            revision: 1,
            policy_digest: provider_id,
        }
    }

    fn test_source_registration(
        source: Arc<TestProviderSource>,
        binding: ProviderIngestAuthenticatedSourceBindingV1,
    ) -> ProviderIngestAuthenticatedSourceRegistrationV1<Vec<u8>> {
        let source: Arc<dyn ProviderIngestAuthenticatedProviderSourceV1<Fetched = Vec<u8>>> =
            source;
        ProviderIngestAuthenticatedSourceRegistrationV1::new(binding, source)
    }

    fn test_source_pool(
        sources: Vec<Arc<TestProviderSource>>,
    ) -> Result<
        ProviderIngestAuthenticatedSourcePoolV1<Vec<u8>>,
        ProviderIngestAuthenticatedSourcePoolErrorV1,
    > {
        let sources = sources
            .into_iter()
            .map(|source| {
                let binding = test_source_binding(source.provider_id, source.runtime_handle);
                test_source_registration(source, binding)
            })
            .collect();
        ProviderIngestAuthenticatedSourcePoolV1::new(
            "https-pinned-source-pool:region-a",
            ProviderIngestRuntimeProviderQualificationV1::new(9, [0xA9; 32]),
            4,
            sources,
        )
    }

    fn test_source_request_result(
        source_provider_ids: Vec<[u8; 32]>,
    ) -> Result<ProviderIngestSourceRequestV1, ProviderIngestSourceFetchErrorV1> {
        let row = fixture_row(0x31);
        let validated =
            validate_assignment(&row, cursor(8), LOCAL_PROVIDER, runtime_policy()).unwrap();
        ProviderIngestSourceRequestV1::new(validated.authorization, source_provider_ids, None)
    }

    fn test_source_request(source_provider_ids: Vec<[u8; 32]>) -> ProviderIngestSourceRequestV1 {
        test_source_request_result(source_provider_ids).expect("valid test source request")
    }

    #[test]
    fn authenticated_source_qualification_rejects_unsupported_or_zero_pins() {
        let valid = ProviderIngestSourceQualificationV1::new(1, [0x22; 32]);
        assert_eq!(valid.validate(), Ok(()));

        let mut unsupported = valid;
        unsupported.version = 2;
        for invalid in [
            unsupported,
            ProviderIngestSourceQualificationV1::new(0, [0x22; 32]),
            ProviderIngestSourceQualificationV1::new(1, [0; 32]),
        ] {
            assert_eq!(
                invalid.validate(),
                Err(ProviderIngestAuthenticatedSourcePoolErrorV1::InvalidSourceQualification)
            );
        }
    }

    #[test]
    fn runtime_provider_qualification_requires_both_public_pins() {
        assert!(ProviderIngestRuntimeProviderQualificationV1::new(9, [0xA9; 32]).is_valid());
        assert!(!ProviderIngestRuntimeProviderQualificationV1::new(0, [0xA9; 32]).is_valid());
        assert!(!ProviderIngestRuntimeProviderQualificationV1::new(9, [0; 32]).is_valid());
    }

    #[test]
    fn authenticated_source_pool_rejects_incomplete_duplicate_and_test_marked_inventory() {
        let calls = Arc::new(Mutex::new(Vec::new()));
        let source_a = test_provider_source(
            [0x22; 32],
            "https-pinned:provider-a",
            Ok(()),
            Err(ProviderIngestSourceFetchErrorV1::Unavailable),
            false,
            Arc::clone(&calls),
        );
        assert_eq!(
            test_source_pool(vec![Arc::clone(&source_a)]).unwrap_err(),
            ProviderIngestAuthenticatedSourcePoolErrorV1::InvalidSourceCount
        );

        let duplicate_provider = test_provider_source(
            [0x22; 32],
            "https-pinned:provider-b",
            Ok(()),
            Err(ProviderIngestSourceFetchErrorV1::Unavailable),
            false,
            Arc::clone(&calls),
        );
        assert_eq!(
            test_source_pool(vec![Arc::clone(&source_a), duplicate_provider]).unwrap_err(),
            ProviderIngestAuthenticatedSourcePoolErrorV1::DuplicateProvider
        );

        let duplicate_handle = test_provider_source(
            [0x33; 32],
            "https-pinned:provider-a",
            Ok(()),
            Err(ProviderIngestSourceFetchErrorV1::Unavailable),
            false,
            Arc::clone(&calls),
        );
        assert_eq!(
            test_source_pool(vec![Arc::clone(&source_a), duplicate_handle]).unwrap_err(),
            ProviderIngestAuthenticatedSourcePoolErrorV1::DuplicateSourceHandle
        );

        let credential_handle = test_provider_source(
            [0x33; 32],
            "https://operator:secret@provider.example",
            Ok(()),
            Err(ProviderIngestSourceFetchErrorV1::Unavailable),
            false,
            Arc::clone(&calls),
        );
        assert_eq!(
            test_source_pool(vec![Arc::clone(&source_a), credential_handle]).unwrap_err(),
            ProviderIngestAuthenticatedSourcePoolErrorV1::InvalidSourceHandle
        );

        let test_marked = test_provider_source(
            [0x33; 32],
            "https-pinned:provider-test",
            Ok(()),
            Err(ProviderIngestSourceFetchErrorV1::Unavailable),
            false,
            calls,
        );
        assert_eq!(
            test_source_pool(vec![source_a, test_marked]).unwrap_err(),
            ProviderIngestAuthenticatedSourcePoolErrorV1::InvalidSourceHandle
        );
    }

    #[test]
    fn authenticated_source_pool_requires_independent_valid_qualification_pins() {
        let calls = Arc::new(Mutex::new(Vec::new()));
        let source_a = test_provider_source(
            [0x22; 32],
            "https-pinned:provider-a",
            Ok(()),
            Err(ProviderIngestSourceFetchErrorV1::Unavailable),
            false,
            Arc::clone(&calls),
        );
        let source_b = test_provider_source(
            [0x33; 32],
            "https-pinned:provider-b",
            Ok(()),
            Err(ProviderIngestSourceFetchErrorV1::Unavailable),
            false,
            calls,
        );

        for invalid_pool_qualification in [
            ProviderIngestRuntimeProviderQualificationV1::new(0, [0xA9; 32]),
            ProviderIngestRuntimeProviderQualificationV1::new(9, [0; 32]),
        ] {
            assert_eq!(
                ProviderIngestAuthenticatedSourcePoolV1::new(
                    "https-pinned-source-pool:region-a",
                    invalid_pool_qualification,
                    4,
                    vec![
                        test_source_registration(
                            Arc::clone(&source_a),
                            test_source_binding(source_a.provider_id, source_a.runtime_handle),
                        ),
                        test_source_registration(
                            Arc::clone(&source_b),
                            test_source_binding(source_b.provider_id, source_b.runtime_handle),
                        ),
                    ],
                )
                .unwrap_err(),
                ProviderIngestAuthenticatedSourcePoolErrorV1::InvalidPoolQualification
            );
        }

        let mut invalid_binding =
            test_source_binding(source_b.provider_id, source_b.runtime_handle);
        invalid_binding.revision = 0;
        assert_eq!(
            ProviderIngestAuthenticatedSourcePoolV1::new(
                "https-pinned-source-pool:region-a",
                ProviderIngestRuntimeProviderQualificationV1::new(9, [0xA9; 32]),
                4,
                vec![
                    test_source_registration(
                        Arc::clone(&source_a),
                        test_source_binding(source_a.provider_id, source_a.runtime_handle),
                    ),
                    test_source_registration(Arc::clone(&source_b), invalid_binding),
                ],
            )
            .unwrap_err(),
            ProviderIngestAuthenticatedSourcePoolErrorV1::InvalidSourceQualification
        );

        let mut substituted_binding =
            test_source_binding(source_b.provider_id, source_b.runtime_handle);
        substituted_binding.revision = 2;
        assert_eq!(
            ProviderIngestAuthenticatedSourcePoolV1::new(
                "https-pinned-source-pool:region-a",
                ProviderIngestRuntimeProviderQualificationV1::new(9, [0xA9; 32]),
                4,
                vec![
                    test_source_registration(
                        Arc::clone(&source_a),
                        test_source_binding(source_a.provider_id, source_a.runtime_handle),
                    ),
                    test_source_registration(source_b, substituted_binding),
                ],
            )
            .unwrap_err(),
            ProviderIngestAuthenticatedSourcePoolErrorV1::SourceBindingMismatch
        );
    }

    #[test]
    fn authenticated_source_pool_rejects_qualification_drift_at_readiness() {
        let calls = Arc::new(Mutex::new(Vec::new()));
        let source_a = test_provider_source(
            [0x22; 32],
            "https-pinned:provider-a",
            Ok(()),
            Err(ProviderIngestSourceFetchErrorV1::Unavailable),
            false,
            Arc::clone(&calls),
        );
        let source_b = test_provider_source(
            [0x33; 32],
            "https-pinned:provider-b",
            Ok(()),
            Err(ProviderIngestSourceFetchErrorV1::Unavailable),
            false,
            Arc::clone(&calls),
        );
        let pool = test_source_pool(vec![Arc::clone(&source_a), source_b]).unwrap();
        *source_a.qualification.lock().unwrap() =
            ProviderIngestSourceQualificationV1::new(2, [0x22; 32]);

        assert_eq!(
            pool.check_readiness(),
            Err(ProviderIngestSourceFetchErrorV1::Rejected)
        );
        assert!(calls.lock().unwrap().is_empty());
    }

    #[test]
    fn authenticated_source_pool_is_ready_when_one_qualified_source_is_available() {
        let calls = Arc::new(Mutex::new(Vec::new()));
        let source_a = test_provider_source(
            [0x22; 32],
            "https-pinned:provider-a",
            Err(ProviderIngestSourceFetchErrorV1::Unavailable),
            Err(ProviderIngestSourceFetchErrorV1::Unavailable),
            false,
            Arc::clone(&calls),
        );
        let source_b = test_provider_source(
            [0x33; 32],
            "https-pinned:provider-b",
            Ok(()),
            Err(ProviderIngestSourceFetchErrorV1::Unavailable),
            false,
            calls,
        );
        let pool = test_source_pool(vec![source_a, source_b]).unwrap();

        assert_eq!(pool.check_readiness(), Ok(()));
    }

    #[tokio::test]
    async fn authenticated_source_pool_fails_over_in_canonical_provider_order() {
        let calls = Arc::new(Mutex::new(Vec::new()));
        let source_a = test_provider_source(
            [0x22; 32],
            "https-pinned:provider-a",
            Ok(()),
            Err(ProviderIngestSourceFetchErrorV1::Unavailable),
            false,
            Arc::clone(&calls),
        );
        let source_b = test_provider_source(
            [0x33; 32],
            "https-pinned:provider-b",
            Ok(()),
            Ok(vec![0xA5]),
            false,
            Arc::clone(&calls),
        );
        let pool = test_source_pool(vec![source_a, source_b]).unwrap();

        assert_eq!(pool.runtime_handle(), "https-pinned-source-pool:region-a");
        assert_eq!(pool.source_provider_ids(), &[[0x22; 32], [0x33; 32]]);
        assert_eq!(pool.max_sources_per_fetch(), 4);
        assert!(pool.check_readiness().is_ok());
        assert_eq!(
            pool.fetch(test_source_request(vec![[0x22; 32], [0x33; 32]]))
                .await,
            Ok(vec![0xA5])
        );
        assert_eq!(*calls.lock().unwrap(), vec![[0x22; 32], [0x33; 32]]);
    }

    #[tokio::test]
    async fn authenticated_source_pool_preserves_exact_musubi_fetch_binding() {
        let calls = Arc::new(Mutex::new(Vec::new()));
        let source = test_provider_source(
            SOURCE_PROVIDER,
            "https-pinned:provider-musubi",
            Ok(()),
            Ok(vec![0xA5]),
            false,
            Arc::clone(&calls),
        );
        let fallback = test_provider_source(
            [0x33; 32],
            "https-pinned:provider-musubi-fallback",
            Ok(()),
            Err(ProviderIngestSourceFetchErrorV1::Unavailable),
            false,
            Arc::clone(&calls),
        );
        let pool = test_source_pool(vec![Arc::clone(&source), fallback]).unwrap();
        let row = fixture_musubi_row(0x68, 0xB4);
        let validated =
            validate_assignment(&row, cursor(8), LOCAL_PROVIDER, runtime_policy()).unwrap();
        let musubi_archive = ProviderIngestMusubiArchiveFetchBindingV1::from_finalized_claim(
            row.musubi_archive.as_ref().unwrap(),
        );
        let request = ProviderIngestSourceRequestV1::new(
            validated.authorization,
            vec![SOURCE_PROVIDER],
            Some(musubi_archive.clone()),
        )
        .unwrap();

        assert_eq!(pool.fetch(request).await, Ok(vec![0xA5]));
        assert_eq!(*calls.lock().unwrap(), vec![SOURCE_PROVIDER]);
        assert_eq!(
            *source.musubi_calls.lock().unwrap(),
            vec![Some(musubi_archive)]
        );
    }

    #[tokio::test]
    async fn authenticated_source_pool_fails_over_after_content_rejection() {
        let calls = Arc::new(Mutex::new(Vec::new()));
        let source_a = test_provider_source(
            [0x22; 32],
            "https-pinned:provider-a",
            Ok(()),
            Err(ProviderIngestSourceFetchErrorV1::ContentRejected),
            false,
            Arc::clone(&calls),
        );
        let source_b = test_provider_source(
            [0x33; 32],
            "https-pinned:provider-b",
            Ok(()),
            Ok(vec![0xA5]),
            false,
            Arc::clone(&calls),
        );
        let pool = test_source_pool(vec![source_a, source_b]).unwrap();

        assert_eq!(
            pool.fetch(test_source_request(vec![[0x22; 32], [0x33; 32]]))
                .await,
            Ok(vec![0xA5])
        );
        assert_eq!(*calls.lock().unwrap(), vec![[0x22; 32], [0x33; 32]]);
    }

    #[tokio::test]
    async fn authenticated_source_pool_rejects_noncanonical_or_unpinned_requests_before_io() {
        for source_provider_ids in [
            vec![[0x33; 32], [0x22; 32]],
            vec![[0x22; 32], [0x44; 32]],
            vec![LOCAL_PROVIDER, [0x22; 32]],
        ] {
            let calls = Arc::new(Mutex::new(Vec::new()));
            let source_a = test_provider_source(
                [0x22; 32],
                "https-pinned:provider-a",
                Ok(()),
                Err(ProviderIngestSourceFetchErrorV1::Unavailable),
                false,
                Arc::clone(&calls),
            );
            let source_b = test_provider_source(
                [0x33; 32],
                "https-pinned:provider-b",
                Ok(()),
                Err(ProviderIngestSourceFetchErrorV1::Unavailable),
                false,
                Arc::clone(&calls),
            );
            let pool = test_source_pool(vec![source_a, source_b]).unwrap();

            match test_source_request_result(source_provider_ids) {
                Ok(request) => assert_eq!(
                    pool.fetch(request).await,
                    Err(ProviderIngestSourceFetchErrorV1::Rejected)
                ),
                Err(error) => assert_eq!(error, ProviderIngestSourceFetchErrorV1::Rejected),
            }
            assert!(calls.lock().unwrap().is_empty());
        }
    }

    #[tokio::test]
    async fn authenticated_source_pool_does_not_mask_identity_drift_with_later_success() {
        let calls = Arc::new(Mutex::new(Vec::new()));
        let source_a = test_provider_source(
            [0x22; 32],
            "https-pinned:provider-a",
            Ok(()),
            Ok(vec![0xA5]),
            true,
            Arc::clone(&calls),
        );
        let source_b = test_provider_source(
            [0x33; 32],
            "https-pinned:provider-b",
            Ok(()),
            Ok(vec![0xB6]),
            false,
            Arc::clone(&calls),
        );
        let pool = test_source_pool(vec![source_a, source_b]).unwrap();

        assert_eq!(
            pool.fetch(test_source_request(vec![[0x22; 32], [0x33; 32]]))
                .await,
            Err(ProviderIngestSourceFetchErrorV1::Rejected)
        );
        assert_eq!(*calls.lock().unwrap(), vec![[0x22; 32]]);
    }

    #[tokio::test]
    async fn authenticated_source_pool_does_not_mask_qualification_drift_with_later_success() {
        let calls = Arc::new(Mutex::new(Vec::new()));
        let source_a = test_provider_source(
            [0x22; 32],
            "https-pinned:provider-a",
            Ok(()),
            Ok(vec![0xA5]),
            false,
            Arc::clone(&calls),
        );
        *source_a.qualification_after_fetch.lock().unwrap() =
            Some(ProviderIngestSourceQualificationV1::new(2, [0x22; 32]));
        let source_b = test_provider_source(
            [0x33; 32],
            "https-pinned:provider-b",
            Ok(()),
            Ok(vec![0xB6]),
            false,
            Arc::clone(&calls),
        );
        let pool = test_source_pool(vec![source_a, source_b]).unwrap();

        assert_eq!(
            pool.fetch(test_source_request(vec![[0x22; 32], [0x33; 32]]))
                .await,
            Err(ProviderIngestSourceFetchErrorV1::Rejected)
        );
        assert_eq!(*calls.lock().unwrap(), vec![[0x22; 32]]);
    }

    struct TestStorage {
        existing: AtomicBool,
    }

    impl ProviderIngestLocalStorageV1<Vec<u8>> for TestStorage {
        fn verify_existing<'a>(
            &'a self,
            authorization: FinalizedProviderIngestAuthorizationV1,
            musubi_archive: Option<ProviderIngestFinalizedMusubiArchiveClaimV1>,
        ) -> ProviderIngestFutureV1<
            'a,
            Result<Option<ProviderIngestLocalStoredV1>, ProviderIngestLocalStorageErrorV1>,
        > {
            let existing = self.existing.load(Ordering::SeqCst);
            Box::pin(async move {
                if musubi_archive.is_some() {
                    return Err(ProviderIngestLocalStorageErrorV1::Permanent);
                }
                Ok(existing.then(|| {
                    ProviderIngestLocalStoredV1::generic(hex::encode(
                        authorization.manifest_digest(),
                    ))
                }))
            })
        }

        fn store<'a>(
            &'a self,
            authorization: FinalizedProviderIngestAuthorizationV1,
            musubi_archive: Option<ProviderIngestFinalizedMusubiArchiveClaimV1>,
            fetched: Vec<u8>,
        ) -> ProviderIngestFutureV1<
            'a,
            Result<ProviderIngestLocalStoredV1, ProviderIngestLocalStorageErrorV1>,
        > {
            Box::pin(async move {
                if fetched != vec![0xA5] || musubi_archive.is_some() {
                    return Err(ProviderIngestLocalStorageErrorV1::Retryable);
                }
                if authorization.order_id() == [0x3E; 32] {
                    tokio::time::sleep(Duration::from_millis(300)).await;
                }
                Ok(ProviderIngestLocalStoredV1::generic(hex::encode(
                    authorization.manifest_digest(),
                )))
            })
        }
    }

    struct TestPayloadBuilder;

    impl ProviderIngestCompletionPayloadBuilderV1 for TestPayloadBuilder {
        fn build_payload<'a>(
            &'a self,
            request: ProviderIngestCompletionPayloadRequestV1,
        ) -> ProviderIngestFutureV1<
            'a,
            Result<TransactionPayload, ProviderIngestCompletionPayloadErrorV1>,
        > {
            Box::pin(async move {
                if request.authorization.order_id() == [0x3B; 32] {
                    return Err(ProviderIngestCompletionPayloadErrorV1::Rejected);
                }
                let mut builder = TransactionBuilder::new(
                    request.network_id,
                    request.provider_owner,
                    FeePaymentIntent::authority(Vec::new(), None),
                )
                .with_instructions([InstructionBox::from(
                    CompleteReplicationOrder {
                        order_id: ReplicationOrderId::new(request.authorization.order_id()),
                        provider_id: ProviderId::new(request.authorization.provider_id()),
                        completion_epoch: request.completion_epoch,
                        expected_authority: request.expected_authority,
                        expected_assignment_revision: request.expected_assignment_revision,
                        finalized_anchor: ProviderIngestFinalizedAnchorV1 {
                            height: request.finalized_cursor.height,
                            block_hash: request.finalized_cursor.block_hash,
                        },
                    },
                )]);
                builder.set_creation_time(Duration::from_millis(1_000));
                builder.set_ttl(Duration::from_secs(30));
                builder
                    .into_payload()
                    .map_err(|_| ProviderIngestCompletionPayloadErrorV1::Rejected)
            })
        }
    }

    struct TestSigner {
        key: KeyPair,
        authority: AccountId,
        signer_policy_revision: Arc<AtomicU64>,
        eligibility_flip_on_call: usize,
        eligibility_flip_to_revision: u64,
        eligibility_calls: AtomicUsize,
    }

    impl ProviderIngestCompletionSignerV1 for TestSigner {
        fn runtime_handle(&self) -> &str {
            "pkcs11:sorafs-provider-ingest-unit"
        }

        fn authority(&self) -> &AccountId {
            &self.authority
        }

        fn qualification(
            &self,
        ) -> Result<
            ProviderIngestCompletionSignerQualificationV1,
            ProviderIngestCompletionSignerErrorV1,
        > {
            Ok(ProviderIngestCompletionSignerQualificationV1::new(
                1,
                self.signer_policy(),
                self.key.public_key().algorithm(),
                self.key.public_key().clone(),
            ))
        }

        fn signer_policy(&self) -> ProviderIngestCompletionSignerPolicyV1 {
            completion_signer_policy(self.signer_policy_revision.load(Ordering::SeqCst))
        }

        fn current_eligibility(
            &self,
        ) -> Result<ProviderIngestCompletionSignerPolicyV1, ProviderIngestCompletionSignerErrorV1>
        {
            let call = self
                .eligibility_calls
                .fetch_add(1, Ordering::SeqCst)
                .saturating_add(1);
            if self.eligibility_flip_on_call != 0 && call == self.eligibility_flip_on_call {
                self.signer_policy_revision
                    .store(self.eligibility_flip_to_revision, Ordering::SeqCst);
            }
            let signer_policy = self.signer_policy();
            if signer_policy.is_valid() {
                Ok(signer_policy)
            } else {
                Err(ProviderIngestCompletionSignerErrorV1::Rejected)
            }
        }

        fn sign<'a>(
            &'a self,
            payload: TransactionPayload,
        ) -> ProviderIngestFutureV1<
            'a,
            Result<SignedTransaction, ProviderIngestCompletionSignerErrorV1>,
        > {
            Box::pin(async move {
                TransactionBuilder::from_payload(payload)
                    .and_then(|builder| builder.try_sign(self.key.private_key()))
                    .map_err(|_| ProviderIngestCompletionSignerErrorV1::Rejected)
            })
        }
    }

    struct TestResolver {
        wrong_authority: AtomicBool,
        signer_policy_revision: Arc<AtomicU64>,
        eligibility_flip_on_call: AtomicUsize,
        eligibility_flip_to_revision: AtomicU64,
    }

    impl ProviderIngestCompletionSignerResolverV1 for TestResolver {
        type Signer = TestSigner;

        fn resolve<'a>(
            &'a self,
            _context: ProviderIngestCompletionSignerResolutionContextV1,
        ) -> ProviderIngestFutureV1<
            'a,
            Result<Option<Self::Signer>, ProviderIngestCompletionSignerResolverErrorV1>,
        > {
            let seed = if self.wrong_authority.load(Ordering::SeqCst) {
                9
            } else {
                8
            };
            let signer_policy_revision = Arc::clone(&self.signer_policy_revision);
            let eligibility_flip_on_call = self.eligibility_flip_on_call.load(Ordering::SeqCst);
            let eligibility_flip_to_revision =
                self.eligibility_flip_to_revision.load(Ordering::SeqCst);
            Box::pin(async move {
                let key = KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519).expect("key");
                let authority = AccountId::new(key.public_key().clone());
                Ok(Some(TestSigner {
                    key,
                    authority,
                    signer_policy_revision,
                    eligibility_flip_on_call,
                    eligibility_flip_to_revision,
                    eligibility_calls: AtomicUsize::new(0),
                }))
            })
        }
    }

    struct TestIngress {
        outbox: ProviderIngestOutbox,
        job_id: [u8; 32],
        prepare_error: Mutex<Option<ProviderIngestIngressPrepareErrorV1>>,
        disposition: Mutex<ProviderIngestIngressDispositionV1>,
        observation: Mutex<ProviderIngestTransactionObservationV1>,
        observe_calls: AtomicUsize,
        events: Mutex<Vec<&'static str>>,
    }

    impl ProviderIngestTransactionIngressV1 for TestIngress {
        type Prepared = SignedTransaction;

        fn prepare<'a>(
            &'a self,
            transaction: SignedTransaction,
        ) -> ProviderIngestFutureV1<'a, Result<Self::Prepared, ProviderIngestIngressPrepareErrorV1>>
        {
            let state = self.outbox.status(self.job_id).unwrap().state;
            assert!(matches!(
                state,
                ProviderIngestDeliveryStateV1::LocalStored {
                    completion: ProviderIngestCompletionStateV1::Signed { .. },
                    ..
                }
            ));
            self.events.lock().unwrap().push("prepare_signed");
            let error = *self.prepare_error.lock().unwrap();
            Box::pin(async move {
                if let Some(error) = error {
                    Err(error)
                } else {
                    Ok(transaction)
                }
            })
        }

        fn expose<'a>(
            &'a self,
            prepared: Self::Prepared,
            transaction: SignedTransaction,
        ) -> ProviderIngestFutureV1<'a, ProviderIngestIngressDispositionV1> {
            assert_eq!(prepared, transaction);
            let state = self.outbox.status(self.job_id).unwrap().state;
            assert!(matches!(
                state,
                ProviderIngestDeliveryStateV1::LocalStored {
                    completion: ProviderIngestCompletionStateV1::Ambiguous { .. },
                    ..
                }
            ));
            self.events.lock().unwrap().push("expose_ambiguous");
            let disposition = *self.disposition.lock().unwrap();
            Box::pin(async move { disposition })
        }

        fn observe<'a>(
            &'a self,
            _transaction_hash: [u8; 32],
        ) -> ProviderIngestFutureV1<'a, ProviderIngestTransactionObservationV1> {
            self.observe_calls.fetch_add(1, Ordering::SeqCst);
            let observation = *self.observation.lock().unwrap();
            Box::pin(async move { observation })
        }
    }

    struct TestClock {
        start: Instant,
        base_ms: AtomicU64,
    }

    impl ProviderIngestClockV1 for TestClock {
        fn now_ms(&self) -> u64 {
            self.base_ms
                .load(Ordering::SeqCst)
                .saturating_add(u64::try_from(self.start.elapsed().as_millis()).unwrap_or(u64::MAX))
        }
    }

    type TestRuntime = ProviderIngestRuntimeV1<
        TestLedger,
        TestFetch,
        TestStorage,
        TestPayloadBuilder,
        TestResolver,
        TestIngress,
        TestClock,
    >;

    fn test_runtime_with_chain_and_genesis(
        row: ProviderIngestFinalizedAssignmentV1,
        existing: bool,
        fetch_result: Result<Vec<u8>, ProviderIngestSourceFetchErrorV1>,
        fetch_delay_ms: u64,
        disposition: ProviderIngestIngressDispositionV1,
        wrong_signer: bool,
        chain_id: ChainId,
        genesis_block_hash: [u8; 32],
    ) -> Result<
        (
            TestRuntime,
            Arc<TestLedger>,
            Arc<TestFetch>,
            Arc<TestIngress>,
        ),
        ProviderIngestRuntimeErrorV1,
    > {
        let page = fixture_page(row.clone());
        let finalized_cursor = page.finalized_cursor;
        let ledger = Arc::new(TestLedger {
            page: Mutex::new(page),
        });
        let fetch = Arc::new(TestFetch {
            result: Mutex::new(fetch_result),
            delay_ms: fetch_delay_ms,
            calls: AtomicUsize::new(0),
        });
        let storage = Arc::new(TestStorage {
            existing: AtomicBool::new(existing),
        });
        let outbox = ProviderIngestOutbox::in_memory(outbox_policy()).expect("outbox");
        let validated =
            validate_assignment(&row, finalized_cursor, LOCAL_PROVIDER, runtime_policy()).unwrap();
        let ingress = Arc::new(TestIngress {
            outbox: outbox.clone(),
            job_id: validated.authorization.job_id(),
            prepare_error: Mutex::new(None),
            disposition: Mutex::new(disposition),
            observation: Mutex::new(ProviderIngestTransactionObservationV1::Unavailable),
            observe_calls: AtomicUsize::new(0),
            events: Mutex::new(Vec::new()),
        });
        let runtime = ProviderIngestRuntimeV1::new(
            LOCAL_PROVIDER,
            chain_id,
            genesis_block_hash,
            ProviderIngestClaimOwnerV1::new([0xCC; 32]).unwrap(),
            runtime_policy(),
            outbox,
            ledger.clone(),
            fetch.clone(),
            storage,
            Arc::new(TestPayloadBuilder),
            Arc::new(TestResolver {
                wrong_authority: AtomicBool::new(wrong_signer),
                signer_policy_revision: Arc::new(AtomicU64::new(1)),
                eligibility_flip_on_call: AtomicUsize::new(0),
                eligibility_flip_to_revision: AtomicU64::new(0),
            }),
            ingress.clone(),
            Arc::new(TestClock {
                start: Instant::now(),
                base_ms: AtomicU64::new(1_000),
            }),
        )?;
        Ok((runtime, ledger, fetch, ingress))
    }

    fn test_runtime(
        row: ProviderIngestFinalizedAssignmentV1,
        existing: bool,
        fetch_result: Result<Vec<u8>, ProviderIngestSourceFetchErrorV1>,
        fetch_delay_ms: u64,
        disposition: ProviderIngestIngressDispositionV1,
        wrong_signer: bool,
    ) -> (
        TestRuntime,
        Arc<TestLedger>,
        Arc<TestFetch>,
        Arc<TestIngress>,
    ) {
        test_runtime_with_chain_and_genesis(
            row,
            existing,
            fetch_result,
            fetch_delay_ms,
            disposition,
            wrong_signer,
            test_chain_id(),
            TEST_GENESIS_BLOCK_HASH,
        )
        .expect("runtime")
    }

    #[test]
    fn runtime_requires_bounded_chain_and_nonzero_genesis_and_preserves_them_exactly() {
        let row = fixture_row(0x30);
        assert!(matches!(
            test_runtime_with_chain_and_genesis(
                row.clone(),
                true,
                Ok(vec![0xA5]),
                0,
                ProviderIngestIngressDispositionV1::Submitted,
                false,
                test_chain_id(),
                [0; 32],
            ),
            Err(ProviderIngestRuntimeErrorV1::InvalidGenesisBlockHash)
        ));
        assert!(matches!(
            test_runtime_with_chain_and_genesis(
                row.clone(),
                true,
                Ok(vec![0xA5]),
                0,
                ProviderIngestIngressDispositionV1::Submitted,
                false,
                ChainId::from(""),
                TEST_GENESIS_BLOCK_HASH,
            ),
            Err(ProviderIngestRuntimeErrorV1::InvalidChainId)
        ));

        let (runtime, _, _, _) = test_runtime(
            row,
            true,
            Ok(vec![0xA5]),
            0,
            ProviderIngestIngressDispositionV1::Submitted,
            false,
        );
        assert_eq!(runtime.chain_id, test_chain_id());
        assert_eq!(runtime.genesis_block_hash, TEST_GENESIS_BLOCK_HASH);
    }

    #[test]
    fn finalized_page_rejects_cursor_order_and_pagination_substitution() {
        let row = fixture_row(0x31);
        let page = fixture_page(row.clone());
        validate_page(&page, None, cursor(8), 16).expect("valid page");

        let mut wrong_cursor = page.clone();
        wrong_cursor.finalized_cursor = cursor(9);
        assert!(matches!(
            validate_page(&wrong_cursor, None, cursor(8), 16),
            Err(ProviderIngestRuntimeErrorV1::InvalidFinalizedPage)
        ));

        let mut duplicate = page.clone();
        duplicate.rows.push(row);
        assert!(matches!(
            validate_page(&duplicate, None, cursor(8), 16),
            Err(ProviderIngestRuntimeErrorV1::InvalidFinalizedPage)
        ));

        let mut forged_next = page;
        forged_next.next_after_order_id = Some([0xFF; 32]);
        assert!(matches!(
            validate_page(&forged_next, None, cursor(8), 16),
            Err(ProviderIngestRuntimeErrorV1::InvalidFinalizedPage)
        ));
    }

    #[test]
    fn finalized_cursor_and_order_lifecycle_fail_closed_on_substitution() {
        assert!(validate_monotonic_finalized_cursor(None, cursor(8)).is_ok());
        assert!(validate_monotonic_finalized_cursor(Some(cursor(8)), cursor(8)).is_ok());
        assert!(matches!(
            validate_monotonic_finalized_cursor(Some(cursor(8)), cursor(7)),
            Err(ProviderIngestRuntimeErrorV1::InvalidFinalizedPage)
        ));
        let fork = ProviderIngestFinalizedCursorV1 {
            height: 8,
            block_hash: [0xFE; 32],
        };
        assert!(matches!(
            validate_monotonic_finalized_cursor(Some(cursor(8)), fork),
            Err(ProviderIngestRuntimeErrorV1::InvalidFinalizedPage)
        ));

        let mut unassigned_completion = fixture_row(0x30);
        unassigned_completion
            .order
            .provider_completions
            .push(completion_record(
                ProviderId::new([0x99; 32]),
                account(9),
                8,
            ));
        assert!(matches!(
            validate_assignment(
                &unassigned_completion,
                cursor(8),
                LOCAL_PROVIDER,
                runtime_policy(),
            ),
            Err(ProviderIngestRuntimeErrorV1::InvalidFinalizedBinding)
        ));

        let mut inconsistent_status = fixture_row(0x31);
        inconsistent_status.order.status = ReplicationOrderStatus::Completed(8);
        assert!(matches!(
            validate_assignment(
                &inconsistent_status,
                cursor(8),
                LOCAL_PROVIDER,
                runtime_policy(),
            ),
            Err(ProviderIngestRuntimeErrorV1::InvalidFinalizedBinding)
        ));
    }

    #[test]
    fn finalized_claim_factory_and_runtime_reject_musubi_binding_substitution() {
        let factory = ProviderIngestFinalizedClaimFactoryV1::new(
            test_chain_id(),
            TEST_GENESIS_BLOCK_HASH,
            LOCAL_PROVIDER,
        );
        let mut row = fixture_row(0x32);
        let binding = musubi_binding_for_row(&row, 0x81);
        let mut substituted_pin = row.pin.manifest.clone();
        substituted_pin.content_length = substituted_pin.content_length.saturating_add(1);
        assert_eq!(
            factory.seal_musubi_archive(
                &test_chain_id(),
                cursor(8),
                *row.order.order_id.as_bytes(),
                &substituted_pin,
                binding.clone(),
            ),
            Err(ProviderIngestFinalizedLedgerErrorV1::Rejected),
            "a reader cannot seal a publisher-substituted pin commitment"
        );
        let claim = factory
            .seal_musubi_archive(
                &test_chain_id(),
                cursor(8),
                *row.order.order_id.as_bytes(),
                &row.pin.manifest,
                binding.clone(),
            )
            .expect("seal finalized Musubi binding");
        assert_eq!(claim.replication_order(), *row.order.order_id.as_bytes());
        assert_eq!(claim.archive_id(), binding.archive_id);
        row.musubi_archive = Some(claim);
        assert!(matches!(
            validate_assignment(&row, cursor(8), LOCAL_PROVIDER, runtime_policy()),
            Err(ProviderIngestRuntimeErrorV1::InvalidFinalizedBinding)
        ));
        row.order.musubi_archive = Some(binding.archive_id);
        assert!(
            validate_assignment(&row, cursor(8), LOCAL_PROVIDER, runtime_policy()).is_ok(),
            "an exact finalized claim must remain usable"
        );
        let validated =
            validate_assignment(&row, cursor(8), LOCAL_PROVIDER, runtime_policy()).unwrap();
        let receipt = test_verified_musubi_receipt(
            row.musubi_archive.as_ref().unwrap(),
            &validated.authorization,
        );
        assert!(receipt.validate_stored(&validated.authorization));
        assert!(
            norito::to_bytes(&receipt.to_stored()).unwrap().len()
                <= PROVIDER_INGEST_VERIFIED_MUSUBI_RECEIPT_MAX_CANONICAL_BYTES_V1
        );

        let mut missing_claim = row.clone();
        missing_claim.musubi_archive = None;
        assert!(matches!(
            validate_assignment(&missing_claim, cursor(8), LOCAL_PROVIDER, runtime_policy(),),
            Err(ProviderIngestRuntimeErrorV1::InvalidFinalizedBinding)
        ));

        let other_order = fixture_row(0x33);
        assert_eq!(
            factory.seal_musubi_archive(
                &test_chain_id(),
                cursor(8),
                *other_order.order.order_id.as_bytes(),
                &other_order.pin.manifest,
                binding,
            ),
            Err(ProviderIngestFinalizedLedgerErrorV1::Rejected),
            "a reader cannot seal another order's publisher-shaped binding"
        );

        row.pin.manifest.content_length = row.pin.manifest.content_length.saturating_add(1);
        assert!(matches!(
            validate_assignment(&row, cursor(8), LOCAL_PROVIDER, runtime_policy()),
            Err(ProviderIngestRuntimeErrorV1::InvalidFinalizedBinding)
        ));
    }

    #[test]
    fn completed_musubi_claim_exists_only_for_the_local_finalized_completion() {
        let factory = ProviderIngestFinalizedClaimFactoryV1::new(
            test_chain_id(),
            TEST_GENESIS_BLOCK_HASH,
            LOCAL_PROVIDER,
        );
        let mut pending = fixture_musubi_row(0x34, 0x82);
        let binding = musubi_binding_for_row(&pending, 0x82);
        assert_eq!(
            factory.seal_completed_musubi_archive(
                &test_chain_id(),
                cursor(8),
                ProviderId::new(LOCAL_PROVIDER),
                &pending.order,
                &pending.pin.manifest,
                binding.clone(),
            ),
            Err(ProviderIngestFinalizedLedgerErrorV1::Rejected),
            "a pending local provider cannot receive a completed-row capability"
        );
        assert!(validate_assignment(&pending, cursor(8), LOCAL_PROVIDER, runtime_policy()).is_ok());

        let mut completed_other = pending.clone();
        completed_other
            .order
            .provider_completions
            .push(completion_record(
                ProviderId::new(SOURCE_PROVIDER),
                account(9),
                8,
            ));
        assert!(
            validate_assignment(
                &completed_other,
                cursor(8),
                LOCAL_PROVIDER,
                runtime_policy(),
            )
            .is_ok(),
            "another provider's completion must not require a local completed claim"
        );

        pending.order.provider_completions.push(completion_record(
            ProviderId::new(LOCAL_PROVIDER),
            account(8),
            8,
        ));
        let completed = factory
            .seal_completed_musubi_archive(
                &test_chain_id(),
                cursor(8),
                ProviderId::new(LOCAL_PROVIDER),
                &pending.order,
                &pending.pin.manifest,
                binding,
            )
            .expect("seal exact local finalized completion");
        assert_eq!(completed.provider_id(), LOCAL_PROVIDER);
        assert_eq!(
            completed.completion(),
            &pending.order.provider_completions[0]
        );
        pending.completed_musubi_archive = Some(completed);
        assert!(
            validate_assignment(&pending, cursor(8), LOCAL_PROVIDER, runtime_policy()).is_ok(),
            "a completed Musubi row must carry its exact post-completion capability"
        );

        let mut substituted_claim = pending.clone();
        substituted_claim
            .completed_musubi_archive
            .as_mut()
            .expect("completed claim")
            .observed_finalized_cursor = cursor(7);
        assert!(matches!(
            validate_assignment(
                &substituted_claim,
                cursor(8),
                LOCAL_PROVIDER,
                runtime_policy(),
            ),
            Err(ProviderIngestRuntimeErrorV1::InvalidFinalizedBinding)
        ));

        let mut missing = pending.clone();
        missing.completed_musubi_archive = None;
        assert!(matches!(
            validate_assignment(&missing, cursor(8), LOCAL_PROVIDER, runtime_policy()),
            Err(ProviderIngestRuntimeErrorV1::InvalidFinalizedBinding)
        ));

        let mut generic = fixture_row(0x35);
        generic.order.provider_completions.push(completion_record(
            ProviderId::new(LOCAL_PROVIDER),
            account(8),
            8,
        ));
        assert!(
            validate_assignment(&generic, cursor(8), LOCAL_PROVIDER, runtime_policy()).is_ok(),
            "a generic finalized completion must not carry a Musubi capability"
        );
        generic.completed_musubi_archive = pending.completed_musubi_archive;
        assert!(matches!(
            validate_assignment(&generic, cursor(8), LOCAL_PROVIDER, runtime_policy()),
            Err(ProviderIngestRuntimeErrorV1::InvalidFinalizedBinding)
        ));
    }

    #[test]
    fn musubi_claims_and_receipts_remain_valid_across_later_finalized_scans() {
        let row = fixture_musubi_row(0x38, 0x85);
        let admission = validate_assignment(&row, cursor(8), LOCAL_PROVIDER, runtime_policy())
            .expect("validate admission")
            .authorization;
        let admitted_claim = row.musubi_archive.as_ref().expect("Musubi admission claim");
        let admitted_receipt = test_verified_musubi_receipt(admitted_claim, &admission);
        assert!(admitted_receipt.validate_stored(&admission));

        let mut later_claim = admitted_claim.clone();
        later_claim.observed_finalized_cursor = cursor(9);
        assert!(later_claim.matches_authorization(&admission));
        assert!(admitted_receipt.matches(&later_claim, &admission));
        assert!(
            ProviderIngestMusubiArchiveFetchBindingV1::from_finalized_claim(&later_claim)
                .matches_authorization(&admission)
        );
        assert_eq!(
            ProviderIngestSourceRequestV1::new(admission.clone(), vec![SOURCE_PROVIDER], None,),
            Err(ProviderIngestSourceFetchErrorV1::Rejected),
            "a durable Musubi authorization cannot be downgraded to a generic fetch"
        );

        let generic_authorization = FinalizedProviderIngestAuthorizationV1::from_finalized_state(
            admission.finalized_height(),
            admission.finalized_block_hash(),
            admission.provider_id(),
            admission.order_id(),
            admission.manifest_digest(),
            admission.manifest_cid().to_vec(),
            admission.chunker_handle().to_owned(),
            admission.chunk_digest_sha3_256(),
            admission.por_root(),
            admission.content_length(),
        )
        .expect("generic authorization with the same storage binding");
        assert_eq!(
            ProviderIngestSourceRequestV1::new(
                generic_authorization,
                vec![SOURCE_PROVIDER],
                Some(
                    ProviderIngestMusubiArchiveFetchBindingV1::from_finalized_claim(&later_claim,)
                ),
            ),
            Err(ProviderIngestSourceFetchErrorV1::Rejected),
            "a generic authorization cannot be upgraded by an informational fetch binding"
        );

        let later_receipt = test_verified_musubi_receipt(&later_claim, &admission);
        assert!(later_receipt.validate_stored(&admission));
        let mut latest_claim = later_claim.clone();
        latest_claim.observed_finalized_cursor = cursor(10);
        assert!(later_receipt.matches(&latest_claim, &admission));

        let replay_authorization =
            FinalizedProviderIngestAuthorizationV1::from_finalized_musubi_state(
                cursor(9).height,
                cursor(9).block_hash,
                admission.provider_id(),
                admission.order_id(),
                admission.manifest_digest(),
                admission.manifest_cid().to_vec(),
                admission.chunker_handle().to_owned(),
                admission.chunk_digest_sha3_256(),
                admission.por_root(),
                admission.content_length(),
                admission
                    .musubi_context()
                    .expect("Musubi authorization context")
                    .clone(),
            )
            .expect("later finalized replay authorization");
        let outbox = ProviderIngestOutbox::in_memory(outbox_policy()).expect("Musubi outbox");
        outbox
            .enqueue(admission.clone())
            .expect("enqueue admission authorization");
        assert!(matches!(
            outbox
                .enqueue(replay_authorization)
                .expect("replay at later finalized head"),
            crate::provider_ingest_outbox::ProviderIngestEnqueueResultV1::ExistingActive { .. }
        ));
        let retained = outbox
            .authorization(admission.job_id())
            .expect("retained admission authorization");
        assert_eq!(retained, admission);
        outbox
            .observe_finalized_snapshot(cursor(9), 9_000)
            .expect("advance durable finalized high-water for later receipt");
        let source_claim = outbox
            .claim_source(
                retained.job_id(),
                ProviderIngestClaimOwnerV1::new([0xD4; 32]).expect("claim owner"),
                100,
                cursor(9),
            )
            .expect("claim replayed Musubi source work");
        outbox
            .mark_local_stored_verified(
                &source_claim,
                101,
                hex::encode(retained.manifest_digest()),
                Some(later_receipt.clone()),
            )
            .expect("persist verifier receipt from later finalized scan");
        let status = outbox.status(retained.job_id()).expect("stored status");
        let ProviderIngestDeliveryStateV1::LocalStored {
            musubi_bundle: Some(stored_receipt),
            ..
        } = status.state
        else {
            panic!("expected persisted Musubi verifier receipt");
        };
        assert!(persisted_receipt_matches(
            &retained,
            Some(&latest_claim),
            Some(stored_receipt.as_ref()),
        ));

        let mut stale_claim = admitted_claim.clone();
        stale_claim.observed_finalized_cursor = cursor(7);
        assert!(!stale_claim.matches_authorization(&admission));

        let mut forked_claim = admitted_claim.clone();
        forked_claim.observed_finalized_cursor.block_hash = [0xF8; 32];
        assert!(!forked_claim.matches_authorization(&admission));

        let mut receipt_from_future = later_receipt;
        receipt_from_future.observed_finalized_cursor = cursor(11);
        assert!(!receipt_from_future.matches(&latest_claim, &admission));
    }

    #[test]
    fn musubi_attestation_approval_request_binds_exact_completed_verified_evidence() {
        let VerifiedAttestationBundleFixtureV1 {
            verified,
            commitment,
            ..
        } = verified_attestation_bundle_fixture(0xD1);
        let claim = completed_attestation_claim(commitment.clone());

        let first = ProviderIngestMusubiAttestationApprovalRequestV1::from_verified_completion(
            &claim, &verified,
        )
        .expect("derive approval request");
        let second = ProviderIngestMusubiAttestationApprovalRequestV1::from_verified_completion(
            &claim, &verified,
        )
        .expect("derive deterministic approval request");

        assert_eq!(first, second);
        assert_ne!(first.completion_claim_digest(), [0; 32]);
        assert_eq!(first.observed_finalized_cursor(), cursor(8));
        assert_eq!(
            first.signer_policy(),
            claim.completion().completion_authority.signer_policy
        );
        assert_eq!(first.payload().version, MUSUBI_REGISTRY_VERSION_V1);
        assert_eq!(
            first.payload().binding.chain_id,
            *claim.chain_id(),
            "payload must bind the runtime-selected chain"
        );
        assert_eq!(
            first.payload().binding.provider_id,
            ProviderId::new(LOCAL_PROVIDER)
        );
        assert_eq!(
            first.payload().binding.replication_order.as_bytes(),
            &claim.replication_order()
        );
        assert_eq!(first.payload().binding.archive_id, commitment.archive_id());
        assert_eq!(
            first.payload().binding.bundle_digest,
            commitment.bundle_digest
        );
        assert_eq!(
            first.payload().binding.descriptor_digest,
            commitment.descriptor_digest
        );
        assert_eq!(
            first.payload().binding.semantic_release_manifest_digest,
            verified.semantic_release().semantic_digest()
        );
        assert_eq!(
            first.payload().binding.verification_lock_digest,
            verified.verification_lock().digest()
        );
        assert_eq!(
            first.payload().signing_hash(),
            second.payload().signing_hash()
        );

        let mut later_claim = claim.clone();
        later_claim.observed_finalized_cursor = cursor(9);
        let later = ProviderIngestMusubiAttestationApprovalRequestV1::from_verified_completion(
            &later_claim,
            &verified,
        )
        .expect("the identical completed row can be reverified at a later finalized head");
        assert_eq!(later.payload(), first.payload());
        assert_eq!(
            later.completion_claim_digest(),
            first.completion_claim_digest()
        );
        assert_eq!(later.observed_finalized_cursor(), cursor(9));
    }

    #[test]
    fn musubi_attestation_approval_request_rejects_substituted_evidence() {
        let VerifiedAttestationBundleFixtureV1 {
            verified,
            commitment,
            ..
        } = verified_attestation_bundle_fixture(0xD2);
        let claim = completed_attestation_claim(commitment);
        let other_verified = verified_attestation_bundle_fixture(0xD3).verified;
        assert_eq!(
            ProviderIngestMusubiAttestationApprovalRequestV1::from_verified_completion(
                &claim,
                &other_verified,
            ),
            Err(ProviderIngestMusubiAttestationApprovalRequestErrorV1::Rejected),
            "verified bundle evidence from another commitment must fail"
        );

        let mut substituted_bundle_commitment = claim.clone();
        substituted_bundle_commitment
            .binding
            .commitment
            .bundle_digest = MusubiContentDigestV1::new([0xD6; 32]);
        substituted_bundle_commitment.binding.archive_id = substituted_bundle_commitment
            .binding
            .commitment
            .archive_id();
        assert_eq!(
            ProviderIngestMusubiAttestationApprovalRequestV1::from_verified_completion(
                &substituted_bundle_commitment,
                &verified,
            ),
            Err(ProviderIngestMusubiAttestationApprovalRequestErrorV1::Rejected),
            "verified evidence must retain the exact archive identity, not only projected fields"
        );

        let mut substituted_descriptor_commitment = claim.clone();
        substituted_descriptor_commitment
            .binding
            .commitment
            .descriptor_digest = MusubiContentDigestV1::new([0xD4; 32]);
        substituted_descriptor_commitment.binding.archive_id = substituted_descriptor_commitment
            .binding
            .commitment
            .archive_id();
        assert_eq!(
            ProviderIngestMusubiAttestationApprovalRequestV1::from_verified_completion(
                &substituted_descriptor_commitment,
                &verified,
            ),
            Err(ProviderIngestMusubiAttestationApprovalRequestErrorV1::Rejected),
            "a substituted descriptor commitment must fail even with a self-consistent archive ID"
        );

        let mut substituted_completion = claim.clone();
        substituted_completion.completion.provider_id = ProviderId::new(SOURCE_PROVIDER);
        assert_eq!(
            ProviderIngestMusubiAttestationApprovalRequestV1::from_verified_completion(
                &substituted_completion,
                &verified,
            ),
            Err(ProviderIngestMusubiAttestationApprovalRequestErrorV1::Rejected),
            "another provider's completion must fail"
        );

        let mut substituted_cursor = claim;
        substituted_cursor.observed_finalized_cursor.block_hash = [0xD5; 32];
        assert_eq!(
            ProviderIngestMusubiAttestationApprovalRequestV1::from_verified_completion(
                &substituted_cursor,
                &verified,
            ),
            Err(ProviderIngestMusubiAttestationApprovalRequestErrorV1::Rejected),
            "a same-height cursor that does not cover the completion anchor must fail"
        );

        let mut lower_cursor = substituted_cursor;
        lower_cursor.observed_finalized_cursor = cursor(7);
        assert_eq!(
            ProviderIngestMusubiAttestationApprovalRequestV1::from_verified_completion(
                &lower_cursor,
                &verified,
            ),
            Err(ProviderIngestMusubiAttestationApprovalRequestErrorV1::Rejected),
            "a cursor below the completed-row anchor must fail"
        );
    }

    #[test]
    fn completed_musubi_claim_matches_only_exact_authorization_and_finalized_prefix() {
        let fixture = verified_attestation_bundle_fixture(0xD8);
        let mut claim = completed_attestation_claim(fixture.commitment.clone());
        let authorization = completed_attestation_authorization(&claim, [0x93; 32]);
        assert!(claim.matches_authorization(&authorization));

        claim.observed_finalized_cursor = cursor(10);
        claim.completion.finalized_anchor = ProviderIngestFinalizedAnchorV1 {
            height: 9,
            block_hash: cursor(9).block_hash,
        };
        let late_admission = completed_attestation_authorization(&claim, [0x93; 32]);
        assert!(
            claim.matches_authorization(&late_admission),
            "a completed row may first be observed after its own finalized anchor"
        );

        let mut stale = claim.clone();
        stale.observed_finalized_cursor = cursor(8);
        assert!(!stale.matches_authorization(&late_admission));

        let mut substituted_chain = claim.clone();
        substituted_chain.chain_id = ChainId::from("substituted-completion-chain");
        assert!(!substituted_chain.matches_authorization(&late_admission));

        let mut substituted_commitment = claim.clone();
        substituted_commitment.binding.commitment.por_root = MusubiContentDigestV1::new([0xDA; 32]);
        substituted_commitment.binding.archive_id =
            substituted_commitment.binding.commitment.archive_id();
        assert!(!substituted_commitment.matches_authorization(&late_admission));

        let mut inert_cursor = claim.clone();
        inert_cursor.observed_finalized_cursor.block_hash = [0; 32];
        assert!(!inert_cursor.matches_authorization(&late_admission));

        let conflicting_admission =
            FinalizedProviderIngestAuthorizationV1::from_finalized_musubi_state(
                claim.completion.finalized_anchor.height,
                [0xDB; 32],
                claim.provider_id(),
                claim.replication_order(),
                [0x93; 32],
                claim.commitment().root_cid.as_bytes().to_vec(),
                claim.commitment().chunker.to_handle(),
                *claim.commitment().chunk_plan_digest.as_bytes(),
                *claim.commitment().por_root.as_bytes(),
                claim.commitment().content_length,
                FinalizedProviderIngestMusubiContextV1::new(
                    claim.chain_id().clone(),
                    claim.genesis_block_hash(),
                    claim.archive_id(),
                )
                .expect("conflicting-admission Musubi context"),
            )
            .expect("conflicting retained authorization");
        assert!(
            !claim.matches_authorization(&conflicting_admission),
            "admission and completion cannot name different blocks at one height"
        );
    }

    #[test]
    fn admitted_payload_lease_alone_mints_completed_musubi_approval_request() {
        let fixture = verified_attestation_bundle_fixture(0xD9);
        let manifest = completed_attestation_manifest(&fixture);
        let temp_dir = tempfile::tempdir().expect("completed-attestation storage tempdir");
        let data_dir = temp_dir
            .path()
            .canonicalize()
            .expect("canonical completed-attestation tempdir")
            .join("storage");
        let backend = StorageBackend::new(
            StorageConfig::builder()
                .enabled(true)
                .data_dir(data_dir)
                .build(),
        )
        .expect("completed-attestation storage backend");
        let mut payload_reader = fixture.payload.as_slice();
        let manifest_id = backend
            .ingest_manifest(&manifest, &fixture.plan, &mut payload_reader)
            .expect("ingest completed-attestation payload");
        let stored = backend
            .manifest(&manifest_id)
            .expect("completed-attestation stored manifest");
        let manifest_digest = *stored.manifest_digest();
        let claim = completed_attestation_claim(fixture.commitment.clone());
        let authorization = completed_attestation_authorization(&claim, manifest_digest);
        let schedulers = StorageSchedulersRuntime::new(StorageSchedulerConfig::default());

        assert_eq!(fixture.verified.archive_id(), claim.archive_id());
        let (request, fourth_reader_rejected) = backend
            .with_admitted_payload_read_lease_by_digest(&manifest_digest, &schedulers, |lease| {
                let request =
                    lease.verify_completed_musubi_bundle(&fixture.plan, &authorization, &claim);
                let fourth_reader_rejected = matches!(
                    lease.open_reader(),
                    Err(error) if error.kind() == io::ErrorKind::PermissionDenied
                );
                (request, fourth_reader_rejected)
            })
            .expect("acquire completed-attestation lifecycle lease")
            .expect("completed-attestation payload remains admitted");
        let request = request.expect("fresh lease verification mints approval request");
        assert!(
            fourth_reader_rejected,
            "verification must consume exactly three fresh readers"
        );
        assert_eq!(request.payload().binding.archive_id, claim.archive_id());

        let mut substituted_plan = fixture.plan.clone();
        substituted_plan.payload_digest = blake3::hash(b"substituted admitted payload");
        assert_eq!(
            backend
                .with_admitted_payload_read_lease_by_digest(
                    &manifest_digest,
                    &schedulers,
                    |lease| lease.verify_completed_musubi_bundle(
                        &substituted_plan,
                        &authorization,
                        &claim,
                    ),
                )
                .expect("acquire substituted-plan lifecycle lease")
                .expect("completed-attestation payload remains admitted"),
            Err(ProviderIngestLocalStorageErrorV1::Permanent),
        );

        let substituted_authorization = completed_attestation_authorization(&claim, [0xDC; 32]);
        assert_eq!(
            backend
                .with_admitted_payload_read_lease_by_digest(
                    &manifest_digest,
                    &schedulers,
                    |lease| lease.verify_completed_musubi_bundle(
                        &fixture.plan,
                        &substituted_authorization,
                        &claim,
                    ),
                )
                .expect("acquire substituted-authorization lifecycle lease")
                .expect("completed-attestation payload remains admitted"),
            Err(ProviderIngestLocalStorageErrorV1::Permanent),
        );

        let mut stale_claim = claim.clone();
        stale_claim.observed_finalized_cursor = cursor(7);
        assert_eq!(
            backend
                .with_admitted_payload_read_lease_by_digest(
                    &manifest_digest,
                    &schedulers,
                    |lease| lease.verify_completed_musubi_bundle(
                        &fixture.plan,
                        &authorization,
                        &stale_claim,
                    ),
                )
                .expect("acquire stale-claim lifecycle lease")
                .expect("completed-attestation payload remains admitted"),
            Err(ProviderIngestLocalStorageErrorV1::Permanent),
        );

        let mut substituted_claim = claim;
        substituted_claim.binding.commitment.bundle_digest = MusubiContentDigestV1::new([0xDD; 32]);
        substituted_claim.binding.archive_id = substituted_claim.binding.commitment.archive_id();
        assert_eq!(
            backend
                .with_admitted_payload_read_lease_by_digest(
                    &manifest_digest,
                    &schedulers,
                    |lease| lease.verify_completed_musubi_bundle(
                        &fixture.plan,
                        &authorization,
                        &substituted_claim,
                    ),
                )
                .expect("acquire substituted-claim lifecycle lease")
                .expect("completed-attestation payload remains admitted"),
            Err(ProviderIngestLocalStorageErrorV1::Permanent),
        );
    }

    #[test]
    fn completed_musubi_lease_preserves_transient_reader_classification() {
        for kind in [
            io::ErrorKind::Interrupted,
            io::ErrorKind::WouldBlock,
            io::ErrorKind::TimedOut,
            io::ErrorKind::NotFound,
            io::ErrorKind::Other,
        ] {
            assert!(provider_ingest_admitted_payload_read_error_is_retryable(
                kind
            ));
        }
        for kind in [
            io::ErrorKind::InvalidData,
            io::ErrorKind::UnexpectedEof,
            io::ErrorKind::PermissionDenied,
        ] {
            assert!(!provider_ingest_admitted_payload_read_error_is_retryable(
                kind
            ));
        }
    }

    #[test]
    fn verified_musubi_receipt_rejects_archive_identity_substitution() {
        let VerifiedAttestationBundleFixtureV1 {
            verified,
            commitment,
            ..
        } = verified_attestation_bundle_fixture(0xD7);
        let binding = MusubiReplicationOrderArchiveBindingV1::new(
            ReplicationOrderId::new([0xAD; 32]),
            commitment.archive_id(),
            commitment,
        );
        let claim = ProviderIngestFinalizedMusubiArchiveClaimV1 {
            chain_id: test_chain_id(),
            genesis_block_hash: TEST_GENESIS_BLOCK_HASH,
            provider_id: LOCAL_PROVIDER,
            observed_finalized_cursor: cursor(8),
            binding,
        };
        let authorization_for = |claim: &ProviderIngestFinalizedMusubiArchiveClaimV1| {
            FinalizedProviderIngestAuthorizationV1::from_finalized_musubi_state(
                8,
                cursor(8).block_hash,
                LOCAL_PROVIDER,
                claim.replication_order(),
                [0xAE; 32],
                claim.commitment().root_cid.as_bytes().to_vec(),
                claim.commitment().chunker.to_handle(),
                *claim.commitment().chunk_plan_digest.as_bytes(),
                *claim.commitment().por_root.as_bytes(),
                claim.commitment().content_length,
                FinalizedProviderIngestMusubiContextV1::new(
                    test_chain_id(),
                    TEST_GENESIS_BLOCK_HASH,
                    claim.archive_id(),
                )
                .expect("Musubi context"),
            )
            .expect("Musubi authorization")
        };
        let authorization = authorization_for(&claim);
        ProviderIngestVerifiedMusubiBundleReceiptV1::from_verified_bundle(
            &claim,
            &authorization,
            &verified,
        )
        .expect("exact verifier evidence");

        let mut substituted = claim;
        substituted.binding.commitment.bundle_digest = MusubiContentDigestV1::new([0xAF; 32]);
        substituted.binding.archive_id = substituted.binding.commitment.archive_id();
        substituted
            .binding
            .validate()
            .expect("structurally valid substituted binding");
        let substituted_authorization = authorization_for(&substituted);
        assert_eq!(
            ProviderIngestVerifiedMusubiBundleReceiptV1::from_verified_bundle(
                &substituted,
                &substituted_authorization,
                &verified,
            ),
            Err(ProviderIngestLocalStorageErrorV1::Permanent),
            "a verifier result cannot be replayed under a different archive identity"
        );
    }

    #[test]
    fn completed_musubi_claim_factory_rejects_completion_substitutions() {
        let factory = ProviderIngestFinalizedClaimFactoryV1::new(
            test_chain_id(),
            TEST_GENESIS_BLOCK_HASH,
            LOCAL_PROVIDER,
        );
        let mut row = fixture_musubi_row(0x36, 0x83);
        row.order.provider_completions.push(completion_record(
            ProviderId::new(LOCAL_PROVIDER),
            account(8),
            8,
        ));
        let binding = musubi_binding_for_row(&row, 0x83);

        assert_eq!(
            factory.seal_completed_musubi_archive(
                &ChainId::from("substituted-chain"),
                cursor(8),
                ProviderId::new(LOCAL_PROVIDER),
                &row.order,
                &row.pin.manifest,
                binding.clone(),
            ),
            Err(ProviderIngestFinalizedLedgerErrorV1::Rejected)
        );
        assert_eq!(
            factory.seal_completed_musubi_archive(
                &test_chain_id(),
                cursor(8),
                ProviderId::new(SOURCE_PROVIDER),
                &row.order,
                &row.pin.manifest,
                binding.clone(),
            ),
            Err(ProviderIngestFinalizedLedgerErrorV1::Rejected)
        );
        let mut earlier_cursor = cursor(7);
        earlier_cursor.block_hash = [0xE7; 32];
        assert_eq!(
            factory.seal_completed_musubi_archive(
                &test_chain_id(),
                earlier_cursor,
                ProviderId::new(LOCAL_PROVIDER),
                &row.order,
                &row.pin.manifest,
                binding.clone(),
            ),
            Err(ProviderIngestFinalizedLedgerErrorV1::Rejected)
        );

        let mut substituted_pin = row.pin.manifest.clone();
        substituted_pin.por_root = [0xE8; 32];
        assert_eq!(
            factory.seal_completed_musubi_archive(
                &test_chain_id(),
                cursor(8),
                ProviderId::new(LOCAL_PROVIDER),
                &row.order,
                &substituted_pin,
                binding.clone(),
            ),
            Err(ProviderIngestFinalizedLedgerErrorV1::Rejected)
        );

        let mut substituted_completion = row.order.clone();
        substituted_completion.provider_completions[0].assignment_revision = 2;
        assert_eq!(
            factory.seal_completed_musubi_archive(
                &test_chain_id(),
                cursor(8),
                ProviderId::new(LOCAL_PROVIDER),
                &substituted_completion,
                &row.pin.manifest,
                binding.clone(),
            ),
            Err(ProviderIngestFinalizedLedgerErrorV1::Rejected)
        );

        let mut duplicate_completion = row.order.clone();
        duplicate_completion
            .provider_completions
            .push(duplicate_completion.provider_completions[0].clone());
        assert_eq!(
            factory.seal_completed_musubi_archive(
                &test_chain_id(),
                cursor(8),
                ProviderId::new(LOCAL_PROVIDER),
                &duplicate_completion,
                &row.pin.manifest,
                binding.clone(),
            ),
            Err(ProviderIngestFinalizedLedgerErrorV1::Rejected)
        );

        let mut unassigned_order = row.order.clone();
        let mut canonical_order = decode_from_bytes_with_limits::<ReplicationOrderV1>(
            &unassigned_order.canonical_order,
            REPLICATION_ORDER_DECODE_LIMITS_V1,
        )
        .expect("decode fixture order");
        canonical_order
            .assignments
            .retain(|assignment| assignment.provider_id != LOCAL_PROVIDER);
        unassigned_order.canonical_order = norito::to_bytes(&canonical_order).expect("order bytes");
        assert_eq!(
            factory.seal_completed_musubi_archive(
                &test_chain_id(),
                cursor(8),
                ProviderId::new(LOCAL_PROVIDER),
                &unassigned_order,
                &row.pin.manifest,
                binding.clone(),
            ),
            Err(ProviderIngestFinalizedLedgerErrorV1::Rejected)
        );

        let other = fixture_musubi_row(0x37, 0x84);
        assert_eq!(
            factory.seal_completed_musubi_archive(
                &test_chain_id(),
                cursor(8),
                ProviderId::new(LOCAL_PROVIDER),
                &row.order,
                &row.pin.manifest,
                musubi_binding_for_row(&other, 0x84),
            ),
            Err(ProviderIngestFinalizedLedgerErrorV1::Rejected)
        );
    }

    #[tokio::test]
    async fn runtime_recovers_durable_finalized_high_water_before_scanning() {
        let row = fixture_row(0x2F);
        let (runtime, _, _, _) = test_runtime(
            row,
            true,
            Ok(vec![0xA5]),
            0,
            ProviderIngestIngressDispositionV1::Submitted,
            false,
        );
        runtime
            .outbox
            .observe_finalized_snapshot(cursor(9), 9_000)
            .expect("persist later cursor");
        let mut restarted = ProviderIngestRuntimeV1::new(
            runtime.provider_id,
            runtime.chain_id.clone(),
            runtime.genesis_block_hash,
            runtime.claim_owner,
            runtime.policy,
            runtime.outbox.clone(),
            runtime.ledger.clone(),
            runtime.fetch.clone(),
            runtime.storage.clone(),
            runtime.payload_builder.clone(),
            runtime.signer_resolver.clone(),
            runtime.ingress.clone(),
            runtime.clock.clone(),
        )
        .expect("restart runtime");
        assert_eq!(restarted.last_finalized_cursor, Some(cursor(9)));
        assert!(matches!(
            restarted.tick().await,
            Err(ProviderIngestRuntimeErrorV1::InvalidFinalizedPage)
        ));
        assert_eq!(
            restarted.outbox.finalized_cursor_high_water().unwrap(),
            Some(cursor(9))
        );
    }

    #[tokio::test]
    async fn finalized_block_time_equivocation_is_rejected_after_restart() {
        let row = fixture_row(0x44);
        let (runtime, ledger, _, _) = test_runtime(
            row,
            true,
            Ok(vec![0xA5]),
            0,
            ProviderIngestIngressDispositionV1::Submitted,
            false,
        );
        runtime
            .outbox
            .observe_finalized_snapshot(cursor(8), 8_000)
            .expect("persist finalized snapshot");
        ledger.page.lock().unwrap().finalized_block_time_ms = 8_001;
        let mut restarted = ProviderIngestRuntimeV1::new(
            runtime.provider_id,
            runtime.chain_id.clone(),
            runtime.genesis_block_hash,
            runtime.claim_owner,
            runtime.policy,
            runtime.outbox.clone(),
            runtime.ledger.clone(),
            runtime.fetch.clone(),
            runtime.storage.clone(),
            runtime.payload_builder.clone(),
            runtime.signer_resolver.clone(),
            runtime.ingress.clone(),
            runtime.clock.clone(),
        )
        .expect("restart runtime");
        assert!(matches!(
            restarted.tick().await,
            Err(ProviderIngestRuntimeErrorV1::Outbox(
                ProviderIngestOutboxError::FinalizedSnapshotConflict
            ))
        ));
        assert_eq!(
            restarted.outbox.finalized_snapshot_high_water().unwrap(),
            Some((cursor(8), 8_000))
        );
    }

    #[tokio::test]
    async fn local_existing_path_skips_network_and_preflights_before_ambiguity() {
        let row = fixture_row(0x32);
        let (mut runtime, _, fetch, ingress) = test_runtime(
            row,
            true,
            Ok(vec![0xA5]),
            0,
            ProviderIngestIngressDispositionV1::Submitted,
            false,
        );
        let outcome = runtime.tick().await.expect("tick");
        assert_eq!(outcome.manifests_stored, 1);
        assert_eq!(outcome.completions_signed, 1);
        assert_eq!(outcome.completion_submissions, 1);
        assert_eq!(fetch.calls.load(Ordering::SeqCst), 0);
        assert_eq!(
            *ingress.events.lock().unwrap(),
            vec!["prepare_signed", "expose_ambiguous"]
        );
        assert!(matches!(
            runtime.outbox.status(ingress.job_id).unwrap().state,
            ProviderIngestDeliveryStateV1::LocalStored {
                completion: ProviderIngestCompletionStateV1::Submitted { .. },
                ..
            }
        ));
    }

    #[test]
    fn musubi_local_stored_without_verifier_receipt_is_never_checkpointed() {
        let row = fixture_musubi_row(0x6A, 0xB1);
        let authorization = validate_assignment(&row, cursor(8), LOCAL_PROVIDER, runtime_policy())
            .expect("valid Musubi assignment")
            .authorization;
        let (runtime, _, _, ingress) = test_runtime(
            row,
            true,
            Ok(vec![0xA5]),
            0,
            ProviderIngestIngressDispositionV1::Submitted,
            false,
        );
        runtime
            .outbox
            .enqueue(authorization.clone())
            .expect("enqueue Musubi job");
        let source_claim = runtime
            .outbox
            .claim_source(
                authorization.job_id(),
                runtime.claim_owner,
                1_000,
                cursor(8),
            )
            .expect("claim Musubi source job");
        assert_eq!(
            runtime.outbox.mark_local_stored(
                &source_claim,
                1_001,
                hex::encode(authorization.manifest_digest()),
            ),
            Err(ProviderIngestOutboxError::InvalidAuthorization)
        );
        assert!(matches!(
            runtime.outbox.status(authorization.job_id()).unwrap().state,
            ProviderIngestDeliveryStateV1::SourceClaimed { .. }
        ));
        assert!(ingress.events.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn single_replica_without_remote_sources_releases_source_claim_for_retry() {
        let mut row = fixture_row(0x35);
        let mut order = decode_from_bytes_with_limits::<ReplicationOrderV1>(
            &row.order.canonical_order,
            REPLICATION_ORDER_DECODE_LIMITS_V1,
        )
        .expect("decode fixture order");
        order.target_replicas = 1;
        order
            .assignments
            .retain(|assignment| assignment.provider_id == LOCAL_PROVIDER);
        order.validate().expect("valid single-replica order");
        row.order.canonical_order = norito::to_bytes(&order).expect("order bytes");

        let (mut runtime, _, fetch, ingress) = test_runtime(
            row,
            false,
            Ok(vec![0xA5]),
            0,
            ProviderIngestIngressDispositionV1::Submitted,
            false,
        );

        let outcome = runtime.tick().await.expect("single-replica source tick");
        assert_eq!(outcome.source_jobs_claimed, 1);
        assert_eq!(fetch.calls.load(Ordering::SeqCst), 0);
        assert!(matches!(
            runtime.outbox.status(ingress.job_id).unwrap().state,
            ProviderIngestDeliveryStateV1::RetryScheduled {
                failure_class: ProviderIngestFailureClassV1::SourceUnavailable,
                ..
            }
        ));
    }

    #[tokio::test]
    async fn corrupt_authenticated_source_is_retryable_not_a_permanent_dead_letter() {
        let row = fixture_row(0x33);
        let (mut runtime, _, fetch, ingress) = test_runtime(
            row,
            false,
            Err(ProviderIngestSourceFetchErrorV1::ContentRejected),
            0,
            ProviderIngestIngressDispositionV1::Submitted,
            false,
        );
        runtime.tick().await.expect("tick");
        assert_eq!(fetch.calls.load(Ordering::SeqCst), 1);
        assert!(matches!(
            runtime.outbox.status(ingress.job_id).unwrap().state,
            ProviderIngestDeliveryStateV1::RetryScheduled {
                failure_class: ProviderIngestFailureClassV1::SourceRejected,
                ..
            }
        ));
    }

    #[tokio::test]
    async fn authenticated_source_binding_rejection_is_terminal_for_the_tick() {
        let row = fixture_row(0x34);
        let (mut runtime, _, fetch, ingress) = test_runtime(
            row,
            false,
            Err(ProviderIngestSourceFetchErrorV1::Rejected),
            0,
            ProviderIngestIngressDispositionV1::Submitted,
            false,
        );

        assert!(matches!(
            runtime.tick().await,
            Err(ProviderIngestRuntimeErrorV1::SourceProtocolViolation)
        ));
        assert_eq!(fetch.calls.load(Ordering::SeqCst), 1);
        assert!(matches!(
            runtime.outbox.status(ingress.job_id).unwrap().state,
            ProviderIngestDeliveryStateV1::RetryScheduled {
                failure_class: ProviderIngestFailureClassV1::SourceRejected,
                ..
            }
        ));
    }

    #[tokio::test]
    async fn ineligible_early_source_does_not_consume_fair_work_budget() {
        let first = fixture_row(0x10);
        let second = fixture_row(0x20);
        let (mut runtime, ledger, fetch, _) = test_runtime(
            first.clone(),
            false,
            Err(ProviderIngestSourceFetchErrorV1::ContentRejected),
            0,
            ProviderIngestIngressDispositionV1::Submitted,
            false,
        );
        ledger.page.lock().unwrap().rows = vec![first.clone(), second.clone()];
        let first_authorization =
            validate_assignment(&first, cursor(8), LOCAL_PROVIDER, runtime_policy())
                .unwrap()
                .authorization;
        runtime.outbox.enqueue(first_authorization.clone()).unwrap();
        let claim = runtime
            .outbox
            .claim_source(
                first_authorization.job_id(),
                ProviderIngestClaimOwnerV1::new([0xDD; 32]).unwrap(),
                10_000,
                cursor(8),
            )
            .unwrap();
        runtime
            .outbox
            .schedule_source_retry(
                &claim,
                10_001,
                cursor(8),
                ProviderIngestFailureClassV1::SourceUnavailable,
            )
            .unwrap();
        runtime.policy.max_source_jobs_per_tick = 1;

        let outcome = runtime.tick().await.expect("fair source tick");
        assert_eq!(outcome.source_jobs_claimed, 1);
        assert_eq!(fetch.calls.load(Ordering::SeqCst), 1);
        let second_authorization =
            validate_assignment(&second, cursor(8), LOCAL_PROVIDER, runtime_policy())
                .unwrap()
                .authorization;
        assert!(matches!(
            runtime
                .outbox
                .status(second_authorization.job_id())
                .unwrap()
                .state,
            ProviderIngestDeliveryStateV1::RetryScheduled {
                failure_class: ProviderIngestFailureClassV1::SourceRejected,
                ..
            }
        ));
    }

    #[tokio::test]
    async fn slow_fetch_renews_the_source_lease_until_atomic_storage_finishes() {
        let row = fixture_row(0x34);
        let (mut runtime, _, fetch, ingress) = test_runtime(
            row,
            false,
            Ok(vec![0xA5]),
            45,
            ProviderIngestIngressDispositionV1::Submitted,
            false,
        );
        runtime.tick().await.expect("renewed tick");
        assert_eq!(fetch.calls.load(Ordering::SeqCst), 1);
        assert!(matches!(
            runtime.outbox.status(ingress.job_id).unwrap().state,
            ProviderIngestDeliveryStateV1::LocalStored { .. }
        ));
    }

    #[tokio::test]
    async fn semantic_completion_from_another_replica_wins_over_ambiguous_local_hash() {
        let row = fixture_row(0x35);
        let (mut runtime, ledger, _, ingress) = test_runtime(
            row,
            true,
            Ok(vec![0xA5]),
            0,
            ProviderIngestIngressDispositionV1::Ambiguous,
            false,
        );
        runtime.tick().await.expect("first tick");
        assert!(matches!(
            runtime.outbox.status(ingress.job_id).unwrap().state,
            ProviderIngestDeliveryStateV1::LocalStored {
                completion: ProviderIngestCompletionStateV1::Ambiguous { .. },
                ..
            }
        ));

        {
            let mut page = ledger.page.lock().unwrap();
            page.finalized_cursor = cursor(9);
            page.finalized_block_time_ms = 9_000;
            page.rows[0].pin.finalized_cursor = PinManifestFinalizedCursorV1 {
                height: 9,
                block_hash: cursor(9).block_hash,
            };
            page.rows[0]
                .order
                .provider_completions
                .push(completion_record(
                    ProviderId::new(SOURCE_PROVIDER),
                    account(7),
                    8,
                ));
            page.rows[0]
                .order
                .provider_completions
                .push(completion_record(
                    ProviderId::new(LOCAL_PROVIDER),
                    account(9),
                    9,
                ));
            page.rows[0].order.status = ReplicationOrderStatus::Completed(9);
            page.rows[0].completion_epoch = Some(9);
        }

        runtime.tick().await.expect("semantic reconciliation");
        assert!(matches!(
            runtime.outbox.status(ingress.job_id).unwrap().state,
            ProviderIngestDeliveryStateV1::FinalizedCompleted {
                completion_epoch: 9,
                completed_by,
                committed_transaction_hash: None,
                ..
            } if completed_by == account(9)
        ));
    }

    #[tokio::test]
    async fn finalized_completion_first_row_bypasses_full_active_capacity() {
        let mut completed = fixture_row(0x2E);
        completed.order.provider_completions = vec![
            completion_record(ProviderId::new(SOURCE_PROVIDER), account(7), 8),
            completion_record(ProviderId::new(LOCAL_PROVIDER), account(9), 9),
        ];
        completed.order.status = ReplicationOrderStatus::Completed(9);
        completed.pin.finalized_cursor = PinManifestFinalizedCursorV1 {
            height: 9,
            block_hash: cursor(9).block_hash,
        };
        completed.completion_epoch = Some(9);
        let (mut runtime, _, fetch, ingress) = test_runtime(
            completed,
            false,
            Ok(vec![0xA5]),
            0,
            ProviderIngestIngressDispositionV1::Submitted,
            false,
        );
        for seed in 0x40_u8..=0x5F {
            let pending = fixture_row(seed);
            let authorization =
                validate_assignment(&pending, cursor(8), LOCAL_PROVIDER, runtime_policy())
                    .unwrap()
                    .authorization;
            runtime.outbox.enqueue(authorization).expect("fill active");
        }
        assert_eq!(
            runtime
                .outbox
                .aggregate_counts()
                .expect("full active inventory")
                .active,
            runtime.outbox.policy().max_active_entries
        );

        let outcome = runtime.tick().await.expect("finalized reconciliation");
        assert_eq!(outcome.rows_scanned, 1);
        assert_eq!(outcome.jobs_inserted, 0);
        assert_eq!(outcome.jobs_finalized, 1);
        assert_eq!(fetch.calls.load(Ordering::SeqCst), 0);
        assert!(matches!(
            runtime.outbox.status(ingress.job_id).unwrap().state,
            ProviderIngestDeliveryStateV1::FinalizedCompleted {
                manifest_id: None,
                completion_epoch: 9,
                ..
            }
        ));
    }

    #[tokio::test]
    async fn preflight_rejection_resigns_without_entering_ambiguous_state() {
        let row = fixture_row(0x39);
        let (mut runtime, _, _, ingress) = test_runtime(
            row,
            true,
            Ok(vec![0xA5]),
            0,
            ProviderIngestIngressDispositionV1::Submitted,
            false,
        );
        *ingress.prepare_error.lock().unwrap() =
            Some(ProviderIngestIngressPrepareErrorV1::Rejected);
        runtime.tick().await.expect("preflight rejection");
        assert_eq!(*ingress.events.lock().unwrap(), vec!["prepare_signed"]);
        assert!(matches!(
            runtime.outbox.status(ingress.job_id).unwrap().state,
            ProviderIngestDeliveryStateV1::LocalStored {
                completion: ProviderIngestCompletionStateV1::Ready {
                    last_failure_class: Some(ProviderIngestFailureClassV1::TransactionRejected),
                    ..
                },
                ..
            }
        ));
    }

    #[tokio::test]
    async fn payload_and_preflight_failures_are_durably_backed_off() {
        let payload_row = fixture_row(0x3B);
        let (mut payload_runtime, _, _, payload_ingress) = test_runtime(
            payload_row,
            true,
            Ok(vec![0xA5]),
            0,
            ProviderIngestIngressDispositionV1::Submitted,
            false,
        );
        payload_runtime.tick().await.expect("payload failure tick");
        assert!(matches!(
            payload_runtime
                .outbox
                .status(payload_ingress.job_id)
                .unwrap()
                .state,
            ProviderIngestDeliveryStateV1::LocalStored {
                completion: ProviderIngestCompletionStateV1::Ready {
                    attempts: 1,
                    last_failure_class: Some(
                        ProviderIngestFailureClassV1::PayloadPreparationFailed
                    ),
                    ..
                },
                ..
            }
        ));
        payload_runtime.tick().await.expect("payload backoff tick");
        assert!(matches!(
            payload_runtime
                .outbox
                .status(payload_ingress.job_id)
                .unwrap()
                .state,
            ProviderIngestDeliveryStateV1::LocalStored {
                completion: ProviderIngestCompletionStateV1::Ready { attempts: 1, .. },
                ..
            }
        ));

        let ingress_row = fixture_row(0x3C);
        let (mut ingress_runtime, _, _, ingress) = test_runtime(
            ingress_row,
            true,
            Ok(vec![0xA5]),
            0,
            ProviderIngestIngressDispositionV1::Submitted,
            false,
        );
        *ingress.prepare_error.lock().unwrap() =
            Some(ProviderIngestIngressPrepareErrorV1::Unavailable);
        ingress_runtime.tick().await.expect("ingress unavailable");
        assert_eq!(*ingress.events.lock().unwrap(), vec!["prepare_signed"]);
        assert!(matches!(
            ingress_runtime
                .outbox
                .status(ingress.job_id)
                .unwrap()
                .state,
            ProviderIngestDeliveryStateV1::LocalStored {
                completion: ProviderIngestCompletionStateV1::Signed {
                    attempts: 2,
                    next_attempt_at_ms,
                    ..
                },
                ..
            } if next_attempt_at_ms > ingress_runtime.clock.now_ms()
        ));
        ingress_runtime.tick().await.expect("signed retry not due");
        assert_eq!(*ingress.events.lock().unwrap(), vec!["prepare_signed"]);
    }

    #[tokio::test]
    async fn ambiguous_unknown_retries_only_after_a_later_finalized_cursor() {
        let row = fixture_row(0x3A);
        let (mut runtime, ledger, _, ingress) = test_runtime(
            row,
            true,
            Ok(vec![0xA5]),
            0,
            ProviderIngestIngressDispositionV1::Ambiguous,
            false,
        );
        runtime.tick().await.expect("ambiguous submit");
        *ingress.observation.lock().unwrap() = ProviderIngestTransactionObservationV1::Unknown;

        {
            let mut page = ledger.page.lock().unwrap();
            page.finalized_cursor = cursor(9);
            page.finalized_block_time_ms = 9_000;
            page.rows[0].pin.finalized_cursor = PinManifestFinalizedCursorV1 {
                height: 9,
                block_hash: cursor(9).block_hash,
            };
            page.rows[0].completion_epoch = Some(9);
        }

        runtime.tick().await.expect("finalized absence");
        assert!(matches!(
            runtime.outbox.status(ingress.job_id).unwrap().state,
            ProviderIngestDeliveryStateV1::LocalStored {
                completion: ProviderIngestCompletionStateV1::Signed {
                    baseline_finalized_cursor,
                    ..
                },
                ..
            } if baseline_finalized_cursor == cursor(9)
        ));
    }

    #[tokio::test]
    async fn committed_hash_outcome_never_substitutes_for_semantic_completion() {
        let row = fixture_row(0x3D);
        let (mut runtime, ledger, _, ingress) = test_runtime(
            row,
            true,
            Ok(vec![0xA5]),
            0,
            ProviderIngestIngressDispositionV1::Submitted,
            false,
        );
        runtime.tick().await.expect("submitted transaction");

        *ingress.observation.lock().unwrap() =
            ProviderIngestTransactionObservationV1::CommittedSuccess;
        {
            let mut page = ledger.page.lock().unwrap();
            page.finalized_cursor = cursor(9);
            page.finalized_block_time_ms = 9_000;
            page.rows[0].pin.finalized_cursor = PinManifestFinalizedCursorV1 {
                height: 9,
                block_hash: cursor(9).block_hash,
            };
            page.rows[0].completion_epoch = Some(9);
        }

        let outcome = runtime.tick().await.expect("committed-success observation");
        assert_eq!(outcome.jobs_finalized, 0);
        assert!(matches!(
            runtime.outbox.status(ingress.job_id).unwrap().state,
            ProviderIngestDeliveryStateV1::LocalStored {
                completion: ProviderIngestCompletionStateV1::Submitted { .. },
                ..
            }
        ));

        *ingress.observation.lock().unwrap() =
            ProviderIngestTransactionObservationV1::CommittedRejected;
        runtime
            .tick()
            .await
            .expect("committed rejection is retryable");
        assert!(matches!(
            runtime.outbox.status(ingress.job_id).unwrap().state,
            ProviderIngestDeliveryStateV1::LocalStored {
                completion: ProviderIngestCompletionStateV1::Ready {
                    last_failure_class: Some(ProviderIngestFailureClassV1::TransactionRejected),
                    ..
                },
                ..
            }
        ));
    }

    #[tokio::test]
    async fn owner_rotation_reconciles_exposed_transaction_before_authority_change() {
        let row = fixture_row(0x3F);
        let (mut runtime, ledger, _, ingress) = test_runtime(
            row,
            true,
            Ok(vec![0xA5]),
            0,
            ProviderIngestIngressDispositionV1::Submitted,
            false,
        );
        runtime.tick().await.expect("submitted transaction");

        *ingress.observation.lock().unwrap() =
            ProviderIngestTransactionObservationV1::CommittedSuccess;
        {
            let mut page = ledger.page.lock().unwrap();
            page.finalized_cursor = cursor(9);
            page.finalized_block_time_ms = 9_000;
            page.rows[0].pin.finalized_cursor = PinManifestFinalizedCursorV1 {
                height: 9,
                block_hash: cursor(9).block_hash,
            };
            page.rows[0].provider_owner = Some(account(9));
            page.rows[0].completion_authority = Some(ProviderIngestCompletionAuthorityV1::new(
                account(9),
                completion_signer_policy(1),
            ));
            page.rows[0].completion_epoch = Some(9);
        }

        runtime.tick().await.expect("owner rotation");
        assert_eq!(ingress.observe_calls.load(Ordering::SeqCst), 1);
        assert!(matches!(
            runtime.outbox.status(ingress.job_id).unwrap().state,
            ProviderIngestDeliveryStateV1::LocalStored {
                completion: ProviderIngestCompletionStateV1::Submitted { .. },
                ..
            }
        ));
    }

    #[tokio::test]
    async fn signer_policy_rotation_reconciles_exposed_transaction_before_authority_change() {
        let row = fixture_row(0x41);
        let (mut runtime, ledger, _, ingress) = test_runtime(
            row,
            true,
            Ok(vec![0xA5]),
            0,
            ProviderIngestIngressDispositionV1::Submitted,
            false,
        );
        runtime.tick().await.expect("submitted transaction");

        *ingress.observation.lock().unwrap() =
            ProviderIngestTransactionObservationV1::CommittedSuccess;
        runtime
            .signer_resolver
            .signer_policy_revision
            .store(2, Ordering::SeqCst);
        {
            let mut page = ledger.page.lock().unwrap();
            page.finalized_cursor = cursor(9);
            page.finalized_block_time_ms = 9_000;
            page.rows[0].pin.finalized_cursor = PinManifestFinalizedCursorV1 {
                height: 9,
                block_hash: cursor(9).block_hash,
            };
            page.rows[0].completion_epoch = Some(9);
        }

        runtime.tick().await.expect("signer policy rotation");
        assert_eq!(ingress.observe_calls.load(Ordering::SeqCst), 1);
        assert!(matches!(
            runtime.outbox.status(ingress.job_id).unwrap().state,
            ProviderIngestDeliveryStateV1::LocalStored {
                completion: ProviderIngestCompletionStateV1::Submitted { .. },
                ..
            }
        ));
    }

    #[tokio::test]
    async fn owner_removal_reconciles_exposed_transaction_before_authority_change() {
        let row = fixture_row(0x40);
        let (mut runtime, ledger, _, ingress) = test_runtime(
            row,
            true,
            Ok(vec![0xA5]),
            0,
            ProviderIngestIngressDispositionV1::Submitted,
            false,
        );
        runtime.tick().await.expect("submitted transaction");

        *ingress.observation.lock().unwrap() =
            ProviderIngestTransactionObservationV1::CommittedSuccess;
        {
            let mut page = ledger.page.lock().unwrap();
            page.finalized_cursor = cursor(9);
            page.finalized_block_time_ms = 9_000;
            page.rows[0].pin.finalized_cursor = PinManifestFinalizedCursorV1 {
                height: 9,
                block_hash: cursor(9).block_hash,
            };
            page.rows[0].provider_owner = None;
            page.rows[0].completion_authority = None;
            page.rows[0].completion_epoch = None;
        }

        runtime.tick().await.expect("owner removal");
        assert_eq!(ingress.observe_calls.load(Ordering::SeqCst), 1);
        assert!(matches!(
            runtime.outbox.status(ingress.job_id).unwrap().state,
            ProviderIngestDeliveryStateV1::LocalStored {
                completion: ProviderIngestCompletionStateV1::Submitted { .. },
                ..
            }
        ));
    }

    #[tokio::test]
    async fn owner_rotation_invalidates_never_exposed_signed_bytes_without_preflight() {
        let row = fixture_row(0x42);
        let (mut runtime, ledger, _, ingress) = test_runtime(
            row,
            true,
            Ok(vec![0xA5]),
            0,
            ProviderIngestIngressDispositionV1::Submitted,
            false,
        );
        *ingress.prepare_error.lock().unwrap() =
            Some(ProviderIngestIngressPrepareErrorV1::Unavailable);
        let first = runtime
            .tick()
            .await
            .expect("sign before unavailable preflight");
        assert_eq!(first.completions_signed, 1);
        assert_eq!(first.completion_submissions, 0);
        assert!(matches!(
            runtime.outbox.status(ingress.job_id).unwrap().state,
            ProviderIngestDeliveryStateV1::LocalStored {
                completion: ProviderIngestCompletionStateV1::Signed {
                    ever_exposed: false,
                    ..
                },
                ..
            }
        ));

        {
            let mut page = ledger.page.lock().unwrap();
            page.finalized_cursor = cursor(9);
            page.finalized_block_time_ms = 9_000;
            page.rows[0].pin.finalized_cursor = PinManifestFinalizedCursorV1 {
                height: 9,
                block_hash: cursor(9).block_hash,
            };
            page.rows[0].provider_owner = Some(account(9));
            page.rows[0].completion_authority = Some(ProviderIngestCompletionAuthorityV1::new(
                account(9),
                completion_signer_policy(1),
            ));
            page.rows[0].completion_epoch = Some(9);
        }

        runtime.tick().await.expect("invalidate stale owner");
        assert_eq!(ingress.observe_calls.load(Ordering::SeqCst), 0);
        assert_eq!(*ingress.events.lock().unwrap(), vec!["prepare_signed"]);
        assert!(matches!(
            runtime.outbox.status(ingress.job_id).unwrap().state,
            ProviderIngestDeliveryStateV1::LocalStored {
                completion: ProviderIngestCompletionStateV1::Ready {
                    last_failure_class: Some(ProviderIngestFailureClassV1::ProviderOwnerChanged),
                    ..
                },
                ..
            }
        ));
    }

    #[tokio::test]
    async fn signer_policy_rotation_invalidates_never_exposed_signed_bytes_without_preflight() {
        let row = fixture_row(0x43);
        let (mut runtime, ledger, _, ingress) = test_runtime(
            row,
            true,
            Ok(vec![0xA5]),
            0,
            ProviderIngestIngressDispositionV1::Submitted,
            false,
        );
        *ingress.prepare_error.lock().unwrap() =
            Some(ProviderIngestIngressPrepareErrorV1::Unavailable);
        runtime
            .tick()
            .await
            .expect("sign before unavailable preflight");
        runtime
            .signer_resolver
            .signer_policy_revision
            .store(2, Ordering::SeqCst);
        {
            let mut page = ledger.page.lock().unwrap();
            page.finalized_cursor = cursor(9);
            page.finalized_block_time_ms = 9_000;
            page.rows[0].pin.finalized_cursor = PinManifestFinalizedCursorV1 {
                height: 9,
                block_hash: cursor(9).block_hash,
            };
            page.rows[0].completion_epoch = Some(9);
        }

        runtime
            .tick()
            .await
            .expect("invalidate stale signer policy");
        assert_eq!(ingress.observe_calls.load(Ordering::SeqCst), 0);
        assert_eq!(*ingress.events.lock().unwrap(), vec!["prepare_signed"]);
        assert!(matches!(
            runtime.outbox.status(ingress.job_id).unwrap().state,
            ProviderIngestDeliveryStateV1::LocalStored {
                completion: ProviderIngestCompletionStateV1::Ready {
                    last_failure_class: Some(ProviderIngestFailureClassV1::SignerPolicyChanged),
                    ..
                },
                ..
            }
        ));
    }

    #[tokio::test]
    async fn policy_rotation_after_durable_begin_never_reaches_ingress_exposure() {
        let row = fixture_row(0x44);
        let (mut runtime, _, _, ingress) = test_runtime(
            row,
            true,
            Ok(vec![0xA5]),
            0,
            ProviderIngestIngressDispositionV1::Submitted,
            false,
        );
        *ingress.prepare_error.lock().unwrap() =
            Some(ProviderIngestIngressPrepareErrorV1::Unavailable);
        let first = runtime
            .tick()
            .await
            .expect("retain signed bytes after unavailable preflight");
        assert_eq!(first.completions_signed, 1);
        assert_eq!(first.completion_submissions, 0);
        let next_attempt_at_ms = match runtime.outbox.status(ingress.job_id).unwrap().state {
            ProviderIngestDeliveryStateV1::LocalStored {
                completion:
                    ProviderIngestCompletionStateV1::Signed {
                        next_attempt_at_ms,
                        ever_exposed: false,
                        ..
                    },
                ..
            } => next_attempt_at_ms,
            other => panic!("expected a signed retry, got {other:?}"),
        };

        *ingress.prepare_error.lock().unwrap() = None;
        runtime
            .signer_resolver
            .eligibility_flip_on_call
            .store(3, Ordering::SeqCst);
        runtime
            .signer_resolver
            .eligibility_flip_to_revision
            .store(2, Ordering::SeqCst);
        runtime
            .clock
            .base_ms
            .store(next_attempt_at_ms, Ordering::SeqCst);

        let second = runtime
            .tick()
            .await
            .expect("policy loss after durable begin is retryable");
        assert_eq!(second.completions_signed, 0);
        assert_eq!(second.completion_submissions, 0);
        assert_eq!(
            *ingress.events.lock().unwrap(),
            vec!["prepare_signed", "prepare_signed"]
        );
        assert!(matches!(
            runtime.outbox.status(ingress.job_id).unwrap().state,
            ProviderIngestDeliveryStateV1::LocalStored {
                completion: ProviderIngestCompletionStateV1::Signed {
                    ever_exposed: true,
                    ..
                },
                ..
            }
        ));
    }

    #[tokio::test]
    async fn mutating_storage_soft_timeout_awaits_late_success_without_retry() {
        let row = fixture_row(0x3E);
        let (mut runtime, _, fetch, ingress) = test_runtime(
            row,
            false,
            Ok(vec![0xA5]),
            0,
            ProviderIngestIngressDispositionV1::Submitted,
            false,
        );

        let outcome = runtime.tick().await.expect("late atomic store");
        assert_eq!(fetch.calls.load(Ordering::SeqCst), 1);
        assert_eq!(outcome.manifests_stored, 1);
        assert!(matches!(
            runtime.outbox.status(ingress.job_id).unwrap().state,
            ProviderIngestDeliveryStateV1::LocalStored { .. }
        ));
    }

    #[tokio::test]
    async fn wrong_owner_signer_is_released_and_fails_the_supervised_runtime() {
        let row = fixture_row(0x36);
        let (mut runtime, _, _, ingress) = test_runtime(
            row,
            true,
            Ok(vec![0xA5]),
            0,
            ProviderIngestIngressDispositionV1::Submitted,
            true,
        );
        assert!(matches!(
            runtime.tick().await,
            Err(ProviderIngestRuntimeErrorV1::SignerProtocolViolation)
        ));
        assert!(matches!(
            runtime.outbox.status(ingress.job_id).unwrap().state,
            ProviderIngestDeliveryStateV1::LocalStored {
                completion: ProviderIngestCompletionStateV1::Ready {
                    last_failure_class: Some(ProviderIngestFailureClassV1::SignerUnavailable),
                    ..
                },
                ..
            }
        ));
    }

    #[tokio::test]
    async fn finalized_expiry_cancels_retained_work_without_fetching() {
        let mut row = fixture_row(0x37);
        let authorization = validate_assignment(&row, cursor(8), LOCAL_PROVIDER, runtime_policy())
            .unwrap()
            .authorization;
        row.order.status = ReplicationOrderStatus::Expired(8);
        let (mut runtime, _, fetch, ingress) = test_runtime(
            row,
            false,
            Ok(vec![0xA5]),
            0,
            ProviderIngestIngressDispositionV1::Submitted,
            false,
        );
        assert_eq!(authorization.job_id(), ingress.job_id);
        runtime.tick().await.expect("expiry tick");
        assert_eq!(fetch.calls.load(Ordering::SeqCst), 0);
        assert!(matches!(
            runtime.outbox.status(ingress.job_id).unwrap().state,
            ProviderIngestDeliveryStateV1::Cancelled {
                reason: ProviderIngestCancellationReasonV1::OrderExpired,
                ..
            }
        ));
    }

    #[tokio::test]
    async fn cooperative_shutdown_drains_active_store_before_skipping_next_row() {
        let first = fixture_row(0x3E);
        let second = fixture_row(0x40);
        let second_authorization =
            validate_assignment(&second, cursor(8), LOCAL_PROVIDER, runtime_policy())
                .expect("second assignment")
                .authorization;
        let (mut runtime, ledger, fetch, _) = test_runtime(
            first.clone(),
            false,
            Ok(vec![0xA5]),
            0,
            ProviderIngestIngressDispositionV1::Submitted,
            false,
        );
        ledger.page.lock().unwrap().rows = vec![first, second];
        let shutdown_requested = AtomicBool::new(false);

        let request_shutdown = async {
            tokio::time::sleep(Duration::from_millis(270)).await;
            shutdown_requested.store(true, Ordering::Release);
        };
        let (result, ()) = tokio::join!(
            runtime.tick_with_shutdown(&shutdown_requested),
            request_shutdown
        );
        let outcome = result.expect("drained cooperative shutdown");

        assert_eq!(outcome.rows_scanned, 1);
        assert_eq!(fetch.calls.load(Ordering::SeqCst), 1);
        assert!(matches!(
            runtime.outbox.status(second_authorization.job_id()),
            Err(ProviderIngestOutboxError::UnknownJob)
        ));
    }

    #[tokio::test]
    async fn pre_signalled_shutdown_returns_without_detaching_work() {
        let row = fixture_row(0x38);
        let (runtime, _, _, _) = test_runtime(
            row,
            true,
            Ok(vec![0xA5]),
            0,
            ProviderIngestIngressDispositionV1::Submitted,
            false,
        );
        let (sender, receiver) = watch::channel(true);
        runtime.run(receiver).await.expect("clean shutdown");
        drop(sender);
    }
}
