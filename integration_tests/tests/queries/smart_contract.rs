//! Smart contract query behaviour checks.
use crate::contract_fixture;
use eyre::{Result, WrapErr};
use integration_tests::sandbox;
use iroha::{client::QueryError, data_model::prelude::*};
use iroha_core::smartcontracts::ivm::gas_limit_for_meta;
use iroha_data_model::smart_contract::ContractAlias;
use iroha_model_base::metadata::Metadata;
use iroha_model_base::name::Name;
use iroha_test_network::*;
use iroha_test_samples::load_sample_ivm;
use std::num::NonZeroU64;
fn fee_payment_with_gas_limit(bytecode: &IvmBytecode) -> Result<FeePaymentIntent> {
    let parsed =
        ivm::ProgramMetadata::parse(bytecode.as_ref()).wrap_err("parse IVM program metadata")?;
    let gas_limit = gas_limit_for_meta(&parsed.metadata)
        .map_err(|error| eyre::eyre!("invalid IVM cycle limit: {error:?}"))?;
    Ok(FeePaymentIntent::authority(
        Vec::new(),
        NonZeroU64::new(gas_limit),
    ))
}

fn query_contract_metadata(alias: &ContractAlias) -> Metadata {
    contract_fixture::call_metadata(alias)
}

#[test]
fn query_contract_samples_select_their_declared_entrypoint() -> Result<()> {
    let alias = ContractAlias::from_components("query_fixture", None, "universal")?;
    let metadata = query_contract_metadata(&alias);
    let name: String = metadata
        .get(&"contract_entrypoint".parse::<Name>()?)
        .expect("explicit entrypoint")
        .try_into_any()?;
    for sample in [
        "query_assets_and_save_cursor",
        "smart_contract_can_filter_queries",
    ] {
        let bytecode = load_sample_ivm(sample);
        let artifact = ivm::verify_contract_artifact(bytecode.as_ref())
            .map_err(|error| eyre::eyre!("verify {sample}: {error}"))?;
        let entrypoint = artifact
            .contract_interface
            .entrypoints
            .iter()
            .find(|entrypoint| entrypoint.name == name)
            .expect("the signed metadata selects the sample's declared entrypoint");
        assert!(entrypoint.params.is_empty());
        assert!(entrypoint.argument_schema.is_none());
    }
    Ok(())
}
#[test]
fn smart_contract_query_scenarios() -> Result<()> {
    let aliases = ["query_cursor_fixture", "query_filter_fixture"]
        .map(|name| ContractAlias::from_components(name, None, "universal"))
        .into_iter()
        .collect::<Result<Vec<_>, _>>()?;
    let mut builder = NetworkBuilder::new().with_config_layer(|layer| {
        layer.write(["pipeline", "query_default_cursor_mode"], "stored");
    });
    for permission in
        contract_fixture::deployment_permissions(&iroha_test_samples::ALICE_ID, &aliases)
    {
        builder = builder.with_genesis_instruction(permission);
    }
    let Some((network, _rt)) = sandbox::start_network_blocking_or_skip(
        builder,
        stringify!(smart_contract_query_scenarios),
    )?
    else {
        return Ok(());
    };
    let client = network.client();
    let effect_permissions = [
        iroha_executor_data_model::permission::account::CanModifyAccountMetadata {
            account: client.client().account().clone(),
        }
        .into(),
    ];
    let torii = client.client().endpoint().clone();
    let env_dir = network.env_dir().to_path_buf();
    // The contract writes a synthetic cursor that has no server-owned stored query.
    // Continuing it must fail before executor validation or cursor advancement.
    {
        let bytecode = load_sample_ivm("query_assets_and_save_cursor");
        contract_fixture::deploy(
            &client,
            bytecode.as_ref(),
            aliases[0].clone(),
            &effect_permissions,
        )?;
        let fee_payment = fee_payment_with_gas_limit(&bytecode)?;
        let transaction = {
            let account = client.account_client();
            let mut payload =
                account.prepare_transaction(iroha::client::AccountTransactionDraft::new(
                    bytecode,
                    fee_payment,
                    query_contract_metadata(&aliases[0]),
                ))?;
            let quote = client.quote_fees(iroha::client::FeeQuoteRequest::AccountSignature {
                payload: &payload,
            })?;
            assert!(
                payload
                    .fee_payment
                    .has_same_payer_and_gas_bound(&quote.intent)
            );
            payload.fee_payment = quote.intent;
            account.sign_transaction(payload)
        }
        .expect("build integration-test transaction");
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
        contract_fixture::deploy(
            &client,
            bytecode.as_ref(),
            aliases[1].clone(),
            &effect_permissions,
        )?;
        let fee_payment = fee_payment_with_gas_limit(&bytecode)?;
        let transaction = {
            let account = client.account_client();
            let mut payload =
                account.prepare_transaction(iroha::client::AccountTransactionDraft::new(
                    bytecode,
                    fee_payment,
                    query_contract_metadata(&aliases[1]),
                ))?;
            let quote = client.quote_fees(iroha::client::FeeQuoteRequest::AccountSignature {
                payload: &payload,
            })?;
            assert!(
                payload
                    .fee_payment
                    .has_same_payer_and_gas_bound(&quote.intent)
            );
            payload.fee_payment = quote.intent;
            account.sign_transaction(payload)
        }
        .expect("build integration-test transaction");
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
