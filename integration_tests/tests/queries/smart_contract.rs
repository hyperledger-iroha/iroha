//! Smart contract query behaviour checks.
#[path = "../support/contract_deployment.rs"]
mod contract_deployment;
use eyre::{Result, WrapErr};
use integration_tests::sandbox;
use iroha::{client::QueryError, data_model::prelude::*};
use iroha_model_base::metadata::Metadata;
use iroha_model_base::name::Name;
use iroha_test_network::*;
use iroha_test_samples::load_sample_ivm;
use std::num::NonZeroU64;
/// Gas the caller signs for one sample invocation; the artifact derives no budget.
const SAMPLE_GAS_LIMIT: u64 = 20_000_000;
fn fee_payment_with_gas_limit(bytecode: &IvmBytecode) -> Result<FeePaymentIntent> {
    ivm::ProgramMetadata::parse(bytecode.as_ref()).wrap_err("parse IVM program metadata")?;
    Ok(FeePaymentIntent::authority(
        Vec::new(),
        NonZeroU64::new(SAMPLE_GAS_LIMIT),
    ))
}
/// Deploy the exact sample and delegate only its declared invocation and metadata effect.
fn sample_contract_metadata(
    client: &iroha::blocking::Client,
    bytecode: &IvmBytecode,
    alias: &str,
) -> Result<Metadata> {
    use iroha_executor_data_model::permission::{
        account::CanModifyAccountMetadata, smart_contract::CanUseContractPermission,
    };
    let alias =
        iroha_data_model::smart_contract::ContractAlias::from_components(alias, None, "universal")?;
    let (address, code_hash, _, _) =
        contract_deployment::deploy_contract_locally_signed(client, bytecode.as_ref(), alias)?;
    client.submit_all(
        [
            Grant::account_permission(
                CanUseContractPermission {
                    contract: address.clone(),
                    permission: "Admin".parse()?,
                },
                client.client().account().clone(),
            ),
            Grant::account_permission(
                CanModifyAccountMetadata {
                    account: client.client().account().clone(),
                },
                address.subject_id(),
            ),
        ],
        FeePaymentIntent::authority(Vec::new(), None),
    )?;
    let mut metadata = Metadata::default();
    for (key, value) in [
        ("contract_address", address.to_string()),
        ("contract_code_hash", code_hash),
        ("contract_entrypoint", "run".to_owned()),
    ] {
        metadata.insert(
            key.parse::<Name>()?,
            iroha_primitives::json::Json::new(value),
        );
    }
    Ok(metadata)
}

#[test]
fn smart_contract_query_scenarios() -> Result<()> {
    let mut builder = NetworkBuilder::new()
        .with_genesis_instruction(Grant::account_permission(
            iroha_executor_data_model::permission::smart_contract::CanManageSmartContractCode,
            iroha_test_samples::ALICE_ID.clone(),
        ))
        .with_config_layer(|layer| {
            layer.write(["pipeline", "query_default_cursor_mode"], "stored");
        });
    for alias in ["query_assets_cursor", "query_filter_sample"] {
        builder = builder.with_genesis_instruction(Grant::account_permission(
            contract_deployment::contract_alias_management_permission(alias),
            iroha_test_samples::ALICE_ID.clone(),
        ));
    }
    let Some((network, _rt)) = sandbox::start_network_blocking_or_skip(
        builder,
        stringify!(smart_contract_query_scenarios),
    )?
    else {
        return Ok(());
    };
    let client = network.client();
    let torii = client.client().endpoint().clone();
    let env_dir = network.env_dir().to_path_buf();
    // The contract writes a synthetic cursor that has no server-owned stored query.
    // Continuing it must fail before executor validation or cursor advancement.
    {
        let bytecode = load_sample_ivm("query_assets_and_save_cursor");
        let metadata = sample_contract_metadata(&client, &bytecode, "query_assets_cursor")?;
        let fee_payment = fee_payment_with_gas_limit(&bytecode)?;
        let account = client.account_client();
        let mut payload = account.prepare_transaction(
            iroha::client::AccountTransactionDraft::new(bytecode, fee_payment, metadata),
        )?;
        let quote = client
            .quote_fees(iroha::client::FeeQuoteRequest::AccountSignature { payload: &payload })?;
        eyre::ensure!(
            payload
                .fee_payment
                .has_same_payer_and_gas_bound(&quote.intent),
            "contract query fixture quote changed payer or gas bound"
        );
        payload.fee_payment = quote.intent;
        let transaction = account.sign_transaction(payload)?;
        client.submit_transaction_and_wait(&transaction)?;
        let cursor_key: Name = "cursor".parse().unwrap();
        let asset_cursor = client
            .client()
            .query_single(FindAccountById::new(client.client().account().clone()))?
            .metadata()
            .get(&cursor_key)
            .cloned()
            .expect("account metadata must contain cursor")
            .try_into_any_norito()?;
        let err = client
            .client()
            .raw_continue_iterable_query(asset_cursor)
            .expect_err("a contract-supplied synthetic cursor must not resume a stored query");
        assert!(
            matches!(&err, QueryError::Http { status, code, .. }
            if *status == reqwest::StatusCode::GONE && code == "query_validation_failed"),
            "expected canonical stored-cursor absence, got {err:?}"
        );
        assert!(
            err.to_string()
                .starts_with("query failed; HTTP 410 Gone; query_validation_failed: "),
            "expected the decoded Torii envelope for a contract-supplied cursor absent from the store: {err:?}"
        );
    }
    // smart_contract_can_filter_queries
    {
        let bytecode = load_sample_ivm("smart_contract_can_filter_queries");
        let metadata = sample_contract_metadata(&client, &bytecode, "query_filter_sample")?;
        let fee_payment = fee_payment_with_gas_limit(&bytecode)?;
        let account = client.account_client();
        let mut payload = account.prepare_transaction(
            iroha::client::AccountTransactionDraft::new(bytecode, fee_payment, metadata),
        )?;
        let quote = client
            .quote_fees(iroha::client::FeeQuoteRequest::AccountSignature { payload: &payload })?;
        eyre::ensure!(
            payload
                .fee_payment
                .has_same_payer_and_gas_bound(&quote.intent),
            "contract query fixture quote changed payer or gas bound"
        );
        payload.fee_payment = quote.intent;
        let transaction = account.sign_transaction(payload)?;
        client
            .submit_transaction_and_wait(&transaction)
            .wrap_err_with(|| {
                format!(
                    "submit smart_contract_can_filter_queries failed; torii={torii}, env_dir={}",
                    env_dir.display()
                )
            })?;
    }
    Ok(())
}
