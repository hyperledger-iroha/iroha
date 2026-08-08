//! Supervised production wiring for finalized-ledger SoraFS provider ingest.
//!
//! Authoritative assignments come only from the daemon-owned immutable archive
//! captured inside the Sumeragi commit corridor. Runtime-only source
//! authentication and governed HSM/KMS signing remain deployment-injected
//! boundaries: config contains only identity-pinned opaque handles and public
//! revision/policy-digest qualifications.

use std::{
    cell::Cell,
    fmt,
    io::{self, Read},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};

use eyre::{Result, WrapErr, bail};
use iroha_config::parameters::{
    actual::SorafsProviderIngestRuntime,
    defaults::sorafs::storage::provider_ingest_runtime::outbox as provider_ingest_outbox_defaults,
    is_production_runtime_handle,
};
use iroha_core::{
    queue::{Error as QueueError, Queue},
    state::{State, StateReadOnly as _, WorldReadOnly as _, WorldStateSnapshot as _},
    tx::AcceptedTransaction,
};
use iroha_crypto::{Hash, HashOf};
use iroha_data_model::{
    ChainId,
    account::AccountId,
    block::BlockHeader,
    isi::sorafs::CompleteReplicationOrder,
    musubi::MusubiArchiveCommitmentV1,
    sorafs::{
        capacity::ProviderId,
        pin_registry::{
            PinManifestRecord, PinStatus, ProviderIngestFinalizedAnchorV1, ReplicationOrderId,
            ReplicationOrderRecord, ReplicationOrderStatus,
        },
    },
    transaction::{
        FeePaymentIntent, SignedTransaction, TransactionBuilder, TransactionEntrypoint,
        TransactionPayload,
    },
};
use iroha_futures::supervisor::{Child, OnShutdown, ShutdownSignal};
use mv::storage::StorageReadOnly as _;
use norito::{core::DecodeLimits, decode_from_bytes_with_limits};
use rand::{rand_core::TryRngCore as _, rngs::OsRng};
use sorafs_car::{
    CarBuildPlan, ChunkStoreError, compute_chunk_plan_digest_sha3,
    musubi::{MusubiBundleVerifierV1, VerifiedMusubiBundleV1},
};
use sorafs_manifest::{
    ManifestV1,
    capacity::{
        MAX_CAPACITY_METADATA_VALUE_BYTES, MAX_REPLICATION_ORDER_ASSIGNMENTS, ReplicationOrderV1,
    },
    validate_registered_chunker_profile,
};
use sorafs_node::provider_ingest_runtime::{
    ProviderIngestAuthenticatedSourcePoolV1, ProviderIngestCompletionSignerBindingV1,
    ProviderIngestCompletionSignerQualificationV1, ProviderIngestLocalStoredV1,
    ProviderIngestRuntimeProviderQualificationV1, ProviderIngestVerifiedMusubiBundleReceiptV1,
};
use sorafs_node::{
    AdmittedPayloadReadLeaseErrorV1, FinalizedProviderIngestAuthorizationV1, NodeHandle,
    NodeStorageError, ProviderIngestAuthenticatedSourceFetchV1, ProviderIngestClaimOwnerV1,
    ProviderIngestCompletionPayloadBuilderV1, ProviderIngestCompletionPayloadErrorV1,
    ProviderIngestCompletionPayloadRequestV1, ProviderIngestCompletionSignerErrorV1,
    ProviderIngestCompletionSignerPolicyV1, ProviderIngestCompletionSignerResolutionContextV1,
    ProviderIngestCompletionSignerResolverErrorV1, ProviderIngestCompletionSignerResolverV1,
    ProviderIngestCompletionSignerV1, ProviderIngestFinalizedAssignmentPageV1,
    ProviderIngestFinalizedClaimFactoryV1, ProviderIngestFinalizedCursorV1,
    ProviderIngestFinalizedLedgerErrorV1, ProviderIngestFinalizedLedgerV1,
    ProviderIngestFinalizedMusubiArchiveClaimV1, ProviderIngestFinalizedMusubiCompletionClaimV1,
    ProviderIngestFutureV1, ProviderIngestIngressDispositionV1,
    ProviderIngestIngressPrepareErrorV1, ProviderIngestLocalStorageErrorV1,
    ProviderIngestLocalStorageV1, ProviderIngestMusubiAttestationApprovalRequestV1,
    ProviderIngestRuntimeErrorV1, ProviderIngestRuntimePolicyV1, ProviderIngestRuntimeV1,
    ProviderIngestSourceFetchErrorV1, ProviderIngestSourceRequestV1, ProviderIngestSystemClockV1,
    ProviderIngestTickOutcomeV1, ProviderIngestTransactionIngressV1,
    ProviderIngestTransactionObservationV1,
    store::{StorageError, StoredManifest},
};

use crate::sorafs_provider_ingest_finalized_query::ArchivedProviderIngestFinalizedLedgerV1;

const SHUTDOWN_WAIT_FLOOR: Duration = Duration::from_secs(2);
const READINESS_STALE_TICK_MULTIPLIER_V1: u32 = 3;
const REPLICATION_ORDER_MAX_CANONICAL_BYTES_V1: usize = 256 * 1024;
const REPLICATION_ORDER_DECODE_LIMITS_V1: DecodeLimits = DecodeLimits::new(
    MAX_CAPACITY_METADATA_VALUE_BYTES,
    REPLICATION_ORDER_MAX_CANONICAL_BYTES_V1,
    131_072,
    REPLICATION_ORDER_MAX_CANONICAL_BYTES_V1 * 4,
    32,
);

/// Exact verified source material passed directly into local `SoraFS` storage.
///
/// The reader may stream from a bounded authenticated transport or a
/// deployment-owned temporary object. Its manifest, plan, declared length,
/// PoR root, and governed advert have already been authenticated. Payload
/// digest, exact length, zero trailing bytes, and any post-stream provider
/// qualification are finalized only when the reader reaches authenticated
/// EOF; dropping or failing the reader before EOF invalidates the stream.
/// Every underlying read must carry a hard transport deadline no longer than
/// the configured source-operation deadline. URLs, grants, bearer tokens, and
/// credentials are intentionally absent.
pub struct VerifiedProviderIngestPayloadV1 {
    /// Canonical manifest returned by the authenticated source.
    pub manifest: ManifestV1,
    /// Exact CAR build plan bound to `manifest`.
    pub plan: CarBuildPlan,
    /// Authenticated payload stream whose verification completes at exact EOF.
    pub reader: Box<dyn Read + Send>,
}

impl fmt::Debug for VerifiedProviderIngestPayloadV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("VerifiedProviderIngestPayloadV1")
            .field("manifest_digest", &self.manifest.digest().ok())
            .field("content_length", &self.plan.content_length)
            .field("reader", &"<verified stream>")
            .finish()
    }
}

impl VerifiedProviderIngestPayloadV1 {
    /// Construct exact verified material without copying payload bytes.
    #[must_use]
    pub fn new(manifest: ManifestV1, plan: CarBuildPlan, reader: Box<dyn Read + Send>) -> Self {
        Self {
            manifest,
            plan,
            reader,
        }
    }
}

/// Runtime-only authenticated source provider.
///
/// Implementations must resolve only current governance-admitted signed
/// adverts, use authenticated bounded grants with pinned trust, and perform
/// exact streaming verification before returning. Secrets and source
/// locations must never be exposed through `Debug`, logs, or readiness
/// artifacts.
pub trait ProviderIngestAuthenticatedSourceRuntimeV1:
    ProviderIngestAuthenticatedSourceFetchV1<Fetched = VerifiedProviderIngestPayloadV1>
{
    /// Stable production identity compared with `iroha_config`.
    fn runtime_handle(&self) -> &str;

    /// Exact public source-pool revision and policy digest.
    ///
    /// # Errors
    ///
    /// Returns a payload-free failure when the public qualification cannot be
    /// authenticated.
    fn qualification(
        &self,
    ) -> std::result::Result<
        ProviderIngestRuntimeProviderQualificationV1,
        ProviderIngestSourceFetchErrorV1,
    >;

    /// Canonical, identity-pinned governed provider inventory.
    ///
    /// Production startup requires at least two non-local providers and reads
    /// this exact slice again after readiness probing and on every worker tick.
    fn source_provider_ids(&self) -> &[[u8; 32]];

    /// Non-mutating authenticated readiness probe.
    ///
    /// # Errors
    ///
    /// Returns a payload-free failure when the authenticated source cannot be
    /// qualified without mutation.
    fn check_readiness(&self) -> std::result::Result<(), ProviderIngestSourceFetchErrorV1>;
}

impl ProviderIngestAuthenticatedSourceRuntimeV1
    for ProviderIngestAuthenticatedSourcePoolV1<VerifiedProviderIngestPayloadV1>
{
    fn runtime_handle(&self) -> &str {
        ProviderIngestAuthenticatedSourcePoolV1::runtime_handle(self)
    }

    fn qualification(
        &self,
    ) -> std::result::Result<
        ProviderIngestRuntimeProviderQualificationV1,
        ProviderIngestSourceFetchErrorV1,
    > {
        Ok(ProviderIngestAuthenticatedSourcePoolV1::qualification(self))
    }

    fn source_provider_ids(&self) -> &[[u8; 32]] {
        ProviderIngestAuthenticatedSourcePoolV1::source_provider_ids(self)
    }

    fn check_readiness(&self) -> std::result::Result<(), ProviderIngestSourceFetchErrorV1> {
        ProviderIngestAuthenticatedSourcePoolV1::check_readiness(self)
    }
}

/// Runtime-only governed signer resolver.
///
/// Resolution must validate the requested owner, signer policy, and exact
/// non-zero assignment revision against immutable governance/assignment state
/// at the supplied finalized cursor, including current key rotation and
/// revocation. It must not infer any of those values from the transaction
/// payload. The returned signer must repeat the resolved checks atomically with
/// signing and sign only a payload matching that independently pinned context.
pub trait ProviderIngestGovernedSignerResolverRuntimeV1: Send + Sync + 'static {
    /// Stable production identity compared with `iroha_config`.
    fn runtime_handle(&self) -> &str;

    /// Exact public resolver revision and policy digest.
    ///
    /// # Errors
    ///
    /// Returns a payload-free failure when the public qualification cannot be
    /// authenticated.
    fn qualification(
        &self,
    ) -> std::result::Result<
        ProviderIngestRuntimeProviderQualificationV1,
        ProviderIngestCompletionSignerResolverErrorV1,
    >;

    /// Exact public signer/key binding exposed by this resolver.
    ///
    /// # Errors
    ///
    /// Returns a fixed payload-free resolver failure when the binding cannot
    /// be authenticated.
    fn signer_binding(
        &self,
    ) -> std::result::Result<
        ProviderIngestCompletionSignerBindingV1,
        ProviderIngestCompletionSignerResolverErrorV1,
    >;

    /// Non-mutating HSM/KMS and governance-readiness probe.
    ///
    /// # Errors
    ///
    /// Returns a payload-free failure when the configured signer binding
    /// cannot be qualified without mutation.
    fn check_readiness(
        &self,
    ) -> std::result::Result<(), ProviderIngestCompletionSignerResolverErrorV1>;

    /// Resolve one governed signer for the exact finalized authorization.
    fn resolve(
        &self,
        context: ProviderIngestCompletionSignerResolutionContextV1,
    ) -> ProviderIngestFutureV1<
        '_,
        std::result::Result<
            Option<Arc<dyn ProviderIngestCompletionSignerV1>>,
            ProviderIngestCompletionSignerResolverErrorV1,
        >,
    >;
}

fn validate_authenticated_source_qualification(
    source: &dyn ProviderIngestAuthenticatedSourceRuntimeV1,
    expected: ProviderIngestRuntimeProviderQualificationV1,
) -> std::result::Result<(), ProviderIngestSourceFetchErrorV1> {
    if !expected.is_valid() {
        return Err(ProviderIngestSourceFetchErrorV1::Rejected);
    }
    let actual = match source.qualification() {
        Ok(qualification) => qualification,
        Err(ProviderIngestSourceFetchErrorV1::Unavailable) => {
            return Err(ProviderIngestSourceFetchErrorV1::Unavailable);
        }
        Err(
            ProviderIngestSourceFetchErrorV1::ContentRejected
            | ProviderIngestSourceFetchErrorV1::Rejected,
        ) => return Err(ProviderIngestSourceFetchErrorV1::Rejected),
    };
    if !actual.is_valid() || actual != expected {
        return Err(ProviderIngestSourceFetchErrorV1::Rejected);
    }
    Ok(())
}

#[derive(Clone)]
struct AuthenticatedSourceAdapterV1 {
    source: Arc<dyn ProviderIngestAuthenticatedSourceRuntimeV1>,
    expected_qualification: ProviderIngestRuntimeProviderQualificationV1,
}

impl ProviderIngestAuthenticatedSourceFetchV1 for AuthenticatedSourceAdapterV1 {
    type Fetched = VerifiedProviderIngestPayloadV1;

    fn fetch(
        &self,
        request: ProviderIngestSourceRequestV1,
    ) -> ProviderIngestFutureV1<
        '_,
        std::result::Result<Self::Fetched, ProviderIngestSourceFetchErrorV1>,
    > {
        Box::pin(async move {
            validate_authenticated_source_qualification(
                self.source.as_ref(),
                self.expected_qualification,
            )?;
            let result = self.source.fetch(request).await;
            let post_fetch_qualification = validate_authenticated_source_qualification(
                self.source.as_ref(),
                self.expected_qualification,
            );
            match (result, post_fetch_qualification) {
                (
                    Err(ProviderIngestSourceFetchErrorV1::Rejected),
                    Ok(()) | Err(ProviderIngestSourceFetchErrorV1::Unavailable),
                )
                | (
                    _,
                    Err(
                        ProviderIngestSourceFetchErrorV1::ContentRejected
                        | ProviderIngestSourceFetchErrorV1::Rejected,
                    ),
                ) => Err(ProviderIngestSourceFetchErrorV1::Rejected),
                (
                    Err(ProviderIngestSourceFetchErrorV1::ContentRejected),
                    Err(ProviderIngestSourceFetchErrorV1::Unavailable),
                ) => Err(ProviderIngestSourceFetchErrorV1::ContentRejected),
                (_, Err(ProviderIngestSourceFetchErrorV1::Unavailable)) => {
                    Err(ProviderIngestSourceFetchErrorV1::Unavailable)
                }
                (result, Ok(())) => result,
            }
        })
    }
}

#[derive(Clone)]
struct GovernedCompletionSignerV1 {
    signer: Arc<dyn ProviderIngestCompletionSignerV1>,
    owner_authority: Arc<dyn ProviderIngestFinalizedOwnerAuthorityV1>,
    provider_id: ProviderId,
    expected_context: ProviderIngestCompletionSignerResolutionContextV1,
    expected_binding: ProviderIngestCompletionSignerBindingV1,
}

fn completion_payload_matches_resolution_context(
    payload: &TransactionPayload,
    context: &ProviderIngestCompletionSignerResolutionContextV1,
    expected_provider_id: ProviderId,
) -> bool {
    if !context.is_valid() || payload.authority() != &context.provider_owner {
        return false;
    }
    let iroha_data_model::transaction::Executable::Instructions(instructions) =
        payload.instructions()
    else {
        return false;
    };
    if instructions.len() != 1 {
        return false;
    }
    let Some(completion) = instructions[0]
        .as_any()
        .downcast_ref::<CompleteReplicationOrder>()
    else {
        return false;
    };
    let authority = completion.expected_authority();
    let anchor = completion.finalized_anchor();
    completion.order_id().as_bytes() != &[0; 32]
        && completion.provider_id() == &expected_provider_id
        && *completion.completion_epoch() != 0
        && *completion.expected_assignment_revision() == context.expected_assignment_revision
        && authority.is_valid()
        && authority.provider_owner == context.provider_owner
        && authority.signer_policy == context.signer_policy
        && anchor.height == context.finalized_cursor.height
        && anchor.block_hash == context.finalized_cursor.block_hash
}

impl ProviderIngestCompletionSignerV1 for GovernedCompletionSignerV1 {
    fn runtime_handle(&self) -> &str {
        self.signer.runtime_handle()
    }

    fn authority(&self) -> &AccountId {
        self.signer.authority()
    }

    fn qualification(
        &self,
    ) -> std::result::Result<
        ProviderIngestCompletionSignerQualificationV1,
        ProviderIngestCompletionSignerErrorV1,
    > {
        let qualification = self.signer.qualification()?;
        if self.expected_binding.validate().is_err()
            || self.signer.runtime_handle() != self.expected_binding.runtime_handle.as_str()
            || qualification != self.expected_binding.qualification
            || qualification.validate().is_err()
            || !qualification.matches_authority(&self.expected_context.provider_owner)
        {
            return Err(ProviderIngestCompletionSignerErrorV1::Unavailable);
        }
        Ok(qualification)
    }

    fn signer_policy(&self) -> ProviderIngestCompletionSignerPolicyV1 {
        self.signer.signer_policy()
    }

    fn current_eligibility(
        &self,
    ) -> std::result::Result<
        ProviderIngestCompletionSignerPolicyV1,
        ProviderIngestCompletionSignerErrorV1,
    > {
        let qualification = self.qualification()?;
        let current_policy = self.signer.current_eligibility()?;
        if !self
            .owner_authority
            .owner_matches(self.provider_id, &self.expected_context.provider_owner)
            || self.signer.authority() != &self.expected_context.provider_owner
            || self.signer.signer_policy() != current_policy
            || current_policy != qualification.signer_policy
            || current_policy != self.expected_context.signer_policy
            || !current_policy.is_valid()
        {
            return Err(ProviderIngestCompletionSignerErrorV1::Unavailable);
        }
        Ok(current_policy)
    }

    fn sign(
        &self,
        payload: TransactionPayload,
    ) -> ProviderIngestFutureV1<
        '_,
        std::result::Result<SignedTransaction, ProviderIngestCompletionSignerErrorV1>,
    > {
        Box::pin(async move {
            if !completion_payload_matches_resolution_context(
                &payload,
                &self.expected_context,
                self.provider_id,
            ) {
                return Err(ProviderIngestCompletionSignerErrorV1::Rejected);
            }
            self.current_eligibility()?;
            let expected_payload = payload.clone();
            let transaction = self.signer.sign(payload).await?;
            self.current_eligibility()?;
            if transaction.payload() != &expected_payload
                || !completion_payload_matches_resolution_context(
                    transaction.payload(),
                    &self.expected_context,
                    self.provider_id,
                )
            {
                return Err(ProviderIngestCompletionSignerErrorV1::Rejected);
            }
            Ok(transaction)
        })
    }
}

#[derive(Clone)]
struct GovernedSignerResolverAdapterV1 {
    resolver: Arc<dyn ProviderIngestGovernedSignerResolverRuntimeV1>,
    owner_authority: Arc<dyn ProviderIngestFinalizedOwnerAuthorityV1>,
    provider_id: ProviderId,
    expected_resolver_qualification: ProviderIngestRuntimeProviderQualificationV1,
    expected_signer_binding: ProviderIngestCompletionSignerBindingV1,
}

fn validate_resolver_qualification(
    resolver: &dyn ProviderIngestGovernedSignerResolverRuntimeV1,
    expected: ProviderIngestRuntimeProviderQualificationV1,
) -> std::result::Result<(), ProviderIngestCompletionSignerResolverErrorV1> {
    if !expected.is_valid() {
        return Err(ProviderIngestCompletionSignerResolverErrorV1::Rejected);
    }
    let actual = resolver.qualification()?;
    if !actual.is_valid() || actual != expected {
        return Err(ProviderIngestCompletionSignerResolverErrorV1::Rejected);
    }
    Ok(())
}

fn validate_resolver_signer_binding(
    resolver: &dyn ProviderIngestGovernedSignerResolverRuntimeV1,
    expected: &ProviderIngestCompletionSignerBindingV1,
) -> std::result::Result<(), ProviderIngestCompletionSignerResolverErrorV1> {
    if expected.validate().is_err() {
        return Err(ProviderIngestCompletionSignerResolverErrorV1::Rejected);
    }
    let actual = resolver.signer_binding()?;
    if actual.validate().is_err() || &actual != expected {
        return Err(ProviderIngestCompletionSignerResolverErrorV1::Rejected);
    }
    Ok(())
}

impl ProviderIngestCompletionSignerResolverV1 for GovernedSignerResolverAdapterV1 {
    type Signer = GovernedCompletionSignerV1;

    fn resolve(
        &self,
        context: ProviderIngestCompletionSignerResolutionContextV1,
    ) -> ProviderIngestFutureV1<
        '_,
        std::result::Result<Option<Self::Signer>, ProviderIngestCompletionSignerResolverErrorV1>,
    > {
        Box::pin(async move {
            if !context.is_valid()
                || context.signer_policy != self.expected_signer_binding.qualification.signer_policy
            {
                return Err(ProviderIngestCompletionSignerResolverErrorV1::Rejected);
            }
            validate_resolver_qualification(
                self.resolver.as_ref(),
                self.expected_resolver_qualification,
            )?;
            validate_resolver_signer_binding(
                self.resolver.as_ref(),
                &self.expected_signer_binding,
            )?;
            if !self
                .owner_authority
                .owner_matches(self.provider_id, &context.provider_owner)
            {
                return Err(ProviderIngestCompletionSignerResolverErrorV1::Unavailable);
            }
            let expected_context = context.clone();
            let signer = self.resolver.resolve(context).await;
            validate_resolver_qualification(
                self.resolver.as_ref(),
                self.expected_resolver_qualification,
            )?;
            validate_resolver_signer_binding(
                self.resolver.as_ref(),
                &self.expected_signer_binding,
            )?;
            let signer = signer?;
            let Some(signer) = signer else {
                return Ok(None);
            };
            let signer_qualification = match signer.qualification() {
                Ok(qualification) => qualification,
                Err(ProviderIngestCompletionSignerErrorV1::Unavailable) => {
                    return Err(ProviderIngestCompletionSignerResolverErrorV1::Unavailable);
                }
                Err(ProviderIngestCompletionSignerErrorV1::Rejected) => {
                    return Err(ProviderIngestCompletionSignerResolverErrorV1::Rejected);
                }
            };
            let expected_policy = match signer.current_eligibility() {
                Ok(policy) => policy,
                Err(ProviderIngestCompletionSignerErrorV1::Unavailable) => {
                    return Err(ProviderIngestCompletionSignerResolverErrorV1::Unavailable);
                }
                Err(ProviderIngestCompletionSignerErrorV1::Rejected) => {
                    return Err(ProviderIngestCompletionSignerResolverErrorV1::Rejected);
                }
            };
            if !self
                .owner_authority
                .owner_matches(self.provider_id, &expected_context.provider_owner)
            {
                return Err(ProviderIngestCompletionSignerResolverErrorV1::Unavailable);
            }
            if signer.authority() != &expected_context.provider_owner
                || signer.signer_policy() != expected_policy
                || !expected_policy.is_valid()
                || expected_policy != expected_context.signer_policy
                || signer.runtime_handle() != self.expected_signer_binding.runtime_handle.as_str()
                || signer_qualification != self.expected_signer_binding.qualification
                || signer_qualification.validate().is_err()
                || !signer_qualification.matches_authority(&expected_context.provider_owner)
                || expected_policy != signer_qualification.signer_policy
            {
                return Err(ProviderIngestCompletionSignerResolverErrorV1::Rejected);
            }
            Ok(Some(GovernedCompletionSignerV1 {
                signer,
                owner_authority: Arc::clone(&self.owner_authority),
                provider_id: self.provider_id,
                expected_context,
                expected_binding: self.expected_signer_binding.clone(),
            }))
        })
    }
}

trait ProviderIngestFinalizedOwnerAuthorityV1: Send + Sync + 'static {
    fn owner_matches(&self, provider_id: ProviderId, expected_owner: &AccountId) -> bool;
}

impl ProviderIngestFinalizedOwnerAuthorityV1 for State {
    fn owner_matches(&self, provider_id: ProviderId, expected_owner: &AccountId) -> bool {
        self.query_view()
            .world()
            .provider_owners()
            .get(&provider_id)
            == Some(expected_owner)
    }
}

#[derive(Debug, Clone, Copy)]
struct FinalizedSnapshotProbeV1 {
    completed_cursor: Option<ProviderIngestFinalizedCursorV1>,
}

/// Archive-only finalized assignment reader with a payload-free terminal-page
/// observation used by daemon readiness.
#[derive(Clone)]
struct ObservedArchivedFinalizedAssignmentLedgerV1 {
    archived: Arc<ArchivedProviderIngestFinalizedLedgerV1>,
    probe: Arc<Mutex<FinalizedSnapshotProbeV1>>,
}

impl ObservedArchivedFinalizedAssignmentLedgerV1 {
    fn new(
        archived: Arc<ArchivedProviderIngestFinalizedLedgerV1>,
        probe: Arc<Mutex<FinalizedSnapshotProbeV1>>,
    ) -> Self {
        Self { archived, probe }
    }
}

impl ProviderIngestFinalizedLedgerV1 for ObservedArchivedFinalizedAssignmentLedgerV1 {
    fn read_assignment_page(
        &self,
        claim_factory: ProviderIngestFinalizedClaimFactoryV1,
        at_finalized_cursor: Option<ProviderIngestFinalizedCursorV1>,
        after_order_id: Option<[u8; 32]>,
        limit: usize,
    ) -> ProviderIngestFutureV1<
        '_,
        std::result::Result<
            ProviderIngestFinalizedAssignmentPageV1,
            ProviderIngestFinalizedLedgerErrorV1,
        >,
    > {
        let archived = Arc::clone(&self.archived);
        let probe = Arc::clone(&self.probe);
        Box::pin(async move {
            let page = archived
                .read_assignment_page(claim_factory, at_finalized_cursor, after_order_id, limit)
                .await?;
            if page.next_after_order_id.is_none() {
                probe
                    .lock()
                    .map_err(|_| ProviderIngestFinalizedLedgerErrorV1::Unavailable)?
                    .completed_cursor = Some(page.finalized_cursor);
            }
            Ok(page)
        })
    }
}

#[derive(Clone)]
struct NativeProviderIngestLocalStorageV1 {
    node: NodeHandle,
    operation_timeout: Duration,
}

impl NativeProviderIngestLocalStorageV1 {
    fn new(node: NodeHandle, operation_timeout: Duration) -> Self {
        Self {
            node,
            operation_timeout,
        }
    }

    // TODO: Invoke this boundary from the finalized post-completion attestation coordinator once
    // its governed signer/approval handoff exists. Keeping it uncalled makes this slice externally
    // inert while fixing the only admissible way to derive the unsigned request.
    #[allow(
        dead_code,
        reason = "the post-completion coordinator is intentionally not activated in this slice"
    )]
    fn prepare_musubi_attestation_approval_request(
        &self,
        retained_authorization: &FinalizedProviderIngestAuthorizationV1,
        completed_claim: &ProviderIngestFinalizedMusubiCompletionClaimV1,
    ) -> std::result::Result<
        ProviderIngestMusubiAttestationApprovalRequestV1,
        ProviderIngestLocalStorageErrorV1,
    > {
        if !completed_claim.matches_authorization(retained_authorization) {
            return Err(ProviderIngestLocalStorageErrorV1::Permanent);
        }
        let stored = self
            .node
            .manifest_metadata_by_digest(&retained_authorization.manifest_digest())
            .map_err(|error| classify_completed_attestation_manifest_lookup_error(&error))?;
        if stored.manifest_digest() != &retained_authorization.manifest_digest()
            || stored.manifest_cid() != retained_authorization.manifest_cid()
            || stored.content_length() != retained_authorization.content_length()
            || stored.chunk_profile_handle() != retained_authorization.chunker_handle()
        {
            return Err(ProviderIngestLocalStorageErrorV1::Permanent);
        }
        let manifest = stored
            .load_manifest()
            .map_err(|error| classify_storage_backend_error(&error))?;
        validate_manifest_binding(retained_authorization, &manifest)?;
        let registered_profile = validate_registered_chunker_profile(&manifest.chunking)
            .map_err(|_| ProviderIngestLocalStorageErrorV1::Permanent)?;
        let plan = stored
            .try_to_car_plan_with_hint(registered_profile.profile, None)
            .map_err(|error| classify_storage_backend_error(&error))?;
        validate_verified_payload(retained_authorization, &manifest, &plan)?;

        self.node
            .with_admitted_payload_read_lease(&retained_authorization.manifest_digest(), |lease| {
                lease.verify_completed_musubi_bundle(&plan, retained_authorization, completed_claim)
            })
            .map_err(classify_admitted_payload_lease_error)?
    }
}

struct DeadlineBoundedReaderV1 {
    inner: Box<dyn Read + Send>,
    deadline: Instant,
    remaining: u64,
    terminal_state: DeadlineBoundedReaderTerminalStateV1,
    #[cfg(test)]
    clock: Arc<dyn Fn() -> Instant + Send + Sync>,
}

struct ObservedAdmittedPayloadReaderV1<'observation, R> {
    inner: R,
    first_error_kind: &'observation Cell<Option<io::ErrorKind>>,
}

impl<R: Read> Read for ObservedAdmittedPayloadReaderV1<'_, R> {
    fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
        self.inner.read(output).inspect_err(|error| {
            if self.first_error_kind.get().is_none() {
                self.first_error_kind.set(Some(error.kind()));
            }
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DeadlineBoundedReaderTerminalStateV1 {
    Pending,
    Authenticated,
    Failed(io::ErrorKind),
}

impl DeadlineBoundedReaderV1 {
    fn new(inner: Box<dyn Read + Send>, timeout: Duration, expected_bytes: u64) -> Self {
        Self {
            inner,
            deadline: Instant::now()
                .checked_add(timeout)
                .unwrap_or_else(Instant::now),
            remaining: expected_bytes,
            terminal_state: DeadlineBoundedReaderTerminalStateV1::Pending,
            #[cfg(test)]
            clock: Arc::new(Instant::now),
        }
    }

    #[cfg(test)]
    fn new_with_clock(
        inner: Box<dyn Read + Send>,
        timeout: Duration,
        expected_bytes: u64,
        clock: Arc<dyn Fn() -> Instant + Send + Sync>,
    ) -> Self {
        let started_at = clock();
        Self {
            inner,
            deadline: started_at.checked_add(timeout).unwrap_or(started_at),
            remaining: expected_bytes,
            terminal_state: DeadlineBoundedReaderTerminalStateV1::Pending,
            clock,
        }
    }

    #[cfg(not(test))]
    fn current_time(&self) -> Instant {
        Instant::now()
    }

    #[cfg(test)]
    fn current_time(&self) -> Instant {
        (self.clock)()
    }

    fn failure(&mut self, kind: io::ErrorKind, message: &'static str) -> io::Error {
        self.terminal_state = DeadlineBoundedReaderTerminalStateV1::Failed(kind);
        io::Error::new(kind, message)
    }

    fn record_inner_failure(&mut self, error: io::Error) -> io::Error {
        self.terminal_state = DeadlineBoundedReaderTerminalStateV1::Failed(error.kind());
        error
    }

    fn require_live_deadline(&mut self) -> io::Result<()> {
        if self.current_time() >= self.deadline {
            return Err(self.failure(
                io::ErrorKind::TimedOut,
                "provider-ingest verified reader exceeded its operation deadline",
            ));
        }
        Ok(())
    }

    fn authenticate_terminal_eof(&mut self) -> io::Result<usize> {
        self.require_live_deadline()?;
        let mut trailing = [0_u8; 1];
        let result = self.inner.read(&mut trailing);
        self.require_live_deadline()?;
        match result {
            Ok(0) => {
                self.terminal_state = DeadlineBoundedReaderTerminalStateV1::Authenticated;
                Ok(0)
            }
            Ok(_) => Err(self.failure(
                io::ErrorKind::InvalidData,
                "provider-ingest verified reader exceeded its authorized length",
            )),
            Err(error) => Err(self.record_inner_failure(error)),
        }
    }
}

impl Read for DeadlineBoundedReaderV1 {
    fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
        if output.is_empty() {
            return Ok(0);
        }
        match self.terminal_state {
            DeadlineBoundedReaderTerminalStateV1::Authenticated => return Ok(0),
            DeadlineBoundedReaderTerminalStateV1::Failed(kind) => {
                return Err(io::Error::new(
                    kind,
                    "provider-ingest verified reader previously failed",
                ));
            }
            DeadlineBoundedReaderTerminalStateV1::Pending => {}
        }
        if self.remaining == 0 {
            return self.authenticate_terminal_eof();
        }
        self.require_live_deadline()?;
        let remaining = usize::try_from(self.remaining).unwrap_or(usize::MAX);
        let read_limit = remaining.min(output.len());
        let result = self.inner.read(&mut output[..read_limit]);
        self.require_live_deadline()?;
        let read = match result {
            Ok(0) => {
                return Err(self.failure(
                    io::ErrorKind::UnexpectedEof,
                    "provider-ingest verified reader ended before its authorized length",
                ));
            }
            Ok(read) if read <= read_limit => read,
            Ok(_) => {
                return Err(self.failure(
                    io::ErrorKind::InvalidData,
                    "provider-ingest verified reader violated its bounded read contract",
                ));
            }
            Err(error) => return Err(self.record_inner_failure(error)),
        };
        let Some(remaining) = self
            .remaining
            .checked_sub(u64::try_from(read).unwrap_or(u64::MAX))
        else {
            return Err(self.failure(
                io::ErrorKind::InvalidData,
                "provider-ingest verified reader exceeded its authorized length",
            ));
        };
        self.remaining = remaining;
        Ok(read)
    }
}

struct BlockingStoreJoinGuardV1(Option<std::thread::JoinHandle<()>>);

impl BlockingStoreJoinGuardV1 {
    fn join(mut self) -> bool {
        self.0.take().is_some_and(|thread| thread.join().is_ok())
    }
}

impl Drop for BlockingStoreJoinGuardV1 {
    fn drop(&mut self) {
        if let Some(thread) = self.0.take() {
            let _ = thread.join();
        }
    }
}

impl ProviderIngestLocalStorageV1<VerifiedProviderIngestPayloadV1>
    for NativeProviderIngestLocalStorageV1
{
    fn verify_existing(
        &self,
        authorization: FinalizedProviderIngestAuthorizationV1,
        musubi_archive: Option<ProviderIngestFinalizedMusubiArchiveClaimV1>,
    ) -> ProviderIngestFutureV1<
        '_,
        std::result::Result<Option<ProviderIngestLocalStoredV1>, ProviderIngestLocalStorageErrorV1>,
    > {
        let node = self.node.clone();
        Box::pin(async move {
            tokio::task::spawn_blocking(move || {
                verify_existing_manifest(&node, &authorization, musubi_archive.as_ref())
            })
            .await
            .map_err(|_| ProviderIngestLocalStorageErrorV1::Retryable)?
        })
    }

    fn store(
        &self,
        authorization: FinalizedProviderIngestAuthorizationV1,
        musubi_archive: Option<ProviderIngestFinalizedMusubiArchiveClaimV1>,
        mut fetched: VerifiedProviderIngestPayloadV1,
    ) -> ProviderIngestFutureV1<
        '_,
        std::result::Result<ProviderIngestLocalStoredV1, ProviderIngestLocalStorageErrorV1>,
    > {
        let node = self.node.clone();
        let operation_timeout = self.operation_timeout;
        Box::pin(async move {
            validate_verified_payload(&authorization, &fetched.manifest, &fetched.plan)?;
            fetched.reader = Box::new(DeadlineBoundedReaderV1::new(
                fetched.reader,
                operation_timeout,
                authorization.content_length(),
            ));
            let (result_sender, result_receiver) = tokio::sync::oneshot::channel();
            let thread = std::thread::Builder::new()
                .name("sorafs-provider-ingest-store".to_owned())
                .spawn(move || {
                    let result = match node.ingest_manifest(
                        &fetched.manifest,
                        &fetched.plan,
                        &mut fetched.reader,
                    ) {
                        Ok(manifest_id) => {
                            // TODO: Quarantine a newly admitted, permanently rejected Musubi
                            // payload through an audited receipt-bound transition once that
                            // primitive exists. Raw eviction here could delete a concurrently
                            // reused generic SoraFS object. Until then the bytes remain admitted,
                            // but this path cannot return the receipt required for completion.
                            match verify_existing_manifest(
                                &node,
                                &authorization,
                                musubi_archive.as_ref(),
                            ) {
                                Ok(Some(stored))
                                    if stored.manifest_id() == manifest_id.as_str() =>
                                {
                                    Ok(stored)
                                }
                                Ok(Some(_)) | Ok(None) => {
                                    Err(ProviderIngestLocalStorageErrorV1::Permanent)
                                }
                                Err(error) => Err(error),
                            }
                        }
                        Err(NodeStorageError::Storage(StorageError::ManifestExists {
                            manifest_id,
                        })) => {
                            match verify_existing_manifest(
                                &node,
                                &authorization,
                                musubi_archive.as_ref(),
                            ) {
                                Ok(Some(stored)) if stored.manifest_id() == manifest_id => {
                                    Ok(stored)
                                }
                                Ok(Some(_)) | Ok(None) => {
                                    Err(ProviderIngestLocalStorageErrorV1::Permanent)
                                }
                                Err(error) => Err(error),
                            }
                        }
                        Err(error) => Err(classify_storage_error(&error)),
                    };
                    let _ = result_sender.send(result);
                })
                .map_err(|_| ProviderIngestLocalStorageErrorV1::Retryable)?;
            let guard = BlockingStoreJoinGuardV1(Some(thread));
            let result = result_receiver
                .await
                .map_err(|_| ProviderIngestLocalStorageErrorV1::Retryable)?;
            if !guard.join() {
                return Err(ProviderIngestLocalStorageErrorV1::Retryable);
            }
            result
        })
    }
}

fn verify_existing_manifest(
    node: &NodeHandle,
    authorization: &FinalizedProviderIngestAuthorizationV1,
    musubi_archive: Option<&ProviderIngestFinalizedMusubiArchiveClaimV1>,
) -> std::result::Result<Option<ProviderIngestLocalStoredV1>, ProviderIngestLocalStorageErrorV1> {
    authorization
        .validate()
        .map_err(|_| ProviderIngestLocalStorageErrorV1::Permanent)?;
    let stored = match node.manifest_metadata_by_digest(&authorization.manifest_digest()) {
        Ok(stored) => stored,
        Err(NodeStorageError::Storage(StorageError::ManifestNotFound { .. })) => return Ok(None),
        Err(error) => return Err(classify_storage_error(&error)),
    };
    if stored.manifest_digest() != &authorization.manifest_digest()
        || stored.manifest_cid() != authorization.manifest_cid()
        || stored.content_length() != authorization.content_length()
        || stored.chunk_profile_handle() != authorization.chunker_handle()
    {
        return Err(ProviderIngestLocalStorageErrorV1::Permanent);
    }
    let manifest = stored
        .load_manifest()
        .map_err(|error| classify_storage_backend_error(&error))?;
    validate_manifest_binding(authorization, &manifest)?;
    // Existing records are accepted only through StorageBackend's admission
    // invariant, which separately binds raw bytes to the stored plan payload
    // digest and ManifestV1.car_digest to the reconstructed full CARv2 archive.
    let manifest_id = stored.manifest_id().to_owned();
    let claim = match (authorization.musubi_context(), musubi_archive) {
        (None, None) => {
            return Ok(Some(ProviderIngestLocalStoredV1::generic(manifest_id)));
        }
        (Some(_), Some(claim)) => claim,
        (None, Some(_)) | (Some(_), None) => {
            return Err(ProviderIngestLocalStorageErrorV1::Permanent);
        }
    };
    let receipt = verify_existing_musubi_bundle(node, authorization, claim, &stored, &manifest)?;
    Ok(Some(ProviderIngestLocalStoredV1::musubi(
        manifest_id,
        receipt,
    )))
}

fn verify_existing_musubi_bundle(
    node: &NodeHandle,
    authorization: &FinalizedProviderIngestAuthorizationV1,
    claim: &ProviderIngestFinalizedMusubiArchiveClaimV1,
    stored: &StoredManifest,
    manifest: &ManifestV1,
) -> std::result::Result<
    ProviderIngestVerifiedMusubiBundleReceiptV1,
    ProviderIngestLocalStorageErrorV1,
> {
    validate_musubi_claim_binding(authorization, claim)?;
    let verified =
        verify_admitted_musubi_bundle(node, authorization, claim.commitment(), stored, manifest)?;

    ProviderIngestVerifiedMusubiBundleReceiptV1::from_verified_bundle(
        claim,
        authorization,
        &verified,
    )
}

/// Reconstruct and verify one admitted Musubi payload under a fresh lifecycle lease.
///
/// This helper deliberately returns the verifier's closed evidence instead of a persisted
/// pre-completion receipt. The post-completion attestation path instead enters a new admitted
/// payload lifecycle lease and calls
/// [`sorafs_node::AdmittedPayloadReadLeaseV1::verify_completed_musubi_bundle`] with the
/// independently sealed completed-row claim; possession of this earlier evidence never skips
/// that read or authorizes signing.
fn verify_admitted_musubi_bundle(
    node: &NodeHandle,
    authorization: &FinalizedProviderIngestAuthorizationV1,
    commitment: &MusubiArchiveCommitmentV1,
    stored: &StoredManifest,
    manifest: &ManifestV1,
) -> std::result::Result<VerifiedMusubiBundleV1, ProviderIngestLocalStorageErrorV1> {
    validate_musubi_commitment_binding(authorization, commitment)?;
    let registered_profile = validate_registered_chunker_profile(&manifest.chunking)
        .map_err(|_| ProviderIngestLocalStorageErrorV1::Permanent)?;
    let plan = stored
        .try_to_car_plan_with_hint(registered_profile.profile, None)
        .map_err(|error| classify_storage_backend_error(&error))?;
    validate_verified_payload(authorization, manifest, &plan)?;

    let verification = node
        .with_admitted_payload_read_lease(&authorization.manifest_digest(), |lease| {
            if lease.manifest_digest() != &authorization.manifest_digest()
                || lease.content_length() != authorization.content_length()
                || lease.payload_digest() != plan.payload_digest.as_bytes()
            {
                return None;
            }
            let first_read_error = Cell::new(None);
            let verified =
                MusubiBundleVerifierV1::verify_payload_with_factory(&plan, commitment, || {
                    lease
                        .open_reader()
                        .inspect_err(|error| {
                            if first_read_error.get().is_none() {
                                first_read_error.set(Some(error.kind()));
                            }
                        })
                        .map(|inner| ObservedAdmittedPayloadReaderV1 {
                            inner,
                            first_error_kind: &first_read_error,
                        })
                });
            Some((verified, first_read_error.get()))
        })
        .map_err(classify_admitted_payload_lease_error)?
        .ok_or(ProviderIngestLocalStorageErrorV1::Permanent)?;
    match verification {
        (Ok(verified), _) => Ok(verified),
        (Err(_), Some(kind)) if admitted_payload_read_error_is_retryable(kind) => {
            return Err(ProviderIngestLocalStorageErrorV1::Retryable);
        }
        (Err(_), _) => return Err(ProviderIngestLocalStorageErrorV1::Permanent),
    }
}

fn validate_musubi_claim_binding(
    authorization: &FinalizedProviderIngestAuthorizationV1,
    claim: &ProviderIngestFinalizedMusubiArchiveClaimV1,
) -> std::result::Result<(), ProviderIngestLocalStorageErrorV1> {
    if !claim.matches_authorization(authorization) {
        return Err(ProviderIngestLocalStorageErrorV1::Permanent);
    }
    Ok(())
}

fn validate_musubi_commitment_binding(
    authorization: &FinalizedProviderIngestAuthorizationV1,
    commitment: &MusubiArchiveCommitmentV1,
) -> std::result::Result<(), ProviderIngestLocalStorageErrorV1> {
    let Some(musubi_context) = authorization.musubi_context() else {
        return Err(ProviderIngestLocalStorageErrorV1::Permanent);
    };
    if authorization.validate().is_err()
        || commitment.validate().is_err()
        || musubi_context.archive_id() != commitment.archive_id()
        || commitment.root_cid.as_bytes() != authorization.manifest_cid()
        || commitment.chunker.to_handle() != authorization.chunker_handle()
        || commitment.chunk_plan_digest.as_bytes() != &authorization.chunk_digest_sha3_256()
        || commitment.por_root.as_bytes() != &authorization.por_root()
        || commitment.content_length != authorization.content_length()
    {
        return Err(ProviderIngestLocalStorageErrorV1::Permanent);
    }
    Ok(())
}

const fn classify_admitted_payload_lease_error(
    error: AdmittedPayloadReadLeaseErrorV1,
) -> ProviderIngestLocalStorageErrorV1 {
    match error {
        AdmittedPayloadReadLeaseErrorV1::StorageUnavailable => {
            ProviderIngestLocalStorageErrorV1::Retryable
        }
        AdmittedPayloadReadLeaseErrorV1::NotAdmitted => {
            ProviderIngestLocalStorageErrorV1::Retryable
        }
        AdmittedPayloadReadLeaseErrorV1::Disabled => ProviderIngestLocalStorageErrorV1::Permanent,
    }
}

const fn admitted_payload_read_error_is_retryable(kind: io::ErrorKind) -> bool {
    matches!(
        kind,
        io::ErrorKind::Interrupted
            | io::ErrorKind::WouldBlock
            | io::ErrorKind::TimedOut
            | io::ErrorKind::NotFound
            | io::ErrorKind::Other
    )
}

fn validate_manifest_binding(
    authorization: &FinalizedProviderIngestAuthorizationV1,
    manifest: &ManifestV1,
) -> std::result::Result<(), ProviderIngestLocalStorageErrorV1> {
    let digest = manifest
        .digest()
        .map_err(|_| ProviderIngestLocalStorageErrorV1::Permanent)?;
    let profile = format!(
        "{}.{}@{}",
        manifest.chunking.namespace, manifest.chunking.name, manifest.chunking.semver
    );
    if digest.as_bytes() != &authorization.manifest_digest()
        || manifest.root_cid.as_slice() != authorization.manifest_cid()
        || profile != authorization.chunker_handle()
        || manifest.chunk_digest_sha3_256 != authorization.chunk_digest_sha3_256()
        || manifest.por_root != authorization.por_root()
        || manifest.content_length != authorization.content_length()
    {
        return Err(ProviderIngestLocalStorageErrorV1::Permanent);
    }
    Ok(())
}

fn validate_verified_payload(
    authorization: &FinalizedProviderIngestAuthorizationV1,
    manifest: &ManifestV1,
    plan: &CarBuildPlan,
) -> std::result::Result<(), ProviderIngestLocalStorageErrorV1> {
    authorization
        .validate()
        .map_err(|_| ProviderIngestLocalStorageErrorV1::Permanent)?;
    validate_manifest_binding(authorization, manifest)?;
    if plan.content_length != authorization.content_length()
        || compute_chunk_plan_digest_sha3(&plan.chunks) != authorization.chunk_digest_sha3_256()
        || u32::try_from(plan.chunk_profile.min_size).ok() != Some(manifest.chunking.min_size)
        || u32::try_from(plan.chunk_profile.target_size).ok() != Some(manifest.chunking.target_size)
        || u32::try_from(plan.chunk_profile.max_size).ok() != Some(manifest.chunking.max_size)
        || u32::try_from(plan.chunk_profile.break_mask).ok() != Some(manifest.chunking.break_mask)
    {
        return Err(ProviderIngestLocalStorageErrorV1::Permanent);
    }
    Ok(())
}

fn classify_storage_error(error: &NodeStorageError) -> ProviderIngestLocalStorageErrorV1 {
    match error {
        NodeStorageError::Storage(error) => classify_storage_backend_error(error),
        NodeStorageError::Disabled | NodeStorageError::Scheduler(_) => {
            ProviderIngestLocalStorageErrorV1::Retryable
        }
    }
}

fn classify_completed_attestation_manifest_lookup_error(
    error: &NodeStorageError,
) -> ProviderIngestLocalStorageErrorV1 {
    match error {
        NodeStorageError::Disabled => ProviderIngestLocalStorageErrorV1::Permanent,
        NodeStorageError::Storage(StorageError::ManifestNotFound { .. }) => {
            ProviderIngestLocalStorageErrorV1::Retryable
        }
        other => classify_storage_error(other),
    }
}

fn classify_storage_backend_error(error: &StorageError) -> ProviderIngestLocalStorageErrorV1 {
    match error {
        StorageError::ChunkDigestMismatch { .. }
        | StorageError::PayloadLengthMismatch { .. }
        | StorageError::UnsupportedManifestVersion { .. }
        | StorageError::ManifestContentLengthMismatch
        | StorageError::ManifestChunkPlanDigestMismatch
        | StorageError::CarArchiveReconstruction { .. }
        | StorageError::ManifestCarArchiveDigestMismatch
        | StorageError::ManifestCarSizeMismatch { .. }
        | StorageError::ManifestDagCodecMismatch { .. }
        | StorageError::ChunkProfileMismatch
        | StorageError::PorRootMismatch
        | StorageError::ChunkStore(
            ChunkStoreError::UnexpectedEof { .. }
            | ChunkStoreError::DigestMismatch { .. }
            | ChunkStoreError::LengthMismatch { .. }
            | ChunkStoreError::PayloadDigestMismatch
            | ChunkStoreError::SinkChunkOrder { .. }
            | ChunkStoreError::SinkChunkMetadataMismatch { .. }
            | ChunkStoreError::SinkChunkLengthMismatch { .. }
            | ChunkStoreError::SinkChunkDigestMismatch { .. }
            | ChunkStoreError::SinkIncomplete { .. },
        )
        | StorageError::Norito(_)
        | StorageError::PersistentArtifactTooLarge { .. }
        | StorageError::LayoutValueTooLarge { .. }
        | StorageError::InvalidFileLayout { .. }
        | StorageError::CorruptStorageState { .. }
        | StorageError::UnsupportedIndexVersion { .. } => {
            ProviderIngestLocalStorageErrorV1::Permanent
        }
        _ => ProviderIngestLocalStorageErrorV1::Retryable,
    }
}

fn validate_completion_order_binding(
    request: &ProviderIngestCompletionPayloadRequestV1,
    provider_id: ProviderId,
    order_record: &ReplicationOrderRecord,
    pin: &PinManifestRecord,
    current_height: u64,
) -> std::result::Result<(), ProviderIngestCompletionPayloadErrorV1> {
    let order_id = ReplicationOrderId::new(request.authorization.order_id());
    if !matches!(order_record.status, ReplicationOrderStatus::Pending)
        || order_record.provider_completion(provider_id).is_some()
        || order_record.assignment_revision != request.expected_assignment_revision
        || request.completion_epoch < order_record.issued_epoch
        || request.completion_epoch > order_record.deadline_epoch
        || current_height > order_record.deadline_epoch
        || order_record.order_id != order_id
        || *order_record.manifest_digest.as_bytes() != request.authorization.manifest_digest()
        || order_record.manifest_root_cid.as_bytes() != request.authorization.manifest_cid()
        || order_record.canonical_order.is_empty()
        || order_record.canonical_order.len() > REPLICATION_ORDER_MAX_CANONICAL_BYTES_V1
    {
        return Err(ProviderIngestCompletionPayloadErrorV1::Rejected);
    }
    let order = decode_from_bytes_with_limits::<ReplicationOrderV1>(
        &order_record.canonical_order,
        REPLICATION_ORDER_DECODE_LIMITS_V1,
    )
    .map_err(|_| ProviderIngestCompletionPayloadErrorV1::Rejected)?;
    order
        .validate()
        .map_err(|_| ProviderIngestCompletionPayloadErrorV1::Rejected)?;
    if norito::to_bytes(&order).map_err(|_| ProviderIngestCompletionPayloadErrorV1::Rejected)?
        != order_record.canonical_order
        || order.order_id != request.authorization.order_id()
        || order.manifest_digest != request.authorization.manifest_digest()
        || order.manifest_cid.as_slice() != request.authorization.manifest_cid()
        || !order
            .assignments
            .iter()
            .any(|assignment| assignment.provider_id == request.authorization.provider_id())
        || !matches!(pin.status, PinStatus::Approved(_))
        || pin.digest != order_record.manifest_digest
        || pin.root_cid != order_record.manifest_root_cid
        || pin.chunker.to_handle() != request.authorization.chunker_handle()
        || pin.chunk_digest_sha3_256 != request.authorization.chunk_digest_sha3_256()
        || pin.por_root != request.authorization.por_root()
        || pin.content_length != request.authorization.content_length()
    {
        return Err(ProviderIngestCompletionPayloadErrorV1::Rejected);
    }
    Ok(())
}

#[derive(Clone)]
struct NativeCompletionPayloadBuilderV1 {
    chain_id: ChainId,
    state: Arc<State>,
    queue: Arc<Queue>,
    ttl: Duration,
    max_signed_transaction_bytes: u64,
}

impl NativeCompletionPayloadBuilderV1 {
    fn unsigned_completion_payload(
        &self,
        request: ProviderIngestCompletionPayloadRequestV1,
        order_id: ReplicationOrderId,
        provider_id: ProviderId,
    ) -> std::result::Result<TransactionPayload, ProviderIngestCompletionPayloadErrorV1> {
        let instruction = CompleteReplicationOrder::new(
            order_id,
            provider_id,
            request.completion_epoch,
            request.expected_authority,
            request.expected_assignment_revision,
            ProviderIngestFinalizedAnchorV1 {
                height: request.finalized_cursor.height,
                block_hash: request.finalized_cursor.block_hash,
            },
        );
        let mut builder = TransactionBuilder::new(
            self.chain_id.clone(),
            request.provider_owner,
            FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_instructions([instruction]);
        builder.set_ttl(self.ttl);
        builder
            .into_payload()
            .map_err(|_| ProviderIngestCompletionPayloadErrorV1::Rejected)
    }

    fn build_payload_sync(
        &self,
        request: ProviderIngestCompletionPayloadRequestV1,
    ) -> std::result::Result<TransactionPayload, ProviderIngestCompletionPayloadErrorV1> {
        if request.chain_id != self.chain_id
            || request.finalized_cursor.height == 0
            || request.finalized_cursor.block_hash == [0; 32]
            || request.expected_assignment_revision == 0
            || !request.expected_authority.is_valid()
            || request.expected_authority.provider_owner != request.provider_owner
        {
            return Err(ProviderIngestCompletionPayloadErrorV1::Rejected);
        }
        let view = self.state.query_view();
        let height = u64::try_from(view.height())
            .map_err(|_| ProviderIngestCompletionPayloadErrorV1::Rejected)?;
        let head_hash = view
            .latest_block_hash()
            .map(|hash| *hash.as_ref())
            .ok_or(ProviderIngestCompletionPayloadErrorV1::Unavailable)?;
        if view.chain_id != self.chain_id
            || !completion_payload_anchor_matches_committed_chain(
                request.finalized_cursor,
                request.completion_epoch,
                height,
                head_hash,
                view.block_hashes(),
            )
        {
            return Err(ProviderIngestCompletionPayloadErrorV1::Rejected);
        }
        let provider_id = ProviderId::new(request.authorization.provider_id());
        let order_id = ReplicationOrderId::new(request.authorization.order_id());
        let world = view.world();
        if world.provider_owners().get(&provider_id) != Some(&request.provider_owner) {
            return Err(ProviderIngestCompletionPayloadErrorV1::Rejected);
        }
        if world
            .provider_ingest_completion_authorities()
            .get(&provider_id)
            != Some(&request.expected_authority)
        {
            return Err(ProviderIngestCompletionPayloadErrorV1::Rejected);
        }
        let order_record = world
            .replication_orders()
            .get(&order_id)
            .ok_or(ProviderIngestCompletionPayloadErrorV1::Rejected)?;
        let pin = world
            .pin_manifests()
            .get(&order_record.manifest_digest)
            .ok_or(ProviderIngestCompletionPayloadErrorV1::Rejected)?;
        validate_completion_order_binding(&request, provider_id, order_record, pin, height)?;
        let mut payload = self.unsigned_completion_payload(request, order_id, provider_id)?;
        let route = self
            .queue
            .route_payload_with_state(&payload, self.state.as_ref())
            .map_err(|_| ProviderIngestCompletionPayloadErrorV1::Rejected)?;
        let next_height = height
            .checked_add(1)
            .ok_or(ProviderIngestCompletionPayloadErrorV1::Rejected)?;
        let quote = iroha_core::executor::quote_nexus_fee_admission_draft(
            view.world(),
            &view.nexus,
            &view.pipeline,
            &payload,
            payload.creation_time_ms,
            next_height,
            Some(route.dataspace_id),
        )
        .map_err(|_| ProviderIngestCompletionPayloadErrorV1::Rejected)?;
        payload.fee_payment = quote.recommended_intent;
        let encoded = norito::to_bytes(&payload)
            .map_err(|_| ProviderIngestCompletionPayloadErrorV1::Rejected)?;
        let encoded_len = u64::try_from(encoded.len())
            .map_err(|_| ProviderIngestCompletionPayloadErrorV1::Rejected)?;
        if encoded_len
            .checked_add(
                provider_ingest_outbox_defaults::SIGNED_TRANSACTION_ENVELOPE_RESERVE_BYTES_V1,
            )
            .is_none_or(|bytes| bytes > self.max_signed_transaction_bytes)
        {
            return Err(ProviderIngestCompletionPayloadErrorV1::Rejected);
        }
        Ok(payload)
    }
}

impl ProviderIngestCompletionPayloadBuilderV1 for NativeCompletionPayloadBuilderV1 {
    fn build_payload(
        &self,
        request: ProviderIngestCompletionPayloadRequestV1,
    ) -> ProviderIngestFutureV1<
        '_,
        std::result::Result<TransactionPayload, ProviderIngestCompletionPayloadErrorV1>,
    > {
        let builder = self.clone();
        Box::pin(async move {
            tokio::task::spawn_blocking(move || builder.build_payload_sync(request))
                .await
                .map_err(|_| ProviderIngestCompletionPayloadErrorV1::Unavailable)?
        })
    }
}

#[derive(Clone)]
struct NativeTransactionIngressV1 {
    chain_id: ChainId,
    state: Arc<State>,
    queue: Arc<Queue>,
}

impl NativeTransactionIngressV1 {
    fn prepare_sync(
        &self,
        transaction: SignedTransaction,
    ) -> std::result::Result<AcceptedTransaction<'static>, ProviderIngestIngressPrepareErrorV1>
    {
        let (max_clock_drift, transaction_parameters) = self.state.transaction_admission_limits();
        AcceptedTransaction::accept(
            transaction,
            &self.chain_id,
            max_clock_drift,
            transaction_parameters,
            self.state.crypto().as_ref(),
        )
        .map_err(|_| ProviderIngestIngressPrepareErrorV1::Rejected)
    }

    fn expose_sync(
        &self,
        prepared: AcceptedTransaction<'static>,
        transaction: &SignedTransaction,
    ) -> ProviderIngestIngressDispositionV1 {
        if prepared.hash() != transaction.hash() {
            return ProviderIngestIngressDispositionV1::Rejected;
        }
        match self
            .queue
            .push_with_lane_with_state(prepared, self.state.as_ref())
        {
            Ok(_) => ProviderIngestIngressDispositionV1::Submitted,
            Err(failure)
                if matches!(
                    failure.err,
                    QueueError::InBlockchain | QueueError::IsInQueue
                ) =>
            {
                ProviderIngestIngressDispositionV1::Submitted
            }
            Err(failure)
                if matches!(
                    failure.err,
                    QueueError::PlanJournalDurabilityIndeterminate { .. }
                ) =>
            {
                ProviderIngestIngressDispositionV1::Ambiguous
            }
            Err(failure)
                if matches!(
                    failure.err,
                    QueueError::Full
                        | QueueError::LatencySaturated
                        | QueueError::MaximumTransactionsPerUser
                        | QueueError::PlanJournalDurabilityRejected { .. }
                ) =>
            {
                ProviderIngestIngressDispositionV1::DefinitelyNotSubmitted
            }
            Err(_) => ProviderIngestIngressDispositionV1::Rejected,
        }
    }

    fn observe_sync(&self, transaction_hash: [u8; 32]) -> ProviderIngestTransactionObservationV1 {
        if transaction_hash == [0; 32] {
            return ProviderIngestTransactionObservationV1::CommittedRejected;
        }
        let hash =
            HashOf::<SignedTransaction>::from_untyped_unchecked(Hash::prehashed(transaction_hash));
        let Some(height) = self.state.committed_transaction_height(&hash) else {
            return if self.queue.contains_pending_hash(hash, self.state.as_ref()) {
                ProviderIngestTransactionObservationV1::Pending
            } else {
                ProviderIngestTransactionObservationV1::Unknown
            };
        };
        let Some(block) = self.state.block_by_height(height) else {
            return ProviderIngestTransactionObservationV1::Unavailable;
        };
        let Some(index) =
            block
                .external_entrypoints_cloned()
                .enumerate()
                .find_map(|(index, entrypoint)| match entrypoint {
                    TransactionEntrypoint::External(transaction) if transaction.hash() == hash => {
                        Some(index)
                    }
                    _ => None,
                })
        else {
            return ProviderIngestTransactionObservationV1::Unavailable;
        };
        if !block.has_results() {
            return ProviderIngestTransactionObservationV1::Unavailable;
        }
        match block.results().nth(index).map(|result| result.as_ref()) {
            Some(Ok(_)) => ProviderIngestTransactionObservationV1::CommittedSuccess,
            Some(Err(_)) => ProviderIngestTransactionObservationV1::CommittedRejected,
            None => ProviderIngestTransactionObservationV1::Unavailable,
        }
    }
}

impl ProviderIngestTransactionIngressV1 for NativeTransactionIngressV1 {
    type Prepared = AcceptedTransaction<'static>;

    fn prepare(
        &self,
        transaction: SignedTransaction,
    ) -> ProviderIngestFutureV1<
        '_,
        std::result::Result<Self::Prepared, ProviderIngestIngressPrepareErrorV1>,
    > {
        let ingress = self.clone();
        Box::pin(async move {
            tokio::task::spawn_blocking(move || ingress.prepare_sync(transaction))
                .await
                .map_err(|_| ProviderIngestIngressPrepareErrorV1::Rejected)?
        })
    }

    fn expose(
        &self,
        prepared: Self::Prepared,
        transaction: SignedTransaction,
    ) -> ProviderIngestFutureV1<'_, ProviderIngestIngressDispositionV1> {
        let ingress = self.clone();
        Box::pin(async move {
            tokio::task::spawn_blocking(move || ingress.expose_sync(prepared, &transaction))
                .await
                .unwrap_or(ProviderIngestIngressDispositionV1::Ambiguous)
        })
    }

    fn observe(
        &self,
        transaction_hash: [u8; 32],
    ) -> ProviderIngestFutureV1<'_, ProviderIngestTransactionObservationV1> {
        let ingress = self.clone();
        Box::pin(async move {
            tokio::task::spawn_blocking(move || ingress.observe_sync(transaction_hash))
                .await
                .unwrap_or(ProviderIngestTransactionObservationV1::Unavailable)
        })
    }
}

#[derive(Debug, Default)]
struct ProviderIngestDaemonCountersV1 {
    successful_ticks: AtomicU64,
    failed_ticks: AtomicU64,
    rows_scanned: AtomicU64,
    jobs_inserted: AtomicU64,
    jobs_finalized: AtomicU64,
    jobs_cancelled: AtomicU64,
    source_jobs_claimed: AtomicU64,
    manifests_stored: AtomicU64,
    completions_signed: AtomicU64,
    completion_submissions: AtomicU64,
}

/// Payload-free supervised provider-ingest metrics.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProviderIngestDaemonMetricsV1 {
    /// Successful bounded reconciliation ticks.
    pub successful_ticks: u64,
    /// Failed dependency probes or reconciliation ticks.
    pub failed_ticks: u64,
    /// Finalized assignment rows inspected.
    pub rows_scanned: u64,
    /// Durable jobs first admitted.
    pub jobs_inserted: u64,
    /// Jobs reconciled to finalized completion.
    pub jobs_finalized: u64,
    /// Jobs reconciled to authoritative cancellation.
    pub jobs_cancelled: u64,
    /// Source jobs claimed.
    pub source_jobs_claimed: u64,
    /// Exact manifests confirmed durable.
    pub manifests_stored: u64,
    /// Completion transactions signed.
    pub completions_signed: u64,
    /// Completion transaction exposure attempts.
    pub completion_submissions: u64,
}

/// Internal two-state status flag with an explicit boolean projection.
#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProviderIngestStatusFlagV1(bool);

impl ProviderIngestStatusFlagV1 {
    /// Return the externally emitted boolean value.
    #[must_use]
    pub const fn is_set(self) -> bool {
        self.0
    }
}

impl From<bool> for ProviderIngestStatusFlagV1 {
    fn from(value: bool) -> Self {
        Self(value)
    }
}

/// Payload-free provider-ingest readiness projection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProviderIngestDaemonStatusV1 {
    /// Whether the supervised worker task is still alive.
    pub worker_running: ProviderIngestStatusFlagV1,
    /// Whether both runtime-only adapters passed the latest probe.
    pub external_dependencies_healthy: ProviderIngestStatusFlagV1,
    /// Whether one bounded tick is currently executing.
    pub tick_in_flight: ProviderIngestStatusFlagV1,
    /// Whether a successful tick is within the configured freshness bound.
    pub last_tick_fresh: ProviderIngestStatusFlagV1,
    /// Latest fully scanned immutable finalized cursor.
    pub completed_scan_cursor: Option<ProviderIngestFinalizedCursorV1>,
    /// Current committed head height.
    pub finalized_head_height: u64,
    /// Whether the completed cursor can still be a prefix of the current
    /// finalized head.
    pub finalized_cursor_consistent: ProviderIngestStatusFlagV1,
    /// Difference between current head and completed scan.
    pub finalized_lag_blocks: u64,
    /// Non-terminal durable outbox rows.
    pub active_jobs: usize,
    /// Terminal durable outbox rows.
    pub terminal_jobs: usize,
    /// Terminal dead letters, which block readiness.
    pub dead_letters: usize,
    /// Operational health/readiness, independent of whether admitted work is
    /// currently drained.
    pub ready: bool,
    /// Whether all admitted work reached a terminal non-dead-letter state.
    pub drained: bool,
    /// Release-gate readiness: operationally ready and fully drained.
    pub release_ready: bool,
}

impl ProviderIngestDaemonStatusV1 {
    /// Return whether the supervised worker is alive.
    #[must_use]
    pub const fn worker_running(&self) -> bool {
        self.worker_running.is_set()
    }

    /// Return whether the external runtime adapters are healthy.
    #[must_use]
    pub const fn external_dependencies_healthy(&self) -> bool {
        self.external_dependencies_healthy.is_set()
    }

    /// Return whether one bounded tick is executing.
    #[must_use]
    pub const fn tick_in_flight(&self) -> bool {
        self.tick_in_flight.is_set()
    }

    /// Return whether the latest successful tick is fresh.
    #[must_use]
    pub const fn last_tick_fresh(&self) -> bool {
        self.last_tick_fresh.is_set()
    }

    /// Return whether the retained finalized cursor matches committed history.
    #[must_use]
    pub const fn finalized_cursor_consistent(&self) -> bool {
        self.finalized_cursor_consistent.is_set()
    }
}

/// Cloneable status/metrics handle retained by `irohad`.
#[derive(Clone)]
pub struct ProviderIngestRuntimeHandleV1 {
    node: NodeHandle,
    state: Arc<State>,
    config: SorafsProviderIngestRuntime,
    probe: Arc<Mutex<FinalizedSnapshotProbeV1>>,
    counters: Arc<ProviderIngestDaemonCountersV1>,
    worker_running: Arc<AtomicBool>,
    external_dependencies_healthy: Arc<AtomicBool>,
    tick_in_flight: Arc<AtomicBool>,
    last_successful_tick: Arc<Mutex<Option<Instant>>>,
}

impl fmt::Debug for ProviderIngestRuntimeHandleV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ProviderIngestRuntimeHandleV1")
            .field("runtime_state", &"[PAYLOAD-FREE]")
            .finish_non_exhaustive()
    }
}

impl ProviderIngestRuntimeHandleV1 {
    /// Return payload-free status without performing runtime work.
    ///
    /// # Errors
    ///
    /// Returns an error when the finalized view or one payload-free runtime
    /// status projection cannot be read consistently.
    pub fn status(&self) -> Result<ProviderIngestDaemonStatusV1> {
        let completed_scan_cursor = self
            .probe
            .lock()
            .map_err(|_| eyre::eyre!("provider-ingest finalized snapshot probe is poisoned"))?
            .completed_cursor;
        let view = self.state.query_view();
        let finalized_head_height = u64::try_from(view.height())
            .wrap_err("provider-ingest committed height is not representable")?;
        let finalized_head_hash = view
            .latest_block_hash()
            .map(|hash| *hash.as_ref())
            .filter(|hash| *hash != [0; 32])
            .ok_or_else(|| eyre::eyre!("provider-ingest committed head is unavailable"))?;
        let finalized_cursor_consistent = completed_cursor_matches_committed_chain(
            completed_scan_cursor,
            finalized_head_height,
            finalized_head_hash,
            view.block_hashes(),
        );
        let finalized_lag_blocks = finalized_head_height
            .saturating_sub(completed_scan_cursor.map_or(0, |cursor| cursor.height));
        let last_tick_fresh = self
            .last_successful_tick
            .lock()
            .map_err(|_| eyre::eyre!("provider-ingest tick freshness state is poisoned"))?
            .as_ref()
            .is_some_and(|instant| {
                instant.elapsed()
                    <= Duration::from_millis(self.config.scan_interval_ms)
                        .checked_mul(READINESS_STALE_TICK_MULTIPLIER_V1)
                        .unwrap_or(Duration::MAX)
            });
        let (active_jobs, terminal_jobs, dead_letters) = self.outbox_counts()?;
        let worker_running = self.worker_running.load(Ordering::Acquire);
        let external_dependencies_healthy =
            self.external_dependencies_healthy.load(Ordering::Acquire);
        let tick_in_flight = self.tick_in_flight.load(Ordering::Acquire);
        let ready = worker_running
            && external_dependencies_healthy
            && !tick_in_flight
            && last_tick_fresh
            && finalized_cursor_consistent
            && finalized_lag_blocks <= self.config.finalized_archive.max_kura_tip_lag_blocks
            && dead_letters == 0;
        let drained = active_jobs == 0 && dead_letters == 0;
        let release_ready = ready && drained;
        Ok(ProviderIngestDaemonStatusV1 {
            worker_running: worker_running.into(),
            external_dependencies_healthy: external_dependencies_healthy.into(),
            tick_in_flight: tick_in_flight.into(),
            last_tick_fresh: last_tick_fresh.into(),
            completed_scan_cursor,
            finalized_head_height,
            finalized_cursor_consistent: finalized_cursor_consistent.into(),
            finalized_lag_blocks,
            active_jobs,
            terminal_jobs,
            dead_letters,
            ready,
            drained,
            release_ready,
        })
    }

    /// Return payload-free monotonic counters.
    #[must_use]
    pub fn metrics(&self) -> ProviderIngestDaemonMetricsV1 {
        ProviderIngestDaemonMetricsV1 {
            successful_ticks: self.counters.successful_ticks.load(Ordering::Relaxed),
            failed_ticks: self.counters.failed_ticks.load(Ordering::Relaxed),
            rows_scanned: self.counters.rows_scanned.load(Ordering::Relaxed),
            jobs_inserted: self.counters.jobs_inserted.load(Ordering::Relaxed),
            jobs_finalized: self.counters.jobs_finalized.load(Ordering::Relaxed),
            jobs_cancelled: self.counters.jobs_cancelled.load(Ordering::Relaxed),
            source_jobs_claimed: self.counters.source_jobs_claimed.load(Ordering::Relaxed),
            manifests_stored: self.counters.manifests_stored.load(Ordering::Relaxed),
            completions_signed: self.counters.completions_signed.load(Ordering::Relaxed),
            completion_submissions: self.counters.completion_submissions.load(Ordering::Relaxed),
        }
    }

    fn outbox_counts(&self) -> Result<(usize, usize, usize)> {
        let counts = self
            .node
            .finalized_provider_ingest_counts()
            .wrap_err("inspect provider-ingest durable outbox counts")?;
        Ok((counts.active, counts.terminal, counts.dead_letters))
    }
}

fn committed_head_matches_hash_journal(
    head_height: u64,
    head_hash: [u8; 32],
    committed_hashes: &[HashOf<BlockHeader>],
) -> bool {
    usize::try_from(head_height).ok() == Some(committed_hashes.len())
        && committed_hashes
            .last()
            .is_some_and(|hash| *hash.as_ref() == head_hash)
}

fn cursor_matches_committed_hashes(
    cursor: ProviderIngestFinalizedCursorV1,
    committed_hashes: &[HashOf<BlockHeader>],
) -> bool {
    let Some(index) = usize::try_from(cursor.height)
        .ok()
        .and_then(|height| height.checked_sub(1))
    else {
        return false;
    };
    cursor.block_hash != [0; 32]
        && committed_hashes
            .get(index)
            .is_some_and(|hash| *hash.as_ref() == cursor.block_hash)
}

fn completion_payload_anchor_matches_committed_chain(
    cursor: ProviderIngestFinalizedCursorV1,
    completion_epoch: u64,
    head_height: u64,
    head_hash: [u8; 32],
    committed_hashes: &[HashOf<BlockHeader>],
) -> bool {
    cursor.height <= head_height
        && completion_epoch == cursor.height
        && committed_head_matches_hash_journal(head_height, head_hash, committed_hashes)
        && cursor_matches_committed_hashes(cursor, committed_hashes)
}

fn completed_cursor_matches_committed_chain(
    completed: Option<ProviderIngestFinalizedCursorV1>,
    head_height: u64,
    head_hash: [u8; 32],
    committed_hashes: &[HashOf<BlockHeader>],
) -> bool {
    let Some(completed) = completed else {
        return false;
    };
    completed.height <= head_height
        && committed_head_matches_hash_journal(head_height, head_hash, committed_hashes)
        && cursor_matches_committed_hashes(completed, committed_hashes)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RuntimeDependencyProbeV1 {
    Ready,
    Unavailable,
    Rejected,
    TimedOut,
    Panicked,
}

async fn bounded_blocking_readiness_probe<F>(
    deadline: Duration,
    probe: F,
) -> RuntimeDependencyProbeV1
where
    F: FnOnce() -> RuntimeDependencyProbeV1 + Send + 'static,
{
    match tokio::time::timeout(deadline, tokio::task::spawn_blocking(probe)).await {
        Ok(Ok(result)) => result,
        Ok(Err(_)) => RuntimeDependencyProbeV1::Panicked,
        Err(_) => RuntimeDependencyProbeV1::TimedOut,
    }
}

fn source_readiness_probe(
    result: std::result::Result<(), ProviderIngestSourceFetchErrorV1>,
) -> RuntimeDependencyProbeV1 {
    match result {
        Ok(()) => RuntimeDependencyProbeV1::Ready,
        Err(ProviderIngestSourceFetchErrorV1::Unavailable) => RuntimeDependencyProbeV1::Unavailable,
        Err(
            ProviderIngestSourceFetchErrorV1::ContentRejected
            | ProviderIngestSourceFetchErrorV1::Rejected,
        ) => RuntimeDependencyProbeV1::Rejected,
    }
}

fn signer_readiness_probe(
    result: std::result::Result<(), ProviderIngestCompletionSignerResolverErrorV1>,
) -> RuntimeDependencyProbeV1 {
    match result {
        Ok(()) => RuntimeDependencyProbeV1::Ready,
        Err(ProviderIngestCompletionSignerResolverErrorV1::Unavailable) => {
            RuntimeDependencyProbeV1::Unavailable
        }
        Err(ProviderIngestCompletionSignerResolverErrorV1::Rejected) => {
            RuntimeDependencyProbeV1::Rejected
        }
    }
}

fn combine_runtime_dependency_probes(
    source: RuntimeDependencyProbeV1,
    signer: RuntimeDependencyProbeV1,
) -> RuntimeDependencyProbeV1 {
    if source == RuntimeDependencyProbeV1::Rejected || signer == RuntimeDependencyProbeV1::Rejected
    {
        RuntimeDependencyProbeV1::Rejected
    } else if source == RuntimeDependencyProbeV1::Panicked
        || signer == RuntimeDependencyProbeV1::Panicked
    {
        RuntimeDependencyProbeV1::Panicked
    } else if source == RuntimeDependencyProbeV1::TimedOut
        || signer == RuntimeDependencyProbeV1::TimedOut
    {
        RuntimeDependencyProbeV1::TimedOut
    } else if source == RuntimeDependencyProbeV1::Unavailable
        || signer == RuntimeDependencyProbeV1::Unavailable
    {
        RuntimeDependencyProbeV1::Unavailable
    } else {
        RuntimeDependencyProbeV1::Ready
    }
}

async fn probe_runtime_dependencies(
    authenticated_source: Arc<dyn ProviderIngestAuthenticatedSourceRuntimeV1>,
    signer_resolver: Arc<dyn ProviderIngestGovernedSignerResolverRuntimeV1>,
    source_deadline: Duration,
    signer_deadline: Duration,
) -> RuntimeDependencyProbeV1 {
    let source = bounded_blocking_readiness_probe(source_deadline, move || {
        source_readiness_probe(authenticated_source.check_readiness())
    });
    let signer = bounded_blocking_readiness_probe(signer_deadline, move || {
        signer_readiness_probe(signer_resolver.check_readiness())
    });
    let (source, signer) = tokio::join!(source, signer);
    combine_runtime_dependency_probes(source, signer)
}

fn provider_ingest_shutdown_wait(config: &SorafsProviderIngestRuntime) -> Duration {
    let source_budget = config.source_operation_timeout_ms.saturating_mul(3);
    let signer_budget = config.signer_timeout_ms.saturating_mul(4);
    let ingress_budget = config.ingress_timeout_ms.saturating_mul(4);
    Duration::from_millis(
        source_budget
            .saturating_add(signer_budget)
            .saturating_add(ingress_budget),
    )
    .saturating_add(SHUTDOWN_WAIT_FLOOR)
}

fn configured_authenticated_source_qualification(
    config: &SorafsProviderIngestRuntime,
) -> ProviderIngestRuntimeProviderQualificationV1 {
    ProviderIngestRuntimeProviderQualificationV1::new(
        config.authenticated_source_fetch_revision,
        config.authenticated_source_fetch_policy_digest,
    )
}

fn configured_completion_signer_resolver_qualification(
    config: &SorafsProviderIngestRuntime,
) -> ProviderIngestRuntimeProviderQualificationV1 {
    ProviderIngestRuntimeProviderQualificationV1::new(
        config.completion_signer_resolver_revision,
        config.completion_signer_resolver_policy_digest,
    )
}

fn configured_completion_signer_binding(
    config: &SorafsProviderIngestRuntime,
) -> ProviderIngestCompletionSignerBindingV1 {
    ProviderIngestCompletionSignerBindingV1::new(
        config.completion_signer_handle.clone(),
        ProviderIngestCompletionSignerQualificationV1::new(
            config.completion_signer_adapter_revision,
            config.completion_signer_policy,
            config.completion_signer_algorithm,
            config.completion_signer_public_key.clone(),
        ),
    )
}

type NativeProviderIngestRuntimeV1 = ProviderIngestRuntimeV1<
    ObservedArchivedFinalizedAssignmentLedgerV1,
    AuthenticatedSourceAdapterV1,
    NativeProviderIngestLocalStorageV1,
    NativeCompletionPayloadBuilderV1,
    GovernedSignerResolverAdapterV1,
    NativeTransactionIngressV1,
    ProviderIngestSystemClockV1,
>;

/// Deployment-owned provider-ingest runtime adapters.
pub struct ProviderIngestRuntimeAdaptersV1 {
    authenticated_source: Arc<dyn ProviderIngestAuthenticatedSourceRuntimeV1>,
    signer_resolver: Arc<dyn ProviderIngestGovernedSignerResolverRuntimeV1>,
}

impl ProviderIngestRuntimeAdaptersV1 {
    /// Bundle the authenticated source and governed signer resolver.
    #[must_use]
    pub fn new(
        authenticated_source: Arc<dyn ProviderIngestAuthenticatedSourceRuntimeV1>,
        signer_resolver: Arc<dyn ProviderIngestGovernedSignerResolverRuntimeV1>,
    ) -> Self {
        Self {
            authenticated_source,
            signer_resolver,
        }
    }
}

struct ProviderIngestStartContextV1 {
    chain_id: ChainId,
    genesis_block_hash: [u8; 32],
    state: Arc<State>,
    queue: Arc<Queue>,
    node: NodeHandle,
    finalized_ledger: Arc<ArchivedProviderIngestFinalizedLedgerV1>,
    authenticated_source: Arc<dyn ProviderIngestAuthenticatedSourceRuntimeV1>,
    signer_resolver: Arc<dyn ProviderIngestGovernedSignerResolverRuntimeV1>,
}

/// Daemon-owned inputs needed to launch the supervised provider-ingest worker.
pub(crate) struct ProviderIngestRuntimeStartArgsV1 {
    chain_id: ChainId,
    genesis_block_hash: [u8; 32],
    state: Arc<State>,
    queue: Arc<Queue>,
    node: NodeHandle,
    finalized_ledger: Arc<ArchivedProviderIngestFinalizedLedgerV1>,
}

impl ProviderIngestRuntimeStartArgsV1 {
    /// Bind one runtime launch to its configured chain, node, and archive-only
    /// finalized reader.
    pub(crate) fn new(
        chain_id: ChainId,
        genesis_block_hash: [u8; 32],
        state: Arc<State>,
        queue: Arc<Queue>,
        node: NodeHandle,
        finalized_ledger: Arc<ArchivedProviderIngestFinalizedLedgerV1>,
    ) -> Self {
        Self {
            chain_id,
            genesis_block_hash,
            state,
            queue,
            node,
            finalized_ledger,
        }
    }
}

struct ProviderIngestStartupQualificationV1 {
    expected_authenticated_source_qualification: ProviderIngestRuntimeProviderQualificationV1,
    expected_resolver_qualification: ProviderIngestRuntimeProviderQualificationV1,
    expected_signer_binding: ProviderIngestCompletionSignerBindingV1,
    provider_id: ProviderId,
    source_provider_ids: Vec<[u8; 32]>,
}

fn validate_startup_dependency_qualifications(
    config: &SorafsProviderIngestRuntime,
    context: &ProviderIngestStartContextV1,
    expected_authenticated_source: ProviderIngestRuntimeProviderQualificationV1,
    expected_resolver: ProviderIngestRuntimeProviderQualificationV1,
    expected_signer: &ProviderIngestCompletionSignerBindingV1,
) -> Result<()> {
    validate_dependency_identity(
        "authenticated source-fetch",
        &config.authenticated_source_fetch_handle,
        context.authenticated_source.runtime_handle(),
    )?;
    validate_dependency_identity(
        "completion signer-resolver",
        &config.completion_signer_resolver_handle,
        context.signer_resolver.runtime_handle(),
    )?;
    validate_authenticated_source_qualification(
        context.authenticated_source.as_ref(),
        expected_authenticated_source,
    )
    .map_err(|_| {
        eyre::eyre!(
            "authenticated source-fetch qualification does not match SoraFS provider-ingest configuration"
        )
    })?;
    validate_resolver_qualification(context.signer_resolver.as_ref(), expected_resolver).map_err(
        |_| {
            eyre::eyre!(
                "completion signer-resolver qualification does not match SoraFS provider-ingest configuration"
            )
        },
    )?;
    validate_resolver_signer_binding(context.signer_resolver.as_ref(), expected_signer).map_err(
        |_| {
            eyre::eyre!(
                "completion signer binding does not match SoraFS provider-ingest configuration"
            )
        },
    )
}

fn revalidate_startup_dependencies_after_probe(
    config: &SorafsProviderIngestRuntime,
    context: &ProviderIngestStartContextV1,
    expected_authenticated_source: ProviderIngestRuntimeProviderQualificationV1,
    expected_resolver: ProviderIngestRuntimeProviderQualificationV1,
    expected_signer: &ProviderIngestCompletionSignerBindingV1,
    provider_id: [u8; 32],
    source_provider_ids: &[[u8; 32]],
) -> Result<()> {
    validate_dependency_identity(
        "authenticated source-fetch",
        &config.authenticated_source_fetch_handle,
        context.authenticated_source.runtime_handle(),
    )?;
    validate_dependency_identity(
        "completion signer-resolver",
        &config.completion_signer_resolver_handle,
        context.signer_resolver.runtime_handle(),
    )?;
    validate_authenticated_source_qualification(
        context.authenticated_source.as_ref(),
        expected_authenticated_source,
    )
    .map_err(|_| {
        eyre::eyre!(
            "authenticated source-fetch qualification changed during provider-ingest startup readiness"
        )
    })?;
    validate_resolver_qualification(context.signer_resolver.as_ref(), expected_resolver).map_err(
        |_| {
            eyre::eyre!(
                "completion signer-resolver qualification changed during provider-ingest startup readiness"
            )
        },
    )?;
    validate_resolver_signer_binding(context.signer_resolver.as_ref(), expected_signer).map_err(
        |_| {
            eyre::eyre!(
                "completion signer binding changed during provider-ingest startup readiness"
            )
        },
    )?;
    validate_authenticated_source_inventory(
        context.authenticated_source.as_ref(),
        provider_id,
        Some(source_provider_ids),
    )
}

async fn qualify_provider_ingest_startup(
    config: &SorafsProviderIngestRuntime,
    context: &ProviderIngestStartContextV1,
) -> Result<ProviderIngestStartupQualificationV1> {
    validate_config(config)?;
    if context.finalized_ledger.chain_id() != &context.chain_id {
        bail!("daemon-owned finalized provider-ingest query has a substituted chain identity");
    }
    context.finalized_ledger.activation_ready().wrap_err(
        "qualify daemon-owned finalized provider-ingest archive activation gate at runtime startup",
    )?;
    let expected_authenticated_source_qualification =
        configured_authenticated_source_qualification(config);
    let expected_resolver_qualification =
        configured_completion_signer_resolver_qualification(config);
    let expected_signer_binding = configured_completion_signer_binding(config);
    validate_startup_dependency_qualifications(
        config,
        context,
        expected_authenticated_source_qualification,
        expected_resolver_qualification,
        &expected_signer_binding,
    )?;
    let provider_id =
        context.node.config().provider_id().ok_or_else(|| {
            eyre::eyre!("provider-ingest runtime requires a configured provider id")
        })?;
    if context.finalized_ledger.provider_id() != provider_id {
        bail!("daemon-owned finalized provider-ingest query has a substituted provider identity");
    }
    validate_authenticated_source_inventory(
        context.authenticated_source.as_ref(),
        *provider_id.as_bytes(),
        None,
    )?;
    let source_provider_ids = context.authenticated_source.source_provider_ids().to_vec();
    let dependency_probe = probe_runtime_dependencies(
        Arc::clone(&context.authenticated_source),
        Arc::clone(&context.signer_resolver),
        Duration::from_millis(config.source_operation_timeout_ms),
        Duration::from_millis(config.signer_timeout_ms),
    )
    .await;
    revalidate_startup_dependencies_after_probe(
        config,
        context,
        expected_authenticated_source_qualification,
        expected_resolver_qualification,
        &expected_signer_binding,
        *provider_id.as_bytes(),
        &source_provider_ids,
    )?;
    match dependency_probe {
        RuntimeDependencyProbeV1::Ready => {}
        RuntimeDependencyProbeV1::Unavailable => {
            bail!("SoraFS provider-ingest runtime dependencies are temporarily unavailable");
        }
        RuntimeDependencyProbeV1::Rejected => {
            bail!("SoraFS provider-ingest runtime dependency qualification was rejected");
        }
        RuntimeDependencyProbeV1::TimedOut => {
            bail!("SoraFS provider-ingest runtime dependency readiness probe failed its deadline");
        }
        RuntimeDependencyProbeV1::Panicked => {
            bail!("SoraFS provider-ingest runtime dependency readiness probe panicked");
        }
    }
    Ok(ProviderIngestStartupQualificationV1 {
        expected_authenticated_source_qualification,
        expected_resolver_qualification,
        expected_signer_binding,
        provider_id,
        source_provider_ids,
    })
}

fn provider_ingest_runtime_policy(
    config: &SorafsProviderIngestRuntime,
) -> ProviderIngestRuntimePolicyV1 {
    ProviderIngestRuntimePolicyV1 {
        max_page_rows: config.max_page_rows,
        max_pages_per_tick: config.max_pages_per_tick,
        max_source_jobs_per_tick: config.max_source_jobs_per_tick,
        max_source_providers: config.max_source_providers,
        scan_interval_ms: config.scan_interval_ms,
        source_operation_timeout_ms: config.source_operation_timeout_ms,
        source_lease_renew_interval_ms: config.source_lease_renew_interval_ms,
        signer_timeout_ms: config.signer_timeout_ms,
        ingress_timeout_ms: config.ingress_timeout_ms,
    }
}

fn assemble_native_provider_ingest_runtime(
    config: &SorafsProviderIngestRuntime,
    context: &ProviderIngestStartContextV1,
    qualification: &ProviderIngestStartupQualificationV1,
) -> Result<(NativeProviderIngestRuntimeV1, ProviderIngestRuntimeHandleV1)> {
    let claim_owner = random_claim_owner()?;
    let probe = Arc::new(Mutex::new(FinalizedSnapshotProbeV1 {
        completed_cursor: None,
    }));
    let ledger = Arc::new(ObservedArchivedFinalizedAssignmentLedgerV1::new(
        Arc::clone(&context.finalized_ledger),
        Arc::clone(&probe),
    ));
    let fetch = Arc::new(AuthenticatedSourceAdapterV1 {
        source: Arc::clone(&context.authenticated_source),
        expected_qualification: qualification.expected_authenticated_source_qualification,
    });
    let storage = Arc::new(NativeProviderIngestLocalStorageV1::new(
        context.node.clone(),
        Duration::from_millis(config.source_operation_timeout_ms),
    ));
    let payload_builder = Arc::new(NativeCompletionPayloadBuilderV1 {
        chain_id: context.chain_id.clone(),
        state: Arc::clone(&context.state),
        queue: Arc::clone(&context.queue),
        ttl: Duration::from_millis(config.completion_transaction_ttl_ms),
        max_signed_transaction_bytes: config.outbox.max_signed_transaction_bytes.0,
    });
    let owner_authority: Arc<dyn ProviderIngestFinalizedOwnerAuthorityV1> = context.state.clone();
    let resolver = Arc::new(GovernedSignerResolverAdapterV1 {
        resolver: Arc::clone(&context.signer_resolver),
        owner_authority,
        provider_id: qualification.provider_id,
        expected_resolver_qualification: qualification.expected_resolver_qualification,
        expected_signer_binding: qualification.expected_signer_binding.clone(),
    });
    let ingress = Arc::new(NativeTransactionIngressV1 {
        chain_id: context.chain_id.clone(),
        state: Arc::clone(&context.state),
        queue: Arc::clone(&context.queue),
    });
    let runtime = context
        .node
        .build_provider_ingest_runtime(
            context.chain_id.clone(),
            context.genesis_block_hash,
            claim_owner,
            provider_ingest_runtime_policy(config),
            ledger,
            fetch,
            storage,
            payload_builder,
            resolver,
            ingress,
            Arc::new(ProviderIngestSystemClockV1),
        )
        .wrap_err("assemble finalized-ledger provider-ingest runtime")?;
    let handle = ProviderIngestRuntimeHandleV1 {
        node: context.node.clone(),
        state: Arc::clone(&context.state),
        config: config.clone(),
        probe,
        counters: Arc::new(ProviderIngestDaemonCountersV1::default()),
        worker_running: Arc::new(AtomicBool::new(false)),
        external_dependencies_healthy: Arc::new(AtomicBool::new(false)),
        tick_in_flight: Arc::new(AtomicBool::new(false)),
        last_successful_tick: Arc::new(Mutex::new(None)),
    };
    Ok((runtime, handle))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ProviderIngestWorkerControlV1 {
    Continue,
    Stop,
}

fn provider_ingest_tick_error_is_transient(error: &ProviderIngestRuntimeErrorV1) -> bool {
    matches!(
        error,
        ProviderIngestRuntimeErrorV1::FinalizedLedgerUnavailable
    )
}

struct ProviderIngestWorkerV1 {
    config: SorafsProviderIngestRuntime,
    runtime: NativeProviderIngestRuntimeV1,
    handle: ProviderIngestRuntimeHandleV1,
    finalized_ledger: Arc<ArchivedProviderIngestFinalizedLedgerV1>,
    authenticated_source: Arc<dyn ProviderIngestAuthenticatedSourceRuntimeV1>,
    signer_resolver: Arc<dyn ProviderIngestGovernedSignerResolverRuntimeV1>,
    expected_authenticated_source_qualification: ProviderIngestRuntimeProviderQualificationV1,
    expected_resolver_qualification: ProviderIngestRuntimeProviderQualificationV1,
    expected_signer_binding: ProviderIngestCompletionSignerBindingV1,
    provider_id: ProviderId,
    source_provider_ids: Vec<[u8; 32]>,
}

impl ProviderIngestWorkerV1 {
    fn adapter_identity_probe(&self) -> RuntimeDependencyProbeV1 {
        if validate_dependency_identity(
            "authenticated source-fetch",
            &self.config.authenticated_source_fetch_handle,
            self.authenticated_source.runtime_handle(),
        )
        .is_err()
            || validate_dependency_identity(
                "completion signer-resolver",
                &self.config.completion_signer_resolver_handle,
                self.signer_resolver.runtime_handle(),
            )
            .is_err()
            || validate_authenticated_source_inventory(
                self.authenticated_source.as_ref(),
                *self.provider_id.as_bytes(),
                Some(&self.source_provider_ids),
            )
            .is_err()
        {
            return RuntimeDependencyProbeV1::Rejected;
        }
        let source = source_readiness_probe(validate_authenticated_source_qualification(
            self.authenticated_source.as_ref(),
            self.expected_authenticated_source_qualification,
        ));
        let resolver = signer_readiness_probe(validate_resolver_qualification(
            self.signer_resolver.as_ref(),
            self.expected_resolver_qualification,
        ));
        let signer = signer_readiness_probe(validate_resolver_signer_binding(
            self.signer_resolver.as_ref(),
            &self.expected_signer_binding,
        ));
        combine_runtime_dependency_probes(
            source,
            combine_runtime_dependency_probes(resolver, signer),
        )
    }

    async fn probe_dependencies_or_shutdown(
        &self,
        shutdown_signal: &ShutdownSignal,
    ) -> Option<RuntimeDependencyProbeV1> {
        let dependency_probe = probe_runtime_dependencies(
            Arc::clone(&self.authenticated_source),
            Arc::clone(&self.signer_resolver),
            Duration::from_millis(self.config.source_operation_timeout_ms),
            Duration::from_millis(self.config.signer_timeout_ms),
        );
        tokio::pin!(dependency_probe);
        tokio::select! {
            probe = &mut dependency_probe => Some(probe),
            () = shutdown_signal.receive() => {
                iroha_logger::debug!(
                    "SoraFS provider-ingest runtime is being shut down during dependency probing"
                );
                None
            }
        }
    }

    fn record_successful_tick(&self, outcome: ProviderIngestTickOutcomeV1) -> bool {
        record_tick_outcome(&self.handle.counters, outcome);
        self.handle
            .counters
            .successful_ticks
            .fetch_add(1, Ordering::Relaxed);
        self.handle.last_successful_tick.lock().map_or_else(
            |_| {
                self.handle.tick_in_flight.store(false, Ordering::Release);
                iroha_logger::error!(
                    "SoraFS provider-ingest freshness state is poisoned; stopping supervised worker"
                );
                false
            },
            |mut last_tick| {
                *last_tick = Some(Instant::now());
                true
            },
        )
    }

    async fn reconcile_or_shutdown(
        &mut self,
        shutdown_signal: &ShutdownSignal,
    ) -> ProviderIngestWorkerControlV1 {
        self.handle
            .external_dependencies_healthy
            .store(true, Ordering::Release);
        let shutdown_requested = AtomicBool::new(false);
        let mut stop_after_tick = false;
        let tick_result = {
            let tick = self.runtime.tick_with_shutdown(&shutdown_requested);
            tokio::pin!(tick);
            loop {
                tokio::select! {
                    result = &mut tick => break result,
                    () = shutdown_signal.receive(), if !stop_after_tick => {
                        shutdown_requested.store(true, Ordering::Release);
                        self.handle
                            .external_dependencies_healthy
                            .store(false, Ordering::Release);
                        stop_after_tick = true;
                    }
                }
            }
        };
        match tick_result {
            Ok(outcome) if stop_after_tick => {
                record_tick_outcome(&self.handle.counters, outcome);
                self.handle.tick_in_flight.store(false, Ordering::Release);
                iroha_logger::debug!(
                    "SoraFS provider-ingest runtime drained its active row for shutdown"
                );
                ProviderIngestWorkerControlV1::Stop
            }
            Ok(outcome) => {
                if self.record_successful_tick(outcome) {
                    self.handle.tick_in_flight.store(false, Ordering::Release);
                    ProviderIngestWorkerControlV1::Continue
                } else {
                    ProviderIngestWorkerControlV1::Stop
                }
            }
            Err(error) => {
                self.handle
                    .counters
                    .failed_ticks
                    .fetch_add(1, Ordering::Relaxed);
                self.handle
                    .external_dependencies_healthy
                    .store(false, Ordering::Release);
                self.handle.tick_in_flight.store(false, Ordering::Release);
                if provider_ingest_tick_error_is_transient(&error) {
                    iroha_logger::warn!(
                        error = %error,
                        "SoraFS provider-ingest reconciliation dependency is temporarily unavailable; retrying on the next bounded tick"
                    );
                    ProviderIngestWorkerControlV1::Continue
                } else {
                    iroha_logger::error!(
                        error = %error,
                        "SoraFS provider-ingest reconciliation failed fatally; stopping supervised worker"
                    );
                    ProviderIngestWorkerControlV1::Stop
                }
            }
        }
    }

    async fn tick(&mut self, shutdown_signal: &ShutdownSignal) -> ProviderIngestWorkerControlV1 {
        self.handle.tick_in_flight.store(true, Ordering::Release);
        self.handle
            .external_dependencies_healthy
            .store(false, Ordering::Release);
        match self.finalized_ledger.activation_ready() {
            Ok(true) => {}
            Ok(false) => {
                self.handle.tick_in_flight.store(false, Ordering::Release);
                iroha_logger::debug!(
                    "SoraFS provider-ingest runtime is awaiting finalized archive activation"
                );
                return ProviderIngestWorkerControlV1::Continue;
            }
            Err(error) => {
                self.handle.tick_in_flight.store(false, Ordering::Release);
                iroha_logger::error!(
                    %error,
                    "SoraFS provider-ingest finalized archive activation gate failed closed"
                );
                return ProviderIngestWorkerControlV1::Stop;
            }
        }
        match self.adapter_identity_probe() {
            RuntimeDependencyProbeV1::Ready => {}
            RuntimeDependencyProbeV1::Unavailable | RuntimeDependencyProbeV1::TimedOut => {
                self.handle
                    .counters
                    .failed_ticks
                    .fetch_add(1, Ordering::Relaxed);
                self.handle.tick_in_flight.store(false, Ordering::Release);
                iroha_logger::warn!(
                    "SoraFS provider-ingest runtime adapter qualification is temporarily unavailable"
                );
                return ProviderIngestWorkerControlV1::Continue;
            }
            RuntimeDependencyProbeV1::Rejected | RuntimeDependencyProbeV1::Panicked => {
                self.handle.tick_in_flight.store(false, Ordering::Release);
                iroha_logger::error!(
                    "SoraFS provider-ingest runtime adapter identity or qualification was rejected; stopping supervised worker"
                );
                return ProviderIngestWorkerControlV1::Stop;
            }
        }
        let Some(dependency_probe) = self.probe_dependencies_or_shutdown(shutdown_signal).await
        else {
            return ProviderIngestWorkerControlV1::Stop;
        };
        let dependency_probe =
            combine_runtime_dependency_probes(dependency_probe, self.adapter_identity_probe());
        match dependency_probe {
            RuntimeDependencyProbeV1::Ready => {}
            RuntimeDependencyProbeV1::Unavailable => {
                self.handle
                    .counters
                    .failed_ticks
                    .fetch_add(1, Ordering::Relaxed);
                self.handle.tick_in_flight.store(false, Ordering::Release);
                iroha_logger::warn!(
                    "SoraFS provider-ingest runtime dependency is temporarily unavailable"
                );
                return ProviderIngestWorkerControlV1::Continue;
            }
            RuntimeDependencyProbeV1::TimedOut => {
                self.handle
                    .counters
                    .failed_ticks
                    .fetch_add(1, Ordering::Relaxed);
                self.handle.tick_in_flight.store(false, Ordering::Release);
                iroha_logger::warn!(
                    "SoraFS provider-ingest runtime dependency probe exceeded its deadline; retrying on the next bounded tick"
                );
                return ProviderIngestWorkerControlV1::Continue;
            }
            RuntimeDependencyProbeV1::Rejected => {
                self.handle
                    .counters
                    .failed_ticks
                    .fetch_add(1, Ordering::Relaxed);
                self.handle.tick_in_flight.store(false, Ordering::Release);
                iroha_logger::error!(
                    "SoraFS provider-ingest runtime dependency identity or qualification was rejected; stopping supervised worker"
                );
                return ProviderIngestWorkerControlV1::Stop;
            }
            RuntimeDependencyProbeV1::Panicked => {
                self.handle
                    .counters
                    .failed_ticks
                    .fetch_add(1, Ordering::Relaxed);
                self.handle.tick_in_flight.store(false, Ordering::Release);
                iroha_logger::error!(
                    "SoraFS provider-ingest runtime dependency probe panicked; stopping supervised worker"
                );
                return ProviderIngestWorkerControlV1::Stop;
            }
        }
        self.reconcile_or_shutdown(shutdown_signal).await
    }

    async fn run(mut self, shutdown_signal: ShutdownSignal) {
        let _liveness = ProviderIngestWorkerLivenessGuardV1::new(
            Arc::clone(&self.handle.worker_running),
            Arc::clone(&self.handle.tick_in_flight),
        );
        let mut interval =
            tokio::time::interval(Duration::from_millis(self.config.scan_interval_ms));
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            tokio::select! {
                _ = interval.tick() => {
                    if self.tick(&shutdown_signal).await == ProviderIngestWorkerControlV1::Stop {
                        break;
                    }
                }
                () = shutdown_signal.receive() => {
                    self.handle.tick_in_flight.store(false, Ordering::Release);
                    iroha_logger::debug!(
                        "SoraFS provider-ingest runtime is being shut down"
                    );
                    break;
                }
                else => break,
            }
        }
    }
}

/// Assemble and start supervised finalized-ledger provider ingest.
///
/// Missing, test-marked, unready, or identity-substituted runtime adapters
/// fail startup before the worker is spawned.
pub(crate) async fn start(
    config: SorafsProviderIngestRuntime,
    args: ProviderIngestRuntimeStartArgsV1,
    adapters: ProviderIngestRuntimeAdaptersV1,
    shutdown_signal: ShutdownSignal,
) -> Result<(ProviderIngestRuntimeHandleV1, Child)> {
    let ProviderIngestRuntimeStartArgsV1 {
        chain_id,
        genesis_block_hash,
        state,
        queue,
        node,
        finalized_ledger,
    } = args;
    let ProviderIngestRuntimeAdaptersV1 {
        authenticated_source,
        signer_resolver,
    } = adapters;
    let context = ProviderIngestStartContextV1 {
        chain_id,
        genesis_block_hash,
        state,
        queue,
        node,
        finalized_ledger,
        authenticated_source,
        signer_resolver,
    };
    let qualification = qualify_provider_ingest_startup(&config, &context).await?;
    let (runtime, handle) =
        assemble_native_provider_ingest_runtime(&config, &context, &qualification)?;
    let shutdown_wait = provider_ingest_shutdown_wait(&config);
    let worker = ProviderIngestWorkerV1 {
        config,
        runtime,
        handle: handle.clone(),
        finalized_ledger: Arc::clone(&context.finalized_ledger),
        authenticated_source: context.authenticated_source,
        signer_resolver: context.signer_resolver,
        expected_authenticated_source_qualification: qualification
            .expected_authenticated_source_qualification,
        expected_resolver_qualification: qualification.expected_resolver_qualification,
        expected_signer_binding: qualification.expected_signer_binding,
        provider_id: qualification.provider_id,
        source_provider_ids: qualification.source_provider_ids,
    };
    let task = tokio::spawn(worker.run(shutdown_signal));
    Ok((handle, Child::new(task, OnShutdown::Wait(shutdown_wait))))
}
struct ProviderIngestWorkerLivenessGuardV1 {
    worker_running: Arc<AtomicBool>,
    tick_in_flight: Arc<AtomicBool>,
}

impl ProviderIngestWorkerLivenessGuardV1 {
    fn new(worker_running: Arc<AtomicBool>, tick_in_flight: Arc<AtomicBool>) -> Self {
        worker_running.store(true, Ordering::Release);
        Self {
            worker_running,
            tick_in_flight,
        }
    }
}

impl Drop for ProviderIngestWorkerLivenessGuardV1 {
    fn drop(&mut self) {
        self.tick_in_flight.store(false, Ordering::Release);
        self.worker_running.store(false, Ordering::Release);
    }
}

fn record_tick_outcome(
    counters: &ProviderIngestDaemonCountersV1,
    outcome: ProviderIngestTickOutcomeV1,
) {
    counters.rows_scanned.fetch_add(
        u64::try_from(outcome.rows_scanned).unwrap_or(u64::MAX),
        Ordering::Relaxed,
    );
    counters.jobs_inserted.fetch_add(
        u64::try_from(outcome.jobs_inserted).unwrap_or(u64::MAX),
        Ordering::Relaxed,
    );
    counters.jobs_finalized.fetch_add(
        u64::try_from(outcome.jobs_finalized).unwrap_or(u64::MAX),
        Ordering::Relaxed,
    );
    counters.jobs_cancelled.fetch_add(
        u64::try_from(outcome.jobs_cancelled).unwrap_or(u64::MAX),
        Ordering::Relaxed,
    );
    counters.source_jobs_claimed.fetch_add(
        u64::try_from(outcome.source_jobs_claimed).unwrap_or(u64::MAX),
        Ordering::Relaxed,
    );
    counters.manifests_stored.fetch_add(
        u64::try_from(outcome.manifests_stored).unwrap_or(u64::MAX),
        Ordering::Relaxed,
    );
    counters.completions_signed.fetch_add(
        u64::try_from(outcome.completions_signed).unwrap_or(u64::MAX),
        Ordering::Relaxed,
    );
    counters.completion_submissions.fetch_add(
        u64::try_from(outcome.completion_submissions).unwrap_or(u64::MAX),
        Ordering::Relaxed,
    );
}

fn random_claim_owner() -> Result<ProviderIngestClaimOwnerV1> {
    for _ in 0..8 {
        let mut bytes = [0_u8; 32];
        OsRng
            .try_fill_bytes(&mut bytes)
            .wrap_err("operating-system randomness unavailable for provider-ingest claim owner")?;
        if let Ok(owner) = ProviderIngestClaimOwnerV1::new(bytes) {
            return Ok(owner);
        }
    }
    bail!("operating-system randomness repeatedly returned a zero provider-ingest claim owner")
}

fn validate_config(config: &SorafsProviderIngestRuntime) -> Result<()> {
    let authenticated_source_qualification = configured_authenticated_source_qualification(config);
    let completion_signer_resolver_qualification =
        configured_completion_signer_resolver_qualification(config);
    let completion_signer_binding = configured_completion_signer_binding(config);
    if !is_production_runtime_handle(&config.authenticated_source_fetch_handle)
        || !is_production_runtime_handle(&config.completion_signer_resolver_handle)
        || !authenticated_source_qualification.is_valid()
        || !completion_signer_resolver_qualification.is_valid()
        || completion_signer_binding.validate().is_err()
        || config.scan_interval_ms == 0
        || config.max_page_rows == 0
        || config.max_pages_per_tick == 0
        || config.max_source_jobs_per_tick == 0
        || config.max_source_providers == 0
        || config.source_operation_timeout_ms == 0
        || config.source_lease_renew_interval_ms == 0
        || config.signer_timeout_ms == 0
        || config.ingress_timeout_ms == 0
        || config.completion_transaction_ttl_ms == 0
        || config.finalized_archive.max_record_bytes == 0
        || config.finalized_archive.max_archive_entries == 0
        || config.finalized_archive.max_total_bytes < config.finalized_archive.max_record_bytes
        || config.finalized_archive.max_providers_per_anchor == 0
        || config.finalized_archive.max_orders_per_provider == 0
        || config.finalized_archive.max_total_orders_per_anchor == 0
        || config.finalized_archive.max_page_rows == 0
        || config.max_page_rows > config.finalized_archive.max_page_rows
        || config.max_page_rows > config.finalized_archive.max_orders_per_provider
        || config.source_lease_renew_interval_ms >= config.outbox.source_lease_ttl_ms
        || config.outbox.max_signed_transaction_bytes.0
            < provider_ingest_outbox_defaults::MAX_SIGNED_TRANSACTION_BYTES_MIN
        || config
            .max_page_rows
            .checked_mul(config.max_pages_per_tick)
            .is_none()
    {
        bail!("SoraFS provider-ingest runtime configuration is invalid");
    }
    let policy = sorafs_node::ProviderIngestOutboxPolicyV1 {
        max_active_entries: config.outbox.max_active_entries,
        max_terminal_entries: config.outbox.max_terminal_entries,
        max_attempts: config.outbox.max_attempts,
        checkpoint_max_bytes: config.outbox.checkpoint_max_bytes.0,
        checkpoint_operation_timeout_ms: config.outbox.checkpoint_operation_timeout_ms,
        source_lease_ttl_ms: config.outbox.source_lease_ttl_ms,
        retry_base_delay_ms: config.outbox.retry_base_delay_ms,
        retry_max_delay_ms: config.outbox.retry_max_delay_ms,
        terminal_retention_blocks: config.outbox.terminal_retention_blocks,
        max_signed_transaction_bytes: config.outbox.max_signed_transaction_bytes.0,
        max_status_page_size: config.outbox.max_status_page_size,
    };
    policy
        .validate()
        .wrap_err("validate provider-ingest durable outbox policy")?;
    Ok(())
}

fn validate_dependency_identity(label: &str, expected: &str, actual: &str) -> Result<()> {
    if !is_production_runtime_handle(actual) || actual != expected {
        bail!("{label} adapter identity does not match SoraFS provider-ingest configuration");
    }
    Ok(())
}

fn validate_authenticated_source_inventory(
    source: &dyn ProviderIngestAuthenticatedSourceRuntimeV1,
    local_provider_id: [u8; 32],
    expected: Option<&[[u8; 32]]>,
) -> Result<()> {
    let provider_ids = source.source_provider_ids();
    if provider_ids.len() < 2
        || provider_ids.len() > MAX_REPLICATION_ORDER_ASSIGNMENTS
        || provider_ids
            .iter()
            .any(|provider_id| *provider_id == [0; 32] || *provider_id == local_provider_id)
        || provider_ids.windows(2).any(|pair| pair[0] >= pair[1])
        || expected.is_some_and(|expected| expected != provider_ids)
    {
        bail!(
            "authenticated provider-ingest source inventory is missing, substituted, noncanonical, or out of bounds"
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use iroha_crypto::{Algorithm, KeyPair};
    use iroha_data_model::{
        isi::InstructionBox,
        musubi::MusubiContentDigestV1,
        sorafs::pin_registry::{
            ChunkerProfileHandle, ManifestRootCid, ProviderIngestCompletionAuthorityV1,
        },
    };
    use sorafs_node::FinalizedProviderIngestMusubiContextV1;
    use sorafs_node::provider_ingest_runtime::{
        ProviderIngestAuthenticatedProviderSourceV1, ProviderIngestAuthenticatedSourceBindingV1,
        ProviderIngestAuthenticatedSourceRegistrationV1, ProviderIngestMusubiArchiveFetchBindingV1,
        ProviderIngestSourceQualificationV1,
    };

    use super::*;

    #[derive(Debug)]
    struct TestClockV1 {
        now: Mutex<Instant>,
    }

    impl TestClockV1 {
        fn new() -> Self {
            Self {
                now: Mutex::new(Instant::now()),
            }
        }

        fn now(&self) -> Instant {
            *self.now.lock().expect("test clock lock")
        }

        fn advance(&self, duration: Duration) {
            let mut now = self.now.lock().expect("test clock lock");
            *now = now.checked_add(duration).expect("test clock advance");
        }
    }

    #[derive(Debug)]
    enum TestTerminalBehaviorV1 {
        Eof,
        Error {
            kind: io::ErrorKind,
            message: &'static str,
        },
        ExtraByte(u8),
        AdvancingEof {
            clock: Arc<TestClockV1>,
            advance: Duration,
        },
    }

    struct TestTerminalReaderV1 {
        payload: Vec<u8>,
        offset: usize,
        terminal_behavior: TestTerminalBehaviorV1,
        terminal_probe_count: Arc<AtomicU64>,
        terminal_probe_width: Arc<AtomicU64>,
    }

    impl TestTerminalReaderV1 {
        fn new(
            payload: impl Into<Vec<u8>>,
            terminal_behavior: TestTerminalBehaviorV1,
        ) -> (Self, Arc<AtomicU64>, Arc<AtomicU64>) {
            let terminal_probe_count = Arc::new(AtomicU64::new(0));
            let terminal_probe_width = Arc::new(AtomicU64::new(0));
            (
                Self {
                    payload: payload.into(),
                    offset: 0,
                    terminal_behavior,
                    terminal_probe_count: Arc::clone(&terminal_probe_count),
                    terminal_probe_width: Arc::clone(&terminal_probe_width),
                },
                terminal_probe_count,
                terminal_probe_width,
            )
        }
    }

    impl Read for TestTerminalReaderV1 {
        fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
            if output.is_empty() {
                return Ok(0);
            }
            if self.offset < self.payload.len() {
                let copied = output.len().min(self.payload.len() - self.offset);
                output[..copied].copy_from_slice(&self.payload[self.offset..self.offset + copied]);
                self.offset += copied;
                return Ok(copied);
            }
            self.terminal_probe_count.fetch_add(1, Ordering::SeqCst);
            self.terminal_probe_width.store(
                u64::try_from(output.len()).unwrap_or(u64::MAX),
                Ordering::SeqCst,
            );
            match &self.terminal_behavior {
                TestTerminalBehaviorV1::Eof => Ok(0),
                TestTerminalBehaviorV1::Error { kind, message } => {
                    Err(io::Error::new(*kind, *message))
                }
                TestTerminalBehaviorV1::ExtraByte(byte) => {
                    output[0] = *byte;
                    Ok(1)
                }
                TestTerminalBehaviorV1::AdvancingEof { clock, advance } => {
                    clock.advance(*advance);
                    Ok(0)
                }
            }
        }
    }

    #[derive(Clone)]
    struct TestOwnerAuthorityV1 {
        owner: Arc<Mutex<Option<AccountId>>>,
    }

    impl TestOwnerAuthorityV1 {
        fn new(owner: AccountId) -> Self {
            Self {
                owner: Arc::new(Mutex::new(Some(owner))),
            }
        }

        fn replace(&self, owner: AccountId) {
            *self.owner.lock().expect("owner authority lock") = Some(owner);
        }
    }

    impl ProviderIngestFinalizedOwnerAuthorityV1 for TestOwnerAuthorityV1 {
        fn owner_matches(&self, _provider_id: ProviderId, expected_owner: &AccountId) -> bool {
            self.owner.lock().expect("owner authority lock").as_ref() == Some(expected_owner)
        }
    }

    enum TestSignerMutationV1 {
        Owner(AccountId),
        Policy(ProviderIngestCompletionSignerPolicyV1),
        QualificationRevision(u64),
    }

    struct TestGovernedCompletionSignerV1 {
        key: KeyPair,
        authority: AccountId,
        policy: Mutex<ProviderIngestCompletionSignerPolicyV1>,
        qualification_revision: AtomicU64,
        owner_authority: TestOwnerAuthorityV1,
        mutation: Mutex<Option<TestSignerMutationV1>>,
        sign_calls: AtomicU64,
    }

    impl ProviderIngestCompletionSignerV1 for TestGovernedCompletionSignerV1 {
        fn runtime_handle(&self) -> &'static str {
            "pkcs11:sorafs-provider-ingest-primary"
        }

        fn authority(&self) -> &AccountId {
            &self.authority
        }

        fn qualification(
            &self,
        ) -> std::result::Result<
            ProviderIngestCompletionSignerQualificationV1,
            ProviderIngestCompletionSignerErrorV1,
        > {
            Ok(ProviderIngestCompletionSignerQualificationV1::new(
                self.qualification_revision.load(Ordering::SeqCst),
                self.signer_policy(),
                self.key.public_key().algorithm(),
                self.key.public_key().clone(),
            ))
        }

        fn signer_policy(&self) -> ProviderIngestCompletionSignerPolicyV1 {
            *self.policy.lock().expect("signer policy lock")
        }

        fn current_eligibility(
            &self,
        ) -> std::result::Result<
            ProviderIngestCompletionSignerPolicyV1,
            ProviderIngestCompletionSignerErrorV1,
        > {
            let policy = self.signer_policy();
            if policy.is_valid() {
                Ok(policy)
            } else {
                Err(ProviderIngestCompletionSignerErrorV1::Rejected)
            }
        }

        fn sign(
            &self,
            payload: TransactionPayload,
        ) -> ProviderIngestFutureV1<
            '_,
            std::result::Result<SignedTransaction, ProviderIngestCompletionSignerErrorV1>,
        > {
            Box::pin(async move {
                self.sign_calls.fetch_add(1, Ordering::SeqCst);
                let transaction = TransactionBuilder::from_payload(payload)
                    .and_then(|builder| builder.try_sign(self.key.private_key()))
                    .map_err(|_| ProviderIngestCompletionSignerErrorV1::Rejected)?;
                match self.mutation.lock().expect("signer mutation lock").take() {
                    Some(TestSignerMutationV1::Owner(owner)) => {
                        self.owner_authority.replace(owner);
                    }
                    Some(TestSignerMutationV1::Policy(policy)) => {
                        *self.policy.lock().expect("signer policy lock") = policy;
                    }
                    Some(TestSignerMutationV1::QualificationRevision(revision)) => {
                        self.qualification_revision
                            .store(revision, Ordering::SeqCst);
                    }
                    None => {}
                }
                Ok(transaction)
            })
        }
    }

    struct TestGovernedSignerResolverV1 {
        signer: Arc<dyn ProviderIngestCompletionSignerV1>,
        qualification: Mutex<ProviderIngestRuntimeProviderQualificationV1>,
        qualification_after_readiness: Mutex<Option<ProviderIngestRuntimeProviderQualificationV1>>,
        qualification_after_resolve: Mutex<Option<ProviderIngestRuntimeProviderQualificationV1>>,
        readiness: Mutex<std::result::Result<(), ProviderIngestCompletionSignerResolverErrorV1>>,
        last_resolution_context: Mutex<Option<ProviderIngestCompletionSignerResolutionContextV1>>,
    }

    impl TestGovernedSignerResolverV1 {
        fn new(signer: Arc<dyn ProviderIngestCompletionSignerV1>) -> Self {
            Self {
                signer,
                qualification: Mutex::new(ProviderIngestRuntimeProviderQualificationV1::new(
                    6, [0xB2; 32],
                )),
                qualification_after_readiness: Mutex::new(None),
                qualification_after_resolve: Mutex::new(None),
                readiness: Mutex::new(Ok(())),
                last_resolution_context: Mutex::new(None),
            }
        }
    }

    impl ProviderIngestGovernedSignerResolverRuntimeV1 for TestGovernedSignerResolverV1 {
        fn runtime_handle(&self) -> &'static str {
            "hsm:sorafs-provider-ingest-resolver"
        }

        fn qualification(
            &self,
        ) -> std::result::Result<
            ProviderIngestRuntimeProviderQualificationV1,
            ProviderIngestCompletionSignerResolverErrorV1,
        > {
            Ok(*self
                .qualification
                .lock()
                .expect("resolver qualification lock"))
        }

        fn signer_binding(
            &self,
        ) -> std::result::Result<
            ProviderIngestCompletionSignerBindingV1,
            ProviderIngestCompletionSignerResolverErrorV1,
        > {
            let qualification = self.signer.qualification().map_err(|error| match error {
                ProviderIngestCompletionSignerErrorV1::Unavailable => {
                    ProviderIngestCompletionSignerResolverErrorV1::Unavailable
                }
                ProviderIngestCompletionSignerErrorV1::Rejected => {
                    ProviderIngestCompletionSignerResolverErrorV1::Rejected
                }
            })?;
            Ok(ProviderIngestCompletionSignerBindingV1::new(
                self.signer.runtime_handle(),
                qualification,
            ))
        }

        fn check_readiness(
            &self,
        ) -> std::result::Result<(), ProviderIngestCompletionSignerResolverErrorV1> {
            if let Some(qualification) = self
                .qualification_after_readiness
                .lock()
                .expect("resolver readiness mutation lock")
                .take()
            {
                *self
                    .qualification
                    .lock()
                    .expect("resolver qualification lock") = qualification;
            }
            *self.readiness.lock().expect("resolver readiness lock")
        }

        fn resolve(
            &self,
            context: ProviderIngestCompletionSignerResolutionContextV1,
        ) -> ProviderIngestFutureV1<
            '_,
            std::result::Result<
                Option<Arc<dyn ProviderIngestCompletionSignerV1>>,
                ProviderIngestCompletionSignerResolverErrorV1,
            >,
        > {
            *self
                .last_resolution_context
                .lock()
                .expect("resolver context lock") = Some(context);
            let signer = Arc::clone(&self.signer);
            if let Some(qualification) = self
                .qualification_after_resolve
                .lock()
                .expect("resolver resolution mutation lock")
                .take()
            {
                *self
                    .qualification
                    .lock()
                    .expect("resolver qualification lock") = qualification;
            }
            Box::pin(async move { Ok(Some(signer)) })
        }
    }

    fn test_signer_policy(revision: u64) -> ProviderIngestCompletionSignerPolicyV1 {
        let digest_byte = u8::try_from(revision).unwrap_or(0xFE);
        ProviderIngestCompletionSignerPolicyV1 {
            policy_id: [0xA1; 32],
            revision,
            predecessor_digest: (revision > 1).then(|| [digest_byte.saturating_sub(1); 32]),
            policy_digest: [digest_byte; 32],
        }
    }

    fn test_completion_payload(
        key: &KeyPair,
        provider_id: ProviderId,
        completion_epoch: u64,
        expected_assignment_revision: u64,
    ) -> TransactionPayload {
        let provider_owner = AccountId::new(key.public_key().clone());
        let signer_policy = test_signer_policy(1);
        let mut builder = TransactionBuilder::new(
            ChainId::from("provider-ingest-governed-signer-test"),
            provider_owner.clone(),
            FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_instructions([InstructionBox::from(CompleteReplicationOrder {
            order_id: ReplicationOrderId::new([0xB1; 32]),
            provider_id,
            completion_epoch,
            expected_authority:
                iroha_data_model::sorafs::pin_registry::ProviderIngestCompletionAuthorityV1::new(
                    provider_owner,
                    signer_policy,
                ),
            expected_assignment_revision,
            finalized_anchor: ProviderIngestFinalizedAnchorV1 {
                height: completion_epoch,
                block_hash: [0xB2; 32],
            },
        })]);
        builder.set_creation_time(Duration::from_secs(1));
        builder.set_ttl(Duration::from_secs(30));
        builder
            .try_sign(key.private_key())
            .expect("sign payload fixture")
            .payload()
            .clone()
    }

    #[test]
    fn canonical_completion_payload_fixture_fits_production_floor() {
        assert_eq!(
            provider_ingest_outbox_defaults::MAX_SIGNED_TRANSACTION_BYTES_MIN,
            64 * 1024
        );
        let key =
            KeyPair::try_from_seed(vec![0x31; 32], Algorithm::Ed25519).expect("derive signer key");
        let payload = test_completion_payload(&key, ProviderId::new([0x41; 32]), 8, 1);
        let payload_bytes =
            norito::to_bytes(&payload).expect("encode canonical completion payload");
        let decoded_payload = norito::decode_from_bytes::<TransactionPayload>(&payload_bytes)
            .expect("decode canonical completion payload");
        assert_eq!(decoded_payload, payload);
        assert_eq!(
            norito::to_bytes(&decoded_payload).expect("re-encode canonical completion payload"),
            payload_bytes
        );

        let signed = TransactionBuilder::from_payload(payload.clone())
            .expect("rebuild canonical completion transaction")
            .try_sign(key.private_key())
            .expect("sign canonical completion transaction");
        let signed_bytes =
            norito::to_bytes(&signed).expect("encode canonical signed completion transaction");
        let decoded_signed = norito::decode_from_bytes::<SignedTransaction>(&signed_bytes)
            .expect("decode canonical signed completion transaction");
        assert_eq!(decoded_signed, signed);
        assert_eq!(
            norito::to_bytes(&decoded_signed)
                .expect("re-encode canonical signed completion transaction"),
            signed_bytes
        );
        let repeated_signed = TransactionBuilder::from_payload(payload.clone())
            .expect("rebuild repeated canonical completion transaction")
            .try_sign(key.private_key())
            .expect("repeat canonical completion signature");
        assert_eq!(
            norito::to_bytes(&repeated_signed)
                .expect("encode repeated canonical signed completion transaction"),
            signed_bytes
        );

        let payload_with_envelope = u64::try_from(payload_bytes.len())
            .expect("payload length fits u64")
            .checked_add(
                provider_ingest_outbox_defaults::SIGNED_TRANSACTION_ENVELOPE_RESERVE_BYTES_V1,
            )
            .expect("payload plus envelope reserve");
        assert!(
            payload_with_envelope
                <= provider_ingest_outbox_defaults::MAX_SIGNED_TRANSACTION_BYTES_MIN
        );
        assert!(
            u64::try_from(signed_bytes.len()).expect("signed length fits u64")
                <= provider_ingest_outbox_defaults::MAX_SIGNED_TRANSACTION_BYTES_MIN
        );
    }

    fn test_governed_signer(
        policy: ProviderIngestCompletionSignerPolicyV1,
        mutation: Option<TestSignerMutationV1>,
    ) -> (
        Arc<TestGovernedCompletionSignerV1>,
        TestOwnerAuthorityV1,
        ProviderId,
        TransactionPayload,
    ) {
        let key =
            KeyPair::try_from_seed(vec![0x31; 32], Algorithm::Ed25519).expect("derive signer key");
        let authority = AccountId::new(key.public_key().clone());
        let owner_authority = TestOwnerAuthorityV1::new(authority.clone());
        let provider_id = ProviderId::new([0x41; 32]);
        let payload = test_completion_payload(&key, provider_id, 8, 1);
        let signer = Arc::new(TestGovernedCompletionSignerV1 {
            key,
            authority,
            policy: Mutex::new(policy),
            qualification_revision: AtomicU64::new(1),
            owner_authority: owner_authority.clone(),
            mutation: Mutex::new(mutation),
            sign_calls: AtomicU64::new(0),
        });
        (signer, owner_authority, provider_id, payload)
    }

    fn test_readiness_resolver(
        readiness: std::result::Result<(), ProviderIngestCompletionSignerResolverErrorV1>,
    ) -> Arc<dyn ProviderIngestGovernedSignerResolverRuntimeV1> {
        let (signer, _, _, _) = test_governed_signer(test_signer_policy(1), None);
        let signer: Arc<dyn ProviderIngestCompletionSignerV1> = signer;
        let resolver = Arc::new(TestGovernedSignerResolverV1::new(signer));
        *resolver.readiness.lock().expect("resolver readiness lock") = readiness;
        resolver
    }

    fn governed_signer_adapter(
        signer: Arc<TestGovernedCompletionSignerV1>,
        owner_authority: TestOwnerAuthorityV1,
        provider_id: ProviderId,
    ) -> GovernedSignerResolverAdapterV1 {
        let expected_signer_binding = ProviderIngestCompletionSignerBindingV1::new(
            signer.runtime_handle(),
            signer.qualification().expect("test signer qualification"),
        );
        let signer: Arc<dyn ProviderIngestCompletionSignerV1> = signer;
        let resolver: Arc<dyn ProviderIngestGovernedSignerResolverRuntimeV1> =
            Arc::new(TestGovernedSignerResolverV1::new(signer));
        let owner_authority: Arc<dyn ProviderIngestFinalizedOwnerAuthorityV1> =
            Arc::new(owner_authority);
        GovernedSignerResolverAdapterV1 {
            resolver,
            owner_authority,
            provider_id,
            expected_resolver_qualification: ProviderIngestRuntimeProviderQualificationV1::new(
                6, [0xB2; 32],
            ),
            expected_signer_binding,
        }
    }

    fn signer_test_cursor() -> ProviderIngestFinalizedCursorV1 {
        ProviderIngestFinalizedCursorV1 {
            height: 8,
            block_hash: [0xB2; 32],
        }
    }

    fn signer_resolution_context(
        provider_owner: AccountId,
    ) -> ProviderIngestCompletionSignerResolutionContextV1 {
        ProviderIngestCompletionSignerResolutionContextV1::new(
            provider_owner,
            test_signer_policy(1),
            1,
            signer_test_cursor(),
        )
    }

    #[test]
    fn governed_signer_resolver_rejects_stale_advertised_binding() {
        let (signer, _owner_authority, _provider_id, _payload) =
            test_governed_signer(test_signer_policy(1), None);
        let signer: Arc<dyn ProviderIngestCompletionSignerV1> = signer;
        let resolver = TestGovernedSignerResolverV1::new(Arc::clone(&signer));
        let mut expected = resolver.signer_binding().expect("signer binding");
        expected.qualification.adapter_revision = 2;

        assert_eq!(
            validate_resolver_signer_binding(&resolver, &expected),
            Err(ProviderIngestCompletionSignerResolverErrorV1::Rejected)
        );
    }

    #[test]
    fn governed_signer_resolver_rejects_qualification_drift_across_readiness() {
        let (signer, _owner_authority, _provider_id, _payload) =
            test_governed_signer(test_signer_policy(1), None);
        let signer: Arc<dyn ProviderIngestCompletionSignerV1> = signer;
        let resolver = TestGovernedSignerResolverV1::new(signer);
        let expected = ProviderIngestRuntimeProviderQualificationV1::new(6, [0xB2; 32]);
        assert!(validate_resolver_qualification(&resolver, expected).is_ok());
        *resolver
            .qualification_after_readiness
            .lock()
            .expect("resolver readiness mutation lock") = Some(
            ProviderIngestRuntimeProviderQualificationV1::new(7, [0xB3; 32]),
        );

        resolver.check_readiness().expect("readiness probe");

        assert_eq!(
            validate_resolver_qualification(&resolver, expected),
            Err(ProviderIngestCompletionSignerResolverErrorV1::Rejected)
        );
    }

    #[tokio::test]
    async fn governed_signer_resolver_rechecks_qualification_after_resolution() {
        let (signer, owner_authority, provider_id, _payload) =
            test_governed_signer(test_signer_policy(1), None);
        let provider_owner = signer.authority().clone();
        let expected_signer_binding = ProviderIngestCompletionSignerBindingV1::new(
            signer.runtime_handle(),
            signer.qualification().expect("test signer qualification"),
        );
        let signer: Arc<dyn ProviderIngestCompletionSignerV1> = signer;
        let resolver = Arc::new(TestGovernedSignerResolverV1::new(signer));
        let observed_resolver = Arc::clone(&resolver);
        *resolver
            .qualification_after_resolve
            .lock()
            .expect("resolver resolution mutation lock") = Some(
            ProviderIngestRuntimeProviderQualificationV1::new(7, [0xB3; 32]),
        );
        let resolver: Arc<dyn ProviderIngestGovernedSignerResolverRuntimeV1> = resolver;
        let adapter = GovernedSignerResolverAdapterV1 {
            resolver,
            owner_authority: Arc::new(owner_authority),
            provider_id,
            expected_resolver_qualification: ProviderIngestRuntimeProviderQualificationV1::new(
                6, [0xB2; 32],
            ),
            expected_signer_binding,
        };

        let expected_context = signer_resolution_context(provider_owner);
        assert!(matches!(
            adapter.resolve(expected_context.clone()).await,
            Err(ProviderIngestCompletionSignerResolverErrorV1::Rejected)
        ));
        assert_eq!(
            *observed_resolver
                .last_resolution_context
                .lock()
                .expect("resolver context lock"),
            Some(expected_context)
        );
    }

    #[tokio::test]
    async fn governed_signer_resolver_rejects_invalid_initial_policy() {
        let (signer, owner_authority, provider_id, _payload) = test_governed_signer(
            ProviderIngestCompletionSignerPolicyV1 {
                policy_id: [0; 32],
                revision: 0,
                predecessor_digest: None,
                policy_digest: [0; 32],
            },
            None,
        );
        let provider_owner = signer.authority().clone();
        let adapter = governed_signer_adapter(signer, owner_authority, provider_id);

        assert!(matches!(
            adapter
                .resolve(signer_resolution_context(provider_owner))
                .await,
            Err(ProviderIngestCompletionSignerResolverErrorV1::Rejected)
        ));
    }

    #[tokio::test]
    async fn governed_signer_pins_assignment_revision_before_hsm_signing() {
        let (signer, owner_authority, provider_id, exact_payload) =
            test_governed_signer(test_signer_policy(1), None);
        let provider_owner = signer.authority().clone();
        let adapter = governed_signer_adapter(Arc::clone(&signer), owner_authority, provider_id);
        let governed = adapter
            .resolve(signer_resolution_context(provider_owner))
            .await
            .expect("resolve governed signer")
            .expect("governed signer");

        let signed = governed
            .sign(exact_payload.clone())
            .await
            .expect("sign exact assignment revision");
        assert_eq!(signed.payload(), &exact_payload);
        assert_eq!(signer.sign_calls.load(Ordering::SeqCst), 1);

        let substituted_payload = test_completion_payload(&signer.key, provider_id, 8, 2);
        assert_eq!(
            governed.sign(substituted_payload).await,
            Err(ProviderIngestCompletionSignerErrorV1::Rejected)
        );
        assert_eq!(
            signer.sign_calls.load(Ordering::SeqCst),
            1,
            "substituted assignment revision must not reach the HSM signer"
        );
    }

    #[tokio::test]
    async fn governed_signer_rejects_provider_substitution_before_hsm_signing() {
        let (signer, owner_authority, provider_id, _exact_payload) =
            test_governed_signer(test_signer_policy(1), None);
        let provider_owner = signer.authority().clone();
        let adapter = governed_signer_adapter(Arc::clone(&signer), owner_authority, provider_id);
        let governed = adapter
            .resolve(signer_resolution_context(provider_owner))
            .await
            .expect("resolve governed signer")
            .expect("governed signer");

        let substituted_payload =
            test_completion_payload(&signer.key, ProviderId::new([0x42; 32]), 8, 1);
        assert_eq!(
            governed.sign(substituted_payload).await,
            Err(ProviderIngestCompletionSignerErrorV1::Rejected)
        );
        assert_eq!(
            signer.sign_calls.load(Ordering::SeqCst),
            0,
            "a completion for another provider must not reach the HSM signer"
        );
    }

    #[tokio::test]
    async fn governed_signer_rechecks_policy_after_signing() {
        let (signer, owner_authority, provider_id, payload) = test_governed_signer(
            test_signer_policy(1),
            Some(TestSignerMutationV1::Policy(test_signer_policy(2))),
        );
        let provider_owner = signer.authority().clone();
        let adapter = governed_signer_adapter(signer, owner_authority, provider_id);
        let governed = adapter
            .resolve(signer_resolution_context(provider_owner))
            .await
            .expect("resolve governed signer")
            .expect("governed signer");

        assert_eq!(
            governed.sign(payload).await,
            Err(ProviderIngestCompletionSignerErrorV1::Unavailable)
        );
    }

    #[tokio::test]
    async fn governed_signer_rechecks_qualification_after_signing() {
        let (signer, owner_authority, provider_id, payload) = test_governed_signer(
            test_signer_policy(1),
            Some(TestSignerMutationV1::QualificationRevision(2)),
        );
        let provider_owner = signer.authority().clone();
        let adapter = governed_signer_adapter(signer, owner_authority, provider_id);
        let governed = adapter
            .resolve(signer_resolution_context(provider_owner))
            .await
            .expect("resolve governed signer")
            .expect("governed signer");

        assert_eq!(
            governed.sign(payload).await,
            Err(ProviderIngestCompletionSignerErrorV1::Unavailable)
        );
    }

    #[tokio::test]
    async fn governed_signer_surfaces_policy_rotation_before_authorization() {
        let (signer, owner_authority, provider_id, _payload) =
            test_governed_signer(test_signer_policy(1), None);
        let provider_owner = signer.authority().clone();
        let adapter = governed_signer_adapter(Arc::clone(&signer), owner_authority, provider_id);
        let governed = adapter
            .resolve(signer_resolution_context(provider_owner))
            .await
            .expect("resolve governed signer")
            .expect("governed signer");

        *signer.policy.lock().expect("signer policy lock") = test_signer_policy(2);

        assert_eq!(governed.signer_policy(), test_signer_policy(2));
        assert_eq!(
            governed.current_eligibility(),
            Err(ProviderIngestCompletionSignerErrorV1::Unavailable)
        );
    }

    #[tokio::test]
    async fn governed_signer_reports_owner_rotation_before_authorization() {
        let replacement_key = KeyPair::try_from_seed(vec![0x33; 32], Algorithm::Ed25519)
            .expect("derive replacement owner");
        let replacement_owner = AccountId::new(replacement_key.public_key().clone());
        let (signer, owner_authority, provider_id, _payload) =
            test_governed_signer(test_signer_policy(1), None);
        let provider_owner = signer.authority().clone();
        let adapter =
            governed_signer_adapter(Arc::clone(&signer), owner_authority.clone(), provider_id);
        let governed = adapter
            .resolve(signer_resolution_context(provider_owner))
            .await
            .expect("resolve governed signer")
            .expect("governed signer");

        owner_authority.replace(replacement_owner);

        assert_eq!(
            governed.current_eligibility(),
            Err(ProviderIngestCompletionSignerErrorV1::Unavailable)
        );
    }

    #[tokio::test]
    async fn governed_signer_rechecks_owner_after_signing() {
        let replacement_key = KeyPair::try_from_seed(vec![0x32; 32], Algorithm::Ed25519)
            .expect("derive replacement owner");
        let replacement_owner = AccountId::new(replacement_key.public_key().clone());
        let (signer, owner_authority, provider_id, payload) = test_governed_signer(
            test_signer_policy(1),
            Some(TestSignerMutationV1::Owner(replacement_owner)),
        );
        let provider_owner = signer.authority().clone();
        let adapter = governed_signer_adapter(signer, owner_authority, provider_id);
        let governed = adapter
            .resolve(signer_resolution_context(provider_owner))
            .await
            .expect("resolve governed signer")
            .expect("governed signer");

        assert_eq!(
            governed.sign(payload).await,
            Err(ProviderIngestCompletionSignerErrorV1::Unavailable)
        );
    }

    #[test]
    fn production_handle_validation_rejects_placeholders_and_whitespace() {
        for handle in [
            "",
            "pkcs11 test",
            "source-mock-primary",
            "fake",
            "dummy",
            "kms-placeholder",
            "source\nprimary",
            "https://operator:secret@host",
            "https://host/source?token=secret",
            "https://host/source#fragment",
        ] {
            assert!(!is_production_runtime_handle(handle), "{handle:?}");
        }
        assert!(is_production_runtime_handle(
            "hsm://sorafs/provider-ingest/primary"
        ));
        assert!(is_production_runtime_handle(
            "https-pinned-source-pool:eu-1"
        ));
    }

    #[test]
    fn dependency_identity_rejects_runtime_substitution() {
        assert!(validate_dependency_identity("source", "source:eu-1", "source:eu-2").is_err());
        assert!(validate_dependency_identity("source", "source:eu-1", "source:eu-1").is_ok());
    }

    struct TestPoolProviderSourceV1 {
        provider_id: [u8; 32],
        runtime_handle: &'static str,
        readiness: std::result::Result<(), ProviderIngestSourceFetchErrorV1>,
    }

    impl ProviderIngestAuthenticatedProviderSourceV1 for TestPoolProviderSourceV1 {
        type Fetched = VerifiedProviderIngestPayloadV1;

        fn provider_id(&self) -> [u8; 32] {
            self.provider_id
        }

        fn runtime_handle(&self) -> &str {
            self.runtime_handle
        }

        fn qualification(
            &self,
        ) -> std::result::Result<
            ProviderIngestSourceQualificationV1,
            ProviderIngestSourceFetchErrorV1,
        > {
            Ok(ProviderIngestSourceQualificationV1::new(
                1,
                self.provider_id,
            ))
        }

        fn check_readiness(&self) -> std::result::Result<(), ProviderIngestSourceFetchErrorV1> {
            self.readiness
        }

        fn fetch_provider(
            &self,
            _authorization: FinalizedProviderIngestAuthorizationV1,
            _musubi_archive: Option<ProviderIngestMusubiArchiveFetchBindingV1>,
        ) -> ProviderIngestFutureV1<
            '_,
            std::result::Result<Self::Fetched, ProviderIngestSourceFetchErrorV1>,
        > {
            Box::pin(async { Err(ProviderIngestSourceFetchErrorV1::Unavailable) })
        }
    }

    fn test_runtime_source_pool(
        first_readiness: std::result::Result<(), ProviderIngestSourceFetchErrorV1>,
        second_readiness: std::result::Result<(), ProviderIngestSourceFetchErrorV1>,
    ) -> ProviderIngestAuthenticatedSourcePoolV1<VerifiedProviderIngestPayloadV1> {
        let registrations = [
            ([0x22; 32], "https-pinned:provider-a", first_readiness),
            ([0x33; 32], "https-pinned:provider-b", second_readiness),
        ]
        .into_iter()
        .map(|(provider_id, runtime_handle, readiness)| {
            let source: Arc<
                dyn ProviderIngestAuthenticatedProviderSourceV1<
                    Fetched = VerifiedProviderIngestPayloadV1,
                >,
            > = Arc::new(TestPoolProviderSourceV1 {
                provider_id,
                runtime_handle,
                readiness,
            });
            ProviderIngestAuthenticatedSourceRegistrationV1::new(
                ProviderIngestAuthenticatedSourceBindingV1 {
                    provider_id,
                    runtime_handle: runtime_handle.to_owned(),
                    revision: 1,
                    policy_digest: provider_id,
                },
                source,
            )
        })
        .collect();
        ProviderIngestAuthenticatedSourcePoolV1::new(
            "https-pinned-source-pool:region-a",
            ProviderIngestRuntimeProviderQualificationV1::new(5, [0xB1; 32]),
            4,
            registrations,
        )
        .expect("test source pool")
    }

    struct TestAuthenticatedSourceInventoryV1 {
        provider_ids: Vec<[u8; 32]>,
        qualification: Mutex<ProviderIngestRuntimeProviderQualificationV1>,
        qualification_after_readiness: Mutex<Option<ProviderIngestRuntimeProviderQualificationV1>>,
        qualification_after_fetch: Mutex<Option<ProviderIngestRuntimeProviderQualificationV1>>,
        readiness: Mutex<std::result::Result<(), ProviderIngestSourceFetchErrorV1>>,
    }

    impl TestAuthenticatedSourceInventoryV1 {
        fn new(provider_ids: Vec<[u8; 32]>) -> Self {
            Self {
                provider_ids,
                qualification: Mutex::new(ProviderIngestRuntimeProviderQualificationV1::new(
                    5, [0xB1; 32],
                )),
                qualification_after_readiness: Mutex::new(None),
                qualification_after_fetch: Mutex::new(None),
                readiness: Mutex::new(Ok(())),
            }
        }
    }

    fn test_readiness_source(
        readiness: std::result::Result<(), ProviderIngestSourceFetchErrorV1>,
    ) -> Arc<dyn ProviderIngestAuthenticatedSourceRuntimeV1> {
        let source = Arc::new(TestAuthenticatedSourceInventoryV1::new(vec![
            [0x22; 32], [0x33; 32],
        ]));
        *source.readiness.lock().expect("source readiness lock") = readiness;
        source
    }

    impl ProviderIngestAuthenticatedSourceFetchV1 for TestAuthenticatedSourceInventoryV1 {
        type Fetched = VerifiedProviderIngestPayloadV1;

        fn fetch(
            &self,
            _request: ProviderIngestSourceRequestV1,
        ) -> ProviderIngestFutureV1<
            '_,
            std::result::Result<Self::Fetched, ProviderIngestSourceFetchErrorV1>,
        > {
            if let Some(qualification) = self
                .qualification_after_fetch
                .lock()
                .expect("source fetch mutation lock")
                .take()
            {
                *self
                    .qualification
                    .lock()
                    .expect("source qualification lock") = qualification;
            }
            Box::pin(async { Err(ProviderIngestSourceFetchErrorV1::Unavailable) })
        }
    }

    impl ProviderIngestAuthenticatedSourceRuntimeV1 for TestAuthenticatedSourceInventoryV1 {
        fn runtime_handle(&self) -> &'static str {
            "https-pinned-source-pool:region-a"
        }

        fn qualification(
            &self,
        ) -> std::result::Result<
            ProviderIngestRuntimeProviderQualificationV1,
            ProviderIngestSourceFetchErrorV1,
        > {
            Ok(*self
                .qualification
                .lock()
                .expect("source qualification lock"))
        }

        fn source_provider_ids(&self) -> &[[u8; 32]] {
            &self.provider_ids
        }

        fn check_readiness(&self) -> std::result::Result<(), ProviderIngestSourceFetchErrorV1> {
            if let Some(qualification) = self
                .qualification_after_readiness
                .lock()
                .expect("source readiness mutation lock")
                .take()
            {
                *self
                    .qualification
                    .lock()
                    .expect("source qualification lock") = qualification;
            }
            *self.readiness.lock().expect("source readiness lock")
        }
    }

    #[test]
    fn authenticated_source_inventory_is_multi_provider_canonical_and_identity_stable() {
        let local_provider_id = [0x11; 32];
        let valid = TestAuthenticatedSourceInventoryV1::new(vec![[0x22; 32], [0x33; 32]]);
        assert!(
            validate_authenticated_source_inventory(
                &valid,
                local_provider_id,
                Some(&[[0x22; 32], [0x33; 32]])
            )
            .is_ok()
        );

        for invalid in [
            vec![[0x22; 32]],
            vec![[0; 32], [0x22; 32]],
            vec![local_provider_id, [0x22; 32]],
            vec![[0x22; 32], [0x22; 32]],
            vec![[0x33; 32], [0x22; 32]],
        ] {
            let source = TestAuthenticatedSourceInventoryV1::new(invalid);
            assert!(
                validate_authenticated_source_inventory(&source, local_provider_id, None).is_err()
            );
        }

        assert!(
            validate_authenticated_source_inventory(
                &valid,
                local_provider_id,
                Some(&[[0x22; 32], [0x44; 32]])
            )
            .is_err()
        );
        let oversized = TestAuthenticatedSourceInventoryV1::new(
            (0..=MAX_REPLICATION_ORDER_ASSIGNMENTS)
                .map(|index| {
                    let mut provider_id = [0x55; 32];
                    provider_id[..8].copy_from_slice(
                        &u64::try_from(index)
                            .expect("provider index fits u64")
                            .to_be_bytes(),
                    );
                    provider_id
                })
                .collect(),
        );
        assert!(
            validate_authenticated_source_inventory(&oversized, local_provider_id, None).is_err()
        );
    }

    #[test]
    fn authenticated_source_rejects_qualification_drift_across_readiness() {
        let source = TestAuthenticatedSourceInventoryV1::new(vec![[0x22; 32], [0x33; 32]]);
        let expected = ProviderIngestRuntimeProviderQualificationV1::new(5, [0xB1; 32]);
        assert!(validate_authenticated_source_qualification(&source, expected).is_ok());
        *source
            .qualification_after_readiness
            .lock()
            .expect("source readiness mutation lock") = Some(
            ProviderIngestRuntimeProviderQualificationV1::new(6, [0xB4; 32]),
        );

        source.check_readiness().expect("readiness probe");

        assert_eq!(
            validate_authenticated_source_qualification(&source, expected),
            Err(ProviderIngestSourceFetchErrorV1::Rejected)
        );
    }

    #[test]
    fn source_and_resolver_qualifications_remain_independent() {
        let source = ProviderIngestRuntimeProviderQualificationV1::new(5, [0xB1; 32]);
        let resolver = ProviderIngestRuntimeProviderQualificationV1::new(6, [0xB2; 32]);
        assert!(source.is_valid());
        assert!(resolver.is_valid());
        assert_ne!(source, resolver);
    }

    #[test]
    fn worker_liveness_guard_fails_readiness_closed_on_every_exit() {
        let running = Arc::new(AtomicBool::new(false));
        let in_flight = Arc::new(AtomicBool::new(true));
        {
            let _guard =
                ProviderIngestWorkerLivenessGuardV1::new(running.clone(), in_flight.clone());
            assert!(running.load(Ordering::Acquire));
            assert!(in_flight.load(Ordering::Acquire));
        }
        assert!(!running.load(Ordering::Acquire));
        assert!(!in_flight.load(Ordering::Acquire));
    }

    #[test]
    fn completed_cursor_consistency_rejects_historical_and_head_forks() {
        let committed_hashes = (1_u8..=10)
            .map(|byte| HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([byte; 32])))
            .collect::<Vec<_>>();
        let cursor_hash = *committed_hashes[8].as_ref();
        let head_hash = *committed_hashes[9].as_ref();
        let cursor = ProviderIngestFinalizedCursorV1 {
            height: 9,
            block_hash: cursor_hash,
        };
        assert!(!completed_cursor_matches_committed_chain(
            None,
            10,
            head_hash,
            &committed_hashes
        ));
        assert!(completed_cursor_matches_committed_chain(
            Some(cursor),
            10,
            head_hash,
            &committed_hashes
        ));
        assert!(!completed_cursor_matches_committed_chain(
            Some(ProviderIngestFinalizedCursorV1 {
                height: 9,
                block_hash: [0xA9; 32],
            }),
            10,
            head_hash,
            &committed_hashes
        ));
        assert!(!completed_cursor_matches_committed_chain(
            Some(cursor),
            10,
            [0xAA; 32],
            &committed_hashes
        ));
        assert!(!completed_cursor_matches_committed_chain(
            Some(cursor),
            9,
            cursor_hash,
            &committed_hashes
        ));
    }

    #[test]
    fn completion_payload_anchor_accepts_an_authenticated_committed_prefix() {
        let committed_hashes = (1_u8..=10)
            .map(|byte| HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([byte; 32])))
            .collect::<Vec<_>>();
        let cursor = ProviderIngestFinalizedCursorV1 {
            height: 9,
            block_hash: *committed_hashes[8].as_ref(),
        };
        let head_hash = *committed_hashes[9].as_ref();

        assert!(completion_payload_anchor_matches_committed_chain(
            cursor,
            9,
            10,
            head_hash,
            &committed_hashes,
        ));
        assert!(!completion_payload_anchor_matches_committed_chain(
            ProviderIngestFinalizedCursorV1 {
                block_hash: [0xA9; 32],
                ..cursor
            },
            9,
            10,
            head_hash,
            &committed_hashes,
        ));
        assert!(!completion_payload_anchor_matches_committed_chain(
            cursor,
            10,
            10,
            head_hash,
            &committed_hashes,
        ));
        assert!(!completion_payload_anchor_matches_committed_chain(
            cursor,
            9,
            10,
            [0xAA; 32],
            &committed_hashes,
        ));
    }

    #[test]
    fn cancelling_store_wait_joins_late_writer() {
        let completed = Arc::new(AtomicBool::new(false));
        let late = Arc::clone(&completed);
        let thread = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(10));
            late.store(true, Ordering::Release);
        });
        drop(BlockingStoreJoinGuardV1(Some(thread)));
        assert!(completed.load(Ordering::Acquire));
    }

    #[test]
    fn deadline_bounded_reader_authenticates_terminal_eof_once() {
        let payload = b"authenticated provider payload".to_vec();
        let expected_len = u64::try_from(payload.len()).expect("payload length fits u64");
        let (inner, terminal_probe_count, terminal_probe_width) =
            TestTerminalReaderV1::new(payload.clone(), TestTerminalBehaviorV1::Eof);
        let mut reader =
            DeadlineBoundedReaderV1::new(Box::new(inner), Duration::from_secs(1), expected_len);

        let mut observed = Vec::new();
        reader
            .read_to_end(&mut observed)
            .expect("authenticate terminal EOF");
        assert_eq!(observed, payload);
        assert_eq!(terminal_probe_count.load(Ordering::SeqCst), 1);
        assert_eq!(terminal_probe_width.load(Ordering::SeqCst), 1);

        let mut trailing = [0_u8; 8];
        assert_eq!(reader.read(&mut trailing).expect("cached EOF"), 0);
        assert_eq!(
            terminal_probe_count.load(Ordering::SeqCst),
            1,
            "authenticated EOF must not re-enter the underlying transport"
        );
    }

    #[test]
    fn deadline_bounded_reader_rejects_premature_eof() {
        let payload = b"short".to_vec();
        let expected_len = u64::try_from(payload.len() + 1).expect("payload length fits u64");
        let (inner, terminal_probe_count, _) =
            TestTerminalReaderV1::new(payload.clone(), TestTerminalBehaviorV1::Eof);
        let mut reader =
            DeadlineBoundedReaderV1::new(Box::new(inner), Duration::from_secs(1), expected_len);

        let mut observed = Vec::new();
        let error = reader
            .read_to_end(&mut observed)
            .expect_err("premature EOF must fail closed");
        assert_eq!(error.kind(), io::ErrorKind::UnexpectedEof);
        assert_eq!(observed, payload);
        assert_eq!(terminal_probe_count.load(Ordering::SeqCst), 1);

        let mut trailing = [0_u8; 1];
        assert_eq!(
            reader
                .read(&mut trailing)
                .expect_err("premature EOF failure is sticky")
                .kind(),
            io::ErrorKind::UnexpectedEof
        );
        assert_eq!(terminal_probe_count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn deadline_bounded_reader_propagates_terminal_verification_failures() {
        for (kind, message) in [
            (
                io::ErrorKind::InvalidData,
                "authenticated source trailer rejected",
            ),
            (
                io::ErrorKind::PermissionDenied,
                "authenticated source qualification drifted",
            ),
        ] {
            let payload = b"exact bytes".to_vec();
            let expected_len = u64::try_from(payload.len()).expect("payload length fits u64");
            let (inner, terminal_probe_count, terminal_probe_width) = TestTerminalReaderV1::new(
                payload.clone(),
                TestTerminalBehaviorV1::Error { kind, message },
            );
            let mut reader =
                DeadlineBoundedReaderV1::new(Box::new(inner), Duration::from_secs(1), expected_len);
            let mut observed = vec![0_u8; payload.len()];
            reader
                .read_exact(&mut observed)
                .expect("read exact authorized bytes");
            assert_eq!(observed, payload);

            let mut trailing = [0_u8; 8];
            let error = reader
                .read(&mut trailing)
                .expect_err("terminal source verification must propagate");
            assert_eq!(error.kind(), kind);
            assert_eq!(error.to_string(), message);
            assert_eq!(terminal_probe_count.load(Ordering::SeqCst), 1);
            assert_eq!(terminal_probe_width.load(Ordering::SeqCst), 1);

            assert_eq!(
                reader
                    .read(&mut trailing)
                    .expect_err("terminal verification failure is sticky")
                    .kind(),
                kind
            );
            assert_eq!(terminal_probe_count.load(Ordering::SeqCst), 1);
        }
    }

    #[test]
    fn deadline_bounded_reader_rejects_extra_bytes_at_terminal_probe() {
        let payload = b"exact bytes".to_vec();
        let expected_len = u64::try_from(payload.len()).expect("payload length fits u64");
        let (inner, terminal_probe_count, terminal_probe_width) =
            TestTerminalReaderV1::new(payload.clone(), TestTerminalBehaviorV1::ExtraByte(0xA5));
        let mut reader =
            DeadlineBoundedReaderV1::new(Box::new(inner), Duration::from_secs(1), expected_len);
        let mut observed = vec![0_u8; payload.len()];
        reader
            .read_exact(&mut observed)
            .expect("read exact authorized bytes");

        let mut trailing = [0_u8; 8];
        assert_eq!(
            reader
                .read(&mut trailing)
                .expect_err("extra byte must fail closed")
                .kind(),
            io::ErrorKind::InvalidData
        );
        assert_eq!(terminal_probe_count.load(Ordering::SeqCst), 1);
        assert_eq!(terminal_probe_width.load(Ordering::SeqCst), 1);

        assert_eq!(
            reader
                .read(&mut trailing)
                .expect_err("extra-byte failure is sticky")
                .kind(),
            io::ErrorKind::InvalidData
        );
        assert_eq!(terminal_probe_count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn deadline_bounded_reader_checks_deadline_after_terminal_probe() {
        let payload = b"exact bytes".to_vec();
        let expected_len = u64::try_from(payload.len()).expect("payload length fits u64");
        let clock = Arc::new(TestClockV1::new());
        let (inner, terminal_probe_count, terminal_probe_width) = TestTerminalReaderV1::new(
            payload.clone(),
            TestTerminalBehaviorV1::AdvancingEof {
                clock: Arc::clone(&clock),
                advance: Duration::from_secs(2),
            },
        );
        let reader_clock = Arc::clone(&clock);
        let mut reader = DeadlineBoundedReaderV1::new_with_clock(
            Box::new(inner),
            Duration::from_secs(1),
            expected_len,
            Arc::new(move || reader_clock.now()),
        );
        let mut observed = vec![0_u8; payload.len()];
        reader
            .read_exact(&mut observed)
            .expect("read exact authorized bytes before deadline");

        let mut trailing = [0_u8; 8];
        assert_eq!(
            reader
                .read(&mut trailing)
                .expect_err("late terminal EOF must fail closed")
                .kind(),
            io::ErrorKind::TimedOut
        );
        assert_eq!(terminal_probe_count.load(Ordering::SeqCst), 1);
        assert_eq!(terminal_probe_width.load(Ordering::SeqCst), 1);
        assert_eq!(
            reader
                .read(&mut trailing)
                .expect_err("post-probe deadline failure is sticky")
                .kind(),
            io::ErrorKind::TimedOut
        );
        assert_eq!(
            terminal_probe_count.load(Ordering::SeqCst),
            1,
            "sticky timeout must not re-enter the underlying transport"
        );
    }

    #[test]
    fn archive_binding_storage_failures_are_permanent() {
        for error in [
            StorageError::ManifestChunkPlanDigestMismatch,
            StorageError::CarArchiveReconstruction {
                reason: "staged chunk is corrupt".to_owned(),
            },
            StorageError::ManifestCarArchiveDigestMismatch,
            StorageError::ManifestCarSizeMismatch {
                expected: 128,
                actual: 127,
            },
            StorageError::ManifestDagCodecMismatch {
                expected: 0x71,
                actual: 0x55,
            },
        ] {
            assert_eq!(
                classify_storage_error(&NodeStorageError::Storage(error)),
                ProviderIngestLocalStorageErrorV1::Permanent
            );
        }

        for error in [
            ChunkStoreError::UnexpectedEof {
                chunk_index: 0,
                expected: 64,
            },
            ChunkStoreError::DigestMismatch { chunk_index: 0 },
            ChunkStoreError::LengthMismatch {
                expected: 64,
                actual: 65,
            },
            ChunkStoreError::PayloadDigestMismatch,
        ] {
            assert_eq!(
                classify_storage_error(&NodeStorageError::Storage(StorageError::ChunkStore(error))),
                ProviderIngestLocalStorageErrorV1::Permanent
            );
        }
    }

    #[test]
    fn admitted_musubi_verification_classifies_storage_failures() {
        assert_eq!(
            classify_completed_attestation_manifest_lookup_error(&NodeStorageError::Disabled),
            ProviderIngestLocalStorageErrorV1::Permanent,
            "statically disabled storage cannot become available on retry"
        );
        assert_eq!(
            classify_completed_attestation_manifest_lookup_error(&NodeStorageError::Storage(
                StorageError::ManifestNotFound {
                    manifest_id: "temporarily-absent-completed-bundle".to_owned(),
                },
            )),
            ProviderIngestLocalStorageErrorV1::Retryable,
            "an admitted bundle may be reconciled back into storage"
        );
        assert_eq!(
            classify_admitted_payload_lease_error(
                AdmittedPayloadReadLeaseErrorV1::StorageUnavailable,
            ),
            ProviderIngestLocalStorageErrorV1::Retryable
        );
        assert_eq!(
            classify_admitted_payload_lease_error(AdmittedPayloadReadLeaseErrorV1::NotAdmitted),
            ProviderIngestLocalStorageErrorV1::Retryable
        );
        assert_eq!(
            classify_admitted_payload_lease_error(AdmittedPayloadReadLeaseErrorV1::Disabled),
            ProviderIngestLocalStorageErrorV1::Permanent
        );
        assert!(admitted_payload_read_error_is_retryable(
            io::ErrorKind::Interrupted
        ));
        assert!(admitted_payload_read_error_is_retryable(
            io::ErrorKind::WouldBlock
        ));
        assert!(admitted_payload_read_error_is_retryable(
            io::ErrorKind::TimedOut
        ));
        assert!(admitted_payload_read_error_is_retryable(
            io::ErrorKind::NotFound
        ));
        assert!(admitted_payload_read_error_is_retryable(
            io::ErrorKind::Other
        ));
        assert!(!admitted_payload_read_error_is_retryable(
            io::ErrorKind::InvalidData
        ));
        assert!(!admitted_payload_read_error_is_retryable(
            io::ErrorKind::UnexpectedEof
        ));
        assert!(!admitted_payload_read_error_is_retryable(
            io::ErrorKind::PermissionDenied
        ));

        let transient = StorageError::Io(io::Error::new(
            io::ErrorKind::Interrupted,
            "injected transient storage read",
        ));
        assert_eq!(
            classify_storage_backend_error(&transient),
            ProviderIngestLocalStorageErrorV1::Retryable
        );
    }

    #[tokio::test]
    async fn daemon_dependency_probe_allows_one_ready_source_for_request_failover() {
        let source: Arc<dyn ProviderIngestAuthenticatedSourceRuntimeV1> = Arc::new(
            test_runtime_source_pool(Err(ProviderIngestSourceFetchErrorV1::Unavailable), Ok(())),
        );
        let result = probe_runtime_dependencies(
            source,
            test_readiness_resolver(Ok(())),
            Duration::from_secs(1),
            Duration::from_secs(1),
        )
        .await;

        assert_eq!(result, RuntimeDependencyProbeV1::Ready);
    }

    #[tokio::test]
    async fn daemon_dependency_probe_preserves_rejected_and_unavailable_outcomes() {
        let unavailable = probe_runtime_dependencies(
            test_readiness_source(Err(ProviderIngestSourceFetchErrorV1::Unavailable)),
            test_readiness_resolver(Ok(())),
            Duration::from_secs(1),
            Duration::from_secs(1),
        )
        .await;
        assert_eq!(unavailable, RuntimeDependencyProbeV1::Unavailable);

        let source_rejected = probe_runtime_dependencies(
            test_readiness_source(Err(ProviderIngestSourceFetchErrorV1::Rejected)),
            test_readiness_resolver(Ok(())),
            Duration::from_secs(1),
            Duration::from_secs(1),
        )
        .await;
        assert_eq!(source_rejected, RuntimeDependencyProbeV1::Rejected);

        let signer_unavailable = probe_runtime_dependencies(
            test_readiness_source(Ok(())),
            test_readiness_resolver(Err(
                ProviderIngestCompletionSignerResolverErrorV1::Unavailable,
            )),
            Duration::from_secs(1),
            Duration::from_secs(1),
        )
        .await;
        assert_eq!(signer_unavailable, RuntimeDependencyProbeV1::Unavailable);

        let signer_rejected = probe_runtime_dependencies(
            test_readiness_source(Ok(())),
            test_readiness_resolver(Err(ProviderIngestCompletionSignerResolverErrorV1::Rejected)),
            Duration::from_secs(1),
            Duration::from_secs(1),
        )
        .await;
        assert_eq!(signer_rejected, RuntimeDependencyProbeV1::Rejected);
    }

    #[tokio::test]
    async fn hung_readiness_probe_fails_at_explicit_deadline() {
        let result = bounded_blocking_readiness_probe(Duration::from_millis(1), || {
            std::thread::sleep(Duration::from_millis(25));
            RuntimeDependencyProbeV1::Ready
        })
        .await;
        assert_eq!(result, RuntimeDependencyProbeV1::TimedOut);
    }

    #[tokio::test]
    async fn panicked_readiness_probe_is_distinct_from_transient_timeout() {
        let result = bounded_blocking_readiness_probe(Duration::from_secs(1), || {
            panic!("synthetic readiness probe panic");
        })
        .await;
        assert_eq!(result, RuntimeDependencyProbeV1::Panicked);
    }

    #[test]
    fn only_temporary_finalized_ledger_loss_is_a_retryable_tick_error() {
        assert!(provider_ingest_tick_error_is_transient(
            &ProviderIngestRuntimeErrorV1::FinalizedLedgerUnavailable
        ));
        assert!(!provider_ingest_tick_error_is_transient(
            &ProviderIngestRuntimeErrorV1::InvalidFinalizedPage
        ));
    }
}
