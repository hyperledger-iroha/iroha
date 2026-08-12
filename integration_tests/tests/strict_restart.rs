use super::*;

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "requires local process orchestration"]
async fn taira_localnet_restart_catchup_behavior() -> Result<()> {
    init_instruction_registry();
    let _guard = sandbox::serial_guard();

    let temp_dir = localnet_tempdir("taira-restart")?;
    let out_dir = temp_dir.path().join("localnet");
    let result: Result<()> = async {
        let mut harness = setup_taira_harness::<true>(&out_dir, "taira-restart", 0).await?;
        let _ = process_churn_cycle(&mut harness, 0, Duration::from_secs(PROCESS_DOWNTIME_SECS))
            .await?;
        Ok(())
    }
    .await;

    finalize_result(temp_dir, "taira_localnet_restart_catchup_behavior", result)
}
