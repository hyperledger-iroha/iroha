#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! Ensure Sumeragi commits blocks end-to-end using the Vote/QC pipeline.
use eyre::Result;
use integration_tests::sandbox;
use iroha::data_model::{
    account::Account,
    isi::Register,
    prelude::*,
    query::{account::prelude::FindAccounts, prelude::QueryBuilderExt},
};
use iroha_test_network::{NetworkBuilder, init_instruction_registry};
use iroha_test_samples::gen_account_in;
use std::time::{Duration, Instant};

const TEST_NEXUS_LOCAL_STORAGE_BUDGET_BYTES: i64 = 1024 * 1024 * 1024;

#[test]
fn commits_via_vote_qc_pipeline() -> Result<()> {
    init_instruction_registry();
    let builder = NetworkBuilder::new()
        .with_peers(4)
        .with_auto_populated_trusted_peers()
        .with_config_layer(|layer| {
            layer
                .write("telemetry_profile", "full")
                .write(
                    ["nexus", "storage", "local_budget_bytes"],
                    TEST_NEXUS_LOCAL_STORAGE_BUDGET_BYTES,
                )
                // This gate isolates Vote/QC liveness from fee funding. Keep
                // the canonical fee asset configured, but quote zero charges.
                .write(["nexus", "fees", "base_fee"], "0")
                .write(["nexus", "fees", "per_byte_fee"], "0")
                .write(["nexus", "fees", "per_instruction_fee"], "0")
                .write(["nexus", "fees", "per_gas_unit_fee"], "0");
        });
    let Some((network, rt)) =
        sandbox::start_network_blocking_or_skip(builder, stringify!(commits_via_vote_qc_pipeline))?
    else {
        return Ok(());
    };
    let result = (|| -> Result<()> {
        let client = network.client();
        let genesis_application_deadline = Instant::now() + network.sync_timeout();
        let baseline_non_empty = loop {
            match client.get_status() {
                Ok(status) if status.blocks >= 1 && status.blocks_non_empty >= 1 => {
                    break status.blocks_non_empty;
                }
                Ok(status) if Instant::now() >= genesis_application_deadline => {
                    eyre::bail!(
                        "genesis was not durably applied before transaction submission: blocks={}, blocks_non_empty={}",
                        status.blocks,
                        status.blocks_non_empty
                    );
                }
                Err(error) if Instant::now() >= genesis_application_deadline => {
                    eyre::bail!(
                        "genesis was not durably applied before transaction submission: {error}"
                    );
                }
                Ok(_) | Err(_) => {}
            }
            std::thread::sleep(Duration::from_millis(200));
        };
        let (new_account_id, _) = gen_account_in("wonderland");
        let register_new_account = Register::account(Account::new(new_account_id.clone()));
        client.submit_blocking(
            register_new_account,
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )?;
        let target_non_empty = baseline_non_empty + 1;
        rt.block_on(async {
            network
                .ensure_blocks_with(|height| height.non_empty >= target_non_empty)
                .await
        })?;
        let account_visibility_deadline = Instant::now() + Duration::from_secs(30);
        let accounts = loop {
            let accounts = client.query(FindAccounts).execute_all()?;
            if accounts
                .iter()
                .any(|account| account.id() == &new_account_id)
                || Instant::now() >= account_visibility_deadline
            {
                break accounts;
            }
            std::thread::sleep(Duration::from_millis(200));
        };
        assert!(
            accounts
                .iter()
                .any(|account| account.id() == &new_account_id),
            "new account must exist in WSV after commit"
        );
        let status = client.get_sumeragi_status()?;
        assert!(
            status.last_committed_height >= target_non_empty,
            "exact reducer status should observe the committed transaction"
        );
        assert!(status.height >= status.last_committed_height);
        let qc_json = client.get_sumeragi_qc_json()?;
        assert!(
            qc_json.get("highest_qc").is_some() && qc_json.get("locked_qc").is_some(),
            "qc endpoint should include highest_qc and locked_qc"
        );
        Ok(())
    })();
    rt.block_on(async { network.shutdown().await });
    result
}
