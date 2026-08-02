//! Authenticated HTTPS transport for the private Musubi publication control plane.
//!
//! The public Torii SoraFS upload route is deliberately not used here. Every request
//! targets one fixed publication-specific route, carries a bounded canonical Norito
//! authorization approved by the configured Iroha account controller, and rejects redirects.

use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
    io::Read,
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use base64::Engine as _;
use iroha_crypto::{PublicKey, SignatureOf};
use iroha_data_model::{
    ChainId,
    account::{AccountController, AccountId, MultisigPolicy},
    musubi::{
        ArchiveId, MUSUBI_MAX_ARCHIVE_LOCATIONS_V1, MUSUBI_MAX_CAR_BYTES_V1,
        MUSUBI_MAX_LOCATION_PROVIDERS_V1, MUSUBI_MAX_PUBLICATION_ATTESTATION_APPROVALS_V1,
        MUSUBI_MAX_SEED_INGRESS_RECEIPT_LIFETIME_MS_V1, MUSUBI_MIN_HEALTHY_REPLICAS_V1,
        MusubiArchiveCommitmentV1, MusubiArchiveLocationIdV1, MusubiArchiveLocationStateV1,
        MusubiArchiveLocationV1, MusubiArchiveRecordV1, MusubiArchiveRegistrationProjectionV1,
        MusubiContentDigestV1, MusubiProviderBundleVerificationAttestationV1,
        MusubiRegistrySnapshotV1, MusubiSeedIngressReceiptApprovalV1,
        MusubiSeedIngressReceiptBindingV1, MusubiSeedIngressReceiptPayloadV1,
        MusubiSeedIngressReceiptV1, MusubiSemanticReleaseDigestV1, MusubiVerificationLockDigestV1,
    },
    sorafs::{
        capacity::ProviderId,
        pin_registry::{ManifestDigest, ReplicationOrderId},
    },
};
use iroha_telemetry::metrics::{
    global as global_metrics,
    musubi::{MusubiIngestDeadletterReasonV1, MusubiIntegritySurfaceV1},
};
use norito::DecodeLimits;
use reqwest::{
    StatusCode, blocking::Client as HttpClient, header::HeaderValue,
    redirect::Policy as RedirectPolicy,
};
use url::Url;

use crate::{client::Client, crypto::KeyPair};

mod publication_clock;
mod publication_journal;

pub use publication_clock::{
    DurableMusubiPublicationServiceClockOpenErrorV1, DurableMusubiPublicationServiceClockV1,
};
pub use publication_journal::{
    DurableMusubiPublicationServiceJournalLimitsV1,
    DurableMusubiPublicationServiceJournalOpenErrorV1, DurableMusubiPublicationServiceJournalV1,
};

const AUTH_DOMAIN_V1: [u8; 32] = *b"musubi-pub-runtime-auth-v1\0\0\0\0\0\0";
/// Exact security-sensitive authorization header accepted by the private service.
pub const MUSUBI_PUBLICATION_AUTHORIZATION_HEADER_V1: &str =
    "x-iroha-musubi-publication-authorization";
/// Exact seed metadata header accepted only by the raw-CAR route.
pub const MUSUBI_PUBLICATION_SEED_METADATA_HEADER_V1: &str = "x-iroha-musubi-seed-ingress-metadata";
/// Canonical Norito media type used by control requests and every response.
pub const MUSUBI_PUBLICATION_NORITO_MEDIA_TYPE_V1: &str = "application/x-norito";
/// Canonical raw SoraFS CAR media type used only by seed ingress.
pub const MUSUBI_PUBLICATION_CAR_MEDIA_TYPE_V1: &str = "application/vnd.sorafs.car";
const AUTHORIZATION_HEADER: &str = MUSUBI_PUBLICATION_AUTHORIZATION_HEADER_V1;
const SEED_INGRESS_METADATA_HEADER: &str = MUSUBI_PUBLICATION_SEED_METADATA_HEADER_V1;
const APPLICATION_NORITO: &str = MUSUBI_PUBLICATION_NORITO_MEDIA_TYPE_V1;
const APPLICATION_SORAFS_CAR: &str = MUSUBI_PUBLICATION_CAR_MEDIA_TYPE_V1;
const SEED_INGRESS_ROUTE: &str = "v1/musubi/publication/seed-ingress";
const STORAGE_COORDINATION_ROUTE: &str = "v1/musubi/publication/storage-coordinate";
const PROVIDER_READBACK_ROUTE: &str = "v1/musubi/publication/provider-readback";
const MAX_AUTHORIZATION_LIFETIME_MS: u64 = 60_000;
const DEFAULT_AUTHORIZATION_LIFETIME_MS: u64 = 30_000;
const MAX_AUTHORIZATION_BYTES: usize = 64 * 1024;
const MAX_SEED_INGRESS_METADATA_BYTES: usize = 64 * 1024;
const MAX_CONTROL_REQUEST_BYTES: usize = 8 * 1024 * 1024;
const MAX_CONTROL_RESPONSE_BYTES: usize = 16 * 1024 * 1024;
const MAX_CONTROL_RESPONSE_BYTES_U64: u64 = 16 * 1024 * 1024;
const RESPONSE_DECODE_LIMITS: DecodeLimits = DecodeLimits::new(
    4_096,
    MAX_CONTROL_RESPONSE_BYTES,
    32_768,
    32 * 1024 * 1024,
    64,
);

// TODO: Deployments must inject and qualify their private HTTPS listener, durable replay journal,
// broker HSM/signer, and authoritative SoraFS backends around the transport-independent server
// below. Before declaring a deployment production-qualified, bind each configured hostname to
// deployment-signed provider-advert IPs and add DNS-rebinding tests; disabling proxies and
// redirects alone is not DNS pinning. Do not adapt the daemon-private provider broker or restore
// `/v1/sorafs/upload`; this protocol intentionally exposes neither interface.

/// Fixed private publication-control operation covered by account authorization.
#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    norito::derive::Encode,
    norito::derive::Decode,
)]
pub enum MusubiPublicationRuntimeOperationV1 {
    /// Stage one exact CAR through authenticated seed ingress.
    #[codec(index = 0)]
    SeedIngress,
    /// Register/reuse the permanent pin and replication order.
    #[codec(index = 1)]
    StorageCoordination,
    /// Read and verify the complete archive through one exact provider.
    #[codec(index = 2)]
    ProviderReadback,
}

/// Domain-separated short-lived authorization for one exact publication request.
#[derive(Clone, Debug, PartialEq, Eq, norito::derive::Encode, norito::derive::Decode)]
pub struct MusubiPublicationRuntimeAuthorizationPayloadV1 {
    /// Fixed domain marker preventing cross-protocol signature reuse.
    pub domain: [u8; 32],
    /// Closed schema version; must equal one.
    pub version: u8,
    /// Exact private control-plane operation.
    pub operation: MusubiPublicationRuntimeOperationV1,
    /// Stable idempotency key derived from the immutable publication request.
    pub operation_id: [u8; 32],
    /// Deployment-selected chain identity.
    pub chain_id: ChainId,
    /// Account authorizing this request.
    pub publisher: AccountId,
    /// Domain-separated digest of the exact typed request metadata.
    pub request_digest: [u8; 32],
    /// Authorization issuance time in Unix milliseconds.
    pub issued_at_ms: u64,
    /// Inclusive authorization expiry in Unix milliseconds.
    pub expires_at_ms: u64,
}

impl MusubiPublicationRuntimeAuthorizationPayloadV1 {
    fn validate(&self) -> Result<(), MusubiPublicationRuntimeTransportErrorV1> {
        let lifetime = self.expires_at_ms.checked_sub(self.issued_at_ms);
        if self.domain != AUTH_DOMAIN_V1
            || self.version != 1
            || self.operation_id.iter().all(|byte| *byte == 0)
            || self.request_digest.iter().all(|byte| *byte == 0)
            || self.issued_at_ms == 0
            || !matches!(lifetime, Some(1..=MAX_AUTHORIZATION_LIFETIME_MS))
        {
            return Err(MusubiPublicationRuntimeTransportErrorV1::permanent(
                "MUSUBI_RUNTIME_AUTHORIZATION_INVALID",
            ));
        }
        Ok(())
    }
}

/// One controller approval for a bounded private publication request.
#[derive(Clone, Debug, PartialEq, Eq, norito::derive::Encode, norito::derive::Decode)]
pub struct MusubiPublicationRuntimeAuthorizationApprovalV1 {
    /// Publisher-controller key that produced this approval.
    pub public_key: PublicKey,
    /// Signature over the exact service authorization payload.
    pub signature: SignatureOf<MusubiPublicationRuntimeAuthorizationPayloadV1>,
}

/// Bounded publisher-controller authorization for one private publication request.
#[derive(Clone, Debug, PartialEq, Eq, norito::derive::Encode, norito::derive::Decode)]
pub struct MusubiPublicationRuntimeAuthorizationV1 {
    /// Exact statement covered by the signature.
    pub payload: MusubiPublicationRuntimeAuthorizationPayloadV1,
    /// Canonically ordered, distinct publisher-controller approvals.
    pub approvals: Vec<MusubiPublicationRuntimeAuthorizationApprovalV1>,
}

impl MusubiPublicationRuntimeAuthorizationV1 {
    /// Verify the exact operation, request digest, validity window, and account controller.
    pub fn verify(
        &self,
        operation: MusubiPublicationRuntimeOperationV1,
        operation_id: [u8; 32],
        request_digest: [u8; 32],
        current_time_ms: u64,
    ) -> Result<(), MusubiPublicationRuntimeTransportErrorV1> {
        self.verify_with_clock_skew(operation, operation_id, request_digest, current_time_ms, 0)
    }

    /// Verify an exact request while tolerating only a bounded future clock skew.
    ///
    /// Expiry remains strict: skew can accommodate a publisher clock that is slightly
    /// ahead of the service, but never extends an authorization beyond `expires_at_ms`.
    ///
    /// # Errors
    ///
    /// Returns a redacted permanent error when any request, clock, controller, or signature
    /// binding differs.
    pub fn verify_with_clock_skew(
        &self,
        operation: MusubiPublicationRuntimeOperationV1,
        operation_id: [u8; 32],
        request_digest: [u8; 32],
        current_time_ms: u64,
        max_future_clock_skew_ms: u64,
    ) -> Result<(), MusubiPublicationRuntimeTransportErrorV1> {
        self.payload.validate()?;
        if current_time_ms == 0
            || max_future_clock_skew_ms > MUSUBI_PUBLICATION_SERVICE_MAX_CLOCK_SKEW_MS_V1
        {
            return Err(MusubiPublicationRuntimeTransportErrorV1::permanent(
                "MUSUBI_RUNTIME_CLOCK_INVALID",
            ));
        }
        let latest_accepted_issue_time = current_time_ms
            .checked_add(max_future_clock_skew_ms)
            .ok_or_else(|| {
                MusubiPublicationRuntimeTransportErrorV1::permanent("MUSUBI_RUNTIME_CLOCK_INVALID")
            })?;
        if self.payload.operation != operation
            || self.payload.operation_id != operation_id
            || self.payload.request_digest != request_digest
            || self.payload.issued_at_ms > latest_accepted_issue_time
            || current_time_ms > self.payload.expires_at_ms
        {
            return Err(MusubiPublicationRuntimeTransportErrorV1::permanent(
                "MUSUBI_RUNTIME_AUTHORIZATION_MISMATCH",
            ));
        }
        if self.approvals.is_empty()
            || self.approvals.len() > MUSUBI_MAX_PUBLICATION_ATTESTATION_APPROVALS_V1
            || self
                .approvals
                .windows(2)
                .any(|pair| pair[0].public_key >= pair[1].public_key)
        {
            return Err(MusubiPublicationRuntimeTransportErrorV1::permanent(
                "MUSUBI_RUNTIME_AUTHORIZATION_APPROVALS_INVALID",
            ));
        }
        if !controller_fits_publication_approval_bound(&self.payload.publisher) {
            return Err(MusubiPublicationRuntimeTransportErrorV1::permanent(
                "MUSUBI_RUNTIME_AUTHORIZATION_CONTROLLER_UNSUPPORTED",
            ));
        }
        let signing_hash = iroha_crypto::HashOf::new(&self.payload);
        match self.payload.publisher.controller() {
            AccountController::Single(expected_key) => {
                let [approval] = self.approvals.as_slice() else {
                    return Err(MusubiPublicationRuntimeTransportErrorV1::permanent(
                        "MUSUBI_RUNTIME_AUTHORIZATION_APPROVALS_INVALID",
                    ));
                };
                if expected_key != &approval.public_key {
                    return Err(MusubiPublicationRuntimeTransportErrorV1::permanent(
                        "MUSUBI_RUNTIME_AUTHORIZATION_KEY_MISMATCH",
                    ));
                }
                approval
                    .signature
                    .verify_hash(&approval.public_key, signing_hash)
                    .map_err(|_| {
                        MusubiPublicationRuntimeTransportErrorV1::permanent(
                            "MUSUBI_RUNTIME_AUTHORIZATION_SIGNATURE_INVALID",
                        )
                    })
            }
            AccountController::Multisig(policy) => {
                let mut approved_weight = 0_u32;
                for approval in &self.approvals {
                    let Some(member) = policy
                        .members()
                        .iter()
                        .find(|member| member.public_key() == &approval.public_key)
                    else {
                        return Err(MusubiPublicationRuntimeTransportErrorV1::permanent(
                            "MUSUBI_RUNTIME_AUTHORIZATION_KEY_MISMATCH",
                        ));
                    };
                    approval
                        .signature
                        .verify_hash(&approval.public_key, signing_hash)
                        .map_err(|_| {
                            MusubiPublicationRuntimeTransportErrorV1::permanent(
                                "MUSUBI_RUNTIME_AUTHORIZATION_SIGNATURE_INVALID",
                            )
                        })?;
                    approved_weight = approved_weight
                        .checked_add(u32::from(member.weight()))
                        .ok_or_else(|| {
                            MusubiPublicationRuntimeTransportErrorV1::permanent(
                                "MUSUBI_RUNTIME_AUTHORIZATION_APPROVALS_INVALID",
                            )
                        })?;
                }
                if approved_weight < u32::from(policy.threshold()) {
                    return Err(MusubiPublicationRuntimeTransportErrorV1::permanent(
                        "MUSUBI_RUNTIME_AUTHORIZATION_THRESHOLD_UNMET",
                    ));
                }
                Ok(())
            }
        }
    }
}

/// Metadata accompanying a raw seed-ingress CAR body.
#[derive(Clone, Debug, PartialEq, Eq, norito::derive::Encode, norito::derive::Decode)]
pub struct MusubiSeedIngressStageRequestV1 {
    /// Closed schema version; must equal one.
    pub version: u8,
    /// Stable idempotency key for the immutable publication request.
    pub operation_id: [u8; 32],
    /// Exact chain, actor, broker, provider, archive, and CAR binding.
    pub binding: MusubiSeedIngressReceiptBindingV1,
}

impl MusubiSeedIngressStageRequestV1 {
    /// Validate the closed request and its exact receipt binding.
    pub fn validate(&self) -> Result<(), MusubiPublicationRuntimeTransportErrorV1> {
        self.binding.validate().map_err(|_| {
            MusubiPublicationRuntimeTransportErrorV1::permanent(
                "MUSUBI_SEED_INGRESS_REQUEST_INVALID",
            )
        })?;
        if self.version != 1 || self.operation_id.iter().all(|byte| *byte == 0) {
            return Err(MusubiPublicationRuntimeTransportErrorV1::permanent(
                "MUSUBI_SEED_INGRESS_REQUEST_INVALID",
            ));
        }
        Ok(())
    }
}

/// Finalized immutable archive-registration evidence sent to the storage coordinator.
///
/// The named registry snapshot proves when the immutable registration became
/// observable. A backend may reproduce `registration` from any later finalized
/// archive read because Core permits only the omitted location directory to
/// change after registration.
#[derive(Clone, Debug, PartialEq, Eq, norito::derive::Encode, norito::derive::Decode)]
pub struct MusubiFinalizedArchiveRegistrationEvidenceV1 {
    /// Closed schema version; must equal one.
    pub version: u8,
    /// Deployment-selected chain identity.
    pub chain_id: ChainId,
    /// Exact committed genesis block hash.
    pub genesis_block_hash: [u8; 32],
    /// Exact finalized transaction identity that registered the archive.
    pub transaction_hash: [u8; 32],
    /// Finalized registry snapshot at or after registration.
    pub snapshot: MusubiRegistrySnapshotV1,
    /// Immutable projection reproduced from the authoritative archive record.
    pub registration: MusubiArchiveRegistrationProjectionV1,
}

impl MusubiFinalizedArchiveRegistrationEvidenceV1 {
    /// Validate deployment, finality, and immutable archive-registration bindings.
    pub fn validate(&self) -> Result<(), MusubiPublicationRuntimeTransportErrorV1> {
        self.snapshot.validate().map_err(|_| {
            MusubiPublicationRuntimeTransportErrorV1::permanent(
                "MUSUBI_ARCHIVE_REGISTRATION_EVIDENCE_INVALID",
            )
        })?;
        self.registration.validate().map_err(|_| {
            MusubiPublicationRuntimeTransportErrorV1::permanent(
                "MUSUBI_ARCHIVE_REGISTRATION_EVIDENCE_INVALID",
            )
        })?;
        let binding = &self.registration.staging_receipt.payload.binding;
        if self.version != 1
            || self.chain_id.as_str().is_empty()
            || self.genesis_block_hash.iter().all(|byte| *byte == 0)
            || self.transaction_hash.iter().all(|byte| *byte == 0)
            || binding.chain_id != self.chain_id
            || binding.genesis_block_hash != self.genesis_block_hash
            || self.registration.registered_at_height > self.snapshot.finalized_height
        {
            return Err(MusubiPublicationRuntimeTransportErrorV1::permanent(
                "MUSUBI_ARCHIVE_REGISTRATION_EVIDENCE_INVALID",
            ));
        }
        Ok(())
    }
}

/// Exact archive and receipt inputs sent to the private storage coordinator.
#[derive(Clone, Debug, PartialEq, Eq, norito::derive::Encode, norito::derive::Decode)]
pub struct MusubiStorageCoordinationRequestV1 {
    /// Closed schema version; must equal one.
    pub version: u8,
    /// Stable idempotency key for the immutable publication request.
    pub operation_id: [u8; 32],
    /// One-based append-only archive-location transaction generation.
    pub generation: u8,
    /// Sorted identities used by every earlier generation; none may be returned again.
    pub prior_location_ids: Vec<MusubiArchiveLocationIdV1>,
    /// Deployment-selected chain identity.
    pub chain_id: ChainId,
    /// Exact committed genesis block hash.
    pub genesis_block_hash: [u8; 32],
    /// Account that registered the archive.
    pub publisher: AccountId,
    /// Complete immutable archive commitment.
    pub commitment: MusubiArchiveCommitmentV1,
    /// Verification-lock digest every provider must parse from the exact bundle.
    pub verification_lock_digest: MusubiVerificationLockDigestV1,
    /// Authenticated receipt for the exact staged CAR body.
    pub staging_receipt: MusubiSeedIngressReceiptV1,
    /// Registry admission revision used by archive registration.
    pub expected_policy_revision: u64,
    /// Finalized immutable registration evidence recovered before storage coordination.
    pub finalized_registration: MusubiFinalizedArchiveRegistrationEvidenceV1,
}

impl MusubiStorageCoordinationRequestV1 {
    /// Validate every immutable storage-coordination input.
    pub fn validate(&self) -> Result<(), MusubiPublicationRuntimeTransportErrorV1> {
        self.commitment.validate().map_err(|_| {
            MusubiPublicationRuntimeTransportErrorV1::permanent(
                "MUSUBI_STORAGE_COORDINATION_REQUEST_INVALID",
            )
        })?;
        self.staging_receipt.validate().map_err(|_| {
            MusubiPublicationRuntimeTransportErrorV1::permanent(
                "MUSUBI_STORAGE_COORDINATION_REQUEST_INVALID",
            )
        })?;
        self.finalized_registration.validate().map_err(|_| {
            MusubiPublicationRuntimeTransportErrorV1::permanent(
                "MUSUBI_STORAGE_COORDINATION_REQUEST_INVALID",
            )
        })?;
        let binding = &self.staging_receipt.payload.binding;
        if self.version != 1
            || self.operation_id.iter().all(|byte| *byte == 0)
            || self.generation == 0
            || usize::from(self.generation) > MUSUBI_MAX_PUBLICATION_LOCATION_ATTEMPTS_V1
            || self.prior_location_ids.len() + 1 != usize::from(self.generation)
            || self
                .prior_location_ids
                .iter()
                .any(MusubiArchiveLocationIdV1::is_zero)
            || self
                .prior_location_ids
                .windows(2)
                .any(|pair| pair[0] >= pair[1])
            || self.genesis_block_hash.iter().all(|byte| *byte == 0)
            || self.expected_policy_revision == 0
            || self.verification_lock_digest.is_zero()
            || binding.chain_id != self.chain_id
            || binding.genesis_block_hash != self.genesis_block_hash
            || binding.publisher != self.publisher
            || binding.archive_id != self.commitment.archive_id()
            || binding.car_body_digest != self.commitment.car_digest
            || binding.car_body_length != self.commitment.car_size
            || self.finalized_registration.chain_id != self.chain_id
            || self.finalized_registration.genesis_block_hash != self.genesis_block_hash
            || self.finalized_registration.registration.archive_id != self.commitment.archive_id()
            || self.finalized_registration.registration.commitment != self.commitment
            || self.finalized_registration.registration.staging_receipt != self.staging_receipt
            || self.finalized_registration.registration.registered_by != self.publisher
        {
            return Err(MusubiPublicationRuntimeTransportErrorV1::permanent(
                "MUSUBI_STORAGE_COORDINATION_REQUEST_INVALID",
            ));
        }
        Ok(())
    }
}

/// Whether the publisher must add the returned location or may reuse finalized state.
#[derive(Clone, Debug, PartialEq, Eq, norito::derive::Encode, norito::derive::Decode)]
pub enum MusubiStorageLocationDispositionV1 {
    /// Coordinator has a pin/order and at least one finalized provider completion.
    #[codec(index = 0)]
    NeedsRegistration {
        /// Provider-signed parsed-bundle attestations for the current order.
        provider_attestations: Vec<MusubiProviderBundleVerificationAttestationV1>,
        /// Exact current archive location-set revision for compare-and-set.
        expected_location_revision: u64,
    },
    /// Identical retry found the exact finalized location already registered.
    #[codec(index = 1)]
    Registered(MusubiArchiveLocationV1),
}

/// Idempotent private storage-coordinator result.
#[derive(Clone, Debug, PartialEq, Eq, norito::derive::Encode, norito::derive::Decode)]
pub struct MusubiStorageCoordinationResponseV1 {
    /// Closed schema version; must equal one.
    pub version: u8,
    /// Current finalized authoritative archive record observed by the coordinator.
    pub archive: MusubiArchiveRecordV1,
    /// Stable location identity reserved for this archive.
    pub location_id: MusubiArchiveLocationIdV1,
    /// Permanent registry-grade SoraFS pin manifest.
    pub pin_manifest: ManifestDigest,
    /// Replication order assigned to the location.
    pub replication_order: ReplicationOrderId,
    /// Earliest renewal epoch selected for the location.
    pub renew_after_epoch: u64,
    /// Expiry epoch selected for the renewable location.
    pub expires_at_epoch: u64,
    /// Finalized retry disposition.
    pub disposition: MusubiStorageLocationDispositionV1,
}

impl MusubiStorageCoordinationResponseV1 {
    /// Validate the response against the exact authenticated request.
    pub fn validate_for(
        &self,
        request: &MusubiStorageCoordinationRequestV1,
    ) -> Result<(), MusubiPublicationRuntimeTransportErrorV1> {
        request.validate()?;
        self.archive.validate().map_err(|_| {
            MusubiPublicationRuntimeTransportErrorV1::permanent(
                "MUSUBI_STORAGE_COORDINATION_RESPONSE_INVALID",
            )
        })?;
        let registered_binding = &self.archive.staging_receipt.payload.binding;
        self.archive
            .staging_receipt
            .verify(
                registered_binding,
                self.archive.staging_receipt.payload.issued_at_ms,
            )
            .map_err(|_| {
                MusubiPublicationRuntimeTransportErrorV1::permanent(
                    "MUSUBI_STORAGE_COORDINATION_RESPONSE_INVALID",
                )
            })?;
        if self.version != 1
            || self.archive.registration_projection()
                != request.finalized_registration.registration.clone()
            || self.location_id.is_zero()
            || self.pin_manifest.as_bytes().iter().all(|byte| *byte == 0)
            || self
                .replication_order
                .as_bytes()
                .iter()
                .all(|byte| *byte == 0)
            || self.renew_after_epoch >= self.expires_at_epoch
            || request
                .prior_location_ids
                .binary_search(&self.location_id)
                .is_ok()
            || request
                .prior_location_ids
                .iter()
                .any(|location_id| self.archive.location_ids.binary_search(location_id).is_ok())
        {
            return Err(MusubiPublicationRuntimeTransportErrorV1::permanent(
                "MUSUBI_STORAGE_COORDINATION_RESPONSE_INVALID",
            ));
        }
        match &self.disposition {
            MusubiStorageLocationDispositionV1::NeedsRegistration {
                provider_attestations,
                expected_location_revision,
            } => {
                if *expected_location_revision == 0
                    || *expected_location_revision != self.archive.location_revision
                    || self
                        .archive
                        .location_ids
                        .binary_search(&self.location_id)
                        .is_ok()
                    || provider_attestations.len() < usize::from(MUSUBI_MIN_HEALTHY_REPLICAS_V1)
                    || provider_attestations.len() > MUSUBI_MAX_LOCATION_PROVIDERS_V1
                {
                    return Err(MusubiPublicationRuntimeTransportErrorV1::permanent(
                        "MUSUBI_STORAGE_COORDINATION_RESPONSE_INVALID",
                    ));
                }
                let mut previous = None;
                for attestation in provider_attestations {
                    attestation
                        .verify(&attestation.payload.binding)
                        .map_err(|_| {
                            MusubiPublicationRuntimeTransportErrorV1::permanent(
                                "MUSUBI_STORAGE_COORDINATION_RESPONSE_INVALID",
                            )
                        })?;
                    let binding = &attestation.payload.binding;
                    if binding.chain_id != request.chain_id
                        || binding.genesis_block_hash != request.genesis_block_hash
                        || binding.archive_id != request.commitment.archive_id()
                        || binding.replication_order != self.replication_order
                        || binding.bundle_digest != request.commitment.bundle_digest
                        || binding.descriptor_digest != request.commitment.descriptor_digest
                        || binding.source_tree_digest != request.commitment.source_tree_digest
                        || binding.semantic_release_manifest_digest
                            != request
                                .staging_receipt
                                .payload
                                .binding
                                .semantic_release_manifest_digest
                        || binding.verification_lock_digest != request.verification_lock_digest
                        || previous.is_some_and(|provider| provider >= binding.provider_id)
                    {
                        return Err(MusubiPublicationRuntimeTransportErrorV1::permanent(
                            "MUSUBI_STORAGE_COORDINATION_RESPONSE_INVALID",
                        ));
                    }
                    previous = Some(binding.provider_id);
                }
            }
            MusubiStorageLocationDispositionV1::Registered(location) => {
                location.validate().map_err(|_| {
                    MusubiPublicationRuntimeTransportErrorV1::permanent(
                        "MUSUBI_STORAGE_COORDINATION_RESPONSE_INVALID",
                    )
                })?;
                if location.location_id != self.location_id
                    || location.archive_id != request.commitment.archive_id()
                    || location.state == MusubiArchiveLocationStateV1::Retired
                    || self
                        .archive
                        .location_ids
                        .binary_search(&self.location_id)
                        .is_err()
                    || location.pin_manifest != self.pin_manifest
                    || location.replication_order != self.replication_order
                    || location.renew_after_epoch != self.renew_after_epoch
                    || location.expires_at_epoch != self.expires_at_epoch
                {
                    return Err(MusubiPublicationRuntimeTransportErrorV1::permanent(
                        "MUSUBI_STORAGE_COORDINATION_RESPONSE_INVALID",
                    ));
                }
                for attestation in &location.provider_attestations {
                    let binding = &attestation.payload.binding;
                    attestation.verify(binding).map_err(|_| {
                        MusubiPublicationRuntimeTransportErrorV1::permanent(
                            "MUSUBI_STORAGE_COORDINATION_RESPONSE_INVALID",
                        )
                    })?;
                    if binding.chain_id != request.chain_id
                        || binding.genesis_block_hash != request.genesis_block_hash
                        || binding.bundle_digest != request.commitment.bundle_digest
                        || binding.descriptor_digest != request.commitment.descriptor_digest
                        || binding.source_tree_digest != request.commitment.source_tree_digest
                        || binding.semantic_release_manifest_digest
                            != request
                                .staging_receipt
                                .payload
                                .binding
                                .semantic_release_manifest_digest
                        || binding.verification_lock_digest != request.verification_lock_digest
                    {
                        return Err(MusubiPublicationRuntimeTransportErrorV1::permanent(
                            "MUSUBI_STORAGE_COORDINATION_RESPONSE_INVALID",
                        ));
                    }
                }
            }
        }
        Ok(())
    }
}

/// Exact provider-specific full-archive readback request.
#[derive(Clone, Debug, PartialEq, Eq, norito::derive::Encode, norito::derive::Decode)]
pub struct MusubiProviderReadbackRequestV1 {
    /// Closed schema version; must equal one.
    pub version: u8,
    /// Stable idempotency key for the immutable publication request.
    pub operation_id: [u8; 32],
    /// Deployment-selected chain identity.
    pub chain_id: ChainId,
    /// Exact committed genesis block hash.
    pub genesis_block_hash: [u8; 32],
    /// Account publishing the release.
    pub publisher: AccountId,
    /// Exact finalized location and provider selected for readback.
    pub location: MusubiArchiveLocationV1,
    /// Provider whose endpoint must serve the complete archive.
    pub provider: ProviderId,
    /// Commitment that must be reproduced from the returned CAR.
    pub commitment: MusubiArchiveCommitmentV1,
    /// Semantic release digest that must be parsed from the returned bundle.
    pub semantic_release_digest: MusubiSemanticReleaseDigestV1,
    /// Verification-lock digest that must be parsed from the returned bundle.
    pub verification_lock_digest: MusubiVerificationLockDigestV1,
}

impl MusubiProviderReadbackRequestV1 {
    /// Validate exact location, provider, archive, and bundle bindings.
    pub fn validate(&self) -> Result<(), MusubiPublicationRuntimeTransportErrorV1> {
        self.location.validate().map_err(|_| {
            MusubiPublicationRuntimeTransportErrorV1::permanent(
                "MUSUBI_PROVIDER_READBACK_REQUEST_INVALID",
            )
        })?;
        self.commitment.validate().map_err(|_| {
            MusubiPublicationRuntimeTransportErrorV1::permanent(
                "MUSUBI_PROVIDER_READBACK_REQUEST_INVALID",
            )
        })?;
        if self.version != 1
            || self.operation_id.iter().all(|byte| *byte == 0)
            || self.genesis_block_hash.iter().all(|byte| *byte == 0)
            || self.location.archive_id != self.commitment.archive_id()
            || self.location.state == MusubiArchiveLocationStateV1::Retired
            || self
                .location
                .providers
                .binary_search(&self.provider)
                .is_err()
            || self.semantic_release_digest.is_zero()
            || self.verification_lock_digest.is_zero()
        {
            return Err(MusubiPublicationRuntimeTransportErrorV1::permanent(
                "MUSUBI_PROVIDER_READBACK_REQUEST_INVALID",
            ));
        }
        for attestation in &self.location.provider_attestations {
            let binding = &attestation.payload.binding;
            if attestation.verify(binding).is_err()
                || binding.chain_id != self.chain_id
                || binding.genesis_block_hash != self.genesis_block_hash
                || binding.archive_id != self.commitment.archive_id()
                || binding.bundle_digest != self.commitment.bundle_digest
                || binding.descriptor_digest != self.commitment.descriptor_digest
                || binding.source_tree_digest != self.commitment.source_tree_digest
                || binding.semantic_release_manifest_digest != self.semantic_release_digest
                || binding.verification_lock_digest != self.verification_lock_digest
            {
                return Err(MusubiPublicationRuntimeTransportErrorV1::permanent(
                    "MUSUBI_PROVIDER_READBACK_REQUEST_INVALID",
                ));
            }
        }
        Ok(())
    }
}

/// Exact commitment evidence returned by one provider-specific readback service.
#[derive(Clone, Debug, PartialEq, Eq, norito::derive::Encode, norito::derive::Decode)]
pub struct MusubiProviderReadbackResponseV1 {
    /// Closed schema version; must equal one.
    pub version: u8,
    /// Provider through which the full CAR was read.
    pub provider: ProviderId,
    /// Finalized location used for the readback.
    pub location_id: MusubiArchiveLocationIdV1,
    /// Replication order whose completion authorized this provider.
    pub replication_order: ReplicationOrderId,
    /// Commitment reproduced by parsing and validating the complete CAR.
    pub commitment: MusubiArchiveCommitmentV1,
    /// Semantic release digest parsed from the canonical bundle.
    pub semantic_release_digest: MusubiSemanticReleaseDigestV1,
    /// Verification-lock digest parsed from the canonical bundle.
    pub verification_lock_digest: MusubiVerificationLockDigestV1,
}

impl MusubiProviderReadbackResponseV1 {
    /// Validate the response against one exact provider request.
    pub fn validate_for(
        &self,
        request: &MusubiProviderReadbackRequestV1,
    ) -> Result<(), MusubiPublicationRuntimeTransportErrorV1> {
        if self.version != 1
            || self.provider != request.provider
            || self.location_id != request.location.location_id
            || self.replication_order != request.location.replication_order
            || self.commitment != request.commitment
            || self.semantic_release_digest != request.semantic_release_digest
            || self.verification_lock_digest != request.verification_lock_digest
        {
            return Err(MusubiPublicationRuntimeTransportErrorV1::permanent(
                "MUSUBI_PROVIDER_READBACK_RESPONSE_INVALID",
            ));
        }
        Ok(())
    }
}

/// Stable transport failure class without remote diagnostics or credentials.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MusubiPublicationRuntimeTransportFailureClassV1 {
    /// An identical idempotent request may succeed later.
    Retryable,
    /// Configuration, authorization, or response content must change.
    Permanent,
}

/// Redacted failure returned by the authenticated publication HTTPS client.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MusubiPublicationRuntimeTransportErrorV1 {
    class: MusubiPublicationRuntimeTransportFailureClassV1,
    code: &'static str,
}

impl MusubiPublicationRuntimeTransportErrorV1 {
    const fn retryable(code: &'static str) -> Self {
        Self {
            class: MusubiPublicationRuntimeTransportFailureClassV1::Retryable,
            code,
        }
    }

    const fn permanent(code: &'static str) -> Self {
        Self {
            class: MusubiPublicationRuntimeTransportFailureClassV1::Permanent,
            code,
        }
    }

    /// Return whether an identical idempotent call may be retried.
    #[must_use]
    pub const fn class(&self) -> MusubiPublicationRuntimeTransportFailureClassV1 {
        self.class
    }

    /// Return the stable payload-free failure code.
    #[must_use]
    pub const fn code(&self) -> &'static str {
        self.code
    }
}

impl fmt::Display for MusubiPublicationRuntimeTransportErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code)
    }
}

impl std::error::Error for MusubiPublicationRuntimeTransportErrorV1 {}

/// Maximum future clock skew accepted by the private publication service.
pub const MUSUBI_PUBLICATION_SERVICE_MAX_CLOCK_SKEW_MS_V1: u64 = 30_000;
/// Maximum number of append-only location transaction generations in one publication operation.
pub const MUSUBI_MAX_PUBLICATION_LOCATION_ATTEMPTS_V1: usize = 8;

fn storage_generation_target(generation: u8) -> [u8; 32] {
    let mut target = [0_u8; 32];
    target[0] = generation;
    target
}

fn valid_storage_generation_target(target: [u8; 32]) -> bool {
    target[0] > 0
        && usize::from(target[0]) <= MUSUBI_MAX_PUBLICATION_LOCATION_ATTEMPTS_V1
        && target[1..].iter().all(|byte| *byte == 0)
}
/// Exact private path for authenticated seed ingress.
pub const MUSUBI_PUBLICATION_SEED_INGRESS_PATH_V1: &str = "/v1/musubi/publication/seed-ingress";
/// Exact private path for permanent-pin and replication coordination.
pub const MUSUBI_PUBLICATION_STORAGE_COORDINATION_PATH_V1: &str =
    "/v1/musubi/publication/storage-coordinate";
/// Exact private path for provider-specific full-archive readback.
pub const MUSUBI_PUBLICATION_PROVIDER_READBACK_PATH_V1: &str =
    "/v1/musubi/publication/provider-readback";

/// One of the three closed private publication routes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum MusubiPublicationPrivateRouteV1 {
    /// Authenticate, verify, and stage one raw CAR.
    SeedIngress,
    /// Coordinate a permanent pin and finalized replication order.
    StorageCoordination,
    /// Verify a complete archive through one exact provider.
    ProviderReadback,
}

impl MusubiPublicationPrivateRouteV1 {
    /// Parse an exact private route path without prefix or suffix matching.
    #[must_use]
    pub fn parse(path: &str) -> Option<Self> {
        match path {
            MUSUBI_PUBLICATION_SEED_INGRESS_PATH_V1 => Some(Self::SeedIngress),
            MUSUBI_PUBLICATION_STORAGE_COORDINATION_PATH_V1 => Some(Self::StorageCoordination),
            MUSUBI_PUBLICATION_PROVIDER_READBACK_PATH_V1 => Some(Self::ProviderReadback),
            _ => None,
        }
    }

    /// Return the exact private route path.
    #[must_use]
    pub const fn path(self) -> &'static str {
        match self {
            Self::SeedIngress => MUSUBI_PUBLICATION_SEED_INGRESS_PATH_V1,
            Self::StorageCoordination => MUSUBI_PUBLICATION_STORAGE_COORDINATION_PATH_V1,
            Self::ProviderReadback => MUSUBI_PUBLICATION_PROVIDER_READBACK_PATH_V1,
        }
    }
}

/// Secret-free wire error returned by the private publication service.
#[derive(Clone, Copy, Debug, PartialEq, Eq, norito::derive::Encode, norito::derive::Decode)]
pub enum MusubiPublicationServiceErrorCodeV1 {
    /// The request did not select one exact fixed route.
    #[codec(index = 0)]
    RouteNotFound,
    /// The route's fixed request media type was not used.
    #[codec(index = 1)]
    MediaTypeInvalid,
    /// The authorization header was absent, malformed, noncanonical, or too large.
    #[codec(index = 2)]
    AuthorizationInvalid,
    /// The authorization is expired or outside the bounded clock-skew window.
    #[codec(index = 3)]
    AuthorizationExpired,
    /// The exact authorization was replayed before an idempotent result existed.
    #[codec(index = 4)]
    AuthorizationReplay,
    /// The request body or seed metadata was malformed, noncanonical, or too large.
    #[codec(index = 5)]
    RequestInvalid,
    /// The chain, genesis, publisher, broker, provider, or operation binding differs.
    #[codec(index = 6)]
    IdentityMismatch,
    /// The raw CAR length or digest differs from the authenticated binding.
    #[codec(index = 7)]
    CarBodyMismatch,
    /// An operation id was reused with different immutable request material.
    #[codec(index = 8)]
    OperationConflict,
    /// An identical operation is already being processed.
    #[codec(index = 9)]
    OperationBusy,
    /// The durable replay/idempotency journal is unavailable or full.
    #[codec(index = 10)]
    JournalUnavailable,
    /// The admitted seed-ingress backend could not stage the exact CAR.
    #[codec(index = 11)]
    SeedIngressUnavailable,
    /// The permanent-pin or replication coordinator could not complete.
    #[codec(index = 12)]
    StorageCoordinationUnavailable,
    /// The selected provider could not produce verified full-archive readback.
    #[codec(index = 13)]
    ProviderReadbackUnavailable,
    /// The injected broker signer could not issue the exact receipt.
    #[codec(index = 14)]
    ReceiptSigningUnavailable,
    /// An injected backend returned evidence that did not match its request.
    #[codec(index = 15)]
    BackendResponseInvalid,
    /// The service could not canonically encode its bounded response.
    #[codec(index = 16)]
    ResponseEncodingFailed,
    /// A method other than exact uppercase `POST` selected a private route.
    #[codec(index = 17)]
    MethodInvalid,
    /// The trusted service clock failed or regressed.
    #[codec(index = 18)]
    TrustedClockUnavailable,
}

impl MusubiPublicationServiceErrorCodeV1 {
    /// Return a stable payload-free diagnostic code.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::RouteNotFound => "MUSUBI_PUBLICATION_ROUTE_NOT_FOUND",
            Self::MediaTypeInvalid => "MUSUBI_PUBLICATION_MEDIA_TYPE_INVALID",
            Self::AuthorizationInvalid => "MUSUBI_PUBLICATION_AUTHORIZATION_INVALID",
            Self::AuthorizationExpired => "MUSUBI_PUBLICATION_AUTHORIZATION_EXPIRED",
            Self::AuthorizationReplay => "MUSUBI_PUBLICATION_AUTHORIZATION_REPLAY",
            Self::RequestInvalid => "MUSUBI_PUBLICATION_REQUEST_INVALID",
            Self::IdentityMismatch => "MUSUBI_PUBLICATION_IDENTITY_MISMATCH",
            Self::CarBodyMismatch => "MUSUBI_PUBLICATION_CAR_BODY_MISMATCH",
            Self::OperationConflict => "MUSUBI_PUBLICATION_OPERATION_CONFLICT",
            Self::OperationBusy => "MUSUBI_PUBLICATION_OPERATION_BUSY",
            Self::JournalUnavailable => "MUSUBI_PUBLICATION_JOURNAL_UNAVAILABLE",
            Self::SeedIngressUnavailable => "MUSUBI_PUBLICATION_SEED_INGRESS_UNAVAILABLE",
            Self::StorageCoordinationUnavailable => {
                "MUSUBI_PUBLICATION_STORAGE_COORDINATION_UNAVAILABLE"
            }
            Self::ProviderReadbackUnavailable => "MUSUBI_PUBLICATION_PROVIDER_READBACK_UNAVAILABLE",
            Self::ReceiptSigningUnavailable => "MUSUBI_PUBLICATION_RECEIPT_SIGNING_UNAVAILABLE",
            Self::BackendResponseInvalid => "MUSUBI_PUBLICATION_BACKEND_RESPONSE_INVALID",
            Self::ResponseEncodingFailed => "MUSUBI_PUBLICATION_RESPONSE_ENCODING_FAILED",
            Self::MethodInvalid => "MUSUBI_PUBLICATION_METHOD_INVALID",
            Self::TrustedClockUnavailable => "MUSUBI_PUBLICATION_TRUSTED_CLOCK_UNAVAILABLE",
        }
    }
}

/// Canonical bounded error response for every private publication route.
#[derive(Clone, Copy, Debug, PartialEq, Eq, norito::derive::Encode, norito::derive::Decode)]
pub struct MusubiPublicationServiceErrorResponseV1 {
    /// Closed response schema version; always one.
    pub version: u8,
    /// Typed stable error code with no remote or credential material.
    pub code: MusubiPublicationServiceErrorCodeV1,
    /// Whether a newly authorized identical request may succeed later.
    pub retryable: bool,
}

/// Transport-neutral private HTTP request passed by a deployment-owned HTTPS ingress.
#[derive(Clone, Copy, Debug)]
pub struct MusubiPublicationPrivateHttpRequestV1<'a> {
    /// Exact uppercase HTTP method; V1 accepts only `POST`.
    pub method: &'a str,
    /// Exact path after the deployment's private mount prefix is removed.
    pub path: &'a str,
    /// Exact `Content-Type` header value.
    pub content_type: &'a str,
    /// URL-safe, unpadded base64 canonical authorization header.
    pub authorization: Option<&'a str>,
    /// URL-safe, unpadded base64 canonical seed metadata header.
    pub seed_ingress_metadata: Option<&'a str>,
    /// Exact bounded request body.
    pub body: &'a [u8],
}

/// Transport-neutral private HTTP response emitted by the service core.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MusubiPublicationPrivateHttpResponseV1 {
    /// HTTP status code selected without exposing backend details.
    pub status: u16,
    /// Fixed response media type.
    pub content_type: &'static str,
    /// Canonical bounded Norito success or error body.
    pub body: Vec<u8>,
}

/// Public, non-secret identity constraints for one private publication service.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MusubiPublicationServiceConfigurationV1 {
    /// Exact chain accepted by every request.
    pub chain_id: ChainId,
    /// Exact committed genesis hash accepted by every request.
    pub genesis_block_hash: [u8; 32],
    /// Account whose controller signs seed-ingress receipts.
    pub ingress_broker: AccountId,
    /// Exact admitted provider served by seed ingress.
    pub seed_provider: ProviderId,
    /// Maximum publisher-clock lead accepted by authorization verification.
    pub max_future_clock_skew_ms: u64,
    /// Positive lifetime assigned to broker-signed staging receipts.
    pub receipt_lifetime_ms: u64,
}

impl MusubiPublicationServiceConfigurationV1 {
    fn validate(&self) -> Result<(), MusubiPublicationServiceErrorCodeV1> {
        if self.genesis_block_hash.iter().all(|byte| *byte == 0)
            || self.seed_provider.as_bytes().iter().all(|byte| *byte == 0)
            || self.max_future_clock_skew_ms > MUSUBI_PUBLICATION_SERVICE_MAX_CLOCK_SKEW_MS_V1
            || self.receipt_lifetime_ms == 0
            || self.receipt_lifetime_ms > MUSUBI_MAX_SEED_INGRESS_RECEIPT_LIFETIME_MS_V1
            || !controller_fits_publication_approval_bound(&self.ingress_broker)
        {
            return Err(MusubiPublicationServiceErrorCodeV1::IdentityMismatch);
        }
        Ok(())
    }
}

/// Stable deployment identity durably bound by a publication-service journal.
///
/// Timing policy is intentionally excluded so operators can adjust authorization skew or receipt
/// lifetime without discarding immutable replay/idempotency history.
#[derive(Clone, Debug, PartialEq, Eq, norito::derive::Encode, norito::derive::Decode)]
pub struct MusubiPublicationServiceJournalBindingV1 {
    /// Exact chain accepted by every retained operation.
    pub chain_id: ChainId,
    /// Exact committed genesis hash for that chain incarnation.
    pub genesis_block_hash: [u8; 32],
    /// Account whose controller signs retained seed-ingress receipts.
    pub ingress_broker: AccountId,
    /// Exact admitted seed provider for this private service.
    pub seed_provider: ProviderId,
}

impl MusubiPublicationServiceJournalBindingV1 {
    /// Derive the durable identity boundary from one service configuration.
    #[must_use]
    pub fn from_configuration(configuration: &MusubiPublicationServiceConfigurationV1) -> Self {
        Self {
            chain_id: configuration.chain_id.clone(),
            genesis_block_hash: configuration.genesis_block_hash,
            ingress_broker: configuration.ingress_broker.clone(),
            seed_provider: configuration.seed_provider,
        }
    }

    fn validate(&self) -> Result<(), MusubiPublicationServiceJournalErrorV1> {
        if self.genesis_block_hash.iter().all(|byte| *byte == 0)
            || self.seed_provider.as_bytes().iter().all(|byte| *byte == 0)
            || !controller_fits_publication_approval_bound(&self.ingress_broker)
            || norito::encode_canonical(self)
                .ok()
                .is_none_or(|encoded| encoded.len() > MAX_SEED_INGRESS_METADATA_BYTES)
        {
            return Err(MusubiPublicationServiceJournalErrorV1::Invalid);
        }
        Ok(())
    }
}

fn controller_fits_publication_approval_bound(account: &AccountId) -> bool {
    let AccountController::Multisig(policy) = account.controller() else {
        return true;
    };

    // `AccountId`'s generic Norito decode reconstructs its controller directly. Re-run the
    // policy constructor here so a canonical but structurally invalid wire policy cannot turn a
    // zero/duplicate/unsupported controller into publication authority.
    if MultisigPolicy::from_serialized(
        policy.version(),
        policy.threshold(),
        policy.members().to_vec(),
    )
    .is_err()
    {
        return false;
    }

    // Keep only the largest weights that could fit in one bounded publication approval set. This
    // is fixed-memory even if a deployment supplies a policy with an unusually large member set.
    let mut largest_weights = [0_u16; MUSUBI_MAX_PUBLICATION_ATTESTATION_APPROVALS_V1];
    for member in policy.members() {
        let weight = member.weight();
        let Some(insert_at) = largest_weights
            .iter()
            .position(|existing| weight > *existing)
        else {
            continue;
        };
        for index in (insert_at + 1..largest_weights.len()).rev() {
            largest_weights[index] = largest_weights[index - 1];
        }
        largest_weights[insert_at] = weight;
    }
    largest_weights.into_iter().map(u32::from).sum::<u32>() >= u32::from(policy.threshold())
}

/// Backend failure class deliberately carrying no provider diagnostics or secrets.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MusubiPublicationServiceBackendErrorV1 {
    /// A newly authorized identical request may succeed later.
    Retryable,
    /// Deployment configuration or immutable backend state must change.
    Permanent,
}

/// Admitted backend that durably stages an already verified exact CAR.
pub trait MusubiSeedIngressBackendV1: Send {
    /// Return the exact admitted provider served by this backend instance.
    ///
    /// The service constructor compares this identity with its public configuration before
    /// accepting traffic, so an injected backend cannot silently stage for another provider.
    fn provider_id(&self) -> ProviderId;

    /// Stage or idempotently reuse one exact operation and receipt binding.
    fn stage_exact_car(
        &mut self,
        operation_id: [u8; 32],
        binding: &MusubiSeedIngressReceiptBindingV1,
        car: &[u8],
    ) -> Result<(), MusubiPublicationServiceBackendErrorV1>;
}

/// Backend coordinating permanent pins, replication, and finalized provider completions.
pub trait MusubiStorageCoordinationBackendV1: Send {
    /// Coordinate or idempotently return evidence for one exact immutable request.
    ///
    /// The backend must independently retrieve the exact transaction, prove that its sole
    /// instruction is the matching archive registration finalized by the named snapshot, then
    /// match the immutable registration projection against a finalized archive read at that or a
    /// later snapshot. The authenticated publisher request binds those bytes but is not itself a
    /// finality proof. Mutable location fields are returned from the backend's current read and
    /// are deliberately excluded from the historical registration evidence.
    fn coordinate_storage(
        &mut self,
        request: &MusubiStorageCoordinationRequestV1,
    ) -> Result<MusubiStorageCoordinationResponseV1, MusubiPublicationServiceBackendErrorV1>;
}

/// Backend performing complete provider-specific archive and bundle verification.
pub trait MusubiProviderReadbackBackendV1: Send {
    /// Read, parse, and verify one exact committed CAR through the selected provider.
    fn readback_provider(
        &mut self,
        request: &MusubiProviderReadbackRequestV1,
    ) -> Result<MusubiProviderReadbackResponseV1, MusubiPublicationServiceBackendErrorV1>;
}

/// Immutable operation-wide binding enforced across all three private routes.
#[derive(
    Clone, Debug, PartialEq, Eq, PartialOrd, Ord, norito::derive::Encode, norito::derive::Decode,
)]
pub struct MusubiPublicationOperationBindingV1 {
    /// Stable publisher-selected operation id.
    pub operation_id: [u8; 32],
    /// Exact chain identity.
    pub chain_id: ChainId,
    /// Exact committed genesis hash for the selected chain incarnation.
    pub genesis_block_hash: [u8; 32],
    /// Exact publisher identity.
    pub publisher: AccountId,
    /// Derived immutable archive identity.
    pub archive_id: ArchiveId,
    /// Digest of the exact canonical CAR.
    pub car_body_digest: MusubiContentDigestV1,
    /// Length of the exact canonical CAR.
    pub car_body_length: u64,
}

impl MusubiPublicationOperationBindingV1 {
    fn validate(&self) -> Result<(), MusubiPublicationServiceJournalErrorV1> {
        if self.operation_id.iter().all(|byte| *byte == 0)
            || self.genesis_block_hash.iter().all(|byte| *byte == 0)
            || self.archive_id.is_zero()
            || self.car_body_digest.is_zero()
            || self.car_body_length == 0
            || self.car_body_length > MUSUBI_MAX_CAR_BYTES_V1
            || norito::encode_canonical(self)
                .ok()
                .is_none_or(|encoded| encoded.len() > MAX_SEED_INGRESS_METADATA_BYTES)
        {
            return Err(MusubiPublicationServiceJournalErrorV1::Invalid);
        }
        Ok(())
    }
}

/// Canonical key for one idempotent route result within a publication operation.
#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    norito::derive::Encode,
    norito::derive::Decode,
)]
pub struct MusubiPublicationIdempotencyKeyV1 {
    /// Exact private operation.
    pub operation: MusubiPublicationRuntimeOperationV1,
    /// Stable publisher-selected operation id.
    pub operation_id: [u8; 32],
    /// Route-specific target: zero for ingress, one-based location generation for storage
    /// coordination, or provider id for readback.
    pub target: [u8; 32],
}

/// Atomic journal attempt binding authorization replay to immutable operation state.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MusubiPublicationJournalAttemptV1 {
    /// Exact idempotent route key.
    pub key: MusubiPublicationIdempotencyKeyV1,
    /// Operation-wide immutable commitment.
    pub binding: MusubiPublicationOperationBindingV1,
    /// Domain-separated digest of the exact canonical typed request.
    pub request_digest: [u8; 32],
    /// Digest of the complete canonical signed authorization.
    pub authorization_digest: [u8; 32],
    /// Inclusive expiry used to bound replay retention.
    pub authorization_expires_at_ms: u64,
}

/// Result of atomically beginning one journaled service attempt.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MusubiPublicationJournalBeginV1 {
    /// This caller owns the new in-flight attempt and may invoke a backend.
    Execute,
    /// The exact request already completed; return these canonical response bytes.
    Cached(Vec<u8>),
}

/// Stable journal failure with no filesystem, database, or credential details.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MusubiPublicationServiceJournalErrorV1 {
    /// The request binding or digest is structurally invalid.
    Invalid,
    /// An operation or route key was reused with substituted immutable material.
    Conflict,
    /// The exact authorization was already consumed by an unfinished attempt.
    Replay,
    /// Another caller owns the exact in-flight attempt.
    Busy,
    /// The bounded journal has no safe capacity for a new operation.
    Capacity,
    /// Durable journal state could not be read or committed.
    Unavailable,
}

/// Durable atomic replay and idempotency boundary required by production services.
///
/// A crash-safe implementation must recover an interrupted fresh attempt as an aborted
/// request-digest tombstone and an interrupted seed-receipt refresh as its retained prior
/// completed response before accepting traffic. Authorization digests already consumed by either
/// attempt remain consumed through their expiry.
pub trait MusubiPublicationServiceJournalV1: Send {
    /// Return the exact deployment identity bound by every retained record.
    fn deployment_binding(&self) -> &MusubiPublicationServiceJournalBindingV1;

    /// Atomically validate the operation binding and detect a cached result before reserving a
    /// new attempt and consuming its authorization digest. Cached seed receipts must remain
    /// available to [`Self::refresh_expired_seed_receipt`] with that same fresh authorization.
    fn begin(
        &mut self,
        attempt: &MusubiPublicationJournalAttemptV1,
        current_time_ms: u64,
    ) -> Result<MusubiPublicationJournalBeginV1, MusubiPublicationServiceJournalErrorV1>;

    /// Atomically reopen an exact completed seed-ingress result after its receipt expired.
    ///
    /// The implementation must compare `expected_response`, consume the fresh authorization,
    /// and retain the prior completed response so [`Self::abort`] can restore it on failure.
    fn refresh_expired_seed_receipt(
        &mut self,
        attempt: &MusubiPublicationJournalAttemptV1,
        expected_response: &[u8],
        current_time_ms: u64,
    ) -> Result<(), MusubiPublicationServiceJournalErrorV1>;

    /// Atomically persist the canonical successful response for the reserved attempt.
    fn commit(
        &mut self,
        key: MusubiPublicationIdempotencyKeyV1,
        request_digest: [u8; 32],
        response: &[u8],
    ) -> Result<(), MusubiPublicationServiceJournalErrorV1>;

    /// Release an unfinished reservation while retaining its request-digest tombstone and
    /// consumed-authorization replay state.
    fn abort(
        &mut self,
        key: MusubiPublicationIdempotencyKeyV1,
        request_digest: [u8; 32],
    ) -> Result<(), MusubiPublicationServiceJournalErrorV1>;
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum InMemoryPublicationResultV1 {
    Pending([u8; 32]),
    Aborted([u8; 32]),
    Refreshing {
        request_digest: [u8; 32],
        previous_response: Vec<u8>,
    },
    Complete {
        request_digest: [u8; 32],
        response: Vec<u8>,
    },
}

/// Bounded process-local journal intended for tests and ephemeral development services.
///
/// Production deployments must inject a crash-safe implementation of
/// [`MusubiPublicationServiceJournalV1`]; this type deliberately makes no durability claim.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InMemoryMusubiPublicationServiceJournalV1 {
    binding: MusubiPublicationServiceJournalBindingV1,
    max_operations: usize,
    max_results: usize,
    max_authorizations: usize,
    operation_bindings: BTreeMap<[u8; 32], MusubiPublicationOperationBindingV1>,
    results: BTreeMap<MusubiPublicationIdempotencyKeyV1, InMemoryPublicationResultV1>,
    authorization_expiry: BTreeMap<[u8; 32], u64>,
    expiry_index: BTreeSet<(u64, [u8; 32])>,
}

impl InMemoryMusubiPublicationServiceJournalV1 {
    /// Construct a bounded ephemeral journal.
    ///
    /// # Errors
    ///
    /// Returns [`MusubiPublicationServiceJournalErrorV1::Invalid`] when the deployment binding is
    /// invalid, either bound is zero, or the protocol-derived result capacity overflows the
    /// platform size.
    pub fn new(
        binding: MusubiPublicationServiceJournalBindingV1,
        max_operations: usize,
        max_authorizations: usize,
    ) -> Result<Self, MusubiPublicationServiceJournalErrorV1> {
        binding.validate()?;
        if max_operations == 0 || max_authorizations == 0 {
            return Err(MusubiPublicationServiceJournalErrorV1::Invalid);
        }
        let max_results = max_operations
            .checked_mul(
                MUSUBI_MAX_ARCHIVE_LOCATIONS_V1
                    .saturating_mul(MUSUBI_MAX_LOCATION_PROVIDERS_V1)
                    .saturating_add(1)
                    .saturating_add(MUSUBI_MAX_PUBLICATION_LOCATION_ATTEMPTS_V1),
            )
            .ok_or(MusubiPublicationServiceJournalErrorV1::Invalid)?;
        Ok(Self {
            binding,
            max_operations,
            max_results,
            max_authorizations,
            operation_bindings: BTreeMap::new(),
            results: BTreeMap::new(),
            authorization_expiry: BTreeMap::new(),
            expiry_index: BTreeSet::new(),
        })
    }

    fn prune_authorizations(&mut self, current_time_ms: u64) {
        let expired = self
            .expiry_index
            .range(..(current_time_ms, [0_u8; 32]))
            .copied()
            .collect::<Vec<_>>();
        for entry @ (_, digest) in expired {
            self.expiry_index.remove(&entry);
            self.authorization_expiry.remove(&digest);
        }
    }

    fn validate_attempt(
        &self,
        attempt: &MusubiPublicationJournalAttemptV1,
        current_time_ms: u64,
    ) -> Result<(), MusubiPublicationServiceJournalErrorV1> {
        attempt.binding.validate()?;
        if current_time_ms == 0
            || attempt.binding.chain_id != self.binding.chain_id
            || attempt.binding.genesis_block_hash != self.binding.genesis_block_hash
            || attempt.key.operation_id != attempt.binding.operation_id
            || match attempt.key.operation {
                MusubiPublicationRuntimeOperationV1::ProviderReadback => {
                    attempt.key.target.iter().all(|byte| *byte == 0)
                }
                MusubiPublicationRuntimeOperationV1::SeedIngress => {
                    attempt.key.target.iter().any(|byte| *byte != 0)
                }
                MusubiPublicationRuntimeOperationV1::StorageCoordination => {
                    !valid_storage_generation_target(attempt.key.target)
                }
            }
            || attempt.request_digest.iter().all(|byte| *byte == 0)
            || attempt.authorization_digest.iter().all(|byte| *byte == 0)
            || attempt.authorization_expires_at_ms == 0
            || attempt.authorization_expires_at_ms < current_time_ms
        {
            return Err(MusubiPublicationServiceJournalErrorV1::Invalid);
        }
        Ok(())
    }

    fn consume_authorization(
        &mut self,
        attempt: &MusubiPublicationJournalAttemptV1,
        current_time_ms: u64,
    ) -> Result<(), MusubiPublicationServiceJournalErrorV1> {
        self.prune_authorizations(current_time_ms);
        if self
            .authorization_expiry
            .contains_key(&attempt.authorization_digest)
        {
            return Err(MusubiPublicationServiceJournalErrorV1::Replay);
        }
        if self.authorization_expiry.len() >= self.max_authorizations {
            return Err(MusubiPublicationServiceJournalErrorV1::Capacity);
        }
        self.authorization_expiry.insert(
            attempt.authorization_digest,
            attempt.authorization_expires_at_ms,
        );
        self.expiry_index.insert((
            attempt.authorization_expires_at_ms,
            attempt.authorization_digest,
        ));
        Ok(())
    }
}

impl MusubiPublicationServiceJournalV1 for InMemoryMusubiPublicationServiceJournalV1 {
    fn deployment_binding(&self) -> &MusubiPublicationServiceJournalBindingV1 {
        &self.binding
    }

    fn begin(
        &mut self,
        attempt: &MusubiPublicationJournalAttemptV1,
        current_time_ms: u64,
    ) -> Result<MusubiPublicationJournalBeginV1, MusubiPublicationServiceJournalErrorV1> {
        self.validate_attempt(attempt, current_time_ms)?;
        if let Some(existing) = self.operation_bindings.get(&attempt.binding.operation_id) {
            if existing != &attempt.binding {
                return Err(MusubiPublicationServiceJournalErrorV1::Conflict);
            }
        } else if self.operation_bindings.len() >= self.max_operations {
            return Err(MusubiPublicationServiceJournalErrorV1::Capacity);
        }

        let retrying_aborted = if let Some(existing) = self.results.get(&attempt.key) {
            match existing {
                InMemoryPublicationResultV1::Pending(request_digest) => {
                    if request_digest == &attempt.request_digest {
                        return Err(MusubiPublicationServiceJournalErrorV1::Busy);
                    } else {
                        return Err(MusubiPublicationServiceJournalErrorV1::Conflict);
                    }
                }
                InMemoryPublicationResultV1::Aborted(request_digest) => {
                    if request_digest == &attempt.request_digest {
                        true
                    } else {
                        return Err(MusubiPublicationServiceJournalErrorV1::Conflict);
                    }
                }
                InMemoryPublicationResultV1::Refreshing { request_digest, .. } => {
                    if request_digest == &attempt.request_digest {
                        return Err(MusubiPublicationServiceJournalErrorV1::Busy);
                    } else {
                        return Err(MusubiPublicationServiceJournalErrorV1::Conflict);
                    }
                }
                InMemoryPublicationResultV1::Complete {
                    request_digest,
                    response,
                } => {
                    if request_digest == &attempt.request_digest {
                        return Ok(MusubiPublicationJournalBeginV1::Cached(response.clone()));
                    } else {
                        return Err(MusubiPublicationServiceJournalErrorV1::Conflict);
                    }
                }
            }
        } else {
            false
        };
        if !retrying_aborted && self.results.len() >= self.max_results {
            return Err(MusubiPublicationServiceJournalErrorV1::Capacity);
        }
        if !retrying_aborted
            && attempt.key.operation == MusubiPublicationRuntimeOperationV1::ProviderReadback
            && self
                .results
                .keys()
                .filter(|key| {
                    key.operation_id == attempt.key.operation_id
                        && key.operation == MusubiPublicationRuntimeOperationV1::ProviderReadback
                })
                .count()
                >= MUSUBI_MAX_ARCHIVE_LOCATIONS_V1.saturating_mul(MUSUBI_MAX_LOCATION_PROVIDERS_V1)
        {
            return Err(MusubiPublicationServiceJournalErrorV1::Capacity);
        }
        if !retrying_aborted
            && attempt.key.operation == MusubiPublicationRuntimeOperationV1::StorageCoordination
            && self
                .results
                .keys()
                .filter(|key| {
                    key.operation_id == attempt.key.operation_id
                        && key.operation == MusubiPublicationRuntimeOperationV1::StorageCoordination
                })
                .count()
                >= MUSUBI_MAX_PUBLICATION_LOCATION_ATTEMPTS_V1
        {
            return Err(MusubiPublicationServiceJournalErrorV1::Capacity);
        }

        self.consume_authorization(attempt, current_time_ms)?;
        self.operation_bindings
            .entry(attempt.binding.operation_id)
            .or_insert_with(|| attempt.binding.clone());
        self.results.insert(
            attempt.key,
            InMemoryPublicationResultV1::Pending(attempt.request_digest),
        );
        Ok(MusubiPublicationJournalBeginV1::Execute)
    }

    fn refresh_expired_seed_receipt(
        &mut self,
        attempt: &MusubiPublicationJournalAttemptV1,
        expected_response: &[u8],
        current_time_ms: u64,
    ) -> Result<(), MusubiPublicationServiceJournalErrorV1> {
        self.validate_attempt(attempt, current_time_ms)?;
        if attempt.key.operation != MusubiPublicationRuntimeOperationV1::SeedIngress
            || expected_response.is_empty()
            || expected_response.len() > MAX_CONTROL_RESPONSE_BYTES
        {
            return Err(MusubiPublicationServiceJournalErrorV1::Invalid);
        }
        if self.operation_bindings.get(&attempt.binding.operation_id) != Some(&attempt.binding) {
            return Err(MusubiPublicationServiceJournalErrorV1::Conflict);
        }
        let previous_response = match self.results.get(&attempt.key) {
            Some(InMemoryPublicationResultV1::Complete {
                request_digest,
                response,
            }) if *request_digest == attempt.request_digest && response == expected_response => {
                response.clone()
            }
            Some(InMemoryPublicationResultV1::Complete { request_digest, .. })
                if *request_digest != attempt.request_digest =>
            {
                return Err(MusubiPublicationServiceJournalErrorV1::Conflict);
            }
            Some(InMemoryPublicationResultV1::Complete { .. })
            | Some(InMemoryPublicationResultV1::Pending(_))
            | Some(InMemoryPublicationResultV1::Aborted(_))
            | Some(InMemoryPublicationResultV1::Refreshing { .. }) => {
                return Err(MusubiPublicationServiceJournalErrorV1::Busy);
            }
            None => return Err(MusubiPublicationServiceJournalErrorV1::Conflict),
        };
        self.consume_authorization(attempt, current_time_ms)?;
        self.results.insert(
            attempt.key,
            InMemoryPublicationResultV1::Refreshing {
                request_digest: attempt.request_digest,
                previous_response,
            },
        );
        Ok(())
    }

    fn commit(
        &mut self,
        key: MusubiPublicationIdempotencyKeyV1,
        request_digest: [u8; 32],
        response: &[u8],
    ) -> Result<(), MusubiPublicationServiceJournalErrorV1> {
        if response.is_empty() || response.len() > MAX_CONTROL_RESPONSE_BYTES {
            return Err(MusubiPublicationServiceJournalErrorV1::Invalid);
        }
        let Some(existing) = self.results.get_mut(&key) else {
            return Err(MusubiPublicationServiceJournalErrorV1::Unavailable);
        };
        match existing {
            InMemoryPublicationResultV1::Pending(existing_digest)
                if *existing_digest == request_digest =>
            {
                *existing = InMemoryPublicationResultV1::Complete {
                    request_digest,
                    response: response.to_vec(),
                };
                Ok(())
            }
            InMemoryPublicationResultV1::Refreshing {
                request_digest: existing_digest,
                ..
            } if *existing_digest == request_digest => {
                *existing = InMemoryPublicationResultV1::Complete {
                    request_digest,
                    response: response.to_vec(),
                };
                Ok(())
            }
            InMemoryPublicationResultV1::Complete {
                request_digest: existing_digest,
                response: existing_response,
            } if *existing_digest == request_digest && existing_response == response => Ok(()),
            _ => Err(MusubiPublicationServiceJournalErrorV1::Conflict),
        }
    }

    fn abort(
        &mut self,
        key: MusubiPublicationIdempotencyKeyV1,
        request_digest: [u8; 32],
    ) -> Result<(), MusubiPublicationServiceJournalErrorV1> {
        match self.results.get(&key) {
            Some(InMemoryPublicationResultV1::Pending(existing)) if *existing == request_digest => {
                self.results
                    .insert(key, InMemoryPublicationResultV1::Aborted(request_digest));
                Ok(())
            }
            Some(InMemoryPublicationResultV1::Aborted(existing)) if *existing == request_digest => {
                Ok(())
            }
            Some(InMemoryPublicationResultV1::Complete {
                request_digest: existing,
                ..
            }) if *existing == request_digest => Ok(()),
            Some(InMemoryPublicationResultV1::Refreshing {
                request_digest: existing,
                previous_response,
            }) if *existing == request_digest => {
                let previous_response = previous_response.clone();
                self.results.insert(
                    key,
                    InMemoryPublicationResultV1::Complete {
                        request_digest,
                        response: previous_response,
                    },
                );
                Ok(())
            }
            Some(InMemoryPublicationResultV1::Pending(_)) => {
                Err(MusubiPublicationServiceJournalErrorV1::Conflict)
            }
            Some(InMemoryPublicationResultV1::Aborted(_)) => {
                Err(MusubiPublicationServiceJournalErrorV1::Conflict)
            }
            Some(InMemoryPublicationResultV1::Refreshing { .. }) => {
                Err(MusubiPublicationServiceJournalErrorV1::Conflict)
            }
            Some(InMemoryPublicationResultV1::Complete { .. }) => {
                Err(MusubiPublicationServiceJournalErrorV1::Conflict)
            }
            None => Ok(()),
        }
    }
}

/// Deployment-owned signing boundary for one exact seed-ingress receipt payload.
///
/// Implementations may call an HSM, KMS, or threshold collection service. They return only
/// controller approvals: the publication service constructs the payload and lifetime, then
/// verifies the assembled receipt before committing it to the replay journal. This prevents a
/// signer implementation from substituting any chain, publisher, archive, body, nonce, or expiry
/// field.
pub trait MusubiSeedIngressReceiptSigningProviderV1: Send {
    /// Exact broker account controlled by this signing provider.
    fn broker(&self) -> &AccountId;

    /// Sign the exact service-constructed payload with the broker controller.
    ///
    /// # Errors
    ///
    /// Returns a redacted retryable failure for a transient signer outage or a permanent failure
    /// when deployment policy cannot sign the payload. Returned approvals are independently
    /// verified by the service. Remote adapters should use `payload.expires_at_ms` as the signing
    /// call deadline so work that cannot produce a live receipt is cancelled promptly.
    fn sign_approvals(
        &mut self,
        payload: &MusubiSeedIngressReceiptPayloadV1,
    ) -> Result<Vec<MusubiSeedIngressReceiptApprovalV1>, MusubiPublicationServiceBackendErrorV1>;
}

/// Trusted wall-clock boundary used for authorization and receipt freshness.
///
/// Production implementations must persist or otherwise enforce a non-regressing time floor
/// across service restarts. The service additionally rejects regressions observed during one
/// process lifetime.
pub trait MusubiPublicationServiceClockV1: Send {
    /// Return current Unix time in milliseconds.
    ///
    /// # Errors
    ///
    /// Returns a redacted backend failure when the trusted clock cannot be sampled.
    fn current_time_ms(&mut self) -> Result<u64, MusubiPublicationServiceBackendErrorV1>;
}

/// Raw system wall clock for qualified private-publication service runners.
///
/// This adapter does not itself persist a high-water mark. A production platform may use it only
/// when its clock is rollback-resistant across process restarts; otherwise it must inject a clock
/// backed by a durable time floor.
#[derive(Clone, Copy, Debug, Default)]
pub struct MusubiPublicationSystemClockV1;

impl MusubiPublicationServiceClockV1 for MusubiPublicationSystemClockV1 {
    fn current_time_ms(&mut self) -> Result<u64, MusubiPublicationServiceBackendErrorV1> {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| MusubiPublicationServiceBackendErrorV1::Retryable)
            .and_then(|elapsed| {
                u64::try_from(elapsed.as_millis())
                    .map_err(|_| MusubiPublicationServiceBackendErrorV1::Permanent)
            })
    }
}

/// Runtime-only software signing adapter for a single-controller ingress broker.
///
/// Production deployments should inject an HSM/KMS implementation of
/// [`MusubiSeedIngressReceiptSigningProviderV1`]. This adapter exists for focused tests and
/// explicitly controlled development deployments.
pub struct SoftwareMusubiSeedIngressReceiptSignerV1 {
    broker: AccountId,
    key_pair: KeyPair,
}

impl fmt::Debug for SoftwareMusubiSeedIngressReceiptSignerV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SoftwareMusubiSeedIngressReceiptSignerV1")
            .field("broker", &self.broker)
            .finish_non_exhaustive()
    }
}

impl SoftwareMusubiSeedIngressReceiptSignerV1 {
    /// Construct from deployment-owned key material that exactly controls the broker account.
    ///
    /// # Errors
    ///
    /// Returns an identity error when the account is multisig or the key does not control it.
    pub fn new(
        broker: AccountId,
        key_pair: KeyPair,
    ) -> Result<Self, MusubiPublicationServiceErrorCodeV1> {
        let AccountController::Single(expected_key) = broker.controller() else {
            return Err(MusubiPublicationServiceErrorCodeV1::IdentityMismatch);
        };
        if expected_key != key_pair.public_key() {
            return Err(MusubiPublicationServiceErrorCodeV1::IdentityMismatch);
        }
        Ok(Self { broker, key_pair })
    }
}

impl MusubiSeedIngressReceiptSigningProviderV1 for SoftwareMusubiSeedIngressReceiptSignerV1 {
    fn broker(&self) -> &AccountId {
        &self.broker
    }

    fn sign_approvals(
        &mut self,
        payload: &MusubiSeedIngressReceiptPayloadV1,
    ) -> Result<Vec<MusubiSeedIngressReceiptApprovalV1>, MusubiPublicationServiceBackendErrorV1>
    {
        if payload.binding.ingress_broker != self.broker {
            return Err(MusubiPublicationServiceBackendErrorV1::Permanent);
        }
        let signature =
            SignatureOf::try_from_hash(self.key_pair.private_key(), payload.signing_hash())
                .map_err(|_| MusubiPublicationServiceBackendErrorV1::Permanent)?;
        Ok(vec![MusubiSeedIngressReceiptApprovalV1 {
            public_key: self.key_pair.public_key().clone(),
            signature,
        }])
    }
}

/// Complete transport-independent server counterpart for private Musubi publication routes.
///
/// The service owns no listener, TLS key, platform credential loader, or SoraFS implementation.
/// A deployment injects those boundaries together with a crash-safe journal. Requests are
/// handled serially by `&mut self`; an ingress may place the service behind one mutex when it
/// needs concurrency.
pub struct MusubiPublicationPrivateServiceV1 {
    config: MusubiPublicationServiceConfigurationV1,
    clock: Box<dyn MusubiPublicationServiceClockV1>,
    last_clock_ms: Option<u64>,
    receipt_signer: Box<dyn MusubiSeedIngressReceiptSigningProviderV1>,
    journal: Box<dyn MusubiPublicationServiceJournalV1>,
    seed_ingress: Box<dyn MusubiSeedIngressBackendV1>,
    storage: Box<dyn MusubiStorageCoordinationBackendV1>,
    readback: Box<dyn MusubiProviderReadbackBackendV1>,
}

impl fmt::Debug for MusubiPublicationPrivateServiceV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("MusubiPublicationPrivateServiceV1")
            .field("config", &self.config)
            .finish_non_exhaustive()
    }
}

impl MusubiPublicationPrivateServiceV1 {
    /// Construct a fail-closed service from deployment-owned runtime dependencies.
    ///
    /// # Errors
    ///
    /// Returns an identity error when public configuration is invalid, differs from the injected
    /// signing provider's broker, differs from the seed backend's admitted provider, or differs
    /// from the journal's immutable deployment binding. Production clock and journal adapters
    /// must reject rollback across service restarts.
    pub fn new(
        config: MusubiPublicationServiceConfigurationV1,
        clock: Box<dyn MusubiPublicationServiceClockV1>,
        receipt_signer: Box<dyn MusubiSeedIngressReceiptSigningProviderV1>,
        journal: Box<dyn MusubiPublicationServiceJournalV1>,
        seed_ingress: Box<dyn MusubiSeedIngressBackendV1>,
        storage: Box<dyn MusubiStorageCoordinationBackendV1>,
        readback: Box<dyn MusubiProviderReadbackBackendV1>,
    ) -> Result<Self, MusubiPublicationServiceErrorCodeV1> {
        config.validate()?;
        let journal_binding = MusubiPublicationServiceJournalBindingV1::from_configuration(&config);
        journal_binding
            .validate()
            .map_err(|_| MusubiPublicationServiceErrorCodeV1::IdentityMismatch)?;
        if &config.ingress_broker != receipt_signer.broker()
            || config.seed_provider != seed_ingress.provider_id()
            || journal.deployment_binding() != &journal_binding
        {
            return Err(MusubiPublicationServiceErrorCodeV1::IdentityMismatch);
        }
        Ok(Self {
            config,
            clock,
            last_clock_ms: None,
            receipt_signer,
            journal,
            seed_ingress,
            storage,
            readback,
        })
    }

    /// Handle one exact private HTTP request and always return a bounded typed response.
    #[must_use]
    pub fn handle(
        &mut self,
        request: MusubiPublicationPrivateHttpRequestV1<'_>,
    ) -> MusubiPublicationPrivateHttpResponseV1 {
        match self.try_handle(request) {
            Ok(body) => MusubiPublicationPrivateHttpResponseV1 {
                status: 200,
                content_type: APPLICATION_NORITO,
                body,
            },
            Err(error) => error.into_response(),
        }
    }

    fn try_handle(
        &mut self,
        request: MusubiPublicationPrivateHttpRequestV1<'_>,
    ) -> Result<Vec<u8>, MusubiPublicationServiceErrorV1> {
        if request.method != "POST" {
            return Err(MusubiPublicationServiceErrorV1::permanent(
                MusubiPublicationServiceErrorCodeV1::MethodInvalid,
            ));
        }
        let route = MusubiPublicationPrivateRouteV1::parse(request.path).ok_or_else(|| {
            MusubiPublicationServiceErrorV1::permanent(
                MusubiPublicationServiceErrorCodeV1::RouteNotFound,
            )
        })?;
        match route {
            MusubiPublicationPrivateRouteV1::SeedIngress => {
                if request.content_type != APPLICATION_SORAFS_CAR {
                    return Err(MusubiPublicationServiceErrorV1::permanent(
                        MusubiPublicationServiceErrorCodeV1::MediaTypeInvalid,
                    ));
                }
                let current_time_ms = self.sample_time()?;
                self.handle_seed_ingress(request, current_time_ms)
            }
            MusubiPublicationPrivateRouteV1::StorageCoordination => {
                if request.content_type != APPLICATION_NORITO
                    || request.seed_ingress_metadata.is_some()
                {
                    return Err(MusubiPublicationServiceErrorV1::permanent(
                        MusubiPublicationServiceErrorCodeV1::MediaTypeInvalid,
                    ));
                }
                let current_time_ms = self.sample_time()?;
                self.handle_storage_coordination(request, current_time_ms)
            }
            MusubiPublicationPrivateRouteV1::ProviderReadback => {
                if request.content_type != APPLICATION_NORITO
                    || request.seed_ingress_metadata.is_some()
                {
                    return Err(MusubiPublicationServiceErrorV1::permanent(
                        MusubiPublicationServiceErrorCodeV1::MediaTypeInvalid,
                    ));
                }
                let current_time_ms = self.sample_time()?;
                self.handle_provider_readback(request, current_time_ms)
            }
        }
    }

    fn handle_seed_ingress(
        &mut self,
        http: MusubiPublicationPrivateHttpRequestV1<'_>,
        current_time_ms: u64,
    ) -> Result<Vec<u8>, MusubiPublicationServiceErrorV1> {
        let metadata = decode_canonical_base64::<MusubiSeedIngressStageRequestV1>(
            http.seed_ingress_metadata.ok_or_else(|| {
                MusubiPublicationServiceErrorV1::permanent(
                    MusubiPublicationServiceErrorCodeV1::RequestInvalid,
                )
            })?,
            MAX_SEED_INGRESS_METADATA_BYTES,
            REQUEST_METADATA_DECODE_LIMITS,
            MusubiPublicationServiceErrorCodeV1::RequestInvalid,
        )?;
        metadata.value.validate().map_err(|_| {
            MusubiPublicationServiceErrorV1::permanent(
                MusubiPublicationServiceErrorCodeV1::RequestInvalid,
            )
        })?;
        self.validate_seed_identity(&metadata.value)?;
        if http.body.is_empty()
            || http.body.len() > usize::try_from(MUSUBI_MAX_CAR_BYTES_V1).unwrap_or(usize::MAX)
            || u64::try_from(http.body.len()).ok() != Some(metadata.value.binding.car_body_length)
        {
            return Err(MusubiPublicationServiceErrorV1::permanent(
                MusubiPublicationServiceErrorCodeV1::CarBodyMismatch,
            ));
        }
        let digest = request_digest(
            MusubiPublicationRuntimeOperationV1::SeedIngress,
            &metadata.canonical,
        )
        .map_err(|_| {
            MusubiPublicationServiceErrorV1::permanent(
                MusubiPublicationServiceErrorCodeV1::RequestInvalid,
            )
        })?;
        let authorization = self.decode_and_verify_authorization(
            &http,
            MusubiPublicationRuntimeOperationV1::SeedIngress,
            metadata.value.operation_id,
            digest,
            &metadata.value.binding.chain_id,
            &metadata.value.binding.publisher,
            current_time_ms,
        )?;
        if blake3::hash(http.body).as_bytes() != metadata.value.binding.car_body_digest.as_bytes() {
            return Err(MusubiPublicationServiceErrorV1::permanent(
                MusubiPublicationServiceErrorCodeV1::CarBodyMismatch,
            )
            .integrity_failure(MusubiIntegritySurfaceV1::ArchiveCommitment));
        }
        let attempt = journal_attempt(
            MusubiPublicationRuntimeOperationV1::SeedIngress,
            metadata.value.operation_id,
            [0_u8; 32],
            &metadata.value.binding.chain_id,
            metadata.value.binding.genesis_block_hash,
            &metadata.value.binding.publisher,
            metadata.value.binding.archive_id,
            metadata.value.binding.car_body_digest,
            metadata.value.binding.car_body_length,
            digest,
            &authorization,
        );
        match self
            .journal
            .begin(&attempt, current_time_ms)
            .map_err(seed_ingress_journal_error)?
        {
            MusubiPublicationJournalBeginV1::Cached(response) => {
                let receipt: MusubiSeedIngressReceiptV1 = decode_cached_response(&response)
                    .map_err(|error| {
                        error.ingest_deadletter(MusubiIngestDeadletterReasonV1::ReceiptInvalid)
                    })?
                    .value;
                if receipt
                    .verify(&metadata.value.binding, current_time_ms)
                    .is_ok()
                {
                    return Ok(response);
                }
                receipt
                    .verify(&metadata.value.binding, receipt.payload.issued_at_ms)
                    .map_err(|_| {
                        MusubiPublicationServiceErrorV1::permanent(
                            MusubiPublicationServiceErrorCodeV1::BackendResponseInvalid,
                        )
                        .ingest_deadletter(MusubiIngestDeadletterReasonV1::ReceiptInvalid)
                    })?;
                if current_time_ms <= receipt.payload.expires_at_ms {
                    return Err(MusubiPublicationServiceErrorV1::permanent(
                        MusubiPublicationServiceErrorCodeV1::BackendResponseInvalid,
                    )
                    .ingest_deadletter(MusubiIngestDeadletterReasonV1::ReceiptInvalid));
                }
                self.journal
                    .refresh_expired_seed_receipt(&attempt, &response, current_time_ms)
                    .map_err(seed_ingress_journal_error)?;
            }
            MusubiPublicationJournalBeginV1::Execute => {}
        }

        let result = self
            .seed_ingress
            .stage_exact_car(
                metadata.value.operation_id,
                &metadata.value.binding,
                http.body,
            )
            .map_err(seed_ingress_backend_error)
            .and_then(|()| self.issue_seed_ingress_receipt(metadata.value.binding))
            .and_then(encode_service_response);
        self.finish_attempt(attempt.key, digest, result)
    }

    fn issue_seed_ingress_receipt(
        &mut self,
        binding: MusubiSeedIngressReceiptBindingV1,
    ) -> Result<MusubiSeedIngressReceiptV1, MusubiPublicationServiceErrorV1> {
        if binding.ingress_broker != self.config.ingress_broker {
            return Err(MusubiPublicationServiceErrorV1::permanent(
                MusubiPublicationServiceErrorCodeV1::IdentityMismatch,
            ));
        }
        let issued_at_ms = self.sample_time()?;
        let expires_at_ms = issued_at_ms
            .checked_add(self.config.receipt_lifetime_ms)
            .ok_or_else(|| {
                MusubiPublicationServiceErrorV1::permanent(
                    MusubiPublicationServiceErrorCodeV1::TrustedClockUnavailable,
                )
            })?;
        let payload = MusubiSeedIngressReceiptPayloadV1 {
            version: 1,
            binding,
            issued_at_ms,
            expires_at_ms,
        };
        payload.validate().map_err(|_| {
            MusubiPublicationServiceErrorV1::permanent(
                MusubiPublicationServiceErrorCodeV1::ReceiptSigningUnavailable,
            )
            .ingest_deadletter(MusubiIngestDeadletterReasonV1::ReceiptInvalid)
        })?;
        let approvals = self
            .receipt_signer
            .sign_approvals(&payload)
            .map_err(|error| {
                service_backend_error(
                    MusubiPublicationServiceErrorCodeV1::ReceiptSigningUnavailable,
                    error,
                )
            })?;
        let receipt = MusubiSeedIngressReceiptV1 { payload, approvals };
        let verification_time_ms = self.sample_time()?;
        if verification_time_ms >= receipt.payload.expires_at_ms {
            return Err(MusubiPublicationServiceErrorV1::retryable(
                MusubiPublicationServiceErrorCodeV1::ReceiptSigningUnavailable,
            ));
        }
        receipt
            .verify(&receipt.payload.binding, verification_time_ms)
            .map_err(|_| {
                MusubiPublicationServiceErrorV1::permanent(
                    MusubiPublicationServiceErrorCodeV1::ReceiptSigningUnavailable,
                )
                .ingest_deadletter(MusubiIngestDeadletterReasonV1::ReceiptInvalid)
            })?;
        Ok(receipt)
    }

    fn handle_storage_coordination(
        &mut self,
        http: MusubiPublicationPrivateHttpRequestV1<'_>,
        current_time_ms: u64,
    ) -> Result<Vec<u8>, MusubiPublicationServiceErrorV1> {
        let decoded = decode_canonical_body::<MusubiStorageCoordinationRequestV1>(
            http.body,
            MAX_CONTROL_REQUEST_BYTES,
            CONTROL_REQUEST_DECODE_LIMITS,
        )?;
        self.validate_control_identity(
            &decoded.value.chain_id,
            decoded.value.genesis_block_hash,
            &decoded.value.publisher,
        )?;
        let digest = request_digest(
            MusubiPublicationRuntimeOperationV1::StorageCoordination,
            &decoded.canonical,
        )
        .map_err(|_| {
            MusubiPublicationServiceErrorV1::permanent(
                MusubiPublicationServiceErrorCodeV1::RequestInvalid,
            )
        })?;
        let authorization = self.decode_and_verify_authorization(
            &http,
            MusubiPublicationRuntimeOperationV1::StorageCoordination,
            decoded.value.operation_id,
            digest,
            &decoded.value.chain_id,
            &decoded.value.publisher,
            current_time_ms,
        )?;
        decoded.value.validate().map_err(|_| {
            MusubiPublicationServiceErrorV1::permanent(
                MusubiPublicationServiceErrorCodeV1::RequestInvalid,
            )
        })?;
        let receipt_binding = &decoded.value.staging_receipt.payload.binding;
        if receipt_binding.ingress_broker != self.config.ingress_broker
            || receipt_binding.seed_provider != self.config.seed_provider
        {
            return Err(MusubiPublicationServiceErrorV1::permanent(
                MusubiPublicationServiceErrorCodeV1::IdentityMismatch,
            ));
        }
        decoded
            .value
            .staging_receipt
            .verify(
                receipt_binding,
                decoded.value.staging_receipt.payload.issued_at_ms,
            )
            .map_err(|_| {
                MusubiPublicationServiceErrorV1::permanent(
                    MusubiPublicationServiceErrorCodeV1::RequestInvalid,
                )
                .ingest_deadletter(MusubiIngestDeadletterReasonV1::ReceiptInvalid)
            })?;
        // Registration admission already consumed receipt freshness. Storage coordination is
        // authorized by the finalized immutable registration projection and transaction identity,
        // so a crash after registration remains recoverable after the embedded receipt expires.
        if decoded.value.staging_receipt.payload.issued_at_ms
            > current_time_ms.saturating_add(self.config.max_future_clock_skew_ms)
        {
            return Err(MusubiPublicationServiceErrorV1::permanent(
                MusubiPublicationServiceErrorCodeV1::RequestInvalid,
            )
            .ingest_deadletter(MusubiIngestDeadletterReasonV1::ReceiptInvalid));
        }
        let attempt = journal_attempt(
            MusubiPublicationRuntimeOperationV1::StorageCoordination,
            decoded.value.operation_id,
            storage_generation_target(decoded.value.generation),
            &decoded.value.chain_id,
            decoded.value.genesis_block_hash,
            &decoded.value.publisher,
            decoded.value.commitment.archive_id(),
            decoded.value.commitment.car_digest,
            decoded.value.commitment.car_size,
            digest,
            &authorization,
        );
        match self.begin_attempt(&attempt, current_time_ms)? {
            MusubiPublicationJournalBeginV1::Cached(response) => {
                let cached: MusubiStorageCoordinationResponseV1 = decode_cached_response(&response)
                    .map_err(|error| error.integrity_failure(MusubiIntegritySurfaceV1::Other))?
                    .value;
                cached.validate_for(&decoded.value).map_err(|_| {
                    MusubiPublicationServiceErrorV1::permanent(
                        MusubiPublicationServiceErrorCodeV1::BackendResponseInvalid,
                    )
                    .integrity_failure(MusubiIntegritySurfaceV1::Other)
                })?;
                return Ok(response);
            }
            MusubiPublicationJournalBeginV1::Execute => {}
        }

        let result = self
            .storage
            .coordinate_storage(&decoded.value)
            .map_err(|error| {
                service_backend_error(
                    MusubiPublicationServiceErrorCodeV1::StorageCoordinationUnavailable,
                    error,
                )
            })
            .and_then(|response| {
                response.validate_for(&decoded.value).map_err(|_| {
                    MusubiPublicationServiceErrorV1::permanent(
                        MusubiPublicationServiceErrorCodeV1::BackendResponseInvalid,
                    )
                    .integrity_failure(MusubiIntegritySurfaceV1::Other)
                })?;
                encode_service_response(response)
            });
        self.finish_attempt(attempt.key, digest, result)
    }

    fn handle_provider_readback(
        &mut self,
        http: MusubiPublicationPrivateHttpRequestV1<'_>,
        current_time_ms: u64,
    ) -> Result<Vec<u8>, MusubiPublicationServiceErrorV1> {
        let decoded = decode_canonical_body::<MusubiProviderReadbackRequestV1>(
            http.body,
            MAX_CONTROL_REQUEST_BYTES,
            CONTROL_REQUEST_DECODE_LIMITS,
        )?;
        self.validate_control_identity(
            &decoded.value.chain_id,
            decoded.value.genesis_block_hash,
            &decoded.value.publisher,
        )?;
        let digest = request_digest(
            MusubiPublicationRuntimeOperationV1::ProviderReadback,
            &decoded.canonical,
        )
        .map_err(|_| {
            MusubiPublicationServiceErrorV1::permanent(
                MusubiPublicationServiceErrorCodeV1::RequestInvalid,
            )
        })?;
        let authorization = self.decode_and_verify_authorization(
            &http,
            MusubiPublicationRuntimeOperationV1::ProviderReadback,
            decoded.value.operation_id,
            digest,
            &decoded.value.chain_id,
            &decoded.value.publisher,
            current_time_ms,
        )?;
        decoded.value.validate().map_err(|_| {
            MusubiPublicationServiceErrorV1::permanent(
                MusubiPublicationServiceErrorCodeV1::RequestInvalid,
            )
        })?;
        let attempt = journal_attempt(
            MusubiPublicationRuntimeOperationV1::ProviderReadback,
            decoded.value.operation_id,
            *decoded.value.provider.as_bytes(),
            &decoded.value.chain_id,
            decoded.value.genesis_block_hash,
            &decoded.value.publisher,
            decoded.value.commitment.archive_id(),
            decoded.value.commitment.car_digest,
            decoded.value.commitment.car_size,
            digest,
            &authorization,
        );
        match self.begin_attempt(&attempt, current_time_ms)? {
            MusubiPublicationJournalBeginV1::Cached(response) => {
                let cached: MusubiProviderReadbackResponseV1 = decode_cached_response(&response)
                    .map_err(|error| {
                        error.integrity_failure(MusubiIntegritySurfaceV1::ProviderReadback)
                    })?
                    .value;
                cached.validate_for(&decoded.value).map_err(|_| {
                    MusubiPublicationServiceErrorV1::permanent(
                        MusubiPublicationServiceErrorCodeV1::BackendResponseInvalid,
                    )
                    .integrity_failure(MusubiIntegritySurfaceV1::ProviderReadback)
                })?;
                return Ok(response);
            }
            MusubiPublicationJournalBeginV1::Execute => {}
        }

        let result = self
            .readback
            .readback_provider(&decoded.value)
            .map_err(|error| {
                service_backend_error(
                    MusubiPublicationServiceErrorCodeV1::ProviderReadbackUnavailable,
                    error,
                )
            })
            .and_then(|response| {
                response.validate_for(&decoded.value).map_err(|_| {
                    MusubiPublicationServiceErrorV1::permanent(
                        MusubiPublicationServiceErrorCodeV1::BackendResponseInvalid,
                    )
                    .integrity_failure(MusubiIntegritySurfaceV1::ProviderReadback)
                })?;
                encode_service_response(response)
            });
        self.finish_attempt(attempt.key, digest, result)
    }

    fn validate_seed_identity(
        &self,
        request: &MusubiSeedIngressStageRequestV1,
    ) -> Result<(), MusubiPublicationServiceErrorV1> {
        let binding = &request.binding;
        self.validate_control_identity(
            &binding.chain_id,
            binding.genesis_block_hash,
            &binding.publisher,
        )?;
        if binding.ingress_broker != self.config.ingress_broker
            || binding.seed_provider != self.config.seed_provider
        {
            return Err(MusubiPublicationServiceErrorV1::permanent(
                MusubiPublicationServiceErrorCodeV1::IdentityMismatch,
            ));
        }
        Ok(())
    }

    fn validate_control_identity(
        &self,
        chain_id: &ChainId,
        genesis_block_hash: [u8; 32],
        _publisher: &AccountId,
    ) -> Result<(), MusubiPublicationServiceErrorV1> {
        if chain_id != &self.config.chain_id || genesis_block_hash != self.config.genesis_block_hash
        {
            return Err(MusubiPublicationServiceErrorV1::permanent(
                MusubiPublicationServiceErrorCodeV1::IdentityMismatch,
            ));
        }
        Ok(())
    }

    fn decode_and_verify_authorization(
        &self,
        http: &MusubiPublicationPrivateHttpRequestV1<'_>,
        operation: MusubiPublicationRuntimeOperationV1,
        operation_id: [u8; 32],
        digest: [u8; 32],
        chain_id: &ChainId,
        publisher: &AccountId,
        current_time_ms: u64,
    ) -> Result<
        CanonicalDecodedV1<MusubiPublicationRuntimeAuthorizationV1>,
        MusubiPublicationServiceErrorV1,
    > {
        let authorization: CanonicalDecodedV1<MusubiPublicationRuntimeAuthorizationV1> =
            decode_canonical_base64(
                http.authorization.ok_or_else(|| {
                    MusubiPublicationServiceErrorV1::permanent(
                        MusubiPublicationServiceErrorCodeV1::AuthorizationInvalid,
                    )
                })?,
                MAX_AUTHORIZATION_BYTES,
                AUTHORIZATION_DECODE_LIMITS,
                MusubiPublicationServiceErrorCodeV1::AuthorizationInvalid,
            )?;
        if &authorization.value.payload.chain_id != chain_id
            || &authorization.value.payload.publisher != publisher
            || chain_id != &self.config.chain_id
        {
            return Err(MusubiPublicationServiceErrorV1::permanent(
                MusubiPublicationServiceErrorCodeV1::IdentityMismatch,
            ));
        }
        authorization
            .value
            .verify_with_clock_skew(
                operation,
                operation_id,
                digest,
                current_time_ms,
                self.config.max_future_clock_skew_ms,
            )
            .map_err(|_| {
                let code = if current_time_ms > authorization.value.payload.expires_at_ms
                    || authorization.value.payload.issued_at_ms
                        > current_time_ms.saturating_add(self.config.max_future_clock_skew_ms)
                {
                    MusubiPublicationServiceErrorCodeV1::AuthorizationExpired
                } else {
                    MusubiPublicationServiceErrorCodeV1::AuthorizationInvalid
                };
                MusubiPublicationServiceErrorV1::permanent(code)
            })?;
        Ok(authorization)
    }

    fn begin_attempt(
        &mut self,
        attempt: &MusubiPublicationJournalAttemptV1,
        current_time_ms: u64,
    ) -> Result<MusubiPublicationJournalBeginV1, MusubiPublicationServiceErrorV1> {
        self.journal
            .begin(attempt, current_time_ms)
            .map_err(service_journal_error)
    }

    fn sample_time(&mut self) -> Result<u64, MusubiPublicationServiceErrorV1> {
        let current_time_ms = self.clock.current_time_ms().map_err(|error| {
            service_backend_error(
                MusubiPublicationServiceErrorCodeV1::TrustedClockUnavailable,
                error,
            )
        })?;
        if current_time_ms == 0
            || self
                .last_clock_ms
                .is_some_and(|previous| current_time_ms < previous)
        {
            return Err(MusubiPublicationServiceErrorV1::retryable(
                MusubiPublicationServiceErrorCodeV1::TrustedClockUnavailable,
            ));
        }
        self.last_clock_ms = Some(current_time_ms);
        Ok(current_time_ms)
    }

    fn finish_attempt(
        &mut self,
        key: MusubiPublicationIdempotencyKeyV1,
        request_digest: [u8; 32],
        result: Result<Vec<u8>, MusubiPublicationServiceErrorV1>,
    ) -> Result<Vec<u8>, MusubiPublicationServiceErrorV1> {
        let operation = key.operation;
        match result {
            Ok(response) => {
                if let Err(commit_error) = self.journal.commit(key, request_digest, &response) {
                    let error = self
                        .journal
                        .abort(key, request_digest)
                        .err()
                        .unwrap_or(commit_error);
                    return Err(publication_journal_error(operation, error));
                }
                Ok(response)
            }
            Err(error) => {
                if let Err(journal_error) = self.journal.abort(key, request_digest) {
                    // The original verification/deadletter observation must not disappear merely
                    // because the durable journal then failed to release its reservation.
                    record_publication_service_telemetry(error.telemetry);
                    return Err(if error.telemetry.is_some() {
                        service_journal_error(journal_error)
                    } else {
                        publication_journal_error(operation, journal_error)
                    });
                }
                Err(error)
            }
        }
    }
}

const AUTHORIZATION_DECODE_LIMITS: DecodeLimits =
    DecodeLimits::new(128, MAX_AUTHORIZATION_BYTES, 512, 256 * 1024, 16);
const REQUEST_METADATA_DECODE_LIMITS: DecodeLimits =
    DecodeLimits::new(256, MAX_SEED_INGRESS_METADATA_BYTES, 2_048, 512 * 1024, 32);
const CONTROL_REQUEST_DECODE_LIMITS: DecodeLimits = DecodeLimits::new(
    4_096,
    MAX_CONTROL_REQUEST_BYTES,
    32_768,
    32 * 1024 * 1024,
    64,
);

struct CanonicalDecodedV1<T> {
    value: T,
    canonical: Vec<u8>,
}

fn decode_canonical_base64<T>(
    encoded: &str,
    max_decoded_bytes: usize,
    limits: DecodeLimits,
    code: MusubiPublicationServiceErrorCodeV1,
) -> Result<CanonicalDecodedV1<T>, MusubiPublicationServiceErrorV1>
where
    T: norito::core::NoritoSerialize,
    for<'de> T: norito::core::NoritoDeserialize<'de>,
{
    let max_encoded_bytes = max_decoded_bytes
        .checked_mul(4)
        .and_then(|value| value.checked_add(2))
        .map(|value| value / 3)
        .ok_or_else(|| MusubiPublicationServiceErrorV1::permanent(code))?;
    if encoded.is_empty() || encoded.len() > max_encoded_bytes || !encoded.is_ascii() {
        return Err(MusubiPublicationServiceErrorV1::permanent(code));
    }
    let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(encoded)
        .map_err(|_| MusubiPublicationServiceErrorV1::permanent(code))?;
    if bytes.is_empty()
        || bytes.len() > max_decoded_bytes
        || base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(&bytes) != encoded
    {
        return Err(MusubiPublicationServiceErrorV1::permanent(code));
    }
    decode_canonical_bytes(&bytes, limits, code)
}

fn decode_canonical_body<T>(
    bytes: &[u8],
    max_bytes: usize,
    limits: DecodeLimits,
) -> Result<CanonicalDecodedV1<T>, MusubiPublicationServiceErrorV1>
where
    T: norito::core::NoritoSerialize,
    for<'de> T: norito::core::NoritoDeserialize<'de>,
{
    if bytes.is_empty() || bytes.len() > max_bytes {
        return Err(MusubiPublicationServiceErrorV1::permanent(
            MusubiPublicationServiceErrorCodeV1::RequestInvalid,
        ));
    }
    decode_canonical_bytes(
        bytes,
        limits,
        MusubiPublicationServiceErrorCodeV1::RequestInvalid,
    )
}

fn decode_cached_response<T>(
    bytes: &[u8],
) -> Result<CanonicalDecodedV1<T>, MusubiPublicationServiceErrorV1>
where
    T: norito::core::NoritoSerialize,
    for<'de> T: norito::core::NoritoDeserialize<'de>,
{
    if bytes.is_empty() || bytes.len() > MAX_CONTROL_RESPONSE_BYTES {
        return Err(MusubiPublicationServiceErrorV1::permanent(
            MusubiPublicationServiceErrorCodeV1::BackendResponseInvalid,
        ));
    }
    decode_canonical_bytes(
        bytes,
        RESPONSE_DECODE_LIMITS,
        MusubiPublicationServiceErrorCodeV1::BackendResponseInvalid,
    )
}

fn decode_canonical_bytes<T>(
    bytes: &[u8],
    limits: DecodeLimits,
    code: MusubiPublicationServiceErrorCodeV1,
) -> Result<CanonicalDecodedV1<T>, MusubiPublicationServiceErrorV1>
where
    T: norito::core::NoritoSerialize,
    for<'de> T: norito::core::NoritoDeserialize<'de>,
{
    let value = norito::decode_canonical_with_limits(bytes, limits)
        .map_err(|_| MusubiPublicationServiceErrorV1::permanent(code))?;
    let canonical = norito::encode_canonical(&value)
        .map_err(|_| MusubiPublicationServiceErrorV1::permanent(code))?;
    if canonical != bytes {
        return Err(MusubiPublicationServiceErrorV1::permanent(code));
    }
    Ok(CanonicalDecodedV1 { value, canonical })
}

fn journal_attempt(
    operation: MusubiPublicationRuntimeOperationV1,
    operation_id: [u8; 32],
    target: [u8; 32],
    chain_id: &ChainId,
    genesis_block_hash: [u8; 32],
    publisher: &AccountId,
    archive_id: ArchiveId,
    car_body_digest: MusubiContentDigestV1,
    car_body_length: u64,
    request_digest: [u8; 32],
    authorization: &CanonicalDecodedV1<MusubiPublicationRuntimeAuthorizationV1>,
) -> MusubiPublicationJournalAttemptV1 {
    MusubiPublicationJournalAttemptV1 {
        key: MusubiPublicationIdempotencyKeyV1 {
            operation,
            operation_id,
            target,
        },
        binding: MusubiPublicationOperationBindingV1 {
            operation_id,
            chain_id: chain_id.clone(),
            genesis_block_hash,
            publisher: publisher.clone(),
            archive_id,
            car_body_digest,
            car_body_length,
        },
        request_digest,
        authorization_digest: *blake3::hash(&authorization.canonical).as_bytes(),
        authorization_expires_at_ms: authorization.value.payload.expires_at_ms,
    }
}

fn encode_service_response<T>(value: T) -> Result<Vec<u8>, MusubiPublicationServiceErrorV1>
where
    T: norito::core::NoritoSerialize,
{
    let bytes = norito::encode_canonical(&value).map_err(|_| {
        MusubiPublicationServiceErrorV1::permanent(
            MusubiPublicationServiceErrorCodeV1::ResponseEncodingFailed,
        )
    })?;
    if bytes.is_empty() || bytes.len() > MAX_CONTROL_RESPONSE_BYTES {
        return Err(MusubiPublicationServiceErrorV1::permanent(
            MusubiPublicationServiceErrorCodeV1::ResponseEncodingFailed,
        ));
    }
    Ok(bytes)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum MusubiPublicationServiceTelemetryEventV1 {
    IngestDeadletter(MusubiIngestDeadletterReasonV1),
    IntegrityFailure(MusubiIntegritySurfaceV1),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct MusubiPublicationServiceErrorV1 {
    code: MusubiPublicationServiceErrorCodeV1,
    retryable: bool,
    telemetry: Option<MusubiPublicationServiceTelemetryEventV1>,
}

impl MusubiPublicationServiceErrorV1 {
    const fn permanent(code: MusubiPublicationServiceErrorCodeV1) -> Self {
        Self {
            code,
            retryable: false,
            telemetry: None,
        }
    }

    const fn retryable(code: MusubiPublicationServiceErrorCodeV1) -> Self {
        Self {
            code,
            retryable: true,
            telemetry: None,
        }
    }

    const fn ingest_deadletter(mut self, reason: MusubiIngestDeadletterReasonV1) -> Self {
        debug_assert!(!self.retryable);
        self.telemetry = Some(MusubiPublicationServiceTelemetryEventV1::IngestDeadletter(
            reason,
        ));
        self
    }

    const fn integrity_failure(mut self, surface: MusubiIntegritySurfaceV1) -> Self {
        self.telemetry = Some(MusubiPublicationServiceTelemetryEventV1::IntegrityFailure(
            surface,
        ));
        self
    }

    fn into_response(self) -> MusubiPublicationPrivateHttpResponseV1 {
        record_publication_service_telemetry(self.telemetry);
        let status = match self.code {
            MusubiPublicationServiceErrorCodeV1::RouteNotFound => 404,
            MusubiPublicationServiceErrorCodeV1::MethodInvalid => 405,
            MusubiPublicationServiceErrorCodeV1::MediaTypeInvalid => 415,
            MusubiPublicationServiceErrorCodeV1::AuthorizationInvalid
            | MusubiPublicationServiceErrorCodeV1::AuthorizationExpired
            | MusubiPublicationServiceErrorCodeV1::AuthorizationReplay => 401,
            MusubiPublicationServiceErrorCodeV1::OperationBusy => 425,
            MusubiPublicationServiceErrorCodeV1::TrustedClockUnavailable => 503,
            MusubiPublicationServiceErrorCodeV1::JournalUnavailable
            | MusubiPublicationServiceErrorCodeV1::SeedIngressUnavailable
            | MusubiPublicationServiceErrorCodeV1::StorageCoordinationUnavailable
            | MusubiPublicationServiceErrorCodeV1::ProviderReadbackUnavailable
            | MusubiPublicationServiceErrorCodeV1::ReceiptSigningUnavailable
                if self.retryable =>
            {
                503
            }
            _ => 422,
        };
        let error = MusubiPublicationServiceErrorResponseV1 {
            version: 1,
            code: self.code,
            retryable: self.retryable,
        };
        let body = norito::encode_canonical(&error)
            .expect("fixed bounded Musubi publication error response must encode");
        MusubiPublicationPrivateHttpResponseV1 {
            status,
            content_type: APPLICATION_NORITO,
            body,
        }
    }
}

fn record_publication_service_telemetry(event: Option<MusubiPublicationServiceTelemetryEventV1>) {
    let (Some(metrics), Some(event)) = (global_metrics(), event) else {
        return;
    };
    match event {
        MusubiPublicationServiceTelemetryEventV1::IngestDeadletter(reason) => {
            metrics.musubi.inc_ingest_deadletter(reason);
        }
        MusubiPublicationServiceTelemetryEventV1::IntegrityFailure(surface) => {
            metrics.musubi.inc_integrity_failure(surface);
        }
    }
}

fn service_backend_error(
    code: MusubiPublicationServiceErrorCodeV1,
    error: MusubiPublicationServiceBackendErrorV1,
) -> MusubiPublicationServiceErrorV1 {
    match error {
        MusubiPublicationServiceBackendErrorV1::Retryable => {
            MusubiPublicationServiceErrorV1::retryable(code)
        }
        MusubiPublicationServiceBackendErrorV1::Permanent => {
            MusubiPublicationServiceErrorV1::permanent(code)
        }
    }
}

fn seed_ingress_backend_error(
    error: MusubiPublicationServiceBackendErrorV1,
) -> MusubiPublicationServiceErrorV1 {
    let service_error = service_backend_error(
        MusubiPublicationServiceErrorCodeV1::SeedIngressUnavailable,
        error,
    );
    if error == MusubiPublicationServiceBackendErrorV1::Permanent {
        service_error.ingest_deadletter(MusubiIngestDeadletterReasonV1::StorageRejected)
    } else {
        service_error
    }
}

fn service_journal_error(
    error: MusubiPublicationServiceJournalErrorV1,
) -> MusubiPublicationServiceErrorV1 {
    match error {
        MusubiPublicationServiceJournalErrorV1::Invalid
        | MusubiPublicationServiceJournalErrorV1::Conflict => {
            MusubiPublicationServiceErrorV1::permanent(
                MusubiPublicationServiceErrorCodeV1::OperationConflict,
            )
        }
        MusubiPublicationServiceJournalErrorV1::Replay => {
            MusubiPublicationServiceErrorV1::permanent(
                MusubiPublicationServiceErrorCodeV1::AuthorizationReplay,
            )
        }
        MusubiPublicationServiceJournalErrorV1::Busy => MusubiPublicationServiceErrorV1::retryable(
            MusubiPublicationServiceErrorCodeV1::OperationBusy,
        ),
        MusubiPublicationServiceJournalErrorV1::Capacity
        | MusubiPublicationServiceJournalErrorV1::Unavailable => {
            MusubiPublicationServiceErrorV1::retryable(
                MusubiPublicationServiceErrorCodeV1::JournalUnavailable,
            )
        }
    }
}

fn seed_ingress_journal_error(
    error: MusubiPublicationServiceJournalErrorV1,
) -> MusubiPublicationServiceErrorV1 {
    publication_journal_error(MusubiPublicationRuntimeOperationV1::SeedIngress, error)
}

fn publication_journal_error(
    operation: MusubiPublicationRuntimeOperationV1,
    error: MusubiPublicationServiceJournalErrorV1,
) -> MusubiPublicationServiceErrorV1 {
    let service_error = service_journal_error(error);
    if operation == MusubiPublicationRuntimeOperationV1::SeedIngress
        && error == MusubiPublicationServiceJournalErrorV1::Conflict
    {
        service_error.ingest_deadletter(MusubiIngestDeadletterReasonV1::ReceiptReplay)
    } else {
        service_error
    }
}

/// Closed failure returned by a deployment-owned publisher authorization signer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MusubiPublicationRuntimeAuthorizationSigningErrorV1 {
    /// The same exact payload may be signed after a transient provider outage.
    Retryable,
    /// Deployment policy or key state cannot authorize the exact payload.
    Permanent,
}

/// Deployment-owned controller signing boundary for private publication requests.
///
/// Implementations may collect approvals from an HSM, KMS, or threshold service. The client
/// constructs every payload field and accepts only approvals, then independently verifies the
/// assembled authorization before any network request is built.
pub trait MusubiPublicationRuntimeAuthorizationSigningProviderV1: Send + Sync {
    /// Exact publisher account controlled by this provider.
    fn publisher(&self) -> &AccountId;

    /// Sign one exact client-constructed authorization payload.
    ///
    /// # Errors
    /// Returns a closed retryable or permanent failure without provider diagnostics or secrets.
    fn sign_approvals(
        &self,
        payload: &MusubiPublicationRuntimeAuthorizationPayloadV1,
    ) -> Result<
        Vec<MusubiPublicationRuntimeAuthorizationApprovalV1>,
        MusubiPublicationRuntimeAuthorizationSigningErrorV1,
    >;
}

/// Runtime-only single-key authorization signer used by the platform Iroha client.
///
/// Multisig publishers use a deployment-owned threshold implementation of
/// [`MusubiPublicationRuntimeAuthorizationSigningProviderV1`] instead.
#[derive(Clone)]
pub struct SoftwareMusubiPublicationRuntimeAuthorizationSignerV1 {
    publisher: AccountId,
    key_pair: KeyPair,
}

impl fmt::Debug for SoftwareMusubiPublicationRuntimeAuthorizationSignerV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SoftwareMusubiPublicationRuntimeAuthorizationSignerV1")
            .field("publisher", &self.publisher)
            .finish_non_exhaustive()
    }
}

impl SoftwareMusubiPublicationRuntimeAuthorizationSignerV1 {
    /// Bind one software key to the exact single-controller publisher.
    ///
    /// # Errors
    /// Returns a stable identity error for multisig accounts or mismatched keys.
    pub fn new(
        publisher: AccountId,
        key_pair: KeyPair,
    ) -> Result<Self, MusubiPublicationRuntimeTransportErrorV1> {
        let AccountController::Single(expected_key) = publisher.controller() else {
            return Err(MusubiPublicationRuntimeTransportErrorV1::permanent(
                "MUSUBI_RUNTIME_MULTISIG_AUTH_PROVIDER_REQUIRED",
            ));
        };
        if expected_key != key_pair.public_key() {
            return Err(MusubiPublicationRuntimeTransportErrorV1::permanent(
                "MUSUBI_RUNTIME_SIGNER_MISMATCH",
            ));
        }
        Ok(Self {
            publisher,
            key_pair,
        })
    }
}

impl MusubiPublicationRuntimeAuthorizationSigningProviderV1
    for SoftwareMusubiPublicationRuntimeAuthorizationSignerV1
{
    fn publisher(&self) -> &AccountId {
        &self.publisher
    }

    fn sign_approvals(
        &self,
        payload: &MusubiPublicationRuntimeAuthorizationPayloadV1,
    ) -> Result<
        Vec<MusubiPublicationRuntimeAuthorizationApprovalV1>,
        MusubiPublicationRuntimeAuthorizationSigningErrorV1,
    > {
        if payload.publisher != self.publisher {
            return Err(MusubiPublicationRuntimeAuthorizationSigningErrorV1::Permanent);
        }
        let signature = SignatureOf::try_new(self.key_pair.private_key(), payload)
            .map_err(|_| MusubiPublicationRuntimeAuthorizationSigningErrorV1::Permanent)?;
        Ok(vec![MusubiPublicationRuntimeAuthorizationApprovalV1 {
            public_key: self.key_pair.public_key().clone(),
            signature,
        }])
    }
}

/// Account-authenticated client for the fixed private publication route inventory.
#[derive(Clone)]
pub struct AuthenticatedMusubiPublicationRuntimeClientV1 {
    chain_id: ChainId,
    publisher: AccountId,
    authorization_signer: Arc<dyn MusubiPublicationRuntimeAuthorizationSigningProviderV1>,
    http: HttpClient,
}

impl fmt::Debug for AuthenticatedMusubiPublicationRuntimeClientV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AuthenticatedMusubiPublicationRuntimeClientV1")
            .field("chain_id", &self.chain_id)
            .field("publisher", &self.publisher)
            .finish_non_exhaustive()
    }
}

impl AuthenticatedMusubiPublicationRuntimeClientV1 {
    /// Construct from an already validated platform Iroha client configuration.
    ///
    /// Torii headers and Basic Auth are deliberately not copied to the private service.
    pub fn from_iroha_client(
        client: &Client,
        timeout: Duration,
    ) -> Result<Self, MusubiPublicationRuntimeTransportErrorV1> {
        let signer = SoftwareMusubiPublicationRuntimeAuthorizationSignerV1::new(
            client.account.clone(),
            client.key_pair.clone(),
        )?;
        Self::from_authorization_signer(
            client.chain.clone(),
            client.account.clone(),
            Arc::new(signer),
            timeout,
        )
    }

    /// Construct with a deployment-owned HSM/KMS or threshold authorization provider.
    ///
    /// # Errors
    /// Returns a stable error when the provider identity or timeout is invalid.
    pub fn from_authorization_signer(
        chain_id: ChainId,
        publisher: AccountId,
        authorization_signer: Arc<dyn MusubiPublicationRuntimeAuthorizationSigningProviderV1>,
        timeout: Duration,
    ) -> Result<Self, MusubiPublicationRuntimeTransportErrorV1> {
        if timeout == Duration::ZERO || timeout > Duration::from_secs(60) {
            return Err(MusubiPublicationRuntimeTransportErrorV1::permanent(
                "MUSUBI_RUNTIME_TIMEOUT_INVALID",
            ));
        }
        if authorization_signer.publisher() != &publisher {
            return Err(MusubiPublicationRuntimeTransportErrorV1::permanent(
                "MUSUBI_RUNTIME_SIGNER_MISMATCH",
            ));
        }
        if !controller_fits_publication_approval_bound(&publisher) {
            return Err(MusubiPublicationRuntimeTransportErrorV1::permanent(
                "MUSUBI_RUNTIME_AUTHORIZATION_CONTROLLER_UNSUPPORTED",
            ));
        }
        let http = HttpClient::builder()
            .https_only(true)
            .no_proxy()
            .redirect(RedirectPolicy::none())
            .connect_timeout(timeout)
            .timeout(timeout)
            .build()
            .map_err(|_| {
                MusubiPublicationRuntimeTransportErrorV1::permanent(
                    "MUSUBI_RUNTIME_HTTP_CLIENT_INVALID",
                )
            })?;
        Ok(Self {
            chain_id,
            publisher,
            authorization_signer,
            http,
        })
    }

    /// Return the exact configured chain identity.
    #[must_use]
    pub const fn chain_id(&self) -> &ChainId {
        &self.chain_id
    }

    /// Return the exact configured publisher identity.
    #[must_use]
    pub const fn publisher(&self) -> &AccountId {
        &self.publisher
    }

    /// Send one bounded CAR through the fixed authenticated seed-ingress route.
    pub fn stage_seed_ingress(
        &self,
        base_url: &Url,
        request: &MusubiSeedIngressStageRequestV1,
        car: &mut dyn Read,
        current_time_ms: u64,
    ) -> Result<MusubiSeedIngressReceiptV1, MusubiPublicationRuntimeTransportErrorV1> {
        request.validate()?;
        self.ensure_request_identity(&request.binding.chain_id, &request.binding.publisher)?;
        validate_publication_service_base_url(base_url)?;
        let car_length = usize::try_from(request.binding.car_body_length).map_err(|_| {
            MusubiPublicationRuntimeTransportErrorV1::permanent("MUSUBI_SEED_INGRESS_BODY_INVALID")
        })?;
        if car_length == 0 || request.binding.car_body_length > MUSUBI_MAX_CAR_BYTES_V1 {
            return Err(MusubiPublicationRuntimeTransportErrorV1::permanent(
                "MUSUBI_SEED_INGRESS_BODY_INVALID",
            ));
        }
        let mut bytes = Vec::with_capacity(car_length);
        car.take(request.binding.car_body_length.saturating_add(1))
            .read_to_end(&mut bytes)
            .map_err(|_| {
                MusubiPublicationRuntimeTransportErrorV1::retryable(
                    "MUSUBI_SEED_INGRESS_BODY_READ_FAILED",
                )
            })?;
        if bytes.len() != car_length
            || blake3::hash(&bytes).as_bytes() != request.binding.car_body_digest.as_bytes()
        {
            return Err(MusubiPublicationRuntimeTransportErrorV1::permanent(
                "MUSUBI_SEED_INGRESS_BODY_INVALID",
            ));
        }
        let request_bytes = norito::encode_canonical(request).map_err(|_| {
            MusubiPublicationRuntimeTransportErrorV1::permanent(
                "MUSUBI_SEED_INGRESS_REQUEST_INVALID",
            )
        })?;
        if request_bytes.is_empty() || request_bytes.len() > MAX_SEED_INGRESS_METADATA_BYTES {
            return Err(MusubiPublicationRuntimeTransportErrorV1::permanent(
                "MUSUBI_SEED_INGRESS_METADATA_TOO_LARGE",
            ));
        }
        let authorization = self.authorization(
            MusubiPublicationRuntimeOperationV1::SeedIngress,
            request.operation_id,
            request_digest(
                MusubiPublicationRuntimeOperationV1::SeedIngress,
                &request_bytes,
            )?,
            current_time_ms,
        )?;
        let endpoint = publication_route(base_url, SEED_INGRESS_ROUTE)?;
        let response = self.send(
            endpoint,
            APPLICATION_SORAFS_CAR,
            &authorization,
            Some(&request_bytes),
            bytes,
        )?;
        let receipt: MusubiSeedIngressReceiptV1 = decode_response(&response)?;
        let verification_time_ms = system_time_ms()?;
        receipt
            .verify(&request.binding, verification_time_ms)
            .map_err(|_| {
                MusubiPublicationRuntimeTransportErrorV1::permanent(
                    "MUSUBI_SEED_INGRESS_RECEIPT_INVALID",
                )
            })?;
        Ok(receipt)
    }

    /// Request an idempotent permanent pin/order coordination result.
    pub fn coordinate_storage(
        &self,
        base_url: &Url,
        request: &MusubiStorageCoordinationRequestV1,
        current_time_ms: u64,
    ) -> Result<MusubiStorageCoordinationResponseV1, MusubiPublicationRuntimeTransportErrorV1> {
        request.validate()?;
        self.ensure_request_identity(&request.chain_id, &request.publisher)?;
        let request_bytes = norito::encode_canonical(request).map_err(|_| {
            MusubiPublicationRuntimeTransportErrorV1::permanent(
                "MUSUBI_STORAGE_COORDINATION_REQUEST_INVALID",
            )
        })?;
        let response = self.post_control(
            base_url,
            STORAGE_COORDINATION_ROUTE,
            MusubiPublicationRuntimeOperationV1::StorageCoordination,
            request.operation_id,
            &request_bytes,
            current_time_ms,
        )?;
        let response: MusubiStorageCoordinationResponseV1 = decode_response(&response)?;
        response.validate_for(request)?;
        Ok(response)
    }

    /// Read back one complete archive from one exact provider-specific HTTPS origin.
    pub fn readback_provider(
        &self,
        base_url: &Url,
        request: &MusubiProviderReadbackRequestV1,
        current_time_ms: u64,
    ) -> Result<MusubiProviderReadbackResponseV1, MusubiPublicationRuntimeTransportErrorV1> {
        request.validate()?;
        self.ensure_request_identity(&request.chain_id, &request.publisher)?;
        let request_bytes = norito::encode_canonical(request).map_err(|_| {
            MusubiPublicationRuntimeTransportErrorV1::permanent(
                "MUSUBI_PROVIDER_READBACK_REQUEST_INVALID",
            )
        })?;
        let response = self.post_control(
            base_url,
            PROVIDER_READBACK_ROUTE,
            MusubiPublicationRuntimeOperationV1::ProviderReadback,
            request.operation_id,
            &request_bytes,
            current_time_ms,
        )?;
        let response: MusubiProviderReadbackResponseV1 = decode_response(&response)?;
        response.validate_for(request)?;
        Ok(response)
    }

    fn post_control(
        &self,
        base_url: &Url,
        route: &str,
        operation: MusubiPublicationRuntimeOperationV1,
        operation_id: [u8; 32],
        body: &[u8],
        current_time_ms: u64,
    ) -> Result<Vec<u8>, MusubiPublicationRuntimeTransportErrorV1> {
        validate_publication_service_base_url(base_url)?;
        if body.is_empty() || body.len() > MAX_CONTROL_REQUEST_BYTES {
            return Err(MusubiPublicationRuntimeTransportErrorV1::permanent(
                "MUSUBI_RUNTIME_REQUEST_TOO_LARGE",
            ));
        }
        let authorization = self.authorization(
            operation,
            operation_id,
            request_digest(operation, body)?,
            current_time_ms,
        )?;
        let endpoint = publication_route(base_url, route)?;
        self.send(
            endpoint,
            APPLICATION_NORITO,
            &authorization,
            None,
            body.to_vec(),
        )
    }

    fn authorization(
        &self,
        operation: MusubiPublicationRuntimeOperationV1,
        operation_id: [u8; 32],
        digest: [u8; 32],
        current_time_ms: u64,
    ) -> Result<MusubiPublicationRuntimeAuthorizationV1, MusubiPublicationRuntimeTransportErrorV1>
    {
        let payload = MusubiPublicationRuntimeAuthorizationPayloadV1 {
            domain: AUTH_DOMAIN_V1,
            version: 1,
            operation,
            operation_id,
            chain_id: self.chain_id.clone(),
            publisher: self.publisher.clone(),
            request_digest: digest,
            issued_at_ms: current_time_ms,
            expires_at_ms: current_time_ms
                .checked_add(DEFAULT_AUTHORIZATION_LIFETIME_MS)
                .ok_or_else(|| {
                    MusubiPublicationRuntimeTransportErrorV1::permanent(
                        "MUSUBI_RUNTIME_CLOCK_INVALID",
                    )
                })?,
        };
        payload.validate()?;
        let approvals = self
            .authorization_signer
            .sign_approvals(&payload)
            .map_err(|error| match error {
                MusubiPublicationRuntimeAuthorizationSigningErrorV1::Retryable => {
                    MusubiPublicationRuntimeTransportErrorV1::retryable(
                        "MUSUBI_RUNTIME_AUTHORIZATION_SIGNER_UNAVAILABLE",
                    )
                }
                MusubiPublicationRuntimeAuthorizationSigningErrorV1::Permanent => {
                    MusubiPublicationRuntimeTransportErrorV1::permanent(
                        "MUSUBI_RUNTIME_AUTHORIZATION_SIGNING_FAILED",
                    )
                }
            })?;
        let authorization = MusubiPublicationRuntimeAuthorizationV1 { payload, approvals };
        authorization.verify(operation, operation_id, digest, current_time_ms)?;
        Ok(authorization)
    }

    fn ensure_request_identity(
        &self,
        chain_id: &ChainId,
        publisher: &AccountId,
    ) -> Result<(), MusubiPublicationRuntimeTransportErrorV1> {
        if chain_id != &self.chain_id || publisher != &self.publisher {
            return Err(MusubiPublicationRuntimeTransportErrorV1::permanent(
                "MUSUBI_RUNTIME_REQUEST_IDENTITY_MISMATCH",
            ));
        }
        Ok(())
    }

    fn send(
        &self,
        endpoint: Url,
        content_type: &'static str,
        authorization: &MusubiPublicationRuntimeAuthorizationV1,
        seed_ingress_metadata: Option<&[u8]>,
        body: Vec<u8>,
    ) -> Result<Vec<u8>, MusubiPublicationRuntimeTransportErrorV1> {
        let request = self.prepare_request(
            endpoint,
            content_type,
            authorization,
            seed_ingress_metadata,
            body,
        )?;
        let mut response = self.http.execute(request).map_err(|_| {
            MusubiPublicationRuntimeTransportErrorV1::retryable("MUSUBI_RUNTIME_TRANSPORT_FAILED")
        })?;
        let status = response.status();
        if !status.is_success() {
            return Err(if retryable_status(status) {
                MusubiPublicationRuntimeTransportErrorV1::retryable(
                    "MUSUBI_RUNTIME_REMOTE_RETRYABLE",
                )
            } else {
                MusubiPublicationRuntimeTransportErrorV1::permanent(
                    "MUSUBI_RUNTIME_REMOTE_REJECTED",
                )
            });
        }
        if response
            .content_length()
            .is_some_and(|length| length > MAX_CONTROL_RESPONSE_BYTES_U64)
        {
            return Err(MusubiPublicationRuntimeTransportErrorV1::permanent(
                "MUSUBI_RUNTIME_RESPONSE_TOO_LARGE",
            ));
        }
        let mut bytes = Vec::new();
        response
            .by_ref()
            .take(MAX_CONTROL_RESPONSE_BYTES_U64 + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| {
                MusubiPublicationRuntimeTransportErrorV1::retryable(
                    "MUSUBI_RUNTIME_RESPONSE_READ_FAILED",
                )
            })?;
        if bytes.is_empty() || bytes.len() > MAX_CONTROL_RESPONSE_BYTES {
            return Err(MusubiPublicationRuntimeTransportErrorV1::permanent(
                "MUSUBI_RUNTIME_RESPONSE_TOO_LARGE",
            ));
        }
        Ok(bytes)
    }

    fn prepare_request(
        &self,
        endpoint: Url,
        content_type: &'static str,
        authorization: &MusubiPublicationRuntimeAuthorizationV1,
        seed_ingress_metadata: Option<&[u8]>,
        body: Vec<u8>,
    ) -> Result<reqwest::blocking::Request, MusubiPublicationRuntimeTransportErrorV1> {
        let authorization = norito::encode_canonical(authorization).map_err(|_| {
            MusubiPublicationRuntimeTransportErrorV1::permanent(
                "MUSUBI_RUNTIME_AUTHORIZATION_INVALID",
            )
        })?;
        if authorization.is_empty() || authorization.len() > MAX_AUTHORIZATION_BYTES {
            return Err(MusubiPublicationRuntimeTransportErrorV1::permanent(
                "MUSUBI_RUNTIME_AUTHORIZATION_TOO_LARGE",
            ));
        }
        let authorization = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(authorization);
        let mut authorization = HeaderValue::try_from(authorization).map_err(|_| {
            MusubiPublicationRuntimeTransportErrorV1::permanent(
                "MUSUBI_RUNTIME_AUTHORIZATION_INVALID",
            )
        })?;
        authorization.set_sensitive(true);
        let mut request = self
            .http
            .post(endpoint)
            .header("Content-Type", content_type)
            .header("Accept", APPLICATION_NORITO)
            .header(AUTHORIZATION_HEADER, authorization);
        if let Some(metadata) = seed_ingress_metadata {
            if metadata.is_empty() || metadata.len() > MAX_SEED_INGRESS_METADATA_BYTES {
                return Err(MusubiPublicationRuntimeTransportErrorV1::permanent(
                    "MUSUBI_SEED_INGRESS_METADATA_TOO_LARGE",
                ));
            }
            let metadata = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(metadata);
            let mut metadata = HeaderValue::try_from(metadata).map_err(|_| {
                MusubiPublicationRuntimeTransportErrorV1::permanent(
                    "MUSUBI_SEED_INGRESS_METADATA_TOO_LARGE",
                )
            })?;
            metadata.set_sensitive(true);
            request = request.header(SEED_INGRESS_METADATA_HEADER, metadata);
        }
        request.body(body).build().map_err(|_| {
            MusubiPublicationRuntimeTransportErrorV1::permanent("MUSUBI_RUNTIME_REQUEST_INVALID")
        })
    }
}

/// Validate a credential-free HTTPS base used only by the private publication routes.
pub fn validate_publication_service_base_url(
    url: &Url,
) -> Result<(), MusubiPublicationRuntimeTransportErrorV1> {
    if url.scheme() != "https"
        || !url.username().is_empty()
        || url.password().is_some()
        || url.host_str().is_none()
        || url.port() == Some(0)
        || url.query().is_some()
        || url.fragment().is_some()
        || !url.path().ends_with('/')
        || url.path().contains("/v1/sorafs/upload")
    {
        return Err(MusubiPublicationRuntimeTransportErrorV1::permanent(
            "MUSUBI_RUNTIME_SERVICE_URL_INVALID",
        ));
    }
    Ok(())
}

/// Return the scheme/host/port identity used to enforce distinct provider origins.
#[must_use]
pub fn publication_service_origin(url: &Url) -> Option<(String, String, u16)> {
    let host = url.host_str()?.to_ascii_lowercase();
    let port = url.port_or_known_default()?;
    Some((url.scheme().to_owned(), host, port))
}

fn publication_route(
    base_url: &Url,
    route: &str,
) -> Result<Url, MusubiPublicationRuntimeTransportErrorV1> {
    base_url.join(route).map_err(|_| {
        MusubiPublicationRuntimeTransportErrorV1::permanent("MUSUBI_RUNTIME_SERVICE_URL_INVALID")
    })
}

fn system_time_ms() -> Result<u64, MusubiPublicationRuntimeTransportErrorV1> {
    let elapsed = SystemTime::now().duration_since(UNIX_EPOCH).map_err(|_| {
        MusubiPublicationRuntimeTransportErrorV1::permanent("MUSUBI_RUNTIME_CLOCK_INVALID")
    })?;
    u64::try_from(elapsed.as_millis()).map_err(|_| {
        MusubiPublicationRuntimeTransportErrorV1::permanent("MUSUBI_RUNTIME_CLOCK_INVALID")
    })
}

fn request_digest(
    operation: MusubiPublicationRuntimeOperationV1,
    body: &[u8],
) -> Result<[u8; 32], MusubiPublicationRuntimeTransportErrorV1> {
    let body_length = u64::try_from(body.len()).map_err(|_| {
        MusubiPublicationRuntimeTransportErrorV1::permanent("MUSUBI_RUNTIME_REQUEST_TOO_LARGE")
    })?;
    let operation = norito::encode_canonical(&operation).map_err(|_| {
        MusubiPublicationRuntimeTransportErrorV1::permanent("MUSUBI_RUNTIME_REQUEST_INVALID")
    })?;
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"iroha-musubi-publication-runtime-request-v1\0");
    hasher.update(&operation);
    hasher.update(&body_length.to_be_bytes());
    hasher.update(body);
    Ok(*hasher.finalize().as_bytes())
}

fn decode_response<T>(bytes: &[u8]) -> Result<T, MusubiPublicationRuntimeTransportErrorV1>
where
    T: norito::core::NoritoSerialize,
    for<'de> T: norito::core::NoritoDeserialize<'de>,
{
    norito::decode_canonical_with_limits(bytes, RESPONSE_DECODE_LIMITS).map_err(|_| {
        MusubiPublicationRuntimeTransportErrorV1::permanent("MUSUBI_RUNTIME_RESPONSE_INVALID")
    })
}

fn retryable_status(status: StatusCode) -> bool {
    matches!(
        status,
        StatusCode::REQUEST_TIMEOUT
            | StatusCode::TOO_EARLY
            | StatusCode::TOO_MANY_REQUESTS
            | StatusCode::INTERNAL_SERVER_ERROR
            | StatusCode::BAD_GATEWAY
            | StatusCode::SERVICE_UNAVAILABLE
            | StatusCode::GATEWAY_TIMEOUT
    )
}

#[cfg(test)]
mod tests {
    use std::sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
    };

    use super::*;
    use iroha_crypto::{Algorithm, HashOf};
    use iroha_data_model::{
        account::{MultisigMember, MultisigPolicy},
        musubi::{
            ArchiveId, MusubiContentDigestV1, MusubiProviderBundleVerificationApprovalV1,
            MusubiProviderBundleVerificationBindingV1, MusubiProviderBundleVerificationPayloadV1,
        },
        sorafs::pin_registry::{
            ChunkerProfileHandle, ManifestRootCid, ProviderIngestCompletionAuthorityV1,
            ProviderIngestCompletionSignerPolicyV1, ProviderIngestFinalizedAnchorV1,
        },
    };

    #[derive(Clone)]
    struct TestPublicationClock {
        current_time_ms: Arc<AtomicU64>,
    }

    impl TestPublicationClock {
        fn new(initial_time_ms: u64) -> (Self, Arc<AtomicU64>) {
            let current_time_ms = Arc::new(AtomicU64::new(initial_time_ms));
            (
                Self {
                    current_time_ms: Arc::clone(&current_time_ms),
                },
                current_time_ms,
            )
        }
    }

    impl MusubiPublicationServiceClockV1 for TestPublicationClock {
        fn current_time_ms(&mut self) -> Result<u64, MusubiPublicationServiceBackendErrorV1> {
            Ok(self.current_time_ms.load(Ordering::SeqCst))
        }
    }

    #[test]
    fn publication_service_telemetry_annotations_are_closed_and_terminal() {
        let conflict = seed_ingress_journal_error(MusubiPublicationServiceJournalErrorV1::Conflict);
        assert_eq!(
            conflict.telemetry,
            Some(MusubiPublicationServiceTelemetryEventV1::IngestDeadletter(
                MusubiIngestDeadletterReasonV1::ReceiptReplay,
            ))
        );

        let authorization_replay =
            seed_ingress_journal_error(MusubiPublicationServiceJournalErrorV1::Replay);
        assert_eq!(authorization_replay.telemetry, None);

        let retryable_storage =
            seed_ingress_backend_error(MusubiPublicationServiceBackendErrorV1::Retryable);
        assert!(retryable_storage.retryable);
        assert_eq!(retryable_storage.telemetry, None);

        let terminal_storage =
            seed_ingress_backend_error(MusubiPublicationServiceBackendErrorV1::Permanent);
        assert!(!terminal_storage.retryable);
        assert_eq!(
            terminal_storage.telemetry,
            Some(MusubiPublicationServiceTelemetryEventV1::IngestDeadletter(
                MusubiIngestDeadletterReasonV1::StorageRejected,
            ))
        );

        let readback = MusubiPublicationServiceErrorV1::permanent(
            MusubiPublicationServiceErrorCodeV1::BackendResponseInvalid,
        )
        .integrity_failure(MusubiIntegritySurfaceV1::ProviderReadback);
        assert_eq!(
            readback.telemetry,
            Some(MusubiPublicationServiceTelemetryEventV1::IntegrityFailure(
                MusubiIntegritySurfaceV1::ProviderReadback,
            ))
        );
    }

    #[derive(Clone)]
    struct RecordingSeedIngress {
        provider: ProviderId,
        calls: Arc<Mutex<usize>>,
        fail_first: bool,
        clock_after_stage: Option<(Arc<AtomicU64>, u64)>,
    }

    impl MusubiSeedIngressBackendV1 for RecordingSeedIngress {
        fn provider_id(&self) -> ProviderId {
            self.provider
        }

        fn stage_exact_car(
            &mut self,
            _operation_id: [u8; 32],
            _binding: &MusubiSeedIngressReceiptBindingV1,
            _car: &[u8],
        ) -> Result<(), MusubiPublicationServiceBackendErrorV1> {
            let mut calls = self.calls.lock().expect("seed call counter");
            *calls += 1;
            if self.fail_first && *calls == 1 {
                Err(MusubiPublicationServiceBackendErrorV1::Retryable)
            } else {
                if let Some((clock, current_time_ms)) = &self.clock_after_stage {
                    clock.store(*current_time_ms, Ordering::SeqCst);
                }
                Ok(())
            }
        }
    }

    #[derive(Clone, Copy)]
    enum TestSigningBehavior {
        Correct,
        WrongPayload,
        WrongController,
        DuplicateApproval,
        RetryableFailure,
    }

    struct TestReceiptSigningProvider {
        broker: AccountId,
        key_pair: KeyPair,
        behavior: TestSigningBehavior,
        observed_payloads: Arc<Mutex<Vec<MusubiSeedIngressReceiptPayloadV1>>>,
        clock_after_signing: Option<(Arc<AtomicU64>, u64)>,
    }

    impl TestReceiptSigningProvider {
        fn new(
            broker: AccountId,
            key_pair: KeyPair,
            behavior: TestSigningBehavior,
        ) -> (Self, Arc<Mutex<Vec<MusubiSeedIngressReceiptPayloadV1>>>) {
            let observed_payloads = Arc::new(Mutex::new(Vec::new()));
            (
                Self {
                    broker,
                    key_pair,
                    behavior,
                    observed_payloads: Arc::clone(&observed_payloads),
                    clock_after_signing: None,
                },
                observed_payloads,
            )
        }

        fn with_clock_after_signing(mut self, clock: Arc<AtomicU64>, current_time_ms: u64) -> Self {
            self.clock_after_signing = Some((clock, current_time_ms));
            self
        }
    }

    impl MusubiSeedIngressReceiptSigningProviderV1 for TestReceiptSigningProvider {
        fn broker(&self) -> &AccountId {
            &self.broker
        }

        fn sign_approvals(
            &mut self,
            payload: &MusubiSeedIngressReceiptPayloadV1,
        ) -> Result<Vec<MusubiSeedIngressReceiptApprovalV1>, MusubiPublicationServiceBackendErrorV1>
        {
            self.observed_payloads
                .lock()
                .expect("signing payload observations")
                .push(payload.clone());
            if matches!(self.behavior, TestSigningBehavior::RetryableFailure) {
                return Err(MusubiPublicationServiceBackendErrorV1::Retryable);
            }
            let mut signed_payload = payload.clone();
            if matches!(self.behavior, TestSigningBehavior::WrongPayload) {
                signed_payload.expires_at_ms = signed_payload.expires_at_ms.saturating_add(1);
            }
            let approval = if matches!(self.behavior, TestSigningBehavior::WrongController) {
                let attacker = KeyPair::try_from_seed(
                    b"musubi-publication-signing-provider-attacker".to_vec(),
                    Algorithm::Ed25519,
                )
                .expect("attacker signing key");
                MusubiSeedIngressReceiptApprovalV1 {
                    public_key: attacker.public_key().clone(),
                    signature: SignatureOf::try_from_hash(
                        attacker.private_key(),
                        signed_payload.signing_hash(),
                    )
                    .expect("attacker receipt signature"),
                }
            } else {
                MusubiSeedIngressReceiptApprovalV1 {
                    public_key: self.key_pair.public_key().clone(),
                    signature: SignatureOf::try_from_hash(
                        self.key_pair.private_key(),
                        signed_payload.signing_hash(),
                    )
                    .expect("test receipt signature"),
                }
            };
            let approvals = if matches!(self.behavior, TestSigningBehavior::DuplicateApproval) {
                vec![approval.clone(), approval]
            } else {
                vec![approval]
            };
            if let Some((clock, current_time_ms)) = &self.clock_after_signing {
                clock.store(*current_time_ms, Ordering::SeqCst);
            }
            Ok(approvals)
        }
    }

    #[derive(Clone, Copy)]
    enum ThresholdSigningBehavior {
        Correct,
        BelowThreshold,
        Empty,
        Unsorted,
        OverApprovalBound,
    }

    struct ThresholdReceiptSigningProvider {
        broker: AccountId,
        key_pairs: Vec<KeyPair>,
        behavior: ThresholdSigningBehavior,
    }

    impl MusubiSeedIngressReceiptSigningProviderV1 for ThresholdReceiptSigningProvider {
        fn broker(&self) -> &AccountId {
            &self.broker
        }

        fn sign_approvals(
            &mut self,
            payload: &MusubiSeedIngressReceiptPayloadV1,
        ) -> Result<Vec<MusubiSeedIngressReceiptApprovalV1>, MusubiPublicationServiceBackendErrorV1>
        {
            let signing_hash = payload.signing_hash();
            let mut approvals: Vec<_> = self
                .key_pairs
                .iter()
                .map(|key_pair| MusubiSeedIngressReceiptApprovalV1 {
                    public_key: key_pair.public_key().clone(),
                    signature: SignatureOf::try_from_hash(key_pair.private_key(), signing_hash)
                        .expect("threshold test signature"),
                })
                .collect();
            approvals.sort_by(|left, right| left.public_key.cmp(&right.public_key));
            match self.behavior {
                ThresholdSigningBehavior::Correct => approvals.truncate(2),
                ThresholdSigningBehavior::BelowThreshold => approvals.truncate(1),
                ThresholdSigningBehavior::Empty => approvals.clear(),
                ThresholdSigningBehavior::Unsorted => approvals.reverse(),
                ThresholdSigningBehavior::OverApprovalBound => {}
            }
            Ok(approvals)
        }
    }

    #[derive(Clone, Copy)]
    enum ThresholdAuthorizationSigningBehavior {
        Correct,
        BelowThreshold,
        Duplicate,
        Unsorted,
        OverApprovalBound,
        WrongPayload,
        RetryableFailure,
        PermanentFailure,
    }

    struct ThresholdAuthorizationSigningProvider {
        publisher: AccountId,
        key_pairs: Vec<KeyPair>,
        behavior: ThresholdAuthorizationSigningBehavior,
    }

    impl MusubiPublicationRuntimeAuthorizationSigningProviderV1
        for ThresholdAuthorizationSigningProvider
    {
        fn publisher(&self) -> &AccountId {
            &self.publisher
        }

        fn sign_approvals(
            &self,
            payload: &MusubiPublicationRuntimeAuthorizationPayloadV1,
        ) -> Result<
            Vec<MusubiPublicationRuntimeAuthorizationApprovalV1>,
            MusubiPublicationRuntimeAuthorizationSigningErrorV1,
        > {
            if matches!(
                self.behavior,
                ThresholdAuthorizationSigningBehavior::RetryableFailure
            ) {
                return Err(MusubiPublicationRuntimeAuthorizationSigningErrorV1::Retryable);
            }
            if matches!(
                self.behavior,
                ThresholdAuthorizationSigningBehavior::PermanentFailure
            ) {
                return Err(MusubiPublicationRuntimeAuthorizationSigningErrorV1::Permanent);
            }
            let mut signed_payload = payload.clone();
            if matches!(
                self.behavior,
                ThresholdAuthorizationSigningBehavior::WrongPayload
            ) {
                signed_payload.expires_at_ms = signed_payload.expires_at_ms.saturating_add(1);
            }
            let signing_hash = HashOf::new(&signed_payload);
            let mut approvals: Vec<_> = self
                .key_pairs
                .iter()
                .map(|key_pair| MusubiPublicationRuntimeAuthorizationApprovalV1 {
                    public_key: key_pair.public_key().clone(),
                    signature: SignatureOf::try_from_hash(key_pair.private_key(), signing_hash)
                        .expect("threshold authorization signature"),
                })
                .collect();
            approvals.sort_by(|left, right| left.public_key.cmp(&right.public_key));
            match self.behavior {
                ThresholdAuthorizationSigningBehavior::Correct
                | ThresholdAuthorizationSigningBehavior::WrongPayload => approvals.truncate(2),
                ThresholdAuthorizationSigningBehavior::BelowThreshold => approvals.truncate(1),
                ThresholdAuthorizationSigningBehavior::Duplicate => {
                    approvals.truncate(1);
                    approvals.push(approvals[0].clone());
                }
                ThresholdAuthorizationSigningBehavior::Unsorted => approvals.reverse(),
                ThresholdAuthorizationSigningBehavior::OverApprovalBound => {}
                ThresholdAuthorizationSigningBehavior::RetryableFailure => {
                    return Err(MusubiPublicationRuntimeAuthorizationSigningErrorV1::Retryable);
                }
                ThresholdAuthorizationSigningBehavior::PermanentFailure => {
                    return Err(MusubiPublicationRuntimeAuthorizationSigningErrorV1::Permanent);
                }
            }
            Ok(approvals)
        }
    }

    struct ConflictJournal {
        binding: MusubiPublicationServiceJournalBindingV1,
    }

    impl MusubiPublicationServiceJournalV1 for ConflictJournal {
        fn deployment_binding(&self) -> &MusubiPublicationServiceJournalBindingV1 {
            &self.binding
        }

        fn begin(
            &mut self,
            _attempt: &MusubiPublicationJournalAttemptV1,
            _current_time_ms: u64,
        ) -> Result<MusubiPublicationJournalBeginV1, MusubiPublicationServiceJournalErrorV1>
        {
            Err(MusubiPublicationServiceJournalErrorV1::Unavailable)
        }

        fn refresh_expired_seed_receipt(
            &mut self,
            _attempt: &MusubiPublicationJournalAttemptV1,
            _expected_response: &[u8],
            _current_time_ms: u64,
        ) -> Result<(), MusubiPublicationServiceJournalErrorV1> {
            Err(MusubiPublicationServiceJournalErrorV1::Unavailable)
        }

        fn commit(
            &mut self,
            _key: MusubiPublicationIdempotencyKeyV1,
            _request_digest: [u8; 32],
            _response: &[u8],
        ) -> Result<(), MusubiPublicationServiceJournalErrorV1> {
            Err(MusubiPublicationServiceJournalErrorV1::Conflict)
        }

        fn abort(
            &mut self,
            _key: MusubiPublicationIdempotencyKeyV1,
            _request_digest: [u8; 32],
        ) -> Result<(), MusubiPublicationServiceJournalErrorV1> {
            Err(MusubiPublicationServiceJournalErrorV1::Conflict)
        }
    }

    struct CommitCapacityJournal {
        binding: MusubiPublicationServiceJournalBindingV1,
        aborts: Arc<Mutex<usize>>,
    }

    impl MusubiPublicationServiceJournalV1 for CommitCapacityJournal {
        fn deployment_binding(&self) -> &MusubiPublicationServiceJournalBindingV1 {
            &self.binding
        }

        fn begin(
            &mut self,
            _attempt: &MusubiPublicationJournalAttemptV1,
            _current_time_ms: u64,
        ) -> Result<MusubiPublicationJournalBeginV1, MusubiPublicationServiceJournalErrorV1>
        {
            Err(MusubiPublicationServiceJournalErrorV1::Unavailable)
        }

        fn refresh_expired_seed_receipt(
            &mut self,
            _attempt: &MusubiPublicationJournalAttemptV1,
            _expected_response: &[u8],
            _current_time_ms: u64,
        ) -> Result<(), MusubiPublicationServiceJournalErrorV1> {
            Err(MusubiPublicationServiceJournalErrorV1::Unavailable)
        }

        fn commit(
            &mut self,
            _key: MusubiPublicationIdempotencyKeyV1,
            _request_digest: [u8; 32],
            _response: &[u8],
        ) -> Result<(), MusubiPublicationServiceJournalErrorV1> {
            Err(MusubiPublicationServiceJournalErrorV1::Capacity)
        }

        fn abort(
            &mut self,
            _key: MusubiPublicationIdempotencyKeyV1,
            _request_digest: [u8; 32],
        ) -> Result<(), MusubiPublicationServiceJournalErrorV1> {
            *self.aborts.lock().expect("abort counter") += 1;
            Ok(())
        }
    }

    #[test]
    fn seed_ingress_commit_and_abort_conflicts_are_deadletters_without_double_annotation() {
        let mut fixture = private_service_fixture(false);
        fixture.service.journal = Box::new(ConflictJournal {
            binding: MusubiPublicationServiceJournalBindingV1::from_configuration(
                &fixture.service.config,
            ),
        });
        let key = MusubiPublicationIdempotencyKeyV1 {
            operation: MusubiPublicationRuntimeOperationV1::SeedIngress,
            operation_id: fixture.request.operation_id,
            target: [0; 32],
        };

        let commit_error = fixture
            .service
            .finish_attempt(key, [0x71; 32], Ok(vec![0x01]))
            .expect_err("commit conflict");
        assert_eq!(
            commit_error.telemetry,
            Some(MusubiPublicationServiceTelemetryEventV1::IngestDeadletter(
                MusubiIngestDeadletterReasonV1::ReceiptReplay,
            ))
        );

        let abort_error = fixture
            .service
            .finish_attempt(
                key,
                [0x72; 32],
                Err(MusubiPublicationServiceErrorV1::retryable(
                    MusubiPublicationServiceErrorCodeV1::SeedIngressUnavailable,
                )),
            )
            .expect_err("abort conflict");
        assert_eq!(
            abort_error.telemetry,
            Some(MusubiPublicationServiceTelemetryEventV1::IngestDeadletter(
                MusubiIngestDeadletterReasonV1::ReceiptReplay,
            ))
        );

        let already_annotated = fixture
            .service
            .finish_attempt(
                key,
                [0x73; 32],
                Err(MusubiPublicationServiceErrorV1::permanent(
                    MusubiPublicationServiceErrorCodeV1::BackendResponseInvalid,
                )
                .integrity_failure(MusubiIntegritySurfaceV1::ArchiveCommitment)),
            )
            .expect_err("abort conflict after annotated failure");
        assert_eq!(already_annotated.telemetry, None);
    }

    #[test]
    fn commit_capacity_releases_the_attempt_before_returning_unavailable() {
        let mut fixture = private_service_fixture(false);
        let aborts = Arc::new(Mutex::new(0));
        fixture.service.journal = Box::new(CommitCapacityJournal {
            binding: MusubiPublicationServiceJournalBindingV1::from_configuration(
                &fixture.service.config,
            ),
            aborts: Arc::clone(&aborts),
        });
        let error = fixture
            .service
            .finish_attempt(
                MusubiPublicationIdempotencyKeyV1 {
                    operation: MusubiPublicationRuntimeOperationV1::SeedIngress,
                    operation_id: fixture.request.operation_id,
                    target: [0; 32],
                },
                [0x74; 32],
                Ok(vec![0x01]),
            )
            .expect_err("commit capacity is fail-closed");
        assert_eq!(
            error.code,
            MusubiPublicationServiceErrorCodeV1::JournalUnavailable
        );
        assert!(error.retryable);
        assert_eq!(error.telemetry, None);
        assert_eq!(*aborts.lock().expect("abort counter"), 1);
    }

    struct UnusedStorage;

    impl MusubiStorageCoordinationBackendV1 for UnusedStorage {
        fn coordinate_storage(
            &mut self,
            _request: &MusubiStorageCoordinationRequestV1,
        ) -> Result<MusubiStorageCoordinationResponseV1, MusubiPublicationServiceBackendErrorV1>
        {
            Err(MusubiPublicationServiceBackendErrorV1::Permanent)
        }
    }

    struct UnusedReadback;

    impl MusubiProviderReadbackBackendV1 for UnusedReadback {
        fn readback_provider(
            &mut self,
            _request: &MusubiProviderReadbackRequestV1,
        ) -> Result<MusubiProviderReadbackResponseV1, MusubiPublicationServiceBackendErrorV1>
        {
            Err(MusubiPublicationServiceBackendErrorV1::Permanent)
        }
    }

    struct FixedStorage {
        response: MusubiStorageCoordinationResponseV1,
        substitute: bool,
    }

    impl MusubiStorageCoordinationBackendV1 for FixedStorage {
        fn coordinate_storage(
            &mut self,
            _request: &MusubiStorageCoordinationRequestV1,
        ) -> Result<MusubiStorageCoordinationResponseV1, MusubiPublicationServiceBackendErrorV1>
        {
            let mut response = self.response.clone();
            if self.substitute {
                response.archive.registered_by = AccountId::new(
                    KeyPair::try_from_seed(
                        b"musubi-storage-substitution".to_vec(),
                        Algorithm::Ed25519,
                    )
                    .expect("substitution key")
                    .public_key()
                    .clone(),
                );
            }
            Ok(response)
        }
    }

    struct FixedReadback {
        response: MusubiProviderReadbackResponseV1,
        substitute: bool,
    }

    impl MusubiProviderReadbackBackendV1 for FixedReadback {
        fn readback_provider(
            &mut self,
            _request: &MusubiProviderReadbackRequestV1,
        ) -> Result<MusubiProviderReadbackResponseV1, MusubiPublicationServiceBackendErrorV1>
        {
            let mut response = self.response.clone();
            if self.substitute {
                response.provider = ProviderId::new([0xee; 32]);
            }
            Ok(response)
        }
    }

    struct PrivateServiceFixture {
        service: MusubiPublicationPrivateServiceV1,
        runtime: AuthenticatedMusubiPublicationRuntimeClientV1,
        request: MusubiSeedIngressStageRequestV1,
        metadata: Vec<u8>,
        car: Vec<u8>,
        calls: Arc<Mutex<usize>>,
        clock: Arc<AtomicU64>,
    }

    struct ControlServiceFixture {
        service: MusubiPublicationPrivateServiceV1,
        runtime: AuthenticatedMusubiPublicationRuntimeClientV1,
        storage_request: MusubiStorageCoordinationRequestV1,
        storage_response: MusubiStorageCoordinationResponseV1,
        readback_request: MusubiProviderReadbackRequestV1,
        readback_response: MusubiProviderReadbackResponseV1,
        clock: Arc<AtomicU64>,
    }

    fn control_commitment() -> MusubiArchiveCommitmentV1 {
        MusubiArchiveCommitmentV1 {
            root_cid: ManifestRootCid::from_blake3_digest([0x91; 32]).expect("root CID"),
            chunker: ChunkerProfileHandle {
                profile_id: 1,
                namespace: "sorafs".to_owned(),
                name: "sf1".to_owned(),
                semver: "1.0.0".to_owned(),
                multihash_code: 0x1f,
            },
            chunk_plan_digest: MusubiContentDigestV1::new([0x92; 32]),
            por_root: MusubiContentDigestV1::new([0x93; 32]),
            content_length: 1_024,
            car_digest: MusubiContentDigestV1::new([0x94; 32]),
            car_size: 2_048,
            bundle_digest: MusubiContentDigestV1::new([0x95; 32]),
            source_tree_digest: MusubiContentDigestV1::new([0x96; 32]),
            descriptor_digest: MusubiContentDigestV1::new([0x97; 32]),
            file_count: 2,
            chunk_count: 4,
        }
    }

    fn control_service_fixture(
        substitute_storage: bool,
        substitute_readback: bool,
    ) -> ControlServiceFixture {
        let (client, _) = client();
        let runtime = AuthenticatedMusubiPublicationRuntimeClientV1::from_iroha_client(
            &client,
            Duration::from_secs(5),
        )
        .expect("runtime client");
        let broker_key = KeyPair::try_from_seed(
            b"musubi-publication-control-broker".to_vec(),
            Algorithm::Ed25519,
        )
        .expect("broker key");
        let broker = AccountId::new(broker_key.public_key().clone());
        let genesis_block_hash = [0x99; 32];
        let commitment = control_commitment();
        let semantic_release_digest = MusubiSemanticReleaseDigestV1::new([0x9a; 32]);
        let verification_lock_digest = MusubiVerificationLockDigestV1::new([0x9b; 32]);
        let replication_order = ReplicationOrderId::new([0x9e; 32]);
        let provider_attestations = (0_u16..MUSUBI_MIN_HEALTHY_REPLICAS_V1)
            .map(|index| {
                let index = u8::try_from(index).expect("replica bound fits u8");
                let provider_key =
                    KeyPair::try_from_seed(vec![0xb0 + index; 32], Algorithm::Ed25519)
                        .expect("provider key");
                let provider_owner = AccountId::new(provider_key.public_key().clone());
                let provider_binding = MusubiProviderBundleVerificationBindingV1 {
                    chain_id: client.chain.clone(),
                    genesis_block_hash,
                    provider_id: ProviderId::new([0xb8 + index; 32]),
                    completed_by: provider_owner.clone(),
                    completion_authority: ProviderIngestCompletionAuthorityV1::new(
                        provider_owner,
                        ProviderIngestCompletionSignerPolicyV1 {
                            policy_id: [0xc0 + index; 32],
                            revision: 1,
                            predecessor_digest: None,
                            policy_digest: [0xc8 + index; 32],
                        },
                    ),
                    replication_order,
                    assignment_revision: 1,
                    completion_epoch: 12,
                    finalized_anchor: ProviderIngestFinalizedAnchorV1 {
                        height: 60,
                        block_hash: [0xd0 + index; 32],
                    },
                    archive_id: commitment.archive_id(),
                    bundle_digest: commitment.bundle_digest,
                    descriptor_digest: commitment.descriptor_digest,
                    semantic_release_manifest_digest: semantic_release_digest,
                    verification_lock_digest,
                    source_tree_digest: commitment.source_tree_digest,
                };
                let provider_payload = MusubiProviderBundleVerificationPayloadV1 {
                    version: 1,
                    binding: provider_binding,
                };
                MusubiProviderBundleVerificationAttestationV1 {
                    approvals: vec![MusubiProviderBundleVerificationApprovalV1 {
                        public_key: provider_key.public_key().clone(),
                        signature: SignatureOf::try_from_hash(
                            provider_key.private_key(),
                            provider_payload.signing_hash(),
                        )
                        .expect("provider signature"),
                    }],
                    payload: provider_payload,
                }
            })
            .collect::<Vec<_>>();
        let provider = provider_attestations[0].payload.binding.provider_id;
        let binding = MusubiSeedIngressReceiptBindingV1 {
            chain_id: client.chain.clone(),
            genesis_block_hash,
            publisher: client.account.clone(),
            ingress_broker: broker.clone(),
            seed_provider: provider,
            semantic_release_manifest_digest: semantic_release_digest,
            archive_id: commitment.archive_id(),
            car_body_digest: commitment.car_digest,
            car_body_length: commitment.car_size,
            nonce: [0x9c; 32],
        };
        let receipt_payload = MusubiSeedIngressReceiptPayloadV1 {
            version: 1,
            binding,
            issued_at_ms: 1_000,
            expires_at_ms: 120_000,
        };
        let receipt = MusubiSeedIngressReceiptV1 {
            approvals: vec![MusubiSeedIngressReceiptApprovalV1 {
                public_key: broker_key.public_key().clone(),
                signature: SignatureOf::try_from_hash(
                    broker_key.private_key(),
                    receipt_payload.signing_hash(),
                )
                .expect("receipt signature"),
            }],
            payload: receipt_payload,
        };
        let operation_id = [0x9d; 32];
        let location_id = MusubiArchiveLocationIdV1::new([0x9f; 32]);
        let pin_manifest = ManifestDigest::new([0xa0; 32]);
        let archive = MusubiArchiveRecordV1 {
            archive_id: commitment.archive_id(),
            commitment: commitment.clone(),
            staging_receipt: receipt,
            registered_by: client.account.clone(),
            registered_at_height: 50,
            location_revision: 1,
            location_ids: Vec::new(),
        };
        let storage_request = MusubiStorageCoordinationRequestV1 {
            version: 1,
            operation_id,
            generation: 1,
            prior_location_ids: Vec::new(),
            chain_id: client.chain.clone(),
            genesis_block_hash,
            publisher: client.account.clone(),
            commitment: commitment.clone(),
            verification_lock_digest,
            staging_receipt: archive.staging_receipt.clone(),
            expected_policy_revision: 7,
            finalized_registration: MusubiFinalizedArchiveRegistrationEvidenceV1 {
                version: 1,
                chain_id: client.chain.clone(),
                genesis_block_hash,
                transaction_hash: [0xa4; 32],
                snapshot: MusubiRegistrySnapshotV1 {
                    finalized_height: 55,
                    finalized_block_hash: [0xa5; 32],
                    index_revision: 2,
                },
                registration: archive.registration_projection(),
            },
        };
        let storage_response = MusubiStorageCoordinationResponseV1 {
            version: 1,
            archive,
            location_id,
            pin_manifest,
            replication_order,
            renew_after_epoch: 10,
            expires_at_epoch: 20,
            disposition: MusubiStorageLocationDispositionV1::NeedsRegistration {
                provider_attestations: provider_attestations.clone(),
                expected_location_revision: 1,
            },
        };
        let location = MusubiArchiveLocationV1 {
            location_id,
            archive_id: commitment.archive_id(),
            pin_manifest,
            replication_order,
            providers: provider_attestations
                .iter()
                .map(|attestation| attestation.payload.binding.provider_id)
                .collect(),
            provider_attestations,
            renew_after_epoch: 10,
            expires_at_epoch: 20,
            finalized_height: 70,
            revision: 1,
            state: MusubiArchiveLocationStateV1::Healthy,
        };
        let readback_request = MusubiProviderReadbackRequestV1 {
            version: 1,
            operation_id,
            chain_id: client.chain.clone(),
            genesis_block_hash,
            publisher: client.account.clone(),
            location,
            provider,
            commitment: commitment.clone(),
            semantic_release_digest,
            verification_lock_digest,
        };
        let readback_response = MusubiProviderReadbackResponseV1 {
            version: 1,
            provider,
            location_id,
            replication_order,
            commitment,
            semantic_release_digest,
            verification_lock_digest,
        };
        storage_request.validate().expect("storage request");
        storage_response
            .validate_for(&storage_request)
            .expect("storage response");
        readback_request.validate().expect("readback request");
        readback_response
            .validate_for(&readback_request)
            .expect("readback response");
        let config = MusubiPublicationServiceConfigurationV1 {
            chain_id: client.chain,
            genesis_block_hash,
            ingress_broker: broker.clone(),
            seed_provider: provider,
            max_future_clock_skew_ms: 2_000,
            receipt_lifetime_ms: 60_000,
        };
        let journal_binding = MusubiPublicationServiceJournalBindingV1::from_configuration(&config);
        let signer = SoftwareMusubiSeedIngressReceiptSignerV1::new(broker, broker_key)
            .expect("receipt signer");
        let (clock_adapter, clock) = TestPublicationClock::new(1);
        let service = MusubiPublicationPrivateServiceV1::new(
            config,
            Box::new(clock_adapter),
            Box::new(signer),
            Box::new(
                InMemoryMusubiPublicationServiceJournalV1::new(journal_binding, 16, 32)
                    .expect("bounded journal"),
            ),
            Box::new(RecordingSeedIngress {
                provider,
                calls: Arc::new(Mutex::new(0)),
                fail_first: false,
                clock_after_stage: None,
            }),
            Box::new(FixedStorage {
                response: storage_response.clone(),
                substitute: substitute_storage,
            }),
            Box::new(FixedReadback {
                response: readback_response.clone(),
                substitute: substitute_readback,
            }),
        )
        .expect("control service");
        ControlServiceFixture {
            service,
            runtime,
            storage_request,
            storage_response,
            readback_request,
            readback_response,
            clock,
        }
    }

    fn private_service_fixture(fail_first: bool) -> PrivateServiceFixture {
        let (client, _) = client();
        let runtime = AuthenticatedMusubiPublicationRuntimeClientV1::from_iroha_client(
            &client,
            Duration::from_secs(5),
        )
        .expect("runtime client");
        let broker_key = KeyPair::try_from_seed(
            b"musubi-publication-runtime-broker-test".to_vec(),
            Algorithm::Ed25519,
        )
        .expect("derive broker key");
        let broker = AccountId::new(broker_key.public_key().clone());
        let car = b"canonical private-service CAR".to_vec();
        let request = MusubiSeedIngressStageRequestV1 {
            version: 1,
            operation_id: [0x61; 32],
            binding: MusubiSeedIngressReceiptBindingV1 {
                chain_id: client.chain.clone(),
                genesis_block_hash: [0x62; 32],
                publisher: client.account.clone(),
                ingress_broker: broker.clone(),
                seed_provider: ProviderId::new([0x63; 32]),
                semantic_release_manifest_digest: MusubiSemanticReleaseDigestV1::new([0x64; 32]),
                archive_id: ArchiveId::new([0x65; 32]),
                car_body_digest: MusubiContentDigestV1::new(*blake3::hash(&car).as_bytes()),
                car_body_length: u64::try_from(car.len()).expect("fixture length"),
                nonce: [0x66; 32],
            },
        };
        let metadata = norito::encode_canonical(&request).expect("canonical metadata");
        let calls = Arc::new(Mutex::new(0));
        let config = MusubiPublicationServiceConfigurationV1 {
            chain_id: client.chain,
            genesis_block_hash: request.binding.genesis_block_hash,
            ingress_broker: broker.clone(),
            seed_provider: request.binding.seed_provider,
            max_future_clock_skew_ms: 2_000,
            receipt_lifetime_ms: 60_000,
        };
        let journal_binding = MusubiPublicationServiceJournalBindingV1::from_configuration(&config);
        let signer = SoftwareMusubiSeedIngressReceiptSignerV1::new(broker, broker_key)
            .expect("receipt signer");
        let (clock_adapter, clock) = TestPublicationClock::new(1);
        let service = MusubiPublicationPrivateServiceV1::new(
            config,
            Box::new(clock_adapter),
            Box::new(signer),
            Box::new(
                InMemoryMusubiPublicationServiceJournalV1::new(journal_binding, 16, 32)
                    .expect("bounded journal"),
            ),
            Box::new(RecordingSeedIngress {
                provider: request.binding.seed_provider,
                calls: Arc::clone(&calls),
                fail_first,
                clock_after_stage: None,
            }),
            Box::new(UnusedStorage),
            Box::new(UnusedReadback),
        )
        .expect("private service");
        PrivateServiceFixture {
            service,
            runtime,
            request,
            metadata,
            car,
            calls,
            clock,
        }
    }

    fn threshold_private_service_fixture(
        behavior: ThresholdSigningBehavior,
    ) -> PrivateServiceFixture {
        let mut fixture = private_service_fixture(false);
        let member_count = if matches!(behavior, ThresholdSigningBehavior::OverApprovalBound) {
            u8::try_from(MUSUBI_MAX_PUBLICATION_ATTESTATION_APPROVALS_V1 + 1)
                .expect("approval bound fits u8")
        } else {
            3
        };
        let key_pairs: Vec<_> = (0_u8..member_count)
            .map(|index| {
                KeyPair::try_from_seed(vec![0xb0_u8.saturating_add(index); 32], Algorithm::Ed25519)
                    .expect("derive threshold broker key")
            })
            .collect();
        let members = key_pairs
            .iter()
            .map(|key_pair| {
                MultisigMember::new(key_pair.public_key().clone(), 1)
                    .expect("threshold broker member")
            })
            .collect();
        let broker = AccountId::new_multisig(
            MultisigPolicy::new(2, members).expect("threshold broker policy"),
        );
        fixture.request.binding.ingress_broker = broker.clone();
        fixture.metadata =
            norito::encode_canonical(&fixture.request).expect("threshold stage metadata");
        let config = MusubiPublicationServiceConfigurationV1 {
            chain_id: fixture.request.binding.chain_id.clone(),
            genesis_block_hash: fixture.request.binding.genesis_block_hash,
            ingress_broker: broker.clone(),
            seed_provider: fixture.request.binding.seed_provider,
            max_future_clock_skew_ms: 2_000,
            receipt_lifetime_ms: 60_000,
        };
        let journal_binding = MusubiPublicationServiceJournalBindingV1::from_configuration(&config);
        let signer = ThresholdReceiptSigningProvider {
            broker,
            key_pairs,
            behavior,
        };
        let (clock_adapter, clock) = TestPublicationClock::new(1);
        let calls = Arc::new(Mutex::new(0));
        fixture.service = MusubiPublicationPrivateServiceV1::new(
            config,
            Box::new(clock_adapter),
            Box::new(signer),
            Box::new(
                InMemoryMusubiPublicationServiceJournalV1::new(journal_binding, 16, 32)
                    .expect("bounded journal"),
            ),
            Box::new(RecordingSeedIngress {
                provider: fixture.request.binding.seed_provider,
                calls: Arc::clone(&calls),
                fail_first: false,
                clock_after_stage: None,
            }),
            Box::new(UnusedStorage),
            Box::new(UnusedReadback),
        )
        .expect("threshold private service");
        fixture.clock = clock;
        fixture.calls = calls;
        fixture
    }

    fn authorization_header(
        runtime: &AuthenticatedMusubiPublicationRuntimeClientV1,
        request: &MusubiSeedIngressStageRequestV1,
        metadata: &[u8],
        issued_at_ms: u64,
    ) -> String {
        let digest = request_digest(MusubiPublicationRuntimeOperationV1::SeedIngress, metadata)
            .expect("request digest");
        let authorization = runtime
            .authorization(
                MusubiPublicationRuntimeOperationV1::SeedIngress,
                request.operation_id,
                digest,
                issued_at_ms,
            )
            .expect("authorization");
        base64::engine::general_purpose::URL_SAFE_NO_PAD
            .encode(norito::encode_canonical(&authorization).expect("canonical authorization"))
    }

    fn control_authorization_header(
        runtime: &AuthenticatedMusubiPublicationRuntimeClientV1,
        operation: MusubiPublicationRuntimeOperationV1,
        operation_id: [u8; 32],
        body: &[u8],
        issued_at_ms: u64,
    ) -> String {
        let digest = request_digest(operation, body).expect("request digest");
        let authorization = runtime
            .authorization(operation, operation_id, digest, issued_at_ms)
            .expect("authorization");
        base64::engine::general_purpose::URL_SAFE_NO_PAD
            .encode(norito::encode_canonical(&authorization).expect("canonical authorization"))
    }

    fn seed_http_response(
        fixture: &mut PrivateServiceFixture,
        authorization: &str,
        metadata: &[u8],
        body: &[u8],
        current_time_ms: u64,
    ) -> MusubiPublicationPrivateHttpResponseV1 {
        let metadata = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(metadata);
        fixture.clock.store(current_time_ms, Ordering::SeqCst);
        fixture
            .service
            .handle(MusubiPublicationPrivateHttpRequestV1 {
                method: "POST",
                path: MUSUBI_PUBLICATION_SEED_INGRESS_PATH_V1,
                content_type: APPLICATION_SORAFS_CAR,
                authorization: Some(authorization),
                seed_ingress_metadata: Some(&metadata),
                body,
            })
    }

    fn decode_service_error(
        response: &MusubiPublicationPrivateHttpResponseV1,
    ) -> MusubiPublicationServiceErrorResponseV1 {
        norito::decode_canonical_with_limits(&response.body, RESPONSE_DECODE_LIMITS)
            .expect("typed service error")
    }

    fn client() -> (Client, KeyPair) {
        let key_pair = KeyPair::try_from_seed(
            b"musubi-publication-runtime-client-test".to_vec(),
            Algorithm::Ed25519,
        )
        .expect("derive fixture key");
        let account = AccountId::new(key_pair.public_key().clone());
        let client = Client {
            chain: ChainId::from("musubi-runtime-test"),
            torii_url: Url::parse("https://torii.example/").expect("Torii URL"),
            key_pair: key_pair.clone(),
            transaction_ttl: Some(Duration::from_secs(10)),
            transaction_status_timeout: Duration::from_secs(5),
            torii_request_timeout: Duration::from_secs(5),
            account,
            headers: Default::default(),
            operator_key_pair: None,
            add_transaction_nonce: false,
            alias_cache_policy: sorafs_manifest::alias_cache::AliasCachePolicy::new(
                Duration::from_secs(1),
                Duration::from_secs(1),
                Duration::from_secs(1),
                Duration::from_secs(1),
                Duration::from_secs(1),
                Duration::from_secs(1),
                Duration::from_secs(1),
                Duration::from_secs(1),
            ),
            default_anonymity_policy: Default::default(),
            rollout_phase: Default::default(),
            data_model_compatibility: Arc::new(Mutex::new(
                crate::client::DataModelCompatibility::Unchecked,
            )),
            wire_format_preference: Default::default(),
        };
        (client, key_pair)
    }

    fn threshold_authorization_runtime(
        behavior: ThresholdAuthorizationSigningBehavior,
    ) -> AuthenticatedMusubiPublicationRuntimeClientV1 {
        let member_count = if matches!(
            behavior,
            ThresholdAuthorizationSigningBehavior::OverApprovalBound
        ) {
            u8::try_from(MUSUBI_MAX_PUBLICATION_ATTESTATION_APPROVALS_V1 + 1)
                .expect("approval bound fits u8")
        } else {
            3
        };
        let key_pairs: Vec<_> = (0_u8..member_count)
            .map(|index| {
                KeyPair::try_from_seed(vec![index.saturating_add(1); 32], Algorithm::Ed25519)
                    .expect("derive threshold publisher key")
            })
            .collect();
        let members = key_pairs
            .iter()
            .map(|key_pair| {
                MultisigMember::new(key_pair.public_key().clone(), 1)
                    .expect("threshold publisher member")
            })
            .collect();
        let publisher = AccountId::new_multisig(
            MultisigPolicy::new(2, members).expect("threshold publisher policy"),
        );
        let signer = ThresholdAuthorizationSigningProvider {
            publisher: publisher.clone(),
            key_pairs,
            behavior,
        };
        AuthenticatedMusubiPublicationRuntimeClientV1::from_authorization_signer(
            ChainId::from("musubi-runtime-threshold-test"),
            publisher,
            Arc::new(signer),
            Duration::from_secs(5),
        )
        .expect("threshold runtime client")
    }

    #[test]
    fn authorization_is_domain_bound_and_verifiable() {
        let (client, _) = client();
        let runtime = AuthenticatedMusubiPublicationRuntimeClientV1::from_iroha_client(
            &client,
            Duration::from_secs(5),
        )
        .expect("runtime client");
        let operation_id = [0x44; 32];
        let digest = [0x55; 32];
        let authorization = runtime
            .authorization(
                MusubiPublicationRuntimeOperationV1::StorageCoordination,
                operation_id,
                digest,
                1_000,
            )
            .expect("authorization");
        authorization
            .verify(
                MusubiPublicationRuntimeOperationV1::StorageCoordination,
                operation_id,
                digest,
                1_001,
            )
            .expect("verify exact authorization");
        assert!(
            authorization
                .verify(
                    MusubiPublicationRuntimeOperationV1::ProviderReadback,
                    operation_id,
                    digest,
                    1_001,
                )
                .is_err()
        );

        let mut substituted = authorization.clone();
        substituted.payload.domain = [0x99; 32];
        assert!(substituted.payload.validate().is_err());
        assert!(
            substituted.approvals[0]
                .signature
                .verify_hash(
                    &substituted.approvals[0].public_key,
                    HashOf::new(&substituted.payload),
                )
                .is_err()
        );
    }

    #[test]
    fn authorization_verifier_enforces_fixed_clock_skew_bound() {
        let (client, _) = client();
        let runtime = AuthenticatedMusubiPublicationRuntimeClientV1::from_iroha_client(
            &client,
            Duration::from_secs(5),
        )
        .expect("runtime client");
        let operation = MusubiPublicationRuntimeOperationV1::StorageCoordination;
        let operation_id = [0x46; 32];
        let digest = [0x57; 32];
        let current_time_ms = 1_000;
        let issued_at_ms = current_time_ms + MUSUBI_PUBLICATION_SERVICE_MAX_CLOCK_SKEW_MS_V1;
        let authorization = runtime
            .authorization(operation, operation_id, digest, issued_at_ms)
            .expect("future-skew authorization");

        authorization
            .verify_with_clock_skew(
                operation,
                operation_id,
                digest,
                current_time_ms,
                MUSUBI_PUBLICATION_SERVICE_MAX_CLOCK_SKEW_MS_V1,
            )
            .expect("the exact protocol skew boundary is accepted");

        for (verification_time_ms, skew_ms) in [
            (
                current_time_ms,
                MUSUBI_PUBLICATION_SERVICE_MAX_CLOCK_SKEW_MS_V1 + 1,
            ),
            (0, 0),
        ] {
            let error = authorization
                .verify_with_clock_skew(
                    operation,
                    operation_id,
                    digest,
                    verification_time_ms,
                    skew_ms,
                )
                .expect_err("an invalid verifier clock policy must fail closed");
            assert_eq!(error.code(), "MUSUBI_RUNTIME_CLOCK_INVALID");
            assert_eq!(
                error.class(),
                MusubiPublicationRuntimeTransportFailureClassV1::Permanent
            );
        }
    }

    #[test]
    fn publisher_authorization_accepts_exact_multisig_quorum_and_rejects_bad_sets() {
        let operation = MusubiPublicationRuntimeOperationV1::StorageCoordination;
        let operation_id = [0x45; 32];
        let digest = [0x56; 32];
        let runtime =
            threshold_authorization_runtime(ThresholdAuthorizationSigningBehavior::Correct);
        let authorization = runtime
            .authorization(operation, operation_id, digest, 1_000)
            .expect("2-of-3 publisher authorization");
        assert_eq!(authorization.approvals.len(), 2);
        authorization
            .verify(operation, operation_id, digest, 1_001)
            .expect("verify multisig publisher authorization");

        for (behavior, code, class) in [
            (
                ThresholdAuthorizationSigningBehavior::BelowThreshold,
                "MUSUBI_RUNTIME_AUTHORIZATION_THRESHOLD_UNMET",
                MusubiPublicationRuntimeTransportFailureClassV1::Permanent,
            ),
            (
                ThresholdAuthorizationSigningBehavior::Duplicate,
                "MUSUBI_RUNTIME_AUTHORIZATION_APPROVALS_INVALID",
                MusubiPublicationRuntimeTransportFailureClassV1::Permanent,
            ),
            (
                ThresholdAuthorizationSigningBehavior::Unsorted,
                "MUSUBI_RUNTIME_AUTHORIZATION_APPROVALS_INVALID",
                MusubiPublicationRuntimeTransportFailureClassV1::Permanent,
            ),
            (
                ThresholdAuthorizationSigningBehavior::OverApprovalBound,
                "MUSUBI_RUNTIME_AUTHORIZATION_APPROVALS_INVALID",
                MusubiPublicationRuntimeTransportFailureClassV1::Permanent,
            ),
            (
                ThresholdAuthorizationSigningBehavior::WrongPayload,
                "MUSUBI_RUNTIME_AUTHORIZATION_SIGNATURE_INVALID",
                MusubiPublicationRuntimeTransportFailureClassV1::Permanent,
            ),
            (
                ThresholdAuthorizationSigningBehavior::RetryableFailure,
                "MUSUBI_RUNTIME_AUTHORIZATION_SIGNER_UNAVAILABLE",
                MusubiPublicationRuntimeTransportFailureClassV1::Retryable,
            ),
            (
                ThresholdAuthorizationSigningBehavior::PermanentFailure,
                "MUSUBI_RUNTIME_AUTHORIZATION_SIGNING_FAILED",
                MusubiPublicationRuntimeTransportFailureClassV1::Permanent,
            ),
        ] {
            let error = threshold_authorization_runtime(behavior)
                .authorization(operation, operation_id, digest, 1_000)
                .expect_err("invalid approval set must fail before transport");
            assert_eq!(error.code(), code);
            assert_eq!(error.class(), class);
        }
    }

    #[test]
    fn publisher_authorization_counts_weight_and_revalidates_decoded_policy() {
        let operation = MusubiPublicationRuntimeOperationV1::StorageCoordination;
        let operation_id = [0x47; 32];
        let digest = [0x58; 32];
        let key_pairs: Vec<_> = (0_u8..3)
            .map(|index| {
                KeyPair::try_from_seed(vec![0x70 + index; 32], Algorithm::Ed25519)
                    .expect("derive weighted publisher key")
            })
            .collect();
        let members = key_pairs
            .iter()
            .zip([3_u16, 1, 1])
            .map(|(key_pair, weight)| {
                MultisigMember::new(key_pair.public_key().clone(), weight)
                    .expect("weighted publisher member")
            })
            .collect();
        let publisher = AccountId::new_multisig(
            MultisigPolicy::new(3, members).expect("weighted publisher policy"),
        );
        let payload = MusubiPublicationRuntimeAuthorizationPayloadV1 {
            domain: AUTH_DOMAIN_V1,
            version: 1,
            operation,
            operation_id,
            chain_id: ChainId::from("musubi-runtime-weighted-test"),
            publisher,
            request_digest: digest,
            issued_at_ms: 1_000,
            expires_at_ms: 2_000,
        };
        let authorization_for = |indices: &[usize]| {
            let mut approvals: Vec<_> = indices
                .iter()
                .map(|index| MusubiPublicationRuntimeAuthorizationApprovalV1 {
                    public_key: key_pairs[*index].public_key().clone(),
                    signature: SignatureOf::try_new(key_pairs[*index].private_key(), &payload)
                        .expect("weighted publisher signature"),
                })
                .collect();
            approvals.sort_by(|left, right| left.public_key.cmp(&right.public_key));
            MusubiPublicationRuntimeAuthorizationV1 {
                payload: payload.clone(),
                approvals,
            }
        };

        authorization_for(&[0])
            .verify(operation, operation_id, digest, 1_001)
            .expect("one weight-three member meets threshold three");
        let error = authorization_for(&[1, 2])
            .verify(operation, operation_id, digest, 1_001)
            .expect_err("two weight-one members remain below threshold three");
        assert_eq!(error.code(), "MUSUBI_RUNTIME_AUTHORIZATION_THRESHOLD_UNMET");

        let valid_policy = MultisigPolicy::new(
            1,
            vec![
                MultisigMember::new(key_pairs[0].public_key().clone(), 1)
                    .expect("publisher member"),
            ],
        )
        .expect("valid policy fixture");
        let mut unchecked_json =
            norito::json::to_value(&valid_policy).expect("serialize policy fixture");
        *unchecked_json
            .as_object_mut()
            .and_then(|object| object.get_mut("threshold"))
            .expect("policy threshold field") = norito::json::Value::from(0_u64);
        let unchecked_policy: MultisigPolicy = norito::json::from_value(unchecked_json)
            .expect("generic decoding materializes unchecked policy fields");
        let mut malformed = payload;
        malformed.publisher = AccountId::new_multisig(unchecked_policy);
        let malformed = MusubiPublicationRuntimeAuthorizationV1 {
            approvals: vec![MusubiPublicationRuntimeAuthorizationApprovalV1 {
                public_key: key_pairs[0].public_key().clone(),
                signature: SignatureOf::try_new(key_pairs[0].private_key(), &malformed)
                    .expect("malformed-policy fixture signature"),
            }],
            payload: malformed,
        };
        let error = malformed
            .verify(operation, operation_id, digest, 1_001)
            .expect_err("a structurally invalid decoded controller must fail closed");
        assert_eq!(
            error.code(),
            "MUSUBI_RUNTIME_AUTHORIZATION_CONTROLLER_UNSUPPORTED"
        );
    }

    #[test]
    fn software_publisher_authorizer_requires_one_matching_controller_key() {
        let keys: Vec<_> = (0_u8..2)
            .map(|index| {
                KeyPair::try_from_seed(vec![0xe0_u8 + index; 32], Algorithm::Ed25519)
                    .expect("derive publisher key")
            })
            .collect();
        let members = keys
            .iter()
            .map(|key_pair| {
                MultisigMember::new(key_pair.public_key().clone(), 1).expect("publisher member")
            })
            .collect();
        let publisher =
            AccountId::new_multisig(MultisigPolicy::new(2, members).expect("publisher policy"));
        let error =
            SoftwareMusubiPublicationRuntimeAuthorizationSignerV1::new(publisher, keys[0].clone())
                .expect_err("software adapter must not pretend to collect a threshold");
        assert_eq!(
            error.code(),
            "MUSUBI_RUNTIME_MULTISIG_AUTH_PROVIDER_REQUIRED"
        );
    }

    #[test]
    fn private_service_urls_reject_credentials_redirect_primitives_and_retired_upload() {
        for valid in [
            "https://seed.example/",
            "https://seed.example/private/",
            "https://127.0.0.1:8443/",
        ] {
            validate_publication_service_base_url(&Url::parse(valid).expect("valid URL"))
                .expect("accepted private HTTPS base");
        }
        for invalid in [
            "http://seed.example/",
            "https://user:secret@seed.example/",
            "https://seed.example/path",
            "https://seed.example/?token=secret",
            "https://seed.example/#fragment",
            "https://seed.example/v1/sorafs/upload/",
        ] {
            assert!(
                validate_publication_service_base_url(
                    &Url::parse(invalid).expect("syntactically valid URL")
                )
                .is_err(),
                "accepted {invalid}"
            );
        }
    }

    #[test]
    fn request_digest_binds_operation_length_and_body() {
        let body = b"canonical request";
        let digest = request_digest(
            MusubiPublicationRuntimeOperationV1::StorageCoordination,
            body,
        )
        .expect("bounded digest input");
        assert_ne!(
            digest,
            request_digest(MusubiPublicationRuntimeOperationV1::ProviderReadback, body)
                .expect("bounded digest input")
        );
        assert_ne!(
            digest,
            request_digest(
                MusubiPublicationRuntimeOperationV1::StorageCoordination,
                b"canonical request!"
            )
            .expect("bounded digest input")
        );
    }

    #[test]
    fn seed_ingress_carries_exact_metadata_and_authorization_with_raw_car_body() {
        let (mut client, _) = client();
        client.headers.insert(
            "Authorization".to_owned(),
            "Basic must-not-cross-boundary".to_owned(),
        );
        client.headers.insert(
            "x-platform-secret".to_owned(),
            "must-not-cross-boundary".to_owned(),
        );
        let runtime = AuthenticatedMusubiPublicationRuntimeClientV1::from_iroha_client(
            &client,
            Duration::from_secs(5),
        )
        .expect("runtime client");
        let car = b"raw canonical SoraFS CAR bytes".to_vec();
        let operation_id = [0x31; 32];
        let stage_request = MusubiSeedIngressStageRequestV1 {
            version: 1,
            operation_id,
            binding: MusubiSeedIngressReceiptBindingV1 {
                chain_id: client.chain.clone(),
                genesis_block_hash: [0x32; 32],
                publisher: client.account.clone(),
                ingress_broker: client.account.clone(),
                seed_provider: ProviderId::new([0x33; 32]),
                semantic_release_manifest_digest: MusubiSemanticReleaseDigestV1::new([0x34; 32]),
                archive_id: ArchiveId::new([0x35; 32]),
                car_body_digest: MusubiContentDigestV1::new(*blake3::hash(&car).as_bytes()),
                car_body_length: u64::try_from(car.len()).expect("fixture CAR length fits u64"),
                nonce: [0x36; 32],
            },
        };
        stage_request.validate().expect("valid stage metadata");
        let metadata = norito::encode_canonical(&stage_request).expect("encode stage metadata");
        let digest = request_digest(MusubiPublicationRuntimeOperationV1::SeedIngress, &metadata)
            .expect("bounded metadata digest");
        let authorization = runtime
            .authorization(
                MusubiPublicationRuntimeOperationV1::SeedIngress,
                operation_id,
                digest,
                1_000,
            )
            .expect("authorize exact metadata");
        let endpoint = publication_route(
            &Url::parse("https://seed.example/private/").expect("base URL"),
            SEED_INGRESS_ROUTE,
        )
        .expect("stage endpoint");
        let prepared = runtime
            .prepare_request(
                endpoint,
                APPLICATION_SORAFS_CAR,
                &authorization,
                Some(&metadata),
                car.clone(),
            )
            .expect("prepare seed-ingress request");

        assert_eq!(
            prepared.body().and_then(reqwest::blocking::Body::as_bytes),
            Some(car.as_slice())
        );
        assert!(prepared.headers().get("Authorization").is_none());
        assert!(prepared.headers().get("x-platform-secret").is_none());
        assert_eq!(
            prepared.url().path(),
            "/private/v1/musubi/publication/seed-ingress"
        );
        let encoded_metadata = prepared
            .headers()
            .get(SEED_INGRESS_METADATA_HEADER)
            .expect("typed metadata header")
            .to_str()
            .expect("ASCII metadata header");
        assert!(
            prepared
                .headers()
                .get(SEED_INGRESS_METADATA_HEADER)
                .expect("typed metadata header")
                .is_sensitive()
        );
        assert_eq!(
            encoded_metadata,
            base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(&metadata)
        );
        let decoded_metadata = base64::engine::general_purpose::URL_SAFE_NO_PAD
            .decode(encoded_metadata)
            .expect("decode metadata header");
        let decoded_request: MusubiSeedIngressStageRequestV1 =
            norito::decode_canonical(&decoded_metadata).expect("decode canonical metadata");
        assert_eq!(decoded_request, stage_request);

        let encoded_authorization = prepared
            .headers()
            .get(AUTHORIZATION_HEADER)
            .expect("authorization header")
            .to_str()
            .expect("ASCII authorization header");
        assert!(
            prepared
                .headers()
                .get(AUTHORIZATION_HEADER)
                .expect("authorization header")
                .is_sensitive()
        );
        let authorization_bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
            .decode(encoded_authorization)
            .expect("decode authorization header");
        let decoded_authorization: MusubiPublicationRuntimeAuthorizationV1 =
            norito::decode_canonical(&authorization_bytes).expect("decode authorization");
        decoded_authorization
            .verify(
                MusubiPublicationRuntimeOperationV1::SeedIngress,
                operation_id,
                digest,
                1_001,
            )
            .expect("authorization binds exact metadata");
    }

    #[test]
    fn private_service_constructs_and_verifies_exact_external_signer_payload() {
        let mut fixture = private_service_fixture(false);
        let broker_key = KeyPair::try_from_seed(
            b"musubi-publication-runtime-broker-test".to_vec(),
            Algorithm::Ed25519,
        )
        .expect("derive broker key");
        let (provider, observed) = TestReceiptSigningProvider::new(
            fixture.request.binding.ingress_broker.clone(),
            broker_key,
            TestSigningBehavior::Correct,
        );
        fixture.service.receipt_signer = Box::new(provider);
        fixture.service.seed_ingress = Box::new(RecordingSeedIngress {
            provider: fixture.request.binding.seed_provider,
            calls: Arc::clone(&fixture.calls),
            fail_first: false,
            clock_after_stage: Some((Arc::clone(&fixture.clock), 1_901)),
        });

        let metadata = fixture.metadata.clone();
        let car = fixture.car.clone();
        let authorization =
            authorization_header(&fixture.runtime, &fixture.request, &metadata, 900);
        let response = seed_http_response(&mut fixture, &authorization, &metadata, &car, 901);
        assert_eq!(response.status, 200);
        let receipt: MusubiSeedIngressReceiptV1 =
            norito::decode_canonical_with_limits(&response.body, RESPONSE_DECODE_LIMITS)
                .expect("externally signed receipt");
        receipt
            .verify(&fixture.request.binding, 1_901)
            .expect("service verifies exact external approval");
        let observed = observed.lock().expect("observed signing payloads");
        assert_eq!(observed.as_slice(), &[receipt.payload.clone()]);
        assert_eq!(receipt.payload.binding, fixture.request.binding);
        assert_eq!(receipt.payload.issued_at_ms, 1_901);
        assert_eq!(receipt.payload.expires_at_ms, 61_901);
    }

    #[test]
    fn private_service_rejects_invalid_or_unavailable_external_signer_and_retries_exactly() {
        for (behavior, expected_status, expected_retryable) in [
            (TestSigningBehavior::WrongPayload, 422, false),
            (TestSigningBehavior::WrongController, 422, false),
            (TestSigningBehavior::DuplicateApproval, 422, false),
            (TestSigningBehavior::RetryableFailure, 503, true),
        ] {
            let mut fixture = private_service_fixture(false);
            let broker_key = KeyPair::try_from_seed(
                b"musubi-publication-runtime-broker-test".to_vec(),
                Algorithm::Ed25519,
            )
            .expect("derive broker key");
            let (provider, _) = TestReceiptSigningProvider::new(
                fixture.request.binding.ingress_broker.clone(),
                broker_key.clone(),
                behavior,
            );
            fixture.service.receipt_signer = Box::new(provider);
            let metadata = fixture.metadata.clone();
            let car = fixture.car.clone();
            let authorization =
                authorization_header(&fixture.runtime, &fixture.request, &metadata, 10_000);
            let failed = seed_http_response(&mut fixture, &authorization, &metadata, &car, 10_001);
            let failure = decode_service_error(&failed);
            assert_eq!(failed.status, expected_status);
            assert_eq!(
                failure.code,
                MusubiPublicationServiceErrorCodeV1::ReceiptSigningUnavailable
            );
            assert_eq!(failure.retryable, expected_retryable);
            assert_eq!(*fixture.calls.lock().expect("seed calls"), 1);

            let (correct, _) = TestReceiptSigningProvider::new(
                fixture.request.binding.ingress_broker.clone(),
                broker_key,
                TestSigningBehavior::Correct,
            );
            fixture.service.receipt_signer = Box::new(correct);
            let fresh = authorization_header(&fixture.runtime, &fixture.request, &metadata, 10_002);
            let retried = seed_http_response(&mut fixture, &fresh, &metadata, &car, 10_003);
            assert_eq!(
                retried.status, 200,
                "exact signer retry must leave its tombstone usable"
            );
            assert_eq!(*fixture.calls.lock().expect("seed calls"), 2);
        }

        let mut fixture = private_service_fixture(false);
        let broker_key = KeyPair::try_from_seed(
            b"musubi-publication-runtime-broker-test".to_vec(),
            Algorithm::Ed25519,
        )
        .expect("derive broker key");
        let (slow_provider, _) = TestReceiptSigningProvider::new(
            fixture.request.binding.ingress_broker.clone(),
            broker_key.clone(),
            TestSigningBehavior::Correct,
        );
        fixture.service.receipt_signer =
            Box::new(slow_provider.with_clock_after_signing(Arc::clone(&fixture.clock), 70_001));
        let metadata = fixture.metadata.clone();
        let car = fixture.car.clone();
        let authorization =
            authorization_header(&fixture.runtime, &fixture.request, &metadata, 10_000);
        let expired = seed_http_response(&mut fixture, &authorization, &metadata, &car, 10_001);
        let expired_error = decode_service_error(&expired);
        assert_eq!(expired.status, 503);
        assert_eq!(
            expired_error.code,
            MusubiPublicationServiceErrorCodeV1::ReceiptSigningUnavailable
        );
        assert!(expired_error.retryable);

        let (correct, _) = TestReceiptSigningProvider::new(
            fixture.request.binding.ingress_broker.clone(),
            broker_key,
            TestSigningBehavior::Correct,
        );
        fixture.service.receipt_signer = Box::new(correct);
        let fresh = authorization_header(&fixture.runtime, &fixture.request, &metadata, 70_002);
        let retried = seed_http_response(&mut fixture, &fresh, &metadata, &car, 70_003);
        assert_eq!(retried.status, 200);
        assert_eq!(*fixture.calls.lock().expect("seed calls"), 2);
    }

    #[test]
    fn private_service_constructor_binds_signer_and_seed_backend_identities() {
        let broker_key = KeyPair::try_from_seed(
            b"musubi-publication-constructor-broker".to_vec(),
            Algorithm::Ed25519,
        )
        .expect("derive broker key");
        let broker = AccountId::new(broker_key.public_key().clone());
        let provider = ProviderId::new([0xa1; 32]);
        let config = MusubiPublicationServiceConfigurationV1 {
            chain_id: ChainId::from("musubi-constructor-test"),
            genesis_block_hash: [0xa2; 32],
            ingress_broker: broker.clone(),
            seed_provider: provider,
            max_future_clock_skew_ms: 1_000,
            receipt_lifetime_ms: 60_000,
        };
        let journal_binding = MusubiPublicationServiceJournalBindingV1::from_configuration(&config);

        let wrong_broker_key = KeyPair::try_from_seed(
            b"musubi-publication-constructor-wrong-broker".to_vec(),
            Algorithm::Ed25519,
        )
        .expect("derive wrong broker key");
        let wrong_broker = AccountId::new(wrong_broker_key.public_key().clone());
        let wrong_signer =
            SoftwareMusubiSeedIngressReceiptSignerV1::new(wrong_broker, wrong_broker_key)
                .expect("internally consistent wrong signer");
        let (signer_mismatch_clock, _) = TestPublicationClock::new(1_000);
        let signer_mismatch = MusubiPublicationPrivateServiceV1::new(
            config.clone(),
            Box::new(signer_mismatch_clock),
            Box::new(wrong_signer),
            Box::new(
                InMemoryMusubiPublicationServiceJournalV1::new(journal_binding.clone(), 2, 4)
                    .expect("journal"),
            ),
            Box::new(RecordingSeedIngress {
                provider,
                calls: Arc::new(Mutex::new(0)),
                fail_first: false,
                clock_after_stage: None,
            }),
            Box::new(UnusedStorage),
            Box::new(UnusedReadback),
        );
        assert!(matches!(
            signer_mismatch,
            Err(MusubiPublicationServiceErrorCodeV1::IdentityMismatch)
        ));

        let signer = SoftwareMusubiSeedIngressReceiptSignerV1::new(broker, broker_key)
            .expect("matching software signer");
        let signer_debug = format!("{signer:?}");
        assert!(!signer_debug.contains("key_pair"));
        assert!(!signer_debug.contains("private"));
        let (provider_mismatch_clock, _) = TestPublicationClock::new(1_000);
        let provider_mismatch = MusubiPublicationPrivateServiceV1::new(
            config,
            Box::new(provider_mismatch_clock),
            Box::new(signer),
            Box::new(
                InMemoryMusubiPublicationServiceJournalV1::new(journal_binding, 2, 4)
                    .expect("journal"),
            ),
            Box::new(RecordingSeedIngress {
                provider: ProviderId::new([0xa3; 32]),
                calls: Arc::new(Mutex::new(0)),
                fail_first: false,
                clock_after_stage: None,
            }),
            Box::new(UnusedStorage),
            Box::new(UnusedReadback),
        );
        assert!(matches!(
            provider_mismatch,
            Err(MusubiPublicationServiceErrorCodeV1::IdentityMismatch)
        ));
    }

    #[test]
    fn private_service_verifies_bounded_threshold_signing_provider() {
        let mut fixture = threshold_private_service_fixture(ThresholdSigningBehavior::Correct);
        let metadata = fixture.metadata.clone();
        let car = fixture.car.clone();
        let authorization =
            authorization_header(&fixture.runtime, &fixture.request, &metadata, 1_000);
        let response = seed_http_response(&mut fixture, &authorization, &metadata, &car, 1_001);
        assert_eq!(response.status, 200);
        let receipt: MusubiSeedIngressReceiptV1 =
            norito::decode_canonical_with_limits(&response.body, RESPONSE_DECODE_LIMITS)
                .expect("threshold receipt");
        receipt
            .verify(&fixture.request.binding, 1_001)
            .expect("2-of-3 broker receipt");
        assert_eq!(receipt.approvals.len(), 2);

        for behavior in [
            ThresholdSigningBehavior::BelowThreshold,
            ThresholdSigningBehavior::Empty,
            ThresholdSigningBehavior::Unsorted,
            ThresholdSigningBehavior::OverApprovalBound,
        ] {
            let mut fixture = threshold_private_service_fixture(behavior);
            let metadata = fixture.metadata.clone();
            let car = fixture.car.clone();
            let authorization =
                authorization_header(&fixture.runtime, &fixture.request, &metadata, 2_000);
            let response = seed_http_response(&mut fixture, &authorization, &metadata, &car, 2_001);
            let error = decode_service_error(&response);
            assert_eq!(response.status, 422);
            assert_eq!(
                error.code,
                MusubiPublicationServiceErrorCodeV1::ReceiptSigningUnavailable
            );
            assert!(!error.retryable);
        }
    }

    #[test]
    fn private_service_rejects_broker_quorum_larger_than_receipt_bound() {
        let key_pairs: Vec<_> =
            (0_u8..u8::try_from(MUSUBI_MAX_PUBLICATION_ATTESTATION_APPROVALS_V1 + 1)
                .expect("approval bound fits u8"))
                .map(|index| {
                    KeyPair::try_from_seed(vec![index.saturating_add(1); 32], Algorithm::Ed25519)
                        .expect("derive impossible broker key")
                })
                .collect();
        let members = key_pairs
            .iter()
            .map(|key_pair| {
                MultisigMember::new(key_pair.public_key().clone(), 1)
                    .expect("impossible broker member")
            })
            .collect();
        let broker = AccountId::new_multisig(
            MultisigPolicy::new(
                u16::try_from(MUSUBI_MAX_PUBLICATION_ATTESTATION_APPROVALS_V1 + 1)
                    .expect("approval bound fits u16"),
                members,
            )
            .expect("65-of-65 account policy is structurally valid"),
        );
        let provider = ProviderId::new([0xb5; 32]);
        let config = MusubiPublicationServiceConfigurationV1 {
            chain_id: ChainId::from("musubi-impossible-broker-test"),
            genesis_block_hash: [0xb6; 32],
            ingress_broker: broker.clone(),
            seed_provider: provider,
            max_future_clock_skew_ms: 1_000,
            receipt_lifetime_ms: 60_000,
        };
        let journal_binding = MusubiPublicationServiceJournalBindingV1::from_configuration(&config);
        let signer = ThresholdReceiptSigningProvider {
            broker,
            key_pairs,
            behavior: ThresholdSigningBehavior::Correct,
        };
        let (clock, _) = TestPublicationClock::new(1_000);
        let service = MusubiPublicationPrivateServiceV1::new(
            config,
            Box::new(clock),
            Box::new(signer),
            Box::new(ConflictJournal {
                binding: journal_binding,
            }),
            Box::new(RecordingSeedIngress {
                provider,
                calls: Arc::new(Mutex::new(0)),
                fail_first: false,
                clock_after_stage: None,
            }),
            Box::new(UnusedStorage),
            Box::new(UnusedReadback),
        );
        assert!(matches!(
            service,
            Err(MusubiPublicationServiceErrorCodeV1::IdentityMismatch)
        ));
    }

    #[test]
    fn private_service_rejects_trusted_clock_regression() {
        let mut fixture = private_service_fixture(false);
        let metadata = fixture.metadata.clone();
        let car = fixture.car.clone();
        let authorization =
            authorization_header(&fixture.runtime, &fixture.request, &metadata, 3_000);
        let first = seed_http_response(&mut fixture, &authorization, &metadata, &car, 3_001);
        assert_eq!(first.status, 200);

        let regressed_authorization =
            authorization_header(&fixture.runtime, &fixture.request, &metadata, 2_999);
        let regressed = seed_http_response(
            &mut fixture,
            &regressed_authorization,
            &metadata,
            &car,
            3_000,
        );
        let error = decode_service_error(&regressed);
        assert_eq!(regressed.status, 503);
        assert_eq!(
            error.code,
            MusubiPublicationServiceErrorCodeV1::TrustedClockUnavailable
        );
        assert!(error.retryable);

        let recovered_authorization =
            authorization_header(&fixture.runtime, &fixture.request, &metadata, 3_002);
        let recovered = seed_http_response(
            &mut fixture,
            &recovered_authorization,
            &metadata,
            &car,
            3_003,
        );
        assert_eq!(recovered.status, 200);
        assert_eq!(recovered.body, first.body);
        assert_eq!(*fixture.calls.lock().expect("seed calls"), 1);
    }

    #[test]
    fn private_service_classifies_receipt_expiry_overflow_as_clock_failure() {
        let mut fixture = private_service_fixture(false);
        fixture.service.seed_ingress = Box::new(RecordingSeedIngress {
            provider: fixture.request.binding.seed_provider,
            calls: Arc::clone(&fixture.calls),
            fail_first: false,
            clock_after_stage: Some((Arc::clone(&fixture.clock), u64::MAX.saturating_sub(59_999))),
        });
        let metadata = fixture.metadata.clone();
        let car = fixture.car.clone();
        let authorization =
            authorization_header(&fixture.runtime, &fixture.request, &metadata, 4_000);
        let response = seed_http_response(&mut fixture, &authorization, &metadata, &car, 4_001);
        let error = decode_service_error(&response);
        assert_eq!(response.status, 503);
        assert_eq!(
            error.code,
            MusubiPublicationServiceErrorCodeV1::TrustedClockUnavailable
        );
        assert!(!error.retryable);
        assert_eq!(*fixture.calls.lock().expect("seed calls"), 1);
    }

    #[test]
    fn private_service_returns_one_broker_receipt_and_reuses_exact_completed_operation() {
        let mut fixture = private_service_fixture(false);
        let metadata = fixture.metadata.clone();
        let car = fixture.car.clone();
        let first_authorization =
            authorization_header(&fixture.runtime, &fixture.request, &metadata, 1_000);
        let first = seed_http_response(&mut fixture, &first_authorization, &metadata, &car, 1_001);
        assert_eq!(first.status, 200);
        let receipt: MusubiSeedIngressReceiptV1 =
            norito::decode_canonical_with_limits(&first.body, RESPONSE_DECODE_LIMITS)
                .expect("signed receipt");
        receipt
            .verify(&fixture.request.binding, 1_001)
            .expect("receipt binds the exact staged CAR");

        let retry_authorization =
            authorization_header(&fixture.runtime, &fixture.request, &metadata, 1_002);
        let retry = seed_http_response(&mut fixture, &retry_authorization, &metadata, &car, 1_003);
        assert_eq!(retry.status, 200);
        assert_eq!(retry.body, first.body);
        assert_eq!(*fixture.calls.lock().expect("seed calls"), 1);

        let mut substituted = fixture.request.clone();
        substituted.binding.archive_id = ArchiveId::new([0x7a; 32]);
        substituted.binding.nonce = [0x7b; 32];
        let substituted_metadata =
            norito::encode_canonical(&substituted).expect("substituted metadata");
        let substituted_authorization =
            authorization_header(&fixture.runtime, &substituted, &substituted_metadata, 1_004);
        let conflict = seed_http_response(
            &mut fixture,
            &substituted_authorization,
            &substituted_metadata,
            &car,
            1_005,
        );
        assert_eq!(conflict.status, 422);
        assert_eq!(
            decode_service_error(&conflict).code,
            MusubiPublicationServiceErrorCodeV1::OperationConflict
        );
        assert_eq!(*fixture.calls.lock().expect("seed calls"), 1);

        let refresh_authorization =
            authorization_header(&fixture.runtime, &fixture.request, &metadata, 62_000);
        let refreshed = seed_http_response(
            &mut fixture,
            &refresh_authorization,
            &metadata,
            &car,
            62_001,
        );
        assert_eq!(refreshed.status, 200);
        assert_ne!(refreshed.body, first.body);
        let refreshed_receipt: MusubiSeedIngressReceiptV1 =
            norito::decode_canonical_with_limits(&refreshed.body, RESPONSE_DECODE_LIMITS)
                .expect("refreshed receipt");
        refreshed_receipt
            .verify(&fixture.request.binding, 62_001)
            .expect("refreshed receipt is live and exact");
        assert_eq!(*fixture.calls.lock().expect("seed calls"), 2);
    }

    #[test]
    fn private_service_rejects_consumed_authorization_but_accepts_fresh_retry() {
        let mut fixture = private_service_fixture(true);
        let metadata = fixture.metadata.clone();
        let car = fixture.car.clone();
        let authorization =
            authorization_header(&fixture.runtime, &fixture.request, &metadata, 2_000);
        let failed = seed_http_response(&mut fixture, &authorization, &metadata, &car, 2_001);
        let failed_error = decode_service_error(&failed);
        assert_eq!(failed.status, 503);
        assert_eq!(
            failed_error.code,
            MusubiPublicationServiceErrorCodeV1::SeedIngressUnavailable
        );
        assert!(failed_error.retryable);

        let replay = seed_http_response(&mut fixture, &authorization, &metadata, &car, 2_002);
        assert_eq!(replay.status, 401);
        assert_eq!(
            decode_service_error(&replay).code,
            MusubiPublicationServiceErrorCodeV1::AuthorizationReplay
        );
        assert_eq!(*fixture.calls.lock().expect("seed calls"), 1);

        let mut substituted = fixture.request.clone();
        substituted.binding.nonce = [0x7c; 32];
        let substituted_metadata =
            norito::encode_canonical(&substituted).expect("substituted metadata");
        let substituted_authorization =
            authorization_header(&fixture.runtime, &substituted, &substituted_metadata, 2_003);
        let conflict = seed_http_response(
            &mut fixture,
            &substituted_authorization,
            &substituted_metadata,
            &car,
            2_004,
        );
        assert_eq!(conflict.status, 422);
        assert_eq!(
            decode_service_error(&conflict).code,
            MusubiPublicationServiceErrorCodeV1::OperationConflict
        );
        assert_eq!(*fixture.calls.lock().expect("seed calls"), 1);

        let fresh = authorization_header(&fixture.runtime, &fixture.request, &metadata, 2_005);
        let success = seed_http_response(&mut fixture, &fresh, &metadata, &car, 2_006);
        assert_eq!(success.status, 200);
        assert_eq!(*fixture.calls.lock().expect("seed calls"), 2);
    }

    #[test]
    fn private_service_rejects_expiry_signer_metadata_and_car_substitution() {
        let mut fixture = private_service_fixture(false);
        let metadata = fixture.metadata.clone();
        let car = fixture.car.clone();

        let expired_authorization =
            authorization_header(&fixture.runtime, &fixture.request, &metadata, 3_000);
        let expired = seed_http_response(
            &mut fixture,
            &expired_authorization,
            &metadata,
            &car,
            33_001,
        );
        assert_eq!(
            decode_service_error(&expired).code,
            MusubiPublicationServiceErrorCodeV1::AuthorizationExpired
        );

        let future_authorization =
            authorization_header(&fixture.runtime, &fixture.request, &metadata, 50_000);
        let future =
            seed_http_response(&mut fixture, &future_authorization, &metadata, &car, 33_002);
        assert_eq!(
            decode_service_error(&future).code,
            MusubiPublicationServiceErrorCodeV1::AuthorizationExpired
        );

        let valid_authorization =
            authorization_header(&fixture.runtime, &fixture.request, &metadata, 33_003);
        let mut substituted_request = fixture.request.clone();
        substituted_request.binding.nonce = [0x77; 32];
        let substituted_metadata =
            norito::encode_canonical(&substituted_request).expect("substituted metadata");
        let substituted = seed_http_response(
            &mut fixture,
            &valid_authorization,
            &substituted_metadata,
            &car,
            33_004,
        );
        assert_eq!(
            decode_service_error(&substituted).code,
            MusubiPublicationServiceErrorCodeV1::AuthorizationInvalid
        );

        let mut wrong_body = car.clone();
        wrong_body[0] ^= 0x01;
        let wrong_body_response = seed_http_response(
            &mut fixture,
            &valid_authorization,
            &metadata,
            &wrong_body,
            33_004,
        );
        assert_eq!(
            decode_service_error(&wrong_body_response).code,
            MusubiPublicationServiceErrorCodeV1::CarBodyMismatch
        );

        let authorization_bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
            .decode(&valid_authorization)
            .expect("decode authorization");
        let mut wrong_signer: MusubiPublicationRuntimeAuthorizationV1 =
            norito::decode_canonical(&authorization_bytes).expect("authorization");
        let attacker = KeyPair::try_from_seed(
            b"musubi-publication-runtime-attacker".to_vec(),
            Algorithm::Ed25519,
        )
        .expect("attacker key");
        wrong_signer.approvals[0] = MusubiPublicationRuntimeAuthorizationApprovalV1 {
            public_key: attacker.public_key().clone(),
            signature: SignatureOf::try_new(attacker.private_key(), &wrong_signer.payload)
                .expect("attacker signature"),
        };
        let wrong_signer = base64::engine::general_purpose::URL_SAFE_NO_PAD
            .encode(norito::encode_canonical(&wrong_signer).expect("wrong-signer authorization"));
        let wrong_signer_response =
            seed_http_response(&mut fixture, &wrong_signer, &metadata, &car, 33_004);
        assert_eq!(
            decode_service_error(&wrong_signer_response).code,
            MusubiPublicationServiceErrorCodeV1::AuthorizationInvalid
        );
        assert_eq!(*fixture.calls.lock().expect("seed calls"), 0);
    }

    #[test]
    fn private_service_rejects_noncanonical_headers_and_nonexact_routes() {
        let mut fixture = private_service_fixture(false);
        let metadata = fixture.metadata.clone();
        let car = fixture.car.clone();
        let authorization =
            authorization_header(&fixture.runtime, &fixture.request, &metadata, 6_000);
        let encoded_metadata = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(&metadata);
        fixture.clock.store(6_001, Ordering::SeqCst);

        let route = fixture
            .service
            .handle(MusubiPublicationPrivateHttpRequestV1 {
                method: "POST",
                path: "/v1/musubi/publication/seed-ingress/extra",
                content_type: APPLICATION_SORAFS_CAR,
                authorization: Some(&authorization),
                seed_ingress_metadata: Some(&encoded_metadata),
                body: &car,
            });
        assert_eq!(route.status, 404);
        assert_eq!(
            decode_service_error(&route).code,
            MusubiPublicationServiceErrorCodeV1::RouteNotFound
        );

        let method = fixture
            .service
            .handle(MusubiPublicationPrivateHttpRequestV1 {
                method: "GET",
                path: MUSUBI_PUBLICATION_SEED_INGRESS_PATH_V1,
                content_type: APPLICATION_SORAFS_CAR,
                authorization: Some(&authorization),
                seed_ingress_metadata: Some(&encoded_metadata),
                body: &car,
            });
        assert_eq!(method.status, 405);
        assert_eq!(
            decode_service_error(&method).code,
            MusubiPublicationServiceErrorCodeV1::MethodInvalid
        );

        let padded_authorization = format!("{authorization}=");
        let noncanonical = fixture
            .service
            .handle(MusubiPublicationPrivateHttpRequestV1 {
                method: "POST",
                path: MUSUBI_PUBLICATION_SEED_INGRESS_PATH_V1,
                content_type: APPLICATION_SORAFS_CAR,
                authorization: Some(&padded_authorization),
                seed_ingress_metadata: Some(&encoded_metadata),
                body: &car,
            });
        assert_eq!(noncanonical.status, 401);
        assert_eq!(
            decode_service_error(&noncanonical).code,
            MusubiPublicationServiceErrorCodeV1::AuthorizationInvalid
        );

        let malformed_control = fixture
            .service
            .handle(MusubiPublicationPrivateHttpRequestV1 {
                method: "POST",
                path: MUSUBI_PUBLICATION_STORAGE_COORDINATION_PATH_V1,
                content_type: APPLICATION_NORITO,
                authorization: Some(&authorization),
                seed_ingress_metadata: None,
                body: b"not norito",
            });
        assert_eq!(malformed_control.status, 422);
        assert_eq!(
            decode_service_error(&malformed_control).code,
            MusubiPublicationServiceErrorCodeV1::RequestInvalid
        );
    }

    #[test]
    fn private_service_authenticates_control_requests_before_embedded_signatures() {
        let mut fixture = control_service_fixture(false, false);
        let attacker = KeyPair::try_from_seed(
            b"musubi-publication-control-signature-attacker".to_vec(),
            Algorithm::Ed25519,
        )
        .expect("attacker key");

        let mut storage_request = fixture.storage_request.clone();
        let receipt_hash = storage_request.staging_receipt.payload.signing_hash();
        storage_request.staging_receipt.approvals[0].signature =
            SignatureOf::try_from_hash(attacker.private_key(), receipt_hash)
                .expect("mismatched receipt signature");
        let storage_body =
            norito::encode_canonical(&storage_request).expect("storage request bytes");
        fixture.clock.store(2_001, Ordering::SeqCst);
        let anonymous_storage = fixture
            .service
            .handle(MusubiPublicationPrivateHttpRequestV1 {
                method: "POST",
                path: MUSUBI_PUBLICATION_STORAGE_COORDINATION_PATH_V1,
                content_type: APPLICATION_NORITO,
                authorization: None,
                seed_ingress_metadata: None,
                body: &storage_body,
            });
        assert_eq!(anonymous_storage.status, 401);
        assert_eq!(
            decode_service_error(&anonymous_storage).code,
            MusubiPublicationServiceErrorCodeV1::AuthorizationInvalid
        );

        let storage_authorization = control_authorization_header(
            &fixture.runtime,
            MusubiPublicationRuntimeOperationV1::StorageCoordination,
            storage_request.operation_id,
            &storage_body,
            2_000,
        );
        let authenticated_storage = fixture
            .service
            .handle(MusubiPublicationPrivateHttpRequestV1 {
                method: "POST",
                path: MUSUBI_PUBLICATION_STORAGE_COORDINATION_PATH_V1,
                content_type: APPLICATION_NORITO,
                authorization: Some(&storage_authorization),
                seed_ingress_metadata: None,
                body: &storage_body,
            });
        assert_eq!(authenticated_storage.status, 422);
        assert_eq!(
            decode_service_error(&authenticated_storage).code,
            MusubiPublicationServiceErrorCodeV1::RequestInvalid
        );

        let valid_storage_body = norito::encode_canonical(&fixture.storage_request)
            .expect("valid storage request bytes");
        let valid_storage_authorization = control_authorization_header(
            &fixture.runtime,
            MusubiPublicationRuntimeOperationV1::StorageCoordination,
            fixture.storage_request.operation_id,
            &valid_storage_body,
            2_002,
        );
        fixture.clock.store(2_003, Ordering::SeqCst);
        let valid_storage = fixture
            .service
            .handle(MusubiPublicationPrivateHttpRequestV1 {
                method: "POST",
                path: MUSUBI_PUBLICATION_STORAGE_COORDINATION_PATH_V1,
                content_type: APPLICATION_NORITO,
                authorization: Some(&valid_storage_authorization),
                seed_ingress_metadata: None,
                body: &valid_storage_body,
            });
        assert_eq!(valid_storage.status, 200);

        let mut readback_request = fixture.readback_request.clone();
        let attestation = &mut readback_request.location.provider_attestations[0];
        let attestation_hash = attestation.payload.signing_hash();
        attestation.approvals[0].signature =
            SignatureOf::try_from_hash(attacker.private_key(), attestation_hash)
                .expect("mismatched provider signature");
        let readback_body =
            norito::encode_canonical(&readback_request).expect("readback request bytes");
        fixture.clock.store(3_001, Ordering::SeqCst);
        let anonymous_readback = fixture
            .service
            .handle(MusubiPublicationPrivateHttpRequestV1 {
                method: "POST",
                path: MUSUBI_PUBLICATION_PROVIDER_READBACK_PATH_V1,
                content_type: APPLICATION_NORITO,
                authorization: None,
                seed_ingress_metadata: None,
                body: &readback_body,
            });
        assert_eq!(anonymous_readback.status, 401);
        assert_eq!(
            decode_service_error(&anonymous_readback).code,
            MusubiPublicationServiceErrorCodeV1::AuthorizationInvalid
        );

        let readback_authorization = control_authorization_header(
            &fixture.runtime,
            MusubiPublicationRuntimeOperationV1::ProviderReadback,
            readback_request.operation_id,
            &readback_body,
            3_000,
        );
        let authenticated_readback =
            fixture
                .service
                .handle(MusubiPublicationPrivateHttpRequestV1 {
                    method: "POST",
                    path: MUSUBI_PUBLICATION_PROVIDER_READBACK_PATH_V1,
                    content_type: APPLICATION_NORITO,
                    authorization: Some(&readback_authorization),
                    seed_ingress_metadata: None,
                    body: &readback_body,
                });
        assert_eq!(authenticated_readback.status, 422);
        assert_eq!(
            decode_service_error(&authenticated_readback).code,
            MusubiPublicationServiceErrorCodeV1::RequestInvalid
        );
    }

    #[test]
    fn authenticated_staging_receipt_failures_have_exact_deadletter_reasons() {
        let mut invalid_fixture = control_service_fixture(false, false);
        let attacker = KeyPair::try_from_seed(
            b"musubi-publication-invalid-receipt-observer".to_vec(),
            Algorithm::Ed25519,
        )
        .expect("attacker key");
        let mut invalid_request = invalid_fixture.storage_request.clone();
        let receipt_hash = invalid_request.staging_receipt.payload.signing_hash();
        invalid_request.staging_receipt.approvals[0].signature =
            SignatureOf::try_from_hash(attacker.private_key(), receipt_hash)
                .expect("mismatched receipt signature");
        let invalid_body =
            norito::encode_canonical(&invalid_request).expect("invalid receipt request bytes");
        let invalid_authorization = control_authorization_header(
            &invalid_fixture.runtime,
            MusubiPublicationRuntimeOperationV1::StorageCoordination,
            invalid_request.operation_id,
            &invalid_body,
            2_000,
        );
        let invalid_error = invalid_fixture
            .service
            .handle_storage_coordination(
                MusubiPublicationPrivateHttpRequestV1 {
                    method: "POST",
                    path: MUSUBI_PUBLICATION_STORAGE_COORDINATION_PATH_V1,
                    content_type: APPLICATION_NORITO,
                    authorization: Some(&invalid_authorization),
                    seed_ingress_metadata: None,
                    body: &invalid_body,
                },
                2_001,
            )
            .expect_err("authenticated invalid staging receipt");
        assert_eq!(
            invalid_error.telemetry,
            Some(MusubiPublicationServiceTelemetryEventV1::IngestDeadletter(
                MusubiIngestDeadletterReasonV1::ReceiptInvalid,
            ))
        );

        let mut future_fixture = control_service_fixture(false, false);
        future_fixture.service.config.max_future_clock_skew_ms = 1;
        let future_body = norito::encode_canonical(&future_fixture.storage_request)
            .expect("future receipt request bytes");
        let future_authorization = control_authorization_header(
            &future_fixture.runtime,
            MusubiPublicationRuntimeOperationV1::StorageCoordination,
            future_fixture.storage_request.operation_id,
            &future_body,
            2,
        );
        let future_error = future_fixture
            .service
            .handle_storage_coordination(
                MusubiPublicationPrivateHttpRequestV1 {
                    method: "POST",
                    path: MUSUBI_PUBLICATION_STORAGE_COORDINATION_PATH_V1,
                    content_type: APPLICATION_NORITO,
                    authorization: Some(&future_authorization),
                    seed_ingress_metadata: None,
                    body: &future_body,
                },
                2,
            )
            .expect_err("authenticated future-skewed staging receipt");
        assert_eq!(
            future_error.telemetry,
            Some(MusubiPublicationServiceTelemetryEventV1::IngestDeadletter(
                MusubiIngestDeadletterReasonV1::ReceiptInvalid,
            ))
        );
    }

    #[test]
    fn storage_coordination_accepts_an_expired_receipt_for_the_exact_finalized_archive() {
        let mut fixture = control_service_fixture(false, false);
        let body = norito::encode_canonical(&fixture.storage_request)
            .expect("expired finalized archive request bytes");
        let authorization = control_authorization_header(
            &fixture.runtime,
            MusubiPublicationRuntimeOperationV1::StorageCoordination,
            fixture.storage_request.operation_id,
            &body,
            120_000,
        );
        let response = fixture
            .service
            .handle_storage_coordination(
                MusubiPublicationPrivateHttpRequestV1 {
                    method: "POST",
                    path: MUSUBI_PUBLICATION_STORAGE_COORDINATION_PATH_V1,
                    content_type: APPLICATION_NORITO,
                    authorization: Some(&authorization),
                    seed_ingress_metadata: None,
                    body: &body,
                },
                120_001,
            )
            .expect("finalized archive outlives its registration receipt");
        let decoded: MusubiStorageCoordinationResponseV1 =
            norito::decode_canonical_with_limits(&response, RESPONSE_DECODE_LIMITS)
                .expect("storage response");
        assert_eq!(decoded, fixture.storage_response);
    }

    #[test]
    fn private_service_accepts_exact_storage_and_provider_readback_evidence() {
        let mut fixture = control_service_fixture(false, false);
        let storage_body =
            norito::encode_canonical(&fixture.storage_request).expect("storage request bytes");
        let storage_authorization = control_authorization_header(
            &fixture.runtime,
            MusubiPublicationRuntimeOperationV1::StorageCoordination,
            fixture.storage_request.operation_id,
            &storage_body,
            2_000,
        );
        fixture.clock.store(2_001, Ordering::SeqCst);
        let storage = fixture
            .service
            .handle(MusubiPublicationPrivateHttpRequestV1 {
                method: "POST",
                path: MUSUBI_PUBLICATION_STORAGE_COORDINATION_PATH_V1,
                content_type: APPLICATION_NORITO,
                authorization: Some(&storage_authorization),
                seed_ingress_metadata: None,
                body: &storage_body,
            });
        assert_eq!(storage.status, 200);
        let storage_response: MusubiStorageCoordinationResponseV1 =
            norito::decode_canonical_with_limits(&storage.body, RESPONSE_DECODE_LIMITS)
                .expect("storage response");
        assert_eq!(storage_response, fixture.storage_response);

        let readback_body =
            norito::encode_canonical(&fixture.readback_request).expect("readback request bytes");
        let readback_authorization = control_authorization_header(
            &fixture.runtime,
            MusubiPublicationRuntimeOperationV1::ProviderReadback,
            fixture.readback_request.operation_id,
            &readback_body,
            3_000,
        );
        fixture.clock.store(3_001, Ordering::SeqCst);
        let readback = fixture
            .service
            .handle(MusubiPublicationPrivateHttpRequestV1 {
                method: "POST",
                path: MUSUBI_PUBLICATION_PROVIDER_READBACK_PATH_V1,
                content_type: APPLICATION_NORITO,
                authorization: Some(&readback_authorization),
                seed_ingress_metadata: None,
                body: &readback_body,
            });
        assert_eq!(readback.status, 200);
        let readback_response: MusubiProviderReadbackResponseV1 =
            norito::decode_canonical_with_limits(&readback.body, RESPONSE_DECODE_LIMITS)
                .expect("readback response");
        assert_eq!(readback_response, fixture.readback_response);
    }

    #[test]
    fn storage_response_rejects_a_refreshed_receipt_after_registration() {
        let fixture = control_service_fixture(false, false);
        let broker_key = KeyPair::try_from_seed(
            b"musubi-publication-control-broker".to_vec(),
            Algorithm::Ed25519,
        )
        .expect("broker key");
        let mut request = fixture.storage_request.clone();
        let payload = MusubiSeedIngressReceiptPayloadV1 {
            version: 1,
            binding: request.staging_receipt.payload.binding.clone(),
            issued_at_ms: request.staging_receipt.payload.expires_at_ms + 1,
            expires_at_ms: request.staging_receipt.payload.expires_at_ms + 60_001,
        };
        request.staging_receipt = MusubiSeedIngressReceiptV1 {
            approvals: vec![MusubiSeedIngressReceiptApprovalV1 {
                public_key: broker_key.public_key().clone(),
                signature: SignatureOf::try_from_hash(
                    broker_key.private_key(),
                    payload.signing_hash(),
                )
                .expect("refreshed receipt signature"),
            }],
            payload,
        };
        assert_ne!(
            fixture.storage_response.archive.staging_receipt,
            request.staging_receipt
        );
        assert!(fixture.storage_response.validate_for(&request).is_err());

        request.staging_receipt.payload.binding.nonce = [0xee; 32];
        request.staging_receipt.approvals[0].signature = SignatureOf::try_from_hash(
            broker_key.private_key(),
            request.staging_receipt.payload.signing_hash(),
        )
        .expect("different-operation receipt signature");
        assert!(fixture.storage_response.validate_for(&request).is_err());
    }

    #[test]
    fn storage_response_requires_replication_quorum_and_exact_lock_digest() {
        let fixture = control_service_fixture(false, false);
        let mut below_quorum = fixture.storage_response.clone();
        let MusubiStorageLocationDispositionV1::NeedsRegistration {
            provider_attestations,
            ..
        } = &mut below_quorum.disposition
        else {
            panic!("fixture requires location registration")
        };
        provider_attestations
            .truncate(usize::from(MUSUBI_MIN_HEALTHY_REPLICAS_V1).saturating_sub(1));
        assert!(below_quorum.validate_for(&fixture.storage_request).is_err());

        let mut wrong_lock = fixture.storage_request.clone();
        wrong_lock.verification_lock_digest = MusubiVerificationLockDigestV1::new([0xee; 32]);
        assert!(fixture.storage_response.validate_for(&wrong_lock).is_err());
    }

    #[test]
    fn storage_location_generations_have_distinct_bounded_journal_targets() {
        let first = storage_generation_target(1);
        let last = storage_generation_target(
            u8::try_from(MUSUBI_MAX_PUBLICATION_LOCATION_ATTEMPTS_V1)
                .expect("location generation bound fits u8"),
        );
        assert_ne!(first, last);
        assert!(valid_storage_generation_target(first));
        assert!(valid_storage_generation_target(last));
        assert!(!valid_storage_generation_target([0; 32]));
        assert!(!valid_storage_generation_target(storage_generation_target(
            u8::try_from(MUSUBI_MAX_PUBLICATION_LOCATION_ATTEMPTS_V1 + 1)
                .expect("one past the bound fits u8"),
        )));
        let mut malformed = first;
        malformed[31] = 1;
        assert!(!valid_storage_generation_target(malformed));
    }

    #[test]
    fn storage_response_never_reuses_a_prior_location_generation() {
        let fixture = control_service_fixture(false, false);
        let retired_location_id = fixture.storage_response.location_id;
        let mut replacement_request = fixture.storage_request.clone();
        replacement_request.generation = 2;
        replacement_request.prior_location_ids = vec![retired_location_id];
        replacement_request
            .validate()
            .expect("a sorted second generation is structurally valid");
        assert!(
            fixture
                .storage_response
                .validate_for(&replacement_request)
                .is_err(),
            "the coordinator cannot return a retired stable identity"
        );

        let mut replacement = fixture.storage_response;
        replacement.location_id = MusubiArchiveLocationIdV1::new([0xee; 32]);
        replacement
            .validate_for(&replacement_request)
            .expect("a never-before-used replacement identity remains valid");

        let mut unsorted_third = replacement_request;
        unsorted_third.generation = 3;
        unsorted_third.prior_location_ids = vec![
            MusubiArchiveLocationIdV1::new([0xff; 32]),
            retired_location_id,
        ];
        assert!(unsorted_third.validate().is_err());
    }

    #[test]
    fn storage_request_binds_immutable_registration_without_freezing_location_state() {
        let fixture = control_service_fixture(false, false);

        let mut wrong_height = fixture.storage_request.clone();
        wrong_height
            .finalized_registration
            .registration
            .registered_at_height += 1;
        wrong_height.validate().expect(
            "a different internally valid projection is digest-bound but must be authorized upstream",
        );
        let exact = norito::encode_canonical(&fixture.storage_request).expect("exact request");
        let substituted = norito::encode_canonical(&wrong_height).expect("substituted request");
        assert_ne!(
            request_digest(
                MusubiPublicationRuntimeOperationV1::StorageCoordination,
                &exact,
            )
            .expect("exact request digest"),
            request_digest(
                MusubiPublicationRuntimeOperationV1::StorageCoordination,
                &substituted,
            )
            .expect("substituted request digest")
        );
        assert!(
            fixture
                .storage_response
                .validate_for(&wrong_height)
                .is_err()
        );

        let mut wrong_registrant = fixture.storage_request.clone();
        let other = KeyPair::try_from_seed(
            b"musubi-storage-authoritative-record-substitution".to_vec(),
            Algorithm::Ed25519,
        )
        .expect("other registrant key");
        wrong_registrant
            .finalized_registration
            .registration
            .registered_by = AccountId::new(other.public_key().clone());
        assert!(wrong_registrant.validate().is_err());

        let mut missing_transaction = fixture.storage_request.clone();
        missing_transaction.finalized_registration.transaction_hash = [0; 32];
        assert!(missing_transaction.validate().is_err());

        let mut wrong_lock = fixture.storage_request.clone();
        wrong_lock.verification_lock_digest = MusubiVerificationLockDigestV1::new([0xee; 32]);
        wrong_lock
            .validate()
            .expect("a nonzero lock digest remains structurally valid");
        assert!(fixture.storage_response.validate_for(&wrong_lock).is_err());
        wrong_lock.verification_lock_digest = MusubiVerificationLockDigestV1::new([0; 32]);
        assert!(wrong_lock.validate().is_err());

        let mut pre_registration_snapshot = fixture.storage_request.clone();
        pre_registration_snapshot
            .finalized_registration
            .snapshot
            .finalized_height = pre_registration_snapshot
            .finalized_registration
            .registration
            .registered_at_height
            .saturating_sub(1);
        assert!(pre_registration_snapshot.validate().is_err());

        let mut later_current_archive = fixture.storage_response;
        later_current_archive.archive.location_revision = 2;
        later_current_archive.archive.location_ids = vec![MusubiArchiveLocationIdV1::new([
            0xdd; 32,
        ])];
        let MusubiStorageLocationDispositionV1::NeedsRegistration {
            expected_location_revision,
            ..
        } = &mut later_current_archive.disposition
        else {
            panic!("fixture requires location registration")
        };
        *expected_location_revision = 2;
        later_current_archive
            .validate_for(&fixture.storage_request)
            .expect("a later finalized location directory preserves registration evidence");
    }

    #[test]
    fn private_service_rejects_substituted_storage_and_readback_backend_evidence() {
        let mut fixture = control_service_fixture(true, true);
        let storage_body =
            norito::encode_canonical(&fixture.storage_request).expect("storage request bytes");
        let storage_authorization = control_authorization_header(
            &fixture.runtime,
            MusubiPublicationRuntimeOperationV1::StorageCoordination,
            fixture.storage_request.operation_id,
            &storage_body,
            4_000,
        );
        fixture.clock.store(4_001, Ordering::SeqCst);
        let storage = fixture
            .service
            .handle(MusubiPublicationPrivateHttpRequestV1 {
                method: "POST",
                path: MUSUBI_PUBLICATION_STORAGE_COORDINATION_PATH_V1,
                content_type: APPLICATION_NORITO,
                authorization: Some(&storage_authorization),
                seed_ingress_metadata: None,
                body: &storage_body,
            });
        assert_eq!(storage.status, 422);
        assert_eq!(
            decode_service_error(&storage).code,
            MusubiPublicationServiceErrorCodeV1::BackendResponseInvalid
        );

        let readback_body =
            norito::encode_canonical(&fixture.readback_request).expect("readback request bytes");
        let readback_authorization = control_authorization_header(
            &fixture.runtime,
            MusubiPublicationRuntimeOperationV1::ProviderReadback,
            fixture.readback_request.operation_id,
            &readback_body,
            5_000,
        );
        fixture.clock.store(5_001, Ordering::SeqCst);
        let readback = fixture
            .service
            .handle(MusubiPublicationPrivateHttpRequestV1 {
                method: "POST",
                path: MUSUBI_PUBLICATION_PROVIDER_READBACK_PATH_V1,
                content_type: APPLICATION_NORITO,
                authorization: Some(&readback_authorization),
                seed_ingress_metadata: None,
                body: &readback_body,
            });
        assert_eq!(readback.status, 422);
        assert_eq!(
            decode_service_error(&readback).code,
            MusubiPublicationServiceErrorCodeV1::BackendResponseInvalid
        );
    }

    #[test]
    fn restored_journal_preserves_completed_idempotency_and_replay_state() {
        let (client, _) = client();
        let binding = MusubiPublicationOperationBindingV1 {
            operation_id: [0x81; 32],
            chain_id: client.chain,
            genesis_block_hash: [0x80; 32],
            publisher: client.account,
            archive_id: ArchiveId::new([0x82; 32]),
            car_body_digest: MusubiContentDigestV1::new([0x83; 32]),
            car_body_length: 99,
        };
        let journal_binding = MusubiPublicationServiceJournalBindingV1 {
            chain_id: binding.chain_id.clone(),
            genesis_block_hash: binding.genesis_block_hash,
            ingress_broker: binding.publisher.clone(),
            seed_provider: ProviderId::new([0x88; 32]),
        };
        let mut journal =
            InMemoryMusubiPublicationServiceJournalV1::new(journal_binding, 8, 8).expect("journal");
        let attempt = MusubiPublicationJournalAttemptV1 {
            key: MusubiPublicationIdempotencyKeyV1 {
                operation: MusubiPublicationRuntimeOperationV1::SeedIngress,
                operation_id: binding.operation_id,
                target: [0; 32],
            },
            binding: binding.clone(),
            request_digest: [0x84; 32],
            authorization_digest: [0x85; 32],
            authorization_expires_at_ms: 20_000,
        };
        assert_eq!(
            journal.begin(&attempt, 10_000).expect("reserve"),
            MusubiPublicationJournalBeginV1::Execute
        );
        journal
            .commit(attempt.key, attempt.request_digest, b"canonical response")
            .expect("commit");

        // Cloning models restoring the same durable records into a replacement service process.
        let mut restored = journal.clone();
        let mut retry = attempt.clone();
        retry.authorization_digest = [0x86; 32];
        assert_eq!(
            restored.begin(&retry, 10_001).expect("cached retry"),
            MusubiPublicationJournalBeginV1::Cached(b"canonical response".to_vec())
        );
        assert_eq!(
            restored.begin(&retry, 10_001).expect("cached replay"),
            MusubiPublicationJournalBeginV1::Cached(b"canonical response".to_vec())
        );

        let mut conflict = retry.clone();
        conflict.binding.archive_id = ArchiveId::new([0x87; 32]);
        assert_eq!(
            restored.begin(&conflict, 10_002),
            Err(MusubiPublicationServiceJournalErrorV1::Conflict)
        );

        let mut reset_conflict = retry.clone();
        reset_conflict.binding.genesis_block_hash = [0x8a; 32];
        assert_eq!(
            restored.begin(&reset_conflict, 10_002),
            Err(MusubiPublicationServiceJournalErrorV1::Invalid)
        );

        let replay = MusubiPublicationJournalAttemptV1 {
            key: MusubiPublicationIdempotencyKeyV1 {
                operation: MusubiPublicationRuntimeOperationV1::ProviderReadback,
                operation_id: binding.operation_id,
                target: [0x88; 32],
            },
            binding,
            request_digest: [0x89; 32],
            authorization_digest: attempt.authorization_digest,
            authorization_expires_at_ms: attempt.authorization_expires_at_ms,
        };
        assert_eq!(
            restored.begin(&replay, 10_003),
            Err(MusubiPublicationServiceJournalErrorV1::Replay)
        );
    }
}
