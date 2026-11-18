//! Exact C10 closure, scheduled cutover and family-preserving registration.
use super::*;
use crate::{
    manifest_signing_test_support::ManifestSigningFixture,
    smartcontracts::ivm::profile::{ResolvedIvmProfile, is_selected},
    state::{StorageReadOnly, World, WorldReadOnly},
    sumeragi::test_chain::{CertifiedTestChain, TestChainConfig},
};
use iroha_data_model::{
    block::execution_output::ExecutionOutputV1,
    isi::smart_contract_code::RegisterSmartContractBytes,
    ivm_c10::{ContractManifestC10V1, RegisterSmartContractCodeC10V1},
    smart_contract::ContractArtifactId,
};
use iroha_model_base::topology::DataSpaceId;

fn draft(chain: &CertifiedTestChain) -> ChangeSetV1 {
    let view = chain.state().view();
    let world = view.world();
    let published = world
        .runtime_definitions()
        .iter()
        .map(|(id, descriptor)| (*id, descriptor.clone()))
        .collect::<BTreeMap<_, _>>();
    release_catalog()
        .unwrap()
        .activation(
            additions::IVM_C10.module,
            world.runtime_selection(),
            &published,
            chain.network_id(),
            chain.height() + 2,
            [0x10; 32],
        )
        .unwrap()
}

#[test]
fn governed_c10_draft_activates_only_at_its_certified_boundary() {
    let mut chain = CertifiedTestChain::start(TestChainConfig::new(World::new(), 1000)).unwrap();
    let catalog = release_catalog().unwrap();
    let host = ResolvedIvmProfile::of_id(ivm::IvmProfileId::C10_ABI_V1);
    assert_eq!(
        hex::encode(catalog.baseline_id.0),
        "56c2328657d63a48e18c7d1bac5ae8f2406e6e71249a630ece7b504d0f9cd491"
    );
    let change = draft(&chain);
    let published = catalog
        .baseline_definitions
        .iter()
        .map(|id| (*id, catalog.definitions[id].clone()))
        .collect::<BTreeMap<_, _>>();
    let successor = change
        .apply(&catalog.baseline, &published)
        .unwrap()
        .selection;
    assert_eq!(successor.id().unwrap(), change.successor);
    validate_transition(&catalog.baseline, &successor).unwrap();
    let mut partial = successor.clone();
    partial.active.retain(|id| *id != host.definition_id());
    assert!(validate_transition(&catalog.baseline, &partial).is_err());
    let mut partial = successor.clone();
    partial
        .bindings
        .retain(|binding| binding.wire_id != RegisterSmartContractCodeC10V1::WIRE_ID);
    assert!(validate_transition(&catalog.baseline, &partial).is_err());
    let at = chain.committed(chain.height()).block_time_ms() + 1000;
    crate::runtime::ironwood::test_activation::schedule_change(&chain, at, change.clone());
    chain.commit_at(at, vec![]);
    assert!(!is_selected(chain.state().view().world(), host));
    chain.commit_at(at + 1000, vec![]);
    let view = chain.state().view();
    assert_eq!(*view.world().runtime_selection(), successor);
    assert!(is_selected(view.world(), host));
    assert!(is_selected(
        view.world(),
        ResolvedIvmProfile::interface_default()
    ));
}

#[test]
fn certified_c10_registration_keeps_exact_manifest_profile_and_result() {
    verify_certified_c10_registration(false);
}

#[test]
fn keyed_c10_registration_restores_the_certified_complete_state() {
    verify_certified_c10_registration(true);
}

fn verify_certified_c10_registration(keyed: bool) {
    let fixture = || TestChainConfig::new(World::new(), 1000);
    let config = fixture();
    let owner = config.genesis_key.clone();
    let mut chain = CertifiedTestChain::start(config).unwrap();
    let change = draft(&chain);
    let at = chain.committed(chain.height()).block_time_ms() + 1000;
    crate::runtime::ironwood::test_activation::schedule_change(&chain, at, change.clone());
    chain.commit_at(at, vec![]);
    chain.commit_at(at + 1000, vec![]);
    if keyed {
        // Activate the complete-State commitment through its two certified predecessors.
        // A World-only fixture overlay has no authority to edit an already keyed State.
        crate::runtime::keyed_state::activate_for_test(&mut chain);
    }
    let (code, manifest) = kotodama_lang::compiler::Compiler::new_c10()
        .compile_source_with_manifest(
            "seiyaku Counter { kotoage fn answer() authorize(anyone) { } }",
        )
        .unwrap();
    assert_eq!(
        crate::smartcontracts::ivm::profile::resolve_raw_artifact(
            chain.state().view().world(),
            &code
        )
        .id(),
        ivm::IvmProfileId::BASELINE_ABI_V1
    );
    assert_eq!(
        crate::smartcontracts::ivm::artifact_profile(chain.state().view().world(), &code),
        ivm::IvmProfileId::C10_ABI_V1
    );
    let key = ContractArtifactId::new(DataSpaceId::UNIVERSAL, manifest.code_hash.unwrap());
    let signing = ManifestSigningFixture::new();
    let manifest = ContractManifestC10V1::try_from(manifest)
        .unwrap()
        .try_signed(signing.context(), signing.max_frame_bytes(), &owner)
        .unwrap();
    let mut invalid = code.clone();
    let layout = ivm::IvmProfileId::C10_ABI_V1.engine().header_layout();
    invalid[layout.budget_offset..layout.abi_version_offset].copy_from_slice(&17u64.to_le_bytes());
    let upload = RegisterSmartContractBytes {
        artifact_id: key,
        code,
    };
    let register = RegisterSmartContractCodeC10V1 {
        artifact_id: key,
        manifest: manifest.clone(),
    };
    let tx = chain.sign(&owner, [upload.into(), register.into()], at + 2000);
    let outcomes = chain.commit_at(at + 2000, vec![tx]);
    let committed = chain.committed(chain.height());
    let (_, row) = committed.block().network_execution_output_at(0).unwrap();
    assert!(matches!(row, ExecutionOutputV1::C10(_)), "{row:?}");
    assert!(row.result().is_ok(), "{row:?}");
    assert_eq!(outcomes, [true]);
    let c10_key = key;
    let state = chain.state().view();
    let world = state.world();
    assert_eq!(world.c10_contract_manifests().get(&key), Some(&manifest));
    assert_eq!(
        world.c10_contract_profiles().get(&key),
        Some(&ResolvedIvmProfile::of_id(ivm::IvmProfileId::C10_ABI_V1).definition_id())
    );
    assert!(world.contract_manifests().get(&key).is_none());
    drop(state);

    let account = chain.genesis_account().clone();
    let address = iroha_data_model::smart_contract::ContractAddress::derive(
        &chain.network_id(),
        &account,
        0,
        DataSpaceId::UNIVERSAL,
    )
    .unwrap();
    let deployment = iroha_data_model::isi::smart_contract_code::CommitContractDeployment {
        expected_deploy_nonce: 0,
        contract_address: address.clone(),
        code_hash: key.code_hash,
        contract_alias: iroha_data_model::smart_contract::ContractAlias::from_components(
            "c10answer",
            None,
            "universal",
        )
        .unwrap(),
        lease_expiry_ms: None,
        expected_previous_contract_address: None,
    };
    let deploy = chain.sign(&owner, [deployment.into()], at + 3000);
    let outcomes = chain.commit_at(at + 3000, vec![deploy]);
    let tip = chain.committed(chain.height());
    assert_eq!(outcomes, [true], "{:?}", tip.block().execution_outputs());
    assert!(matches!(
        tip.block().network_execution_output_at(0).unwrap().1,
        ExecutionOutputV1::C10(_)
    ));

    let intent = iroha_data_model::transaction::FeePaymentIntent::authority(
        Vec::new(),
        std::num::NonZeroU64::new(1_000_000),
    );
    let mut builder =
        iroha_data_model::transaction::TransactionBuilder::new(chain.network_id(), account, intent);
    builder.set_creation_time(std::time::Duration::from_millis(at + 4000));
    let invocation = iroha_data_model::transaction::executable::ContractInvocation {
        contract_address: address,
        expected_code_hash: key.code_hash,
        entrypoint: "answer".to_owned(),
        arguments: None,
    };
    let call = builder
        .with_executable(iroha_data_model::transaction::Executable::ContractCall(
            invocation.clone(),
        ))
        .sign(owner.private_key());
    let outcomes = chain.commit_at(at + 4000, vec![call]);
    let tip = chain.committed(chain.height());
    let (_, row) = tip.block().network_execution_output_at(0).unwrap();
    assert_eq!(outcomes, [true], "{row:?}");
    assert!(matches!(row, ExecutionOutputV1::C10(_)), "{row:?}");
    assert!(row.result().is_ok(), "{row:?}");

    let invalid_id =
        ContractArtifactId::new(DataSpaceId::UNIVERSAL, ivm::contract_code_hash(&invalid));
    let bad = RegisterSmartContractBytes {
        artifact_id: invalid_id,
        code: invalid,
    };
    let tx = chain.sign(&owner, [bad.into()], at + 5000);
    let outcomes = chain.commit_at(at + 5000, vec![tx]);
    let tip = chain.committed(chain.height());
    let (_, row) = tip.block().network_execution_output_at(0).unwrap();
    let ExecutionOutputV1::C10(output) = row else {
        panic!("C10 admission row: {row:?}")
    };
    use iroha_data_model::ivm_c10::{
        IvmAdmissionErrorC10V1, TransactionRejectionReasonC10V1, ValidationFailC10V1,
    };
    assert!(
        matches!(
            &output.result.0,
            Err(TransactionRejectionReasonC10V1::Validation(
                ValidationFailC10V1::IvmAdmission(IvmAdmissionErrorC10V1::MaxCyclesWithoutZk(17))
            ))
        ),
        "{row:?}"
    );
    assert_eq!(outcomes, [false]);
    assert!(
        chain
            .state()
            .view()
            .world()
            .contract_code()
            .get(&invalid_id)
            .is_none()
    );

    let (baseline, baseline_manifest) = kotodama_lang::compiler::Compiler::new()
        .compile_source_with_manifest(
            "seiyaku Old { view fn answer() authorize(anyone) -> int { return 1; } }",
        )
        .unwrap();
    let key = ContractArtifactId::new(DataSpaceId::UNIVERSAL, baseline_manifest.code_hash.unwrap());
    let baseline_manifest = baseline_manifest
        .try_signed(signing.context(), signing.max_frame_bytes(), &owner)
        .unwrap();
    let tx = chain.sign(
        &owner,
        [
            RegisterSmartContractBytes {
                artifact_id: key,
                code: baseline,
            }
            .into(),
            iroha_data_model::isi::smart_contract_code::RegisterSmartContractCode {
                artifact_id: key,
                manifest: baseline_manifest,
            }
            .into(),
        ],
        at + 6000,
    );
    let outcomes = chain.commit_at(at + 6000, vec![tx]);
    let tip = chain.committed(chain.height());
    let (_, row) = tip.block().network_execution_output_at(0).unwrap();
    assert_eq!(outcomes, [true], "{row:?}");
    assert!(row.network().is_some(), "{row:?}");
    assert!(row.result().is_ok(), "{row:?}");

    crate::state::c10_trigger_runtime::verify_deployed_callbacks(&mut chain, &owner, invocation);
    let original = std::sync::Arc::clone(chain.state());
    let snapshot = crate::snapshot::CapturedStateSnapshot::capture(&original).unwrap();
    let captured_height = chain.height();
    let kura = std::sync::Arc::clone(chain.kura());
    let restore = || {
        crate::state::deserialize::KuraSeed {
            execution_budget: iroha_allocation::AllocationBudget::new(
                iroha_config::parameters::defaults::pipeline::IVM_EXECUTION_MAX_BYTES,
            ),
            lane_manifests: original.lane_manifests.read().clone(),
            kura: std::sync::Arc::clone(&kura),
            query_handle: crate::query::store::LiveQueryStore::start_test(),
            #[cfg(feature = "telemetry")]
            telemetry: crate::telemetry::StateTelemetry::default(),
        }
        .into_state_from_json_str_with_configured_nexus(
            snapshot.as_json(),
            original.nexus_snapshot(),
        )
    };
    assert!(
        matches!(restore(), Err(crate::state::deserialize::StateRestoreError::SuccessorRequired {height}) if height == captured_height)
    );
    let next_time = chain.committed(captured_height).block_time_ms() + 1_000;
    chain.commit_at(next_time, vec![]);
    let verify_cut = |world: &crate::state::WorldView<'_>| {
        assert_eq!(
            world.c10_contract_manifests().get(&c10_key),
            Some(&manifest)
        );
        assert_eq!(
            world.c10_contract_profiles().get(&c10_key),
            Some(&ResolvedIvmProfile::of_id(ivm::IvmProfileId::C10_ABI_V1).definition_id())
        );
        assert!(world.contract_manifests().get(&c10_key).is_none());
        assert!(world.contract_manifests().get(&key).is_some());
        for id in ["c10_direct_callback", "c10_prepared_callback"] {
            let id: iroha_data_model::trigger::TriggerId = id.parse().unwrap();
            assert_eq!(
                world
                    .c10_triggers()
                    .get(&id)
                    .unwrap()
                    .trigger()
                    .action
                    .repeats(),
                iroha_data_model::trigger::action::Repeats::Exactly(1),
            );
            assert!(world.c10_trigger_candidates().get(&id).is_none());
        }
        let rejected = "c10_invalid_program".parse().unwrap();
        assert!(world.c10_triggers().get(&rejected).is_none());
    };
    if keyed {
        let mut restored = restore().expect("the successor commits the complete keyed State");
        restored.set_gov(original.gov.clone());
        restored.set_pipeline(iroha_config::parameters::actual::Pipeline::default());
        restored.install_node_fraud_monitoring(
            iroha_config::parameters::actual::FraudMonitoring::default(),
        );
        assert!(restored.authenticate_restored_keyed_state().unwrap());
        assert_eq!(restored.committed_height() as u64, captured_height);
        assert_eq!(
            crate::snapshot::canonical_state_snapshot_hash(&restored).unwrap(),
            snapshot.canonical_hash().unwrap()
        );
        verify_cut(restored.view().world());
        return;
    }
    assert!(matches!(
        restore(),
        Err(crate::state::deserialize::StateRestoreError::NativeExecutionReplayRequired)
    ));

    // Each fixture stages the same attested authorization before replay verifies the original
    // certified blocks, including the activation boundary and the typed rejected transaction.
    let mut replay = CertifiedTestChain::start(fixture()).unwrap();
    crate::runtime::ironwood::test_activation::schedule_change(&replay, at, change);
    replay.replay_from(&chain).unwrap();
    verify_cut(replay.state().view().world());
    assert_eq!(
        crate::snapshot::canonical_state_snapshot_hash(replay.state()).unwrap(),
        crate::snapshot::canonical_state_snapshot_hash(chain.state()).unwrap()
    );
}
