//! Deployment-owned runtime-provider registry boundary for the standard daemon launcher.
//!
//! This module deliberately projects only public provider bindings out of
//! [`iroha_config`]. The deployment registry never receives the full node
//! configuration, because that structure also contains validator keys, API
//! tokens, and other values that runtime-provider discovery must not observe.

use std::fmt;

use iroha_config::parameters::{
    actual::Root as Config,
    defaults::sorafs::storage::provider_ingest_runtime::outbox as provider_ingest_outbox_defaults,
    is_production_runtime_handle,
};
use rand::{rand_core::TryRngCore as _, rngs::OsRng};

use crate::IrohaRuntimeDeps;

mod binding_collection;
mod dependency_scope;

use binding_collection::{
    append_required_governance_request_auth_binding, append_required_governance_service_binding,
    collect_configured_bindings,
};
use dependency_scope::{dependency_is_present, has_unrequested_dependency};

const MAX_PROVIDER_INGEST_SOURCE_STREAMS_V1: u32 = 1_024;
const GOVERNANCE_DAG_SIGNER_STARTUP_CHALLENGE_DOMAIN_V1: &[u8] =
    b"sorafs.governance-dag.registry-startup-possession.v1\0";

/// One runtime-provider role understood by the V1 daemon launcher.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u16)]
#[non_exhaustive]
pub enum IrohaRuntimeProviderSlotV1 {
    /// Moderation quarantine-object data-key wrapper.
    ModerationQuarantineKeyWrapper = 1,
    /// Threshold-PRF provider used by transparency publication.
    PrivacyCyclePrfProvider = 2,
    /// Finalized release-anchor provider used by transparency publication.
    PrivacyReleaseAnchor = 3,
    /// External sealed-CAS leader lease used by transparency publication.
    TransparencyLeaderLease = 4,
    /// Fused privacy Governance target writer.
    FencedPrivacyPublisher = 5,
    /// Authenticated authoritative-head reader paired with the fused writer.
    FencedPrivacyHeadReader = 6,
    /// HSM/KMS signer used by the embedded Governance DAG publisher.
    GovernanceDagSigner = 7,
    /// Authenticator used for Governance DAG Kubo/IPFS/IPNS requests.
    GovernanceDagIpfsAuthenticator = 8,
    /// Authenticator used for signed Governance DAG head compare-and-swap.
    GovernanceDagHeadAuthenticator = 9,
    /// Sealed monotonic Governance DAG service and local-producer state store.
    GovernanceDagCheckpointStore = 10,
    /// HSM/KMS signer used for `SoraFS` stream-token issuance.
    StreamTokenSigner = 11,
    /// One appeal-finance transaction signer.
    AppealFinanceTransactionSigner = 12,
    /// Appeal-finance sealed checkpoint provider.
    AppealFinanceCheckpoint = 13,
    /// Native proof-outcome transaction signer.
    ProofOutcomeTransactionSigner = 14,
    /// Native repair transaction signer.
    RepairTransactionSigner = 15,
    /// Native reserve/rent transaction signer.
    ReserveTransactionSigner = 16,
    /// Native orderbook transaction signer.
    OrderbookTransactionSigner = 17,
    /// Moderation native-transaction signer.
    ModerationTransactionSigner = 18,
    /// Moderation settlement durable handoff.
    ModerationSettlementHandoff = 19,
    /// Moderation publication durable handoff.
    ModerationPublicationHandoff = 20,
    /// Moderation panel-notification delivery boundary.
    ModerationPanelNotification = 21,
    /// Evidence-viewer `WebAuthn` boundary.
    EvidenceViewerWebAuthn = 22,
    /// Evidence-viewer finalized grant authority.
    EvidenceViewerGrantAuthority = 23,
    /// Evidence-viewer receipt signer.
    EvidenceViewerReceiptSigner = 24,
    /// Evidence-viewer erasure boundary.
    EvidenceViewerErasure = 25,
    /// Evidence-viewer authoritative checkpoint store.
    EvidenceViewerCheckpointStore = 26,
    /// `PoP` credential provider registry.
    PopCredentialProviderRegistry = 27,
    /// Independently administered `PoTR` gateway signer.
    PotrGatewaySigner = 28,
    /// Independently administered `PoTR` provider signer.
    PotrProviderSigner = 29,
    /// Gateway ACME client.
    GatewayAcmeClient = 30,
    /// Gateway authenticated compliance-feed transport.
    GatewayComplianceFeedTransport = 31,
    /// Reputation native journal-transaction submitter.
    ReputationJournalTransactionSubmitter = 32,
    /// Reputation external threshold signer.
    ReputationThresholdSigner = 33,
    /// Reputation Governance DAG publication/readback provider.
    ReputationGovernanceDag = 34,
    /// Billing immutable finalized-query provider.
    BillingFinalizedQuery = 35,
    /// Billing consensus journal verifier.
    BillingJournalVerifier = 36,
    /// Billing statement HSM/KMS signer.
    BillingStatementSigner = 37,
    /// Billing immutable statement publisher.
    BillingStatementPublisher = 38,
    /// Billing acknowledgement authority.
    BillingAcknowledgementAuthority = 39,
    /// Billing sealed epoch-witness store.
    BillingEpochWitnessStore = 40,
    /// Provider-ingest authenticated source transport.
    ProviderIngestAuthenticatedSource = 41,
    /// Provider-ingest governed completion-signer resolver.
    ProviderIngestCompletionSignerResolver = 42,
    /// Provider-ingest completion signer resolved by the governed resolver.
    ProviderIngestCompletionSigner = 43,
    /// Provider-ingest sealed monotonic checkpoint store.
    ProviderIngestCheckpointStore = 44,
    /// Provider-ingest finalized-archive retention approval authority.
    ProviderIngestRetentionAuthority = 45,
    /// Authenticated immutable finalized-PoR replay archive.
    PorFinalizedReplayArchive = 46,
    /// Authenticated immutable evidence-viewer compaction archive.
    EvidenceViewerCompactionArchive = 47,
    /// Reputation finalized-archive sealed retention approval authority.
    ReputationFinalizedArchiveRetentionAuthority = 48,
    /// Soracloud runtime mutation and purpose-separated provenance signer.
    SoracloudRuntimeMutationSigner = 49,
    /// Reputation journal externally sealed monotonic checkpoint provider.
    ReputationJournalCheckpoint = 50,
    /// Authenticated Hugging Face inference credential provider.
    SoracloudHfInferenceCredentialProvider = 51,
    /// Moderation sealed predecessor-bound monotonic checkpoint store.
    ModerationCheckpointStore = 52,
    /// Evidence-viewer signed monotonic transparency-head publisher.
    EvidenceViewerTransparencyPublisher = 53,
}

impl IrohaRuntimeProviderSlotV1 {
    /// Return the stable first-release broker protocol identifier for this role.
    #[must_use]
    pub const fn wire_id(self) -> u16 {
        self as u16
    }
}

/// Public identity and optional exact qualification of one requested provider.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IrohaRuntimeProviderBindingV1 {
    slot: IrohaRuntimeProviderSlotV1,
    handle: String,
    revision: Option<u64>,
    policy_digest: Option<[u8; 32]>,
    stream_token_signer_public_key: Option<[u8; 32]>,
    appeal_finance_signer_binding:
        Option<iroha_config::parameters::actual::SorafsAppealFinanceSignerBinding>,
    appeal_finance_checkpoint_binding:
        Option<iroha_config::parameters::actual::SorafsAppealFinanceCheckpointBinding>,
    appeal_finance_checkpoint_max_bytes: Option<u64>,
    pop_credential_runtime_binding: Option<PopCredentialRuntimeBindingV1>,
    potr_runtime_binding: Option<iroha_config::parameters::actual::SorafsPotrRuntimeBinding>,
    native_signer_binding: Option<iroha_torii::SorafsNativeTransactionSignerBindingV1>,
    soracloud_runtime_signer_binding:
        Option<crate::soracloud_runtime_signer::SoracloudRuntimeSignerBindingV1>,
    provider_ingest_signer_binding: Option<sorafs_node::ProviderIngestCompletionSignerBindingV1>,
    provider_ingest_source_limits: Option<ProviderIngestSourceLimitsV1>,
    provider_ingest_checkpoint_max_bytes: Option<u64>,
    provider_ingest_max_signed_transaction_bytes: Option<u64>,
    por_replay_archive_binding: Option<sorafs_node::PorFinalizedReplayArchiveBindingV1>,
    por_replay_archive_proof_limits: Option<PorReplayArchiveProofLimitsV1>,
    evidence_viewer_webauthn_binding: Option<EvidenceViewerWebAuthnBindingV1>,
    evidence_viewer_grant_ttl_ms: Option<u64>,
    evidence_viewer_receipt_signer_public_key: Option<[u8; 32]>,
    evidence_viewer_transparency_publisher_public_key: Option<[u8; 32]>,
    evidence_viewer_checkpoint_max_bytes: Option<u64>,
    moderation_checkpoint_max_bytes: Option<u64>,
    evidence_viewer_archive_id: Option<[u8; 32]>,
    evidence_viewer_archive_public_key: Option<[u8; 32]>,
    evidence_viewer_archive_max_bytes: Option<u64>,
    governance_dag_publisher_peer_id: Option<Vec<u8>>,
    governance_dag_publisher_public_key: Option<[u8; 32]>,
    governance_request_auth_public_key: Option<[u8; 32]>,
    governance_request_auth_max_body_bytes: Option<u64>,
}

/// Public resource limits for the authenticated provider-ingest source broker.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ProviderIngestSourceLimitsV1 {
    /// Exact configured deadline for one source operation.
    pub operation_timeout_ms: u64,
    /// Maximum finalized payload length admitted by local storage.
    pub max_content_bytes: u64,
    /// Maximum governed source identities accepted in one fetch.
    pub max_source_providers: u32,
    /// Maximum independently streamed source fetches.
    pub max_concurrent_streams: u32,
}

/// Public allocation bounds for authenticated finalized-PoR archive proofs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct PorReplayArchiveProofLimitsV1 {
    /// Maximum signed successor receipts admitted before inner decoding.
    pub max_successor_receipts: u32,
    /// Maximum canonical bytes admitted for the successor-receipt proof.
    pub max_successor_proof_bytes: u64,
}

/// Exact public WebAuthn policy carried to the local runtime-provider broker.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct EvidenceViewerWebAuthnBindingV1 {
    /// Canonical relying-party identifier.
    pub rp_id: String,
    /// Exact ordered canonical HTTPS origins accepted by the service.
    pub allowed_origins: Vec<String>,
    /// Maximum lifetime admitted for one issued challenge.
    pub challenge_ttl_ms: u64,
}

/// Exact public inputs accepted by the deployment-owned PoP provider registry.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct PopCredentialRuntimeBindingV1 {
    /// Exact active finalized issuer-policy digest.
    pub issuer_policy_digest: [u8; 32],
    /// Exact governed issuer identity.
    pub issuer_id: String,
    /// Exact non-secret issuer HSM key handle.
    pub issuer_hsm_key_id: String,
    /// Exact governed issuer verification key.
    pub issuer_public_key: [u8; 32],
    /// Exact non-secret encrypted-enrollment recipient handle.
    pub enrollment_recipient_key_id: String,
    /// Exact digest of the hybrid enrollment-recipient public key.
    pub enrollment_recipient_public_key_digest: [u8; 32],
    /// Exact non-secret wallet wrapping-key handle.
    pub wallet_wrapping_key_id: String,
}

impl IrohaRuntimeProviderBindingV1 {
    fn try_new(
        slot: IrohaRuntimeProviderSlotV1,
        handle: impl Into<String>,
        revision: Option<u64>,
        policy_digest: Option<[u8; 32]>,
    ) -> Result<Self, IrohaRuntimeProviderRegistryErrorV1> {
        let handle = handle.into();
        if !is_production_runtime_handle(&handle) {
            return Err(IrohaRuntimeProviderRegistryErrorV1::InvalidBinding(slot));
        }
        match (revision, policy_digest) {
            (Some(0), _) | (Some(_), None) | (None, Some(_)) => {
                return Err(IrohaRuntimeProviderRegistryErrorV1::InvalidBinding(slot));
            }
            (_, Some(digest)) if digest == [0; 32] => {
                return Err(IrohaRuntimeProviderRegistryErrorV1::InvalidBinding(slot));
            }
            (Some(_), Some(_)) | (None, None) => {}
        }
        Ok(Self {
            slot,
            handle,
            revision,
            policy_digest,
            stream_token_signer_public_key: None,
            appeal_finance_signer_binding: None,
            appeal_finance_checkpoint_binding: None,
            appeal_finance_checkpoint_max_bytes: None,
            pop_credential_runtime_binding: None,
            potr_runtime_binding: None,
            native_signer_binding: None,
            soracloud_runtime_signer_binding: None,
            provider_ingest_signer_binding: None,
            provider_ingest_source_limits: None,
            provider_ingest_checkpoint_max_bytes: None,
            provider_ingest_max_signed_transaction_bytes: None,
            por_replay_archive_binding: None,
            por_replay_archive_proof_limits: None,
            evidence_viewer_webauthn_binding: None,
            evidence_viewer_grant_ttl_ms: None,
            evidence_viewer_receipt_signer_public_key: None,
            evidence_viewer_transparency_publisher_public_key: None,
            evidence_viewer_checkpoint_max_bytes: None,
            moderation_checkpoint_max_bytes: None,
            evidence_viewer_archive_id: None,
            evidence_viewer_archive_public_key: None,
            evidence_viewer_archive_max_bytes: None,
            governance_dag_publisher_peer_id: None,
            governance_dag_publisher_public_key: None,
            governance_request_auth_public_key: None,
            governance_request_auth_max_body_bytes: None,
        })
    }

    fn try_new_governance_dag_signer(
        handle: impl Into<String>,
        revision: u64,
        policy_digest: [u8; 32],
        publisher_peer_id: &str,
        publisher_public_key_hex: &str,
    ) -> Result<Self, IrohaRuntimeProviderRegistryErrorV1> {
        let slot = IrohaRuntimeProviderSlotV1::GovernanceDagSigner;
        if publisher_peer_id.is_empty()
            || publisher_peer_id.len()
                > sorafs_manifest::GOVERNANCE_DAG_PUBLISHER_PEER_ID_MAX_BYTES_V1
            || !publisher_peer_id
                .bytes()
                .all(|byte| byte.is_ascii_graphic())
            || publisher_public_key_hex.len() != 64
            || !publisher_public_key_hex
                .bytes()
                .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'))
        {
            return Err(IrohaRuntimeProviderRegistryErrorV1::InvalidBinding(slot));
        }
        let public_key = iroha_crypto::PublicKey::from_hex(
            iroha_crypto::Algorithm::Ed25519,
            publisher_public_key_hex,
        )
        .map_err(|_| IrohaRuntimeProviderRegistryErrorV1::InvalidBinding(slot))?;
        let (algorithm, public_key_bytes) = public_key.to_bytes();
        let public_key_bytes: [u8; 32] = public_key_bytes
            .try_into()
            .map_err(|_| IrohaRuntimeProviderRegistryErrorV1::InvalidBinding(slot))?;
        if algorithm != iroha_crypto::Algorithm::Ed25519
            || iroha_crypto::ed25519_parse_public_key(&public_key_bytes).is_err()
        {
            return Err(IrohaRuntimeProviderRegistryErrorV1::InvalidBinding(slot));
        }

        let mut projected = Self::try_new(slot, handle, Some(revision), Some(policy_digest))?;
        projected.governance_dag_publisher_peer_id = Some(publisher_peer_id.as_bytes().to_vec());
        projected.governance_dag_publisher_public_key = Some(public_key_bytes);
        Ok(projected)
    }

    fn try_new_stream_token_signer(
        handle: impl Into<String>,
        public_key: [u8; 32],
    ) -> Result<Self, IrohaRuntimeProviderRegistryErrorV1> {
        let slot = IrohaRuntimeProviderSlotV1::StreamTokenSigner;
        if public_key == [0; 32] || iroha_crypto::ed25519_parse_public_key(&public_key).is_err() {
            return Err(IrohaRuntimeProviderRegistryErrorV1::InvalidBinding(slot));
        }
        let mut projected = Self::try_new(slot, handle, None, None)?;
        projected.stream_token_signer_public_key = Some(public_key);
        Ok(projected)
    }

    fn try_new_moderation_checkpoint_store(
        moderation: &iroha_config::parameters::actual::SorafsModerationOrchestrator,
    ) -> Result<Self, IrohaRuntimeProviderRegistryErrorV1> {
        let slot = IrohaRuntimeProviderSlotV1::ModerationCheckpointStore;
        if moderation.checkpoint_max_bytes.0 == 0 {
            return Err(IrohaRuntimeProviderRegistryErrorV1::InvalidBinding(slot));
        }
        let mut projected = Self::try_new(
            slot,
            moderation.checkpoint_store_handle.clone(),
            Some(moderation.checkpoint_store_revision),
            Some(moderation.checkpoint_store_policy_digest),
        )?;
        projected.moderation_checkpoint_max_bytes = Some(moderation.checkpoint_max_bytes.0);
        Ok(projected)
    }

    fn try_new_pop_credential_registry(
        pop: &iroha_config::parameters::actual::SorafsPopCredentialService,
    ) -> Result<Self, IrohaRuntimeProviderRegistryErrorV1> {
        let slot = IrohaRuntimeProviderSlotV1::PopCredentialProviderRegistry;
        let canonical_issuer_id = !pop.issuer_id.is_empty()
            && pop.issuer_id.len()
                <= sorafs_manifest::pop_credentials::POP_IDENTITY_TEXT_MAX_BYTES_V1
            && pop.issuer_id.trim() == pop.issuer_id
            && !pop.issuer_id.chars().any(char::is_control);
        if pop.issuer_policy_digest == [0; 32]
            || !canonical_issuer_id
            || !is_production_runtime_handle(&pop.issuer_hsm_key_id)
            || !is_production_runtime_handle(&pop.enrollment_recipient_key_id)
            || !is_production_runtime_handle(&pop.wallet_wrapping_key_id)
            || pop.enrollment_recipient_public_key_digest == [0; 32]
            || pop.issuer_public_key == [0; 32]
            || iroha_crypto::ed25519_parse_public_key(&pop.issuer_public_key).is_err()
        {
            return Err(IrohaRuntimeProviderRegistryErrorV1::InvalidBinding(slot));
        }
        let mut projected = Self::try_new(
            slot,
            pop.runtime_provider_registry_handle.clone(),
            Some(pop.runtime_provider_registry_revision),
            Some(pop.runtime_provider_registry_policy_digest),
        )?;
        projected.pop_credential_runtime_binding = Some(PopCredentialRuntimeBindingV1 {
            issuer_policy_digest: pop.issuer_policy_digest,
            issuer_id: pop.issuer_id.clone(),
            issuer_hsm_key_id: pop.issuer_hsm_key_id.clone(),
            issuer_public_key: pop.issuer_public_key,
            enrollment_recipient_key_id: pop.enrollment_recipient_key_id.clone(),
            enrollment_recipient_public_key_digest: pop.enrollment_recipient_public_key_digest,
            wallet_wrapping_key_id: pop.wallet_wrapping_key_id.clone(),
        });
        Ok(projected)
    }

    fn try_new_appeal_finance_signer(
        binding: &iroha_config::parameters::actual::SorafsAppealFinanceSignerBinding,
    ) -> Result<Self, IrohaRuntimeProviderRegistryErrorV1> {
        let slot = IrohaRuntimeProviderSlotV1::AppealFinanceTransactionSigner;
        if !matches!(
            binding.public_key.try_algorithm(),
            Ok(iroha_crypto::Algorithm::Ed25519)
        ) || iroha_data_model::account::AccountId::new(binding.public_key.clone())
            != binding.authority
            || binding.valid_from_block_height == 0
            || binding
                .revoked_at_block_height
                .is_some_and(|height| height <= binding.valid_from_block_height)
        {
            return Err(IrohaRuntimeProviderRegistryErrorV1::InvalidBinding(slot));
        }
        let mut projected = Self::try_new(
            slot,
            binding.handle.clone(),
            Some(binding.revision),
            Some(binding.policy_digest),
        )?;
        projected.appeal_finance_signer_binding = Some(binding.clone());
        Ok(projected)
    }

    fn try_new_appeal_finance_checkpoint(
        binding: &iroha_config::parameters::actual::SorafsAppealFinanceCheckpointBinding,
        checkpoint_max_bytes: u64,
    ) -> Result<Self, IrohaRuntimeProviderRegistryErrorV1> {
        let slot = IrohaRuntimeProviderSlotV1::AppealFinanceCheckpoint;
        if !matches!(
            binding.public_key.try_algorithm(),
            Ok(iroha_crypto::Algorithm::Ed25519)
        ) || checkpoint_max_bytes == 0
        {
            return Err(IrohaRuntimeProviderRegistryErrorV1::InvalidBinding(slot));
        }
        let mut projected = Self::try_new(
            slot,
            binding.handle.clone(),
            Some(binding.revision),
            Some(binding.policy_digest),
        )?;
        projected.appeal_finance_checkpoint_binding = Some(binding.clone());
        projected.appeal_finance_checkpoint_max_bytes = Some(checkpoint_max_bytes);
        Ok(projected)
    }

    fn try_new_potr_signer(
        slot: IrohaRuntimeProviderSlotV1,
        runtime: &iroha_config::parameters::actual::SorafsPotrRuntimeBinding,
    ) -> Result<Self, IrohaRuntimeProviderRegistryErrorV1> {
        let signer = match slot {
            IrohaRuntimeProviderSlotV1::PotrGatewaySigner => &runtime.gateway_signer,
            IrohaRuntimeProviderSlotV1::PotrProviderSigner => &runtime.provider_signer,
            _ => return Err(IrohaRuntimeProviderRegistryErrorV1::InvalidBinding(slot)),
        };
        let qualification = iroha_torii::sorafs::PotrRuntimeProviderQualificationV1::new(
            signer.revision,
            signer.policy_digest,
        );
        iroha_torii::sorafs::PotrRuntimeProviderBindingV1::try_new(
            signer.handle.clone(),
            signer.signer_id,
            qualification,
        )
        .map_err(|_| IrohaRuntimeProviderRegistryErrorV1::InvalidBinding(slot))?;
        iroha_torii::sorafs::PotrRuntimeReaderBindingsV1::try_new(
            runtime.reader_id,
            runtime.source_id,
            runtime.resolver_id,
        )
        .map_err(|_| IrohaRuntimeProviderRegistryErrorV1::InvalidBinding(slot))?;
        let admission = &runtime.baseline_admission_policy;
        let admission = sorafs_node::PotrAdmissionPolicyBindingV1 {
            provider_id: admission.provider_id,
            policy_identity: admission.policy_identity,
            policy_digest: admission.policy_digest,
            policy_sequence: admission.policy_sequence,
            finalized_height: admission.finalized_height,
            finalized_block_hash: admission.finalized_block_hash,
            admission_envelope_digest: admission.admission_envelope_digest,
        };
        admission
            .validate()
            .map_err(|_| IrohaRuntimeProviderRegistryErrorV1::InvalidBinding(slot))?;
        if runtime.gateway_signer.handle == runtime.provider_signer.handle
            || runtime.gateway_signer.signer_id == runtime.provider_signer.signer_id
            || runtime.gateway_public_key == [0; 32]
            || iroha_crypto::ed25519_parse_public_key(&runtime.gateway_public_key).is_err()
            || runtime.provider_signer.revision != admission.policy_sequence
            || runtime.provider_signer.policy_digest != admission.policy_digest
        {
            return Err(IrohaRuntimeProviderRegistryErrorV1::InvalidBinding(slot));
        }
        let mut projected = Self::try_new(
            slot,
            signer.handle.clone(),
            Some(signer.revision),
            Some(signer.policy_digest),
        )?;
        projected.potr_runtime_binding = Some(runtime.clone());
        Ok(projected)
    }

    fn try_new_governance_request_auth(
        slot: IrohaRuntimeProviderSlotV1,
        handle: impl Into<String>,
        revision: u64,
        policy_digest: [u8; 32],
        public_key: [u8; 32],
        max_body_bytes: u64,
    ) -> Result<Self, IrohaRuntimeProviderRegistryErrorV1> {
        if !matches!(
            slot,
            IrohaRuntimeProviderSlotV1::GovernanceDagIpfsAuthenticator
                | IrohaRuntimeProviderSlotV1::GovernanceDagHeadAuthenticator
        ) || public_key == [0; 32]
            || max_body_bytes == 0
            || iroha_crypto::ed25519_parse_public_key(&public_key).is_err()
        {
            return Err(IrohaRuntimeProviderRegistryErrorV1::InvalidBinding(slot));
        }
        let mut projected = Self::try_new(slot, handle, Some(revision), Some(policy_digest))?;
        projected.governance_request_auth_public_key = Some(public_key);
        projected.governance_request_auth_max_body_bytes = Some(max_body_bytes);
        Ok(projected)
    }

    fn try_new_evidence_viewer_webauthn(
        viewer: &iroha_config::parameters::actual::SorafsEvidenceViewer,
    ) -> Result<Self, IrohaRuntimeProviderRegistryErrorV1> {
        let slot = IrohaRuntimeProviderSlotV1::EvidenceViewerWebAuthn;
        if viewer.webauthn_rp_id.is_empty()
            || viewer.webauthn_rp_id.len() > 253
            || viewer.webauthn_rp_id.as_bytes().contains(&0)
            || viewer.webauthn_allowed_origins.is_empty()
            || viewer.webauthn_allowed_origins.len() > 16
            || viewer.webauthn_allowed_origins.iter().any(|origin| {
                origin.is_empty() || origin.len() > 512 || origin.as_bytes().contains(&0)
            })
        {
            return Err(IrohaRuntimeProviderRegistryErrorV1::InvalidBinding(slot));
        }
        let challenge_ttl_ms = u64::try_from(viewer.challenge_ttl.as_millis())
            .map_err(|_| IrohaRuntimeProviderRegistryErrorV1::InvalidBinding(slot))?;
        if challenge_ttl_ms == 0 {
            return Err(IrohaRuntimeProviderRegistryErrorV1::InvalidBinding(slot));
        }
        let mut origins = viewer.webauthn_allowed_origins.clone();
        origins.sort();
        origins.dedup();
        if origins != viewer.webauthn_allowed_origins {
            return Err(IrohaRuntimeProviderRegistryErrorV1::InvalidBinding(slot));
        }
        let mut projected = Self::try_new(
            slot,
            viewer.webauthn_handle.clone(),
            Some(viewer.webauthn_revision),
            Some(viewer.webauthn_policy_digest),
        )?;
        projected.evidence_viewer_webauthn_binding = Some(EvidenceViewerWebAuthnBindingV1 {
            rp_id: viewer.webauthn_rp_id.clone(),
            allowed_origins: viewer.webauthn_allowed_origins.clone(),
            challenge_ttl_ms,
        });
        Ok(projected)
    }

    fn try_new_evidence_viewer_grants(
        viewer: &iroha_config::parameters::actual::SorafsEvidenceViewer,
    ) -> Result<Self, IrohaRuntimeProviderRegistryErrorV1> {
        let slot = IrohaRuntimeProviderSlotV1::EvidenceViewerGrantAuthority;
        let grant_ttl_ms = u64::try_from(viewer.grant_ttl.as_millis())
            .map_err(|_| IrohaRuntimeProviderRegistryErrorV1::InvalidBinding(slot))?;
        if grant_ttl_ms == 0 {
            return Err(IrohaRuntimeProviderRegistryErrorV1::InvalidBinding(slot));
        }
        let mut projected = Self::try_new(
            slot,
            viewer.grant_handle.clone(),
            Some(viewer.grant_revision),
            Some(viewer.grant_policy_digest),
        )?;
        projected.evidence_viewer_grant_ttl_ms = Some(grant_ttl_ms);
        Ok(projected)
    }

    fn try_new_evidence_viewer_receipt_signer(
        viewer: &iroha_config::parameters::actual::SorafsEvidenceViewer,
    ) -> Result<Self, IrohaRuntimeProviderRegistryErrorV1> {
        let slot = IrohaRuntimeProviderSlotV1::EvidenceViewerReceiptSigner;
        if viewer.receipt_signer_public_key == [0; 32]
            || iroha_crypto::PublicKey::from_bytes(
                iroha_crypto::Algorithm::Ed25519,
                &viewer.receipt_signer_public_key,
            )
            .is_err()
        {
            return Err(IrohaRuntimeProviderRegistryErrorV1::InvalidBinding(slot));
        }
        let mut projected = Self::try_new(
            slot,
            viewer.receipt_signer_handle.clone(),
            Some(viewer.receipt_signer_revision),
            Some(viewer.receipt_signer_policy_digest),
        )?;
        projected.evidence_viewer_receipt_signer_public_key =
            Some(viewer.receipt_signer_public_key);
        Ok(projected)
    }

    fn try_new_evidence_viewer_checkpoint_store(
        viewer: &iroha_config::parameters::actual::SorafsEvidenceViewer,
    ) -> Result<Self, IrohaRuntimeProviderRegistryErrorV1> {
        let slot = IrohaRuntimeProviderSlotV1::EvidenceViewerCheckpointStore;
        let checkpoint_max_bytes = viewer.checkpoint_max_bytes.0;
        if checkpoint_max_bytes == 0
            || checkpoint_max_bytes
                > sorafs_node::evidence_viewer::EVIDENCE_VIEWER_MAX_CHECKPOINT_BYTES_V1
        {
            return Err(IrohaRuntimeProviderRegistryErrorV1::InvalidBinding(slot));
        }
        let mut projected = Self::try_new(
            slot,
            viewer.checkpoint_store_handle.clone(),
            Some(viewer.checkpoint_store_revision),
            Some(viewer.checkpoint_store_policy_digest),
        )?;
        projected.evidence_viewer_checkpoint_max_bytes = Some(checkpoint_max_bytes);
        Ok(projected)
    }

    fn try_new_evidence_viewer_archive(
        viewer: &iroha_config::parameters::actual::SorafsEvidenceViewer,
    ) -> Result<Self, IrohaRuntimeProviderRegistryErrorV1> {
        let slot = IrohaRuntimeProviderSlotV1::EvidenceViewerCompactionArchive;
        let archive_max_bytes = viewer
            .checkpoint_max_bytes
            .0
            .checked_add(16 * 1024)
            .ok_or(IrohaRuntimeProviderRegistryErrorV1::InvalidBinding(slot))?;
        if viewer.compaction_archive_id == [0; 32]
            || viewer.compaction_archive_public_key == [0; 32]
            || viewer.checkpoint_max_bytes.0 == 0
            || viewer.checkpoint_max_bytes.0
                > sorafs_node::evidence_viewer::EVIDENCE_VIEWER_MAX_CHECKPOINT_BYTES_V1
            || iroha_crypto::PublicKey::from_bytes(
                iroha_crypto::Algorithm::Ed25519,
                &viewer.compaction_archive_public_key,
            )
            .is_err()
        {
            return Err(IrohaRuntimeProviderRegistryErrorV1::InvalidBinding(slot));
        }
        let mut projected = Self::try_new(
            slot,
            viewer.compaction_archive_handle.clone(),
            Some(viewer.compaction_archive_revision),
            Some(viewer.compaction_archive_policy_digest),
        )?;
        projected.evidence_viewer_archive_id = Some(viewer.compaction_archive_id);
        projected.evidence_viewer_archive_public_key = Some(viewer.compaction_archive_public_key);
        projected.evidence_viewer_archive_max_bytes = Some(archive_max_bytes);
        Ok(projected)
    }

    fn try_new_evidence_viewer_transparency_publisher(
        viewer: &iroha_config::parameters::actual::SorafsEvidenceViewer,
    ) -> Result<Self, IrohaRuntimeProviderRegistryErrorV1> {
        let slot = IrohaRuntimeProviderSlotV1::EvidenceViewerTransparencyPublisher;
        if viewer.transparency_publisher_public_key == [0; 32]
            || iroha_crypto::PublicKey::from_bytes(
                iroha_crypto::Algorithm::Ed25519,
                &viewer.transparency_publisher_public_key,
            )
            .is_err()
        {
            return Err(IrohaRuntimeProviderRegistryErrorV1::InvalidBinding(slot));
        }
        let mut projected = Self::try_new(
            slot,
            viewer.transparency_publisher_handle.clone(),
            Some(viewer.transparency_publisher_revision),
            Some(viewer.transparency_publisher_policy_digest),
        )?;
        projected.evidence_viewer_transparency_publisher_public_key =
            Some(viewer.transparency_publisher_public_key);
        Ok(projected)
    }

    fn try_new_por_replay_archive(
        archive: &iroha_config::parameters::actual::SorafsPorReplayArchive,
    ) -> Result<Self, IrohaRuntimeProviderRegistryErrorV1> {
        let slot = IrohaRuntimeProviderSlotV1::PorFinalizedReplayArchive;
        let binding = sorafs_node::PorFinalizedReplayArchiveBindingV1::try_new(
            archive.archive_id,
            archive.revision,
            archive.policy_digest,
            archive.signing_public_key,
        )
        .map_err(|_| IrohaRuntimeProviderRegistryErrorV1::InvalidBinding(slot))?;
        sorafs_node::PorFinalizedReplayArchiveProofBoundsV1::try_new(
            archive.max_successor_receipts,
            archive.max_successor_proof_bytes,
        )
        .map_err(|_| IrohaRuntimeProviderRegistryErrorV1::InvalidBinding(slot))?;
        if archive.max_successor_receipts
            > iroha_config::parameters::defaults::sorafs::storage::por_replay_archive::
                MAX_SUCCESSOR_RECEIPTS_LIMIT
            || archive.max_successor_proof_bytes
                > iroha_config::parameters::defaults::sorafs::storage::por_replay_archive::
                    MAX_SUCCESSOR_PROOF_BYTES_LIMIT
        {
            return Err(IrohaRuntimeProviderRegistryErrorV1::InvalidBinding(slot));
        }
        let mut projected = Self::try_new(
            slot,
            archive.handle.clone(),
            Some(archive.revision),
            Some(archive.policy_digest),
        )?;
        projected.por_replay_archive_binding = Some(binding);
        projected.por_replay_archive_proof_limits = Some(PorReplayArchiveProofLimitsV1 {
            max_successor_receipts: archive.max_successor_receipts,
            max_successor_proof_bytes: archive.max_successor_proof_bytes,
        });
        Ok(projected)
    }

    fn try_new_provider_ingest_source(
        handle: impl Into<String>,
        revision: u64,
        policy_digest: [u8; 32],
        limits: ProviderIngestSourceLimitsV1,
    ) -> Result<Self, IrohaRuntimeProviderRegistryErrorV1> {
        let slot = IrohaRuntimeProviderSlotV1::ProviderIngestAuthenticatedSource;
        if limits.operation_timeout_ms == 0
            || limits.max_content_bytes == 0
            || limits.max_source_providers == 0
            || limits.max_concurrent_streams == 0
            || limits.max_concurrent_streams > MAX_PROVIDER_INGEST_SOURCE_STREAMS_V1
        {
            return Err(IrohaRuntimeProviderRegistryErrorV1::InvalidBinding(slot));
        }
        let mut projected = Self::try_new(slot, handle, Some(revision), Some(policy_digest))?;
        projected.provider_ingest_source_limits = Some(limits);
        Ok(projected)
    }

    fn try_new_native_signer(
        slot: IrohaRuntimeProviderSlotV1,
        binding: iroha_torii::SorafsNativeTransactionSignerBindingV1,
    ) -> Result<Self, IrohaRuntimeProviderRegistryErrorV1> {
        if native_signer_role_for_slot(slot) != Some(binding.role()) {
            return Err(IrohaRuntimeProviderRegistryErrorV1::InvalidBinding(slot));
        }
        let mut projected = Self::try_new(
            slot,
            binding.handle(),
            Some(binding.qualification().revision()),
            Some(binding.qualification().policy_digest()),
        )?;
        projected.native_signer_binding = Some(binding);
        Ok(projected)
    }

    fn try_new_soracloud_runtime_signer(
        binding: &iroha_config::parameters::actual::SoracloudRuntimeMutationSignerBinding,
    ) -> Result<Self, IrohaRuntimeProviderRegistryErrorV1> {
        let slot = IrohaRuntimeProviderSlotV1::SoracloudRuntimeMutationSigner;
        let exact =
            crate::soracloud_runtime_signer::SoracloudRuntimeSignerBindingV1::try_from_config(
                binding,
            )
            .map_err(|_| IrohaRuntimeProviderRegistryErrorV1::InvalidBinding(slot))?;
        let mut projected = Self::try_new(
            slot,
            exact.handle(),
            Some(exact.qualification().revision()),
            Some(exact.qualification().policy_digest()),
        )?;
        projected.soracloud_runtime_signer_binding = Some(exact);
        Ok(projected)
    }

    fn try_new_soracloud_hf_credential_provider(
        binding: &iroha_config::parameters::actual::SoracloudRuntimeHfCredentialProviderBinding,
    ) -> Result<Self, IrohaRuntimeProviderRegistryErrorV1> {
        let slot = IrohaRuntimeProviderSlotV1::SoracloudHfInferenceCredentialProvider;
        let exact =
            crate::soracloud_hf_credential::SoracloudHfCredentialProviderBindingV1::try_from_config(
                binding,
            )
            .map_err(|_| IrohaRuntimeProviderRegistryErrorV1::InvalidBinding(slot))?;
        Self::try_new(
            slot,
            exact.handle(),
            Some(exact.qualification().revision()),
            Some(exact.qualification().policy_digest()),
        )
    }

    fn try_new_provider_ingest_signer(
        slot: IrohaRuntimeProviderSlotV1,
        handle: impl Into<String>,
        revision: u64,
        policy_digest: [u8; 32],
        signer_binding: sorafs_node::ProviderIngestCompletionSignerBindingV1,
        max_signed_transaction_bytes: u64,
    ) -> Result<Self, IrohaRuntimeProviderRegistryErrorV1> {
        if !matches!(
            slot,
            IrohaRuntimeProviderSlotV1::ProviderIngestCompletionSignerResolver
                | IrohaRuntimeProviderSlotV1::ProviderIngestCompletionSigner
        ) || signer_binding.validate().is_err()
            || max_signed_transaction_bytes
                < provider_ingest_outbox_defaults::MAX_SIGNED_TRANSACTION_BYTES_MIN
            || max_signed_transaction_bytes
                > provider_ingest_outbox_defaults::MAX_SIGNED_TRANSACTION_BYTES_LIMIT
        {
            return Err(IrohaRuntimeProviderRegistryErrorV1::InvalidBinding(slot));
        }
        let mut projected = Self::try_new(slot, handle, Some(revision), Some(policy_digest))?;
        projected.provider_ingest_signer_binding = Some(signer_binding);
        projected.provider_ingest_max_signed_transaction_bytes = Some(max_signed_transaction_bytes);
        Ok(projected)
    }

    fn try_new_provider_ingest_checkpoint(
        handle: impl Into<String>,
        revision: u64,
        policy_digest: [u8; 32],
        checkpoint_max_bytes: u64,
    ) -> Result<Self, IrohaRuntimeProviderRegistryErrorV1> {
        let slot = IrohaRuntimeProviderSlotV1::ProviderIngestCheckpointStore;
        if checkpoint_max_bytes == 0
            || checkpoint_max_bytes > provider_ingest_outbox_defaults::CHECKPOINT_MAX_BYTES_LIMIT
        {
            return Err(IrohaRuntimeProviderRegistryErrorV1::InvalidBinding(slot));
        }
        let mut projected = Self::try_new(slot, handle, Some(revision), Some(policy_digest))?;
        projected.provider_ingest_checkpoint_max_bytes = Some(checkpoint_max_bytes);
        Ok(projected)
    }

    /// Return the provider role.
    #[must_use]
    pub const fn slot(&self) -> IrohaRuntimeProviderSlotV1 {
        self.slot
    }

    /// Return the stable non-secret provider handle.
    #[must_use]
    pub fn handle(&self) -> &str {
        &self.handle
    }

    /// Return the exact configured adapter/public-policy revision, when that
    /// service's V1 configuration defines one.
    #[must_use]
    pub const fn revision(&self) -> Option<u64> {
        self.revision
    }

    /// Return the exact configured public-policy digest, when that service's
    /// V1 configuration defines one.
    #[must_use]
    pub const fn policy_digest(&self) -> Option<[u8; 32]> {
        self.policy_digest
    }

    /// Return the exact configured stream-token Ed25519 verification key.
    #[must_use]
    pub const fn stream_token_signer_public_key(&self) -> Option<[u8; 32]> {
        self.stream_token_signer_public_key
    }

    /// Return the exact configured appeal-finance transaction-signer binding.
    pub(crate) const fn appeal_finance_signer_binding(
        &self,
    ) -> Option<&iroha_config::parameters::actual::SorafsAppealFinanceSignerBinding> {
        self.appeal_finance_signer_binding.as_ref()
    }

    /// Return the exact configured appeal-finance checkpoint binding.
    pub(crate) const fn appeal_finance_checkpoint_binding(
        &self,
    ) -> Option<&iroha_config::parameters::actual::SorafsAppealFinanceCheckpointBinding> {
        self.appeal_finance_checkpoint_binding.as_ref()
    }

    /// Return the configured appeal-finance canonical checkpoint byte bound.
    pub(crate) const fn appeal_finance_checkpoint_max_bytes(&self) -> Option<u64> {
        self.appeal_finance_checkpoint_max_bytes
    }

    /// Return the complete public PoTR signer, reader, and finalized-policy pins.
    pub(crate) const fn potr_runtime_binding(
        &self,
    ) -> Option<&iroha_config::parameters::actual::SorafsPotrRuntimeBinding> {
        self.potr_runtime_binding.as_ref()
    }

    /// Return the exact public PoP provider-registry resolution inputs.
    pub(crate) const fn pop_credential_runtime_binding(
        &self,
    ) -> Option<&PopCredentialRuntimeBindingV1> {
        self.pop_credential_runtime_binding.as_ref()
    }

    /// Return the exact role-separated native transaction-signer binding when
    /// this request represents one of the four native signer roles.
    #[must_use]
    pub const fn native_signer_binding(
        &self,
    ) -> Option<&iroha_torii::SorafsNativeTransactionSignerBindingV1> {
        self.native_signer_binding.as_ref()
    }

    /// Return the configured native signer algorithm, when applicable.
    #[must_use]
    pub fn native_signer_algorithm(&self) -> Option<iroha_crypto::Algorithm> {
        self.native_signer_binding
            .as_ref()?
            .public_key()
            .try_algorithm()
            .ok()
    }

    /// Return the exact Soracloud runtime signer binding, when requested.
    #[must_use]
    pub const fn soracloud_runtime_signer_binding(
        &self,
    ) -> Option<&crate::soracloud_runtime_signer::SoracloudRuntimeSignerBindingV1> {
        self.soracloud_runtime_signer_binding.as_ref()
    }

    /// Return the exact governed provider-ingest completion-signer binding.
    pub(crate) fn provider_ingest_signer_binding(
        &self,
    ) -> Option<&sorafs_node::ProviderIngestCompletionSignerBindingV1> {
        self.provider_ingest_signer_binding.as_ref()
    }

    /// Return the configured authenticated-source deadline and resource bounds.
    pub(crate) const fn provider_ingest_source_limits(
        &self,
    ) -> Option<ProviderIngestSourceLimitsV1> {
        self.provider_ingest_source_limits
    }

    /// Return the configured provider-ingest sealed-checkpoint byte bound.
    pub(crate) const fn provider_ingest_checkpoint_max_bytes(&self) -> Option<u64> {
        self.provider_ingest_checkpoint_max_bytes
    }

    /// Return the configured canonical signed-completion-transaction byte bound.
    pub(crate) const fn provider_ingest_max_signed_transaction_bytes(&self) -> Option<u64> {
        self.provider_ingest_max_signed_transaction_bytes
    }

    /// Return the exact configured Governance DAG publisher peer identity.
    #[must_use]
    pub(crate) fn governance_dag_publisher_peer_id(&self) -> Option<&[u8]> {
        self.governance_dag_publisher_peer_id.as_deref()
    }

    /// Return the exact configured Governance DAG publisher verification key.
    #[must_use]
    pub(crate) const fn governance_dag_publisher_public_key(&self) -> Option<[u8; 32]> {
        self.governance_dag_publisher_public_key
    }

    /// Return the exact finalized-PoR archive identity and verification binding.
    #[must_use]
    pub const fn por_replay_archive_binding(
        &self,
    ) -> Option<sorafs_node::PorFinalizedReplayArchiveBindingV1> {
        self.por_replay_archive_binding
    }

    /// Return exact allocation bounds for authenticated lookup/readback proofs.
    #[must_use]
    pub(crate) const fn por_replay_archive_proof_limits(
        &self,
    ) -> Option<PorReplayArchiveProofLimitsV1> {
        self.por_replay_archive_proof_limits
    }

    /// Return the exact public WebAuthn RP/origin policy.
    pub(crate) const fn evidence_viewer_webauthn_binding(
        &self,
    ) -> Option<&EvidenceViewerWebAuthnBindingV1> {
        self.evidence_viewer_webauthn_binding.as_ref()
    }

    /// Return the maximum configured rotating-grant lifetime.
    pub(crate) const fn evidence_viewer_grant_ttl_ms(&self) -> Option<u64> {
        self.evidence_viewer_grant_ttl_ms
    }

    /// Return the exact governed Ed25519 receipt-verification key.
    pub(crate) const fn evidence_viewer_receipt_signer_public_key(&self) -> Option<[u8; 32]> {
        self.evidence_viewer_receipt_signer_public_key
    }

    /// Return the exact governed transparency-publisher verification key.
    pub(crate) const fn evidence_viewer_transparency_publisher_public_key(
        &self,
    ) -> Option<[u8; 32]> {
        self.evidence_viewer_transparency_publisher_public_key
    }

    /// Return the exact configured evidence-viewer checkpoint byte bound.
    pub(crate) const fn evidence_viewer_checkpoint_max_bytes(&self) -> Option<u64> {
        self.evidence_viewer_checkpoint_max_bytes
    }

    /// Return the configured moderation checkpoint byte bound.
    pub(crate) const fn moderation_checkpoint_max_bytes(&self) -> Option<u64> {
        self.moderation_checkpoint_max_bytes
    }

    /// Return the exact non-secret evidence-viewer archive namespace.
    #[must_use]
    pub const fn evidence_viewer_archive_id(&self) -> Option<[u8; 32]> {
        self.evidence_viewer_archive_id
    }

    /// Return the exact Ed25519 evidence-viewer archive verification key.
    #[must_use]
    pub const fn evidence_viewer_archive_public_key(&self) -> Option<[u8; 32]> {
        self.evidence_viewer_archive_public_key
    }

    /// Return the exact canonical compaction-artifact byte bound.
    pub(crate) const fn evidence_viewer_archive_max_bytes(&self) -> Option<u64> {
        self.evidence_viewer_archive_max_bytes
    }

    /// Return the exact Ed25519 key verifying Governance request-auth envelopes.
    #[must_use]
    pub const fn governance_request_auth_public_key(&self) -> Option<[u8; 32]> {
        self.governance_request_auth_public_key
    }

    /// Return the exact maximum request body bytes this authenticator may sign.
    #[must_use]
    pub const fn governance_request_auth_max_body_bytes(&self) -> Option<u64> {
        self.governance_request_auth_max_body_bytes
    }
}

const fn native_signer_role_for_slot(
    slot: IrohaRuntimeProviderSlotV1,
) -> Option<iroha_torii::SorafsNativeTransactionSignerRoleV1> {
    use iroha_torii::SorafsNativeTransactionSignerRoleV1 as Role;

    match slot {
        IrohaRuntimeProviderSlotV1::ProofOutcomeTransactionSigner => Some(Role::ProofOutcome),
        IrohaRuntimeProviderSlotV1::RepairTransactionSigner => Some(Role::Repair),
        IrohaRuntimeProviderSlotV1::ReserveTransactionSigner => Some(Role::Reserve),
        IrohaRuntimeProviderSlotV1::OrderbookTransactionSigner => Some(Role::Orderbook),
        _ => None,
    }
}

/// Sanitized, deterministically ordered provider requests for one node launch.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IrohaRuntimeProviderBindingsV1 {
    chain_id: String,
    bindings: Vec<IrohaRuntimeProviderBindingV1>,
}

#[derive(Clone, Copy)]
struct GovernanceDagRequestAuthBindingProjectionV1<'a> {
    handle: &'a str,
    qualification: sorafs_node::GovernanceDagRuntimeProviderQualificationV1,
    public_key: [u8; 32],
}

#[derive(Clone, Copy)]
struct GovernanceDagServiceBindingProjectionV1<'a> {
    ipfs_authenticator: GovernanceDagRequestAuthBindingProjectionV1<'a>,
    head_authenticator: Option<GovernanceDagRequestAuthBindingProjectionV1<'a>>,
    request_auth_max_body_bytes: u64,
    checkpoint_store_handle: &'a str,
    checkpoint_store_qualification: sorafs_node::GovernanceDagRuntimeProviderQualificationV1,
}

impl IrohaRuntimeProviderBindingsV1 {
    /// Project only stable public provider identities from validated node
    /// configuration.
    ///
    /// # Errors
    ///
    /// Returns an error if an in-memory `Config` was manually substituted
    /// after parsing with a noncanonical, zero-qualified, or test-marked
    /// binding.
    pub fn try_from_config(config: &Config) -> Result<Self, IrohaRuntimeProviderRegistryErrorV1> {
        let mut bindings = Vec::new();
        collect_configured_bindings(config, &mut bindings)?;

        bindings.sort_unstable_by(|left, right| {
            left.slot
                .cmp(&right.slot)
                .then_with(|| left.handle.cmp(&right.handle))
                .then_with(|| left.revision.cmp(&right.revision))
                .then_with(|| left.policy_digest.cmp(&right.policy_digest))
        });
        Ok(Self {
            chain_id: config.common.chain.to_string(),
            bindings,
        })
    }

    /// Project the exact public provider catalog requested by the standalone
    /// Governance DAG service.
    ///
    /// The standalone service does not execute producer signing. This catalog
    /// therefore contains only its IPFS authenticator, optional signed-head
    /// authenticator, and sealed checkpoint store.
    ///
    /// # Errors
    ///
    /// Returns an error when a service binding is incomplete, noncanonical,
    /// zero-qualified, test-marked, or carries an invalid Ed25519 public key or
    /// request-size bound.
    pub fn try_from_governance_dag_service(
        chain_id: &iroha_data_model::ChainId,
        service: &sorafs_node::GovernanceDagServiceRuntimeProviderBindingsV1,
    ) -> Result<Self, IrohaRuntimeProviderRegistryErrorV1> {
        let head_authenticator = match (
            service.head_authenticator_handle(),
            service.head_authenticator_qualification(),
            service.head_request_auth_public_key(),
        ) {
            (Some(handle), Some(qualification), Some(public_key)) => {
                Some(GovernanceDagRequestAuthBindingProjectionV1 {
                    handle,
                    qualification,
                    public_key,
                })
            }
            (None, None, None) => None,
            _ => {
                return Err(IrohaRuntimeProviderRegistryErrorV1::InvalidBinding(
                    IrohaRuntimeProviderSlotV1::GovernanceDagHeadAuthenticator,
                ));
            }
        };
        Self::try_from_governance_dag_service_projection(
            chain_id,
            GovernanceDagServiceBindingProjectionV1 {
                ipfs_authenticator: GovernanceDagRequestAuthBindingProjectionV1 {
                    handle: service.ipfs_authenticator_handle(),
                    qualification: service.ipfs_authenticator_qualification(),
                    public_key: service.ipfs_request_auth_public_key(),
                },
                head_authenticator,
                request_auth_max_body_bytes: service.request_auth_max_body_bytes(),
                checkpoint_store_handle: service.checkpoint_store_handle(),
                checkpoint_store_qualification: service.checkpoint_store_qualification(),
            },
        )
    }

    fn try_from_governance_dag_service_projection(
        chain_id: &iroha_data_model::ChainId,
        service: GovernanceDagServiceBindingProjectionV1<'_>,
    ) -> Result<Self, IrohaRuntimeProviderRegistryErrorV1> {
        let mut bindings = Vec::with_capacity(3);
        append_required_governance_request_auth_binding(
            &mut bindings,
            IrohaRuntimeProviderSlotV1::GovernanceDagIpfsAuthenticator,
            Some(service.ipfs_authenticator.handle),
            Some(service.ipfs_authenticator.qualification.revision),
            Some(service.ipfs_authenticator.qualification.policy_digest),
            Some(service.ipfs_authenticator.public_key),
            service.request_auth_max_body_bytes,
        )?;
        if let Some(head_authenticator) = service.head_authenticator {
            append_required_governance_request_auth_binding(
                &mut bindings,
                IrohaRuntimeProviderSlotV1::GovernanceDagHeadAuthenticator,
                Some(head_authenticator.handle),
                Some(head_authenticator.qualification.revision),
                Some(head_authenticator.qualification.policy_digest),
                Some(head_authenticator.public_key),
                service.request_auth_max_body_bytes,
            )?;
        }
        append_required_governance_service_binding(
            &mut bindings,
            IrohaRuntimeProviderSlotV1::GovernanceDagCheckpointStore,
            Some(service.checkpoint_store_handle),
            Some(service.checkpoint_store_qualification.revision),
            Some(service.checkpoint_store_qualification.policy_digest),
        )?;
        bindings.sort_unstable_by(|left, right| {
            left.slot
                .cmp(&right.slot)
                .then_with(|| left.handle.cmp(&right.handle))
                .then_with(|| left.revision.cmp(&right.revision))
                .then_with(|| left.policy_digest.cmp(&right.policy_digest))
        });
        Ok(Self {
            chain_id: chain_id.to_string(),
            bindings,
        })
    }

    /// Return the public chain identity associated with this resolution.
    #[must_use]
    pub fn chain_id(&self) -> &str {
        &self.chain_id
    }

    /// Iterate over the stable, deterministically ordered provider requests.
    pub fn iter(&self) -> impl ExactSizeIterator<Item = &IrohaRuntimeProviderBindingV1> {
        self.bindings.iter()
    }

    /// Return whether the validated configuration requests no external
    /// runtime provider.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.bindings.is_empty()
    }

    /// Return the number of requested provider bindings.
    #[must_use]
    pub fn len(&self) -> usize {
        self.bindings.len()
    }

    /// Construct an empty test catalog without loading daemon configuration.
    #[cfg(test)]
    pub(crate) fn empty_for_test(chain_id: impl Into<String>) -> Self {
        Self {
            chain_id: chain_id.into(),
            bindings: Vec::new(),
        }
    }

    /// Construct one exactly qualified test catalog.
    #[cfg(test)]
    pub(crate) fn qualified_for_test(
        chain_id: impl Into<String>,
        slot: IrohaRuntimeProviderSlotV1,
        handle: impl Into<String>,
        revision: u64,
        policy_digest: [u8; 32],
    ) -> Self {
        Self {
            chain_id: chain_id.into(),
            bindings: vec![
                IrohaRuntimeProviderBindingV1::try_new(
                    slot,
                    handle,
                    Some(revision),
                    Some(policy_digest),
                )
                .expect("test binding must be production-shaped"),
            ],
        }
    }

    /// Construct one exactly qualified evidence-viewer transparency publisher
    /// test catalog.
    #[cfg(test)]
    pub(crate) fn qualified_evidence_viewer_transparency_publisher_for_test(
        chain_id: impl Into<String>,
        handle: impl Into<String>,
        revision: u64,
        policy_digest: [u8; 32],
        public_key: [u8; 32],
    ) -> Self {
        let mut binding = IrohaRuntimeProviderBindingV1::try_new(
            IrohaRuntimeProviderSlotV1::EvidenceViewerTransparencyPublisher,
            handle,
            Some(revision),
            Some(policy_digest),
        )
        .expect("test transparency-publisher binding must be production-shaped");
        binding.evidence_viewer_transparency_publisher_public_key = Some(public_key);
        Self {
            chain_id: chain_id.into(),
            bindings: vec![binding],
        }
    }

    /// Construct one exactly qualified Governance DAG request-auth test catalog.
    #[cfg(test)]
    pub(crate) fn qualified_governance_request_auth_for_test(
        chain_id: impl Into<String>,
        slot: IrohaRuntimeProviderSlotV1,
        handle: impl Into<String>,
        revision: u64,
        policy_digest: [u8; 32],
        public_key: [u8; 32],
        max_body_bytes: u64,
    ) -> Self {
        Self {
            chain_id: chain_id.into(),
            bindings: vec![
                IrohaRuntimeProviderBindingV1::try_new_governance_request_auth(
                    slot,
                    handle,
                    revision,
                    policy_digest,
                    public_key,
                    max_body_bytes,
                )
                .expect("test request-auth binding must be production-shaped"),
            ],
        }
    }

    /// Construct an exactly qualified native transaction-signer test catalog.
    #[cfg(test)]
    pub(crate) fn qualified_native_transaction_signers_for_test(
        chain_id: impl Into<String>,
        entries: impl IntoIterator<
            Item = (
                IrohaRuntimeProviderSlotV1,
                iroha_torii::SorafsNativeTransactionSignerBindingV1,
            ),
        >,
    ) -> Self {
        let mut bindings = entries
            .into_iter()
            .map(|(slot, binding)| {
                IrohaRuntimeProviderBindingV1::try_new_native_signer(slot, binding)
                    .expect("test native signer binding must be production-shaped")
            })
            .collect::<Vec<_>>();
        bindings.sort_unstable_by_key(IrohaRuntimeProviderBindingV1::slot);
        Self {
            chain_id: chain_id.into(),
            bindings,
        }
    }

    /// Construct one exactly qualified authenticated-source test catalog.
    #[cfg(test)]
    pub(crate) fn qualified_provider_ingest_source_for_test(
        chain_id: impl Into<String>,
        handle: impl Into<String>,
        revision: u64,
        policy_digest: [u8; 32],
        limits: ProviderIngestSourceLimitsV1,
    ) -> Self {
        Self {
            chain_id: chain_id.into(),
            bindings: vec![
                IrohaRuntimeProviderBindingV1::try_new_provider_ingest_source(
                    handle,
                    revision,
                    policy_digest,
                    limits,
                )
                .expect("test source binding must be production-shaped"),
            ],
        }
    }
}

/// Payload-free failure returned by the deployment registry or launcher.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum IrohaRuntimeProviderRegistryErrorV1 {
    /// One configured binding was noncanonical, zero-qualified, or test-marked.
    InvalidBinding(IrohaRuntimeProviderSlotV1),
    /// At least one provider binding was configured but no registry was supplied.
    MissingRegistry,
    /// The deployment registry could not be reached or initialized.
    Unavailable,
    /// A configured or resolved binding has a different handle, policy, or role.
    BindingMismatch,
    /// A configured or resolved provider binding is stale or revoked.
    StaleOrRevoked,
    /// A test/development binding was presented to a production launch.
    TestProviderRejected,
    /// One or more configured provider bindings had no resolved dependency.
    IncompleteResolution,
    /// Dependencies were returned even though no provider binding was configured.
    UnexpectedProviders,
}

impl fmt::Display for IrohaRuntimeProviderRegistryErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            Self::InvalidBinding(_) => "runtime-provider binding is invalid",
            Self::MissingRegistry => {
                "runtime-provider bindings are configured but no deployment registry was supplied"
            }
            Self::Unavailable => "deployment runtime-provider registry is unavailable",
            Self::BindingMismatch => "runtime-provider binding is substituted",
            Self::StaleOrRevoked => "runtime-provider binding is stale or revoked",
            Self::TestProviderRejected => "runtime-provider binding is test-marked",
            Self::IncompleteResolution => {
                "deployment runtime-provider registry returned an incomplete dependency set"
            }
            Self::UnexpectedProviders => {
                "deployment runtime-provider registry returned dependencies without configured bindings"
            }
        };
        formatter.write_str(message)
    }
}

impl std::error::Error for IrohaRuntimeProviderRegistryErrorV1 {}

/// Deployment-owned factory for runtime-only daemon adapters.
///
/// Implementations should use the stable handles in `bindings` to locate
/// already-provisioned adapters whose credentials and private keys remain
/// inside the deployment runtime. This registry boundary validates only the
/// sanitized request/result shape. Service-owned constructors perform the
/// binding checks their public interfaces support; callers must not treat
/// successful registry resolution as independent provider attestation.
/// The launcher additionally wraps each configured native proof, repair,
/// reserve, and orderbook signer in Torii's immutable exact-binding facade
/// before forwarding the dependency set. The Soracloud mutation signer is
/// likewise bound to its exact authority and key plus an active, non-test
/// qualification before the runtime manager can start.
///
/// Governance DAG signers are independently cross-bound here to the configured
/// handle, qualification, publisher peer identity, and Ed25519 public key.
/// Stream-token providers currently have only handle/public-key binding.
pub trait IrohaRuntimeProviderRegistryV1: Send + Sync {
    /// Resolve the complete dependency set for one standard daemon launch.
    ///
    /// # Errors
    ///
    /// Returns a payload-free error when the registry is unavailable or any
    /// requested provider is missing, substituted, stale, revoked, or
    /// test-marked.
    fn resolve(
        &self,
        bindings: &IrohaRuntimeProviderBindingsV1,
    ) -> Result<IrohaRuntimeDeps, IrohaRuntimeProviderRegistryErrorV1>;
}

/// Resolve standard-launcher dependencies without exposing the full config to
/// the deployment registry.
///
/// # Errors
///
/// Returns a payload-free error when a configured binding is invalid, no
/// deployment registry is available, or the registry's resolution is not an
/// exact match for the sanitized binding catalog.
pub(crate) fn resolve_runtime_deps(
    config: &Config,
    registry: Option<&dyn IrohaRuntimeProviderRegistryV1>,
) -> Result<IrohaRuntimeDeps, IrohaRuntimeProviderRegistryErrorV1> {
    let bindings = IrohaRuntimeProviderBindingsV1::try_from_config(config)?;
    resolve_runtime_deps_from_bindings(&bindings, registry)
}

pub(crate) fn resolve_runtime_deps_from_bindings(
    bindings: &IrohaRuntimeProviderBindingsV1,
    registry: Option<&dyn IrohaRuntimeProviderRegistryV1>,
) -> Result<IrohaRuntimeDeps, IrohaRuntimeProviderRegistryErrorV1> {
    validate_fenced_privacy_binding_pair(bindings)?;
    let Some(registry) = registry else {
        return if bindings.is_empty() {
            Ok(IrohaRuntimeDeps::default())
        } else {
            Err(IrohaRuntimeProviderRegistryErrorV1::MissingRegistry)
        };
    };

    let mut dependencies = registry.resolve(bindings)?;
    if has_unrequested_dependency(bindings, &dependencies) {
        return Err(IrohaRuntimeProviderRegistryErrorV1::UnexpectedProviders);
    }
    if bindings
        .iter()
        .any(|binding| !dependency_is_present(&dependencies, binding.slot()))
    {
        return Err(IrohaRuntimeProviderRegistryErrorV1::IncompleteResolution);
    }
    qualify_fenced_privacy_dependencies(bindings, &dependencies)?;
    qualify_governance_dag_signer_dependency(bindings, &dependencies)?;
    qualify_governance_request_auth_dependencies(bindings, &dependencies)?;
    qualify_native_transaction_signers(bindings, &mut dependencies)?;
    qualify_soracloud_runtime_signer(bindings, &mut dependencies)?;
    qualify_soracloud_hf_credential_provider(bindings, &mut dependencies)?;
    qualify_moderation_checkpoint_dependency(bindings, &dependencies)?;
    qualify_provider_ingest_dependencies(bindings, &dependencies)?;
    qualify_reputation_journal_checkpoint_dependency(bindings, &dependencies)?;
    qualify_reputation_retention_dependency(bindings, &dependencies)?;
    qualify_por_replay_archive_dependency(bindings, &dependencies)?;
    qualify_evidence_viewer_archive_dependency(bindings, &dependencies)?;
    qualify_evidence_viewer_transparency_publisher_dependency(bindings, &dependencies)?;
    Ok(dependencies)
}

fn map_soracloud_runtime_signer_qualification_error(
    error: crate::soracloud_runtime_signer::SoracloudRuntimeSignerQualificationErrorV1,
) -> IrohaRuntimeProviderRegistryErrorV1 {
    use crate::soracloud_runtime_signer::SoracloudRuntimeSignerQualificationErrorV1 as Error;

    match error {
        Error::ProviderUnavailable => IrohaRuntimeProviderRegistryErrorV1::Unavailable,
        Error::InvalidProviderHandle | Error::TestProviderRejected => {
            IrohaRuntimeProviderRegistryErrorV1::TestProviderRejected
        }
        Error::InvalidProviderQualification
        | Error::ProviderInactive
        | Error::RevisionMismatch
        | Error::PolicyDigestMismatch
        | Error::ProviderDrift => IrohaRuntimeProviderRegistryErrorV1::StaleOrRevoked,
        Error::UnsupportedProviderKeyAlgorithm
        | Error::HandleMismatch
        | Error::AuthorityMismatch
        | Error::PublicKeyMismatch
        | Error::ProviderAuthorityKeyMismatch => {
            IrohaRuntimeProviderRegistryErrorV1::BindingMismatch
        }
    }
}

fn qualify_moderation_checkpoint_dependency(
    bindings: &IrohaRuntimeProviderBindingsV1,
    dependencies: &IrohaRuntimeDeps,
) -> Result<(), IrohaRuntimeProviderRegistryErrorV1> {
    let Some(expected) = bindings
        .iter()
        .find(|binding| binding.slot() == IrohaRuntimeProviderSlotV1::ModerationCheckpointStore)
    else {
        return Ok(());
    };
    let store = dependencies
        .sorafs_moderation_checkpoint_store
        .as_ref()
        .ok_or(IrohaRuntimeProviderRegistryErrorV1::IncompleteResolution)?;
    let revision =
        expected
            .revision()
            .ok_or(IrohaRuntimeProviderRegistryErrorV1::InvalidBinding(
                expected.slot(),
            ))?;
    let policy_digest =
        expected
            .policy_digest()
            .ok_or(IrohaRuntimeProviderRegistryErrorV1::InvalidBinding(
                expected.slot(),
            ))?;
    let qualification =
        sorafs_node::moderation_orchestrator::ModerationRuntimeProviderQualificationV1::new(
            revision,
            policy_digest,
        );
    sorafs_node::moderation_orchestrator::qualify_moderation_runtime_provider_v1(
        expected.handle(),
        qualification,
        store.as_ref(),
    )
    .map_err(|_| IrohaRuntimeProviderRegistryErrorV1::BindingMismatch)?;
    let latest = store.load_latest().map_err(|error| match error {
        sorafs_node::moderation_orchestrator::ModerationCheckpointStoreExternalErrorV1::Unavailable => {
            IrohaRuntimeProviderRegistryErrorV1::Unavailable
        }
        sorafs_node::moderation_orchestrator::ModerationCheckpointStoreExternalErrorV1::Rejected
        | sorafs_node::moderation_orchestrator::ModerationCheckpointStoreExternalErrorV1::Ambiguous => {
            IrohaRuntimeProviderRegistryErrorV1::StaleOrRevoked
        }
    })?;
    if let Some(record) = latest {
        let max_bytes = expected.moderation_checkpoint_max_bytes().ok_or(
            IrohaRuntimeProviderRegistryErrorV1::InvalidBinding(expected.slot()),
        )?;
        let encoded = norito::to_bytes(&record)
            .map_err(|_| IrohaRuntimeProviderRegistryErrorV1::BindingMismatch)?;
        if !record.has_valid_provider_envelope(expected.handle(), qualification, max_bytes)
            || encoded.len()
                > usize::try_from(max_bytes)
                    .unwrap_or(usize::MAX)
                    .saturating_add(4 * 1024)
        {
            return Err(IrohaRuntimeProviderRegistryErrorV1::BindingMismatch);
        }
    }
    sorafs_node::moderation_orchestrator::revalidate_moderation_runtime_provider_v1(
        expected.handle(),
        qualification,
        store.as_ref(),
    )
    .map_err(|_| IrohaRuntimeProviderRegistryErrorV1::StaleOrRevoked)
}

fn qualify_soracloud_runtime_signer(
    bindings: &IrohaRuntimeProviderBindingsV1,
    dependencies: &mut IrohaRuntimeDeps,
) -> Result<(), IrohaRuntimeProviderRegistryErrorV1> {
    let slot = IrohaRuntimeProviderSlotV1::SoracloudRuntimeMutationSigner;
    let Some(binding) = bindings.iter().find(|binding| binding.slot() == slot) else {
        return Ok(());
    };
    let exact = binding
        .soracloud_runtime_signer_binding()
        .ok_or(IrohaRuntimeProviderRegistryErrorV1::InvalidBinding(slot))?;
    let provider = dependencies
        .soracloud_runtime_mutation_signer
        .take()
        .ok_or(IrohaRuntimeProviderRegistryErrorV1::IncompleteResolution)?;
    dependencies.soracloud_runtime_mutation_signer = Some(
        crate::soracloud_runtime_signer::qualify_soracloud_runtime_mutation_signer_v1(
            exact.clone(),
            provider,
        )
        .map_err(map_soracloud_runtime_signer_qualification_error)?,
    );
    Ok(())
}

fn map_soracloud_hf_credential_qualification_error(
    error: crate::soracloud_hf_credential::SoracloudHfCredentialProviderQualificationErrorV1,
) -> IrohaRuntimeProviderRegistryErrorV1 {
    use crate::soracloud_hf_credential::SoracloudHfCredentialProviderQualificationErrorV1 as Error;

    match error {
        Error::ProviderUnavailable => IrohaRuntimeProviderRegistryErrorV1::Unavailable,
        Error::InvalidProviderHandle | Error::TestProviderRejected => {
            IrohaRuntimeProviderRegistryErrorV1::TestProviderRejected
        }
        Error::InvalidProviderQualification
        | Error::ProviderInactive
        | Error::RevisionMismatch
        | Error::PolicyDigestMismatch
        | Error::ProviderDrift => IrohaRuntimeProviderRegistryErrorV1::StaleOrRevoked,
        Error::HandleMismatch => IrohaRuntimeProviderRegistryErrorV1::BindingMismatch,
    }
}

fn qualify_soracloud_hf_credential_provider(
    bindings: &IrohaRuntimeProviderBindingsV1,
    dependencies: &mut IrohaRuntimeDeps,
) -> Result<(), IrohaRuntimeProviderRegistryErrorV1> {
    let slot = IrohaRuntimeProviderSlotV1::SoracloudHfInferenceCredentialProvider;
    let Some(binding) = bindings.iter().find(|binding| binding.slot() == slot) else {
        return Ok(());
    };
    let exact = crate::soracloud_hf_credential::SoracloudHfCredentialProviderBindingV1::try_new(
        binding.handle(),
        crate::soracloud_hf_credential::SoracloudHfCredentialProviderQualificationV1::new(
            binding
                .revision()
                .ok_or(IrohaRuntimeProviderRegistryErrorV1::InvalidBinding(slot))?,
            binding
                .policy_digest()
                .ok_or(IrohaRuntimeProviderRegistryErrorV1::InvalidBinding(slot))?,
            true,
            false,
        ),
    )
    .map_err(map_soracloud_hf_credential_qualification_error)?;
    let provider = dependencies
        .soracloud_hf_inference_credential_provider
        .take()
        .ok_or(IrohaRuntimeProviderRegistryErrorV1::IncompleteResolution)?;
    dependencies.soracloud_hf_inference_credential_provider = Some(
        crate::soracloud_hf_credential::qualify_soracloud_hf_inference_credential_provider_v1(
            exact, provider,
        )
        .map_err(map_soracloud_hf_credential_qualification_error)?,
    );
    Ok(())
}

fn qualify_governance_dag_signer_dependency(
    bindings: &IrohaRuntimeProviderBindingsV1,
    dependencies: &IrohaRuntimeDeps,
) -> Result<(), IrohaRuntimeProviderRegistryErrorV1> {
    let slot = IrohaRuntimeProviderSlotV1::GovernanceDagSigner;
    let Some(expected) = bindings.iter().find(|binding| binding.slot() == slot) else {
        return Ok(());
    };
    let signer = dependencies
        .sorafs_governance_dag_signer
        .as_ref()
        .ok_or(IrohaRuntimeProviderRegistryErrorV1::IncompleteResolution)?;
    let expected_revision = expected
        .revision()
        .ok_or(IrohaRuntimeProviderRegistryErrorV1::InvalidBinding(slot))?;
    let expected_policy_digest = expected
        .policy_digest()
        .ok_or(IrohaRuntimeProviderRegistryErrorV1::InvalidBinding(slot))?;
    let expected_publisher_peer_id = expected
        .governance_dag_publisher_peer_id()
        .ok_or(IrohaRuntimeProviderRegistryErrorV1::InvalidBinding(slot))?;
    let expected_public_key = expected
        .governance_dag_publisher_public_key()
        .ok_or(IrohaRuntimeProviderRegistryErrorV1::InvalidBinding(slot))?;

    let observe = || {
        let handle = signer.handle().to_owned();
        if !is_production_runtime_handle(&handle) {
            return Err(IrohaRuntimeProviderRegistryErrorV1::TestProviderRejected);
        }
        let qualification = signer
            .qualification()
            .map_err(|_| IrohaRuntimeProviderRegistryErrorV1::Unavailable)?;
        Ok((
            handle,
            qualification,
            signer.publisher_peer_id().to_vec(),
            signer.public_key(),
        ))
    };
    let first = observe()?;
    if first.0 != expected.handle()
        || first.2.as_slice() != expected_publisher_peer_id
        || first.3 != expected_public_key
    {
        return Err(IrohaRuntimeProviderRegistryErrorV1::BindingMismatch);
    }
    if first.1.revision == 0
        || first.1.policy_digest == [0; 32]
        || first.1.revision != expected_revision
        || first.1.policy_digest != expected_policy_digest
    {
        return Err(IrohaRuntimeProviderRegistryErrorV1::StaleOrRevoked);
    }
    let mut nonce = [0_u8; 32];
    OsRng
        .try_fill_bytes(&mut nonce)
        .map_err(|_| IrohaRuntimeProviderRegistryErrorV1::Unavailable)?;
    let challenge = governance_dag_signer_startup_challenge_v1(
        nonce,
        bindings.chain_id(),
        expected.handle(),
        expected_revision,
        expected_policy_digest,
        expected_publisher_peer_id,
        expected_public_key,
    )?;
    let signature_result = signer.sign(&challenge);
    let second = observe()?;
    if second != first {
        return Err(IrohaRuntimeProviderRegistryErrorV1::StaleOrRevoked);
    }
    let signature_bytes =
        signature_result.map_err(|_| IrohaRuntimeProviderRegistryErrorV1::Unavailable)?;
    let public_key =
        iroha_crypto::PublicKey::from_bytes(iroha_crypto::Algorithm::Ed25519, &expected_public_key)
            .map_err(|_| IrohaRuntimeProviderRegistryErrorV1::InvalidBinding(slot))?;
    let signature = iroha_crypto::Signature::try_from_bytes(&signature_bytes)
        .map_err(|_| IrohaRuntimeProviderRegistryErrorV1::BindingMismatch)?;
    signature
        .verify(&public_key, &challenge)
        .map_err(|_| IrohaRuntimeProviderRegistryErrorV1::BindingMismatch)?;
    Ok(())
}

fn governance_dag_signer_startup_challenge_v1(
    nonce: [u8; 32],
    chain_id: &str,
    handle: &str,
    revision: u64,
    policy_digest: [u8; 32],
    publisher_peer_id: &[u8],
    public_key: [u8; 32],
) -> Result<[u8; 32], IrohaRuntimeProviderRegistryErrorV1> {
    let slot = IrohaRuntimeProviderSlotV1::GovernanceDagSigner;
    let mut hasher = blake3::Hasher::new();
    hasher.update(GOVERNANCE_DAG_SIGNER_STARTUP_CHALLENGE_DOMAIN_V1);
    hasher.update(&nonce);
    for component in [chain_id.as_bytes(), handle.as_bytes(), publisher_peer_id] {
        let length = u64::try_from(component.len())
            .map_err(|_| IrohaRuntimeProviderRegistryErrorV1::InvalidBinding(slot))?;
        hasher.update(&length.to_le_bytes());
        hasher.update(component);
    }
    hasher.update(&revision.to_le_bytes());
    hasher.update(&policy_digest);
    hasher.update(&public_key);
    Ok(*hasher.finalize().as_bytes())
}

fn qualify_governance_request_auth_dependencies(
    bindings: &IrohaRuntimeProviderBindingsV1,
    dependencies: &IrohaRuntimeDeps,
) -> Result<(), IrohaRuntimeProviderRegistryErrorV1> {
    use IrohaRuntimeProviderSlotV1 as Slot;

    for (slot, authenticator) in [
        (
            Slot::GovernanceDagIpfsAuthenticator,
            dependencies
                .sorafs_governance_dag_ipfs_authenticator
                .as_ref(),
        ),
        (
            Slot::GovernanceDagHeadAuthenticator,
            dependencies
                .sorafs_governance_dag_head_authenticator
                .as_ref(),
        ),
    ] {
        let Some(expected) = bindings.iter().find(|binding| binding.slot() == slot) else {
            continue;
        };
        let authenticator =
            authenticator.ok_or(IrohaRuntimeProviderRegistryErrorV1::IncompleteResolution)?;
        if !is_production_runtime_handle(authenticator.handle()) {
            return Err(IrohaRuntimeProviderRegistryErrorV1::TestProviderRejected);
        }
        let expected_revision = expected
            .revision()
            .ok_or(IrohaRuntimeProviderRegistryErrorV1::InvalidBinding(slot))?;
        let expected_policy_digest = expected
            .policy_digest()
            .ok_or(IrohaRuntimeProviderRegistryErrorV1::InvalidBinding(slot))?;
        let expected_public_key = expected
            .governance_request_auth_public_key()
            .ok_or(IrohaRuntimeProviderRegistryErrorV1::InvalidBinding(slot))?;
        if expected
            .governance_request_auth_max_body_bytes()
            .is_none_or(|bound| bound == 0)
        {
            return Err(IrohaRuntimeProviderRegistryErrorV1::InvalidBinding(slot));
        }
        let observe = || {
            let qualification = authenticator
                .qualification()
                .map_err(|_| IrohaRuntimeProviderRegistryErrorV1::Unavailable)?;
            if authenticator.handle() != expected.handle()
                || authenticator.public_key() != expected_public_key
            {
                return Err(IrohaRuntimeProviderRegistryErrorV1::BindingMismatch);
            }
            if qualification.revision != expected_revision
                || qualification.policy_digest != expected_policy_digest
            {
                return Err(IrohaRuntimeProviderRegistryErrorV1::StaleOrRevoked);
            }
            Ok(qualification)
        };
        let first = observe()?;
        if observe()? != first {
            return Err(IrohaRuntimeProviderRegistryErrorV1::StaleOrRevoked);
        }
    }
    Ok(())
}

fn fenced_privacy_binding_pair(
    bindings: &IrohaRuntimeProviderBindingsV1,
) -> Result<
    Option<(
        &IrohaRuntimeProviderBindingV1,
        &IrohaRuntimeProviderBindingV1,
    )>,
    IrohaRuntimeProviderRegistryErrorV1,
> {
    let writer = bindings
        .iter()
        .find(|binding| binding.slot() == IrohaRuntimeProviderSlotV1::FencedPrivacyPublisher);
    let reader = bindings
        .iter()
        .find(|binding| binding.slot() == IrohaRuntimeProviderSlotV1::FencedPrivacyHeadReader);
    match (writer, reader) {
        (None, None) => Ok(None),
        (Some(writer), Some(reader)) => Ok(Some((writer, reader))),
        (Some(_), None) | (None, Some(_)) => {
            Err(IrohaRuntimeProviderRegistryErrorV1::IncompleteResolution)
        }
    }
}

fn validate_fenced_privacy_binding_pair(
    bindings: &IrohaRuntimeProviderBindingsV1,
) -> Result<(), IrohaRuntimeProviderRegistryErrorV1> {
    let Some((writer, reader)) = fenced_privacy_binding_pair(bindings)? else {
        return Ok(());
    };
    if writer.handle() != reader.handle()
        || writer.revision() != reader.revision()
        || writer.policy_digest() != reader.policy_digest()
    {
        return Err(IrohaRuntimeProviderRegistryErrorV1::BindingMismatch);
    }
    Ok(())
}

fn validate_fenced_privacy_provider_observation(
    binding: &IrohaRuntimeProviderBindingV1,
    provider_handle: &str,
    qualification: Result<sorafs_node::GovernanceDagRuntimeProviderQualificationV1, String>,
) -> Result<(), IrohaRuntimeProviderRegistryErrorV1> {
    if !is_production_runtime_handle(provider_handle) {
        return Err(IrohaRuntimeProviderRegistryErrorV1::TestProviderRejected);
    }
    if provider_handle != binding.handle() {
        return Err(IrohaRuntimeProviderRegistryErrorV1::BindingMismatch);
    }
    let qualification =
        qualification.map_err(|_| IrohaRuntimeProviderRegistryErrorV1::Unavailable)?;
    let expected_revision =
        binding
            .revision()
            .ok_or(IrohaRuntimeProviderRegistryErrorV1::InvalidBinding(
                binding.slot(),
            ))?;
    let expected_policy_digest =
        binding
            .policy_digest()
            .ok_or(IrohaRuntimeProviderRegistryErrorV1::InvalidBinding(
                binding.slot(),
            ))?;
    if qualification.revision == 0
        || qualification.policy_digest == [0; 32]
        || qualification.revision != expected_revision
        || qualification.policy_digest != expected_policy_digest
    {
        return Err(IrohaRuntimeProviderRegistryErrorV1::StaleOrRevoked);
    }
    Ok(())
}

fn qualify_fenced_privacy_dependencies(
    bindings: &IrohaRuntimeProviderBindingsV1,
    dependencies: &IrohaRuntimeDeps,
) -> Result<(), IrohaRuntimeProviderRegistryErrorV1> {
    let Some((writer_binding, reader_binding)) = fenced_privacy_binding_pair(bindings)? else {
        return Ok(());
    };
    let writer = dependencies
        .sorafs_fenced_transparency_publisher
        .as_ref()
        .ok_or(IrohaRuntimeProviderRegistryErrorV1::IncompleteResolution)?;
    validate_fenced_privacy_provider_observation(
        writer_binding,
        writer.handle(),
        writer.qualification(),
    )?;
    let reader = dependencies
        .sorafs_fenced_transparency_head_reader
        .as_ref()
        .ok_or(IrohaRuntimeProviderRegistryErrorV1::IncompleteResolution)?;
    validate_fenced_privacy_provider_observation(
        reader_binding,
        reader.handle(),
        reader.qualification(),
    )?;
    Ok(())
}

fn native_signer_binding_for_slot(
    bindings: &IrohaRuntimeProviderBindingsV1,
    slot: IrohaRuntimeProviderSlotV1,
) -> Result<
    Option<&iroha_torii::SorafsNativeTransactionSignerBindingV1>,
    IrohaRuntimeProviderRegistryErrorV1,
> {
    let Some(binding) = bindings.iter().find(|binding| binding.slot() == slot) else {
        return Ok(None);
    };
    binding
        .native_signer_binding()
        .map(Some)
        .ok_or(IrohaRuntimeProviderRegistryErrorV1::InvalidBinding(slot))
}

fn map_native_signer_qualification_error(
    error: iroha_torii::SorafsNativeTransactionSignerQualificationErrorV1,
) -> IrohaRuntimeProviderRegistryErrorV1 {
    use iroha_torii::SorafsNativeTransactionSignerQualificationErrorV1 as Error;

    match error {
        Error::ProviderUnavailable => IrohaRuntimeProviderRegistryErrorV1::Unavailable,
        Error::InvalidProviderHandle => IrohaRuntimeProviderRegistryErrorV1::TestProviderRejected,
        Error::InvalidProviderQualification
        | Error::RevisionMismatch
        | Error::PolicyDigestMismatch
        | Error::ProviderDrift => IrohaRuntimeProviderRegistryErrorV1::StaleOrRevoked,
        Error::BindingRoleMismatch
        | Error::ProviderRoleMismatch
        | Error::UnsupportedProviderKeyAlgorithm
        | Error::HandleMismatch
        | Error::AuthorityMismatch
        | Error::PublicKeyMismatch
        | Error::ProviderAuthorityKeyMismatch => {
            IrohaRuntimeProviderRegistryErrorV1::BindingMismatch
        }
    }
}

macro_rules! qualify_native_signer_dependency {
    ($bindings:expr, $dependencies:expr, $slot:expr, $field:ident, $qualifier:path) => {
        if let Some(binding) = native_signer_binding_for_slot($bindings, $slot)? {
            let provider = $dependencies
                .$field
                .take()
                .ok_or(IrohaRuntimeProviderRegistryErrorV1::IncompleteResolution)?;
            $dependencies.$field = Some(
                $qualifier(binding.clone(), provider)
                    .map_err(map_native_signer_qualification_error)?,
            );
        }
    };
}

fn qualify_native_transaction_signers(
    bindings: &IrohaRuntimeProviderBindingsV1,
    dependencies: &mut IrohaRuntimeDeps,
) -> Result<(), IrohaRuntimeProviderRegistryErrorV1> {
    qualify_native_signer_dependency!(
        bindings,
        dependencies,
        IrohaRuntimeProviderSlotV1::ProofOutcomeTransactionSigner,
        sorafs_proof_outcome_signer,
        iroha_torii::qualify_sorafs_proof_outcome_transaction_signer_v1
    );
    qualify_native_signer_dependency!(
        bindings,
        dependencies,
        IrohaRuntimeProviderSlotV1::RepairTransactionSigner,
        sorafs_repair_transaction_signer,
        iroha_torii::qualify_sorafs_repair_transaction_signer_v1
    );
    qualify_native_signer_dependency!(
        bindings,
        dependencies,
        IrohaRuntimeProviderSlotV1::ReserveTransactionSigner,
        sorafs_reserve_transaction_signer,
        iroha_torii::qualify_sorafs_reserve_transaction_signer_v1
    );
    qualify_native_signer_dependency!(
        bindings,
        dependencies,
        IrohaRuntimeProviderSlotV1::OrderbookTransactionSigner,
        sorafs_orderbook_transaction_signer,
        iroha_torii::qualify_sorafs_orderbook_transaction_signer_v1
    );
    Ok(())
}

fn qualify_provider_ingest_dependencies(
    bindings: &IrohaRuntimeProviderBindingsV1,
    dependencies: &IrohaRuntimeDeps,
) -> Result<(), IrohaRuntimeProviderRegistryErrorV1> {
    use IrohaRuntimeProviderSlotV1 as Slot;

    let binding = |slot| bindings.iter().find(|binding| binding.slot() == slot);
    if let Some(expected) = binding(Slot::ProviderIngestAuthenticatedSource) {
        let source = dependencies
            .sorafs_provider_ingest_authenticated_source
            .as_ref()
            .ok_or(IrohaRuntimeProviderRegistryErrorV1::IncompleteResolution)?;
        let limits = expected
            .provider_ingest_source_limits()
            .ok_or(IrohaRuntimeProviderRegistryErrorV1::BindingMismatch)?;
        let observe = || {
            let qualification = source.qualification().map_err(|error| match error {
                sorafs_node::ProviderIngestSourceFetchErrorV1::Unavailable => {
                    IrohaRuntimeProviderRegistryErrorV1::Unavailable
                }
                sorafs_node::ProviderIngestSourceFetchErrorV1::ContentRejected
                | sorafs_node::ProviderIngestSourceFetchErrorV1::Rejected => {
                    IrohaRuntimeProviderRegistryErrorV1::StaleOrRevoked
                }
            })?;
            if source.runtime_handle() != expected.handle()
                || expected.revision() != Some(qualification.revision)
                || expected.policy_digest() != Some(qualification.policy_digest)
            {
                return Err(IrohaRuntimeProviderRegistryErrorV1::BindingMismatch);
            }
            let source_provider_ids = source.source_provider_ids();
            if source_provider_ids.len() < 2
                || source_provider_ids.len()
                    > usize::try_from(limits.max_source_providers)
                        .map_err(|_| IrohaRuntimeProviderRegistryErrorV1::BindingMismatch)?
                || source_provider_ids
                    .iter()
                    .any(|provider_id| *provider_id == [0; 32])
                || source_provider_ids
                    .windows(2)
                    .any(|pair| pair[0] >= pair[1])
            {
                return Err(IrohaRuntimeProviderRegistryErrorV1::BindingMismatch);
            }
            Ok(source_provider_ids.to_vec())
        };
        let before = observe()?;
        source.check_readiness().map_err(|error| match error {
            sorafs_node::ProviderIngestSourceFetchErrorV1::Unavailable => {
                IrohaRuntimeProviderRegistryErrorV1::Unavailable
            }
            sorafs_node::ProviderIngestSourceFetchErrorV1::ContentRejected
            | sorafs_node::ProviderIngestSourceFetchErrorV1::Rejected => {
                IrohaRuntimeProviderRegistryErrorV1::StaleOrRevoked
            }
        })?;
        if observe()? != before {
            return Err(IrohaRuntimeProviderRegistryErrorV1::StaleOrRevoked);
        }
    }

    match (
        binding(Slot::ProviderIngestCompletionSignerResolver),
        binding(Slot::ProviderIngestCompletionSigner),
    ) {
        (None, None) => {}
        (Some(resolver_binding), Some(signer_binding)) => {
            let resolver = dependencies
                .sorafs_provider_ingest_signer_resolver
                .as_ref()
                .ok_or(IrohaRuntimeProviderRegistryErrorV1::IncompleteResolution)?;
            let expected_signer = resolver_binding
                .provider_ingest_signer_binding()
                .ok_or(IrohaRuntimeProviderRegistryErrorV1::BindingMismatch)?;
            if signer_binding.provider_ingest_signer_binding() != Some(expected_signer) {
                return Err(IrohaRuntimeProviderRegistryErrorV1::BindingMismatch);
            }
            let observe = || {
                let qualification = resolver.qualification().map_err(|error| match error {
                    sorafs_node::ProviderIngestCompletionSignerResolverErrorV1::Unavailable => {
                        IrohaRuntimeProviderRegistryErrorV1::Unavailable
                    }
                    sorafs_node::ProviderIngestCompletionSignerResolverErrorV1::Rejected => {
                        IrohaRuntimeProviderRegistryErrorV1::StaleOrRevoked
                    }
                })?;
                let actual_signer = resolver.signer_binding().map_err(|error| match error {
                    sorafs_node::ProviderIngestCompletionSignerResolverErrorV1::Unavailable => {
                        IrohaRuntimeProviderRegistryErrorV1::Unavailable
                    }
                    sorafs_node::ProviderIngestCompletionSignerResolverErrorV1::Rejected => {
                        IrohaRuntimeProviderRegistryErrorV1::StaleOrRevoked
                    }
                })?;
                if resolver.runtime_handle() != resolver_binding.handle()
                    || resolver_binding.revision() != Some(qualification.revision)
                    || resolver_binding.policy_digest() != Some(qualification.policy_digest)
                    || &actual_signer != expected_signer
                {
                    return Err(IrohaRuntimeProviderRegistryErrorV1::BindingMismatch);
                }
                Ok(())
            };
            observe()?;
            resolver.check_readiness().map_err(|error| match error {
                sorafs_node::ProviderIngestCompletionSignerResolverErrorV1::Unavailable => {
                    IrohaRuntimeProviderRegistryErrorV1::Unavailable
                }
                sorafs_node::ProviderIngestCompletionSignerResolverErrorV1::Rejected => {
                    IrohaRuntimeProviderRegistryErrorV1::StaleOrRevoked
                }
            })?;
            observe()?;
        }
        (Some(_), None) | (None, Some(_)) => {
            return Err(IrohaRuntimeProviderRegistryErrorV1::IncompleteResolution);
        }
    }

    if let Some(expected) = binding(Slot::ProviderIngestCheckpointStore) {
        let store = dependencies
            .sorafs_provider_ingest_checkpoint_runtime
            .as_ref()
            .ok_or(IrohaRuntimeProviderRegistryErrorV1::IncompleteResolution)?;
        let observe = || {
            let qualification = store.qualification().map_err(|error| match error {
                sorafs_node::ProviderIngestCheckpointExternalErrorV1::Unavailable => {
                    IrohaRuntimeProviderRegistryErrorV1::Unavailable
                }
                sorafs_node::ProviderIngestCheckpointExternalErrorV1::Rejected
                | sorafs_node::ProviderIngestCheckpointExternalErrorV1::Ambiguous => {
                    IrohaRuntimeProviderRegistryErrorV1::StaleOrRevoked
                }
            })?;
            if store.handle() != expected.handle()
                || expected.revision() != Some(qualification.revision)
                || expected.policy_digest() != Some(qualification.policy_digest)
            {
                return Err(IrohaRuntimeProviderRegistryErrorV1::BindingMismatch);
            }
            Ok(())
        };
        observe()?;
        if let Some(record) = store.load_latest().map_err(|error| match error {
            sorafs_node::ProviderIngestCheckpointExternalErrorV1::Unavailable => {
                IrohaRuntimeProviderRegistryErrorV1::Unavailable
            }
            sorafs_node::ProviderIngestCheckpointExternalErrorV1::Rejected
            | sorafs_node::ProviderIngestCheckpointExternalErrorV1::Ambiguous => {
                IrohaRuntimeProviderRegistryErrorV1::StaleOrRevoked
            }
        })? {
            record
                .validate(
                    expected
                        .provider_ingest_checkpoint_max_bytes()
                        .ok_or(IrohaRuntimeProviderRegistryErrorV1::BindingMismatch)?,
                )
                .map_err(|_| IrohaRuntimeProviderRegistryErrorV1::BindingMismatch)?;
        }
        observe()?;
    }

    if let Some(expected) = binding(Slot::ProviderIngestRetentionAuthority) {
        let authority = dependencies
            .sorafs_provider_ingest_retention_authority
            .as_ref()
            .ok_or(IrohaRuntimeProviderRegistryErrorV1::IncompleteResolution)?;
        let expected_revision = expected
            .revision()
            .ok_or(IrohaRuntimeProviderRegistryErrorV1::BindingMismatch)?;
        let expected_digest = expected
            .policy_digest()
            .ok_or(IrohaRuntimeProviderRegistryErrorV1::BindingMismatch)?;
        let observe = || {
            let qualification = authority.qualification().map_err(|error| match error {
                iroha_core::query::provider_ingest_finalized::
                    ProviderIngestFinalizedArchiveRetentionAuthorityExternalErrorV1::Unavailable => {
                        IrohaRuntimeProviderRegistryErrorV1::Unavailable
                    }
                iroha_core::query::provider_ingest_finalized::
                    ProviderIngestFinalizedArchiveRetentionAuthorityExternalErrorV1::Rejected
                | iroha_core::query::provider_ingest_finalized::
                    ProviderIngestFinalizedArchiveRetentionAuthorityExternalErrorV1::Ambiguous => {
                        IrohaRuntimeProviderRegistryErrorV1::StaleOrRevoked
                    }
            })?;
            if authority.handle() != expected.handle()
                || qualification.revision() != expected_revision
                || qualification.policy_digest() != expected_digest
            {
                return Err(IrohaRuntimeProviderRegistryErrorV1::BindingMismatch);
            }
            Ok(qualification)
        };
        let expected_qualification = observe()?;
        let chain_id = bindings
            .chain_id()
            .parse::<iroha_data_model::ChainId>()
            .map_err(|_| IrohaRuntimeProviderRegistryErrorV1::BindingMismatch)?;
        if let Some(record) = authority.load_latest(&chain_id).map_err(|error| {
            match error {
            iroha_core::query::provider_ingest_finalized::
                ProviderIngestFinalizedArchiveRetentionAuthorityExternalErrorV1::Unavailable => {
                    IrohaRuntimeProviderRegistryErrorV1::Unavailable
                }
            iroha_core::query::provider_ingest_finalized::
                ProviderIngestFinalizedArchiveRetentionAuthorityExternalErrorV1::Rejected
            | iroha_core::query::provider_ingest_finalized::
                ProviderIngestFinalizedArchiveRetentionAuthorityExternalErrorV1::Ambiguous => {
                    IrohaRuntimeProviderRegistryErrorV1::StaleOrRevoked
                }
        }
        })? {
            if record.authority_qualification() != expected_qualification
                || record.to_canonical_bytes().is_err()
            {
                return Err(IrohaRuntimeProviderRegistryErrorV1::BindingMismatch);
            }
        }
        observe()?;
    }
    Ok(())
}

fn qualify_reputation_retention_dependency(
    bindings: &IrohaRuntimeProviderBindingsV1,
    dependencies: &IrohaRuntimeDeps,
) -> Result<(), IrohaRuntimeProviderRegistryErrorV1> {
    let Some(expected) = bindings.iter().find(|binding| {
        binding.slot() == IrohaRuntimeProviderSlotV1::ReputationFinalizedArchiveRetentionAuthority
    }) else {
        return Ok(());
    };
    let authority = dependencies
        .sorafs_reputation_retention_authority
        .as_ref()
        .ok_or(IrohaRuntimeProviderRegistryErrorV1::IncompleteResolution)?;
    let expected_revision = expected
        .revision()
        .ok_or(IrohaRuntimeProviderRegistryErrorV1::BindingMismatch)?;
    let expected_digest = expected
        .policy_digest()
        .ok_or(IrohaRuntimeProviderRegistryErrorV1::BindingMismatch)?;
    let observe = || {
        let qualification = authority.qualification().map_err(|error| {
            match error {
            iroha_core::query::reputation_finalized::
                ReputationFinalizedArchiveRetentionAuthorityExternalErrorV1::Unavailable => {
                    IrohaRuntimeProviderRegistryErrorV1::Unavailable
                }
            iroha_core::query::reputation_finalized::
                ReputationFinalizedArchiveRetentionAuthorityExternalErrorV1::Rejected
            | iroha_core::query::reputation_finalized::
                ReputationFinalizedArchiveRetentionAuthorityExternalErrorV1::Ambiguous => {
                    IrohaRuntimeProviderRegistryErrorV1::StaleOrRevoked
                }
        }
        })?;
        if authority.handle() != expected.handle()
            || qualification.revision() != expected_revision
            || qualification.policy_digest() != expected_digest
        {
            return Err(IrohaRuntimeProviderRegistryErrorV1::BindingMismatch);
        }
        Ok(qualification)
    };
    let expected_qualification = observe()?;
    let chain_id = bindings
        .chain_id()
        .parse::<iroha_data_model::ChainId>()
        .map_err(|_| IrohaRuntimeProviderRegistryErrorV1::BindingMismatch)?;
    if let Some(record) = authority.load_latest(&chain_id).map_err(|error| {
        match error {
        iroha_core::query::reputation_finalized::
            ReputationFinalizedArchiveRetentionAuthorityExternalErrorV1::Unavailable => {
                IrohaRuntimeProviderRegistryErrorV1::Unavailable
            }
        iroha_core::query::reputation_finalized::
            ReputationFinalizedArchiveRetentionAuthorityExternalErrorV1::Rejected
        | iroha_core::query::reputation_finalized::
            ReputationFinalizedArchiveRetentionAuthorityExternalErrorV1::Ambiguous => {
                IrohaRuntimeProviderRegistryErrorV1::StaleOrRevoked
            }
    }
    })? {
        if record.authority_qualification() != expected_qualification
            || record.to_canonical_bytes().is_err()
        {
            return Err(IrohaRuntimeProviderRegistryErrorV1::BindingMismatch);
        }
    }
    observe()?;
    Ok(())
}

fn qualify_reputation_journal_checkpoint_dependency(
    bindings: &IrohaRuntimeProviderBindingsV1,
    dependencies: &IrohaRuntimeDeps,
) -> Result<(), IrohaRuntimeProviderRegistryErrorV1> {
    let Some(expected) = bindings
        .iter()
        .find(|binding| binding.slot() == IrohaRuntimeProviderSlotV1::ReputationJournalCheckpoint)
    else {
        return Ok(());
    };
    let provider = dependencies
        .sorafs_reputation_journal_checkpoint_provider
        .as_ref()
        .ok_or(IrohaRuntimeProviderRegistryErrorV1::IncompleteResolution)?;
    if !is_production_runtime_handle(provider.handle()) {
        return Err(IrohaRuntimeProviderRegistryErrorV1::TestProviderRejected);
    }
    if provider.handle() != expected.handle() {
        return Err(IrohaRuntimeProviderRegistryErrorV1::BindingMismatch);
    }
    let expected_revision =
        expected
            .revision()
            .ok_or(IrohaRuntimeProviderRegistryErrorV1::InvalidBinding(
                expected.slot(),
            ))?;
    let expected_policy_digest =
        expected
            .policy_digest()
            .ok_or(IrohaRuntimeProviderRegistryErrorV1::InvalidBinding(
                expected.slot(),
            ))?;
    sorafs_node::reputation::runtime::ReputationJournalCheckpointSealingPolicyV1::try_new(
        expected.handle().to_owned(),
        expected_revision,
        expected_policy_digest,
    )
    .map_err(|_| IrohaRuntimeProviderRegistryErrorV1::InvalidBinding(expected.slot()))?;
    let expected_qualification =
        sorafs_node::reputation::runtime::ReputationRuntimeProviderQualificationV1::new(
            expected_revision,
            expected_policy_digest,
        );
    let first = provider
        .qualification()
        .map_err(|_| IrohaRuntimeProviderRegistryErrorV1::Unavailable)?;
    if first != expected_qualification || provider.handle() != expected.handle() {
        return Err(IrohaRuntimeProviderRegistryErrorV1::BindingMismatch);
    }
    let record = provider.load_latest().map_err(|error| match error {
        sorafs_node::reputation::runtime::ReputationJournalCheckpointExternalErrorV1::Unavailable => {
            IrohaRuntimeProviderRegistryErrorV1::Unavailable
        }
        sorafs_node::reputation::runtime::ReputationJournalCheckpointExternalErrorV1::Rejected
        | sorafs_node::reputation::runtime::ReputationJournalCheckpointExternalErrorV1::Ambiguous => {
            IrohaRuntimeProviderRegistryErrorV1::StaleOrRevoked
        }
    })?;
    if record.is_some_and(|record| {
        record
            .to_canonical_bytes(
                sorafs_node::reputation::runtime::
                    REPUTATION_JOURNAL_PRODUCER_MAX_CHECKPOINT_BYTES_V1,
            )
            .is_err()
    }) {
        return Err(IrohaRuntimeProviderRegistryErrorV1::BindingMismatch);
    }
    let second = provider
        .qualification()
        .map_err(|_| IrohaRuntimeProviderRegistryErrorV1::Unavailable)?;
    if second != first || provider.handle() != expected.handle() {
        return Err(IrohaRuntimeProviderRegistryErrorV1::StaleOrRevoked);
    }
    Ok(())
}

fn qualify_por_replay_archive_dependency(
    bindings: &IrohaRuntimeProviderBindingsV1,
    dependencies: &IrohaRuntimeDeps,
) -> Result<(), IrohaRuntimeProviderRegistryErrorV1> {
    let Some(expected) = bindings
        .iter()
        .find(|binding| binding.slot() == IrohaRuntimeProviderSlotV1::PorFinalizedReplayArchive)
    else {
        return Ok(());
    };
    let archive = dependencies
        .sorafs_por_finalized_replay_archive
        .as_ref()
        .ok_or(IrohaRuntimeProviderRegistryErrorV1::IncompleteResolution)?;
    if !is_production_runtime_handle(archive.runtime_handle()) {
        return Err(IrohaRuntimeProviderRegistryErrorV1::TestProviderRejected);
    }
    if archive.runtime_handle() != expected.handle() {
        return Err(IrohaRuntimeProviderRegistryErrorV1::BindingMismatch);
    }
    let expected_binding = expected.por_replay_archive_binding().ok_or(
        IrohaRuntimeProviderRegistryErrorV1::InvalidBinding(expected.slot()),
    )?;
    let observe = || {
        archive.binding().map_err(|error| match error {
            sorafs_node::PorFinalizedReplayArchiveExternalErrorV1::Unavailable => {
                IrohaRuntimeProviderRegistryErrorV1::Unavailable
            }
            sorafs_node::PorFinalizedReplayArchiveExternalErrorV1::Rejected => {
                IrohaRuntimeProviderRegistryErrorV1::StaleOrRevoked
            }
        })
    };
    let first = observe()?;
    if first != expected_binding {
        return Err(IrohaRuntimeProviderRegistryErrorV1::BindingMismatch);
    }
    archive.check_readiness().map_err(|error| match error {
        sorafs_node::PorFinalizedReplayArchiveExternalErrorV1::Unavailable => {
            IrohaRuntimeProviderRegistryErrorV1::Unavailable
        }
        sorafs_node::PorFinalizedReplayArchiveExternalErrorV1::Rejected => {
            IrohaRuntimeProviderRegistryErrorV1::StaleOrRevoked
        }
    })?;
    if observe()? != first {
        return Err(IrohaRuntimeProviderRegistryErrorV1::StaleOrRevoked);
    }
    Ok(())
}

fn qualify_evidence_viewer_archive_dependency(
    bindings: &IrohaRuntimeProviderBindingsV1,
    dependencies: &IrohaRuntimeDeps,
) -> Result<(), IrohaRuntimeProviderRegistryErrorV1> {
    let Some(expected) = bindings.iter().find(|binding| {
        binding.slot() == IrohaRuntimeProviderSlotV1::EvidenceViewerCompactionArchive
    }) else {
        return Ok(());
    };
    let archive = dependencies
        .sorafs_evidence_viewer_compaction_archive
        .as_ref()
        .ok_or(IrohaRuntimeProviderRegistryErrorV1::IncompleteResolution)?;
    if !is_production_runtime_handle(archive.handle()) {
        return Err(IrohaRuntimeProviderRegistryErrorV1::TestProviderRejected);
    }
    let expected_revision =
        expected
            .revision()
            .ok_or(IrohaRuntimeProviderRegistryErrorV1::InvalidBinding(
                expected.slot(),
            ))?;
    let expected_policy_digest =
        expected
            .policy_digest()
            .ok_or(IrohaRuntimeProviderRegistryErrorV1::InvalidBinding(
                expected.slot(),
            ))?;
    let expected_archive_id = expected.evidence_viewer_archive_id().ok_or(
        IrohaRuntimeProviderRegistryErrorV1::InvalidBinding(expected.slot()),
    )?;
    let expected_public_key = expected.evidence_viewer_archive_public_key().ok_or(
        IrohaRuntimeProviderRegistryErrorV1::InvalidBinding(expected.slot()),
    )?;
    let observe = || {
        let qualification = archive.qualification().map_err(|error| match error {
            sorafs_node::evidence_viewer::EvidenceViewerRuntimeProviderReadinessErrorV1::Unavailable => {
                IrohaRuntimeProviderRegistryErrorV1::Unavailable
            }
            sorafs_node::evidence_viewer::EvidenceViewerRuntimeProviderReadinessErrorV1::Rejected => {
                IrohaRuntimeProviderRegistryErrorV1::StaleOrRevoked
            }
        })?;
        if archive.handle() != expected.handle()
            || qualification.revision() != expected_revision
            || qualification.policy_digest() != expected_policy_digest
        {
            return Err(IrohaRuntimeProviderRegistryErrorV1::BindingMismatch);
        }
        Ok(qualification)
    };
    let observe_identity = || {
        let qualification = observe()?;
        let identity = (archive.archive_id(), archive.signing_public_key());
        if observe()? != qualification {
            return Err(IrohaRuntimeProviderRegistryErrorV1::StaleOrRevoked);
        }
        Ok((qualification, identity))
    };
    let (first_qualification, first_identity) = observe_identity()?;
    if first_identity != (expected_archive_id, expected_public_key) {
        return Err(IrohaRuntimeProviderRegistryErrorV1::BindingMismatch);
    }
    if observe_identity()? != (first_qualification, first_identity) {
        return Err(IrohaRuntimeProviderRegistryErrorV1::StaleOrRevoked);
    }
    Ok(())
}

fn qualify_evidence_viewer_transparency_publisher_dependency(
    bindings: &IrohaRuntimeProviderBindingsV1,
    dependencies: &IrohaRuntimeDeps,
) -> Result<(), IrohaRuntimeProviderRegistryErrorV1> {
    let Some(expected) = bindings.iter().find(|binding| {
        binding.slot() == IrohaRuntimeProviderSlotV1::EvidenceViewerTransparencyPublisher
    }) else {
        return Ok(());
    };
    let publisher = dependencies
        .sorafs_evidence_viewer_transparency_publisher
        .as_ref()
        .ok_or(IrohaRuntimeProviderRegistryErrorV1::IncompleteResolution)?;
    if !is_production_runtime_handle(publisher.handle()) {
        return Err(IrohaRuntimeProviderRegistryErrorV1::TestProviderRejected);
    }
    let expected_revision =
        expected
            .revision()
            .ok_or(IrohaRuntimeProviderRegistryErrorV1::InvalidBinding(
                expected.slot(),
            ))?;
    let expected_policy_digest =
        expected
            .policy_digest()
            .ok_or(IrohaRuntimeProviderRegistryErrorV1::InvalidBinding(
                expected.slot(),
            ))?;
    let expected_public_key = expected
        .evidence_viewer_transparency_publisher_public_key()
        .ok_or(IrohaRuntimeProviderRegistryErrorV1::InvalidBinding(
            expected.slot(),
        ))?;
    let observe = || {
        let qualification = publisher.qualification().map_err(|error| match error {
            sorafs_node::evidence_viewer::EvidenceViewerRuntimeProviderReadinessErrorV1::Unavailable => {
                IrohaRuntimeProviderRegistryErrorV1::Unavailable
            }
            sorafs_node::evidence_viewer::EvidenceViewerRuntimeProviderReadinessErrorV1::Rejected => {
                IrohaRuntimeProviderRegistryErrorV1::StaleOrRevoked
            }
        })?;
        if publisher.handle() != expected.handle()
            || qualification.revision() != expected_revision
            || qualification.policy_digest() != expected_policy_digest
        {
            return Err(IrohaRuntimeProviderRegistryErrorV1::BindingMismatch);
        }
        Ok((qualification, publisher.public_key()))
    };
    let first = observe()?;
    if first.1 != expected_public_key {
        return Err(IrohaRuntimeProviderRegistryErrorV1::BindingMismatch);
    }
    if observe()? != first {
        return Err(IrohaRuntimeProviderRegistryErrorV1::StaleOrRevoked);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::{
        path::Path,
        sync::{
            Arc, Mutex,
            atomic::{AtomicU64, AtomicUsize, Ordering},
        },
        time::Duration,
    };

    use super::*;
    use iroha_config_base::{toml::TomlSource, util::Bytes};
    use iroha_crypto::{Algorithm, KeyPair};

    struct EmptyRegistry;

    impl IrohaRuntimeProviderRegistryV1 for EmptyRegistry {
        fn resolve(
            &self,
            _bindings: &IrohaRuntimeProviderBindingsV1,
        ) -> Result<IrohaRuntimeDeps, IrohaRuntimeProviderRegistryErrorV1> {
            Ok(IrohaRuntimeDeps::default())
        }
    }

    struct FixedRegistry(IrohaRuntimeDeps);

    impl IrohaRuntimeProviderRegistryV1 for FixedRegistry {
        fn resolve(
            &self,
            _bindings: &IrohaRuntimeProviderBindingsV1,
        ) -> Result<IrohaRuntimeDeps, IrohaRuntimeProviderRegistryErrorV1> {
            Ok(self.0.clone())
        }
    }

    const GOVERNANCE_IPFS_HANDLE: &str = "vault://governance/ipfs-primary";
    const GOVERNANCE_HEAD_HANDLE: &str = "vault://governance/head-primary";
    const GOVERNANCE_CHECKPOINT_HANDLE: &str = "kms://governance/checkpoint-primary";
    const GOVERNANCE_SIGNER_HANDLE: &str = "hsm://governance/producer-signer-primary";
    const GOVERNANCE_PUBLISHER_PEER_ID: &str = "12D3KooWGovernanceProducerPrimary";
    const GOVERNANCE_QUALIFICATION: sorafs_node::GovernanceDagRuntimeProviderQualificationV1 =
        sorafs_node::GovernanceDagRuntimeProviderQualificationV1::new(7, [0x71; 32]);
    const EVIDENCE_CHECKPOINT_HANDLE: &str = "sealed://sorafs/evidence-viewer/checkpoint-primary";
    const EVIDENCE_CHECKPOINT_QUALIFICATION:
        sorafs_node::evidence_viewer::EvidenceViewerRuntimeProviderQualificationV1 =
        sorafs_node::evidence_viewer::EvidenceViewerRuntimeProviderQualificationV1::new(
            15, [0xA5; 32],
        );
    const EVIDENCE_ARCHIVE_HANDLE: &str = "object-lock://sorafs/evidence-viewer/archive-primary";
    const EVIDENCE_ARCHIVE_ID: [u8; 32] = [0xA7; 32];
    const EVIDENCE_ARCHIVE_QUALIFICATION:
        sorafs_node::evidence_viewer::EvidenceViewerRuntimeProviderQualificationV1 =
        sorafs_node::evidence_viewer::EvidenceViewerRuntimeProviderQualificationV1::new(
            16, [0xA8; 32],
        );
    const EVIDENCE_TRANSPARENCY_PUBLISHER_HANDLE: &str =
        "transparency://sorafs/evidence-viewer/publisher-primary";
    const EVIDENCE_TRANSPARENCY_PUBLISHER_QUALIFICATION:
        sorafs_node::evidence_viewer::EvidenceViewerRuntimeProviderQualificationV1 =
        sorafs_node::evidence_viewer::EvidenceViewerRuntimeProviderQualificationV1::new(
            17, [0xA9; 32],
        );
    const TRANSPARENCY_LEADER_LEASE_HANDLE: &str = "sealed-cas:transparency:leader-primary";
    const TRANSPARENCY_LEADER_LEASE_QUALIFICATION:
        sorafs_node::TransparencyRuntimeProviderQualificationV1 =
        sorafs_node::TransparencyRuntimeProviderQualificationV1::new(11, [0xF1; 32]);
    const FENCED_PRIVACY_HANDLE: &str = "governance-cas:transparency:privacy-primary";
    const FENCED_PRIVACY_QUALIFICATION: sorafs_node::GovernanceDagRuntimeProviderQualificationV1 =
        sorafs_node::GovernanceDagRuntimeProviderQualificationV1::new(13, [0x91; 32]);
    const REPUTATION_CHECKPOINT_HANDLE: &str =
        "sealed://sorafs/reputation/journal-checkpoint-primary";
    const REPUTATION_CHECKPOINT_QUALIFICATION:
        sorafs_node::reputation::runtime::ReputationRuntimeProviderQualificationV1 =
        sorafs_node::reputation::runtime::ReputationRuntimeProviderQualificationV1::new(
            sorafs_node::reputation::runtime::REPUTATION_RUNTIME_PROVIDER_QUALIFICATION_REVISION_V1,
            [0xA4; 32],
        );

    #[derive(Debug)]
    struct ReputationJournalCheckpointProvider {
        handle: &'static str,
        first: sorafs_node::reputation::runtime::ReputationRuntimeProviderQualificationV1,
        later: Option<sorafs_node::reputation::runtime::ReputationRuntimeProviderQualificationV1>,
        qualification_calls: AtomicUsize,
        load_error:
            Option<sorafs_node::reputation::runtime::ReputationJournalCheckpointExternalErrorV1>,
    }

    impl ReputationJournalCheckpointProvider {
        fn exact() -> Self {
            Self {
                handle: REPUTATION_CHECKPOINT_HANDLE,
                first: REPUTATION_CHECKPOINT_QUALIFICATION,
                later: None,
                qualification_calls: AtomicUsize::new(0),
                load_error: None,
            }
        }
    }

    impl sorafs_node::reputation::runtime::ReputationRuntimeProviderV1
        for ReputationJournalCheckpointProvider
    {
        fn handle(&self) -> &str {
            self.handle
        }

        fn qualification(
            &self,
        ) -> Result<
            sorafs_node::reputation::runtime::ReputationRuntimeProviderQualificationV1,
            sorafs_node::reputation::runtime::ReputationExternalFailureV1,
        > {
            let call = self.qualification_calls.fetch_add(1, Ordering::Relaxed);
            Ok(if call == 0 {
                self.first
            } else {
                self.later.unwrap_or(self.first)
            })
        }
    }

    impl sorafs_node::reputation::runtime::ReputationJournalCheckpointRuntimeV1
        for ReputationJournalCheckpointProvider
    {
        fn load_latest(
            &self,
        ) -> Result<
            Option<sorafs_node::reputation::runtime::ReputationJournalSealedCheckpointRecordV1>,
            sorafs_node::reputation::runtime::ReputationJournalCheckpointExternalErrorV1,
        > {
            self.load_error.map_or(Ok(None), Err)
        }

        fn compare_and_swap_latest(
            &self,
            _expected_revision: Option<[u8; 32]>,
            _next: &sorafs_node::reputation::runtime::ReputationJournalSealedCheckpointRecordV1,
        ) -> Result<(), sorafs_node::reputation::runtime::ReputationJournalCheckpointExternalErrorV1>
        {
            Err(
                sorafs_node::reputation::runtime::
                    ReputationJournalCheckpointExternalErrorV1::Rejected,
            )
        }
    }

    fn evidence_archive_public_key() -> [u8; 32] {
        let key_pair = KeyPair::try_from_seed(vec![0x63; 32], Algorithm::Ed25519)
            .expect("evidence-viewer archive signer key");
        let public_key = key_pair.public_key().to_bytes().1;
        let mut bytes = [0_u8; 32];
        bytes.copy_from_slice(&public_key);
        bytes
    }

    fn governance_signer_keypair(seed: u8) -> KeyPair {
        KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519)
            .expect("Governance DAG signer key")
    }

    fn governance_signer_public_key(seed: u8) -> [u8; 32] {
        governance_signer_keypair(seed)
            .public_key()
            .to_bytes()
            .1
            .try_into()
            .expect("Ed25519 public key has 32 bytes")
    }

    fn por_archive_binding(seed: u8) -> sorafs_node::PorFinalizedReplayArchiveBindingV1 {
        let key_pair = KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519)
            .expect("test Ed25519 keypair");
        let public_key = key_pair.public_key().to_bytes().1;
        let mut signing_public_key = [0_u8; 32];
        signing_public_key.copy_from_slice(&public_key);
        sorafs_node::PorFinalizedReplayArchiveBindingV1::try_new(
            [0xB1; 32],
            17,
            [0xB2; 32],
            signing_public_key,
        )
        .expect("valid test replay-archive binding")
    }

    fn por_archive_config(
        binding: sorafs_node::PorFinalizedReplayArchiveBindingV1,
    ) -> iroha_config::parameters::actual::SorafsPorReplayArchive {
        iroha_config::parameters::actual::SorafsPorReplayArchive {
            handle: "hsm://sorafs/por-replay-archive/primary".to_owned(),
            archive_id: binding.archive_id,
            revision: binding.revision,
            policy_digest: binding.policy_digest,
            signing_public_key: binding.signing_public_key,
            poll_interval: Duration::from_secs(1),
            max_records_per_tick: 64,
            max_successor_receipts: 1_024,
            max_successor_proof_bytes: 1_048_576,
        }
    }

    fn por_archive_request(
        binding: sorafs_node::PorFinalizedReplayArchiveBindingV1,
    ) -> IrohaRuntimeProviderBindingsV1 {
        let archive = por_archive_config(binding);
        IrohaRuntimeProviderBindingsV1 {
            chain_id: "por-replay-test-chain".to_owned(),
            bindings: vec![
                IrohaRuntimeProviderBindingV1::try_new_por_replay_archive(&archive)
                    .expect("valid archive request"),
            ],
        }
    }

    #[derive(Debug)]
    struct PorReplayArchive {
        handle: &'static str,
        first_binding: sorafs_node::PorFinalizedReplayArchiveBindingV1,
        later_binding: Option<sorafs_node::PorFinalizedReplayArchiveBindingV1>,
        binding_calls: AtomicUsize,
        readiness_error: Option<sorafs_node::PorFinalizedReplayArchiveExternalErrorV1>,
    }

    impl PorReplayArchive {
        fn exact(binding: sorafs_node::PorFinalizedReplayArchiveBindingV1) -> Self {
            Self {
                handle: "hsm://sorafs/por-replay-archive/primary",
                first_binding: binding,
                later_binding: None,
                binding_calls: AtomicUsize::new(0),
                readiness_error: None,
            }
        }
    }

    impl sorafs_node::PorFinalizedReplayArchiveV1 for PorReplayArchive {
        fn runtime_handle(&self) -> &str {
            self.handle
        }

        fn binding(
            &self,
        ) -> Result<
            sorafs_node::PorFinalizedReplayArchiveBindingV1,
            sorafs_node::PorFinalizedReplayArchiveExternalErrorV1,
        > {
            let call = self.binding_calls.fetch_add(1, Ordering::Relaxed);
            Ok(if call == 0 {
                self.first_binding
            } else {
                self.later_binding.unwrap_or(self.first_binding)
            })
        }

        fn check_readiness(
            &self,
        ) -> Result<(), sorafs_node::PorFinalizedReplayArchiveExternalErrorV1> {
            self.readiness_error.map_or(Ok(()), Err)
        }

        fn current_head(
            &self,
        ) -> Result<
            Option<sorafs_node::PorFinalizedReplayArchiveReceiptV1>,
            sorafs_node::PorFinalizedReplayArchiveExternalErrorV1,
        > {
            Ok(None)
        }

        fn append(
            &self,
            _record: &sorafs_node::PorFinalizedReplayArchiveRecordV1,
            _expected_previous_head: Option<[u8; 32]>,
        ) -> Result<
            sorafs_node::PorFinalizedReplayArchiveReceiptV1,
            sorafs_node::PorFinalizedReplayArchiveExternalErrorV1,
        > {
            Err(sorafs_node::PorFinalizedReplayArchiveExternalErrorV1::Rejected)
        }

        fn lookup(
            &self,
            _challenge_id: [u8; 32],
            _expected_checkpoint_head: sorafs_node::PorFinalizedReplayArchiveReceiptV1,
            _proof_bounds: sorafs_node::PorFinalizedReplayArchiveProofBoundsV1,
        ) -> Result<
            sorafs_node::PorFinalizedReplayArchiveLookupV1,
            sorafs_node::PorFinalizedReplayArchiveExternalErrorV1,
        > {
            Err(sorafs_node::PorFinalizedReplayArchiveExternalErrorV1::Rejected)
        }
    }

    #[test]
    fn runtime_provider_slot_wire_ids_are_stable_and_ordered() {
        use IrohaRuntimeProviderSlotV1 as Slot;

        let slots = [
            Slot::ModerationQuarantineKeyWrapper,
            Slot::PrivacyCyclePrfProvider,
            Slot::PrivacyReleaseAnchor,
            Slot::TransparencyLeaderLease,
            Slot::FencedPrivacyPublisher,
            Slot::FencedPrivacyHeadReader,
            Slot::GovernanceDagSigner,
            Slot::GovernanceDagIpfsAuthenticator,
            Slot::GovernanceDagHeadAuthenticator,
            Slot::GovernanceDagCheckpointStore,
            Slot::StreamTokenSigner,
            Slot::AppealFinanceTransactionSigner,
            Slot::AppealFinanceCheckpoint,
            Slot::ProofOutcomeTransactionSigner,
            Slot::RepairTransactionSigner,
            Slot::ReserveTransactionSigner,
            Slot::OrderbookTransactionSigner,
            Slot::ModerationTransactionSigner,
            Slot::ModerationSettlementHandoff,
            Slot::ModerationPublicationHandoff,
            Slot::ModerationPanelNotification,
            Slot::EvidenceViewerWebAuthn,
            Slot::EvidenceViewerGrantAuthority,
            Slot::EvidenceViewerReceiptSigner,
            Slot::EvidenceViewerErasure,
            Slot::EvidenceViewerCheckpointStore,
            Slot::PopCredentialProviderRegistry,
            Slot::PotrGatewaySigner,
            Slot::PotrProviderSigner,
            Slot::GatewayAcmeClient,
            Slot::GatewayComplianceFeedTransport,
            Slot::ReputationJournalTransactionSubmitter,
            Slot::ReputationThresholdSigner,
            Slot::ReputationGovernanceDag,
            Slot::BillingFinalizedQuery,
            Slot::BillingJournalVerifier,
            Slot::BillingStatementSigner,
            Slot::BillingStatementPublisher,
            Slot::BillingAcknowledgementAuthority,
            Slot::BillingEpochWitnessStore,
            Slot::ProviderIngestAuthenticatedSource,
            Slot::ProviderIngestCompletionSignerResolver,
            Slot::ProviderIngestCompletionSigner,
            Slot::ProviderIngestCheckpointStore,
            Slot::ProviderIngestRetentionAuthority,
            Slot::PorFinalizedReplayArchive,
            Slot::EvidenceViewerCompactionArchive,
            Slot::ReputationFinalizedArchiveRetentionAuthority,
            Slot::SoracloudRuntimeMutationSigner,
            Slot::ReputationJournalCheckpoint,
            Slot::SoracloudHfInferenceCredentialProvider,
            Slot::ModerationCheckpointStore,
            Slot::EvidenceViewerTransparencyPublisher,
        ];
        for (index, slot) in slots.into_iter().enumerate() {
            assert_eq!(
                usize::from(slot.wire_id()),
                index + 1,
                "V1 broker role identifiers must stay contiguous and immutable"
            );
        }
    }

    #[test]
    fn soracloud_runtime_signer_projects_only_exact_public_metadata() {
        let key_pair =
            KeyPair::try_from_seed(vec![0xD1; 32], Algorithm::Ed25519).expect("test key");
        let configured = iroha_config::parameters::actual::SoracloudRuntimeMutationSignerBinding {
            handle: "hsm://soracloud/runtime-primary".to_owned(),
            authority: iroha_data_model::account::AccountId::new(key_pair.public_key().clone()),
            algorithm: Algorithm::Ed25519,
            public_key: key_pair.public_key().clone(),
            revision: 11,
            policy_digest: [0xD2; 32],
        };
        let projected =
            IrohaRuntimeProviderBindingV1::try_new_soracloud_runtime_signer(&configured)
                .expect("valid public signer binding");

        assert_eq!(
            projected.slot(),
            IrohaRuntimeProviderSlotV1::SoracloudRuntimeMutationSigner
        );
        assert_eq!(projected.handle(), configured.handle);
        assert_eq!(projected.revision(), Some(configured.revision));
        assert_eq!(projected.policy_digest(), Some(configured.policy_digest));
        let exact = projected
            .soracloud_runtime_signer_binding()
            .expect("exact signer metadata");
        assert_eq!(exact.authority(), &configured.authority);
        assert_eq!(exact.public_key(), &configured.public_key);
        assert!(exact.qualification().active());
        assert!(!exact.qualification().test_only());

        let mut test_marked = configured;
        test_marked.handle = "hsm://soracloud/test".to_owned();
        assert_eq!(
            IrohaRuntimeProviderBindingV1::try_new_soracloud_runtime_signer(&test_marked),
            Err(IrohaRuntimeProviderRegistryErrorV1::InvalidBinding(
                IrohaRuntimeProviderSlotV1::SoracloudRuntimeMutationSigner,
            ))
        );
    }

    #[derive(Debug)]
    struct HfCredentialProvider {
        handle: &'static str,
        qualification: crate::soracloud_hf_credential::SoracloudHfCredentialProviderQualificationV1,
    }

    impl crate::soracloud_hf_credential::SoracloudHfInferenceCredentialProviderV1
        for HfCredentialProvider
    {
        fn handle(&self) -> &str {
            self.handle
        }

        fn qualification(
            &self,
        ) -> Result<
            crate::soracloud_hf_credential::SoracloudHfCredentialProviderQualificationV1,
            crate::soracloud_hf_credential::SoracloudHfCredentialProviderProbeErrorV1,
        > {
            Ok(self.qualification)
        }

        fn check_readiness(
            &self,
        ) -> Result<(), crate::soracloud_hf_credential::SoracloudHfCredentialProviderProbeErrorV1>
        {
            Ok(())
        }

        fn execute_authenticated(
            &self,
            request: &crate::soracloud_hf_credential::SoracloudHfAuthenticatedInferenceRequestV1,
        ) -> Result<
            crate::soracloud_hf_credential::SoracloudHfAuthenticatedInferenceResponseV1,
            crate::soracloud_hf_credential::SoracloudHfCredentialProviderOperationErrorV1,
        > {
            crate::soracloud_hf_credential::SoracloudHfAuthenticatedInferenceResponseV1::try_new(
                200,
                Some("application/json".to_owned()),
                None,
                request.body().to_vec(),
                request.maximum_response_bytes(),
            )
        }
    }

    fn hf_credential_config()
    -> iroha_config::parameters::actual::SoracloudRuntimeHfCredentialProviderBinding {
        iroha_config::parameters::actual::SoracloudRuntimeHfCredentialProviderBinding {
            handle: "kms://soracloud/hf-inference-primary".to_owned(),
            revision: 7,
            policy_digest: [0xA7; 32],
        }
    }

    fn hf_credential_catalog(
        configured: &iroha_config::parameters::actual::SoracloudRuntimeHfCredentialProviderBinding,
    ) -> IrohaRuntimeProviderBindingsV1 {
        IrohaRuntimeProviderBindingsV1 {
            chain_id: "hf-credential-registry-test".to_owned(),
            bindings: vec![
                IrohaRuntimeProviderBindingV1::try_new_soracloud_hf_credential_provider(configured)
                    .expect("valid HF credential-provider binding"),
            ],
        }
    }

    #[test]
    fn hf_credential_provider_projects_only_public_identity() {
        let configured = hf_credential_config();
        let projected =
            IrohaRuntimeProviderBindingV1::try_new_soracloud_hf_credential_provider(&configured)
                .expect("valid HF credential-provider binding");

        assert_eq!(
            projected.slot(),
            IrohaRuntimeProviderSlotV1::SoracloudHfInferenceCredentialProvider
        );
        assert_eq!(projected.handle(), configured.handle);
        assert_eq!(projected.revision(), Some(configured.revision));
        assert_eq!(projected.policy_digest(), Some(configured.policy_digest));
    }

    #[test]
    fn registry_rejects_missing_substituted_stale_and_test_hf_credential_providers() {
        let configured = hf_credential_config();
        let catalog = hf_credential_catalog(&configured);
        assert!(matches!(
            resolve_runtime_deps_from_bindings(&catalog, Some(&EmptyRegistry)),
            Err(IrohaRuntimeProviderRegistryErrorV1::IncompleteResolution)
        ));

        let provider = |handle, revision, test_only| {
            Arc::new(HfCredentialProvider {
                handle,
                qualification: crate::soracloud_hf_credential::
                    SoracloudHfCredentialProviderQualificationV1::new(
                        revision,
                        configured.policy_digest,
                        true,
                        test_only,
                    ),
            })
        };
        let substituted = FixedRegistry(
            IrohaRuntimeDeps::default().with_soracloud_hf_inference_credential_provider(provider(
                "kms://soracloud/hf-inference-secondary",
                configured.revision,
                false,
            )),
        );
        assert!(matches!(
            resolve_runtime_deps_from_bindings(&catalog, Some(&substituted)),
            Err(IrohaRuntimeProviderRegistryErrorV1::BindingMismatch)
        ));

        let stale = FixedRegistry(
            IrohaRuntimeDeps::default().with_soracloud_hf_inference_credential_provider(provider(
                "kms://soracloud/hf-inference-primary",
                configured.revision + 1,
                false,
            )),
        );
        assert!(matches!(
            resolve_runtime_deps_from_bindings(&catalog, Some(&stale)),
            Err(IrohaRuntimeProviderRegistryErrorV1::StaleOrRevoked)
        ));

        let test_marked = FixedRegistry(
            IrohaRuntimeDeps::default().with_soracloud_hf_inference_credential_provider(provider(
                "kms://soracloud/hf-inference-primary",
                configured.revision,
                true,
            )),
        );
        assert!(matches!(
            resolve_runtime_deps_from_bindings(&catalog, Some(&test_marked)),
            Err(IrohaRuntimeProviderRegistryErrorV1::TestProviderRejected)
        ));
    }

    fn pop_registry_config() -> iroha_config::parameters::actual::SorafsPopCredentialService {
        let issuer = KeyPair::try_from_seed(vec![0x91; 32], Algorithm::Ed25519)
            .expect("derive PoP issuer test key");
        let issuer_public_key = issuer
            .public_key()
            .to_bytes()
            .1
            .try_into()
            .expect("Ed25519 public key width");
        iroha_config::parameters::actual::SorafsPopCredentialService {
            issuer_state_dir: std::path::PathBuf::from("/runtime/pop/issuer"),
            wallet_state_dir: std::path::PathBuf::from("/runtime/pop/wallet"),
            issuer_policy_digest: [0x92; 32],
            issuer_id: "pop-issuer-production-primary".to_owned(),
            issuer_hsm_key_id: "pkcs11:pop/issuer:primary".to_owned(),
            issuer_public_key,
            enrollment_recipient_key_id: "kms:pop/enrollment:primary".to_owned(),
            enrollment_recipient_public_key_digest: [0x94; 32],
            wallet_wrapping_key_id: "kms:pop/wallet:primary".to_owned(),
            runtime_provider_registry_handle: "runtime://sorafs/pop/provider-registry-primary"
                .to_owned(),
            runtime_provider_registry_revision: 7,
            runtime_provider_registry_policy_digest: [0x93; 32],
            approval_quorum: 2,
            approval_signers: Vec::new(),
            max_pending_enrollments: 16,
            max_outbox_entries: 16,
            max_dead_letters: 16,
            max_seen_nullifiers: 16,
            max_submission_attempts: 3,
            worker_interval: Duration::from_secs(1),
            max_finalized_time_skew: Duration::from_secs(30),
        }
    }

    #[test]
    fn pop_registry_binding_projects_only_exact_non_secret_public_metadata() {
        let config = pop_registry_config();
        let binding = IrohaRuntimeProviderBindingV1::try_new_pop_credential_registry(&config)
            .expect("canonical PoP registry binding");
        assert_eq!(
            binding.slot(),
            IrohaRuntimeProviderSlotV1::PopCredentialProviderRegistry
        );
        assert_eq!(
            binding.handle(),
            "runtime://sorafs/pop/provider-registry-primary"
        );
        assert_eq!(binding.revision(), Some(7));
        assert_eq!(binding.policy_digest(), Some([0x93; 32]));
        let exact = binding
            .pop_credential_runtime_binding()
            .expect("exact PoP public metadata");
        assert_eq!(exact.issuer_policy_digest, [0x92; 32]);
        assert_eq!(exact.issuer_id, "pop-issuer-production-primary");
        assert_eq!(exact.issuer_hsm_key_id, "pkcs11:pop/issuer:primary");
        assert_eq!(exact.issuer_public_key, config.issuer_public_key);
        assert_eq!(
            exact.enrollment_recipient_key_id,
            "kms:pop/enrollment:primary"
        );
        assert_eq!(exact.enrollment_recipient_public_key_digest, [0x94; 32]);
        assert_eq!(exact.wallet_wrapping_key_id, "kms:pop/wallet:primary");

        for invalid in [
            {
                let mut value = config.clone();
                value.issuer_policy_digest = [0; 32];
                value
            },
            {
                let mut value = config.clone();
                value.issuer_id.push('\n');
                value
            },
            {
                let mut value = config.clone();
                value.issuer_hsm_key_id = "pkcs11:pop/test".to_owned();
                value
            },
            {
                let mut value = config.clone();
                value.enrollment_recipient_key_id = "kms://pop/mock/enrollment".to_owned();
                value
            },
            {
                let mut value = config.clone();
                value.enrollment_recipient_public_key_digest = [0; 32];
                value
            },
            {
                let mut value = config.clone();
                value.wallet_wrapping_key_id = "kms://pop/fake/wallet".to_owned();
                value
            },
            {
                let mut value = config.clone();
                value.issuer_public_key = [0; 32];
                value
            },
        ] {
            assert_eq!(
                IrohaRuntimeProviderBindingV1::try_new_pop_credential_registry(&invalid),
                Err(IrohaRuntimeProviderRegistryErrorV1::InvalidBinding(
                    IrohaRuntimeProviderSlotV1::PopCredentialProviderRegistry,
                ))
            );
        }
    }

    fn resolve_por_archive(
        requested: &IrohaRuntimeProviderBindingsV1,
        archive: PorReplayArchive,
    ) -> Result<IrohaRuntimeDeps, IrohaRuntimeProviderRegistryErrorV1> {
        let archive: Arc<dyn sorafs_node::PorFinalizedReplayArchiveV1> = Arc::new(archive);
        let registry = FixedRegistry(
            IrohaRuntimeDeps::default().with_sorafs_por_finalized_replay_archive(archive),
        );
        resolve_runtime_deps_from_bindings(requested, Some(&registry))
    }

    #[test]
    fn por_replay_archive_slot_projects_and_qualifies_the_exact_public_binding() {
        let binding = por_archive_binding(0xB3);
        let requested = por_archive_request(binding);
        let projected = requested
            .iter()
            .find(|candidate| {
                candidate.slot() == IrohaRuntimeProviderSlotV1::PorFinalizedReplayArchive
            })
            .expect("archive request");

        assert_eq!(
            IrohaRuntimeProviderSlotV1::PorFinalizedReplayArchive.wire_id(),
            46
        );
        assert_eq!(
            projected.handle(),
            "hsm://sorafs/por-replay-archive/primary"
        );
        assert_eq!(projected.revision(), Some(binding.revision));
        assert_eq!(projected.policy_digest(), Some(binding.policy_digest));
        assert_eq!(projected.por_replay_archive_binding(), Some(binding));
        assert_eq!(
            projected.por_replay_archive_proof_limits(),
            Some(PorReplayArchiveProofLimitsV1 {
                max_successor_receipts: 1_024,
                max_successor_proof_bytes: 1_048_576,
            })
        );
        assert!(resolve_por_archive(&requested, PorReplayArchive::exact(binding)).is_ok());

        let mut config = default_runtime_config();
        config.torii.sorafs_storage.por_replay_archive = Some(por_archive_config(binding));
        let from_config = IrohaRuntimeProviderBindingsV1::try_from_config(&config)
            .expect("project archive through the sanitized config collector");
        assert!(from_config.iter().any(|candidate| {
            candidate.slot() == IrohaRuntimeProviderSlotV1::PorFinalizedReplayArchive
                && candidate.por_replay_archive_binding() == Some(binding)
                && candidate.por_replay_archive_proof_limits()
                    == Some(PorReplayArchiveProofLimitsV1 {
                        max_successor_receipts: 1_024,
                        max_successor_proof_bytes: 1_048_576,
                    })
        }));
    }

    #[test]
    fn por_replay_archive_projection_rejects_zero_and_excessive_proof_limits() {
        let binding = por_archive_binding(0xB7);
        let mut zero_receipts = por_archive_config(binding);
        zero_receipts.max_successor_receipts = 0;
        let mut zero_bytes = por_archive_config(binding);
        zero_bytes.max_successor_proof_bytes = 0;
        let mut excessive_receipts = por_archive_config(binding);
        excessive_receipts.max_successor_receipts =
            iroha_config::parameters::defaults::sorafs::storage::por_replay_archive::
                MAX_SUCCESSOR_RECEIPTS_LIMIT
                + 1;
        let mut excessive_bytes = por_archive_config(binding);
        excessive_bytes.max_successor_proof_bytes =
            iroha_config::parameters::defaults::sorafs::storage::por_replay_archive::
                MAX_SUCCESSOR_PROOF_BYTES_LIMIT
                + 1;

        for invalid in [
            zero_receipts,
            zero_bytes,
            excessive_receipts,
            excessive_bytes,
        ] {
            assert_eq!(
                IrohaRuntimeProviderBindingV1::try_new_por_replay_archive(&invalid),
                Err(IrohaRuntimeProviderRegistryErrorV1::InvalidBinding(
                    IrohaRuntimeProviderSlotV1::PorFinalizedReplayArchive,
                ))
            );
        }
    }

    #[test]
    fn por_replay_archive_resolution_rejects_missing_and_unrequested_dependencies() {
        let binding = por_archive_binding(0xB4);
        let requested = por_archive_request(binding);
        assert!(matches!(
            resolve_runtime_deps_from_bindings(&requested, Some(&EmptyRegistry)),
            Err(IrohaRuntimeProviderRegistryErrorV1::IncompleteResolution)
        ));

        let empty = IrohaRuntimeProviderBindingsV1 {
            chain_id: "por-replay-test-chain".to_owned(),
            bindings: Vec::new(),
        };
        assert!(matches!(
            resolve_por_archive(&empty, PorReplayArchive::exact(binding)),
            Err(IrohaRuntimeProviderRegistryErrorV1::UnexpectedProviders)
        ));
    }

    #[test]
    fn por_replay_archive_resolution_rejects_substitution_staleness_and_drift() {
        let binding = por_archive_binding(0xB5);
        let requested = por_archive_request(binding);
        let mut substituted = PorReplayArchive::exact(binding);
        substituted.first_binding = sorafs_node::PorFinalizedReplayArchiveBindingV1::try_new(
            [0xB6; 32],
            binding.revision,
            binding.policy_digest,
            binding.signing_public_key,
        )
        .expect("valid substituted binding");
        assert!(matches!(
            resolve_por_archive(&requested, substituted),
            Err(IrohaRuntimeProviderRegistryErrorV1::BindingMismatch)
        ));

        let mut test_marked = PorReplayArchive::exact(binding);
        test_marked.handle = "hsm://sorafs/por-replay-archive/test-provider";
        assert!(matches!(
            resolve_por_archive(&requested, test_marked),
            Err(IrohaRuntimeProviderRegistryErrorV1::TestProviderRejected)
        ));

        let mut stale = PorReplayArchive::exact(binding);
        stale.readiness_error =
            Some(sorafs_node::PorFinalizedReplayArchiveExternalErrorV1::Rejected);
        assert!(matches!(
            resolve_por_archive(&requested, stale),
            Err(IrohaRuntimeProviderRegistryErrorV1::StaleOrRevoked)
        ));

        let mut drifted = PorReplayArchive::exact(binding);
        drifted.later_binding = Some(
            sorafs_node::PorFinalizedReplayArchiveBindingV1::try_new(
                binding.archive_id,
                binding.revision + 1,
                binding.policy_digest,
                binding.signing_public_key,
            )
            .expect("valid later binding"),
        );
        assert!(matches!(
            resolve_por_archive(&requested, drifted),
            Err(IrohaRuntimeProviderRegistryErrorV1::StaleOrRevoked)
        ));
    }

    #[derive(Debug)]
    struct FencedPrivacyRuntime {
        handle: &'static str,
        qualification: Option<sorafs_node::GovernanceDagRuntimeProviderQualificationV1>,
    }

    impl FencedPrivacyRuntime {
        const fn exact() -> Self {
            Self {
                handle: FENCED_PRIVACY_HANDLE,
                qualification: Some(FENCED_PRIVACY_QUALIFICATION),
            }
        }
    }

    impl sorafs_node::FencedTransparencyPublisherV1 for FencedPrivacyRuntime {
        fn handle(&self) -> &str {
            self.handle
        }

        fn qualification(
            &self,
        ) -> Result<sorafs_node::GovernanceDagRuntimeProviderQualificationV1, String> {
            self.qualification
                .ok_or_else(|| "redacted test qualification failure".to_owned())
        }

        fn compare_and_append_privacy(
            &self,
            _request: &sorafs_node::FencedPrivacyPublicationRequestV1,
        ) -> Result<
            sorafs_node::FencedPrivacyPublicationReceiptV1,
            sorafs_node::FencedTransparencyPublishErrorV1,
        > {
            Err(sorafs_node::FencedTransparencyPublishErrorV1::Rejected)
        }
    }

    impl sorafs_node::FencedTransparencyAuthoritativeHeadReaderV1 for FencedPrivacyRuntime {
        fn handle(&self) -> &str {
            self.handle
        }

        fn qualification(
            &self,
        ) -> Result<sorafs_node::GovernanceDagRuntimeProviderQualificationV1, String> {
            self.qualification
                .ok_or_else(|| "redacted test qualification failure".to_owned())
        }

        fn read_authoritative_head_with_ancestry(
            &self,
            required_ancestors: &[sorafs_node::FencedTransparencyTargetHeadV1],
            required_publications: &[sorafs_node::FencedTransparencyPublicationInclusionV1],
        ) -> Result<sorafs_node::FencedTransparencyHeadAncestryProofV1, String> {
            if !required_ancestors.is_empty() || !required_publications.is_empty() {
                return Err(
                    "fresh fused privacy registry target cannot prove retained ancestry or publication inclusion"
                        .to_owned(),
                );
            }
            sorafs_node::FencedTransparencyHeadAncestryProofV1::try_new(
                None,
                Vec::new(),
                Vec::new(),
                [0xFA; 32],
            )
            .map_err(|_| {
                "fresh fused privacy registry target returned a malformed genesis proof".to_owned()
            })
        }
    }

    fn fenced_privacy_dependencies(
        writer: Option<Arc<FencedPrivacyRuntime>>,
        reader: Option<Arc<FencedPrivacyRuntime>>,
    ) -> IrohaRuntimeDeps {
        let mut dependencies = IrohaRuntimeDeps::default();
        if let Some(writer) = writer {
            let writer: Arc<dyn sorafs_node::FencedTransparencyPublisherV1> = writer;
            dependencies = dependencies.with_sorafs_fenced_transparency_publisher(writer);
        }
        if let Some(reader) = reader {
            let reader: Arc<dyn sorafs_node::FencedTransparencyAuthoritativeHeadReaderV1> = reader;
            dependencies = dependencies.with_sorafs_fenced_transparency_head_reader(reader);
        }
        dependencies
    }

    fn configure_fenced_privacy_runtime(config: &mut Config) {
        config
            .torii
            .sorafs_storage
            .privacy_aggregates
            .fenced_privacy_publisher = Some(
            iroha_config::parameters::actual::SorafsTransparencyRuntimeProviderBinding {
                handle: FENCED_PRIVACY_HANDLE.to_owned(),
                revision: FENCED_PRIVACY_QUALIFICATION.revision,
                policy_digest: FENCED_PRIVACY_QUALIFICATION.policy_digest,
            },
        );
    }

    struct TransparencyLeaderLeaseProvider;

    impl sorafs_node::ProductionTransparencyRuntimeProviderV1 for TransparencyLeaderLeaseProvider {
        fn handle(&self) -> &str {
            TRANSPARENCY_LEADER_LEASE_HANDLE
        }

        fn qualification(
            &self,
        ) -> Result<sorafs_node::TransparencyRuntimeProviderQualificationV1, String> {
            Ok(TRANSPARENCY_LEADER_LEASE_QUALIFICATION)
        }
    }

    impl sorafs_node::TransparencyLeaderLeaseProviderV1 for TransparencyLeaderLeaseProvider {
        fn acquire(
            &self,
            _request: &sorafs_node::TransparencyLeaderLeaseAcquireRequestV1,
        ) -> Result<
            sorafs_node::TransparencyLeaderLeaseGrantV1,
            sorafs_node::TransparencyLeaderLeaseProviderErrorV1,
        > {
            Err(sorafs_node::TransparencyLeaderLeaseProviderErrorV1::Unavailable)
        }

        fn renew(
            &self,
            _request: &sorafs_node::TransparencyLeaderLeaseRenewRequestV1,
        ) -> Result<
            sorafs_node::TransparencyLeaderLeaseGrantV1,
            sorafs_node::TransparencyLeaderLeaseProviderErrorV1,
        > {
            Err(sorafs_node::TransparencyLeaderLeaseProviderErrorV1::Unavailable)
        }

        fn release(
            &self,
            _request: &sorafs_node::TransparencyLeaderLeaseReleaseRequestV1,
        ) -> Result<
            sorafs_node::TransparencyLeaderLeaseReleaseReceiptV1,
            sorafs_node::TransparencyLeaderLeaseProviderErrorV1,
        > {
            Err(sorafs_node::TransparencyLeaderLeaseProviderErrorV1::Unavailable)
        }
    }

    #[derive(Debug)]
    struct GovernanceAuthenticator {
        handle: &'static str,
        key_handle: &'static str,
        nonce: AtomicU64,
    }

    impl GovernanceAuthenticator {
        fn new(handle: &'static str) -> Self {
            Self {
                handle,
                key_handle: handle,
                nonce: AtomicU64::new(0),
            }
        }

        fn with_key_from(mut self, handle: &'static str) -> Self {
            self.key_handle = handle;
            self
        }
    }

    #[derive(Debug)]
    struct GovernanceSigner {
        handle: &'static str,
        later_handle: Option<&'static str>,
        handle_calls: AtomicUsize,
        publisher_peer_id: Vec<u8>,
        later_publisher_peer_id: Option<Vec<u8>>,
        publisher_peer_id_calls: AtomicUsize,
        key_pair: KeyPair,
        signing_key_pair: Option<KeyPair>,
        sign_error: bool,
        first_qualification: sorafs_node::GovernanceDagRuntimeProviderQualificationV1,
        later_qualification: Option<sorafs_node::GovernanceDagRuntimeProviderQualificationV1>,
        qualification_calls: AtomicUsize,
        later_public_key: Option<[u8; 32]>,
        public_key_calls: AtomicUsize,
    }

    impl GovernanceSigner {
        fn exact() -> Self {
            Self {
                handle: GOVERNANCE_SIGNER_HANDLE,
                later_handle: None,
                handle_calls: AtomicUsize::new(0),
                publisher_peer_id: GOVERNANCE_PUBLISHER_PEER_ID.as_bytes().to_vec(),
                later_publisher_peer_id: None,
                publisher_peer_id_calls: AtomicUsize::new(0),
                key_pair: governance_signer_keypair(0x73),
                signing_key_pair: None,
                sign_error: false,
                first_qualification: GOVERNANCE_QUALIFICATION,
                later_qualification: None,
                qualification_calls: AtomicUsize::new(0),
                later_public_key: None,
                public_key_calls: AtomicUsize::new(0),
            }
        }
    }

    impl sorafs_node::GovernanceDagRuntimeSigner for GovernanceSigner {
        fn handle(&self) -> &str {
            let call = self.handle_calls.fetch_add(1, Ordering::Relaxed);
            if call == 0 {
                self.handle
            } else {
                self.later_handle.unwrap_or(self.handle)
            }
        }

        fn qualification(
            &self,
        ) -> Result<sorafs_node::GovernanceDagRuntimeProviderQualificationV1, String> {
            let call = self.qualification_calls.fetch_add(1, Ordering::Relaxed);
            Ok(if call == 0 {
                self.first_qualification
            } else {
                self.later_qualification.unwrap_or(self.first_qualification)
            })
        }

        fn publisher_peer_id(&self) -> &[u8] {
            let call = self.publisher_peer_id_calls.fetch_add(1, Ordering::Relaxed);
            if call == 0 {
                &self.publisher_peer_id
            } else {
                self.later_publisher_peer_id
                    .as_deref()
                    .unwrap_or(&self.publisher_peer_id)
            }
        }

        fn public_key(&self) -> [u8; 32] {
            let first = self
                .key_pair
                .public_key()
                .to_bytes()
                .1
                .try_into()
                .expect("Ed25519 public key has 32 bytes");
            let call = self.public_key_calls.fetch_add(1, Ordering::Relaxed);
            if call == 0 {
                first
            } else {
                self.later_public_key.unwrap_or(first)
            }
        }

        fn sign(&self, payload: &[u8]) -> Result<[u8; 64], String> {
            if self.sign_error {
                return Err("redacted Governance DAG signing failure".to_owned());
            }
            let key_pair = self.signing_key_pair.as_ref().unwrap_or(&self.key_pair);
            iroha_crypto::Signature::try_new(key_pair.private_key(), payload)
                .map_err(|_| "redacted Governance DAG signing failure".to_owned())?
                .payload()
                .try_into()
                .map_err(|_| "redacted Governance DAG signature width failure".to_owned())
        }
    }

    fn governance_auth_keypair(handle: &str) -> KeyPair {
        let seed = if handle == GOVERNANCE_IPFS_HANDLE {
            0x71
        } else {
            0x72
        };
        KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519)
            .expect("Governance DAG request-auth test key")
    }

    fn governance_auth_public_key(handle: &str) -> [u8; 32] {
        let keypair = governance_auth_keypair(handle);
        let public_key = keypair.public_key().to_bytes().1;
        let mut bytes = [0_u8; 32];
        bytes.copy_from_slice(&public_key);
        bytes
    }

    impl sorafs_node::GovernanceDagRequestAuthenticator for GovernanceAuthenticator {
        fn handle(&self) -> &str {
            self.handle
        }

        fn qualification(
            &self,
        ) -> Result<sorafs_node::GovernanceDagRuntimeProviderQualificationV1, String> {
            Ok(GOVERNANCE_QUALIFICATION)
        }

        fn public_key(&self) -> [u8; 32] {
            governance_auth_public_key(self.key_handle)
        }

        fn authenticate(
            &self,
            request: &sorafs_node::GovernanceDagCanonicalRequestV1,
        ) -> Result<sorafs_node::GovernanceDagRequestAuthenticationEnvelopeV1, String> {
            let issued_at_unix_secs = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_err(|_| "redacted request-auth clock failure".to_owned())?
                .as_secs();
            let expires_at_unix_secs = issued_at_unix_secs
                .checked_add(30)
                .ok_or_else(|| "redacted request-auth clock failure".to_owned())?;
            let sequence = self
                .nonce
                .fetch_add(1, Ordering::Relaxed)
                .checked_add(1)
                .ok_or_else(|| "redacted request-auth nonce exhaustion".to_owned())?;
            let mut nonce = request.request_digest();
            nonce[..8].copy_from_slice(&sequence.to_be_bytes());
            let public_key = self.public_key();
            let payload =
                sorafs_node::GovernanceDagRequestAuthenticationEnvelopeV1::signing_payload(
                    request,
                    issued_at_unix_secs,
                    expires_at_unix_secs,
                    nonce,
                    public_key,
                );
            let signature = iroha_crypto::Signature::try_new(
                governance_auth_keypair(self.key_handle).private_key(),
                &payload,
            )
            .map_err(|_| "redacted request-auth signing failure".to_owned())?;
            let mut signature_bytes = [0_u8; 64];
            signature_bytes.copy_from_slice(signature.payload());
            sorafs_node::GovernanceDagRequestAuthenticationEnvelopeV1::try_new(
                request,
                issued_at_unix_secs,
                expires_at_unix_secs,
                nonce,
                public_key,
                signature_bytes,
            )
            .map_err(str::to_owned)
        }
    }

    #[derive(Debug, Default)]
    struct GovernanceCheckpointStoreState {
        checkpoint: Option<sorafs_node::GovernanceDagSealedStateRecord>,
        publish_intent: Option<sorafs_node::GovernanceDagSealedStateRecord>,
        producer_checkpoint: Option<sorafs_node::GovernanceDagSealedStateRecord>,
        producer_publish_intent: Option<sorafs_node::GovernanceDagSealedStateRecord>,
        checkpoint_generation_floor: u64,
        publish_intent_generation_floor: u64,
        producer_checkpoint_generation_floor: u64,
        producer_publish_intent_generation_floor: u64,
    }

    impl GovernanceCheckpointStoreState {
        fn slot(
            &self,
            slot: sorafs_node::GovernanceDagSealedStateSlot,
        ) -> &Option<sorafs_node::GovernanceDagSealedStateRecord> {
            match slot {
                sorafs_node::GovernanceDagSealedStateSlot::Checkpoint => &self.checkpoint,
                sorafs_node::GovernanceDagSealedStateSlot::PublishIntent => &self.publish_intent,
                sorafs_node::GovernanceDagSealedStateSlot::ProducerCheckpoint => {
                    &self.producer_checkpoint
                }
                sorafs_node::GovernanceDagSealedStateSlot::ProducerPublishIntent => {
                    &self.producer_publish_intent
                }
            }
        }

        fn slot_mut(
            &mut self,
            slot: sorafs_node::GovernanceDagSealedStateSlot,
        ) -> &mut Option<sorafs_node::GovernanceDagSealedStateRecord> {
            match slot {
                sorafs_node::GovernanceDagSealedStateSlot::Checkpoint => &mut self.checkpoint,
                sorafs_node::GovernanceDagSealedStateSlot::PublishIntent => {
                    &mut self.publish_intent
                }
                sorafs_node::GovernanceDagSealedStateSlot::ProducerCheckpoint => {
                    &mut self.producer_checkpoint
                }
                sorafs_node::GovernanceDagSealedStateSlot::ProducerPublishIntent => {
                    &mut self.producer_publish_intent
                }
            }
        }

        const fn generation_floor(&self, slot: sorafs_node::GovernanceDagSealedStateSlot) -> u64 {
            match slot {
                sorafs_node::GovernanceDagSealedStateSlot::Checkpoint => {
                    self.checkpoint_generation_floor
                }
                sorafs_node::GovernanceDagSealedStateSlot::PublishIntent => {
                    self.publish_intent_generation_floor
                }
                sorafs_node::GovernanceDagSealedStateSlot::ProducerCheckpoint => {
                    self.producer_checkpoint_generation_floor
                }
                sorafs_node::GovernanceDagSealedStateSlot::ProducerPublishIntent => {
                    self.producer_publish_intent_generation_floor
                }
            }
        }

        fn set_generation_floor(
            &mut self,
            slot: sorafs_node::GovernanceDagSealedStateSlot,
            generation: u64,
        ) {
            match slot {
                sorafs_node::GovernanceDagSealedStateSlot::Checkpoint => {
                    self.checkpoint_generation_floor = generation;
                }
                sorafs_node::GovernanceDagSealedStateSlot::PublishIntent => {
                    self.publish_intent_generation_floor = generation;
                }
                sorafs_node::GovernanceDagSealedStateSlot::ProducerCheckpoint => {
                    self.producer_checkpoint_generation_floor = generation;
                }
                sorafs_node::GovernanceDagSealedStateSlot::ProducerPublishIntent => {
                    self.producer_publish_intent_generation_floor = generation;
                }
            }
        }
    }

    #[derive(Debug, Default)]
    struct GovernanceCheckpointStore {
        state: Mutex<GovernanceCheckpointStoreState>,
    }

    impl sorafs_node::GovernanceDagSealedCheckpointStore for GovernanceCheckpointStore {
        fn handle(&self) -> &str {
            GOVERNANCE_CHECKPOINT_HANDLE
        }

        fn qualification(
            &self,
        ) -> Result<sorafs_node::GovernanceDagRuntimeProviderQualificationV1, String> {
            Ok(GOVERNANCE_QUALIFICATION)
        }

        fn load(
            &self,
            slot: sorafs_node::GovernanceDagSealedStateSlot,
        ) -> Result<Option<sorafs_node::GovernanceDagSealedStateRecord>, String> {
            let state = self
                .state
                .lock()
                .map_err(|_| "governance checkpoint store lock poisoned".to_owned())?;
            Ok(state.slot(slot).clone())
        }

        fn compare_and_swap(
            &self,
            slot: sorafs_node::GovernanceDagSealedStateSlot,
            expected_revision: Option<[u8; 32]>,
            next: sorafs_node::GovernanceDagSealedStateRecord,
        ) -> Result<(), String> {
            if next.generation == 0 || !next.has_valid_revision(slot) {
                return Err("invalid governance sealed-state record".to_owned());
            }
            let mut state = self
                .state
                .lock()
                .map_err(|_| "governance checkpoint store lock poisoned".to_owned())?;
            let current = state.slot(slot);
            if current.as_ref().map(|record| record.revision) != expected_revision {
                return Err("governance checkpoint compare-and-swap conflict".to_owned());
            }
            let floor = state.generation_floor(slot);
            let generation_is_valid = match slot {
                sorafs_node::GovernanceDagSealedStateSlot::Checkpoint
                | sorafs_node::GovernanceDagSealedStateSlot::ProducerCheckpoint => {
                    next.generation > floor
                }
                sorafs_node::GovernanceDagSealedStateSlot::PublishIntent
                | sorafs_node::GovernanceDagSealedStateSlot::ProducerPublishIntent
                    if current.is_some() =>
                {
                    next.generation >= floor
                }
                sorafs_node::GovernanceDagSealedStateSlot::PublishIntent
                | sorafs_node::GovernanceDagSealedStateSlot::ProducerPublishIntent => {
                    next.generation > floor
                }
            };
            if !generation_is_valid {
                return Err("governance checkpoint generation rollback".to_owned());
            }
            state.set_generation_floor(slot, next.generation);
            *state.slot_mut(slot) = Some(next);
            Ok(())
        }

        fn delete(
            &self,
            slot: sorafs_node::GovernanceDagSealedStateSlot,
            expected_revision: [u8; 32],
        ) -> Result<(), String> {
            if matches!(
                slot,
                sorafs_node::GovernanceDagSealedStateSlot::Checkpoint
                    | sorafs_node::GovernanceDagSealedStateSlot::ProducerCheckpoint
            ) {
                return Err("governance checkpoint record is not transient".to_owned());
            }
            let mut state = self
                .state
                .lock()
                .map_err(|_| "governance checkpoint store lock poisoned".to_owned())?;
            if state.slot(slot).as_ref().map(|record| record.revision) != Some(expected_revision) {
                return Err("governance checkpoint compare-and-swap conflict".to_owned());
            }
            *state.slot_mut(slot) = None;
            Ok(())
        }
    }

    #[test]
    fn governance_checkpoint_store_keeps_sealed_state_slots_isolated_and_monotonic() {
        use sorafs_node::{
            GovernanceDagSealedCheckpointStore as _, GovernanceDagSealedStateRecord,
            GovernanceDagSealedStateSlot as Slot,
        };

        let store = GovernanceCheckpointStore::default();
        let records = [
            (
                Slot::Checkpoint,
                GovernanceDagSealedStateRecord::new(Slot::Checkpoint, 1, vec![0x11]),
            ),
            (
                Slot::PublishIntent,
                GovernanceDagSealedStateRecord::new(Slot::PublishIntent, 1, vec![0x22]),
            ),
            (
                Slot::ProducerCheckpoint,
                GovernanceDagSealedStateRecord::new(Slot::ProducerCheckpoint, 1, vec![0x33]),
            ),
            (
                Slot::ProducerPublishIntent,
                GovernanceDagSealedStateRecord::new(Slot::ProducerPublishIntent, 1, vec![0x44]),
            ),
        ];

        for (slot, record) in &records {
            store
                .compare_and_swap(*slot, None, record.clone())
                .expect("install independent sealed-state record");
        }
        for (slot, record) in &records {
            assert_eq!(
                store.load(*slot).expect("load sealed-state slot"),
                Some(record.clone())
            );
        }

        for slot in [Slot::Checkpoint, Slot::ProducerCheckpoint] {
            let current = store
                .load(slot)
                .expect("load checkpoint")
                .expect("checkpoint exists");
            let replacement = GovernanceDagSealedStateRecord::new(slot, 1, vec![0x55]);
            assert!(
                store
                    .compare_and_swap(slot, Some(current.revision), replacement)
                    .is_err(),
                "checkpoint generations must strictly advance"
            );
            assert!(
                store.delete(slot, current.revision).is_err(),
                "durable checkpoints are not transient"
            );
        }

        for slot in [Slot::PublishIntent, Slot::ProducerPublishIntent] {
            let current = store
                .load(slot)
                .expect("load publish intent")
                .expect("publish intent exists");
            let replacement = GovernanceDagSealedStateRecord::new(slot, 1, vec![0x66]);
            store
                .compare_and_swap(slot, Some(current.revision), replacement.clone())
                .expect("an active intent may advance at the same generation");
            store
                .delete(slot, replacement.revision)
                .expect("delete exact transient intent revision");
            assert!(
                store
                    .compare_and_swap(
                        slot,
                        None,
                        GovernanceDagSealedStateRecord::new(slot, 1, vec![0x77]),
                    )
                    .is_err(),
                "deleting an intent must retain its monotonic generation floor"
            );
            store
                .compare_and_swap(
                    slot,
                    None,
                    GovernanceDagSealedStateRecord::new(slot, 2, vec![0x88]),
                )
                .expect("a later intent generation may follow deletion");
        }
    }

    #[derive(Debug)]
    struct EvidenceViewerCheckpointStore;

    impl sorafs_node::evidence_viewer::EvidenceViewerRuntimeProviderV1
        for EvidenceViewerCheckpointStore
    {
        fn handle(&self) -> &str {
            EVIDENCE_CHECKPOINT_HANDLE
        }

        fn qualification(
            &self,
        ) -> Result<
            sorafs_node::evidence_viewer::EvidenceViewerRuntimeProviderQualificationV1,
            sorafs_node::evidence_viewer::EvidenceViewerRuntimeProviderReadinessErrorV1,
        > {
            Ok(EVIDENCE_CHECKPOINT_QUALIFICATION)
        }
    }

    impl sorafs_node::evidence_viewer::EvidenceViewerCheckpointStoreV1
        for EvidenceViewerCheckpointStore
    {
        fn load_latest(
            &self,
        ) -> Result<
            Option<sorafs_node::evidence_viewer::EvidenceViewerCheckpointStoreRecordV1>,
            sorafs_node::evidence_viewer::EvidenceViewerCheckpointStoreExternalErrorV1,
        > {
            Ok(None)
        }

        fn compare_and_swap_latest(
            &self,
            _expected_revision: Option<[u8; 32]>,
            _next: &sorafs_node::evidence_viewer::EvidenceViewerCheckpointStoreRecordV1,
        ) -> Result<(), sorafs_node::evidence_viewer::EvidenceViewerCheckpointStoreExternalErrorV1>
        {
            Ok(())
        }
    }

    #[derive(Debug)]
    struct EvidenceViewerCompactionArchive {
        handle: &'static str,
        archive_id: [u8; 32],
        public_key: [u8; 32],
        first_qualification:
            sorafs_node::evidence_viewer::EvidenceViewerRuntimeProviderQualificationV1,
        later_qualification:
            Option<sorafs_node::evidence_viewer::EvidenceViewerRuntimeProviderQualificationV1>,
        qualification_error:
            Option<sorafs_node::evidence_viewer::EvidenceViewerRuntimeProviderReadinessErrorV1>,
        qualification_calls: AtomicUsize,
    }

    impl EvidenceViewerCompactionArchive {
        fn exact() -> Self {
            Self {
                handle: EVIDENCE_ARCHIVE_HANDLE,
                archive_id: EVIDENCE_ARCHIVE_ID,
                public_key: evidence_archive_public_key(),
                first_qualification: EVIDENCE_ARCHIVE_QUALIFICATION,
                later_qualification: None,
                qualification_error: None,
                qualification_calls: AtomicUsize::new(0),
            }
        }
    }

    impl sorafs_node::evidence_viewer::EvidenceViewerRuntimeProviderV1
        for EvidenceViewerCompactionArchive
    {
        fn handle(&self) -> &str {
            self.handle
        }

        fn qualification(
            &self,
        ) -> Result<
            sorafs_node::evidence_viewer::EvidenceViewerRuntimeProviderQualificationV1,
            sorafs_node::evidence_viewer::EvidenceViewerRuntimeProviderReadinessErrorV1,
        > {
            if let Some(error) = self.qualification_error {
                return Err(error);
            }
            let call = self.qualification_calls.fetch_add(1, Ordering::Relaxed);
            Ok(if call == 0 {
                self.first_qualification
            } else {
                self.later_qualification.unwrap_or(self.first_qualification)
            })
        }
    }

    impl sorafs_node::evidence_viewer::EvidenceViewerCompactionArchiveV1
        for EvidenceViewerCompactionArchive
    {
        fn archive_id(&self) -> [u8; 32] {
            self.archive_id
        }

        fn signing_public_key(&self) -> [u8; 32] {
            self.public_key
        }

        fn install(
            &self,
            _operation_id: [u8; 32],
            _receipt_message: [u8; 32],
            _canonical_artifact: &[u8],
        ) -> Result<[u8; 64], sorafs_node::evidence_viewer::EvidenceViewerExternalErrorV1> {
            Err(sorafs_node::evidence_viewer::EvidenceViewerExternalErrorV1::Rejected)
        }

        fn read(
            &self,
            _operation_id: [u8; 32],
        ) -> Result<
            Option<sorafs_node::evidence_viewer::EvidenceViewerCompactionArchiveReadbackV1>,
            sorafs_node::evidence_viewer::EvidenceViewerExternalErrorV1,
        > {
            Err(sorafs_node::evidence_viewer::EvidenceViewerExternalErrorV1::Rejected)
        }
    }

    #[derive(Debug)]
    struct EvidenceViewerTransparencyPublisher {
        handle: &'static str,
        public_key: [u8; 32],
        qualification: sorafs_node::evidence_viewer::EvidenceViewerRuntimeProviderQualificationV1,
        qualification_error:
            Option<sorafs_node::evidence_viewer::EvidenceViewerRuntimeProviderReadinessErrorV1>,
    }

    impl EvidenceViewerTransparencyPublisher {
        fn exact() -> Self {
            Self {
                handle: EVIDENCE_TRANSPARENCY_PUBLISHER_HANDLE,
                public_key: evidence_archive_public_key(),
                qualification: EVIDENCE_TRANSPARENCY_PUBLISHER_QUALIFICATION,
                qualification_error: None,
            }
        }
    }

    impl sorafs_node::evidence_viewer::EvidenceViewerRuntimeProviderV1
        for EvidenceViewerTransparencyPublisher
    {
        fn handle(&self) -> &str {
            self.handle
        }

        fn qualification(
            &self,
        ) -> Result<
            sorafs_node::evidence_viewer::EvidenceViewerRuntimeProviderQualificationV1,
            sorafs_node::evidence_viewer::EvidenceViewerRuntimeProviderReadinessErrorV1,
        > {
            if let Some(error) = self.qualification_error {
                Err(error)
            } else {
                Ok(self.qualification)
            }
        }
    }

    impl sorafs_node::evidence_viewer::transparency_producer::EvidenceViewerTransparencyPublisherV1
        for EvidenceViewerTransparencyPublisher
    {
        fn public_key(&self) -> [u8; 32] {
            self.public_key
        }

        fn load_head(
            &self,
        ) -> Result<
            Option<
                sorafs_node::evidence_viewer::transparency_producer::
                    EvidenceViewerSignedTransparencyHeadV1,
            >,
            sorafs_node::evidence_viewer::transparency_producer::
                EvidenceViewerTransparencyPublisherExternalErrorV1,
        >{
            Ok(None)
        }

        fn compare_and_publish(
            &self,
            _body: &sorafs_node::evidence_viewer::transparency_producer::
                EvidenceViewerTransparencyHeadBodyV1,
        ) -> Result<
            (),
            sorafs_node::evidence_viewer::transparency_producer::
                EvidenceViewerTransparencyPublisherExternalErrorV1,
        >{
            Ok(())
        }
    }

    fn configure_governance_service(config: &mut Config, head_mode: &str) {
        let service = &mut config.torii.sorafs_storage.governance_dag_service;
        service.enabled = true;
        service.head_mode = head_mode.to_owned();
        service.ipfs_authenticator_handle = Some(GOVERNANCE_IPFS_HANDLE.to_owned());
        service.ipfs_authenticator_revision = Some(GOVERNANCE_QUALIFICATION.revision);
        service.ipfs_authenticator_policy_digest = Some(GOVERNANCE_QUALIFICATION.policy_digest);
        service.ipfs_request_auth_public_key =
            Some(governance_auth_public_key(GOVERNANCE_IPFS_HANDLE));
        service.checkpoint_store_handle = Some(GOVERNANCE_CHECKPOINT_HANDLE.to_owned());
        service.checkpoint_store_revision = Some(GOVERNANCE_QUALIFICATION.revision);
        service.checkpoint_store_policy_digest = Some(GOVERNANCE_QUALIFICATION.policy_digest);
        if head_mode == "signed_http" {
            service.head_authenticator_handle = Some(GOVERNANCE_HEAD_HANDLE.to_owned());
            service.head_authenticator_revision = Some(GOVERNANCE_QUALIFICATION.revision);
            service.head_authenticator_policy_digest = Some(GOVERNANCE_QUALIFICATION.policy_digest);
            service.head_request_auth_public_key =
                Some(governance_auth_public_key(GOVERNANCE_HEAD_HANDLE));
        } else {
            service.head_authenticator_handle = None;
            service.head_authenticator_revision = None;
            service.head_authenticator_policy_digest = None;
            service.head_request_auth_public_key = None;
        }
    }

    fn governance_service_projection(
        signed_head: bool,
    ) -> GovernanceDagServiceBindingProjectionV1<'static> {
        GovernanceDagServiceBindingProjectionV1 {
            ipfs_authenticator: GovernanceDagRequestAuthBindingProjectionV1 {
                handle: GOVERNANCE_IPFS_HANDLE,
                qualification: GOVERNANCE_QUALIFICATION,
                public_key: governance_auth_public_key(GOVERNANCE_IPFS_HANDLE),
            },
            head_authenticator: signed_head.then(|| GovernanceDagRequestAuthBindingProjectionV1 {
                handle: GOVERNANCE_HEAD_HANDLE,
                qualification: GOVERNANCE_QUALIFICATION,
                public_key: governance_auth_public_key(GOVERNANCE_HEAD_HANDLE),
            }),
            request_auth_max_body_bytes: 65_536,
            checkpoint_store_handle: GOVERNANCE_CHECKPOINT_HANDLE,
            checkpoint_store_qualification: GOVERNANCE_QUALIFICATION,
        }
    }

    fn configure_governance_producer(config: &mut Config) {
        let storage = &mut config.torii.sorafs_storage;
        storage.enabled = true;
        storage.governance_dag_dir = Some("/var/lib/iroha/governance-producer".into());
        storage.governance_dag_publisher_peer_id = Some(GOVERNANCE_PUBLISHER_PEER_ID.to_owned());
        storage.governance_dag_signer_handle = Some(GOVERNANCE_SIGNER_HANDLE.to_owned());
        storage.governance_dag_signer_revision = Some(GOVERNANCE_QUALIFICATION.revision);
        storage.governance_dag_signer_policy_digest = Some(GOVERNANCE_QUALIFICATION.policy_digest);
        storage.governance_dag_publisher_public_key_hex =
            Some(hex::encode(governance_signer_public_key(0x73)));

        let service = &mut storage.governance_dag_service;
        service.enabled = false;
        service.checkpoint_store_handle = Some(GOVERNANCE_CHECKPOINT_HANDLE.to_owned());
        service.checkpoint_store_revision = Some(GOVERNANCE_QUALIFICATION.revision);
        service.checkpoint_store_policy_digest = Some(GOVERNANCE_QUALIFICATION.policy_digest);
    }

    fn governance_service_dependencies(include_head: bool) -> IrohaRuntimeDeps {
        let dependencies = IrohaRuntimeDeps::default()
            .with_sorafs_governance_dag_ipfs_authenticator(Arc::new(GovernanceAuthenticator::new(
                GOVERNANCE_IPFS_HANDLE,
            )))
            .with_sorafs_governance_dag_checkpoint_store(Arc::new(
                GovernanceCheckpointStore::default(),
            ));
        if include_head {
            dependencies.with_sorafs_governance_dag_head_authenticator(Arc::new(
                GovernanceAuthenticator::new(GOVERNANCE_HEAD_HANDLE),
            ))
        } else {
            dependencies
        }
    }

    macro_rules! define_explicit_test_signer {
        ($name:ident, $role:ident, $handle:literal, $seed:literal, $signer_trait:ident, $error:ident) => {
            struct $name {
                public_key: iroha_crypto::PublicKey,
            }

            impl $name {
                fn new() -> Self {
                    let keypair = iroha_crypto::KeyPair::try_from_seed(
                        vec![$seed; 32],
                        iroha_crypto::Algorithm::Ed25519,
                    )
                    .expect("derive explicit runtime-signer fixture");
                    Self {
                        public_key: keypair.public_key().clone(),
                    }
                }

                fn expected_binding(&self) -> iroha_torii::SorafsNativeTransactionSignerBindingV1 {
                    iroha_torii::SorafsNativeTransactionSignerBindingV1::try_new(
                        iroha_torii::SorafsNativeTransactionSignerRoleV1::$role,
                        $handle,
                        iroha_data_model::account::AccountId::new(self.public_key.clone()),
                        self.public_key.clone(),
                        iroha_torii::SorafsNativeTransactionSignerQualificationV1::new(
                            1,
                            [$seed; 32],
                        ),
                    )
                    .expect("valid explicit runtime-signer binding")
                }
            }

            impl iroha_torii::SorafsNativeTransactionSignerProviderV1 for $name {
                fn role(&self) -> iroha_torii::SorafsNativeTransactionSignerRoleV1 {
                    iroha_torii::SorafsNativeTransactionSignerRoleV1::$role
                }

                fn handle(&self) -> &str {
                    $handle
                }

                fn authority(&self) -> iroha_data_model::account::AccountId {
                    iroha_data_model::account::AccountId::new(self.public_key.clone())
                }

                fn public_key(
                    &self,
                ) -> Result<
                    iroha_crypto::PublicKey,
                    iroha_torii::SorafsNativeTransactionSignerProbeErrorV1,
                > {
                    Ok(self.public_key.clone())
                }

                fn qualification(
                    &self,
                ) -> Result<
                    iroha_torii::SorafsNativeTransactionSignerQualificationV1,
                    iroha_torii::SorafsNativeTransactionSignerProbeErrorV1,
                > {
                    Ok(
                        iroha_torii::SorafsNativeTransactionSignerQualificationV1::new(
                            1,
                            [$seed; 32],
                        ),
                    )
                }
            }

            impl iroha_torii::$signer_trait for $name {
                fn sign(
                    &self,
                    _payload: iroha_data_model::transaction::TransactionPayload,
                ) -> Result<iroha_data_model::transaction::SignedTransaction, iroha_torii::$error>
                {
                    Err(iroha_torii::$error::Refused)
                }
            }
        };
    }

    define_explicit_test_signer!(
        ProofOutcomeTestSigner,
        ProofOutcome,
        "hsm://registry/proof-outcome/primary",
        0x31,
        SoraFsProofOutcomeTransactionSigner,
        SoraFsProofOutcomeSigningError
    );
    define_explicit_test_signer!(
        RepairTestSigner,
        Repair,
        "hsm://registry/repair/primary",
        0x32,
        SoraFsRepairTransactionSigner,
        SoraFsRepairTransactionSigningError
    );
    define_explicit_test_signer!(
        ReserveTestSigner,
        Reserve,
        "hsm://registry/reserve/primary",
        0x33,
        SoraFsReserveTransactionSigner,
        SoraFsReserveTransactionSigningError
    );
    define_explicit_test_signer!(
        OrderbookTestSigner,
        Orderbook,
        "hsm://registry/orderbook/primary",
        0x34,
        SoraFsOrderbookTransactionSigner,
        SoraFsOrderbookTransactionSigningError
    );

    struct RoleConfusedProofOutcomeSigner(ProofOutcomeTestSigner);

    impl iroha_torii::SorafsNativeTransactionSignerProviderV1 for RoleConfusedProofOutcomeSigner {
        fn role(&self) -> iroha_torii::SorafsNativeTransactionSignerRoleV1 {
            iroha_torii::SorafsNativeTransactionSignerRoleV1::Repair
        }

        fn handle(&self) -> &str {
            iroha_torii::SorafsNativeTransactionSignerProviderV1::handle(&self.0)
        }

        fn authority(&self) -> iroha_data_model::account::AccountId {
            iroha_torii::SorafsNativeTransactionSignerProviderV1::authority(&self.0)
        }

        fn public_key(
            &self,
        ) -> Result<iroha_crypto::PublicKey, iroha_torii::SorafsNativeTransactionSignerProbeErrorV1>
        {
            iroha_torii::SorafsNativeTransactionSignerProviderV1::public_key(&self.0)
        }

        fn qualification(
            &self,
        ) -> Result<
            iroha_torii::SorafsNativeTransactionSignerQualificationV1,
            iroha_torii::SorafsNativeTransactionSignerProbeErrorV1,
        > {
            iroha_torii::SorafsNativeTransactionSignerProviderV1::qualification(&self.0)
        }
    }

    impl iroha_torii::SoraFsProofOutcomeTransactionSigner for RoleConfusedProofOutcomeSigner {
        fn sign(
            &self,
            _payload: iroha_data_model::transaction::TransactionPayload,
        ) -> Result<
            iroha_data_model::transaction::SignedTransaction,
            iroha_torii::SoraFsProofOutcomeSigningError,
        > {
            Err(iroha_torii::SoraFsProofOutcomeSigningError::Refused)
        }
    }

    fn default_runtime_config() -> Config {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../defaults/kagami/iroha3-dev/config.toml");
        Config::from_toml_source(
            TomlSource::from_file(path).expect("read checked-in default daemon config"),
        )
        .expect("resolve checked-in default daemon config")
    }

    fn configure_moderation_runtime(config: &mut Config) {
        let maintenance_key = KeyPair::try_from_seed(vec![0xC1; 32], Algorithm::Ed25519)
            .expect("moderation maintenance key");
        let strict_qualification = iroha_torii::sorafs::moderation_runtime::
            torii_moderation_strict_ingress_qualification_v1();
        config.torii.sorafs_storage.moderation_orchestrator = Some(
            iroha_config::parameters::actual::SorafsModerationOrchestrator {
                checkpoint_path: Path::new("/var/lib/iroha/sorafs/moderation.to").to_path_buf(),
                checkpoint_store_handle: "sealed://sorafs/moderation/checkpoint-primary".to_owned(),
                checkpoint_store_revision: 7,
                checkpoint_store_policy_digest: [0xC7; 32],
                maintenance_authority: iroha_data_model::account::AccountId::new(
                    maintenance_key.public_key().clone(),
                ),
                transaction_signer_handle: "hsm://sorafs/moderation/signer-primary".to_owned(),
                transaction_signer_revision: 8,
                transaction_signer_policy_digest: [0xC8; 32],
                strict_ingress_handle: iroha_torii::sorafs::moderation_runtime::
                    TORII_MODERATION_STRICT_INGRESS_HANDLE_V1.to_owned(),
                strict_ingress_revision: strict_qualification.revision(),
                strict_ingress_policy_digest: strict_qualification.policy_digest(),
                settlement_handoff_handle: "queue://sorafs/moderation/settlement-primary".to_owned(),
                settlement_handoff_revision: 9,
                settlement_handoff_policy_digest: [0xC9; 32],
                publication_handoff_handle: "dag://sorafs/moderation/publication-primary".to_owned(),
                publication_handoff_revision: 10,
                publication_handoff_policy_digest: [0xCA; 32],
                panel_notification_handle: "queue://sorafs/moderation/notification-primary".to_owned(),
                panel_notification_revision: 11,
                panel_notification_policy_digest: [0xCB; 32],
                max_cases: 128,
                max_events: 512,
                max_outbox_entries: 128,
                max_idempotency_records: 512,
                max_handoffs: 128,
                max_submit_attempts: 4,
                checkpoint_max_bytes: Bytes(4 * 1024 * 1024),
                worker_interval: Duration::from_secs(1),
                maintenance_batch_limit: 64,
            },
        );
    }

    #[test]
    fn moderation_strict_ingress_is_qualified_during_catalog_projection() {
        let mut config = default_runtime_config();
        configure_moderation_runtime(&mut config);

        let bindings = IrohaRuntimeProviderBindingsV1::try_from_config(&config)
            .expect("qualify Torii strict ingress before broker projection");
        assert!(bindings.iter().all(|binding| {
            binding.handle()
                != iroha_torii::sorafs::moderation_runtime::
                    TORII_MODERATION_STRICT_INGRESS_HANDLE_V1
        }));
    }

    #[test]
    fn moderation_strict_ingress_preflight_rejects_missing_substituted_stale_and_test_bindings() {
        for (mutation, expected) in [
            (0, IrohaRuntimeProviderRegistryErrorV1::BindingMismatch),
            (1, IrohaRuntimeProviderRegistryErrorV1::BindingMismatch),
            (2, IrohaRuntimeProviderRegistryErrorV1::StaleOrRevoked),
            (3, IrohaRuntimeProviderRegistryErrorV1::StaleOrRevoked),
            (4, IrohaRuntimeProviderRegistryErrorV1::TestProviderRejected),
        ] {
            let mut config = default_runtime_config();
            configure_moderation_runtime(&mut config);
            let moderation = config
                .torii
                .sorafs_storage
                .moderation_orchestrator
                .as_mut()
                .expect("configured moderation runtime");
            match mutation {
                0 => moderation.strict_ingress_handle.clear(),
                1 => {
                    moderation.strict_ingress_handle =
                        "torii.sorafs.moderation-strict-ingress.secondary".to_owned();
                }
                2 => moderation.strict_ingress_revision += 1,
                3 => moderation.strict_ingress_policy_digest = [0xD1; 32],
                4 => {
                    moderation.strict_ingress_handle =
                        "torii.sorafs.moderation-test-ingress.v1".to_owned();
                }
                _ => unreachable!(),
            }

            assert!(matches!(
                resolve_runtime_deps(&config, Some(&EmptyRegistry)),
                Err(error) if error == expected
            ));
        }
    }

    fn configure_provider_ingest_runtime(config: &mut Config) {
        let completion_signer =
            iroha_crypto::KeyPair::try_from_seed(vec![0x61; 32], iroha_crypto::Algorithm::Ed25519)
                .expect("provider-ingest completion signer key");
        config.torii.sorafs_storage.provider_ingest_runtime = Some(
            iroha_config::parameters::actual::SorafsProviderIngestRuntime {
                authenticated_source_fetch_handle:
                    "network://sorafs/provider-ingest/source-primary".to_owned(),
                authenticated_source_fetch_revision: 5,
                authenticated_source_fetch_policy_digest: [0xB1; 32],
                completion_signer_resolver_handle: "hsm://sorafs/provider-ingest/resolver-primary"
                    .to_owned(),
                completion_signer_resolver_revision: 6,
                completion_signer_resolver_policy_digest: [0xB2; 32],
                completion_signer_handle: "pkcs11://sorafs/provider-ingest/signer-primary"
                    .to_owned(),
                completion_signer_adapter_revision: 3,
                completion_signer_policy: sorafs_node::ProviderIngestCompletionSignerPolicyV1 {
                    policy_id: [0xA1; 32],
                    revision: 1,
                    predecessor_digest: None,
                    policy_digest: [0xA2; 32],
                },
                completion_signer_algorithm: iroha_crypto::Algorithm::Ed25519,
                completion_signer_public_key: completion_signer.public_key().clone(),
                checkpoint_store_handle: "sealed://sorafs/provider-ingest/checkpoint-primary"
                    .to_owned(),
                checkpoint_store_revision: 7,
                checkpoint_store_policy_digest: [0xA7; 32],
                scan_interval_ms: 1_000,
                max_page_rows: 64,
                max_pages_per_tick: 4,
                max_source_jobs_per_tick: 32,
                max_source_providers: 1_024,
                source_operation_timeout_ms: 30_000,
                source_lease_renew_interval_ms: 5_000,
                signer_timeout_ms: 10_000,
                ingress_timeout_ms: 10_000,
                completion_transaction_ttl_ms: 30_000,
                finalized_archive:
                    iroha_config::parameters::actual::SorafsProviderIngestFinalizedArchive {
                        relative_root: "provider-ingest-finalized-archive-v1".into(),
                        max_record_bytes: 128 * 1024 * 1024,
                        max_archive_entries: 1_000_000,
                        max_total_bytes: 64 * 1024 * 1024 * 1024,
                        max_providers_per_anchor: 1_024,
                        max_orders_per_provider: 256,
                        max_total_orders_per_anchor: 256,
                        max_page_rows: 64,
                        max_kura_tip_lag_blocks: 2,
                        retention_authority: None,
                    },
                outbox: iroha_config::parameters::actual::SorafsProviderIngestOutbox {
                    max_active_entries: 32,
                    max_terminal_entries: 4_096,
                    max_attempts: 8,
                    checkpoint_max_bytes: Bytes(160 * 1024 * 1024),
                    checkpoint_operation_timeout_ms: 30_000,
                    source_lease_ttl_ms: 30_000,
                    retry_base_delay_ms: 1_000,
                    retry_max_delay_ms: 60_000,
                    terminal_retention_blocks: 100_000,
                    max_signed_transaction_bytes: Bytes(1024 * 1024),
                    max_status_page_size: 256,
                },
            },
        );
    }

    fn configure_reputation_runtime(config: &mut Config) {
        config.torii.sorafs_storage.reputation_runtime = Some(
            iroha_config::parameters::actual::SorafsReputationRuntime {
                state_dir: Path::new("/var/lib/iroha/sorafs/reputation").to_path_buf(),
                finalized_archive_root: Path::new(
                    "/var/lib/iroha/sorafs/reputation-finalized-archive",
                )
                .to_path_buf(),
                finalized_archive_max_record_bytes: 4 * 1024 * 1024,
                finalized_archive_max_entries: 4_096,
                finalized_archive_max_total_bytes: 256 * 1024 * 1024,
                finalized_archive_max_kura_tip_lag_blocks: 2,
                finalized_archive_retention_authority: Some(
                    iroha_config::parameters::actual::
                        SorafsReputationFinalizedArchiveRetentionAuthority {
                            handle: "sealed://sorafs/reputation/retention-primary".to_owned(),
                            revision: 9,
                            policy_digest: [0xC9; 32],
                        },
                ),
                window_start_height: 1,
                window_end_height: 10,
                finalized_query_handle: "ledger://sorafs/reputation/finalized-primary".to_owned(),
                journal_checkpoint_provider_handle:
                    "sealed://sorafs/reputation/journal-primary".to_owned(),
                journal_checkpoint_provider_revision: 1,
                journal_checkpoint_provider_policy_digest: [0x60; 32],
                journal_transaction_submitter_handle:
                    "queue://sorafs/reputation/journal-primary".to_owned(),
                journal_transaction_submitter_revision: 11,
                journal_transaction_submitter_policy_digest: [0x61; 32],
                threshold_signer_handle: "hsm://sorafs/reputation/threshold-primary".to_owned(),
                threshold_signer_revision: 12,
                threshold_signer_policy_digest: [0x62; 32],
                governance_dag_handle: "dag://sorafs/reputation/publisher-primary".to_owned(),
                governance_dag_revision: 13,
                governance_dag_policy_digest: [0x63; 32],
                governance_publisher_peer_id: b"12D3KooWProductionPublisher".to_vec(),
                governance_publisher_public_key: [0x73; 32],
                poll_interval: Duration::from_secs(1),
                page_items: 64,
                max_pages_per_batch: 4_096,
                max_providers: 65_536,
                max_pending_events: 65_536,
                max_replay_receipts: 262_144,
                max_material_delivery_failures: 64,
                ingest_checkpoint_max_bytes: Bytes(64 * 1024 * 1024),
                publication_checkpoint_max_bytes: Bytes(32 * 1024 * 1024),
                por_success_bps: 2_200,
                pdp_success_bps: 2_000,
                potr_success_bps: 1_800,
                latency_bps: 1_500,
                dispute_bps: 1_000,
                token_violation_bps: 500,
                repair_breach_bps: 1_000,
            },
        );
    }

    fn configure_evidence_viewer(config: &mut Config) {
        config.torii.sorafs_storage.evidence_viewer =
            Some(iroha_config::parameters::actual::SorafsEvidenceViewer {
                checkpoint_path: Path::new("/var/lib/iroha/sorafs/evidence-viewer.to")
                    .to_path_buf(),
                checkpoint_max_bytes: Bytes(64 * 1024 * 1024),
                checkpoint_store_handle: EVIDENCE_CHECKPOINT_HANDLE.to_owned(),
                checkpoint_store_revision: EVIDENCE_CHECKPOINT_QUALIFICATION.revision(),
                checkpoint_store_policy_digest: EVIDENCE_CHECKPOINT_QUALIFICATION.policy_digest(),
                session_ttl: Duration::from_secs(15 * 60),
                grant_ttl: Duration::from_secs(5 * 60),
                challenge_ttl: Duration::from_secs(60),
                max_range_bytes: Bytes(64 * 1024 * 1024),
                max_challenges: 1_024,
                max_sessions: 1_024,
                max_receipts: 4_096,
                max_idempotency_records: 4_096,
                retention_after_expiry: Duration::from_secs(24 * 60 * 60),
                webauthn_rp_id: "review.example".to_owned(),
                webauthn_allowed_origins: vec!["https://review.example".to_owned()],
                webauthn_handle: "webauthn://sorafs/evidence-viewer/primary".to_owned(),
                webauthn_revision: 11,
                webauthn_policy_digest: [0xA1; 32],
                grant_handle: "kms://sorafs/evidence-viewer/grants-primary".to_owned(),
                grant_revision: 12,
                grant_policy_digest: [0xA2; 32],
                erasure_handle: "kms://sorafs/evidence-viewer/erasure-primary".to_owned(),
                erasure_revision: 13,
                erasure_policy_digest: [0xA3; 32],
                compaction_archive_handle: EVIDENCE_ARCHIVE_HANDLE.to_owned(),
                compaction_archive_id: EVIDENCE_ARCHIVE_ID,
                compaction_archive_revision: EVIDENCE_ARCHIVE_QUALIFICATION.revision(),
                compaction_archive_policy_digest: EVIDENCE_ARCHIVE_QUALIFICATION.policy_digest(),
                compaction_archive_public_key: evidence_archive_public_key(),
                compaction_interval: Duration::from_secs(60),
                compaction_max_records: 256,
                receipt_signer_handle: "hsm://sorafs/evidence-viewer/receipts-primary".to_owned(),
                receipt_signer_revision: 14,
                receipt_signer_policy_digest: [0xA4; 32],
                receipt_signer_public_key: evidence_archive_public_key(),
                transparency_publisher_handle: EVIDENCE_TRANSPARENCY_PUBLISHER_HANDLE.to_owned(),
                transparency_publisher_revision: EVIDENCE_TRANSPARENCY_PUBLISHER_QUALIFICATION
                    .revision(),
                transparency_publisher_policy_digest: EVIDENCE_TRANSPARENCY_PUBLISHER_QUALIFICATION
                    .policy_digest(),
                transparency_publisher_public_key: evidence_archive_public_key(),
            });
    }

    fn evidence_archive_request() -> IrohaRuntimeProviderBindingsV1 {
        let mut config = default_runtime_config();
        configure_evidence_viewer(&mut config);
        let viewer = config
            .torii
            .sorafs_storage
            .evidence_viewer
            .as_ref()
            .expect("configured evidence viewer");
        IrohaRuntimeProviderBindingsV1 {
            chain_id: "evidence-viewer-archive-test-chain".to_owned(),
            bindings: vec![
                IrohaRuntimeProviderBindingV1::try_new_evidence_viewer_archive(viewer)
                    .expect("valid evidence-viewer archive request"),
            ],
        }
    }

    fn evidence_transparency_publisher_request() -> IrohaRuntimeProviderBindingsV1 {
        let mut config = default_runtime_config();
        configure_evidence_viewer(&mut config);
        let viewer = config
            .torii
            .sorafs_storage
            .evidence_viewer
            .as_ref()
            .expect("configured evidence viewer");
        IrohaRuntimeProviderBindingsV1 {
            chain_id: "evidence-viewer-transparency-test-chain".to_owned(),
            bindings: vec![
                IrohaRuntimeProviderBindingV1::try_new_evidence_viewer_transparency_publisher(
                    viewer,
                )
                .expect("valid evidence-viewer transparency publisher request"),
            ],
        }
    }

    fn actual_native_signer_binding(
        binding: &iroha_torii::SorafsNativeTransactionSignerBindingV1,
    ) -> iroha_config::parameters::actual::SorafsNativeTransactionSignerBinding {
        iroha_config::parameters::actual::SorafsNativeTransactionSignerBinding {
            handle: binding.handle().to_owned(),
            authority: binding.authority().clone(),
            algorithm: binding
                .public_key()
                .try_algorithm()
                .expect("fixture key algorithm"),
            public_key: binding.public_key().clone(),
            revision: binding.qualification().revision(),
            policy_digest: binding.qualification().policy_digest(),
        }
    }

    fn observed_native_signer_binding(
        provider: &(impl iroha_torii::SorafsNativeTransactionSignerProviderV1 + ?Sized),
    ) -> iroha_torii::SorafsNativeTransactionSignerBindingV1 {
        iroha_torii::SorafsNativeTransactionSignerBindingV1::try_new(
            provider.role(),
            provider.handle(),
            provider.authority(),
            provider
                .public_key()
                .expect("qualified provider public key"),
            provider
                .qualification()
                .expect("qualified provider qualification"),
        )
        .expect("qualified provider exposes a valid immutable binding")
    }

    fn native_signer_catalog(
        entries: impl IntoIterator<
            Item = (
                IrohaRuntimeProviderSlotV1,
                iroha_torii::SorafsNativeTransactionSignerBindingV1,
            ),
        >,
    ) -> IrohaRuntimeProviderBindingsV1 {
        let mut bindings = entries
            .into_iter()
            .map(|(slot, binding)| {
                IrohaRuntimeProviderBindingV1::try_new_native_signer(slot, binding)
                    .expect("valid native signer catalog entry")
            })
            .collect::<Vec<_>>();
        bindings.sort_unstable_by_key(IrohaRuntimeProviderBindingV1::slot);
        IrohaRuntimeProviderBindingsV1 {
            chain_id: "production-chain".to_owned(),
            bindings,
        }
    }

    fn one_binding_catalog() -> IrohaRuntimeProviderBindingsV1 {
        IrohaRuntimeProviderBindingsV1 {
            chain_id: "production-chain".to_owned(),
            bindings: vec![
                IrohaRuntimeProviderBindingV1::try_new(
                    IrohaRuntimeProviderSlotV1::BillingStatementSigner,
                    "kms://billing/statement-primary",
                    Some(1),
                    Some([0x51; 32]),
                )
                .expect("valid production binding"),
            ],
        }
    }

    #[test]
    fn production_handle_filter_rejects_test_markers_and_noncanonical_text() {
        for rejected in [
            "",
            "kms://cluster/test/signer",
            "mock.signer",
            "provider placeholder",
            "provider\nprimary",
            "https://operator:secret@host",
            "https://host/source?token=secret",
            "https://host/source#fragment",
            "https://host/%73ource",
        ] {
            assert!(!is_production_runtime_handle(rejected), "{rejected:?}");
        }
        assert!(is_production_runtime_handle(
            "pkcs11://cluster-a/sorafs-primary"
        ));
    }

    #[test]
    fn binding_rejects_partial_or_zero_qualification() {
        let slot = IrohaRuntimeProviderSlotV1::BillingStatementSigner;
        for (revision, digest) in [
            (Some(0), Some([1; 32])),
            (Some(1), Some([0; 32])),
            (Some(1), None),
            (None, Some([1; 32])),
        ] {
            assert_eq!(
                IrohaRuntimeProviderBindingV1::try_new(
                    slot,
                    "kms://billing/statement-primary",
                    revision,
                    digest,
                ),
                Err(IrohaRuntimeProviderRegistryErrorV1::InvalidBinding(slot))
            );
        }
    }

    #[test]
    fn registry_errors_do_not_echo_handles_or_provider_diagnostics() {
        for error in [
            IrohaRuntimeProviderRegistryErrorV1::InvalidBinding(
                IrohaRuntimeProviderSlotV1::GatewayAcmeClient,
            ),
            IrohaRuntimeProviderRegistryErrorV1::MissingRegistry,
            IrohaRuntimeProviderRegistryErrorV1::Unavailable,
            IrohaRuntimeProviderRegistryErrorV1::BindingMismatch,
            IrohaRuntimeProviderRegistryErrorV1::StaleOrRevoked,
            IrohaRuntimeProviderRegistryErrorV1::TestProviderRejected,
            IrohaRuntimeProviderRegistryErrorV1::IncompleteResolution,
            IrohaRuntimeProviderRegistryErrorV1::UnexpectedProviders,
        ] {
            let rendered = error.to_string();
            assert!(!rendered.contains("pkcs11"));
            assert!(!rendered.contains("credential"));
            assert!(!rendered.contains("private"));
        }
    }

    #[test]
    fn disabled_services_need_no_registry() {
        let bindings = IrohaRuntimeProviderBindingsV1 {
            chain_id: "default-chain".to_owned(),
            bindings: Vec::new(),
        };

        let dependencies =
            resolve_runtime_deps_from_bindings(&bindings, None).expect("empty default resolution");

        assert!(dependencies.is_empty());
    }

    #[test]
    fn configured_provider_binding_requires_registry() {
        assert!(matches!(
            resolve_runtime_deps_from_bindings(&one_binding_catalog(), None),
            Err(IrohaRuntimeProviderRegistryErrorV1::MissingRegistry)
        ));
    }

    #[test]
    fn configured_provider_binding_rejects_empty_resolution() {
        let registry: Arc<dyn IrohaRuntimeProviderRegistryV1> = Arc::new(EmptyRegistry);
        assert!(matches!(
            resolve_runtime_deps_from_bindings(&one_binding_catalog(), Some(registry.as_ref())),
            Err(IrohaRuntimeProviderRegistryErrorV1::IncompleteResolution)
        ));
    }

    #[test]
    fn evidence_viewer_catalog_projects_exact_checkpoint_store_qualification() {
        let mut config = default_runtime_config();
        configure_evidence_viewer(&mut config);

        let bindings = IrohaRuntimeProviderBindingsV1::try_from_config(&config)
            .expect("project evidence-viewer provider bindings");
        assert_eq!(
            bindings
                .iter()
                .filter(|binding| matches!(
                    binding.slot(),
                    IrohaRuntimeProviderSlotV1::EvidenceViewerWebAuthn
                        | IrohaRuntimeProviderSlotV1::EvidenceViewerGrantAuthority
                        | IrohaRuntimeProviderSlotV1::EvidenceViewerReceiptSigner
                        | IrohaRuntimeProviderSlotV1::EvidenceViewerErasure
                        | IrohaRuntimeProviderSlotV1::EvidenceViewerCheckpointStore
                        | IrohaRuntimeProviderSlotV1::EvidenceViewerCompactionArchive
                        | IrohaRuntimeProviderSlotV1::EvidenceViewerTransparencyPublisher
                ))
                .count(),
            7
        );
        let webauthn = bindings
            .iter()
            .find(|binding| binding.slot() == IrohaRuntimeProviderSlotV1::EvidenceViewerWebAuthn)
            .expect("WebAuthn binding");
        assert_eq!(
            webauthn.evidence_viewer_webauthn_binding(),
            Some(&EvidenceViewerWebAuthnBindingV1 {
                rp_id: "review.example".to_owned(),
                allowed_origins: vec!["https://review.example".to_owned()],
                challenge_ttl_ms: 60_000,
            })
        );
        let grants = bindings
            .iter()
            .find(|binding| {
                binding.slot() == IrohaRuntimeProviderSlotV1::EvidenceViewerGrantAuthority
            })
            .expect("grant binding");
        assert_eq!(grants.evidence_viewer_grant_ttl_ms(), Some(300_000));
        let receipt_signer = bindings
            .iter()
            .find(|binding| {
                binding.slot() == IrohaRuntimeProviderSlotV1::EvidenceViewerReceiptSigner
            })
            .expect("receipt-signer binding");
        assert_eq!(
            receipt_signer.evidence_viewer_receipt_signer_public_key(),
            Some(evidence_archive_public_key())
        );
        let checkpoint_store = bindings
            .iter()
            .find(|binding| {
                binding.slot() == IrohaRuntimeProviderSlotV1::EvidenceViewerCheckpointStore
            })
            .expect("checkpoint-store binding");

        assert_eq!(checkpoint_store.handle(), EVIDENCE_CHECKPOINT_HANDLE);
        assert_eq!(
            checkpoint_store.revision(),
            Some(EVIDENCE_CHECKPOINT_QUALIFICATION.revision())
        );
        assert_eq!(
            checkpoint_store.policy_digest(),
            Some(EVIDENCE_CHECKPOINT_QUALIFICATION.policy_digest())
        );
        assert_eq!(
            checkpoint_store.evidence_viewer_checkpoint_max_bytes(),
            Some(64 * 1024 * 1024)
        );
        let archive = bindings
            .iter()
            .find(|binding| {
                binding.slot() == IrohaRuntimeProviderSlotV1::EvidenceViewerCompactionArchive
            })
            .expect("compaction-archive binding");
        assert_eq!(archive.handle(), EVIDENCE_ARCHIVE_HANDLE);
        assert_eq!(
            archive.revision(),
            Some(EVIDENCE_ARCHIVE_QUALIFICATION.revision())
        );
        assert_eq!(
            archive.policy_digest(),
            Some(EVIDENCE_ARCHIVE_QUALIFICATION.policy_digest())
        );
        assert_eq!(
            archive.evidence_viewer_archive_id(),
            Some(EVIDENCE_ARCHIVE_ID)
        );
        assert_eq!(
            archive.evidence_viewer_archive_public_key(),
            Some(evidence_archive_public_key())
        );
        assert_eq!(
            archive.evidence_viewer_archive_max_bytes(),
            Some(64 * 1024 * 1024 + 16 * 1024)
        );
        let transparency_publisher = bindings
            .iter()
            .find(|binding| {
                binding.slot() == IrohaRuntimeProviderSlotV1::EvidenceViewerTransparencyPublisher
            })
            .expect("transparency-publisher binding");
        assert_eq!(
            transparency_publisher.handle(),
            EVIDENCE_TRANSPARENCY_PUBLISHER_HANDLE
        );
        assert_eq!(
            transparency_publisher.revision(),
            Some(EVIDENCE_TRANSPARENCY_PUBLISHER_QUALIFICATION.revision())
        );
        assert_eq!(
            transparency_publisher.policy_digest(),
            Some(EVIDENCE_TRANSPARENCY_PUBLISHER_QUALIFICATION.policy_digest())
        );
        assert_eq!(
            transparency_publisher.evidence_viewer_transparency_publisher_public_key(),
            Some(evidence_archive_public_key())
        );
    }

    #[test]
    fn evidence_viewer_catalog_rejects_test_marked_checkpoint_store() {
        let mut config = default_runtime_config();
        configure_evidence_viewer(&mut config);
        config
            .torii
            .sorafs_storage
            .evidence_viewer
            .as_mut()
            .expect("configured evidence viewer")
            .checkpoint_store_handle = "sealed://sorafs/evidence-viewer/test".to_owned();

        assert!(matches!(
            IrohaRuntimeProviderBindingsV1::try_from_config(&config),
            Err(IrohaRuntimeProviderRegistryErrorV1::InvalidBinding(
                IrohaRuntimeProviderSlotV1::EvidenceViewerCheckpointStore
            ))
        ));
    }

    #[test]
    fn evidence_viewer_checkpoint_store_resolution_is_exactly_scoped() {
        let binding = IrohaRuntimeProviderBindingV1::try_new(
            IrohaRuntimeProviderSlotV1::EvidenceViewerCheckpointStore,
            EVIDENCE_CHECKPOINT_HANDLE,
            Some(EVIDENCE_CHECKPOINT_QUALIFICATION.revision()),
            Some(EVIDENCE_CHECKPOINT_QUALIFICATION.policy_digest()),
        )
        .expect("valid evidence-viewer checkpoint-store binding");
        let requested = IrohaRuntimeProviderBindingsV1 {
            chain_id: "production-chain".to_owned(),
            bindings: vec![binding],
        };
        assert!(matches!(
            resolve_runtime_deps_from_bindings(&requested, Some(&EmptyRegistry)),
            Err(IrohaRuntimeProviderRegistryErrorV1::IncompleteResolution)
        ));

        let checkpoint_store = Arc::new(EvidenceViewerCheckpointStore);
        let registry = FixedRegistry(
            IrohaRuntimeDeps::default()
                .with_sorafs_evidence_viewer_checkpoint_store(checkpoint_store.clone()),
        );
        let resolved = resolve_runtime_deps_from_bindings(&requested, Some(&registry))
            .expect("resolve the requested checkpoint-store dependency");
        assert!(resolved.sorafs_evidence_viewer_checkpoint_store.is_some());

        let unrequested = IrohaRuntimeProviderBindingsV1 {
            chain_id: "production-chain".to_owned(),
            bindings: Vec::new(),
        };
        let registry = FixedRegistry(
            IrohaRuntimeDeps::default()
                .with_sorafs_evidence_viewer_checkpoint_store(checkpoint_store),
        );
        assert!(matches!(
            resolve_runtime_deps_from_bindings(&unrequested, Some(&registry)),
            Err(IrohaRuntimeProviderRegistryErrorV1::UnexpectedProviders)
        ));
    }

    #[test]
    fn evidence_viewer_archive_resolution_is_exactly_scoped_and_qualified() {
        let requested = evidence_archive_request();
        assert!(matches!(
            resolve_runtime_deps_from_bindings(&requested, Some(&EmptyRegistry)),
            Err(IrohaRuntimeProviderRegistryErrorV1::IncompleteResolution)
        ));

        let archive = Arc::new(EvidenceViewerCompactionArchive::exact());
        let registry = FixedRegistry(
            IrohaRuntimeDeps::default()
                .with_sorafs_evidence_viewer_compaction_archive(archive.clone()),
        );
        let resolved = resolve_runtime_deps_from_bindings(&requested, Some(&registry))
            .expect("resolve exact evidence-viewer compaction archive");
        assert!(resolved.sorafs_evidence_viewer_compaction_archive.is_some());

        let unrequested = IrohaRuntimeProviderBindingsV1 {
            chain_id: "production-chain".to_owned(),
            bindings: Vec::new(),
        };
        let registry = FixedRegistry(
            IrohaRuntimeDeps::default().with_sorafs_evidence_viewer_compaction_archive(archive),
        );
        assert!(matches!(
            resolve_runtime_deps_from_bindings(&unrequested, Some(&registry)),
            Err(IrohaRuntimeProviderRegistryErrorV1::UnexpectedProviders)
        ));
    }

    #[test]
    fn evidence_viewer_archive_resolution_rejects_substitution_staleness_and_drift() {
        let requested = evidence_archive_request();

        let mut substituted = EvidenceViewerCompactionArchive::exact();
        substituted.archive_id = [0xF1; 32];
        let registry = FixedRegistry(
            IrohaRuntimeDeps::default()
                .with_sorafs_evidence_viewer_compaction_archive(Arc::new(substituted)),
        );
        assert!(matches!(
            resolve_runtime_deps_from_bindings(&requested, Some(&registry)),
            Err(IrohaRuntimeProviderRegistryErrorV1::BindingMismatch)
        ));

        let mut stale = EvidenceViewerCompactionArchive::exact();
        stale.qualification_error = Some(
            sorafs_node::evidence_viewer::EvidenceViewerRuntimeProviderReadinessErrorV1::Rejected,
        );
        let registry = FixedRegistry(
            IrohaRuntimeDeps::default()
                .with_sorafs_evidence_viewer_compaction_archive(Arc::new(stale)),
        );
        assert!(matches!(
            resolve_runtime_deps_from_bindings(&requested, Some(&registry)),
            Err(IrohaRuntimeProviderRegistryErrorV1::StaleOrRevoked)
        ));

        let mut drifting = EvidenceViewerCompactionArchive::exact();
        drifting.later_qualification = Some(
            sorafs_node::evidence_viewer::EvidenceViewerRuntimeProviderQualificationV1::new(
                EVIDENCE_ARCHIVE_QUALIFICATION.revision() + 1,
                EVIDENCE_ARCHIVE_QUALIFICATION.policy_digest(),
            ),
        );
        let registry = FixedRegistry(
            IrohaRuntimeDeps::default()
                .with_sorafs_evidence_viewer_compaction_archive(Arc::new(drifting)),
        );
        assert!(matches!(
            resolve_runtime_deps_from_bindings(&requested, Some(&registry)),
            Err(IrohaRuntimeProviderRegistryErrorV1::BindingMismatch)
                | Err(IrohaRuntimeProviderRegistryErrorV1::StaleOrRevoked)
        ));
    }

    #[test]
    fn evidence_viewer_transparency_publisher_resolution_is_exact_and_fail_closed() {
        let requested = evidence_transparency_publisher_request();
        assert!(matches!(
            resolve_runtime_deps_from_bindings(&requested, Some(&EmptyRegistry)),
            Err(IrohaRuntimeProviderRegistryErrorV1::IncompleteResolution)
        ));

        let publisher = Arc::new(EvidenceViewerTransparencyPublisher::exact());
        let registry = FixedRegistry(
            IrohaRuntimeDeps::default()
                .with_sorafs_evidence_viewer_transparency_publisher(publisher.clone()),
        );
        let resolved = resolve_runtime_deps_from_bindings(&requested, Some(&registry))
            .expect("resolve exact evidence-viewer transparency publisher");
        assert!(
            resolved
                .sorafs_evidence_viewer_transparency_publisher
                .is_some()
        );

        let unrequested = IrohaRuntimeProviderBindingsV1 {
            chain_id: "production-chain".to_owned(),
            bindings: Vec::new(),
        };
        let registry = FixedRegistry(
            IrohaRuntimeDeps::default()
                .with_sorafs_evidence_viewer_transparency_publisher(publisher),
        );
        assert!(matches!(
            resolve_runtime_deps_from_bindings(&unrequested, Some(&registry)),
            Err(IrohaRuntimeProviderRegistryErrorV1::UnexpectedProviders)
        ));

        let mut substituted = EvidenceViewerTransparencyPublisher::exact();
        substituted.public_key = [0xF1; 32];
        let registry = FixedRegistry(
            IrohaRuntimeDeps::default()
                .with_sorafs_evidence_viewer_transparency_publisher(Arc::new(substituted)),
        );
        assert!(matches!(
            resolve_runtime_deps_from_bindings(&requested, Some(&registry)),
            Err(IrohaRuntimeProviderRegistryErrorV1::BindingMismatch)
        ));

        let mut stale = EvidenceViewerTransparencyPublisher::exact();
        stale.qualification_error = Some(
            sorafs_node::evidence_viewer::EvidenceViewerRuntimeProviderReadinessErrorV1::Rejected,
        );
        let registry = FixedRegistry(
            IrohaRuntimeDeps::default()
                .with_sorafs_evidence_viewer_transparency_publisher(Arc::new(stale)),
        );
        assert!(matches!(
            resolve_runtime_deps_from_bindings(&requested, Some(&registry)),
            Err(IrohaRuntimeProviderRegistryErrorV1::StaleOrRevoked)
        ));
    }

    #[test]
    fn transparency_leader_lease_catalog_and_resolution_are_exactly_scoped() {
        let mut config = default_runtime_config();
        config
            .torii
            .sorafs_storage
            .privacy_aggregates
            .leader_lease_provider = Some(
            iroha_config::parameters::actual::SorafsTransparencyRuntimeProviderBinding {
                handle: TRANSPARENCY_LEADER_LEASE_HANDLE.to_owned(),
                revision: TRANSPARENCY_LEADER_LEASE_QUALIFICATION.revision(),
                policy_digest: TRANSPARENCY_LEADER_LEASE_QUALIFICATION.policy_digest(),
            },
        );
        let projected = IrohaRuntimeProviderBindingsV1::try_from_config(&config)
            .expect("project transparency leader-lease binding");
        let binding = projected
            .iter()
            .find(|binding| binding.slot() == IrohaRuntimeProviderSlotV1::TransparencyLeaderLease)
            .expect("transparency leader-lease binding");
        assert_eq!(binding.handle(), TRANSPARENCY_LEADER_LEASE_HANDLE);
        assert_eq!(
            binding.revision(),
            Some(TRANSPARENCY_LEADER_LEASE_QUALIFICATION.revision())
        );
        assert_eq!(
            binding.policy_digest(),
            Some(TRANSPARENCY_LEADER_LEASE_QUALIFICATION.policy_digest())
        );

        let requested = IrohaRuntimeProviderBindingsV1 {
            chain_id: "production-chain".to_owned(),
            bindings: vec![binding.clone()],
        };
        assert!(matches!(
            resolve_runtime_deps_from_bindings(&requested, Some(&EmptyRegistry)),
            Err(IrohaRuntimeProviderRegistryErrorV1::IncompleteResolution)
        ));

        let provider: Arc<dyn sorafs_node::ProductionTransparencyLeaderLeaseProviderV1> =
            Arc::new(TransparencyLeaderLeaseProvider);
        let registry = FixedRegistry(
            IrohaRuntimeDeps::default()
                .with_transparency_leader_lease_provider(Arc::clone(&provider)),
        );
        let resolved = resolve_runtime_deps_from_bindings(&requested, Some(&registry))
            .expect("resolve requested transparency leader-lease dependency");
        assert!(resolved.transparency_leader_lease_provider.is_some());

        let unrequested = IrohaRuntimeProviderBindingsV1 {
            chain_id: "production-chain".to_owned(),
            bindings: Vec::new(),
        };
        let registry = FixedRegistry(
            IrohaRuntimeDeps::default().with_transparency_leader_lease_provider(provider),
        );
        assert!(matches!(
            resolve_runtime_deps_from_bindings(&unrequested, Some(&registry)),
            Err(IrohaRuntimeProviderRegistryErrorV1::UnexpectedProviders)
        ));
    }

    #[test]
    fn fenced_privacy_catalog_projects_two_exact_roles_from_one_binding() {
        let mut config = default_runtime_config();
        configure_fenced_privacy_runtime(&mut config);

        let projected = IrohaRuntimeProviderBindingsV1::try_from_config(&config)
            .expect("project fused privacy runtime bindings");
        let fenced = projected
            .iter()
            .filter(|binding| {
                matches!(
                    binding.slot(),
                    IrohaRuntimeProviderSlotV1::FencedPrivacyPublisher
                        | IrohaRuntimeProviderSlotV1::FencedPrivacyHeadReader
                )
            })
            .collect::<Vec<_>>();

        assert_eq!(fenced.len(), 2);
        assert_eq!(
            fenced
                .iter()
                .map(|binding| binding.slot())
                .collect::<Vec<_>>(),
            vec![
                IrohaRuntimeProviderSlotV1::FencedPrivacyPublisher,
                IrohaRuntimeProviderSlotV1::FencedPrivacyHeadReader,
            ]
        );
        for binding in fenced {
            assert_eq!(binding.handle(), FENCED_PRIVACY_HANDLE);
            assert_eq!(
                binding.revision(),
                Some(FENCED_PRIVACY_QUALIFICATION.revision)
            );
            assert_eq!(
                binding.policy_digest(),
                Some(FENCED_PRIVACY_QUALIFICATION.policy_digest)
            );
        }

        let mut test_marked = config;
        test_marked
            .torii
            .sorafs_storage
            .privacy_aggregates
            .fenced_privacy_publisher
            .as_mut()
            .expect("configured fused runtime")
            .handle = "governance-cas:transparency:privacy-test".to_owned();
        assert!(matches!(
            IrohaRuntimeProviderBindingsV1::try_from_config(&test_marked),
            Err(IrohaRuntimeProviderRegistryErrorV1::InvalidBinding(
                IrohaRuntimeProviderSlotV1::FencedPrivacyPublisher
            ))
        ));
    }

    #[test]
    fn fenced_privacy_resolution_requires_the_complete_role_pair() {
        let mut config = default_runtime_config();
        configure_fenced_privacy_runtime(&mut config);
        let mut requested = IrohaRuntimeProviderBindingsV1::try_from_config(&config)
            .expect("project fused privacy runtime bindings");
        requested.bindings.retain(|binding| {
            matches!(
                binding.slot(),
                IrohaRuntimeProviderSlotV1::FencedPrivacyPublisher
                    | IrohaRuntimeProviderSlotV1::FencedPrivacyHeadReader
            )
        });

        assert!(matches!(
            resolve_runtime_deps_from_bindings(&requested, None),
            Err(IrohaRuntimeProviderRegistryErrorV1::MissingRegistry)
        ));
        assert!(matches!(
            resolve_runtime_deps_from_bindings(&requested, Some(&EmptyRegistry)),
            Err(IrohaRuntimeProviderRegistryErrorV1::IncompleteResolution)
        ));
        for (writer, reader) in [(true, false), (false, true)] {
            let runtime = Arc::new(FencedPrivacyRuntime::exact());
            let registry = FixedRegistry(fenced_privacy_dependencies(
                writer.then(|| Arc::clone(&runtime)),
                reader.then(|| Arc::clone(&runtime)),
            ));
            assert!(matches!(
                resolve_runtime_deps_from_bindings(&requested, Some(&registry)),
                Err(IrohaRuntimeProviderRegistryErrorV1::IncompleteResolution)
            ));
        }

        let runtime = Arc::new(FencedPrivacyRuntime::exact());
        let registry = FixedRegistry(fenced_privacy_dependencies(
            Some(Arc::clone(&runtime)),
            Some(runtime),
        ));
        let resolved = resolve_runtime_deps_from_bindings(&requested, Some(&registry))
            .expect("resolve both exact fused privacy roles");
        assert!(resolved.sorafs_fenced_transparency_publisher.is_some());
        assert!(resolved.sorafs_fenced_transparency_head_reader.is_some());
        assert!(!resolved.is_empty());

        let mut incomplete_catalog = requested.clone();
        incomplete_catalog
            .bindings
            .retain(|binding| binding.slot() == IrohaRuntimeProviderSlotV1::FencedPrivacyPublisher);
        let runtime = Arc::new(FencedPrivacyRuntime::exact());
        let registry = FixedRegistry(fenced_privacy_dependencies(Some(runtime), None));
        assert!(matches!(
            resolve_runtime_deps_from_bindings(&incomplete_catalog, Some(&registry)),
            Err(IrohaRuntimeProviderRegistryErrorV1::IncompleteResolution)
        ));

        let mut conflicting_catalog = requested.clone();
        conflicting_catalog
            .bindings
            .iter_mut()
            .find(|binding| binding.slot() == IrohaRuntimeProviderSlotV1::FencedPrivacyHeadReader)
            .expect("authenticated head-reader binding")
            .policy_digest = Some([0x92; 32]);
        let runtime = Arc::new(FencedPrivacyRuntime::exact());
        let registry = FixedRegistry(fenced_privacy_dependencies(
            Some(Arc::clone(&runtime)),
            Some(runtime),
        ));
        assert!(matches!(
            resolve_runtime_deps_from_bindings(&conflicting_catalog, Some(&registry)),
            Err(IrohaRuntimeProviderRegistryErrorV1::BindingMismatch)
        ));

        let unrequested = IrohaRuntimeProviderBindingsV1 {
            chain_id: "production-chain".to_owned(),
            bindings: Vec::new(),
        };
        let runtime = Arc::new(FencedPrivacyRuntime::exact());
        let registry = FixedRegistry(fenced_privacy_dependencies(
            Some(Arc::clone(&runtime)),
            Some(runtime),
        ));
        assert!(matches!(
            resolve_runtime_deps_from_bindings(&unrequested, Some(&registry)),
            Err(IrohaRuntimeProviderRegistryErrorV1::UnexpectedProviders)
        ));
    }

    #[test]
    fn fenced_privacy_resolution_rejects_substituted_stale_and_test_marked_roles() {
        let mut config = default_runtime_config();
        configure_fenced_privacy_runtime(&mut config);
        let mut requested = IrohaRuntimeProviderBindingsV1::try_from_config(&config)
            .expect("project fused privacy runtime bindings");
        requested.bindings.retain(|binding| {
            matches!(
                binding.slot(),
                IrohaRuntimeProviderSlotV1::FencedPrivacyPublisher
                    | IrohaRuntimeProviderSlotV1::FencedPrivacyHeadReader
            )
        });
        let exact = Arc::new(FencedPrivacyRuntime::exact());

        let substituted = Arc::new(FencedPrivacyRuntime {
            handle: "governance-cas:transparency:privacy-secondary",
            qualification: Some(FENCED_PRIVACY_QUALIFICATION),
        });
        let registry = FixedRegistry(fenced_privacy_dependencies(
            Some(substituted),
            Some(Arc::clone(&exact)),
        ));
        assert!(matches!(
            resolve_runtime_deps_from_bindings(&requested, Some(&registry)),
            Err(IrohaRuntimeProviderRegistryErrorV1::BindingMismatch)
        ));

        let stale = Arc::new(FencedPrivacyRuntime {
            handle: FENCED_PRIVACY_HANDLE,
            qualification: Some(
                sorafs_node::GovernanceDagRuntimeProviderQualificationV1::new(
                    FENCED_PRIVACY_QUALIFICATION.revision + 1,
                    FENCED_PRIVACY_QUALIFICATION.policy_digest,
                ),
            ),
        });
        let registry = FixedRegistry(fenced_privacy_dependencies(
            Some(Arc::clone(&exact)),
            Some(stale),
        ));
        assert!(matches!(
            resolve_runtime_deps_from_bindings(&requested, Some(&registry)),
            Err(IrohaRuntimeProviderRegistryErrorV1::StaleOrRevoked)
        ));

        let test_marked = Arc::new(FencedPrivacyRuntime {
            handle: "governance-cas:transparency:privacy-test",
            qualification: Some(FENCED_PRIVACY_QUALIFICATION),
        });
        let registry = FixedRegistry(fenced_privacy_dependencies(
            Some(test_marked),
            Some(Arc::clone(&exact)),
        ));
        assert!(matches!(
            resolve_runtime_deps_from_bindings(&requested, Some(&registry)),
            Err(IrohaRuntimeProviderRegistryErrorV1::TestProviderRejected)
        ));

        let unavailable = Arc::new(FencedPrivacyRuntime {
            handle: FENCED_PRIVACY_HANDLE,
            qualification: None,
        });
        let registry = FixedRegistry(fenced_privacy_dependencies(
            Some(Arc::clone(&exact)),
            Some(unavailable),
        ));
        assert!(matches!(
            resolve_runtime_deps_from_bindings(&requested, Some(&registry)),
            Err(IrohaRuntimeProviderRegistryErrorV1::Unavailable)
        ));
    }

    #[test]
    fn provider_ingest_catalog_projects_independent_source_and_resolver_qualifications() {
        let mut config = default_runtime_config();
        configure_provider_ingest_runtime(&mut config);

        let bindings = IrohaRuntimeProviderBindingsV1::try_from_config(&config)
            .expect("project provider-ingest provider bindings");
        let source = bindings
            .iter()
            .find(|binding| {
                binding.slot() == IrohaRuntimeProviderSlotV1::ProviderIngestAuthenticatedSource
            })
            .expect("source-pool binding");
        assert_eq!(
            source.handle(),
            "network://sorafs/provider-ingest/source-primary"
        );
        assert_eq!(source.revision(), Some(5));
        assert_eq!(source.policy_digest(), Some([0xB1; 32]));
        assert_eq!(
            source.provider_ingest_source_limits(),
            Some(ProviderIngestSourceLimitsV1 {
                operation_timeout_ms: 30_000,
                max_content_bytes: config.torii.sorafs_storage.max_capacity_bytes.0,
                max_source_providers: 1_024,
                max_concurrent_streams: u32::try_from(
                    config.torii.sorafs_storage.max_parallel_fetches
                )
                .expect("configured parallel-fetch bound fits u32"),
            })
        );

        let resolver = bindings
            .iter()
            .find(|binding| {
                binding.slot() == IrohaRuntimeProviderSlotV1::ProviderIngestCompletionSignerResolver
            })
            .expect("signer-resolver binding");
        assert_eq!(
            resolver.handle(),
            "hsm://sorafs/provider-ingest/resolver-primary"
        );
        assert_eq!(resolver.revision(), Some(6));
        assert_eq!(resolver.policy_digest(), Some([0xB2; 32]));
        assert_ne!(source.revision(), resolver.revision());
        assert_ne!(source.policy_digest(), resolver.policy_digest());

        let signer = bindings
            .iter()
            .find(|binding| {
                binding.slot() == IrohaRuntimeProviderSlotV1::ProviderIngestCompletionSigner
            })
            .expect("leaf completion-signer binding");
        assert_eq!(
            signer.handle(),
            "pkcs11://sorafs/provider-ingest/signer-primary"
        );
        assert_eq!(signer.revision(), Some(3));
        assert_eq!(signer.policy_digest(), Some([0xA2; 32]));
        assert_ne!(resolver.revision(), signer.revision());
        assert_ne!(resolver.policy_digest(), signer.policy_digest());
        assert_eq!(
            resolver.provider_ingest_signer_binding(),
            signer.provider_ingest_signer_binding(),
            "resolver and leaf roles pin one exact algorithm/key/policy binding"
        );
        let detailed = signer
            .provider_ingest_signer_binding()
            .expect("exact completion-signer binding");
        assert_eq!(detailed.qualification.adapter_revision, 3);
        assert_eq!(
            detailed.qualification.signer_policy.policy_digest,
            [0xA2; 32]
        );
        assert_eq!(
            signer.provider_ingest_max_signed_transaction_bytes(),
            Some(1024 * 1024)
        );
        let checkpoint = bindings
            .iter()
            .find(|binding| {
                binding.slot() == IrohaRuntimeProviderSlotV1::ProviderIngestCheckpointStore
            })
            .expect("checkpoint-store binding");
        assert_eq!(
            checkpoint.provider_ingest_checkpoint_max_bytes(),
            Some(160 * 1024 * 1024)
        );

        let mut excessive_streams = config;
        excessive_streams.torii.sorafs_storage.max_parallel_fetches =
            usize::try_from(MAX_PROVIDER_INGEST_SOURCE_STREAMS_V1)
                .expect("stream ceiling fits usize")
                + 1;
        assert!(matches!(
            IrohaRuntimeProviderBindingsV1::try_from_config(&excessive_streams),
            Err(IrohaRuntimeProviderRegistryErrorV1::InvalidBinding(
                IrohaRuntimeProviderSlotV1::ProviderIngestAuthenticatedSource
            ))
        ));
    }

    #[test]
    fn provider_ingest_catalog_accepts_enabled_default_outbox_capacity() {
        let mut config = default_runtime_config();
        configure_provider_ingest_runtime(&mut config);
        let ingest = config
            .torii
            .sorafs_storage
            .provider_ingest_runtime
            .as_mut()
            .expect("configured provider ingest");
        ingest.outbox = iroha_config::parameters::actual::SorafsProviderIngestOutbox {
            max_active_entries: provider_ingest_outbox_defaults::MAX_ACTIVE_ENTRIES,
            max_terminal_entries: provider_ingest_outbox_defaults::MAX_TERMINAL_ENTRIES,
            max_attempts: provider_ingest_outbox_defaults::MAX_ATTEMPTS,
            checkpoint_max_bytes: provider_ingest_outbox_defaults::CHECKPOINT_MAX_BYTES,
            checkpoint_operation_timeout_ms:
                provider_ingest_outbox_defaults::CHECKPOINT_OPERATION_TIMEOUT_MS,
            source_lease_ttl_ms: provider_ingest_outbox_defaults::SOURCE_LEASE_TTL_MS,
            retry_base_delay_ms: provider_ingest_outbox_defaults::RETRY_BASE_DELAY_MS,
            retry_max_delay_ms: provider_ingest_outbox_defaults::RETRY_MAX_DELAY_MS,
            terminal_retention_blocks: provider_ingest_outbox_defaults::TERMINAL_RETENTION_BLOCKS,
            max_signed_transaction_bytes:
                provider_ingest_outbox_defaults::MAX_SIGNED_TRANSACTION_BYTES,
            max_status_page_size: provider_ingest_outbox_defaults::MAX_STATUS_PAGE_SIZE,
        };

        let bindings = IrohaRuntimeProviderBindingsV1::try_from_config(&config)
            .expect("enabled default provider-ingest outbox must project");
        assert_eq!(
            bindings
                .iter()
                .find(|binding| {
                    binding.slot() == IrohaRuntimeProviderSlotV1::ProviderIngestCheckpointStore
                })
                .and_then(IrohaRuntimeProviderBindingV1::provider_ingest_checkpoint_max_bytes),
            Some(provider_ingest_outbox_defaults::CHECKPOINT_MAX_BYTES.0)
        );
        assert_eq!(
            bindings
                .iter()
                .find(|binding| {
                    binding.slot() == IrohaRuntimeProviderSlotV1::ProviderIngestCompletionSigner
                })
                .and_then(
                    IrohaRuntimeProviderBindingV1::provider_ingest_max_signed_transaction_bytes
                ),
            Some(provider_ingest_outbox_defaults::MAX_SIGNED_TRANSACTION_BYTES.0)
        );
    }

    #[test]
    fn provider_ingest_catalog_rejects_broker_incompatible_outbox_limits() {
        let mut exact = default_runtime_config();
        configure_provider_ingest_runtime(&mut exact);
        let exact_ingest = exact
            .torii
            .sorafs_storage
            .provider_ingest_runtime
            .as_mut()
            .expect("configured provider ingest");
        exact_ingest.max_source_jobs_per_tick = 1;
        let exact_outbox = &mut exact_ingest.outbox;
        exact_outbox.max_active_entries = 1;
        exact_outbox.max_terminal_entries = 1;
        exact_outbox.checkpoint_max_bytes =
            Bytes(provider_ingest_outbox_defaults::CHECKPOINT_MAX_BYTES_LIMIT);
        exact_outbox.max_signed_transaction_bytes =
            Bytes(provider_ingest_outbox_defaults::MAX_SIGNED_TRANSACTION_BYTES_LIMIT);
        let exact_bindings = IrohaRuntimeProviderBindingsV1::try_from_config(&exact)
            .expect("stock broker ceilings must project");
        assert_eq!(
            exact_bindings
                .iter()
                .find(|binding| {
                    binding.slot() == IrohaRuntimeProviderSlotV1::ProviderIngestCheckpointStore
                })
                .and_then(IrohaRuntimeProviderBindingV1::provider_ingest_checkpoint_max_bytes),
            Some(provider_ingest_outbox_defaults::CHECKPOINT_MAX_BYTES_LIMIT)
        );
        assert_eq!(
            exact_bindings
                .iter()
                .find(|binding| {
                    binding.slot() == IrohaRuntimeProviderSlotV1::ProviderIngestCompletionSigner
                })
                .and_then(
                    IrohaRuntimeProviderBindingV1::provider_ingest_max_signed_transaction_bytes
                ),
            Some(provider_ingest_outbox_defaults::MAX_SIGNED_TRANSACTION_BYTES_LIMIT)
        );

        let mut oversized_checkpoint = default_runtime_config();
        configure_provider_ingest_runtime(&mut oversized_checkpoint);
        oversized_checkpoint
            .torii
            .sorafs_storage
            .provider_ingest_runtime
            .as_mut()
            .expect("configured provider ingest")
            .outbox
            .checkpoint_max_bytes =
            Bytes(provider_ingest_outbox_defaults::CHECKPOINT_MAX_BYTES_LIMIT + 1);
        assert_eq!(
            IrohaRuntimeProviderBindingsV1::try_from_config(&oversized_checkpoint),
            Err(IrohaRuntimeProviderRegistryErrorV1::InvalidBinding(
                IrohaRuntimeProviderSlotV1::ProviderIngestCheckpointStore,
            ))
        );

        let mut oversized_transaction = default_runtime_config();
        configure_provider_ingest_runtime(&mut oversized_transaction);
        oversized_transaction
            .torii
            .sorafs_storage
            .provider_ingest_runtime
            .as_mut()
            .expect("configured provider ingest")
            .outbox
            .max_signed_transaction_bytes =
            Bytes(provider_ingest_outbox_defaults::MAX_SIGNED_TRANSACTION_BYTES_LIMIT + 1);
        assert_eq!(
            IrohaRuntimeProviderBindingsV1::try_from_config(&oversized_transaction),
            Err(IrohaRuntimeProviderRegistryErrorV1::InvalidBinding(
                IrohaRuntimeProviderSlotV1::ProviderIngestCompletionSignerResolver,
            ))
        );

        let mut legacy_128_mib_transaction = default_runtime_config();
        configure_provider_ingest_runtime(&mut legacy_128_mib_transaction);
        let legacy_ingest = legacy_128_mib_transaction
            .torii
            .sorafs_storage
            .provider_ingest_runtime
            .as_mut()
            .expect("configured provider ingest");
        legacy_ingest.max_source_jobs_per_tick = 1;
        legacy_ingest.outbox.max_active_entries = 1;
        legacy_ingest.outbox.max_terminal_entries = 1;
        legacy_ingest.outbox.checkpoint_max_bytes =
            Bytes(provider_ingest_outbox_defaults::CHECKPOINT_MAX_BYTES_LIMIT);
        legacy_ingest.outbox.max_signed_transaction_bytes = Bytes(128 * 1024 * 1024);
        assert_eq!(
            IrohaRuntimeProviderBindingsV1::try_from_config(&legacy_128_mib_transaction),
            Err(IrohaRuntimeProviderRegistryErrorV1::InvalidBinding(
                IrohaRuntimeProviderSlotV1::ProviderIngestCompletionSignerResolver,
            )),
            "the former 128 MiB transaction limit must not project under a 192 MiB checkpoint"
        );

        let mut below_envelope_reserve = default_runtime_config();
        configure_provider_ingest_runtime(&mut below_envelope_reserve);
        below_envelope_reserve
            .torii
            .sorafs_storage
            .provider_ingest_runtime
            .as_mut()
            .expect("configured provider ingest")
            .outbox
            .max_signed_transaction_bytes =
            Bytes(provider_ingest_outbox_defaults::MAX_SIGNED_TRANSACTION_BYTES_MIN - 1);
        assert_eq!(
            IrohaRuntimeProviderBindingsV1::try_from_config(&below_envelope_reserve),
            Err(IrohaRuntimeProviderRegistryErrorV1::InvalidBinding(
                IrohaRuntimeProviderSlotV1::ProviderIngestCompletionSignerResolver,
            ))
        );

        let mut exact_minimum = default_runtime_config();
        configure_provider_ingest_runtime(&mut exact_minimum);
        exact_minimum
            .torii
            .sorafs_storage
            .provider_ingest_runtime
            .as_mut()
            .expect("configured provider ingest")
            .outbox
            .max_signed_transaction_bytes =
            Bytes(provider_ingest_outbox_defaults::MAX_SIGNED_TRANSACTION_BYTES_MIN);
        IrohaRuntimeProviderBindingsV1::try_from_config(&exact_minimum)
            .expect("exact signed-transaction minimum must project");

        let mut impossible_aggregate = default_runtime_config();
        configure_provider_ingest_runtime(&mut impossible_aggregate);
        impossible_aggregate
            .torii
            .sorafs_storage
            .provider_ingest_runtime
            .as_mut()
            .expect("configured provider ingest")
            .outbox
            .max_active_entries = 128;
        assert_eq!(
            IrohaRuntimeProviderBindingsV1::try_from_config(&impossible_aggregate),
            Err(IrohaRuntimeProviderRegistryErrorV1::InvalidBinding(
                IrohaRuntimeProviderSlotV1::ProviderIngestCheckpointStore,
            )),
            "an aggregate outbox capacity above its checkpoint bound must fail projection"
        );
    }

    #[test]
    fn provider_ingest_catalog_projects_retention_authority_binding() {
        let mut config = default_runtime_config();
        configure_provider_ingest_runtime(&mut config);
        let ingest = config
            .torii
            .sorafs_storage
            .provider_ingest_runtime
            .as_mut()
            .expect("configured provider ingest");
        ingest.finalized_archive.retention_authority = Some(
            iroha_config::parameters::actual::
                SorafsProviderIngestFinalizedArchiveRetentionAuthority {
                    handle: "sealed://sorafs/provider-ingest/retention-primary".to_owned(),
                    revision: 9,
                    policy_digest: [0xC9; 32],
                },
        );
        let bindings = IrohaRuntimeProviderBindingsV1::try_from_config(&config)
            .expect("project provider-ingest retention binding");
        let retention = bindings
            .iter()
            .find(|binding| {
                binding.slot() == IrohaRuntimeProviderSlotV1::ProviderIngestRetentionAuthority
            })
            .expect("retention-authority binding");
        assert_eq!(
            retention.handle(),
            "sealed://sorafs/provider-ingest/retention-primary"
        );
        assert_eq!(retention.revision(), Some(9));
        assert_eq!(retention.policy_digest(), Some([0xC9; 32]));
    }

    #[test]
    fn reputation_catalog_projects_exact_retention_authority_binding() {
        let mut config = default_runtime_config();
        configure_reputation_runtime(&mut config);

        let bindings = IrohaRuntimeProviderBindingsV1::try_from_config(&config)
            .expect("project reputation retention binding");
        let retention = bindings
            .iter()
            .find(|binding| {
                binding.slot()
                    == IrohaRuntimeProviderSlotV1::ReputationFinalizedArchiveRetentionAuthority
            })
            .expect("reputation retention-authority binding");
        assert_eq!(
            retention.handle(),
            "sealed://sorafs/reputation/retention-primary"
        );
        assert_eq!(retention.revision(), Some(9));
        assert_eq!(retention.policy_digest(), Some([0xC9; 32]));
        for (slot, handle, revision, policy_digest) in [
            (
                IrohaRuntimeProviderSlotV1::ReputationJournalCheckpoint,
                "sealed://sorafs/reputation/journal-primary",
                1,
                [0x60; 32],
            ),
            (
                IrohaRuntimeProviderSlotV1::ReputationJournalTransactionSubmitter,
                "queue://sorafs/reputation/journal-primary",
                11,
                [0x61; 32],
            ),
            (
                IrohaRuntimeProviderSlotV1::ReputationThresholdSigner,
                "hsm://sorafs/reputation/threshold-primary",
                12,
                [0x62; 32],
            ),
            (
                IrohaRuntimeProviderSlotV1::ReputationGovernanceDag,
                "dag://sorafs/reputation/publisher-primary",
                13,
                [0x63; 32],
            ),
        ] {
            let binding = bindings
                .iter()
                .find(|binding| binding.slot() == slot)
                .expect("reputation runtime provider binding");
            assert_eq!(binding.handle(), handle);
            assert_eq!(binding.revision(), Some(revision));
            assert_eq!(binding.policy_digest(), Some(policy_digest));
        }

        let mut dormant = config;
        dormant
            .torii
            .sorafs_storage
            .reputation_runtime
            .as_mut()
            .expect("configured reputation runtime")
            .finalized_archive_retention_authority = None;
        assert!(
            IrohaRuntimeProviderBindingsV1::try_from_config(&dormant)
                .expect("project dormant reputation runtime")
                .iter()
                .all(|binding| {
                    binding.slot()
                        != IrohaRuntimeProviderSlotV1::ReputationFinalizedArchiveRetentionAuthority
                })
        );
    }

    #[test]
    fn reputation_catalog_rejects_zero_public_qualification_bindings() {
        for mutation in 0..8 {
            let mut config = default_runtime_config();
            configure_reputation_runtime(&mut config);
            let reputation = config
                .torii
                .sorafs_storage
                .reputation_runtime
                .as_mut()
                .expect("configured reputation runtime");
            match mutation {
                0 => reputation.journal_checkpoint_provider_revision = 0,
                1 => reputation.journal_checkpoint_provider_policy_digest = [0; 32],
                2 => reputation.journal_transaction_submitter_revision = 0,
                3 => reputation.journal_transaction_submitter_policy_digest = [0; 32],
                4 => reputation.threshold_signer_revision = 0,
                5 => reputation.threshold_signer_policy_digest = [0; 32],
                6 => reputation.governance_dag_revision = 0,
                7 => reputation.governance_dag_policy_digest = [0; 32],
                _ => unreachable!(),
            }
            assert!(matches!(
                IrohaRuntimeProviderBindingsV1::try_from_config(&config),
                Err(IrohaRuntimeProviderRegistryErrorV1::InvalidBinding(_))
            ));
        }
    }

    fn reputation_checkpoint_request() -> IrohaRuntimeProviderBindingsV1 {
        IrohaRuntimeProviderBindingsV1 {
            chain_id: "reputation-checkpoint-registry-test".to_owned(),
            bindings: vec![
                IrohaRuntimeProviderBindingV1::try_new(
                    IrohaRuntimeProviderSlotV1::ReputationJournalCheckpoint,
                    REPUTATION_CHECKPOINT_HANDLE,
                    Some(REPUTATION_CHECKPOINT_QUALIFICATION.revision()),
                    Some(REPUTATION_CHECKPOINT_QUALIFICATION.policy_digest()),
                )
                .expect("valid reputation checkpoint binding"),
            ],
        }
    }

    fn resolve_reputation_checkpoint(
        requested: &IrohaRuntimeProviderBindingsV1,
        provider: ReputationJournalCheckpointProvider,
    ) -> Result<IrohaRuntimeDeps, IrohaRuntimeProviderRegistryErrorV1> {
        let registry = FixedRegistry(
            IrohaRuntimeDeps::default()
                .with_sorafs_reputation_journal_checkpoint_provider(Arc::new(provider)),
        );
        resolve_runtime_deps_from_bindings(requested, Some(&registry))
    }

    #[test]
    fn reputation_checkpoint_resolution_is_exactly_scoped_and_qualified() {
        let requested = reputation_checkpoint_request();
        assert!(matches!(
            resolve_runtime_deps_from_bindings(&requested, Some(&EmptyRegistry)),
            Err(IrohaRuntimeProviderRegistryErrorV1::IncompleteResolution)
        ));
        assert!(
            resolve_reputation_checkpoint(
                &requested,
                ReputationJournalCheckpointProvider::exact(),
            )
            .is_ok()
        );

        let unrequested = IrohaRuntimeProviderBindingsV1 {
            chain_id: "reputation-checkpoint-registry-test".to_owned(),
            bindings: Vec::new(),
        };
        assert!(matches!(
            resolve_reputation_checkpoint(
                &unrequested,
                ReputationJournalCheckpointProvider::exact(),
            ),
            Err(IrohaRuntimeProviderRegistryErrorV1::UnexpectedProviders)
        ));

        let mut substituted = ReputationJournalCheckpointProvider::exact();
        substituted.handle = "sealed://sorafs/reputation/substituted-checkpoint";
        assert!(matches!(
            resolve_reputation_checkpoint(&requested, substituted),
            Err(IrohaRuntimeProviderRegistryErrorV1::BindingMismatch)
        ));

        let mut stale = ReputationJournalCheckpointProvider::exact();
        stale.first =
            sorafs_node::reputation::runtime::ReputationRuntimeProviderQualificationV1::new(
                REPUTATION_CHECKPOINT_QUALIFICATION.revision(),
                [0xA5; 32],
            );
        assert!(matches!(
            resolve_reputation_checkpoint(&requested, stale),
            Err(IrohaRuntimeProviderRegistryErrorV1::BindingMismatch)
        ));

        let mut drifting = ReputationJournalCheckpointProvider::exact();
        drifting.later = Some(
            sorafs_node::reputation::runtime::ReputationRuntimeProviderQualificationV1::new(
                REPUTATION_CHECKPOINT_QUALIFICATION.revision(),
                [0xA6; 32],
            ),
        );
        assert!(matches!(
            resolve_reputation_checkpoint(&requested, drifting),
            Err(IrohaRuntimeProviderRegistryErrorV1::StaleOrRevoked)
        ));

        let mut unavailable = ReputationJournalCheckpointProvider::exact();
        unavailable.load_error = Some(
            sorafs_node::reputation::runtime::
                ReputationJournalCheckpointExternalErrorV1::Unavailable,
        );
        assert!(matches!(
            resolve_reputation_checkpoint(&requested, unavailable),
            Err(IrohaRuntimeProviderRegistryErrorV1::Unavailable)
        ));

        let mut ambiguous = ReputationJournalCheckpointProvider::exact();
        ambiguous.load_error = Some(
            sorafs_node::reputation::runtime::ReputationJournalCheckpointExternalErrorV1::Ambiguous,
        );
        assert!(matches!(
            resolve_reputation_checkpoint(&requested, ambiguous),
            Err(IrohaRuntimeProviderRegistryErrorV1::StaleOrRevoked)
        ));
    }

    #[test]
    fn reputation_checkpoint_binding_rejects_non_v1_profile_and_test_handle() {
        for mutation in 0..2 {
            let mut config = default_runtime_config();
            configure_reputation_runtime(&mut config);
            let reputation = config
                .torii
                .sorafs_storage
                .reputation_runtime
                .as_mut()
                .expect("configured reputation runtime");
            if mutation == 0 {
                reputation.journal_checkpoint_provider_revision = 2;
            } else {
                reputation.journal_checkpoint_provider_handle =
                    "sealed://sorafs/reputation/test-checkpoint".to_owned();
            }
            assert_eq!(
                IrohaRuntimeProviderBindingsV1::try_from_config(&config),
                Err(IrohaRuntimeProviderRegistryErrorV1::InvalidBinding(
                    IrohaRuntimeProviderSlotV1::ReputationJournalCheckpoint,
                ))
            );
        }
    }

    #[test]
    fn reputation_retention_projection_rejects_test_marked_and_stale_bindings() {
        for mutation in 0..3 {
            let mut config = default_runtime_config();
            configure_reputation_runtime(&mut config);
            let retention = config
                .torii
                .sorafs_storage
                .reputation_runtime
                .as_mut()
                .expect("configured reputation runtime")
                .finalized_archive_retention_authority
                .as_mut()
                .expect("configured reputation retention authority");
            match mutation {
                0 => {
                    retention.handle = "sealed://sorafs/reputation/test-retention".to_owned();
                }
                1 => retention.revision = 0,
                2 => retention.policy_digest = [0; 32],
                _ => unreachable!(),
            }
            assert_eq!(
                IrohaRuntimeProviderBindingsV1::try_from_config(&config),
                Err(IrohaRuntimeProviderRegistryErrorV1::InvalidBinding(
                    IrohaRuntimeProviderSlotV1::ReputationFinalizedArchiveRetentionAuthority,
                ))
            );
        }
    }

    #[test]
    fn governance_service_catalog_projects_only_exact_public_provider_bindings() {
        let mut config = default_runtime_config();
        configure_governance_service(&mut config, "signed_http");

        let bindings = IrohaRuntimeProviderBindingsV1::try_from_config(&config)
            .expect("project Governance DAG service provider bindings");
        let expected = [
            (
                IrohaRuntimeProviderSlotV1::GovernanceDagIpfsAuthenticator,
                GOVERNANCE_IPFS_HANDLE,
            ),
            (
                IrohaRuntimeProviderSlotV1::GovernanceDagHeadAuthenticator,
                GOVERNANCE_HEAD_HANDLE,
            ),
            (
                IrohaRuntimeProviderSlotV1::GovernanceDagCheckpointStore,
                GOVERNANCE_CHECKPOINT_HANDLE,
            ),
        ];
        for (slot, handle) in expected {
            let binding = bindings
                .iter()
                .find(|binding| binding.slot() == slot)
                .expect("projected Governance DAG service role");
            assert_eq!(binding.handle(), handle);
            assert_eq!(binding.revision(), Some(GOVERNANCE_QUALIFICATION.revision));
            assert_eq!(
                binding.policy_digest(),
                Some(GOVERNANCE_QUALIFICATION.policy_digest)
            );
            if matches!(
                slot,
                IrohaRuntimeProviderSlotV1::GovernanceDagIpfsAuthenticator
                    | IrohaRuntimeProviderSlotV1::GovernanceDagHeadAuthenticator
            ) {
                assert_eq!(
                    binding.governance_request_auth_public_key(),
                    Some(governance_auth_public_key(handle))
                );
                assert_eq!(
                    binding.governance_request_auth_max_body_bytes(),
                    Some(
                        config
                            .torii
                            .sorafs_storage
                            .governance_dag_service
                            .max_request_bytes
                            .0
                    )
                );
            } else {
                assert_eq!(binding.governance_request_auth_public_key(), None);
                assert_eq!(binding.governance_request_auth_max_body_bytes(), None);
            }
        }
    }

    #[test]
    fn standalone_governance_service_projection_is_exact_and_mode_scoped() {
        let chain_id = iroha_data_model::ChainId::from("governance-service-projection");
        let signed_head =
            IrohaRuntimeProviderBindingsV1::try_from_governance_dag_service_projection(
                &chain_id,
                governance_service_projection(true),
            )
            .expect("project signed-head standalone service bindings");
        assert_eq!(signed_head.chain_id(), chain_id.to_string());
        assert_eq!(
            signed_head
                .iter()
                .map(IrohaRuntimeProviderBindingV1::slot)
                .collect::<Vec<_>>(),
            vec![
                IrohaRuntimeProviderSlotV1::GovernanceDagIpfsAuthenticator,
                IrohaRuntimeProviderSlotV1::GovernanceDagHeadAuthenticator,
                IrohaRuntimeProviderSlotV1::GovernanceDagCheckpointStore,
            ]
        );
        for binding in signed_head.iter().take(2) {
            assert_eq!(
                binding.governance_request_auth_max_body_bytes(),
                Some(65_536)
            );
            assert!(binding.governance_request_auth_public_key().is_some());
        }
        assert!(
            signed_head.iter().all(|binding| {
                binding.slot() != IrohaRuntimeProviderSlotV1::GovernanceDagSigner
            })
        );

        let ipns = IrohaRuntimeProviderBindingsV1::try_from_governance_dag_service_projection(
            &chain_id,
            governance_service_projection(false),
        )
        .expect("project IPNS standalone service bindings");
        assert_eq!(
            ipns.iter()
                .map(IrohaRuntimeProviderBindingV1::slot)
                .collect::<Vec<_>>(),
            vec![
                IrohaRuntimeProviderSlotV1::GovernanceDagIpfsAuthenticator,
                IrohaRuntimeProviderSlotV1::GovernanceDagCheckpointStore,
            ]
        );
    }

    #[test]
    fn standalone_governance_service_projection_rejects_invalid_public_bounds() {
        let chain_id = iroha_data_model::ChainId::from("governance-service-projection");
        let mut projection = governance_service_projection(true);
        projection.request_auth_max_body_bytes = 0;
        assert_eq!(
            IrohaRuntimeProviderBindingsV1::try_from_governance_dag_service_projection(
                &chain_id, projection,
            ),
            Err(IrohaRuntimeProviderRegistryErrorV1::InvalidBinding(
                IrohaRuntimeProviderSlotV1::GovernanceDagIpfsAuthenticator,
            ))
        );
    }

    #[test]
    fn governance_producer_catalog_projects_store_while_public_service_is_disabled() {
        let mut config = default_runtime_config();
        configure_governance_producer(&mut config);

        let bindings = IrohaRuntimeProviderBindingsV1::try_from_config(&config)
            .expect("project signed local Governance DAG producer bindings");
        let signer = bindings
            .iter()
            .find(|binding| binding.slot() == IrohaRuntimeProviderSlotV1::GovernanceDagSigner)
            .expect("producer signer binding");
        assert_eq!(signer.handle(), GOVERNANCE_SIGNER_HANDLE);
        assert_eq!(signer.revision(), Some(GOVERNANCE_QUALIFICATION.revision));
        assert_eq!(
            signer.policy_digest(),
            Some(GOVERNANCE_QUALIFICATION.policy_digest)
        );
        assert_eq!(
            signer.governance_dag_publisher_peer_id(),
            Some(GOVERNANCE_PUBLISHER_PEER_ID.as_bytes())
        );
        assert_eq!(
            signer.governance_dag_publisher_public_key(),
            Some(governance_signer_public_key(0x73))
        );
        let checkpoint_store = bindings
            .iter()
            .find(|binding| {
                binding.slot() == IrohaRuntimeProviderSlotV1::GovernanceDagCheckpointStore
            })
            .expect("producer checkpoint-store binding");
        assert_eq!(checkpoint_store.handle(), GOVERNANCE_CHECKPOINT_HANDLE);
        assert_eq!(
            checkpoint_store.revision(),
            Some(GOVERNANCE_QUALIFICATION.revision)
        );
        assert_eq!(
            checkpoint_store.policy_digest(),
            Some(GOVERNANCE_QUALIFICATION.policy_digest)
        );
        assert!(bindings.iter().all(|binding| {
            !matches!(
                binding.slot(),
                IrohaRuntimeProviderSlotV1::GovernanceDagIpfsAuthenticator
                    | IrohaRuntimeProviderSlotV1::GovernanceDagHeadAuthenticator
            )
        }));
    }

    #[test]
    fn governance_signer_resolution_rejects_substitution_staleness_and_drift() {
        let mut config = default_runtime_config();
        configure_governance_producer(&mut config);
        let mut requested = IrohaRuntimeProviderBindingsV1::try_from_config(&config)
            .expect("project Governance DAG signer binding");
        requested
            .bindings
            .retain(|binding| binding.slot() == IrohaRuntimeProviderSlotV1::GovernanceDagSigner);
        assert_eq!(requested.bindings.len(), 1);

        let resolve = |signer: GovernanceSigner| {
            let registry = FixedRegistry(
                IrohaRuntimeDeps::default().with_sorafs_governance_dag_signer(Arc::new(signer)),
            );
            resolve_runtime_deps_from_bindings(&requested, Some(&registry))
        };

        let resolved =
            resolve(GovernanceSigner::exact()).expect("exact Governance DAG signer must resolve");
        assert!(resolved.sorafs_governance_dag_signer.is_some());

        let mut substituted_peer = GovernanceSigner::exact();
        substituted_peer.publisher_peer_id = b"12D3KooWGovernanceProducerSubstitute".to_vec();
        assert!(matches!(
            resolve(substituted_peer),
            Err(IrohaRuntimeProviderRegistryErrorV1::BindingMismatch)
        ));

        let mut substituted_key = GovernanceSigner::exact();
        substituted_key.key_pair = governance_signer_keypair(0x74);
        assert!(matches!(
            resolve(substituted_key),
            Err(IrohaRuntimeProviderRegistryErrorV1::BindingMismatch)
        ));

        let mut lying_key = GovernanceSigner::exact();
        lying_key.signing_key_pair = Some(governance_signer_keypair(0x74));
        assert!(matches!(
            resolve(lying_key),
            Err(IrohaRuntimeProviderRegistryErrorV1::BindingMismatch)
        ));

        let mut failed_sign = GovernanceSigner::exact();
        failed_sign.sign_error = true;
        assert!(matches!(
            resolve(failed_sign),
            Err(IrohaRuntimeProviderRegistryErrorV1::Unavailable)
        ));

        let mut failed_sign_with_drift = GovernanceSigner::exact();
        failed_sign_with_drift.sign_error = true;
        failed_sign_with_drift.later_public_key = Some(governance_signer_public_key(0x75));
        assert!(matches!(
            resolve(failed_sign_with_drift),
            Err(IrohaRuntimeProviderRegistryErrorV1::StaleOrRevoked)
        ));

        let mut substituted_handle = GovernanceSigner::exact();
        substituted_handle.handle = "hsm://governance/producer-signer-secondary";
        assert!(matches!(
            resolve(substituted_handle),
            Err(IrohaRuntimeProviderRegistryErrorV1::BindingMismatch)
        ));

        let mut stale = GovernanceSigner::exact();
        stale.first_qualification = sorafs_node::GovernanceDagRuntimeProviderQualificationV1::new(
            GOVERNANCE_QUALIFICATION.revision + 1,
            GOVERNANCE_QUALIFICATION.policy_digest,
        );
        assert!(matches!(
            resolve(stale),
            Err(IrohaRuntimeProviderRegistryErrorV1::StaleOrRevoked)
        ));

        let mut qualification_drift = GovernanceSigner::exact();
        qualification_drift.later_qualification = Some(
            sorafs_node::GovernanceDagRuntimeProviderQualificationV1::new(
                GOVERNANCE_QUALIFICATION.revision + 1,
                GOVERNANCE_QUALIFICATION.policy_digest,
            ),
        );
        assert!(matches!(
            resolve(qualification_drift),
            Err(IrohaRuntimeProviderRegistryErrorV1::StaleOrRevoked)
        ));

        let mut handle_drift = GovernanceSigner::exact();
        handle_drift.later_handle = Some("hsm://governance/producer-signer-secondary");
        assert!(matches!(
            resolve(handle_drift),
            Err(IrohaRuntimeProviderRegistryErrorV1::StaleOrRevoked)
        ));

        let mut peer_drift = GovernanceSigner::exact();
        peer_drift.later_publisher_peer_id = Some(b"12D3KooWGovernanceProducerRotated".to_vec());
        assert!(matches!(
            resolve(peer_drift),
            Err(IrohaRuntimeProviderRegistryErrorV1::StaleOrRevoked)
        ));

        let mut key_drift = GovernanceSigner::exact();
        key_drift.later_public_key = Some(governance_signer_public_key(0x75));
        assert!(matches!(
            resolve(key_drift),
            Err(IrohaRuntimeProviderRegistryErrorV1::StaleOrRevoked)
        ));
    }

    #[test]
    fn governance_signer_catalog_rejects_unqualified_manual_actual_config() {
        let mut config = default_runtime_config();
        configure_governance_producer(&mut config);
        let storage = &mut config.torii.sorafs_storage;
        storage.governance_dag_signer_revision = None;
        storage.governance_dag_signer_policy_digest = None;

        assert_eq!(
            IrohaRuntimeProviderBindingsV1::try_from_config(&config),
            Err(IrohaRuntimeProviderRegistryErrorV1::InvalidBinding(
                IrohaRuntimeProviderSlotV1::GovernanceDagSigner,
            ))
        );

        let mut missing_peer = default_runtime_config();
        configure_governance_producer(&mut missing_peer);
        missing_peer
            .torii
            .sorafs_storage
            .governance_dag_publisher_peer_id = None;
        assert_eq!(
            IrohaRuntimeProviderBindingsV1::try_from_config(&missing_peer),
            Err(IrohaRuntimeProviderRegistryErrorV1::InvalidBinding(
                IrohaRuntimeProviderSlotV1::GovernanceDagSigner,
            ))
        );

        let mut invalid_key = default_runtime_config();
        configure_governance_producer(&mut invalid_key);
        invalid_key
            .torii
            .sorafs_storage
            .governance_dag_publisher_public_key_hex = Some("00".repeat(32));
        assert_eq!(
            IrohaRuntimeProviderBindingsV1::try_from_config(&invalid_key),
            Err(IrohaRuntimeProviderRegistryErrorV1::InvalidBinding(
                IrohaRuntimeProviderSlotV1::GovernanceDagSigner,
            ))
        );

        let mut missing_directory = default_runtime_config();
        configure_governance_producer(&mut missing_directory);
        missing_directory.torii.sorafs_storage.governance_dag_dir = None;
        assert_eq!(
            IrohaRuntimeProviderBindingsV1::try_from_config(&missing_directory),
            Err(IrohaRuntimeProviderRegistryErrorV1::InvalidBinding(
                IrohaRuntimeProviderSlotV1::GovernanceDagSigner,
            ))
        );

        let mut disabled_storage = default_runtime_config();
        configure_governance_producer(&mut disabled_storage);
        disabled_storage.torii.sorafs_storage.enabled = false;
        assert_eq!(
            IrohaRuntimeProviderBindingsV1::try_from_config(&disabled_storage),
            Err(IrohaRuntimeProviderRegistryErrorV1::InvalidBinding(
                IrohaRuntimeProviderSlotV1::GovernanceDagSigner,
            ))
        );
    }

    #[test]
    fn governance_producer_catalog_rejects_missing_partial_and_dormant_store_bindings() {
        for (label, handle, revision, policy_digest) in [
            ("missing", None, None, None),
            (
                "handle only",
                Some(GOVERNANCE_CHECKPOINT_HANDLE.to_owned()),
                None,
                None,
            ),
            (
                "missing policy",
                Some(GOVERNANCE_CHECKPOINT_HANDLE.to_owned()),
                Some(GOVERNANCE_QUALIFICATION.revision),
                None,
            ),
        ] {
            let mut config = default_runtime_config();
            configure_governance_producer(&mut config);
            let service = &mut config.torii.sorafs_storage.governance_dag_service;
            service.checkpoint_store_handle = handle;
            service.checkpoint_store_revision = revision;
            service.checkpoint_store_policy_digest = policy_digest;
            assert!(
                matches!(
                    IrohaRuntimeProviderBindingsV1::try_from_config(&config),
                    Err(IrohaRuntimeProviderRegistryErrorV1::InvalidBinding(
                        IrohaRuntimeProviderSlotV1::GovernanceDagCheckpointStore
                    ))
                ),
                "{label} producer store binding must fail"
            );
        }

        let mut dormant = default_runtime_config();
        let service = &mut dormant.torii.sorafs_storage.governance_dag_service;
        service.checkpoint_store_handle = Some(GOVERNANCE_CHECKPOINT_HANDLE.to_owned());
        service.checkpoint_store_revision = Some(GOVERNANCE_QUALIFICATION.revision);
        service.checkpoint_store_policy_digest = Some(GOVERNANCE_QUALIFICATION.policy_digest);
        assert!(matches!(
            IrohaRuntimeProviderBindingsV1::try_from_config(&dormant),
            Err(IrohaRuntimeProviderRegistryErrorV1::InvalidBinding(
                IrohaRuntimeProviderSlotV1::GovernanceDagCheckpointStore
            ))
        ));
    }

    #[test]
    fn governance_producer_catalog_rejects_disabled_service_authentication_bindings() {
        let mut config = default_runtime_config();
        configure_governance_producer(&mut config);
        let service = &mut config.torii.sorafs_storage.governance_dag_service;
        service.ipfs_authenticator_handle = Some(GOVERNANCE_IPFS_HANDLE.to_owned());
        service.ipfs_authenticator_revision = Some(GOVERNANCE_QUALIFICATION.revision);
        service.ipfs_authenticator_policy_digest = Some(GOVERNANCE_QUALIFICATION.policy_digest);

        assert!(matches!(
            IrohaRuntimeProviderBindingsV1::try_from_config(&config),
            Err(IrohaRuntimeProviderRegistryErrorV1::InvalidBinding(
                IrohaRuntimeProviderSlotV1::GovernanceDagIpfsAuthenticator
            ))
        ));
    }

    #[test]
    fn governance_service_catalog_rejects_incomplete_or_test_marked_bindings() {
        let mut incomplete = default_runtime_config();
        incomplete
            .torii
            .sorafs_storage
            .governance_dag_service
            .enabled = true;
        assert!(matches!(
            IrohaRuntimeProviderBindingsV1::try_from_config(&incomplete),
            Err(IrohaRuntimeProviderRegistryErrorV1::InvalidBinding(
                IrohaRuntimeProviderSlotV1::GovernanceDagIpfsAuthenticator
            ))
        ));

        let mut test_marked = default_runtime_config();
        configure_governance_service(&mut test_marked, "signed_http");
        test_marked
            .torii
            .sorafs_storage
            .governance_dag_service
            .checkpoint_store_handle = Some("kms://governance/checkpoint-test".to_owned());
        assert!(matches!(
            IrohaRuntimeProviderBindingsV1::try_from_config(&test_marked),
            Err(IrohaRuntimeProviderRegistryErrorV1::InvalidBinding(
                IrohaRuntimeProviderSlotV1::GovernanceDagCheckpointStore
            ))
        ));
    }

    #[test]
    fn governance_service_resolution_rejects_missing_and_unrequested_adapters() {
        let mut signed_http = default_runtime_config();
        configure_governance_service(&mut signed_http, "signed_http");
        let mut signed_http_bindings =
            IrohaRuntimeProviderBindingsV1::try_from_config(&signed_http)
                .expect("project signed-head Governance DAG bindings");
        signed_http_bindings.bindings.retain(|binding| {
            matches!(
                binding.slot(),
                IrohaRuntimeProviderSlotV1::GovernanceDagIpfsAuthenticator
                    | IrohaRuntimeProviderSlotV1::GovernanceDagHeadAuthenticator
                    | IrohaRuntimeProviderSlotV1::GovernanceDagCheckpointStore
            )
        });

        let missing_head = FixedRegistry(governance_service_dependencies(false));
        assert!(matches!(
            resolve_runtime_deps_from_bindings(&signed_http_bindings, Some(&missing_head)),
            Err(IrohaRuntimeProviderRegistryErrorV1::IncompleteResolution)
        ));
        let complete = FixedRegistry(governance_service_dependencies(true));
        resolve_runtime_deps_from_bindings(&signed_http_bindings, Some(&complete))
            .expect("resolve the complete signed-head adapter set");

        let mut substituted_dependencies = governance_service_dependencies(true);
        substituted_dependencies.sorafs_governance_dag_ipfs_authenticator = Some(Arc::new(
            GovernanceAuthenticator::new(GOVERNANCE_IPFS_HANDLE)
                .with_key_from(GOVERNANCE_HEAD_HANDLE),
        ));
        let substituted = FixedRegistry(substituted_dependencies);
        assert!(matches!(
            resolve_runtime_deps_from_bindings(&signed_http_bindings, Some(&substituted)),
            Err(IrohaRuntimeProviderRegistryErrorV1::BindingMismatch)
        ));

        let mut ipns = default_runtime_config();
        configure_governance_service(&mut ipns, "ipns");
        let mut ipns_bindings = IrohaRuntimeProviderBindingsV1::try_from_config(&ipns)
            .expect("project IPNS Governance DAG bindings");
        ipns_bindings.bindings.retain(|binding| {
            matches!(
                binding.slot(),
                IrohaRuntimeProviderSlotV1::GovernanceDagIpfsAuthenticator
                    | IrohaRuntimeProviderSlotV1::GovernanceDagHeadAuthenticator
                    | IrohaRuntimeProviderSlotV1::GovernanceDagCheckpointStore
            )
        });
        assert!(
            ipns_bindings.iter().all(|binding| binding.slot()
                != IrohaRuntimeProviderSlotV1::GovernanceDagHeadAuthenticator)
        );
        let unexpected_head = FixedRegistry(governance_service_dependencies(true));
        assert!(matches!(
            resolve_runtime_deps_from_bindings(&ipns_bindings, Some(&unexpected_head)),
            Err(IrohaRuntimeProviderRegistryErrorV1::UnexpectedProviders)
        ));
    }

    #[test]
    fn native_signer_config_projection_preserves_every_public_identity_field() {
        let proof = ProofOutcomeTestSigner::new();
        let repair = RepairTestSigner::new();
        let reserve = ReserveTestSigner::new();
        let orderbook = OrderbookTestSigner::new();
        let expected = [
            (
                IrohaRuntimeProviderSlotV1::ProofOutcomeTransactionSigner,
                proof.expected_binding(),
            ),
            (
                IrohaRuntimeProviderSlotV1::RepairTransactionSigner,
                repair.expected_binding(),
            ),
            (
                IrohaRuntimeProviderSlotV1::ReserveTransactionSigner,
                reserve.expected_binding(),
            ),
            (
                IrohaRuntimeProviderSlotV1::OrderbookTransactionSigner,
                orderbook.expected_binding(),
            ),
        ];
        let mut config = default_runtime_config();
        let configured = &mut config.torii.sorafs_storage.native_transaction_signers;
        configured.proof_outcome = Some(actual_native_signer_binding(&expected[0].1));
        configured.repair = Some(actual_native_signer_binding(&expected[1].1));
        configured.reserve = Some(actual_native_signer_binding(&expected[2].1));
        configured.orderbook = Some(actual_native_signer_binding(&expected[3].1));

        let projected = IrohaRuntimeProviderBindingsV1::try_from_config(&config)
            .expect("project exact native signer bindings");

        for (slot, exact) in expected {
            let binding = projected
                .iter()
                .find(|binding| binding.slot() == slot)
                .expect("projected native signer role");
            assert_eq!(binding.handle(), exact.handle());
            assert_eq!(binding.revision(), Some(exact.qualification().revision()));
            assert_eq!(
                binding.policy_digest(),
                Some(exact.qualification().policy_digest())
            );
            assert_eq!(binding.native_signer_binding(), Some(&exact));
            assert_eq!(
                binding.native_signer_algorithm(),
                exact.public_key().try_algorithm().ok()
            );
        }
    }

    #[test]
    fn native_signer_config_projection_rejects_algorithm_and_authority_substitution() {
        let provider = ProofOutcomeTestSigner::new();
        let exact = provider.expected_binding();
        let mut config = default_runtime_config();
        let mut substituted = actual_native_signer_binding(&exact);
        substituted.algorithm = iroha_crypto::Algorithm::Secp256k1;
        config
            .torii
            .sorafs_storage
            .native_transaction_signers
            .proof_outcome = Some(substituted);
        assert!(matches!(
            IrohaRuntimeProviderBindingsV1::try_from_config(&config),
            Err(IrohaRuntimeProviderRegistryErrorV1::InvalidBinding(
                IrohaRuntimeProviderSlotV1::ProofOutcomeTransactionSigner
            ))
        ));

        let other =
            iroha_crypto::KeyPair::try_from_seed(vec![0xA1; 32], iroha_crypto::Algorithm::Ed25519)
                .expect("derive substituted authority");
        let mut substituted = actual_native_signer_binding(&exact);
        substituted.authority =
            iroha_data_model::account::AccountId::new(other.public_key().clone());
        config
            .torii
            .sorafs_storage
            .native_transaction_signers
            .proof_outcome = Some(substituted);
        assert!(matches!(
            IrohaRuntimeProviderBindingsV1::try_from_config(&config),
            Err(IrohaRuntimeProviderRegistryErrorV1::InvalidBinding(
                IrohaRuntimeProviderSlotV1::ProofOutcomeTransactionSigner
            ))
        ));
    }

    #[test]
    fn native_signer_slot_rejects_role_confusion_in_public_binding() {
        let proof = ProofOutcomeTestSigner::new();
        assert!(matches!(
            IrohaRuntimeProviderBindingV1::try_new_native_signer(
                IrohaRuntimeProviderSlotV1::RepairTransactionSigner,
                proof.expected_binding(),
            ),
            Err(IrohaRuntimeProviderRegistryErrorV1::InvalidBinding(
                IrohaRuntimeProviderSlotV1::RepairTransactionSigner
            ))
        ));
    }

    #[test]
    fn registry_qualifies_all_four_native_signers_before_forwarding() {
        let proof = Arc::new(ProofOutcomeTestSigner::new());
        let repair = Arc::new(RepairTestSigner::new());
        let reserve = Arc::new(ReserveTestSigner::new());
        let orderbook = Arc::new(OrderbookTestSigner::new());
        let expected = [
            (
                IrohaRuntimeProviderSlotV1::ProofOutcomeTransactionSigner,
                proof.expected_binding(),
            ),
            (
                IrohaRuntimeProviderSlotV1::RepairTransactionSigner,
                repair.expected_binding(),
            ),
            (
                IrohaRuntimeProviderSlotV1::ReserveTransactionSigner,
                reserve.expected_binding(),
            ),
            (
                IrohaRuntimeProviderSlotV1::OrderbookTransactionSigner,
                orderbook.expected_binding(),
            ),
        ];
        let bindings = native_signer_catalog(expected.clone());
        let registry = FixedRegistry(
            IrohaRuntimeDeps::default()
                .with_sorafs_proof_outcome_signer(proof)
                .with_sorafs_repair_transaction_signer(repair)
                .with_sorafs_reserve_transaction_signer(reserve)
                .with_sorafs_orderbook_transaction_signer(orderbook),
        );

        let resolved = resolve_runtime_deps_from_bindings(&bindings, Some(&registry))
            .expect("qualify all native signers");
        let observed = [
            observed_native_signer_binding(
                resolved
                    .sorafs_proof_outcome_signer
                    .as_ref()
                    .expect("qualified proof signer")
                    .as_ref(),
            ),
            observed_native_signer_binding(
                resolved
                    .sorafs_repair_transaction_signer
                    .as_ref()
                    .expect("qualified repair signer")
                    .as_ref(),
            ),
            observed_native_signer_binding(
                resolved
                    .sorafs_reserve_transaction_signer
                    .as_ref()
                    .expect("qualified reserve signer")
                    .as_ref(),
            ),
            observed_native_signer_binding(
                resolved
                    .sorafs_orderbook_transaction_signer
                    .as_ref()
                    .expect("qualified orderbook signer")
                    .as_ref(),
            ),
        ];
        assert_eq!(
            observed,
            expected.map(|(_, binding)| binding),
            "qualified facades must expose only their immutable config bindings"
        );
    }

    #[test]
    fn registry_rejects_missing_role_confused_substituted_and_stale_native_signers() {
        let good = Arc::new(ProofOutcomeTestSigner::new());
        let exact = good.expected_binding();
        let exact_catalog = native_signer_catalog([(
            IrohaRuntimeProviderSlotV1::ProofOutcomeTransactionSigner,
            exact.clone(),
        )]);
        assert!(matches!(
            resolve_runtime_deps_from_bindings(&exact_catalog, Some(&EmptyRegistry)),
            Err(IrohaRuntimeProviderRegistryErrorV1::IncompleteResolution)
        ));

        let confused = Arc::new(RoleConfusedProofOutcomeSigner(ProofOutcomeTestSigner::new()));
        let confused_registry =
            FixedRegistry(IrohaRuntimeDeps::default().with_sorafs_proof_outcome_signer(confused));
        assert!(matches!(
            resolve_runtime_deps_from_bindings(&exact_catalog, Some(&confused_registry)),
            Err(IrohaRuntimeProviderRegistryErrorV1::BindingMismatch)
        ));

        let substituted = iroha_torii::SorafsNativeTransactionSignerBindingV1::try_new(
            iroha_torii::SorafsNativeTransactionSignerRoleV1::ProofOutcome,
            "hsm://registry/proof-outcome/secondary",
            exact.authority().clone(),
            exact.public_key().clone(),
            exact.qualification(),
        )
        .expect("valid substituted config binding");
        let substituted_catalog = native_signer_catalog([(
            IrohaRuntimeProviderSlotV1::ProofOutcomeTransactionSigner,
            substituted,
        )]);
        let good_registry = FixedRegistry(
            IrohaRuntimeDeps::default().with_sorafs_proof_outcome_signer(good.clone()),
        );
        assert!(matches!(
            resolve_runtime_deps_from_bindings(&substituted_catalog, Some(&good_registry)),
            Err(IrohaRuntimeProviderRegistryErrorV1::BindingMismatch)
        ));

        let stale = iroha_torii::SorafsNativeTransactionSignerBindingV1::try_new(
            iroha_torii::SorafsNativeTransactionSignerRoleV1::ProofOutcome,
            exact.handle(),
            exact.authority().clone(),
            exact.public_key().clone(),
            iroha_torii::SorafsNativeTransactionSignerQualificationV1::new(
                exact.qualification().revision() + 1,
                exact.qualification().policy_digest(),
            ),
        )
        .expect("valid stale config binding");
        let stale_catalog = native_signer_catalog([(
            IrohaRuntimeProviderSlotV1::ProofOutcomeTransactionSigner,
            stale,
        )]);
        let good_registry =
            FixedRegistry(IrohaRuntimeDeps::default().with_sorafs_proof_outcome_signer(good));
        assert!(matches!(
            resolve_runtime_deps_from_bindings(&stale_catalog, Some(&good_registry)),
            Err(IrohaRuntimeProviderRegistryErrorV1::StaleOrRevoked)
        ));
    }

    #[test]
    fn unrequested_native_signers_are_rejected_individually() {
        let proof_provider = Arc::new(ProofOutcomeTestSigner::new());
        let proof_binding = proof_provider.expected_binding();
        let proof_signer = iroha_torii::qualify_sorafs_proof_outcome_transaction_signer_v1(
            proof_binding,
            proof_provider,
        )
        .expect("qualify proof-outcome test signer");
        let repair_provider = Arc::new(RepairTestSigner::new());
        let repair_binding = repair_provider.expected_binding();
        let repair_signer = iroha_torii::qualify_sorafs_repair_transaction_signer_v1(
            repair_binding,
            repair_provider,
        )
        .expect("qualify repair test signer");
        let reserve_provider = Arc::new(ReserveTestSigner::new());
        let reserve_binding = reserve_provider.expected_binding();
        let reserve_signer = iroha_torii::qualify_sorafs_reserve_transaction_signer_v1(
            reserve_binding,
            reserve_provider,
        )
        .expect("qualify reserve test signer");
        let orderbook_provider = Arc::new(OrderbookTestSigner::new());
        let orderbook_binding = orderbook_provider.expected_binding();
        let orderbook_signer = iroha_torii::qualify_sorafs_orderbook_transaction_signer_v1(
            orderbook_binding,
            orderbook_provider,
        )
        .expect("qualify orderbook test signer");
        let unrequested_dependencies = [
            IrohaRuntimeDeps::default().with_sorafs_proof_outcome_signer(proof_signer),
            IrohaRuntimeDeps::default().with_sorafs_repair_transaction_signer(repair_signer),
            IrohaRuntimeDeps::default().with_sorafs_reserve_transaction_signer(reserve_signer),
            IrohaRuntimeDeps::default().with_sorafs_orderbook_transaction_signer(orderbook_signer),
        ];
        let empty_bindings = IrohaRuntimeProviderBindingsV1 {
            chain_id: "production-chain".to_owned(),
            bindings: Vec::new(),
        };

        for dependencies in unrequested_dependencies {
            let registry = FixedRegistry(dependencies);
            assert!(matches!(
                resolve_runtime_deps_from_bindings(&empty_bindings, Some(&registry)),
                Err(IrohaRuntimeProviderRegistryErrorV1::UnexpectedProviders)
            ));
        }
    }
}
