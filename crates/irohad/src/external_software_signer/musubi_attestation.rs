//! Approval-only Musubi custody through the existing authenticated software signer.
//!
//! The daemon accepts only fresh opaque verifier requests. The isolated service
//! independently checks the complete public subject before signing its typed hash.
//! No transaction, publication listener, or offline rollback guarantee is added.
use super::musubi_subject::{member_weight, subject, validated_signing_message};
use super::{
    adapter::{ExternalSoftwareSignerAdapterErrorV1, map_client_error},
    protocol::{SignerRoleV1, SoftwareSignerPublicBindingV1, digest_parts},
    typed_payload::{SoftwareSignerPurposeV1, encode_typed_signing_payload},
    unix::SoftwareSignerClientV1,
};
use iroha_crypto::{Signature, SignatureOf};
use iroha_data_model::{
    account::{AccountController, AccountId},
    musubi::{
        MUSUBI_MAX_PUBLICATION_ATTESTATION_APPROVALS_V1,
        MusubiProviderBundleVerificationApprovalV1, MusubiProviderBundleVerificationAttestationV1,
        MusubiProviderBundleVerificationPayloadV1,
    },
    sorafs::pin_registry::ProviderIngestCompletionSignerPolicyV1,
};
use sorafs_node::{
    MusubiProviderAttestationSignerErrorV1, MusubiProviderAttestationSignerQualificationV1,
    MusubiProviderAttestationSignerV1, ProviderIngestMusubiAttestationApprovalRequestV1,
    musubi_provider_attestation_controller_policy_digest_v1,
};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

/// Fixed, canonically ordered controller set for approval-only software custody.
///
/// Inject this through `IrohaRuntimeDeps`' existing Musubi approval-signer slot;
/// the daemon's governed wrapper independently checks finalized provider ownership.
/// Every configured member must approve each operation, even when a smaller
/// subset could satisfy the controller threshold. This preserves exact replay.
#[derive(Clone)]
pub struct ExternalSoftwareSignerMusubiProviderAttestationAdapterV1 {
    handle: String,
    qualification: MusubiProviderAttestationSignerQualificationV1,
    clients: Arc<Vec<SoftwareSignerClientV1>>,
    admission: Arc<tokio::sync::Semaphore>,
    eligible: Arc<AtomicBool>,
}
impl std::fmt::Debug for ExternalSoftwareSignerMusubiProviderAttestationAdapterV1 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ExternalSoftwareSignerMusubiProviderAttestationAdapterV1")
            .field("handle", &self.handle)
            .field("members", &self.clients.len())
            .finish_non_exhaustive()
    }
}
impl ExternalSoftwareSignerMusubiProviderAttestationAdapterV1 {
    /// Commit the complete ordered public custody set for independent deployment configuration.
    ///
    /// # Errors
    /// Rejects an invalid handle, revision, empty/oversized set, duplicate key, or invalid binding.
    pub fn policy_digest_for_bindings(
        handle: &str,
        revision: u64,
        bindings: &[SoftwareSignerPublicBindingV1],
    ) -> Result<[u8; 32], ExternalSoftwareSignerAdapterErrorV1> {
        let invalid = || ExternalSoftwareSignerAdapterErrorV1::BindingMismatch;
        if !iroha_config::parameters::is_production_runtime_handle(handle)
            || revision == 0
            || bindings.is_empty()
            || bindings.len() > MUSUBI_MAX_PUBLICATION_ATTESTATION_APPROVALS_V1
            || !bindings
                .windows(2)
                .all(|pair| pair[0].public_key < pair[1].public_key)
        {
            return Err(invalid());
        }
        for binding in bindings {
            binding.validate().map_err(|()| invalid())?;
        }
        let encoded = norito::encode_canonical(&bindings.to_vec()).map_err(|_| invalid())?;
        Ok(digest_parts(
            b"iroha.musubi.software-approval-adapter.v1",
            &[handle.as_bytes(), &revision.to_be_bytes(), &encoded],
        ))
    }
    /// Qualify every endpoint against one independently configured complete custody-set digest.
    ///
    /// This blocking deployment constructor performs authenticated, non-mutating probes;
    /// normal trait snapshots are local and all signing I/O stays inside its bounded future.
    /// # Errors
    /// Rejects cross-subject, duplicate, incomplete, unqualified, or substituted controller sets.
    pub fn try_new(
        handle: String,
        revision: u64,
        expected_policy_digest: [u8; 32],
        mut clients: Vec<SoftwareSignerClientV1>,
    ) -> Result<Self, ExternalSoftwareSignerAdapterErrorV1> {
        let invalid = || ExternalSoftwareSignerAdapterErrorV1::BindingMismatch;
        if clients.is_empty()
            || clients.len() > MUSUBI_MAX_PUBLICATION_ATTESTATION_APPROVALS_V1
            || expected_policy_digest == [0; 32]
        {
            return Err(invalid());
        }
        clients.sort_by(|a, b| {
            a.expected_binding()
                .public_key
                .cmp(&b.expected_binding().public_key)
        });
        let bindings: Vec<_> = clients
            .iter()
            .map(|client| client.expected_binding().clone())
            .collect();
        if Self::policy_digest_for_bindings(&handle, revision, &bindings)? != expected_policy_digest
        {
            return Err(invalid());
        }
        let first = &bindings[0];
        let (_, signer, policy) = subject(&first.purpose_binding).map_err(|()| invalid())?;
        let mut weight = 0u32;
        for (client, binding) in clients.iter().zip(&bindings) {
            if binding.role != SignerRoleV1::MusubiProviderAttestation
                || binding.purpose_binding != first.purpose_binding
            {
                return Err(invalid());
            }
            weight = weight
                .checked_add(member_weight(&signer, &binding.public_key).ok_or_else(invalid)?)
                .ok_or_else(invalid)?;
            let before = client.qualify().map_err(map_client_error)?;
            let after = client.qualify().map_err(map_client_error)?;
            if before.binding != *binding || before.revoked || !before.has_same_stable_state(&after)
            {
                return Err(ExternalSoftwareSignerAdapterErrorV1::QualificationChanged);
            }
        }
        let required = match signer.controller() {
            AccountController::Single(_) => 1,
            AccountController::Multisig(policy) => u32::from(policy.threshold()),
        };
        if weight < required {
            return Err(invalid());
        }
        let controller_digest = musubi_provider_attestation_controller_policy_digest_v1(&signer)
            .map_err(|_| invalid())?;
        let qualification = MusubiProviderAttestationSignerQualificationV1::new(
            revision,
            expected_policy_digest,
            policy,
            signer,
            controller_digest,
        );
        qualification.validate().map_err(|_| invalid())?;
        Ok(Self {
            handle,
            qualification,
            clients: Arc::new(clients),
            admission: Arc::new(tokio::sync::Semaphore::new(1)),
            eligible: Arc::new(AtomicBool::new(true)),
        })
    }
    async fn approve_payload(
        &self,
        payload: MusubiProviderBundleVerificationPayloadV1,
        operation_id: [u8; 32],
    ) -> Result<MusubiProviderBundleVerificationAttestationV1, MusubiProviderAttestationSignerErrorV1>
    {
        self.current_eligibility()?;
        let permit = Arc::clone(&self.admission)
            .try_acquire_owned()
            .map_err(|_| MusubiProviderAttestationSignerErrorV1::Unavailable)?;
        let clients = Arc::clone(&self.clients);
        let eligible = Arc::clone(&self.eligible);
        approve_payload_recoverably(move || {
            // Keep admission through cancellation until the actual bounded socket operations end.
            let _permit = permit;
            let encoded = norito::encode_canonical(&payload)
                .map_err(|_| MusubiProviderAttestationSignerErrorV1::Rejected)?;
            let on_client_error = |error| {
                // Exact key/policy generations are immutable. A confirmed revocation
                // cannot be repaired by retrying this deployment-qualified adapter.
                if error == super::unix::ExternalSoftwareSignerClientErrorV1::StaleOrRevoked {
                    eligible.store(false, Ordering::Release);
                }
                signer_error(error)
            };
            let mut approvals = Vec::with_capacity(clients.len());
            for client in clients.iter() {
                let binding = client.expected_binding();
                validated_signing_message(binding, &encoded)
                    .map_err(|()| MusubiProviderAttestationSignerErrorV1::Rejected)?;
                let before = client.qualify().map_err(on_client_error)?;
                if before.binding != *binding || before.revoked {
                    eligible.store(false, Ordering::Release);
                    return Err(MusubiProviderAttestationSignerErrorV1::Rejected);
                }
                let request = encode_typed_signing_payload(
                    binding.role,
                    SoftwareSignerPurposeV1::MusubiProviderAttestation,
                    &encoded,
                )
                .map_err(|()| MusubiProviderAttestationSignerErrorV1::Rejected)?;
                let receipt = client
                    .sign(operation_id, &request)
                    .map_err(on_client_error)?;
                let signature = SignatureOf::from_signature(
                    Signature::try_from_bytes(&receipt.signature)
                        .map_err(|_| MusubiProviderAttestationSignerErrorV1::Rejected)?,
                );
                approvals.push(MusubiProviderBundleVerificationApprovalV1 {
                    public_key: binding.public_key.clone(),
                    signature,
                });
            }
            // Recheck the complete fixed set after the final member's operation.
            for client in clients.iter() {
                let after = client.qualify().map_err(on_client_error)?;
                if after.binding != *client.expected_binding() || after.revoked {
                    eligible.store(false, Ordering::Release);
                    return Err(MusubiProviderAttestationSignerErrorV1::Rejected);
                }
            }
            let attestation = MusubiProviderBundleVerificationAttestationV1 { payload, approvals };
            attestation
                .verify(&attestation.payload.binding)
                .map_err(|_| MusubiProviderAttestationSignerErrorV1::Rejected)?;
            Ok(attestation)
        })
        .await
    }
}

async fn approve_payload_recoverably<F, T>(
    operation: F,
) -> Result<T, MusubiProviderAttestationSignerErrorV1>
where
    F: FnOnce() -> Result<T, MusubiProviderAttestationSignerErrorV1> + Send + 'static,
    T: Send + 'static,
{
    crate::panic_recovery::join_recoverable(crate::panic_recovery::spawn_blocking_recoverable(
        operation,
    ))
    .await
    .map_err(|_| MusubiProviderAttestationSignerErrorV1::Unavailable)?
}

fn signer_error(
    error: super::unix::ExternalSoftwareSignerClientErrorV1,
) -> MusubiProviderAttestationSignerErrorV1 {
    match error {
        super::unix::ExternalSoftwareSignerClientErrorV1::Unavailable => {
            MusubiProviderAttestationSignerErrorV1::Unavailable
        }
        _ => MusubiProviderAttestationSignerErrorV1::Rejected,
    }
}
impl MusubiProviderAttestationSignerV1
    for ExternalSoftwareSignerMusubiProviderAttestationAdapterV1
{
    fn runtime_handle(&self) -> &str {
        &self.handle
    }
    fn authority(&self) -> &AccountId {
        &self.qualification.authority
    }
    fn qualification(
        &self,
    ) -> Result<
        MusubiProviderAttestationSignerQualificationV1,
        MusubiProviderAttestationSignerErrorV1,
    > {
        self.current_eligibility()?;
        Ok(self.qualification.clone())
    }
    fn signer_policy(&self) -> ProviderIngestCompletionSignerPolicyV1 {
        self.qualification.signer_policy
    }
    fn current_eligibility(
        &self,
    ) -> Result<ProviderIngestCompletionSignerPolicyV1, MusubiProviderAttestationSignerErrorV1>
    {
        if !self.eligible.load(Ordering::Acquire) {
            return Err(MusubiProviderAttestationSignerErrorV1::Rejected);
        }
        Ok(self.qualification.signer_policy)
    }
    fn approve<'a>(
        &'a self,
        request: &'a ProviderIngestMusubiAttestationApprovalRequestV1,
    ) -> sorafs_node::provider_ingest_runtime::ProviderIngestFutureV1<
        'a,
        Result<
            MusubiProviderBundleVerificationAttestationV1,
            MusubiProviderAttestationSignerErrorV1,
        >,
    > {
        Box::pin(async move {
            if request.signer_policy() != self.qualification.signer_policy {
                return Err(MusubiProviderAttestationSignerErrorV1::Rejected);
            }
            let operation = sorafs_node::musubi_provider_attestation_approval_id_v1(request)
                .map_err(|_| MusubiProviderAttestationSignerErrorV1::Rejected)?;
            self.approve_payload(request.payload().clone(), *operation.as_bytes())
                .await
        })
    }
}

#[cfg(test)]
pub(super) mod tests;
