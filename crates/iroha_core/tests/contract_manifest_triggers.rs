//! Contract manifest trigger registration tests.
#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]

use iroha_core::{
    kura::Kura,
    query::store::LiveQueryStore,
    smartcontracts::Execute,
    smartcontracts::triggers::set::SetReadOnly,
    state::{State, World, WorldReadOnly},
};
use iroha_crypto::KeyPair;
use iroha_data_model::{
    events::EventFilterBox,
    events::data::{
        DataEventFilter,
        prelude::{AssetEventFilter, AssetEventSet},
    },
    events::pipeline::{BlockEventFilter, PipelineEventFilterBox},
    events::time::{ExecutionTime, TimeEventFilter},
    isi::smart_contract_code::{
        ActivateContractInstance, DeactivateContractInstance, RegisterSmartContractBytes,
        RegisterSmartContractCode,
    },
    name::Name,
    permission,
    prelude::*,
    smart_contract::manifest::{
        ContractManifest, EntryPointKind, EntrypointDescriptor, TriggerCallback, TriggerDescriptor,
    },
    trigger::action::Repeats,
};
use iroha_primitives::json::Json;
use mv::storage::StorageReadOnly;
use nonzero_ext::nonzero;

fn minimal_ivm_program(abi_version: u8) -> Vec<u8> {
    let mut code = Vec::new();
    code.extend_from_slice(&ivm::encoding::wide::encode_halt().to_le_bytes());
    let meta = ivm::ProgramMetadata {
        version_major: 1,
        version_minor: 1,
        mode: 0,
        vector_length: 0,
        max_cycles: 1,
        abi_version,
    };
    let mut out = meta.encode();
    out.extend_from_slice(&code);
    out
}

fn setup_state() -> (State, AccountId, KeyPair) {
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let kp = KeyPair::random();
    let domain_id: DomainId = "wonderland".parse().expect("domain");
    let account_id = AccountId::new(kp.public_key().clone());
    let domain = Domain::new(domain_id.clone()).build(&account_id);
    let account = Account::new(account_id.clone().to_account_id(domain_id)).build(&account_id);
    let world = World::with([domain], [account], core::iter::empty::<AssetDefinition>());
    let state = State::new_for_testing(world, kura, query_handle);
    (state, account_id, kp)
}

fn assert_contract_trigger_metadata(
    metadata: &Metadata,
    namespace: &str,
    contract_id: &str,
    entrypoint: &str,
    code_hash: iroha_crypto::Hash,
    trigger_id: &TriggerId,
    user_key: &str,
    user_value: &str,
) {
    let key_namespace: Name = "contract_namespace".parse().expect("namespace key");
    let key_contract: Name = "contract_id".parse().expect("contract_id key");
    let key_entrypoint: Name = "contract_entrypoint".parse().expect("entrypoint key");
    let key_code: Name = "contract_code_hash".parse().expect("code hash key");
    let key_trigger: Name = "contract_trigger_id".parse().expect("trigger id key");
    assert_eq!(metadata.get(&key_namespace), Some(&Json::from(namespace)));
    assert_eq!(metadata.get(&key_contract), Some(&Json::from(contract_id)));
    assert_eq!(metadata.get(&key_entrypoint), Some(&Json::from(entrypoint)));
    let code_hash_json = Json::from(code_hash.to_string().as_str());
    assert_eq!(metadata.get(&key_code), Some(&code_hash_json));
    let trigger_id_json = Json::from(trigger_id.to_string().as_str());
    assert_eq!(metadata.get(&key_trigger), Some(&trigger_id_json));
    let user_name: Name = user_key.parse().expect("user metadata key");
    assert_eq!(metadata.get(&user_name), Some(&Json::from(user_value)));
}

#[test]
#[allow(clippy::too_many_lines)]
fn activate_registers_manifest_triggers_and_deactivate_removes() {
    let (state, authority, kp) = setup_state();
    let header = iroha_data_model::block::BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
    let mut block = state.block(header);
    let mut stx = block.transaction();

    let register_perm: permission::Permission =
        iroha_executor_data_model::permission::smart_contract::CanRegisterSmartContractCode.into();
    Grant::account_permission(register_perm, authority.clone())
        .execute(&authority, &mut stx)
        .expect("grant CanRegisterSmartContractCode");
    let enact_perm: permission::Permission =
        iroha_executor_data_model::permission::governance::CanEnactGovernance.into();
    Grant::account_permission(enact_perm, authority.clone())
        .execute(&authority, &mut stx)
        .expect("grant CanEnactGovernance");

    let program = minimal_ivm_program(1);
    let parsed = ivm::ProgramMetadata::parse(&program).expect("ivm header");
    let code_hash = iroha_crypto::Hash::new(&program[parsed.header_len..]);

    RegisterSmartContractBytes {
        code_hash,
        code: program,
    }
    .execute(&authority, &mut stx)
    .expect("register contract bytes");

    let trigger_id: TriggerId = "wake".parse().expect("trigger id");
    let mut descriptor_metadata = Metadata::default();
    descriptor_metadata.insert("tag".parse::<Name>().expect("tag key"), Json::from("alpha"));
    let trigger = TriggerDescriptor {
        id: trigger_id.clone(),
        repeats: Repeats::Indefinitely,
        filter: EventFilterBox::Time(TimeEventFilter(ExecutionTime::PreCommit)),
        authority: None,
        metadata: descriptor_metadata,
        callback: TriggerCallback {
            namespace: None,
            entrypoint: "run".to_string(),
        },
    };
    let entrypoint = EntrypointDescriptor {
        name: "run".to_string(),
        kind: EntryPointKind::Public,
        permission: None,
        read_keys: Vec::new(),
        write_keys: Vec::new(),
        access_hints_complete: None,
        access_hints_skipped: Vec::new(),
        triggers: vec![trigger],
    };
    let abi_hash = ivm::syscalls::compute_abi_hash(ivm::SyscallPolicy::AbiV1);
    let manifest = ContractManifest {
        code_hash: Some(code_hash),
        abi_hash: Some(iroha_crypto::Hash::prehashed(abi_hash)),
        compiler_fingerprint: None,
        features_bitmap: None,
        access_set_hints: None,
        entrypoints: Some(vec![entrypoint]),
        kotoba: None,
        provenance: None,
    }
    .signed(&kp);
    RegisterSmartContractCode { manifest }
        .execute(&authority, &mut stx)
        .expect("register manifest");

    ActivateContractInstance {
        namespace: "apps".to_string(),
        contract_id: "demo.contract".to_string(),
        code_hash,
    }
    .execute(&authority, &mut stx)
    .expect("activate");

    let action = stx
        .world
        .triggers()
        .time_triggers()
        .get(&trigger_id)
        .expect("trigger registered");
    let metadata = &action.metadata;
    let key_namespace: Name = "contract_namespace".parse().expect("namespace key");
    let key_contract: Name = "contract_id".parse().expect("contract_id key");
    let key_entrypoint: Name = "contract_entrypoint".parse().expect("entrypoint key");
    let key_code: Name = "contract_code_hash".parse().expect("code hash key");
    let key_trigger: Name = "contract_trigger_id".parse().expect("trigger id key");
    assert_eq!(metadata.get(&key_namespace), Some(&Json::from("apps")));
    assert_eq!(
        metadata.get(&key_contract),
        Some(&Json::from("demo.contract"))
    );
    assert_eq!(metadata.get(&key_entrypoint), Some(&Json::from("run")));
    let code_hash_json = Json::from(code_hash.to_string().as_str());
    assert_eq!(metadata.get(&key_code), Some(&code_hash_json));
    let trigger_id_json = Json::from(trigger_id.to_string().as_str());
    assert_eq!(metadata.get(&key_trigger), Some(&trigger_id_json));
    let tag_key: Name = "tag".parse().expect("tag key");
    assert_eq!(metadata.get(&tag_key), Some(&Json::from("alpha")));

    DeactivateContractInstance {
        namespace: "apps".to_string(),
        contract_id: "demo.contract".to_string(),
        reason: None,
    }
    .execute(&authority, &mut stx)
    .expect("deactivate");

    assert!(stx.world.triggers().ids().get(&trigger_id).is_none());
}

#[test]
#[allow(clippy::too_many_lines)]
fn activate_registers_manifest_data_and_pipeline_triggers_and_deactivate_removes_them() {
    let (state, authority, kp) = setup_state();
    let header = iroha_data_model::block::BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
    let mut block = state.block(header);
    let mut stx = block.transaction();

    let register_perm: permission::Permission =
        iroha_executor_data_model::permission::smart_contract::CanRegisterSmartContractCode.into();
    Grant::account_permission(register_perm, authority.clone())
        .execute(&authority, &mut stx)
        .expect("grant CanRegisterSmartContractCode");
    let enact_perm: permission::Permission =
        iroha_executor_data_model::permission::governance::CanEnactGovernance.into();
    Grant::account_permission(enact_perm, authority.clone())
        .execute(&authority, &mut stx)
        .expect("grant CanEnactGovernance");

    let program = minimal_ivm_program(1);
    let parsed = ivm::ProgramMetadata::parse(&program).expect("ivm header");
    let code_hash = iroha_crypto::Hash::new(&program[parsed.header_len..]);

    RegisterSmartContractBytes {
        code_hash,
        code: program,
    }
    .execute(&authority, &mut stx)
    .expect("register contract bytes");

    let asset_definition: AssetDefinitionId = "aid:6872454e9c044641aa581ec5f3801619"
        .parse()
        .expect("asset definition id");
    let data_trigger_id: TriggerId = "asset_added".parse().expect("data trigger id");
    let pipeline_trigger_id: TriggerId = "block_seen".parse().expect("pipeline trigger id");

    let mut data_metadata = Metadata::default();
    data_metadata.insert("tag".parse::<Name>().expect("tag key"), Json::from("data"));
    let data_trigger = TriggerDescriptor {
        id: data_trigger_id.clone(),
        repeats: Repeats::Indefinitely,
        filter: EventFilterBox::Data(DataEventFilter::Asset(
            AssetEventFilter::new()
                .for_events(AssetEventSet::Added)
                .for_asset_definition(asset_definition.clone()),
        )),
        authority: None,
        metadata: data_metadata,
        callback: TriggerCallback {
            namespace: None,
            entrypoint: "run".to_string(),
        },
    };

    let mut pipeline_metadata = Metadata::default();
    pipeline_metadata.insert(
        "tag".parse::<Name>().expect("tag key"),
        Json::from("pipeline"),
    );
    let pipeline_trigger = TriggerDescriptor {
        id: pipeline_trigger_id.clone(),
        repeats: Repeats::Indefinitely,
        filter: EventFilterBox::Pipeline(
            PipelineEventFilterBox::Block(BlockEventFilter::default()),
        ),
        authority: None,
        metadata: pipeline_metadata,
        callback: TriggerCallback {
            namespace: None,
            entrypoint: "run".to_string(),
        },
    };

    let entrypoint = EntrypointDescriptor {
        name: "run".to_string(),
        kind: EntryPointKind::Public,
        permission: None,
        read_keys: Vec::new(),
        write_keys: Vec::new(),
        access_hints_complete: None,
        access_hints_skipped: Vec::new(),
        triggers: vec![data_trigger, pipeline_trigger],
    };
    let abi_hash = ivm::syscalls::compute_abi_hash(ivm::SyscallPolicy::AbiV1);
    let manifest = ContractManifest {
        code_hash: Some(code_hash),
        abi_hash: Some(iroha_crypto::Hash::prehashed(abi_hash)),
        compiler_fingerprint: None,
        features_bitmap: None,
        access_set_hints: None,
        entrypoints: Some(vec![entrypoint]),
        kotoba: None,
        provenance: None,
    }
    .signed(&kp);
    RegisterSmartContractCode { manifest }
        .execute(&authority, &mut stx)
        .expect("register manifest");

    ActivateContractInstance {
        namespace: "apps".to_string(),
        contract_id: "demo.contract".to_string(),
        code_hash,
    }
    .execute(&authority, &mut stx)
    .expect("activate");

    let data_action = stx
        .world
        .triggers()
        .data_triggers()
        .get(&data_trigger_id)
        .expect("data trigger registered");
    assert_eq!(
        data_action.filter,
        DataEventFilter::Asset(
            AssetEventFilter::new()
                .for_events(AssetEventSet::Added)
                .for_asset_definition(asset_definition),
        )
    );
    assert_contract_trigger_metadata(
        &data_action.metadata,
        "apps",
        "demo.contract",
        "run",
        code_hash,
        &data_trigger_id,
        "tag",
        "data",
    );

    let pipeline_action = stx
        .world
        .triggers()
        .pipeline_triggers()
        .get(&pipeline_trigger_id)
        .expect("pipeline trigger registered");
    assert_eq!(
        pipeline_action.filter,
        PipelineEventFilterBox::Block(BlockEventFilter::default())
    );
    assert_contract_trigger_metadata(
        &pipeline_action.metadata,
        "apps",
        "demo.contract",
        "run",
        code_hash,
        &pipeline_trigger_id,
        "tag",
        "pipeline",
    );

    DeactivateContractInstance {
        namespace: "apps".to_string(),
        contract_id: "demo.contract".to_string(),
        reason: None,
    }
    .execute(&authority, &mut stx)
    .expect("deactivate");

    assert!(
        stx.world
            .triggers()
            .data_triggers()
            .get(&data_trigger_id)
            .is_none()
    );
    assert!(
        stx.world
            .triggers()
            .pipeline_triggers()
            .get(&pipeline_trigger_id)
            .is_none()
    );
    assert!(stx.world.triggers().ids().get(&data_trigger_id).is_none());
    assert!(
        stx.world
            .triggers()
            .ids()
            .get(&pipeline_trigger_id)
            .is_none()
    );
}
