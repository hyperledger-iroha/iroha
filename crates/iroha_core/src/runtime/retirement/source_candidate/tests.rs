//! Original root cuts and fail-closed source joins; census fixtures confer no authority.

use super::*;
use crate::{
    state::{State, World},
    sumeragi::test_chain::{CertifiedTestChain, TestChainConfig},
};
use iroha_data_model::{
    NetworkId,
    private_dataspace::{
        PrivateDataspaceAdmissionPolicy, PrivateDataspaceAnchor, PrivateDataspaceRegistration,
    },
    sumeragi_finality::authenticated_genesis,
};
use iroha_model_base::topology::DataSpaceId;
use sha2::{Digest as _, Sha256};
use source_ownership::{SourceManifestV1, SourceReadLimits, SourceRemovalRequestV1, SourceUnitV1};

mod certified;

fn state(world: World) -> State {
    State::new_for_testing(
        world,
        crate::kura::Kura::blank_kura_for_testing(),
        crate::query::store::LiveQueryStore::start_test(),
    )
}

fn native() -> (DefinitionId, DefinitionDescriptorV1, ImplementationCodeV1) {
    let catalog = crate::runtime::release_catalog().unwrap();
    let (id, descriptor) = catalog
        .definitions
        .iter()
        .find(|(_, value)| matches!(code(value), Some(ImplementationCodeV1::Native { .. })))
        .unwrap();
    (*id, descriptor.clone(), code(descriptor).unwrap().clone())
}

fn root() -> RuntimeRetirementRootV1 {
    RuntimeRetirementRootV1 {
        network: NetworkId::from_genesis_hash(iroha_crypto::HashOf::from_untyped_unchecked(
            Hash::new(b"source census fixture"),
        )),
        scope: SumeragiRootScope::Global,
        instance: [1; 32],
    }
}

#[test]
fn child_only_aliases_extend_the_exact_union_and_shared_source_cannot_hide_them() {
    let (compiled_id, descriptor, owner) = native();
    let mut child_descriptor = descriptor.clone();
    let DefinitionDescriptorV1::Implementation(child) = &mut child_descriptor else {
        unreachable!()
    };
    child.limits.max_gas = if child.limits.max_gas > 1 {
        child.limits.max_gas - 1
    } else {
        2
    };
    let child_id = descriptor_identity(&child_descriptor).unwrap();
    assert_ne!(child_id, compiled_id);
    let compiled = BTreeMap::from([(compiled_id, descriptor.clone())]);
    let parent = state(World::new());
    let child_world = World::new();
    let mut block = child_world.block();
    block.runtime_definitions.insert(compiled_id, descriptor);
    block.runtime_definitions.insert(child_id, child_descriptor);
    block.commit();
    let child = state(child_world);
    let parent = parent.query_view();
    let children = [child.query_view()];
    // Only the internal alias enumerator sees this unauthenticated census fixture.
    // The authority constructor below refuses these views without a native execution tip.
    let views = std::iter::once(&parent).chain(children.iter());
    let mut observed = Vec::new();
    assert_eq!(
        visit_aliases(&owner, &compiled, views.clone(), |id| {
            observed.push(id);
            Ok(())
        })
        .unwrap(),
        2
    );
    assert_eq!(observed, [compiled_id, child_id]);
    let error = visit_aliases(&owner, &compiled, views.clone(), |id| {
        if id == child_id {
            Err(invalid("child-only alias lacks retirement history").into())
        } else {
            Ok(())
        }
    })
    .unwrap_err();
    assert!(error.to_string().contains("child-only alias"));
    assert!(original_root(&parent).is_err());
    assert!(original_root(&children[0]).is_err());
    let mut foreign = owner;
    let ImplementationCodeV1::Native {
        reference_closure, ..
    } = &mut foreign
    else {
        unreachable!()
    };
    reference_closure[0] ^= 1;
    assert!(visit_aliases(&foreign, &compiled, views.clone(), |_| unreachable!()).is_err());
}

#[test]
fn descriptor_census_streams_the_frozen_identity_and_rejects_substitution() {
    let (id, descriptor, _) = native();
    assert_eq!(
        descriptor_identity(&descriptor).unwrap(),
        descriptor.id().unwrap()
    );
    validate_definitions(std::iter::once((&id, &descriptor))).unwrap();
    let mut wrong = id;
    wrong.0[0] ^= 1;
    assert!(validate_definitions(std::iter::once((&wrong, &descriptor))).is_err());
}

#[test]
fn retained_migration_and_original_reader_deferral_block_source_evidence() {
    use iroha_data_model::runtime::definition::{
        CarrierNodeV1, CarrierSchemaV1, MigrationDescriptorV1, SchemaV1,
    };
    let (id, descriptor, owner) = native();
    let DefinitionDescriptorV1::Implementation(implementation) = &descriptor else {
        unreachable!()
    };
    let migration = DefinitionDescriptorV1::Migration(MigrationDescriptorV1 {
        code: owner.clone(),
        from: vec![id],
        to: vec![id],
        cursor: SchemaV1::Carrier(CarrierSchemaV1 {
            nodes: vec![CarrierNodeV1::Unit],
        }),
        host_profile: None,
        services_profile: implementation.services_profile,
        limits: implementation.limits,
    });
    let migration_id = descriptor_identity(&migration).unwrap();
    let compiled = BTreeMap::from([(id, descriptor)]);
    let parent = state(World::new());
    let child_world = World::new();
    let mut block = child_world.block();
    block.runtime_definitions.insert(migration_id, migration);
    block.commit();
    let child = state(child_world);
    let parent = parent.query_view();
    let children = [child.query_view()];
    let views = std::iter::once(&parent).chain(children.iter());
    assert!(
        visit_aliases(&owner, &compiled, views.clone(), |_| Ok(()))
            .unwrap_err()
            .to_string()
            .contains("retained migration")
    );
    let original = Attempt::Deferred(ivm::ExecutionDeferral::LocalInvariantViolation.into());
    let mut owner_once = Some(original);
    let refusal = visit_aliases(&owner, &compiled, views.clone(), |_| {
        Err(owner_once.take().unwrap())
    })
    .unwrap_err();
    let CandidateError::Evidence(Attempt::Deferred(refusal)) = CandidateError::from(refusal) else {
        panic!("original reader refusal must remain unfinished")
    };
    assert_eq!(
        refusal.reason(),
        ivm::ExecutionDeferral::LocalInvariantViolation
    );
    assert!(owner_once.is_none());
}

fn source_fixture(
    owner: ImplementationCodeV1,
) -> (tempfile::TempDir, SourceManifestV1, SourceRemovalRequestV1) {
    let directory = tempfile::tempdir().unwrap();
    let bytes = b"fn reviewed_native_owner() {}\n";
    std::fs::write(directory.path().join("handler.rs"), bytes).unwrap();
    let manifest = SourceManifestV1 {
        version: 1,
        units: vec![SourceUnitV1 {
            path: "handler.rs".into(),
            bytes: bytes.len() as u64,
            sha256: Sha256::digest(bytes).into(),
            native_owners: vec![owner.clone()],
            retained_uses: Vec::new(),
        }],
    };
    let request = SourceRemovalRequestV1 {
        paths: vec!["handler.rs".into()],
        native_owners: vec![owner],
    };
    (directory, manifest, request)
}

#[test]
fn source_join_requires_original_execution_and_rechecks_source_before_evidence() {
    let (_, _, owner) = native();
    let (directory, manifest, request) = source_fixture(owner);
    let sources = source_ownership::check_sources(
        directory.path(),
        &manifest,
        source_ownership::manifest_digest(&manifest).unwrap(),
        &request,
        SourceReadLimits {
            max_file_bytes: 4096,
            max_total_bytes: 4096,
        },
    )
    .unwrap();
    let parent = state(World::new());
    assert!(matches!(
        parent.runtime_retirement_cut(),
        Err(crate::state::RetirementCutError::Unexecuted)
    ));
    // The retained native file owner detects changed custody before the digest
    // comparison. Preserve that original I/O refusal instead of reclassifying it.
    std::fs::write(directory.path().join("handler.rs"), b"substituted").unwrap();
    assert!(matches!(
        sources.recheck_preimages(),
        Err(source_ownership::Error::Io { path, source })
            if path == "handler.rs" && source.kind() == std::io::ErrorKind::Other
    ));
}

fn register_child(chain: &CertifiedTestChain, dataspace: DataSpaceId) -> PrivateDataspaceRegistry {
    let epoch = authenticated_genesis(chain.genesis())
        .unwrap()
        .into_parts()
        .0;
    let registration = PrivateDataspaceRegistration::new(
        chain.root_scope(),
        chain.state().query_view().chain_id().clone(),
        chain.network_id(),
        chain.committed(1).result().0,
        epoch,
    )
    .unwrap();
    let mut registry = PrivateDataspaceRegistry::default();
    registry
        .register_authorized(
            PrivateDataspaceAdmissionPolicy {
                max_registered_roots: 1,
                max_roots_per_owner: 1,
            },
            "source-child".into(),
            chain.genesis_account().clone(),
            1,
            registration,
        )
        .unwrap();
    for height in 2..=chain.height() {
        anchor_child(&mut registry, chain, dataspace, height);
    }
    registry
}

fn anchor_child(
    registry: &mut PrivateDataspaceRegistry,
    chain: &CertifiedTestChain,
    dataspace: DataSpaceId,
    height: u64,
) {
    let committed = chain.committed(height);
    let anchor = PrivateDataspaceAnchor::from_certificate(
        registry.records()[0].anchor.registration(),
        committed.block().commit_certificate().unwrap(),
    )
    .unwrap();
    registry
        .apply_authorized(dataspace, chain.genesis_account(), 1, &anchor)
        .unwrap();
}

#[test]
fn original_child_cut_requires_complete_membership_and_exact_current_registered_decision() {
    std::thread::Builder::new()
        .stack_size(32 * 1024 * 1024)
        .spawn(|| {
            let parent = root().network;
            let dataspace = DataSpaceId::new((1_u64 << 43) + 17);
            let config =
                crate::runtime::child_admin::certified_tests::config_for_root(parent, dataspace);
            let mut chain = CertifiedTestChain::start(config).unwrap();
            let genesis = chain.state().query_view();
            let mut registry = register_child(&chain, dataspace);
            assert!(
                bind_child(parent, &registry.records()[0], &genesis).is_err(),
                "a signed genesis alone does not establish non-genesis retirement authority"
            );
            drop(genesis);
            chain.commit_at(2_000, vec![]); // The fixture signs actual clock work; no empty block bypass.
            anchor_child(&mut registry, &chain, dataspace, 2);
            let state_owner = chain.state().clone();
            let old_view = state_owner.query_view();
            bind_child(parent, &registry.records()[0], &old_view).unwrap();
            require_child_count(&registry, 1).unwrap();
            assert!(require_child_count(&registry, 0).is_err());
            assert!(require_child_count(&registry, 2).is_err());
            assert!(bind_child(chain.network_id(), &registry.records()[0], &old_view).is_err());
            let foreign = state(World::new());
            assert!(bind_child(parent, &registry.records()[0], &foreign.query_view()).is_err());
            chain.commit_at(3_000, vec![]);
            let current = state_owner.query_view();
            assert!(
                bind_child(parent, &registry.records()[0], &current).is_err(),
                "unanchored future cut"
            );
            anchor_child(&mut registry, &chain, dataspace, 3);
            assert!(
                bind_child(parent, &registry.records()[0], &old_view).is_err(),
                "stale child cut"
            );
            bind_child(parent, &registry.records()[0], &current).unwrap();
        })
        .unwrap()
        .join()
        .unwrap();
}
