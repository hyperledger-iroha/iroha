use iroha::{
    client::Client,
    data_model::{
        asset::AssetId,
        prelude::{FindAssets, Numeric, QueryBuilderExt, *},
    },
};
use iroha_test_network::*;
use iroha_test_samples::{gen_account_in, load_sample_wasm, ALICE_ID};

mod by_call_trigger;
mod data_trigger;
mod event_trigger;
mod orphans;
// FIXME: rewrite all in async and with shorter timings
mod time_trigger;
mod trigger_rollback;

fn get_asset_value(client: &Client, asset_id: AssetId) -> Numeric {
    let asset = client
        .query(FindAssets::new())
        .filter_with(|asset| asset.id.eq(asset_id))
        .execute_single()
        .unwrap();

    *asset.value()
}

/// See the corresponding unit test `iroha_tree::state::tests::detects_event_loop`.
#[cfg(feature = "prediction")]
#[test]
fn not_registered_when_potential_event_loop_detected() -> eyre::Result<()> {
    // Trigger that:
    // - Subscribes to changes in the domain "dom_{i}" with statuses "{s}", which denotes `xxx`.
    // - Publishes the deletion of the domain "dom_{j}".
    let when_i_xxx_then_j_del = |i: usize, s: &str, xxx: DomainEventSet, j: usize| {
        Trigger::new(
            format!("trg_{i}{s}_{j}d").parse().unwrap(),
            Action::new(
                vec![Unregister::domain(format!("dom_{j}").parse().unwrap())],
                Repeats::Indefinitely,
                ALICE_ID.clone(),
                DomainEventFilter::new()
                    .for_domain(format!("dom_{i}").parse().unwrap())
                    .for_events(xxx),
            ),
        )
    };
    let when_0_del_then_1_del = when_i_xxx_then_j_del(0, "d", DomainEventSet::Deleted, 1);
    // A potential connection exists through the deletion of "dom_1".
    let when_1_del_then_2_del = when_i_xxx_then_j_del(1, "d", DomainEventSet::Deleted, 2);
    let (network, _rt) = NetworkBuilder::new()
        .with_genesis_instruction(Register::trigger(when_0_del_then_1_del))
        .with_genesis_instruction(Register::trigger(when_1_del_then_2_del))
        .start_blocking()?;
    let test_client = network.client();

    for (entry, leads_to_event_loop) in [
        // Short-circuiting.
        (
            when_i_xxx_then_j_del(2, "d", DomainEventSet::Deleted, 0),
            true,
        ),
        // No short-circuiting due to status mismatch.
        (
            when_i_xxx_then_j_del(
                2,
                "cu",
                DomainEventSet::Created | DomainEventSet::OwnerChanged,
                0,
            ),
            false,
        ),
        // Extending the graph.
        (
            when_i_xxx_then_j_del(2, "d", DomainEventSet::Deleted, 3),
            false,
        ),
        // Creating another cyclic cluster.
        (
            when_i_xxx_then_j_del(3, "d", DomainEventSet::Deleted, 3),
            true,
        ),
        // Creating another acyclic cluster.
        (
            when_i_xxx_then_j_del(3, "d", DomainEventSet::Deleted, 4),
            false,
        ),
        {
            let when_3_del_then_register_another = Trigger::new(
                "trg_3d_register_another".parse().unwrap(),
                Action::new(
                    vec![Register::trigger(when_i_xxx_then_j_del(
                        10,
                        "d",
                        DomainEventSet::Deleted,
                        20,
                    ))],
                    Repeats::Indefinitely,
                    ALICE_ID.clone(),
                    DomainEventFilter::new()
                        .for_domain("dom_3".parse().unwrap())
                        .for_events(DomainEventSet::Deleted),
                ),
            );
            // Creating an additional trigger.
            (when_3_del_then_register_another, true)
        },
    ] {
        match test_client.submit_blocking(Register::trigger(entry)) {
            Ok(_) => assert!(!leads_to_event_loop),
            Err(err) => {
                use iroha_data_model::isi::error::InstructionExecutionError;
                match err.root_cause().downcast_ref::<InstructionExecutionError>() {
                    Some(InstructionExecutionError::InvariantViolation(msg)) => {
                        assert!(msg.contains("trigger registration leads to event loop"))
                    }
                    _ => eyre::bail!("failed due to an unexpected error:{err}"),
                }
                assert!(leads_to_event_loop);
            }
        }
    }

    Ok(())
}
