//! Purpose-separated adapters for detached SoraFS runtime signing roles.

use std::{fmt, sync::Arc};

use iroha_crypto::Algorithm;
use sorafs_node::pop_credentials::PopIssuerSigner as _;

use super::{
    adapter::ExternalSoftwareSignerAdapterErrorV1,
    protocol::{
        SoftwareSignerPublicBindingV1, SoftwareSignerPurposeBindingV1, SoftwareSignerRoleV1,
        digest_parts,
    },
    typed_payload::{SoftwareSignerPurposeV1, encode_typed_signing_payload},
    unix::{ExternalSoftwareSignerClientErrorV1, SoftwareSignerClientV1},
};

const DETACHED_OPERATION_ID_DOMAIN_V1: &[u8] = b"iroha.external-signer.detached-operation.v1";
const DETACHED_MESSAGE_DIGEST_DOMAIN_V1: &[u8] = b"iroha.external-signer.detached-message.v1";
const REDACTED_SIGNER_FAILURE_V1: &str = "external software signer unavailable";

#[derive(Clone)]
struct DetachedSignerClientV1 {
    client: SoftwareSignerClientV1,
    binding: SoftwareSignerPublicBindingV1,
}

impl fmt::Debug for DetachedSignerClientV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("DetachedSignerClientV1")
            .field("handle", &self.binding.handle)
            .field("role", &self.binding.role)
            .finish_non_exhaustive()
    }
}

impl DetachedSignerClientV1 {
    fn try_new(
        client: SoftwareSignerClientV1,
        expected_role: SoftwareSignerRoleV1,
    ) -> Result<Self, ExternalSoftwareSignerAdapterErrorV1> {
        let binding = client.expected_binding().clone();
        binding
            .validate()
            .map_err(|()| ExternalSoftwareSignerAdapterErrorV1::BindingMismatch)?;
        if binding.role != expected_role || !binding.role.allows_algorithm(binding.key_algorithm) {
            return Err(ExternalSoftwareSignerAdapterErrorV1::RoleMismatch);
        }
        let first = client.qualify().map_err(map_client_error)?;
        let second = client.qualify().map_err(map_client_error)?;
        // Each probe verifies its own attestation. Compare the complete signed
        // live state, not randomized ML-DSA signature bytes.
        if !first.has_same_stable_state(&second) || first.binding != binding || first.revoked {
            return Err(ExternalSoftwareSignerAdapterErrorV1::QualificationChanged);
        }
        Ok(Self { client, binding })
    }

    fn binding(&self) -> &SoftwareSignerPublicBindingV1 {
        &self.binding
    }

    fn qualify_live(&self) -> Result<(), ExternalSoftwareSignerAdapterErrorV1> {
        let provenance = self.client.qualify().map_err(map_client_error)?;
        if provenance.binding != self.binding || provenance.revoked {
            return Err(ExternalSoftwareSignerAdapterErrorV1::QualificationChanged);
        }
        Ok(())
    }

    fn sign(
        &self,
        purpose: SoftwareSignerPurposeV1,
        message: &[u8],
    ) -> Result<Vec<u8>, ExternalSoftwareSignerAdapterErrorV1> {
        self.qualify_live()?;
        let payload = encode_typed_signing_payload(self.binding.role, purpose, message)
            .map_err(|()| ExternalSoftwareSignerAdapterErrorV1::Refused)?;
        let binding_digest = self
            .binding
            .digest()
            .map_err(|()| ExternalSoftwareSignerAdapterErrorV1::BindingMismatch)?;
        let purpose_id = [purpose.wire_id()];
        let key_revision = self.binding.key_revision.to_be_bytes();
        let policy_revision = self.binding.policy_revision.to_be_bytes();
        let message_digest = digest_parts(DETACHED_MESSAGE_DIGEST_DOMAIN_V1, &[message]);
        let operation_id = digest_parts(
            DETACHED_OPERATION_ID_DOMAIN_V1,
            &[
                &binding_digest,
                &purpose_id,
                &key_revision,
                &policy_revision,
                &self.binding.policy_digest,
                &message_digest,
            ],
        );
        let receipt = self
            .client
            .sign(operation_id, &payload)
            .map_err(map_client_error)?;
        if receipt.provenance.binding != self.binding || receipt.provenance.revoked {
            return Err(ExternalSoftwareSignerAdapterErrorV1::QualificationChanged);
        }
        self.qualify_live()?;
        Ok(receipt.signature)
    }

    fn ed25519_public_key(&self) -> Result<[u8; 32], ExternalSoftwareSignerAdapterErrorV1> {
        let (algorithm, bytes) = self
            .binding
            .public_key
            .try_to_bytes()
            .map_err(|_| ExternalSoftwareSignerAdapterErrorV1::BindingMismatch)?;
        if algorithm != Algorithm::Ed25519 {
            return Err(ExternalSoftwareSignerAdapterErrorV1::BindingMismatch);
        }
        bytes
            .try_into()
            .map_err(|_| ExternalSoftwareSignerAdapterErrorV1::BindingMismatch)
    }

    fn public_key_bytes(&self) -> Result<Vec<u8>, ExternalSoftwareSignerAdapterErrorV1> {
        self.binding
            .public_key
            .try_to_bytes()
            .map(|(_, bytes)| bytes.to_vec())
            .map_err(|_| ExternalSoftwareSignerAdapterErrorV1::BindingMismatch)
    }

    fn sign_ed25519(
        &self,
        purpose: SoftwareSignerPurposeV1,
        message: &[u8],
    ) -> Result<[u8; 64], ExternalSoftwareSignerAdapterErrorV1> {
        self.sign(purpose, message)?
            .try_into()
            .map_err(|_| ExternalSoftwareSignerAdapterErrorV1::Refused)
    }
}

/// External software signer for the embedded Governance DAG publisher.
#[derive(Clone, Debug)]
pub struct ExternalSoftwareSignerGovernanceDagAdapterV1 {
    signer: DetachedSignerClientV1,
    publisher_peer_id: Vec<u8>,
}

impl ExternalSoftwareSignerGovernanceDagAdapterV1 {
    /// Construct an exact Governance DAG signer binding.
    pub fn try_new(
        client: SoftwareSignerClientV1,
        publisher_peer_id: Vec<u8>,
    ) -> Result<Self, ExternalSoftwareSignerAdapterErrorV1> {
        if publisher_peer_id.is_empty()
            || publisher_peer_id.len()
                > sorafs_manifest::GOVERNANCE_DAG_PUBLISHER_PEER_ID_MAX_BYTES_V1
        {
            return Err(ExternalSoftwareSignerAdapterErrorV1::BindingMismatch);
        }
        let signer = DetachedSignerClientV1::try_new(client, SoftwareSignerRoleV1::GovernanceDag)?;
        if signer.binding.purpose_binding
            != (SoftwareSignerPurposeBindingV1::GovernanceDag {
                publisher_peer_id: publisher_peer_id.clone(),
            })
        {
            return Err(ExternalSoftwareSignerAdapterErrorV1::BindingMismatch);
        }
        signer.ed25519_public_key()?;
        Ok(Self {
            signer,
            publisher_peer_id,
        })
    }

    /// Exact public software-signer binding.
    #[must_use]
    pub fn signer_binding(&self) -> &SoftwareSignerPublicBindingV1 {
        self.signer.binding()
    }
}

impl sorafs_node::GovernanceDagRuntimeSigner for ExternalSoftwareSignerGovernanceDagAdapterV1 {
    fn handle(&self) -> &str {
        &self.signer.binding.handle
    }

    fn qualification(
        &self,
    ) -> Result<sorafs_node::GovernanceDagRuntimeProviderQualificationV1, String> {
        self.signer
            .qualify_live()
            .map_err(|_| REDACTED_SIGNER_FAILURE_V1.to_owned())?;
        Ok(
            sorafs_node::GovernanceDagRuntimeProviderQualificationV1::new(
                self.signer.binding.policy_revision,
                self.signer.binding.policy_digest,
            ),
        )
    }

    fn publisher_peer_id(&self) -> &[u8] {
        &self.publisher_peer_id
    }

    fn public_key(&self) -> [u8; 32] {
        self.signer
            .ed25519_public_key()
            .expect("constructor pins one canonical Ed25519 public key")
    }

    fn sign(
        &self,
        purpose: sorafs_node::GovernanceDagSigningPurposeV1,
        payload: &[u8],
    ) -> Result<[u8; 64], String> {
        let public_key = self.public_key();
        let (purpose, valid) = match purpose {
            sorafs_node::GovernanceDagSigningPurposeV1::LogNode => (
                SoftwareSignerPurposeV1::GovernanceLogNode,
                sorafs_manifest::governance::
                    validate_governance_log_node_signing_payload_for_publisher_v1(
                        payload,
                        &self.publisher_peer_id,
                    )
                    .is_ok(),
            ),
            sorafs_node::GovernanceDagSigningPurposeV1::DagBlock => (
                SoftwareSignerPurposeV1::GovernanceDagBlock,
                sorafs_manifest::governance::
                    validate_governance_dag_block_signing_payload_for_publisher_v1(
                        payload,
                        &self.publisher_peer_id,
                        public_key,
                    )
                    .is_ok(),
            ),
            sorafs_node::GovernanceDagSigningPurposeV1::DagHead => (
                SoftwareSignerPurposeV1::GovernanceDagHead,
                sorafs_manifest::governance::
                    validate_governance_dag_head_signing_payload_for_publisher_v1(
                        payload,
                        &self.publisher_peer_id,
                )
                .is_ok(),
            ),
            sorafs_node::GovernanceDagSigningPurposeV1::KeyTransition => (
                SoftwareSignerPurposeV1::GovernanceKeyTransition,
                sorafs_node::validate_governance_dag_control_signing_payload_v1(
                    purpose,
                    payload,
                    &self.publisher_peer_id,
                    public_key,
                )
                .is_ok(),
            ),
            sorafs_node::GovernanceDagSigningPurposeV1::QualificationArchive => (
                SoftwareSignerPurposeV1::GovernanceQualificationArchive,
                sorafs_node::validate_governance_dag_control_signing_payload_v1(
                    purpose,
                    payload,
                    &self.publisher_peer_id,
                    public_key,
                )
                .is_ok(),
            ),
        };
        if !valid {
            return Err(REDACTED_SIGNER_FAILURE_V1.to_owned());
        }
        self.signer
            .sign_ed25519(purpose, payload)
            .map_err(|_| REDACTED_SIGNER_FAILURE_V1.to_owned())
    }
}

/// External software signer for gateway-side PoTR receipts.
#[derive(Clone, Debug)]
pub struct ExternalSoftwareSignerPotrGatewayAdapterV1 {
    signer: DetachedSignerClientV1,
    signer_id: [u8; 32],
}

impl ExternalSoftwareSignerPotrGatewayAdapterV1 {
    /// Construct one exact gateway signer.
    pub fn try_new(
        client: SoftwareSignerClientV1,
        signer_id: [u8; 32],
    ) -> Result<Self, ExternalSoftwareSignerAdapterErrorV1> {
        if signer_id == [0; 32] {
            return Err(ExternalSoftwareSignerAdapterErrorV1::BindingMismatch);
        }
        let signer = DetachedSignerClientV1::try_new(client, SoftwareSignerRoleV1::PotrGateway)?;
        if signer.binding.purpose_binding
            != (SoftwareSignerPurposeBindingV1::PotrGateway { signer_id })
        {
            return Err(ExternalSoftwareSignerAdapterErrorV1::BindingMismatch);
        }
        signer.ed25519_public_key()?;
        Ok(Self { signer, signer_id })
    }

    /// Exact public software-signer binding.
    #[must_use]
    pub fn signer_binding(&self) -> &SoftwareSignerPublicBindingV1 {
        self.signer.binding()
    }
}

impl iroha_torii::sorafs::PotrGatewaySignerV1 for ExternalSoftwareSignerPotrGatewayAdapterV1 {
    fn handle(&self) -> &str {
        &self.signer.binding.handle
    }

    fn signer_id(&self) -> [u8; 32] {
        self.signer_id
    }

    fn qualification(
        &self,
    ) -> Result<
        iroha_torii::sorafs::PotrRuntimeProviderQualificationV1,
        iroha_torii::sorafs::PotrSignerServiceError,
    > {
        self.signer.qualify_live().map_err(map_potr_error)?;
        Ok(
            iroha_torii::sorafs::PotrRuntimeProviderQualificationV1::new(
                self.signer.binding.policy_revision,
                self.signer.binding.policy_digest,
            ),
        )
    }

    fn public_key(&self) -> Result<[u8; 32], iroha_torii::sorafs::PotrSignerServiceError> {
        self.signer.ed25519_public_key().map_err(map_potr_error)
    }

    fn sign(&self, payload: &[u8]) -> Result<Vec<u8>, iroha_torii::sorafs::PotrSignerServiceError> {
        self.signer
            .sign(SoftwareSignerPurposeV1::PotrGatewayReceipt, payload)
            .map_err(map_potr_error)
    }
}

/// External software signer for provider-side PoTR receipts.
#[derive(Clone, Debug)]
pub struct ExternalSoftwareSignerPotrProviderAdapterV1 {
    signer: DetachedSignerClientV1,
    signer_id: [u8; 32],
    provider_id: [u8; 32],
}

impl ExternalSoftwareSignerPotrProviderAdapterV1 {
    /// Construct one exact provider signer.
    pub fn try_new(
        client: SoftwareSignerClientV1,
        signer_id: [u8; 32],
        provider_id: [u8; 32],
    ) -> Result<Self, ExternalSoftwareSignerAdapterErrorV1> {
        if signer_id == [0; 32] || provider_id == [0; 32] || signer_id == provider_id {
            return Err(ExternalSoftwareSignerAdapterErrorV1::BindingMismatch);
        }
        let signer = DetachedSignerClientV1::try_new(client, SoftwareSignerRoleV1::PotrProvider)?;
        if signer.binding.purpose_binding
            != (SoftwareSignerPurposeBindingV1::PotrProvider {
                signer_id,
                provider_id,
            })
        {
            return Err(ExternalSoftwareSignerAdapterErrorV1::BindingMismatch);
        }
        if signer.binding.key_algorithm != super::protocol::SoftwareSignerKeyAlgorithmV1::MlDsa {
            return Err(ExternalSoftwareSignerAdapterErrorV1::BindingMismatch);
        }
        Ok(Self {
            signer,
            signer_id,
            provider_id,
        })
    }

    /// Exact public software-signer binding.
    #[must_use]
    pub fn signer_binding(&self) -> &SoftwareSignerPublicBindingV1 {
        self.signer.binding()
    }
}

impl iroha_torii::sorafs::PotrProviderSignerV1 for ExternalSoftwareSignerPotrProviderAdapterV1 {
    fn handle(&self) -> &str {
        &self.signer.binding.handle
    }

    fn signer_id(&self) -> [u8; 32] {
        self.signer_id
    }

    fn qualification(
        &self,
    ) -> Result<
        iroha_torii::sorafs::PotrRuntimeProviderQualificationV1,
        iroha_torii::sorafs::PotrSignerServiceError,
    > {
        self.signer.qualify_live().map_err(map_potr_error)?;
        Ok(
            iroha_torii::sorafs::PotrRuntimeProviderQualificationV1::new(
                self.signer.binding.policy_revision,
                self.signer.binding.policy_digest,
            ),
        )
    }

    fn provider_id(&self) -> Result<[u8; 32], iroha_torii::sorafs::PotrSignerServiceError> {
        self.signer.qualify_live().map_err(map_potr_error)?;
        Ok(self.provider_id)
    }

    fn public_key(&self) -> Result<Vec<u8>, iroha_torii::sorafs::PotrSignerServiceError> {
        self.signer.public_key_bytes().map_err(map_potr_error)
    }

    fn sign(&self, payload: &[u8]) -> Result<Vec<u8>, iroha_torii::sorafs::PotrSignerServiceError> {
        self.signer
            .sign(SoftwareSignerPurposeV1::PotrProviderReceipt, payload)
            .map_err(map_potr_error)
    }
}

/// External software signer for governed billing-statement digests.
#[derive(Clone, Debug)]
pub struct ExternalSoftwareSignerBillingStatementAdapterV1 {
    signer: DetachedSignerClientV1,
    signer_id: String,
}

impl ExternalSoftwareSignerBillingStatementAdapterV1 {
    /// Construct one exact billing statement signer.
    pub fn try_new(
        client: SoftwareSignerClientV1,
        signer_id: String,
    ) -> Result<Self, ExternalSoftwareSignerAdapterErrorV1> {
        if signer_id.is_empty()
            || signer_id.len()
                > sorafs_node::hedging_billing_service::BILLING_SIGNER_ID_MAX_BYTES_V1
            || signer_id.trim() != signer_id
            || signer_id.chars().any(char::is_control)
        {
            return Err(ExternalSoftwareSignerAdapterErrorV1::BindingMismatch);
        }
        let signer =
            DetachedSignerClientV1::try_new(client, SoftwareSignerRoleV1::BillingStatement)?;
        if signer.binding.purpose_binding
            != (SoftwareSignerPurposeBindingV1::BillingStatement {
                signer_id: signer_id.clone(),
            })
        {
            return Err(ExternalSoftwareSignerAdapterErrorV1::BindingMismatch);
        }
        signer.ed25519_public_key()?;
        Ok(Self { signer, signer_id })
    }

    /// Exact public software-signer binding.
    #[must_use]
    pub fn signer_binding(&self) -> &SoftwareSignerPublicBindingV1 {
        self.signer.binding()
    }
}

impl sorafs_node::hedging_billing_service::HedgingBillingRuntimeProviderV1
    for ExternalSoftwareSignerBillingStatementAdapterV1
{
    fn handle(&self) -> &str {
        &self.signer.binding.handle
    }

    fn qualification(
        &self,
    ) -> Result<
        sorafs_node::hedging_billing_service::HedgingBillingRuntimeProviderQualificationV1,
        sorafs_node::hedging_billing_service::HedgingBillingRuntimeProviderReadinessErrorV1,
    > {
        self.signer
            .qualify_live()
            .map_err(map_billing_readiness_error)?;
        Ok(
            sorafs_node::hedging_billing_service::HedgingBillingRuntimeProviderQualificationV1::new(
                self.signer.binding.policy_revision,
                self.signer.binding.policy_digest,
            ),
        )
    }
}

impl sorafs_node::hedging_billing_service::BillingStatementRuntimeSigner
    for ExternalSoftwareSignerBillingStatementAdapterV1
{
    fn identity(
        &self,
    ) -> Result<
        sorafs_node::hedging_billing_service::BillingStatementSignerIdentityV1,
        sorafs_node::hedging_billing_service::HedgingBillingExternalError,
    > {
        self.signer.qualify_live().map_err(map_billing_error)?;
        Ok(
            sorafs_node::hedging_billing_service::BillingStatementSignerIdentityV1 {
                provider_handle: self.signer.binding.handle.clone(),
                signer_id: self.signer_id.clone(),
                public_key: self
                    .signer
                    .ed25519_public_key()
                    .map_err(map_billing_error)?,
            },
        )
    }

    fn check_readiness(
        &self,
    ) -> Result<(), sorafs_node::hedging_billing_service::HedgingBillingExternalError> {
        self.signer.qualify_live().map_err(map_billing_error)
    }

    fn sign_digest(
        &self,
        digest: [u8; 32],
    ) -> Result<[u8; 64], sorafs_node::hedging_billing_service::HedgingBillingExternalError> {
        if digest == [0; 32] {
            return Err(
                sorafs_node::hedging_billing_service::HedgingBillingExternalError::Rejected,
            );
        }
        // The qualified billing producer owns semantic recomputation; this
        // adapter binds its exact non-zero digest to the sole billing purpose.
        self.signer
            .sign_ed25519(SoftwareSignerPurposeV1::BillingStatement, &digest)
            .map_err(map_billing_error)
    }
}

/// External software signer for all four evidence-viewer signing purposes.
#[derive(Clone, Debug)]
pub struct ExternalSoftwareSignerEvidenceViewerAdapterV1 {
    signer: DetachedSignerClientV1,
}

impl ExternalSoftwareSignerEvidenceViewerAdapterV1 {
    /// Construct one exact evidence-viewer signer.
    pub fn try_new(
        client: SoftwareSignerClientV1,
    ) -> Result<Self, ExternalSoftwareSignerAdapterErrorV1> {
        let signer = DetachedSignerClientV1::try_new(client, SoftwareSignerRoleV1::EvidenceViewer)?;
        if signer.binding.purpose_binding != SoftwareSignerPurposeBindingV1::EvidenceViewer {
            return Err(ExternalSoftwareSignerAdapterErrorV1::BindingMismatch);
        }
        signer.ed25519_public_key()?;
        Ok(Self { signer })
    }

    /// Exact public software-signer binding.
    #[must_use]
    pub fn signer_binding(&self) -> &SoftwareSignerPublicBindingV1 {
        self.signer.binding()
    }
}

impl sorafs_node::evidence_viewer::EvidenceViewerRuntimeProviderV1
    for ExternalSoftwareSignerEvidenceViewerAdapterV1
{
    fn handle(&self) -> &str {
        &self.signer.binding.handle
    }

    fn qualification(
        &self,
    ) -> Result<
        sorafs_node::evidence_viewer::EvidenceViewerRuntimeProviderQualificationV1,
        sorafs_node::evidence_viewer::EvidenceViewerRuntimeProviderReadinessErrorV1,
    > {
        self.signer
            .qualify_live()
            .map_err(map_evidence_readiness_error)?;
        Ok(
            sorafs_node::evidence_viewer::EvidenceViewerRuntimeProviderQualificationV1::new(
                self.signer.binding.policy_revision,
                self.signer.binding.policy_digest,
            ),
        )
    }
}

impl sorafs_node::evidence_viewer::EvidenceViewerReceiptSignerV1
    for ExternalSoftwareSignerEvidenceViewerAdapterV1
{
    fn public_key(&self) -> [u8; 32] {
        self.signer
            .ed25519_public_key()
            .expect("constructor pins one canonical Ed25519 public key")
    }

    fn sign(
        &self,
        purpose: sorafs_node::evidence_viewer::EvidenceViewerSigningPurposeV1,
        message: &[u8],
    ) -> Result<[u8; 64], sorafs_node::evidence_viewer::EvidenceViewerExternalErrorV1> {
        let external_purpose = match purpose {
            sorafs_node::evidence_viewer::EvidenceViewerSigningPurposeV1::Receipt => {
                SoftwareSignerPurposeV1::EvidenceReceipt
            }
            sorafs_node::evidence_viewer::EvidenceViewerSigningPurposeV1::CheckpointStoreRecord => {
                SoftwareSignerPurposeV1::EvidenceCheckpointStoreRecord
            }
            sorafs_node::evidence_viewer::EvidenceViewerSigningPurposeV1::CheckpointAnchor => {
                SoftwareSignerPurposeV1::EvidenceCheckpointAnchor
            }
            sorafs_node::evidence_viewer::EvidenceViewerSigningPurposeV1::CompactionArchive => {
                SoftwareSignerPurposeV1::EvidenceCompactionArchive
            }
        };
        self.signer
            .sign_ed25519(external_purpose, message)
            .map_err(map_evidence_error)
    }
}

/// External software signer for canonical stream-token bodies.
#[derive(Clone, Debug)]
pub struct ExternalSoftwareSignerStreamTokenAdapterV1 {
    signer: DetachedSignerClientV1,
}

impl ExternalSoftwareSignerStreamTokenAdapterV1 {
    /// Construct one exact stream-token signer.
    pub fn try_new(
        client: SoftwareSignerClientV1,
    ) -> Result<Self, ExternalSoftwareSignerAdapterErrorV1> {
        let signer = DetachedSignerClientV1::try_new(client, SoftwareSignerRoleV1::StreamToken)?;
        if signer.binding.purpose_binding != SoftwareSignerPurposeBindingV1::StreamToken {
            return Err(ExternalSoftwareSignerAdapterErrorV1::BindingMismatch);
        }
        signer.ed25519_public_key()?;
        Ok(Self { signer })
    }

    /// Exact public software-signer binding.
    #[must_use]
    pub fn signer_binding(&self) -> &SoftwareSignerPublicBindingV1 {
        self.signer.binding()
    }
}

impl iroha_torii::sorafs::StreamTokenRuntimeSigner for ExternalSoftwareSignerStreamTokenAdapterV1 {
    fn handle(&self) -> &str {
        &self.signer.binding.handle
    }

    fn public_key(&self) -> [u8; 32] {
        self.signer
            .ed25519_public_key()
            .expect("constructor pins one canonical Ed25519 public key")
    }

    fn qualification(
        &self,
    ) -> Result<
        iroha_torii::sorafs::StreamTokenRuntimeSignerQualificationV1,
        iroha_torii::sorafs::StreamTokenRuntimeSignerProbeErrorV1,
    > {
        self.signer.qualify_live().map_err(map_stream_probe_error)?;
        Ok(
            iroha_torii::sorafs::StreamTokenRuntimeSignerQualificationV1::new(
                self.signer.binding.policy_revision,
                self.signer.binding.policy_digest,
            ),
        )
    }

    fn sign(
        &self,
        signing_payload: &[u8],
    ) -> Result<[u8; 64], iroha_torii::sorafs::StreamTokenSigningError> {
        self.signer
            .sign_ed25519(SoftwareSignerPurposeV1::StreamToken, signing_payload)
            .map_err(map_stream_error)
    }
}

/// External software signer for PoP credential/root/revocation digests.
#[derive(Clone, Debug)]
pub struct ExternalSoftwareSignerPopIssuerAdapterV1 {
    signer: DetachedSignerClientV1,
}

impl ExternalSoftwareSignerPopIssuerAdapterV1 {
    /// Construct one exact PoP issuer signer.
    pub fn try_new(
        client: SoftwareSignerClientV1,
        issuer_id: String,
    ) -> Result<Self, ExternalSoftwareSignerAdapterErrorV1> {
        if issuer_id.is_empty() {
            return Err(ExternalSoftwareSignerAdapterErrorV1::BindingMismatch);
        }
        let signer = DetachedSignerClientV1::try_new(client, SoftwareSignerRoleV1::PopCredentials)?;
        if signer.binding.purpose_binding
            != (SoftwareSignerPurposeBindingV1::PopCredentials { issuer_id })
        {
            return Err(ExternalSoftwareSignerAdapterErrorV1::BindingMismatch);
        }
        signer.ed25519_public_key()?;
        Ok(Self { signer })
    }

    /// Exact public software-signer binding.
    #[must_use]
    pub fn signer_binding(&self) -> &SoftwareSignerPublicBindingV1 {
        self.signer.binding()
    }

    fn issuer_id(&self) -> &str {
        match &self.signer.binding.purpose_binding {
            SoftwareSignerPurposeBindingV1::PopCredentials { issuer_id } => issuer_id,
            _ => unreachable!("constructor pins the PoP purpose binding"),
        }
    }
}

impl sorafs_node::pop_credentials::PopIssuerSigner for ExternalSoftwareSignerPopIssuerAdapterV1 {
    fn key_id(&self) -> &str {
        &self.signer.binding.handle
    }

    fn public_key(&self) -> [u8; 32] {
        self.signer
            .ed25519_public_key()
            .expect("constructor pins one canonical Ed25519 public key")
    }

    fn sign_digest(
        &self,
        purpose: sorafs_node::pop_credentials::PopIssuerSigningPurposeV1,
        digest: [u8; 32],
    ) -> Result<[u8; 64], String> {
        if digest == [0; 32] {
            return Err(REDACTED_SIGNER_FAILURE_V1.to_owned());
        }
        let purpose = match purpose {
            sorafs_node::pop_credentials::PopIssuerSigningPurposeV1::Credential => {
                SoftwareSignerPurposeV1::PopCredential
            }
            sorafs_node::pop_credentials::PopIssuerSigningPurposeV1::CommitmentRoot => {
                SoftwareSignerPurposeV1::PopCommitmentRoot
            }
            sorafs_node::pop_credentials::PopIssuerSigningPurposeV1::RevocationList => {
                SoftwareSignerPurposeV1::PopRevocationList
            }
        };
        // The qualified PoP service owns semantic digest recomputation; the
        // signer still binds the exact purpose and 32-byte digest shape.
        self.signer
            .sign_ed25519(purpose, &digest)
            .map_err(|_| REDACTED_SIGNER_FAILURE_V1.to_owned())
    }
}

/// PoP registry decorator that replaces only the issuer signer.
pub struct ExternalSoftwareSignerPopRegistryV1 {
    base: Arc<dyn iroha_torii::sorafs::pop_api::PopCredentialRuntimeProviderRegistryV1>,
    issuer_signer: Arc<ExternalSoftwareSignerPopIssuerAdapterV1>,
}

impl fmt::Debug for ExternalSoftwareSignerPopRegistryV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ExternalSoftwareSignerPopRegistryV1")
            .field("handle", &self.base.handle())
            .field("issuer_signer_handle", &self.issuer_signer.key_id())
            .finish_non_exhaustive()
    }
}

impl ExternalSoftwareSignerPopRegistryV1 {
    /// Compose an existing coherent PoP registry with one isolated signer.
    pub fn try_new(
        base: Arc<dyn iroha_torii::sorafs::pop_api::PopCredentialRuntimeProviderRegistryV1>,
        issuer_signer: Arc<ExternalSoftwareSignerPopIssuerAdapterV1>,
    ) -> Result<Self, ExternalSoftwareSignerAdapterErrorV1> {
        let first = base
            .qualification()
            .map_err(|_| ExternalSoftwareSignerAdapterErrorV1::Unavailable)?;
        issuer_signer.signer.qualify_live()?;
        let second = base
            .qualification()
            .map_err(|_| ExternalSoftwareSignerAdapterErrorV1::Unavailable)?;
        if first != second || first.revision == 0 || first.policy_digest == [0; 32] {
            return Err(ExternalSoftwareSignerAdapterErrorV1::QualificationChanged);
        }
        Ok(Self {
            base,
            issuer_signer,
        })
    }

    fn qualified_base(
        &self,
    ) -> Result<
        iroha_torii::sorafs::pop_api::PopCredentialRuntimeProviderQualificationV1,
        iroha_torii::sorafs::pop_api::PopCredentialRuntimeProviderRegistryErrorV1,
    > {
        let qualification = self.base.qualification()?;
        self.issuer_signer.signer.qualify_live().map_err(|_| {
            iroha_torii::sorafs::pop_api::
                    PopCredentialRuntimeProviderRegistryErrorV1::StaleOrRevoked
        })?;
        if qualification.revision == 0 || qualification.policy_digest == [0; 32] {
            return Err(iroha_torii::sorafs::pop_api::
                PopCredentialRuntimeProviderRegistryErrorV1::StaleOrRevoked);
        }
        Ok(qualification)
    }
}

impl iroha_torii::sorafs::pop_api::PopCredentialRuntimeProviderRegistryV1
    for ExternalSoftwareSignerPopRegistryV1
{
    fn handle(&self) -> &str {
        self.base.handle()
    }

    fn qualification(
        &self,
    ) -> Result<
        iroha_torii::sorafs::pop_api::PopCredentialRuntimeProviderQualificationV1,
        iroha_torii::sorafs::pop_api::PopCredentialRuntimeProviderRegistryErrorV1,
    > {
        let first = self.qualified_base()?;
        let second = self.base.qualification()?;
        if first != second {
            return Err(iroha_torii::sorafs::pop_api::
                PopCredentialRuntimeProviderRegistryErrorV1::StaleOrRevoked);
        }
        Ok(first)
    }

    fn resolve(
        &self,
        bindings: &iroha_torii::sorafs::pop_api::PopCredentialRuntimeProviderBindingsV1,
    ) -> Result<
        iroha_torii::sorafs::pop_api::PopCredentialRuntimeProvidersV1,
        iroha_torii::sorafs::pop_api::PopCredentialRuntimeProviderRegistryErrorV1,
    > {
        let before = self.qualification()?;
        let signer_binding = self.issuer_signer.signer_binding();
        if bindings.issuer_signer_handle() != signer_binding.handle
            || bindings.issuer_id() != self.issuer_signer.issuer_id()
            || bindings.issuer_public_key() != self.issuer_signer.public_key()
            || bindings.issuer_policy_digest() != signer_binding.policy_digest
        {
            return Err(iroha_torii::sorafs::pop_api::
                PopCredentialRuntimeProviderRegistryErrorV1::RejectedBindings);
        }
        let mut providers = self.base.resolve(bindings)?;
        let after = self.qualification()?;
        if before != after
            || providers.issuer_signer.key_id() != bindings.issuer_signer_handle()
            || providers.issuer_signer.public_key() != bindings.issuer_public_key()
        {
            return Err(iroha_torii::sorafs::pop_api::
                PopCredentialRuntimeProviderRegistryErrorV1::StaleOrRevoked);
        }
        providers.issuer_signer = self.issuer_signer.clone();
        Ok(providers)
    }
}

fn map_client_error(
    error: ExternalSoftwareSignerClientErrorV1,
) -> ExternalSoftwareSignerAdapterErrorV1 {
    match error {
        ExternalSoftwareSignerClientErrorV1::Unavailable
        | ExternalSoftwareSignerClientErrorV1::Authentication
        | ExternalSoftwareSignerClientErrorV1::Protocol => {
            ExternalSoftwareSignerAdapterErrorV1::Unavailable
        }
        ExternalSoftwareSignerClientErrorV1::Rejected
        | ExternalSoftwareSignerClientErrorV1::Equivocation => {
            ExternalSoftwareSignerAdapterErrorV1::Refused
        }
        ExternalSoftwareSignerClientErrorV1::BindingMismatch
        | ExternalSoftwareSignerClientErrorV1::StaleOrRevoked => {
            ExternalSoftwareSignerAdapterErrorV1::QualificationChanged
        }
    }
}

fn map_potr_error(
    error: ExternalSoftwareSignerAdapterErrorV1,
) -> iroha_torii::sorafs::PotrSignerServiceError {
    match error {
        ExternalSoftwareSignerAdapterErrorV1::Unavailable => {
            iroha_torii::sorafs::PotrSignerServiceError::Unavailable
        }
        _ => iroha_torii::sorafs::PotrSignerServiceError::Refused,
    }
}

fn map_billing_readiness_error(
    error: ExternalSoftwareSignerAdapterErrorV1,
) -> sorafs_node::hedging_billing_service::HedgingBillingRuntimeProviderReadinessErrorV1 {
    match error {
        ExternalSoftwareSignerAdapterErrorV1::Unavailable => sorafs_node::hedging_billing_service::
            HedgingBillingRuntimeProviderReadinessErrorV1::Unavailable,
        _ => sorafs_node::hedging_billing_service::
            HedgingBillingRuntimeProviderReadinessErrorV1::Rejected,
    }
}

fn map_billing_error(
    error: ExternalSoftwareSignerAdapterErrorV1,
) -> sorafs_node::hedging_billing_service::HedgingBillingExternalError {
    match error {
        ExternalSoftwareSignerAdapterErrorV1::Unavailable => {
            sorafs_node::hedging_billing_service::HedgingBillingExternalError::Unavailable
        }
        _ => sorafs_node::hedging_billing_service::HedgingBillingExternalError::Rejected,
    }
}

fn map_evidence_readiness_error(
    error: ExternalSoftwareSignerAdapterErrorV1,
) -> sorafs_node::evidence_viewer::EvidenceViewerRuntimeProviderReadinessErrorV1 {
    match error {
        ExternalSoftwareSignerAdapterErrorV1::Unavailable => {
            sorafs_node::evidence_viewer::EvidenceViewerRuntimeProviderReadinessErrorV1::Unavailable
        }
        _ => sorafs_node::evidence_viewer::EvidenceViewerRuntimeProviderReadinessErrorV1::Rejected,
    }
}

fn map_evidence_error(
    error: ExternalSoftwareSignerAdapterErrorV1,
) -> sorafs_node::evidence_viewer::EvidenceViewerExternalErrorV1 {
    match error {
        ExternalSoftwareSignerAdapterErrorV1::Unavailable => {
            sorafs_node::evidence_viewer::EvidenceViewerExternalErrorV1::Unavailable
        }
        _ => sorafs_node::evidence_viewer::EvidenceViewerExternalErrorV1::Rejected,
    }
}

fn map_stream_probe_error(
    error: ExternalSoftwareSignerAdapterErrorV1,
) -> iroha_torii::sorafs::StreamTokenRuntimeSignerProbeErrorV1 {
    match error {
        ExternalSoftwareSignerAdapterErrorV1::Unavailable => {
            iroha_torii::sorafs::StreamTokenRuntimeSignerProbeErrorV1::Unavailable
        }
        _ => iroha_torii::sorafs::StreamTokenRuntimeSignerProbeErrorV1::StaleOrRevoked,
    }
}

fn map_stream_error(
    error: ExternalSoftwareSignerAdapterErrorV1,
) -> iroha_torii::sorafs::StreamTokenSigningError {
    match error {
        ExternalSoftwareSignerAdapterErrorV1::Unavailable => {
            iroha_torii::sorafs::StreamTokenSigningError::Unavailable
        }
        _ => iroha_torii::sorafs::StreamTokenSigningError::Refused,
    }
}
