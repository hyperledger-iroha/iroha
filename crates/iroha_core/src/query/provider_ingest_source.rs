//! Exact live native replication authority for initial provider source reads.
use crate::{
    query::{
        provider_admission::read_finalized_provider_admission_v1,
        signer_finality::verify_signer_finality_v1,
    },
    state::{StateReadOnly, StateView, WorldReadOnly},
};
use iroha_data_model::{
    account::AccountId,
    sorafs::{
        capacity::ProviderId,
        pin_registry::{ManifestDigest, PinStatus, ReplicationOrderId, ReplicationOrderStatus},
        publication::SorafsAssignedSourceRequestV1,
    },
};
use mv::storage::StorageReadOnly;
use sorafs_manifest::capacity::ReplicationOrderV1;

/// An initial source request is stale, revoked, unassigned or outside durable finality.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("native provider source authority rejected")]
pub struct ProviderSourceAuthorizationErrorV1;

/// Exact pin and assignment selected for a publisher staging operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PublisherSourceBindingV1 {
    /// Assigned receiving provider.
    pub provider_id: ProviderId,
    /// Exact native replication order.
    pub order_id: ReplicationOrderId,
    /// Canonical manifest digest.
    pub manifest_digest: ManifestDigest,
    /// Current nonzero assignment revision.
    pub assignment_revision: u64,
}

/// Consensus-bound staging lifetime authenticated from one immutable finalized State view.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PublisherSourceAuthorizationV1 {
    /// Inclusive native order deadline, bounded by the pin's retention deadline.
    pub deadline_epoch: u64,
    /// Timestamp of the authenticated finalized head.
    pub finalized_epoch: u64,
}

/// Authenticate the pin submitter, admitted provider and exact live pending assignment.
///
/// The caller authenticates `publisher` and repeats this check after staging I/O. This permits
/// isolated source staging only; normal storage verification and native completion remain required.
///
/// # Errors
/// Rejects missing durable finality, expired or revoked admission, inactive pins and changed orders.
pub fn authorize_publisher_source_v1(
    view: &StateView<'_>,
    publisher: &AccountId,
    binding: &PublisherSourceBindingV1,
    now_secs: u64,
) -> Result<PublisherSourceAuthorizationV1, ProviderSourceAuthorizationErrorV1> {
    let rejected = ProviderSourceAuthorizationErrorV1;
    if now_secs == 0 || binding.assignment_revision == 0 {
        return Err(rejected);
    }
    let finalized_epoch = view.authenticated_query_ledger_time_ms().ok_or(rejected)? / 1_000;
    let now = finalized_epoch.max(now_secs);
    verify_signer_finality_v1(
        view,
        u64::try_from(view.height()).map_err(|_| rejected)?,
        *view.latest_block_hash().ok_or(rejected)?.as_ref(),
    )
    .map_err(|_| rejected)?;
    read_finalized_provider_admission_v1(view, binding.provider_id, now)
        .map_err(|_| rejected)?
        .ok_or(rejected)?;
    let world = view.world();
    let pin = world
        .pin_manifests()
        .get(&binding.manifest_digest)
        .ok_or(rejected)?;
    let order = world
        .replication_orders()
        .get(&binding.order_id)
        .ok_or(rejected)?;
    if pin.submitted_by != *publisher
        || !matches!(pin.status, PinStatus::Approved(_))
        || pin.policy.retention_epoch <= now
        || order.manifest_digest != pin.digest
        || order.manifest_root_cid != pin.root_cid
        || order.assignment_revision != binding.assignment_revision
        || !matches!(order.status, ReplicationOrderStatus::Pending)
        || order.deadline_epoch < now
        || order.issued_epoch > finalized_epoch
        || order.provider_completion(binding.provider_id).is_some()
    {
        return Err(rejected);
    }
    let limit = 256 * 1024;
    let canonical: ReplicationOrderV1 = norito::decode_canonical_with_limits(
        &order.canonical_order,
        norito::DecodeLimits::new(limit, limit, limit, limit * 4, 32),
    )
    .map_err(|_| rejected)?;
    if canonical.validate().is_err()
        || canonical.order_id != *binding.order_id.as_bytes()
        || canonical.manifest_digest != *binding.manifest_digest.as_bytes()
        || canonical.manifest_cid != pin.root_cid.as_bytes()
        || canonical.chunking_profile != pin.chunker.to_handle()
        || canonical.issued_at != order.issued_epoch
        || canonical.deadline_at != order.deadline_epoch
        || !canonical
            .assignments
            .iter()
            .any(|assignment| assignment.provider_id == *binding.provider_id.as_bytes())
    {
        return Err(rejected);
    }
    Ok(PublisherSourceAuthorizationV1 {
        deadline_epoch: order.deadline_epoch.min(pin.policy.retention_epoch),
        finalized_epoch,
    })
}

/// Verify both admissions, the requester's current owner/permission, exact assignment and pin.
/// Call before and after source I/O; the caller independently authenticates `authority`.
///
/// # Errors
/// Refuses unavailable durable finality, changed assignments, revoked authority and expired pins.
pub fn authorize_provider_source_v1(
    view: &StateView<'_>,
    authority: &AccountId,
    request: &SorafsAssignedSourceRequestV1,
    now_secs: u64,
) -> Result<(), ProviderSourceAuthorizationErrorV1> {
    let rejected = ProviderSourceAuthorizationErrorV1;
    if now_secs == 0
        || request.floor_height == 0
        || request.floor_block_hash == [0; 32]
        || request.assignment_revision == 0
        || request.source_provider == request.target_provider
        || [
            request.target_provider,
            request.source_provider,
            request.order_id,
            request.manifest_digest,
        ]
        .contains(&[0; 32])
    {
        return Err(rejected);
    }
    verify_signer_finality_v1(
        view,
        view.height() as u64,
        *view.latest_block_hash().ok_or(rejected)?.as_ref(),
    )
    .map_err(|_| rejected)?;
    verify_signer_finality_v1(view, request.floor_height, request.floor_block_hash)
        .map_err(|_| rejected)?;
    let world = view.world();
    let target = ProviderId::new(request.target_provider);
    if world.provider_owners().get(&target) != Some(authority) {
        return Err(rejected);
    }
    let permission = |permission: &iroha_data_model::permission::Permission| {
        permission.name() == "CanCompleteSorafsReplicationOrder"
            && permission.payload().get().as_str() == "null"
    };
    if !world
        .account_permissions()
        .get(authority)
        .is_some_and(|permissions| permissions.iter().any(permission))
        && !world
            .account_roles_iter(authority)
            .filter_map(|id| world.roles().get(id))
            .any(|role| role.permissions().any(permission))
    {
        return Err(rejected);
    }
    for provider in [target, ProviderId::new(request.source_provider)] {
        read_finalized_provider_admission_v1(view, provider, now_secs)
            .map_err(|_| rejected)?
            .ok_or(rejected)?;
    }
    let pin = world
        .pin_manifests()
        .get(&ManifestDigest::new(request.manifest_digest))
        .ok_or(rejected)?;
    let order = world
        .replication_orders()
        .get(&ReplicationOrderId::new(request.order_id))
        .ok_or(rejected)?;
    let finalized_secs = view.authenticated_query_ledger_time_ms().ok_or(rejected)? / 1_000;
    let now = now_secs.max(finalized_secs);
    if !matches!(pin.status, PinStatus::Approved(_))
        || pin.policy.retention_epoch <= now
        || !matches!(order.status, ReplicationOrderStatus::Pending)
        || order.provider_completion(target).is_some()
        || order.assignment_revision != request.assignment_revision
        || order.manifest_digest != pin.digest
        || order.manifest_root_cid != pin.root_cid
        || order.issued_epoch > finalized_secs
        || order.deadline_epoch < now
    {
        return Err(rejected);
    }
    let limit = 256 * 1024;
    let canonical: ReplicationOrderV1 = norito::decode_canonical_with_limits(
        &order.canonical_order,
        norito::DecodeLimits::new(limit, limit, limit, limit * 4, 32),
    )
    .map_err(|_| rejected)?;
    if canonical.validate().is_err()
        || canonical.order_id != request.order_id
        || canonical.manifest_digest != request.manifest_digest
        || canonical.manifest_cid != pin.root_cid.as_bytes()
        || canonical.chunking_profile != pin.chunker.to_handle()
        || canonical.issued_at != order.issued_epoch
        || canonical.deadline_at != order.deadline_epoch
        || [request.target_provider, request.source_provider]
            .iter()
            .any(|provider| {
                !canonical
                    .assignments
                    .iter()
                    .any(|assignment| assignment.provider_id == *provider)
            })
    {
        return Err(rejected);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        kura::Kura,
        query::store::LiveQueryStore,
        state::{State, World},
    };
    use iroha_crypto::{Hash, HashOf, KeyPair};
    use iroha_data_model::{NetworkId, block::BlockHeader};

    #[test]
    fn publisher_and_assigned_sources_require_durable_native_finality() {
        let network = NetworkId::from_genesis_hash(HashOf::<BlockHeader>::from_untyped_unchecked(
            Hash::prehashed([7; 32]),
        ));
        let state = State::new_with_chain_and_network_id_for_testing(
            World::default(),
            Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test(),
            "source-authority-test".parse().unwrap(),
            network,
        );
        let account = AccountId::new(KeyPair::random().public_key().clone());
        let binding = PublisherSourceBindingV1 {
            provider_id: ProviderId::new([1; 32]),
            order_id: ReplicationOrderId::new([2; 32]),
            manifest_digest: ManifestDigest::new([3; 32]),
            assignment_revision: 1,
        };
        let view = state.view();
        assert_eq!(
            authorize_publisher_source_v1(&view, &account, &binding, 1),
            Err(ProviderSourceAuthorizationErrorV1)
        );
        let request = SorafsAssignedSourceRequestV1 {
            target_provider: [1; 32],
            source_provider: [4; 32],
            order_id: [2; 32],
            assignment_revision: 1,
            manifest_digest: [3; 32],
            chunk_index: None,
            floor_height: 1,
            floor_block_hash: [7; 32],
        };
        assert_eq!(
            authorize_provider_source_v1(&view, &account, &request, 1),
            Err(ProviderSourceAuthorizationErrorV1)
        );
    }
}
