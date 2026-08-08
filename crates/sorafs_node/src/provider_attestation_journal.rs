//! Durable, approval-only handoff for completed Musubi provider attestations.
//!
//! This module is intentionally inert: it defines the isolated signer,
//! content-addressed journal, and idempotent coordinator-inventory contracts,
//! but no daemon launcher or transaction ingress. An approval intent retains
//! the exact finalized evidence identity while deliberately omitting the
//! opaque approval request. After restart, callers must rederive that request
//! from a fresh completed-row claim and lifecycle-leased bundle verification.

use std::{collections::BTreeSet, sync::Arc};

use iroha_config::parameters::{
    defaults::sorafs::storage::provider_ingest_runtime::outbox as provider_ingest_outbox_defaults,
    is_production_runtime_handle,
};
use iroha_data_model::{
    ChainId,
    account::AccountId,
    musubi::{
        MUSUBI_MAX_LOCATION_PROVIDERS_V1, MusubiProviderBundleAttestationDigestV1,
        MusubiProviderBundleAttestationKeyV1, MusubiProviderBundleVerificationAttestationV1,
        MusubiProviderBundleVerificationBindingV1, MusubiProviderBundleVerificationPayloadV1,
    },
    sorafs::pin_registry::{ProviderIngestCompletionSignerPolicyV1, ReplicationOrderId},
};
use norito::{
    DecodeLimits,
    derive::{NoritoDeserialize, NoritoSerialize},
};
use thiserror::Error;

use crate::{
    ProviderIngestFinalizedCursorV1, ProviderIngestFutureV1,
    ProviderIngestMusubiAttestationApprovalRequestV1,
};

const APPROVAL_SIGNER_QUALIFICATION_VERSION_V1: u8 = 1;
const JOURNAL_CHECKPOINT_VERSION_V1: u8 = 1;
const APPROVAL_ID_DOMAIN_V1: &[u8] = b"sorafs.musubi.provider-attestation.approval.v1\0";
const INVENTORY_HANDOFF_ID_DOMAIN_V1: &[u8] =
    b"sorafs.musubi.provider-attestation.inventory-handoff.v1\0";
const JOURNAL_CHECKPOINT_REVISION_DOMAIN_V1: &[u8] =
    b"sorafs.musubi.provider-attestation.journal-checkpoint.v1\0";
/// Hard upper bound admitted by the journal decoder regardless of runtime policy.
pub const MUSUBI_PROVIDER_ATTESTATION_JOURNAL_CHECKPOINT_MAX_BYTES_V1: usize = 128 * 1024 * 1024;
/// Hard upper bound on journal entries regardless of runtime policy.
pub const MUSUBI_PROVIDER_ATTESTATION_JOURNAL_MAX_ENTRIES_V1: usize = 4_096;

const JOURNAL_CHECKPOINT_DECODE_LIMITS_V1: DecodeLimits = DecodeLimits::new(
    MUSUBI_PROVIDER_ATTESTATION_JOURNAL_MAX_ENTRIES_V1 * 8,
    MUSUBI_PROVIDER_ATTESTATION_JOURNAL_CHECKPOINT_MAX_BYTES_V1,
    MUSUBI_PROVIDER_ATTESTATION_JOURNAL_MAX_ENTRIES_V1 * 64,
    MUSUBI_PROVIDER_ATTESTATION_JOURNAL_CHECKPOINT_MAX_BYTES_V1 * 4,
    128,
);

/// Payload-free public qualification of an approval-only attestation signer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MusubiProviderAttestationSignerQualificationV1 {
    /// Qualification schema version.
    pub version: u8,
    /// Monotonic adapter and public key-set revision.
    pub adapter_revision: u64,
    /// Exact governed signer policy currently implemented by the adapter.
    pub signer_policy: ProviderIngestCompletionSignerPolicyV1,
    /// Provider-owner account whose controller quorum the adapter represents.
    pub authority: AccountId,
    /// Non-zero digest of the adapter's exact public controller-key configuration.
    pub controller_policy_digest: [u8; 32],
}

impl MusubiProviderAttestationSignerQualificationV1 {
    /// Construct a first-release approval-only signer qualification.
    #[must_use]
    pub const fn new(
        adapter_revision: u64,
        signer_policy: ProviderIngestCompletionSignerPolicyV1,
        authority: AccountId,
        controller_policy_digest: [u8; 32],
    ) -> Self {
        Self {
            version: APPROVAL_SIGNER_QUALIFICATION_VERSION_V1,
            adapter_revision,
            signer_policy,
            authority,
            controller_policy_digest,
        }
    }

    /// Validate bounded schema, revision, policy lineage, and key-set identity.
    ///
    /// # Errors
    ///
    /// Returns an error for an unsupported schema or inert identity component.
    pub fn validate(&self) -> Result<(), MusubiProviderAttestationSignerBindingErrorV1> {
        if self.version != APPROVAL_SIGNER_QUALIFICATION_VERSION_V1
            || self.adapter_revision == 0
            || !self.signer_policy.is_valid()
            || self.controller_policy_digest == [0; 32]
        {
            return Err(MusubiProviderAttestationSignerBindingErrorV1::InvalidQualification);
        }
        Ok(())
    }
}

/// Invalid public binding exposed by an approval-only provider signer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum MusubiProviderAttestationSignerBindingErrorV1 {
    /// The stable signer handle is malformed or marked for test/development use.
    #[error("Musubi provider-attestation signer handle is not production-safe")]
    InvalidRuntimeHandle,
    /// The signer qualification is malformed or inert.
    #[error("Musubi provider-attestation signer qualification is invalid")]
    InvalidQualification,
}

/// Bounded failure returned by an isolated approval-only signer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum MusubiProviderAttestationSignerErrorV1 {
    /// The qualified signer is temporarily unavailable.
    #[error("Musubi provider-attestation signer is unavailable")]
    Unavailable,
    /// The exact request or live policy is no longer eligible.
    #[error("Musubi provider-attestation request was rejected")]
    Rejected,
}

/// Isolated signer capable only of producing complete Musubi provider attestations.
///
/// The trait intentionally has no transaction payload, signed transaction,
/// queue, or registry-submission surface.
pub trait MusubiProviderAttestationSignerV1: Send + Sync + 'static {
    /// Return the stable credential-free HSM/KMS adapter identity.
    fn runtime_handle(&self) -> &str;

    /// Return the provider-owner account controlled by this signer.
    fn authority(&self) -> &AccountId;

    /// Return an independently authenticated, payload-free qualification.
    fn qualification(
        &self,
    ) -> Result<
        MusubiProviderAttestationSignerQualificationV1,
        MusubiProviderAttestationSignerErrorV1,
    >;

    /// Return the signer's configured governed policy identity.
    fn signer_policy(&self) -> ProviderIngestCompletionSignerPolicyV1;

    /// Revalidate the locally maintained live eligibility snapshot.
    ///
    /// This must be a bounded, non-blocking read. The timed approval operation
    /// remains responsible for the signer-side atomic revocation check.
    fn current_eligibility(
        &self,
    ) -> Result<ProviderIngestCompletionSignerPolicyV1, MusubiProviderAttestationSignerErrorV1>;

    /// Approve only the supplied opaque, post-completion request.
    fn approve<'a>(
        &'a self,
        request: &'a ProviderIngestMusubiAttestationApprovalRequestV1,
    ) -> ProviderIngestFutureV1<
        'a,
        Result<
            MusubiProviderBundleVerificationAttestationV1,
            MusubiProviderAttestationSignerErrorV1,
        >,
    >;
}

/// Coordinator-side failure while invoking an approval-only signer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum MusubiProviderAttestationApprovalErrorV1 {
    /// The opaque request is internally inconsistent.
    #[error("Musubi provider-attestation approval request is invalid")]
    InvalidRequest,
    /// The configured or injected signer handle is not production-safe.
    #[error("Musubi provider-attestation signer handle is invalid")]
    InvalidSignerHandle,
    /// The signer qualification or authority contradicts the request.
    #[error("Musubi provider-attestation signer qualification is inconsistent")]
    InvalidSignerQualification,
    /// The signer was unavailable before or during the approval operation.
    #[error("Musubi provider-attestation signer is unavailable")]
    SignerUnavailable,
    /// The signer rejected the exact request.
    #[error("Musubi provider-attestation signer rejected the request")]
    SignerRejected,
    /// Eligibility or qualification changed across the signing boundary.
    #[error("Musubi provider-attestation signer eligibility changed")]
    EligibilityChanged,
    /// The signer returned a different payload or an invalid approval quorum.
    #[error("Musubi provider-attestation signer returned invalid evidence")]
    InvalidAttestation,
}

/// Validate, invoke, and revalidate one approval-only provider signer.
///
/// The coordinator checks the production handle, exact provider-owner
/// authority, governed policy, and eligibility both before and after signing.
/// It then requires byte-for-byte payload equality and verifies the complete
/// controller approval quorum.
///
/// # Errors
///
/// Returns a closed failure when the request, signer binding, live policy, or
/// resulting attestation is unavailable, rejected, substituted, or invalid.
pub async fn approve_musubi_provider_attestation_v1<Signer>(
    signer: &Signer,
    request: &ProviderIngestMusubiAttestationApprovalRequestV1,
) -> Result<MusubiProviderBundleVerificationAttestationV1, MusubiProviderAttestationApprovalErrorV1>
where
    Signer: MusubiProviderAttestationSignerV1 + ?Sized,
{
    validate_approval_request(request)?;
    let handle_before = signer.runtime_handle().to_owned();
    if !is_production_runtime_handle(&handle_before) {
        return Err(MusubiProviderAttestationApprovalErrorV1::InvalidSignerHandle);
    }
    let qualification_before = qualified_signer_snapshot(signer, request)?;

    let attestation = signer.approve(request).await.map_err(|error| match error {
        MusubiProviderAttestationSignerErrorV1::Unavailable => {
            MusubiProviderAttestationApprovalErrorV1::SignerUnavailable
        }
        MusubiProviderAttestationSignerErrorV1::Rejected => {
            MusubiProviderAttestationApprovalErrorV1::SignerRejected
        }
    })?;

    if signer.runtime_handle() != handle_before
        || !is_production_runtime_handle(signer.runtime_handle())
    {
        return Err(MusubiProviderAttestationApprovalErrorV1::EligibilityChanged);
    }
    let qualification_after = qualified_signer_snapshot(signer, request).map_err(|error| {
        if error == MusubiProviderAttestationApprovalErrorV1::SignerUnavailable {
            error
        } else {
            MusubiProviderAttestationApprovalErrorV1::EligibilityChanged
        }
    })?;
    if qualification_after != qualification_before {
        return Err(MusubiProviderAttestationApprovalErrorV1::EligibilityChanged);
    }
    if &attestation.payload != request.payload()
        || attestation.verify(&request.payload().binding).is_err()
    {
        return Err(MusubiProviderAttestationApprovalErrorV1::InvalidAttestation);
    }
    Ok(attestation)
}

fn validate_approval_request(
    request: &ProviderIngestMusubiAttestationApprovalRequestV1,
) -> Result<(), MusubiProviderAttestationApprovalErrorV1> {
    let cursor = request.observed_finalized_cursor();
    if request.payload().validate().is_err()
        || request.completion_claim_digest() == [0; 32]
        || cursor.height == 0
        || cursor.block_hash == [0; 32]
        || !request.signer_policy().is_valid()
        || request.payload().binding.completion_authority.signer_policy != request.signer_policy()
        || request.payload().binding.completed_by
            != request
                .payload()
                .binding
                .completion_authority
                .provider_owner
    {
        return Err(MusubiProviderAttestationApprovalErrorV1::InvalidRequest);
    }
    Ok(())
}

fn qualified_signer_snapshot<Signer>(
    signer: &Signer,
    request: &ProviderIngestMusubiAttestationApprovalRequestV1,
) -> Result<MusubiProviderAttestationSignerQualificationV1, MusubiProviderAttestationApprovalErrorV1>
where
    Signer: MusubiProviderAttestationSignerV1 + ?Sized,
{
    let qualification = signer.qualification().map_err(|error| match error {
        MusubiProviderAttestationSignerErrorV1::Unavailable => {
            MusubiProviderAttestationApprovalErrorV1::SignerUnavailable
        }
        MusubiProviderAttestationSignerErrorV1::Rejected => {
            MusubiProviderAttestationApprovalErrorV1::SignerRejected
        }
    })?;
    qualification
        .validate()
        .map_err(|_| MusubiProviderAttestationApprovalErrorV1::InvalidSignerQualification)?;
    let eligibility = signer.current_eligibility().map_err(|error| match error {
        MusubiProviderAttestationSignerErrorV1::Unavailable => {
            MusubiProviderAttestationApprovalErrorV1::SignerUnavailable
        }
        MusubiProviderAttestationSignerErrorV1::Rejected => {
            MusubiProviderAttestationApprovalErrorV1::SignerRejected
        }
    })?;
    let expected_owner = &request.payload().binding.completed_by;
    let expected_policy = request.signer_policy();
    if signer.authority() != expected_owner
        || &qualification.authority != expected_owner
        || qualification.signer_policy != expected_policy
        || signer.signer_policy() != expected_policy
        || eligibility != expected_policy
    {
        return Err(MusubiProviderAttestationApprovalErrorV1::InvalidSignerQualification);
    }
    Ok(qualification)
}

/// Stable domain-separated identity of one exact approval intent.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, NoritoSerialize, NoritoDeserialize,
)]
pub struct MusubiProviderAttestationApprovalIdV1([u8; 32]);

impl MusubiProviderAttestationApprovalIdV1 {
    /// Return exact approval-identity bytes.
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    fn is_valid(self) -> bool {
        self.0 != [0; 32]
    }
}

/// Stable domain-separated identity of one coordinator-inventory handoff.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, NoritoSerialize, NoritoDeserialize,
)]
pub struct MusubiProviderAttestationInventoryHandoffIdV1([u8; 32]);

impl MusubiProviderAttestationInventoryHandoffIdV1 {
    /// Return exact inventory-handoff identity bytes.
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    fn is_valid(self) -> bool {
        self.0 != [0; 32]
    }
}

/// Deployment and archive/order scope of a coordinator attestation inventory.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, NoritoSerialize, NoritoDeserialize)]
pub struct MusubiProviderAttestationInventoryScopeV1 {
    /// Exact deployment-selected chain identity.
    pub chain_id: ChainId,
    /// Exact genesis block hash for that deployment.
    pub genesis_block_hash: [u8; 32],
    /// Exact canonical archive identity.
    pub archive_id: iroha_data_model::musubi::ArchiveId,
    /// Exact replication order shared by the inventory.
    pub replication_order: ReplicationOrderId,
}

impl MusubiProviderAttestationInventoryScopeV1 {
    /// Derive the scope selected by one exact verified binding.
    #[must_use]
    pub fn from_binding(binding: &MusubiProviderBundleVerificationBindingV1) -> Self {
        Self {
            chain_id: binding.chain_id.clone(),
            genesis_block_hash: binding.genesis_block_hash,
            archive_id: binding.archive_id,
            replication_order: binding.replication_order,
        }
    }

    /// Validate every scope identity component.
    ///
    /// # Errors
    ///
    /// Returns an error for an inert genesis, archive, or order identity.
    pub fn validate(&self) -> Result<(), MusubiProviderAttestationInventoryErrorV1> {
        if self.chain_id.as_str().is_empty()
            || self.chain_id.as_str().len()
                > provider_ingest_outbox_defaults::COMPLETION_CHAIN_ID_MAX_BYTES_V1
            || self.genesis_block_hash == [0; 32]
            || self.archive_id.is_zero()
            || self.replication_order.as_bytes() == &[0; 32]
        {
            return Err(MusubiProviderAttestationInventoryErrorV1::InvalidItem);
        }
        Ok(())
    }
}

/// One immutable full attestation prepared for idempotent coordinator handoff.
#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
pub struct MusubiProviderAttestationInventoryItemV1 {
    scope: MusubiProviderAttestationInventoryScopeV1,
    key: MusubiProviderBundleAttestationKeyV1,
    attestation_digest: MusubiProviderBundleAttestationDigestV1,
    attestation: MusubiProviderBundleVerificationAttestationV1,
    handoff_id: MusubiProviderAttestationInventoryHandoffIdV1,
}

impl MusubiProviderAttestationInventoryItemV1 {
    /// Construct a canonical item from one complete verified attestation.
    ///
    /// # Errors
    ///
    /// Returns an error when the attestation, scope, identity, or digest is invalid.
    pub fn new(
        attestation: MusubiProviderBundleVerificationAttestationV1,
    ) -> Result<Self, MusubiProviderAttestationInventoryErrorV1> {
        attestation
            .verify(&attestation.payload.binding)
            .map_err(|_| MusubiProviderAttestationInventoryErrorV1::InvalidItem)?;
        let scope =
            MusubiProviderAttestationInventoryScopeV1::from_binding(&attestation.payload.binding);
        let key = attestation.key();
        let attestation_digest = attestation.digest();
        let handoff_id =
            musubi_provider_attestation_inventory_handoff_id_v1(&scope, key, attestation_digest)?;
        let item = Self {
            scope,
            key,
            attestation_digest,
            attestation,
            handoff_id,
        };
        item.validate()?;
        Ok(item)
    }

    /// Return the exact deployment/archive/order inventory scope.
    #[must_use]
    pub const fn scope(&self) -> &MusubiProviderAttestationInventoryScopeV1 {
        &self.scope
    }

    /// Return the immutable archive/order/provider key.
    #[must_use]
    pub const fn key(&self) -> MusubiProviderBundleAttestationKeyV1 {
        self.key
    }

    /// Return the digest of the complete canonical attestation.
    #[must_use]
    pub const fn attestation_digest(&self) -> MusubiProviderBundleAttestationDigestV1 {
        self.attestation_digest
    }

    /// Borrow the complete signed provider attestation.
    #[must_use]
    pub const fn attestation(&self) -> &MusubiProviderBundleVerificationAttestationV1 {
        &self.attestation
    }

    /// Return the stable idempotency identity for this handoff.
    #[must_use]
    pub const fn handoff_id(&self) -> MusubiProviderAttestationInventoryHandoffIdV1 {
        self.handoff_id
    }

    /// Validate scope, exact attestation projection, digest, and handoff identity.
    ///
    /// # Errors
    ///
    /// Returns an error if any field is malformed or substituted.
    pub fn validate(&self) -> Result<(), MusubiProviderAttestationInventoryErrorV1> {
        self.scope.validate()?;
        self.key
            .validate()
            .map_err(|_| MusubiProviderAttestationInventoryErrorV1::InvalidItem)?;
        self.attestation
            .verify(&self.attestation.payload.binding)
            .map_err(|_| MusubiProviderAttestationInventoryErrorV1::InvalidItem)?;
        let expected_scope = MusubiProviderAttestationInventoryScopeV1::from_binding(
            &self.attestation.payload.binding,
        );
        let expected_handoff = musubi_provider_attestation_inventory_handoff_id_v1(
            &self.scope,
            self.key,
            self.attestation_digest,
        )?;
        if self.scope != expected_scope
            || self.key != self.attestation.key()
            || self.attestation_digest != self.attestation.digest()
            || !self.handoff_id.is_valid()
            || self.handoff_id != expected_handoff
        {
            return Err(MusubiProviderAttestationInventoryErrorV1::InvalidItem);
        }
        Ok(())
    }
}

/// Bounded acknowledgement for one immutable inventory handoff.
#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
pub struct MusubiProviderAttestationInventoryReceiptV1 {
    scope: MusubiProviderAttestationInventoryScopeV1,
    key: MusubiProviderBundleAttestationKeyV1,
    attestation_digest: MusubiProviderBundleAttestationDigestV1,
    handoff_id: MusubiProviderAttestationInventoryHandoffIdV1,
    inventory_revision: u64,
}

impl MusubiProviderAttestationInventoryReceiptV1 {
    /// Construct an acknowledgement bound to one exact immutable item.
    ///
    /// # Errors
    ///
    /// Returns an error for an invalid item or zero inventory revision.
    pub fn new(
        item: &MusubiProviderAttestationInventoryItemV1,
        inventory_revision: u64,
    ) -> Result<Self, MusubiProviderAttestationInventoryErrorV1> {
        item.validate()?;
        if inventory_revision == 0 {
            return Err(MusubiProviderAttestationInventoryErrorV1::InvalidReceipt);
        }
        Ok(Self {
            scope: item.scope.clone(),
            key: item.key,
            attestation_digest: item.attestation_digest,
            handoff_id: item.handoff_id,
            inventory_revision,
        })
    }

    /// Return the stable handoff identity acknowledged by the inventory.
    #[must_use]
    pub const fn handoff_id(&self) -> MusubiProviderAttestationInventoryHandoffIdV1 {
        self.handoff_id
    }

    /// Return the coordinator inventory revision at acknowledgement.
    #[must_use]
    pub const fn inventory_revision(&self) -> u64 {
        self.inventory_revision
    }

    /// Return whether this receipt acknowledges exactly `item`.
    #[must_use]
    pub fn matches(&self, item: &MusubiProviderAttestationInventoryItemV1) -> bool {
        self.inventory_revision != 0
            && self.scope == item.scope
            && self.key == item.key
            && self.attestation_digest == item.attestation_digest
            && self.handoff_id == item.handoff_id
            && item.validate().is_ok()
    }
}

/// Provider-sorted, duplicate-free inventory for one archive and replication order.
#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
pub struct MusubiProviderAttestationInventoryV1 {
    scope: MusubiProviderAttestationInventoryScopeV1,
    items: Vec<MusubiProviderAttestationInventoryItemV1>,
}

impl MusubiProviderAttestationInventoryV1 {
    /// Canonicalize an unordered bounded item set by provider identity.
    ///
    /// # Errors
    ///
    /// Returns an error for an empty, oversized, invalid, cross-scope, or
    /// duplicate-provider item set.
    pub fn new(
        scope: MusubiProviderAttestationInventoryScopeV1,
        mut items: Vec<MusubiProviderAttestationInventoryItemV1>,
    ) -> Result<Self, MusubiProviderAttestationInventoryErrorV1> {
        scope.validate()?;
        if items.is_empty() || items.len() > MUSUBI_MAX_LOCATION_PROVIDERS_V1 {
            return Err(MusubiProviderAttestationInventoryErrorV1::InvalidInventory);
        }
        items.sort_by_key(|item| item.key.provider_id);
        let inventory = Self { scope, items };
        inventory.validate()?;
        Ok(inventory)
    }

    /// Return the common deployment/archive/order scope.
    #[must_use]
    pub const fn scope(&self) -> &MusubiProviderAttestationInventoryScopeV1 {
        &self.scope
    }

    /// Return provider-sorted immutable inventory items.
    #[must_use]
    pub fn items(&self) -> &[MusubiProviderAttestationInventoryItemV1] {
        &self.items
    }

    /// Validate the common scope and strict provider ordering.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid, empty, oversized, unsorted, duplicate, or
    /// cross-scope inventory contents.
    pub fn validate(&self) -> Result<(), MusubiProviderAttestationInventoryErrorV1> {
        self.scope.validate()?;
        if self.items.is_empty()
            || self.items.len() > MUSUBI_MAX_LOCATION_PROVIDERS_V1
            || self
                .items
                .windows(2)
                .any(|pair| pair[0].key.provider_id >= pair[1].key.provider_id)
        {
            return Err(MusubiProviderAttestationInventoryErrorV1::InvalidInventory);
        }
        self.items.iter().try_for_each(|item| {
            item.validate()?;
            if item.scope != self.scope {
                return Err(MusubiProviderAttestationInventoryErrorV1::InvalidInventory);
            }
            Ok(())
        })
    }
}

/// Closed coordinator inventory or transport failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum MusubiProviderAttestationInventoryErrorV1 {
    /// The immutable item is malformed or substituted.
    #[error("Musubi provider-attestation inventory item is invalid")]
    InvalidItem,
    /// The inventory page is empty, oversized, cross-scope, or noncanonical.
    #[error("Musubi provider-attestation inventory is invalid")]
    InvalidInventory,
    /// The handoff acknowledgement is malformed or inconsistent.
    #[error("Musubi provider-attestation inventory receipt is invalid")]
    InvalidReceipt,
    /// A different attestation already occupies the immutable scope/key.
    #[error("Musubi provider-attestation inventory contains a conflicting digest")]
    Conflict,
    /// The inventory adapter is temporarily unavailable.
    #[error("Musubi provider-attestation inventory is unavailable")]
    Unavailable,
    /// The inventory adapter permanently rejected the item or request.
    #[error("Musubi provider-attestation inventory rejected the request")]
    Rejected,
}

/// Idempotent write boundary for the coordinator's immutable attestation inventory.
pub trait MusubiProviderAttestationInventorySinkV1: Send + Sync + 'static {
    /// Insert or replay one exact immutable item.
    ///
    /// An identical replay must return the exact same receipt, including its
    /// original inventory revision. A different digest at the same scope/key
    /// must return [`MusubiProviderAttestationInventoryErrorV1::Conflict`].
    fn put<'a>(
        &'a self,
        item: MusubiProviderAttestationInventoryItemV1,
    ) -> ProviderIngestFutureV1<
        'a,
        Result<
            MusubiProviderAttestationInventoryReceiptV1,
            MusubiProviderAttestationInventoryErrorV1,
        >,
    >;
}

/// Read-only boundary for exact and archive/order-scoped coordinator inventory.
pub trait MusubiProviderAttestationInventoryReaderV1: Send + Sync + 'static {
    /// Read one exact immutable scope/key entry.
    fn get<'a>(
        &'a self,
        scope: &'a MusubiProviderAttestationInventoryScopeV1,
        key: MusubiProviderBundleAttestationKeyV1,
    ) -> ProviderIngestFutureV1<
        'a,
        Result<
            Option<MusubiProviderAttestationInventoryItemV1>,
            MusubiProviderAttestationInventoryErrorV1,
        >,
    >;

    /// Read the complete bounded provider-sorted inventory for one scope.
    fn inventory<'a>(
        &'a self,
        scope: &'a MusubiProviderAttestationInventoryScopeV1,
    ) -> ProviderIngestFutureV1<
        'a,
        Result<
            Option<MusubiProviderAttestationInventoryV1>,
            MusubiProviderAttestationInventoryErrorV1,
        >,
    >;
}

#[derive(NoritoSerialize)]
struct ApprovalIdPreimageV1 {
    key: MusubiProviderBundleAttestationKeyV1,
    payload_signing_hash: [u8; 32],
    completion_claim_digest: [u8; 32],
    signer_policy: ProviderIngestCompletionSignerPolicyV1,
}

#[derive(NoritoSerialize)]
struct InventoryHandoffIdPreimageV1 {
    scope: MusubiProviderAttestationInventoryScopeV1,
    key: MusubiProviderBundleAttestationKeyV1,
    attestation_digest: MusubiProviderBundleAttestationDigestV1,
}

/// Derive the approval identity from the exact key, payload hash, completed
/// claim digest, and governed signer policy.
///
/// # Errors
///
/// Returns an error when the opaque request is invalid or canonical encoding fails.
pub fn musubi_provider_attestation_approval_id_v1(
    request: &ProviderIngestMusubiAttestationApprovalRequestV1,
) -> Result<MusubiProviderAttestationApprovalIdV1, MusubiProviderAttestationJournalErrorV1> {
    validate_approval_request(request)
        .map_err(|_| MusubiProviderAttestationJournalErrorV1::InvalidIntent)?;
    derive_approval_id(
        request.payload(),
        request.completion_claim_digest(),
        request.signer_policy(),
    )
}

fn derive_approval_id(
    payload: &MusubiProviderBundleVerificationPayloadV1,
    completion_claim_digest: [u8; 32],
    signer_policy: ProviderIngestCompletionSignerPolicyV1,
) -> Result<MusubiProviderAttestationApprovalIdV1, MusubiProviderAttestationJournalErrorV1> {
    let signing_hash = payload.signing_hash();
    let preimage = ApprovalIdPreimageV1 {
        key: MusubiProviderBundleAttestationKeyV1 {
            archive_id: payload.binding.archive_id,
            replication_order: payload.binding.replication_order,
            provider_id: payload.binding.provider_id,
        },
        payload_signing_hash: *signing_hash.as_ref(),
        completion_claim_digest,
        signer_policy,
    };
    let digest = domain_hash_norito(APPROVAL_ID_DOMAIN_V1, &preimage)
        .ok_or(MusubiProviderAttestationJournalErrorV1::InvalidIntent)?;
    let id = MusubiProviderAttestationApprovalIdV1(digest);
    if !id.is_valid() {
        return Err(MusubiProviderAttestationJournalErrorV1::InvalidIntent);
    }
    Ok(id)
}

/// Derive the idempotent coordinator handoff identity from exact scope, key,
/// and complete attestation digest.
///
/// # Errors
///
/// Returns an error when a component is invalid or canonical encoding fails.
pub fn musubi_provider_attestation_inventory_handoff_id_v1(
    scope: &MusubiProviderAttestationInventoryScopeV1,
    key: MusubiProviderBundleAttestationKeyV1,
    attestation_digest: MusubiProviderBundleAttestationDigestV1,
) -> Result<MusubiProviderAttestationInventoryHandoffIdV1, MusubiProviderAttestationInventoryErrorV1>
{
    scope.validate()?;
    key.validate()
        .map_err(|_| MusubiProviderAttestationInventoryErrorV1::InvalidItem)?;
    if key.archive_id != scope.archive_id
        || key.replication_order != scope.replication_order
        || attestation_digest.is_zero()
    {
        return Err(MusubiProviderAttestationInventoryErrorV1::InvalidItem);
    }
    let preimage = InventoryHandoffIdPreimageV1 {
        scope: scope.clone(),
        key,
        attestation_digest,
    };
    let digest = domain_hash_norito(INVENTORY_HANDOFF_ID_DOMAIN_V1, &preimage)
        .ok_or(MusubiProviderAttestationInventoryErrorV1::InvalidItem)?;
    let id = MusubiProviderAttestationInventoryHandoffIdV1(digest);
    if !id.is_valid() {
        return Err(MusubiProviderAttestationInventoryErrorV1::InvalidItem);
    }
    Ok(id)
}

fn domain_hash_norito<T: norito::core::NoritoSerialize>(
    domain: &[u8],
    value: &T,
) -> Option<[u8; 32]> {
    let canonical = norito::to_bytes(value).ok()?;
    let domain_len = u64::try_from(domain.len()).ok()?;
    let canonical_len = u64::try_from(canonical.len()).ok()?;
    let mut hasher = blake3::Hasher::new();
    hasher.update(&domain_len.to_be_bytes());
    hasher.update(domain);
    hasher.update(&canonical_len.to_be_bytes());
    hasher.update(&canonical);
    Some(*hasher.finalize().as_bytes())
}

/// Bounded persistence, lease, retry, and CAS policy for the attestation journal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MusubiProviderAttestationJournalPolicyV1 {
    /// Maximum retained active and terminal entries.
    pub max_entries: usize,
    /// Maximum approval or inventory-handoff attempts per stage.
    pub max_attempts: u32,
    /// Lease duration for approval and inventory-handoff claims.
    pub lease_ttl_ms: u64,
    /// Delay before retrying a transient stage failure.
    pub retry_delay_ms: u64,
    /// Maximum canonical checkpoint bytes.
    pub checkpoint_max_bytes: usize,
    /// Maximum CAS conflicts retried by one journal operation.
    pub max_cas_retries: u32,
}

impl Default for MusubiProviderAttestationJournalPolicyV1 {
    fn default() -> Self {
        Self {
            max_entries: 1_024,
            max_attempts: 8,
            lease_ttl_ms: 60_000,
            retry_delay_ms: 1_000,
            checkpoint_max_bytes: 64 * 1024 * 1024,
            max_cas_retries: 16,
        }
    }
}

impl MusubiProviderAttestationJournalPolicyV1 {
    /// Validate every hard and runtime bound.
    ///
    /// # Errors
    ///
    /// Returns an error when a bound is zero or exceeds a hard decoder limit.
    pub fn validate(self) -> Result<(), MusubiProviderAttestationJournalErrorV1> {
        if self.max_entries == 0
            || self.max_entries > MUSUBI_PROVIDER_ATTESTATION_JOURNAL_MAX_ENTRIES_V1
            || self.max_attempts == 0
            || self.lease_ttl_ms == 0
            || self.retry_delay_ms == 0
            || self.checkpoint_max_bytes == 0
            || self.checkpoint_max_bytes
                > MUSUBI_PROVIDER_ATTESTATION_JOURNAL_CHECKPOINT_MAX_BYTES_V1
            || self.max_cas_retries == 0
        {
            return Err(MusubiProviderAttestationJournalErrorV1::InvalidPolicy);
        }
        Ok(())
    }
}

/// Opaque non-zero runtime owner of one journal lease.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MusubiProviderAttestationClaimOwnerV1([u8; 32]);

impl MusubiProviderAttestationClaimOwnerV1 {
    /// Construct an owner identity from runtime entropy.
    ///
    /// # Errors
    ///
    /// Returns an error for the all-zero sentinel.
    pub fn new(bytes: [u8; 32]) -> Result<Self, MusubiProviderAttestationJournalErrorV1> {
        if bytes == [0; 32] {
            return Err(MusubiProviderAttestationJournalErrorV1::InvalidClaimOwner);
        }
        Ok(Self(bytes))
    }

    /// Return exact owner-identity bytes.
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// Serializable public snapshot returned by an abstract CAS store.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MusubiProviderAttestationJournalStoreSnapshotV1 {
    revision: Option<[u8; 32]>,
    checkpoint_bytes: Option<Vec<u8>>,
}

impl MusubiProviderAttestationJournalStoreSnapshotV1 {
    /// Construct the unique empty-store snapshot.
    #[must_use]
    pub const fn empty() -> Self {
        Self {
            revision: None,
            checkpoint_bytes: None,
        }
    }

    /// Construct a content-addressed snapshot from exact checkpoint bytes.
    ///
    /// # Errors
    ///
    /// Returns an error for empty or hard-oversized bytes.
    pub fn from_checkpoint_bytes(
        checkpoint_bytes: Vec<u8>,
    ) -> Result<Self, MusubiProviderAttestationJournalStoreErrorV1> {
        if checkpoint_bytes.is_empty()
            || checkpoint_bytes.len() > MUSUBI_PROVIDER_ATTESTATION_JOURNAL_CHECKPOINT_MAX_BYTES_V1
        {
            return Err(MusubiProviderAttestationJournalStoreErrorV1::Rejected);
        }
        let revision =
            musubi_provider_attestation_journal_checkpoint_revision_v1(&checkpoint_bytes);
        Ok(Self {
            revision: Some(revision),
            checkpoint_bytes: Some(checkpoint_bytes),
        })
    }

    /// Return the content-addressed revision, absent only for an empty store.
    #[must_use]
    pub const fn revision(&self) -> Option<[u8; 32]> {
        self.revision
    }

    /// Borrow exact checkpoint bytes, absent only for an empty store.
    #[must_use]
    pub fn checkpoint_bytes(&self) -> Option<&[u8]> {
        self.checkpoint_bytes.as_deref()
    }

    fn validate(&self) -> bool {
        match (&self.revision, &self.checkpoint_bytes) {
            (None, None) => true,
            (Some(revision), Some(bytes)) => {
                !bytes.is_empty()
                    && bytes.len() <= MUSUBI_PROVIDER_ATTESTATION_JOURNAL_CHECKPOINT_MAX_BYTES_V1
                    && *revision
                        == musubi_provider_attestation_journal_checkpoint_revision_v1(bytes)
            }
            _ => false,
        }
    }
}

/// Outcome of one content-addressed compare-and-swap operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MusubiProviderAttestationJournalCasOutcomeV1 {
    /// The replacement became the latest durable checkpoint.
    Stored {
        /// Content-addressed revision of the exact replacement bytes.
        revision: [u8; 32],
    },
    /// The expected revision was stale and no replacement occurred.
    Conflict,
}

/// Payload-free error returned by the abstract journal checkpoint store.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum MusubiProviderAttestationJournalStoreErrorV1 {
    /// The backing store is temporarily unavailable.
    #[error("Musubi provider-attestation journal store is unavailable")]
    Unavailable,
    /// The store rejected malformed, oversized, or non-monotonic state.
    #[error("Musubi provider-attestation journal store rejected the checkpoint")]
    Rejected,
}

/// Abstract content-addressed compare-and-swap checkpoint store.
///
/// Implementations must durably persist replacement bytes before returning
/// [`MusubiProviderAttestationJournalCasOutcomeV1::Stored`].
pub trait MusubiProviderAttestationJournalStoreV1: Send + Sync + 'static {
    /// Load the latest exact checkpoint or the unique empty snapshot.
    fn load<'a>(
        &'a self,
    ) -> ProviderIngestFutureV1<
        'a,
        Result<
            MusubiProviderAttestationJournalStoreSnapshotV1,
            MusubiProviderAttestationJournalStoreErrorV1,
        >,
    >;

    /// Atomically replace the latest bytes only at `expected_revision`.
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
    >;
}

/// Derive the deterministic revision of exact canonical checkpoint bytes.
#[must_use]
pub fn musubi_provider_attestation_journal_checkpoint_revision_v1(
    checkpoint_bytes: &[u8],
) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(JOURNAL_CHECKPOINT_REVISION_DOMAIN_V1);
    hasher.update(
        &u64::try_from(checkpoint_bytes.len())
            .unwrap_or(u64::MAX)
            .to_be_bytes(),
    );
    hasher.update(checkpoint_bytes);
    *hasher.finalize().as_bytes()
}

/// Exact serializable intent retained before approval.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MusubiProviderAttestationApprovalIntentV1 {
    approval_id: MusubiProviderAttestationApprovalIdV1,
    payload: MusubiProviderBundleVerificationPayloadV1,
    completion_claim_digest: [u8; 32],
    observed_finalized_cursor: ProviderIngestFinalizedCursorV1,
    signer_policy: ProviderIngestCompletionSignerPolicyV1,
    attestation_key: MusubiProviderBundleAttestationKeyV1,
    sequence: u64,
}

impl MusubiProviderAttestationApprovalIntentV1 {
    /// Return the stable approval identity.
    #[must_use]
    pub const fn approval_id(&self) -> MusubiProviderAttestationApprovalIdV1 {
        self.approval_id
    }

    /// Borrow the exact full unsigned attestation payload.
    #[must_use]
    pub const fn payload(&self) -> &MusubiProviderBundleVerificationPayloadV1 {
        &self.payload
    }

    /// Return the opaque finalized completion-claim digest.
    #[must_use]
    pub const fn completion_claim_digest(&self) -> [u8; 32] {
        self.completion_claim_digest
    }

    /// Return the finalized cursor at which the completed row was observed.
    #[must_use]
    pub const fn observed_finalized_cursor(&self) -> ProviderIngestFinalizedCursorV1 {
        self.observed_finalized_cursor
    }

    /// Return the exact governed signer policy.
    #[must_use]
    pub const fn signer_policy(&self) -> ProviderIngestCompletionSignerPolicyV1 {
        self.signer_policy
    }

    /// Return the immutable archive/order/provider key.
    #[must_use]
    pub const fn attestation_key(&self) -> MusubiProviderBundleAttestationKeyV1 {
        self.attestation_key
    }

    /// Return the insertion sequence.
    #[must_use]
    pub const fn sequence(&self) -> u64 {
        self.sequence
    }

    /// Return whether a freshly rederived opaque request is exactly this intent.
    #[must_use]
    pub fn matches_request(
        &self,
        request: &ProviderIngestMusubiAttestationApprovalRequestV1,
    ) -> bool {
        validate_approval_request(request).is_ok()
            && self.payload == *request.payload()
            && self.completion_claim_digest == request.completion_claim_digest()
            && self.observed_finalized_cursor == request.observed_finalized_cursor()
            && self.signer_policy == request.signer_policy()
            && self.attestation_key == attestation_key(request.payload())
            && derive_approval_id(
                request.payload(),
                request.completion_claim_digest(),
                request.signer_policy(),
            )
            .is_ok_and(|approval_id| approval_id == self.approval_id)
    }
}

/// Public stage of one durable approval/handoff entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MusubiProviderAttestationJournalStageV1 {
    /// Exact intent is durable and ready for a freshly derived approval request.
    AwaitingApproval,
    /// One runtime owns a bounded approval lease.
    ApprovalClaimed,
    /// A complete verified attestation is durable and ready for inventory handoff.
    ApprovedPendingHandoff,
    /// One runtime owns a bounded inventory-handoff lease.
    HandoffClaimed,
    /// The exact immutable inventory receipt is durable.
    Delivered,
    /// The entry reached a terminal bounded failure.
    DeadLetter,
}

/// Read-only journal status for one approval identity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MusubiProviderAttestationJournalStatusV1 {
    /// Exact retained approval intent.
    pub intent: MusubiProviderAttestationApprovalIntentV1,
    /// Current crash-safe stage.
    pub stage: MusubiProviderAttestationJournalStageV1,
    /// Monotonic entry generation used to fence stale claims.
    pub generation: u64,
    /// Terminal reason, present only while the entry is a dead letter.
    pub dead_letter_reason: Option<MusubiProviderAttestationDeadLetterReasonV1>,
}

/// Exact approval lease returned to an isolated worker.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MusubiProviderAttestationApprovalClaimV1 {
    intent: MusubiProviderAttestationApprovalIntentV1,
    owner: MusubiProviderAttestationClaimOwnerV1,
    generation: u64,
    lease_expires_at_ms: u64,
}

impl MusubiProviderAttestationApprovalClaimV1 {
    /// Borrow the persisted approval intent.
    #[must_use]
    pub const fn intent(&self) -> &MusubiProviderAttestationApprovalIntentV1 {
        &self.intent
    }

    /// Return the runtime lease owner.
    #[must_use]
    pub const fn owner(&self) -> MusubiProviderAttestationClaimOwnerV1 {
        self.owner
    }

    /// Return the fenced entry generation.
    #[must_use]
    pub const fn generation(&self) -> u64 {
        self.generation
    }

    /// Return the lease expiry in caller-defined monotonic milliseconds.
    #[must_use]
    pub const fn lease_expires_at_ms(&self) -> u64 {
        self.lease_expires_at_ms
    }
}

/// Exact inventory-handoff lease returned after approval is durable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MusubiProviderAttestationHandoffClaimV1 {
    approval_id: MusubiProviderAttestationApprovalIdV1,
    item: MusubiProviderAttestationInventoryItemV1,
    owner: MusubiProviderAttestationClaimOwnerV1,
    generation: u64,
    lease_expires_at_ms: u64,
}

impl MusubiProviderAttestationHandoffClaimV1 {
    /// Return the originating approval identity.
    #[must_use]
    pub const fn approval_id(&self) -> MusubiProviderAttestationApprovalIdV1 {
        self.approval_id
    }

    /// Borrow the exact immutable inventory item.
    #[must_use]
    pub const fn item(&self) -> &MusubiProviderAttestationInventoryItemV1 {
        &self.item
    }

    /// Return the runtime lease owner.
    #[must_use]
    pub const fn owner(&self) -> MusubiProviderAttestationClaimOwnerV1 {
        self.owner
    }

    /// Return the fenced entry generation.
    #[must_use]
    pub const fn generation(&self) -> u64 {
        self.generation
    }

    /// Return the lease expiry in caller-defined monotonic milliseconds.
    #[must_use]
    pub const fn lease_expires_at_ms(&self) -> u64 {
        self.lease_expires_at_ms
    }
}

/// Result of durably inserting or replaying an exact intent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MusubiProviderAttestationEnqueueOutcomeV1 {
    /// A new approval intent was inserted.
    Inserted {
        /// Stable approval identity.
        approval_id: MusubiProviderAttestationApprovalIdV1,
    },
    /// The exact intent was already retained in any stage.
    Existing {
        /// Stable approval identity.
        approval_id: MusubiProviderAttestationApprovalIdV1,
    },
}

impl MusubiProviderAttestationEnqueueOutcomeV1 {
    /// Return the stable approval identity.
    #[must_use]
    pub const fn approval_id(self) -> MusubiProviderAttestationApprovalIdV1 {
        match self {
            Self::Inserted { approval_id } | Self::Existing { approval_id } => approval_id,
        }
    }
}

/// Result of persisting a complete approved attestation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MusubiProviderAttestationApprovalStoreOutcomeV1 {
    /// This call advanced the entry to durable approved state.
    Stored,
    /// The same complete attestation was already durable.
    Existing,
}

/// Result of recording a stage failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MusubiProviderAttestationRetryOutcomeV1 {
    /// The entry was scheduled for a later retry.
    RetryScheduled,
    /// The entry moved to its bounded terminal dead-letter state.
    DeadLettered,
}

/// Result of persisting an exact inventory receipt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MusubiProviderAttestationDeliveryOutcomeV1 {
    /// This call durably completed the handoff.
    Delivered,
    /// The same receipt was already durable.
    Existing,
}

/// Runtime classification of an approval or handoff failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MusubiProviderAttestationFailureClassV1 {
    /// Retry after the configured delay while attempts remain.
    Retryable,
    /// Move immediately to a terminal dead letter.
    Permanent,
}

/// Payload-free reason retained for a terminal journal entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
pub enum MusubiProviderAttestationDeadLetterReasonV1 {
    /// Approval retries or expired approval claims consumed the bound.
    ApprovalRetryExhausted,
    /// The approval operation failed permanently.
    ApprovalRejected,
    /// Inventory retries or expired handoff claims consumed the bound.
    HandoffRetryExhausted,
    /// The coordinator inventory permanently rejected the item.
    HandoffRejected,
}

/// Journal validation, state-transition, or persistence failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum MusubiProviderAttestationJournalErrorV1 {
    /// The configured journal policy is zero or exceeds a hard bound.
    #[error("Musubi provider-attestation journal policy is invalid")]
    InvalidPolicy,
    /// The approval intent or freshly rederived request is malformed.
    #[error("Musubi provider-attestation approval intent is invalid")]
    InvalidIntent,
    /// The runtime claim owner is the forbidden all-zero sentinel.
    #[error("Musubi provider-attestation claim owner is invalid")]
    InvalidClaimOwner,
    /// The bounded journal has no capacity for another retained identity.
    #[error("Musubi provider-attestation journal capacity is exhausted")]
    CapacityExceeded,
    /// A different intent already occupies the immutable attestation key.
    #[error("Musubi provider-attestation approval intent conflicts with retained state")]
    IntentConflict,
    /// No journal entry exists for the requested approval identity.
    #[error("Musubi provider-attestation approval identity was not found")]
    NotFound,
    /// A claim no longer matches the durable owner, generation, or live lease.
    #[error("Musubi provider-attestation claim is stale")]
    StaleClaim,
    /// The complete attestation is invalid or differs from retained evidence.
    #[error("Musubi provider-attestation approval evidence is invalid")]
    InvalidAttestation,
    /// A different complete attestation is already durable for this intent.
    #[error("Musubi provider-attestation approval evidence conflicts")]
    AttestationConflict,
    /// The inventory acknowledgement is invalid or conflicts with durable state.
    #[error("Musubi provider-attestation inventory acknowledgement is invalid")]
    InvalidInventoryReceipt,
    /// The persisted checkpoint is malformed, noncanonical, or violates bounds.
    #[error("Musubi provider-attestation journal checkpoint is corrupt")]
    CorruptCheckpoint,
    /// The abstract checkpoint store is temporarily unavailable.
    #[error("Musubi provider-attestation journal store is unavailable")]
    StoreUnavailable,
    /// The abstract checkpoint store rejected the operation.
    #[error("Musubi provider-attestation journal store rejected the operation")]
    StoreRejected,
    /// Concurrent writers exceeded the bounded CAS retry budget.
    #[error("Musubi provider-attestation journal CAS retry budget is exhausted")]
    CasRetryExhausted,
    /// A counter, lease, or retry timestamp overflowed.
    #[error("Musubi provider-attestation journal counter overflowed")]
    ArithmeticOverflow,
}

#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
struct StoredApprovalIntentV1 {
    approval_id: MusubiProviderAttestationApprovalIdV1,
    payload: MusubiProviderBundleVerificationPayloadV1,
    completion_claim_digest: [u8; 32],
    observed_finalized_cursor: ProviderIngestFinalizedCursorV1,
    signer_policy: ProviderIngestCompletionSignerPolicyV1,
    attestation_key: MusubiProviderBundleAttestationKeyV1,
    sequence: u64,
}

impl StoredApprovalIntentV1 {
    fn public(&self) -> MusubiProviderAttestationApprovalIntentV1 {
        MusubiProviderAttestationApprovalIntentV1 {
            approval_id: self.approval_id,
            payload: self.payload.clone(),
            completion_claim_digest: self.completion_claim_digest,
            observed_finalized_cursor: self.observed_finalized_cursor,
            signer_policy: self.signer_policy,
            attestation_key: self.attestation_key,
            sequence: self.sequence,
        }
    }

    fn exact_fields_match(&self, other: &Self) -> bool {
        self == other
    }

    fn validate(&self) -> bool {
        let cursor = self.observed_finalized_cursor;
        self.approval_id.is_valid()
            && self.sequence != 0
            && self.completion_claim_digest != [0; 32]
            && cursor.height != 0
            && cursor.block_hash != [0; 32]
            && self.payload.validate().is_ok()
            && self.signer_policy.is_valid()
            && self.payload.binding.completion_authority.signer_policy == self.signer_policy
            && self.attestation_key == attestation_key(&self.payload)
            && derive_approval_id(
                &self.payload,
                self.completion_claim_digest,
                self.signer_policy,
            )
            .is_ok_and(|approval_id| approval_id == self.approval_id)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
enum StoredJournalStateV1 {
    AwaitingApproval {
        attempts: u32,
        next_attempt_after_ms: u64,
    },
    ApprovalClaimed {
        attempts: u32,
        owner: [u8; 32],
        lease_expires_at_ms: u64,
    },
    ApprovedPendingHandoff {
        attestation: Box<MusubiProviderBundleVerificationAttestationV1>,
        attempts: u32,
        next_attempt_after_ms: u64,
    },
    HandoffClaimed {
        attestation: Box<MusubiProviderBundleVerificationAttestationV1>,
        attempts: u32,
        owner: [u8; 32],
        lease_expires_at_ms: u64,
    },
    Delivered {
        attestation: Box<MusubiProviderBundleVerificationAttestationV1>,
        receipt: Box<MusubiProviderAttestationInventoryReceiptV1>,
    },
    DeadLetter {
        reason: MusubiProviderAttestationDeadLetterReasonV1,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
struct StoredJournalEntryV1 {
    intent: StoredApprovalIntentV1,
    generation: u64,
    state: StoredJournalStateV1,
}

impl StoredJournalEntryV1 {
    fn status(&self) -> MusubiProviderAttestationJournalStatusV1 {
        let (stage, dead_letter_reason) = match &self.state {
            StoredJournalStateV1::AwaitingApproval { .. } => (
                MusubiProviderAttestationJournalStageV1::AwaitingApproval,
                None,
            ),
            StoredJournalStateV1::ApprovalClaimed { .. } => (
                MusubiProviderAttestationJournalStageV1::ApprovalClaimed,
                None,
            ),
            StoredJournalStateV1::ApprovedPendingHandoff { .. } => (
                MusubiProviderAttestationJournalStageV1::ApprovedPendingHandoff,
                None,
            ),
            StoredJournalStateV1::HandoffClaimed { .. } => (
                MusubiProviderAttestationJournalStageV1::HandoffClaimed,
                None,
            ),
            StoredJournalStateV1::Delivered { .. } => {
                (MusubiProviderAttestationJournalStageV1::Delivered, None)
            }
            StoredJournalStateV1::DeadLetter { reason } => (
                MusubiProviderAttestationJournalStageV1::DeadLetter,
                Some(*reason),
            ),
        };
        MusubiProviderAttestationJournalStatusV1 {
            intent: self.intent.public(),
            stage,
            generation: self.generation,
            dead_letter_reason,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
struct StoredJournalCheckpointV1 {
    version: u8,
    checkpoint_sequence: u64,
    next_intent_sequence: u64,
    entries: Vec<StoredJournalEntryV1>,
}

impl StoredJournalCheckpointV1 {
    const fn empty() -> Self {
        Self {
            version: JOURNAL_CHECKPOINT_VERSION_V1,
            checkpoint_sequence: 0,
            next_intent_sequence: 1,
            entries: Vec::new(),
        }
    }

    fn validate(&self, policy: MusubiProviderAttestationJournalPolicyV1) -> bool {
        if self.version != JOURNAL_CHECKPOINT_VERSION_V1
            || self.next_intent_sequence == 0
            || self.entries.len() > policy.max_entries
            || self.entries.len() > MUSUBI_PROVIDER_ATTESTATION_JOURNAL_MAX_ENTRIES_V1
            || self
                .entries
                .windows(2)
                .any(|pair| pair[0].intent.approval_id >= pair[1].intent.approval_id)
        {
            return false;
        }
        if self.checkpoint_sequence == 0 && !self.entries.is_empty() {
            return false;
        }

        let mut keys = BTreeSet::new();
        let mut sequences = BTreeSet::new();
        for entry in &self.entries {
            if !entry.intent.validate()
                || entry.intent.sequence >= self.next_intent_sequence
                || !keys.insert(entry.intent.attestation_key)
                || !sequences.insert(entry.intent.sequence)
                || !validate_stored_state(entry, policy.max_attempts)
            {
                return false;
            }
        }
        true
    }
}

fn validate_stored_state(entry: &StoredJournalEntryV1, max_attempts: u32) -> bool {
    let valid_attestation = |attestation: &MusubiProviderBundleVerificationAttestationV1| {
        attestation.payload == entry.intent.payload
            && attestation.verify(&entry.intent.payload.binding).is_ok()
    };
    match &entry.state {
        StoredJournalStateV1::AwaitingApproval {
            attempts,
            next_attempt_after_ms,
        } => {
            *attempts < max_attempts
                && ((*attempts == 0 && *next_attempt_after_ms == 0 && entry.generation == 0)
                    || (*attempts != 0 && *next_attempt_after_ms != 0 && entry.generation != 0))
        }
        StoredJournalStateV1::ApprovalClaimed {
            attempts,
            owner,
            lease_expires_at_ms,
        } => {
            *attempts != 0
                && *attempts <= max_attempts
                && *owner != [0; 32]
                && *lease_expires_at_ms != 0
                && entry.generation != 0
        }
        StoredJournalStateV1::ApprovedPendingHandoff {
            attestation,
            attempts,
            next_attempt_after_ms,
        } => {
            valid_attestation(attestation)
                && *attempts < max_attempts
                && entry.generation != 0
                && ((*attempts == 0 && *next_attempt_after_ms == 0)
                    || (*attempts != 0 && *next_attempt_after_ms != 0))
        }
        StoredJournalStateV1::HandoffClaimed {
            attestation,
            attempts,
            owner,
            lease_expires_at_ms,
        } => {
            valid_attestation(attestation)
                && *attempts != 0
                && *attempts <= max_attempts
                && *owner != [0; 32]
                && *lease_expires_at_ms != 0
                && entry.generation != 0
        }
        StoredJournalStateV1::Delivered {
            attestation,
            receipt,
        } => MusubiProviderAttestationInventoryItemV1::new((**attestation).clone()).is_ok_and(
            |item| {
                entry.generation != 0 && valid_attestation(attestation) && receipt.matches(&item)
            },
        ),
        StoredJournalStateV1::DeadLetter { .. } => entry.generation != 0,
    }
}

fn attestation_key(
    payload: &MusubiProviderBundleVerificationPayloadV1,
) -> MusubiProviderBundleAttestationKeyV1 {
    MusubiProviderBundleAttestationKeyV1 {
        archive_id: payload.binding.archive_id,
        replication_order: payload.binding.replication_order,
        provider_id: payload.binding.provider_id,
    }
}

fn intent_from_request(
    request: &ProviderIngestMusubiAttestationApprovalRequestV1,
    sequence: u64,
) -> Result<StoredApprovalIntentV1, MusubiProviderAttestationJournalErrorV1> {
    validate_approval_request(request)
        .map_err(|_| MusubiProviderAttestationJournalErrorV1::InvalidIntent)?;
    let approval_id = musubi_provider_attestation_approval_id_v1(request)?;
    let intent = StoredApprovalIntentV1 {
        approval_id,
        payload: request.payload().clone(),
        completion_claim_digest: request.completion_claim_digest(),
        observed_finalized_cursor: request.observed_finalized_cursor(),
        signer_policy: request.signer_policy(),
        attestation_key: attestation_key(request.payload()),
        sequence,
    };
    if !intent.validate() {
        return Err(MusubiProviderAttestationJournalErrorV1::InvalidIntent);
    }
    Ok(intent)
}

fn increment_generation(
    entry: &mut StoredJournalEntryV1,
) -> Result<u64, MusubiProviderAttestationJournalErrorV1> {
    entry.generation = entry
        .generation
        .checked_add(1)
        .ok_or(MusubiProviderAttestationJournalErrorV1::ArithmeticOverflow)?;
    Ok(entry.generation)
}

fn approval_claim_from_entry(
    entry: &StoredJournalEntryV1,
) -> Option<MusubiProviderAttestationApprovalClaimV1> {
    let StoredJournalStateV1::ApprovalClaimed {
        owner,
        lease_expires_at_ms,
        ..
    } = &entry.state
    else {
        return None;
    };
    Some(MusubiProviderAttestationApprovalClaimV1 {
        intent: entry.intent.public(),
        owner: MusubiProviderAttestationClaimOwnerV1(*owner),
        generation: entry.generation,
        lease_expires_at_ms: *lease_expires_at_ms,
    })
}

fn handoff_claim_from_entry(
    entry: &StoredJournalEntryV1,
) -> Option<MusubiProviderAttestationHandoffClaimV1> {
    let StoredJournalStateV1::HandoffClaimed {
        attestation,
        owner,
        lease_expires_at_ms,
        ..
    } = &entry.state
    else {
        return None;
    };
    let item = MusubiProviderAttestationInventoryItemV1::new((**attestation).clone()).ok()?;
    Some(MusubiProviderAttestationHandoffClaimV1 {
        approval_id: entry.intent.approval_id,
        item,
        owner: MusubiProviderAttestationClaimOwnerV1(*owner),
        generation: entry.generation,
        lease_expires_at_ms: *lease_expires_at_ms,
    })
}

enum JournalMutationV1<T> {
    NoWrite(T),
    Write(T),
}

/// Durable bounded state machine for approval-only signing and inventory handoff.
pub struct MusubiProviderAttestationJournalV1 {
    store: Arc<dyn MusubiProviderAttestationJournalStoreV1>,
    policy: MusubiProviderAttestationJournalPolicyV1,
}

impl MusubiProviderAttestationJournalV1 {
    /// Bind the journal to one abstract durable CAS store and validated policy.
    ///
    /// # Errors
    ///
    /// Returns an error when any policy bound is invalid.
    pub fn new(
        store: Arc<dyn MusubiProviderAttestationJournalStoreV1>,
        policy: MusubiProviderAttestationJournalPolicyV1,
    ) -> Result<Self, MusubiProviderAttestationJournalErrorV1> {
        policy.validate()?;
        Ok(Self { store, policy })
    }

    /// Return the validated persistence and retry policy.
    #[must_use]
    pub const fn policy(&self) -> MusubiProviderAttestationJournalPolicyV1 {
        self.policy
    }

    /// Insert or idempotently replay one exact opaque approval request.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid evidence, capacity exhaustion, an immutable
    /// key conflict, corrupt persistence, or store failure.
    pub async fn enqueue(
        &self,
        request: &ProviderIngestMusubiAttestationApprovalRequestV1,
    ) -> Result<MusubiProviderAttestationEnqueueOutcomeV1, MusubiProviderAttestationJournalErrorV1>
    {
        validate_approval_request(request)
            .map_err(|_| MusubiProviderAttestationJournalErrorV1::InvalidIntent)?;
        let requested_approval_id = musubi_provider_attestation_approval_id_v1(request)?;
        let requested_key = attestation_key(request.payload());
        self.mutate(|checkpoint| {
            let candidate = intent_from_request(request, checkpoint.next_intent_sequence)?;
            if let Some(existing) = checkpoint
                .entries
                .iter()
                .find(|entry| entry.intent.attestation_key == requested_key)
            {
                if existing.intent.approval_id == requested_approval_id {
                    let mut replay_candidate = candidate;
                    replay_candidate.sequence = existing.intent.sequence;
                    if existing.intent.exact_fields_match(&replay_candidate) {
                        return Ok(JournalMutationV1::NoWrite(
                            MusubiProviderAttestationEnqueueOutcomeV1::Existing {
                                approval_id: requested_approval_id,
                            },
                        ));
                    }
                }
                return Err(MusubiProviderAttestationJournalErrorV1::IntentConflict);
            }
            if checkpoint.entries.len() >= self.policy.max_entries {
                if let Some((oldest_delivered_index, _)) = checkpoint
                    .entries
                    .iter()
                    .enumerate()
                    .filter(|(_, entry)| {
                        matches!(&entry.state, StoredJournalStateV1::Delivered { .. })
                    })
                    .min_by_key(|(_, entry)| entry.intent.sequence)
                {
                    checkpoint.entries.remove(oldest_delivered_index);
                }
            }
            if checkpoint.entries.len() >= self.policy.max_entries {
                return Err(MusubiProviderAttestationJournalErrorV1::CapacityExceeded);
            }
            checkpoint.next_intent_sequence = checkpoint
                .next_intent_sequence
                .checked_add(1)
                .ok_or(MusubiProviderAttestationJournalErrorV1::ArithmeticOverflow)?;
            checkpoint.entries.push(StoredJournalEntryV1 {
                intent: candidate,
                generation: 0,
                state: StoredJournalStateV1::AwaitingApproval {
                    attempts: 0,
                    next_attempt_after_ms: 0,
                },
            });
            checkpoint
                .entries
                .sort_by_key(|entry| entry.intent.approval_id);
            Ok(JournalMutationV1::Write(
                MusubiProviderAttestationEnqueueOutcomeV1::Inserted {
                    approval_id: requested_approval_id,
                },
            ))
        })
        .await
    }

    /// Read one durable journal status without modifying its checkpoint.
    ///
    /// # Errors
    ///
    /// Returns an error for corrupt persistence or store failure.
    pub async fn status(
        &self,
        approval_id: MusubiProviderAttestationApprovalIdV1,
    ) -> Result<
        Option<MusubiProviderAttestationJournalStatusV1>,
        MusubiProviderAttestationJournalErrorV1,
    > {
        let snapshot = self.load_checkpoint().await?;
        Ok(snapshot
            .entries
            .iter()
            .find(|entry| entry.intent.approval_id == approval_id)
            .map(StoredJournalEntryV1::status))
    }

    /// Claim ready approval work or reclaim an expired approval lease.
    ///
    /// An unexpired claim owned by the same runtime replays exactly. Another
    /// owner's live claim, a future retry time, or a later stage returns `None`.
    ///
    /// # Errors
    ///
    /// Returns an error for an invalid owner, arithmetic overflow, corrupt
    /// persistence, or store failure.
    pub async fn claim_approval(
        &self,
        approval_id: MusubiProviderAttestationApprovalIdV1,
        owner: MusubiProviderAttestationClaimOwnerV1,
        now_ms: u64,
    ) -> Result<
        Option<MusubiProviderAttestationApprovalClaimV1>,
        MusubiProviderAttestationJournalErrorV1,
    > {
        self.mutate(|checkpoint| {
            let Some(index) = checkpoint
                .entries
                .iter()
                .position(|entry| entry.intent.approval_id == approval_id)
            else {
                return Ok(JournalMutationV1::NoWrite(None));
            };
            let entry = &mut checkpoint.entries[index];
            match entry.state.clone() {
                StoredJournalStateV1::AwaitingApproval {
                    attempts,
                    next_attempt_after_ms,
                } => {
                    if now_ms < next_attempt_after_ms {
                        return Ok(JournalMutationV1::NoWrite(None));
                    }
                    if attempts >= self.policy.max_attempts {
                        increment_generation(entry)?;
                        entry.state = StoredJournalStateV1::DeadLetter {
                            reason:
                                MusubiProviderAttestationDeadLetterReasonV1::ApprovalRetryExhausted,
                        };
                        return Ok(JournalMutationV1::Write(None));
                    }
                    claim_approval_entry(entry, owner, now_ms, self.policy, attempts)
                }
                StoredJournalStateV1::ApprovalClaimed {
                    attempts,
                    owner: retained_owner,
                    lease_expires_at_ms,
                } => {
                    if now_ms < lease_expires_at_ms {
                        if retained_owner == owner.0 {
                            return Ok(JournalMutationV1::NoWrite(approval_claim_from_entry(
                                entry,
                            )));
                        }
                        return Ok(JournalMutationV1::NoWrite(None));
                    }
                    if attempts >= self.policy.max_attempts {
                        increment_generation(entry)?;
                        entry.state = StoredJournalStateV1::DeadLetter {
                            reason:
                                MusubiProviderAttestationDeadLetterReasonV1::ApprovalRetryExhausted,
                        };
                        return Ok(JournalMutationV1::Write(None));
                    }
                    claim_approval_entry(entry, owner, now_ms, self.policy, attempts)
                }
                _ => Ok(JournalMutationV1::NoWrite(None)),
            }
        })
        .await
    }

    /// Return a claimed approval to retry or move it to a terminal dead letter.
    ///
    /// # Errors
    ///
    /// Returns an error for a stale claim, arithmetic overflow, corrupt
    /// persistence, or store failure.
    pub async fn record_approval_failure(
        &self,
        claim: &MusubiProviderAttestationApprovalClaimV1,
        now_ms: u64,
        failure: MusubiProviderAttestationFailureClassV1,
    ) -> Result<MusubiProviderAttestationRetryOutcomeV1, MusubiProviderAttestationJournalErrorV1>
    {
        self.mutate(|checkpoint| {
            let entry = exact_approval_claim_entry(checkpoint, claim, now_ms)?;
            let StoredJournalStateV1::ApprovalClaimed { attempts, .. } = &entry.state else {
                return Err(MusubiProviderAttestationJournalErrorV1::StaleClaim);
            };
            let attempts = *attempts;
            increment_generation(entry)?;
            if failure == MusubiProviderAttestationFailureClassV1::Permanent
                || attempts >= self.policy.max_attempts
            {
                entry.state = StoredJournalStateV1::DeadLetter {
                    reason: if failure == MusubiProviderAttestationFailureClassV1::Permanent {
                        MusubiProviderAttestationDeadLetterReasonV1::ApprovalRejected
                    } else {
                        MusubiProviderAttestationDeadLetterReasonV1::ApprovalRetryExhausted
                    },
                };
                return Ok(JournalMutationV1::Write(
                    MusubiProviderAttestationRetryOutcomeV1::DeadLettered,
                ));
            }
            let retry_at = now_ms
                .checked_add(self.policy.retry_delay_ms)
                .ok_or(MusubiProviderAttestationJournalErrorV1::ArithmeticOverflow)?;
            entry.state = StoredJournalStateV1::AwaitingApproval {
                attempts,
                next_attempt_after_ms: retry_at,
            };
            Ok(JournalMutationV1::Write(
                MusubiProviderAttestationRetryOutcomeV1::RetryScheduled,
            ))
        })
        .await
    }

    /// Persist a complete signer-validated attestation under an exact live claim.
    ///
    /// A caller must supply the freshly rederived opaque request used for
    /// signing; persisted intent alone can never authorize approval after a restart.
    ///
    /// # Errors
    ///
    /// Returns an error for a stale claim/request, invalid or conflicting
    /// attestation, corrupt persistence, or store failure.
    pub async fn store_approved(
        &self,
        claim: &MusubiProviderAttestationApprovalClaimV1,
        request: &ProviderIngestMusubiAttestationApprovalRequestV1,
        attestation: MusubiProviderBundleVerificationAttestationV1,
        now_ms: u64,
    ) -> Result<
        MusubiProviderAttestationApprovalStoreOutcomeV1,
        MusubiProviderAttestationJournalErrorV1,
    > {
        if !claim.intent.matches_request(request)
            || attestation.payload != *request.payload()
            || attestation.verify(&request.payload().binding).is_err()
        {
            return Err(MusubiProviderAttestationJournalErrorV1::InvalidAttestation);
        }
        self.mutate(|checkpoint| {
            let Some(entry) = checkpoint
                .entries
                .iter_mut()
                .find(|entry| entry.intent.approval_id == claim.intent.approval_id)
            else {
                return Err(MusubiProviderAttestationJournalErrorV1::NotFound);
            };
            match &entry.state {
                StoredJournalStateV1::ApprovedPendingHandoff {
                    attestation: retained,
                    ..
                }
                | StoredJournalStateV1::HandoffClaimed {
                    attestation: retained,
                    ..
                }
                | StoredJournalStateV1::Delivered {
                    attestation: retained,
                    ..
                } => {
                    return if retained.as_ref() == &attestation {
                        Ok(JournalMutationV1::NoWrite(
                            MusubiProviderAttestationApprovalStoreOutcomeV1::Existing,
                        ))
                    } else {
                        Err(MusubiProviderAttestationJournalErrorV1::AttestationConflict)
                    };
                }
                StoredJournalStateV1::DeadLetter { .. } => {
                    return Err(MusubiProviderAttestationJournalErrorV1::StaleClaim);
                }
                _ => {}
            }
            validate_exact_approval_claim(entry, claim, now_ms)?;
            increment_generation(entry)?;
            entry.state = StoredJournalStateV1::ApprovedPendingHandoff {
                attestation: Box::new(attestation.clone()),
                attempts: 0,
                next_attempt_after_ms: 0,
            };
            Ok(JournalMutationV1::Write(
                MusubiProviderAttestationApprovalStoreOutcomeV1::Stored,
            ))
        })
        .await
    }

    /// Claim approved inventory work or reclaim an expired handoff lease.
    ///
    /// # Errors
    ///
    /// Returns an error for arithmetic overflow, invalid retained evidence,
    /// corrupt persistence, or store failure.
    pub async fn claim_handoff(
        &self,
        approval_id: MusubiProviderAttestationApprovalIdV1,
        owner: MusubiProviderAttestationClaimOwnerV1,
        now_ms: u64,
    ) -> Result<
        Option<MusubiProviderAttestationHandoffClaimV1>,
        MusubiProviderAttestationJournalErrorV1,
    > {
        self.mutate(|checkpoint| {
            let Some(index) = checkpoint
                .entries
                .iter()
                .position(|entry| entry.intent.approval_id == approval_id)
            else {
                return Ok(JournalMutationV1::NoWrite(None));
            };
            let entry = &mut checkpoint.entries[index];
            match entry.state.clone() {
                StoredJournalStateV1::ApprovedPendingHandoff {
                    attestation,
                    attempts,
                    next_attempt_after_ms,
                } => {
                    if now_ms < next_attempt_after_ms {
                        return Ok(JournalMutationV1::NoWrite(None));
                    }
                    if attempts >= self.policy.max_attempts {
                        increment_generation(entry)?;
                        entry.state = StoredJournalStateV1::DeadLetter {
                            reason:
                                MusubiProviderAttestationDeadLetterReasonV1::HandoffRetryExhausted,
                        };
                        return Ok(JournalMutationV1::Write(None));
                    }
                    claim_handoff_entry(entry, owner, now_ms, self.policy, attestation, attempts)
                }
                StoredJournalStateV1::HandoffClaimed {
                    attestation,
                    attempts,
                    owner: retained_owner,
                    lease_expires_at_ms,
                } => {
                    if now_ms < lease_expires_at_ms {
                        if retained_owner == owner.0 {
                            return Ok(JournalMutationV1::NoWrite(handoff_claim_from_entry(entry)));
                        }
                        return Ok(JournalMutationV1::NoWrite(None));
                    }
                    if attempts >= self.policy.max_attempts {
                        increment_generation(entry)?;
                        entry.state = StoredJournalStateV1::DeadLetter {
                            reason:
                                MusubiProviderAttestationDeadLetterReasonV1::HandoffRetryExhausted,
                        };
                        return Ok(JournalMutationV1::Write(None));
                    }
                    claim_handoff_entry(entry, owner, now_ms, self.policy, attestation, attempts)
                }
                _ => Ok(JournalMutationV1::NoWrite(None)),
            }
        })
        .await
    }

    /// Return a claimed inventory handoff to retry or dead-letter it.
    ///
    /// # Errors
    ///
    /// Returns an error for a stale claim, arithmetic overflow, corrupt
    /// persistence, or store failure.
    pub async fn record_handoff_failure(
        &self,
        claim: &MusubiProviderAttestationHandoffClaimV1,
        now_ms: u64,
        failure: MusubiProviderAttestationFailureClassV1,
    ) -> Result<MusubiProviderAttestationRetryOutcomeV1, MusubiProviderAttestationJournalErrorV1>
    {
        self.mutate(|checkpoint| {
            let entry = exact_handoff_claim_entry(checkpoint, claim, now_ms)?;
            let StoredJournalStateV1::HandoffClaimed {
                attestation,
                attempts,
                ..
            } = &entry.state
            else {
                return Err(MusubiProviderAttestationJournalErrorV1::StaleClaim);
            };
            let attestation = attestation.clone();
            let attempts = *attempts;
            increment_generation(entry)?;
            if failure == MusubiProviderAttestationFailureClassV1::Permanent
                || attempts >= self.policy.max_attempts
            {
                entry.state = StoredJournalStateV1::DeadLetter {
                    reason: if failure == MusubiProviderAttestationFailureClassV1::Permanent {
                        MusubiProviderAttestationDeadLetterReasonV1::HandoffRejected
                    } else {
                        MusubiProviderAttestationDeadLetterReasonV1::HandoffRetryExhausted
                    },
                };
                return Ok(JournalMutationV1::Write(
                    MusubiProviderAttestationRetryOutcomeV1::DeadLettered,
                ));
            }
            let retry_at = now_ms
                .checked_add(self.policy.retry_delay_ms)
                .ok_or(MusubiProviderAttestationJournalErrorV1::ArithmeticOverflow)?;
            entry.state = StoredJournalStateV1::ApprovedPendingHandoff {
                attestation,
                attempts,
                next_attempt_after_ms: retry_at,
            };
            Ok(JournalMutationV1::Write(
                MusubiProviderAttestationRetryOutcomeV1::RetryScheduled,
            ))
        })
        .await
    }

    /// Durably record an exact idempotent coordinator inventory receipt.
    ///
    /// # Errors
    ///
    /// Returns an error for a stale claim, substituted acknowledgement,
    /// corrupt persistence, or store failure.
    pub async fn mark_delivered(
        &self,
        claim: &MusubiProviderAttestationHandoffClaimV1,
        receipt: MusubiProviderAttestationInventoryReceiptV1,
        now_ms: u64,
    ) -> Result<MusubiProviderAttestationDeliveryOutcomeV1, MusubiProviderAttestationJournalErrorV1>
    {
        if !receipt.matches(&claim.item) {
            return Err(MusubiProviderAttestationJournalErrorV1::InvalidInventoryReceipt);
        }
        self.mutate(|checkpoint| {
            let Some(entry) = checkpoint
                .entries
                .iter_mut()
                .find(|entry| entry.intent.approval_id == claim.approval_id)
            else {
                return Err(MusubiProviderAttestationJournalErrorV1::NotFound);
            };
            if let StoredJournalStateV1::Delivered {
                receipt: retained, ..
            } = &entry.state
            {
                return if retained.as_ref() == &receipt {
                    Ok(JournalMutationV1::NoWrite(
                        MusubiProviderAttestationDeliveryOutcomeV1::Existing,
                    ))
                } else {
                    Err(MusubiProviderAttestationJournalErrorV1::InvalidInventoryReceipt)
                };
            }
            validate_exact_handoff_claim(entry, claim, now_ms)?;
            let StoredJournalStateV1::HandoffClaimed { attestation, .. } = &entry.state else {
                return Err(MusubiProviderAttestationJournalErrorV1::StaleClaim);
            };
            let attestation = attestation.clone();
            increment_generation(entry)?;
            entry.state = StoredJournalStateV1::Delivered {
                attestation,
                receipt: Box::new(receipt.clone()),
            };
            Ok(JournalMutationV1::Write(
                MusubiProviderAttestationDeliveryOutcomeV1::Delivered,
            ))
        })
        .await
    }

    async fn load_checkpoint(
        &self,
    ) -> Result<StoredJournalCheckpointV1, MusubiProviderAttestationJournalErrorV1> {
        let snapshot = self.store.load().await.map_err(map_store_error)?;
        decode_checkpoint(&snapshot, self.policy)
    }

    async fn mutate<T, Transition>(
        &self,
        mut transition: Transition,
    ) -> Result<T, MusubiProviderAttestationJournalErrorV1>
    where
        Transition: FnMut(
            &mut StoredJournalCheckpointV1,
        )
            -> Result<JournalMutationV1<T>, MusubiProviderAttestationJournalErrorV1>,
    {
        for _ in 0..self.policy.max_cas_retries {
            let snapshot = self.store.load().await.map_err(map_store_error)?;
            let expected_revision = snapshot.revision();
            let mut checkpoint = decode_checkpoint(&snapshot, self.policy)?;
            let result = match transition(&mut checkpoint)? {
                JournalMutationV1::NoWrite(result) => return Ok(result),
                JournalMutationV1::Write(result) => result,
            };
            checkpoint.checkpoint_sequence = checkpoint
                .checkpoint_sequence
                .checked_add(1)
                .ok_or(MusubiProviderAttestationJournalErrorV1::ArithmeticOverflow)?;
            let replacement = encode_checkpoint(&checkpoint, self.policy)?;
            let expected_stored_revision =
                musubi_provider_attestation_journal_checkpoint_revision_v1(&replacement);
            match self
                .store
                .compare_and_swap(expected_revision, replacement)
                .await
                .map_err(map_store_error)?
            {
                MusubiProviderAttestationJournalCasOutcomeV1::Stored { revision }
                    if revision == expected_stored_revision =>
                {
                    return Ok(result);
                }
                MusubiProviderAttestationJournalCasOutcomeV1::Stored { .. } => {
                    return Err(MusubiProviderAttestationJournalErrorV1::StoreRejected);
                }
                MusubiProviderAttestationJournalCasOutcomeV1::Conflict => {}
            }
        }
        Err(MusubiProviderAttestationJournalErrorV1::CasRetryExhausted)
    }
}

fn claim_approval_entry(
    entry: &mut StoredJournalEntryV1,
    owner: MusubiProviderAttestationClaimOwnerV1,
    now_ms: u64,
    policy: MusubiProviderAttestationJournalPolicyV1,
    attempts: u32,
) -> Result<
    JournalMutationV1<Option<MusubiProviderAttestationApprovalClaimV1>>,
    MusubiProviderAttestationJournalErrorV1,
> {
    let attempts = attempts
        .checked_add(1)
        .ok_or(MusubiProviderAttestationJournalErrorV1::ArithmeticOverflow)?;
    let lease_expires_at_ms = now_ms
        .checked_add(policy.lease_ttl_ms)
        .ok_or(MusubiProviderAttestationJournalErrorV1::ArithmeticOverflow)?;
    increment_generation(entry)?;
    entry.state = StoredJournalStateV1::ApprovalClaimed {
        attempts,
        owner: owner.0,
        lease_expires_at_ms,
    };
    Ok(JournalMutationV1::Write(approval_claim_from_entry(entry)))
}

fn claim_handoff_entry(
    entry: &mut StoredJournalEntryV1,
    owner: MusubiProviderAttestationClaimOwnerV1,
    now_ms: u64,
    policy: MusubiProviderAttestationJournalPolicyV1,
    attestation: Box<MusubiProviderBundleVerificationAttestationV1>,
    attempts: u32,
) -> Result<
    JournalMutationV1<Option<MusubiProviderAttestationHandoffClaimV1>>,
    MusubiProviderAttestationJournalErrorV1,
> {
    let attempts = attempts
        .checked_add(1)
        .ok_or(MusubiProviderAttestationJournalErrorV1::ArithmeticOverflow)?;
    let lease_expires_at_ms = now_ms
        .checked_add(policy.lease_ttl_ms)
        .ok_or(MusubiProviderAttestationJournalErrorV1::ArithmeticOverflow)?;
    increment_generation(entry)?;
    entry.state = StoredJournalStateV1::HandoffClaimed {
        attestation,
        attempts,
        owner: owner.0,
        lease_expires_at_ms,
    };
    let claim = handoff_claim_from_entry(entry)
        .ok_or(MusubiProviderAttestationJournalErrorV1::InvalidAttestation)?;
    Ok(JournalMutationV1::Write(Some(claim)))
}

fn validate_exact_approval_claim(
    entry: &StoredJournalEntryV1,
    claim: &MusubiProviderAttestationApprovalClaimV1,
    now_ms: u64,
) -> Result<(), MusubiProviderAttestationJournalErrorV1> {
    let StoredJournalStateV1::ApprovalClaimed {
        owner,
        lease_expires_at_ms,
        ..
    } = &entry.state
    else {
        return Err(MusubiProviderAttestationJournalErrorV1::StaleClaim);
    };
    if entry.intent.public() != claim.intent
        || entry.generation != claim.generation
        || *owner != claim.owner.0
        || *lease_expires_at_ms != claim.lease_expires_at_ms
        || now_ms >= *lease_expires_at_ms
    {
        return Err(MusubiProviderAttestationJournalErrorV1::StaleClaim);
    }
    Ok(())
}

fn exact_approval_claim_entry<'a>(
    checkpoint: &'a mut StoredJournalCheckpointV1,
    claim: &MusubiProviderAttestationApprovalClaimV1,
    now_ms: u64,
) -> Result<&'a mut StoredJournalEntryV1, MusubiProviderAttestationJournalErrorV1> {
    let entry = checkpoint
        .entries
        .iter_mut()
        .find(|entry| entry.intent.approval_id == claim.intent.approval_id)
        .ok_or(MusubiProviderAttestationJournalErrorV1::NotFound)?;
    validate_exact_approval_claim(entry, claim, now_ms)?;
    Ok(entry)
}

fn validate_exact_handoff_claim(
    entry: &StoredJournalEntryV1,
    claim: &MusubiProviderAttestationHandoffClaimV1,
    now_ms: u64,
) -> Result<(), MusubiProviderAttestationJournalErrorV1> {
    let StoredJournalStateV1::HandoffClaimed {
        attestation,
        owner,
        lease_expires_at_ms,
        ..
    } = &entry.state
    else {
        return Err(MusubiProviderAttestationJournalErrorV1::StaleClaim);
    };
    let retained_item = MusubiProviderAttestationInventoryItemV1::new((**attestation).clone())
        .map_err(|_| MusubiProviderAttestationJournalErrorV1::InvalidAttestation)?;
    if entry.intent.approval_id != claim.approval_id
        || retained_item != claim.item
        || entry.generation != claim.generation
        || *owner != claim.owner.0
        || *lease_expires_at_ms != claim.lease_expires_at_ms
        || now_ms >= *lease_expires_at_ms
    {
        return Err(MusubiProviderAttestationJournalErrorV1::StaleClaim);
    }
    Ok(())
}

fn exact_handoff_claim_entry<'a>(
    checkpoint: &'a mut StoredJournalCheckpointV1,
    claim: &MusubiProviderAttestationHandoffClaimV1,
    now_ms: u64,
) -> Result<&'a mut StoredJournalEntryV1, MusubiProviderAttestationJournalErrorV1> {
    let entry = checkpoint
        .entries
        .iter_mut()
        .find(|entry| entry.intent.approval_id == claim.approval_id)
        .ok_or(MusubiProviderAttestationJournalErrorV1::NotFound)?;
    validate_exact_handoff_claim(entry, claim, now_ms)?;
    Ok(entry)
}

fn map_store_error(
    error: MusubiProviderAttestationJournalStoreErrorV1,
) -> MusubiProviderAttestationJournalErrorV1 {
    match error {
        MusubiProviderAttestationJournalStoreErrorV1::Unavailable => {
            MusubiProviderAttestationJournalErrorV1::StoreUnavailable
        }
        MusubiProviderAttestationJournalStoreErrorV1::Rejected => {
            MusubiProviderAttestationJournalErrorV1::StoreRejected
        }
    }
}

fn decode_checkpoint(
    snapshot: &MusubiProviderAttestationJournalStoreSnapshotV1,
    policy: MusubiProviderAttestationJournalPolicyV1,
) -> Result<StoredJournalCheckpointV1, MusubiProviderAttestationJournalErrorV1> {
    if !snapshot.validate() {
        return Err(MusubiProviderAttestationJournalErrorV1::CorruptCheckpoint);
    }
    let Some(bytes) = snapshot.checkpoint_bytes() else {
        return Ok(StoredJournalCheckpointV1::empty());
    };
    if bytes.len() > policy.checkpoint_max_bytes {
        return Err(MusubiProviderAttestationJournalErrorV1::CorruptCheckpoint);
    }
    let checkpoint: StoredJournalCheckpointV1 =
        norito::decode_canonical_with_limits(bytes, JOURNAL_CHECKPOINT_DECODE_LIMITS_V1)
            .map_err(|_| MusubiProviderAttestationJournalErrorV1::CorruptCheckpoint)?;
    if checkpoint.checkpoint_sequence == 0 || !checkpoint.validate(policy) {
        return Err(MusubiProviderAttestationJournalErrorV1::CorruptCheckpoint);
    }
    Ok(checkpoint)
}

fn encode_checkpoint(
    checkpoint: &StoredJournalCheckpointV1,
    policy: MusubiProviderAttestationJournalPolicyV1,
) -> Result<Vec<u8>, MusubiProviderAttestationJournalErrorV1> {
    if checkpoint.checkpoint_sequence == 0 || !checkpoint.validate(policy) {
        return Err(MusubiProviderAttestationJournalErrorV1::CorruptCheckpoint);
    }
    let bytes = norito::to_bytes(checkpoint)
        .map_err(|_| MusubiProviderAttestationJournalErrorV1::CorruptCheckpoint)?;
    if bytes.is_empty() || bytes.len() > policy.checkpoint_max_bytes {
        return Err(MusubiProviderAttestationJournalErrorV1::CapacityExceeded);
    }
    Ok(bytes)
}

// TODO: add a no-follow, fsync-and-rename sealed file adapter in the daemon
// deployment layer. The journal remains abstract and inert until the real HSM
// signer and coordinator inventory bindings are configured.

#[cfg(test)]
mod tests {
    use std::sync::{
        Mutex,
        atomic::{AtomicBool, Ordering},
    };

    use iroha_crypto::{Algorithm, KeyPair, SignatureOf};
    use iroha_data_model::{
        musubi::{
            ArchiveId, MUSUBI_REGISTRY_VERSION_V1, MusubiContentDigestV1,
            MusubiProviderBundleVerificationApprovalV1, MusubiSemanticReleaseDigestV1,
            MusubiVerificationLockDigestV1,
        },
        sorafs::capacity::ProviderId,
        sorafs::pin_registry::{
            ProviderIngestCompletionAuthorityV1, ProviderIngestFinalizedAnchorV1,
        },
    };

    use super::*;

    #[derive(Default)]
    struct MemoryJournalStore {
        latest: Mutex<MusubiProviderAttestationJournalStoreSnapshotV1>,
    }

    impl Default for MusubiProviderAttestationJournalStoreSnapshotV1 {
        fn default() -> Self {
            Self::empty()
        }
    }

    impl MusubiProviderAttestationJournalStoreV1 for MemoryJournalStore {
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
                self.latest
                    .lock()
                    .map(Clone::clone)
                    .map_err(|_| MusubiProviderAttestationJournalStoreErrorV1::Unavailable)
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
                let mut latest = self
                    .latest
                    .lock()
                    .map_err(|_| MusubiProviderAttestationJournalStoreErrorV1::Unavailable)?;
                if latest.revision() != expected_revision {
                    return Ok(MusubiProviderAttestationJournalCasOutcomeV1::Conflict);
                }
                let revision = replacement
                    .revision()
                    .ok_or(MusubiProviderAttestationJournalStoreErrorV1::Rejected)?;
                *latest = replacement;
                Ok(MusubiProviderAttestationJournalCasOutcomeV1::Stored { revision })
            })
        }
    }

    #[derive(Default)]
    struct MemoryInventory {
        entries: Mutex<Vec<(MusubiProviderAttestationInventoryItemV1, u64)>>,
    }

    impl MusubiProviderAttestationInventorySinkV1 for MemoryInventory {
        fn put<'a>(
            &'a self,
            item: MusubiProviderAttestationInventoryItemV1,
        ) -> ProviderIngestFutureV1<
            'a,
            Result<
                MusubiProviderAttestationInventoryReceiptV1,
                MusubiProviderAttestationInventoryErrorV1,
            >,
        > {
            Box::pin(async move {
                item.validate()?;
                let mut entries = self
                    .entries
                    .lock()
                    .map_err(|_| MusubiProviderAttestationInventoryErrorV1::Unavailable)?;
                if let Some((retained, revision)) = entries
                    .iter()
                    .find(|(retained, _)| retained.scope == item.scope && retained.key == item.key)
                {
                    if retained != &item {
                        return Err(MusubiProviderAttestationInventoryErrorV1::Conflict);
                    }
                    return MusubiProviderAttestationInventoryReceiptV1::new(&item, *revision);
                }
                let revision = u64::try_from(entries.len())
                    .ok()
                    .and_then(|count| count.checked_add(1))
                    .ok_or(MusubiProviderAttestationInventoryErrorV1::Rejected)?;
                entries.push((item.clone(), revision));
                MusubiProviderAttestationInventoryReceiptV1::new(&item, revision)
            })
        }
    }

    impl MusubiProviderAttestationInventoryReaderV1 for MemoryInventory {
        fn get<'a>(
            &'a self,
            scope: &'a MusubiProviderAttestationInventoryScopeV1,
            key: MusubiProviderBundleAttestationKeyV1,
        ) -> ProviderIngestFutureV1<
            'a,
            Result<
                Option<MusubiProviderAttestationInventoryItemV1>,
                MusubiProviderAttestationInventoryErrorV1,
            >,
        > {
            Box::pin(async move {
                scope.validate()?;
                let entries = self
                    .entries
                    .lock()
                    .map_err(|_| MusubiProviderAttestationInventoryErrorV1::Unavailable)?;
                Ok(entries
                    .iter()
                    .find(|(item, _)| item.scope == *scope && item.key == key)
                    .map(|(item, _)| item.clone()))
            })
        }

        fn inventory<'a>(
            &'a self,
            scope: &'a MusubiProviderAttestationInventoryScopeV1,
        ) -> ProviderIngestFutureV1<
            'a,
            Result<
                Option<MusubiProviderAttestationInventoryV1>,
                MusubiProviderAttestationInventoryErrorV1,
            >,
        > {
            Box::pin(async move {
                scope.validate()?;
                let entries = self
                    .entries
                    .lock()
                    .map_err(|_| MusubiProviderAttestationInventoryErrorV1::Unavailable)?;
                let items = entries
                    .iter()
                    .filter(|(item, _)| item.scope == *scope)
                    .map(|(item, _)| item.clone())
                    .collect::<Vec<_>>();
                if items.is_empty() {
                    Ok(None)
                } else {
                    MusubiProviderAttestationInventoryV1::new(scope.clone(), items).map(Some)
                }
            })
        }
    }

    struct Fixture {
        request: ProviderIngestMusubiAttestationApprovalRequestV1,
        owner_key: KeyPair,
    }

    fn signer_policy(revision: u64) -> ProviderIngestCompletionSignerPolicyV1 {
        ProviderIngestCompletionSignerPolicyV1 {
            policy_id: [0x31; 32],
            revision,
            predecessor_digest: (revision > 1).then_some([0x32; 32]),
            policy_digest: [u8::try_from(0x40 + revision).expect("small revision"); 32],
        }
    }

    fn fixture(provider_seed: u8, claim_seed: u8) -> Fixture {
        let owner_key = KeyPair::try_from_seed(vec![0x71; 32], Algorithm::Ed25519)
            .expect("provider owner fixture key");
        let owner = AccountId::new(owner_key.public_key().clone());
        let policy = signer_policy(1);
        let payload = MusubiProviderBundleVerificationPayloadV1 {
            version: MUSUBI_REGISTRY_VERSION_V1,
            binding: MusubiProviderBundleVerificationBindingV1 {
                chain_id: ChainId::from("musubi-attestation-journal-test"),
                genesis_block_hash: [0x21; 32],
                provider_id: ProviderId::new([provider_seed; 32]),
                completed_by: owner.clone(),
                completion_authority: ProviderIngestCompletionAuthorityV1::new(owner, policy),
                replication_order: ReplicationOrderId::new([0x23; 32]),
                assignment_revision: 3,
                completion_epoch: 9,
                finalized_anchor: ProviderIngestFinalizedAnchorV1 {
                    height: 77,
                    block_hash: [0x24; 32],
                },
                archive_id: ArchiveId::new([0x25; 32]),
                bundle_digest: MusubiContentDigestV1::new([0x26; 32]),
                descriptor_digest: MusubiContentDigestV1::new([0x27; 32]),
                semantic_release_manifest_digest: MusubiSemanticReleaseDigestV1::new([0x28; 32]),
                verification_lock_digest: MusubiVerificationLockDigestV1::new([0x29; 32]),
                source_tree_digest: MusubiContentDigestV1::new([0x2A; 32]),
            },
        };
        payload.validate().expect("valid fixture payload");
        let request = ProviderIngestMusubiAttestationApprovalRequestV1::test_fixture(
            payload,
            [claim_seed; 32],
            ProviderIngestFinalizedCursorV1 {
                height: 80,
                block_hash: [0x2B; 32],
            },
            policy,
        )
        .expect("valid approval request fixture");
        Fixture { request, owner_key }
    }

    fn signed_attestation(fixture: &Fixture) -> MusubiProviderBundleVerificationAttestationV1 {
        let payload = fixture.request.payload().clone();
        let approval = MusubiProviderBundleVerificationApprovalV1 {
            public_key: fixture.owner_key.public_key().clone(),
            signature: SignatureOf::try_from_hash(
                fixture.owner_key.private_key(),
                payload.signing_hash(),
            )
            .expect("sign fixture attestation"),
        };
        let attestation = MusubiProviderBundleVerificationAttestationV1 {
            payload,
            approvals: vec![approval],
        };
        attestation
            .verify(&attestation.payload.binding)
            .expect("fixture attestation verifies");
        attestation
    }

    struct FakeApprovalSigner {
        handle: String,
        owner: AccountId,
        owner_key: KeyPair,
        policy: Mutex<ProviderIngestCompletionSignerPolicyV1>,
        rotate_after_approval: AtomicBool,
    }

    impl FakeApprovalSigner {
        fn new(fixture: &Fixture) -> Self {
            Self {
                handle: "hsm://sorafs/musubi/provider-attestation/primary".to_owned(),
                owner: fixture.request.payload().binding.completed_by.clone(),
                owner_key: fixture.owner_key.clone(),
                policy: Mutex::new(fixture.request.signer_policy()),
                rotate_after_approval: AtomicBool::new(false),
            }
        }

        fn policy(&self) -> ProviderIngestCompletionSignerPolicyV1 {
            *self.policy.lock().expect("fake signer policy lock")
        }
    }

    impl MusubiProviderAttestationSignerV1 for FakeApprovalSigner {
        fn runtime_handle(&self) -> &str {
            &self.handle
        }

        fn authority(&self) -> &AccountId {
            &self.owner
        }

        fn qualification(
            &self,
        ) -> Result<
            MusubiProviderAttestationSignerQualificationV1,
            MusubiProviderAttestationSignerErrorV1,
        > {
            Ok(MusubiProviderAttestationSignerQualificationV1::new(
                1,
                self.policy(),
                self.owner.clone(),
                [0x55; 32],
            ))
        }

        fn signer_policy(&self) -> ProviderIngestCompletionSignerPolicyV1 {
            self.policy()
        }

        fn current_eligibility(
            &self,
        ) -> Result<ProviderIngestCompletionSignerPolicyV1, MusubiProviderAttestationSignerErrorV1>
        {
            Ok(self.policy())
        }

        fn approve<'a>(
            &'a self,
            request: &'a ProviderIngestMusubiAttestationApprovalRequestV1,
        ) -> ProviderIngestFutureV1<
            'a,
            Result<
                MusubiProviderBundleVerificationAttestationV1,
                MusubiProviderAttestationSignerErrorV1,
            >,
        > {
            Box::pin(async move {
                let payload = request.payload().clone();
                let attestation = MusubiProviderBundleVerificationAttestationV1 {
                    approvals: vec![MusubiProviderBundleVerificationApprovalV1 {
                        public_key: self.owner_key.public_key().clone(),
                        signature: SignatureOf::try_from_hash(
                            self.owner_key.private_key(),
                            payload.signing_hash(),
                        )
                        .map_err(|_| MusubiProviderAttestationSignerErrorV1::Rejected)?,
                    }],
                    payload,
                };
                if self.rotate_after_approval.load(Ordering::SeqCst) {
                    *self
                        .policy
                        .lock()
                        .map_err(|_| MusubiProviderAttestationSignerErrorV1::Unavailable)? =
                        signer_policy(2);
                }
                Ok(attestation)
            })
        }
    }

    fn test_policy() -> MusubiProviderAttestationJournalPolicyV1 {
        MusubiProviderAttestationJournalPolicyV1 {
            max_entries: 16,
            max_attempts: 2,
            lease_ttl_ms: 10,
            retry_delay_ms: 5,
            checkpoint_max_bytes: 8 * 1024 * 1024,
            max_cas_retries: 4,
        }
    }

    #[tokio::test]
    async fn exact_enqueue_replays_and_same_key_substitution_conflicts() {
        let store = Arc::new(MemoryJournalStore::default());
        let journal = MusubiProviderAttestationJournalV1::new(store, test_policy())
            .expect("construct journal");
        let fixture = fixture(0x41, 0x42);
        let inserted = journal
            .enqueue(&fixture.request)
            .await
            .expect("insert intent");
        assert!(matches!(
            inserted,
            MusubiProviderAttestationEnqueueOutcomeV1::Inserted { .. }
        ));
        assert!(matches!(
            journal
                .enqueue(&fixture.request)
                .await
                .expect("replay intent"),
            MusubiProviderAttestationEnqueueOutcomeV1::Existing { .. }
        ));

        let conflicting = fixture(0x41, 0x43);
        assert_eq!(
            journal.enqueue(&conflicting.request).await,
            Err(MusubiProviderAttestationJournalErrorV1::IntentConflict)
        );
    }

    #[tokio::test]
    async fn expired_reclaim_fences_the_stale_approval_claim() {
        let store = Arc::new(MemoryJournalStore::default());
        let journal = MusubiProviderAttestationJournalV1::new(store, test_policy())
            .expect("construct journal");
        let fixture = fixture(0x44, 0x45);
        let id = journal
            .enqueue(&fixture.request)
            .await
            .expect("enqueue")
            .approval_id();
        let first = journal
            .claim_approval(
                id,
                MusubiProviderAttestationClaimOwnerV1::new([0x51; 32]).expect("owner"),
                100,
            )
            .await
            .expect("first claim")
            .expect("ready work");
        let second = journal
            .claim_approval(
                id,
                MusubiProviderAttestationClaimOwnerV1::new([0x52; 32]).expect("owner"),
                first.lease_expires_at_ms(),
            )
            .await
            .expect("reclaim")
            .expect("expired work is reclaimable");
        assert!(second.generation() > first.generation());
        assert_eq!(
            journal
                .record_approval_failure(
                    &first,
                    first.lease_expires_at_ms(),
                    MusubiProviderAttestationFailureClassV1::Retryable,
                )
                .await,
            Err(MusubiProviderAttestationJournalErrorV1::StaleClaim)
        );
    }

    #[tokio::test]
    async fn approved_state_survives_restart_and_hands_off_idempotently() {
        let store = Arc::new(MemoryJournalStore::default());
        let fixture = fixture(0x46, 0x47);
        let journal = MusubiProviderAttestationJournalV1::new(store.clone(), test_policy())
            .expect("construct journal");
        let id = journal
            .enqueue(&fixture.request)
            .await
            .expect("enqueue")
            .approval_id();
        let approval_claim = journal
            .claim_approval(
                id,
                MusubiProviderAttestationClaimOwnerV1::new([0x53; 32]).expect("owner"),
                100,
            )
            .await
            .expect("claim")
            .expect("approval ready");
        let signer = FakeApprovalSigner::new(&fixture);
        let attestation = approve_musubi_provider_attestation_v1(&signer, &fixture.request)
            .await
            .expect("approval-only signer succeeds");
        journal
            .store_approved(&approval_claim, &fixture.request, attestation, 101)
            .await
            .expect("store approved evidence");
        drop(journal);

        let restarted =
            MusubiProviderAttestationJournalV1::new(store, test_policy()).expect("restart journal");
        let handoff = restarted
            .claim_handoff(
                id,
                MusubiProviderAttestationClaimOwnerV1::new([0x54; 32]).expect("owner"),
                200,
            )
            .await
            .expect("claim handoff")
            .expect("approved handoff survived restart");
        let inventory = MemoryInventory::default();
        let receipt = inventory
            .put(handoff.item().clone())
            .await
            .expect("inventory insert");
        restarted
            .mark_delivered(&handoff, receipt.clone(), 201)
            .await
            .expect("persist delivery");
        assert_eq!(
            restarted.mark_delivered(&handoff, receipt, 201).await,
            Ok(MusubiProviderAttestationDeliveryOutcomeV1::Existing)
        );
        assert_eq!(
            restarted
                .status(id)
                .await
                .expect("read status")
                .expect("retained status")
                .stage,
            MusubiProviderAttestationJournalStageV1::Delivered
        );
    }

    #[tokio::test]
    async fn capacity_prunes_only_oldest_delivered_entry() {
        let store = Arc::new(MemoryJournalStore::default());
        let mut policy = test_policy();
        policy.max_entries = 3;
        let journal = MusubiProviderAttestationJournalV1::new(store, policy)
            .expect("construct bounded journal");

        let delivered_fixture = fixture(0x31, 0x81);
        let delivered_id = journal
            .enqueue(&delivered_fixture.request)
            .await
            .expect("enqueue delivered candidate")
            .approval_id();
        let approval_claim = journal
            .claim_approval(
                delivered_id,
                MusubiProviderAttestationClaimOwnerV1::new([0x81; 32]).expect("owner"),
                100,
            )
            .await
            .expect("claim approval")
            .expect("approval work");
        journal
            .store_approved(
                &approval_claim,
                &delivered_fixture.request,
                signed_attestation(&delivered_fixture),
                101,
            )
            .await
            .expect("store attestation");
        let handoff_claim = journal
            .claim_handoff(
                delivered_id,
                MusubiProviderAttestationClaimOwnerV1::new([0x82; 32]).expect("owner"),
                102,
            )
            .await
            .expect("claim handoff")
            .expect("handoff work");
        let receipt = MusubiProviderAttestationInventoryReceiptV1::new(handoff_claim.item(), 1)
            .expect("exact receipt");
        journal
            .mark_delivered(&handoff_claim, receipt, 103)
            .await
            .expect("deliver first entry");

        let dead_fixture = fixture(0x32, 0x82);
        let dead_id = journal
            .enqueue(&dead_fixture.request)
            .await
            .expect("enqueue dead-letter candidate")
            .approval_id();
        let dead_claim = journal
            .claim_approval(
                dead_id,
                MusubiProviderAttestationClaimOwnerV1::new([0x83; 32]).expect("owner"),
                200,
            )
            .await
            .expect("claim dead-letter candidate")
            .expect("approval work");
        journal
            .record_approval_failure(
                &dead_claim,
                201,
                MusubiProviderAttestationFailureClassV1::Permanent,
            )
            .await
            .expect("dead-letter exact entry");

        let active_fixture = fixture(0x33, 0x83);
        let active_id = journal
            .enqueue(&active_fixture.request)
            .await
            .expect("enqueue active entry")
            .approval_id();
        let replacement_fixture = fixture(0x34, 0x84);
        let replacement_id = journal
            .enqueue(&replacement_fixture.request)
            .await
            .expect("delivered tombstone makes room")
            .approval_id();

        assert!(
            journal
                .status(delivered_id)
                .await
                .expect("status")
                .is_none()
        );
        assert_eq!(
            journal
                .status(dead_id)
                .await
                .expect("dead status")
                .expect("dead letter retained")
                .stage,
            MusubiProviderAttestationJournalStageV1::DeadLetter
        );
        assert_eq!(
            journal
                .status(active_id)
                .await
                .expect("active status")
                .expect("active entry retained")
                .stage,
            MusubiProviderAttestationJournalStageV1::AwaitingApproval
        );
        assert!(
            journal
                .status(replacement_id)
                .await
                .expect("replacement status")
                .is_some()
        );
    }

    #[tokio::test]
    async fn inventory_replays_exact_item_and_rejects_same_key_different_digest() {
        let first_fixture = fixture(0x48, 0x49);
        let first =
            MusubiProviderAttestationInventoryItemV1::new(signed_attestation(&first_fixture))
                .expect("first item");
        let inventory = MemoryInventory::default();
        let inserted = inventory.put(first.clone()).await.expect("insert item");
        let replayed = inventory.put(first.clone()).await.expect("replay item");
        assert_eq!(
            inserted, replayed,
            "identical replay returns the exact receipt"
        );
        assert_eq!(inserted.inventory_revision(), replayed.inventory_revision());

        let mut conflicting_attestation = signed_attestation(&first_fixture);
        conflicting_attestation.payload.binding.bundle_digest =
            MusubiContentDigestV1::new([0xEE; 32]);
        let payload = conflicting_attestation.payload.clone();
        conflicting_attestation.approvals = vec![MusubiProviderBundleVerificationApprovalV1 {
            public_key: first_fixture.owner_key.public_key().clone(),
            signature: SignatureOf::try_from_hash(
                first_fixture.owner_key.private_key(),
                payload.signing_hash(),
            )
            .expect("sign conflicting valid attestation"),
        }];
        let conflicting = MusubiProviderAttestationInventoryItemV1::new(conflicting_attestation)
            .expect("different digest remains structurally valid");
        assert_eq!(first.key(), conflicting.key());
        assert_ne!(first.attestation_digest(), conflicting.attestation_digest());
        assert_eq!(
            inventory.put(conflicting).await,
            Err(MusubiProviderAttestationInventoryErrorV1::Conflict)
        );
    }

    #[test]
    fn inventory_is_canonicalized_by_unique_provider_identity() {
        let item_three =
            MusubiProviderAttestationInventoryItemV1::new(signed_attestation(&fixture(0x63, 0x73)))
                .expect("provider three item");
        let scope = item_three.scope().clone();
        let item_one =
            MusubiProviderAttestationInventoryItemV1::new(signed_attestation(&fixture(0x61, 0x71)))
                .expect("provider one item");
        let item_two =
            MusubiProviderAttestationInventoryItemV1::new(signed_attestation(&fixture(0x62, 0x72)))
                .expect("provider two item");
        let inventory =
            MusubiProviderAttestationInventoryV1::new(scope, vec![item_three, item_one, item_two])
                .expect("canonical inventory");
        assert_eq!(
            inventory
                .items()
                .iter()
                .map(|item| *item.key().provider_id.as_bytes())
                .collect::<Vec<_>>(),
            vec![[0x61; 32], [0x62; 32], [0x63; 32]]
        );
    }

    #[tokio::test]
    async fn signer_validation_rechecks_eligibility_after_approval() {
        let fixture = fixture(0x64, 0x74);
        let signer = FakeApprovalSigner::new(&fixture);
        let attestation = approve_musubi_provider_attestation_v1(&signer, &fixture.request)
            .await
            .expect("qualified signer succeeds");
        assert_eq!(attestation.payload, *fixture.request.payload());

        let rotating = FakeApprovalSigner::new(&fixture);
        rotating.rotate_after_approval.store(true, Ordering::SeqCst);
        assert_eq!(
            approve_musubi_provider_attestation_v1(&rotating, &fixture.request).await,
            Err(MusubiProviderAttestationApprovalErrorV1::EligibilityChanged)
        );
    }
}
