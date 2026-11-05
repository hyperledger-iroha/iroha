//! Tests for executing triggers directly
#[path = "../support/contract_deployment.rs"]
mod contract_deployment;
use crate::triggers::get_asset_value;
use eyre::{Result, WrapErr};
use futures_util::StreamExt as _;
use integration_tests::{sandbox, sync::run_on_dedicated_thread};
use iroha::{
    crypto::KeyPair,
    data_model::{
        IdBox, ValidationFail,
        events::pipeline::{TransactionEventFilter, TransactionStatus},
        isi::{
            Instruction, InstructionType,
            error::{InstructionExecutionError, InvalidParameterError, RepetitionError},
        },
        prelude::*,
        query::{
            error::FindError,
            parameters::FetchSize,
            trigger::{FindActiveTriggerIds, FindTriggerById, FindTriggers},
        },
        smart_contract::{ContractAddress, ContractAlias, ContractArtifactId},
        transaction::Executable,
        transaction::error::TransactionRejectionReason,
    },
};
use iroha_executor_data_model::permission::trigger::CanRegisterTrigger;
use iroha_model_base::domain::DomainId;
use iroha_model_base::metadata::Metadata;
use iroha_model_base::name::Name;
use iroha_test_network::*;
use iroha_test_samples::{ALICE_ID, load_sample_ivm};
use mint_rose_trigger_data_model::MintRoseArgs;
use std::{collections::BTreeSet, num::NonZeroU64};
use tokio::time::{Duration, Instant, sleep, timeout};
const TRIGGER_NAME: &str = "mint_rose";
fn contract_entrypoint_metadata(entrypoint: &str) -> Metadata {
    let mut metadata = Metadata::default();
    metadata.insert(
        "contract_entrypoint"
            .parse::<Name>()
            .expect("static contract entrypoint metadata key is valid"),
        Json::new(entrypoint.to_owned()),
    );
    metadata
}
// The genesis trigger may name its eventual alias before deployment, but no
// callback executes until the exact artifact and live subject have been bound.
fn mint_rose_fixture_builder(alias: &str) -> NetworkBuilder {
    NetworkBuilder::new()
        .with_genesis_instruction(Grant::account_permission(
            iroha_executor_data_model::permission::smart_contract::CanManageSmartContractCode,
            ALICE_ID.clone(),
        ))
        .with_genesis_instruction(Grant::account_permission(
            contract_deployment::contract_alias_management_permission(alias),
            ALICE_ID.clone(),
        ))
}

fn mint_rose_trigger_metadata(bytecode: &IvmBytecode, alias: &ContractAlias) -> Result<Metadata> {
    let verified = ivm::verify_contract_artifact(bytecode.as_ref())?;
    let mut metadata = contract_entrypoint_metadata("run");
    metadata.insert(
        "contract_alias".parse::<Name>()?,
        Json::new(alias.to_string()),
    );
    metadata.insert(
        "contract_code_hash".parse::<Name>()?,
        Json::new(hex::encode(verified.code_hash.as_ref())),
    );
    // The signed ExecuteTrigger event remains the only source of callback arguments.
    Ok(metadata)
}

fn mint_rose_deployment_grants(address: &ContractAddress) -> Result<[InstructionBox; 2]> {
    use iroha_executor_data_model::permission::{
        asset::CanMintAssetWithDefinition, smart_contract::CanUseContractPermission,
    };
    Ok([
        Grant::account_permission(
            CanUseContractPermission {
                contract: address.clone(),
                permission: "Admin".parse()?,
            },
            ALICE_ID.clone(),
        )
        .into(),
        Grant::account_permission(
            CanMintAssetWithDefinition {
                asset_definition: AssetDefinitionId::derive_from_components(
                    DomainId::try_new("wonderland", "universal")?,
                    "rose".parse()?,
                ),
            },
            address.subject_id(),
        )
        .into(),
    ])
}

fn require_mint_rose_binding(
    binding: &iroha_torii::ContractAliasResolveResponseDto,
    alias: &ContractAlias,
    address: &ContractAddress,
) -> Result<()> {
    eyre::ensure!(
        binding.contract_alias == alias.as_ref()
            && binding.contract_address == address.as_ref()
            && binding.contract_subject_account == address.subject_id().to_string()
            && binding.dataspace == "universal"
            && binding.source == "world_state"
            && binding.contract_alias_binding.alias == alias.as_ref()
            && binding.contract_alias_binding.status == "permanent"
            && binding.contract_alias_binding.lease_expiry_ms.is_none()
            && binding.contract_alias_binding.grace_until_ms.is_none(),
        "mint callback alias did not resolve to its exact live contract and immutable subject"
    );
    Ok(())
}

async fn deploy_mint_rose_callback(
    client: &iroha::blocking::Client,
    bytecode: IvmBytecode,
    alias: ContractAlias,
) -> Result<()> {
    let client = client.clone();
    run_on_dedicated_thread(move || -> Result<()> {
        let verified = ivm::verify_contract_artifact(bytecode.as_ref())?;
        let (address, code_hash, _, _) = contract_deployment::deploy_contract_locally_signed(
            &client,
            bytecode.as_ref(),
            alias.clone(),
        )?;
        eyre::ensure!(
            code_hash == hex::encode(verified.code_hash.as_ref()),
            "deployment changed the trigger artifact hash"
        );
        client.submit_all(
            mint_rose_deployment_grants(&address)?,
            FeePaymentIntent::authority(Vec::new(), None),
        )?;
        let response = client.client().post_contract_alias_resolve(&alias)?;
        eyre::ensure!(
            response.status() == reqwest::StatusCode::OK,
            "mint callback alias read failed with {}",
            response.status()
        );
        eyre::ensure!(
            response.body().len() <= 16 * 1024,
            "mint callback alias response exceeds fixture bound"
        );
        let binding = norito::json::from_slice(response.body())?;
        require_mint_rose_binding(&binding, &alias, &address)?;
        let actual = client
            .client()
            .get_contract_code_bytes(&ContractArtifactId::for_address(
                &address,
                verified.code_hash,
            )?)?;
        eyre::ensure!(
            actual.as_slice() == bytecode.as_ref(),
            "live callback artifact differs from registered trigger bytes"
        );
        Ok(())
    })
    .await??;
    Ok(())
}

#[test]
fn mint_rose_metadata_keeps_event_arguments_and_exact_artifact_alias() -> Result<()> {
    let bytecode = load_sample_ivm("mint_rose_trigger");
    let alias = ContractAlias::from_components("argument_mint_rose", None, "universal")?;
    let verified = ivm::verify_contract_artifact(bytecode.as_ref())?;
    let metadata = mint_rose_trigger_metadata(&bytecode, &alias)?;
    let mut expected = Metadata::default();
    expected.insert(
        "contract_entrypoint".parse::<Name>()?,
        Json::new("run".to_owned()),
    );
    expected.insert(
        "contract_alias".parse::<Name>()?,
        Json::new(alias.to_string()),
    );
    expected.insert(
        "contract_code_hash".parse::<Name>()?,
        Json::new(hex::encode(verified.code_hash.as_ref())),
    );
    assert_eq!(metadata, expected);
    assert!(metadata.get(&"contract_payload".parse::<Name>()?).is_none());
    assert!(mint_rose_trigger_metadata(&IvmBytecode::from_compiled(vec![0]), &alias).is_err());
    Ok(())
}

#[test]
fn mint_rose_execute_arguments_encode_the_admitted_callback_quantity() -> Result<()> {
    use ivm_abi::entrypoint::{
        EntrypointArgumentRecordV1, EntrypointValueAtomV1, EntrypointValueKindV1,
        EntrypointValueTypeNodeV1,
    };

    let artifact = load_sample_ivm("mint_rose_trigger");
    let verified = ivm::verify_contract_artifact(artifact.as_ref())?;
    let schema = verified
        .contract_interface
        .entrypoints
        .iter()
        .find(|entrypoint| entrypoint.name == "run")
        .and_then(|entrypoint| entrypoint.argument_schema.as_ref())
        .expect("the admitted mint callback declares its argument schema");
    assert_eq!(schema.fields.len(), 1);
    assert_eq!(schema.fields[0].name, "val");
    assert!(matches!(
        schema.fields[0].ty.nodes.as_slice(),
        [EntrypointValueTypeNodeV1::Leaf(
            EntrypointValueKindV1::Quantity
        )]
    ));

    let trigger_id = TRIGGER_NAME.parse::<TriggerId>()?;
    for val in [0, 1, 42, u32::MAX] {
        let call = ExecuteTrigger::new(trigger_id.clone()).with_args(MintRoseArgs { val });
        let encoded = ivm_abi::arguments::encode_argument_record_from_json(schema, &call.args)?;
        let record: EntrypointArgumentRecordV1 = ivm_abi::codec::decode_canonical_norito(&encoded)?;
        assert_eq!(
            record.atoms,
            vec![EntrypointValueAtomV1::Pointer(
                ivm_abi::numeric_tlv::encode_quantity(&Quantity::from(val))?
            )],
        );
    }
    let numeric_call = ExecuteTrigger::new(trigger_id).with_args(norito::json!({"val": 42}));
    assert!(matches!(
        ivm_abi::arguments::encode_argument_record_from_json(schema, &numeric_call.args),
        Err(ivm::VMError::DecodeError),
    ));
    Ok(())
}

#[test]
fn mint_rose_grants_are_exact_invocation_and_subject_owned_definition() -> Result<()> {
    use iroha_executor_data_model::permission::{
        asset::CanMintAssetWithDefinition, smart_contract::CanUseContractPermission,
    };
    let network = NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(Hash::new(
        b"mint-trigger-fixture",
    )));
    let address = ContractAddress::derive(
        &network,
        &ALICE_ID,
        0,
        iroha_model_base::topology::DataSpaceId::UNIVERSAL,
    )?;
    let expected: [InstructionBox; 2] = [
        Grant::account_permission(
            CanUseContractPermission {
                contract: address.clone(),
                permission: "Admin".parse()?,
            },
            ALICE_ID.clone(),
        )
        .into(),
        Grant::account_permission(
            CanMintAssetWithDefinition {
                asset_definition: "6pEP9RjNoZ7beWkT3pLfKoM1dyfi".parse()?,
            },
            address.subject_id(),
        )
        .into(),
    ];
    assert_eq!(mint_rose_deployment_grants(&address)?, expected);
    let other = ContractAddress::derive(
        &network,
        &ALICE_ID,
        1,
        iroha_model_base::topology::DataSpaceId::UNIVERSAL,
    )?;
    assert_ne!(mint_rose_deployment_grants(&other)?, expected);
    let wrong_effect: InstructionBox = Grant::account_permission(
        CanMintAssetWithDefinition {
            asset_definition: "6pEP9RjNoZ7beWkT3pLfKoM1dyfi".parse()?,
        },
        ALICE_ID.clone(),
    )
    .into();
    assert_ne!(expected[1], wrong_effect);
    Ok(())
}

#[test]
fn mint_rose_live_binding_rejects_identity_or_lease_substitution() -> Result<()> {
    let network = NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(Hash::new(
        b"mint-trigger-binding",
    )));
    let address = ContractAddress::derive(
        &network,
        &ALICE_ID,
        0,
        iroha_model_base::topology::DataSpaceId::UNIVERSAL,
    )?;
    let alias = ContractAlias::from_components("genesis_mint_rose", None, "universal")?;
    let make = || -> Result<iroha_torii::ContractAliasResolveResponseDto> {
        Ok(norito::json::from_value(norito::json!({
            "contract_alias": (alias.to_string()), "contract_address": (address.to_string()),
            "contract_subject_account": (address.subject_id().to_string()), "dataspace": "universal", "source": "world_state",
            "contract_alias_binding": {"alias": (alias.to_string()), "bound_at_ms": 1, "status": "permanent"}
        }))?)
    };
    require_mint_rose_binding(&make()?, &alias, &address)?;
    for field in 0..9 {
        let mut row = make()?;
        match field {
            0 => row.contract_alias = "other::universal".into(),
            1 => row.contract_address = "other".into(),
            2 => row.contract_subject_account = ALICE_ID.to_string(),
            3 => row.dataspace = "other".into(),
            4 => row.source = "cache".into(),
            5 => row.contract_alias_binding.alias = "other::universal".into(),
            6 => row.contract_alias_binding.status = "expired_pending_cleanup".into(),
            7 => row.contract_alias_binding.lease_expiry_ms = Some(2),
            _ => row.contract_alias_binding.grace_until_ms = Some(3),
        }
        assert!(require_mint_rose_binding(&row, &alias, &address).is_err());
    }
    Ok(())
}

async fn start_network(context: &'static str) -> Result<Option<sandbox::SerializedNetwork>> {
    sandbox::start_network_async_or_skip(NetworkBuilder::new(), context).await
}
async fn run_or_skip<F, Fut>(context: &'static str, test: F) -> Result<()>
where
    F: FnOnce() -> Fut,
    Fut: std::future::Future<Output = Result<()>>,
{
    if sandbox::handle_result(test().await, context)?.is_none() {
        return Ok(());
    }
    Ok(())
}
fn checked_random_trigger_account_keypair() -> KeyPair {
    KeyPair::try_random().expect("generate checked trigger account keypair")
}
#[test]
fn trigger_account_fixture_uses_checked_randomness() {
    let _keypair = checked_random_trigger_account_keypair();
}
async fn submit_instruction_and_wait(
    network: &sandbox::SerializedNetwork,
    client: iroha::blocking::Client,
    instruction: impl Into<InstructionBox>,
    context: &str,
) -> Result<HashOf<SignedTransaction>> {
    let client = integration_tests::sync::rebind_blocking_client(&client, |client| {
        client.transaction_status_timeout = network.sync_timeout();
    });
    let instruction = instruction.into();
    let context = context.to_string();
    run_on_dedicated_thread(move || {
        client
            .submit(
                instruction,
                iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
            )
            .wrap_err(context)
    })
    .await?
}
async fn quoted_transaction(
    client: &iroha::blocking::Client,
    instruction: InstructionBox,
) -> Result<SignedTransaction> {
    let account = client.account_client();
    let mut payload = account.prepare_transaction(iroha::client::AccountTransactionDraft::new(
        [instruction],
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        Metadata::default(),
    ))?;
    let quote = account
        .quote_fees(iroha::client::FeeQuoteRequest::AccountSignature { payload: &payload })
        .await?;
    eyre::ensure!(
        payload
            .fee_payment
            .has_same_payer_and_gas_bound(&quote.intent),
        "trigger fixture quote changed payer or gas bound"
    );
    payload.fee_payment = quote.intent;
    account.sign_transaction(payload).map_err(Into::into)
}
async fn wait_for_asset_value(
    client: &iroha::blocking::Client,
    asset_id: &AssetId,
    expected: Quantity,
    timeout_after: Duration,
    context: &str,
) -> Result<Quantity> {
    let deadline = Instant::now() + timeout_after;
    let mut last_observed = String::from("asset value was not queried");
    loop {
        if Instant::now() >= deadline {
            return Err(eyre::eyre!(
                "timed out waiting for asset {asset_id} to equal {expected} after {context}; last_observed={last_observed}"
            ));
        }
        match run_on_dedicated_thread({
            let client = client.clone();
            let asset_id = asset_id.clone();
            move || get_asset_value(&client, &asset_id)
        })
        .await?
        {
            Ok(value) => {
                if value == expected {
                    return Ok(value);
                }
                last_observed = value.to_string();
            }
            Err(error) => {
                last_observed = format!("query failed: {error}");
            }
        }
        sleep(Duration::from_millis(250)).await;
    }
}
async fn wait_for_trigger(
    client: &iroha::blocking::Client,
    trigger_id: &TriggerId,
    timeout_after: Duration,
    context: &str,
) -> Result<Trigger> {
    let deadline = Instant::now() + timeout_after;
    let mut last_observed = String::from("trigger was not queried");
    loop {
        if Instant::now() >= deadline {
            return Err(eyre::eyre!(
                "timed out waiting for trigger {trigger_id} after {context}; last_observed={last_observed}"
            ));
        }
        let found = run_on_dedicated_thread({
            let client = client.with_request_deadline(deadline.into_std())?;
            let trigger_id = trigger_id.clone();
            move || fixture_trigger(&client, &trigger_id)
        })
        .await??;
        eyre::ensure!(
            Instant::now() < deadline,
            "trigger read deadline elapsed after {context}"
        );
        if let Some(trigger) = found {
            return Ok(trigger);
        }
        last_observed = "trigger not found".to_string();
        sleep(Duration::from_millis(250)).await;
    }
}
fn fixture_trigger(client: &iroha::blocking::Client, id: &TriggerId) -> Result<Option<Trigger>> {
    // These fixtures register at most two triggers. Request one complete small
    // batch; a cursor/truncation cannot silently stand in for absence.
    let rows = client
        .client()
        .query(FindTriggers::new())
        .with_fetch_size(FetchSize {
            fetch_size: NonZeroU64::new(2),
        })
        .execute()?;
    eyre::ensure!(
        rows.continue_cursor().is_none(),
        "fixture trigger page is incomplete"
    );
    let rows = rows.take(3).collect::<std::result::Result<Vec<_>, _>>()?;
    eyre::ensure!(rows.len() <= 2, "fixture trigger page exceeds two rows");
    let ids = rows.iter().map(Trigger::id).collect::<BTreeSet<_>>();
    eyre::ensure!(
        ids.len() == rows.len(),
        "fixture trigger page has duplicate IDs"
    );
    Ok(rows.into_iter().find(|trigger| trigger.id() == id))
}
fn trigger_with_registration_header(trigger: &Trigger, header: BlockHeader) -> Result<Trigger> {
    let mut metadata = trigger.action().metadata().clone();
    metadata.insert(
        "__registered_block_height".parse::<Name>()?,
        Json::from(header.height().get()),
    );
    metadata.insert(
        "__registered_at_ms".parse::<Name>()?,
        Json::from(u64::try_from(header.creation_time().as_millis())?),
    );
    Ok(Trigger::new(
        trigger.id().clone(),
        trigger.action().clone().with_metadata(metadata),
    ))
}
fn require_identical_trigger(actual: &Trigger, expected: &Trigger) -> Result<()> {
    // Trigger's ordinary equality intentionally compares IDs only. This oracle
    // covers the executable, filter, authority, repeats and every metadata byte.
    eyre::ensure!(
        norito::codec::Encode::encode(actual) == norito::codec::Encode::encode(expected),
        "stored trigger action differs from its exact registered action"
    );
    Ok(())
}
#[test]
fn registered_trigger_oracle_covers_action_and_engine_metadata() -> Result<()> {
    let id: TriggerId = "identical_contract".parse()?;
    let trigger = Trigger::new(
        id.clone(),
        Action::new(
            vec![Log::new(Level::INFO, "original".to_owned())],
            Repeats::Indefinitely,
            ALICE_ID.clone(),
            ExecuteTriggerEventFilter::new()
                .for_trigger(id.clone())
                .under_authority(ALICE_ID.clone()),
        )?
        .with_metadata(contract_entrypoint_metadata("run")),
    );
    let header = BlockHeader::new(NonZeroU64::new(3).unwrap(), None, None, 5_000, 0);
    let expected = trigger_with_registration_header(&trigger, header)?;
    assert_eq!(
        expected
            .action()
            .metadata()
            .get(&"__registered_block_height".parse::<Name>()?),
        Some(&Json::from(3u64))
    );
    assert_eq!(
        expected
            .action()
            .metadata()
            .get(&"__registered_at_ms".parse::<Name>()?),
        Some(&Json::from(5_000u64))
    );
    require_identical_trigger(&expected, &expected)?;
    let wrong_id = Trigger::new("other_contract".parse()?, expected.action().clone());
    assert_ne!(wrong_id.id(), expected.id());
    assert!(require_identical_trigger(&wrong_id, &expected).is_err());
    // ID equality alone accepts both of these corrupted action oracles.
    assert_eq!(trigger, expected);
    assert!(require_identical_trigger(&trigger, &expected).is_err());
    let changed = Trigger::new(
        id,
        Action::new(
            vec![Log::new(Level::INFO, "substituted".to_owned())],
            Repeats::Indefinitely,
            ALICE_ID.clone(),
            trigger.action().filter().clone(),
        )?
        .with_metadata(expected.action().metadata().clone()),
    );
    assert_eq!(changed, expected);
    assert!(require_identical_trigger(&changed, &expected).is_err());
    Ok(())
}
fn is_trigger_register_collision(err: &eyre::Report, trigger_id: &TriggerId) -> bool {
    err.chain().any(|cause| {
        if let Some(InstructionExecutionError::Repetition(repetition)) =
            cause.downcast_ref::<InstructionExecutionError>()
            && *repetition.instruction() == InstructionType::Register
            && let IdBox::TriggerId(id) = &repetition.id
        {
            return id == trigger_id;
        }
        if let Some(ValidationFail::InstructionFailed(InstructionExecutionError::Repetition(
            repetition,
        ))) = cause.downcast_ref::<ValidationFail>()
            && *repetition.instruction() == InstructionType::Register
            && let IdBox::TriggerId(id) = &repetition.id
        {
            return id == trigger_id;
        }
        false
    })
}
fn is_permission_grant_repetition(
    err: &eyre::Report,
    expected: &iroha::data_model::permission::Permission,
) -> bool {
    err.chain().any(|cause| {
        let matches = |repetition: &RepetitionError| {
            *repetition.instruction() == InstructionType::Grant
                && matches!(&repetition.id, IdBox::Permission(permission) if permission == expected)
        };
        if let Some(InstructionExecutionError::Repetition(repetition)) =
            cause.downcast_ref::<InstructionExecutionError>()
        {
            return matches(repetition);
        }
        if let Some(ValidationFail::InstructionFailed(InstructionExecutionError::Repetition(
            repetition,
        ))) = cause.downcast_ref::<ValidationFail>()
        {
            return matches(repetition);
        }
        false
    })
}
fn is_missing_domain_rejection(reason: &TransactionRejectionReason, expected: &DomainId) -> bool {
    matches!(reason,
        TransactionRejectionReason::Validation(ValidationFail::InstructionFailed(
            InstructionExecutionError::Find(FindError::Domain(id)),
        )) if id == expected)
}
fn is_unknown_trigger_rejection(reason: &TransactionRejectionReason, expected: &TriggerId) -> bool {
    // Native authority validation precedes execution and rejects a pruned trigger
    // before the instruction executor could perform its own entity lookup.
    matches!(reason,
        TransactionRejectionReason::Validation(ValidationFail::NotPermitted(message))
            if message == &format!("permission references unknown trigger `{expected}`"))
}
#[test]
fn trigger_rejections_require_exact_typed_cause_and_identity() -> Result<()> {
    let missing_domain = DomainId::try_new("dummy", "universal")?;
    let other_domain = DomainId::try_new("other", "universal")?;
    let domain_error = TransactionRejectionReason::Validation(ValidationFail::InstructionFailed(
        InstructionExecutionError::Find(FindError::Domain(missing_domain.clone())),
    ));
    assert!(is_missing_domain_rejection(&domain_error, &missing_domain));
    assert!(!is_missing_domain_rejection(&domain_error, &other_domain));
    let trigger_id: TriggerId = "self_modifying_trigger".parse()?;
    let trigger_error = TransactionRejectionReason::Validation(ValidationFail::NotPermitted(
        format!("permission references unknown trigger `{trigger_id}`"),
    ));
    assert!(is_unknown_trigger_rejection(&trigger_error, &trigger_id));
    assert!(!is_unknown_trigger_rejection(
        &trigger_error,
        &"other".parse()?
    ));
    assert!(!is_missing_domain_rejection(
        &trigger_error,
        &missing_domain
    ));
    assert!(!is_unknown_trigger_rejection(&domain_error, &trigger_id));
    let wrong_reason = TransactionRejectionReason::Validation(ValidationFail::NotPermitted(
        format!("authority cannot execute trigger `{trigger_id}`"),
    ));
    assert!(!is_unknown_trigger_rejection(&wrong_reason, &trigger_id));
    Ok(())
}
#[test]
fn trigger_register_collision_is_detected() {
    let trigger_id: TriggerId = "collision_test".parse().expect("valid trigger id");
    let repetition = iroha::data_model::isi::error::RepetitionError {
        instruction: InstructionType::Register,
        id: IdBox::TriggerId(trigger_id.clone()),
    };
    let err = InstructionExecutionError::Repetition(repetition);
    let report = eyre::Report::new(err);
    assert!(is_trigger_register_collision(&report, &trigger_id));
}
#[test]
fn permission_grant_repetition_is_detected() {
    let permission: iroha::data_model::permission::Permission = CanRegisterTrigger {
        authority: ALICE_ID.clone(),
    }
    .into();
    let repetition = RepetitionError {
        instruction: InstructionType::Grant,
        id: IdBox::Permission(permission.clone()),
    };
    let err = InstructionExecutionError::Repetition(repetition);
    let report = eyre::Report::new(err);
    assert!(is_permission_grant_repetition(&report, &permission));
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn call_execute_trigger() -> Result<()> {
    let Some(network) = start_network(stringify!(call_execute_trigger)).await? else {
        return Ok(());
    };
    let test_client = network.client();
    network.ensure_blocks_with(|h| h.total >= 1).await?;
    run_or_skip(stringify!(call_execute_trigger), || async {
        let asset_definition_id = AssetDefinitionId::derive_from_components(
            DomainId::try_new("wonderland", "universal")?,
            "rose".parse()?,
        );
        let account_id = ALICE_ID.clone();
        let asset_id = AssetId::new(asset_definition_id, account_id);
        let prev_value = run_on_dedicated_thread({
            let client = test_client.clone();
            let asset_id = asset_id.clone();
            move || get_asset_value(&client, &asset_id)
        })
        .await??;
        let instruction = Mint::asset_quantity(1u32, asset_id.clone());
        let register_trigger =
            build_register_trigger_isi(asset_id.account(), vec![instruction.into()]);
        submit_instruction_and_wait(
            &network,
            test_client.clone(),
            register_trigger,
            stringify!(call_execute_trigger),
        )
        .await?;
        let trigger_id = TRIGGER_NAME.parse()?;
        let call_trigger = ExecuteTrigger::new(trigger_id);
        submit_instruction_and_wait(
            &network,
            test_client.clone(),
            Instruction::into_instruction_box(Box::new(call_trigger)),
            stringify!(call_execute_trigger),
        )
        .await?;
        let new_value = run_on_dedicated_thread({
            let client = test_client.clone();
            let asset_id = asset_id.clone();
            move || get_asset_value(&client, &asset_id)
        })
        .await??;
        assert_eq!(new_value, prev_value.checked_add(&Quantity::one()).unwrap());
        Ok(())
    })
    .await
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn execute_trigger_should_produce_event() -> Result<()> {
    let Some(network) = start_network(stringify!(execute_trigger_should_produce_event)).await?
    else {
        return Ok(());
    };
    let test_client = network.client();
    Box::pin(run_or_skip(
        stringify!(execute_trigger_should_produce_event),
        move || async move {
            let asset_definition_id = AssetDefinitionId::derive_from_components(
                DomainId::try_new("wonderland", "universal")?,
                "rose".parse()?,
            );
            let account_id = ALICE_ID.clone();
            let asset_id = AssetId::new(asset_definition_id, account_id.clone());
            let instruction = Mint::asset_quantity(1u32, asset_id.clone());
            let register_trigger =
                build_register_trigger_isi(asset_id.account(), vec![instruction.into()]);
            submit_instruction_and_wait(
                &network,
                test_client.clone(),
                register_trigger,
                stringify!(execute_trigger_should_produce_event),
            )
            .await?;
            let trigger_id = TRIGGER_NAME.parse::<TriggerId>()?;
            let call_trigger = ExecuteTrigger::new(trigger_id.clone());
            let filter = ExecuteTriggerEventFilter::new()
                .for_trigger(trigger_id)
                .under_authority(account_id);
            let mut events = timeout(
                network.sync_timeout(),
                test_client.account_client().events().subscribe([filter]),
            )
            .await
            .wrap_err("Timed out opening ExecuteTrigger event stream")??;
            submit_instruction_and_wait(
                &network,
                test_client.clone(),
                Instruction::into_instruction_box(Box::new(call_trigger)),
                stringify!(execute_trigger_should_produce_event),
            )
            .await?;
            let result = async {
                let next_event = timeout(network.sync_timeout(), events.next())
                    .await
                    .wrap_err("Timed out waiting for ExecuteTrigger event")?;
                let event =
                    next_event.ok_or_else(|| eyre::eyre!("Execute trigger event stream closed"))?;
                event
                    .wrap_err("ExecuteTrigger event listener returned an error")
                    .map(|_| ())?;
                Ok(())
            }
            .await;
            events.close().await?;
            result
        },
    ))
    .await
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn trigger_failure_should_not_cancel_other_triggers_execution() -> Result<()> {
    let Some(network) = start_network(stringify!(
        trigger_failure_should_not_cancel_other_triggers_execution
    ))
    .await?
    else {
        return Ok(());
    };
    let test_client = network.client();
    run_or_skip(
        stringify!(trigger_failure_should_not_cancel_other_triggers_execution),
        || async {
            let asset_definition_id = AssetDefinitionId::derive_from_components(
                DomainId::try_new("wonderland", "universal")?,
                "rose".parse()?,
            );
            let account_id = ALICE_ID.clone();
            let asset_id = AssetId::new(asset_definition_id, account_id.clone());
            let bad_trigger_id = "bad_trigger".parse::<TriggerId>()?;
            let missing_domain = DomainId::try_new("dummy", "universal")?;
            let fail_isi = Unregister::domain(missing_domain.clone());
            let register_bad_trigger = Register::trigger(Trigger::new(
                bad_trigger_id.clone(),
                Action::new(
                    vec![fail_isi],
                    Repeats::Indefinitely,
                    account_id.clone(),
                    ExecuteTriggerEventFilter::new()
                        .for_trigger(bad_trigger_id.clone())
                        .under_authority(account_id.clone()),
                )
                .expect("trigger action fixture satisfies validation invariants"),
            ));
            submit_instruction_and_wait(
                &network,
                test_client.clone(),
                register_bad_trigger,
                stringify!(trigger_failure_should_not_cancel_other_triggers_execution),
            )
            .await?;
            let trigger_id = TRIGGER_NAME.parse()?;
            let register_trigger = Register::trigger(Trigger::new(
                trigger_id,
                Action::new(
                    vec![Mint::asset_quantity(1u32, asset_id.clone())],
                    Repeats::Indefinitely,
                    account_id.clone(),
                    TimeEventFilter::new(ExecutionTime::PreCommit),
                )
                .expect("trigger action fixture satisfies validation invariants"),
            ));
            submit_instruction_and_wait(
                &network,
                test_client.clone(),
                register_trigger,
                stringify!(trigger_failure_should_not_cancel_other_triggers_execution),
            )
            .await?;
            let prev_asset_value = run_on_dedicated_thread({
                let client = test_client.clone();
                let asset_id = asset_id.clone();
                move || get_asset_value(&client, &asset_id)
            })
            .await??;
            let err = run_on_dedicated_thread({
                let client =
                    integration_tests::sync::rebind_blocking_client(&test_client, |client| {
                        client.transaction_status_timeout = network
                            .sync_timeout()
                            .min(std::time::Duration::from_secs(120));
                    });
                let bad_trigger_id = bad_trigger_id.clone();
                move || {
                    client.submit(
                        Instruction::into_instruction_box(Box::new(ExecuteTrigger::new(
                            bad_trigger_id,
                        ))),
                        iroha_data_model::transaction::FeePaymentIntent::authority(
                            Vec::new(),
                            None,
                        ),
                    )
                }
            })
            .await?
            .expect_err("should immediately result in error");
            let has_domain_error = err
                .chain()
                .find_map(|cause| cause.downcast_ref::<TransactionRejectionReason>())
                .is_some_and(|reason| is_missing_domain_rejection(reason, &missing_domain));
            eyre::ensure!(
                has_domain_error,
                "unexpected failing-trigger result: {err:?}"
            );
            // Submit without tx-status confirmation to avoid flakiness after a failed by-call
            // trigger; wait for the next non-empty block instead.
            let baseline_non_empty = network
                .peers()
                .iter()
                .filter_map(iroha_test_network::NetworkPeer::best_effort_block_height)
                .map(|height| height.non_empty)
                .max()
                .unwrap_or(0);
            let target_non_empty = baseline_non_empty.saturating_add(1);
            let log_tx = quoted_transaction(
                &test_client,
                Log::new(Level::INFO, "trigger probe".to_string()).into(),
            )
            .await?;
            run_on_dedicated_thread({
                let client = test_client.clone();
                move || client.submit_transaction(&log_tx)
            })
            .await??;
            network
                .ensure_blocks_with(|height| height.non_empty >= target_non_empty)
                .await?;
            let new_asset_value = run_on_dedicated_thread({
                let client = test_client.clone();
                let asset_id = asset_id.clone();
                move || get_asset_value(&client, &asset_id)
            })
            .await??;
            assert!(
                new_asset_value >= prev_asset_value.checked_add(&Quantity::one()).unwrap(),
                "expected pre-commit trigger to execute despite failing trigger"
            );
            Ok(())
        },
    )
    .await
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn trigger_should_not_be_executed_with_zero_repeats_count() -> Result<()> {
    let Some(network) = start_network(stringify!(
        trigger_should_not_be_executed_with_zero_repeats_count
    ))
    .await?
    else {
        return Ok(());
    };
    let test_client = network.client();
    network.ensure_blocks_with(|h| h.total >= 1).await?;
    run_or_skip(
        stringify!(trigger_should_not_be_executed_with_zero_repeats_count),
        || async {
            let asset_definition_id = AssetDefinitionId::derive_from_components(
                DomainId::try_new("wonderland", "universal")?,
                "rose".parse()?,
            );
            let account_id = ALICE_ID.clone();
            let asset_id = AssetId::new(asset_definition_id, account_id.clone());
            let trigger_id = "self_modifying_trigger".parse::<TriggerId>()?;
            let trigger_instructions = vec![Mint::asset_quantity(1u32, asset_id.clone())];
            let register_trigger = Register::trigger(Trigger::new(
                trigger_id.clone(),
                Action::new(
                    trigger_instructions,
                    1_u32,
                    account_id.clone(),
                    ExecuteTriggerEventFilter::new()
                        .for_trigger(trigger_id.clone())
                        .under_authority(account_id),
                )
                .expect("trigger action fixture satisfies validation invariants"),
            ));
            submit_instruction_and_wait(
                &network,
                test_client.clone(),
                register_trigger,
                stringify!(trigger_should_not_be_executed_with_zero_repeats_count),
            )
            .await?;
            // Saving current asset value
            let prev_asset_value = run_on_dedicated_thread({
                let client = test_client.clone();
                let asset_id = asset_id.clone();
                move || get_asset_value(&client, &asset_id)
            })
            .await??;
            // Executing trigger first time
            let execute_trigger = ExecuteTrigger::new(trigger_id.clone());
            let execute_trigger_first = execute_trigger.clone();
            submit_instruction_and_wait(
                &network,
                test_client.clone(),
                Instruction::into_instruction_box(Box::new(execute_trigger_first)),
                stringify!(trigger_should_not_be_executed_with_zero_repeats_count),
            )
            .await?;
            // Executing trigger second time
            let error = run_on_dedicated_thread({
                let client = test_client.clone();
                move || {
                    client.submit(
                        Instruction::into_instruction_box(Box::new(execute_trigger)),
                        iroha_data_model::transaction::FeePaymentIntent::authority(
                            Vec::new(),
                            None,
                        ),
                    )
                }
            })
            .await?
            .expect_err("Error expected");
            let rejected_as_unknown = error
                .chain()
                .find_map(|cause| cause.downcast_ref::<TransactionRejectionReason>())
                .is_some_and(|reason| is_unknown_trigger_rejection(reason, &trigger_id));
            assert!(rejected_as_unknown, "Unexpected error received: {error:?}",);
            // Checking results
            let new_asset_value = run_on_dedicated_thread({
                let client = test_client.clone();
                let asset_id = asset_id.clone();
                move || get_asset_value(&client, &asset_id)
            })
            .await??;
            assert_eq!(
                new_asset_value,
                prev_asset_value.checked_add(&Quantity::one()).unwrap()
            );
            Ok(())
        },
    )
    .await
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[allow(clippy::too_many_lines)]
async fn trigger_should_be_able_to_modify_its_own_repeats_count() -> Result<()> {
    let Some(network) = start_network(stringify!(
        trigger_should_be_able_to_modify_its_own_repeats_count
    ))
    .await?
    else {
        return Ok(());
    };
    let test_client = network.client();
    network.ensure_blocks_with(|h| h.total >= 1).await?;
    run_or_skip(
        stringify!(trigger_should_be_able_to_modify_its_own_repeats_count),
        || async {
            let asset_definition_id = AssetDefinitionId::derive_from_components(
                DomainId::try_new("wonderland", "universal")?,
                "rose".parse()?,
            );
            let account_id = ALICE_ID.clone();
            let asset_id = AssetId::new(asset_definition_id, account_id.clone());
            let trigger_id = "self_modifying_trigger".parse::<TriggerId>()?;
            let permission_on_registration = CanRegisterTrigger {
                authority: account_id.clone(),
            };
            let expected_permission: iroha::data_model::permission::Permission =
                permission_on_registration.clone().into();
            // Ensure the caller explicitly has permission to register the trigger on their own behalf.
            if let Err(err) = submit_instruction_and_wait(
                &network,
                test_client.clone(),
                Grant::account_permission(permission_on_registration, account_id.clone()),
                stringify!(trigger_should_be_able_to_modify_its_own_repeats_count),
            )
            .await
                && !is_permission_grant_repetition(&err, &expected_permission)
            {
                return Err(err);
            }
            let trigger_instructions: Vec<InstructionBox> = vec![
                Mint::trigger_repetitions(1_u32, trigger_id.clone()).into(),
                Mint::asset_quantity(1u32, asset_id.clone()).into(),
            ];
            let register_trigger = Register::trigger(Trigger::new(
                trigger_id.clone(),
                Action::new(
                    trigger_instructions,
                    1_u32,
                    account_id.clone(),
                    ExecuteTriggerEventFilter::new()
                        .for_trigger(trigger_id.clone())
                        .under_authority(account_id),
                )
                .expect("trigger action fixture satisfies validation invariants"),
            ));
            submit_instruction_and_wait(
                &network,
                test_client.clone(),
                register_trigger,
                stringify!(trigger_should_be_able_to_modify_its_own_repeats_count),
            )
            .await?;
            // Saving current asset value
            let prev_asset_value = run_on_dedicated_thread({
                let client = test_client.clone();
                let asset_id = asset_id.clone();
                move || get_asset_value(&client, &asset_id)
            })
            .await??;
            // Executing trigger first time
            let execute_trigger = ExecuteTrigger::new(trigger_id);
            let execute_trigger_first = execute_trigger.clone();
            submit_instruction_and_wait(
                &network,
                test_client.clone(),
                Instruction::into_instruction_box(Box::new(execute_trigger_first)),
                stringify!(trigger_should_be_able_to_modify_its_own_repeats_count),
            )
            .await?;
            // Executing trigger second time
            submit_instruction_and_wait(
                &network,
                test_client.clone(),
                Instruction::into_instruction_box(Box::new(execute_trigger)),
                stringify!(trigger_should_be_able_to_modify_its_own_repeats_count),
            )
            .await?;
            // Checking results
            let new_asset_value = run_on_dedicated_thread({
                let client = test_client.clone();
                let asset_id = asset_id.clone();
                move || get_asset_value(&client, &asset_id)
            })
            .await??;
            assert_eq!(
                new_asset_value,
                prev_asset_value
                    .checked_add(&Quantity::from(2_u32))
                    .unwrap()
            );
            Ok(())
        },
    )
    .await
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[allow(clippy::too_many_lines)]
async fn only_account_with_permission_can_register_trigger() -> Result<()> {
    let Some(network) = start_network(stringify!(
        only_account_with_permission_can_register_trigger
    ))
    .await?
    else {
        return Ok(());
    };
    let test_client = network.client();
    run_or_skip(
        stringify!(only_account_with_permission_can_register_trigger),
        || async {
            let alice_account_id = ALICE_ID.clone();
            let rabbit_keys = checked_random_trigger_account_keypair();
            let rabbit_account_id = AccountId::new(rabbit_keys.public_key().clone());
            let rabbit_account = Account::new(rabbit_account_id.clone());
            let rabbit_client =
                integration_tests::sync::rebind_blocking_client(&test_client, |client| {
                    client.account = rabbit_account_id.clone();
                    client.key_pair = rabbit_keys;
                });
            // Permission for the trigger registration on behalf of alice
            let permission_on_registration = CanRegisterTrigger {
                authority: ALICE_ID.clone(),
            };
            let expected_permission: iroha::data_model::permission::Permission =
                permission_on_registration.clone().into();
            // Trigger with 'alice' as authority
            let trigger_id = "alice_trigger".parse::<TriggerId>()?;
            let trigger = Trigger::new(
                trigger_id.clone(),
                Action::new(
                    Vec::<InstructionBox>::new(),
                    Repeats::Indefinitely,
                    alice_account_id.clone(),
                    ExecuteTriggerEventFilter::new()
                        .for_trigger(trigger_id.clone())
                        .under_authority(alice_account_id.clone()),
                )
                .expect("trigger action fixture satisfies validation invariants"),
            );
            submit_instruction_and_wait(
                &network,
                test_client.clone(),
                Register::account(rabbit_account),
                stringify!(only_account_with_permission_can_register_trigger),
            )
            .await?;
            run_on_dedicated_thread({
                let client = test_client.clone();
                let rabbit_account_id = rabbit_account_id.clone();
                move || -> Result<()> {
                    let account = client
                        .client()
                        .query_single(FindAccountById::new(rabbit_account_id.clone()))
                        .expect("Account not found");
                    assert_eq!(account.id(), &rabbit_account_id);
                    Ok(())
                }
            })
            .await??;
            println!("Rabbit is found.");
            // Fund the registered signer so the following rejection exercises
            // trigger authority rather than transaction fee admission.
            let fee_definition: AssetDefinitionId =
                iroha_config::parameters::defaults::nexus::fees::fee_asset_id().parse()?;
            submit_instruction_and_wait(
                &network,
                test_client.clone(),
                Transfer::asset_quantity(
                    AssetId::new(fee_definition, ALICE_ID.clone()),
                    100u32,
                    rabbit_account_id.clone(),
                ),
                stringify!(only_account_with_permission_can_register_trigger),
            )
            .await?;
            // Trying register the trigger without permissions
            let err = run_on_dedicated_thread({
                let client = rabbit_client.clone();
                let trigger = trigger.clone();
                move || {
                    client.submit(
                        Register::trigger(trigger),
                        iroha_data_model::transaction::FeePaymentIntent::authority(
                            Vec::new(),
                            None,
                        ),
                    )
                }
            })
            .await?
            .expect_err("Trigger should not be registered!");
            let Some(InstructionExecutionError::InvalidParameter(
                InvalidParameterError::SmartContract(msg),
            )) = err
                .chain()
                .find_map(|cause| cause.downcast_ref::<InstructionExecutionError>())
            else {
                panic!("Unexpected error: {err:?}");
            };
            assert!(msg.contains("CanRegisterTrigger"));
            println!("Rabbit couldn't register the trigger");
            // Give permissions to the rabbit
            if let Err(err) = submit_instruction_and_wait(
                &network,
                test_client.clone(),
                Grant::account_permission(permission_on_registration, rabbit_account_id.clone()),
                stringify!(only_account_with_permission_can_register_trigger),
            )
            .await
                && !is_permission_grant_repetition(&err, &expected_permission)
            {
                return Err(err);
            }
            println!("Rabbit has got the permission");
            // Trying register the trigger with permissions
            submit_instruction_and_wait(
                &network,
                rabbit_client.clone(),
                Register::trigger(trigger),
                stringify!(only_account_with_permission_can_register_trigger),
            )
            .await?;
            let found_trigger = wait_for_trigger(
                &test_client,
                &trigger_id,
                network.sync_timeout(),
                stringify!(only_account_with_permission_can_register_trigger),
            )
            .await?;
            assert_eq!(*found_trigger.id(), trigger_id);
            Ok(())
        },
    )
    .await
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn unregister_trigger() -> Result<()> {
    let Some(network) = start_network(stringify!(unregister_trigger)).await? else {
        return Ok(());
    };
    let test_client = network.client();
    run_or_skip(stringify!(unregister_trigger), || async {
        let account_id = ALICE_ID.clone();
        // Registering trigger
        let trigger_id = "empty_trigger".parse::<TriggerId>()?;
        let trigger = Trigger::new(
            trigger_id.clone(),
            Action::new(
                Vec::<InstructionBox>::new(),
                Repeats::Indefinitely,
                account_id.clone(),
                ExecuteTriggerEventFilter::new()
                    .for_trigger(trigger_id.clone())
                    .under_authority(account_id),
            )
            .expect("trigger action fixture satisfies validation invariants"),
        );
        let register_trigger = Register::trigger(trigger.clone());
        submit_instruction_and_wait(
            &network,
            test_client.clone(),
            register_trigger,
            stringify!(unregister_trigger),
        )
        .await?;
        // Finding trigger
        let deadline = Instant::now() + Duration::from_secs(20);
        let found_trigger = loop {
            let found_trigger = run_on_dedicated_thread({
                let client = test_client.with_request_deadline(deadline.into_std())?;
                let trigger_id = trigger_id.clone();
                move || fixture_trigger(&client, &trigger_id)
            })
            .await??;
            eyre::ensure!(Instant::now() < deadline, "trigger read deadline elapsed");
            if let Some(found_trigger) = found_trigger {
                break found_trigger;
            }
            assert!(Instant::now() < deadline, "trigger not found");
            sleep(Duration::from_millis(250)).await;
        };
        let found_action = found_trigger.action();
        let Executable::Instructions(found_instructions) = found_action.executable() else {
            panic!("Expected instructions");
        };
        let found_trigger = Trigger::new(
            found_trigger.id().clone(),
            Action::new(
                Executable::Instructions(found_instructions.to_owned()),
                found_action.repeats(),
                found_action.authority().clone(),
                found_action.filter().clone(),
            )
            .expect("queried trigger action preserves server-validated invariants"),
        );
        assert_eq!(found_trigger, trigger);
        // Unregistering trigger
        let unregister_trigger = Unregister::trigger(trigger_id.clone());
        submit_instruction_and_wait(
            &network,
            test_client.clone(),
            unregister_trigger,
            stringify!(unregister_trigger),
        )
        .await?;
        // Checking result
        let deadline = Instant::now() + Duration::from_secs(20);
        loop {
            let still_present = run_on_dedicated_thread({
                let client = test_client.with_request_deadline(deadline.into_std())?;
                let trigger_id = trigger_id.clone();
                move || fixture_trigger(&client, &trigger_id).map(|trigger| trigger.is_some())
            })
            .await??;
            eyre::ensure!(
                Instant::now() < deadline,
                "trigger removal read deadline elapsed"
            );
            if !still_present {
                break;
            }
            if Instant::now() >= deadline {
                assert!(!still_present);
            }
            sleep(Duration::from_millis(250)).await;
        }
        Ok(())
    })
    .await
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn trigger_in_genesis() -> Result<()> {
    let bytecode = load_sample_ivm("mint_rose_trigger");
    let alias = ContractAlias::from_components("genesis_mint_rose", None, "universal")?;
    let metadata = mint_rose_trigger_metadata(&bytecode, &alias)?;
    let account_id = ALICE_ID.clone();
    let trigger_id = "genesis_trigger".parse::<TriggerId>()?;
    let trigger = Trigger::new(
        trigger_id.clone(),
        Action::new(
            bytecode.clone(),
            Repeats::Indefinitely,
            account_id.clone(),
            ExecuteTriggerEventFilter::new()
                .for_trigger(trigger_id.clone())
                .under_authority(account_id.clone()),
        )
        .expect("trigger action fixture satisfies validation invariants")
        .with_metadata(metadata),
    );
    let Some(network) = sandbox::start_network_async_or_skip(
        mint_rose_fixture_builder("genesis_mint_rose")
            .with_genesis_instruction(Register::trigger(trigger)),
        stringify!(trigger_in_genesis),
    )
    .await?
    else {
        return Ok(());
    };
    network.ensure_blocks_with(|h| h.total >= 1).await?;
    let test_client = network.client();
    run_or_skip(stringify!(trigger_in_genesis), || async {
        deploy_mint_rose_callback(&test_client, bytecode, alias).await?;
        let asset_definition_id = AssetDefinitionId::derive_from_components(
            DomainId::try_new("wonderland", "universal")?,
            "rose".parse()?,
        );
        let asset_id = AssetId::new(asset_definition_id, account_id);
        let prev_value = run_on_dedicated_thread({
            let client = test_client.clone();
            let asset_id = asset_id.clone();
            move || get_asset_value(&client, &asset_id)
        })
        .await??;
        // Executing trigger
        submit_instruction_and_wait(
            &network,
            test_client.clone(),
            SetKeyValue::trigger(trigger_id.clone(), "VAL".parse()?, 1_u32),
            stringify!(trigger_in_genesis),
        )
        .await?;
        let call_trigger = ExecuteTrigger::new(trigger_id).with_args(MintRoseArgs { val: 1 });
        submit_instruction_and_wait(
            &network,
            test_client.clone(),
            Instruction::into_instruction_box(Box::new(call_trigger)),
            stringify!(trigger_in_genesis),
        )
        .await?;
        // Checking result
        let new_value = run_on_dedicated_thread({
            let client = test_client.clone();
            let asset_id = asset_id.clone();
            move || get_asset_value(&client, &asset_id)
        })
        .await??;
        assert_eq!(new_value, prev_value.checked_add(&Quantity::one()).unwrap());
        Ok(())
    })
    .await
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[allow(clippy::too_many_lines)]
async fn trigger_should_be_able_to_modify_other_trigger() -> Result<()> {
    let Some(network) =
        start_network(stringify!(trigger_should_be_able_to_modify_other_trigger)).await?
    else {
        return Ok(());
    };
    let test_client = network.client();
    network.ensure_blocks_with(|h| h.total >= 1).await?;
    run_or_skip(
        stringify!(trigger_should_be_able_to_modify_other_trigger),
        || async {
            let asset_definition_id = AssetDefinitionId::derive_from_components(
                DomainId::try_new("wonderland", "universal")?,
                "rose".parse()?,
            );
            let account_id = ALICE_ID.clone();
            let asset_id = AssetId::new(asset_definition_id, account_id.clone());
            let trigger_id_unregister = "unregister_other_trigger".parse::<TriggerId>()?;
            let trigger_id_to_be_unregistered =
                "should_be_unregistered_trigger".parse::<TriggerId>()?;
            let trigger_unregister_instructions =
                vec![Unregister::trigger(trigger_id_to_be_unregistered.clone())];
            let register_trigger = Register::trigger(Trigger::new(
                trigger_id_unregister.clone(),
                Action::new(
                    trigger_unregister_instructions,
                    1_u32,
                    account_id.clone(),
                    ExecuteTriggerEventFilter::new()
                        .for_trigger(trigger_id_unregister.clone())
                        .under_authority(account_id.clone()),
                )
                .expect("trigger action fixture satisfies validation invariants"),
            ));
            submit_instruction_and_wait(
                &network,
                test_client.clone(),
                register_trigger,
                stringify!(trigger_should_be_able_to_modify_other_trigger),
            )
            .await?;
            let trigger_should_be_unregistered_instructions =
                vec![Mint::asset_quantity(1u32, asset_id.clone())];
            let register_trigger = Register::trigger(Trigger::new(
                trigger_id_to_be_unregistered.clone(),
                Action::new(
                    trigger_should_be_unregistered_instructions,
                    1_u32,
                    account_id.clone(),
                    ExecuteTriggerEventFilter::new()
                        .for_trigger(trigger_id_to_be_unregistered.clone())
                        .under_authority(account_id),
                )
                .expect("trigger action fixture satisfies validation invariants"),
            ));
            submit_instruction_and_wait(
                &network,
                test_client.clone(),
                register_trigger,
                stringify!(trigger_should_be_able_to_modify_other_trigger),
            )
            .await?;
            // Saving current asset value
            let prev_asset_value = run_on_dedicated_thread({
                let client = test_client.clone();
                let asset_id = asset_id.clone();
                move || get_asset_value(&client, &asset_id)
            })
            .await??;
            // Executing triggers
            let execute_trigger_unregister = ExecuteTrigger::new(trigger_id_unregister);
            let execute_trigger_should_be_unregistered =
                ExecuteTrigger::new(trigger_id_to_be_unregistered.clone());
            let err = run_on_dedicated_thread({
                let client = test_client.clone();
                move || {
                    client.submit_all(
                        [
                            Instruction::into_instruction_box(Box::new(execute_trigger_unregister)),
                            Instruction::into_instruction_box(Box::new(
                                execute_trigger_should_be_unregistered,
                            )),
                        ],
                        iroha_data_model::transaction::FeePaymentIntent::authority(
                            Vec::new(),
                            None,
                        ),
                    )
                }
            })
            .await?
            .expect_err("should immediately result in error");
            // The by-call execution now surfaces as a validation failure; accept either the
            // original `FindError::Trigger` or a `ValidationFail::NotPermitted` wrapper.
            if let Some(FindError::Trigger(not_found_trigger)) = err
                .chain()
                .find_map(|cause| cause.downcast_ref::<FindError>())
            {
                assert_eq!(*not_found_trigger, trigger_id_to_be_unregistered);
            } else if let Some(ValidationFail::NotPermitted(msg)) = err
                .chain()
                .find_map(|cause| cause.downcast_ref::<ValidationFail>())
            {
                assert!(
                    msg.contains(&trigger_id_to_be_unregistered.to_string()),
                    "expected missing-trigger message, got: {msg}"
                );
            } else {
                panic!("unexpected error: {err:?}");
            }
            // Checking results
            // First trigger should cancel second one, so value should stay the same
            let new_asset_value = run_on_dedicated_thread({
                let client = test_client.clone();
                let asset_id = asset_id.clone();
                move || get_asset_value(&client, &asset_id)
            })
            .await??;
            assert_eq!(new_asset_value, prev_asset_value);
            Ok(())
        },
    )
    .await
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn trigger_burn_repetitions() -> Result<()> {
    let Some(network) = sandbox::start_network_async_or_skip(
        NetworkBuilder::new(),
        stringify!(trigger_burn_repetitions),
    )
    .await?
    else {
        return Ok(());
    };
    let test_client = network.client();
    run_or_skip(stringify!(trigger_burn_repetitions), || async {
        // Explicitly allow Alice to register by-call triggers.
        let permission_on_registration = CanRegisterTrigger {
            authority: ALICE_ID.clone(),
        };
        let expected_permission: iroha::data_model::permission::Permission =
            permission_on_registration.clone().into();
        if let Err(err) = submit_instruction_and_wait(
            &network,
            test_client.clone(),
            Grant::account_permission(permission_on_registration, ALICE_ID.clone()),
            stringify!(trigger_burn_repetitions),
        )
        .await
            && !is_permission_grant_repetition(&err, &expected_permission)
        {
            return Err(err);
        }
        let asset_definition_id = AssetDefinitionId::derive_from_components(
            DomainId::try_new("wonderland", "universal")?,
            "rose".parse()?,
        );
        let account_id = ALICE_ID.clone();
        let asset_id = AssetId::new(asset_definition_id, account_id.clone());
        let base_trigger_name = "trigger_burn_repetitions";
        let mut attempt = 0_u32;
        let max_attempts = 5_u32;
        let trigger_id = loop {
            let candidate = if attempt == 0 {
                base_trigger_name.to_owned()
            } else {
                format!("{base_trigger_name}_{attempt}")
            };
            let trigger_id = candidate.parse::<TriggerId>()?;
            let trigger_instructions = vec![Mint::asset_quantity(1u32, asset_id.clone())];
            let register_trigger = Register::trigger(Trigger::new(
                trigger_id.clone(),
                Action::new(
                    trigger_instructions,
                    1_u32,
                    account_id.clone(),
                    ExecuteTriggerEventFilter::new()
                        .for_trigger(trigger_id.clone())
                        .under_authority(account_id.clone()),
                )
                .expect("trigger action fixture satisfies validation invariants"),
            ));
            match submit_instruction_and_wait(
                &network,
                test_client.clone(),
                register_trigger,
                stringify!(trigger_burn_repetitions),
            )
            .await
            {
                Ok(_) => break trigger_id,
                Err(err) => {
                    if is_trigger_register_collision(&err, &trigger_id) {
                        if attempt >= max_attempts {
                            return Err(err.wrap_err(format!(
                                "trigger id collision retry limit ({max_attempts}) exceeded"
                            )));
                        }
                        let next = format!("{base_trigger_name}_{}", attempt + 1);
                        eprintln!(
                            "Trigger id collision for `{}` (attempt {}); retrying with `{}`",
                            trigger_id,
                            attempt + 1,
                            next
                        );
                        attempt += 1;
                        continue;
                    }
                    return Err(err);
                }
            }
        };
        submit_instruction_and_wait(
            &network,
            test_client.clone(),
            Burn::trigger_repetitions(1_u32, trigger_id.clone()),
            stringify!(trigger_burn_repetitions),
        )
        .await?;
        timeout(network.sync_timeout(), async {
            loop {
                let still_active = run_on_dedicated_thread({
                    let client = test_client.clone();
                    let trigger_id = trigger_id.clone();
                    move || -> Result<bool> {
                        let ids = client
                            .client()
                            .query(FindActiveTriggerIds)
                            .execute_all()
                            .wrap_err("query active trigger ids")?;
                        Ok(ids.into_iter().any(|id| id == trigger_id))
                    }
                })
                .await??;
                if !still_active {
                    break;
                }
                tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            }
            Ok::<(), eyre::Report>(())
        })
        .await
        .wrap_err("wait for trigger to be pruned after burn")??;
        // Executing trigger should be rejected without repetitions, but avoid logging a warning
        // by observing the pipeline event instead of waiting on the confirmation stream.
        let execute_trigger = ExecuteTrigger::new(trigger_id.clone());
        let instruction = Instruction::into_instruction_box(Box::new(execute_trigger));
        let transaction = quoted_transaction(&test_client, instruction).await?;
        let hash = transaction.hash();
        let mut events = timeout(
            network.sync_timeout(),
            test_client
                .account_client()
                .events()
                .subscribe([TransactionEventFilter::default().for_hash(hash)]),
        )
        .await
        .wrap_err("timed out opening pipeline event stream")??;
        run_on_dedicated_thread({
            let client = test_client.clone();
            let transaction = transaction.clone();
            move || client.submit_transaction(&transaction)
        })
        .await??;
        timeout(network.sync_timeout(), async {
            loop {
                let Some(next) = events.next().await else {
                    return Err(eyre::eyre!("transaction event stream closed"));
                };
                let EventBox::Pipeline(PipelineEventBox::Transaction(event)) = next? else {
                    continue;
                };
                match event.status() {
                    TransactionStatus::Queued => {}
                    TransactionStatus::Rejected(reason) => {
                        eyre::ensure!(
                            is_unknown_trigger_rejection(reason, &trigger_id),
                            "unexpected rejection reason: {reason:?}"
                        );
                        break;
                    }
                    status => {
                        return Err(eyre::eyre!("unexpected transaction status: {status:?}"));
                    }
                }
            }
            Ok::<(), eyre::Report>(())
        })
        .await
        .wrap_err("timed out waiting for trigger rejection")??;
        events.close().await?;
        Ok(())
    })
    .await
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn unregistering_one_of_two_triggers_with_identical_contract_should_not_cause_original_contract_loss()
-> Result<()> {
    let Some(network) =
        sandbox::start_network_async_or_skip(mint_rose_fixture_builder("shared_mint_rose"), stringify!(unregistering_one_of_two_triggers_with_identical_contract_should_not_cause_original_contract_loss))
            .await?
    else {
        return Ok(());
    };
    let test_client = network.client();
    run_or_skip(
        stringify!(unregistering_one_of_two_triggers_with_identical_contract_should_not_cause_original_contract_loss),
        || async {
            let account_id = ALICE_ID.clone();
            let artifact = load_sample_ivm("mint_rose_trigger");
            let alias = ContractAlias::from_components("shared_mint_rose", None, "universal")?;
            let metadata = mint_rose_trigger_metadata(&artifact, &alias)?;
            deploy_mint_rose_callback(&test_client, artifact.clone(), alias).await?;
            let first_trigger_id = "mint_rose_1".parse::<TriggerId>()?;
            let second_trigger_id = "mint_rose_2".parse::<TriggerId>()?;
            let build_trigger = |trigger_id: TriggerId| {
                Trigger::new(
                    trigger_id.clone(),
                    Action::new(
                        artifact.clone(),
                        Repeats::Indefinitely,
                        account_id.clone(),
                        ExecuteTriggerEventFilter::new()
                            .for_trigger(trigger_id)
                            .under_authority(account_id.clone()),
                    ).expect("trigger action fixture satisfies validation invariants")
                    .with_metadata(metadata.clone()),
                )
            };
            let first_trigger = build_trigger(first_trigger_id.clone());
            let second_trigger = build_trigger(second_trigger_id.clone());
            let second_trigger_for_register = second_trigger.clone();
            submit_instruction_and_wait(
                &network,
                test_client.clone(),
                Register::trigger(first_trigger),
                stringify!(unregistering_one_of_two_triggers_with_identical_contract_should_not_cause_original_contract_loss),
            )
            .await?;
            let second_registered = submit_instruction_and_wait(
                &network,
                test_client.clone(),
                Register::trigger(second_trigger_for_register),
                stringify!(unregistering_one_of_two_triggers_with_identical_contract_should_not_cause_original_contract_loss),
            )
            .await?;
            submit_instruction_and_wait(
                &network,
                test_client.clone(),
                Unregister::trigger(first_trigger_id.clone()),
                stringify!(unregistering_one_of_two_triggers_with_identical_contract_should_not_cause_original_contract_loss),
            )
            .await?;
            let (got_second_trigger, expected_second_trigger) = run_on_dedicated_thread({
                let client = test_client.with_request_deadline(std::time::Instant::now() + network.sync_timeout())?;
                let second_trigger_id = second_trigger_id.clone();
                move || -> Result<_> {
                    let registered = client.client().get_explorer_transaction_proof(
                        HashOf::<TransactionEntrypoint>::from_untyped_unchecked(second_registered.into()),
                    )?;
                    eyre::ensure!(registered.transaction.result().is_ok(), "second trigger registration was rejected");
                    let expected = trigger_with_registration_header(&second_trigger, registered.block_header)?;
                    // This is one known retained trigger, so use the native bounded
                    // exact-ID read; the full action-byte oracle below is unchanged.
                    let actual = client.client().query_single(FindTriggerById::new(second_trigger_id.clone()))?;
                    eyre::ensure!(actual.id() == &second_trigger_id, "trigger point read substituted its requested identity");
                    Ok((actual, expected))
                }
            })
            .await??;
            require_identical_trigger(&got_second_trigger, &expected_second_trigger)?;
            Ok(())
        },
    )
    .await
}
fn build_register_trigger_isi(
    account_id: &AccountId,
    trigger_instructions: Vec<InstructionBox>,
) -> Register<Trigger> {
    let trigger_id: TriggerId = TRIGGER_NAME.parse().expect("Valid");
    Register::trigger(Trigger::new(
        trigger_id.clone(),
        Action::new(
            trigger_instructions,
            Repeats::Indefinitely,
            account_id.clone(),
            ExecuteTriggerEventFilter::new()
                .for_trigger(trigger_id)
                .under_authority(account_id.clone()),
        )
        .expect("trigger action fixture satisfies validation invariants"),
    ))
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn call_execute_trigger_with_args() -> Result<()> {
    let Some(network) = sandbox::start_network_async_or_skip(
        mint_rose_fixture_builder("argument_mint_rose"),
        stringify!(call_execute_trigger_with_args),
    )
    .await?
    else {
        return Ok(());
    };
    let test_client = network.client();
    network.ensure_blocks_with(|h| h.total >= 1).await?;
    run_or_skip(stringify!(call_execute_trigger_with_args), || async {
        let bytecode = load_sample_ivm("mint_rose_trigger");
        let alias = ContractAlias::from_components("argument_mint_rose", None, "universal")?;
        let metadata = mint_rose_trigger_metadata(&bytecode, &alias)?;
        deploy_mint_rose_callback(&test_client, bytecode.clone(), alias).await?;
        let asset_definition_id = AssetDefinitionId::derive_from_components(
            DomainId::try_new("wonderland", "universal")?,
            "rose".parse()?,
        );
        let account_id = ALICE_ID.clone();
        let asset_id = AssetId::new(asset_definition_id, account_id.clone());
        let prev_value = run_on_dedicated_thread({
            let client = test_client.clone();
            let asset_id = asset_id.clone();
            move || get_asset_value(&client, &asset_id)
        })
        .await??;
        let trigger_id = TRIGGER_NAME.parse::<TriggerId>()?;
        let trigger = Trigger::new(
            trigger_id.clone(),
            Action::new(
                bytecode,
                Repeats::Indefinitely,
                account_id.clone(),
                ExecuteTriggerEventFilter::new()
                    .for_trigger(trigger_id.clone())
                    .under_authority(account_id.clone()),
            )
            .expect("trigger action fixture satisfies validation invariants")
            .with_metadata(metadata),
        );
        submit_instruction_and_wait(
            &network,
            test_client.clone(),
            Register::trigger(trigger),
            stringify!(call_execute_trigger_with_args),
        )
        .await?;
        let args = MintRoseArgs { val: 42 };
        let call_trigger = ExecuteTrigger::new(trigger_id).with_args(args);
        submit_instruction_and_wait(
            &network,
            test_client.clone(),
            Instruction::into_instruction_box(Box::new(call_trigger)),
            stringify!(call_execute_trigger_with_args),
        )
        .await?;
        let expected_value = prev_value.checked_add(&Quantity::from(42_u32)).unwrap();
        let new_value = wait_for_asset_value(
            &test_client,
            &asset_id,
            expected_value.clone(),
            network.sync_timeout(),
            stringify!(call_execute_trigger_with_args),
        )
        .await?;
        assert_eq!(new_value, expected_value);
        Ok(())
    })
    .await
}
