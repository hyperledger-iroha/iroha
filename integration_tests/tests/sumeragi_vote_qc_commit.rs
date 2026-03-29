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
use reqwest::Client as HttpClient;

#[test]
fn commits_via_vote_qc_pipeline() -> Result<()> {
    init_instruction_registry();

    let builder = NetworkBuilder::new()
        .with_peers(4)
        .with_auto_populated_trusted_peers()
        .with_config_layer(|layer| {
            layer
                .write("telemetry_enabled", true)
                .write("telemetry_profile", "full");
        });
    let Some((network, rt)) =
        sandbox::start_network_blocking_or_skip(builder, stringify!(commits_via_vote_qc_pipeline))?
    else {
        return Ok(());
    };

    let result = (|| -> Result<()> {
        let client = network.client();
        let baseline_non_empty = client.get_status()?.blocks_non_empty;

        let (new_account_id, _) = gen_account_in("wonderland");
        let wonderland: DomainId = "wonderland".parse()?;
        let new_scoped_account_id = new_account_id.to_account_id(wonderland);
        let register_new_account = Register::account(Account::new_in_domain(
            new_scoped_account_id.account().clone(),
            new_scoped_account_id.domain().clone(),
        ));
        client.submit_blocking(register_new_account)?;

        let target_non_empty = baseline_non_empty + 1;
        rt.block_on(async {
            network
                .ensure_blocks_with(|height| height.non_empty >= target_non_empty)
                .await
        })?;

        let accounts = client.query(FindAccounts).execute_all()?;
        assert!(
            accounts
                .iter()
                .any(|account| account.id() == &new_account_id),
            "new account must exist in WSV after commit"
        );

        let status_json = client.get_sumeragi_status_json()?;
        assert!(
            status_json.get("leader_index").is_some(),
            "status endpoint should return leader_index"
        );

        let qc_json = client.get_sumeragi_qc_json()?;
        assert!(
            qc_json.get("highest_qc").is_some() && qc_json.get("locked_qc").is_some(),
            "qc endpoint should include highest_qc and locked_qc"
        );

        let phases_json = client.get_sumeragi_phases_json()?;
        assert!(
            phases_json.get("commit_ms").is_some(),
            "phases endpoint should expose commit_ms"
        );

        let rbc_status_json = client.get_sumeragi_rbc_status_json()?;
        assert!(
            rbc_status_json.get("sessions_active").is_some(),
            "RBC status endpoint should expose sessions_active"
        );

        let rbc_sessions_json = client.get_sumeragi_rbc_sessions_json()?;
        assert!(
            rbc_sessions_json.get("items").is_some(),
            "RBC sessions endpoint should expose items array"
        );

        let telemetry_url = client.torii_url.join("v1/sumeragi/telemetry")?;
        rt.block_on(async {
            let http = HttpClient::new();
            let resp = http.get(telemetry_url.clone()).send().await?;
            if !resp.status().is_success() {
                eyre::bail!("telemetry endpoint returned {}", resp.status());
            }
            let body = resp.text().await?;
            let payload: norito::json::Value = norito::json::from_str(&body)?;
            eyre::ensure!(
                payload.get("availability").is_some(),
                "telemetry payload should include availability section"
            );
            let vrf = payload
                .get("vrf")
                .and_then(|v| v.as_object())
                .ok_or_else(|| eyre::eyre!("telemetry payload missing vrf summary"))?;
            eyre::ensure!(
                vrf.get("found")
                    .and_then(norito::json::Value::as_bool)
                    .is_some(),
                "vrf summary should expose found boolean"
            );
            eyre::ensure!(
                vrf.contains_key("reveals_total") && vrf.contains_key("late_reveals_total"),
                "vrf summary should expose reveal totals"
            );
            Ok::<(), eyre::Report>(())
        })?;

        Ok(())
    })();

    rt.block_on(async { network.shutdown().await });

    result
}
