// Runtime setter tests exercise catalog authority independently of activation committee eligibility.
use crate::query::store::LiveQueryStore;
use iroha_data_model::nexus::{
    DataSpaceMetadata, NexusRuntimeCatalogV1, RuntimeDataSpaceAdditionV1, dataspace_catalog_hash,
};

fn run_runtime_configuration_test(test: impl FnOnce() + Send + 'static) {
    let result = std::thread::Builder::new()
        .name("runtime-dataspace-setter".to_owned())
        .stack_size(32 * 1024 * 1024)
        .spawn(test)
        .expect("bounded debug State fixture")
        .join();
    if let Err(error) = result {
        std::panic::resume_unwind(error);
    }
}

fn additional_dataspace() -> RuntimeDataSpaceAdditionV1 {
    let manifest_hash = [0x67; 32];
    RuntimeDataSpaceAdditionV1 {
        descriptor: DataSpaceMetadata {
            id: DataSpaceId::from_hash(&manifest_hash),
            alias: "runtime-dataspace".to_owned(),
            description: None,
            fault_tolerance: 1,
        },
        manifest_hash,
    }
}

#[test]
fn runtime_nexus_setter_preserves_configured_dataspaces_and_rejects_post_genesis_drift() {
    run_runtime_configuration_test(|| {
        let mut state = State::new_for_testing(
            World::new(),
            Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test(),
        );
        let original = state.nexus_snapshot();
        let mut expanded = original.dataspace_catalog.entries().to_vec();
        expanded.push(additional_dataspace().descriptor);
        let expanded = DataSpaceCatalog::new(expanded).unwrap();
        let mut requested = original.clone();
        requested.configured_dataspace_catalog = expanded.clone();
        state
            .set_nexus(requested)
            .expect("ordinary setter preserves baseline");
        assert_eq!(
            state.nexus_snapshot().configured_dataspace_catalog,
            original.configured_dataspace_catalog
        );

        let mut hashes = state.block_hashes.block();
        hashes.push(HashOf::<BlockHeader>::from_untyped_unchecked(
            Hash::prehashed([0x23; 32]),
        ));
        hashes.commit();
        let mut requested = state.nexus_snapshot();
        requested.dataspace_catalog = expanded;
        assert!(matches!(
            state.set_nexus(requested),
            Err(LaneLifecycleError::RuntimeCatalog(_))
        ));
        assert_eq!(
            state.nexus_snapshot().dataspace_catalog,
            original.dataspace_catalog
        );
        assert_eq!(
            state.nexus_snapshot().configured_dataspace_catalog,
            original.configured_dataspace_catalog
        );
    });
}

#[test]
fn runtime_nexus_setter_requires_exact_protected_dataspace_projection() {
    run_runtime_configuration_test(|| {
        let mut state = State::new_for_testing(
            World::new(),
            Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test(),
        );
        let original = state.nexus_snapshot();
        let runtime = NexusRuntimeCatalogV1 {
            version: NexusRuntimeCatalogV1::VERSION,
            baseline_dataspaces_hash: dataspace_catalog_hash(
                &original.configured_dataspace_catalog,
            ),
            baseline_manifests_hash: Hash::prehashed(
                state
                    .lane_manifests
                    .read()
                    .baseline_consensus_policy_digest(),
            ),
            dataspaces: vec![additional_dataspace()],
            manifests: vec![],
        };
        let expected =
            runtime_catalog_dataspaces(&original.configured_dataspace_catalog, Some(&runtime))
                .unwrap();
        let mut world = state.world.block();
        world
            .parameters
            .get_mut()
            .set_parameter(iroha_data_model::parameter::Parameter::Custom(
                runtime
                    .into_custom_parameter()
                    .expect("bounded cumulative DS fixture"),
            ));
        world.commit();
        assert!(
            state.set_nexus(original.clone()).is_err(),
            "omitting the committed DS must fail before mutation"
        );
        assert_eq!(
            state.nexus_snapshot().dataspace_catalog,
            original.dataspace_catalog
        );
        let mut requested = original.clone();
        requested.dataspace_catalog = expected.clone();
        state
            .set_nexus(requested)
            .expect("exact protected DS projection");
        assert_eq!(state.nexus_snapshot().dataspace_catalog, expected);
        assert_eq!(
            state.nexus_snapshot().configured_dataspace_catalog,
            original.configured_dataspace_catalog
        );
    });
}
