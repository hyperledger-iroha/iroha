//! Exact live native replication authority for initial provider source reads.
use crate::{
    query::{
        provider_admission::{
            read_finalized_provider_admission_v1, read_provider_admission_at_native_current_v1,
        },
        signer_check::{
            SignerCertifiedBlockV1, SignerCertifiedWalkV1, with_native_check_read_limits,
        },
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
use iroha_executor_data_model::permission::sorafs::CanCompleteSorafsReplicationOrder;
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
        || (&order.manifest_digest, &order.manifest_root_cid) != (&pin.digest, &pin.root_cid)
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

/// Verify both admissions, the requester's exact governed completion signer and scoped permission, exact assignment and pin.
/// Call before and after source I/O; the caller independently authenticates `authority`.
/// One exact-view native reader covers the complete check under the existing native Check limits
/// (64 MiB of source frames and 256 MiB of decoding allocations), retaining narrower outer limits.
/// Unavailable or over-budget history fails closed, including an old requested floor.
///
/// # Errors
/// Refuses unavailable durable finality, changed assignments, revoked authority and expired pins.
pub fn authorize_provider_source_v1(
    view: &StateView<'_>,
    authority: &AccountId,
    request: &SorafsAssignedSourceRequestV1,
    now_secs: u64,
) -> Result<(), ProviderSourceAuthorizationErrorV1> {
    with_native_check_read_limits(|| {
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
        // One original-view reader and cumulative allowance cover this complete authorization.
        // Fresh HTTP/read checks construct fresh readers; no authority survives a State view.
        let reader = SignerCertifiedWalkV1::new(view).map_err(|_| rejected)?;
        let current = read_provider_source_cut_v1(
            view,
            &reader,
            request.floor_height,
            request.floor_block_hash,
        )?;
        let world = view.world();
        let target = ProviderId::new(request.target_provider);
        let completion = world
            .provider_ingest_completion_authorities()
            .get(&target)
            .ok_or(rejected)?;
        if !completion.is_valid()
            || world.provider_owners().get(&target) != Some(&completion.provider_owner)
            || authority != &completion.completion_signer
            || !has_provider_completion_permission_v1(world, authority, target)
        {
            return Err(rejected);
        }
        for provider in [target, ProviderId::new(request.source_provider)] {
            read_provider_admission_at_native_current_v1(view, &current, provider, now_secs)
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
            || (&order.manifest_digest, &order.manifest_root_cid) != (&pin.digest, &pin.root_cid)
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
    })
}

// The caller retains the reader and cumulative decoder scope through all admission checks.
// Both receipts must come from this exact view; genesis additionally needs its certified H2.
fn read_provider_source_cut_v1<'view, 'state>(
    view: &'view StateView<'state>,
    reader: &SignerCertifiedWalkV1<'view, 'state>,
    floor_height: u64,
    floor_block_hash: [u8; 32],
) -> Result<SignerCertifiedBlockV1<'view, 'state>, ProviderSourceAuthorizationErrorV1> {
    let rejected = ProviderSourceAuthorizationErrorV1;
    let height = u64::try_from(view.height()).map_err(|_| rejected)?;
    let current_hash = view.latest_block_hash().ok_or(rejected)?;
    if floor_height == 0 || floor_height > height || floor_block_hash == [0; 32] {
        return Err(rejected);
    }
    let current = reader
        .walk(height, height)
        .next()
        .ok_or(rejected)?
        .map_err(|_| rejected)?;
    let block = current.in_view(view).map_err(|_| rejected)?;
    if block.height() != height || block.block_hash() != current_hash {
        return Err(rejected);
    }
    if floor_height == height {
        if *block.block_hash().as_ref() != floor_block_hash {
            return Err(rejected);
        }
    } else {
        let floor = reader
            .walk(floor_height, floor_height)
            .next()
            .ok_or(rejected)?
            .map_err(|_| rejected)?;
        let block = floor.in_view(view).map_err(|_| rejected)?;
        if block.height() != floor_height || *block.block_hash().as_ref() != floor_block_hash {
            return Err(rejected);
        }
    }
    Ok(current)
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

    use crate::sumeragi::test_chain::{CertifiedTestChain, Signers, TestChainConfig};

    fn native_chain(height: u64) -> CertifiedTestChain {
        let mut chain =
            CertifiedTestChain::start(TestChainConfig::new(World::new(), 1_000)).unwrap();
        for next in 2..=height {
            chain.commit_at(next * 1_000, Vec::new());
        }
        chain
    }

    fn source_cut(
        view: &StateView<'_>,
        height: u64,
        hash: [u8; 32],
    ) -> Result<(), ProviderSourceAuthorizationErrorV1> {
        with_native_check_read_limits(|| {
            let reader =
                SignerCertifiedWalkV1::new(view).map_err(|_| ProviderSourceAuthorizationErrorV1)?;
            read_provider_source_cut_v1(view, &reader, height, hash).map(|_| ())
        })
    }

    #[test]
    fn provider_source_cut_preserves_exact_finality_and_genesis_successor() {
        let mut chain = native_chain(1);
        let genesis_hash = *chain.genesis().hash().as_ref();
        assert!(source_cut(&chain.state().view(), 1, genesis_hash).is_err());
        assert!(verify_signer_finality_v1(&chain.state().view(), 1, genesis_hash).is_err());
        chain.commit_at(2_000, Vec::new());
        assert!(source_cut(&chain.state().view(), 1, genesis_hash).is_ok());
        chain.commit_at(3_000, Vec::new());
        let view = chain.state().view();
        for height in 1..=3 {
            let hash = *chain.committed(height).block_hash().as_ref();
            let original = verify_signer_finality_v1(&view, height, hash).unwrap();
            with_native_check_read_limits(|| {
                let reader = SignerCertifiedWalkV1::new(&view).unwrap();
                let current = read_provider_source_cut_v1(&view, &reader, height, hash).unwrap();
                let certified = current.in_view(&view).unwrap();
                assert_eq!(certified.height(), 3);
                assert_eq!(certified.block_hash(), chain.committed(3).block_hash());
                assert_eq!(original.context_id(), chain.committed(height).id());
            });
        }
        for (height, hash) in [
            (0, genesis_hash),
            (4, genesis_hash),
            (1, [0; 32]),
            (1, *chain.committed(2).block_hash().as_ref()),
            (3, genesis_hash),
        ] {
            assert!(source_cut(&view, height, hash).is_err());
        }
    }

    #[test]
    fn provider_source_cut_refuses_foreign_views_and_exhausted_outer_budget() {
        let chain = native_chain(2);
        let view = chain.state().view();
        let foreign_view = chain.state().view();
        let hash = *chain.genesis().hash().as_ref();
        with_native_check_read_limits(|| {
            let reader = SignerCertifiedWalkV1::new(&view).unwrap();
            assert!(read_provider_source_cut_v1(&foreign_view, &reader, 1, hash).is_err());
        });
        let zero =
            norito::DecodeLimits::new(64 * 1024 * 1024, 64 * 1024 * 1024, 64 * 1024 * 1024, 0, 128);
        assert!(norito::with_decode_limits_scope(zero, || source_cut(&view, 1, hash)).is_err());
        assert!(
            source_cut(&view, 1, hash).is_ok(),
            "a fresh bounded attempt remains valid"
        );
    }

    #[test]
    fn provider_source_cut_refuses_missing_frames_and_corrupt_quorums() {
        let chain = native_chain(2);
        let mut missing = State::new_with_chain_and_network_id_for_testing(
            World::default(),
            Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test(),
            chain.state().chain_id_ref().clone(),
            chain.network_id(),
        );
        for height in 1..=2 {
            missing.push_block_hash_for_testing(chain.committed(height).block_hash());
        }
        assert!(source_cut(&missing.view(), 1, *chain.genesis().hash().as_ref()).is_err());
        for corrupt_height in [2, 3] {
            let chain = native_chain(3);
            let hash = *chain.committed(2).block_hash().as_ref();
            chain.corrupt_local_quorum_for_test(corrupt_height, Signers::BelowQuorum);
            assert!(source_cut(&chain.state().view(), 2, hash).is_err());
            if corrupt_height == 2 {
                assert!(
                    source_cut(&chain.state().view(), 1, *chain.genesis().hash().as_ref()).is_err()
                );
            }
        }
        let chain = native_chain(3);
        let hash = *chain.committed(2).block_hash().as_ref();
        chain
            .kura()
            .corrupt_canonical_body_for_testing(std::num::NonZeroUsize::new(2).unwrap())
            .unwrap();
        assert!(source_cut(&chain.state().view(), 2, hash).is_err());
    }

    // Both distinct providers are authored through the original signed genesis initializer.
    fn native_admissions(
        retention_offset: u64,
    ) -> (CertifiedTestChain, [ProviderId; 2], AccountId) {
        use crate::smartcontracts::isi::sorafs_provider_admission::test_fixture::{
            NOW, ProviderAdmissionTestFixtureV1 as Fixture, key,
        };
        use iroha_data_model::{
            isi::sorafs::InitializeSorafsProviderAdmissionV1,
            sorafs::provider_admission::governance::{
                InitialProviderAdmissionCouncilV1, InitialProviderAdmissionV1,
            },
        };
        use sorafs_manifest::provider_admission::ProviderAdmissionGenesisMaterialV1;
        let fixture = Fixture::new();
        let owner = AccountId::new(key(1).public_key().clone());
        let providers = [ProviderId::new([0xA1; 32]), ProviderId::new([0xA2; 32])];
        let initialize = InitializeSorafsProviderAdmissionV1 {
            council: InitialProviderAdmissionCouncilV1 {
                policy_id: fixture.policy.policy_id,
                trusted_signers: fixture.policy.trusted_signers.clone(),
                signature_threshold: 1,
            },
            providers: providers
                .iter()
                .map(|provider| {
                    let mut material = ProviderAdmissionGenesisMaterialV1 {
                        proposal: fixture.envelope.proposal.clone(),
                        advert_body: fixture.envelope.advert_body.clone(),
                        issued_at: NOW,
                        retention_epoch: NOW + retention_offset,
                    };
                    material.proposal.provider_id = *provider.as_bytes();
                    material.advert_body.provider_id = *provider.as_bytes();
                    InitialProviderAdmissionV1 {
                        owner: owner.clone(),
                        material: norito::encode_canonical(&material).unwrap(),
                    }
                })
                .collect(),
        };
        let mut config = TestChainConfig::new(World::new(), NOW * 1_000);
        config.genesis_key = key(1);
        config.genesis_instructions = vec![initialize.into()];
        (CertifiedTestChain::start(config).unwrap(), providers, owner)
    }

    #[test]
    fn provider_source_current_receipt_preserves_signed_genesis_admission_and_expiry() {
        use crate::smartcontracts::isi::sorafs_provider_admission::test_fixture::NOW;
        let (mut chain, providers, owner) = native_admissions(2);
        let provider = providers[0];
        for (offset, valid) in [(1, true), (2, false)] {
            chain.commit_at((NOW + offset) * 1_000, Vec::new());
            let view = chain.state().view();
            let original = read_finalized_provider_admission_v1(&view, provider, NOW);
            with_native_check_read_limits(|| {
                let reader = SignerCertifiedWalkV1::new(&view).unwrap();
                let current = read_provider_source_cut_v1(
                    &view,
                    &reader,
                    1,
                    *chain.genesis().hash().as_ref(),
                )
                .unwrap();
                let actual =
                    read_provider_admission_at_native_current_v1(&view, &current, provider, NOW);
                if valid {
                    let original = original.unwrap().unwrap();
                    let actual = actual.unwrap().unwrap();
                    assert_eq!(actual.envelope(), original.envelope());
                    assert_eq!(actual.is_genesis_material(), original.is_genesis_material());
                    assert!(
                        read_provider_admission_at_native_current_v1(
                            &view,
                            &current,
                            ProviderId::new([0xAF; 32]),
                            NOW,
                        )
                        .unwrap()
                        .is_none()
                    );
                } else {
                    assert!(original.is_err());
                    assert!(
                        actual.is_err(),
                        "finalized expiry overrides a lagging local clock"
                    );
                }
            });
            let request = SorafsAssignedSourceRequestV1 {
                target_provider: *provider.as_bytes(),
                source_provider: *providers[1].as_bytes(),
                order_id: [2; 32],
                assignment_revision: 1,
                manifest_digest: [3; 32],
                chunk_index: None,
                floor_height: 1,
                floor_block_hash: *chain.genesis().hash().as_ref(),
            };
            assert!(
                authorize_provider_source_v1(&view, &owner, &request, NOW).is_err(),
                "admission and valid finality alone do not confer completion or assignment authority"
            );
        }
    }

    /// Compare only exact authorization read work, not full order authorization or HTTP.
    /// Run with `cargo test -p iroha_core --lib provider_source_read_work_benchmark -- --ignored --nocapture`.
    /// Setup is outside timing; ABBA order shares the original view, providers and floor.
    #[test]
    #[ignore = "manual native history read-work comparison"]
    fn provider_source_read_work_benchmark() {
        use crate::smartcontracts::isi::sorafs_provider_admission::test_fixture::NOW;
        use std::time::Instant;
        let (mut chain, providers, _) = native_admissions(3_600);
        for height in 2..=26 {
            chain.commit_at((NOW + height) * 1_000, Vec::new());
        }
        for head in [26, 27] {
            if head == 27 {
                chain.commit_at((NOW + head) * 1_000, Vec::new());
            }
            let view = chain.state().view();
            let head_hash = *chain.committed(head).block_hash().as_ref();
            for floor in [1, head - 1] {
                let floor_hash = *chain.committed(floor).block_hash().as_ref();
                let mut expected = None;
                for (sample, local) in [false, true, true, false, false, true, true, false]
                    .into_iter()
                    .enumerate()
                {
                    let budget = view.execution_budget();
                    let before = budget.reserved_bytes();
                    let started = Instant::now();
                    let records = if local {
                        with_native_check_read_limits(|| {
                            let reader = SignerCertifiedWalkV1::new(&view).unwrap();
                            let current =
                                read_provider_source_cut_v1(&view, &reader, floor, floor_hash)
                                    .unwrap();
                            providers.map(|provider| {
                                read_provider_admission_at_native_current_v1(
                                    &view, &current, provider, NOW,
                                )
                                .unwrap()
                                .unwrap()
                            })
                        })
                    } else {
                        verify_signer_finality_v1(&view, head, head_hash).unwrap();
                        verify_signer_finality_v1(&view, floor, floor_hash).unwrap();
                        providers.map(|provider| {
                            read_finalized_provider_admission_v1(&view, provider, NOW)
                                .unwrap()
                                .unwrap()
                        })
                    };
                    let nanos = started.elapsed().as_nanos();
                    assert_eq!(budget.reserved_bytes(), before, "original permits refunded");
                    let bytes =
                        records.map(|record| norito::encode_canonical(record.envelope()).unwrap());
                    if let Some(expected) = &expected {
                        assert_eq!(&bytes, expected, "exact admission envelope bytes");
                    } else {
                        expected = Some(bytes);
                    }
                    println!(
                        "provider_source_read_work head={head} floor={floor} providers=2 sample={sample} path={} wall_ns={nanos} reserved_before={before} reserved_after={}",
                        if local { "scoped" } else { "original" },
                        budget.reserved_bytes(),
                    );
                }
            }
        }
    }

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

/// Check one canonical provider-scoped completion grant, including inherited roles.
/// This is a permission check only; callers separately authenticate current full authority.
pub fn has_provider_completion_permission_v1(
    world: &impl WorldReadOnly,
    account: &AccountId,
    provider: ProviderId,
) -> bool {
    if provider.as_bytes() == &[0; 32] {
        return false;
    }
    let required =
        iroha_data_model::permission::Permission::from(CanCompleteSorafsReplicationOrder {
            provider_id: provider,
        });
    world
        .account_permissions()
        .get(account)
        .is_some_and(|permissions| permissions.contains(&required))
        || world
            .account_roles_iter(account)
            .filter_map(|id| world.roles().get(id))
            .any(|role| role.permissions().any(|permission| permission == &required))
}

#[cfg(test)]
mod completion_permission_tests {
    use super::*;
    use crate::state::World;
    use iroha_data_model::{
        IntoKeyValue, Registrable,
        permission::{Permission, Permissions},
        role::{Role, RoleId, RoleIdWithOwner},
    };
    use iroha_primitives::json::Json;

    #[test]
    fn completion_permission_is_exact_provider_scoped_and_inherits_roles() {
        let signer = AccountId::new(iroha_crypto::KeyPair::random().public_key().clone());
        let provider = ProviderId::new([1; 32]);
        let other = ProviderId::new([2; 32]);
        let exact: Permission = CanCompleteSorafsReplicationOrder {
            provider_id: provider,
        }
        .into();
        let mut world = World::default();
        let (account_id, account) = iroha_data_model::account::Account::new(signer.clone())
            .build(&signer)
            .into_key_value();
        world.accounts.insert(account_id, account);
        for permission in [
            Permission::new(exact.name().to_owned(), Json::new(())),
            CanCompleteSorafsReplicationOrder { provider_id: other }.into(),
        ] {
            world
                .account_permissions
                .insert(signer.clone(), Permissions::from([permission]));
            assert!(!has_provider_completion_permission_v1(
                &world.view(),
                &signer,
                provider
            ));
        }
        world
            .account_permissions
            .insert(signer.clone(), Permissions::from([exact.clone()]));
        assert!(has_provider_completion_permission_v1(
            &world.view(),
            &signer,
            provider
        ));
        assert!(!has_provider_completion_permission_v1(
            &world.view(),
            &signer,
            other
        ));
        assert!(!has_provider_completion_permission_v1(
            &world.view(),
            &signer,
            ProviderId::new([0; 32])
        ));
        let mut permissions = world.account_permissions.block();
        assert!(permissions.remove(signer.clone()).is_some());
        permissions.commit();
        let role: RoleId = "dedicated_provider_completion".parse().unwrap();
        world.roles.insert(
            role.clone(),
            Role::new(role.clone(), signer.clone())
                .add_permission(exact)
                .build(&signer),
        );
        world
            .account_roles
            .insert(RoleIdWithOwner::new(signer.clone(), role), ());
        assert!(has_provider_completion_permission_v1(
            &world.view(),
            &signer,
            provider
        ));
        assert!(!has_provider_completion_permission_v1(
            &world.view(),
            &signer,
            other
        ));
    }
}
