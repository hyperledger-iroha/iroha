//! Adapters from the external-signer protocol to existing Torii provider traits.
use super::{
    protocol::{SoftwareSignerPublicBindingV1, SoftwareSignerRoleV1, digest_parts},
    unix::{ExternalSoftwareSignerClientErrorV1, SoftwareSignerClientV1},
};
use iroha_crypto::{PublicKey, Signature};
use iroha_data_model::{
    account::AccountId,
    transaction::{SignedTransaction, TransactionBuilder, TransactionPayload},
};
use iroha_torii::{
    SoraFsOrderbookTransactionSigner, SoraFsOrderbookTransactionSigningError,
    SoraFsProofOutcomeSigningError, SoraFsProofOutcomeTransactionSigner,
    SoraFsRepairTransactionSigner, SoraFsRepairTransactionSigningError,
    SoraFsReserveTransactionSigner, SoraFsReserveTransactionSigningError,
    SorafsNativeTransactionSignerBindingV1, SorafsNativeTransactionSignerProbeErrorV1,
    SorafsNativeTransactionSignerProviderV1, SorafsNativeTransactionSignerQualificationV1,
    SorafsNativeTransactionSignerRoleV1,
};
use std::sync::Arc;
const NATIVE_OPERATION_ID_DOMAIN_V1: &[u8] =
    b"iroha.external-signer.native-transaction.operation.v1";
/// One exact-role external software signer implementing Torii's provider API.
#[derive(Clone, Debug)]
pub struct ExternalSoftwareSignerNativeAdapterV1 {
    client: SoftwareSignerClientV1,
    binding: SoftwareSignerPublicBindingV1,
    native_binding: SorafsNativeTransactionSignerBindingV1,
}
impl ExternalSoftwareSignerNativeAdapterV1 {
    /// Qualify a pinned external service twice and construct its Torii binding.
    ///
    /// # Errors
    ///
    /// Rejects promotion-role services, drift between adjacent probes, revoked
    /// services, or public identity that cannot form the existing native
    /// transaction signer binding.
    pub fn try_new(
        client: SoftwareSignerClientV1,
    ) -> Result<Self, ExternalSoftwareSignerAdapterErrorV1> {
        let binding = client.expected_binding().clone();
        let native_role = binding
            .role
            .native_role()
            .ok_or(ExternalSoftwareSignerAdapterErrorV1::RoleMismatch)?;
        let first = client.qualify().map_err(map_client_error)?;
        let second = client.qualify().map_err(map_client_error)?;
        // Each probe verifies its own attestation. Compare the complete signed
        // live state, not randomized ML-DSA signature bytes.
        if !first.has_same_stable_state(&second) || first.binding != binding || first.revoked {
            return Err(ExternalSoftwareSignerAdapterErrorV1::QualificationChanged);
        }
        let native_binding = SorafsNativeTransactionSignerBindingV1::try_new(
            native_role,
            binding.handle.clone(),
            AccountId::new(binding.public_key.clone()),
            binding.public_key.clone(),
            SorafsNativeTransactionSignerQualificationV1::new(
                binding.policy_revision,
                binding.policy_digest,
            ),
        )
        .map_err(|_| ExternalSoftwareSignerAdapterErrorV1::BindingMismatch)?;
        Ok(Self {
            client,
            binding,
            native_binding,
        })
    }
    /// Return the existing Torii binding consumed by runtime-provider catalogs.
    #[must_use]
    pub const fn native_binding(&self) -> &SorafsNativeTransactionSignerBindingV1 {
        &self.native_binding
    }
    fn sign_native(
        &self,
        expected_role: SoftwareSignerRoleV1,
        payload: TransactionPayload,
    ) -> Result<SignedTransaction, ExternalSoftwareSignerAdapterErrorV1> {
        if self.binding.role != expected_role {
            return Err(ExternalSoftwareSignerAdapterErrorV1::RoleMismatch);
        }
        if payload.authority() != self.native_binding.authority() {
            return Err(ExternalSoftwareSignerAdapterErrorV1::InputAuthorityMismatch);
        }
        let builder = TransactionBuilder::from_payload(payload.clone())
            .map_err(|_| ExternalSoftwareSignerAdapterErrorV1::Refused)?;
        let encoded = builder.encode_payload();
        let operation_id = digest_parts(
            NATIVE_OPERATION_ID_DOMAIN_V1,
            &[
                self.binding.domain.as_bytes(),
                &self.binding.key_revision.to_be_bytes(),
                &builder.payload_hash_bytes(),
            ],
        );
        let receipt = self
            .client
            .sign(operation_id, &encoded)
            .map_err(map_client_error)?;
        if receipt.provenance.binding != self.binding || receipt.provenance.revoked {
            return Err(ExternalSoftwareSignerAdapterErrorV1::QualificationChanged);
        }
        let signature = Signature::try_from_bytes(&receipt.signature)
            .map_err(|_| ExternalSoftwareSignerAdapterErrorV1::SubstitutedTransaction)?;
        let transaction = builder.build_with_signature(signature);
        if transaction.payload() != &payload
            || transaction.authority() != self.native_binding.authority()
            || transaction.verify_signature().is_err()
        {
            return Err(ExternalSoftwareSignerAdapterErrorV1::SubstitutedTransaction);
        }
        Ok(transaction)
    }
    fn qualify_live(&self) -> Result<(), SorafsNativeTransactionSignerProbeErrorV1> {
        let provenance = self
            .client
            .qualify()
            .map_err(|_| SorafsNativeTransactionSignerProbeErrorV1::Unavailable)?;
        if provenance.binding != self.binding || provenance.revoked {
            return Err(SorafsNativeTransactionSignerProbeErrorV1::Refused);
        }
        Ok(())
    }
}
impl SorafsNativeTransactionSignerProviderV1 for ExternalSoftwareSignerNativeAdapterV1 {
    fn role(&self) -> SorafsNativeTransactionSignerRoleV1 {
        self.binding
            .role
            .native_role()
            .expect("native adapter construction excludes promotion role")
    }
    fn handle(&self) -> &str {
        &self.binding.handle
    }
    fn authority(&self) -> AccountId {
        self.native_binding.authority().clone()
    }
    fn public_key(&self) -> Result<PublicKey, SorafsNativeTransactionSignerProbeErrorV1> {
        self.qualify_live()?;
        Ok(self.binding.public_key.clone())
    }
    fn qualification(
        &self,
    ) -> Result<
        SorafsNativeTransactionSignerQualificationV1,
        SorafsNativeTransactionSignerProbeErrorV1,
    > {
        self.qualify_live()?;
        Ok(self.native_binding.qualification())
    }
}
macro_rules! impl_role_signer {
    ($trait_name:ident, $error:ident, $role:expr) => {
        impl $trait_name for ExternalSoftwareSignerNativeAdapterV1 {
            fn sign(&self, payload: TransactionPayload) -> Result<SignedTransaction, $error> {
                self.sign_native($role, payload)
                    .map_err(|error| match error {
                        ExternalSoftwareSignerAdapterErrorV1::Unavailable => $error::Unavailable,
                        ExternalSoftwareSignerAdapterErrorV1::Refused
                        | ExternalSoftwareSignerAdapterErrorV1::RoleMismatch
                        | ExternalSoftwareSignerAdapterErrorV1::BindingMismatch => $error::Refused,
                        ExternalSoftwareSignerAdapterErrorV1::InputAuthorityMismatch => {
                            $error::InputAuthorityMismatch
                        }
                        ExternalSoftwareSignerAdapterErrorV1::SubstitutedTransaction => {
                            $error::SubstitutedTransaction
                        }
                        ExternalSoftwareSignerAdapterErrorV1::QualificationChanged => {
                            $error::QualificationChanged
                        }
                    })
            }
        }
    };
}
impl_role_signer!(
    SoraFsProofOutcomeTransactionSigner,
    SoraFsProofOutcomeSigningError,
    SoftwareSignerRoleV1::ProofOutcome
);
impl_role_signer!(
    SoraFsRepairTransactionSigner,
    SoraFsRepairTransactionSigningError,
    SoftwareSignerRoleV1::Repair
);
impl_role_signer!(
    SoraFsReserveTransactionSigner,
    SoraFsReserveTransactionSigningError,
    SoftwareSignerRoleV1::Reserve
);
impl_role_signer!(
    SoraFsOrderbookTransactionSigner,
    SoraFsOrderbookTransactionSigningError,
    SoftwareSignerRoleV1::Orderbook
);
/// Role-indexed set that attaches external signers to the existing broker API.
#[derive(Clone, Default)]
pub struct ExternalSoftwareSignerNativeBackendsV1 {
    proof_outcome: Option<Arc<ExternalSoftwareSignerNativeAdapterV1>>,
    repair: Option<Arc<ExternalSoftwareSignerNativeAdapterV1>>,
    reserve: Option<Arc<ExternalSoftwareSignerNativeAdapterV1>>,
    orderbook: Option<Arc<ExternalSoftwareSignerNativeAdapterV1>>,
}
impl ExternalSoftwareSignerNativeBackendsV1 {
    /// Create an empty role set.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            proof_outcome: None,
            repair: None,
            reserve: None,
            orderbook: None,
        }
    }
    /// Add one exact native role, rejecting duplicates and promotion services.
    ///
    /// # Errors
    ///
    /// Returns a role mismatch for promotion services or a duplicate native role.
    pub fn insert(
        &mut self,
        adapter: Arc<ExternalSoftwareSignerNativeAdapterV1>,
    ) -> Result<(), ExternalSoftwareSignerAdapterErrorV1> {
        let slot = match adapter.binding.role {
            SoftwareSignerRoleV1::ProofOutcome => &mut self.proof_outcome,
            SoftwareSignerRoleV1::Repair => &mut self.repair,
            SoftwareSignerRoleV1::Reserve => &mut self.reserve,
            SoftwareSignerRoleV1::Orderbook => &mut self.orderbook,
            SoftwareSignerRoleV1::Promotion
            | SoftwareSignerRoleV1::GovernanceDag
            | SoftwareSignerRoleV1::PotrGateway
            | SoftwareSignerRoleV1::PotrProvider
            | SoftwareSignerRoleV1::BillingStatement
            | SoftwareSignerRoleV1::EvidenceViewer
            | SoftwareSignerRoleV1::StreamToken
            | SoftwareSignerRoleV1::PopCredentials => {
                return Err(ExternalSoftwareSignerAdapterErrorV1::RoleMismatch);
            }
        };
        if slot.is_some() {
            return Err(ExternalSoftwareSignerAdapterErrorV1::RoleMismatch);
        }
        *slot = Some(adapter);
        Ok(())
    }
    /// Attach every present role through existing broker builder methods.
    #[must_use]
    pub fn attach_to(
        self,
        mut backends: crate::RuntimeProviderBrokerBackendsV1,
    ) -> crate::RuntimeProviderBrokerBackendsV1 {
        if let Some(signer) = self.proof_outcome {
            backends = backends.with_proof_outcome_transaction_signer(signer);
        }
        if let Some(signer) = self.repair {
            backends = backends.with_repair_transaction_signer(signer);
        }
        if let Some(signer) = self.reserve {
            backends = backends.with_reserve_transaction_signer(signer);
        }
        if let Some(signer) = self.orderbook {
            backends = backends.with_orderbook_transaction_signer(signer);
        }
        backends
    }
}
impl crate::RuntimeProviderBrokerBackendRegistryV1 for ExternalSoftwareSignerNativeBackendsV1 {
    fn resolve(
        &self,
        bindings: &crate::IrohaRuntimeProviderBindingsV1,
    ) -> Result<crate::RuntimeProviderBrokerBackendsV1, crate::IrohaRuntimeProviderRegistryErrorV1>
    {
        let mut requested = [false; 4];
        for configured in bindings.iter() {
            let exact = configured
                .native_signer_binding()
                .ok_or(crate::IrohaRuntimeProviderRegistryErrorV1::IncompleteResolution)?;
            let (index, resolved) = match exact.role() {
                SorafsNativeTransactionSignerRoleV1::ProofOutcome => {
                    (0, self.proof_outcome.as_deref())
                }
                SorafsNativeTransactionSignerRoleV1::Repair => (1, self.repair.as_deref()),
                SorafsNativeTransactionSignerRoleV1::Reserve => (2, self.reserve.as_deref()),
                SorafsNativeTransactionSignerRoleV1::Orderbook => (3, self.orderbook.as_deref()),
            };
            let resolved =
                resolved.ok_or(crate::IrohaRuntimeProviderRegistryErrorV1::IncompleteResolution)?;
            if configured.handle() != resolved.native_binding().handle()
                || exact != resolved.native_binding()
            {
                return Err(crate::IrohaRuntimeProviderRegistryErrorV1::BindingMismatch);
            }
            requested[index] = true;
        }
        let supplied = [
            self.proof_outcome.is_some(),
            self.repair.is_some(),
            self.reserve.is_some(),
            self.orderbook.is_some(),
        ];
        if requested != supplied {
            return Err(crate::IrohaRuntimeProviderRegistryErrorV1::UnexpectedProviders);
        }
        Ok(self
            .clone()
            .attach_to(crate::RuntimeProviderBrokerBackendsV1::new()))
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
/// Payload-free adapter failure classification.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExternalSoftwareSignerAdapterErrorV1 {
    /// Transport or isolated signer service is unavailable.
    Unavailable,
    /// The service refused the exact request.
    Refused,
    /// Service and requested Torii roles differ.
    RoleMismatch,
    /// Public binding cannot form the existing Torii binding.
    BindingMismatch,
    /// Transaction authority differs from the isolated key.
    InputAuthorityMismatch,
    /// Returned signature or signed transaction differs from the input.
    SubstitutedTransaction,
    /// Key, policy, audit state, or revocation changed around the operation.
    QualificationChanged,
}
