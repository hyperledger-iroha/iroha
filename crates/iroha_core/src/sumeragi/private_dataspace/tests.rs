//! Parent registration authorization, persistence and certified cursor regression coverage.

use super::*;
use crate::{
    kura::Kura,
    query::store::LiveQueryStore,
    smartcontracts::isi::sccp::test_support::{authority, header},
    state::{State, StateBlock, World},
    sumeragi::lanes::routing::test_support,
};
use iroha_data_model::{
    Registrable,
    account::{Account, AccountAddress},
    isi::SetParameter,
    parameter::{CustomParameter, Parameter},
    sns::{NameControllerV1, NameRecordV1},
    sumeragi_finality::{genesis_epoch, test_fixtures::NativeFinalityFixture},
};
use iroha_model_base::metadata::Metadata;
use norito::codec::Encode;

fn policy() -> PrivateDataspaceAdmissionPolicy {
    PrivateDataspaceAdmissionPolicy {
        max_registered_roots: 8,
        max_roots_per_owner: 2,
    }
}

fn lease(owner: &AccountId) -> NameRecordV1 {
    NameRecordV1::new(
        crate::sns::selector_for_dataspace_alias("acme").unwrap(),
        owner.clone(),
        vec![NameControllerV1::account(
            &AccountAddress::from_account_id(owner).unwrap(),
        )],
        0,
        0,
        100_000,
        200_000,
        300_000,
        Metadata::default(),
    )
}

fn state(
    scope: Option<SumeragiRootScope>,
    admission: Option<PrivateDataspaceAdmissionPolicy>,
) -> State {
    State::new_for_testing(
        world(scope, admission),
        Kura::blank_kura_for_testing(),
        LiveQueryStore::start_test(),
    )
}

fn world(
    scope: Option<SumeragiRootScope>,
    admission: Option<PrivateDataspaceAdmissionPolicy>,
) -> World {
    let owner = authority(1);
    let relayer = authority(2);
    let accounts = [
        Account::new(owner.clone()).build(&owner),
        Account::new(relayer).build(&owner),
    ];
    let mut world = World::with([], accounts, []);
    let record = lease(&owner);
    world.smart_contract_state_mut_for_testing().insert(
        crate::sns::record_storage_key(&record.selector),
        record.encode(),
    );
    {
        let mut parameters = world.parameters.block();
        if let Some(scope) = scope {
            parameters.set_parameter(test_support::metadata(scope));
        }
        if let Some(policy) = admission {
            parameters.set_parameter(Parameter::Custom(policy.into_custom_parameter().unwrap()));
        }
        parameters.commit();
    }
    world
}

fn registration(state: &State) -> (NativeFinalityFixture, RegisterPrivateDataspace) {
    let dataspace_id = crate::sns::dataspace_id_for_sns_alias("acme").unwrap();
    let scope = SumeragiRootScope::Dataspace {
        parent_network_id: *state.network_id_ref(),
        dataspace_id,
    };
    let child = NativeFinalityFixture::start_with_scope("acme-private", scope);
    let result = child
        .verifier()
        .verify_retained_decision(child.genesis_proof())
        .unwrap()
        .result()
        .0;
    let registration = PrivateDataspaceRegistration::new(
        scope,
        child.chain_id().parse().unwrap(),
        child.network_id(),
        result,
        genesis_epoch(child.genesis()).unwrap(),
    )
    .unwrap();
    (
        child,
        RegisterPrivateDataspace {
            alias: "acme".into(),
            expected_ownership_generation: 1,
            registration: norito::encode_canonical(&registration).unwrap(),
        },
    )
}

fn execute(
    block: &mut StateBlock<'_>,
    signer: &AccountId,
    instruction: impl Execute,
) -> Result<(), Error> {
    let mut transaction = block.transaction();
    instruction.execute(signer, &mut transaction)?;
    transaction.apply();
    Ok(())
}

#[test]
fn parent_registration_requires_committed_global_scope_active_owner_and_admission() {
    let parent = state(Some(SumeragiRootScope::Global), Some(policy()));
    let (_, register) = registration(&parent);
    let mut block = parent.block(header(2));
    for (signer, candidate, message) in [
        (authority(2), register.clone(), "authority"),
        (authority(3), register.clone(), "not registered"),
        (
            authority(1),
            RegisterPrivateDataspace {
                expected_ownership_generation: 2,
                ..register.clone()
            },
            "generation",
        ),
        (
            authority(1),
            RegisterPrivateDataspace {
                alias: "unknown".into(),
                ..register.clone()
            },
            "alias differs",
        ),
        (
            authority(1),
            RegisterPrivateDataspace {
                registration: vec![0; 8],
                ..register.clone()
            },
            "private dataspace",
        ),
    ] {
        let error = execute(&mut block, &signer, candidate).unwrap_err();
        assert!(error.to_string().contains(message), "{error}");
        assert!(block.world.private_dataspaces().records().is_empty());
    }
    execute(&mut block, &authority(1), register.clone()).unwrap();
    assert_eq!(block.world.private_dataspaces().records().len(), 1);
    let private_id = crate::sns::dataspace_id_for_sns_alias("acme").unwrap();
    assert!(ensure_parent_execution_separate(&block.world, [private_id]).is_err());
    assert!(ensure_parent_execution_separate(&block.world, [DataSpaceId::UNIVERSAL]).is_ok());
    for scope in [
        None,
        Some(SumeragiRootScope::Dataspace {
            parent_network_id: *parent.network_id_ref(),
            dataspace_id: DataSpaceId::new(9),
        }),
    ] {
        let other = state(scope, Some(policy()));
        let mut block = other.block(header(2));
        assert!(
            execute(&mut block, &authority(1), register.clone())
                .unwrap_err()
                .to_string()
                .contains("global root")
        );
    }
    let disabled = state(Some(SumeragiRootScope::Global), None);
    let mut block = disabled.block(header(2));
    assert!(
        execute(&mut block, &authority(1), register)
            .unwrap_err()
            .to_string()
            .contains("disabled")
    );
}

#[test]
fn private_registration_waits_for_prior_physical_alias_retirement() {
    use iroha_data_model::nexus::{
        DataSpaceCatalog, DataSpaceMetadata, LaneCatalog, LaneConfig, LaneLifecycleParameterV1,
        NexusCatalogTransitionV1, RuntimeDataSpaceRetirementV1,
    };
    use iroha_model_base::topology::LaneId;
    use std::{num::NonZeroU32, sync::Arc};

    // The old physical identity is retained exactly; the new private identity is
    // independently derived from the current paid name and native child genesis.
    let old_physical = DataSpaceId::new(7);
    assert_ne!(
        old_physical,
        crate::sns::dataspace_id_for_sns_alias("acme").unwrap()
    );
    let lanes = LaneCatalog::new(
        NonZeroU32::new(8).unwrap(),
        vec![
            LaneConfig::default(),
            LaneConfig {
                id: LaneId::new(7),
                dataspace_id: old_physical,
                alias: "acme".into(),
                ..Default::default()
            },
        ],
    )
    .unwrap();
    let dataspaces = DataSpaceCatalog::new(vec![
        DataSpaceMetadata::default(),
        DataSpaceMetadata {
            id: old_physical,
            alias: "acme".into(),
            description: Some("exact original physical descriptor".into()),
            fault_tolerance: 1,
        },
    ])
    .unwrap();
    let mut nexus = iroha_config::parameters::actual::Nexus::default();
    nexus.lane_catalog = lanes.clone();
    nexus.configured_lane_catalog = lanes;
    nexus.lane_config =
        iroha_config::parameters::actual::LaneConfig::from_catalog(&nexus.lane_catalog);
    nexus.dataspace_catalog = dataspaces.clone();
    nexus.configured_dataspace_catalog = dataspaces;
    let mut old_lease = lease(&authority(1));
    old_lease.metadata.insert(
        crate::sns::SNS_DATASPACE_ID_METADATA_KEY.parse().unwrap(),
        iroha_primitives::json::Json::new(7_u64),
    );
    let old_lease_bytes = old_lease.encode();
    let mut parent_world = world(Some(SumeragiRootScope::Global), Some(policy()));
    parent_world.smart_contract_state_mut_for_testing().insert(
        crate::sns::record_storage_key(&old_lease.selector),
        old_lease_bytes.clone(),
    );
    let mut parameters = parent_world.parameters.block();
    parameters.set_parameter(test_support::closed_native_lane_policy(
        SumeragiRootScope::Global,
    ));
    parameters.commit();
    let parent = State::new_with_nexus_for_testing(
        parent_world,
        nexus.clone(),
        LiveQueryStore::start_test(),
    );
    parent.install_lane_manifests_for_testing(&Arc::new(
        crate::governance::manifest::LaneManifestRegistry::from_config(
            &nexus.lane_catalog,
            &nexus.governance,
            &nexus.registry,
        ),
    ));
    let (mut native_child, register) = registration(&parent);
    let child_registration = PrivateDataspaceRegistration::decode(&register.registration).unwrap();
    let mut first = parent.block(header(1));
    assert!(
        execute(&mut first, &authority(1), register.clone())
            .unwrap_err()
            .to_string()
            .contains("locally configured")
    );
    // This publishes exact synthetic test membership through the native State
    // commit path; it does not claim that the fixture has network finality.
    first.commit_empty_block_for_testing().unwrap();

    let request = NexusCatalogTransitionV1 {
        version: 1,
        dataspace_additions: Vec::new(),
        lane_additions: Vec::new(),
        manifest_additions: Vec::new(),
        dataspace_retirements: vec![RuntimeDataSpaceRetirementV1 {
            dataspace_id: old_physical,
            alias: "acme".into(),
            owner: authority(1),
            expected_ownership_generation: 1,
        }],
        lane_retirements: vec![LaneId::new(7)],
        expected_catalog_hash: LaneLifecycleParameterV1::catalog_hash(&nexus.lane_catalog),
        expected_incarnation_root: LaneLifecycleParameterV1::incarnation_root(
            &LaneLifecycleParameterV1::canonical_incarnations(
                &nexus.lane_catalog,
                &parent.lane_incarnations_snapshot(),
            )
            .unwrap(),
        ),
        expected_runtime_catalog_hash: None,
    };
    let mut retiring = parent.block(header(2));
    let mut transaction = retiring.transaction();
    transaction
        .stage_consensus_catalog_transition(&authority(1), &request)
        .unwrap();
    transaction.apply();
    assert!(
        execute(&mut retiring, &authority(1), register.clone())
            .unwrap_err()
            .to_string()
            .contains("staged physical")
    );
    assert!(retiring.world.private_dataspaces().records().is_empty());
    retiring.commit_empty_block_for_testing().unwrap();

    let mut after = parent.block(header(3));
    execute(&mut after, &authority(1), register).unwrap();
    let body = native_child.block_with_submitted_work(native_child.next_header());
    let proof = native_child.certify(body);
    let decision = native_child
        .verifier()
        .verify_retained_decision(&proof)
        .unwrap();
    let anchor = PrivateDataspaceAnchor::from_certificate(
        &child_registration,
        decision.block().commit_certificate().unwrap(),
    )
    .unwrap();
    execute(
        &mut after,
        &authority(2),
        AnchorPrivateDataspace {
            dataspace_id: anchor.dataspace_id,
            anchor: norito::encode_canonical(&anchor).unwrap(),
        },
    )
    .unwrap();
    let child = after.world.private_dataspaces().records().first().unwrap();
    assert_eq!(child.anchor.cursor().height, 2);
    assert_eq!(
        child.anchor.registration().scope,
        SumeragiRootScope::Dataspace {
            parent_network_id: *parent.network_id_ref(),
            dataspace_id: crate::sns::dataspace_id_for_sns_alias("acme").unwrap(),
        }
    );
    let runtime = crate::state::runtime_catalog_from_world(&after.world)
        .unwrap()
        .unwrap();
    assert_eq!(
        runtime.retired_dataspaces[0].retirement.dataspace_id,
        old_physical
    );
    assert_eq!(runtime.retired_lanes[0].lane, nexus.lane_catalog.lanes()[1]);
    assert_eq!(
        after
            .world
            .smart_contract_state()
            .get(&crate::sns::record_storage_key(&old_lease.selector)),
        Some(&old_lease_bytes),
    );
}

#[test]
fn parent_records_only_public_credentials_and_certified_cursor_witness() {
    let parent = state(Some(SumeragiRootScope::Global), Some(policy()));
    let (mut child, register) = registration(&parent);
    let registration = PrivateDataspaceRegistration::decode(&register.registration).unwrap();
    let mut block = parent.block(header(2));
    let _guard = crate::exec_witness::exec_witness_guard();
    crate::exec_witness::start_block();
    execute(&mut block, &authority(1), register.clone()).unwrap();
    let body = child.block_with_submitted_work(child.next_header());
    let proof = child.certify(body);
    let decision = child.verifier().verify_retained_decision(&proof).unwrap();
    let anchor = PrivateDataspaceAnchor::from_certificate(
        &registration,
        decision.block().commit_certificate().unwrap(),
    )
    .unwrap();
    let instruction = AnchorPrivateDataspace {
        dataspace_id: anchor.dataspace_id,
        anchor: norito::encode_canonical(&anchor).unwrap(),
    };
    // Any registered relayer can submit the genuine QC; it cannot replace owner authorization.
    execute(&mut block, &authority(2), instruction.clone()).unwrap();
    execute(&mut block, &authority(2), instruction).unwrap();
    execute(&mut block, &authority(1), register).unwrap();
    let record = block
        .world
        .private_dataspaces()
        .get(anchor.dataspace_id)
        .unwrap();
    assert_eq!(record.anchor.cursor().height, 2);
    let public = norito::encode_canonical(record).unwrap();
    assert!(
        !public
            .windows(b"fixture submitted work".len())
            .any(|chunk| chunk == b"fixture submitted work")
    );
    let witness = crate::exec_witness::drain_exec_witness();
    assert!(
        witness
            .writes
            .iter()
            .any(|write| write.key == record.witness_key() && write.value == public)
    );
    block.world.private_dataspaces().validate().unwrap();
}

#[test]
fn parent_anchor_rechecks_expiry_generation_and_exact_child_binding() {
    let parent = state(Some(SumeragiRootScope::Global), Some(policy()));
    let (mut child, register) = registration(&parent);
    let registration = PrivateDataspaceRegistration::decode(&register.registration).unwrap();
    let body = child.block_with_submitted_work(child.next_header());
    let proof = child.certify(body);
    let decision = child.verifier().verify_retained_decision(&proof).unwrap();
    let anchor = PrivateDataspaceAnchor::from_certificate(
        &registration,
        decision.block().commit_certificate().unwrap(),
    )
    .unwrap();
    let instruction = AnchorPrivateDataspace {
        dataspace_id: anchor.dataspace_id,
        anchor: norito::encode_canonical(&anchor).unwrap(),
    };
    let mut block = parent.block(header(2));
    execute(&mut block, &authority(1), register).unwrap();
    let retained = block.world.private_dataspaces().clone();
    for changed in [
        NameRecordV1 {
            ownership_generation: 2,
            ..lease(&authority(1))
        },
        NameRecordV1 {
            expires_at_ms: 1,
            ..lease(&authority(1))
        },
        lease(&authority(2)),
    ] {
        {
            let mut tx = block.transaction();
            tx.world.smart_contract_state.insert(
                crate::sns::record_storage_key(&changed.selector),
                changed.encode(),
            );
            tx.apply();
        }
        assert!(execute(&mut block, &authority(2), instruction.clone()).is_err());
        assert_eq!(block.world.private_dataspaces(), &retained);
    }
    assert_eq!(
        retained
            .get(anchor.dataspace_id)
            .unwrap()
            .anchor
            .cursor()
            .height,
        1
    );
}

#[test]
fn admission_parameter_rejects_malformed_payload_and_retains_previous_policy() {
    let parent = state(Some(SumeragiRootScope::Global), Some(policy()));
    let mut block = parent.block(header(2));
    let id = PrivateDataspaceAdmissionPolicy::parameter_id();
    for payload in [
        "{}",
        "{\"max_registered_roots\":1,\"max_roots_per_owner\":2}",
    ] {
        let parameter = Parameter::Custom(CustomParameter::new(
            id.clone(),
            payload.parse::<iroha_primitives::json::Json>().unwrap(),
        ));
        assert!(execute(&mut block, &authority(1), SetParameter::new(parameter)).is_err());
        assert_eq!(
            PrivateDataspaceAdmissionPolicy::from_custom_parameter(
                block.world.parameters().custom().get(&id).unwrap()
            )
            .unwrap(),
            policy()
        );
    }
}

#[test]
fn parent_receipt_uses_original_certified_archive_and_survives_native_replay() {
    // Block execution and recovery run on the daemon's bounded Sumeragi owner;
    // libtest's unrelated 2 MiB worker is not that production stack contract.
    crate::sumeragi::threads::sumeragi_thread_builder("private-parent-native-replay")
        .spawn(parent_receipt_replay_on_sumeragi_owner)
        .unwrap()
        .join()
        .unwrap();
}

fn parent_receipt_replay_on_sumeragi_owner() {
    use crate::query::native_receipts::private_dataspace_record_proof;
    use crate::sumeragi::test_chain::{CertifiedTestChain, TestChainConfig};
    use iroha_crypto::{Algorithm, KeyPair};
    use iroha_data_model::sumeragi_finality::{FinalityValidator, SumeragiFinalityVerifier};

    let config = || TestChainConfig::new(world(None, Some(policy())), 1_000);
    let mut chain = CertifiedTestChain::start(config()).unwrap();
    let (_, register) = registration(chain.state());
    let id = crate::sns::dataspace_id_for_sns_alias("acme").unwrap();
    let key = KeyPair::from_seed(vec![1; 32], Algorithm::Ed25519);
    let transaction = chain.sign(&key, [register.into()], 1_999);
    assert_eq!(chain.commit_at(2_000, vec![transaction]), vec![true]);
    let native_state = chain.state();
    let genesis = chain.genesis();
    let receipt = std::thread::scope(|scope| {
        std::thread::Builder::new()
            .name("private-parent-native-receipt-read".to_owned())
            .stack_size(iroha_config::parameters::defaults::concurrency::TOKIO_STACK_BYTES_MIN)
            .spawn_scoped(scope, || {
                let receipt = private_dataspace_record_proof(&native_state.view(), 2, id)
                    .unwrap()
                    .unwrap();
                assert_eq!(receipt.record.anchor.cursor().height, 1);
                let validators = genesis_epoch(genesis)
                    .unwrap()
                    .committee
                    .into_iter()
                    .map(|member| FinalityValidator {
                        public_key: member.validator.public_key().clone(),
                        proof_of_possession: member.proof_of_possession,
                    })
                    .collect();
                let mut verifier = SumeragiFinalityVerifier::new(
                    genesis,
                    "sumeragi-certified-test-chain",
                    validators,
                )
                .unwrap();
                verifier
                    .verify(
                        &crate::sumeragi::finality::build_proof(&native_state.view(), 1).unwrap(),
                    )
                    .unwrap();
                let certified = verifier
                    .verify(
                        &crate::sumeragi::finality::build_proof(&native_state.view(), 2).unwrap(),
                    )
                    .unwrap();
                receipt.verify(id, &certified).unwrap();
                assert!(private_dataspace_record_proof(&native_state.view(), 1, id).is_err());
                assert!(
                    private_dataspace_record_proof(&native_state.view(), 2, DataSpaceId::UNIVERSAL)
                        .unwrap()
                        .is_none()
                );
                receipt
            })
            .unwrap()
            .join()
            .unwrap()
    });
    let mut replayed = CertifiedTestChain::start(config()).unwrap();
    replayed.replay_from(&chain).unwrap();
    let replayed_state = replayed.state();
    std::thread::scope(|scope| {
        std::thread::Builder::new()
            .name("private-parent-replayed-receipt-read".to_owned())
            .stack_size(iroha_config::parameters::defaults::concurrency::TOKIO_STACK_BYTES_MIN)
            .spawn_scoped(scope, || {
                assert_eq!(
                    private_dataspace_record_proof(&replayed_state.view(), 2, id).unwrap(),
                    Some(receipt)
                );
            })
            .unwrap()
            .join()
            .unwrap()
    });
}
