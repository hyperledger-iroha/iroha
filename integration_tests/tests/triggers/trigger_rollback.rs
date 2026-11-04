//! Tests that state changes made by failing triggers are reverted.
use eyre::Result;
use integration_tests::{sandbox, sync::run_on_dedicated_thread};
use iroha::data_model::{ValidationFail, prelude::*, query::error::FindError, trigger::TriggerId};
use iroha_data_model::isi::error::{InstructionExecutionError, InvalidParameterError};
use iroha_model_base::domain::DomainId;
use iroha_test_network::*;
use iroha_test_samples::ALICE_ID;
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn failed_trigger_revert() -> Result<()> {
    let Some(network) = sandbox::start_network_async_or_skip(
        NetworkBuilder::new(),
        stringify!(failed_trigger_revert),
    )
    .await?
    else {
        return Ok(());
    };
    let client = network.client();
    //When
    let trigger_id = "trigger".parse::<TriggerId>()?;
    let account_id = ALICE_ID.clone();
    let asset_definition_id = AssetDefinitionId::derive_from_components(
        DomainId::try_new("wonderland", "universal")?,
        "xor".parse()?,
    );
    let create_asset = Register::asset_definition({
        let __asset_definition_id = asset_definition_id.clone();
        AssetDefinition::numeric(
            __asset_definition_id.clone(),
            "xor".to_owned(),
            iroha_data_model::asset::AssetBalancePolicy::Global,
            Some(DomainId::try_new("wonderland", "universal")?),
        )
    });
    let fail_isi = Unregister::domain(DomainId::try_new("dummy", "universal").unwrap());
    let instructions: [InstructionBox; 2] = [create_asset.into(), fail_isi.into()];
    let register_trigger = Register::trigger(Trigger::new(
        trigger_id.clone(),
        Action::new(
            instructions,
            Repeats::Indefinitely,
            account_id.clone(),
            ExecuteTriggerEventFilter::new()
                .for_trigger(trigger_id.clone())
                .under_authority(account_id),
        )
        .expect("trigger action fixture satisfies validation invariants"),
    ));
    run_on_dedicated_thread({
        let client = client.clone();
        move || {
            client.submit(
                register_trigger,
                iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
            )
        }
    })
    .await??;
    let call_trigger = ExecuteTrigger::new(trigger_id);
    let err = run_on_dedicated_thread({
        let client = client.clone();
        move || {
            client.submit(
                call_trigger,
                iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
            )
        }
    })
    .await?
    .expect_err("should immediately result in error");
    // Newer execution path wraps the original FindError in a validation failure; accept either.
    if let Some(FindError::Domain(_)) = err
        .chain()
        .find_map(|cause| cause.downcast_ref::<FindError>())
    {
        // ok
    } else if let Some(instr_err) = err
        .chain()
        .find_map(|cause| cause.downcast_ref::<InstructionExecutionError>())
    {
        match instr_err {
            InstructionExecutionError::Find(FindError::Domain(_)) => {}
            InstructionExecutionError::InvalidParameter(InvalidParameterError::SmartContract(
                msg,
            )) => {
                assert!(
                    msg.contains("unregister domain"),
                    "unexpected SmartContract message: {msg}"
                );
            }
            _ => panic!("unexpected instruction error: {instr_err:?}"),
        }
    } else if let Some(ValidationFail::NotPermitted(msg)) = err
        .chain()
        .find_map(|cause| cause.downcast_ref::<ValidationFail>())
    {
        assert!(
            !msg.trim().is_empty(),
            "NotPermitted message should not be empty"
        );
    } else {
        panic!("unexpected error: {err:?}");
    }
    // The valid first instruction must be rolled back after the missing-domain failure.
    let page = client
        .account_client()
        .list_page(
            &iroha::collections::Collection::AssetDefinitions,
            &iroha::collections::ListQuery::new()
                .filter(iroha::collections::field("id").eq(asset_definition_id.to_string()))
                .limit(1),
        )
        .await?;
    assert!(
        page.next_cursor.is_none(),
        "exact definition absence must be complete"
    );
    assert!(
        page.total.is_none(),
        "native collections do not advertise totals"
    );
    assert!(
        page.items.is_empty(),
        "failed trigger retained its asset definition: {:?}",
        page.items
    );
    Ok(())
}
