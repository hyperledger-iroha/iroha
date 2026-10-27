//! Core IVM host runtime, syscall and custody regression tests.
use super::*;
use crate::{
    kura::Kura,
    query::store::LiveQueryStore,
    smartcontracts::code::{activate_instance, register_code_bytes, register_manifest},
    smartcontracts::{
        Execute,
        isi::triggers::specialized::{SpecializedAction, SpecializedTrigger},
        ivm::host::pointer_abi_tests::make_tlv,
    },
    state::{ElectionState, State, World, ZkAssetState},
};
use iroha_crypto::{Algorithm, Hash, KeyPair};
use iroha_data_model::query::{
    QueryWithParams,
    account::prelude::FindAccounts,
    dsl::prelude::{CompoundPredicate, SelectorTuple},
    parameters::{FetchSize, ForwardCursor, Pagination, QueryParams, Sorting},
};
use iroha_data_model::{
    parameter::{CustomParameter, Parameter, SmartContractParameter},
    privacy::PrivacyProtocolIdV1,
    proof::{ProofAttachment, VerifyingKeyBox, VerifyingKeyId},
    query::{QueryRequest, QueryResponse, SingularQueryBox, prelude::FindParameters},
    smart_contract::ContractArtifactId,
    zk::BackendTag,
};
use iroha_executor_data_model::permission::account::{
    AccountAliasPermissionScope, CanManageAccountAlias, CanResolveAccountAlias,
};
use iroha_model_base::chain::ChainId;
use iroha_test_samples::{ALICE_ID, ALICE_KEYPAIR, BOB_ID, BOB_KEYPAIR};
use ivm::{IVM, encoding, instruction, syscalls as ivm_sys};
use nonzero_ext::nonzero;
use std::{collections::BTreeMap, sync::Arc};

fn shared_argument_fixture_payload() -> Json {
    let fixture: norito::json::Value = norito::json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../fixtures/kotodama/entrypoint_argument_record_v1.json"
    )))
    .expect("parse shared contract argument fixture");
    let payload = fixture
        .as_object()
        .and_then(|root| root.get("torii_boundary"))
        .and_then(norito::json::Value::as_object)
        .and_then(|boundary| boundary.get("payload"))
        .expect("shared fixture Torii payload")
        .clone();
    Json::from(payload)
}
fn sample_open_verify_envelope() -> iroha_data_model::zk::OpenVerifyEnvelope {
    iroha_data_model::zk::OpenVerifyEnvelope {
        backend: BackendTag::NativePipaRPasta,
        circuit_id: crate::zk::confidential_v2::CONFIDENTIAL_TRANSFER_V2_CIRCUIT_ID.to_owned(),
        vk_hash: [1; 32],
        public_inputs: crate::zk::confidential_v2::CONFIDENTIAL_TRANSFER_V2_PUBLIC_INPUTS_SCHEMA_V1
            .to_vec(),
        proof_bytes: vec![1],
        aux: Vec::new(),
    }
}
pub(super) fn norito_blob<T: NoritoSerialize>(val: &T) -> Vec<u8> {
    norito::to_bytes(val).expect("encode Norito payload")
}
fn state_path_for_test(path: &impl AsRef<str>) -> StatePath {
    path.as_ref().parse().expect("valid durable-state path")
}
fn store_state_path_tlv(vm: &mut IVM, path: &impl AsRef<str>) -> u64 {
    store_tlv(
        vm,
        PointerType::NoritoBytes,
        &norito_blob(&state_path_for_test(path)),
    )
}
fn test_state_path_gas(path: &impl AsRef<str>) -> u64 {
    ivm::host::state_path_gas(norito_blob(&state_path_for_test(path)).len())
}
fn test_state_value_gas(path: &impl AsRef<str>, value_len: usize) -> u64 {
    ivm::host::state_value_gas(norito_blob(&state_path_for_test(path)).len(), value_len)
}
include!("host/core_codec_and_contract_tests.rs");
include!("host/native_event_tests.rs");

#[test]
fn remote_spend_asset_scope_respects_registered_balance_policy() {
    let private = DataSpaceId::new(7);
    let global_scope =
        remote_spend_asset_scope_from_policy(AssetBalancePolicy::Global, DataSpaceId::UNIVERSAL);
    assert_eq!(global_scope, AssetBalanceScope::Global);
    ivm::axt::validate_remote_spend_asset_scope(DataSpaceId::UNIVERSAL, global_scope)
        .expect("global balance is valid only in the universal dataspace");
    assert!(
        ivm::axt::validate_remote_spend_asset_scope(private, global_scope).is_err(),
        "global balance must not be relabelled as private-dataspace state"
    );

    let restricted_scope =
        remote_spend_asset_scope_from_policy(AssetBalancePolicy::DataspaceRestricted, private);
    assert_eq!(restricted_scope, AssetBalanceScope::Dataspace(private));
    ivm::axt::validate_remote_spend_asset_scope(private, restricted_scope)
        .expect("restricted balance uses the exact signed intent dataspace");
}

#[test]
fn deployed_contract_runtime_binding_uses_live_identity_and_exact_artifact() {
    let authority = ALICE_ID.clone();
    let state = contract_test_state(&authority);
    let contract_address = install_contract(
        &state,
        &authority,
        r"
seiyaku RuntimeBinding { permission CanUpdateContract;
  kotoage fn update() authorize(CanUpdateContract) {}
  view fn inspect() authorize(anyone) -> int { return 1; }
}
",
        0,
    );
    let stale_contract_address = install_contract(
        &state,
        &authority,
        r"
seiyaku StaleRuntimeBinding { permission CanUpdateContract;
  kotoage fn update() authorize(CanUpdateContract) {}
}
",
        1,
    );
    grant_contract_entrypoint_to_account(
        &state,
        &authority,
        authority.clone(),
        &contract_address,
        "update",
    );
    let view = state.view();
    let identity =
        crate::smartcontracts::code::fetch_bound_contract_identity(&view, &contract_address)
            .expect("registry read completes")
            .expect("installed contract identity");
    let stale_identity =
        crate::smartcontracts::code::fetch_bound_contract_identity(&view, &stale_contract_address)
            .expect("registry read completes")
            .expect("stale contract identity");
    let code = view
        .world()
        .contract_code()
        .get(&iroha_data_model::smart_contract::ContractArtifactId::new(
            identity.contract_address.dataspace_id().unwrap(),
            identity.code_hash,
        ))
        .expect("installed contract bytecode");
    let stale_code = view
        .world()
        .contract_code()
        .get(&iroha_data_model::smart_contract::ContractArtifactId::new(
            stale_identity.contract_address.dataspace_id().unwrap(),
            stale_identity.code_hash,
        ))
        .expect("stale contract bytecode");
    let mut cache = crate::smartcontracts::ivm::cache::IvmCache::new();
    let summary = cache
        .summarize_program_with_hash(identity.code_hash, code.as_ref())
        .expect("prepare installed contract");
    let stale_summary = cache
        .summarize_program_with_hash(stale_identity.code_hash, stale_code.as_ref())
        .expect("prepare stale contract");
    let expected_subject =
        crate::smartcontracts::code::fetch_bound_contract_subject(&view, &contract_address)
            .expect("installed contract subject");
    let mut host = CoreHost::new(authority.clone());
    host.bind_authorized_deployed_contract_runtime_context(
        &view,
        &contract_address,
        identity.contract_alias.as_ref(),
        summary.prepared_contract(),
        "update",
    )
    .map_err(crate::execution_attempt::expect_completed_rejection)
    .expect("bind callable entrypoint");
    let call_context = host
        .current_contract_runtime_context
        .as_ref()
        .expect("call context");
    assert_eq!(call_context.contract_subject, expected_subject);
    assert_eq!(call_context.entrypoint, "update");
    assert_eq!(
        host.current_entrypoint_authorization
            .as_ref()
            .expect("call authorization")
            .code_hash,
        identity.code_hash
    );
    assert_eq!(
        host.current_entrypoint_authorization
            .as_ref()
            .expect("call authorization")
            .authorization,
        iroha_data_model::smart_contract::manifest::EntrypointAuthorizationV1::Permission(
            "CanUpdateContract".parse().unwrap()
        )
    );
    host.bind_authorized_deployed_contract_view_runtime_context(
        &view,
        &contract_address,
        identity.contract_alias.as_ref(),
        summary.prepared_contract(),
        "inspect",
    )
    .map_err(crate::execution_attempt::expect_completed_rejection)
    .expect("bind view entrypoint");
    assert_eq!(
        host.current_contract_runtime_context
            .as_ref()
            .expect("view context")
            .entrypoint,
        "inspect"
    );
    let error = host
        .bind_authorized_deployed_contract_runtime_context(
            &view,
            &contract_address,
            identity.contract_alias.as_ref(),
            stale_summary.prepared_contract(),
            "update",
        )
        .map_err(crate::execution_attempt::expect_completed_rejection)
        .expect_err("stale prepared code must fail closed");
    assert!(
        matches!(error, ValidationFail::NotPermitted(_)),
        "a cross-artifact substitution must be rejected as unauthorized"
    );
    assert!(host.current_contract_runtime_context.is_none());
    assert!(host.current_entrypoint_authorization.is_none());
}
pub(super) fn grant_named_permission_to_account(
    state: &State,
    authority: &AccountId,
    account_id: &AccountId,
    permission_name: &str,
) {
    let permissions: Vec<Permission> = {
        let view = state.view();
        let world = view.world();
        let mut tokens = Vec::new();
        for (address, code_hash) in world.contract_instances().iter() {
            let artifact = iroha_data_model::smart_contract::ContractArtifactId::for_address(
                address, *code_hash,
            )
            .unwrap();
            let Some(manifest) = world.contract_manifests().get(&artifact) else {
                continue;
            };
            for declaration in &manifest.permissions {
                if declaration.name.as_ref() != permission_name {
                    continue;
                }
                tokens.push(match &declaration.scope {
                    ContractPermissionScopeV1::Instance => CanUseContractPermission {
                        contract: address.clone(),
                        permission: declaration.name.clone(),
                    }
                    .into(),
                    ContractPermissionScopeV1::Chain { permission_name } => {
                        Permission::new(permission_name.to_string(), Json::new(()))
                    }
                });
            }
        }
        tokens.sort();
        tokens.dedup();
        assert!(
            tokens.len() <= 1,
            "ambiguous fixture permission requires an explicit contract address"
        );
        if tokens.is_empty() {
            assert!(
                permission_name != "CanInvokeContractEntrypoint"
                    && permission_name != "CanUseContractPermission",
                "scoped contract permissions require an exact deployed address and selector"
            );
            tokens.push(Permission::new(permission_name.to_owned(), Json::new(())));
        }
        tokens
    };
    let next_height = u64::try_from((state.view().height() + 1).max(2))
        .ok()
        .and_then(core::num::NonZeroU64::new)
        .expect("next block height must fit in u64 and be non-zero");
    let mut block = state.block(BlockHeader::new(
        next_height,
        state.view().latest_block_hash(),
        None,
        0,
        0,
    ));
    let mut tx = block.transaction();
    if tx.world.account(account_id).is_err() {
        Register::account(Account::new(account_id.clone()))
            .execute(authority, &mut tx)
            .expect("register permission holder account");
    }
    for permission in permissions {
        if !tx
            .world
            .account_permissions
            .get(account_id)
            .is_some_and(|held| held.contains(&permission))
        {
            Grant::account_permission(permission, account_id.clone())
                .execute(authority, &mut tx)
                .expect("grant declared contract permission fixture");
        }
    }
    tx.apply();
    block
        .commit_world_overlay_for_testing()
        .expect("commit contract permission grant block");
}
/// Grant only the exact deployed address and selector consumed by a host fixture.
fn grant_contract_entrypoint_to_account(
    state: &State,
    authority: &AccountId,
    account_id: AccountId,
    contract: &ContractAddress,
    entrypoint: &str,
) {
    assert!(
        !entrypoint.is_empty() && entrypoint.trim() == entrypoint,
        "fixture entrypoint selectors must use a non-empty canonical spelling"
    );
    let permission: Permission = {
        let view = state.view();
        let hash = *view
            .world()
            .contract_instances()
            .get(contract)
            .expect("installed instance");
        let artifact =
            iroha_data_model::smart_contract::ContractArtifactId::for_address(contract, hash)
                .unwrap();
        let manifest = view
            .world()
            .contract_manifests()
            .get(&artifact)
            .expect("signed manifest");
        let entry = manifest
            .entrypoints
            .as_ref()
            .unwrap()
            .iter()
            .find(|entry| entry.name == entrypoint)
            .expect("declared entrypoint");
        match &entry.authorization {
            EntrypointAuthorizationV1::Anyone => return,
            EntrypointAuthorizationV1::RuntimeLifecycle => CanInvokeContractEntrypoint {
                contract: contract.clone(),
                entrypoint: entrypoint.to_owned(),
            }
            .into(),
            EntrypointAuthorizationV1::Permission(name) => {
                let declaration = manifest
                    .permissions
                    .iter()
                    .find(|item| &item.name == name)
                    .expect("declared permission");
                match &declaration.scope {
                    ContractPermissionScopeV1::Instance => CanUseContractPermission {
                        contract: contract.clone(),
                        permission: name.clone(),
                    }
                    .into(),
                    ContractPermissionScopeV1::Chain { permission_name } => {
                        Permission::new(permission_name.to_string(), Json::new(()))
                    }
                }
            }
        }
    };
    let next_height = u64::try_from((state.view().height() + 1).max(2))
        .ok()
        .and_then(core::num::NonZeroU64::new)
        .expect("next entrypoint grant height must fit in u64 and be non-zero");
    let mut block = state.block(BlockHeader::new(
        next_height,
        state.view().latest_block_hash(),
        None,
        0,
        0,
    ));
    let mut tx = block.transaction();
    if tx.world.account(&account_id).is_err() {
        Register::account(Account::new(account_id.clone()))
            .execute(authority, &mut tx)
            .expect("register exact entrypoint permission holder account");
    }
    if !tx
        .world
        .account_permissions
        .get(&account_id)
        .is_some_and(|permissions| permissions.contains(&permission))
    {
        Grant::account_permission(permission, account_id)
            .execute(authority, &mut tx)
            .expect("grant exact deployed contract entrypoint permission");
    }
    tx.apply();
    block
        .commit_world_overlay_for_testing()
        .expect("commit exact deployed contract entrypoint permission grant");
}
fn grant_asset_ops_to_account(state: &State, authority: &AccountId, account_id: &AccountId) {
    grant_named_permission_to_account(state, authority, account_id, "AssetOps");
}
fn grant_test_asset_transfer(state: &State, account_id: AccountId, asset: AssetId) {
    let owner = asset.account().clone();
    let next_height = u64::try_from((state.view().height() + 1).max(2))
        .ok()
        .and_then(core::num::NonZeroU64::new)
        .expect("next permission grant height");
    let mut block = state.block(BlockHeader::new(next_height, None, None, 0, 0));
    let mut tx = block.transaction();
    Grant::account_permission(
        iroha_executor_data_model::permission::asset::CanTransferAsset { asset },
        account_id,
    )
    .execute(&owner, &mut tx)
    .expect("grant exact source-asset transfer permission to contract subject");
    tx.apply();
    block
        .commit_world_overlay_for_testing()
        .expect("commit exact source-asset permission grant");
}
fn sanitize_test_contract_artifact_wildcards(code: &mut [u8]) {
    let parsed = ivm::ProgramMetadata::parse(code).expect("parse compiled test contract");
    let section_start = parsed.header_len;
    let section_header_end = section_start
        .checked_add(8)
        .expect("CNTR section header end must fit");
    assert!(
        code.len() >= section_header_end && &code[section_start..section_start + 4] == b"CNTR",
        "test contract artifacts must include a CNTR section"
    );
    let payload_len = u32::from_le_bytes(
        code[section_start + 4..section_header_end]
            .try_into()
            .expect("CNTR length field"),
    ) as usize;
    let payload_start = section_header_end;
    let payload_end = payload_start
        .checked_add(payload_len)
        .expect("CNTR payload end must fit");
    assert!(payload_end <= code.len(), "CNTR payload must fit");
    let mut contract_interface = parsed
        .contract_interface
        .expect("compiled test contract must decode CNTR metadata");
    if let Some(hints) = contract_interface.access_set_hints.as_mut() {
        sanitize_test_access_keys(&mut hints.read_keys);
        sanitize_test_access_keys(&mut hints.write_keys);
        for hint in &mut hints.dynamic_reads {
            sanitize_test_dynamic_access_base(&mut hint.base_key);
        }
        for hint in &mut hints.dynamic_writes {
            sanitize_test_dynamic_access_base(&mut hint.base_key);
        }
    }
    for entrypoint in &mut contract_interface.entrypoints {
        sanitize_test_access_keys(&mut entrypoint.read_keys);
        sanitize_test_access_keys(&mut entrypoint.write_keys);
    }
    let sanitized_section = contract_interface.encode_section();
    assert_eq!(
        sanitized_section.len(),
        payload_end - section_start,
        "sanitized CNTR metadata must preserve section length"
    );
    code[section_start..payload_end].copy_from_slice(&sanitized_section);
}
fn rewrite_test_contract_interface(
    code: &mut Vec<u8>,
    customize: impl FnOnce(&mut ivm::EmbeddedContractInterfaceV1),
) {
    let parsed = ivm::ProgramMetadata::parse(code).expect("parse compiled test contract");
    let section_start = parsed.header_len;
    let section_header_end = section_start
        .checked_add(8)
        .expect("CNTR section header end must fit");
    assert!(
        code.len() >= section_header_end && &code[section_start..section_start + 4] == b"CNTR",
        "test contract artifacts must include a CNTR section"
    );
    let payload_len = u32::from_le_bytes(
        code[section_start + 4..section_header_end]
            .try_into()
            .expect("CNTR length field"),
    ) as usize;
    let section_end = section_header_end
        .checked_add(payload_len)
        .expect("CNTR payload end must fit");
    assert!(section_end <= code.len(), "CNTR payload must fit");
    let mut contract_interface = parsed
        .contract_interface
        .expect("compiled test contract must decode CNTR metadata");
    customize(&mut contract_interface);
    let replacement = contract_interface.encode_section();
    let mut rewritten = code[..section_start].to_vec();
    rewritten.extend_from_slice(&replacement);
    if let Some(literal) = parsed.literal_section {
        assert!(
            literal.start >= section_end,
            "literal table must follow the CNTR section"
        );
        rewritten.extend_from_slice(&code[section_end..literal.start]);
        let new_literal_start = rewritten.len();
        // The LTLB post-padding count is canonical relative to the complete metadata prefix.
        // Re-encoding CNTR can change that prefix length even when the executable and literal
        // payloads are untouched, so carry the table without its old padding and recompute it.
        rewritten.extend_from_slice(&code[literal.start..literal.data_end]);
        let entries_len = literal
            .count
            .checked_mul(8)
            .expect("literal descriptor byte length must fit");
        let data_len = literal
            .data_end
            .checked_sub(literal.data_start)
            .expect("literal data range must be ordered");
        let unpadded_len_from_header = new_literal_start
            .checked_sub(parsed.header_len)
            .and_then(|prefix_len| prefix_len.checked_add(16))
            .and_then(|len| len.checked_add(entries_len))
            .and_then(|len| len.checked_add(data_len))
            .expect("rewritten literal prefix length must fit");
        let post_pad = (4 - (unpadded_len_from_header % 4)) % 4;
        let post_pad_field = new_literal_start
            .checked_add(8)
            .expect("rewritten LTLB padding field must fit");
        rewritten[post_pad_field..post_pad_field + 4].copy_from_slice(
            &u32::try_from(post_pad)
                .expect("LTLB padding is at most three bytes")
                .to_le_bytes(),
        );
        rewritten.resize(
            rewritten
                .len()
                .checked_add(post_pad)
                .expect("rewritten LTLB padding length must fit"),
            0,
        );
        rewritten.extend_from_slice(&code[literal.code_offset..]);
    } else {
        rewritten.extend_from_slice(&code[section_end..]);
    }
    *code = rewritten;
}
fn sanitize_test_access_keys(keys: &mut [String]) {
    for key in keys {
        sanitize_test_access_key(key);
    }
}
fn sanitize_test_access_key(key: &mut String) {
    if key == "*" {
        *key = "x".to_owned();
    } else {
        sanitize_test_dynamic_access_base(key);
    }
}
fn sanitize_test_dynamic_access_base(base_key: &mut String) {
    if base_key == "state:*" {
        *base_key = "state:x".to_owned();
    }
}
fn execute_contract_call_transaction_result(
    state: &State,
    authority: &AccountId,
    keypair: &KeyPair,
    invocation: iroha_data_model::transaction::executable::ContractInvocation,
    ivm_cache: &mut crate::smartcontracts::ivm::cache::IvmCache,
) -> Result<(), crate::execution_attempt::ExecutionAttemptError<iroha_data_model::ValidationFail>> {
    // This helper executes ordinary signed calls after the fixture's bootstrap.
    // Committing only a world overlay does not advance the durable block height.
    if state.view().height() == 0 {
        state
            .block(BlockHeader::new(nonzero!(1_u64), None, None, 0, 0))
            .commit_empty_block_for_testing()
            .expect("commit fixture bootstrap before ordinary contract execution");
    }
    let dataspace = invocation
        .contract_address
        .dataspace_id()
        .expect("fixture contract address identifies its execution dataspace");
    let next_height = u64::try_from(state.view().height() + 1)
        .ok()
        .and_then(core::num::NonZeroU64::new)
        .expect("next block height must fit in u64 and be non-zero");
    let tx = TransactionBuilder::new(
        state.network_id,
        authority.clone(),
        iroha_data_model::transaction::FeePaymentIntent::authority(
            Vec::new(),
            core::num::NonZeroU64::new(1_000_000),
        ),
    )
    .with_executable(Executable::ContractCall(invocation))
    .sign(keypair.private_key());
    let mut block = state.block(BlockHeader::new(
        next_height,
        state.view().latest_block_hash(),
        None,
        0,
        0,
    ));
    let mut stx =
        block.transaction_for_fastpq_testing(iroha_crypto::Hash::from(tx.hash_as_entrypoint()));
    stx.current_dataspace_id = Some(dataspace);
    stx.world.current_dataspace_id = Some(dataspace);
    let result =
        crate::executor::Executor::Initial.execute_transaction(&mut stx, authority, &tx, ivm_cache);
    if result.is_ok() {
        stx.apply();
        block
            .commit_world_overlay_for_testing()
            .expect("commit contract call transaction block");
    }
    result
}
fn execute_contract_call_transaction(
    state: &State,
    authority: &AccountId,
    keypair: &KeyPair,
    invocation: iroha_data_model::transaction::executable::ContractInvocation,
    ivm_cache: &mut crate::smartcontracts::ivm::cache::IvmCache,
) {
    execute_contract_call_transaction_result(state, authority, keypair, invocation, ivm_cache)
        .expect("contract call transaction should execute");
}
fn encoded_contract_arguments_from_json(
    state: &State,
    contract_address: &ContractAddress,
    entrypoint: &str,
    payload: &Json,
) -> Option<Vec<u8>> {
    let view = state.view();
    let code_hash = view
        .world()
        .contract_instances()
        .get(contract_address)
        .copied()
        .expect("installed contract binding");
    let artifact_id = ContractArtifactId::for_address(contract_address, code_hash)
        .expect("installed contract has an exact artifact dataspace");
    let code = view
        .world()
        .contract_code()
        .get(&artifact_id)
        .expect("installed contract code");
    let parsed = ivm::ProgramMetadata::parse(code).expect("parse installed contract");
    let descriptor = parsed
        .contract_interface
        .as_ref()
        .expect("installed contract interface")
        .entrypoints
        .iter()
        .find(|descriptor| descriptor.name == entrypoint)
        .expect("installed contract entrypoint");
    descriptor.argument_schema.as_ref().map(|schema| {
        ivm_abi::arguments::encode_argument_record_from_json(schema, payload)
            .expect("encode test contract arguments")
    })
}
fn contract_invocation_from_json(
    state: &State,
    contract_address: ContractAddress,
    entrypoint: &str,
    payload: &Json,
) -> iroha_data_model::transaction::executable::ContractInvocation {
    let arguments =
        encoded_contract_arguments_from_json(state, &contract_address, entrypoint, payload);
    let expected_code_hash =
        crate::smartcontracts::code::fetch_instance_binding(&state.view(), &contract_address)
            .expect("registry read completes")
            .expect("installed test contract binding");
    iroha_data_model::transaction::executable::ContractInvocation {
        contract_address,
        expected_code_hash,
        entrypoint: entrypoint.to_owned(),
        arguments: arguments.map(|arguments| {
            iroha_data_model::transaction::executable::ContractArgumentRecord::try_new(arguments)
                .expect("bounded test contract arguments")
        }),
    }
}
fn install_alias_payout_contract(
    state: &State,
    authority: &AccountId,
    recipient_expr: &str,
    nonce: u64,
) -> ContractAddress {
    let source = format!(
        r#"
seiyaku AliasPayout {{ permission AssetOps;
  state AssetDefinitionId SettlementAsset;

  hajimari(AssetDefinitionId settlement_asset) {{
    SettlementAsset = settlement_asset;
  }}

  kotoage fn bind(AssetDefinitionId settlement_asset) authorize(AssetOps) {{
    SettlementAsset = settlement_asset;
  }}

  kotoage fn pay(quantity amount) authorize(AssetOps) -> quantity {{
    let merchant = {recipient_expr};
    ledger::asset::transfer(source: context::authority(), destination: merchant, asset_definition: SettlementAsset, amount: amount, dataspace: DataSpaceId::parse("0"));
    return amount;
  }}
}}
"#
    );
    let contract = install_contract(state, authority, &source, nonce);
    grant_asset_ops_to_account(state, authority, authority);
    contract
}
fn bind_nested_caller<QS: Default + QueryStateAccess>(
    host: &mut CoreHostImpl<'_, '_, QS>,
    vm: &mut IVM,
    state: &impl StateReadOnly,
    authority: &AccountId,
    caller: &ContractAddress,
) {
    let identity = crate::smartcontracts::code::fetch_bound_contract_identity(state, caller)
        .unwrap()
        .unwrap();
    let artifact = ContractArtifactId::new(caller.dataspace_id().unwrap(), identity.code_hash);
    let manifest = state.world().contract_manifests().get(&artifact).unwrap();
    let entries = manifest.entrypoints.as_ref().unwrap();
    let entry = entries.iter().find(|entry| entry.name == "main").or_else(|| entries.iter().find(|entry| !matches!(entry.authorization, iroha_data_model::smart_contract::manifest::EntrypointAuthorizationV1::RuntimeLifecycle))).unwrap();
    vm.load_program(state.world().contract_code().get(&artifact).unwrap())
        .unwrap();
    vm.select_entrypoint(&entry.name).unwrap();
    host.set_contract_runtime_context(Some(ContractRuntimeExecutionContext {
        contract_address: caller.clone(),
        contract_subject: caller.subject_id(),
        contract_alias: identity.contract_alias.clone(),
        entrypoint: entry.name.clone(),
    }));
    host.execution_class =
        if entry.kind == iroha_data_model::smart_contract::manifest::EntryPointKind::View {
            HostExecutionClass::View
        } else {
            HostExecutionClass::Contract
        };
    host.set_contract_entrypoint_authorization(Some(ContractEntrypointAuthorizationSnapshot::new(
        authority.clone(),
        entry.name.clone(),
        entry.authorization.clone(),
        &identity,
    )));
}

fn store_typed_contract_call(
    vm: &mut IVM,
    state: &State,
    target: &ContractAddress,
    selector: &str,
    payload: &Json,
) -> [u64; 6] {
    let gas = vm.remaining_gas();
    let view = state.view();
    let code_hash = *view
        .world()
        .contract_instances()
        .get(target)
        .expect("installed callee");
    let artifact = ContractArtifactId::new(target.dataspace_id().unwrap(), code_hash);
    let manifest = view
        .world()
        .contract_manifests()
        .get(&artifact)
        .expect("authenticated callee manifest");
    let entries = manifest.entrypoints.as_ref().unwrap();
    let ordinal = entries
        .iter()
        .position(|entry| entry.name == selector)
        .unwrap_or(usize::MAX);
    let entry = entries.get(ordinal);
    let result_words = entry
        .and_then(|entry| entry.return_schema.as_ref())
        .and_then(ivm::EntrypointValueTypeV1::word_count)
        .unwrap_or(1);
    let (argument_base, argument_words, result_base) = if let Some(schema) =
        entry.and_then(|entry| entry.argument_schema.as_ref())
    {
        let record = ivm_abi::arguments::encode_argument_record_from_json(schema, payload).unwrap();
        let prepared = ivm::prepare_argument_record_with_gas_limit(
            schema,
            Arc::<[u8]>::from(record),
            u64::MAX,
        )
        .unwrap();
        prepared.precharge_vm(vm).unwrap();
        prepared.install_call_arguments(vm, result_words).unwrap();
        (vm.register(10), vm.register(11), vm.register(12))
    } else {
        (0, 0, vm.alloc_heap((result_words * 8) as u64).unwrap())
    };
    let address = store_tlv(vm, PointerType::Blob, target.as_ref().as_bytes());
    let binding = ivm_abi::contract_call::ContractCallBindingV1 {
        code_hash,
        entrypoint: u32::try_from(ordinal).unwrap_or(u32::MAX),
    };
    let binding = store_tlv(vm, PointerType::NoritoBytes, &binding.to_bytes().unwrap());
    // Preparing fixture memory is not part of the measured host invocation.
    vm.set_gas_limit(gas);
    let registers = [
        address,
        binding,
        argument_base,
        argument_words,
        result_base,
        result_words as u64,
    ];
    for (index, word) in registers.iter().enumerate() {
        vm.set_register(10 + index, *word);
    }
    registers
}

fn typed_contract_request_bytes(state: &State, target: &ContractAddress, selector: &str) -> usize {
    let view = state.view();
    let code_hash = *view.world().contract_instances().get(target).unwrap();
    let artifact = ContractArtifactId::new(target.dataspace_id().unwrap(), code_hash);
    let manifest = view.world().contract_manifests().get(&artifact).unwrap();
    let entrypoint = manifest
        .entrypoints
        .as_ref()
        .unwrap()
        .iter()
        .position(|entry| entry.name == selector)
        .unwrap();
    let binding = ivm_abi::contract_call::ContractCallBindingV1 {
        code_hash,
        entrypoint: u32::try_from(entrypoint).expect("fixture value fits u32"),
    };
    target.as_ref().len() + binding.to_bytes().unwrap().len()
}

fn typed_dispatch_setup_gas() -> u64 {
    use ivm::{
        encoding::wide,
        instruction::wide::{arithmetic, system},
    };
    authenticated_test_probe_setup_gas()
        + (0..6)
            .map(|index| {
                ivm::gas::cost_of(wide::encode_ri(arithmetic::ADDI, 10 + index, 18 + index, 0))
                    .unwrap()
            })
            .sum::<u64>()
        + ivm::gas::cost_of(wide::encode_sys(
            system::SCALL,
            u8::try_from(ivm_sys::SYSCALL_CALL_CONTRACT).expect("fixture value fits u8"),
        ))
        .unwrap()
}

fn captured_nested_result(
    vm: &IVM,
    schema: &iroha_data_model::smart_contract::entrypoint::EntrypointValueTypeV1,
) -> iroha_data_model::smart_contract::entrypoint::EntrypointReturnRecordV1 {
    ivm::value_record::capture_value_record(
        vm,
        schema,
        vm.register(14),
        usize::try_from(vm.register(15)).expect("fixture value fits usize"),
    )
    .expect("canonical typed A9 result table")
}
fn render_nested_result(
    vm: &IVM,
    kind: iroha_data_model::smart_contract::entrypoint::EntrypointValueKindV1,
) -> norito::json::Value {
    let schema = exact_return_type(kind);
    ivm::value_record::render_entrypoint_return_record(
        &schema,
        &captured_nested_result(vm, &schema),
    )
    .unwrap()
}
fn call_contract_syscall(
    state: &State,
    outer_authority: &AccountId,
    caller_contract: &ContractAddress,
    nested_contract: &ContractAddress,
    entrypoint: &str,
    payload: &Json,
) -> SyscallOutcome {
    call_contract_syscall_with_prepared_cache(
        state,
        outer_authority,
        caller_contract,
        nested_contract,
        entrypoint,
        payload,
        PreparedContractCache::default(),
    )
}
fn call_contract_syscall_with_prepared_cache(
    state: &State,
    outer_authority: &AccountId,
    caller_contract: &ContractAddress,
    nested_contract: &ContractAddress,
    entrypoint: &str,
    payload: &Json,
    prepared_cache: PreparedContractCache,
) -> SyscallOutcome {
    let view = state.view();
    let mut host = CoreHostImpl::new(outer_authority.clone());
    host.set_prepared_contract_cache(prepared_cache);
    host.set_query_state(&view);
    host.set_durable_state_snapshot_from_world(view.world());
    let mut vm = IVM::new(1_000_000);
    bind_nested_caller(&mut host, &mut vm, &view, outer_authority, caller_contract);
    let registers = store_typed_contract_call(&mut vm, state, nested_contract, entrypoint, payload);
    let result = host.syscall(ivm_sys::SYSCALL_CALL_CONTRACT, &mut vm);
    assert_eq!(
        (10..16).map(|index| vm.register(index)).collect::<Vec<_>>(),
        registers,
        "A9 preserves all input descriptors"
    );
    let durable_state_overlay = host.drain_durable_state_overlay();
    (result, vm, durable_state_overlay)
}
fn call_contract_syscall_access_log(
    state: &State,
    outer_authority: &AccountId,
    caller_contract: &ContractAddress,
    nested_contract: &ContractAddress,
    entrypoint: &str,
    payload: &Json,
) -> (Result<u64, ivm::VMError>, ivm::host::AccessLog) {
    let view = state.view();
    let mut host = CoreHostImpl::new(outer_authority.clone()).with_access_logging();
    host.set_query_state(&view);
    host.set_durable_state_snapshot_from_world(view.world());
    host.begin_tx(&ivm::parallel::StateAccessSet::default())
        .unwrap();
    let mut vm = IVM::new(1_000_000);
    bind_nested_caller(&mut host, &mut vm, &view, outer_authority, caller_contract);
    store_typed_contract_call(&mut vm, state, nested_contract, entrypoint, payload);
    let result = host.syscall(ivm_sys::SYSCALL_CALL_CONTRACT, &mut vm);
    let access_log = host.finish_tx().expect("finish nested access log");
    (result, access_log)
}
fn dispatch_call_contract_syscall(
    state: &State,
    outer_authority: &AccountId,
    caller_contract: &ContractAddress,
    nested_contract: &ContractAddress,
    entrypoint: &str,
    payload: &Json,
    gas_limit: u64,
) -> ContractCallOutcome {
    dispatch_call_contract_syscall_with_cycle_budget(
        state,
        outer_authority,
        caller_contract,
        nested_contract,
        entrypoint,
        payload,
        (gas_limit, None),
    )
}
fn install_nested_dispatch_fixture(state: &State, authority: &AccountId, caller: &ContractAddress) {
    use ivm::{encoding::wide, instruction::wide::arithmetic};
    let mut code = Vec::new();
    // The generic authenticated probe owns its root result table. Separate
    // staging registers preserve all six typed-call operands across root setup.
    for index in 0..6 {
        code.extend_from_slice(
            &wide::encode_ri(arithmetic::ADDI, 10 + index, 18 + index, 0).to_le_bytes(),
        );
    }
    code.extend_from_slice(
        &wide::encode_sys(
            ivm::instruction::wide::system::SCALL,
            u8::try_from(ivm_sys::SYSCALL_CALL_CONTRACT).expect("fixture value fits u8"),
        )
        .to_le_bytes(),
    );
    let program = build_authenticated_test_contract_program(&code, 0, false);
    rebind_test_contract_program(state, authority, caller, program);
}

fn rebind_test_contract_program(
    state: &State,
    authority: &AccountId,
    caller: &ContractAddress,
    program: Vec<u8>,
) {
    let verified =
        ivm::verify_contract_artifact(&program).expect("admit exact typed dispatch fixture");
    let view = state.view();
    if view.world().contract_instances().get(caller) == Some(&verified.code_hash) {
        return;
    }
    let revision = view
        .world()
        .contract_subject_bindings()
        .get(caller)
        .unwrap()
        .lifecycle
        .revision;
    let height = u64::try_from((view.height() + 1).max(2))
        .unwrap()
        .try_into()
        .unwrap();
    let previous = view.latest_block_hash();
    drop(view);
    let signing = crate::manifest_signing_test_support::ManifestSigningFixture::new();
    let manifest = verified
        .manifest
        .try_signed(
            signing.context(),
            signing.max_frame_bytes(),
            &fixture_signing_keypair(authority),
        )
        .unwrap();
    let mut block = state.block(BlockHeader::new(height, previous, None, 0, 0));
    let mut tx = block.transaction();
    let hash = register_code_bytes(authority, DataSpaceId::UNIVERSAL, program, &mut tx).unwrap();
    register_manifest(authority, DataSpaceId::UNIVERSAL, manifest, &mut tx).unwrap();
    activate_instance(authority, caller.clone(), revision, hash, &mut tx).unwrap();
    tx.apply();
    block.commit_world_overlay_for_testing().unwrap();
}
fn dispatch_call_contract_syscall_with_cycle_budget(
    state: &State,
    outer_authority: &AccountId,
    caller_contract: &ContractAddress,
    nested_contract: &ContractAddress,
    entrypoint: &str,
    payload: &Json,
    (gas_limit, cycle_budget): (u64, Option<&ivm::VmCycleBudget>),
) -> ContractCallOutcome {
    install_nested_dispatch_fixture(state, outer_authority, caller_contract);
    let view = state.view();
    let mut host = CoreHostImpl::new(outer_authority.clone());
    host.set_query_state(&view);
    host.set_durable_state_snapshot_from_world(view.world());
    let mut vm = IVM::new(gas_limit.max(1_000_000));
    bind_nested_caller(&mut host, &mut vm, &view, outer_authority, caller_contract);
    let registers = store_typed_contract_call(&mut vm, state, nested_contract, entrypoint, payload);
    for (index, value) in registers.iter().enumerate() {
        vm.set_register(18 + index, *value);
    }
    let target_ptr = registers[0];
    vm.set_register(14, target_ptr);
    vm.set_gas_limit(gas_limit);
    let result = match cycle_budget {
        Some(budget) => vm.run_with_host_and_cycle_budget(&mut host, budget),
        None => vm.run_with_host(&mut host),
    };
    let durable_state_overlay = host.drain_durable_state_overlay();
    (result, vm, durable_state_overlay, target_ptr)
}
fn install_typed_view_fixture(
    state: &State,
    authority: &AccountId,
    actual_asset: &AssetDefinitionId,
    actual_account: &AccountId,
) -> (ContractAddress, ContractAddress) {
    let pool_contract = install_contract(
        state,
        authority,
        r"
seiyaku TypedPoolViews { permission AssetOps;
  state AssetDefinitionId QuoteAsset;
  state AccountId PoolAccount;

  hajimari(AssetDefinitionId initial_quote_asset, AccountId initial_pool_account) {
    QuoteAsset = initial_quote_asset;
    PoolAccount = initial_pool_account;
  }

  kotoage fn bind(AssetDefinitionId bound_quote_asset, AccountId bound_pool_account) authorize(AssetOps) {
    QuoteAsset = bound_quote_asset;
    PoolAccount = bound_pool_account;
  }

  view fn quote_asset() authorize(anyone) -> AssetDefinitionId {
    return QuoteAsset;
  }

  view fn pool_account() authorize(anyone) -> AccountId {
    return PoolAccount;
  }
}
",
        0,
    );
    let outer_caller = install_contract(
        state,
        authority,
        r"
seiyaku OuterCaller {
  view fn main() authorize(anyone) -> int { return 0; }
}
",
        1,
    );
    grant_asset_ops_to_account(state, authority, authority);
    grant_asset_ops_to_account(state, authority, &(outer_caller.subject_id()));
    let mut ivm_cache = crate::smartcontracts::ivm::cache::IvmCache::new();
    let pool_bind_payload = Json::from_str_norito(&format!(
        r#"{{"bound_pool_account":"{actual_account}","bound_quote_asset":"{actual_asset}"}}"#
    ))
    .expect("pool bind payload");
    execute_contract_call_transaction(
        state,
        authority,
        &fixture_signing_keypair(authority),
        contract_invocation_from_json(state, pool_contract.clone(), "bind", &pool_bind_payload),
        &mut ivm_cache,
    );
    let view = state.view();
    let persisted = |suffix: &str| {
        view.world()
            .smart_contract_state()
            .iter()
            .find(|(key, _)| key.as_ref().ends_with(suffix))
            .map_or_else(
                || panic!("missing persisted typed binding state {suffix}"),
                |(_, value)| value.clone(),
            )
    };
    assert!(!persisted("/QuoteAsset").is_empty());
    assert!(!persisted("/PoolAccount").is_empty());
    (outer_caller, pool_contract)
}
include!("host/core_query_execution_tests.rs");
include!("host/query_account_filter_tests.rs");
include!("host/core_query_pagination_tests.rs");
#[test]
#[allow(clippy::too_many_lines)]
fn subscription_bill_fixed_plan_transfers_and_reschedules() {
    let provider = fixture_account_in_domain("acme", "commerce");
    let subscriber = fixture_account_in_domain("alice", "users");
    let plan_id: AssetDefinitionId =
        iroha_data_model::asset::AssetDefinitionId::derive_from_components(
            DomainId::try_new("commerce", "universal").unwrap(),
            "fixed_plan".parse().unwrap(),
        );
    let charge_asset_id: AssetDefinitionId =
        iroha_data_model::asset::AssetDefinitionId::derive_from_components(
            DomainId::try_new("pay", "universal").unwrap(),
            "usd".parse().unwrap(),
        );
    let period_ms = 1_000_u64;
    let scheduled_at_ms = 10_000_u64;
    let trigger_id: TriggerId = "sub-bill".parse().unwrap();
    let amount = Quantity::from(120_u32);
    let plan = SubscriptionPlan {
        provider: provider.clone(),
        billing: SubscriptionBilling {
            cadence: SubscriptionCadence::FixedPeriod(SubscriptionFixedPeriodCadence { period_ms }),
            bill_for: SubscriptionBillFor::PreviousPeriod,
            retry_backoff_ms: 500,
            max_failures: 3,
            grace_ms: 5_000,
        },
        pricing: SubscriptionPricing::Fixed(SubscriptionFixedPricing {
            amount: amount.clone(),
            asset_definition: charge_asset_id.clone(),
        }),
    };
    let mut plan_def = AssetDefinition::new(
        plan_id.clone(),
        "fixed_plan".to_owned(),
        NumericSpec::integer(),
        iroha_data_model::asset::AssetBalancePolicy::Global,
        None,
    )
    .build(&provider);
    let plan_key: Name = SUBSCRIPTION_PLAN_METADATA_KEY.parse().unwrap();
    plan_def.metadata.insert(plan_key, Json::new(plan.clone()));
    let charge_def = AssetDefinition::new(
        charge_asset_id.clone(),
        "usd".to_owned(),
        NumericSpec::integer(),
        iroha_data_model::asset::AssetBalancePolicy::Global,
        None,
    )
    .build(&provider);
    let asset_id = AssetId::of(charge_asset_id.clone(), subscriber.clone());
    let asset = Asset::new(asset_id.clone(), Quantity::from(500_u32));
    let subscription_state = SubscriptionState {
        plan_id: plan_id.clone(),
        provider: provider.clone(),
        subscriber: subscriber.clone(),
        status: SubscriptionStatus::Active,
        current_period_start_ms: scheduled_at_ms - period_ms,
        current_period_end_ms: scheduled_at_ms,
        next_charge_ms: scheduled_at_ms,
        cancel_at_period_end: false,
        cancel_at_ms: None,
        failure_count: 0,
        usage_accumulated: BTreeMap::new(),
        billing_trigger_id: trigger_id.clone(),
    };
    let mut nft_meta = Metadata::default();
    let subscription_key: Name = SUBSCRIPTION_METADATA_KEY.parse().unwrap();
    nft_meta.insert(
        subscription_key.clone(),
        Json::new(subscription_state.clone()),
    );
    let nft_id: NftId = "sub-0$subscriptions.universal".parse().unwrap();
    let nft = Nft::new(nft_id.clone(), nft_meta).build(&subscriber);
    let domains = vec![
        Domain::new(DomainId::try_new("commerce", "universal").unwrap()).build(&provider),
        Domain::new(DomainId::try_new("users", "universal").unwrap()).build(&provider),
        Domain::new(DomainId::try_new("pay", "universal").unwrap()).build(&provider),
        Domain::new(DomainId::try_new("subscriptions", "universal").unwrap()).build(&provider),
    ];
    let accounts = vec![
        build_fixture_account(&provider, &provider),
        build_fixture_account(&subscriber, &provider),
    ];
    let world = World::with_assets(domains, accounts, [plan_def, charge_def], [asset], [nft]);
    let bytecode = IvmBytecode::from_compiled(ivm::ProgramMetadata::default().encode());
    let mut trigger_metadata = Metadata::default();
    let trigger_ref_key: Name = SUBSCRIPTION_TRIGGER_REF_METADATA_KEY.parse().unwrap();
    trigger_metadata.insert(
        trigger_ref_key,
        Json::new(SubscriptionTriggerRef {
            subscription_nft_id: nft_id.clone(),
        }),
    );
    // Runtime registration metadata is present on real triggers and must be stripped
    // before re-registering, otherwise registration rejects reserved keys.
    trigger_metadata.insert(
        "__registered_block_height".parse().unwrap(),
        Json::from(42_u64),
    );
    trigger_metadata.insert(
        "__registered_at_ms".parse().unwrap(),
        Json::from(12_345_u64),
    );
    let schedule = Schedule {
        start_ms: scheduled_at_ms,
        period_ms: None,
    };
    let mut action = SpecializedAction::new(
        Executable::Ivm(bytecode.clone()),
        Repeats::Exactly(1),
        subscriber.clone(),
        TimeEventFilter(ExecutionTime::Schedule(schedule)),
    )
    .expect("test scheduled trigger action satisfies its authority invariant");
    action.metadata = trigger_metadata.clone();
    let trigger = SpecializedTrigger::new(trigger_id.clone(), action);
    {
        let mut block = world.triggers.block();
        let mut tx = block.transaction();
        tx.add_time_trigger(trigger).expect("add trigger");
        tx.apply();
        block.commit();
    }
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let state = State::new_for_testing(world, kura, query);
    let view = state.view();
    let mut host = CoreHostImpl::new(subscriber.clone());
    #[cfg(feature = "telemetry")]
    let metrics = Arc::new(iroha_telemetry::metrics::Metrics::default());
    #[cfg(feature = "telemetry")]
    host.set_telemetry(StateTelemetry::new(Arc::clone(&metrics), true));
    host.set_query_state(&view);
    host.set_trigger_id(trigger_id.clone());
    host.set_block_time_ms(scheduled_at_ms);
    let mut vm = IVM::new(1_000_000);
    let gas = host
        .syscall(ivm_sys::SYSCALL_SUBSCRIPTION_BILL, &mut vm)
        .expect("billing");
    let mut expected_state = subscription_state.clone();
    expected_state.current_period_start_ms = scheduled_at_ms;
    expected_state.current_period_end_ms = scheduled_at_ms + period_ms;
    expected_state.next_charge_ms = scheduled_at_ms + period_ms;
    expected_state.failure_count = 0;
    expected_state.status = SubscriptionStatus::Active;
    let expected_transfer = InstructionBox::from(Transfer::asset_quantity(
        asset_id,
        amount.clone(),
        provider.clone(),
    ));
    let expected_set = InstructionBox::from(SetKeyValue::nft(
        nft_id.clone(),
        subscription_key,
        Json::new(expected_state),
    ));
    let expected_invoice = SubscriptionInvoice {
        subscription_nft_id: nft_id.clone(),
        period_start_ms: scheduled_at_ms - period_ms,
        period_end_ms: scheduled_at_ms,
        attempted_at_ms: scheduled_at_ms,
        amount: amount.clone(),
        asset_definition: charge_asset_id.clone(),
        status: SubscriptionInvoiceStatus::Paid,
        tx_hash: None,
    };
    let invoice_key: Name = SUBSCRIPTION_INVOICE_METADATA_KEY.parse().unwrap();
    let expected_invoice_set = InstructionBox::from(SetKeyValue::nft(
        nft_id.clone(),
        invoice_key,
        Json::new(expected_invoice),
    ));
    let expected_unregister = InstructionBox::from(Unregister::trigger(trigger_id.clone()));
    let expected_schedule = Schedule {
        start_ms: scheduled_at_ms + period_ms,
        period_ms: None,
    };
    let expected_action = iroha_data_model::trigger::action::Action::new(
        Executable::Ivm(bytecode),
        Repeats::Exactly(1),
        subscriber.clone(),
        TimeEventFilter(ExecutionTime::Schedule(expected_schedule)),
    )
    .expect("trigger action fixture satisfies validation invariants")
    .with_metadata({
        let mut metadata = trigger_metadata;
        let registered_height_key: Name = "__registered_block_height".parse().unwrap();
        metadata.remove(&registered_height_key);
        let registered_time_key: Name = "__registered_at_ms".parse().unwrap();
        metadata.remove(&registered_time_key);
        metadata
    });
    let expected_trigger = Trigger::new(trigger_id.clone(), expected_action);
    let expected_register = InstructionBox::from(Register::trigger(expected_trigger));
    assert_eq!(
        host.queued,
        vec![
            expected_transfer.clone(),
            expected_set.clone(),
            expected_invoice_set.clone(),
            expected_unregister.clone(),
            expected_register.clone()
        ]
    );
    let expected_gas =
        crate::gas::meter_instruction(&expected_transfer, &crate::gas::DEFAULT_ZK_GAS)
            .expect("completed fixture gas quote")
            .saturating_add(
                crate::gas::meter_instruction(&expected_set, &crate::gas::DEFAULT_ZK_GAS)
                    .expect("completed fixture gas quote"),
            )
            .saturating_add(
                crate::gas::meter_instruction(&expected_invoice_set, &crate::gas::DEFAULT_ZK_GAS)
                    .expect("completed fixture gas quote"),
            )
            .saturating_add(
                crate::gas::meter_instruction(&expected_unregister, &crate::gas::DEFAULT_ZK_GAS)
                    .expect("completed fixture gas quote"),
            )
            .saturating_add(
                crate::gas::meter_instruction(&expected_register, &crate::gas::DEFAULT_ZK_GAS)
                    .expect("completed fixture gas quote"),
            );
    assert_eq!(gas, expected_gas);
    #[cfg(feature = "telemetry")]
    {
        assert_eq!(
            metrics
                .subscription_billing_attempts_total
                .with_label_values(&["fixed"])
                .get(),
            1
        );
        assert_eq!(
            metrics
                .subscription_billing_outcomes_total
                .with_label_values(&["fixed", "paid"])
                .get(),
            1
        );
    }
}
#[test]
#[allow(clippy::too_many_lines)]
fn subscription_usage_recording_and_billing_use_nominal_quantities() {
    let provider = fixture_account_in_domain("acme", "commerce");
    let subscriber = fixture_account_in_domain("alice", "users");
    let plan_id: AssetDefinitionId =
        iroha_data_model::asset::AssetDefinitionId::derive_from_components(
            DomainId::try_new("commerce", "universal").unwrap(),
            "usage_plan".parse().unwrap(),
        );
    let charge_asset_id: AssetDefinitionId =
        iroha_data_model::asset::AssetDefinitionId::derive_from_components(
            DomainId::try_new("pay", "universal").unwrap(),
            "usd".parse().unwrap(),
        );
    let unit_key: Name = "compute_ms".parse().unwrap();
    let trigger_id: TriggerId = "sub-usage".parse().unwrap();
    let plan = SubscriptionPlan {
        provider: provider.clone(),
        billing: SubscriptionBilling {
            cadence: SubscriptionCadence::FixedPeriod(SubscriptionFixedPeriodCadence {
                period_ms: 1_000,
            }),
            bill_for: SubscriptionBillFor::PreviousPeriod,
            retry_backoff_ms: 100,
            max_failures: 3,
            grace_ms: 500,
        },
        pricing: SubscriptionPricing::Usage(SubscriptionUsagePricing {
            unit_price: Quantity::from(2_u32),
            unit_key: unit_key.clone(),
            asset_definition: charge_asset_id.clone(),
        }),
    };
    let mut plan_def = AssetDefinition::new(
        plan_id.clone(),
        "usage_plan".to_owned(),
        NumericSpec::integer(),
        iroha_data_model::asset::AssetBalancePolicy::Global,
        None,
    )
    .build(&provider);
    let plan_key: Name = SUBSCRIPTION_PLAN_METADATA_KEY.parse().unwrap();
    plan_def.metadata.insert(plan_key, Json::new(plan.clone()));
    let charge_def = AssetDefinition::new(
        charge_asset_id.clone(),
        "usd".to_owned(),
        NumericSpec::integer(),
        iroha_data_model::asset::AssetBalancePolicy::Global,
        None,
    )
    .build(&provider);
    let subscriber_asset_id = AssetId::of(charge_asset_id.clone(), subscriber.clone());
    let subscriber_asset = Asset::new(subscriber_asset_id.clone(), Quantity::from(100_u32));
    let mut usage_accumulated = BTreeMap::new();
    usage_accumulated.insert(unit_key.clone(), Quantity::from(10_u32));
    let subscription_state = SubscriptionState {
        plan_id,
        provider: provider.clone(),
        subscriber: subscriber.clone(),
        status: SubscriptionStatus::Active,
        current_period_start_ms: 0,
        current_period_end_ms: 1_000,
        next_charge_ms: 1_000,
        cancel_at_period_end: false,
        cancel_at_ms: None,
        failure_count: 0,
        usage_accumulated,
        billing_trigger_id: trigger_id.clone(),
    };
    let mut nft_meta = Metadata::default();
    let subscription_key: Name = SUBSCRIPTION_METADATA_KEY.parse().unwrap();
    nft_meta.insert(
        subscription_key.clone(),
        Json::new(subscription_state.clone()),
    );
    let nft_id: NftId = "sub-usage$subscriptions.universal".parse().unwrap();
    let nft = Nft::new(nft_id.clone(), nft_meta).build(&subscriber);
    let domains = vec![
        Domain::new(DomainId::try_new("commerce", "universal").unwrap()).build(&provider),
        Domain::new(DomainId::try_new("users", "universal").unwrap()).build(&provider),
        Domain::new(DomainId::try_new("pay", "universal").unwrap()).build(&provider),
        Domain::new(DomainId::try_new("subscriptions", "universal").unwrap()).build(&provider),
    ];
    let accounts = vec![
        build_fixture_account(&provider, &provider),
        build_fixture_account(&subscriber, &provider),
    ];
    let world = World::with_assets(
        domains,
        accounts,
        [plan_def, charge_def],
        [subscriber_asset],
        [nft],
    );
    let bytecode = IvmBytecode::from_compiled(ivm::ProgramMetadata::default().encode());
    let mut trigger_metadata = Metadata::default();
    let trigger_ref_key: Name = SUBSCRIPTION_TRIGGER_REF_METADATA_KEY.parse().unwrap();
    trigger_metadata.insert(
        trigger_ref_key,
        Json::new(SubscriptionTriggerRef {
            subscription_nft_id: nft_id.clone(),
        }),
    );
    let mut action = SpecializedAction::new(
        Executable::Ivm(bytecode),
        Repeats::Exactly(1),
        subscriber.clone(),
        TimeEventFilter(ExecutionTime::Schedule(Schedule {
            start_ms: 1_000,
            period_ms: None,
        })),
    )
    .expect("test scheduled trigger action satisfies its authority invariant");
    action.metadata = trigger_metadata;
    let trigger = SpecializedTrigger::new(trigger_id.clone(), action);
    {
        let mut block = world.triggers.block();
        let mut transaction = block.transaction();
        transaction
            .add_time_trigger(trigger)
            .expect("add usage billing trigger");
        transaction.apply();
        block.commit();
    }
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let state = State::new_for_testing(world, kura, query);
    let view = state.view();
    let mut billing_host = CoreHostImpl::new(subscriber.clone());
    billing_host.set_query_state(&view);
    billing_host.set_trigger_id(trigger_id);
    billing_host.set_block_time_ms(1_000);
    let mut billing_vm = IVM::new(1_000_000);
    let billing_gas = billing_host
        .syscall(ivm_sys::SYSCALL_SUBSCRIPTION_BILL, &mut billing_vm)
        .expect("usage billing");
    let billed_amount = Quantity::from(20_u32);
    let expected_transfer = InstructionBox::from(Transfer::asset_quantity(
        subscriber_asset_id,
        billed_amount.clone(),
        provider.clone(),
    ));
    assert_eq!(
        billing_host
            .queued
            .first()
            .and_then(QueuedEffect::instruction),
        Some(&expected_transfer),
        "ten accumulated units at two units of price must bill exactly twenty"
    );
    assert_eq!(billing_host.queued.len(), 5);
    assert_eq!(
        billing_gas,
        QueuedEffect::instructions(&billing_host.queued).fold(0_u64, |gas, instruction| {
            gas.saturating_add(
                crate::gas::meter_instruction(instruction, &crate::gas::DEFAULT_ZK_GAS)
                    .expect("completed fixture gas quote"),
            )
        })
    );
    let mut expected_billed_state = subscription_state.clone();
    expected_billed_state.current_period_start_ms = 1_000;
    expected_billed_state.current_period_end_ms = 2_000;
    expected_billed_state.next_charge_ms = 2_000;
    expected_billed_state.usage_accumulated.remove(&unit_key);
    let expected_billed_set = InstructionBox::from(SetKeyValue::nft(
        nft_id.clone(),
        subscription_key.clone(),
        Json::new(expected_billed_state),
    ));
    assert_eq!(
        billing_host
            .queued
            .get(1)
            .and_then(QueuedEffect::instruction),
        Some(&expected_billed_set)
    );
    let expected_invoice = SubscriptionInvoice {
        subscription_nft_id: nft_id.clone(),
        period_start_ms: 0,
        period_end_ms: 1_000,
        attempted_at_ms: 1_000,
        amount: billed_amount,
        asset_definition: charge_asset_id,
        status: SubscriptionInvoiceStatus::Paid,
        tx_hash: None,
    };
    let invoice_key: Name = SUBSCRIPTION_INVOICE_METADATA_KEY.parse().unwrap();
    let expected_invoice_set = InstructionBox::from(SetKeyValue::nft(
        nft_id.clone(),
        invoice_key,
        Json::new(expected_invoice),
    ));
    assert_eq!(
        billing_host
            .queued
            .get(2)
            .and_then(QueuedEffect::instruction),
        Some(&expected_invoice_set)
    );
    let delta = SubscriptionUsageDelta {
        subscription_nft_id: nft_id.clone(),
        unit_key: unit_key.clone(),
        delta: Quantity::from(5_u32),
    };
    let args = Json::new(delta);
    let mut host = CoreHostImpl::with_accounts_and_args(
        subscriber.clone(),
        Arc::new(vec![provider, subscriber]),
        args,
    );
    host.set_query_state(&view);
    let mut vm = IVM::new(1_000_000);
    let gas = host
        .syscall(ivm_sys::SYSCALL_SUBSCRIPTION_RECORD_USAGE, &mut vm)
        .expect("usage record");
    let mut expected_state = subscription_state;
    expected_state
        .usage_accumulated
        .insert(unit_key, Quantity::from(15_u32));
    let expected_set = InstructionBox::from(SetKeyValue::nft(
        nft_id,
        subscription_key,
        Json::new(expected_state),
    ));
    assert_eq!(host.queued, vec![expected_set.clone()]);
    assert_eq!(
        gas,
        crate::gas::meter_instruction(&expected_set, &crate::gas::DEFAULT_ZK_GAS)
            .expect("completed fixture gas quote")
    );
}
#[test]
fn subscription_plan_metadata_rejects_negative_usage_unit_price() {
    let provider = fixture_account_in_domain("acme", "commerce");
    let plan_id: AssetDefinitionId =
        iroha_data_model::asset::AssetDefinitionId::derive_from_components(
            DomainId::try_new("commerce", "universal").unwrap(),
            "usage_plan".parse().unwrap(),
        );
    let charge_asset_id: AssetDefinitionId =
        iroha_data_model::asset::AssetDefinitionId::derive_from_components(
            DomainId::try_new("pay", "universal").unwrap(),
            "usd".parse().unwrap(),
        );
    let plan = SubscriptionPlan {
        provider: provider.clone(),
        billing: SubscriptionBilling {
            cadence: SubscriptionCadence::FixedPeriod(SubscriptionFixedPeriodCadence {
                period_ms: 1_000,
            }),
            bill_for: SubscriptionBillFor::PreviousPeriod,
            retry_backoff_ms: 100,
            max_failures: 3,
            grace_ms: 500,
        },
        pricing: SubscriptionPricing::Usage(SubscriptionUsagePricing {
            unit_price: Quantity::from(2_u32),
            unit_key: "compute_ms".parse().unwrap(),
            asset_definition: charge_asset_id,
        }),
    };
    let valid_json = Json::new(plan);
    let forged_text = valid_json
        .get()
        .replacen("\"unit_price\":\"2\"", "\"unit_price\":\"-2\"", 1);
    assert_ne!(
        forged_text.as_str(),
        valid_json.get().as_str(),
        "fixture must replace the encoded nominal unit price"
    );
    let mut plan_definition = AssetDefinition::new(
        plan_id.clone(),
        "usage_plan".to_owned(),
        NumericSpec::integer(),
        iroha_data_model::asset::AssetBalancePolicy::Global,
        None,
    )
    .build(&provider);
    let plan_key: Name = SUBSCRIPTION_PLAN_METADATA_KEY.parse().unwrap();
    plan_definition.metadata.insert(
        plan_key,
        Json::from_str_norito(&forged_text).expect("syntactically valid forged JSON"),
    );
    let world = World::with_assets(
        [Domain::new(DomainId::try_new("commerce", "universal").unwrap()).build(&provider)],
        [build_fixture_account(&provider, &provider)],
        [plan_definition],
        Vec::<Asset>::new(),
        Vec::<Nft>::new(),
    );
    let state = State::new_for_testing(
        world,
        Kura::blank_kura_for_testing(),
        LiveQueryStore::start_test(),
    );
    let view = state.view();
    assert!(matches!(
        CoreHostImpl::<NoQueryState>::subscription_plan(&view, &plan_id),
        Err(ivm::VMError::NoritoInvalid)
    ));
}
#[test]
#[allow(clippy::too_many_lines)]
fn subscription_bill_failed_reschedules_and_records_invoice() {
    let provider = fixture_account_in_domain("acme", "commerce");
    let subscriber = fixture_account_in_domain("alice", "users");
    let plan_id: AssetDefinitionId =
        iroha_data_model::asset::AssetDefinitionId::derive_from_components(
            DomainId::try_new("commerce", "universal").unwrap(),
            "fixed_plan".parse().unwrap(),
        );
    let charge_asset_id: AssetDefinitionId =
        iroha_data_model::asset::AssetDefinitionId::derive_from_components(
            DomainId::try_new("pay", "universal").unwrap(),
            "usd".parse().unwrap(),
        );
    let period_ms = 1_000_u64;
    let scheduled_at_ms = 10_000_u64;
    let trigger_id: TriggerId = "sub-bill-fail".parse().unwrap();
    let amount = Quantity::from(120_u32);
    let retry_backoff_ms = 500_u64;
    let plan = SubscriptionPlan {
        provider: provider.clone(),
        billing: SubscriptionBilling {
            cadence: SubscriptionCadence::FixedPeriod(SubscriptionFixedPeriodCadence { period_ms }),
            bill_for: SubscriptionBillFor::PreviousPeriod,
            retry_backoff_ms,
            max_failures: 3,
            grace_ms: 0,
        },
        pricing: SubscriptionPricing::Fixed(SubscriptionFixedPricing {
            amount: amount.clone(),
            asset_definition: charge_asset_id.clone(),
        }),
    };
    let mut plan_def = AssetDefinition::new(
        plan_id.clone(),
        "fixed_plan".to_owned(),
        NumericSpec::integer(),
        iroha_data_model::asset::AssetBalancePolicy::Global,
        None,
    )
    .build(&provider);
    let plan_key: Name = SUBSCRIPTION_PLAN_METADATA_KEY.parse().unwrap();
    plan_def.metadata.insert(plan_key, Json::new(plan.clone()));
    let charge_def = AssetDefinition::new(
        charge_asset_id.clone(),
        "usd".to_owned(),
        NumericSpec::integer(),
        iroha_data_model::asset::AssetBalancePolicy::Global,
        None,
    )
    .build(&provider);
    let asset_id = AssetId::of(charge_asset_id.clone(), subscriber.clone());
    let asset = Asset::new(asset_id.clone(), Quantity::from(50_u32));
    let subscription_state = SubscriptionState {
        plan_id: plan_id.clone(),
        provider: provider.clone(),
        subscriber: subscriber.clone(),
        status: SubscriptionStatus::Active,
        current_period_start_ms: scheduled_at_ms - period_ms,
        current_period_end_ms: scheduled_at_ms,
        next_charge_ms: scheduled_at_ms,
        cancel_at_period_end: false,
        cancel_at_ms: None,
        failure_count: 0,
        usage_accumulated: BTreeMap::new(),
        billing_trigger_id: trigger_id.clone(),
    };
    let mut nft_meta = Metadata::default();
    let subscription_key: Name = SUBSCRIPTION_METADATA_KEY.parse().unwrap();
    nft_meta.insert(
        subscription_key.clone(),
        Json::new(subscription_state.clone()),
    );
    let nft_id: NftId = "sub-fail$subscriptions.universal".parse().unwrap();
    let nft = Nft::new(nft_id.clone(), nft_meta).build(&subscriber);
    let domains = vec![
        Domain::new(DomainId::try_new("commerce", "universal").unwrap()).build(&provider),
        Domain::new(DomainId::try_new("users", "universal").unwrap()).build(&provider),
        Domain::new(DomainId::try_new("pay", "universal").unwrap()).build(&provider),
        Domain::new(DomainId::try_new("subscriptions", "universal").unwrap()).build(&provider),
    ];
    let accounts = vec![
        build_fixture_account(&provider, &provider),
        build_fixture_account(&subscriber, &provider),
    ];
    let world = World::with_assets(domains, accounts, [plan_def, charge_def], [asset], [nft]);
    let bytecode = IvmBytecode::from_compiled(ivm::ProgramMetadata::default().encode());
    let mut trigger_metadata = Metadata::default();
    let trigger_ref_key: Name = SUBSCRIPTION_TRIGGER_REF_METADATA_KEY.parse().unwrap();
    trigger_metadata.insert(
        trigger_ref_key,
        Json::new(SubscriptionTriggerRef {
            subscription_nft_id: nft_id.clone(),
        }),
    );
    let schedule = Schedule {
        start_ms: scheduled_at_ms,
        period_ms: None,
    };
    let mut action = SpecializedAction::new(
        Executable::Ivm(bytecode.clone()),
        Repeats::Exactly(1),
        subscriber.clone(),
        TimeEventFilter(ExecutionTime::Schedule(schedule)),
    )
    .expect("test scheduled trigger action satisfies its authority invariant");
    action.metadata = trigger_metadata.clone();
    let trigger = SpecializedTrigger::new(trigger_id.clone(), action);
    {
        let mut block = world.triggers.block();
        let mut tx = block.transaction();
        tx.add_time_trigger(trigger).expect("add trigger");
        tx.apply();
        block.commit();
    }
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let state = State::new_for_testing(world, kura, query);
    let view = state.view();
    let mut host = CoreHostImpl::new(subscriber.clone());
    host.set_query_state(&view);
    host.set_trigger_id(trigger_id.clone());
    host.set_block_time_ms(scheduled_at_ms);
    let mut vm = IVM::new(1_000_000);
    let gas = host
        .syscall(ivm_sys::SYSCALL_SUBSCRIPTION_BILL, &mut vm)
        .expect("billing");
    let mut expected_state = subscription_state.clone();
    expected_state.failure_count = 1;
    expected_state.status = SubscriptionStatus::PastDue;
    expected_state.next_charge_ms = scheduled_at_ms + retry_backoff_ms;
    let expected_set = InstructionBox::from(SetKeyValue::nft(
        nft_id.clone(),
        subscription_key,
        Json::new(expected_state),
    ));
    let expected_invoice = SubscriptionInvoice {
        subscription_nft_id: nft_id.clone(),
        period_start_ms: scheduled_at_ms - period_ms,
        period_end_ms: scheduled_at_ms,
        attempted_at_ms: scheduled_at_ms,
        amount: amount.clone(),
        asset_definition: charge_asset_id.clone(),
        status: SubscriptionInvoiceStatus::Failed,
        tx_hash: None,
    };
    let invoice_key: Name = SUBSCRIPTION_INVOICE_METADATA_KEY.parse().unwrap();
    let expected_invoice_set = InstructionBox::from(SetKeyValue::nft(
        nft_id.clone(),
        invoice_key,
        Json::new(expected_invoice),
    ));
    let expected_unregister = InstructionBox::from(Unregister::trigger(trigger_id.clone()));
    let expected_schedule = Schedule {
        start_ms: scheduled_at_ms + retry_backoff_ms,
        period_ms: None,
    };
    let expected_action = iroha_data_model::trigger::action::Action::new(
        Executable::Ivm(bytecode),
        Repeats::Exactly(1),
        subscriber.clone(),
        TimeEventFilter(ExecutionTime::Schedule(expected_schedule)),
    )
    .expect("trigger action fixture satisfies validation invariants")
    .with_metadata(trigger_metadata);
    let expected_trigger = Trigger::new(trigger_id.clone(), expected_action);
    let expected_register = InstructionBox::from(Register::trigger(expected_trigger));
    assert_eq!(
        host.queued,
        vec![
            expected_set.clone(),
            expected_invoice_set.clone(),
            expected_unregister.clone(),
            expected_register.clone()
        ]
    );
    let expected_gas = crate::gas::meter_instruction(&expected_set, &crate::gas::DEFAULT_ZK_GAS)
        .expect("completed fixture gas quote")
        .saturating_add(
            crate::gas::meter_instruction(&expected_invoice_set, &crate::gas::DEFAULT_ZK_GAS)
                .expect("completed fixture gas quote"),
        )
        .saturating_add(
            crate::gas::meter_instruction(&expected_unregister, &crate::gas::DEFAULT_ZK_GAS)
                .expect("completed fixture gas quote"),
        )
        .saturating_add(
            crate::gas::meter_instruction(&expected_register, &crate::gas::DEFAULT_ZK_GAS)
                .expect("completed fixture gas quote"),
        );
    assert_eq!(gas, expected_gas);
}
#[test]
#[allow(clippy::too_many_lines)]
fn subscription_bill_suspends_after_max_failures() {
    let provider = fixture_account_in_domain("acme", "commerce");
    let subscriber = fixture_account_in_domain("alice", "users");
    let plan_id: AssetDefinitionId =
        iroha_data_model::asset::AssetDefinitionId::derive_from_components(
            DomainId::try_new("commerce", "universal").unwrap(),
            "fixed_plan".parse().unwrap(),
        );
    let charge_asset_id: AssetDefinitionId =
        iroha_data_model::asset::AssetDefinitionId::derive_from_components(
            DomainId::try_new("pay", "universal").unwrap(),
            "usd".parse().unwrap(),
        );
    let period_ms = 1_000_u64;
    let scheduled_at_ms = 10_000_u64;
    let trigger_id: TriggerId = "sub-bill-suspend".parse().unwrap();
    let amount = Quantity::from(120_u32);
    let plan = SubscriptionPlan {
        provider: provider.clone(),
        billing: SubscriptionBilling {
            cadence: SubscriptionCadence::FixedPeriod(SubscriptionFixedPeriodCadence { period_ms }),
            bill_for: SubscriptionBillFor::PreviousPeriod,
            retry_backoff_ms: 500,
            max_failures: 2,
            grace_ms: 0,
        },
        pricing: SubscriptionPricing::Fixed(SubscriptionFixedPricing {
            amount: amount.clone(),
            asset_definition: charge_asset_id.clone(),
        }),
    };
    let mut plan_def = AssetDefinition::new(
        plan_id.clone(),
        "fixed_plan".to_owned(),
        NumericSpec::integer(),
        iroha_data_model::asset::AssetBalancePolicy::Global,
        None,
    )
    .build(&provider);
    let plan_key: Name = SUBSCRIPTION_PLAN_METADATA_KEY.parse().unwrap();
    plan_def.metadata.insert(plan_key, Json::new(plan.clone()));
    let charge_def = AssetDefinition::new(
        charge_asset_id.clone(),
        "usd".to_owned(),
        NumericSpec::integer(),
        iroha_data_model::asset::AssetBalancePolicy::Global,
        None,
    )
    .build(&provider);
    let subscription_state = SubscriptionState {
        plan_id: plan_id.clone(),
        provider: provider.clone(),
        subscriber: subscriber.clone(),
        status: SubscriptionStatus::Active,
        current_period_start_ms: scheduled_at_ms - period_ms,
        current_period_end_ms: scheduled_at_ms,
        next_charge_ms: scheduled_at_ms,
        cancel_at_period_end: false,
        cancel_at_ms: None,
        failure_count: 2,
        usage_accumulated: BTreeMap::new(),
        billing_trigger_id: trigger_id.clone(),
    };
    let mut nft_meta = Metadata::default();
    let subscription_key: Name = SUBSCRIPTION_METADATA_KEY.parse().unwrap();
    nft_meta.insert(
        subscription_key.clone(),
        Json::new(subscription_state.clone()),
    );
    let nft_id: NftId = "sub-suspend$subscriptions.universal".parse().unwrap();
    let nft = Nft::new(nft_id.clone(), nft_meta).build(&subscriber);
    let domains = vec![
        Domain::new(DomainId::try_new("commerce", "universal").unwrap()).build(&provider),
        Domain::new(DomainId::try_new("users", "universal").unwrap()).build(&provider),
        Domain::new(DomainId::try_new("pay", "universal").unwrap()).build(&provider),
        Domain::new(DomainId::try_new("subscriptions", "universal").unwrap()).build(&provider),
    ];
    let accounts = vec![
        build_fixture_account(&provider, &provider),
        build_fixture_account(&subscriber, &provider),
    ];
    let world = World::with_assets(
        domains,
        accounts,
        [plan_def, charge_def],
        Vec::<Asset>::new(),
        [nft],
    );
    let bytecode = IvmBytecode::from_compiled(ivm::ProgramMetadata::default().encode());
    let mut trigger_metadata = Metadata::default();
    let trigger_ref_key: Name = SUBSCRIPTION_TRIGGER_REF_METADATA_KEY.parse().unwrap();
    trigger_metadata.insert(
        trigger_ref_key,
        Json::new(SubscriptionTriggerRef {
            subscription_nft_id: nft_id.clone(),
        }),
    );
    let schedule = Schedule {
        start_ms: scheduled_at_ms,
        period_ms: None,
    };
    let mut action = SpecializedAction::new(
        Executable::Ivm(bytecode.clone()),
        Repeats::Exactly(1),
        subscriber.clone(),
        TimeEventFilter(ExecutionTime::Schedule(schedule)),
    )
    .expect("test scheduled trigger action satisfies its authority invariant");
    action.metadata = trigger_metadata.clone();
    let trigger = SpecializedTrigger::new(trigger_id.clone(), action);
    {
        let mut block = world.triggers.block();
        let mut tx = block.transaction();
        tx.add_time_trigger(trigger).expect("add trigger");
        tx.apply();
        block.commit();
    }
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let state = State::new_for_testing(world, kura, query);
    let view = state.view();
    let mut host = CoreHostImpl::new(subscriber.clone());
    host.set_query_state(&view);
    host.set_trigger_id(trigger_id.clone());
    host.set_block_time_ms(scheduled_at_ms);
    let mut vm = IVM::new(1_000_000);
    let gas = host
        .syscall(ivm_sys::SYSCALL_SUBSCRIPTION_BILL, &mut vm)
        .expect("billing");
    let mut expected_state = subscription_state.clone();
    expected_state.failure_count = 3;
    expected_state.status = SubscriptionStatus::Suspended;
    let expected_set = InstructionBox::from(SetKeyValue::nft(
        nft_id.clone(),
        subscription_key,
        Json::new(expected_state),
    ));
    let expected_invoice = SubscriptionInvoice {
        subscription_nft_id: nft_id.clone(),
        period_start_ms: scheduled_at_ms - period_ms,
        period_end_ms: scheduled_at_ms,
        attempted_at_ms: scheduled_at_ms,
        amount: amount.clone(),
        asset_definition: charge_asset_id.clone(),
        status: SubscriptionInvoiceStatus::Failed,
        tx_hash: None,
    };
    let invoice_key: Name = SUBSCRIPTION_INVOICE_METADATA_KEY.parse().unwrap();
    let expected_invoice_set = InstructionBox::from(SetKeyValue::nft(
        nft_id.clone(),
        invoice_key,
        Json::new(expected_invoice),
    ));
    let expected_unregister = InstructionBox::from(Unregister::trigger(trigger_id));
    assert_eq!(
        host.queued,
        vec![
            expected_set.clone(),
            expected_invoice_set.clone(),
            expected_unregister.clone(),
        ]
    );
    let expected_gas = crate::gas::meter_instruction(&expected_set, &crate::gas::DEFAULT_ZK_GAS)
        .expect("completed fixture gas quote")
        .saturating_add(
            crate::gas::meter_instruction(&expected_invoice_set, &crate::gas::DEFAULT_ZK_GAS)
                .expect("completed fixture gas quote"),
        )
        .saturating_add(
            crate::gas::meter_instruction(&expected_unregister, &crate::gas::DEFAULT_ZK_GAS)
                .expect("completed fixture gas quote"),
        );
    assert_eq!(gas, expected_gas);
}
#[test]
fn queue_instructions_accumulates_gas_and_enqueues() {
    let authority = (*ALICE_ID).clone();
    let mut host = CoreHost::new(authority);
    let instr_one = InstructionBox::from(Log::new(iroha_logger::Level::INFO, "one".to_string()));
    let instr_two = InstructionBox::from(Log::new(iroha_logger::Level::WARN, "two".to_string()));
    let expected = crate::gas::meter_instruction(&instr_one, &crate::gas::DEFAULT_ZK_GAS)
        .expect("completed fixture gas quote")
        .saturating_add(
            crate::gas::meter_instruction(&instr_two, &crate::gas::DEFAULT_ZK_GAS)
                .expect("completed fixture gas quote"),
        );
    let gas = host.queue_instructions(vec![instr_one.clone(), instr_two.clone()]);
    assert_eq!(gas, expected);
    assert_eq!(host.queued, vec![instr_one, instr_two]);
}
#[test]
fn bounded_host_stops_queue_growth_before_push() {
    let authority = (*ALICE_ID).clone();
    let mut host = CoreHost::new(authority);
    host.restrict_output_limits(HostOutputLimits::new(2, 1024 * 1024));
    let instruction =
        InstructionBox::from(Log::new(iroha_logger::Level::INFO, "bounded".to_owned()));
    for _ in 0..10_000 {
        host.queue_instruction(instruction.clone())
            .expect("completed fixture gas quote");
    }
    assert_eq!(host.queued.len(), 2);
    assert!(matches!(
        host.output_budget_violation(),
        Some(HostOutputBudgetViolation::ItemCount {
            attempted: 3,
            limit: 2
        })
    ));
}
#[test]
fn tooling_output_limit_cannot_widen_consensus_budget() {
    let authority = (*ALICE_ID).clone();
    let mut host = CoreHost::new(authority);
    let parameters = SmartContractParameters {
        max_output_items: NonZeroU64::new(7).expect("non-zero item limit"),
        max_output_bytes: NonZeroU64::new(4096).expect("non-zero byte limit"),
        ..Default::default()
    };
    host.set_output_limits_from_parameters(parameters);
    host.restrict_output_limits(HostOutputLimits::new(70, 40_960));
    assert_eq!(
        host.instruction_queue_limits,
        HostOutputLimits::new(7, 4096)
    );
}
/// The committed output budget (`SmartContractParameters::max_output_items` and
/// `max_output_bytes`) is inclusive in both dimensions: queued instructions that use
/// exactly the budget are retained, and one item or one byte more is refused before it is
/// retained, with the attempted and permitted amounts.
#[test]
fn committed_output_budget_accepts_the_exact_limit_and_refuses_one_over() {
    let instruction =
        InstructionBox::from(Log::new(iroha_logger::Level::INFO, "bounded".to_owned()));
    let host_with = |items: u64, bytes: u64| {
        let mut host = CoreHost::new((*ALICE_ID).clone());
        let parameters = SmartContractParameters {
            max_output_items: NonZeroU64::new(items).expect("non-zero item limit"),
            max_output_bytes: NonZeroU64::new(bytes).expect("non-zero byte limit"),
            ..Default::default()
        };
        host.set_output_limits_from_parameters(parameters);
        host
    };
    // The retained cost of one queued instruction, measured under the default budget.
    let mut measure = CoreHost::new((*ALICE_ID).clone());
    measure
        .queue_instruction(instruction.clone())
        .expect("completed fixture gas quote");
    assert_eq!(measure.retained_output_items(), 1);
    let cost = measure.retained_output_bytes();
    assert!(
        cost > 64,
        "the encoded instruction and its retained overhead"
    );

    // Bytes: two instructions use the budget exactly; the third is one item's bytes over.
    let mut host = host_with(8, 2 * cost);
    host.queue_instruction(instruction.clone())
        .expect("completed fixture gas quote");
    host.queue_instruction(instruction.clone())
        .expect("completed fixture gas quote");
    assert_eq!(host.queued.len(), 2);
    assert_eq!(host.retained_output_bytes(), 2 * cost);
    assert_eq!(host.output_budget_violation(), None);
    host.queue_instruction(instruction.clone())
        .expect("completed fixture gas quote");
    assert_eq!(host.queued.len(), 2, "nothing over the budget is retained");
    assert_eq!(
        host.output_budget_violation(),
        Some(HostOutputBudgetViolation::EncodedBytes {
            attempted: 2 * cost + 1,
            limit: 2 * cost,
        }),
        "the bounded writer stops at the first byte over the budget"
    );
    // One byte less budget cannot retain the second instruction.
    let mut host = host_with(8, 2 * cost - 1);
    host.queue_instruction(instruction.clone())
        .expect("completed fixture gas quote");
    host.queue_instruction(instruction.clone())
        .expect("completed fixture gas quote");
    assert_eq!(host.queued.len(), 1);
    assert!(matches!(
        host.output_budget_violation(),
        Some(HostOutputBudgetViolation::EncodedBytes { limit, .. }) if limit == 2 * cost - 1
    ));
    // The aggregate counter itself is exact to the byte.
    let mut host = host_with(8, 1_000);
    assert!(host.try_reserve_output(1, 400));
    assert!(host.try_reserve_output(1, 600));
    assert_eq!(host.retained_output_bytes(), 1_000);
    assert!(!host.try_reserve_output(1, 1));
    assert_eq!(
        host.output_budget_violation(),
        Some(HostOutputBudgetViolation::EncodedBytes {
            attempted: 1_001,
            limit: 1_000,
        })
    );

    // Items: the committed count is inclusive and the next item is refused.
    let mut host = host_with(2, 1024 * 1024);
    host.queue_instruction(instruction.clone())
        .expect("completed fixture gas quote");
    host.queue_instruction(instruction.clone())
        .expect("completed fixture gas quote");
    assert_eq!(host.queued.len(), 2);
    assert_eq!(host.retained_output_items(), 2);
    assert_eq!(host.output_budget_violation(), None);
    host.queue_instruction(instruction)
        .expect("completed fixture gas quote");
    assert_eq!(host.queued.len(), 2);
    assert_eq!(
        host.output_budget_violation(),
        Some(HostOutputBudgetViolation::ItemCount {
            attempted: 3,
            limit: 2,
        })
    );
    // The violation is sticky and surfaces to the guest as a deterministic VM error.
    assert!(!host.try_reserve_output(0, 0));
    assert!(matches!((&(host.output_budget_violation()
                .map(HostOutputBudgetViolation::into_vm_error)), &(ivm::VMError::HostOutputBudgetExceeded {
                resource: ivm::HostOutputResource::Items,
                attempted: 3,
                limit: 2,
            })), (Some(ivm::VMError::HostOutputBudgetExceeded { resource: __actual_resource, attempted: __actual_attempted, limit: __actual_limit }), ivm::VMError::HostOutputBudgetExceeded { resource: __expected_resource, attempted: __expected_attempted, limit: __expected_limit }) if __actual_resource == __expected_resource && __actual_attempted == __expected_attempted && __actual_limit == __expected_limit));
}
#[test]
fn bounded_host_stops_unique_durable_state_growth_before_insert() {
    let authority = (*ALICE_ID).clone();
    let mut host = CoreHost::new(authority);
    host.restrict_output_limits(HostOutputLimits::new(3, 1024 * 1024));
    for index in 0..10_000 {
        let key: StatePath = format!("bounded_state_{index}")
            .parse()
            .expect("valid state path");
        if host.try_reserve_durable_state_update(&key, 32) {
            host.durable_state_overlay.insert(key, Some(vec![0; 32]));
        }
    }
    assert_eq!(host.durable_state_overlay.len(), 3);
    assert!(matches!(
        host.output_budget_violation(),
        Some(HostOutputBudgetViolation::ItemCount {
            attempted: 4,
            limit: 3
        })
    ));
}
fn bind_test_contract_runtime(host: &mut CoreHost, nonce: u64) {
    let authority = host.authority.clone();
    let contract_address = ContractAddress::derive(
        &"hash:0000000000000000000000000000000000000000000000000000000000000001#C50E"
            .parse()
            .expect("canonical test network id"),
        &authority,
        nonce,
        DataSpaceId::UNIVERSAL,
    )
    .expect("derive test contract address");
    let authorization = ContractEntrypointAuthorizationSnapshot::new(
        authority,
        "dispatch_ballot".to_owned(),
        iroha_data_model::smart_contract::manifest::EntrypointAuthorizationV1::Anyone,
        &crate::smartcontracts::code::BoundContractIdentity {
            lifecycle_revision: 1,
            contract_address: contract_address.clone(),
            contract_alias: None,
            contract_alias_binding: None,
            code_hash: Hash::new(b"host test contract"),
        },
    );
    host.bind_contract_runtime_context(contract_address.subject_id(), authorization);
}
#[test]
fn generic_execution_rejects_execute_instruction_before_operation_specific_admission() {
    let mut host = CoreHost::new((*ALICE_ID).clone());
    host.set_generic_execution();
    let mut vm = ivm::IVM::new(1_000_000);
    let instruction = InstructionBox::from(Log::new(
        iroha_logger::Level::INFO,
        "generic opaque instruction".to_owned(),
    ));
    let payload = norito::to_bytes(&instruction).expect("encode instruction");
    let ptr = store_tlv(&mut vm, PointerType::NoritoBytes, &payload);
    vm.set_register(10, ptr);
    vm.set_register(11, ivm_sys::SMARTCONTRACT_INSTRUCTION_TAG_SUBMIT_BALLOT);
    assert!(
        matches!((&(host.syscall(ivm_sys::SYSCALL_SMARTCONTRACT_EXECUTE_INSTRUCTION, &mut vm)), &(ivm::VMError::GenericSyscallNotAllowed {
                syscall: ivm_sys::SYSCALL_SMARTCONTRACT_EXECUTE_INSTRUCTION,
            })), (Err(ivm::VMError::GenericSyscallNotAllowed { syscall: __actual_syscall }), ivm::VMError::GenericSyscallNotAllowed { syscall: __expected_syscall }) if __actual_syscall == __expected_syscall)
    );
    assert!(host.queued.is_empty());
}
#[test]
fn execute_instruction_syscall_rejects_retired_blob_payload_forms() {
    let authority = (*ALICE_ID).clone();
    let instruction = InstructionBox::from(Log::new(
        iroha_logger::Level::INFO,
        "blob payload".to_owned(),
    ));
    let payload = norito::to_bytes(&instruction).expect("encode instruction");
    let retired_payloads = [payload.clone(), hex::encode(payload).into_bytes()];
    for retired_payload in retired_payloads {
        let mut host = CoreHost::new(authority.clone());
        bind_test_contract_runtime(&mut host, 304);
        let mut vm = ivm::IVM::new(1_000_000);
        let ptr = store_tlv(&mut vm, PointerType::Blob, &retired_payload);
        vm.set_register(10, ptr);
        vm.set_register(11, ivm_sys::SMARTCONTRACT_INSTRUCTION_TAG_SUBMIT_BALLOT);
        assert!(matches!(
            host.syscall(ivm_sys::SYSCALL_SMARTCONTRACT_EXECUTE_INSTRUCTION, &mut vm),
            Err(ivm::VMError::NoritoInvalid)
        ));
        assert_eq!(vm.register(10), ptr);
        assert!(host.queued.is_empty());
    }
}
#[test]
fn execute_instruction_syscall_rejects_zero_unknown_and_mismatched_tags() {
    let authority = (*ALICE_ID).clone();
    let instruction = InstructionBox::from(Log::new(
        iroha_logger::Level::INFO,
        "mismatched opaque instruction".to_owned(),
    ));
    let payload = norito::to_bytes(&instruction).expect("encode instruction");
    // Tag 2 is not part of ABI v1 (the retired contract-originated SCCP send).
    for operation_tag in [
        0,
        ivm_sys::SMARTCONTRACT_INSTRUCTION_TAG_SUBMIT_BALLOT,
        2,
        u64::MAX,
    ] {
        let mut host = local_contract_host(authority.clone());
        let mut vm = ivm::IVM::new(1_000_000);
        let ptr = store_tlv(&mut vm, PointerType::NoritoBytes, &payload);
        vm.set_register(10, ptr);
        vm.set_register(11, operation_tag);
        assert!(matches!(
            host.syscall(ivm_sys::SYSCALL_SMARTCONTRACT_EXECUTE_INSTRUCTION, &mut vm),
            Err(ivm::VMError::PermissionDenied)
        ));
        assert!(
            host.queued.is_empty(),
            "rejected tag must not enqueue an ISI"
        );
    }
}
#[test]
fn execute_instruction_syscall_rejects_record_sccp_message_under_every_tag() {
    // specs/sccp.md §4.4: contracts record no SCCP messages. In a contract frame the
    // canonical encoded instruction decodes (a decode failure would be `NoritoInvalid`)
    // and then fails under the only v1 tag, under tag 2 and under every other tag. A
    // generic frame may not call `0xA0` at all. Nothing is queued in either frame.
    let authority = (*ALICE_ID).clone();
    let instruction = InstructionBox::from(
        crate::smartcontracts::isi::sccp::test_support::SampleInstructions::record(),
    );
    let payload = norito::to_bytes(&instruction).expect("encode RecordSccpMessage");
    for contract_frame in [false, true] {
        for operation_tag in [
            0,
            ivm_sys::SMARTCONTRACT_INSTRUCTION_TAG_SUBMIT_BALLOT,
            2,
            3,
            u64::MAX,
        ] {
            let mut host = if contract_frame {
                local_contract_host(authority.clone())
            } else {
                CoreHost::new(authority.clone())
            };
            let mut vm = ivm::IVM::new(1_000_000);
            let ptr = store_tlv(&mut vm, PointerType::NoritoBytes, &payload);
            vm.set_register(10, ptr);
            vm.set_register(11, operation_tag);
            let expected = if contract_frame {
                ivm::VMError::PermissionDenied
            } else {
                ivm::VMError::GenericSyscallNotAllowed {
                    syscall: ivm_sys::SYSCALL_SMARTCONTRACT_EXECUTE_INSTRUCTION,
                }
            };
            let actual = host.syscall(ivm_sys::SYSCALL_SMARTCONTRACT_EXECUTE_INSTRUCTION, &mut vm);
            let equivalent = match (&actual, &expected) {
                (Err(ivm::VMError::PermissionDenied), ivm::VMError::PermissionDenied) => true,
                (
                    Err(ivm::VMError::GenericSyscallNotAllowed { syscall: actual }),
                    ivm::VMError::GenericSyscallNotAllowed { syscall: expected },
                ) => actual == expected,
                _ => false,
            };
            assert!(
                equivalent,
                "tag {operation_tag} (contract frame: {contract_frame}) must not execute RecordSccpMessage: {actual:?} != {expected:?}"
            );
            assert!(
                host.queued.is_empty(),
                "a refused SCCP record must not enqueue an ISI"
            );
        }
    }
}
#[test]
fn execute_instruction_syscall_rejects_retired_generic_instruction_types() {
    let authority = (*ALICE_ID).clone();
    let instruction = InstructionBox::from(Log::new(
        iroha_logger::Level::INFO,
        "opaque generic instruction".to_owned(),
    ));
    let payload = norito::to_bytes(&instruction).expect("encode generic instruction");
    let mut host = CoreHost::new(authority);
    bind_test_contract_runtime(&mut host, 305);
    let mut vm = ivm::IVM::new(1_000_000);
    let ptr = store_tlv(&mut vm, PointerType::NoritoBytes, &payload);
    vm.set_register(10, ptr);
    vm.set_register(11, ivm_sys::SMARTCONTRACT_INSTRUCTION_TAG_SUBMIT_BALLOT);
    assert!(matches!(
        host.syscall(ivm_sys::SYSCALL_SMARTCONTRACT_EXECUTE_INSTRUCTION, &mut vm),
        Err(ivm::VMError::PermissionDenied)
    ));
    assert!(host.queued.is_empty());
}
#[test]
fn execute_instruction_ballot_consumes_each_pending_envelope_once() {
    let first = [0x41; 32];
    let second = [0x42; 32];
    // An explicit envelope is decoded before latch selection and must name its proof bytes.
    let proof_bytes = vec![0xa5];
    let explicit: [u8; 32] = Hash::new(&proof_bytes).into();
    for (provided, expected_hashes) in [
        (None, [Some(first), Some(second), None]),
        (Some(explicit), [Some(explicit); 3]),
    ] {
        let mut host = CoreHost::new((*ALICE_ID).clone());
        bind_test_contract_runtime(&mut host, 306);
        host.__test_seed_ballot_latch(first);
        host.__test_seed_ballot_latch(second);
        let backend: iroha_schema::Ident = "pipa-r/pasta".into();
        let mut proof = ProofAttachment::new_ref(
            backend.clone(),
            ProofBox::new(backend.clone(), proof_bytes.clone()),
            VerifyingKeyId::new(backend.as_str(), "ordered-ballot-envelope"),
        );
        proof.envelope_hash = provided;
        let ballot = DMZk::SubmitBallot {
            election_id: "election".to_owned(),
            ciphertext: vec![0x11; 32],
            ballot_proof: proof,
            nullifier: [0x22; 32],
        };
        let payload = norito::to_bytes(&InstructionBox::from(ballot.clone()))
            .expect("encode ballot instruction");
        assert_eq!(
            decode_canonical_norito::<InstructionBox>(&payload)
                .expect("canonical ballot attachment"),
            InstructionBox::from(ballot.clone()),
        );
        let mut vm = IVM::new(50_000_000);
        let instruction_ptr = store_tlv(&mut vm, PointerType::NoritoBytes, &payload);
        for (index, expected_hash) in expected_hashes.into_iter().enumerate() {
            vm.set_register(10, instruction_ptr);
            vm.set_register(11, ivm_sys::SMARTCONTRACT_INSTRUCTION_TAG_SUBMIT_BALLOT);
            host.syscall(ivm_sys::SYSCALL_SMARTCONTRACT_EXECUTE_INSTRUCTION, &mut vm)
                .expect("affordable ballot instruction");
            let mut expected = ballot.clone();
            expected.ballot_proof.envelope_hash = expected_hash;
            let expected = InstructionBox::from(expected);
            assert_eq!(host.queued.len(), index + 1);
            assert_eq!(host.queued[index].instruction(), Some(&expected));
            let pending = if provided.is_some() {
                vec![first, second]
            } else if index == 0 {
                vec![second]
            } else {
                Vec::new()
            };
            assert_eq!(
                host.zk_last_env_hash_ballot
                    .iter()
                    .copied()
                    .collect::<Vec<_>>(),
                pending,
                "only an inherited envelope consumes one original pending hash"
            );
            assert_eq!(host.zk_verified_ballot.len(), 2);
        }
    }
}
#[test]
fn unaffordable_proof_instruction_preserves_queue_and_verification_latch() {
    // The schedule is the host's own value (the block's frozen ZK settings),
    // so this test cannot disturb any other host in the process.
    let extreme_gas = iroha_config::parameters::actual::ConfidentialGas {
        proof_base: u64::MAX,
        per_public_input: u64::MAX,
        per_proof_byte: u64::MAX,
        per_nullifier: u64::MAX,
        per_commitment: u64::MAX,
    };
    let backend: iroha_schema::Ident = "pipa-r/pasta".into();
    let proof = ProofAttachment::new_ref(
        backend.clone(),
        ProofBox::new(backend.clone(), vec![0xa5]),
        VerifyingKeyId::new(backend.as_str(), "extreme-gas-ballot"),
    );
    let instruction = InstructionBox::from(DMZk::SubmitBallot {
        election_id: "election".to_owned(),
        ciphertext: vec![0x11; 32],
        ballot_proof: proof,
        nullifier: [0x22; 32],
    });
    let payload = norito::to_bytes(&instruction).expect("encode proof instruction");
    let code = [ivm::encoding::wide::encode_sys(
        ivm::instruction::wide::system::SCALL,
        u8::try_from(ivm_sys::SYSCALL_SMARTCONTRACT_EXECUTE_INSTRUCTION).expect("syscall fits"),
    )
    .to_le_bytes()]
    .concat();
    let mut vm = IVM::new(10_000);
    vm.load_program(&build_authenticated_test_contract_program(&code, 0, true))
        .expect("load proof instruction program");
    let instruction_ptr = store_tlv(&mut vm, PointerType::NoritoBytes, &payload);
    vm.set_register(14, instruction_ptr);
    vm.set_register(15, ivm_sys::SMARTCONTRACT_INSTRUCTION_TAG_SUBMIT_BALLOT);
    let pending_hash = [0x42; 32];
    let mut host = local_contract_host((*ALICE_ID).clone());
    host.confidential_gas = extreme_gas;
    Arc::make_mut(&mut host.zk_last_env_hash_ballot).push_back(pending_hash);
    let error = vm
        .run_with_host(&mut host)
        .expect_err("extreme proof gas exceeds the reserved budget");
    assert!(matches!(error, ivm::VMError::OutOfGas));
    assert_eq!(
        vm.remaining_gas(),
        0,
        "a state-dependent proof failure consumes its fail-closed reserve"
    );
    assert!(host.queued.is_empty(), "no instruction may be published");
    assert_eq!(
        host.zk_last_env_hash_ballot.front(),
        Some(&pending_hash),
        "failed affordability must not consume the proof latch"
    );
    assert_eq!(vm.register(10), instruction_ptr);
}
#[test]
fn by_call_bytes_param_can_coexist_with_canonical_asset_operation() {
    let authority = (*ALICE_ID).clone();
    let source = r"
seiyaku BurnWithMemo { permission AssetTransferRole;
  kotoage fn burn_with_memo(
    AccountId sender,
    AssetDefinitionId settlement_asset,
    quantity amount,
    bytes memo
  ) authorize(AssetTransferRole) {
    ledger::asset::burn(account: sender, asset_definition: settlement_asset, amount: amount);
  }
}
";
    let compiler = kotodama_lang::compiler::Compiler::new_with_options(
        kotodama_lang::compiler::CompilerOptions {
            force_zk: true,
            mode: kotodama_lang::compiler::CompilerMode::Production,
            ..kotodama_lang::compiler::CompilerOptions::default()
        },
    );
    let (code, _manifest) = compiler
        .compile_source_with_manifest(source)
        .expect("compile by-call contract");
    let parsed = ivm::ProgramMetadata::parse(&code).expect("parse contract metadata");
    let descriptor = parsed
        .contract_interface
        .as_ref()
        .expect("contract interface")
        .entrypoints
        .iter()
        .find(|candidate| candidate.name == "burn_with_memo")
        .expect("burn_with_memo entrypoint");
    let argument_schema = descriptor
        .argument_schema
        .as_ref()
        .expect("burn_with_memo argument schema");
    let entry_pc = parsed.prefix_len() as u64 + descriptor.entry_pc;
    let settlement_asset_def = AssetDefinitionId::derive_from_components(
        DomainId::try_new("wonderland", "universal").unwrap(),
        "rose".parse().unwrap(),
    );
    let authority_literal = authority
        .canonical_i105()
        .expect("canonical authority literal");
    let settlement_asset_literal = settlement_asset_def.canonical_address();
    let memo_hex = format!("0x{}", hex::encode([1_u8, 2, 3, 4]));
    let args = Json::from_str_norito(&format!(
            r#"{{"amount":"1","memo":"{memo_hex}","sender":"{authority_literal}","settlement_asset":"{settlement_asset_literal}"}}"#,
        ))
        .expect("memo payload JSON");
    let canonical = ivm_abi::arguments::encode_argument_record_from_json(argument_schema, &args)
        .expect("encode canonical burn_with_memo arguments");
    let prepared = ivm::prepare_argument_record_with_gas_limit(
        argument_schema,
        Arc::from(canonical),
        50_000_000,
    )
    .expect("prepare gas-aware burn_with_memo arguments");
    let mut host: CoreHost = CoreHostImpl::with_accounts_and_argument_record(
        authority.clone(),
        Arc::new(vec![authority.clone()]),
        Some(prepared.clone()),
    );
    let mut vm = ivm::IVM::new(50_000_000);
    vm.load_program(&code).expect("load contract");
    vm.set_register(1, vm.memory.code_len());
    vm.set_program_counter(entry_pc).expect("seek entrypoint");
    prepared
        .precharge_vm(&mut vm)
        .expect("precharge canonical burn_with_memo arguments");
    vm.run_with_host(&mut host)
        .expect("bytes-bearing burn entrypoint should run");
    let burn = InstructionBox::from(BurnBox::from(Burn::asset_quantity(
        1_u32,
        AssetId::of(settlement_asset_def, authority),
    )));
    assert_eq!(host.queued, vec![burn]);
}
#[test]
fn kotodama_source_rejects_opaque_instruction_submission() {
    let source = r"
seiyaku OpaqueInstructionSubmission { permission AssetTransferRole;
  kotoage fn submit(bytes record_instruction) authorize(AssetTransferRole) {
    execute_instruction(record_instruction);
  }
}
";
    // This is intentionally the non-deployable local harness profile: the
    // retired opaque-instruction escape must remain outside the public language surface.
    let error = kotodama_lang::compiler::Compiler::new_with_options(
        kotodama_lang::compiler::CompilerOptions {
            mode: kotodama_lang::compiler::CompilerMode::Test,
            ..kotodama_lang::compiler::CompilerOptions::default()
        },
    )
    .compile_source_with_manifest(source)
    .expect_err("opaque instruction submission must remain source-inaccessible");
    let rendered = error.clone();
    assert!(
        rendered.contains("error[K2002]")
            && rendered.contains("unknown function or builtin `execute_instruction`"),
        "unexpected compile error: {error}"
    );
}
#[test]
fn execute_instruction_syscall_accepts_code_literal_ballot_instruction() {
    let authority = (*ALICE_ID).clone();
    let mut host = CoreHost::new(authority);
    bind_test_contract_runtime(&mut host, 303);
    let backend: iroha_schema::Ident = "pipa-r/pasta".into();
    let instruction = InstructionBox::from(DMZk::SubmitBallot {
        election_id: "election".to_owned(),
        ciphertext: vec![0x11; 32],
        ballot_proof: ProofAttachment::new_ref(
            backend.clone(),
            ProofBox::new(backend.clone(), vec![0xa5]),
            VerifyingKeyId::new(backend.as_str(), "code-literal-ballot"),
        ),
        nullifier: [0x22; 32],
    });
    let payload = norito::to_bytes(&instruction).expect("encode instruction");
    let tlv = make_tlv(PointerType::NoritoBytes as u16, &payload);
    let contract_interface = ivm::EmbeddedContractInterfaceV1 {
            events: Vec::new(),
            enum_types: Vec::new(),
            permissions: vec![iroha_data_model::smart_contract::manifest::ContractPermissionDescriptorV1 {name:"CanSubmitBallot".parse().unwrap(),scope:iroha_data_model::smart_contract::manifest::ContractPermissionScopeV1::Instance}],
            callables: vec![crate::ivm_test_support::unit_callable(0)],
            seiyaku_name: "CodeLiteralBallotHarness".to_owned(),
            compiler_fingerprint: "iroha-core-host-tests".to_owned(),
            abi_hash: ivm_sys::compute_abi_hash(ivm::SyscallPolicy::AbiV1),
            features_bitmap: ivm::CONTRACT_FEATURE_BIT_ZK,
            access_set_hints: None,
            kotoba: Vec::new(),
            entrypoints: vec![ivm::EmbeddedEntrypointDescriptor {
                name: "dispatch_ballot".to_owned(),
                kind: iroha_data_model::smart_contract::manifest::EntryPointKind::Kotoage,
                params: Vec::new(),
                argument_schema: None,
                return_type: Some("()".to_owned()),
                return_schema: Some(iroha_data_model::smart_contract::entrypoint::EntrypointValueTypeV1 {
                    nodes: vec![iroha_data_model::smart_contract::entrypoint::EntrypointValueTypeNodeV1::Unit],
                }),
                authorization: iroha_data_model::smart_contract::manifest::EntrypointAuthorizationV1::Permission("CanSubmitBallot".parse().unwrap()),
                read_keys: Vec::new(),
                write_keys: Vec::new(),
                access_hints_complete: None,
                access_hints_skipped: Vec::new(),
                triggers: Vec::new(),
                entry_pc: 0,
            }],
            states: Vec::new(),
            error_messages: Vec::new(),
            error_types: Vec::new(),
        };
    let contract_section = contract_interface.encode_section();
    let literal_descriptor_len = core::mem::size_of::<u64>();
    let literal_data_offset = 16 + literal_descriptor_len;
    let post_pad = (4 - ((contract_section.len() + literal_data_offset + tlv.len()) % 4)) % 4;
    let mut program = ivm::ProgramMetadata {
        max_cycles: 1_000_000,
        mode: ivm::ivm_mode::ZK,
        vector_length: 4,
        ..Default::default()
    }
    .encode();
    program.extend_from_slice(&contract_section);
    program.extend_from_slice(b"LTLB");
    program.extend_from_slice(&1u32.to_le_bytes());
    program.extend_from_slice(
        &u32::try_from(post_pad)
            .expect("fixture value fits u32")
            .to_le_bytes(),
    );
    program.extend_from_slice(
        &u32::try_from(tlv.len())
            .expect("fixture value fits u32")
            .to_le_bytes(),
    );
    let literal_descriptor = ivm::encode_literal_descriptor(
        ivm::LiteralKindV1::PointerTlv,
        u64::try_from(literal_data_offset).expect("literal offset fits u64"),
    )
    .expect("encode pointer literal descriptor");
    program.extend_from_slice(&literal_descriptor.to_le_bytes());
    program.extend_from_slice(&tlv);
    program.extend(std::iter::repeat_n(0u8, post_pad));
    program.extend_from_slice(
        &ivm::encoding::wide::encode_literal(ivm::instruction::wide::memory::LDLIT, 10, 0)
            .to_le_bytes(),
    );
    program.extend_from_slice(
        &ivm::encoding::wide::encode_ri(
            ivm::instruction::wide::arithmetic::ADDI,
            11,
            0,
            i8::try_from(ivm_sys::SMARTCONTRACT_INSTRUCTION_TAG_SUBMIT_BALLOT)
                .expect("operation tag fits in ADDI immediate"),
        )
        .to_le_bytes(),
    );
    program.extend_from_slice(
        &ivm::encoding::wide::encode_sys(
            ivm::instruction::wide::system::SCALL,
            u8::try_from(ivm_sys::SYSCALL_SMARTCONTRACT_EXECUTE_INSTRUCTION)
                .expect("fixture value fits u8"),
        )
        .to_le_bytes(),
    );
    program.extend_from_slice(&crate::ivm_test_support::unit_return());
    let mut vm = ivm::IVM::new(50_000_000);
    vm.load_program(&program).expect("load program");
    vm.select_entrypoint("dispatch_ballot")
        .expect("select the authenticated ballot entrypoint");
    vm.run_with_host(&mut host)
        .expect("code literal ballot instruction should be queued");
    assert_eq!(vm.call_result_word_count().unwrap(), 1);
    assert_eq!(vm.public_call_result_word(0).unwrap(), 0);
    assert_eq!(host.queued.len(), 1);
    assert_eq!(host.queued[0].instruction(), Some(&instruction));
    assert!(host.queued[0].contract_runtime_context.is_some());
    assert!(host.queued[0].entrypoint_authorization.is_some());
}
#[test]
fn execute_instruction_syscall_rejects_unindexed_literal_data_without_host_effects() {
    let authority = (*ALICE_ID).clone();
    let host = CoreHost::new(authority);
    let instruction = InstructionBox::from(Log::new(
        iroha_logger::Level::INFO,
        "unindexed literal".to_owned(),
    ));
    let payload = norito::to_bytes(&instruction).expect("encode instruction");
    let tlv = make_tlv(PointerType::NoritoBytes as u16, &payload);
    let post_pad = (4 - ((16 + tlv.len()) % 4)) % 4;
    let mut program = ivm::ProgramMetadata {
        max_cycles: 1_000_000,
        mode: ivm::ivm_mode::ZK,
        vector_length: 4,
        ..Default::default()
    }
    .encode();
    program.extend_from_slice(b"LTLB");
    program.extend_from_slice(&0u32.to_le_bytes());
    program.extend_from_slice(
        &u32::try_from(post_pad)
            .expect("fixture value fits u32")
            .to_le_bytes(),
    );
    program.extend_from_slice(
        &u32::try_from(tlv.len())
            .expect("fixture value fits u32")
            .to_le_bytes(),
    );
    program.extend_from_slice(&tlv);
    program.extend(std::iter::repeat_n(0u8, post_pad));
    program.extend_from_slice(&ivm::encoding::wide::encode_halt().to_le_bytes());
    let mut vm = ivm::IVM::new(50_000_000);
    assert!(matches!(
        vm.load_program(&program),
        Err(ivm::VMError::InvalidMetadata)
    ));
    assert!(host.queued.is_empty());
}
#[test]
fn create_nfts_for_all_noop_syscall_charges_declared_gas() {
    let authority = (*ALICE_ID).clone();
    let mut host = CoreHost::with_accounts(authority, Arc::new(Vec::new()));
    let mut vm = ivm::IVM::new(1_000);
    let quote = host
        .prepare_syscall(ivm_sys::SYSCALL_CREATE_NFTS_FOR_ALL_USERS, &vm)
        .expect("quote create nfts for all");
    let exact = host.create_nfts_for_all_gas_quote();
    let gas = host
        .syscall(ivm_sys::SYSCALL_CREATE_NFTS_FOR_ALL_USERS, &mut vm)
        .expect("create nfts for all");
    assert_eq!(gas, CoreHost::create_nfts_for_all_gas());
    assert_eq!(exact, gas);
    assert_eq!(quote, vm.remaining_gas());
    assert!(gas <= quote);
    assert!(host.queued.is_empty());
}
#[test]
fn create_nfts_for_all_syscall_charges_at_least_declared_gas() {
    let authority = (*ALICE_ID).clone();
    let mut host = CoreHost::with_accounts(authority.clone(), Arc::new(vec![authority]));
    let mut vm = ivm::IVM::new(1_000);
    let quote = host
        .prepare_syscall(ivm_sys::SYSCALL_CREATE_NFTS_FOR_ALL_USERS, &vm)
        .expect("quote create nfts for all");
    let exact = host.create_nfts_for_all_gas_quote();
    let gas = host
        .syscall(ivm_sys::SYSCALL_CREATE_NFTS_FOR_ALL_USERS, &mut vm)
        .expect("create nfts for all");
    assert!(gas >= CoreHost::create_nfts_for_all_gas());
    assert_eq!(exact, gas);
    assert_eq!(quote, vm.remaining_gas());
    assert!(gas <= quote);
    assert_eq!(host.queued.len(), 2);
}
#[test]
fn create_nfts_for_all_supports_multisig_accounts_without_unwinding() {
    let authority = (*ALICE_ID).clone();
    let member =
        MultisigMember::new(ALICE_KEYPAIR.public_key().clone(), 1).expect("valid multisig member");
    let multisig =
        AccountId::new_multisig(MultisigPolicy::new(1, vec![member]).expect("valid policy"));
    let mut host = CoreHost::with_accounts(authority, Arc::new(vec![multisig]));
    let mut vm = ivm::IVM::new(1_000);
    host.syscall(ivm_sys::SYSCALL_CREATE_NFTS_FOR_ALL_USERS, &mut vm)
        .expect("multisig account identity must produce a deterministic NFT target");
    assert_eq!(host.queued.len(), 2);
}
#[test]
fn create_nfts_for_all_preflights_exact_gas_before_queue_mutation() {
    let authority = (*ALICE_ID).clone();
    let mut host = CoreHost::with_accounts(authority.clone(), Arc::new(vec![authority]));
    let mut code = Vec::new();
    code.extend_from_slice(
        &ivm::encoding::wide::encode_sys(
            ivm::instruction::wide::system::SCALL,
            u8::try_from(ivm_sys::SYSCALL_CREATE_NFTS_FOR_ALL_USERS)
                .expect("syscall id fits in u8"),
        )
        .to_le_bytes(),
    );
    code.extend_from_slice(&ivm::encoding::wide::encode_halt().to_le_bytes());
    let mut vm = IVM::new(u64::MAX);
    vm.load_program(&build_program(&code, 0))
        .expect("load create-NFTs program");
    vm.set_gas_limit(20);
    let error = vm
        .run_with_host(&mut host)
        .expect_err("NFT fan-out exceeds the syscall reserve");
    assert!(matches!(error, ivm::VMError::OutOfGas));
    assert_eq!(vm.remaining_gas(), 0);
    assert!(host.queued.is_empty(), "must not queue partial fan-out");
    assert_eq!(host.nft_seq, 0, "must not advance the NFT sequence");
}
#[test]
fn create_nfts_for_all_quote_caps_cardinality_and_handles_sequence_exhaustion() {
    let authority = fixture_account("alice");
    let accounts = (0..300)
        .map(|index| fixture_account_in_domain(&format!("account-{index}"), "fanout"))
        .collect();
    let mut host = CoreHost::with_accounts(authority.clone(), Arc::new(accounts));
    let mut vm = IVM::new(u64::MAX);
    let quote = host
        .prepare_syscall(ivm_sys::SYSCALL_CREATE_NFTS_FOR_ALL_USERS, &vm)
        .expect("quote capped NFT fan-out");
    let exact = host.create_nfts_for_all_gas_quote();
    let actual = host
        .syscall(ivm_sys::SYSCALL_CREATE_NFTS_FOR_ALL_USERS, &mut vm)
        .expect("execute capped NFT fan-out");
    assert_eq!(exact, actual);
    assert_eq!(quote, vm.remaining_gas());
    assert!(actual <= quote);
    assert_eq!(host.queued.len(), 512, "at most 256 NFT pairs are queued");
    assert_eq!(host.next_nft_sequence(), 256);
    let mut exhausted = CoreHost::with_accounts(
        authority,
        Arc::new(vec![
            fixture_account_in_domain("last-a", "fanout"),
            fixture_account_in_domain("last-b", "fanout"),
        ]),
    );
    exhausted.nft_seq = u64::MAX - 1;
    let quote = exhausted
        .prepare_syscall(ivm_sys::SYSCALL_CREATE_NFTS_FOR_ALL_USERS, &vm)
        .expect("quote final sequence value");
    let exact = exhausted.create_nfts_for_all_gas_quote();
    let actual = exhausted
        .syscall(ivm_sys::SYSCALL_CREATE_NFTS_FOR_ALL_USERS, &mut vm)
        .expect("execute final sequence value");
    assert_eq!(exact, actual);
    assert!(actual <= quote);
    assert_eq!(exhausted.queued.len(), 2);
    assert_eq!(exhausted.next_nft_sequence(), u64::MAX);
    exhausted.queued.clear();
    let quote = exhausted
        .prepare_syscall(ivm_sys::SYSCALL_CREATE_NFTS_FOR_ALL_USERS, &vm)
        .expect("quote exhausted sequence");
    let exact = exhausted.create_nfts_for_all_gas_quote();
    let actual = exhausted
        .syscall(ivm_sys::SYSCALL_CREATE_NFTS_FOR_ALL_USERS, &mut vm)
        .expect("exhausted sequence is a safe no-op");
    assert_eq!(exact, CoreHost::create_nfts_for_all_gas());
    assert_eq!(exact, actual);
    assert!(actual <= quote);
    assert!(exhausted.queued.is_empty());
}
#[test]
fn smartcontract_depth_noop_syscall_charges_declared_gas() {
    let authority = (*ALICE_ID).clone();
    let mut host = CoreHost::new(authority);
    let mut vm = ivm::IVM::new(1_000);
    vm.set_register(10, 0);
    let quote = host
        .prepare_syscall(ivm_sys::SYSCALL_SET_SMARTCONTRACT_EXECUTION_DEPTH, &vm)
        .expect("quote depth syscall");
    let gas = host
        .syscall(ivm_sys::SYSCALL_SET_SMARTCONTRACT_EXECUTION_DEPTH, &mut vm)
        .expect("depth syscall");
    assert_eq!(gas, CoreHost::smartcontract_depth_gas());
    assert!(gas <= quote);
    assert!(host.queued.is_empty());
}
#[test]
fn smartcontract_depth_update_syscall_charges_at_least_declared_gas() {
    let authority = (*ALICE_ID).clone();
    let mut host = CoreHost::new(authority);
    let mut vm = ivm::IVM::new(1_000);
    vm.set_register(10, 3);
    let quote = host
        .prepare_syscall(ivm_sys::SYSCALL_SET_SMARTCONTRACT_EXECUTION_DEPTH, &vm)
        .expect("quote depth syscall");
    let gas = host
        .syscall(ivm_sys::SYSCALL_SET_SMARTCONTRACT_EXECUTION_DEPTH, &mut vm)
        .expect("depth syscall");
    assert!(gas >= CoreHost::smartcontract_depth_gas());
    assert!(gas <= quote);
    assert_eq!(host.queued.len(), 1);
}
#[test]
fn queued_instruction_budget_exhaustion_is_an_immediate_vm_error() {
    let authority = (*ALICE_ID).clone();
    let mut host = CoreHost::new(authority);
    host.restrict_output_limits(HostOutputLimits::new(0, u64::MAX));
    let mut vm = ivm::IVM::new(1_000);
    vm.set_register(10, 3);
    assert!(
        matches!((&(host.syscall(ivm_sys::SYSCALL_SET_SMARTCONTRACT_EXECUTION_DEPTH, &mut vm,)), &(ivm::VMError::HostOutputBudgetExceeded {
                resource: ivm::HostOutputResource::Items,
                attempted: 1,
                limit: 0,
            })), (Err(ivm::VMError::HostOutputBudgetExceeded { resource: __actual_resource, attempted: __actual_attempted, limit: __actual_limit }), ivm::VMError::HostOutputBudgetExceeded { resource: __expected_resource, attempted: __expected_attempted, limit: __expected_limit }) if __actual_resource == __expected_resource && __actual_attempted == __expected_attempted && __actual_limit == __expected_limit)
    );
    assert!(
        host.queued.is_empty(),
        "a rejected instruction must not be retained"
    );
}
#[test]
fn fastpq_batch_entry_syscall_returns_transfer_gas() {
    let authority: AccountId = fixture_account("alice");
    let mut host = CoreHost::new(authority.clone());
    host.restrict_output_limits(HostOutputLimits::new(1, 1024 * 1024));
    let mut vm = IVM::new(1_000);
    vm.load_program(&ivm::ProgramMetadata::default().encode())
        .expect("load meta");
    host.syscall(ivm_sys::SYSCALL_TRANSFER_V1_BATCH_BEGIN, &mut vm)
        .expect("begin batch");
    let from = authority.clone();
    let to: AccountId = fixture_account("bob");
    let asset_def: AssetDefinitionId =
        iroha_data_model::asset::AssetDefinitionId::derive_from_components(
            DomainId::try_new("wonderland", "universal").unwrap(),
            "xor".parse().unwrap(),
        );
    let amount = 7_u64;
    let from_payload = norito::to_bytes(&from).expect("encode from account");
    let to_payload = norito::to_bytes(&to).expect("encode to account");
    let asset_payload = norito::to_bytes(&asset_def).expect("encode asset definition");
    let from_tlv = make_tlv(PointerType::AccountId as u16, &from_payload);
    let to_tlv = make_tlv(PointerType::AccountId as u16, &to_payload);
    let asset_tlv = make_tlv(PointerType::AssetDefinitionId as u16, &asset_payload);
    let amount_tlv = make_tlv(
        PointerType::Quantity as u16,
        &quantity_frame(&Quantity::from(amount)),
    );
    let amount_offset = 768u64;
    vm.memory.preload_input(0, &from_tlv).expect("preload from");
    vm.memory.preload_input(256, &to_tlv).expect("preload to");
    vm.memory
        .preload_input(512, &asset_tlv)
        .expect("preload asset");
    vm.memory
        .preload_input(amount_offset, &amount_tlv)
        .expect("preload amount");
    vm.set_register(10, ivm::Memory::INPUT_START);
    vm.set_register(11, ivm::Memory::INPUT_START + 256);
    vm.set_register(12, ivm::Memory::INPUT_START + 512);
    vm.set_register(13, ivm::Memory::INPUT_START + amount_offset);
    let gas = host
        .syscall(ivm_sys::SYSCALL_TRANSFER_V1, &mut vm)
        .expect("batch entry");
    for _ in 0..10_000 {
        assert!(
            matches!((&(host.syscall(ivm_sys::SYSCALL_TRANSFER_V1, &mut vm)), &(ivm::VMError::HostOutputBudgetExceeded {
                    resource: ivm::HostOutputResource::Items,
                    attempted: 2,
                    limit: 1,
                })), (Err(ivm::VMError::HostOutputBudgetExceeded { resource: __actual_resource, attempted: __actual_attempted, limit: __actual_limit }), ivm::VMError::HostOutputBudgetExceeded { resource: __expected_resource, attempted: __expected_attempted, limit: __expected_limit }) if __actual_resource == __expected_resource && __actual_attempted == __expected_attempted && __actual_limit == __expected_limit)
        );
    }
    let asset_id = AssetId::of(asset_def, from.clone());
    let isi = Transfer::asset_quantity(asset_id, amount, to);
    let expected = crate::gas::meter_instruction(
        &InstructionBox::from(TransferBox::from(isi)),
        &crate::gas::DEFAULT_ZK_GAS,
    )
    .expect("completed fixture gas quote");
    assert_eq!(gas, expected);
    assert!(host.queued.is_empty());
    assert_eq!(host.fastpq_batch_entries.as_ref().map(Vec::len), Some(1));
    assert!(matches!(
        host.output_budget_violation(),
        Some(HostOutputBudgetViolation::ItemCount {
            attempted: 2,
            limit: 1
        })
    ));
}
fn scoped_transfer_state(
    authority: &AccountId,
    destination: &AccountId,
    asset_def: AssetDefinitionId,
    asset_name: &str,
    balance_policy: AssetBalancePolicy,
    owning_domain: Option<DomainId>,
) -> State {
    let domain_id = owning_domain.clone().unwrap_or_else(fixture_domain_id);
    let domain = Domain::new(domain_id.clone()).build(authority);
    let source_account = build_fixture_account(authority, authority);
    let destination_account = build_fixture_account(destination, authority);
    let asset_def = AssetDefinition::numeric(
        asset_def,
        asset_name.to_owned(),
        balance_policy,
        owning_domain,
    )
    .build(authority);
    let world = World::with([domain], [source_account, destination_account], [asset_def]);
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    State::new_for_testing(world, kura, query)
}
fn prepare_scoped_transfer_syscall(
    vm: &mut IVM,
    from: &AccountId,
    to: &AccountId,
    asset_def: &AssetDefinitionId,
    amount: &Quantity,
    dataspace: DataSpaceId,
) {
    let from_ptr = store_tlv(vm, PointerType::AccountId, &norito_blob(from));
    let to_ptr = store_tlv(vm, PointerType::AccountId, &norito_blob(to));
    let asset_ptr = store_tlv(vm, PointerType::AssetDefinitionId, &norito_blob(asset_def));
    let amount_ptr = store_quantity(vm, amount);
    let dataspace_ptr = store_tlv(
        vm,
        PointerType::DataSpaceId,
        &norito::to_bytes(&dataspace).expect("encode dataspace"),
    );
    vm.set_register(10, from_ptr);
    vm.set_register(11, to_ptr);
    vm.set_register(12, asset_ptr);
    vm.set_register(13, amount_ptr);
    vm.set_register(14, dataspace_ptr);
}
#[test]
fn transfer_asset_scoped_syscall_queues_global_source_for_global_definition() {
    let authority: AccountId = fixture_account("alice");
    let destination: AccountId = fixture_account("bob");
    let asset_def =
        AssetDefinitionId::derive_from_components(fixture_domain_id(), "xor".parse().unwrap());
    let state = scoped_transfer_state(
        &authority,
        &destination,
        asset_def.clone(),
        "xor",
        AssetBalancePolicy::Global,
        None,
    );
    let view = state.view();
    let mut host = CoreHostImpl::new(authority.clone());
    host.set_query_state(&view);
    let mut vm = IVM::new(1_000);
    let amount = Quantity::from(5_u32);
    prepare_scoped_transfer_syscall(
        &mut vm,
        &authority,
        &destination,
        &asset_def,
        &amount,
        DataSpaceId::new(7),
    );
    host.syscall(ivm_sys::SYSCALL_TRANSFER_ASSET_SCOPED, &mut vm)
        .expect("global transfer should enqueue");
    let expected = InstructionBox::from(TransferBox::from(Transfer::asset_quantity(
        AssetId::of(asset_def, authority),
        amount,
        destination,
    )));
    assert_eq!(host.queued, vec![expected]);
}
#[test]
fn transfer_asset_scoped_vm_completes_below_generic_response_quote() {
    let authority = fixture_account("alice");
    let destination = fixture_account("bob");
    let owning_domain = fixture_domain_id();
    let asset_def =
        AssetDefinitionId::derive_from_components(owning_domain.clone(), "rose".parse().unwrap());
    let amount = Quantity::from(5_u32);
    let dataspace = DataSpaceId::new(7);
    let instructions = [
        encoding::wide::encode_ri(instruction::wide::arithmetic::ADDI, 13, 17, 0),
        encoding::wide::encode_ri(instruction::wide::arithmetic::ADDI, 14, 18, 0),
        encoding::wide::encode_sys(
            instruction::wide::system::SCALL,
            u8::try_from(ivm_sys::SYSCALL_TRANSFER_ASSET_SCOPED).expect("syscall fits"),
        ),
    ];
    let code: Vec<u8> = instructions
        .into_iter()
        .flat_map(u32::to_le_bytes)
        .collect();
    for balance_policy in [
        AssetBalancePolicy::Global,
        AssetBalancePolicy::DataspaceRestricted,
    ] {
        let state = scoped_transfer_state(
            &authority,
            &destination,
            asset_def.clone(),
            "rose",
            balance_policy,
            Some(owning_domain.clone()),
        );
        let view = state.view();
        let mut host = CoreHostImpl::new(authority.clone());
        host.set_local_contract_debug_execution();
        host.set_query_state(&view);
        let mut vm = IVM::new(1_000);
        vm.load_program(&build_authenticated_test_contract_program(&code, 0, false))
            .expect("load authenticated scoped-transfer probe");
        prepare_scoped_transfer_syscall(
            &mut vm,
            &authority,
            &destination,
            &asset_def,
            &amount,
            dataspace,
        );
        assert_eq!(
            (host.prepare_syscall(ivm_sys::SYSCALL_TRANSFER_ASSET_SCOPED, &vm))
                .expect("expected successful fixture result"),
            crate::gas::BASE_TRANSFER
        );
        assert!(
            ivm::host::conservative_syscall_gas_quote(ivm_sys::SYSCALL_TRANSFER_ASSET_SCOPED, &vm)
                > vm.remaining_gas()
        );
        let operands = [10, 11, 12, 13, 14].map(|register| vm.register(register));
        for (index, pointer) in operands.into_iter().enumerate() {
            vm.set_register(14 + index, pointer);
        }
        vm.run_with_host(&mut host)
            .expect("complete scoped transfer and protected return");
        let scope = match balance_policy {
            AssetBalancePolicy::Global => AssetBalanceScope::Global,
            AssetBalancePolicy::DataspaceRestricted => AssetBalanceScope::Dataspace(dataspace),
        };
        let expected = InstructionBox::from(TransferBox::from(Transfer::asset_quantity(
            AssetId::with_scope(asset_def.clone(), authority.clone(), scope),
            amount.clone(),
            destination.clone(),
        )));
        assert_eq!(
            crate::gas::meter_instruction(&expected, &crate::gas::DEFAULT_ZK_GAS)
                .expect("completed fixture gas quote"),
            crate::gas::BASE_TRANSFER
        );
        assert_eq!(host.queued, vec![expected]);
        assert_eq!(authenticated_test_probe_result(&vm), operands[0]);
        assert_eq!(vm.output_used_len(), 0, "transfer publishes no VM response");
        assert!(vm.remaining_gas() > 0);
        assert!(host.fastpq_batch_entries.is_none());
        assert!(host.durable_state_overlay.is_empty());
    }
}
#[test]
fn transfer_asset_scoped_vm_debits_exact_gas_before_queue_mutation() {
    let authority = fixture_account("alice");
    let destination = fixture_account("bob");
    let asset_def =
        AssetDefinitionId::derive_from_components(fixture_domain_id(), "xor".parse().unwrap());
    let amount = Quantity::from(5_u32);
    let instructions = [
        encoding::wide::encode_ri(instruction::wide::arithmetic::ADDI, 13, 17, 0),
        encoding::wide::encode_ri(instruction::wide::arithmetic::ADDI, 14, 18, 0),
        encoding::wide::encode_sys(
            instruction::wide::system::SCALL,
            u8::try_from(ivm_sys::SYSCALL_TRANSFER_ASSET_SCOPED).expect("syscall fits"),
        ),
    ];
    let gas_before_syscall = authenticated_test_probe_setup_gas()
        + instructions
            .iter()
            .map(|&word| ivm::gas::cost_of(word).unwrap())
            .sum::<u64>();
    let code: Vec<u8> = instructions
        .into_iter()
        .flat_map(u32::to_le_bytes)
        .collect();
    let state = scoped_transfer_state(
        &authority,
        &destination,
        asset_def.clone(),
        "xor",
        AssetBalancePolicy::Global,
        None,
    );
    let view = state.view();
    let expected = InstructionBox::from(TransferBox::from(Transfer::asset_quantity(
        AssetId::of(asset_def.clone(), authority.clone()),
        amount.clone(),
        destination.clone(),
    )));
    for available in [crate::gas::BASE_TRANSFER - 1, crate::gas::BASE_TRANSFER] {
        let mut host = CoreHostImpl::new(authority.clone());
        host.set_local_contract_debug_execution();
        host.set_query_state(&view);
        let mut vm = IVM::new(gas_before_syscall + available);
        vm.load_program(&build_authenticated_test_contract_program(&code, 0, false))
            .expect("load authenticated exact-budget scoped-transfer probe");
        prepare_scoped_transfer_syscall(
            &mut vm,
            &authority,
            &destination,
            &asset_def,
            &amount,
            DataSpaceId::new(7),
        );
        let operands = [10, 11, 12, 13, 14].map(|register| vm.register(register));
        for (index, pointer) in operands.into_iter().enumerate() {
            vm.set_register(14 + index, pointer);
        }
        // The exact budget completes only the syscall; it intentionally has
        // no gas for the protected-return epilogue. One less gas must refuse
        // the syscall itself, before the native instruction is queued.
        assert!(matches!(
            vm.run_with_host(&mut host),
            Err(ivm::VMError::OutOfGas)
        ));
        if available == crate::gas::BASE_TRANSFER {
            assert_eq!(vm.remaining_gas(), 0);
            assert_eq!(host.queued, vec![expected.clone()]);
        } else {
            // Refusing the unaffordable quote precedes its debit and native queue mutation.
            assert_eq!(vm.remaining_gas(), available);
            assert!(
                host.queued.is_empty(),
                "short gas must not queue a transfer"
            );
        }
        assert_eq!(vm.output_used_len(), 0);
        assert!(host.durable_state_overlay.is_empty());
    }
}
#[test]
fn transfer_asset_scoped_quote_rejects_invalid_shapes_and_active_batch() {
    let authority = fixture_account("alice");
    let destination = fixture_account("bob");
    let asset_def =
        AssetDefinitionId::derive_from_components(fixture_domain_id(), "xor".parse().unwrap());
    let amount = Quantity::from(5_u32);
    let mut host = local_contract_host(authority.clone());
    let mut vm = IVM::new(1_000);
    prepare_scoped_transfer_syscall(
        &mut vm,
        &authority,
        &destination,
        &asset_def,
        &amount,
        DataSpaceId::new(7),
    );
    assert_eq!(
        (host.prepare_syscall(ivm_sys::SYSCALL_TRANSFER_ASSET_SCOPED, &vm))
            .expect("expected successful fixture result"),
        crate::gas::BASE_TRANSFER
    );
    host.fastpq_batch_entries = Some(Vec::new());
    assert!(matches!(
        host.prepare_syscall(ivm_sys::SYSCALL_TRANSFER_ASSET_SCOPED, &vm),
        Err(ivm::VMError::PermissionDenied)
    ));
    host.fastpq_batch_entries = None;
    let pointer = store_tlv(&mut vm, PointerType::Name, b"not-a-dataspace");
    vm.set_register(14, pointer);
    assert!(matches!(
        host.prepare_syscall(ivm_sys::SYSCALL_TRANSFER_ASSET_SCOPED, &vm),
        Err(ivm::VMError::NoritoInvalid)
    ));
    assert_eq!(vm.remaining_gas(), 1_000, "quote must not debit gas");
    assert!(host.queued.is_empty());
    assert!(host.durable_state_overlay.is_empty());
}
#[test]
fn transfer_v1_syscall_rejects_outside_an_active_batch() {
    let authority: AccountId = fixture_account("alice");
    let destination: AccountId = fixture_account("bob");
    let asset_def =
        AssetDefinitionId::derive_from_components(fixture_domain_id(), "xor".parse().unwrap());
    let state = scoped_transfer_state(
        &authority,
        &destination,
        asset_def.clone(),
        "xor",
        AssetBalancePolicy::Global,
        None,
    );
    let view = state.view();
    let mut host = CoreHostImpl::new(authority.clone());
    host.set_query_state(&view);
    let mut vm = IVM::new(1_000);
    let amount = Quantity::from(5_u32);
    prepare_scoped_transfer_syscall(
        &mut vm,
        &authority,
        &destination,
        &asset_def,
        &amount,
        DataSpaceId::new(7),
    );
    assert!(matches!(
        host.syscall(ivm_sys::SYSCALL_TRANSFER_V1, &mut vm),
        Err(ivm::VMError::PermissionDenied)
    ));
    assert!(host.queued.is_empty());
}
#[test]
fn transfer_asset_scoped_syscall_queues_dataspace_source_for_restricted_definition() {
    let authority: AccountId = fixture_account("alice");
    let destination: AccountId = fixture_account("bob");
    let owning_domain = fixture_domain_id();
    let asset_def =
        AssetDefinitionId::derive_from_components(owning_domain.clone(), "rose".parse().unwrap());
    let dataspace = DataSpaceId::new(7);
    let state = scoped_transfer_state(
        &authority,
        &destination,
        asset_def.clone(),
        "rose",
        AssetBalancePolicy::DataspaceRestricted,
        Some(owning_domain),
    );
    let view = state.view();
    let mut host = CoreHostImpl::new(authority.clone());
    host.set_query_state(&view);
    let mut vm = IVM::new(1_000);
    let amount = Quantity::from(5_u32);
    prepare_scoped_transfer_syscall(
        &mut vm,
        &authority,
        &destination,
        &asset_def,
        &amount,
        dataspace,
    );
    host.syscall(ivm_sys::SYSCALL_TRANSFER_ASSET_SCOPED, &mut vm)
        .expect("restricted transfer should enqueue");
    let expected = InstructionBox::from(TransferBox::from(Transfer::asset_quantity(
        AssetId::with_scope(
            asset_def,
            authority,
            AssetBalanceScope::Dataspace(dataspace),
        ),
        amount,
        destination,
    )));
    assert_eq!(host.queued, vec![expected]);
}
#[test]
fn transfer_asset_scoped_syscall_rejects_non_dataspace_r14() {
    let authority: AccountId = fixture_account("alice");
    let destination: AccountId = fixture_account("bob");
    let owning_domain = fixture_domain_id();
    let asset_def =
        AssetDefinitionId::derive_from_components(owning_domain.clone(), "rose".parse().unwrap());
    let state = scoped_transfer_state(
        &authority,
        &destination,
        asset_def.clone(),
        "rose",
        AssetBalancePolicy::DataspaceRestricted,
        Some(owning_domain),
    );
    let view = state.view();
    let mut host = CoreHostImpl::new(authority.clone());
    host.set_query_state(&view);
    let mut vm = IVM::new(1_000);
    let amount = Quantity::from(5_u32);
    prepare_scoped_transfer_syscall(
        &mut vm,
        &authority,
        &destination,
        &asset_def,
        &amount,
        DataSpaceId::new(7),
    );
    let wrong_dataspace_ptr = store_tlv(&mut vm, PointerType::Name, b"not-a-dataspace");
    vm.set_register(14, wrong_dataspace_ptr);
    let err = host
        .syscall(ivm_sys::SYSCALL_TRANSFER_ASSET_SCOPED, &mut vm)
        .expect_err("wrong r14 pointer type should reject before queueing");
    assert!(matches!(err, ivm::VMError::NoritoInvalid));
    assert!(host.queued.is_empty());
}
#[test]
fn transfer_asset_scoped_syscall_without_dataspace_is_global_or_rejected() {
    // `ledger::asset::transfer` without `dataspace:` passes r14 = 0: a global
    // definition moves its global balance, and a dataspace-restricted one is
    // rejected instead of falling back to any dataspace.
    let authority: AccountId = fixture_account("alice");
    let destination: AccountId = fixture_account("bob");
    let owning_domain = fixture_domain_id();
    let amount = Quantity::from(5_u32);
    for (asset_name, policy) in [
        ("xor", AssetBalancePolicy::Global),
        ("rose", AssetBalancePolicy::DataspaceRestricted),
    ] {
        let asset_def = AssetDefinitionId::derive_from_components(
            owning_domain.clone(),
            asset_name.parse().unwrap(),
        );
        let state = scoped_transfer_state(
            &authority,
            &destination,
            asset_def.clone(),
            asset_name,
            policy,
            matches!(policy, AssetBalancePolicy::DataspaceRestricted)
                .then(|| owning_domain.clone()),
        );
        let view = state.view();
        let mut host = CoreHostImpl::new(authority.clone());
        host.set_query_state(&view);
        let mut vm = IVM::new(1_000);
        prepare_scoped_transfer_syscall(
            &mut vm,
            &authority,
            &destination,
            &asset_def,
            &amount,
            DataSpaceId::new(7),
        );
        vm.set_register(14, 0);
        assert_eq!(
            (host.prepare_syscall(ivm_sys::SYSCALL_TRANSFER_ASSET_SCOPED, &vm))
                .expect("expected successful fixture result"),
            crate::gas::BASE_TRANSFER,
            "an omitted scope is a valid quote for {asset_name}"
        );
        let result = host.syscall(ivm_sys::SYSCALL_TRANSFER_ASSET_SCOPED, &mut vm);
        match policy {
            AssetBalancePolicy::Global => {
                result.expect("global transfer without a scope should enqueue");
                let expected = InstructionBox::from(TransferBox::from(Transfer::asset_quantity(
                    AssetId::of(asset_def, authority.clone()),
                    amount.clone(),
                    destination.clone(),
                )));
                assert_eq!(host.queued, vec![expected]);
            }
            AssetBalancePolicy::DataspaceRestricted => {
                assert!(matches!(result, Err(ivm::VMError::NoritoInvalid)));
                assert!(host.queued.is_empty());
            }
        }
    }
}
#[test]
fn fastpq_batch_apply_syscall_returns_batch_gas() {
    let authority: AccountId = fixture_account("alice");
    let mut host = CoreHost::new(authority.clone());
    let mut vm = IVM::new(1_000);
    vm.load_program(&ivm::ProgramMetadata::default().encode())
        .expect("load meta");
    let from = authority;
    let to: AccountId = fixture_account("bob");
    let asset_def: AssetDefinitionId =
        iroha_data_model::asset::AssetDefinitionId::derive_from_components(
            DomainId::try_new("wonderland", "universal").unwrap(),
            "xor".parse().unwrap(),
        );
    let entries = vec![
        TransferAssetBatchEntry::new(from.clone(), to.clone(), asset_def.clone(), 1_u64),
        TransferAssetBatchEntry::new(from.clone(), to.clone(), asset_def.clone(), 2_u64),
    ];
    let batch = TransferAssetBatch::new(entries);
    let payload = norito::to_bytes(&batch).expect("encode batch");
    let tlv = make_tlv(PointerType::NoritoBytes as u16, &payload);
    vm.memory.preload_input(0, &tlv).expect("preload batch");
    vm.set_register(10, ivm::Memory::INPUT_START);
    let gas = host
        .syscall(ivm_sys::SYSCALL_TRANSFER_V1_BATCH_APPLY, &mut vm)
        .expect("apply batch");
    let expected = InstructionBox::from(batch.clone());
    let expected_gas = crate::gas::meter_instruction(&expected, &crate::gas::DEFAULT_ZK_GAS)
        .expect("completed fixture gas quote");
    assert_eq!(gas, expected_gas);
    assert_eq!(host.queued, vec![expected]);
}
#[test]
fn fastpq_batch_apply_rejects_noncanonical_amount() {
    #[derive(norito::NoritoSchema)]
    #[norito_schema(
        name = "iroha_core::smartcontracts::ivm::host::tests::fastpq_batch_apply_rejects_noncanonical_amount::NonCanonicalNumeric"
    )]
    #[derive(norito::Encode)]
    struct NonCanonicalNumeric {
        #[codec(compact)]
        mantissa: iroha_primitives::bigint::BigInt,
        #[codec(compact)]
        scale: u32,
    }
    #[derive(norito::NoritoSchema)]
    #[norito_schema(
        name = "iroha_core::smartcontracts::ivm::host::tests::fastpq_batch_apply_rejects_noncanonical_amount::ForgedTransferAssetBatchEntry"
    )]
    #[derive(norito::Encode)]
    struct ForgedTransferAssetBatchEntry {
        from: AccountId,
        to: AccountId,
        asset_definition: AssetDefinitionId,
        amount: NonCanonicalNumeric,
    }
    #[derive(norito::NoritoSchema)]
    #[norito_schema(
        name = "iroha_core::smartcontracts::ivm::host::tests::fastpq_batch_apply_rejects_noncanonical_amount::ForgedTransferAssetBatch"
    )]
    #[derive(norito::Encode)]
    struct ForgedTransferAssetBatch {
        entries: Vec<ForgedTransferAssetBatchEntry>,
    }
    let authority: AccountId = fixture_account("alice");
    let mut host = CoreHost::new(authority.clone());
    let mut vm = IVM::new(1_000);
    let batch = ForgedTransferAssetBatch {
        entries: vec![ForgedTransferAssetBatchEntry {
            from: authority,
            to: fixture_account("bob"),
            asset_definition: AssetDefinitionId::derive_from_components(
                fixture_domain_id(),
                "xor".parse().unwrap(),
            ),
            amount: NonCanonicalNumeric {
                mantissa: 10_u32.into(),
                scale: 1,
            },
        }],
    };
    let entry = &batch.entries[0];
    let current = TransferAssetBatch::new(vec![TransferAssetBatchEntry::new(
        entry.from.clone(),
        entry.to.clone(),
        entry.asset_definition.clone(),
        1_u64,
    )]);
    let (current_payload, current_flags, forged_payload, flags) = {
        let _canonical =
            norito::core::DecodeFlagsGuard::enter(norito::core::default_encode_flags());
        let (current_payload, current_flags) = norito::codec::encode_with_header_flags(&current);
        let (forged_payload, flags) = norito::codec::encode_with_header_flags(&batch);
        (current_payload, current_flags, forged_payload, flags)
    };
    let control = norito::core::frame_bare_with_header_flags::<TransferAssetBatch>(
        &current_payload,
        current_flags,
    )
    .unwrap();
    assert_eq!(control, norito::encode_canonical(&current).unwrap());
    let mut control_host = CoreHost::new(entry.from.clone());
    let mut control_vm = IVM::new(1_000);
    control_vm
        .load_program(&ivm::ProgramMetadata::default().encode())
        .expect("load control metadata");
    let control_ptr = store_tlv(&mut control_vm, PointerType::NoritoBytes, &control);
    control_vm.set_register(10, control_ptr);
    let gas = control_host
        .syscall(ivm_sys::SYSCALL_TRANSFER_V1_BATCH_APPLY, &mut control_vm)
        .expect("current canonical amount passes through the same frame construction");
    let expected = InstructionBox::from(current);
    assert_eq!(
        gas,
        crate::gas::meter_instruction(&expected, &crate::gas::DEFAULT_ZK_GAS)
            .expect("completed fixture gas quote")
    );
    assert_eq!(control_host.queued, vec![expected]);
    let payload =
        norito::core::frame_bare_with_header_flags::<TransferAssetBatch>(&forged_payload, flags)
            .unwrap();
    let view = norito::core::from_bytes_view(&payload)
        .expect("valid current batch owner, length and checksum");
    assert_eq!(
        view.schema(),
        norito::schema::identity::frame_hash::<TransferAssetBatch>()
    );
    assert_eq!(view.as_bytes(), forged_payload.as_slice());
    let error = norito::decode_canonical::<TransferAssetBatch>(&payload)
        .expect_err("noncanonical numeric payload must fail inside the actual batch owner");
    assert!(!matches!(error, norito::Error::SchemaMismatch));
    let ptr = store_tlv(&mut vm, PointerType::NoritoBytes, &payload);
    vm.set_register(10, ptr);
    assert!(matches!(
        host.syscall(ivm_sys::SYSCALL_TRANSFER_V1_BATCH_APPLY, &mut vm),
        Err(ivm::VMError::DecodeError)
    ));
    assert!(host.queued.is_empty());
}
pub(super) fn make_policy_snapshot(
    dsid: DataSpaceId,
    manifest_root: [u8; 32],
    current_slot: u64,
) -> AxtPolicySnapshot {
    let binding = AxtPolicyBinding {
        dsid,
        policy: AxtPolicyEntry {
            manifest_root,
            target_lane: LaneId::new(1),
            active_handle_era: 1,
            next_handle_counter: 1,
            current_slot,
        },
    };
    AxtPolicySnapshot {
        version: AxtPolicySnapshot::compute_version(&[binding]),
        entries: vec![binding],
    }
}
pub(super) fn proof_blob_for(
    dsid: DataSpaceId,
    manifest_root: [u8; 32],
    proof_seed: &[u8],
    expiry_slot: u64,
) -> ProofBlob {
    proof_blob_for_profile(dsid, manifest_root, proof_seed, expiry_slot, false)
}
pub(super) fn opaque_proof_blob_for(
    dsid: DataSpaceId,
    manifest_root: [u8; 32],
    proof_seed: &[u8],
    expiry_slot: u64,
) -> ProofBlob {
    proof_blob_for_profile(dsid, manifest_root, proof_seed, expiry_slot, true)
}
fn proof_blob_for_profile(
    dsid: DataSpaceId,
    manifest_root: [u8; 32],
    proof_seed: &[u8],
    expiry_slot: u64,
    opaque: bool,
) -> ProofBlob {
    let source_tx_commitment = test_digest(b"axt-host-test:source-tx", &[proof_seed]);
    let claim_digest = test_digest(b"axt-host-test:claim", &[proof_seed]);
    let witness_commitment = test_digest(b"axt-host-test:witness", &[proof_seed]);
    let policy_commitment = test_digest(b"axt-host-test:policy", &[&manifest_root[..]]);
    let binding = iroha_data_model::nexus::AxtFastpqBinding {
        parameter: fastpq_prover::AXT_DEFAULT_PARAMETER.to_owned(),
        source_dsid: dsid.as_u64(),
        source_dataspace: format!("test-dataspace-{}", dsid.as_u64()),
        source_receipt_id: format!("receipt-{}", hex::encode(source_tx_commitment.as_ref())),
        source_tx_commitment: hex::encode(source_tx_commitment.as_ref()),
        claim_type: if opaque {
            "authorization".to_owned()
        } else {
            "tx_predicate".to_owned()
        },
        claim_digest: hex::encode(claim_digest.as_ref()),
        witness_commitment: hex::encode(witness_commitment.as_ref()),
        policy_commitment: hex::encode(policy_commitment.as_ref()),
        verified_effect_type: "test_effect".to_owned(),
        corridor: "test-corridor".to_owned(),
        verifier_id: "fastpq".to_owned(),
        verifier_version: "v1".to_owned(),
        target_dsids: vec![dsid.as_u64()],
        effect_binding: None,
        remote_spend_intent_commitments: Vec::new(),
    };
    let mut dsid_bytes = [0_u8; 16];
    dsid_bytes[..8].copy_from_slice(&dsid.as_u64().to_le_bytes());
    let mut batch = fastpq_prover::TransitionBatch::new(
        fastpq_prover::AXT_DEFAULT_PARAMETER,
        fastpq_prover::PublicInputs {
            dsid: dsid_bytes,
            slot: expiry_slot,
            old_root: test_digest(b"axt-host-test:old-root", &[proof_seed]).into(),
            new_root: manifest_root,
            perm_root: test_digest(b"axt-host-test:perm-root", &[proof_seed]).into(),
            tx_set_hash: test_digest(b"axt-host-test:tx-set", &[proof_seed]).into(),
        },
    );
    if opaque {
        batch.push(fastpq_prover::StateTransition::new(
            b"axt/host/proof".to_vec(),
            proof_seed.to_vec(),
            manifest_root.to_vec(),
            fastpq_prover::OperationKind::MetaSet,
        ));
    } else {
        let asset_definition = AssetDefinitionId::derive_from_components(
            fixture_domain_id(),
            "xor".parse().expect("fixture asset name"),
        );
        let from_account = fixture_account("alice");
        let to_account = fixture_account("bob");
        let mut transcripts = vec![iroha_data_model::fastpq::TransferTranscript {
            batch_hash: source_tx_commitment,
            deltas: vec![iroha_data_model::fastpq::TransferDeltaTranscript {
                from_account: from_account.clone(),
                to_account: to_account.clone(),
                asset_definition: asset_definition.clone(),
                amount: Quantity::from(1_u64),
                from_balance_before: Quantity::from(10_u64),
                from_balance_after: Quantity::from(9_u64),
                to_balance_before: Quantity::from(5_u64),
                to_balance_after: Quantity::from(6_u64),
                from_smt_witness: iroha_data_model::fastpq::TransferSmtWitness::default(),
                to_smt_witness: iroha_data_model::fastpq::TransferSmtWitness::default(),
            }],
            authority_digest: Hash::new(b"axt-host-test:transfer-authority"),
            poseidon_preimage_digest: None,
        }];
        let (old_root, new_root) =
            fastpq_prover::gadgets::transfer::attach_transfer_smt_witnesses(&mut transcripts)
                .expect("attach transfer SMT witnesses");
        batch.public_inputs.old_root = old_root;
        batch.public_inputs.new_root = new_root;
        let poseidon_preimage_digest = fastpq_prover::gadgets::transfer::compute_poseidon_digest(
            &transcripts[0].deltas[0],
            &transcripts[0].batch_hash,
        );
        transcripts[0].poseidon_preimage_digest = Some(poseidon_preimage_digest);
        batch.push(fastpq_prover::StateTransition::new(
            iroha_data_model::fastpq::transfer_balance_key(&asset_definition, &from_account)
                .expect("canonical balance key"),
            10_u64.to_le_bytes().to_vec(),
            9_u64.to_le_bytes().to_vec(),
            fastpq_prover::OperationKind::Transfer,
        ));
        batch.push(fastpq_prover::StateTransition::new(
            iroha_data_model::fastpq::transfer_balance_key(&asset_definition, &to_account)
                .expect("canonical balance key"),
            5_u64.to_le_bytes().to_vec(),
            6_u64.to_le_bytes().to_vec(),
            fastpq_prover::OperationKind::Transfer,
        ));
        batch.metadata.insert(
            iroha_data_model::fastpq::TRANSFER_TRANSCRIPTS_METADATA_KEY.to_owned(),
            norito::to_bytes(&transcripts).expect("encode transfer transcripts"),
        );
    }
    batch.sort();
    batch.metadata.insert(
        "entry_hash".to_owned(),
        source_tx_commitment.as_ref().to_vec(),
    );
    crate::fastpq::quantity_fixture::materialize(&mut batch);
    fastpq_prover::bind_axt_batch_with_proof_metadata(
        &mut batch,
        &binding,
        manifest_root,
        None,
        None,
        Some(expiry_slot),
    )
    .expect("bind AXT test batch");
    let fastpq_payload = if opaque {
        // This fixture exercises semantic rejection before child proof decoding.
        vec![0]
    } else {
        crate::unit_test_support::prove_axt_bound_batch_when_available(&batch, &binding)
    };
    let envelope = axt::AxtProofEnvelope {
        dsid,
        manifest_root,
        da_commitment: None,
        proof: fastpq_payload,
        fastpq_binding: Some(binding),
        committed_amount: None,
        amount_commitment: None,
    };
    ProofBlob {
        payload: norito::to_bytes(&envelope).expect("encode proof envelope"),
        expiry_slot: Some(expiry_slot),
    }
}
fn test_digest(domain: &[u8], parts: &[&[u8]]) -> iroha_crypto::Hash {
    let mut payload = Vec::new();
    payload.extend_from_slice(domain);
    for part in parts {
        payload.extend_from_slice(part);
    }
    iroha_crypto::Hash::new(payload)
}
#[test]
fn compute_envelope_hash_rejects_invalid_payload() {
    let err = CoreHost::compute_envelope_hash(b"invalid envelope")
        .expect_err("invalid envelope should fail");
    assert!(matches!(err, ivm::VMError::NoritoInvalid));
}
#[test]
fn compute_envelope_hash_accepts_norito_envelope() {
    let envelope = sample_open_verify_envelope();
    let payload = norito::to_bytes(&envelope).expect("encode envelope");
    let hash = CoreHost::compute_envelope_hash(&payload).expect("hash envelope");
    let expected: [u8; 32] = iroha_crypto::Hash::new(&payload).into();
    assert_eq!(hash, expected);
}
#[test]
fn compute_envelope_hash_rejects_opaque_bytes_unconditionally() {
    assert!(matches!(
        CoreHost::compute_envelope_hash(b"opaque bytes"),
        Err(ivm::VMError::NoritoInvalid)
    ));
}
#[test]
fn norito_blob_roundtrips_with_header_decoder() {
    let proof = ProofBlob {
        payload: vec![1, 2, 3],
        expiry_slot: Some(42),
    };
    let bytes = norito_blob(&proof);
    let decoded: ProofBlob = norito::decode_from_bytes(&bytes).expect("decode via header framing");
    assert_eq!(decoded, proof);
}
#[test]
fn get_authority_allocates_in_input_bump() {
    let authority: AccountId = fixture_account("alice");
    let mut host = CoreHost::new(authority.clone());
    let mut vm = IVM::new(1_000_000);
    vm.load_program(&ivm::ProgramMetadata::default().encode())
        .expect("load meta");
    host.syscall(ivm::syscalls::SYSCALL_GET_AUTHORITY, &mut vm)
        .expect("authority syscall");
    let authority_ptr = vm.register(10);
    let marker: Name = "marker".parse().unwrap();
    let _marker_ptr = store_tlv(&mut vm, PointerType::Name, &norito_blob(&marker));
    let decoded: AccountId = CoreHost::decode_tlv_typed(&vm, authority_ptr, PointerType::AccountId)
        .expect("authority TLV should remain intact");
    assert_eq!(decoded, authority);
}
#[test]
fn get_authority_spills_to_owned_heap_after_input_exhaustion() {
    let authority: AccountId = fixture_account("alice");
    let mut host = CoreHost::new(authority.clone());
    let mut vm = IVM::new(1_000_000);
    vm.load_program(&ivm::ProgramMetadata::default().encode())
        .expect("load metadata");
    vm.alloc_input_tlv(&vec![
        0;
        usize::try_from(ivm::Memory::INPUT_SIZE)
            .expect("fixture value fits usize")
    ])
    .expect("fill INPUT exactly");
    host.syscall(ivm::syscalls::SYSCALL_GET_AUTHORITY, &mut vm)
        .expect("authority syscall must spill");
    let authority_pointer = vm.register(10);
    assert!((ivm::Memory::HEAP_START..ivm::Memory::INPUT_START).contains(&authority_pointer));
    let decoded: AccountId =
        CoreHost::decode_tlv_typed(&vm, authority_pointer, PointerType::AccountId)
            .expect("decode HEAP-backed authority");
    assert_eq!(decoded, authority);
}
#[test]
fn current_time_syscall_uses_block_time() {
    let authority: AccountId = fixture_account("alice");
    let mut host = CoreHost::new(authority);
    host.set_block_time_ms(1_717_171_717_000);
    let mut vm = IVM::new(1_000_000);
    vm.load_program(&ivm::ProgramMetadata::default().encode())
        .expect("load meta");
    assert_eq!(
        (host.syscall(ivm::syscalls::SYSCALL_CURRENT_TIME_MS, &mut vm))
            .expect("expected successful fixture result"),
        CoreHost::sysvar_gas(0)
    );
    assert_eq!(vm.register(10), 1_717_171_717_000);
}
#[test]
fn resolve_account_alias_syscall_reads_current_alias_binding() {
    let authority: AccountId = fixture_account("alice");
    let merchant_account_id: AccountId = fixture_account("bob");
    let replacement_account_id: AccountId = fixture_account("carol");
    let domain = Domain::new(fixture_domain_id()).build(&authority);
    let authority_account = Account::new(authority.clone()).build(&authority);
    let merchant_account = Account::new(merchant_account_id.clone()).build(&authority);
    let replacement_account = Account::new(replacement_account_id.clone()).build(&authority);
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let world = World::with(
        [domain],
        [authority_account, merchant_account, replacement_account],
        [],
    );
    let mut state = State::new_for_testing(world, kura, query);
    let (paynet, catalog) = retail_dataspace_catalog();
    state.set_dataspace_catalog_for_testing(catalog);
    let alias_literal = "merchant@paynet";
    let header = BlockHeader::new(nonzero!(1_u64), None, None, 0, 0);
    let mut block = state.block(header);
    let mut tx = block.transaction();
    seed_test_call_hash(&mut tx, 0xA1);
    tx.nexus.dataspace_catalog = state.nexus.read().dataspace_catalog.clone();
    tx.world.dataspace_catalog = state.nexus.read().dataspace_catalog.clone();
    let alias = AccountAlias::domainless("merchant".parse().expect("alias label"), paynet);
    tx.world_mut_for_testing().add_account_permission(
        &authority,
        Permission::from(CanResolveAccountAlias {
            scope: AccountAliasPermissionScope::Dataspace(paynet),
        }),
    );
    tx.world_mut_for_testing().add_account_permission(
        &authority,
        Permission::from(CanManageAccountAlias {
            scope: AccountAliasPermissionScope::Dataspace(paynet),
        }),
    );
    EnsureTestAccountAliasBinding::bind(merchant_account_id.clone(), alias.clone(), None)
        .execute(&authority, &mut tx)
        .expect("bind alias");
    tx.apply();
    block
        .commit_world_overlay_for_testing()
        .expect("commit permissions");
    let first_view = state.view();
    let mut host = CoreHostImpl::new(authority.clone());
    host.set_query_state(&first_view);
    let mut vm = IVM::new(1_000_000);
    let alias_ptr = store_tlv(&mut vm, PointerType::Blob, alias_literal.as_bytes());
    vm.set_register(10, alias_ptr);
    let gas = host
        .syscall(ivm_sys::SYSCALL_RESOLVE_ACCOUNT_ALIAS, &mut vm)
        .expect("resolve alias syscall");
    assert!(gas > 0);
    let resolved: AccountId =
        CoreHost::decode_tlv_typed(&vm, vm.register(10), PointerType::AccountId)
            .expect("resolved account id");
    assert_eq!(resolved, merchant_account_id);
    let next_height = u64::try_from((state.view().height() + 1).max(2))
        .ok()
        .and_then(core::num::NonZeroU64::new)
        .expect("next block height");
    let mut block = state.block(BlockHeader::new(next_height, None, None, 0, 0));
    let mut tx = block.transaction();
    seed_test_call_hash(&mut tx, 0xA2);
    EnsureTestAccountAliasBinding::bind(replacement_account_id.clone(), alias, None)
        .execute(&authority, &mut tx)
        .expect("rebind alias");
    tx.apply();
    block
        .commit_world_overlay_for_testing()
        .expect("commit alias rebind");
    let rebound_view = state.view();
    host.set_query_state(&rebound_view);
    let alias_ptr = store_tlv(&mut vm, PointerType::Blob, alias_literal.as_bytes());
    vm.set_register(10, alias_ptr);
    let gas = host
        .syscall(ivm_sys::SYSCALL_RESOLVE_ACCOUNT_ALIAS, &mut vm)
        .expect("resolve rebound alias syscall");
    assert!(gas > 0);
    let rebound: AccountId =
        CoreHost::decode_tlv_typed(&vm, vm.register(10), PointerType::AccountId)
            .expect("rebound account id");
    assert_eq!(rebound, replacement_account_id);
}
#[test]
fn resolve_account_alias_syscall_rejects_invalid_alias_literal_without_trimming() {
    let authority: AccountId = fixture_account("alice");
    let domain = Domain::new(fixture_domain_id()).build(&authority);
    let authority_account = Account::new(authority.clone()).build(&authority);
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let world = World::with([domain], [authority_account], []);
    let mut state = State::new_for_testing(world, kura, query);
    let (_paynet, catalog) = retail_dataspace_catalog();
    state.set_dataspace_catalog_for_testing(catalog);
    let header = BlockHeader::new(nonzero!(1_u64), None, None, 0, 0);
    let mut block = state.block(header);
    let mut tx = block.transaction();
    seed_test_call_hash(&mut tx, 0xA3);
    tx.nexus.dataspace_catalog = state.nexus.read().dataspace_catalog.clone();
    tx.world.dataspace_catalog = state.nexus.read().dataspace_catalog.clone();
    tx.apply();
    block
        .commit_world_overlay_for_testing()
        .expect("commit catalog");
    let view = state.view();
    let mut host = CoreHostImpl::new(authority);
    host.set_query_state(&view);
    let mut vm = IVM::new(1_000_000);
    let alias_ptr = store_tlv(&mut vm, PointerType::Blob, b" merchant@paynet ");
    vm.set_register(10, alias_ptr);
    let err = host
        .syscall(ivm_sys::SYSCALL_RESOLVE_ACCOUNT_ALIAS, &mut vm)
        .expect_err("invalid alias literal should fail at runtime");
    assert!(matches!(err, ivm::VMError::DecodeError));
}
#[test]
fn resolve_account_alias_syscall_requires_authoritative_alias_binding() {
    let authority: AccountId = fixture_account("alice");
    let merchant_account_id: AccountId = fixture_account("bob");
    let domain = Domain::new(fixture_domain_id()).build(&authority);
    let authority_account = Account::new(authority.clone()).build(&authority);
    let merchant_account = Account::new(merchant_account_id.clone()).build(&authority);
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let world = World::with([domain], [authority_account, merchant_account], []);
    let mut state = State::new_for_testing(world, kura, query);
    let (paynet, catalog) = retail_dataspace_catalog();
    state.set_dataspace_catalog_for_testing(catalog);
    let alias = AccountAlias::domainless("merchant".parse().expect("alias label"), paynet);
    let header = BlockHeader::new(nonzero!(1_u64), None, None, 0, 0);
    let mut block = state.block(header);
    let mut tx = block.transaction();
    seed_test_call_hash(&mut tx, 0xA4);
    tx.nexus.dataspace_catalog = state.nexus.read().dataspace_catalog.clone();
    tx.world.dataspace_catalog = state.nexus.read().dataspace_catalog.clone();
    tx.world_mut_for_testing().add_account_permission(
        &authority,
        Permission::from(CanResolveAccountAlias {
            scope: AccountAliasPermissionScope::Dataspace(paynet),
        }),
    );
    tx.world_mut_for_testing()
        .account_mut(&merchant_account_id)
        .expect("merchant account exists")
        .set_label(Some(alias));
    tx.apply();
    block
        .commit_world_overlay_for_testing()
        .expect("commit label-only alias state");
    let view = state.view();
    let mut host = CoreHostImpl::new(authority);
    host.set_query_state(&view);
    let mut vm = IVM::new(1_000_000);
    let alias_ptr = store_tlv(&mut vm, PointerType::Blob, b"merchant@paynet");
    vm.set_register(10, alias_ptr);
    let err = host
        .syscall(ivm_sys::SYSCALL_RESOLVE_ACCOUNT_ALIAS, &mut vm)
        .expect_err("primary labels alone must not resolve without a binding");
    assert!(matches!(err, ivm::VMError::DecodeError));
}
#[test]
fn resolve_account_alias_syscall_requires_dataspace_permission() {
    let authority: AccountId = fixture_account("alice");
    let merchant_account_id: AccountId = fixture_account("bob");
    let domain = Domain::new(fixture_domain_id()).build(&authority);
    let authority_account = Account::new(authority.clone()).build(&authority);
    let merchant_account = Account::new(merchant_account_id.clone()).build(&authority);
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let world = World::with([domain], [authority_account, merchant_account], []);
    let mut state = State::new_for_testing(world, kura, query);
    let (paynet, catalog) = retail_dataspace_catalog();
    state.set_dataspace_catalog_for_testing(catalog);
    let alias_literal = "merchant@paynet";
    let alias = AccountAlias::domainless("merchant".parse().expect("alias label"), paynet);
    let header = BlockHeader::new(nonzero!(1_u64), None, None, 0, 0);
    let mut block = state.block(header);
    let mut tx = block.transaction();
    seed_test_call_hash(&mut tx, 0xB1);
    tx.nexus.dataspace_catalog = state.nexus.read().dataspace_catalog.clone();
    tx.world.dataspace_catalog = state.nexus.read().dataspace_catalog.clone();
    tx.world_mut_for_testing().add_account_permission(
        &authority,
        Permission::from(CanManageAccountAlias {
            scope: AccountAliasPermissionScope::Dataspace(paynet),
        }),
    );
    EnsureTestAccountAliasBinding::bind(merchant_account_id, alias, None)
        .execute(&authority, &mut tx)
        .expect("bind alias");
    tx.apply();
    block
        .commit_world_overlay_for_testing()
        .expect("commit alias setup");
    let view = state.view();
    let mut host = CoreHostImpl::new(authority);
    host.set_query_state(&view);
    let mut vm = IVM::new(1_000_000);
    let alias_ptr = store_tlv(&mut vm, PointerType::Blob, alias_literal.as_bytes());
    vm.set_register(10, alias_ptr);
    let err = host
        .syscall(ivm_sys::SYSCALL_RESOLVE_ACCOUNT_ALIAS, &mut vm)
        .expect_err("alias resolution should require dataspace permission");
    assert!(matches!(err, ivm::VMError::PermissionDenied));
}
#[test]
fn resolve_account_alias_syscall_reads_current_domain_qualified_binding() {
    let authority: AccountId = fixture_account("alice");
    let merchant_account_id: AccountId = fixture_account("bob");
    let replacement_account_id: AccountId = fixture_account("carol");
    let payment_asset_definition_id: AssetDefinitionId = "61CtjvNd9T3THAR65GsMVHr82Bjc"
        .parse()
        .expect("payment asset definition id");
    let base_domain = Domain::new(fixture_domain_id()).build(&authority);
    let retail_domain_id = DomainId::try_new("bank", "paynet").expect("retail domain id");
    let retail_domain = Domain::new(retail_domain_id.clone()).build(&authority);
    let genesis_domain =
        Domain::new(DomainId::try_new("genesis", "universal").expect("genesis domain id"))
            .build(&authority);
    let authority_account = Account::new(authority.clone()).build(&authority);
    let merchant_account = Account::new(merchant_account_id.clone()).build(&authority);
    let replacement_account = Account::new(replacement_account_id.clone()).build(&authority);
    let payment_definition = AssetDefinition::numeric(
        payment_asset_definition_id.clone(),
        "xor".to_owned(),
        iroha_data_model::asset::AssetBalancePolicy::Global,
        None,
    )
    .build(&authority);
    let payment_asset = Asset::new(
        AssetId::of(payment_asset_definition_id, authority.clone()),
        Quantity::from(1_000_u32),
    );
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let mut world = World::with_assets(
        [base_domain, retail_domain, genesis_domain],
        [authority_account, merchant_account, replacement_account],
        [payment_definition],
        [payment_asset],
        [],
    );
    crate::sns::seed_default_namespace_policies(&mut world);
    let mut state = State::new_for_testing(world, kura, query);
    let (paynet, catalog) = retail_dataspace_catalog();
    state.set_dataspace_catalog_for_testing(catalog);
    let alias_literal = "merchant@bank.paynet";
    let alias = AccountAlias::new(
        "merchant".parse().expect("alias label"),
        Some(AccountAliasDomain::new(
            "bank".parse().expect("alias domain name"),
        )),
        paynet,
    );
    let header = BlockHeader::new(nonzero!(1_u64), None, None, 0, 0);
    let mut block = state.block(header);
    let mut tx = block.transaction();
    tx.nexus.dataspace_catalog = state.nexus.read().dataspace_catalog.clone();
    tx.world.dataspace_catalog = state.nexus.read().dataspace_catalog.clone();
    seed_test_call_hash(&mut tx, 0xF1);
    SeedTestAccountAliasLease::new(
        alias.clone(),
        merchant_account_id.clone(),
        authority.clone(),
        1,
        None,
    )
    .execute(&authority, &mut tx)
    .expect("acquire domain-qualified alias lease");
    tx.world_mut_for_testing().add_account_permission(
        &authority,
        Permission::from(CanResolveAccountAlias {
            scope: AccountAliasPermissionScope::Dataspace(paynet),
        }),
    );
    tx.world_mut_for_testing().add_account_permission(
        &authority,
        Permission::from(CanResolveAccountAlias {
            scope: AccountAliasPermissionScope::Domain(retail_domain_id.clone()),
        }),
    );
    tx.world_mut_for_testing().add_account_permission(
        &authority,
        Permission::from(CanManageAccountAlias {
            scope: AccountAliasPermissionScope::Dataspace(paynet),
        }),
    );
    tx.world_mut_for_testing().add_account_permission(
        &authority,
        Permission::from(CanManageAccountAlias {
            scope: AccountAliasPermissionScope::Domain(retail_domain_id.clone()),
        }),
    );
    EnsureTestAccountAliasBinding::bind(merchant_account_id.clone(), alias.clone(), None)
        .execute(&authority, &mut tx)
        .expect("bind domain-qualified alias");
    tx.apply();
    block
        .commit_world_overlay_for_testing()
        .expect("commit permissions");
    let first_view = state.view();
    let mut host = CoreHostImpl::new(authority.clone());
    host.set_query_state(&first_view);
    let mut vm = IVM::new(1_000_000);
    let alias_ptr = store_tlv(&mut vm, PointerType::Blob, alias_literal.as_bytes());
    vm.set_register(10, alias_ptr);
    host.syscall(ivm_sys::SYSCALL_RESOLVE_ACCOUNT_ALIAS, &mut vm)
        .expect("resolve domain-qualified alias syscall");
    let resolved: AccountId =
        CoreHost::decode_tlv_typed(&vm, vm.register(10), PointerType::AccountId)
            .expect("resolved account id");
    assert_eq!(resolved, merchant_account_id);
    let next_height = u64::try_from((state.view().height() + 1).max(2))
        .ok()
        .and_then(core::num::NonZeroU64::new)
        .expect("next block height");
    let mut block = state.block(BlockHeader::new(next_height, None, None, 0, 0));
    let mut tx = block.transaction();
    EnsureTestAccountAliasBinding::bind(replacement_account_id.clone(), alias, None)
        .execute(&authority, &mut tx)
        .expect("rebind domain-qualified alias");
    tx.apply();
    block
        .commit_world_overlay_for_testing()
        .expect("commit alias rebind");
    let rebound_view = state.view();
    host.set_query_state(&rebound_view);
    let alias_ptr = store_tlv(&mut vm, PointerType::Blob, alias_literal.as_bytes());
    vm.set_register(10, alias_ptr);
    host.syscall(ivm_sys::SYSCALL_RESOLVE_ACCOUNT_ALIAS, &mut vm)
        .expect("resolve rebound domain-qualified alias syscall");
    let rebound: AccountId =
        CoreHost::decode_tlv_typed(&vm, vm.register(10), PointerType::AccountId)
            .expect("rebound account id");
    assert_eq!(rebound, replacement_account_id);
}
#[test]
fn resolve_account_alias_syscall_requires_domain_permission_for_domain_qualified_alias() {
    let authority: AccountId = fixture_account("alice");
    let merchant_account_id: AccountId = fixture_account("bob");
    let payment_asset_definition_id: AssetDefinitionId = "6TEAJqbb8oEPmLncoNiMRbLEK6tw"
        .parse()
        .expect("payment asset definition id");
    let base_domain = Domain::new(fixture_domain_id()).build(&authority);
    let retail_domain_id = DomainId::try_new("bank", "paynet").expect("retail domain id");
    let retail_domain = Domain::new(retail_domain_id.clone()).build(&authority);
    let genesis_domain =
        Domain::new(DomainId::try_new("genesis", "universal").expect("genesis domain id"))
            .build(&authority);
    let authority_account = Account::new(authority.clone()).build(&authority);
    let merchant_account = Account::new(merchant_account_id.clone()).build(&authority);
    let payment_definition = AssetDefinition::numeric(
        payment_asset_definition_id.clone(),
        "xor".to_owned(),
        iroha_data_model::asset::AssetBalancePolicy::Global,
        None,
    )
    .build(&authority);
    let payment_asset = Asset::new(
        AssetId::of(payment_asset_definition_id.clone(), authority.clone()),
        Quantity::from(1_000_u32),
    );
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let mut world = World::with_assets(
        [base_domain, retail_domain, genesis_domain],
        [authority_account, merchant_account],
        [payment_definition],
        [payment_asset],
        [],
    );
    crate::sns::seed_default_namespace_policies_for_payment_asset(
        &mut world,
        &payment_asset_definition_id.to_string(),
    );
    let mut state = State::new_for_testing(world, kura, query);
    let (paynet, catalog) = retail_dataspace_catalog();
    state.set_dataspace_catalog_for_testing(catalog);
    let alias_literal = "merchant@bank.paynet";
    let alias = AccountAlias::new(
        "merchant".parse().expect("alias label"),
        Some(AccountAliasDomain::new(
            "bank".parse().expect("alias domain name"),
        )),
        paynet,
    );
    let header = BlockHeader::new(nonzero!(1_u64), None, None, 0, 0);
    let mut block = state.block(header);
    let mut tx = block.transaction();
    tx.nexus.dataspace_catalog = state.nexus.read().dataspace_catalog.clone();
    tx.world.dataspace_catalog = state.nexus.read().dataspace_catalog.clone();
    seed_test_call_hash(&mut tx, 0xF2);
    tx.world_mut_for_testing().add_account_permission(
        &authority,
        Permission::from(CanManageAccountAlias {
            scope: AccountAliasPermissionScope::Domain(retail_domain_id.clone()),
        }),
    );
    SeedTestAccountAliasLease::new(
        alias.clone(),
        merchant_account_id.clone(),
        authority.clone(),
        1,
        None,
    )
    .execute(&authority, &mut tx)
    .expect("acquire domain-qualified alias lease");
    tx.world_mut_for_testing().add_account_permission(
        &authority,
        Permission::from(CanResolveAccountAlias {
            scope: AccountAliasPermissionScope::Dataspace(paynet),
        }),
    );
    tx.world_mut_for_testing().add_account_permission(
        &authority,
        Permission::from(CanManageAccountAlias {
            scope: AccountAliasPermissionScope::Dataspace(paynet),
        }),
    );
    tx.world_mut_for_testing().add_account_permission(
        &authority,
        Permission::from(CanManageAccountAlias {
            scope: AccountAliasPermissionScope::Domain(retail_domain_id),
        }),
    );
    EnsureTestAccountAliasBinding::bind(merchant_account_id, alias, None)
        .execute(&authority, &mut tx)
        .expect("bind domain-qualified alias");
    tx.apply();
    block
        .commit_world_overlay_for_testing()
        .expect("commit alias setup");
    let view = state.view();
    let mut host = CoreHostImpl::new(authority);
    host.set_query_state(&view);
    let mut vm = IVM::new(1_000_000);
    let alias_ptr = store_tlv(&mut vm, PointerType::Blob, alias_literal.as_bytes());
    vm.set_register(10, alias_ptr);
    let err = host
        .syscall(ivm_sys::SYSCALL_RESOLVE_ACCOUNT_ALIAS, &mut vm)
        .expect_err("domain-qualified alias resolution should require domain permission");
    assert!(matches!(err, ivm::VMError::PermissionDenied));
}
#[test]
fn resolve_account_alias_syscall_accepts_domain_permission_without_dataspace_permission() {
    let authority: AccountId = fixture_account("alice");
    let merchant_account_id: AccountId = fixture_account("bob");
    let payment_asset_definition_id: AssetDefinitionId = "6TEAJqbb8oEPmLncoNiMRbLEK6tw"
        .parse()
        .expect("payment asset definition id");
    let base_domain = Domain::new(fixture_domain_id()).build(&authority);
    let retail_domain_id = DomainId::try_new("bank", "paynet").expect("retail domain id");
    let retail_domain = Domain::new(retail_domain_id.clone()).build(&authority);
    let genesis_domain =
        Domain::new(DomainId::try_new("genesis", "universal").expect("genesis domain id"))
            .build(&authority);
    let authority_account = Account::new(authority.clone()).build(&authority);
    let merchant_account = Account::new(merchant_account_id.clone()).build(&authority);
    let payment_definition = AssetDefinition::numeric(
        payment_asset_definition_id.clone(),
        "xor".to_owned(),
        iroha_data_model::asset::AssetBalancePolicy::Global,
        None,
    )
    .build(&authority);
    let payment_asset = Asset::new(
        AssetId::of(payment_asset_definition_id.clone(), authority.clone()),
        Quantity::from(1_000_u32),
    );
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let mut world = World::with_assets(
        [base_domain, retail_domain, genesis_domain],
        [authority_account, merchant_account],
        [payment_definition],
        [payment_asset],
        [],
    );
    crate::sns::seed_default_namespace_policies_for_payment_asset(
        &mut world,
        &payment_asset_definition_id.to_string(),
    );
    let mut state = State::new_for_testing(world, kura, query);
    let (paynet, catalog) = retail_dataspace_catalog();
    state.set_dataspace_catalog_for_testing(catalog);
    let alias_literal = "merchant@bank.paynet";
    let alias = AccountAlias::new(
        "merchant".parse().expect("alias label"),
        Some(AccountAliasDomain::new(
            "bank".parse().expect("alias domain name"),
        )),
        paynet,
    );
    let header = BlockHeader::new(nonzero!(1_u64), None, None, 0, 0);
    let mut block = state.block(header);
    let mut tx = block.transaction();
    tx.nexus.dataspace_catalog = state.nexus.read().dataspace_catalog.clone();
    tx.world.dataspace_catalog = state.nexus.read().dataspace_catalog.clone();
    seed_test_call_hash(&mut tx, 0xF3);
    tx.world_mut_for_testing().add_account_permission(
        &authority,
        Permission::from(CanManageAccountAlias {
            scope: AccountAliasPermissionScope::Domain(retail_domain_id.clone()),
        }),
    );
    SeedTestAccountAliasLease::new(
        alias.clone(),
        merchant_account_id.clone(),
        authority.clone(),
        1,
        None,
    )
    .execute(&authority, &mut tx)
    .expect("acquire domain-qualified alias lease");
    tx.world_mut_for_testing().add_account_permission(
        &authority,
        Permission::from(CanResolveAccountAlias {
            scope: AccountAliasPermissionScope::Domain(retail_domain_id.clone()),
        }),
    );
    tx.world_mut_for_testing().add_account_permission(
        &authority,
        Permission::from(CanManageAccountAlias {
            scope: AccountAliasPermissionScope::Dataspace(paynet),
        }),
    );
    tx.world_mut_for_testing().add_account_permission(
        &authority,
        Permission::from(CanManageAccountAlias {
            scope: AccountAliasPermissionScope::Domain(retail_domain_id),
        }),
    );
    EnsureTestAccountAliasBinding::bind(merchant_account_id.clone(), alias, None)
        .execute(&authority, &mut tx)
        .expect("bind domain-qualified alias");
    tx.apply();
    block
        .commit_world_overlay_for_testing()
        .expect("commit alias setup");
    let view = state.view();
    let mut host = CoreHostImpl::new(authority);
    host.set_query_state(&view);
    let mut vm = IVM::new(1_000_000);
    let alias_ptr = store_tlv(&mut vm, PointerType::Blob, alias_literal.as_bytes());
    vm.set_register(10, alias_ptr);
    host.syscall(ivm_sys::SYSCALL_RESOLVE_ACCOUNT_ALIAS, &mut vm)
        .expect("exact domain permission should resolve without dataspace permission");
    let resolved: AccountId =
        CoreHost::decode_tlv_typed(&vm, vm.register(10), PointerType::AccountId)
            .expect("resolved account id");
    assert_eq!(resolved, merchant_account_id);
}
#[test]
fn call_contract_syscall_returns_exact_schema_bound_typed_table() {
    let authority: AccountId = fixture_account("alice");
    let state = contract_test_state(&authority);
    let caller_contract = install_contract(
        &state,
        &authority,
        r"
seiyaku Caller {
  kotoage fn main() authorize(anyone) -> int { return 0; }
}
",
        0,
    );
    let nested_contract = install_contract(
        &state,
        &authority,
        r"
seiyaku Callee {
  view fn value() authorize(anyone) -> int {
    return 42;
  }
}
",
        1,
    );
    let (result, vm, durable_state_overlay) = call_contract_syscall(
        &state,
        &authority,
        &caller_contract,
        &nested_contract,
        "value",
        &(Json::new(())),
    );
    let gas = result.expect("call_contract syscall should succeed");
    assert!(gas > 0);
    assert!(durable_state_overlay.is_empty());
    let schema =
        exact_return_type(iroha_data_model::smart_contract::entrypoint::EntrypointValueKindV1::Int);
    let record = captured_nested_result(&vm, &schema);
    let schema_bytes = norito::to_bytes(&schema).expect("encode return schema");
    assert_eq!(
        record.schema_hash,
        iroha_data_model::smart_contract::entrypoint::entrypoint_return_schema_hash_v1(
            &schema_bytes
        )
    );
    let [iroha_data_model::smart_contract::entrypoint::EntrypointValueAtomV1::Pointer(envelope)] =
        record.atoms.as_slice()
    else {
        panic!("exact Int return must contain one canonical pointer atom");
    };
    let expected_envelope = ivm_abi::numeric_tlv::encode_int(&BigInt::from_i128(42))
        .expect("encode canonical V1 int atom");
    assert_eq!(
        envelope, &expected_envelope,
        "nested Int returns must preserve the exact canonical pointer envelope"
    );
    assert_eq!(
        ivm::numeric_tlv::decode_int_bytes(envelope).expect("decode returned Int"),
        iroha_primitives::bigint::BigInt::from_i128(42),
    );
}
#[test]
fn generic_contract_call_returns_exact_quantity() {
    let authority = fixture_account("alice");
    let state = contract_test_state(&authority);
    let caller = install_contract(
        &state,
        &authority,
        "seiyaku Caller { kotoage fn main() authorize(anyone) {} }",
        0,
    );
    let target_contract = install_contract(
        &state,
        &authority,
        "seiyaku Callee { view fn quote(quantity amount_in, quantity min_out) authorize(anyone) -> quantity { amount_in } }",
        1,
    );
    let (result, vm, effects) = call_contract_syscall(
        &state,
        &authority,
        &caller,
        &target_contract,
        "quote",
        &(Json::from(norito::json!({"amount_in":"10", "min_out":"7"}))),
    );
    assert!(result.unwrap() > 0);
    assert!(effects.is_empty());
    let pointer = vm.load_u64(vm.register(14)).unwrap();
    assert_eq!(
        vm.validate_tlv(pointer).unwrap().type_id,
        PointerType::Quantity
    );
    assert_eq!(decode_quantity_leaf(&vm, pointer), Quantity::from(10u32));
    assert_eq!(
        vm.validate_tlv(vm.register(10)).unwrap().payload,
        target_contract.as_ref().as_bytes(),
        "typed calls preserve the target register"
    );
}
#[test]
fn generic_contract_call_rejects_wrong_argument_type_without_output() {
    let authority = fixture_account("alice");
    let state = contract_test_state(&authority);
    let caller = install_contract(
        &state,
        &authority,
        "seiyaku Caller { kotoage fn main() authorize(anyone) {} }",
        0,
    );
    let target_contract = install_contract(
        &state,
        &authority,
        "seiyaku Callee { view fn quote(quantity amount_in, quantity min_out) authorize(anyone) -> quantity { amount_in } }",
        1,
    );
    let view = state.view();
    let mut host = CoreHostImpl::new(authority.clone());
    host.set_query_state(&view);
    let mut vm = IVM::new(1_000_000);
    bind_nested_caller(&mut host, &mut vm, &view, &authority, &caller);
    let registers = store_typed_contract_call(
        &mut vm,
        &state,
        &target_contract,
        "quote",
        &Json::from(norito::json!({"amount_in":"10","min_out":"7"})),
    );
    let wrong = vm
        .alloc_host_tlv(&ivm_abi::numeric_tlv::encode_int(&BigInt::from_i128(10)).unwrap())
        .unwrap();
    vm.store_u64(registers[2], wrong).unwrap();
    assert!(
        host.syscall(ivm_sys::SYSCALL_CALL_CONTRACT, &mut vm)
            .is_err()
    );
    assert!(host.durable_state_overlay.is_empty());
    assert!(host.queued.is_empty());
    assert_eq!(
        (10..16).map(|index| vm.register(index)).collect::<Vec<_>>(),
        registers
    );
    assert!(matches!(vm.load_u64(registers[4]), Ok(0)));
}
#[test]
fn generic_contract_call_rejects_stale_return_schema_binding_before_state_changes() {
    let authority = fixture_account("alice");
    let state = contract_test_state(&authority);
    let caller = install_contract(
        &state,
        &authority,
        "seiyaku Caller { kotoage fn main() authorize(anyone) {} }",
        0,
    );
    let target_contract = install_contract(
        &state,
        &authority,
        "seiyaku Callee { state int counter; hajimari() {counter=0;} kotoage fn quote(quantity amount_in, quantity min_out) authorize(anyone) -> int { counter=9; return 1; } }",
        1,
    );
    let view = state.view();
    let mut host = CoreHostImpl::new(authority.clone());
    host.set_query_state(&view);
    host.set_durable_state_snapshot_from_world(view.world());
    let mut vm = IVM::new(1_000_000);
    bind_nested_caller(&mut host, &mut vm, &view, &authority, &caller);
    let registers = store_typed_contract_call(
        &mut vm,
        &state,
        &target_contract,
        "quote",
        &Json::from(norito::json!({"amount_in":"10","min_out":"7"})),
    );
    let mut binding = ivm_abi::contract_call::ContractCallBindingV1::from_bytes(
        vm.validate_tlv(registers[1]).unwrap().payload,
    )
    .unwrap();
    binding.code_hash = Hash::new(b"different authenticated Quantity result artifact");
    let changed = store_tlv(
        &mut vm,
        PointerType::NoritoBytes,
        &binding.to_bytes().unwrap(),
    );
    vm.set_register(11, changed);
    assert!(matches!(
        host.syscall(ivm_sys::SYSCALL_CALL_CONTRACT, &mut vm)
            .unwrap_err()
            .as_unmetered(),
        ivm::VMError::PermissionDenied
    ));
    assert!(host.durable_state_overlay.is_empty());
    assert!(host.queued.is_empty());
    assert_eq!(vm.register(10), registers[0]);
    assert!(matches!(vm.load_u64(registers[4]), Ok(0)));
}
#[test]
fn typed_contract_call_rejects_forged_caller_scope_before_authorization() {
    let authority = fixture_account("alice");
    let state = contract_test_state(&authority);
    let caller = install_contract(
        &state,
        &authority,
        "seiyaku Caller { kotoage fn main() authorize(anyone) {} }",
        0,
    );
    let target_contract = install_contract(
        &state,
        &authority,
        "seiyaku Child { view fn read() authorize(anyone) -> bool {true} }",
        1,
    );
    for case in 0..4 {
        let view = state.view();
        let mut host = CoreHostImpl::new(authority.clone());
        host.set_query_state(&view);
        let mut vm = IVM::new(1_000_000);
        bind_nested_caller(&mut host, &mut vm, &view, &authority, &caller);
        let registers =
            store_typed_contract_call(&mut vm, &state, &target_contract, "read", &Json::new(()));
        match case {
            0 => {
                host.current_contract_runtime_context
                    .as_mut()
                    .unwrap()
                    .contract_subject = authority.clone()
            }
            1 => {
                host.current_contract_runtime_context
                    .as_mut()
                    .unwrap()
                    .entrypoint = "forged".into()
            }
            2 => host.authority = fixture_account("bob"),
            3 => {
                host.current_entrypoint_authorization
                    .as_mut()
                    .unwrap()
                    .code_hash = Hash::new(b"forged caller artifact")
            }
            _ => unreachable!(),
        }
        assert!(
            matches!(
                host.syscall(ivm_sys::SYSCALL_CALL_CONTRACT, &mut vm)
                    .unwrap_err()
                    .as_unmetered(),
                ivm::VMError::PermissionDenied
            ),
            "case {case}"
        );
        assert!(matches!(vm.load_u64(registers[4]), Ok(0)));
        assert!(host.queued.is_empty());
        assert!(host.durable_state_overlay.is_empty());
    }
}

#[test]
fn typed_contract_call_rejects_private_witness_callee_before_execution() {
    let authority = fixture_account("alice");
    let state = contract_test_state(&authority);
    let caller = install_contract(
        &state,
        &authority,
        "seiyaku Caller { kotoage fn main() authorize(anyone) {} }",
        0,
    );
    let target_contract = install_contract(
        &state,
        &authority,
        "seiyaku PrivateCallee { kotoage fn main() authorize(anyone) {} }",
        1,
    );
    let syscall = ivm::encoding::wide::encode_sys(
        ivm::instruction::wide::system::SCALL,
        u8::try_from(ivm_sys::SYSCALL_GET_PRIVATE_INPUT).expect("fixture value fits u8"),
    );
    let program = build_authenticated_test_contract_program(&syscall.to_le_bytes(), 0, true);
    assert_eq!(
        ivm::prepare_contract(Arc::<[u8]>::from(program.clone()))
            .unwrap()
            .entrypoint_requires_private_inputs("main"),
        Some(true)
    );
    rebind_test_contract_program(&state, &authority, &target_contract, program);
    let (result, vm, effects) = call_contract_syscall(
        &state,
        &authority,
        &caller,
        &target_contract,
        "main",
        &(Json::new(())),
    );
    assert!(matches!(
        result.unwrap_err().as_unmetered(),
        ivm::VMError::PermissionDenied
    ));
    assert!(effects.is_empty());
    assert!(matches!(vm.load_u64(vm.register(14)), Ok(0)));
}

#[test]
fn typed_contract_call_preserves_view_restriction_before_child_effects() {
    let authority = fixture_account("alice");
    let state = contract_test_state(&authority);
    let caller = install_contract(
        &state,
        &authority,
        "seiyaku Caller { view fn main() authorize(anyone) {} }",
        0,
    );
    let target_contract = install_contract(
        &state,
        &authority,
        "seiyaku Callee { state StateMap<int,int> Values; view fn read() authorize(anyone) -> bool { true } kotoage fn write() authorize(anyone) { Values[1] = 9; } }",
        1,
    );
    let (read, vm, effects) = call_contract_syscall(
        &state,
        &authority,
        &caller,
        &target_contract,
        "read",
        &(Json::new(())),
    );
    read.expect("a view may call another authenticated view");
    assert_eq!(
        render_nested_result(
            &vm,
            iroha_data_model::smart_contract::entrypoint::EntrypointValueKindV1::Bool
        ),
        norito::json!(true)
    );
    assert!(effects.is_empty());
    let (result, vm, effects) = call_contract_syscall(
        &state,
        &authority,
        &caller,
        &target_contract,
        "write",
        &(Json::new(())),
    );
    assert!(matches!(
        result.unwrap_err().as_unmetered(),
        ivm::VMError::PermissionDenied
    ));
    assert!(effects.is_empty());
    assert!(matches!(vm.load_u64(vm.register(14)), Ok(0)));
}

#[test]
fn canonical_quantity_return_record_rejects_wrong_schema_atom_and_pointer_type() {
    use iroha_data_model::smart_contract::entrypoint::{
        EntrypointReturnRecordV1, entrypoint_return_schema_hash_v1,
    };
    let quantity_schema = exact_return_type(
        iroha_data_model::smart_contract::entrypoint::EntrypointValueKindV1::Quantity,
    );
    let quantity_schema_hash =
        entrypoint_return_schema_hash_v1(&norito::encode_canonical(&quantity_schema).unwrap());
    let int_schema =
        exact_return_type(iroha_data_model::smart_contract::entrypoint::EntrypointValueKindV1::Int);
    let int_schema_hash =
        entrypoint_return_schema_hash_v1(&norito::encode_canonical(&int_schema).unwrap());
    let int_envelope = ivm_abi::numeric_tlv::encode_int(&BigInt::from_i128(1)).unwrap();
    for (label, record) in [
        (
            "wrong schema",
            EntrypointReturnRecordV1 {
                schema_hash: int_schema_hash,
                atoms: vec![EntrypointValueAtomV1::Pointer(int_envelope.clone())],
            },
        ),
        (
            "wrong atom",
            EntrypointReturnRecordV1 {
                schema_hash: quantity_schema_hash,
                atoms: vec![EntrypointValueAtomV1::Bool(true)],
            },
        ),
        (
            "wrong pointer type",
            EntrypointReturnRecordV1 {
                schema_hash: quantity_schema_hash,
                atoms: vec![EntrypointValueAtomV1::Pointer(int_envelope)],
            },
        ),
    ] {
        assert!(
            ivm::value_record::decode_entrypoint_return_record(
                &quantity_schema,
                &norito::encode_canonical(&record).unwrap()
            )
            .is_err(),
            "{label} must fail closed"
        );
    }
}
fn completed_unit_return_vm() -> IVM {
    let program = kotodama_lang::compiler::Compiler::new()
        .compile_source("seiyaku ReturnBoundary { view fn main() authorize(anyone) { () } }")
        .expect("compile root result-table fixture");
    let metadata = ivm::ProgramMetadata::parse(&program).unwrap();
    let entry = metadata
        .contract_interface
        .as_ref()
        .unwrap()
        .entrypoints
        .iter()
        .find(|entry| entry.name == "main")
        .unwrap();
    let entry_pc = metadata.prefix_len() as u64 + entry.entry_pc;
    let mut vm = IVM::new(100_000);
    vm.load_program(&program)
        .expect("load root result-table fixture");
    vm.set_program_counter(entry_pc)
        .expect("select authenticated root entrypoint");
    vm.run().expect("complete authenticated root call");
    assert_eq!(vm.call_result_word_count().unwrap(), 1);
    vm
}
#[test]
fn nested_contract_return_rejects_private_pointer_memory_with_either_descriptor_tag() {
    let mut vm = completed_unit_return_vm();
    let table = vm.register(10);
    vm.set_zk_mode(true)
        .expect("private lifecycle cleanup succeeds");
    let record =
        ivm::private_input::int_record(42_u64.into()).expect("encode authenticated private input");
    let kind = record.kind.tag();
    let mut host = ivm::host::DefaultHost::with_private_inputs(vec![record]).unwrap();
    vm.set_register(10, 0);
    vm.set_register(11, kind);
    host.syscall(ivm_sys::SYSCALL_GET_PRIVATE_INPUT, &mut vm)
        .unwrap();
    let pointer = vm.register(10);
    vm.store_u64(table, pointer)
        .expect("trusted host writes return fixture");
    let schema =
        exact_return_type(iroha_data_model::smart_contract::entrypoint::EntrypointValueKindV1::Int);
    for private_descriptor in [true, false] {
        vm.registers.set_tag(10, private_descriptor);
        let error = ivm::value_record::quote_completed_return_record(&vm, &schema, u64::MAX)
            .expect_err("private payload must not be returned");
        assert!(
            matches!(
                (&(error.as_unmetered()), &(&ivm::VMError::PrivacyViolation)),
                (
                    ivm::VMError::PrivacyViolation,
                    ivm::VMError::PrivacyViolation
                )
            ),
            "private payload rejection must depend on completed table memory, not register tags"
        );
        assert!(error.metered_gas().is_some_and(|gas| gas > 0));
    }
}
#[test]
fn nested_contract_return_rejects_repeated_pointer_amplification() {
    use iroha_data_model::smart_contract::entrypoint::{
        EntrypointListTypeNodeV1, EntrypointValueKindV1, EntrypointValueTypeNodeV1,
        EntrypointValueTypeV1, MAX_ENTRYPOINT_RETURN_RECORD_BYTES,
    };
    let payload = vec![0xA5; MAX_ENTRYPOINT_RETURN_RECORD_BYTES / 2 + 1024];
    let envelope = make_tlv(PointerType::Blob as u16, &payload);
    let mut vm = completed_unit_return_vm();
    let pointer = vm
        .alloc_heap(u64::try_from(envelope.len()).expect("TLV length fits u64"))
        .expect("allocate one large public TLV");
    vm.store_bytes(pointer, &envelope)
        .expect("store one large public TLV");
    let list_layout = ivm::list::ListLayoutV1::try_new(2, 1).expect("list layout");
    let list = ivm::list::allocate_words(&mut vm, list_layout, &[vec![pointer], vec![pointer]])
        .expect("repeat one pointer in a list");
    vm.store_u64(vm.register(10), list)
        .expect("write completed result table");
    let schema = EntrypointValueTypeV1 {
        nodes: vec![
            EntrypointValueTypeNodeV1::List(EntrypointListTypeNodeV1 { capacity: 2 }),
            EntrypointValueTypeNodeV1::Leaf(EntrypointValueKindV1::Blob),
        ],
    };
    let error = ivm::value_record::quote_completed_return_record(&vm, &schema, u64::MAX)
        .expect_err("repeated pointers exceed the canonical return record bound");
    assert!(matches!(
        (&(error.as_unmetered()), &(&ivm::VMError::DecodeError)),
        (ivm::VMError::DecodeError, ivm::VMError::DecodeError)
    ));
    assert!(error.metered_gas().is_some_and(|gas| gas > 0));
}
#[test]
fn nested_contract_return_rejects_unaffordable_payload_before_clone() {
    use iroha_data_model::smart_contract::entrypoint::{
        EntrypointValueKindV1, EntrypointValueTypeNodeV1, EntrypointValueTypeV1,
    };
    let payload = vec![0xA5; 64 * 1024];
    let envelope = make_tlv(PointerType::Blob as u16, &payload);
    let mut vm = completed_unit_return_vm();
    let pointer = vm
        .alloc_heap(u64::try_from(envelope.len()).expect("TLV length fits u64"))
        .expect("allocate child return TLV");
    vm.store_bytes(pointer, &envelope)
        .expect("store child return TLV");
    vm.store_u64(vm.register(10), pointer)
        .expect("write completed result table");
    let schema = EntrypointValueTypeV1 {
        nodes: vec![EntrypointValueTypeNodeV1::Leaf(EntrypointValueKindV1::Blob)],
    };
    let error = ivm::value_record::quote_completed_return_record(&vm, &schema, 1024)
        .expect_err("unaffordable return is rejected before cloning");
    assert!(matches!(
        (&(error.as_unmetered()), &(&ivm::VMError::OutOfGas)),
        (ivm::VMError::OutOfGas, ivm::VMError::OutOfGas)
    ));
    assert_eq!(error.metered_gas(), Some(1024));
}
#[test]
fn repeated_nested_calls_reuse_prepared_artifact_and_warmed_runtime() {
    // Other tests deliberately change the process-wide retention limits.
    // The exact child runs every original warmth, reset and gas assertion.
    if crate::unit_test_support::run_in_isolated_harness(
        "smartcontracts::ivm::host::tests::repeated_nested_calls_reuse_prepared_artifact_and_warmed_runtime",
    ) {
        return;
    }
    let authority: AccountId = fixture_account("alice");
    let state = contract_test_state(&authority);
    let caller_contract = install_contract(
        &state,
        &authority,
        r"
seiyaku Caller {
  kotoage fn main() authorize(anyone) -> int { return 0; }
}
",
        0,
    );
    let nested_contract = install_contract(
        &state,
        &authority,
        r"
seiyaku Callee {
  view fn value() authorize(anyone) -> int { return 42; }
}
",
        1,
    );
    let prepared_cache = PreparedContractCache::with_capacity(2);
    let (first, _, _) = call_contract_syscall_with_prepared_cache(
        &state,
        &authority,
        &caller_contract,
        &nested_contract,
        "value",
        &(Json::new(())),
        prepared_cache.clone(),
    );
    let first_gas = first.expect("first nested call");
    let after_first = prepared_cache.stats();
    assert_eq!(after_first.misses, 1);
    assert_eq!(after_first.preparations, 1);
    assert_eq!(after_first.runtime_misses, 1);
    assert_eq!(after_first.runtime_hits, 0);
    assert_eq!(after_first.runtime_prepared_loads, 1);
    assert_eq!(after_first.runtime_template_builds, 1);
    assert_eq!(after_first.runtime_dirty_resets, 1);
    // A genuinely warm nested dispatch resolves from the immutable
    // prepared artifact and embedded interface. The world lookup checks
    // only that the binding and artifacts remain live; it does not clone
    // the deployed byte vector unless this cache lookup misses.
    let (second, _, _) = call_contract_syscall_with_prepared_cache(
        &state,
        &authority,
        &caller_contract,
        &nested_contract,
        "value",
        &(Json::new(())),
        prepared_cache.clone(),
    );
    let second_gas = second.expect("second nested call");
    assert_eq!(
        second_gas, first_gas,
        "prepared-cache warmth must not change consensus gas"
    );
    let after_second = prepared_cache.stats();
    assert_eq!(after_second.hits, after_first.hits + 1);
    assert_eq!(after_second.misses, after_first.misses);
    assert_eq!(after_second.preparations, after_first.preparations);
    assert_eq!(after_second.runtime_hits, after_first.runtime_hits + 1);
    assert_eq!(after_second.runtime_misses, after_first.runtime_misses);
    assert_eq!(
        after_second.runtime_prepared_loads,
        after_first.runtime_prepared_loads
    );
    assert_eq!(
        after_second.runtime_template_builds,
        after_first.runtime_template_builds
    );
    assert_eq!(
        after_second.runtime_dirty_resets,
        after_first.runtime_dirty_resets + 1
    );
}
#[test]
fn live_contract_view_never_falls_back_to_a_stale_binding_snapshot() {
    let authority: AccountId = fixture_account("alice");
    let populated_state = contract_test_state(&authority);
    let contract = install_contract(
        &populated_state,
        &authority,
        r"
seiyaku Callee {
  view fn value() authorize(anyone) -> int { return 42; }
}
",
        7,
    );
    let stale_record = {
        let view = populated_state.view();
        crate::smartcontracts::code::fetch_bound_contract_record(&view, &contract)
            .expect("registry read completes")
            .expect("installed contract record")
    };
    let empty_state = contract_test_state(&authority);
    let empty_view = empty_state.view();
    let mut host = CoreHostImpl::new(authority);
    host.set_bound_contract_records_by_subject_snapshot(BTreeMap::from([(
        stale_record.contract_subject.clone(),
        stale_record,
    )]));
    host.set_query_state(&empty_view);
    assert!(
        host.resolve_bound_contract_dispatch_identity_by_address(&contract)
            .expect("original live scope read completes")
            .is_none(),
        "an authoritative live view must fail closed instead of reviving a stale snapshot"
    );
}
include!("host/nested_contract_state_and_rollback_tests.rs");
include!("host/attachment_lifecycle_tests.rs");
include!("host/x509_attachment_tests.rs");
include!("host/nested_return_funding_tests.rs");
include!("host/shared_vm_cycle_budget_tests.rs");
#[test]
fn dispatched_call_contract_spends_reserved_gas_and_returns_output() {
    let authority: AccountId = fixture_account("alice");
    let state = contract_test_state(&authority);
    let caller_contract = install_contract(
        &state,
        &authority,
        r"
seiyaku Caller {
  kotoage fn main() authorize(anyone) -> int { return 0; }
}
",
        0,
    );
    let nested_contract = install_contract(
        &state,
        &authority,
        r"
seiyaku Callee {
  view fn value() authorize(anyone) -> int { return 42; }
}
",
        1,
    );
    let payload = Json::new(());
    let request_bytes = typed_contract_request_bytes(&state, &nested_contract, "value");
    let gas_limit = 1_000_000;
    let (result, vm, durable_state_overlay, _) = dispatch_call_contract_syscall(
        &state,
        &authority,
        &caller_contract,
        &nested_contract,
        "value",
        &(payload),
        gas_limit,
    );
    result.expect("dispatched nested call should succeed");
    assert!(durable_state_overlay.is_empty());
    let value = render_nested_result(
        &vm,
        iroha_data_model::smart_contract::entrypoint::EntrypointValueKindV1::Int,
    );
    assert_eq!(value, norito::json!("42"));
    let host_overhead = ivm::gas::syscall_byte_gas(ivm::gas::G_CALL_CONTRACT, request_bytes, 0);
    assert!(
        gas_limit.saturating_sub(vm.remaining_gas()) > host_overhead,
        "outer opcode and child execution gas must be charged in addition to host overhead",
    );
}
#[test]
fn dispatched_call_contract_unaffordable_child_leaks_no_output_or_state() {
    let authority: AccountId = fixture_account("alice");
    let state = contract_test_state(&authority);
    let caller_contract = install_contract(
        &state,
        &authority,
        r"
    seiyaku Caller {
      kotoage fn main() authorize(anyone) -> int { return 0; }
    }
    ",
        0,
    );
    let nested_contract = install_contract(
        &state,
        &authority,
        r"
    seiyaku Callee { permission AssetOps;
      state int counter;

      hajimari() { counter = 0; }

      kotoage fn write() authorize(AssetOps) -> int {
        counter = 9;
        return counter;
      }
    }
    ",
        1,
    );
    grant_asset_ops_to_account(&state, &authority, &(caller_contract.subject_id()));
    let payload = Json::new(());
    let request_gas = ivm::gas::syscall_byte_gas(
        ivm::gas::G_CALL_CONTRACT,
        typed_contract_request_bytes(&state, &nested_contract, "write"),
        0,
    );
    // After authenticated staging and request validation, only one gas remains
    // for artifact preparation; the child cannot execute or publish effects.
    let gas_limit = typed_dispatch_setup_gas()
        .saturating_add(request_gas)
        .saturating_add(1);
    let (result, vm, durable_state_overlay, target_ptr) = dispatch_call_contract_syscall(
        &state,
        &authority,
        &caller_contract,
        &nested_contract,
        "write",
        &(payload),
        gas_limit,
    );
    assert!(matches!(result, Err(ivm::VMError::OutOfGas)));
    assert!(
        durable_state_overlay.is_empty(),
        "failed child state must roll back"
    );
    assert_eq!(
        vm.register(10),
        target_ptr,
        "failed call must not publish output"
    );
    assert!(
        vm.remaining_gas() <= 1,
        "only unspent child escrow may be refunded",
    );
}
#[test]
fn dispatched_call_contract_child_failure_rolls_back_and_charges_child_gas() {
    let authority: AccountId = fixture_account("alice");
    let state = contract_test_state(&authority);
    let caller_contract = install_contract(
        &state,
        &authority,
        r#"
seiyaku Caller {
  error enum CalleeError { #[message("Caller explanation")] ForcedFailure = 1 }
  kotoage fn main() authorize(anyone) -> int { return 0; }
}
"#,
        0,
    );
    let callee_source = r#"
seiyaku Callee { permission AssetOps;
  error enum CalleeError {
    #[message("Callee rejected the operation")]
    ForcedFailure = 1,
  }

  state int counter;

  hajimari() { counter = 0; }

  kotoage fn fail_after_write() authorize(AssetOps) -> int {
    counter = 9;
    require(false, CalleeError::ForcedFailure);
    return 0;
  }
}
"#;
    let (_, manifest) = kotodama_lang::compiler::Compiler::new()
        .compile_source_with_manifest(callee_source)
        .expect("compile exact callee error descriptor");
    let expected = manifest
        .error_types
        .unwrap()
        .into_iter()
        .find(|error| error.identity.ends_with("::CalleeError"))
        .expect("callee nominal error");
    let nested_contract = install_contract(&state, &authority, callee_source, 1);
    grant_asset_ops_to_account(&state, &authority, &(caller_contract.subject_id()));
    let payload = Json::new(());
    let request_gas = ivm::gas::syscall_byte_gas(
        ivm::gas::G_CALL_CONTRACT,
        typed_contract_request_bytes(&state, &nested_contract, "fail_after_write"),
        0,
    );
    let gas_limit = 1_000_000;
    let (result, vm, durable_state_overlay, target_ptr) = dispatch_call_contract_syscall(
        &state,
        &authority,
        &caller_contract,
        &nested_contract,
        "fail_after_write",
        &(payload),
        gas_limit,
    );
    let error = result.expect_err("nested assertion should fail the syscall");
    assert!(
        matches!((&(error.as_unmetered()), &(&ivm::VMError::ContractAbort {
                contract: "Callee".into(),
                error_type: expected.identity.clone(),
                schema_hash: expected.schema_hash(),
                name: "ForcedFailure".to_owned(),
                code: 1,
                message: Some("Callee rejected the operation".into()),
            })), (ivm::VMError::ContractAbort { contract: __actual_contract, error_type: __actual_error_type, schema_hash: __actual_schema_hash, name: __actual_name, code: __actual_code, message: __actual_message }, ivm::VMError::ContractAbort { contract: __expected_contract, error_type: __expected_error_type, schema_hash: __expected_schema_hash, name: __expected_name, code: __expected_code, message: __expected_message }) if __actual_contract == __expected_contract && __actual_error_type == __expected_error_type && __actual_schema_hash == __expected_schema_hash && __actual_name == __expected_name && __actual_code == __expected_code && __actual_message == __expected_message)
    );
    assert_eq!(
        crate::execution_attempt::expect_completed_rejection(
            crate::smartcontracts::ivm::map_vm_error_with_context_to_validation(&vm, error),
        ),
        iroha_data_model::ValidationFail::ContractRejected(
            iroha_data_model::executor::ContractRejection {
                contract: "Callee".into(),
                error_type: expected.identity.clone(),
                schema_hash: expected.schema_hash(),
                name: "ForcedFailure".to_owned(),
                code: 1,
                message: Some("Callee rejected the operation".into()),
            }
        ),
        "the caller's same-named variant must never replace the callee identity",
    );
    assert!(
        durable_state_overlay.is_empty(),
        "failed child state must roll back"
    );
    assert_eq!(
        vm.register(10),
        target_ptr,
        "failed call must not publish output"
    );
    assert!(
        gas_limit.saturating_sub(vm.remaining_gas()) > request_gas,
        "failed child execution gas must remain charged",
    );
}
#[test]
fn call_contract_syscall_enforces_callee_entrypoint_authorization() {
    let authority: AccountId = fixture_account("alice");
    let state = contract_test_state(&authority);
    let caller_contract = install_contract(
        &state,
        &authority,
        r"
seiyaku Caller {
  kotoage fn main() authorize(anyone) -> int { return 0; }
}
",
        0,
    );
    let nested_contract = install_contract(
        &state,
        &authority,
        r"
seiyaku Callee { permission AssetOps;
  view fn value() authorize(AssetOps) -> int {
    return 42;
  }
}
",
        1,
    );
    grant_named_permission_to_account(
        &state,
        &authority,
        &(caller_contract.subject_id()),
        "OtherOps",
    );
    let (denied, _, _) = call_contract_syscall(
        &state,
        &authority,
        &caller_contract,
        &nested_contract,
        "value",
        &(Json::new(())),
    );
    let err = denied.expect_err("protected nested entrypoint should require AssetOps");
    assert!(matches!(err.as_unmetered(), ivm::VMError::PermissionDenied));
    grant_asset_ops_to_account(&state, &authority, &(caller_contract.subject_id()));
    let (result, vm, durable_state_overlay) = call_contract_syscall(
        &state,
        &authority,
        &caller_contract,
        &nested_contract,
        "value",
        &(Json::new(())),
    );
    result.expect("protected nested entrypoint should run after grant");
    assert!(durable_state_overlay.is_empty());
    let value = render_nested_result(
        &vm,
        iroha_data_model::smart_contract::entrypoint::EntrypointValueKindV1::Int,
    );
    assert_eq!(value, norito::json!("42"));
}
#[test]
fn call_contract_syscall_rejects_runtime_managed_lifecycle_entrypoints() {
    let authority: AccountId = fixture_account("alice");
    let state = contract_test_state(&authority);
    let caller_contract = install_contract(
        &state,
        &authority,
        r"
seiyaku Caller {
  kotoage fn main() authorize(anyone) -> int { return 0; }
}
",
        0,
    );
    let nested_contract = install_contract(
        &state,
        &authority,
        r"
seiyaku Lifecycle {
  state int initialized;
  hajimari() { initialized = 1; }
  kaizen() {}
}
",
        1,
    );
    for selector in ["hajimari", "kaizen"] {
        let (result, _, durable_state_overlay) = call_contract_syscall(
            &state,
            &authority,
            &caller_contract,
            &nested_contract,
            selector,
            &(Json::new(())),
        );
        let error = result.expect_err("nested calls must not invoke lifecycle hooks");
        assert!(matches!(
            error.as_unmetered(),
            ivm::VMError::PermissionDenied
        ));
        assert!(durable_state_overlay.is_empty());
    }
}
#[test]
fn call_contract_syscall_rejects_other_entrypoints_while_hajimari_is_pending() {
    let authority: AccountId = fixture_account("alice");
    let state = contract_test_state(&authority);
    let caller_contract = install_contract(
        &state,
        &authority,
        r"
seiyaku Caller {
  kotoage fn main() authorize(anyone) -> int { return 0; }
}
",
        0,
    );
    let nested_contract = install_contract_with_pending_lifecycle(
        &state,
        &authority,
        r"
seiyaku AwaitingHajimari {
  hajimari() {}

  view fn value() authorize(anyone) -> int {
    return 42;
  }
}
",
        1,
    );
    let (result, _, durable_state_overlay) = call_contract_syscall(
        &state,
        &authority,
        &caller_contract,
        &nested_contract,
        "value",
        &(Json::new(())),
    );
    let error = result.expect_err("nested calls must wait for hajimari/始まり");
    assert!(matches!(
        error.as_unmetered(),
        ivm::VMError::PermissionDenied
    ));
    assert!(durable_state_overlay.is_empty());
}
#[test]
fn call_contract_syscall_rejects_held_callee_before_argument_decode() {
    let authority: AccountId = fixture_account("alice");
    let state = contract_test_state(&authority);
    let caller_contract = install_contract(
        &state,
        &authority,
        r"
seiyaku Caller {
  kotoage fn main() authorize(anyone) -> int { return 0; }
}
",
        0,
    );
    let nested_contract = install_contract(
        &state,
        &authority,
        r"
seiyaku HeldCallee {
  view fn value(int input) authorize(anyone) -> int { return input; }
}
",
        1,
    );
    // `contract_test_state` has already applied its original signed genesis.
    // A fabricated height-one carrier cannot replace that authenticated tip.
    assert_eq!(state.committed_height(), 1);
    let original_parent = state
        .view()
        .latest_block_hash()
        .expect("original signed host genesis");
    assert!(crate::sumeragi::lanes::routing::committed_root_scope(state.view().world()).is_some());
    let creation_time_ms = state
        .latest_block_creation_time_ms_fast()
        .expect("original signed genesis timestamp")
        .checked_add(1)
        .expect("next component timestamp");
    let mut block = state.block(BlockHeader::new(
        nonzero!(2_u64),
        Some(original_parent),
        None,
        creation_time_ms,
        0,
    ));
    {
        let mut tx = block.transaction();
        let binding = tx
            .world
            .contract_subject_bindings
            .get_mut(&nested_contract)
            .expect("callee lifecycle binding");
        binding.lifecycle.emergency_hold =
            Some(iroha_data_model::smart_contract::ContractEmergencyHoldV1 {
                incident_digest: [0xC1; 32],
                proposal_content_id: [0xC2; 32],
                governance_attempt_id: [0xC3; 32],
                reason: "contain nested callee execution".to_owned(),
                imposed_at_height: 1,
                expires_at_height: 2,
            });
        binding.lifecycle.revision = binding
            .lifecycle
            .revision
            .checked_add(1)
            .expect("test lifecycle revision advances");
        tx.apply();
    }
    block
        .commit_world_overlay_for_testing()
        .expect("commit the emergency hold without advancing height");
    let view = state.view();
    let mut host = CoreHostImpl::new(authority.clone());
    host.set_query_state(&view);
    host.set_durable_state_snapshot_from_world(view.world());
    let mut vm = IVM::new(1_000_000);
    bind_nested_caller(&mut host, &mut vm, &view, &authority, &caller_contract);
    ivm::reset_argument_record_decode_count();
    let registers = store_typed_contract_call(
        &mut vm,
        &state,
        &nested_contract,
        "value",
        &Json::from(norito::json!({ "input": "42" })),
    );
    assert_eq!(
        ivm::argument_record_decode_count(),
        1,
        "fixture preparation decodes its canonical record"
    );
    // Measure only actual nested dispatch, after the fixture populated its typed table.
    ivm::reset_argument_record_decode_count();
    let result = host.syscall(ivm_sys::SYSCALL_CALL_CONTRACT, &mut vm);
    assert_eq!(
        (10..16).map(|index| vm.register(index)).collect::<Vec<_>>(),
        registers
    );
    let durable_state_overlay = host.drain_durable_state_overlay();
    let error = result.expect_err("nested calls must reject an actively held callee");
    assert!(matches!(
        error.as_unmetered(),
        ivm::VMError::PermissionDenied
    ));
    assert_eq!(
        ivm::argument_record_decode_count(),
        0,
        "nested hold validation must precede callee argument decoding"
    );
    assert!(durable_state_overlay.is_empty());
}
#[test]
fn call_contract_syscall_sets_nested_authority_to_caller_contract_subject() {
    let authority: AccountId = fixture_account("alice");
    let state = contract_test_state(&authority);
    let caller_contract = install_contract(
        &state,
        &authority,
        r"
seiyaku Caller {
  kotoage fn main() authorize(anyone) -> int { return 0; }
}
",
        0,
    );
    let nested_contract = install_contract(
        &state,
        &authority,
        r"
seiyaku Callee {
  view fn who_called() authorize(anyone) -> AccountId {
    return context::authority();
  }
}
",
        1,
    );
    let (result, vm, _) = call_contract_syscall(
        &state,
        &authority,
        &caller_contract,
        &nested_contract,
        "who_called",
        &(Json::new(())),
    );
    result.expect("call_contract syscall should succeed");
    let returned = render_nested_result(
        &vm,
        iroha_data_model::smart_contract::entrypoint::EntrypointValueKindV1::AccountId,
    );
    assert_eq!(
        returned,
        norito::json::Value::from(caller_contract.subject_id().to_string())
    );
}
#[test]
fn call_contract_typed_views_return_exact_norito_values() {
    let authority: AccountId = fixture_account("alice");
    let state = contract_test_state(&authority);
    let asset_definition = AssetDefinitionId::derive_from_components(
        fixture_domain_id(),
        "rose".parse().expect("asset name"),
    );
    let (outer_caller, pool_contract) =
        install_typed_view_fixture(&state, &authority, &asset_definition, &authority);
    let (asset_result, asset_vm, asset_overlay) = call_contract_syscall(
        &state,
        &authority,
        &outer_caller,
        &pool_contract,
        "quote_asset",
        &(Json::new(())),
    );
    asset_result.expect("typed asset view should execute");
    assert_eq!(
        render_nested_result(
            &asset_vm,
            iroha_data_model::smart_contract::entrypoint::EntrypointValueKindV1::AssetDefinitionId,
        ),
        norito::json::Value::from(asset_definition.to_string()),
    );
    assert!(asset_overlay.is_empty(), "typed asset view is read-only");
    let (account_result, account_vm, account_overlay) = call_contract_syscall(
        &state,
        &authority,
        &outer_caller,
        &pool_contract,
        "pool_account",
        &(Json::new(())),
    );
    account_result.expect("typed account view should execute");
    assert_eq!(
        render_nested_result(
            &account_vm,
            iroha_data_model::smart_contract::entrypoint::EntrypointValueKindV1::AccountId,
        ),
        norito::json::Value::from(authority.to_string()),
    );
    assert!(
        account_overlay.is_empty(),
        "typed account view is read-only"
    );
}
#[test]
fn call_contract_typed_view_preserves_account_identity_without_coercion() {
    let authority: AccountId = fixture_account("alice");
    let mismatched_account: AccountId = fixture_account("bob");
    let state = contract_test_state(&authority);
    let asset_definition = AssetDefinitionId::derive_from_components(
        fixture_domain_id(),
        "rose".parse().expect("asset name"),
    );
    let (outer_caller, pool_contract) =
        install_typed_view_fixture(&state, &authority, &asset_definition, &authority);
    let (result, vm, durable_state_overlay) = call_contract_syscall(
        &state,
        &authority,
        &outer_caller,
        &pool_contract,
        "pool_account",
        &(Json::new(())),
    );
    result.expect("typed account view should execute");
    let account = render_nested_result(
        &vm,
        iroha_data_model::smart_contract::entrypoint::EntrypointValueKindV1::AccountId,
    );
    assert_eq!(account, norito::json::Value::from(authority.to_string()));
    assert_ne!(
        account,
        norito::json::Value::from(mismatched_account.to_string()),
        "typed AccountId returns must not be coerced through a lossy representation",
    );
    assert!(durable_state_overlay.is_empty(), "typed view is read-only");
}
#[test]
fn registered_manifest_cannot_relabel_effectful_entrypoint_as_view() {
    let manifest_signing = crate::manifest_signing_test_support::ManifestSigningFixture::new();
    let authority: AccountId = fixture_account("alice");
    let state = contract_test_state(&authority);
    let caller = install_contract(
        &state,
        &authority,
        r"
seiyaku ViewCaller { permission NestedView;
  kotoage fn main() authorize(NestedView) -> int { return 0; }
}
",
        0,
    );
    let target_contract = install_contract(
        &state,
        &authority,
        r"
seiyaku EffectfulView { permission NestedView;
  state int Counter;

  hajimari() {
    Counter = 0;
  }

  kotoage fn seed(int value) authorize(NestedView) {
    Counter = value;
  }

  kotoage fn increment_then_return() authorize(NestedView) -> int {
    Counter = Counter + 1;
    return Counter;
  }
}
",
        1,
    );
    grant_contract_entrypoint_to_account(&state, &authority, authority.clone(), &caller, "main");
    grant_contract_entrypoint_to_account(
        &state,
        &authority,
        authority.clone(),
        &target_contract,
        "seed",
    );
    grant_contract_entrypoint_to_account(
        &state,
        &authority,
        caller.subject_id(),
        &target_contract,
        "increment_then_return",
    );
    let mut ivm_cache = crate::smartcontracts::ivm::cache::IvmCache::new();
    let seed_payload = Json::from_str_norito(r#"{"value":"5"}"#).expect("seed payload");
    execute_contract_call_transaction(
        &state,
        &authority,
        &fixture_signing_keypair(&authority),
        contract_invocation_from_json(&state, target_contract.clone(), "seed", &seed_payload),
        &mut ivm_cache,
    );
    // The embedded, verified contract interface is the execution authority.
    // Corrupt the separately registered manifest after deployment and prove
    // that it cannot relabel effectful bytecode as a view whose effects the
    // nested-call boundary would silently roll back.
    let record =
        crate::smartcontracts::code::fetch_bound_contract_record(&state.view(), &target_contract)
            .expect("registry read completes")
            .expect("installed effectful contract record");
    let mut malicious_manifest = record.manifest;
    let descriptor = malicious_manifest
        .entrypoints
        .as_mut()
        .and_then(|entrypoints| {
            entrypoints
                .iter_mut()
                .find(|descriptor| descriptor.name == "increment_then_return")
        })
        .expect("effectful entrypoint descriptor");
    descriptor.kind = iroha_data_model::smart_contract::manifest::EntryPointKind::View;
    malicious_manifest.provenance = None;
    malicious_manifest = malicious_manifest
        .try_signed(
            manifest_signing.context(),
            manifest_signing.max_frame_bytes(),
            &fixture_signing_keypair(&authority),
        )
        .expect("sign bounded fixture manifest");
    let next_height = u64::try_from((state.view().height() + 1).max(2))
        .ok()
        .and_then(core::num::NonZeroU64::new)
        .expect("next block height must fit in u64 and be non-zero");
    let mut block = state.block(BlockHeader::new(next_height, None, None, 0, 0));
    let mut tx = block.transaction();
    let artifact_id = ContractArtifactId::for_address(&target_contract, record.code_hash)
        .expect("callee retains its exact artifact dataspace");
    tx.world
        .contract_manifests
        .insert(artifact_id, malicious_manifest);
    tx.apply();
    block
        .commit_world_overlay_for_testing()
        .expect("commit adversarial registered-manifest mutation");
    for _ in 0..2 {
        let (result, vm, durable_state_overlay) = call_contract_syscall(
            &state,
            &authority,
            &caller,
            &target_contract,
            "increment_then_return",
            &(Json::new(())),
        );
        result.expect("the embedded kotoage entrypoint remains transaction-capable");
        assert_eq!(
            render_nested_result(
                &vm,
                iroha_data_model::smart_contract::entrypoint::EntrypointValueKindV1::Int
            ),
            norito::json!("6")
        );
        assert!(
            !durable_state_overlay.is_empty(),
            "a forged external view label must not roll back an embedded kotoage effect",
        );
    }
}
#[test]
fn contract_view_of_persisted_account_state_returns_account_tlv() {
    let authority: AccountId = fixture_account("alice");
    let state = contract_test_state(&authority);
    let source = r"
seiyaku StoredAccountView { permission AssetOps;
  state AccountId Stored;

  hajimari(AccountId account_id) {
    Stored = account_id;
  }

  kotoage fn bind(AccountId account_id) authorize(AssetOps) {
    Stored = account_id;
  }

  view fn stored() authorize(anyone) -> AccountId {
    return Stored;
  }
}
";
    let contract = install_contract(&state, &authority, source, 0);
    grant_asset_ops_to_account(&state, &authority, &(authority));
    let mut ivm_cache = crate::smartcontracts::ivm::cache::IvmCache::new();
    let bind_payload =
        Json::from_str_norito(&format!(r#"{{"account_id":"{authority}"}}"#)).expect("bind payload");
    execute_contract_call_transaction(
        &state,
        &authority,
        &fixture_signing_keypair(&authority),
        contract_invocation_from_json(&state, contract.clone(), "bind", &bind_payload),
        &mut ivm_cache,
    );
    let (code, _manifest) = kotodama_lang::compiler::Compiler::new()
        .compile_source_with_manifest(source)
        .expect("compile source with manifest");
    let parsed = ivm::ProgramMetadata::parse(&code).expect("parse program metadata");
    let prefix_len = parsed.prefix_len() as u64;
    let descriptor = parsed
        .contract_interface
        .as_ref()
        .expect("contract interface")
        .entrypoints
        .iter()
        .find(|candidate| candidate.name == "stored")
        .expect("stored view descriptor");
    assert!(matches!(
        descriptor.kind,
        iroha_data_model::smart_contract::manifest::EntryPointKind::View
    ));
    let entry_pc = prefix_len + descriptor.entry_pc;
    let view = state.view();
    let mut host = CoreHostImpl::new(authority.clone());
    host.set_query_state(&view);
    host.set_durable_state_snapshot_from_world(view.world());
    host.set_contract_runtime_context(Some(ContractRuntimeExecutionContext {
        contract_subject: contract.subject_id(),
        contract_address: contract,
        contract_alias: None,
        entrypoint: "stored".to_owned(),
    }));
    let mut vm = IVM::new(1_000_000);
    vm.load_program(&code).expect("load program");
    vm.set_register(1, vm.memory.code_len());
    vm.set_program_counter(entry_pc).expect("seek stored view");
    vm.run_with_host(&mut host).expect("run stored view");
    assert!(matches!(vm.call_result_word_count(), Ok(1)));
    let returned_pointer = vm
        .public_call_result_word(0)
        .expect("completed public result owns one account pointer word");
    let tlv = vm
        .validate_tlv(returned_pointer)
        .expect("stored view return tlv");
    assert_eq!(tlv.type_id, PointerType::AccountId);
    let returned: AccountId =
        norito::decode_from_bytes(tlv.payload).expect("decode stored account");
    assert_eq!(returned, authority);
}
#[test]
fn call_contract_syscall_unit_return_uses_canonical_stack_policy() {
    let authority: AccountId = fixture_account("alice");
    let state = contract_test_state(&authority);
    let caller_contract = install_contract(
        &state,
        &authority,
        r"
seiyaku Caller {
  kotoage fn main() authorize(anyone) -> int { return 0; }
}
",
        0,
    );
    let nested_contract = install_contract(
        &state,
        &authority,
        r"
seiyaku Callee { permission AssetOps;
  state int backlog;
  state int safe_mode;

  hajimari() {
    backlog = 0;
    safe_mode = 0;
  }

  kotoage fn report(int backlog_value, int safe_mode_value) authorize(AssetOps) {
    backlog = backlog_value;
    safe_mode = safe_mode_value;
  }
}
",
        1,
    );
    grant_asset_ops_to_account(&state, &authority, &(caller_contract.subject_id()));
    let payload = Json::from(norito::json!({
        "backlog_value": "3",
        "safe_mode_value": "1"
    }));
    let (result, vm, durable_state_overlay) = call_contract_syscall(
        &state,
        &authority,
        &caller_contract,
        &nested_contract,
        "report",
        &(payload),
    );
    result.expect("unit nested contract call should succeed");
    assert_eq!(
        vm.memory.stack_limit(),
        ivm::IvmStackPolicy::V1.stack_limit_for_gas(1_000_000),
        "nested execution must use the canonical V1 guest-stack policy",
    );
    let schema = iroha_data_model::smart_contract::entrypoint::EntrypointValueTypeV1 {
        nodes: vec![iroha_data_model::smart_contract::entrypoint::EntrypointValueTypeNodeV1::Unit],
    };
    let record = captured_nested_result(&vm, &schema);
    assert_eq!(
        super::super::return_value::render_entrypoint_return_record(&schema, &record).unwrap(),
        norito::json::Value::Null,
        "nested Unit returns carry the canonical typed null record",
    );
    assert!(
        durable_state_overlay
            .keys()
            .any(|key| key.as_ref().ends_with("/backlog")),
        "state overlay should include nested unit-return writes",
    );
    assert!(
        durable_state_overlay
            .keys()
            .any(|key| key.as_ref().ends_with("/safe_mode")),
        "state overlay should include all nested unit-return writes",
    );
}
#[test]
fn call_contract_syscall_roundtrips_unit_and_nominal_errors_in_stored_products() {
    let authority: AccountId = fixture_account("alice");
    let state = contract_test_state(&authority);
    let caller_contract = install_contract(
        &state,
        &authority,
        "seiyaku Caller { kotoage fn main() authorize(anyone) -> int { return 0; } }",
        0,
    );
    let source = r"
seiyaku Callee { permission AssetOps;
  error enum ReceiptError { Refused = 7 }
  struct Receipt {
    () marker;
    int sequence;
    ReceiptError reason;
    Result<(), ReceiptError> outcome;
    List<(), 2> markers;
  }
  state Receipt Stored;
  hajimari() {
    Stored = Receipt {
      marker: (), sequence: 0, reason: ReceiptError::Refused,
      outcome: Result::ok(()), markers: [],
    };
  }
  fn reload() -> Receipt { return Stored; }
  kotoage fn roundtrip(Receipt value) authorize(AssetOps) -> Receipt {
    Stored = value;
    return reload();
  }
}
";
    let nested_contract = install_contract(&state, &authority, source, 1);
    grant_asset_ops_to_account(&state, &authority, &(caller_contract.subject_id()));
    let expected = norito::json!({
        "marker": null,
        "sequence": "42",
        "reason": "Refused",
        "outcome": { "err": "Refused" },
        "markers": [null, null],
    });
    let payload = Json::from(
        norito::json::object([("value", expected.clone())]).expect("nested argument object"),
    );
    let (result, vm, durable_state_overlay) = call_contract_syscall(
        &state,
        &authority,
        &caller_contract,
        &nested_contract,
        "roundtrip",
        &(payload),
    );
    result.expect("nested products must retain Unit words and nominal error identities");
    let code = kotodama_lang::compiler::Compiler::new()
        .compile_source(source)
        .unwrap();
    let parsed = ivm::ProgramMetadata::parse(&code).unwrap();
    let interface = parsed.contract_interface.unwrap();
    let schema = interface
        .entrypoints
        .iter()
        .find(|entrypoint| entrypoint.name == "roundtrip")
        .unwrap()
        .return_schema
        .as_ref()
        .unwrap();
    assert_eq!(schema.word_count(), Some(5));
    let record = captured_nested_result(&vm, schema);
    assert_eq!(
        super::super::return_value::render_entrypoint_return_record(schema, &record).unwrap(),
        expected,
    );
    assert!(
        durable_state_overlay
            .keys()
            .any(|key| key.as_ref().ends_with("/Stored")),
        "the returned product was reloaded from a real durable-state write",
    );
}
#[test]
fn call_contract_syscall_rolls_back_failed_nested_state_mutation() {
    let authority: AccountId = fixture_account("alice");
    let state = contract_test_state(&authority);
    let caller_contract = install_contract(
        &state,
        &authority,
        r"
seiyaku Caller {
  kotoage fn main() authorize(anyone) -> int { return 0; }
}
",
        0,
    );
    let nested_contract = install_contract(
        &state,
        &authority,
        r"
seiyaku Callee { permission AssetOps;
  error enum CalleeError {
    ForcedFailure = 1,
  }

  state int counter;

  hajimari() {
    counter = 0;
  }

  kotoage fn fail_after_write() authorize(AssetOps) -> int {
    counter = 9;
    require(false, CalleeError::ForcedFailure);
    return 0;
  }
}
",
        1,
    );
    grant_asset_ops_to_account(&state, &authority, &(caller_contract.subject_id()));
    let (result, _vm, durable_state_overlay) = call_contract_syscall(
        &state,
        &authority,
        &caller_contract,
        &nested_contract,
        "fail_after_write",
        &(Json::new(())),
    );
    assert!(result.is_err(), "nested contract call should fail");
    assert!(
        durable_state_overlay.is_empty(),
        "failed nested call must not leak contract-state writes",
    );
}
#[test]
fn call_contract_syscall_rolls_back_when_typed_return_validation_fails() {
    let authority: AccountId = fixture_account("alice");
    let state = contract_test_state(&authority);
    let caller_contract = install_contract(
        &state,
        &authority,
        r"
seiyaku Caller {
  kotoage fn main() authorize(anyone) -> int { return 0; }
}
",
        0,
    );
    let nested_contract = install_contract_with_interface(
        &state,
        &authority,
        r#"
seiyaku Callee { permission AssetOps;
  state int counter;

  hajimari() {
    counter = 0;
  }

  kotoage fn write_then_return() authorize(AssetOps) -> bytes {
    counter = 9;
    return b"\xff";
  }
}
"#,
        1,
        |interface| {
            let descriptor = interface
                .entrypoints
                .iter_mut()
                .find(|entrypoint| entrypoint.name == "write_then_return")
                .expect("callee entrypoint descriptor");
            // The bytecode produces invalid UTF-8 after writing counter = 9.
            // Bind both metadata surfaces to String so admission succeeds and
            // the complete callable type rejects that value at protected return.
            let original = exact_return_type(
                iroha_data_model::smart_contract::entrypoint::EntrypointValueKindV1::Blob,
            );
            assert_eq!(
                descriptor.return_schema,
                Some(original.clone()),
                "the unmodified compiler artifact declares its actual Bytes result"
            );
            let claimed = exact_return_type(
                iroha_data_model::smart_contract::entrypoint::EntrypointValueKindV1::String,
            );
            descriptor.return_type = Some("string".to_owned());
            descriptor.return_schema = Some(claimed.clone());
            let entry_pc = descriptor.entry_pc;
            let callable = interface
                .callables
                .iter_mut()
                .find(|callable| callable.entry_pc == entry_pc)
                .expect("the compiled entrypoint has its authenticated callable");
            assert_eq!(
                callable.results,
                ivm::call::CallSchemaV1::from_entrypoint_type(&original).unwrap(),
                "the compiled callable preserves the complete Bytes type"
            );
            callable.results = ivm::call::CallSchemaV1::from_entrypoint_type(&claimed).unwrap();
            assert!(
                callable.results.matches_entrypoint_type(&claimed),
                "entrypoint and callable must agree on the complete String type"
            );
            assert!(
                !callable.results.matches_entrypoint_type(&original),
                "String and Bytes remain distinct despite sharing a Blob envelope"
            );
        },
    );
    grant_asset_ops_to_account(&state, &authority, &(caller_contract.subject_id()));
    let view = state.view();
    let mut host = CoreHostImpl::new(authority.clone());
    host.set_query_state(&view);
    host.set_durable_state_snapshot_from_world(view.world());
    let mut vm = IVM::new(1_000_000);
    bind_nested_caller(&mut host, &mut vm, &view, &authority, &caller_contract);
    let caller_context = host.current_contract_runtime_context.clone().unwrap();
    store_typed_contract_call(
        &mut vm,
        &state,
        &nested_contract,
        "write_then_return",
        &Json::new(()),
    );
    let err = host
        .syscall(ivm_sys::SYSCALL_CALL_CONTRACT, &mut vm)
        .expect_err("invalid UTF-8 must fail typed return validation");
    assert!(
        matches!(err.as_unmetered(), ivm::VMError::NoritoInvalid),
        "invalid String payload must be rejected by protected return validation: {err:?}",
    );
    assert_eq!(
        host.authority, authority,
        "outer authority must be restored"
    );
    let restored_context = host
        .current_contract_runtime_context
        .as_ref()
        .expect("outer contract frame must be restored");
    assert_eq!(
        restored_context.contract_address,
        caller_context.contract_address
    );
    assert_eq!(
        restored_context.contract_subject,
        caller_context.contract_subject
    );
    assert_eq!(
        restored_context.contract_alias,
        caller_context.contract_alias
    );
    assert_eq!(restored_context.entrypoint, caller_context.entrypoint);
    assert!(host.durable_state_overlay.is_empty());
    assert!(host.queued.is_empty());
}
#[test]
fn call_contract_syscall_accepts_only_exact_public_authenticated_typed_tables() {
    let authority = fixture_account("alice");
    let state = contract_test_state(&authority);
    let caller = install_contract(
        &state,
        &authority,
        "seiyaku Caller { kotoage fn main() authorize(anyone) {} }",
        0,
    );
    let target_contract = install_contract(
        &state,
        &authority,
        "seiyaku Callee { view fn no_args() authorize(anyone) -> int {1} view fn echo(int value) authorize(anyone) -> int {value} }",
        1,
    );
    let invoke = |selector: &str, change: &dyn Fn(&mut IVM, [u64; 6])| {
        let view = state.view();
        let mut host = CoreHostImpl::new(authority.clone());
        host.set_query_state(&view);
        let mut vm = IVM::new(1_000_000);
        bind_nested_caller(&mut host, &mut vm, &view, &authority, &caller);
        let registers = store_typed_contract_call(
            &mut vm,
            &state,
            &target_contract,
            selector,
            &Json::from(norito::json!({"value":"7"})),
        );
        change(&mut vm, registers);
        ivm::reset_argument_record_decode_count();
        let result = host.syscall(ivm_sys::SYSCALL_CALL_CONTRACT, &mut vm);
        assert_eq!(
            ivm::argument_record_decode_count(),
            0,
            "typed VM calls never decode a transported argument record"
        );
        (result, vm)
    };
    let (result, vm) = invoke("echo", &|_, _| {});
    result.unwrap();
    assert_eq!(
        render_nested_result(
            &vm,
            iroha_data_model::smart_contract::entrypoint::EntrypointValueKindV1::Int
        ),
        norito::json!("7")
    );
    for case in 0..7 {
        let (result, vm) = invoke(
            if case == 1 { "no_args" } else { "echo" },
            &|vm, _registers| match case {
                0 => {
                    vm.set_register(12, 0);
                    vm.set_register(13, 0);
                }
                1 => {
                    let table = vm.alloc_heap(8).unwrap();
                    vm.store_u64(table, 0).unwrap();
                    vm.set_register(12, table);
                    vm.set_register(13, 1);
                }
                2 => {
                    vm.set_register(13, 2);
                }
                3 => {
                    vm.set_register(15, 2);
                }
                4 => {
                    let json = store_tlv(
                        vm,
                        PointerType::Json,
                        &norito_blob(&Json::from(norito::json!({"value":"7"}))),
                    );
                    vm.set_register(12, json);
                }
                5 => {
                    let target = store_tlv(vm, PointerType::Blob, b"echo");
                    vm.set_register(11, target);
                }
                6 => {
                    let address = store_tlv(
                        vm,
                        PointerType::Blob,
                        &[b'a';
                            iroha_data_model::smart_contract::CONTRACT_ADDRESS_LITERAL_LEN_V1 + 1],
                    );
                    vm.set_register(10, address);
                }
                _ => unreachable!(),
            },
        );
        let error = result
            .expect_err("malformed typed tables or retired selector/JSON transport must fail");
        assert!(
            error.execution_deferral().is_none(),
            "case {case}: {error:?}"
        );
        assert!(
            matches!(vm.load_u64(vm.register(14)), Ok(0)),
            "case {case} published no output"
        );
    }
    for wrong_ordinal in [false, true] {
        let (result, _) = invoke("echo", &|vm, registers| {
            let mut binding = ivm_abi::contract_call::ContractCallBindingV1::from_bytes(
                vm.validate_tlv(registers[1]).unwrap().payload,
            )
            .unwrap();
            if wrong_ordinal {
                binding.entrypoint = u32::MAX;
            } else {
                binding.code_hash = Hash::new(b"substituted schema artifact");
            }
            let pointer = store_tlv(vm, PointerType::NoritoBytes, &binding.to_bytes().unwrap());
            vm.set_register(11, pointer);
        });
        assert!(matches!(
            result.unwrap_err().as_unmetered(),
            ivm::VMError::PermissionDenied
        ));
    }
    for register in 10..16 {
        let (result, _) = invoke("no_args", &|vm, _| {
            vm.set_zk_mode(true).unwrap();
            vm.registers.set_tag(register, true);
        });
        assert!(
            matches!(
                result.unwrap_err().as_unmetered(),
                ivm::VMError::PrivacyViolation
            ),
            "private r{register}"
        );
    }
}
#[test]
fn core_host_rejects_raw_private_witness_transport_without_mutation() {
    let authority = fixture_account("alice");
    let mut host = CoreHost::new(authority);
    let mut vm = IVM::new(10_000);
    vm.set_zk_mode(true)
        .expect("private lifecycle cleanup succeeds");
    vm.set_register(10, 0);
    // Private-input ABI V1 tag zero selects Kotodama `int`.
    vm.set_register(11, 0);
    let registers_before = vm.registers.snapshot();
    let tags_before = vm.registers.snapshot_tags();
    let memory_root_before = vm.memory.current_root();
    assert!(
        matches!(
            host.prepare_syscall(ivm_sys::SYSCALL_GET_PRIVATE_INPUT, &vm),
            Err(ivm::VMError::PermissionDenied)
        ),
        "consensus quote preparation must not expose the prover/test witness transport"
    );
    assert!(
        matches!(
            host.syscall(ivm_sys::SYSCALL_GET_PRIVATE_INPUT, &mut vm),
            Err(ivm::VMError::PermissionDenied)
        ),
        "a direct host call must remain fail-closed if entrypoint admission is bypassed"
    );
    assert_eq!(vm.registers.snapshot(), registers_before);
    assert_eq!(vm.registers.snapshot_tags(), tags_before);
    assert_eq!(vm.memory.current_root(), memory_root_before);
    assert!(host.queued.is_empty());
    assert!(host.durable_state_overlay.is_empty());
}
#[test]
fn call_contract_syscall_checks_affordability_before_state_lookup_or_decode() {
    let authority: AccountId = fixture_account("alice");
    let state = contract_test_state(&authority);
    let caller = install_contract(
        &state,
        &authority,
        "seiyaku Caller { kotoage fn main() authorize(anyone) {} }",
        0,
    );
    let address = ContractAddress::derive(
        state.view().network_id(),
        &authority,
        404,
        DataSpaceId::UNIVERSAL,
    )
    .expect("derive unavailable target address");
    let binding = ivm_abi::contract_call::ContractCallBindingV1 {
        code_hash: Hash::new(b"unavailable callee"),
        entrypoint: 0,
    }
    .to_bytes()
    .unwrap();
    let request_gas = ivm::gas::syscall_byte_gas(
        ivm::gas::G_CALL_CONTRACT,
        address.as_ref().len() + binding.len(),
        0,
    );
    let mut host = CoreHost::new(authority.clone());
    let mut vm = IVM::new(1_000_000);
    // Bind the genuine caller but deliberately leave query_state absent.
    bind_nested_caller(&mut host, &mut vm, &state.view(), &authority, &caller);
    let target = store_tlv(&mut vm, PointerType::Blob, address.as_ref().as_bytes());
    let binding = store_tlv(&mut vm, PointerType::NoritoBytes, &binding);
    let result = vm.alloc_heap(8).unwrap();
    for (index, word) in [target, binding, 0, 0, result, 1].into_iter().enumerate() {
        vm.set_register(10 + index, word);
    }
    vm.set_gas_limit(request_gas.saturating_sub(1));
    ivm::reset_argument_record_decode_count();
    assert!(
        matches!(
            host.syscall(ivm_sys::SYSCALL_CALL_CONTRACT, &mut vm),
            Err(ivm::VMError::OutOfGas)
        ),
        "unaffordable calls must fail before the absent query state is consulted"
    );
    assert_eq!(ivm::argument_record_decode_count(), 0);
    assert!(host.queued.is_empty());
    assert_eq!(vm.load_u64(result).unwrap(), 0);
}
// Low-level artifact authorization tests bind a genuine signed Halt root.
// They exercise artifact authority snapshots, not correspondence to that
// program's VM output, full transaction admission, or budget qualification.
fn artifact_test_signed_root(
    network_id: NetworkId,
    authority: &AccountId,
) -> iroha_data_model::transaction::SignedTransaction {
    let keypair = if authority == &*ALICE_ID {
        &*ALICE_KEYPAIR
    } else if authority == &*BOB_ID {
        &*BOB_KEYPAIR
    } else {
        panic!("artifact fixture requires a known actual signer");
    };
    let mut program = ivm::ProgramMetadata::default().encode();
    program.extend_from_slice(&ivm::encoding::wide::encode_halt().to_le_bytes());
    TransactionBuilder::new(
        network_id,
        authority.clone(),
        iroha_data_model::transaction::FeePaymentIntent::authority(
            Vec::new(),
            core::num::NonZeroU64::new(1_000_000),
        ),
    )
    .with_executable(Executable::Ivm(
        iroha_data_model::transaction::executable::IvmBytecode::from_compiled(program),
    ))
    .sign(keypair.private_key())
}

fn bind_artifact_test_source_root(
    tx: &mut StateTransaction<'_, '_>,
    source: &iroha_data_model::transaction::SignedTransaction,
) {
    assert_eq!(tx.current_dataspace_id, tx.world.current_dataspace_id);
    tx.current_tx_hash = Some(source.hash());
    tx.tx_call_hash = Some(Hash::from(source.hash_as_entrypoint()));
    tx.begin_execution_effect_budget(source)
        .expect("bind exact signed artifact test root");
}

pub(super) fn bind_artifact_test_signed_root(
    tx: &mut StateTransaction<'_, '_>,
    authority: &AccountId,
) {
    let source = artifact_test_signed_root(tx.network_id, authority);
    bind_artifact_test_source_root(tx, &source);
}

#[test]
fn call_contract_syscall_preserves_root_and_nested_transfer_authorities_in_artifacts() {
    let authority: AccountId = fixture_account("alice");
    let asset_def_id: AssetDefinitionId = AssetDefinitionId::derive_from_components(
        fixture_domain_id(),
        "rose".parse().expect("asset name"),
    );
    let domain = Domain::new(fixture_domain_id()).build(&authority);
    let account = build_fixture_account(&authority, &authority);
    let asset_def = AssetDefinition::numeric(
        asset_def_id.clone(),
        "rose".to_owned(),
        iroha_data_model::asset::AssetBalancePolicy::Global,
        None,
    )
    .build(&authority);
    let source_asset_id = AssetId::of(asset_def_id.clone(), authority.clone());
    let source_asset = Asset::new(source_asset_id.clone(), Quantity::from(5_u32));
    let mut world = World::with_assets([domain], [account], [asset_def], [source_asset], []);
    let mut permissions = Permissions::new();
    assert!(permissions.insert(
        iroha_executor_data_model::permission::smart_contract::CanManageSmartContractCode.into(),
    ));
    world
        .account_permissions_mut_for_testing()
        .insert(authority.clone(), permissions);
    let state = contract_test_state_with_world(&authority, world);
    let caller_contract = install_contract(
        &state,
        &authority,
        r"
seiyaku Caller {
  kotoage fn open_position() authorize(anyone) -> int { return 0; }
}
",
        0,
    );
    let nested_contract = install_contract(
        &state,
        &authority,
        r#"
seiyaku Callee { permission AssetOps;
  kotoage fn pull_into_vault(AccountId target,
                             AssetDefinitionId asset,
                             quantity amount) authorize(AssetOps) -> quantity {
    ledger::asset::transfer(source: context::authority(), destination: target, asset_definition: asset, amount: amount, dataspace: DataSpaceId::parse("0"));
    return amount;
  }
}
"#,
        1,
    );
    grant_asset_ops_to_account(&state, &authority, &(caller_contract.subject_id()));
    // The source argument follows context::authority(), while emitted ledger
    // effects are authorized by the contract subject of each active frame.
    grant_test_asset_transfer(
        &state,
        caller_contract.subject_id(),
        source_asset_id.clone(),
    );
    grant_test_asset_transfer(
        &state,
        nested_contract.subject_id(),
        AssetId::of(asset_def_id.clone(), caller_contract.subject_id()),
    );
    let view = state.view();
    let caller_context = ContractRuntimeExecutionContext {
        contract_address: caller_contract.clone(),
        contract_subject: caller_contract.subject_id(),
        contract_alias: None,
        entrypoint: "open_position".to_owned(),
    };
    let mut host = CoreHostImpl::new(authority.clone());
    host.set_query_state(&view);
    host.set_durable_state_snapshot_from_world(view.world());
    host.set_contract_runtime_context(Some(caller_context.clone()));
    host.set_contract_entrypoint_authorization(Some(ContractEntrypointAuthorizationSnapshot::new(
        authority.clone(),
        caller_context.entrypoint.clone(),
        iroha_data_model::smart_contract::manifest::EntrypointAuthorizationV1::Anyone,
        &crate::smartcontracts::code::fetch_bound_contract_identity(&view, &caller_contract)
            .expect("read live caller identity")
            .expect("installed caller"),
    )));
    let mut vm = IVM::new(1_000_000);
    bind_nested_caller(&mut host, &mut vm, &view, &authority, &caller_contract);
    let amount = Quantity::from(3_u32);
    let from_ptr = store_tlv(&mut vm, PointerType::AccountId, &norito_blob(&authority));
    let to_ptr = store_tlv(
        &mut vm,
        PointerType::AccountId,
        &norito_blob(&caller_contract.subject_id()),
    );
    let asset_ptr = store_tlv(
        &mut vm,
        PointerType::AssetDefinitionId,
        &norito_blob(&asset_def_id),
    );
    let amount_ptr = store_quantity(&mut vm, &amount);
    let dataspace_ptr = store_tlv(
        &mut vm,
        PointerType::DataSpaceId,
        &norito::to_bytes(&DataSpaceId::UNIVERSAL).expect("encode dataspace"),
    );
    vm.set_register(10, from_ptr);
    vm.set_register(11, to_ptr);
    vm.set_register(12, asset_ptr);
    vm.set_register(13, amount_ptr);
    vm.set_register(14, dataspace_ptr);
    host.syscall(ivm_sys::SYSCALL_TRANSFER_ASSET_SCOPED, &mut vm)
        .expect("root transfer should enqueue");
    let nested_target = nested_contract.subject_id().to_string();
    let nested_asset = asset_def_id.to_string();
    let nested_payload = Json::from(norito::json!({
        "target": nested_target,
        "asset": nested_asset,
        "amount": "3"
    }));
    store_typed_contract_call(
        &mut vm,
        &state,
        &nested_contract,
        "pull_into_vault",
        &nested_payload,
    );
    host.syscall(ivm_sys::SYSCALL_CALL_CONTRACT, &mut vm)
        .expect("nested call should succeed");
    let artifacts = host
        .into_execution_artifacts(Some(caller_context))
        .expect("extract execution artifacts");
    assert_eq!(
        artifacts
            .queued_instructions_with_authority()
            .into_iter()
            .map(|(effect_authority, _)| effect_authority)
            .collect::<Vec<_>>(),
        vec![caller_contract.subject_id(), nested_contract.subject_id()],
        "root and nested effects retain their respective contract subjects"
    );
    // Apply these component artifacts in ordinary execution, with no
    // claim to an authenticated genesis source capability.
    let next_height = u64::try_from((state.view().height() + 1).max(2))
        .ok()
        .and_then(core::num::NonZeroU64::new)
        .expect("next block height");
    let mut block = state.block(BlockHeader::new(next_height, None, None, 0, 0));
    let source = artifact_test_signed_root(*block.network_id(), &authority);
    let source_call = Hash::from(source.hash_as_entrypoint());
    // Retain this exact signed source's invocation identity before borrowing
    // its State child. The signed source stays local; no Network or finality
    // authority is granted by this bounded component owner.
    let mut stx = block.transaction_for_fastpq_testing(source_call);
    bind_artifact_test_source_root(&mut stx, &source);
    assert_eq!(stx.tx_call_hash, Some(source_call));
    artifacts
        .apply_to_transaction(&mut stx, &authority)
        .expect("root and nested transfers should apply");
    stx.finish_execution_effect_budget()
        .expect("close artifact test root");
    let authority_balance = stx
        .world
        .asset(&source_asset_id)
        .expect("authority asset remains")
        .value()
        .clone();
    let caller_asset_id = AssetId::of(asset_def_id.clone(), caller_contract.subject_id());
    let nested_asset = AssetId::of(asset_def_id, nested_contract.subject_id());
    let callee_balance = stx
        .world
        .asset(&nested_asset)
        .expect("callee contract asset exists")
        .value()
        .clone();
    assert_eq!(authority_balance.as_ref(), &Quantity::from(2_u32));
    assert!(
        stx.world.asset(&caller_asset_id).is_err(),
        "fully drained caller balance should remove the asset entry",
    );
    assert_eq!(callee_balance.as_ref(), &Quantity::from(3_u32));
}
const ALIAS_CASES_V1: &[u8] = include_bytes!("host/alias_contract_cases_v1.json");
const ALIAS_CONTRACT_CASE_IDS: [&str; 17] = [
    "contract_call_transaction_account_id_alias_shorthand_uses_current_binding",
    "contract_call_transaction_resolve_account_alias_builtin_uses_current_binding",
    "contract_call_transaction_domain_qualified_resolve_account_alias_builtin_uses_current_binding",
    "contract_call_transaction_resolve_account_alias_builtin_without_permission_is_rejected",
    "contract_call_transaction_resolve_account_alias_builtin_without_binding_is_rejected",
    "contract_call_transaction_resolve_account_alias_builtin_invalid_literal_is_rejected",
    "contract_call_transaction_domain_qualified_resolve_account_alias_builtin_without_domain_permission_is_rejected",
    "contract_call_transaction_account_id_alias_shorthand_without_binding_is_rejected",
    "contract_call_transaction_account_id_alias_shorthand_invalid_literal_is_rejected",
    "contract_call_transaction_domain_qualified_account_id_alias_shorthand_without_domain_permission_is_rejected",
    "contract_call_transaction_domain_qualified_account_id_alias_shorthand_accepts_domain_permission_without_dataspace_permission",
    "contract_call_transaction_domain_qualified_account_id_alias_shorthand_without_binding_is_rejected",
    "contract_call_transaction_domain_qualified_resolve_account_alias_builtin_without_binding_is_rejected",
    "contract_call_transaction_domain_qualified_resolve_account_alias_builtin_accepts_domain_permission_without_dataspace_permission",
    "contract_call_transaction_domain_qualified_resolve_account_alias_builtin_invalid_literal_is_rejected",
    "contract_call_transaction_domain_qualified_account_id_alias_shorthand_invalid_literal_is_rejected",
    "contract_call_transaction_domain_qualified_account_id_alias_shorthand_uses_current_binding",
];
#[derive(Debug, crate::json_macros::JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct AliasContractCaseFileV1 {
    schema: String,
    cases: Vec<AliasContractCaseV1>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, crate::json_macros::JsonDeserialize)]
#[norito(
    tag = "kind",
    content = "value",
    rename_all = "snake_case",
    deny_unknown_fields
)]
enum AliasContractSetupV1 {
    DomainlessCurrent,
    DomainlessWithoutPermission,
    DomainlessWithoutBinding,
    DomainCurrent,
    DomainWithoutPermission,
    DomainPermissionOnly,
    DomainWithoutBinding,
    Invalid,
}
impl AliasContractSetupV1 {
    fn uses_domain(self) -> bool {
        matches!(
            self,
            Self::DomainCurrent
                | Self::DomainWithoutPermission
                | Self::DomainPermissionOnly
                | Self::DomainWithoutBinding
        )
    }
    fn is_current(self) -> bool {
        matches!(self, Self::DomainlessCurrent | Self::DomainCurrent)
    }
    fn uses_configured_policy_payment_asset(self) -> bool {
        matches!(
            self,
            Self::DomainWithoutPermission | Self::DomainPermissionOnly
        )
    }
}
#[derive(Debug, crate::json_macros::JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct AliasContractCaseV1 {
    id: String,
    setup: AliasContractSetupV1,
    recipient_expression: String,
    direct_source: Option<String>,
    setup_seed: Option<u8>,
    transaction_hash_byte: Option<u8>,
    rebind_seed: Option<u8>,
    messages: Vec<String>,
}
impl AliasContractCaseV1 {
    fn validate_profile(&self) {
        assert_eq!(
            self.transaction_hash_byte.is_some(),
            self.setup.uses_domain(),
            "transaction hash/profile mismatch: {}",
            self.id
        );
        assert!(
            self.setup_seed.is_none()
                || self.setup.uses_domain()
                || self.setup == AliasContractSetupV1::DomainlessCurrent,
            "setup seed/profile mismatch: {}",
            self.id
        );
        assert!(
            self.direct_source.is_none() || self.setup.is_current(),
            "{}",
            self.id
        );
        assert!(
            self.rebind_seed.is_none() || self.setup.is_current(),
            "{}",
            self.id
        );
        let expected_message_count = match self.setup {
            AliasContractSetupV1::DomainlessCurrent | AliasContractSetupV1::DomainCurrent => 0,
            AliasContractSetupV1::DomainlessWithoutPermission
            | AliasContractSetupV1::DomainWithoutPermission
            | AliasContractSetupV1::DomainlessWithoutBinding
            | AliasContractSetupV1::DomainWithoutBinding
            | AliasContractSetupV1::DomainPermissionOnly => 2,
            AliasContractSetupV1::Invalid => 1,
        };
        assert_eq!(self.messages.len(), expected_message_count, "{}", self.id);
    }
}
struct AliasContractCaseState {
    state: State,
    authority: AccountId,
    merchant_account: Option<AccountId>,
    replacement_account: Option<AccountId>,
    asset_definition_id: AssetDefinitionId,
    source_asset_id: AssetId,
    paynet: DataSpaceId,
    retail_domain_id: Option<DomainId>,
}
fn load_alias_contract_cases() -> AliasContractCaseFileV1 {
    use sha2::{Digest as _, Sha256};
    assert_eq!(
        Hash::new(ALIAS_CASES_V1).to_string(),
        "207caca5e78d7fab0112a2239473188d396d004498bb71b534f9780cc272c615",
        "the alias contract case fixture must retain its pinned Iroha digest"
    );
    assert_eq!(
        hex::encode(Sha256::digest(ALIAS_CASES_V1)),
        "3cca3543420745decad1f2ea55f8118f850a8362abe8b31bae4fc2f86f96ecf0",
        "the alias contract case fixture must retain its pinned SHA-256 digest"
    );
    let fixture: AliasContractCaseFileV1 = norito::json::from_slice(ALIAS_CASES_V1)
        .expect("decode strict alias contract case fixture");
    assert_eq!(
        fixture.schema,
        "iroha_core.ivm_host.alias_contract_cases.v1"
    );
    assert_eq!(fixture.cases.len(), ALIAS_CONTRACT_CASE_IDS.len());
    for (case, expected_id) in fixture.cases.iter().zip(ALIAS_CONTRACT_CASE_IDS) {
        assert_eq!(case.id, expected_id);
        case.validate_profile();
    }
    fixture
}
static ALIAS_CONTRACT_CASES: std::sync::LazyLock<AliasContractCaseFileV1> =
    std::sync::LazyLock::new(load_alias_contract_cases);
fn build_alias_contract_case_state(case: &AliasContractCaseV1) -> AliasContractCaseState {
    let authority: AccountId = fixture_account("alice");
    let merchant_account = if case.setup == AliasContractSetupV1::Invalid {
        None
    } else {
        Some(fixture_account("bob"))
    };
    let replacement_account = if case.setup.is_current() {
        Some(fixture_account("carol"))
    } else {
        None
    };
    let asset_definition_id = AssetDefinitionId::derive_from_components(
        fixture_domain_id(),
        "rose".parse().expect("asset name"),
    );
    let authority_account = build_fixture_account(&authority, &authority);
    let mut accounts = vec![authority_account];
    if let Some(merchant) = &merchant_account {
        accounts.push(build_fixture_account(merchant, &authority));
    }
    if let Some(replacement) = &replacement_account {
        accounts.push(build_fixture_account(replacement, &authority));
    }
    let asset_definition = AssetDefinition::numeric(
        asset_definition_id.clone(),
        "rose".to_owned(),
        iroha_data_model::asset::AssetBalancePolicy::Global,
        None,
    )
    .build(&authority);
    let source_asset_id = AssetId::of(asset_definition_id.clone(), authority.clone());
    let source_asset = Asset::new(source_asset_id.clone(), Quantity::from(5_u32));
    let (world, retail_domain_id) = if case.setup.uses_domain() {
        let payment_asset_literal = if case.setup.uses_configured_policy_payment_asset() {
            "6TEAJqbb8oEPmLncoNiMRbLEK6tw"
        } else {
            "61CtjvNd9T3THAR65GsMVHr82Bjc"
        };
        let payment_asset_definition_id: AssetDefinitionId = payment_asset_literal
            .parse()
            .expect("payment asset definition id");
        let retail_domain_id = DomainId::try_new("bank", "paynet").expect("retail domain id");
        let base_domain = Domain::new(fixture_domain_id()).build(&authority);
        let retail_domain = Domain::new(retail_domain_id.clone()).build(&authority);
        let genesis_domain =
            Domain::new(DomainId::try_new("genesis", "universal").expect("genesis domain id"))
                .build(&authority);
        let payment_definition = AssetDefinition::numeric(
            payment_asset_definition_id.clone(),
            "xor".to_owned(),
            iroha_data_model::asset::AssetBalancePolicy::Global,
            None,
        )
        .build(&authority);
        let payment_asset = Asset::new(
            AssetId::of(payment_asset_definition_id.clone(), authority.clone()),
            Quantity::from(1_000_u32),
        );
        let mut world = World::with_assets(
            [base_domain, retail_domain, genesis_domain],
            accounts,
            [asset_definition, payment_definition],
            [source_asset, payment_asset],
            [],
        );
        if case.setup.uses_configured_policy_payment_asset() {
            let mut permissions = Permissions::new();
            assert!(
                    permissions.insert(
                        iroha_executor_data_model::permission::smart_contract::CanManageSmartContractCode
                            .into(),
                    )
                );
            world
                .account_permissions_mut_for_testing()
                .insert(authority.clone(), permissions);
        }
        if case.setup.uses_configured_policy_payment_asset() {
            crate::sns::seed_default_namespace_policies_for_payment_asset(
                &mut world,
                &payment_asset_definition_id.to_string(),
            );
        } else {
            crate::sns::seed_default_namespace_policies(&mut world);
        }
        (world, Some(retail_domain_id))
    } else {
        let domain = Domain::new(fixture_domain_id()).build(&authority);
        (
            World::with_assets([domain], accounts, [asset_definition], [source_asset], []),
            None,
        )
    };
    // The original signed genesis authenticates these global-root alias cases;
    // alias permissions and asset balances remain independent fixture inputs.
    let (paynet, catalog) = retail_dataspace_catalog();
    let mut config = crate::sumeragi::test_chain::TestChainConfig::new(world, 0);
    config.chain_id = ChainId::from("test-chain");
    let mut nexus = iroha_config::parameters::actual::Nexus {
        dataspace_catalog: catalog.clone(),
        configured_dataspace_catalog: catalog,
        ..Default::default()
    };
    // Alias authorization is independent of monetary admission. Bind the
    // zero-fee policy and catalog before preparing this exact signed genesis.
    nexus.fees.base_fee = 0_u32.into();
    nexus.fees.per_byte_fee = 0_u32.into();
    nexus.fees.per_instruction_fee = 0_u32.into();
    nexus.fees.per_gas_unit_fee = 0_u32.into();
    let fee_parameter = iroha_data_model::parameter::Parameter::Custom(
        nexus
            .fees
            .clone()
            .into_custom_parameter()
            .expect("canonical signed fixture fee policy"),
    );
    if let Some(slot) = config.genesis_parameters.iter_mut().find(|parameter| {
        matches!(
            parameter,
            iroha_data_model::parameter::Parameter::Custom(value)
                if value.id() == &iroha_data_model::block::consensus::RootFeePolicy::parameter_id()
        )
    }) {
        *slot = fee_parameter;
    } else {
        config.genesis_parameters.push(fee_parameter);
    }
    config.nexus = Some(nexus);
    let state = signed_contract_host_state(config);
    AliasContractCaseState {
        state,
        authority,
        merchant_account,
        replacement_account,
        asset_definition_id,
        source_asset_id,
        paynet,
        retail_domain_id,
    }
}
fn grant_test_alias_permission(
    fixture: &AliasContractCaseState,
    tx: &mut StateTransaction<'_, '_>,
    resolve: bool,
    domain: bool,
) {
    let scope = if domain {
        AccountAliasPermissionScope::Domain(
            fixture
                .retail_domain_id
                .as_ref()
                .expect("domain-qualified fixture domain")
                .clone(),
        )
    } else {
        AccountAliasPermissionScope::Dataspace(fixture.paynet)
    };
    let permission = if resolve {
        Permission::from(CanResolveAccountAlias { scope })
    } else {
        Permission::from(CanManageAccountAlias { scope })
    };
    tx.world
        .add_account_permission(&fixture.authority, permission);
}
fn seed_alias_contract_case(
    case: &AliasContractCaseV1,
    fixture: &AliasContractCaseState,
) -> Option<AccountAlias> {
    let header = BlockHeader::new(
        nonzero!(2_u64),
        fixture.state.view().latest_block_hash(),
        None,
        0,
        0,
    );
    let mut block = fixture.state.block(header);
    let mut tx = block.transaction();
    if let Some(seed) = case.setup_seed {
        seed_test_call_hash(&mut tx, seed);
    }
    tx.nexus.dataspace_catalog = fixture.state.nexus.read().dataspace_catalog.clone();
    tx.world.dataspace_catalog = fixture.state.nexus.read().dataspace_catalog.clone();
    let mut rebound_alias = None;
    match case.setup {
        AliasContractSetupV1::DomainlessCurrent
        | AliasContractSetupV1::DomainlessWithoutPermission => {
            let alias =
                AccountAlias::domainless("merchant".parse().expect("alias label"), fixture.paynet);
            if case.setup == AliasContractSetupV1::DomainlessCurrent {
                grant_test_alias_permission(fixture, &mut tx, true, false);
            }
            grant_test_alias_permission(fixture, &mut tx, false, false);
            EnsureTestAccountAliasBinding::bind(
                fixture
                    .merchant_account
                    .as_ref()
                    .expect("merchant fixture")
                    .clone(),
                alias.clone(),
                None,
            )
            .execute(&fixture.authority, &mut tx)
            .expect("bind merchant alias");
            if case.setup == AliasContractSetupV1::DomainlessCurrent {
                rebound_alias = Some(alias);
            }
        }
        AliasContractSetupV1::DomainlessWithoutBinding => {
            grant_test_alias_permission(fixture, &mut tx, true, false);
        }
        AliasContractSetupV1::DomainCurrent
        | AliasContractSetupV1::DomainWithoutPermission
        | AliasContractSetupV1::DomainPermissionOnly
        | AliasContractSetupV1::DomainWithoutBinding => {
            let alias = AccountAlias::new(
                "merchant".parse().expect("alias label"),
                Some(AccountAliasDomain::new(
                    "bank".parse().expect("alias domain name"),
                )),
                fixture.paynet,
            );
            tx.tx_call_hash = Some(Hash::prehashed(
                [case.transaction_hash_byte
                    .expect("sealed transaction hash byte"); Hash::LENGTH],
            ));
            if matches!(
                case.setup,
                AliasContractSetupV1::DomainWithoutPermission
                    | AliasContractSetupV1::DomainPermissionOnly
            ) {
                grant_test_alias_permission(fixture, &mut tx, false, true);
            }
            let lease_owner = if case.setup == AliasContractSetupV1::DomainWithoutBinding {
                fixture.authority.clone()
            } else {
                fixture
                    .merchant_account
                    .as_ref()
                    .expect("merchant fixture")
                    .clone()
            };
            SeedTestAccountAliasLease::new(
                alias.clone(),
                lease_owner,
                fixture.authority.clone(),
                1,
                None,
            )
            .execute(&fixture.authority, &mut tx)
            .expect("acquire domain-qualified alias lease");
            if case.setup != AliasContractSetupV1::DomainPermissionOnly {
                grant_test_alias_permission(fixture, &mut tx, true, false);
            }
            if case.setup != AliasContractSetupV1::DomainWithoutPermission {
                grant_test_alias_permission(fixture, &mut tx, true, true);
            }
            if case.setup != AliasContractSetupV1::DomainWithoutBinding {
                grant_test_alias_permission(fixture, &mut tx, false, false);
                grant_test_alias_permission(fixture, &mut tx, false, true);
                EnsureTestAccountAliasBinding::bind(
                    fixture
                        .merchant_account
                        .as_ref()
                        .expect("merchant fixture")
                        .clone(),
                    alias.clone(),
                    None,
                )
                .execute(&fixture.authority, &mut tx)
                .expect("bind merchant alias");
            }
            if case.setup == AliasContractSetupV1::DomainCurrent {
                rebound_alias = Some(alias);
            }
        }
        AliasContractSetupV1::Invalid => {}
    }
    tx.apply();
    block
        .commit_world_overlay_for_testing()
        .expect(match case.setup {
            AliasContractSetupV1::DomainlessWithoutBinding
            | AliasContractSetupV1::DomainWithoutBinding => "commit alias permissions",
            AliasContractSetupV1::Invalid => "commit catalog",
            _ => "commit alias setup",
        });
    rebound_alias
}
fn install_alias_contract_case(
    case: &AliasContractCaseV1,
    fixture: &AliasContractCaseState,
) -> ContractAddress {
    let contract = case.direct_source.as_deref().map_or_else(
        || {
            install_alias_payout_contract(
                &fixture.state,
                &fixture.authority,
                &case.recipient_expression,
                0,
            )
        },
        |source| {
            let contract = install_contract(&fixture.state, &fixture.authority, source, 0);
            grant_asset_ops_to_account(&fixture.state, &fixture.authority, &(fixture.authority));
            contract
        },
    );
    // Every case gets only the exact asset permission needed after lookup.
    // Missing alias resolution permissions remain deliberate test inputs.
    grant_test_asset_transfer(
        &fixture.state,
        contract.subject_id(),
        fixture.source_asset_id.clone(),
    );
    contract
}
fn bind_alias_contract_asset(
    fixture: &AliasContractCaseState,
    contract: &ContractAddress,
    ivm_cache: &mut crate::smartcontracts::ivm::cache::IvmCache,
) {
    let bind_payload = Json::from_str_norito(&format!(
        r#"{{"settlement_asset":"{}"}}"#,
        fixture.asset_definition_id
    ))
    .expect("bind payload");
    execute_contract_call_transaction(
        &fixture.state,
        &fixture.authority,
        &fixture_signing_keypair(&fixture.authority),
        contract_invocation_from_json(&fixture.state, contract.clone(), "bind", &bind_payload),
        ivm_cache,
    );
}
fn alias_pay_payload() -> Json {
    Json::from_str_norito(r#"{"amount":"2"}"#).expect("pay payload")
}
fn alias_asset_balance(view: &StateView<'_>, asset_id: &AssetId, expected: &str) -> Quantity {
    view.world()
        .asset(asset_id)
        .expect(expected)
        .value()
        .as_ref()
        .clone()
}
fn assert_alias_current_binding(
    case: &AliasContractCaseV1,
    fixture: &AliasContractCaseState,
    contract: ContractAddress,
    alias: AccountAlias,
    ivm_cache: &mut crate::smartcontracts::ivm::cache::IvmCache,
) {
    let pay_payload = alias_pay_payload();
    execute_contract_call_transaction(
        &fixture.state,
        &fixture.authority,
        &fixture_signing_keypair(&fixture.authority),
        contract_invocation_from_json(&fixture.state, contract.clone(), "pay", &pay_payload),
        ivm_cache,
    );
    let merchant_account = fixture.merchant_account.as_ref().expect("merchant fixture");
    let replacement_account = fixture
        .replacement_account
        .as_ref()
        .expect("replacement fixture");
    let after_first = fixture.state.view();
    let merchant_asset_id = AssetId::of(
        fixture.asset_definition_id.clone(),
        merchant_account.clone(),
    );
    let replacement_asset_id = AssetId::of(
        fixture.asset_definition_id.clone(),
        replacement_account.clone(),
    );
    let merchant_balance = alias_asset_balance(
        &after_first,
        &merchant_asset_id,
        "merchant asset exists after first payout",
    );
    assert_eq!(merchant_balance, Quantity::from(2_u32));
    assert!(
        after_first.world().asset(&replacement_asset_id).is_err(),
        "replacement account should not receive funds before the alias is rebound",
    );
    let next_height = u64::try_from(fixture.state.view().height() + 1)
        .ok()
        .and_then(core::num::NonZeroU64::new)
        .expect("next block height");
    let mut block = fixture
        .state
        .block(BlockHeader::new(next_height, None, None, 0, 0));
    let mut tx = block.transaction();
    if let Some(seed) = case.rebind_seed {
        seed_test_call_hash(&mut tx, seed);
    }
    EnsureTestAccountAliasBinding::bind(replacement_account.clone(), alias, None)
        .execute(&fixture.authority, &mut tx)
        .expect("rebind merchant alias");
    tx.apply();
    block
        .commit_world_overlay_for_testing()
        .expect("commit alias rebind");
    let pay_payload = Json::from_str_norito(r#"{"amount":"1"}"#).expect("pay payload");
    execute_contract_call_transaction(
        &fixture.state,
        &fixture.authority,
        &fixture_signing_keypair(&fixture.authority),
        contract_invocation_from_json(&fixture.state, contract, "pay", &pay_payload),
        ivm_cache,
    );
    let final_view = fixture.state.view();
    let authority_balance = alias_asset_balance(
        &final_view,
        &fixture.source_asset_id,
        "authority asset remains",
    );
    let merchant_balance =
        alias_asset_balance(&final_view, &merchant_asset_id, "merchant asset remains");
    let replacement_balance = alias_asset_balance(
        &final_view,
        &replacement_asset_id,
        "replacement asset exists",
    );
    assert_eq!(authority_balance, Quantity::from(2_u32));
    assert_eq!(merchant_balance, Quantity::from(2_u32));
    assert_eq!(replacement_balance, Quantity::from(1_u32));
}
fn assert_alias_single_payment(
    case: &AliasContractCaseV1,
    fixture: &AliasContractCaseState,
    contract: &ContractAddress,
    ivm_cache: &mut crate::smartcontracts::ivm::cache::IvmCache,
) {
    let pay_payload = alias_pay_payload();
    let result = execute_contract_call_transaction_result(
        &fixture.state,
        &fixture.authority,
        &fixture_signing_keypair(&fixture.authority),
        contract_invocation_from_json(&fixture.state, contract.clone(), "pay", &pay_payload),
        ivm_cache,
    );
    if case.setup == AliasContractSetupV1::DomainPermissionOnly {
        result.expect(&case.messages[0]);
        let view = fixture.state.view();
        let authority_balance =
            alias_asset_balance(&view, &fixture.source_asset_id, "authority asset remains");
        let merchant_asset_id = AssetId::of(
            fixture.asset_definition_id.clone(),
            fixture
                .merchant_account
                .as_ref()
                .expect("merchant fixture")
                .clone(),
        );
        let merchant_balance = alias_asset_balance(&view, &merchant_asset_id, &case.messages[1]);
        assert_eq!(authority_balance, Quantity::from(3_u32));
        assert_eq!(merchant_balance, Quantity::from(2_u32));
        return;
    }
    let result_message = &case.messages[0];
    if matches!(
        case.setup,
        AliasContractSetupV1::DomainlessWithoutPermission
            | AliasContractSetupV1::DomainWithoutPermission
    ) {
        use iroha_data_model::executor::fault::{
            IvmFaultKindV1, IvmFaultPositionV1, IvmInvocationSelectorV1,
        };
        let Err(crate::execution_attempt::ExecutionAttemptError::Rejected(
            iroha_data_model::ValidationFail::IvmFault(fault),
        )) = &result
        else {
            panic!("{result_message}: {result:?}");
        };
        assert_eq!(
            fault.kind,
            IvmFaultKindV1::PermissionDenied,
            "{result_message}"
        );
        let view = fixture.state.view();
        let identity = crate::smartcontracts::code::fetch_bound_contract_identity(&view, contract)
            .expect("read exact alias fixture binding")
            .expect("alias fixture remains installed");
        let artifact = ContractArtifactId::new(
            contract.dataspace_id().expect("fixture dataspace"),
            identity.code_hash,
        );
        let selector = view
            .world()
            .contract_manifests()
            .get(&artifact)
            .expect("authenticated fixture manifest")
            .entrypoints
            .as_ref()
            .expect("fixture entrypoint table")
            .iter()
            .position(|entry| entry.name == "pay")
            .expect("fixture pay selector");
        assert_eq!(fault.site.code_hash, identity.code_hash);
        assert_eq!(
            fault.site.selector,
            IvmInvocationSelectorV1::Entrypoint(u32::try_from(selector).unwrap()),
        );
        assert!(matches!(
            fault.site.position,
            IvmFaultPositionV1::Execute { .. }
        ));
    } else {
        assert!(result.is_err(), "{result_message}");
    }
    let view = fixture.state.view();
    let authority_balance =
        alias_asset_balance(&view, &fixture.source_asset_id, "authority asset remains");
    assert_eq!(authority_balance, Quantity::from(5_u32));
    if let Some(merchant_message) = case.messages.get(1) {
        let merchant_asset_id = AssetId::of(
            fixture.asset_definition_id.clone(),
            fixture
                .merchant_account
                .as_ref()
                .expect("merchant fixture")
                .clone(),
        );
        assert!(
            view.world().asset(&merchant_asset_id).is_err(),
            "{merchant_message}",
        );
    }
}
fn run_alias_contract_case(case_index: usize) {
    let case = &ALIAS_CONTRACT_CASES.cases[case_index];
    assert_eq!(case.id, ALIAS_CONTRACT_CASE_IDS[case_index]);
    let fixture = build_alias_contract_case_state(case);
    let alias = seed_alias_contract_case(case, &fixture);
    let contract = install_alias_contract_case(case, &fixture);
    let mut ivm_cache = crate::smartcontracts::ivm::cache::IvmCache::new();
    bind_alias_contract_asset(&fixture, &contract, &mut ivm_cache);
    if case.setup.is_current() {
        assert_alias_current_binding(
            case,
            &fixture,
            contract,
            alias.expect("current-binding alias fixture"),
            &mut ivm_cache,
        );
    } else {
        assert_alias_single_payment(case, &fixture, &(contract), &mut ivm_cache);
    }
}
#[test]
fn contract_call_transaction_account_id_alias_shorthand_uses_current_binding() {
    run_alias_contract_case(0);
}
#[test]
fn contract_call_transaction_resolve_account_alias_builtin_uses_current_binding() {
    run_alias_contract_case(1);
}
#[test]
fn contract_call_transaction_domain_qualified_resolve_account_alias_builtin_uses_current_binding() {
    run_alias_contract_case(2);
}
#[test]
fn contract_call_transaction_resolve_account_alias_builtin_without_permission_is_rejected() {
    run_alias_contract_case(3);
}
#[test]
fn contract_call_transaction_resolve_account_alias_builtin_without_binding_is_rejected() {
    run_alias_contract_case(4);
}
#[test]
fn contract_call_transaction_resolve_account_alias_builtin_invalid_literal_is_rejected() {
    run_alias_contract_case(5);
}
#[test]
fn contract_call_transaction_domain_qualified_resolve_account_alias_builtin_without_domain_permission_is_rejected()
 {
    run_alias_contract_case(6);
}
#[test]
fn contract_call_transaction_account_id_alias_shorthand_without_binding_is_rejected() {
    run_alias_contract_case(7);
}
#[test]
fn contract_call_transaction_account_id_alias_shorthand_invalid_literal_is_rejected() {
    run_alias_contract_case(8);
}
#[test]
fn contract_call_transaction_domain_qualified_account_id_alias_shorthand_without_domain_permission_is_rejected()
 {
    run_alias_contract_case(9);
}
#[test]
fn contract_call_transaction_domain_qualified_account_id_alias_shorthand_accepts_domain_permission_without_dataspace_permission()
 {
    run_alias_contract_case(10);
}
#[test]
fn contract_call_transaction_domain_qualified_account_id_alias_shorthand_without_binding_is_rejected()
 {
    run_alias_contract_case(11);
}
#[test]
fn contract_call_transaction_domain_qualified_resolve_account_alias_builtin_without_binding_is_rejected()
 {
    run_alias_contract_case(12);
}
#[test]
fn contract_call_transaction_domain_qualified_resolve_account_alias_builtin_accepts_domain_permission_without_dataspace_permission()
 {
    run_alias_contract_case(13);
}
#[test]
fn contract_call_transaction_domain_qualified_resolve_account_alias_builtin_invalid_literal_is_rejected()
 {
    run_alias_contract_case(14);
}
#[test]
fn contract_call_transaction_domain_qualified_account_id_alias_shorthand_invalid_literal_is_rejected()
 {
    run_alias_contract_case(15);
}
#[test]
fn contract_call_transaction_domain_qualified_account_id_alias_shorthand_uses_current_binding() {
    run_alias_contract_case(16);
}
#[test]
fn host_execution_artifacts_apply_queued_uses_queued_authority_snapshot() {
    let outer_authority = fixture_account("alice");
    let nested_authority = fixture_account("bob");
    let recipient = fixture_account("carol");
    let domain = Domain::new(fixture_domain_id()).build(&outer_authority);
    let outer_account = build_fixture_account(&outer_authority, &outer_authority);
    let nested_account = build_fixture_account(&nested_authority, &outer_authority);
    let recipient_account = build_fixture_account(&recipient, &outer_authority);
    let asset_def_id: AssetDefinitionId = AssetDefinitionId::derive_from_components(
        fixture_domain_id(),
        "rose".parse().expect("asset name"),
    );
    let asset_def = AssetDefinition::numeric(
        asset_def_id.clone(),
        "rose".to_owned(),
        iroha_data_model::asset::AssetBalancePolicy::Global,
        None,
    )
    .build(&outer_authority);
    let source_asset_id = AssetId::of(asset_def_id.clone(), nested_authority.clone());
    let source_asset = Asset::new(source_asset_id.clone(), Quantity::from(5_u32));
    let world = World::with_assets(
        [domain],
        [outer_account, nested_account, recipient_account],
        [asset_def],
        [source_asset],
        [],
    );
    let state =
        signed_contract_host_state(crate::sumeragi::test_chain::TestChainConfig::new(world, 0));
    let header = BlockHeader::new(
        nonzero!(2_u64),
        state.view().latest_block_hash(),
        None,
        0,
        0,
    );
    let mut block = state.block(header);
    let source = artifact_test_signed_root(*block.network_id(), &outer_authority);
    let source_call = Hash::from(source.hash_as_entrypoint());
    let mut stx = block.transaction_for_fastpq_testing(source_call);
    let artifacts = HostExecutionArtifacts {
        x509_queue_backing: None,
        queued: vec![QueuedEffect {
            payload: crate::smartcontracts::ivm::host::QueuedEffectPayload::Instruction(
                InstructionBox::from(Transfer::asset_quantity(
                    source_asset_id.clone(),
                    1_u32,
                    recipient.clone(),
                )),
            ),
            authority: nested_authority.clone(),
            contract_runtime_context: None,
            entrypoint_authorization: None,
        }],
        entrypoint_authorization: None,
        confidential_gas_delta: 0,
        completed_axt: Vec::new(),
        durable_state_overlay: BTreeMap::new(),
        durable_state_authorizations: BTreeMap::new(),
    };
    let grouped = artifacts.queued_instructions_by_authority();
    assert_eq!(grouped.len(), 1);
    assert_eq!(grouped.get(&nested_authority).map(Vec::len), Some(1));
    assert!(!grouped.contains_key(&outer_authority));
    bind_artifact_test_source_root(&mut stx, &source);
    assert_eq!(stx.tx_call_hash, Some(source_call));
    artifacts
        .apply_to_transaction(&mut stx, &outer_authority)
        .expect("queued instruction should execute under queued authority");
    stx.finish_execution_effect_budget()
        .expect("close artifact test root");
    let source_balance = stx
        .world
        .asset(&source_asset_id)
        .expect("source asset remains")
        .value()
        .clone();
    let recipient_asset_id = AssetId::of(asset_def_id, recipient);
    let recipient_balance = stx
        .world
        .asset(&recipient_asset_id)
        .expect("recipient asset created")
        .value()
        .clone();
    assert_eq!(source_balance.as_ref(), &Quantity::from(4_u32));
    assert_eq!(recipient_balance.as_ref(), &Quantity::from(1_u32));
}
#[test]
fn rejected_queued_confidential_instruction_retains_host_artifact_gas() {
    let authority = fixture_account("alice");
    let state = contract_test_state(&authority);
    let fixture = crate::zk::test_utils::native_framing_fixture_envelope(
        crate::zk::confidential_v2::CONFIDENTIAL_TRANSFER_V2_CIRCUIT_ID,
        [0_u8; 32],
    );
    let proof = fixture.proof_box("pipa-r/pasta");
    let instruction: InstructionBox = iroha_data_model::isi::zk::VerifyProof::new(
        iroha_data_model::proof::ProofAttachment::new_ref(
            "pipa-r/pasta".into(),
            proof,
            iroha_data_model::proof::VerifyingKeyId::new(
                "pipa-r/pasta",
                "missing-host-artifact-vk",
            ),
        ),
    )
    .into();
    let confidential_gas_delta =
        crate::gas::confidential_gas_cost(&instruction, &crate::gas::DEFAULT_ZK_GAS)
            .expect("completed fixture gas quote");
    assert!(confidential_gas_delta > 0);
    let artifacts = HostExecutionArtifacts {
        x509_queue_backing: None,
        queued: vec![QueuedEffect {
            payload: crate::smartcontracts::ivm::host::QueuedEffectPayload::Instruction(
                instruction.clone(),
            ),
            authority: authority.clone(),
            contract_runtime_context: None,
            entrypoint_authorization: None,
        }],
        entrypoint_authorization: None,
        confidential_gas_delta,
        completed_axt: Vec::new(),
        durable_state_overlay: BTreeMap::new(),
        durable_state_authorizations: BTreeMap::new(),
    };
    let next_height = u64::try_from((state.view().height() + 1).max(2))
        .ok()
        .and_then(core::num::NonZeroU64::new)
        .expect("next block height");
    let mut block = state.block(BlockHeader::new(next_height, None, None, 0, 0));
    let mut transaction = block.transaction();

    bind_artifact_test_signed_root(&mut transaction, &authority);
    let error = artifacts
        .apply_to_transaction(&mut transaction, &authority)
        .expect_err("the proof references a missing verifying key");
    assert!(
        matches!(error,
            ValidationFail::InstructionFailed(
                iroha_data_model::isi::error::InstructionExecutionError::InvariantViolation(ref message)
            ) if message.contains("registered verifying key reference")
        ),
        "{error:?}"
    );
    transaction
        .finish_execution_effect_budget()
        .expect("close artifact test root");

    // Missing-key rejection precedes completed verification registration.
    assert_eq!(transaction.zk_confidential_ops_in_tx, 0);
    assert_eq!(transaction.zk_verify_calls_in_tx, 0);
    assert_eq!(
        transaction.confidential_gas_used_in_tx, confidential_gas_delta,
        "host-artifact gas must be retained before queued execution can reject"
    );
    drop(transaction);

    let mut mutable_host = CoreHost::new(authority.clone());
    mutable_host
        .queue_instruction(instruction)
        .expect("completed fixture quote must enqueue before execution rejects the missing key");
    let mut mutable_host_transaction = block.transaction();
    bind_artifact_test_signed_root(&mut mutable_host_transaction, &authority);
    let error = mutable_host
        .apply_queued(&mut mutable_host_transaction, &authority)
        .expect_err("the queued proof references a missing verifying key");
    assert!(
        matches!(error,
            ValidationFail::InstructionFailed(
                iroha_data_model::isi::error::InstructionExecutionError::InvariantViolation(ref message)
            ) if message.contains("registered verifying key reference")
        ),
        "{error:?}"
    );
    mutable_host_transaction
        .finish_execution_effect_budget()
        .expect("close artifact test root");
    assert_eq!(mutable_host_transaction.zk_confidential_ops_in_tx, 0);
    assert_eq!(mutable_host_transaction.zk_verify_calls_in_tx, 0);
    assert_eq!(
        mutable_host_transaction.confidential_gas_used_in_tx, confidential_gas_delta,
        "mutable-host gas must be retained before queued execution can reject"
    );
}
#[test]
fn actual_host_artifacts_require_signed_root_before_first_effect() {
    let authority = fixture_account("alice");
    for bind_root in [false, true] {
        let state = contract_test_state(&authority);
        let mut host = CoreHost::new(authority.clone());
        host.set_generic_execution();
        let mut program = ivm::ProgramMetadata::default().encode();
        program.extend_from_slice(&ivm::encoding::wide::encode_halt().to_le_bytes());
        let mut vm = IVM::new(1_000_000);
        vm.load_program(&program)
            .expect("load valid generic Halt program");
        let account_ptr = store_tlv(&mut vm, PointerType::AccountId, &norito_blob(&authority));
        let key: Name = "artifact_root_required".parse().unwrap();
        let key_ptr = store_tlv(&mut vm, PointerType::Name, &norito_blob(&key));
        let value_ptr = store_tlv(&mut vm, PointerType::Json, &norito_blob(&Json::new(true)));
        vm.set_register(10, account_ptr);
        vm.set_register(11, key_ptr);
        vm.set_register(12, value_ptr);
        host.syscall(ivm_sys::SYSCALL_SET_ACCOUNT_DETAIL, &mut vm)
            .expect("actual generic syscall produces the queued artifact");
        let artifacts = host
            .into_execution_artifacts(None)
            .expect("export actual artifact");
        assert_eq!(artifacts.queued_instructions().len(), 1);
        // This is ordinary component artifact execution against the fixture's
        // original genesis, not a fabricated genesis execution capability.
        let next_height =
            core::num::NonZeroU64::new(u64::try_from(state.view().height() + 1).unwrap()).unwrap();
        let mut block = state.block(BlockHeader::new(next_height, None, None, 0, 0));
        let fragments = block.committed_fragment_count();
        let mut tx = block.transaction();
        if bind_root {
            bind_artifact_test_signed_root(&mut tx, &authority);
        }
        let result = artifacts.apply_to_transaction(&mut tx, &authority);
        if bind_root {
            result.expect("otherwise valid real artifact applies with its fixture root");
            tx.finish_execution_effect_budget()
                .expect("close actual artifact fixture root");
            assert_eq!(
                tx.world.account(&authority).unwrap().metadata().get(&key),
                Some(&Json::new(true))
            );
        } else {
            let error =
                result.expect_err("missing signed-root owner must refuse before any effect");
            assert!(
                matches!(error, ValidationFail::InternalError(ref reason)
                    if reason == "instruction effects have no exact signed-root budget"),
                "{error:?}"
            );
            assert!(
                tx.world
                    .account(&authority)
                    .unwrap()
                    .metadata()
                    .get(&key)
                    .is_none()
            );
            assert!(
                !tx.execution_effect_limit_exceeded(),
                "missing authority is not a consensus capacity limit"
            );
            assert!(
                tx.finish_execution_effect_budget().is_err(),
                "owner refusal remains sticky"
            );
        }
        drop(tx);
        assert_eq!(block.committed_fragment_count(), fragments);
        assert!(
            block
                .world
                .account(&authority)
                .unwrap()
                .metadata()
                .get(&key)
                .is_none()
        );
    }
}

#[test]
fn host_execution_artifacts_reject_foreign_durable_path_before_any_write() {
    let authority = fixture_account("alice");
    let state = contract_test_state(&authority);
    let contract_address = install_contract(
        &state,
        &authority,
        r"
seiyaku DurableOwner {
  view fn inspect() authorize(anyone) -> int { return 0; }
}
",
        411,
    );
    let identity = crate::smartcontracts::code::fetch_bound_contract_identity(
        &state.view(),
        &contract_address,
    )
    .expect("registry read completes")
    .expect("installed contract identity");
    let authorization = ContractEntrypointAuthorizationSnapshot::new(
        authority.clone(),
        "inspect".to_owned(),
        iroha_data_model::smart_contract::manifest::EntrypointAuthorizationV1::Anyone,
        &identity,
    );
    let foreign_digest = hex::encode(Hash::new(b"foreign durable owner").as_ref());
    let foreign_path: StatePath = format!("sc/{foreign_digest}/counter")
        .parse()
        .expect("foreign durable path");
    assert!(!authorization.owns_durable_state_path(&foreign_path));
    let artifacts = HostExecutionArtifacts {
        x509_queue_backing: None,
        queued: Vec::new(),
        entrypoint_authorization: Some(authorization.clone()),
        confidential_gas_delta: 0,
        completed_axt: Vec::new(),
        durable_state_overlay: BTreeMap::from([(foreign_path.clone(), Some(vec![1]))]),
        durable_state_authorizations: BTreeMap::from([(foreign_path.clone(), Some(authorization))]),
    };
    let next_height = u64::try_from((state.view().height() + 1).max(2))
        .ok()
        .and_then(core::num::NonZeroU64::new)
        .expect("next block height");
    let mut block = state.block(BlockHeader::new(next_height, None, None, 0, 0));
    let mut transaction = block.transaction();
    let error = artifacts
        .apply_to_transaction(&mut transaction, &authority)
        .expect_err("foreign durable path must fail closed");
    assert!(
        error.to_string().contains("does not belong"),
        "unexpected durable ownership error: {error}"
    );
    assert!(
        transaction
            .world
            .smart_contract_state
            .get(&foreign_path)
            .is_none(),
        "foreign durable authorization must apply zero writes"
    );
}
#[test]
fn identical_queued_artifacts_do_not_reuse_call_hash_across_committed_fragments() {
    let authority = fixture_account("alice");
    let queued = vec![QueuedEffect {
        payload: crate::smartcontracts::ivm::host::QueuedEffectPayload::Instruction(
            InstructionBox::from(Log::new(
                iroha_logger::Level::INFO,
                "identical internal artifact".to_owned(),
            )),
        ),
        authority,
        contract_runtime_context: None,
        entrypoint_authorization: None,
    }];
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let state = State::new_for_testing(World::new(), kura, query);
    let header = BlockHeader::new(nonzero!(1_u64), None, None, 0, 0);
    let mut block = state.block(header);
    let first_hash = {
        let mut transaction = block.transaction();
        HostExecutionArtifacts::seed_queued_call_hash_if_missing(&mut transaction, &queued)
            .expect("seed first queued call hash");
        let hash = transaction.tx_call_hash.expect("first call hash");
        transaction.apply();
        hash
    };
    let second_hash = {
        let mut transaction = block.transaction();
        HostExecutionArtifacts::seed_queued_call_hash_if_missing(&mut transaction, &queued)
            .expect("seed second queued call hash");
        let hash = transaction.tx_call_hash.expect("second call hash");
        transaction.apply();
        hash
    };
    assert_ne!(
        first_hash, second_hash,
        "the committed-fragment position must distinguish identical internal executions"
    );
}
pub(super) fn begin_axt_envelope(
    host: &mut CoreHost,
    vm: &mut IVM,
    descriptor: &axt::AxtDescriptor,
) {
    let descriptor_bytes = norito_blob(descriptor);
    let desc_ptr = store_tlv(vm, PointerType::AxtDescriptor, &descriptor_bytes);
    vm.set_register(10, desc_ptr);
    assert_eq!(
        (host.syscall(ivm_sys::SYSCALL_AXT_BEGIN, vm)).expect("expected successful fixture result"),
        CoreHost::axt_gas(descriptor_bytes.len()),
        "AXT_BEGIN should seed envelope state"
    );
}
#[test]
fn backend_label_prefers_stored_vk_backend_and_prefix() {
    let id = VerifyingKeyId::new("pipa-r/pasta", "test_circuit");
    let mut rec = VerifyingKeyRecord::new_with_owner(
        1,
        "halo2/ipa:test_circuit",
        None,
        "ns",
        BackendTag::NativePipaRPasta,
        "vesta",
        [0u8; 32],
        [1u8; 32],
    );
    // When no stored VK bytes are present, use the registry key backend.
    assert_eq!(
        CoreHost::backend_label_for_record(&id, &rec),
        "pipa-r/pasta".to_string()
    );
    // Stored VK backend must override the circuit prefix.
    rec.key = Some(VerifyingKeyBox::new("custom/backend".into(), vec![1, 2, 3]));
    assert_eq!(
        CoreHost::backend_label_for_record(&id, &rec),
        "custom/backend".to_string()
    );
}
#[test]
fn set_verifying_keys_accepts_keyless_record_backend_from_registry_key() {
    let mut host = CoreHost::new(fixture_account("alice"));
    let commitment = [0x77; 32];
    let mut rec = active_vk_record(
        commitment,
        schema_hash(crate::zk::confidential_v2::CONFIDENTIAL_TRANSFER_V2_PUBLIC_INPUTS_SCHEMA_V1),
        "pipa-r/pasta",
        crate::zk::confidential_v2::CONFIDENTIAL_TRANSFER_V2_CIRCUIT_ID,
        "core",
        Vec::new(),
    );
    rec.key = None;
    let mut map = BTreeMap::new();
    map.insert(
        VerifyingKeyId::new("pipa-r/pasta", "ivm-execution-keyless"),
        rec,
    );
    host.set_verifying_keys(&(map))
        .expect("keyless record should use the registry backend label");
}
#[test]
fn load_vk_record_any_namespace_uses_keyless_registry_backend() {
    let mut host = CoreHost::new(fixture_account("alice"));
    let commitment = [0x78; 32];
    let mut rec = active_vk_record(
        commitment,
        schema_hash(crate::zk::confidential_v2::CONFIDENTIAL_TRANSFER_V2_PUBLIC_INPUTS_SCHEMA_V1),
        "pipa-r/pasta",
        crate::zk::confidential_v2::CONFIDENTIAL_TRANSFER_V2_CIRCUIT_ID,
        "core",
        Vec::new(),
    );
    rec.key = None;
    let mut map = BTreeMap::new();
    map.insert(
        VerifyingKeyId::new("pipa-r/pasta", "ivm-execution-keyless"),
        rec,
    );
    host.set_verifying_keys(&(map))
        .expect("keyless record should be accepted");
    let prepared = host
        .load_vk_record_any_namespace(commitment)
        .expect("commitment should resolve");
    assert_eq!(prepared.backend_label.as_ref(), "pipa-r/pasta");
}
#[test]
fn set_verifying_keys_rejects_malformed_native_material_during_rehydration() {
    let backend = "pipa-r/pasta";
    let mut vk_bytes = b"ZK1\0CID1".to_vec();
    vk_bytes.extend_from_slice(&u32::MAX.to_le_bytes());
    let commitment = CoreHost::hash_vk_bytes(backend, &vk_bytes);
    let record = active_vk_record(
        commitment,
        schema_hash(crate::zk::confidential_v2::CONFIDENTIAL_TRANSFER_V2_PUBLIC_INPUTS_SCHEMA_V1),
        backend,
        crate::zk::confidential_v2::CONFIDENTIAL_TRANSFER_V2_CIRCUIT_ID,
        "core",
        vk_bytes,
    );
    let mut host = CoreHost::new(fixture_account("alice"));
    assert!(
        host.set_verifying_keys(
            &(BTreeMap::from([(VerifyingKeyId::new(backend, "malformed-halo2"), record,)]))
        )
        .is_err(),
        "state hydration must run the strict native registration validator"
    );
    assert!(host.prepared_verifying_keys.is_empty());
}
#[cfg(feature = "zk-stark")]
#[test]
fn set_verifying_keys_rejects_weak_stark_parameters_during_rehydration() {
    let backend = "stark/fri/poseidon-x7-goldilocks-6x64-v1";
    let circuit_id = "stark/fri/poseidon-x7-goldilocks-6x64-v1:weak-rehydrated-key";
    let payload = crate::zk_stark::StarkFriVerifyingKeyV1 {
        version: 1,
        circuit_id: circuit_id.to_owned(),
        n_log2: crate::zk_stark::STARK_FRI_CONSENSUS_MIN_N_LOG2,
        blowup_log2: crate::zk_stark::STARK_FRI_CONSENSUS_MIN_BLOWUP_LOG2,
        fold_arity: 2,
        queries: crate::zk_stark::STARK_FRI_CONSENSUS_MIN_QUERIES - 1,
        merkle_arity: 2,
    };
    let vk_bytes = norito::encode_canonical(&payload).expect("encode weak STARK key");
    let commitment = CoreHost::hash_vk_bytes(backend, &vk_bytes);
    let record = active_vk_record(
        commitment,
        schema_hash(crate::zk::confidential_v2::CONFIDENTIAL_TRANSFER_V2_PUBLIC_INPUTS_SCHEMA_V1),
        backend,
        circuit_id,
        "core",
        vk_bytes,
    );
    let mut host = CoreHost::new(fixture_account("alice"));
    assert!(
        host.set_verifying_keys(
            &(BTreeMap::from([(VerifyingKeyId::new(backend, "weak-stark"), record,)]))
        )
        .is_err(),
        "state hydration must enforce the STARK consensus parameter floor"
    );
    assert!(host.prepared_verifying_keys.is_empty());
}
#[test]
fn prepared_vk_index_caches_ipa_metadata_and_revalidates_changed_bytes() {
    let mut host = CoreHost::new(fixture_account("alice"));
    let backend = "pipa-r/pasta";
    let vk_bytes = canonical_confidential_transfer_vk_bytes();
    let commitment = CoreHost::hash_vk_bytes(backend, &vk_bytes);
    let id = VerifyingKeyId::new(backend, "cached-vk");
    let rec = active_vk_record(
        commitment,
        schema_hash(crate::zk::confidential_v2::CONFIDENTIAL_TRANSFER_V2_PUBLIC_INPUTS_SCHEMA_V1),
        backend,
        crate::zk::confidential_v2::CONFIDENTIAL_TRANSFER_V2_CIRCUIT_ID,
        "core",
        vk_bytes,
    );
    host.set_verifying_keys(&(BTreeMap::from([(id.clone(), rec.clone())])))
        .expect("install prepared verifier key");
    let prepared = host
        .prepared_verifying_keys
        .get(&commitment)
        .expect("commitment index")
        .clone();
    assert_eq!(prepared.record.as_ref(), &rec);
    assert_eq!(prepared.backend_label.as_ref(), backend);
    assert_eq!(
        prepared
            .material
            .as_ref()
            .and_then(crate::zk::PreparedVerifyingKeyMaterialV1::ipa_k),
        Some(crate::zk::confidential_v2::CONFIDENTIAL_TRANSFER_V2_IPA_K)
    );
    // Rehydrating the identical registry pair reproduces the prepared entry.
    let mut rehydrated = CoreHost::new(fixture_account("alice"));
    rehydrated
        .set_verifying_keys(&(BTreeMap::from([(id.clone(), rec.clone())])))
        .expect("rehydrate the identical verifier key");
    assert_eq!(
        rehydrated.prepared_verifying_keys.get(&commitment),
        Some(&prepared)
    );
    // One changed key byte under the same id and commitment is validated
    // again and rejected, although the original pair was already prepared.
    let mut tampered = rec;
    tampered.key.as_mut().expect("inline verifier key").bytes[0] ^= 1;
    assert!(
        rehydrated
            .set_verifying_keys(&(BTreeMap::from([(id, tampered)])))
            .is_err()
    );
    assert_eq!(
        rehydrated.prepared_verifying_keys.get(&commitment),
        Some(&prepared),
        "a rejected registry leaves the previous index intact"
    );
}
#[test]
fn prepared_vk_index_rejects_missing_schedule_duplicates_and_updates_atomically() {
    let backend = "pipa-r/pasta";
    let circuit_id = crate::zk::confidential_v2::CONFIDENTIAL_TRANSFER_V2_CIRCUIT_ID;
    let vk_bytes = canonical_confidential_transfer_vk_bytes();
    let commitment = CoreHost::hash_vk_bytes(backend, &vk_bytes);
    let mut host = CoreHost::new(fixture_account("alice"));
    let id = VerifyingKeyId::new(backend, "original");
    let original = active_vk_record(
        commitment,
        schema_hash(crate::zk::confidential_v2::CONFIDENTIAL_TRANSFER_V2_PUBLIC_INPUTS_SCHEMA_V1),
        backend,
        circuit_id,
        "core",
        vk_bytes.clone(),
    );
    host.set_verifying_keys(&(BTreeMap::from([(id.clone(), original)])))
        .expect("install original verifier key");
    let original_record = Arc::clone(
        &host
            .prepared_verifying_keys
            .get(&commitment)
            .expect("original record")
            .record,
    );
    let mut missing_schedule = active_vk_record(
        commitment,
        schema_hash(crate::zk::confidential_v2::CONFIDENTIAL_TRANSFER_V2_PUBLIC_INPUTS_SCHEMA_V1),
        backend,
        circuit_id,
        "core",
        vk_bytes.clone(),
    );
    missing_schedule.gas_schedule_id = None;
    assert!(
        host.set_verifying_keys(
            &(BTreeMap::from([(
                VerifyingKeyId::new(backend, "missing-schedule"),
                missing_schedule,
            )]))
        )
        .is_err()
    );
    assert!(Arc::ptr_eq(
        &host
            .prepared_verifying_keys
            .get(&commitment)
            .expect("unchanged record")
            .record,
        &original_record
    ));
    assert_eq!(host.prepared_verifying_keys.len(), 1);
    let first = active_vk_record(
        commitment,
        schema_hash(crate::zk::confidential_v2::CONFIDENTIAL_TRANSFER_V2_PUBLIC_INPUTS_SCHEMA_V1),
        backend,
        circuit_id,
        "core",
        vk_bytes.clone(),
    );
    let second = active_vk_record(
        commitment,
        schema_hash(crate::zk::confidential_v2::CONFIDENTIAL_TRANSFER_V2_PUBLIC_INPUTS_SCHEMA_V1),
        backend,
        circuit_id,
        "core",
        vk_bytes,
    );
    assert!(
        host.set_verifying_keys(
            &(BTreeMap::from([
                (VerifyingKeyId::new(backend, "first"), first),
                (VerifyingKeyId::new(backend, "second"), second),
            ]))
        )
        .is_err()
    );
    assert!(Arc::ptr_eq(
        &host
            .prepared_verifying_keys
            .get(&commitment)
            .expect("still unchanged")
            .record,
        &original_record
    ));
    assert_eq!(host.prepared_verifying_keys.len(), 1);
}
#[test]
fn set_verifying_keys_rejects_malformed_circuit_ids() {
    let invalid_circuit_ids = [
        ("empty", " "),
        ("uppercase", "halo2/ipa:Transfer"),
        ("control", "halo2/ipa:transfer\nforged"),
        ("zero-width", "halo2/ipa:transfer\u{200B}"),
        ("path-traversal", "halo2/ipa/../transfer"),
        ("dot-segment", "halo2/ipa/./transfer"),
        ("hidden-segment", "halo2/ipa/.transfer"),
        ("slash-colon-adjacent", "halo2/ipa/:transfer"),
        ("colon-slash-adjacent", "halo2/ipa:/transfer"),
        ("dot-colon-adjacent", "halo2/ipa.:transfer"),
        ("colon-dot-adjacent", "halo2/ipa:.transfer"),
        ("backslash", "halo2\\ipa\\transfer"),
    ];
    for (label, circuit_id) in invalid_circuit_ids {
        let mut host = CoreHost::new(fixture_account("alice"));
        let backend = "pipa-r/pasta";
        let vk_bytes = vec![1, 2, 3, 4];
        let commitment = CoreHost::hash_vk_bytes(backend, &vk_bytes);
        let rec = active_vk_record(
            commitment, [0x42; 32], backend, circuit_id, "core", vk_bytes,
        );
        let mut map = BTreeMap::new();
        map.insert(VerifyingKeyId::new(backend, "vk"), rec);
        assert!(
            host.set_verifying_keys(&(map)).is_err(),
            "case {label} must reject malformed circuit id `{circuit_id}`"
        );
    }
    let mut host = CoreHost::new(fixture_account("alice"));
    let backend = "pipa-r/pasta";
    let vk_bytes = vec![1, 2, 3, 4];
    let commitment = CoreHost::hash_vk_bytes(backend, &vk_bytes);
    let oversized_circuit_id =
        "a".repeat(iroha_data_model::zk::OPEN_VERIFY_DEFAULT_MAX_CIRCUIT_ID_BYTES + 1);
    let rec = active_vk_record(
        commitment,
        [0x42; 32],
        backend,
        &oversized_circuit_id,
        "core",
        vk_bytes,
    );
    let mut map = BTreeMap::new();
    map.insert(VerifyingKeyId::new(backend, "vk"), rec);
    assert!(
        host.set_verifying_keys(&(map)).is_err(),
        "oversized circuit id must be rejected"
    );
}
#[test]
fn set_verifying_keys_rejects_exact12_privacy_ids_during_state_rehydration() {
    for circuit_id in PrivacyProtocolIdV1::ALL.map(PrivacyProtocolIdV1::canonical_label) {
        let mut host = CoreHost::new(fixture_account("alice"));
        let backend = "pipa-r/pasta";
        let vk_bytes = vec![1, 2, 3, 4];
        let commitment = CoreHost::hash_vk_bytes(backend, &vk_bytes);
        let record = active_vk_record(
            commitment, [0x42; 32], backend, circuit_id, "core", vk_bytes,
        );
        let map = BTreeMap::from([(VerifyingKeyId::new(backend, "vk"), record)]);
        assert!(
            host.set_verifying_keys(&(map)).is_err(),
            "Exact12 privacy circuit id {circuit_id:?} must not rehydrate"
        );
        assert!(host.prepared_verifying_keys.is_empty());
    }
    let exact12 = PrivacyProtocolIdV1::ZkAcePqAuthorizationV1.canonical_label();
    for (case, circuit_id) in [
        ("leading-whitespace", format!(" {exact12}")),
        ("trailing-whitespace", format!("{exact12} ")),
        ("uppercase", exact12.to_ascii_uppercase()),
    ] {
        let mut host = CoreHost::new(fixture_account("alice"));
        let backend = "pipa-r/pasta";
        let vk_bytes = vec![1, 2, 3, 4];
        let commitment = CoreHost::hash_vk_bytes(backend, &vk_bytes);
        let record = active_vk_record(
            commitment,
            [0x42; 32],
            backend,
            &circuit_id,
            "core",
            vk_bytes,
        );
        let map = BTreeMap::from([(VerifyingKeyId::new(backend, "vk"), record)]);
        assert!(
            host.set_verifying_keys(&(map)).is_err(),
            "{case} non-portable privacy alias {circuit_id:?} must not rehydrate"
        );
        assert!(host.prepared_verifying_keys.is_empty());
    }
    let mut host = CoreHost::new(fixture_account("alice"));
    let backend = "pipa-r/pasta";
    let vk_bytes = vec![1, 2, 3, 4];
    let commitment = CoreHost::hash_vk_bytes(backend, &vk_bytes);
    let near_miss = format!("generic-{exact12}");
    let record = active_vk_record(
        commitment, [0x42; 32], backend, &near_miss, "core", vk_bytes,
    );
    assert!(
        host.set_verifying_keys(&(BTreeMap::from([(VerifyingKeyId::new(backend, "vk"), record,)])))
            .is_err(),
        "a portable but unregistered circuit must not bypass the production circuit registry"
    );
    assert!(host.prepared_verifying_keys.is_empty());
}
#[test]
fn set_verifying_keys_rejects_malformed_registry_ids() {
    const OVERSIZED_VERIFYING_KEY_FIELD_BYTES: usize = 257;
    let invalid_registry_ids = [
        ("blank-backend", " ", "vk"),
        ("blank-name", "pipa-r/pasta", " "),
        ("uppercase-backend", "Halo2/ipa", "vk"),
        ("uppercase-name", "pipa-r/pasta", "Vk"),
        ("control-backend", "halo2/ipa\nforged", "vk"),
        ("control-name", "pipa-r/pasta", "vk\nforged"),
        ("zero-width-backend", "halo2/ipa\u{200B}", "vk"),
        ("zero-width-name", "pipa-r/pasta", "vk\u{200B}forged"),
        ("path-traversal-backend", "halo2/ipa/../vk", "vk"),
        ("path-traversal-name", "pipa-r/pasta", "vk/../forged"),
        ("dot-segment-backend", "halo2/ipa/./vk", "vk"),
        ("dot-segment-name", "pipa-r/pasta", "vk/./forged"),
        ("hidden-backend", "halo2/.ipa", "vk"),
        ("hidden-name", "pipa-r/pasta", ".vk"),
        ("slash-colon-backend", "halo2/ipa/:vk", "vk"),
        ("colon-slash-name", "pipa-r/pasta", "vk:/forged"),
        ("backslash-backend", "halo2\\ipa", "vk"),
        ("backslash-name", "pipa-r/pasta", "vk\\forged"),
        ("leading-delimiter-name", "pipa-r/pasta", "-vk"),
        ("trailing-delimiter-name", "pipa-r/pasta", "vk_"),
        ("backend-double-colon-alias", "halo2/ipa::transfer", "vk"),
    ];
    for (label, id_backend, id_name) in invalid_registry_ids {
        let mut host = CoreHost::new(fixture_account("alice"));
        let backend = "pipa-r/pasta";
        let vk_bytes = vec![1, 2, 3, 4];
        let commitment = CoreHost::hash_vk_bytes(backend, &vk_bytes);
        let rec = active_vk_record(
            commitment,
            [0x42; 32],
            backend,
            "halo2/ipa:test-circuit",
            "core",
            vk_bytes,
        );
        let mut map = BTreeMap::new();
        map.insert(VerifyingKeyId::new(id_backend, id_name), rec);
        assert!(
            host.set_verifying_keys(&(map)).is_err(),
            "case {label} must reject verifier-key registry id `{id_backend}` / `{id_name}`"
        );
    }
    for (label, id_backend, id_name) in [
        (
            "oversized-backend",
            "a".repeat(OVERSIZED_VERIFYING_KEY_FIELD_BYTES),
            "vk".to_string(),
        ),
        (
            "oversized-name",
            "pipa-r/pasta".to_string(),
            "a".repeat(OVERSIZED_VERIFYING_KEY_FIELD_BYTES),
        ),
    ] {
        let mut host = CoreHost::new(fixture_account("alice"));
        let backend = "pipa-r/pasta";
        let vk_bytes = vec![1, 2, 3, 4];
        let commitment = CoreHost::hash_vk_bytes(backend, &vk_bytes);
        let rec = active_vk_record(
            commitment,
            [0x42; 32],
            backend,
            "halo2/ipa:test-circuit",
            "core",
            vk_bytes,
        );
        let mut map = BTreeMap::new();
        map.insert(
            VerifyingKeyId::new(id_backend.as_str(), id_name.as_str()),
            rec,
        );
        assert!(
            host.set_verifying_keys(&(map)).is_err(),
            "case {label} must reject oversized verifier-key registry id"
        );
    }
}
#[test]
fn set_verifying_keys_rejects_registry_backend_tag_mismatches() {
    for (label, registry_backend, record_backend, circuit_id) in [
        (
            "stark-registry-halo2-record",
            "stark/fri",
            "pipa-r/pasta",
            "halo2/ipa:test-circuit",
        ),
        (
            "halo2-registry-stark-record",
            "pipa-r/pasta",
            "stark/fri/poseidon-x7-goldilocks-6x64-v1",
            "stark/fri/poseidon-x7-goldilocks-6x64-v1:zk-ace",
        ),
    ] {
        let mut host = CoreHost::new(fixture_account("alice"));
        let vk_bytes = vec![1, 2, 3, 4];
        let commitment = CoreHost::hash_vk_bytes(record_backend, &vk_bytes);
        let rec = active_vk_record(
            commitment,
            [0x42; 32],
            record_backend,
            circuit_id,
            "core",
            vk_bytes,
        );
        let mut map = BTreeMap::new();
        map.insert(VerifyingKeyId::new(registry_backend, "vk"), rec);
        assert!(
            host.set_verifying_keys(&(map)).is_err(),
            "case {label} must reject registry backend tag mismatch"
        );
    }
}
#[test]
fn set_verifying_keys_rejects_zero_commitment_or_schema_hash() {
    for (label, zero_commitment, zero_schema_hash, keyless) in [
        ("keyless-zero-commitment", true, false, true),
        ("stored-key-zero-schema-hash", false, true, false),
        ("keyless-zero-commitment-and-schema-hash", true, true, true),
    ] {
        let mut host = CoreHost::new(fixture_account("alice"));
        let backend = "pipa-r/pasta";
        let vk_bytes = vec![1, 2, 3, 4];
        let commitment = CoreHost::hash_vk_bytes(backend, &vk_bytes);
        let schema_hash = if zero_schema_hash {
            [0u8; 32]
        } else {
            [0x42; 32]
        };
        let mut rec = active_vk_record(
            commitment,
            schema_hash,
            backend,
            "halo2/ipa:test-circuit",
            "core",
            vk_bytes,
        );
        if zero_commitment {
            rec.commitment = [0u8; 32];
        }
        if keyless {
            rec.key = None;
        }
        let mut map = BTreeMap::new();
        map.insert(VerifyingKeyId::new(backend, "vk"), rec);
        assert!(
            host.set_verifying_keys(&(map)).is_err(),
            "case {label} must reject zero verifier-key binding material"
        );
    }
}
#[test]
fn set_verifying_keys_rejects_malformed_record_metadata() {
    #[derive(Clone, Copy)]
    enum Field {
        Namespace,
        OwnerManifest,
        GasSchedule,
        MetadataUri,
        VkBytesUri,
    }
    let oversized_uri = format!("ipfs://{}", "a".repeat(513));
    let cases = [
        ("blank-namespace", Field::Namespace, " "),
        ("uppercase-namespace", Field::Namespace, "Core"),
        ("control-namespace", Field::Namespace, "core\nforged"),
        ("zero-width-namespace", Field::Namespace, "core\u{200B}"),
        ("traversal-namespace", Field::Namespace, "core/../vk"),
        ("hidden-owner", Field::OwnerManifest, ".core"),
        ("control-owner", Field::OwnerManifest, "core\tforged"),
        ("slash-colon-owner", Field::OwnerManifest, "core/:owner"),
        ("blank-gas-schedule", Field::GasSchedule, " "),
        ("space-gas-schedule", Field::GasSchedule, "sched 0"),
        ("trailing-gas-delimiter", Field::GasSchedule, "sched_0_"),
        ("empty-metadata-uri", Field::MetadataUri, ""),
        ("trimmed-metadata-uri", Field::MetadataUri, " ipfs://vk"),
        ("empty-ipfs-metadata-uri", Field::MetadataUri, "ipfs://"),
        (
            "traversal-metadata-uri",
            Field::MetadataUri,
            "ipfs://vk/../admin",
        ),
        (
            "repeated-slash-metadata-uri",
            Field::MetadataUri,
            "ipfs://vk//admin",
        ),
        (
            "credentialed-metadata-uri",
            Field::MetadataUri,
            "https://user@example.com/vk",
        ),
        (
            "query-metadata-uri",
            Field::MetadataUri,
            "ipfs://vk?token=secret",
        ),
        (
            "fragment-metadata-uri",
            Field::MetadataUri,
            "ipfs://vk#section",
        ),
        (
            "zero-width-metadata-uri",
            Field::MetadataUri,
            "ipfs://vk\u{200B}",
        ),
        (
            "backslash-metadata-uri",
            Field::MetadataUri,
            "ipfs://vk\\admin",
        ),
        ("empty-cid-vk-bytes-uri", Field::VkBytesUri, "cid:"),
        (
            "traversal-vk-bytes-uri",
            Field::VkBytesUri,
            "cid:vk/../bytes",
        ),
        (
            "query-vk-bytes-uri",
            Field::VkBytesUri,
            "cid:vk?token=secret",
        ),
        (
            "oversized-metadata-uri",
            Field::MetadataUri,
            oversized_uri.as_str(),
        ),
    ];
    for (label, field, value) in cases {
        let mut host = CoreHost::new(fixture_account("alice"));
        let backend = "pipa-r/pasta";
        let vk_bytes = vec![1, 2, 3, 4];
        let commitment = CoreHost::hash_vk_bytes(backend, &vk_bytes);
        let mut rec = active_vk_record(
            commitment,
            [0x42; 32],
            backend,
            "halo2/ipa:test-circuit",
            "core",
            vk_bytes,
        );
        match field {
            Field::Namespace => rec.namespace = value.to_owned(),
            Field::OwnerManifest => rec.owner_manifest_id = Some(value.to_owned()),
            Field::GasSchedule => rec.gas_schedule_id = Some(value.to_owned()),
            Field::MetadataUri => rec.metadata_uri_cid = Some(value.to_owned()),
            Field::VkBytesUri => rec.vk_bytes_cid = Some(value.to_owned()),
        }
        let mut map = BTreeMap::new();
        map.insert(VerifyingKeyId::new(backend, "vk"), rec);
        assert!(
            host.set_verifying_keys(&(map)).is_err(),
            "case {label} must reject malformed verifier-key metadata `{value}`"
        );
    }
    for (label, activation_height, withdraw_height) in [
        ("same-height-window", Some(10), Some(10)),
        ("inverted-window", Some(10), Some(9)),
    ] {
        let mut host = CoreHost::new(fixture_account("alice"));
        let backend = "pipa-r/pasta";
        let vk_bytes = vec![1, 2, 3, 4];
        let commitment = CoreHost::hash_vk_bytes(backend, &vk_bytes);
        let mut rec = active_vk_record(
            commitment,
            [0x42; 32],
            backend,
            "halo2/ipa:test-circuit",
            "core",
            vk_bytes,
        );
        rec.activation_height = activation_height;
        rec.withdraw_height = withdraw_height;
        let mut map = BTreeMap::new();
        map.insert(VerifyingKeyId::new(backend, "vk"), rec);
        assert!(
            host.set_verifying_keys(&(map)).is_err(),
            "case {label} must reject invalid verifier-key activation window"
        );
    }
}
#[test]
fn set_verifying_keys_rejects_trusted_setup_backend_labels() {
    for backend in [
        "halo2/kzg",
        "halo2/ipa:KZG",
        "halo2/ipa: KZG",
        "halo2/bn254",
        "groth16/bn254",
    ] {
        let mut host = CoreHost::new(fixture_account("alice"));
        let vk_bytes = vec![1, 2, 3, 4];
        let commitment = CoreHost::hash_vk_bytes(backend, &vk_bytes);
        let rec = active_vk_record(
            commitment,
            [0x42; 32],
            backend,
            "halo2/ipa:test-circuit",
            "core",
            vk_bytes,
        );
        let mut map = BTreeMap::new();
        map.insert(VerifyingKeyId::new(backend, "vk"), rec);
        assert!(
            host.set_verifying_keys(&(map)).is_err(),
            "case {backend} must be rejected"
        );
    }
}
#[test]
fn set_verifying_keys_rejects_developer_only_backend_labels() {
    for backend in [
        "debug/halo2/ipa",
        "halo2/debug",
        "halo2/mock",
        "halo2/ipa:Mock-Proof",
        "stark/fri/debug",
        "stark/fri/Debug",
        "stark/fri/mock",
    ] {
        let mut host = CoreHost::new(fixture_account("alice"));
        let vk_bytes = vec![1, 2, 3, 4];
        let commitment = CoreHost::hash_vk_bytes(backend, &vk_bytes);
        let rec = active_vk_record(
            commitment,
            [0x42; 32],
            backend,
            &format!("{backend}:test-circuit"),
            "core",
            vk_bytes,
        );
        let mut map = BTreeMap::new();
        map.insert(VerifyingKeyId::new(backend, "vk"), rec);
        assert!(
            host.set_verifying_keys(&(map)).is_err(),
            "case {backend} must be rejected"
        );
    }
}
#[test]
fn set_verifying_keys_rejects_protocol_names_as_backend_labels() {
    for backend in [
        "halo2/ipa/orchard",
        "halo2/ipa:zcash-orchard",
        "stark/fri/miden",
        "stark/fri/pq-masp-stark-fri",
        "anonymous-pgc",
        "verange",
        "zk-ams",
        "zk-x509",
        "sis-hints-anoncred-pq-v0",
        "sis-with-hints",
    ] {
        let mut host = CoreHost::new(fixture_account("alice"));
        let vk_bytes = vec![1, 2, 3, 4];
        let commitment = CoreHost::hash_vk_bytes(backend, &vk_bytes);
        let rec = active_vk_record(
            commitment,
            [0x42; 32],
            backend,
            &format!("{backend}:test-circuit"),
            "core",
            vk_bytes,
        );
        let mut map = BTreeMap::new();
        map.insert(VerifyingKeyId::new(backend, "vk"), rec);
        assert!(
            host.set_verifying_keys(&(map)).is_err(),
            "case {backend} must be rejected"
        );
    }
}
#[test]
fn set_verifying_keys_rejects_readiness_claim_and_unsupported_backend_labels() {
    for (backend, backend_tag) in [
        ("halo2/ipa:production-ready", BackendTag::NativePipaRPasta),
        ("halo2/ipa:mainnet-complete", BackendTag::NativePipaRPasta),
        ("stark/fri/audit-signoff", BackendTag::Stark),
        ("halo2/unknown-native-v1", BackendTag::NativePipaRPasta),
        ("stark/fri/random-profile", BackendTag::Stark),
    ] {
        let mut host = CoreHost::new(fixture_account("alice"));
        let vk_bytes = vec![1, 2, 3, 4];
        let commitment = CoreHost::hash_vk_bytes(backend, &vk_bytes);
        let mut rec = active_vk_record(
            commitment,
            [0x42; 32],
            backend,
            &format!("{backend}:test-circuit"),
            "core",
            vk_bytes,
        );
        rec.backend = backend_tag;
        rec.curve = if backend_tag == BackendTag::Stark {
            "goldilocks".to_string()
        } else {
            "vesta".to_string()
        };
        let mut map = BTreeMap::new();
        map.insert(VerifyingKeyId::new(backend, "vk"), rec);
        assert!(
            host.set_verifying_keys(&(map)).is_err(),
            "case {backend} must be rejected before reaching IVM verification"
        );
    }
}
include!("host/contract_state_namespace_tests.rs");
include!("host/stream_token_custody_namespace_tests.rs");
#[test]
fn contract_address_bytes_roundtrip_through_typed_durable_state() {
    let authority: AccountId = fixture_account("alice");
    let contract_address = ContractAddress::derive(
        &"hash:0000000000000000000000000000000000000000000000000000000000000001#C50E"
            .parse()
            .expect("canonical test network id"),
        &authority,
        701,
        DataSpaceId::UNIVERSAL,
    )
    .expect("derive contract address");
    let source = r#"
            seiyaku AddressState { permission WriteState;
                state bytes Stored;

                hajimari() {
                    Stored = b"";
                }

                kotoage fn capture() authorize(WriteState) -> int {
                    Stored = context::seiyaku_address();
                    return 1;
                }

                view fn read() authorize(anyone) -> bytes {
                    return Stored;
                }
            }
        "#;
    let (program, _) = kotodama_lang::compiler::Compiler::new()
        .compile_source_with_manifest(source)
        .expect("compile typed bytes contract");
    let metadata = ivm::ProgramMetadata::parse(&program).expect("parse contract metadata");
    let interface = metadata
        .contract_interface
        .as_ref()
        .expect("CNTR interface");
    let entry_pc = |name: &str| {
        metadata.prefix_len() as u64
            + interface
                .entrypoints
                .iter()
                .find(|entrypoint| entrypoint.name == name)
                .unwrap_or_else(|| panic!("missing `{name}` entrypoint"))
                .entry_pc
    };
    let mut host = CoreHost::new(authority);
    host.set_contract_runtime_context(Some(ContractRuntimeExecutionContext {
        contract_subject: contract_address.subject_id(),
        contract_address: contract_address.clone(),
        contract_alias: None,
        entrypoint: "capture".to_owned(),
    }));
    let mut vm = IVM::new(u64::MAX);
    vm.load_program(&program)
        .expect("load typed bytes contract");
    assert_eq!(
        (host.prepare_syscall(ivm::syscalls::SYSCALL_SYSVAR_CONTRACT_ADDRESS, &vm))
            .expect("expected successful fixture result"),
        CoreHost::sysvar_gas(contract_address.as_str().len())
    );
    vm.set_register(1, vm.memory.code_len());
    vm.set_program_counter(entry_pc("capture"))
        .expect("seek capture entrypoint");
    vm.run_with_host(&mut host)
        .expect("store contract address bytes");
    assert_eq!(host.durable_state_overlay.len(), 1);
    let stored = host
        .durable_state_overlay
        .values()
        .next()
        .and_then(Option::as_ref)
        .expect("typed durable state record");
    let path: StatePath = "Stored".parse().expect("declared scalar path");
    ivm::host::validate_declared_state_value_payload(&vm, &path, stored)
        .expect("persisted record remains schema-bound and canonical");
    host.set_contract_runtime_context(Some(ContractRuntimeExecutionContext {
        contract_subject: contract_address.subject_id(),
        contract_address: contract_address.clone(),
        contract_alias: None,
        entrypoint: "read".to_owned(),
    }));
    let mut read_vm = IVM::new(u64::MAX);
    read_vm
        .load_program(&program)
        .expect("load typed bytes contract for readback");
    read_vm.set_register(1, read_vm.memory.code_len());
    read_vm
        .set_program_counter(entry_pc("read"))
        .expect("seek read entrypoint");
    read_vm
        .run_with_host(&mut host)
        .expect("read canonical durable contract address bytes");
    assert!(matches!(read_vm.call_result_word_count(), Ok(1)));
    let returned_pointer = read_vm
        .public_call_result_word(0)
        .expect("completed public result owns one pointer word");
    let returned = read_vm
        .validate_tlv(returned_pointer)
        .expect("validate bytes return");
    assert_eq!(returned.type_id, PointerType::Blob);
    assert_eq!(returned.version, 1);
    assert_eq!(returned.payload, contract_address.as_str().as_bytes());
}
fn encode_fixture_int_map_key(value: i128) -> Vec<u8> {
    use ivm_abi::state_value::{
        StateValueAtomV1, StateValueKindV1, StateValueNodeV1, StateValueRecordV1,
        StateValueSchemaV1, state_value_schema_hash_v1,
    };
    let schema = StateValueSchemaV1 {
        nodes: vec![StateValueNodeV1::Leaf(StateValueKindV1::Int)],
    };
    let envelope =
        ivm_abi::numeric_tlv::encode_int(&iroha_primitives::bigint::BigInt::from_i128(value))
            .unwrap();
    norito::encode_canonical(&StateValueRecordV1 {
        schema_hash: state_value_schema_hash_v1(&norito::encode_canonical(&schema).unwrap()),
        atoms: vec![StateValueAtomV1::Pointer(envelope)],
    })
    .unwrap()
}
#[test]
fn production_typed_state_map_scans_and_writes_validate_cntr() {
    let authority: AccountId = fixture_account("alice");
    let contract_address = ContractAddress::derive(
        &"hash:0000000000000000000000000000000000000000000000000000000000000001#C50E"
            .parse()
            .expect("canonical test network id"),
        &authority,
        702,
        DataSpaceId::UNIVERSAL,
    )
    .expect("derive contract address");
    let source = r#"
            seiyaku TypedMapState { permission WriteState;
                state StateMap<int, bytes> BytesMap;
                state StateMap<int, int> IntMap;

                kotoage fn write() authorize(WriteState) -> int {
                    BytesMap[1] = b"canonical";
                    return 1;
                }
            }
        "#;
    let (program, _) = kotodama_lang::compiler::Compiler::new()
        .compile_source_with_manifest(source)
        .expect("compile typed map contract");
    let metadata = ivm::ProgramMetadata::parse(&program).expect("parse contract metadata");
    let entry_pc = metadata.prefix_len() as u64
        + metadata
            .contract_interface
            .as_ref()
            .expect("CNTR interface")
            .entrypoints
            .iter()
            .find(|entrypoint| entrypoint.name == "write")
            .expect("write entrypoint")
            .entry_pc;
    let mut host = CoreHost::new(authority);
    host.set_contract_runtime_context(Some(ContractRuntimeExecutionContext {
        contract_subject: contract_address.subject_id(),
        contract_address,
        contract_alias: None,
        entrypoint: "write".to_owned(),
    }));
    let mut vm = IVM::new(u64::MAX);
    vm.load_program(&program).expect("load typed map contract");
    vm.set_register(1, vm.memory.code_len());
    vm.set_program_counter(entry_pc)
        .expect("seek write entrypoint");
    vm.run_with_host(&mut host)
        .expect("canonical typed child write");
    let base: StatePath = "BytesMap".parse().expect("declared map base");
    let base_ptr = store_state_path_tlv(&mut vm, &base);
    vm.set_register(10, base_ptr);
    vm.set_register(11, 0);
    vm.set_register(12, ivm_sys::STATE_SCAN_MAX_ITEMS_V1);
    for register in 13..=15 {
        vm.set_register(register, 0);
    }
    host.syscall(ivm_sys::SYSCALL_STATE_SCAN, &mut vm)
        .expect("bare StateMap base is a valid scan prefix");
    assert_eq!(vm.register(11), 0, "exhausted page has no cursor");
    assert_eq!(vm.register(13), 1);
    assert_eq!(vm.register(12), 1);
    let keys = vm.validate_tlv(vm.register(10)).expect("state-key page");
    let keys: Vec<StatePath> = norito::decode_from_bytes(keys.payload).expect("decode key page");
    assert_eq!(keys.len(), 1);
    vm.set_register(10, base_ptr);
    host.syscall(ivm_sys::SYSCALL_STATE_COUNT, &mut vm)
        .expect("bare StateMap base is a valid count prefix");
    assert_eq!(vm.register(10), 1);
    let before_overlay = host.durable_state_overlay.clone();
    let before_reads = host.state_access_log.read_keys.clone();
    let before_writes = host.state_access_log.write_keys.clone();
    let before_durable_reads = host.state_access_log.durable_read_paths.clone();
    let before_state_write_count = host.state_access_log.state_writes.len();
    vm.set_register(10, base_ptr);
    assert!(matches!(
        host.syscall(ivm_sys::SYSCALL_STATE_HAS, &mut vm),
        Err(ivm::VMError::NoritoInvalid)
    ));
    assert_eq!(host.durable_state_overlay, before_overlay);
    let bytes_record = before_overlay
        .iter()
        .find_map(|(path, value)| {
            path.as_ref()
                .contains("BytesMap/")
                .then_some(value.as_ref())
                .flatten()
        })
        .expect("compiler-emitted bytes record")
        .clone();
    let key = encode_fixture_int_map_key(1);
    let wrong_path: StatePath = format!("IntMap/{}", hex::encode(key))
        .parse()
        .expect("canonical typed child path");
    let wrong_path_ptr = store_state_path_tlv(&mut vm, &wrong_path);
    let wrong_value_ptr = store_tlv(&mut vm, PointerType::NoritoBytes, &bytes_record);
    vm.set_register(10, wrong_path_ptr);
    vm.set_register(11, wrong_value_ptr);
    assert!(
        matches!(
            host.syscall(ivm_sys::SYSCALL_STATE_SET, &mut vm),
            Err(ivm::VMError::DecodeError)
        ),
        "a record for bytes must not be staged under an int declaration"
    );
    let malformed_ptr = store_tlv(&mut vm, PointerType::NoritoBytes, b"not a state record");
    vm.set_register(10, wrong_path_ptr);
    vm.set_register(11, malformed_ptr);
    assert!(matches!(
        host.syscall(ivm_sys::SYSCALL_STATE_SET, &mut vm),
        Err(ivm::VMError::DecodeError)
    ));
    assert_eq!(host.durable_state_overlay, before_overlay);
    assert_eq!(host.state_access_log.read_keys, before_reads);
    assert_eq!(host.state_access_log.write_keys, before_writes);
    assert_eq!(
        host.state_access_log.durable_read_paths,
        before_durable_reads
    );
    assert_eq!(
        host.state_access_log.state_writes.len(),
        before_state_write_count
    );
}
#[test]
fn contract_reads_but_cannot_mutate_consensus_conversion_offer() {
    let authority: AccountId = fixture_account("alice");
    let contract_address = ContractAddress::derive(
        &"hash:0000000000000000000000000000000000000000000000000000000000000001#C50E"
            .parse()
            .expect("canonical test network id"),
        &authority,
        91,
        DataSpaceId::UNIVERSAL,
    )
    .expect("derive contract address");
    let context = ContractRuntimeExecutionContext {
        contract_subject: contract_address.subject_id(),
        contract_address: contract_address.clone(),
        contract_alias: Some("validation_fee::credit_reader".parse().expect("alias")),
        entrypoint: "main".to_owned(),
    };
    let base: iroha_model_base::name::Name = "ValidationFeeConversion".parse().expect("base");
    let encoded_key = encode_fixture_int_map_key(0);
    let relative_key =
        ivm::host::canonical_state_map_path(&base, &encoded_key).expect("canonical map path");
    let scope = hex::encode(Hash::new(contract_address.to_string().as_bytes()).as_ref());
    let credit_key: StatePath = format!("sc/{scope}/{relative_key}")
        .parse()
        .expect("native projection path");
    let credit: Quantity = "18446744073709551616.25"
        .parse()
        .expect("canonical credit above the u64 domain");
    let mut world = World::new();
    world.smart_contract_state.insert(
        credit_key,
        crate::validation_fee::encode_conversion_quantity_state_value(&credit)
            .expect("encode schema-bound native consensus credit"),
    );
    let kura = Kura::blank_kura_for_testing();
    let authenticated_block = iroha_data_model::block::builder::BlockBuilder::new(
        BlockHeader::new(nonzero!(1_u64), None, None, 1_700_000_000_000, 0),
    )
    .build_with_signature(0, ALICE_KEYPAIR.private_key());
    // State authenticates the lane's physical storage before history is retained.
    let state = State::new_for_testing(world, Arc::clone(&kura), LiveQueryStore::start_test());
    kura.store_block(
        crate::block::reserve_block_for_tests().initialize(authenticated_block.clone()),
    )
    .expect("store authenticated ledger-time fixture block");
    state.append_committed_block_header_for_tests(authenticated_block.header());
    let source = r"
            seiyaku ValidationFeeConversionReader {
                state StateMap<int, quantity> ValidationFeeConversion;
                view fn main() authorize(anyone) -> quantity {
                    let quantity zero = 0;
                    return ValidationFeeConversion.get(0).unwrap_or(zero);
                }
            }
        ";
    let (code, _) = kotodama_lang::compiler::Compiler::new()
        .compile_source_with_manifest(source)
        .expect("compile validation-fee credit reader");
    let metadata = ivm::ProgramMetadata::parse(&code).expect("parse credit-reader metadata");
    let main_entry_pc = metadata.prefix_len() as u64
        + metadata
            .contract_interface
            .as_ref()
            .expect("credit-reader CNTR interface")
            .entrypoints
            .iter()
            .find(|entrypoint| entrypoint.name == "main")
            .expect("credit-reader main entrypoint")
            .entry_pc;
    let mut contract_host =
        CoreHost::from_state(authority.clone(), &state).expect("canonical state snapshots");
    contract_host.set_contract_runtime_context(Some(context.clone()));
    let mut contract_vm = IVM::new(u64::MAX);
    contract_vm.set_host(contract_host);
    contract_vm.load_program(&code).expect("load credit reader");
    contract_vm.set_register(1, contract_vm.memory.code_len());
    contract_vm
        .set_program_counter(main_entry_pc)
        .expect("select credit-reader main entrypoint");
    contract_vm.run().expect("read native consensus credit");
    assert!(matches!(contract_vm.call_result_word_count(), Ok(1)));
    let returned_pointer = contract_vm
        .public_call_result_word(0)
        .expect("completed public result owns one pointer word");
    let returned = contract_vm
        .validate_tlv(returned_pointer)
        .expect("returned validation-fee credit must be a valid TLV");
    assert_eq!(returned.type_id, PointerType::Quantity);
    let returned = QuantityValueV1::decode_frame(returned.payload)
        .expect("decode returned validation-fee credit")
        .into_quantity();
    assert_eq!(
        returned, credit,
        "Kotodama state quantity must preserve the exact native consensus value"
    );
    let mut host = CoreHost::from_state(authority, &state).expect("canonical state snapshots");
    host.set_contract_runtime_context(Some(context));
    let mut vm = IVM::new(10_000);
    vm.load_program(&code)
        .expect("load credit-reader contract metadata for mutation checks");
    let value_ptr = store_tlv(
        &mut vm,
        PointerType::NoritoBytes,
        &crate::validation_fee::encode_conversion_quantity_state_value(&Quantity::from(999_u64))
            .expect("encode forged schema-bound credit"),
    );
    for leaf in [relative_key.as_ref(), "ValidationFeeRewards/reserved/State"] {
        let leaf: StatePath = leaf.parse().expect("reserved validation-fee credit leaf");
        let path_ptr = store_state_path_tlv(&mut vm, &leaf);
        vm.set_register(10, path_ptr);
        vm.set_register(11, value_ptr);
        assert!(matches!(
            host.syscall(ivm_sys::SYSCALL_STATE_SET, &mut vm),
            Err(ivm::VMError::PermissionDenied)
        ));
        vm.set_register(10, path_ptr);
        assert!(matches!(
            host.syscall(ivm_sys::SYSCALL_STATE_DEL, &mut vm),
            Err(ivm::VMError::PermissionDenied)
        ));
    }
    assert!(host.drain_durable_state_overlay().is_empty());
}
#[test]
fn full_width_int_pointer_codec_roundtrips_through_ledger_host() {
    let authority: AccountId = fixture_account("alice");
    let mut host = CoreHost::new(authority);
    let mut vm = IVM::new(10_000);
    let value = BigInt::from_i128(42);
    let envelope = ivm_abi::numeric_tlv::encode_int(&value).expect("canonical Int envelope");
    let source = vm.alloc_input_tlv(&envelope).expect("allocate Int");
    vm.set_register(10, source);
    let encode_gas = host
        .syscall(ivm_sys::SYSCALL_POINTER_TO_NORITO, &mut vm)
        .expect("encode full-width Int");
    let encoded_ptr = vm.register(10);
    let encoded = vm.memory.validate_tlv(encoded_ptr).expect("encoded TLV");
    assert_eq!(encoded.type_id, PointerType::NoritoBytes);
    assert_eq!(encoded.payload, envelope);
    assert_eq!(encode_gas, 16 + envelope.len() as u64);
    vm.set_register(10, encoded_ptr);
    vm.set_register(11, u64::from(PointerType::Int as u16));
    assert_eq!(
        (host.syscall(ivm_sys::SYSCALL_POINTER_FROM_NORITO, &mut vm))
            .expect("expected successful fixture result"),
        16 + envelope.len() as u64
    );
    let restored = vm
        .memory
        .validate_tlv(vm.register(10))
        .expect("restored Int");
    assert_eq!(restored.type_id, PointerType::Int);
    assert_eq!(
        (ivm::numeric_tlv::decode_int_bytes(&make_tlv(PointerType::Int as u16, restored.payload)))
            .expect("expected successful fixture result"),
        value
    );
}
#[test]
fn exact_json_getters_accept_only_canonical_strings_through_ledger_host() {
    let mut host = CoreHost::new(fixture_account("alice"));
    let mut vm = IVM::new(100_000);
    let json = shared_argument_fixture_payload();
    let json_ptr = store_tlv(&mut vm, PointerType::Json, &norito_blob(&json));
    for (key, syscall, pointer_type, expected) in [
        (
            "exact_int",
            ivm_sys::SYSCALL_JSON_GET_INT,
            PointerType::Int,
            "1606938044258990275541962092341162602522202993782792835301376",
        ),
        (
            "exact_decimal",
            ivm_sys::SYSCALL_JSON_GET_DECIMAL,
            PointerType::Decimal,
            "-12345678901234567890.125",
        ),
        (
            "exact_quantity",
            ivm_sys::SYSCALL_JSON_GET_QUANTITY,
            PointerType::Quantity,
            "12345678901234567890.0000000000000000000000000001",
        ),
    ] {
        let key: Name = key.parse().expect("exact-number key");
        let key_ptr = store_tlv(&mut vm, PointerType::Name, &norito_blob(&key));
        vm.set_register(10, json_ptr);
        vm.set_register(11, key_ptr);
        host.syscall(syscall, &mut vm).expect("exact JSON getter");
        let (some, words) = ivm::sum::read_words(
            &vm,
            vm.register(10),
            ivm::sum::SumLayoutV1::option(1).expect("exact-number option layout"),
        )
        .expect("exact-number option");
        assert!(some, "{key} must produce Option::some");
        let tlv = vm.memory.validate_tlv(words[0]).expect("exact-number TLV");
        assert_eq!(tlv.type_id, pointer_type);
        let actual = match pointer_type {
            PointerType::Int => IntValueV1::decode_frame(tlv.payload)
                .expect("decode int frame")
                .into_int()
                .to_string(),
            PointerType::Decimal => {
                iroha_primitives::numeric_abi::DecimalValueV1::decode_frame(tlv.payload)
                    .expect("decode decimal frame")
                    .into_numeric()
                    .to_string()
            }
            PointerType::Quantity => QuantityValueV1::decode_frame(tlv.payload)
                .expect("decode quantity frame")
                .into_quantity()
                .to_string(),
            _ => unreachable!("fixed exact-number pointer cases"),
        };
        assert_eq!(actual, expected);
    }
    let token_json =
        Json::from_str_norito(r#"{"decimal_token":1.25,"int_token":7,"quantity_token":7}"#)
            .expect("noncanonical numeric-token JSON");
    let token_json_ptr = store_tlv(&mut vm, PointerType::Json, &norito_blob(&token_json));
    for (key, syscall) in [
        ("int_token", ivm_sys::SYSCALL_JSON_GET_INT),
        ("decimal_token", ivm_sys::SYSCALL_JSON_GET_DECIMAL),
        ("quantity_token", ivm_sys::SYSCALL_JSON_GET_QUANTITY),
    ] {
        let key: Name = key.parse().expect("numeric-token key");
        let key_ptr = store_tlv(&mut vm, PointerType::Name, &norito_blob(&key));
        vm.set_register(10, token_json_ptr);
        vm.set_register(11, key_ptr);
        // A present JSON number token is malformed input, never an absent field.
        assert!(
            matches!(
                host.syscall(syscall, &mut vm),
                Err(ivm::VMError::DecodeError)
            ),
            "{key} must reject a JSON number token"
        );
    }
}
#[test]
fn unassigned_exact_json_getters_fail_before_ledger_host_work() {
    let mut host = CoreHost::new(fixture_account("alice"));
    let mut vm = IVM::new(50_000);
    vm.set_register(10, 0x1111);
    vm.set_register(11, 0x2222);
    for number in 0x01_0163..=0x01_0165 {
        let registers_before = [vm.register(10), vm.register(11)];
        let heap_cursor_before = vm.memory.alloc(0).expect("read heap cursor");
        let gas_before = vm.remaining_gas();
        let queued_before = host.queued.len();
        let overlay_before = host.durable_state_overlay.len();
        assert!(
            matches!(host.prepare_syscall(number, &vm), Err(ivm::VMError::UnknownSyscall(__actual_error_field)) if __actual_error_field == (number))
        );
        assert!(
            matches!(host.syscall(number, &mut vm), Err(ivm::VMError::UnknownSyscall(__actual_error_field)) if __actual_error_field == (number))
        );
        assert_eq!([vm.register(10), vm.register(11)], registers_before);
        assert_eq!(
            vm.memory
                .alloc(0)
                .expect("read heap cursor after rejection"),
            heap_cursor_before
        );
        assert_eq!(vm.remaining_gas(), gas_before);
        assert_eq!(host.queued.len(), queued_before);
        assert_eq!(host.durable_state_overlay.len(), overlay_before);
    }
}
#[test]
fn state_syscall_logs_access_when_enabled() {
    let authority: AccountId = fixture_account("alice");
    let contract = ContractAddress::derive(
        &"hash:0000000000000000000000000000000000000000000000000000000000000001#C50E"
            .parse()
            .expect("canonical test network id"),
        &authority,
        43,
        DataSpaceId::UNIVERSAL,
    )
    .expect("derive contract");
    let context = ContractRuntimeExecutionContext {
        contract_subject: contract.subject_id(),
        contract_address: contract,
        contract_alias: Some("access::log".parse().expect("contract alias")),
        entrypoint: "write_then_read".to_owned(),
    };
    let mut host = CoreHost::new(authority).with_access_logging();
    host.set_local_contract_debug_execution();
    host.set_contract_runtime_context(Some(context));
    let mut vm = IVM::new(10_000);
    let path: StatePath = "counter".parse().unwrap();
    let path_ptr = store_state_path_tlv(&mut vm, &path);
    let value_bytes = norito::to_bytes(&1_u64).expect("encode state value");
    let value_ptr = store_tlv(&mut vm, PointerType::NoritoBytes, &value_bytes);
    vm.set_register(10, path_ptr);
    vm.set_register(11, value_ptr);
    host.begin_tx(&ivm::parallel::StateAccessSet::default())
        .expect("begin_tx should reset access log");
    assert_eq!(
        (host.syscall(ivm_sys::SYSCALL_STATE_SET, &mut vm))
            .expect("expected successful fixture result"),
        test_state_value_gas(&path, value_bytes.len()),
        "STATE_SET should succeed when access logging is enabled"
    );
    vm.set_register(10, path_ptr);
    host.syscall(ivm_sys::SYSCALL_STATE_GET, &mut vm)
        .expect("STATE_GET should log the contract-scoped path");
    let log = host.finish_tx().expect("finish_tx should return log");
    assert!(host.access_logging_supported());
    assert!(log.write_keys.contains("counter"));
    assert!(log.read_keys.contains("counter"));
    assert!(
        !log.durable_read_paths.contains("counter"),
        "contract execution must not expose the raw unscoped namespace"
    );
    let scope_prefix = host
        .durable_state_scope_prefix()
        .expect("contract runtime context creates a scope");
    assert!(
        log.durable_read_paths
            .contains(&format!("{scope_prefix}counter"))
    );
}
#[test]
fn state_syscall_reads_world_snapshot() {
    let mut world = World::new();
    let path: StatePath = "counter".parse().unwrap();
    let value_bytes = norito::to_bytes(&1_u64).expect("encode state value");
    world
        .smart_contract_state
        .insert(path.clone(), value_bytes.clone());
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let state = State::new_for_testing(world, kura, query);
    super::pointer_abi_tests::establish_authenticated_axt_ledger_time(&state, 1);
    let authority: AccountId = fixture_account("alice");
    let mut host = CoreHost::from_state(authority, &state).expect("canonical state snapshots");
    host.set_local_contract_debug_execution();
    let mut vm = IVM::new(10_000);
    let path_ptr = store_state_path_tlv(&mut vm, &path);
    vm.set_register(10, path_ptr);
    assert_eq!(
        (host.syscall(ivm_sys::SYSCALL_STATE_GET, &mut vm))
            .expect("expected successful fixture result"),
        test_state_value_gas(&path, value_bytes.len()),
        "STATE_GET should read from the world snapshot"
    );
    let out_ptr = vm.register(10);
    let tlv = vm.memory.validate_tlv(out_ptr).expect("snapshot tlv");
    assert_eq!(tlv.type_id, PointerType::NoritoBytes);
    let value: u64 = norito::decode_from_bytes(tlv.payload).expect("decode state value");
    assert_eq!(value, 1);
}
#[test]
fn state_syscall_reads_live_query_state_without_eager_snapshot() {
    let mut world = World::new();
    let path: StatePath = "counter".parse().unwrap();
    let value_bytes = norito::to_bytes(&7_u64).expect("encode state value");
    world
        .smart_contract_state
        .insert(path.clone(), value_bytes.clone());
    world.smart_contract_state.insert(
        "unrelated/key".parse().unwrap(),
        norito::to_bytes(&9_u64).expect("encode unrelated value"),
    );
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let state = State::new_for_testing(world, kura, query);
    let view = state.view();
    let authority: AccountId = fixture_account("alice");
    let mut host = CoreHostImpl::new(authority);
    host.set_local_contract_debug_execution();
    host.set_query_state(&view);
    let mut vm = IVM::new(10_000);
    let path_ptr = store_state_path_tlv(&mut vm, &path);
    vm.set_register(10, path_ptr);
    assert_eq!(
        (host.syscall(ivm_sys::SYSCALL_STATE_GET, &mut vm))
            .expect("expected successful fixture result"),
        test_state_value_gas(&path, value_bytes.len()),
        "STATE_GET should read from live query state without cloning the durable map"
    );
    assert!(
        host.durable_state_base.is_empty(),
        "live contract-call hosts should not eagerly clone smart_contract_state"
    );
    let out_ptr = vm.register(10);
    let tlv = vm.memory.validate_tlv(out_ptr).expect("live state tlv");
    assert_eq!(tlv.type_id, PointerType::NoritoBytes);
    let value: u64 = norito::decode_from_bytes(tlv.payload).expect("decode state value");
    assert_eq!(value, 7);
    let prefix: StatePath = "counter".parse().unwrap();
    let prefix_ptr = store_state_path_tlv(&mut vm, &prefix);
    vm.set_register(10, prefix_ptr);
    host.syscall(ivm_sys::SYSCALL_STATE_COUNT, &mut vm)
        .expect("STATE_COUNT should read live query-state keys");
    assert_eq!(vm.register(10), 1);
}
#[test]
fn state_count_uses_scoped_ordered_range_amid_large_unrelated_state() {
    let authority: AccountId = fixture_account("alice");
    let contract = ContractAddress::derive(
        &"hash:0000000000000000000000000000000000000000000000000000000000000001#C50E"
            .parse()
            .expect("canonical test network id"),
        &authority,
        0x51a7e,
        DataSpaceId::UNIVERSAL,
    )
    .expect("derive contract");
    let context = ContractRuntimeExecutionContext {
        contract_subject: contract.subject_id(),
        contract_address: contract,
        contract_alias: Some("range::state".parse().expect("contract alias")),
        entrypoint: "list".to_owned(),
    };
    let mut scope_host = CoreHost::new(authority.clone());
    scope_host.set_local_contract_debug_execution();
    scope_host.set_contract_runtime_context(Some(context.clone()));
    let expected = ["orders/1", "orders/10", "orders/2"]
        .map(|raw| raw.parse::<StatePath>().expect("state key"))
        .to_vec();
    let collisions = ["orders-shadow/1", "orders0/1"]
        .map(|raw| raw.parse::<StatePath>().expect("text-prefix collision"));
    let mut world = World::new();
    for (index, key) in expected.iter().rev().enumerate() {
        let physical = scope_host
            .scoped_durable_state_path(key)
            .expect("scope key")
            .expect("contract scope");
        world.smart_contract_state.insert(
            physical,
            norito::to_bytes(&u64::try_from(index).expect("small index"))
                .expect("encode state value"),
        );
    }
    for key in collisions {
        let physical = scope_host
            .scoped_durable_state_path(&key)
            .expect("scope collision")
            .expect("contract scope");
        world.smart_contract_state.insert(physical, vec![0x5a; 64]);
    }
    for index in 0..4_096_u32 {
        let side = if index % 2 == 0 { "aa" } else { "zz" };
        let key: StatePath = format!("{side}-global-unrelated/{index:05}")
            .parse()
            .expect("unrelated state key");
        world.smart_contract_state.insert(key, vec![0xa5; 256]);
    }
    let state = State::new_for_testing(
        world,
        Kura::blank_kura_for_testing(),
        LiveQueryStore::start_test(),
    );
    let view = state.view();
    let mut host = CoreHostImpl::new(authority);
    host.set_local_contract_debug_execution();
    host.set_query_state(&view);
    host.set_contract_runtime_context(Some(context));
    let mut vm = IVM::new(u64::MAX);
    let prefix: StatePath = "orders".parse().expect("state prefix");
    let prefix_ptr = store_state_path_tlv(&mut vm, &prefix);
    let prefix_payload_len = norito_blob(&prefix).len();
    let (_, scan_work_gas) = host
        .count_durable_state_keys(&vm, &prefix, prefix_payload_len)
        .expect("measure scoped state count");
    vm.set_register(10, prefix_ptr);
    let count_gas = host
        .syscall(ivm_sys::SYSCALL_STATE_COUNT, &mut vm)
        .expect("STATE_COUNT should use the same scoped range");
    assert_eq!(vm.register(10), 3);
    assert_eq!(
        count_gas,
        ivm::host::STATE_QUERY_GAS_BASE.saturating_add(scan_work_gas),
        "count gas covers every range candidate examined"
    );
}
#[test]
fn state_has_and_len_borrow_values_amid_large_unrelated_state() {
    let target: StatePath = "target/large".parse().expect("target state key");
    let target_value = vec![0xc3; 512 * 1024];
    let mut world = World::new();
    world
        .smart_contract_state
        .insert(target.clone(), target_value.clone());
    for index in 0..2_048_u32 {
        let key: StatePath = format!("unrelated/{index:05}")
            .parse()
            .expect("unrelated state key");
        world.smart_contract_state.insert(key, vec![0x3c; 512]);
    }
    let state = State::new_for_testing(
        world,
        Kura::blank_kura_for_testing(),
        LiveQueryStore::start_test(),
    );
    let view = state.view();
    let authority: AccountId = fixture_account("alice");
    let mut host = CoreHostImpl::new(authority);
    host.set_local_contract_debug_execution();
    host.set_query_state(&view);
    let mut vm = IVM::new(u64::MAX);
    let target_ptr = store_state_path_tlv(&mut vm, &target);
    vm.set_register(10, target_ptr);
    assert_eq!(
        (host.syscall(ivm_sys::SYSCALL_STATE_HAS, &mut vm))
            .expect("expected successful fixture result"),
        test_state_path_gas(&target)
    );
    assert_eq!(vm.register(10), 1);
    assert!(host.durable_state_base.is_empty());
    vm.set_register(10, target_ptr);
    assert_eq!(
        (host.syscall(ivm_sys::SYSCALL_STATE_LEN, &mut vm))
            .expect("expected successful fixture result"),
        test_state_path_gas(&target)
    );
    assert_eq!(
        vm.register(10),
        u64::try_from(target_value.len()).expect("length fits")
    );
    assert_eq!(vm.register(11), 1);
    assert!(
        host.durable_state_base.is_empty(),
        "metadata-only queries must not materialize a durable-state snapshot"
    );
}
#[test]
fn state_get_preflights_gas_before_world_state_output() {
    let mut world = World::new();
    let path: StatePath = "large".parse().expect("state key");
    world
        .smart_contract_state
        .insert(path.clone(), vec![0xabu8; 128]);
    let state = State::new_for_testing(
        world,
        Kura::blank_kura_for_testing(),
        LiveQueryStore::start_test(),
    );
    super::pointer_abi_tests::establish_authenticated_axt_ledger_time(&state, 1);
    let authority: AccountId = fixture_account("alice");
    let mut host = CoreHost::from_state(authority, &state).expect("canonical state snapshots");
    host.set_local_contract_debug_execution();
    let mut vm = IVM::new(u64::MAX);
    let path_ptr = store_state_path_tlv(&mut vm, &path);
    let code = [ivm::encoding::wide::encode_sys(
        ivm::instruction::wide::system::SCALL,
        u8::try_from(ivm_sys::SYSCALL_STATE_GET).expect("fixture value fits u8"),
    )
    .to_le_bytes()]
    .concat();
    vm.load_program(&build_authenticated_test_contract_program_with_states(
        &code,
        0,
        false,
        vec![ivm::EmbeddedStateDescriptor {
            name: path.to_string(),
            ty: ivm::EmbeddedStateType::Bytes,
        }],
    ))
    .expect("load authenticated state query program");
    vm.set_register(14, path_ptr);
    vm.set_register(15, 0xfeed);
    vm.set_gas_limit(authenticated_test_probe_setup_gas() + 20);
    let error = vm
        .run_with_host(&mut host)
        .expect_err("world-state response cost exceeds the syscall reserve");
    assert!(matches!(error, ivm::VMError::OutOfGas));
    assert_eq!(
        vm.remaining_gas(),
        15,
        "an unaffordable reserve must not be partially debited"
    );
    assert_eq!(
        vm.register(10),
        path_ptr,
        "STATE_GET must not allocate output"
    );
    assert_eq!(vm.register(11), 0xfeed, "STATE_GET must not mutate outputs");
}
#[test]
fn state_syscall_reads_world_snapshot_spills_to_heap_when_input_fills() {
    let mut world = World::new();
    let path: StatePath = "counter".parse().unwrap();
    let expected = vec![0xA5; 96];
    let value_bytes = norito::to_bytes(&expected).expect("encode state value");
    world.smart_contract_state.insert(path.clone(), value_bytes);
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let state = State::new_for_testing(world, kura, query);
    super::pointer_abi_tests::establish_authenticated_axt_ledger_time(&state, 1);
    let authority: AccountId = fixture_account("alice");
    let mut host = CoreHost::from_state(authority, &state).expect("canonical state snapshots");
    host.set_local_contract_debug_execution();
    let mut vm = IVM::new(u64::MAX);
    let path_ptr = store_state_path_tlv(&mut vm, &path);
    let filler = make_tlv(PointerType::Blob as u16, b"");
    while vm.alloc_input_tlv(&filler).is_ok() {}
    vm.set_register(10, path_ptr);
    assert_eq!(
        (host.syscall(ivm_sys::SYSCALL_STATE_GET, &mut vm))
            .expect("expected successful fixture result"),
        test_state_value_gas(
            &path,
            norito::to_bytes(&expected)
                .expect("encode state value")
                .len()
        ),
        "STATE_GET should spill to heap when input space is exhausted"
    );
    let out_ptr = vm.register(10);
    assert!(
        (ivm::Memory::HEAP_START..ivm::Memory::INPUT_START).contains(&out_ptr),
        "contract runtime state_get should return a heap pointer once input is full"
    );
    let tlv = vm.validate_tlv(out_ptr).expect("spilled snapshot tlv");
    assert_eq!(tlv.type_id, PointerType::NoritoBytes);
    let value: Vec<u8> = norito::decode_from_bytes(tlv.payload).expect("decode state value");
    assert_eq!(value, expected);
}
#[test]
fn lifecycle_completion_tombstone_tracks_the_physical_marker_read() {
    let authority: AccountId = fixture_account("alice");
    let contract_address = ContractAddress::derive(
        &"hash:0000000000000000000000000000000000000000000000000000000000000001#C50E"
            .parse()
            .expect("canonical test network id"),
        &authority,
        203,
        DataSpaceId::UNIVERSAL,
    )
    .expect("derive contract address");
    let transition = crate::smartcontracts::code::PendingContractLifecycle::Hajimari {
        transition_id: Hash::new(b"lifecycle-host-transition"),
        code_hash: Hash::new(b"lifecycle-code"),
    };
    let marker = crate::smartcontracts::code::contract_lifecycle_state_key(&contract_address);
    let mut host = CoreHost::new(authority).with_access_logging();
    host.set_contract_runtime_context(Some(ContractRuntimeExecutionContext {
        contract_subject: contract_address.subject_id(),
        contract_address: contract_address.clone(),
        contract_alias: None,
        entrypoint: "hajimari".to_owned(),
    }));
    host.set_contract_lifecycle_transition(&contract_address, transition);
    assert_eq!(host.durable_state_overlay.get(&marker), Some(&None));
    assert!(
        host.state_access_log
            .durable_read_paths
            .contains(marker.as_ref()),
        "the scheduler must fingerprint the exact lifecycle marker"
    );
    assert!(
        host.state_access_log
            .read_keys
            .contains(&format!("lifecycle:{contract_address}"))
    );
    assert!(
        host.state_access_log
            .write_keys
            .contains(&format!("lifecycle:{contract_address}"))
    );
}
#[test]
fn state_syscall_reads_scoped_overlay_spills_to_heap_when_input_fills() {
    let authority: AccountId = fixture_account("alice");
    let mut host = CoreHost::new(authority.clone());
    host.set_local_contract_debug_execution();
    let mut vm = IVM::new(10_000);
    let path: StatePath = "counter".parse().unwrap();
    let path_ptr = store_state_path_tlv(&mut vm, &path);
    let expected = vec![0x3C; 80];
    let value_ptr = store_tlv(
        &mut vm,
        PointerType::NoritoBytes,
        &norito::to_bytes(&expected).expect("encode state value"),
    );
    let contract = ContractAddress::derive(
        &"hash:0000000000000000000000000000000000000000000000000000000000000001#C50E"
            .parse()
            .expect("canonical test network id"),
        &authority,
        44,
        DataSpaceId::UNIVERSAL,
    )
    .expect("derive contract");
    host.set_contract_runtime_context(Some(ContractRuntimeExecutionContext {
        contract_subject: contract.subject_id(),
        contract_address: contract,
        contract_alias: Some("spill::overlay".parse().expect("contract alias")),
        entrypoint: "read".to_owned(),
    }));
    vm.set_register(10, path_ptr);
    vm.set_register(11, value_ptr);
    assert_eq!(
        (host.syscall(ivm_sys::SYSCALL_STATE_SET, &mut vm))
            .expect("expected successful fixture result"),
        test_state_value_gas(
            &path,
            norito::to_bytes(&expected)
                .expect("encode state value")
                .len()
        )
    );
    let filler = make_tlv(PointerType::Blob as u16, b"");
    while vm.alloc_input_tlv(&filler).is_ok() {}
    vm.set_register(10, path_ptr);
    assert_eq!(
        (host.syscall(ivm_sys::SYSCALL_STATE_GET, &mut vm))
            .expect("expected successful fixture result"),
        test_state_value_gas(
            &path,
            norito::to_bytes(&expected)
                .expect("encode state value")
                .len()
        ),
        "scoped overlay STATE_GET should spill to heap when input space is exhausted"
    );
    let out_ptr = vm.register(10);
    assert!(
        (ivm::Memory::HEAP_START..ivm::Memory::INPUT_START).contains(&out_ptr),
        "scoped overlay state_get should return a heap pointer once input is full"
    );
    let tlv = vm.validate_tlv(out_ptr).expect("spilled overlay tlv");
    assert_eq!(tlv.type_id, PointerType::NoritoBytes);
    let value: Vec<u8> = norito::decode_from_bytes(tlv.payload).expect("decode state value");
    assert_eq!(value, expected);
}
#[test]
fn state_syscall_ignores_unscoped_value_when_scoped_value_exists() {
    let authority: AccountId = fixture_account("alice");
    let path: StatePath = "counter".parse().unwrap();
    let contract = ContractAddress::derive(
        &"hash:0000000000000000000000000000000000000000000000000000000000000001#C50E"
            .parse()
            .expect("canonical test network id"),
        &authority,
        46,
        DataSpaceId::UNIVERSAL,
    )
    .expect("derive contract");
    let context = ContractRuntimeExecutionContext {
        contract_subject: contract.subject_id(),
        contract_address: contract,
        contract_alias: Some("scoped::base".parse().expect("contract alias")),
        entrypoint: "read".to_owned(),
    };
    let mut scope_host = CoreHost::new(authority.clone());
    scope_host.set_local_contract_debug_execution();
    scope_host.set_contract_runtime_context(Some(context.clone()));
    let scoped_path = scope_host
        .scoped_durable_state_path(&path)
        .expect("build scoped path")
        .expect("scoped path should exist");
    let mut world = World::new();
    world.smart_contract_state.insert(
        path.clone(),
        norito::to_bytes(&7_u64).expect("encode unscoped state value"),
    );
    world.smart_contract_state.insert(
        scoped_path,
        norito::to_bytes(&11_u64).expect("encode scoped state value"),
    );
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let state = State::new_for_testing(world, kura, query);
    super::pointer_abi_tests::establish_authenticated_axt_ledger_time(&state, 1);
    let mut host = CoreHost::from_state(authority, &state).expect("canonical state snapshots");
    host.set_local_contract_debug_execution();
    host.set_contract_runtime_context(Some(context));
    let mut vm = IVM::new(10_000);
    let path_ptr = store_state_path_tlv(&mut vm, &path);
    let filler = make_tlv(PointerType::Blob as u16, b"");
    while vm.alloc_input_tlv(&filler).is_ok() {}
    vm.set_register(10, path_ptr);
    assert_eq!(
        (host.syscall(ivm_sys::SYSCALL_STATE_GET, &mut vm))
            .expect("expected successful fixture result"),
        test_state_value_gas(
            &path,
            norito::to_bytes(&11_u64)
                .expect("encode scoped state value")
                .len()
        ),
        "STATE_GET must resolve only the scoped persisted value"
    );
    let out_ptr = vm.register(10);
    assert!(
        (ivm::Memory::HEAP_START..ivm::Memory::INPUT_START).contains(&out_ptr),
        "scoped base STATE_GET should spill to heap once input is exhausted"
    );
    let tlv = vm.validate_tlv(out_ptr).expect("scoped base tlv");
    assert_eq!(tlv.type_id, PointerType::NoritoBytes);
    let value: u64 = norito::decode_from_bytes(tlv.payload).expect("decode scoped state");
    assert_eq!(value, 11);
}
#[test]
fn state_count_syscall_strips_scope_and_applies_tombstones() {
    let authority: AccountId = fixture_account("alice");
    let contract = ContractAddress::derive(
        &"hash:0000000000000000000000000000000000000000000000000000000000000001#C50E"
            .parse()
            .expect("canonical test network id"),
        &authority,
        146,
        DataSpaceId::UNIVERSAL,
    )
    .expect("derive contract");
    let context = ContractRuntimeExecutionContext {
        contract_subject: contract.subject_id(),
        contract_address: contract,
        contract_alias: Some("state::count".parse().expect("contract alias")),
        entrypoint: "list".to_owned(),
    };
    let scoped_key: StatePath = "orders/2".parse().unwrap();
    let mut scope_host = CoreHost::new(authority.clone());
    scope_host.set_local_contract_debug_execution();
    scope_host.set_contract_runtime_context(Some(context.clone()));
    let scoped_path = scope_host
        .scoped_durable_state_path(&scoped_key)
        .expect("build scoped path")
        .expect("scoped path should exist");
    let mut world = World::new();
    world.smart_contract_state.insert(
        "orders/1".parse().unwrap(),
        norito::to_bytes(&1_u64).expect("encode unscoped state value"),
    );
    world.smart_contract_state.insert(
        scoped_path,
        norito::to_bytes(&2_u64).expect("encode scoped state value"),
    );
    world.smart_contract_state.insert(
        "accounts/1".parse().unwrap(),
        norito::to_bytes(&3_u64).expect("encode unrelated state value"),
    );
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let state = State::new_for_testing(world, kura, query);
    super::pointer_abi_tests::establish_authenticated_axt_ledger_time(&state, 1);
    let mut host = CoreHost::from_state(authority, &state).expect("canonical state snapshots");
    host.set_local_contract_debug_execution();
    host.set_contract_runtime_context(Some(context));
    let mut vm = IVM::new(10_000);
    let unscoped_key: StatePath = "orders/1".parse().unwrap();
    let unscoped_ptr = store_state_path_tlv(&mut vm, &unscoped_key);
    vm.set_register(10, unscoped_ptr);
    assert_eq!(
        (host.syscall(ivm_sys::SYSCALL_STATE_DEL, &mut vm))
            .expect("expected successful fixture result"),
        test_state_path_gas(&unscoped_key)
    );
    let prefix: StatePath = "orders".parse().unwrap();
    let prefix_ptr = store_state_path_tlv(&mut vm, &prefix);
    let (_, count_scan_work_gas) = host
        .count_durable_state_keys(&vm, &prefix, norito_blob(&prefix).len())
        .expect("measure scoped state count");
    vm.set_register(10, prefix_ptr);
    assert_eq!(
        (host.syscall(ivm_sys::SYSCALL_STATE_COUNT, &mut vm))
            .expect("expected successful fixture result"),
        ivm::host::STATE_QUERY_GAS_BASE.saturating_add(count_scan_work_gas)
    );
    assert_eq!(vm.register(10), 1);
    let scoped_ptr = store_state_path_tlv(&mut vm, &scoped_key);
    vm.set_register(10, scoped_ptr);
    assert_eq!(
        (host.syscall(ivm_sys::SYSCALL_STATE_HAS, &mut vm))
            .expect("expected successful fixture result"),
        test_state_path_gas(&scoped_key)
    );
    assert_eq!(vm.register(10), 1);
    vm.set_register(10, scoped_ptr);
    assert_eq!(
        (host.syscall(ivm_sys::SYSCALL_STATE_LEN, &mut vm))
            .expect("expected successful fixture result"),
        test_state_path_gas(&scoped_key)
    );
    assert_eq!(
        vm.register(10),
        u64::try_from(
            norito::to_bytes(&2_u64)
                .expect("encode scoped state value")
                .len()
        )
        .expect("length fits")
    );
    assert_eq!(vm.register(11), 1);
}
#[test]
fn state_syscall_scoped_overlay_overrides_scoped_base_without_touching_unscoped_state() {
    let authority: AccountId = fixture_account("alice");
    let path: StatePath = "counter".parse().unwrap();
    let contract = ContractAddress::derive(
        &"hash:0000000000000000000000000000000000000000000000000000000000000001#C50E"
            .parse()
            .expect("canonical test network id"),
        &authority,
        47,
        DataSpaceId::UNIVERSAL,
    )
    .expect("derive contract");
    let context = ContractRuntimeExecutionContext {
        contract_subject: contract.subject_id(),
        contract_address: contract,
        contract_alias: Some("scoped::overlay".parse().expect("contract alias")),
        entrypoint: "write".to_owned(),
    };
    let mut scope_host = CoreHost::new(authority.clone());
    scope_host.set_local_contract_debug_execution();
    scope_host.set_contract_runtime_context(Some(context.clone()));
    let scoped_path = scope_host
        .scoped_durable_state_path(&path)
        .expect("build scoped path")
        .expect("scoped path should exist");
    let mut world = World::new();
    world.smart_contract_state.insert(
        path.clone(),
        norito::to_bytes(&7_u64).expect("encode unscoped state value"),
    );
    world.smart_contract_state.insert(
        scoped_path.clone(),
        norito::to_bytes(&11_u64).expect("encode scoped state value"),
    );
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let state = State::new_for_testing(world, kura, query);
    super::pointer_abi_tests::establish_authenticated_axt_ledger_time(&state, 1);
    let mut host = CoreHost::from_state(authority, &state).expect("canonical state snapshots");
    host.set_local_contract_debug_execution();
    host.set_contract_runtime_context(Some(context));
    let mut vm = IVM::new(10_000);
    let path_ptr = store_state_path_tlv(&mut vm, &path);
    let value_ptr = store_tlv(
        &mut vm,
        PointerType::NoritoBytes,
        &norito::to_bytes(&22_u64).expect("encode overlay state value"),
    );
    vm.set_register(10, path_ptr);
    vm.set_register(11, value_ptr);
    assert_eq!(
        (host.syscall(ivm_sys::SYSCALL_STATE_SET, &mut vm))
            .expect("expected successful fixture result"),
        test_state_value_gas(
            &path,
            norito::to_bytes(&22_u64)
                .expect("encode overlay state value")
                .len()
        )
    );
    vm.set_register(10, path_ptr);
    assert_eq!(
        (host.syscall(ivm_sys::SYSCALL_STATE_GET, &mut vm))
            .expect("expected successful fixture result"),
        test_state_value_gas(
            &path,
            norito::to_bytes(&22_u64)
                .expect("encode overlay state value")
                .len()
        ),
        "scoped overlay should win over scoped persisted state"
    );
    let tlv = vm
        .memory
        .validate_tlv(vm.register(10))
        .expect("overlay state tlv");
    assert_eq!(tlv.type_id, PointerType::NoritoBytes);
    let value: u64 = norito::decode_from_bytes(tlv.payload).expect("decode overlay state");
    assert_eq!(value, 22);
    let overlay = host.drain_durable_state_overlay();
    assert!(
        !overlay.contains_key(&path),
        "scoped overlay should not write through the unscoped path"
    );
    let stored = overlay
        .get(&scoped_path)
        .and_then(Option::as_ref)
        .expect("scoped overlay entry");
    let stored_value: u64 =
        norito::decode_from_bytes(stored).expect("decode raw persisted overlay state");
    assert_eq!(stored_value, 22);
}
#[test]
fn state_syscall_scoped_delete_never_tombstones_unscoped_state() {
    let path: StatePath = "counter".parse().unwrap();
    let authority: AccountId = fixture_account("alice");
    let contract = ContractAddress::derive(
        &"hash:0000000000000000000000000000000000000000000000000000000000000001#C50E"
            .parse()
            .expect("canonical test network id"),
        &authority,
        45,
        DataSpaceId::UNIVERSAL,
    )
    .expect("derive contract");
    let context = ContractRuntimeExecutionContext {
        contract_subject: contract.subject_id(),
        contract_address: contract,
        contract_alias: Some("scope::delete".parse().expect("contract alias")),
        entrypoint: "write".to_owned(),
    };
    let mut path_host = CoreHost::new(authority.clone());
    path_host.set_local_contract_debug_execution();
    path_host.set_contract_runtime_context(Some(context.clone()));
    let scoped_path = path_host
        .scoped_durable_state_path(&path)
        .expect("build scoped path")
        .expect("contract context has a scoped path");
    let mut world = World::new();
    world.smart_contract_state.insert(
        path.clone(),
        norito::to_bytes(&7_u64).expect("encode unscoped sentinel"),
    );
    world.smart_contract_state.insert(
        scoped_path.clone(),
        norito::to_bytes(&9_u64).expect("encode scoped state value"),
    );
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let state = State::new_for_testing(world, kura, query);
    super::pointer_abi_tests::establish_authenticated_axt_ledger_time(&state, 1);
    let mut host = CoreHost::from_state(authority, &state).expect("canonical state snapshots");
    host.set_local_contract_debug_execution();
    host.set_contract_runtime_context(Some(context));
    let mut vm = IVM::new(10_000);
    let path_ptr = store_state_path_tlv(&mut vm, &path);
    vm.set_register(10, path_ptr);
    assert_eq!(
        (host.syscall(ivm_sys::SYSCALL_STATE_DEL, &mut vm))
            .expect("expected successful fixture result"),
        test_state_path_gas(&path)
    );
    vm.set_register(10, path_ptr);
    assert_eq!(
        (host.syscall(ivm_sys::SYSCALL_STATE_GET, &mut vm))
            .expect("expected successful fixture result"),
        test_state_path_gas(&path),
        "scoped tombstone should hide the scoped base value"
    );
    assert_eq!(vm.register(10), 0);
    let overlay = host.drain_durable_state_overlay();
    assert_eq!(overlay.get(&scoped_path), Some(&None));
    assert!(
        !overlay.contains_key(&path),
        "scoped deletion must not mutate the raw namespace"
    );
}
#[test]
fn state_syscall_isolates_paths_by_contract_runtime_context() {
    let authority: AccountId = fixture_account("alice");
    let mut host = CoreHost::new(authority.clone());
    host.set_local_contract_debug_execution();
    let mut vm = IVM::new(10_000);
    let path: StatePath = "counter".parse().unwrap();
    let path_ptr = store_state_path_tlv(&mut vm, &path);
    let contract_a = ContractAddress::derive(
        &"hash:0000000000000000000000000000000000000000000000000000000000000001#C50E"
            .parse()
            .expect("canonical test network id"),
        &authority,
        41,
        DataSpaceId::UNIVERSAL,
    )
    .expect("derive contract A");
    host.set_contract_runtime_context(Some(ContractRuntimeExecutionContext {
        contract_subject: contract_a.subject_id(),
        contract_address: contract_a.clone(),
        contract_alias: Some("fixture::shared".parse().expect("contract alias A")),
        entrypoint: "write".to_owned(),
    }));
    let value_a_ptr = store_tlv(
        &mut vm,
        PointerType::NoritoBytes,
        &norito::to_bytes(&11_u64).expect("encode state value A"),
    );
    vm.set_register(10, path_ptr);
    vm.set_register(11, value_a_ptr);
    assert_eq!(
        (host.syscall(ivm_sys::SYSCALL_STATE_SET, &mut vm))
            .expect("expected successful fixture result"),
        test_state_value_gas(
            &path,
            norito::to_bytes(&11_u64)
                .expect("encode state value A")
                .len()
        )
    );
    let contract_b = ContractAddress::derive(
        &"hash:0000000000000000000000000000000000000000000000000000000000000001#C50E"
            .parse()
            .expect("canonical test network id"),
        &authority,
        42,
        DataSpaceId::UNIVERSAL,
    )
    .expect("derive contract B");
    host.set_contract_runtime_context(Some(ContractRuntimeExecutionContext {
        contract_subject: contract_b.subject_id(),
        contract_address: contract_b.clone(),
        contract_alias: Some("fixture::shared".parse().expect("contract alias B")),
        entrypoint: "write".to_owned(),
    }));
    let alternate_value_pointer = store_tlv(
        &mut vm,
        PointerType::NoritoBytes,
        &norito::to_bytes(&22_u64).expect("encode state value B"),
    );
    vm.set_register(10, path_ptr);
    vm.set_register(11, alternate_value_pointer);
    assert_eq!(
        (host.syscall(ivm_sys::SYSCALL_STATE_SET, &mut vm))
            .expect("expected successful fixture result"),
        test_state_value_gas(
            &path,
            norito::to_bytes(&22_u64)
                .expect("encode state value B")
                .len()
        )
    );
    host.set_contract_runtime_context(Some(ContractRuntimeExecutionContext {
        contract_subject: contract_a.subject_id(),
        contract_address: contract_a,
        contract_alias: Some("fixture::shared".parse().expect("contract alias A read")),
        entrypoint: "read".to_owned(),
    }));
    vm.set_register(10, path_ptr);
    assert_eq!(
        (host.syscall(ivm_sys::SYSCALL_STATE_GET, &mut vm))
            .expect("expected successful fixture result"),
        test_state_value_gas(
            &path,
            norito::to_bytes(&11_u64)
                .expect("encode state value A")
                .len()
        )
    );
    let value_a_tlv = vm
        .memory
        .validate_tlv(vm.register(10))
        .expect("state get tlv A");
    let value_a: u64 =
        norito::decode_from_bytes(value_a_tlv.payload).expect("decode state value A");
    assert_eq!(value_a, 11);
    host.set_contract_runtime_context(Some(ContractRuntimeExecutionContext {
        contract_subject: contract_b.subject_id(),
        contract_address: contract_b,
        contract_alias: Some("fixture::shared".parse().expect("contract alias B read")),
        entrypoint: "read".to_owned(),
    }));
    vm.set_register(10, path_ptr);
    assert_eq!(
        (host.syscall(ivm_sys::SYSCALL_STATE_GET, &mut vm))
            .expect("expected successful fixture result"),
        test_state_value_gas(
            &path,
            norito::to_bytes(&22_u64)
                .expect("encode state value B")
                .len()
        )
    );
    let alternate_value_tlv = vm
        .memory
        .validate_tlv(vm.register(10))
        .expect("state get tlv B");
    let value_b: u64 =
        norito::decode_from_bytes(alternate_value_tlv.payload).expect("decode state value B");
    assert_eq!(value_b, 22);
}
#[test]
fn state_syscall_does_not_read_unscoped_key_when_scoped_key_is_missing() {
    let mut world = World::new();
    let path: StatePath = "counter".parse().unwrap();
    let value_bytes = norito::to_bytes(&7_u64).expect("encode unscoped sentinel");
    world
        .smart_contract_state
        .insert(path.clone(), value_bytes.clone());
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let state = State::new_for_testing(world, kura, query);
    super::pointer_abi_tests::establish_authenticated_axt_ledger_time(&state, 1);
    let authority: AccountId = fixture_account("alice");
    let mut host =
        CoreHost::from_state(authority.clone(), &state).expect("canonical state snapshots");
    host.set_local_contract_debug_execution();
    let contract = ContractAddress::derive(
        &"hash:0000000000000000000000000000000000000000000000000000000000000001#C50E"
            .parse()
            .expect("canonical test network id"),
        &authority,
        43,
        DataSpaceId::UNIVERSAL,
    )
    .expect("derive contract");
    host.set_contract_runtime_context(Some(ContractRuntimeExecutionContext {
        contract_subject: contract.subject_id(),
        contract_address: contract,
        contract_alias: Some("scope::reader".parse().expect("contract alias")),
        entrypoint: "read".to_owned(),
    }));
    let mut vm = IVM::new(10_000);
    let path_ptr = store_state_path_tlv(&mut vm, &path);
    vm.set_register(10, path_ptr);
    assert_eq!(
        (host.syscall(ivm_sys::SYSCALL_STATE_GET, &mut vm))
            .expect("expected successful fixture result"),
        test_state_path_gas(&path)
    );
    assert_eq!(
        vm.register(10),
        0,
        "an unscoped value must never cross into a deployed contract namespace"
    );
}
#[test]
fn vrf_epoch_seed_syscall_reads_only_the_exact_epoch() {
    let mut host = CoreHost::new(fixture_account("alice"));
    host.vrf_epoch_seeds.insert(7, [0x42; 32]);
    host.vrf_epoch_seeds.insert(99, [0xA5; 32]);
    for (epoch, expected) in [
        (0, None),
        (7, Some([0x42; 32])),
        (8, None),
        (99, Some([0xA5; 32])),
        (u64::MAX, None),
    ] {
        let mut vm = IVM::new(10_000);
        vm.set_register(10, epoch);
        vm.set_register(11, 71);
        let quote = host
            .prepare_syscall(ivm_sys::SYSCALL_VRF_EPOCH_SEED, &vm)
            .unwrap();
        let gas = host
            .syscall(ivm_sys::SYSCALL_VRF_EPOCH_SEED, &mut vm)
            .unwrap();
        assert_eq!(quote, ivm::vrf::epoch_seed_gas(expected.is_some()));
        assert_eq!(gas, quote);
        assert_eq!(vm.register(11), 71);
        match expected {
            Some(seed) => {
                let result = vm.validate_tlv(vm.register(10)).unwrap();
                assert_eq!(result.type_id, PointerType::Blob);
                assert_eq!(result.payload, seed);
            }
            None => assert_eq!(
                vm.register(10),
                0,
                "an absent epoch cannot fall back to another seed"
            ),
        }
    }
}
#[test]
fn vrf_epoch_seed_syscall_without_seed_snapshot_is_absent() {
    let world = World::new();
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let state = State::new_for_testing(world, kura, query);
    super::pointer_abi_tests::establish_authenticated_axt_ledger_time(&state, 1);
    let mut host = CoreHost::from_state(fixture_account("alice"), &state).unwrap();
    let mut vm = IVM::new(10_000);
    vm.set_register(10, 42);
    vm.set_register(11, 71);
    let quote = host
        .prepare_syscall(ivm_sys::SYSCALL_VRF_EPOCH_SEED, &vm)
        .unwrap();
    assert_eq!(
        host.syscall(ivm_sys::SYSCALL_VRF_EPOCH_SEED, &mut vm)
            .unwrap(),
        quote
    );
    assert_eq!(quote, ivm::vrf::epoch_seed_gas(false));
    assert_eq!(vm.register(10), 0);
    assert_eq!(vm.register(11), 71);
}
#[test]
fn zk_vote_tally_syscall_reads_world_snapshot() {
    let mut world = World::new();
    let mut election = ElectionState::default();
    let large_weight = u128::from(u64::MAX) + 1;
    election.options = 3;
    election.finalized = true;
    election.tally = vec![large_weight, 1, 0];
    world.elections.insert("election-1".to_string(), election);
    let backend = "pipa-r/pasta";
    let circuit_id = crate::zk::confidential_v2::CONFIDENTIAL_TRANSFER_V2_CIRCUIT_ID;
    let commitment = [0x61; 32];
    let schema_hash =
        schema_hash(crate::zk::confidential_v2::CONFIDENTIAL_TRANSFER_V2_PUBLIC_INPUTS_SCHEMA_V1);
    let mut rec = active_vk_record(
        commitment,
        schema_hash,
        backend,
        circuit_id,
        "transfer",
        Vec::new(),
    );
    rec.key = None;
    let vk_id = VerifyingKeyId::new(backend, "vk");
    world.verifying_keys.insert(vk_id, rec);
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let state = State::new_for_testing(world, kura, query);
    let view = state.view();
    let authority: AccountId = fixture_account("alice");
    let mut host = CoreHost::new(authority);
    host.set_zk_config(&view.zk);
    host.set_chain_id(&view.chain_id);
    host.set_zk_snapshots_from_state(&view)
        .expect("hydrate zk snapshots");
    assert!(host.prepared_verifying_keys.contains_key(&commitment));
    let mut vm = IVM::new(10_000);
    let req = ivm::zk_verify::VoteGetTallyRequest {
        election_id: "election-1".to_string(),
    };
    let req_bytes = norito::to_bytes(&req).expect("encode request");
    let req_ptr = store_tlv(&mut vm, PointerType::NoritoBytes, &req_bytes);
    vm.set_register(10, req_ptr);
    let quote = host
        .prepare_syscall(ivm_sys::SYSCALL_ZK_VOTE_GET_TALLY, &vm)
        .expect("quote vote tally");
    let gas = host
        .syscall(ivm_sys::SYSCALL_ZK_VOTE_GET_TALLY, &mut vm)
        .expect("vote tally");
    let out_ptr = vm.register(10);
    let tlv = vm.memory.validate_tlv(out_ptr).expect("output tlv");
    assert_eq!(tlv.type_id, PointerType::NoritoBytes);
    assert_eq!(
        gas,
        CoreHost::state_query_gas(req_bytes.len().saturating_add(tlv.payload.len()))
    );
    assert_eq!(quote, vm.remaining_gas());
    assert!(gas <= quote);
    let resp: ivm::zk_verify::VoteGetTallyResponse =
        norito::decode_from_bytes(tlv.payload).expect("decode response");
    assert!(resp.finalized);
    assert_eq!(resp.tally, vec![large_weight, 1, 0]);
}
#[test]
fn zk_vote_tally_syscall_rejects_noncanonical_selector_without_publishing_response() {
    let mut host = CoreHost::new(fixture_account("alice"));
    let mut vm = IVM::new(10_000);
    let request = ivm::zk_verify::VoteGetTallyRequest {
        election_id: "election/alias".to_owned(),
    };
    let payload = norito::to_bytes(&request).expect("encode request");
    let request_ptr = store_tlv(&mut vm, PointerType::NoritoBytes, &payload);
    vm.set_register(10, request_ptr);
    assert!(matches!(
        host.syscall(ivm_sys::SYSCALL_ZK_VOTE_GET_TALLY, &mut vm),
        Err(ivm::VMError::NoritoInvalid)
    ));
    assert_eq!(
        vm.register(10),
        request_ptr,
        "failed tally queries must not publish a response pointer"
    );
}
#[test]
fn zk_election_snapshot_rejects_invalid_shapes_without_replacement() {
    let mut host = CoreHost::new(fixture_account("alice"));
    let mut valid = BTreeMap::new();
    valid.insert("valid".to_string(), (2, false, vec![0, 0]));
    host.set_zk_elections_snapshot(valid)
        .expect("two-option election is valid");
    let mut invalid_selector = BTreeMap::new();
    invalid_selector.insert("invalid/election".to_owned(), (2, false, vec![0, 0]));
    assert!(matches!(
        host.set_zk_elections_snapshot(invalid_selector),
        Err(ivm::VMError::NoritoInvalid)
    ));
    assert!(
        host.zk_elections.contains_key("valid"),
        "a noncanonical snapshot key must not replace the prior snapshot"
    );
    for (id, options, tally) in [
        ("zero-options", 0, Vec::new()),
        ("one-option", 1, vec![0]),
        ("zero-tally", 64, Vec::new()),
        ("long-tally", 64, vec![0; 65]),
        ("oversized", 65, vec![0; 65]),
        ("overflow", 2, vec![u128::MAX, 1]),
    ] {
        let mut invalid = BTreeMap::new();
        invalid.insert(id.to_string(), (options, false, tally));
        assert!(matches!(
            host.set_zk_elections_snapshot(invalid),
            Err(ivm::VMError::NoritoInvalid)
        ));
        assert!(
            host.zk_elections.contains_key("valid"),
            "failed replacement must preserve the prior snapshot"
        );
    }
    let mut maximum = BTreeMap::new();
    maximum.insert("maximum".to_string(), (64, true, vec![1; 64]));
    host.set_zk_elections_snapshot(maximum)
        .expect("64-option election is valid");
    let response = host
        .vote_get_tally_response(&ivm::zk_verify::VoteGetTallyRequest {
            election_id: "maximum".to_string(),
        })
        .expect("bounded response");
    assert!(response.finalized);
    assert_eq!(response.tally.len(), 64);
    assert!(
        matches!(
            host.vote_get_tally_response(&ivm::zk_verify::VoteGetTallyRequest {
                election_id: ".maximum".to_owned(),
            }),
            Err(ivm::VMError::NoritoInvalid)
        ),
        "noncanonical requests must fail before lookup or response construction"
    );
    for (id, options, tally) in [
        ("response-zero-options", 0, Vec::new()),
        ("response-one-option", 1, vec![0]),
        ("response-zero-tally", 64, Vec::new()),
        ("response-long-tally", 64, vec![0; 65]),
        ("response-oversized", 65, vec![0; 65]),
        ("response-overflow", 2, vec![u128::MAX, 1]),
    ] {
        let tally_len = tally.len();
        Arc::make_mut(&mut host.zk_elections).insert(id.to_string(), (options, false, tally));
        assert!(matches!(
            host.vote_get_tally_response(&ivm::zk_verify::VoteGetTallyRequest {
                election_id: id.to_string(),
            }),
            Err(ivm::VMError::NoritoInvalid)
        ));
        assert_eq!(
            host.zk_elections
                .get(id)
                .expect("corrupt fixture remains intact")
                .2
                .len(),
            tally_len,
            "response rejection must not truncate the stored tally"
        );
    }
}
#[test]
fn zk_snapshot_hydration_rejects_invalid_election_selector_without_partial_replacement() {
    let mut host = CoreHost::new(fixture_account("alice"));

    let prior_asset = AssetDefinitionId::derive_from_components(
        DomainId::try_new("priorzk", "universal").expect("prior domain"),
        "asset".parse().expect("prior asset name"),
    );
    let mut prior_zk_state = ZkAssetState::default();
    prior_zk_state
        .push_commitments(
            &[[0x11; 32], [0x12; 32]],
            NonZeroUsize::new(8).expect("non-zero root history cap"),
        )
        .expect("canonical prior confidential tree roots");
    host.set_zk_roots_snapshot(&(BTreeMap::from([(prior_asset, prior_zk_state)])))
        .expect("seed prior root snapshot");

    host.set_zk_elections_snapshot(BTreeMap::from([(
        "prior-election".to_owned(),
        (2, true, vec![9, 0]),
    )]))
    .expect("seed prior election snapshot");

    let mut prior_vk = active_vk_record(
        [0x71; 32],
        schema_hash(crate::zk::confidential_v2::CONFIDENTIAL_TRANSFER_V2_PUBLIC_INPUTS_SCHEMA_V1),
        "pipa-r/pasta",
        crate::zk::confidential_v2::CONFIDENTIAL_TRANSFER_V2_CIRCUIT_ID,
        "core",
        Vec::new(),
    );
    prior_vk.key = None;
    host.set_verifying_keys(
        &(BTreeMap::from([(
            VerifyingKeyId::new("pipa-r/pasta", "prior-snapshot-vk"),
            prior_vk,
        )])),
    )
    .expect("seed prior verifying-key snapshot");
    host.set_zk_tree_roots_history_len(
        NonZeroUsize::new(7).expect("non-zero prior root history limit"),
    );

    let roots_before = host.zk_roots.clone();
    let elections_before = host.zk_elections.clone();
    let prepared_verifying_keys_before = host.prepared_verifying_keys.clone();
    let roots_history_len_before = host.zk_tree_roots_history_len;

    let mut world = World::new();
    world.elections.insert(
        "invalid/election".to_owned(),
        ElectionState {
            options: 2,
            tally: vec![0, 0],
            ..ElectionState::default()
        },
    );
    // A hostile WorldReadOnly view still exercises the host's atomic replacement
    // boundary. A durable State must refuse this world before hydration starts.
    let zk_cfg = crate::state::default_zk_config();
    assert!(matches!(
        host.set_zk_snapshots_from_world(&world.view(), &zk_cfg),
        Err(ivm::VMError::NoritoInvalid)
    ));
    assert_eq!(host.zk_roots, roots_before);
    assert_eq!(host.zk_elections, elections_before);
    assert_eq!(host.prepared_verifying_keys, prepared_verifying_keys_before);
    assert_eq!(host.zk_tree_roots_history_len, roots_history_len_before);

    let startup_error = State::try_new(
        world.sccp_committee_generations.allocation_budget().clone(),
        world,
        Kura::blank_kura_for_testing(),
        LiveQueryStore::start_test(),
        #[cfg(feature = "telemetry")]
        <_>::default(),
    )
    .err()
    .expect("a noncanonical election selector must refuse State startup");
    assert!(
        startup_error
            .to_string()
            .contains("elections: current election `invalid/election`"),
        "startup must identify the invalid election: {startup_error}"
    );
}
#[test]
fn zk_snapshot_hydration_enforces_v1_election_boundaries() {
    for (options, tally_len, valid) in [
        (0, 0, false),
        (1, 1, false),
        (2, 1, false),
        (2, 2, true),
        (64, 0, false),
        (64, 64, true),
        (64, 65, false),
        (65, 65, false),
    ] {
        let mut world = World::new();
        let election = ElectionState {
            options,
            tally: vec![0; tally_len],
            ..ElectionState::default()
        };
        world.elections.insert("candidate".to_string(), election);
        let kura = Kura::blank_kura_for_testing();
        let query = LiveQueryStore::start_test();
        let mut host = CoreHost::new(fixture_account("alice"));
        let mut prior = BTreeMap::new();
        prior.insert("prior".to_string(), (2, true, vec![9, 0]));
        host.set_zk_elections_snapshot(prior)
            .expect("seed prior snapshot");
        if valid {
            let state = State::new_for_testing(world, kura, query);
            let view = state.view();
            host.set_zk_snapshots_from_state(&view)
                .expect("valid option and tally widths hydrate from State");
            assert_eq!(
                host.zk_elections.get("candidate").cloned(),
                Some((options, false, vec![0; tally_len]))
            );
            assert!(!host.zk_elections.contains_key("prior"));
        } else {
            assert!(matches!(
                    host.set_zk_snapshots_from_world(
                        &world.view(),
                        &crate::state::default_zk_config(),
                    ),
                    Err(ivm::VMError::NoritoInvalid)
                ));
            assert_eq!(
                host.zk_elections.get("prior").cloned(),
                Some((2, true, vec![9, 0]))
            );
            assert!(!host.zk_elections.contains_key("candidate"));
            let startup_error = State::try_new(
                world.sccp_committee_generations.allocation_budget().clone(),
                world,
                kura,
                query,
                #[cfg(feature = "telemetry")]
                <_>::default(),
            )
            .err()
            .expect("invalid election shape must refuse State startup");
            assert!(
                startup_error
                    .to_string()
                    .contains("elections: current election `candidate`"),
                "startup must identify the invalid election: {startup_error}"
            );
        }
    }
    let mut world = World::new();
    world.elections.insert(
        "candidate/alias".to_owned(),
        ElectionState {
            options: 2,
            tally: vec![0, 0],
            ..ElectionState::default()
        },
    );
    let mut host = CoreHost::new(fixture_account("alice"));
    let mut prior = BTreeMap::new();
    prior.insert("prior".to_owned(), (2, true, vec![9, 0]));
    host.set_zk_elections_snapshot(prior)
        .expect("seed prior snapshot");
    assert!(matches!(
        host.set_zk_snapshots_from_world(&world.view(), &crate::state::default_zk_config(),),
        Err(ivm::VMError::NoritoInvalid)
    ));
    assert_eq!(
        host.zk_elections.get("prior").cloned(),
        Some((2, true, vec![9, 0])),
        "failed hydration must preserve the prior snapshot"
    );
    assert!(!host.zk_elections.contains_key("candidate/alias"));
    let startup_error = State::try_new(
        world.sccp_committee_generations.allocation_budget().clone(),
        world,
        Kura::blank_kura_for_testing(),
        LiveQueryStore::start_test(),
        #[cfg(feature = "telemetry")]
        <_>::default(),
    )
    .err()
    .expect("a noncanonical election selector must refuse State startup");
    assert!(
        startup_error
            .to_string()
            .contains("elections: current election `candidate/alias`"),
        "startup must identify the invalid election: {startup_error}"
    );
}
#[test]
fn from_state_hydrates_zk_snapshots() {
    let mut world = World::new();
    let mut parameters = crate::state::parameter_cell_block_for_testing(&world)
        .expect("funded parameter fixture block");
    for parameter in [
        iroha_data_model::parameter::SmartContractParameter::MaxOutputItems(
            NonZeroU64::new(23).expect("non-zero item limit"),
        ),
        iroha_data_model::parameter::SmartContractParameter::MaxOutputBytes(
            NonZeroU64::new(65_536).expect("non-zero byte limit"),
        ),
    ] {
        crate::state::set_parameter_cell_for_testing(
            &mut parameters,
            iroha_data_model::parameter::Parameter::SmartContract(parameter),
            world.sccp_committee_generations.allocation_budget(),
        )
        .expect("valid fixture parameters");
    }
    parameters.commit();
    let asset_def_id: AssetDefinitionId =
        iroha_data_model::asset::AssetDefinitionId::derive_from_components(
            DomainId::try_new("zkd", "universal").unwrap(),
            "zcoin".parse().unwrap(),
        );
    let mut zk_state = ZkAssetState::default();
    zk_state
        .push_commitments(
            &[[1u8; 32], [2u8; 32]],
            NonZeroUsize::new(64).expect("non-zero root history cap"),
        )
        .expect("canonical confidential tree roots");
    let expected_roots = zk_state.root_history.clone();
    world.zk_assets.insert(asset_def_id.clone(), zk_state);
    let election = ElectionState {
        options: 2,
        finalized: true,
        tally: vec![1, 2],
        ..Default::default()
    };
    world.elections.insert("election-1".to_string(), election);
    let backend = "pipa-r/pasta";
    let circuit_id = crate::zk::confidential_v2::CONFIDENTIAL_TRANSFER_V2_CIRCUIT_ID;
    let commitment = [0x62; 32];
    let schema_hash =
        schema_hash(crate::zk::confidential_v2::CONFIDENTIAL_TRANSFER_V2_PUBLIC_INPUTS_SCHEMA_V1);
    let mut rec = active_vk_record(
        commitment,
        schema_hash,
        backend,
        circuit_id,
        "transfer",
        Vec::new(),
    );
    rec.key = None;
    let vk_id = VerifyingKeyId::new(backend, "vk");
    world.verifying_keys.insert(vk_id, rec);
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let state = State::new_for_testing(world, kura, query);
    super::pointer_abi_tests::establish_authenticated_axt_ledger_time(&state, 1);
    let expected_zk_gas_schedule = {
        let view = state.view();
        ivm::gas::ZkGasScheduleV1::from_rates(
            view.zk.gas.proof_base,
            view.zk.gas.per_public_input,
            view.zk.gas.per_proof_byte,
        )
    };
    let authority: AccountId = fixture_account("alice");
    let mut host = CoreHost::from_state(authority, &state).expect("canonical state snapshots");
    assert_eq!(
        host.instruction_queue_limits,
        HostOutputLimits::new(23, 65_536)
    );
    assert_eq!(host.zk_gas_schedule(), expected_zk_gas_schedule);
    assert_eq!(host.default.zk_gas_schedule(), expected_zk_gas_schedule);
    let config = state.view().zk.clone();
    host.set_zk_config(&config);
    assert_eq!(host.zk_gas_schedule(), expected_zk_gas_schedule);
    assert_eq!(host.default.zk_gas_schedule(), expected_zk_gas_schedule);
    assert_eq!(
        host.zk_roots
            .get(&asset_def_id)
            .map(|snapshot| snapshot.root_history.clone())
            .unwrap_or_default(),
        expected_roots
    );
    let (options, finalized, tally) =
        host.zk_elections
            .get("election-1")
            .cloned()
            .unwrap_or((0, false, Vec::new()));
    assert_eq!(options, 2);
    assert!(finalized);
    assert_eq!(tally, vec![1, 2]);
    assert!(host.prepared_verifying_keys.contains_key(&commitment));
    let mut vm = IVM::new(10_000);
    let req = ivm::zk_verify::RootsGetRequest {
        asset_id: asset_def_id.to_string(),
        max: 2,
    };
    let req_bytes = norito::to_bytes(&req).expect("encode request");
    let req_ptr = store_tlv(&mut vm, PointerType::NoritoBytes, &req_bytes);
    vm.set_register(10, req_ptr);
    let quote = host
        .prepare_syscall(ivm_sys::SYSCALL_ZK_ROOTS_GET, &vm)
        .expect("quote roots get");
    let gas = host
        .syscall(ivm_sys::SYSCALL_ZK_ROOTS_GET, &mut vm)
        .expect("roots get");
    let out_ptr = vm.register(10);
    let tlv = vm.memory.validate_tlv(out_ptr).expect("roots output tlv");
    assert_eq!(tlv.type_id, PointerType::NoritoBytes);
    assert_eq!(
        gas,
        CoreHost::state_query_gas(req_bytes.len().saturating_add(tlv.payload.len()))
    );
    assert_eq!(quote, vm.remaining_gas());
    assert!(gas <= quote);
    let resp: ivm::zk_verify::RootsGetResponse =
        norito::decode_from_bytes(tlv.payload).expect("decode roots response");
    assert_eq!(resp.latest, *expected_roots.last().expect("latest root"));
    assert_eq!(resp.roots, expected_roots);
    assert_eq!(resp.height, 2);
}
#[test]
fn zk_roots_snapshot_uses_profile_empty_root_and_rejects_drift_atomically() {
    let asset = AssetDefinitionId::derive_from_components(
        DomainId::try_new("zkprofile", "universal").expect("domain"),
        "asset".parse().expect("asset name"),
    );
    let mut host = CoreHost::new(fixture_account("alice"));
    let empty_state = ZkAssetState::default();
    host.set_zk_roots_snapshot(&(BTreeMap::from([(asset.clone(), empty_state.clone())])))
        .expect("canonical empty tree snapshot");
    let response = host
        .roots_get_response(&ivm::zk_verify::RootsGetRequest {
            asset_id: asset.to_string(),
            max: 0,
        })
        .expect("read canonical empty root");
    let empty_root = empty_state.tree_profile.empty_root();
    assert_eq!(response.latest, empty_root);
    assert_eq!(response.roots, vec![empty_root]);
    assert_eq!(response.height, 0);
    let mut drifted = ZkAssetState::default();
    drifted.commitments.push([0x25; 32]);
    drifted.root_history.push([0x66; 32]);
    assert!(matches!(
        host.set_zk_roots_snapshot(&(BTreeMap::from([(asset.clone(), drifted)]))),
        Err(ivm::VMError::NoritoInvalid)
    ));
    let mut missing_current_root = ZkAssetState::default();
    missing_current_root.commitments.push([0x25; 32]);
    assert!(matches!(
        host.set_zk_roots_snapshot(&(BTreeMap::from([(asset.clone(), missing_current_root)]))),
        Err(ivm::VMError::NoritoInvalid)
    ));
    let mut drifted_checkpoint = ZkAssetState::default();
    drifted_checkpoint
        .frontier_checkpoints
        .push(crate::state::FrontierCheckpoint {
            height: 1,
            commitment_count: 0,
            root: [0x66; 32],
        });
    assert!(matches!(
        host.set_zk_roots_snapshot(&(BTreeMap::from([(asset.clone(), drifted_checkpoint)]))),
        Err(ivm::VMError::NoritoInvalid)
    ));
    let retained = host
        .roots_get_response(&ivm::zk_verify::RootsGetRequest {
            asset_id: asset.to_string(),
            max: 0,
        })
        .expect("prior snapshot remains readable");
    assert_eq!(retained, response);
}
#[test]
fn zk_roots_prepare_reserves_gas_at_history_cap_and_maximal_request() {
    let authority = fixture_account("alice");
    let asset = AssetDefinitionId::derive_from_components(
        DomainId::try_new("zkquote", "universal").expect("domain"),
        "asset".parse().expect("asset name"),
    );
    let mut host = CoreHost::new(authority);
    host.zk_tree_roots_history_len = NonZeroUsize::new(256).expect("non-zero root history");
    Arc::make_mut(&mut host.zk_roots).insert(
        asset.clone(),
        crate::state::IvmZkTreeRoots {
            tree_profile: crate::state::ConfidentialTreeProfile::PoseidonPastaV1,
            root_history: (0..384_u16)
                .map(|index| {
                    let mut root = [0_u8; 32];
                    root[..2].copy_from_slice(&index.to_le_bytes());
                    root
                })
                .collect(),
            commitment_count: 384,
        },
    );
    let request = ivm::zk_verify::RootsGetRequest {
        asset_id: asset.to_string(),
        max: u32::MAX,
    };
    let request_payload = norito::to_bytes(&request).expect("encode roots request");
    let mut vm = IVM::new(1_000_000);
    let request_ptr = store_tlv(&mut vm, PointerType::NoritoBytes, &request_payload);
    vm.set_register(10, request_ptr);
    let quote = host
        .prepare_syscall(ivm_sys::SYSCALL_ZK_ROOTS_GET, &vm)
        .expect("quote maximal roots request");
    let actual = host
        .syscall(ivm_sys::SYSCALL_ZK_ROOTS_GET, &mut vm)
        .expect("execute maximal roots request");
    assert_eq!(quote, vm.remaining_gas());
    assert!(actual <= quote);
    let response_tlv = vm
        .memory
        .validate_tlv(vm.register(10))
        .expect("roots response TLV");
    let response: ivm::zk_verify::RootsGetResponse =
        norito::decode_from_bytes(response_tlv.payload).expect("decode roots response");
    assert_eq!(response.height, 384);
    assert_eq!(response.roots.len(), 256);
    assert_eq!(
        response.latest,
        *host.zk_roots[&asset]
            .root_history
            .last()
            .expect("latest root")
    );
}
#[test]
fn state_syscall_flushes_to_wsv() {
    let world = World::new();
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let state = State::new_for_testing(world, kura, query);
    let authority: AccountId = fixture_account("alice");
    let header =
        iroha_data_model::block::BlockHeader::new(nonzero_ext::nonzero!(1_u64), None, None, 0, 0);
    let mut block = state.block(header);
    let mut stx = block.transaction();
    let mut host = local_contract_host(authority);
    host.set_durable_state_snapshot_from_world(&stx.world);
    let mut vm = IVM::new(10_000);
    let path: StatePath = "counter".parse().unwrap();
    let path_ptr = store_state_path_tlv(&mut vm, &path);
    let value_bytes = norito::to_bytes(&1_u64).expect("encode state value");
    let value_ptr = store_tlv(&mut vm, PointerType::NoritoBytes, &value_bytes);
    vm.set_register(10, path_ptr);
    vm.set_register(11, value_ptr);
    assert_eq!(
        (host.syscall(ivm_sys::SYSCALL_STATE_SET, &mut vm))
            .expect("expected successful fixture result"),
        test_state_value_gas(&path, value_bytes.len()),
        "STATE_SET should stage the overlay"
    );
    // Exercise the durable-state flush primitive directly. Local-debug
    // artifacts are intentionally rejected by the public apply path.
    host.flush_durable_state(&mut stx)
        .expect("flush overlay into transaction");
    let stored = stx
        .world
        .smart_contract_state
        .get(&path)
        .expect("stored value");
    let value: u64 = norito::decode_from_bytes(stored).expect("decode raw persisted state value");
    assert_eq!(value, 1);
}
fn dummy_env(
    circuit_id: &str,
    vk_hash: [u8; 32],
    public_inputs: Vec<u8>,
    proof_bytes: Vec<u8>,
) -> Vec<u8> {
    let env = iroha_data_model::zk::OpenVerifyEnvelope {
        backend: BackendTag::NativePipaRPasta,
        circuit_id: circuit_id.to_string(),
        vk_hash,
        public_inputs,
        proof_bytes,
        aux: Vec::new(),
    };
    norito::to_bytes(&env).expect("serialize envelope")
}
fn mutated_dummy_env(
    circuit_id: &str,
    vk_hash: [u8; 32],
    public_inputs: Vec<u8>,
    proof_bytes: Vec<u8>,
    mutate: impl FnOnce(&mut iroha_data_model::zk::OpenVerifyEnvelope),
) -> Vec<u8> {
    let mut env: iroha_data_model::zk::OpenVerifyEnvelope =
        ivm::host::decode_canonical_zk_envelope(&dummy_env(
            circuit_id,
            vk_hash,
            public_inputs,
            proof_bytes,
        ))
        .expect("decode dummy envelope");
    mutate(&mut env);
    norito::to_bytes(&env).expect("serialize mutated envelope")
}
fn canonical_confidential_transfer_vk_bytes() -> Vec<u8> {
    crate::zk::confidential_v2::confidential_transfer_v2_vk_box()
        .expect("canonical confidential-transfer verifier key")
        .bytes
}
fn schema_hash(public_inputs: &[u8]) -> [u8; 32] {
    Hash::new(public_inputs).into()
}
fn active_vk_record(
    commitment: [u8; 32],
    schema_hash: [u8; 32],
    backend: &str,
    circuit_id: &str,
    namespace: &str,
    vk_bytes: Vec<u8>,
) -> VerifyingKeyRecord {
    let (backend_tag, curve) = if crate::zk::is_stark_fri_v1_backend(backend) {
        (BackendTag::Stark, "goldilocks")
    } else {
        (BackendTag::NativePipaRPasta, "vesta")
    };
    let mut rec = VerifyingKeyRecord::new_with_owner(
        1,
        circuit_id.to_string(),
        None,
        namespace,
        backend_tag,
        curve,
        schema_hash,
        commitment,
    );
    rec.status = iroha_data_model::confidential::ConfidentialStatus::Active;
    rec.max_proof_bytes = 1024 * 1024;
    rec.gas_schedule_id = Some(
        if crate::zk::is_stark_fri_v1_backend(backend) {
            "stark_default"
        } else {
            "native_pipa_r_default"
        }
        .to_owned(),
    );
    rec.vk_len = u32::try_from(vk_bytes.len()).expect("test verifying key length fits u32");
    rec.key = Some(VerifyingKeyBox::new(backend.into(), vk_bytes));
    rec
}
#[cfg(feature = "zk-stark")]
fn registered_stark_batch_fixture(
    host: &mut CoreHost,
    namespace: &str,
) -> iroha_data_model::zk::OpenVerifyEnvelope {
    let backend = "stark/fri/poseidon-x7-goldilocks-6x64-v1";
    let circuit_id = "stark/fri/poseidon-x7-goldilocks-6x64-v1:host-batch-v1";
    let vk = crate::zk_stark::StarkFriVerifyingKeyV1 {
        version: 1,
        circuit_id: circuit_id.to_owned(),
        n_log2: crate::zk_stark::STARK_FRI_CONSENSUS_MIN_N_LOG2,
        blowup_log2: crate::zk_stark::STARK_FRI_CONSENSUS_MIN_BLOWUP_LOG2,
        fold_arity: 2,
        queries: crate::zk_stark::STARK_FRI_CONSENSUS_MIN_QUERIES,
        merkle_arity: 2,
    };
    let vk_bytes = norito::encode_canonical(&vk).expect("canonical STARK verifier key");
    let vk_box = VerifyingKeyBox::new(backend.into(), vk_bytes.clone());
    let schema = b"host-batch-v1";
    let proof = crate::zk::prove_stark_fri_open_verify_envelope(
        backend,
        circuit_id,
        &vk_box,
        schema,
        vec![vec![[0x11; 32]]],
    )
    .expect("complete native STARK proof");
    let envelope =
        ivm::host::decode_canonical_zk_envelope(&proof.bytes).expect("decode STARK envelope");
    let commitment = CoreHost::hash_vk_bytes(backend, &vk_bytes);
    assert_eq!(envelope.vk_hash, commitment);
    let rec = active_vk_record(
        commitment,
        schema_hash(schema),
        backend,
        circuit_id,
        namespace,
        vk_bytes,
    );
    let mut map = BTreeMap::new();
    map.insert(VerifyingKeyId::new(backend, "vk_stark_batch"), rec);
    host.set_verifying_keys(&(map)).expect("set registry");
    envelope
}
include!("host/zk_verification_tests.rs");
#[test]
fn input_publish_tlv_is_forwarded_to_default_host() {
    // Build a minimal program that calls INPUT_PUBLISH_TLV and then HALT
    let mut vm = IVM::new(10_000);
    let owner: AccountId = fixture_account("alice");
    vm.set_host(CoreHost::with_accounts(
        owner.clone(),
        Arc::new(vec![owner.clone()]),
    ));
    let code = [
        encoding::wide::encode_sys(
            instruction::wide::system::SCALL,
            u8::try_from(ivm_sys::SYSCALL_INPUT_PUBLISH_TLV).expect("syscall id fits in u8"),
        )
        .to_le_bytes(),
        encoding::wide::encode_halt().to_le_bytes(),
    ]
    .concat();
    let program = build_program(&code, 0);
    vm.load_program(&program).unwrap();
    // Prepare a TLV in INPUT region and set x10 to its pointer (source)
    let payload = b"hello".to_vec();
    let mut tlv = Vec::new();
    tlv.extend_from_slice(&0x0006u16.to_be_bytes()); // PointerType::Blob
    tlv.push(1); // version
    let payload_len = u32::try_from(payload.len()).expect("payload length must fit into u32");
    tlv.extend_from_slice(&payload_len.to_be_bytes());
    tlv.extend_from_slice(&payload);
    let h: [u8; 32] = iroha_crypto::Hash::new(&payload).into();
    tlv.extend_from_slice(&h);
    let src_off = 256u64;
    vm.memory
        .preload_input(src_off, &tlv)
        .expect("preload input");
    let src_ptr = ivm::Memory::INPUT_START + src_off;
    vm.set_register(10, src_ptr);
    // Run: CoreHost should forward to DefaultHost and copy TLV, returning new INPUT pointer
    vm.run().expect("syscall ok");
    let dst_ptr = vm.register(10);
    assert!(dst_ptr >= ivm::Memory::INPUT_START);
    // Validate the copied TLV
    let v = vm
        .memory
        .validate_tlv(dst_ptr)
        .expect("validate copied tlv");
    assert_eq!(v.type_id, ivm::PointerType::Blob);
    assert_eq!(v.payload, payload.as_slice());
}
#[test]
fn get_public_input_uses_wsv_registry_and_charges_gas() {
    let authority: AccountId = fixture_account("alice");
    let name: Name = "pub_key".parse().unwrap();
    let payload = b"hello".to_vec();
    let tlv = make_tlv(PointerType::Blob as u16, &payload);
    let entry = norito::json::object([
        ("name", norito::json::Value::from(name.as_ref())),
        (
            "type_id",
            norito::json::Value::from(u64::from(PointerType::Blob as u16)),
        ),
        ("tlv_hex", norito::json::Value::from(hex::encode(&tlv))),
    ])
    .expect("registry entry");
    let registry = norito::json::Value::Array(vec![entry]);
    let custom = CustomParameter::new(ivm_metadata::public_inputs_id(), Json::from(registry));
    let mut params = Parameters::default();
    params.set_parameter(Parameter::Custom(custom));
    let mut host = CoreHost::new(authority);
    host.set_public_inputs_from_parameters(&params);
    let mut vm = IVM::new(10_000);
    let name_ptr = store_tlv(&mut vm, PointerType::Name, &norito_blob(&name));
    vm.set_register(10, name_ptr);
    let quote = host
        .prepare_syscall(ivm_sys::SYSCALL_GET_PUBLIC_INPUT, &vm)
        .expect("quote public input");
    let gas = host
        .syscall(ivm_sys::SYSCALL_GET_PUBLIC_INPUT, &mut vm)
        .expect("public input syscall");
    let out_ptr = vm.register(10);
    let out_tlv = vm.memory.validate_tlv(out_ptr).expect("output tlv");
    assert_eq!(out_tlv.type_id, PointerType::Blob);
    assert_eq!(out_tlv.payload, payload.as_slice());
    let expected_gas = PUBLIC_INPUT_GAS_BASE_DEFAULT.saturating_add(
        PUBLIC_INPUT_GAS_PER_BYTE_DEFAULT
            .saturating_mul(u64::try_from(tlv.len()).unwrap_or(u64::MAX)),
    );
    assert_eq!(gas, expected_gas);
    assert_eq!(quote, vm.remaining_gas());
    assert!(gas <= quote);
}
#[test]
fn get_public_input_uses_programmatic_setters() {
    let authority: AccountId = fixture_account("alice");
    let name: Name = "pub_key".parse().unwrap();
    let payload = b"hello".to_vec();
    let tlv = make_tlv(PointerType::Blob as u16, &payload);
    let mut inputs = BTreeMap::new();
    inputs.insert(name.clone(), tlv.clone());
    let mut host = CoreHost::new(authority).with_public_inputs(inputs);
    let mut vm = IVM::new(10_000);
    let name_ptr = store_tlv(&mut vm, PointerType::Name, &norito_blob(&name));
    vm.set_register(10, name_ptr);
    let quote = host
        .prepare_syscall(ivm_sys::SYSCALL_GET_PUBLIC_INPUT, &vm)
        .expect("quote programmatic public input");
    let gas = host
        .syscall(ivm_sys::SYSCALL_GET_PUBLIC_INPUT, &mut vm)
        .expect("public input syscall");
    let out_ptr = vm.register(10);
    let out_tlv = vm.memory.validate_tlv(out_ptr).expect("output tlv");
    assert_eq!(out_tlv.type_id, PointerType::Blob);
    assert_eq!(out_tlv.payload, payload.as_slice());
    let expected_gas = PUBLIC_INPUT_GAS_BASE_DEFAULT.saturating_add(
        PUBLIC_INPUT_GAS_PER_BYTE_DEFAULT
            .saturating_mul(u64::try_from(tlv.len()).unwrap_or(u64::MAX)),
    );
    assert_eq!(gas, expected_gas);
    assert_eq!(quote, vm.remaining_gas());
    assert!(gas <= quote);
}
#[test]
fn get_public_input_exposes_trigger_event_args() {
    let authority: AccountId = fixture_account("alice");
    let args = Json::from(norito::json!({
        "kind": "asset_change",
        "amount_i64": 10
    }));
    let mut host = CoreHost::with_accounts_and_args(
        authority.clone(),
        Arc::new(vec![authority]),
        args.clone(),
    );
    let mut vm = IVM::new(10_000);
    let name: Name = TRIGGER_EVENT_PUBLIC_INPUT_KEY.parse().unwrap();
    let name_ptr = store_tlv(&mut vm, PointerType::Name, &norito_blob(&name));
    vm.set_register(10, name_ptr);
    let quote = host
        .prepare_syscall(ivm_sys::SYSCALL_GET_PUBLIC_INPUT, &vm)
        .expect("quote trigger event public input");
    let gas = host
        .syscall(ivm_sys::SYSCALL_GET_PUBLIC_INPUT, &mut vm)
        .expect("public input syscall");
    let out_ptr = vm.register(10);
    let out_tlv = vm.memory.validate_tlv(out_ptr).expect("output tlv");
    assert_eq!(out_tlv.type_id, PointerType::Json);
    let decoded: Json = norito::decode_from_bytes(out_tlv.payload).expect("decode json");
    assert_eq!(decoded, args);
    let expected_tlv = make_tlv(PointerType::Json as u16, &norito_blob(&decoded));
    let expected_gas = PUBLIC_INPUT_GAS_BASE_DEFAULT.saturating_add(
        PUBLIC_INPUT_GAS_PER_BYTE_DEFAULT
            .saturating_mul(u64::try_from(expected_tlv.len()).unwrap_or(u64::MAX)),
    );
    assert_eq!(gas, expected_gas);
    assert_eq!(quote, vm.remaining_gas());
    assert!(gas <= quote);
}
include!("host/prepared_public_arguments_tests.rs");
#[test]
#[cfg(debug_assertions)]
fn compiled_wrapper_accepts_exact_argument_record_cap_and_preflights_heap() {
    use iroha_data_model::smart_contract::entrypoint::{
        EntrypointArgumentRecordV1, EntrypointValueAtomV1, MAX_ENTRYPOINT_ARGUMENT_RECORD_BYTES,
        entrypoint_argument_schema_hash_v1,
    };
    let (program, _) = kotodama_lang::compiler::Compiler::new()
        .compile_source_with_manifest(
            r"
seiyaku PreparedBoundaryArguments {
  view fn invoke(bytes small, bytes payload) authorize(anyone) -> bytes {
    let _small = small;
    return payload;
  }
}
",
        )
        .expect("compile bytes-parameter contract");
    let metadata = ivm::ProgramMetadata::parse(&program).expect("parse contract metadata");
    let descriptor = metadata
        .contract_interface
        .as_ref()
        .expect("contract interface")
        .entrypoints
        .iter()
        .find(|entrypoint| entrypoint.name == "invoke")
        .expect("invoke entrypoint");
    let schema = descriptor
        .argument_schema
        .as_ref()
        .expect("argument schema");
    let schema_bytes = norito::to_bytes(schema).expect("encode argument schema");
    let schema_hash = entrypoint_argument_schema_hash_v1(&schema_bytes);
    let small_payload = b"fits-input";
    let small_envelope = make_tlv(PointerType::Blob as u16, small_payload);
    let mut large_payload_len = MAX_ENTRYPOINT_ARGUMENT_RECORD_BYTES.saturating_sub(512);
    let mut large_payload = Vec::new();
    let mut large_envelope_len = 0;
    let mut canonical = Vec::new();
    for _ in 0..8 {
        large_payload = vec![0x5a; large_payload_len];
        let large_envelope = make_tlv(PointerType::Blob as u16, &large_payload);
        large_envelope_len = large_envelope.len();
        canonical = norito::to_bytes(&EntrypointArgumentRecordV1 {
            schema_hash,
            atoms: vec![
                EntrypointValueAtomV1::Pointer(small_envelope.clone()),
                EntrypointValueAtomV1::Pointer(large_envelope),
            ],
        })
        .expect("encode exact-bound argument record");
        match canonical.len().cmp(&MAX_ENTRYPOINT_ARGUMENT_RECORD_BYTES) {
            core::cmp::Ordering::Equal => break,
            core::cmp::Ordering::Less => {
                large_payload_len = large_payload_len
                    .saturating_add(MAX_ENTRYPOINT_ARGUMENT_RECORD_BYTES - canonical.len());
            }
            core::cmp::Ordering::Greater => {
                large_payload_len = large_payload_len
                    .saturating_sub(canonical.len() - MAX_ENTRYPOINT_ARGUMENT_RECORD_BYTES);
            }
        }
    }
    assert_eq!(
        canonical.len(),
        MAX_ENTRYPOINT_ARGUMENT_RECORD_BYTES,
        "the CoreHost fixture must exercise the inclusive signed wire boundary"
    );
    ivm::reset_argument_record_decode_count();
    let prepared =
        ivm::prepare_argument_record_with_gas_limit(schema, Arc::from(canonical), u64::MAX)
            .expect("prepare the exact-cap record with the gas-aware boundary");
    assert_eq!(
        prepared.canonical_bytes().len(),
        MAX_ENTRYPOINT_ARGUMENT_RECORD_BYTES
    );
    let entrypoint_pc =
        u64::try_from(metadata.prefix_len()).expect("prefix fits u64") + descriptor.entry_pc;
    let authority: AccountId = fixture_account("alice");
    let mut host = CoreHost::with_accounts_and_argument_record(
        authority.clone(),
        Arc::new(vec![authority.clone()]),
        Some(prepared.clone()),
    );
    let mut vm = IVM::new(u64::MAX);
    vm.load_program(&program).expect("load compiled contract");
    vm.set_register(1, vm.memory.code_len());
    vm.set_program_counter(entrypoint_pc)
        .expect("select compiled invoke table entry");
    prepared
        .precharge_vm(&mut vm)
        .expect("precharge exact-cap prepared arguments");
    vm.run_with_host(&mut host)
        .expect("host table preparation must admit the inclusive record cap");
    assert_eq!(ivm::argument_record_decode_count(), 1);
    let payload_pointer = vm
        .public_call_result_word(0)
        .expect("completed pointer result");
    assert!(
        (ivm::Memory::HEAP_START..ivm::Memory::INPUT_START).contains(&payload_pointer),
        "the decoded bytes pointer must be owned HEAP"
    );
    let payload = vm
        .validate_tlv(payload_pointer)
        .expect("owned decoded bytes TLV");
    assert_eq!(payload.type_id, PointerType::Blob);
    assert_eq!(payload.payload.len(), large_payload.len());
    assert!(payload.payload.iter().all(|byte| *byte == 0x5a));
    let exact_heap_bytes = (u64::try_from(large_envelope_len)
        .expect("bounded Blob envelope length")
        .checked_add(7)
        .expect("bounded aligned Blob envelope length")
        & !7)
        + 2 * 8
        + 8;
    let mut constrained_host = CoreHost::with_accounts_and_argument_record(
        authority.clone(),
        Arc::new(vec![authority]),
        Some(prepared.clone()),
    );
    let mut constrained_vm = IVM::new(u64::MAX);
    constrained_vm
        .load_program(&program)
        .expect("load constrained compiled contract");
    constrained_vm.set_register(1, constrained_vm.memory.code_len());
    constrained_vm
        .set_program_counter(entrypoint_pc)
        .expect("select constrained invoke table entry");
    constrained_vm
        .memory
        .set_heap_limit(exact_heap_bytes - 1)
        .expect("constrain HEAP by one byte");
    prepared
        .precharge_vm(&mut constrained_vm)
        .expect("precharge constrained prepared arguments");
    assert!(matches!(
        constrained_vm.run_with_host(&mut constrained_host),
        Err(VMError::OutOfMemory)
    ));
    assert_eq!(ivm::argument_record_decode_count(), 1);
    assert!(constrained_vm.call_result_word_count().is_err());
    assert_eq!(
        constrained_vm
            .alloc_heap(1)
            .expect("failed preflight leaves HEAP untouched"),
        ivm::Memory::HEAP_START
    );
    assert_eq!(
        store_tlv(
            &mut constrained_vm,
            PointerType::Blob,
            b"after-materialization-preflight"
        ),
        ivm::Memory::INPUT_START,
        "failed host table preparation must not consume INPUT"
    );
}
#[test]
fn get_public_input_missing_name_is_error() {
    let authority: AccountId = fixture_account("alice");
    let name: Name = "missing".parse().unwrap();
    let mut host = CoreHost::new(authority);
    let mut vm = IVM::new(10_000);
    let name_ptr = store_tlv(&mut vm, PointerType::Name, &norito_blob(&name));
    vm.set_register(10, name_ptr);
    assert_eq!(
        (host.prepare_syscall(ivm_sys::SYSCALL_GET_PUBLIC_INPUT, &vm))
            .expect("expected successful fixture result"),
        vm.remaining_gas()
    );
    let err = host
        .syscall(ivm_sys::SYSCALL_GET_PUBLIC_INPUT, &mut vm)
        .expect_err("missing name should error");
    assert!(matches!(err, VMError::PermissionDenied));
}
#[test]
fn get_public_input_preflights_exact_gas_before_guest_output() {
    let authority: AccountId = fixture_account("alice");
    let name: Name = "large_public_input".parse().expect("public input name");
    let tlv = make_tlv(PointerType::Blob as u16, &[0xabu8; 128]);
    let mut inputs = BTreeMap::new();
    inputs.insert(name.clone(), tlv);
    let mut host = CoreHost::new(authority).with_public_inputs(inputs);
    let mut vm = IVM::new(u64::MAX);
    let name_ptr = store_tlv(&mut vm, PointerType::Name, &norito_blob(&name));
    let mut code = Vec::new();
    code.extend_from_slice(
        &encoding::wide::encode_sys(
            instruction::wide::system::SCALL,
            u8::try_from(ivm_sys::SYSCALL_GET_PUBLIC_INPUT).expect("syscall id fits in u8"),
        )
        .to_le_bytes(),
    );
    code.extend_from_slice(&encoding::wide::encode_halt().to_le_bytes());
    vm.load_program(&build_program(&code, 0))
        .expect("load public-input program");
    vm.set_register(10, name_ptr);
    vm.set_register(11, 0xfeed);
    vm.set_gas_limit(20);
    let error = vm
        .run_with_host(&mut host)
        .expect_err("public input response exceeds the syscall reserve");
    assert!(matches!(error, ivm::VMError::OutOfGas));
    assert_eq!(vm.remaining_gas(), 0);
    assert_eq!(vm.register(10), name_ptr, "must not allocate guest output");
    assert_eq!(vm.register(11), 0xfeed, "must not mutate output registers");
}
// Keep pointer-ABI boundary checks together so host runtime code remains auditable.
include!("host/pointer_abi_validation_tests.rs");
include!("host/pointer_abi_and_sm_tests.rs");
