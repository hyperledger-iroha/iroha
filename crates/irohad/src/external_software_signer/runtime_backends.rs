//! Exact catalog-to-backend assembly for isolated software signer services.
use super::{
    adapter::{ExternalSoftwareSignerAdapterErrorV1, ExternalSoftwareSignerNativeAdapterV1},
    runtime_adapters::{
        ExternalSoftwareSignerBillingStatementAdapterV1,
        ExternalSoftwareSignerEvidenceViewerAdapterV1,
        ExternalSoftwareSignerGovernanceDagAdapterV1, ExternalSoftwareSignerPotrGatewayAdapterV1,
        ExternalSoftwareSignerPotrProviderAdapterV1, ExternalSoftwareSignerStreamTokenAdapterV1,
    },
};
use iroha_torii::sorafs::{
    PotrGatewaySignerV1 as _, PotrProviderSignerV1 as _, StreamTokenRuntimeSigner as _,
};
use sorafs_node::{
    GovernanceDagRuntimeSigner as _, evidence_viewer::EvidenceViewerReceiptSignerV1 as _,
};
use std::sync::Arc;
/// Deployment-owned set of every phase-one software-signing backend.
#[derive(Clone, Default)]
pub struct ExternalSoftwareSignerBackendsV1 {
    base_registry: Option<Arc<dyn crate::RuntimeProviderBrokerBackendRegistryV1>>,
    native: [Option<Arc<ExternalSoftwareSignerNativeAdapterV1>>; 4],
    governance_dag: Option<Arc<ExternalSoftwareSignerGovernanceDagAdapterV1>>,
    potr_gateway: Option<Arc<ExternalSoftwareSignerPotrGatewayAdapterV1>>,
    potr_provider: Option<Arc<ExternalSoftwareSignerPotrProviderAdapterV1>>,
    billing_statement: Option<Arc<ExternalSoftwareSignerBillingStatementAdapterV1>>,
    evidence_viewer: Option<Arc<ExternalSoftwareSignerEvidenceViewerAdapterV1>>,
    stream_token: Option<Arc<ExternalSoftwareSignerStreamTokenAdapterV1>>,
    pop_registry:
        Option<Arc<dyn iroha_torii::sorafs::pop_api::PopCredentialRuntimeProviderRegistryV1>>,
}
impl ExternalSoftwareSignerBackendsV1 {
    /// Construct an empty exact backend set.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            base_registry: None,
            native: [None, None, None, None],
            governance_dag: None,
            potr_gateway: None,
            potr_provider: None,
            billing_statement: None,
            evidence_viewer: None,
            stream_token: None,
            pop_registry: None,
        }
    }
    /// Compose with the deployment registry for every non-signer slot.
    ///
    /// The complete catalog is partitioned before either resolver is called; a base registry that
    /// returns any signer backend is rejected rather than overwritten.
    #[must_use]
    pub fn with_base_registry(
        mut self,
        registry: Arc<dyn crate::RuntimeProviderBrokerBackendRegistryV1>,
    ) -> Self {
        self.base_registry = Some(registry);
        self
    }
    /// Insert one native transaction signer, rejecting duplicates.
    ///
    /// # Errors
    /// Returns a role mismatch when that native signer role is already populated.
    pub fn insert_native(
        &mut self,
        signer: Arc<ExternalSoftwareSignerNativeAdapterV1>,
    ) -> Result<(), ExternalSoftwareSignerAdapterErrorV1> {
        let index = native_role_index(signer.native_binding().role());
        if self.native[index].is_some() {
            return Err(ExternalSoftwareSignerAdapterErrorV1::RoleMismatch);
        }
        self.native[index] = Some(signer);
        Ok(())
    }
    /// Insert the sole Governance DAG signer.
    ///
    /// # Errors
    /// Returns a role mismatch when the slot is already populated.
    pub fn insert_governance_dag(
        &mut self,
        signer: Arc<ExternalSoftwareSignerGovernanceDagAdapterV1>,
    ) -> Result<(), ExternalSoftwareSignerAdapterErrorV1> {
        insert_once(&mut self.governance_dag, signer)
    }
    /// Insert the sole `PoTR` gateway signer.
    ///
    /// # Errors
    /// Returns a role mismatch when the slot is already populated.
    pub fn insert_potr_gateway(
        &mut self,
        signer: Arc<ExternalSoftwareSignerPotrGatewayAdapterV1>,
    ) -> Result<(), ExternalSoftwareSignerAdapterErrorV1> {
        insert_once(&mut self.potr_gateway, signer)
    }
    /// Insert the sole `PoTR` provider signer.
    ///
    /// # Errors
    /// Returns a role mismatch when the slot is already populated.
    pub fn insert_potr_provider(
        &mut self,
        signer: Arc<ExternalSoftwareSignerPotrProviderAdapterV1>,
    ) -> Result<(), ExternalSoftwareSignerAdapterErrorV1> {
        insert_once(&mut self.potr_provider, signer)
    }
    /// Insert the sole billing-statement signer.
    ///
    /// # Errors
    /// Returns a role mismatch when the slot is already populated.
    pub fn insert_billing_statement(
        &mut self,
        signer: Arc<ExternalSoftwareSignerBillingStatementAdapterV1>,
    ) -> Result<(), ExternalSoftwareSignerAdapterErrorV1> {
        insert_once(&mut self.billing_statement, signer)
    }
    /// Insert the sole evidence-viewer signer.
    ///
    /// # Errors
    /// Returns a role mismatch when the slot is already populated.
    pub fn insert_evidence_viewer(
        &mut self,
        signer: Arc<ExternalSoftwareSignerEvidenceViewerAdapterV1>,
    ) -> Result<(), ExternalSoftwareSignerAdapterErrorV1> {
        insert_once(&mut self.evidence_viewer, signer)
    }
    /// Insert the sole stream-token signer.
    ///
    /// # Errors
    /// Returns a role mismatch when the slot is already populated.
    pub fn insert_stream_token(
        &mut self,
        signer: Arc<ExternalSoftwareSignerStreamTokenAdapterV1>,
    ) -> Result<(), ExternalSoftwareSignerAdapterErrorV1> {
        insert_once(&mut self.stream_token, signer)
    }
    /// Insert the approved decorated `PoP` provider registry.
    ///
    /// # Errors
    /// Returns a role mismatch when the slot is already populated.
    pub fn insert_pop_registry(
        &mut self,
        registry: Arc<dyn iroha_torii::sorafs::pop_api::PopCredentialRuntimeProviderRegistryV1>,
    ) -> Result<(), ExternalSoftwareSignerAdapterErrorV1> {
        insert_once(&mut self.pop_registry, registry)
    }
    fn attach(
        self,
        mut backends: crate::RuntimeProviderBrokerBackendsV1,
    ) -> crate::RuntimeProviderBrokerBackendsV1 {
        for signer in self.native.into_iter().flatten() {
            backends = match signer.native_binding().role() {
                iroha_torii::SorafsNativeTransactionSignerRoleV1::ProofOutcome => {
                    backends.with_proof_outcome_transaction_signer(signer)
                }
                iroha_torii::SorafsNativeTransactionSignerRoleV1::Repair => {
                    backends.with_repair_transaction_signer(signer)
                }
                iroha_torii::SorafsNativeTransactionSignerRoleV1::Reserve => {
                    backends.with_reserve_transaction_signer(signer)
                }
                iroha_torii::SorafsNativeTransactionSignerRoleV1::Orderbook => {
                    backends.with_orderbook_transaction_signer(signer)
                }
            };
        }
        if let Some(signer) = self.governance_dag {
            backends = backends.with_governance_dag_signer(signer);
        }
        if let Some(signer) = self.potr_gateway {
            backends = backends.with_potr_gateway_signer(signer);
        }
        if let Some(signer) = self.potr_provider {
            backends = backends.with_potr_provider_signer(signer);
        }
        if let Some(signer) = self.billing_statement {
            backends = backends.with_billing_statement_signer(signer);
        }
        if let Some(signer) = self.evidence_viewer {
            backends = backends.with_evidence_viewer_receipt_signer(signer);
        }
        if let Some(signer) = self.stream_token {
            backends = backends.with_stream_token_signer(signer);
        }
        if let Some(registry) = self.pop_registry {
            backends = backends.with_pop_credential_provider_registry(registry);
        }
        backends
    }
    #[expect(
        clippy::too_many_lines,
        reason = "the signer subset validator keeps all purpose-separated slots in one auditable match"
    )]
    fn validate_signer_subset(
        &self,
        bindings: &crate::IrohaRuntimeProviderBindingsV1,
    ) -> Result<(), crate::IrohaRuntimeProviderRegistryErrorV1> {
        let mut requested_native = [false; 4];
        let mut requested_typed = [false; 7];
        for configured in bindings.iter() {
            use crate::IrohaRuntimeProviderSlotV1 as Slot;
            match configured.slot() {
                Slot::ProofOutcomeTransactionSigner
                | Slot::RepairTransactionSigner
                | Slot::ReserveTransactionSigner
                | Slot::OrderbookTransactionSigner => {
                    let exact = configured
                        .native_signer_binding()
                        .ok_or(registry_incomplete())?;
                    let index = native_role_index(exact.role());
                    let resolved = self.native[index].as_deref().ok_or(registry_incomplete())?;
                    if resolved.native_binding() != exact {
                        return Err(registry_mismatch());
                    }
                    requested_native[index] = true;
                }
                Slot::GovernanceDagSigner => {
                    let resolved = self
                        .governance_dag
                        .as_deref()
                        .ok_or(registry_incomplete())?;
                    exact_public_binding(configured, resolved.signer_binding())?;
                    if configured.governance_dag_publisher_peer_id()
                        != Some(resolved.publisher_peer_id())
                        || configured.governance_dag_publisher_public_key()
                            != Some(resolved.public_key())
                    {
                        return Err(registry_mismatch());
                    }
                    requested_typed[0] = true;
                }
                Slot::PotrGatewaySigner => {
                    let resolved = self.potr_gateway.as_deref().ok_or(registry_incomplete())?;
                    exact_public_binding(configured, resolved.signer_binding())?;
                    let runtime = configured
                        .potr_runtime_binding()
                        .ok_or(registry_mismatch())?;
                    if runtime.gateway_signer.signer_id != resolved.signer_id()
                        || runtime.gateway_public_key
                            != resolved.public_key().map_err(|_| registry_mismatch())?
                    {
                        return Err(registry_mismatch());
                    }
                    requested_typed[1] = true;
                }
                Slot::PotrProviderSigner => {
                    let resolved = self.potr_provider.as_deref().ok_or(registry_incomplete())?;
                    exact_public_binding(configured, resolved.signer_binding())?;
                    let runtime = configured
                        .potr_runtime_binding()
                        .ok_or(registry_mismatch())?;
                    if runtime.provider_signer.signer_id != resolved.signer_id()
                        || runtime.baseline_admission_policy.provider_id
                            != resolved.provider_id().map_err(|_| registry_mismatch())?
                    {
                        return Err(registry_mismatch());
                    }
                    requested_typed[2] = true;
                }
                Slot::BillingStatementSigner => {
                    let resolved = self
                        .billing_statement
                        .as_deref()
                        .ok_or(registry_incomplete())?;
                    exact_public_binding(configured, resolved.signer_binding())?;
                    requested_typed[3] = true;
                }
                Slot::EvidenceViewerReceiptSigner => {
                    let resolved = self
                        .evidence_viewer
                        .as_deref()
                        .ok_or(registry_incomplete())?;
                    exact_public_binding(configured, resolved.signer_binding())?;
                    if configured.evidence_viewer_receipt_signer_public_key()
                        != Some(resolved.public_key())
                    {
                        return Err(registry_mismatch());
                    }
                    requested_typed[4] = true;
                }
                Slot::StreamTokenSigner => {
                    let resolved = self.stream_token.as_deref().ok_or(registry_incomplete())?;
                    exact_public_binding(configured, resolved.signer_binding())?;
                    if configured.stream_token_signer_public_key() != Some(resolved.public_key()) {
                        return Err(registry_mismatch());
                    }
                    requested_typed[5] = true;
                }
                Slot::PopCredentialProviderRegistry => {
                    let resolved = self.pop_registry.as_deref().ok_or(registry_incomplete())?;
                    let qualification =
                        resolved.qualification().map_err(|_| registry_mismatch())?;
                    if configured.handle() != resolved.handle()
                        || configured.revision() != Some(qualification.revision)
                        || configured.policy_digest() != Some(qualification.policy_digest)
                    {
                        return Err(registry_mismatch());
                    }
                    requested_typed[6] = true;
                }
                _ => return Err(registry_incomplete()),
            }
        }
        let supplied_native = self.native.each_ref().map(Option::is_some);
        let supplied_typed = [
            self.governance_dag.is_some(),
            self.potr_gateway.is_some(),
            self.potr_provider.is_some(),
            self.billing_statement.is_some(),
            self.evidence_viewer.is_some(),
            self.stream_token.is_some(),
            self.pop_registry.is_some(),
        ];
        if requested_native != supplied_native || requested_typed != supplied_typed {
            return Err(crate::IrohaRuntimeProviderRegistryErrorV1::UnexpectedProviders);
        }
        Ok(())
    }
}
impl crate::RuntimeProviderBrokerBackendRegistryV1 for ExternalSoftwareSignerBackendsV1 {
    fn resolve(
        &self,
        bindings: &crate::IrohaRuntimeProviderBindingsV1,
    ) -> Result<crate::RuntimeProviderBrokerBackendsV1, crate::IrohaRuntimeProviderRegistryErrorV1>
    {
        let (signer_bindings, base_bindings) = bindings.partition_external_software_signers_v1();
        self.validate_signer_subset(&signer_bindings)?;
        let base = match &self.base_registry {
            Some(registry) => registry.resolve(&base_bindings)?,
            None if base_bindings.is_empty() => crate::RuntimeProviderBrokerBackendsV1::new(),
            None => return Err(registry_incomplete()),
        };
        if base.contains_external_software_signer_v1() {
            return Err(crate::IrohaRuntimeProviderRegistryErrorV1::UnexpectedProviders);
        }
        Ok(self.clone().attach(base))
    }
}
fn exact_public_binding(
    configured: &crate::IrohaRuntimeProviderBindingV1,
    signer: &super::protocol::SoftwareSignerPublicBindingV1,
) -> Result<(), crate::IrohaRuntimeProviderRegistryErrorV1> {
    if configured.handle() != signer.handle
        || configured.revision() != Some(signer.policy_revision)
        || configured.policy_digest() != Some(signer.policy_digest)
    {
        return Err(registry_mismatch());
    }
    Ok(())
}
fn insert_once<T>(
    slot: &mut Option<Arc<T>>,
    value: Arc<T>,
) -> Result<(), ExternalSoftwareSignerAdapterErrorV1>
where
    T: ?Sized,
{
    if slot.is_some() {
        return Err(ExternalSoftwareSignerAdapterErrorV1::RoleMismatch);
    }
    *slot = Some(value);
    Ok(())
}
const fn native_role_index(role: iroha_torii::SorafsNativeTransactionSignerRoleV1) -> usize {
    match role {
        iroha_torii::SorafsNativeTransactionSignerRoleV1::ProofOutcome => 0,
        iroha_torii::SorafsNativeTransactionSignerRoleV1::Repair => 1,
        iroha_torii::SorafsNativeTransactionSignerRoleV1::Reserve => 2,
        iroha_torii::SorafsNativeTransactionSignerRoleV1::Orderbook => 3,
    }
}
const fn registry_incomplete() -> crate::IrohaRuntimeProviderRegistryErrorV1 {
    crate::IrohaRuntimeProviderRegistryErrorV1::IncompleteResolution
}
const fn registry_mismatch() -> crate::IrohaRuntimeProviderRegistryErrorV1 {
    crate::IrohaRuntimeProviderRegistryErrorV1::BindingMismatch
}
