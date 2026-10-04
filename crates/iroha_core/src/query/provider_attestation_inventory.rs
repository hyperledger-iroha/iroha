//! Read-only native authorization for an already-owned provider attestation inventory.
//!
//! Inventory bytes establish neither registry membership nor a completed publication. The
//! manager still registers the unchanged signed attestation through the ordinary native owner.

use crate::{
    query::{
        provider_admission::read_provider_admission_at_native_current_v1,
        provider_ingest_source::has_provider_completion_permission_v1,
        signer_check::{SignerCertifiedWalkV1, with_native_check_read_limits},
    },
    smartcontracts::isi::musubi::{
        ensure_archive_manager, validate_provider_bundle_attestation,
        validate_replication_order_archive_binding,
    },
    state::{StateReadOnly, StateView, WorldReadOnly},
    telemetry::MusubiGovernanceRejectionReasonV1,
};
use iroha_data_model::{
    account::AccountId,
    block::consensus::SumeragiRootScope,
    musubi::{MusubiProviderBundleAttestationKeyV1, MusubiProviderBundleVerificationAttestationV1},
};
use mv::storage::StorageReadOnly;

/// Payload-free refusal; unavailable native sources never become an absence claim.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum ProviderAttestationInventoryReadErrorV1 {
    /// The selected account, provider, archive, order, or signed body does not match current state.
    #[error("native provider attestation inventory read rejected")]
    Rejected,
    /// Native history, resources, or the selected complete cut is unavailable.
    #[error("native provider attestation inventory read unavailable")]
    Unavailable,
}

/// Authenticate the requesting archive manager and the exact provider's completed native order.
///
/// Call before and after inventory I/O against fresh captured views. Supplying `attestation`
/// additionally invokes the sole native Register validator on the returned original body.
/// This does not insert a registry record, authorize signing, or attest that an inventory exists.
/// # Errors
/// Refuses private roots, unavailable native execution, changed manager/owner/completion binding,
/// revoked admission or permission, and a substituted signed attestation.
pub fn authorize_provider_attestation_inventory_read_v1(
    view: &StateView<'_>,
    manager: &AccountId,
    key: MusubiProviderBundleAttestationKeyV1,
    attestation: Option<&MusubiProviderBundleVerificationAttestationV1>,
    now_secs: u64,
) -> Result<(), ProviderAttestationInventoryReadErrorV1> {
    use ProviderAttestationInventoryReadErrorV1::{Rejected, Unavailable};
    with_native_check_read_limits(|| {
        key.validate().map_err(|_| Rejected)?;
        if now_secs == 0
            || view.height() < 2
            || crate::sumeragi::lanes::routing::committed_root_scope(view.world())
                != Some(SumeragiRootScope::Global)
        {
            return Err(Rejected);
        }
        let height = u64::try_from(view.height()).map_err(|_| Unavailable)?;
        let chain = SignerCertifiedWalkV1::new(view).map_err(|_| Unavailable)?;
        let receipt = chain
            .walk(height, height)
            .next()
            .ok_or(Unavailable)?
            .map_err(|_| Unavailable)?;
        let certified = receipt.in_view(view).map_err(|_| Unavailable)?;
        let instance = SumeragiRootScope::Global
            .instance_id(
                &crate::sumeragi::crypto::BlsCrypto::new(),
                *view.network_id(),
                view.chain_id().as_str(),
            )
            .map_err(|_| Rejected)?;
        if certified
            .header()
            .is_none_or(|header| header.instance != instance)
        {
            return Err(Rejected);
        }
        let world = view.world();
        let archive = world
            .musubi_archives()
            .get(&key.archive_id)
            .ok_or(Rejected)?;
        archive.validate().map_err(|_| Rejected)?;
        let mut reason = MusubiGovernanceRejectionReasonV1::Unauthorized;
        ensure_archive_manager(archive, manager, world, &mut reason).map_err(|_| Rejected)?;
        validate_replication_order_archive_binding(archive, &key.replication_order, world)
            .map_err(|_| Rejected)?;
        let order = world
            .replication_orders()
            .get(&key.replication_order)
            .ok_or(Rejected)?;
        let completed = order.provider_completion(key.provider_id).ok_or(Rejected)?;
        let authority = world
            .provider_ingest_completion_authorities()
            .get(&key.provider_id)
            .ok_or(Rejected)?;
        if !authority.is_valid()
            || authority != &completed.completion_authority
            || completed.completed_by != authority.completion_signer
            || world.provider_owners().get(&key.provider_id) != Some(&authority.provider_owner)
            || !has_provider_completion_permission_v1(
                world,
                &authority.completion_signer,
                key.provider_id,
            )
            || archive.archive_id != key.archive_id
            || archive.staging_receipt.payload.binding.network_id != *view.network_id()
            || order.order_id != key.replication_order
            || order.manifest_root_cid != archive.commitment.root_cid
        {
            return Err(Rejected);
        }
        let now = now_secs.max(
            view.authenticated_query_ledger_time_ms()
                .ok_or(Unavailable)?
                / 1_000,
        );
        read_provider_admission_at_native_current_v1(view, &receipt, key.provider_id, now)
            .map_err(|_| Unavailable)?
            .ok_or(Rejected)?;
        if let Some(attestation) = attestation {
            if attestation.key() != key {
                return Err(Rejected);
            }
            validate_provider_bundle_attestation(archive, attestation, view)
                .map_err(|_| Rejected)?;
        }
        Ok(())
    })
}

#[cfg(test)]
mod tests;
