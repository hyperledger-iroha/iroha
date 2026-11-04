//! Additional Torii iterable query checks exercising server-side batching and
//! multiple item types (blocks, transactions).
use eyre::{Result, WrapErr};
use integration_tests::sandbox;
use iroha::{
    collections::{Collection, ListQuery},
    data_model::prelude::*,
};
use iroha_model_base::domain::DomainId;
use iroha_model_base::name::Name;
use std::{thread::sleep, time::Duration};
// use iroha_data_model::query::builder::QueryBuilderExt as _; // trait extension not needed in this test
use iroha_test_network::NetworkBuilder;
use sandbox::start_network_blocking_or_skip as start_network_or_skip;
fn is_permission_grant_repetition(
    err: &eyre::Report,
    expected: &iroha::data_model::permission::Permission,
) -> bool {
    use iroha::data_model::{
        IdBox, ValidationFail,
        isi::{InstructionType, error::InstructionExecutionError},
    };
    err.chain().any(|cause| {
        let matches = |repetition: &iroha::data_model::isi::error::RepetitionError| {
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
#[test]
fn blocks_iterable_start_and_continue() -> Result<()> {
    let Some((network, rt)) = start_network_or_skip(
        NetworkBuilder::new().with_config_layer(|layer| {
            layer.write(["pipeline", "query_default_cursor_mode"], "stored");
        }),
        stringify!(blocks_iterable_start_and_continue),
    )?
    else {
        return Ok(());
    };
    let client = network.client();
    // Submit two transactions so the first fetch-size=1 page has a real
    // continuation even if genesis is not included in the block-header query.
    for name in ["blkcheck_a", "blkcheck_b"] {
        client.submit(
            Register::asset_definition({
                let __asset_definition_id = AssetDefinitionId::derive_from_components(
                    DomainId::try_new("wonderland", "universal")?,
                    name.parse()?,
                );
                AssetDefinition::numeric(
                    __asset_definition_id.clone(),
                    name.to_owned(),
                    iroha_data_model::asset::AssetBalancePolicy::Global,
                    Some(DomainId::try_new("wonderland", "universal")?),
                )
            }),
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )?;
    }
    rt.block_on(async { network.ensure_blocks(3).await })?;
    let headers = retry_block_headers(&client)?;
    assert!(
        headers.len() >= 2,
        "expected at least two block headers before testing cursor continuation"
    );
    let query = ListQuery::new().limit(1);
    let first = client.list_page(&Collection::ExplorerBlocks, &query)?;
    assert_eq!(first.items.len(), 1, "expected one block in the first page");
    let next = query
        .next_page(&first)
        .expect("expected continuation cursor");
    let second = client.list_page(&Collection::ExplorerBlocks, &next)?;
    assert!(
        !second.items.is_empty(),
        "expected another block in the second page"
    );
    let first_height = first.items[0]["height"].as_u64().expect("block height");
    let second_height = second.items[0]["height"].as_u64().expect("block height");
    assert!(
        first_height > second_height,
        "cursor must advance to an older block"
    );
    Ok(())
}
#[test]
fn transactions_iterable_non_empty() -> Result<()> {
    use std::{
        thread,
        time::{Duration, Instant},
    };
    let Some((network, _rt)) = start_network_or_skip(
        NetworkBuilder::new(),
        stringify!(transactions_iterable_non_empty),
    )?
    else {
        return Ok(());
    };
    let client = network.client();
    // At minimum, genesis should exist. Poll briefly to allow any background commits
    // to surface before asserting on the iterable query results.
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let snapshot = client
            .list(Collection::Transactions, ListQuery::new())
            .collect::<iroha::Result<Vec<_>>>()
            .expect("query transactions");
        if !snapshot.is_empty() {
            break;
        }
        assert!(
            Instant::now() <= deadline,
            "expected at least one committed transaction, found none"
        );
        thread::sleep(Duration::from_millis(100));
    }
    Ok(())
}
#[test]
fn find_block_headers_descending() -> Result<()> {
    let Some((network, rt)) = start_network_or_skip(
        NetworkBuilder::new(),
        stringify!(find_block_headers_descending),
    )?
    else {
        return Ok(());
    };
    let client = network.client();
    // Submit a couple of extra transactions so we have more than one header
    // even if the block builder batches them together.
    client.submit(
        Register::asset_definition({
            let __asset_definition_id = AssetDefinitionId::derive_from_components(
                DomainId::try_new("wonderland", "universal")?,
                "blkcheck2".parse()?,
            );
            AssetDefinition::numeric(
                __asset_definition_id.clone(),
                "blkcheck2".to_owned(),
                iroha_data_model::asset::AssetBalancePolicy::Global,
                Some(DomainId::try_new("wonderland", "universal")?),
            )
        }),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )?;
    client.submit(
        Register::asset_definition({
            let __asset_definition_id = AssetDefinitionId::derive_from_components(
                DomainId::try_new("wonderland", "universal")?,
                "blkcheck3".parse()?,
            );
            AssetDefinition::numeric(
                __asset_definition_id.clone(),
                "blkcheck3".to_owned(),
                iroha_data_model::asset::AssetBalancePolicy::Global,
                Some(DomainId::try_new("wonderland", "universal")?),
            )
        }),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )?;
    rt.block_on(async { network.ensure_blocks(3).await })?;
    let headers = retry_block_headers(&client)?;
    assert!(headers.len() >= 2, "expected multiple block headers");
    assert!(headers.windows(2).all(|w| {
        // Compare block heights explicitly; derived ordering for `BlockHeader`
        // includes additional fields that do not mirror ledger height.
        let left_height = w[0]["height"].as_u64().expect("block height");
        let right_height = w[1]["height"].as_u64().expect("block height");
        left_height >= right_height
    }));
    Ok(())
}
fn retry_block_headers(client: &iroha::blocking::Client) -> Result<Vec<norito::json::Value>> {
    const RETRIES: usize = 5;
    const DELAY: Duration = Duration::from_millis(200);
    for attempt in 0..RETRIES {
        match client
            .list(Collection::ExplorerBlocks, ListQuery::new())
            .collect::<iroha::Result<Vec<_>>>()
        {
            Ok(headers) => return Ok(headers),
            Err(_err) if attempt + 1 < RETRIES => sleep(DELAY),
            Err(err) => return Err(err.into()),
        }
    }
    unreachable!()
}
#[test]
fn find_triggers_includes_registered() -> Result<()> {
    use iroha::data_model::events::time::{ExecutionTime, TimeEventFilter};
    use iroha_test_samples::ALICE_ID;
    let Some((network, rt)) = start_network_or_skip(
        NetworkBuilder::new(),
        stringify!(find_triggers_includes_registered),
    )?
    else {
        return Ok(());
    };
    let client = network.client();
    // Register a simple pre-commit time trigger
    let trig_id: TriggerId = "qtrig_iterable".parse()?;
    let key: Name = "iterable_flag".parse()?;
    let instr = SetKeyValue::account(ALICE_ID.clone(), key, Json::new(true));
    let trig = Trigger::new(
        trig_id.clone(),
        Action::new(
            vec![InstructionBox::from(instr)],
            Repeats::Indefinitely,
            ALICE_ID.clone(),
            TimeEventFilter::new(ExecutionTime::PreCommit),
        )
        .expect("trigger action fixture satisfies validation invariants"),
    );
    client.submit(
        Register::trigger(trig),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )?;
    rt.block_on(async { network.ensure_blocks(2).await })?;
    // Query triggers and ensure the registered one is present
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    let snapshot = loop {
        let snapshot = client
            .client()
            .query(FindTriggers)
            .execute_all()
            .expect("query triggers");
        if snapshot.iter().any(|t| t.id() == &trig_id) {
            break snapshot;
        }
        assert!(
            std::time::Instant::now() <= deadline,
            "registered trigger id not found in results; last snapshot: {:?}",
            snapshot.iter().map(|t| t.id().clone()).collect::<Vec<_>>()
        );
        std::thread::sleep(std::time::Duration::from_millis(100));
    };
    assert!(
        !snapshot.is_empty(),
        "expected at least one trigger after registration"
    );
    Ok(())
}
#[test]
fn find_active_trigger_ids_includes_registered() -> Result<()> {
    use iroha_test_samples::ALICE_ID;
    let Some((network, rt)) = start_network_or_skip(
        NetworkBuilder::new(),
        stringify!(find_active_trigger_ids_includes_registered),
    )?
    else {
        return Ok(());
    };
    let client = network.client();
    // Ensure Alice may register by-call triggers explicitly to avoid permission denials.
    let permission = iroha_executor_data_model::permission::trigger::CanRegisterTrigger {
        authority: ALICE_ID.clone(),
    };
    let expected_permission: iroha::data_model::permission::Permission = permission.clone().into();
    if let Err(err) = client.submit(
        Grant::account_permission(permission, ALICE_ID.clone()),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    ) && !is_permission_grant_repetition(&err, &expected_permission)
    {
        return Err(err);
    }
    // Register a by-call trigger that won't fire automatically; keep it active.
    let trig_id: TriggerId = "qtrig_active_ids".parse()?;
    let trig = Trigger::new(
        trig_id.clone(),
        Action::new(
            Vec::<InstructionBox>::new(),
            Repeats::Indefinitely,
            ALICE_ID.clone(),
            ExecuteTriggerEventFilter::new().for_trigger(trig_id.clone()),
        )
        .expect("trigger action fixture satisfies validation invariants"),
    );
    client.submit(
        Register::trigger(trig),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )?;
    rt.block_on(async { network.ensure_blocks(2).await })?;
    // Query active trigger IDs and ensure the one we registered is present.
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    loop {
        let snapshot = client
            .client()
            .query(FindActiveTriggerIds)
            .execute_all()
            .expect("query active trigger ids");
        if snapshot.iter().any(|id| id == &trig_id) {
            break;
        }
        assert!(
            std::time::Instant::now() <= deadline,
            "registered trigger id not found among active trigger IDs; last snapshot: {snapshot:?}"
        );
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    Ok(())
}
#[test]
fn burn_trigger_repetitions_removes_from_active_ids() -> Result<()> {
    use iroha_test_samples::ALICE_ID;
    let Some((network, rt)) = start_network_or_skip(
        NetworkBuilder::new(),
        stringify!(burn_trigger_repetitions_removes_from_active_ids),
    )?
    else {
        return Ok(());
    };
    let client = network.client();
    // Register a by-call trigger with Exactly(1) repeat so it stays active until we burn it.
    let trig_id: TriggerId = "qtrig_disable_via_burn".parse()?;
    let trig = Trigger::new(
        trig_id.clone(),
        Action::new(
            Vec::<InstructionBox>::new(),
            Repeats::Exactly(1),
            ALICE_ID.clone(),
            ExecuteTriggerEventFilter::new().for_trigger(trig_id.clone()),
        )
        .expect("trigger action fixture satisfies validation invariants"),
    );
    client.submit(
        Register::trigger(trig),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )?;
    rt.block_on(async { network.ensure_blocks(2).await })?;
    // Sanity: trigger id is present among active trigger IDs
    let ids = client.client().query(FindActiveTriggerIds).execute_all()?;
    assert!(ids.iter().any(|id| id == &trig_id));
    // Burn 1 repetition -> reaches zero, core prunes trigger immediately in the burn executor
    client.submit(
        Burn::trigger_repetitions(1, trig_id.clone()),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )?;
    rt.block_on(async { network.ensure_blocks(3).await })?;
    // Poll until the trigger is removed from the active set; on slower environments
    // the removal happens asynchronously.
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    loop {
        let snapshot = client.client().query(FindActiveTriggerIds).execute_all()?;
        if snapshot.iter().all(|id| id != &trig_id) {
            break;
        }
        assert!(
            std::time::Instant::now() <= deadline,
            "expected trigger id to be removed from active list after burn to zero; last snapshot: {snapshot:?}"
        );
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    Ok(())
}
#[test]
fn burn_then_execute_trigger_is_rejected() -> Result<()> {
    use iroha_test_samples::ALICE_ID;
    let Some((network, rt)) = start_network_or_skip(
        NetworkBuilder::new(),
        stringify!(burn_then_execute_trigger_is_rejected),
    )?
    else {
        return Ok(());
    };
    let client = network.client();
    let torii = client.client().endpoint().clone();
    let env_dir = network.env_dir().to_path_buf();
    // Register a by-call trigger with Exactly(1) repeat
    let trig_id: TriggerId = "qtrig_burn_then_exec".parse()?;
    let trig = Trigger::new(
        trig_id.clone(),
        Action::new(
            Vec::<InstructionBox>::new(),
            Repeats::Exactly(1),
            ALICE_ID.clone(),
            ExecuteTriggerEventFilter::new().for_trigger(trig_id.clone()),
        )
        .expect("trigger action fixture satisfies validation invariants"),
    );
    client
        .submit(
            Register::trigger(trig),
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .wrap_err(format!(
            "register trigger; torii={torii}, env_dir={}",
            env_dir.display()
        ))?;
    rt.block_on(async { network.ensure_blocks(2).await })
        .wrap_err(format!(
            "ensure_blocks(2) after trigger registration; torii={torii}, env_dir={}",
            env_dir.display()
        ))?;
    // Burn to zero
    client
        .submit(
            Burn::trigger_repetitions(1, trig_id.clone()),
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .wrap_err(format!(
            "burn trigger repetitions; torii={torii}, env_dir={}",
            env_dir.display()
        ))?;
    // Wait for the burn transaction to commit before checking trigger state.
    rt.block_on(async {
        network
            .ensure_blocks_with(|height| height.non_empty >= 3)
            .await
    })
    .wrap_err(format!(
        "ensure_blocks_with(total>=3) after burn; torii={torii}, env_dir={}",
        env_dir.display()
    ))?;
    // Wait until the trigger disappears from the active set; on slower CI
    // pipelines the removal is observed asynchronously.
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    loop {
        let snapshot = client
            .client()
            .query(FindActiveTriggerIds)
            .execute_all()
            .wrap_err(format!(
                "query active trigger ids; torii={torii}, env_dir={}",
                env_dir.display()
            ))?;
        if snapshot.iter().all(|id| id != &trig_id) {
            break;
        }
        assert!(
            std::time::Instant::now() <= deadline,
            "trigger id still reported as active after burn; last snapshot: {snapshot:?}"
        );
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    // Attempt to execute; expect rejection referencing FindError::Trigger(trig_id)
    let err = client
        .submit(
            Instruction::into_instruction_box(Box::new(ExecuteTrigger::new(trig_id.clone()))),
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .expect_err("execute should be rejected for depleted trigger");
    let expected = format!("permission references unknown trigger `{trig_id}`");
    assert!(
        err.chain().any(|cause| matches!(
            cause.downcast_ref::<iroha::data_model::ValidationFail>(),
            Some(iroha::data_model::ValidationFail::NotPermitted(reason)) if reason == &expected
        )),
        "unexpected rejection for the depleted trigger: {err:?}"
    );
    // And confirm it's not among active IDs
    let snapshot = client
        .client()
        .query(FindActiveTriggerIds)
        .execute_all()
        .wrap_err(format!(
            "final active trigger ids query; torii={torii}, env_dir={}",
            env_dir.display()
        ))?;
    assert!(snapshot.iter().all(|id| id != &trig_id));
    Ok(())
}
