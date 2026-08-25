// Authoritative SoraCloud execution-result regression.
#[test]
fn execute_apartment_returns_authoritative_status_and_commitment() -> Result<()> {
    let mut state = test_state();
    let apartment = sample_agent_record()?;
    {
        let world = &mut Arc::get_mut(&mut state).expect("unique test state").world;
        world.soracloud_agent_apartments_mut_for_testing().insert(
            apartment.manifest.apartment_name.to_string(),
            apartment.clone(),
        );
    }
    let temp_dir = tempfile::tempdir()?;
    let manager = SoracloudRuntimeManager::new(
        test_runtime_manager_config(temp_dir.path().to_path_buf()),
        Arc::clone(&state),
    );
    manager.reconcile_once()?;
    let handle = test_runtime_handle(&manager, Arc::clone(&state));
    let result = handle
        .execute_apartment(SoracloudApartmentExecutionRequest {
            observed_height: 0,
            observed_block_hash: None,
            apartment_name: apartment.manifest.apartment_name.to_string(),
            process_generation: apartment.process_generation,
            operation: "checkpoint".to_owned(),
            request_commitment: Hash::new(b"checkpoint-request"),
        })
        .map_err(|error| eyre::eyre!("{error:?}"))?;
    assert_eq!(result.status, apartment.status);
    assert!(result.checkpoint_artifact_hash.is_none());
    assert!(result.journal_artifact_hash.is_none());
    assert_ne!(result.result_commitment, Hash::new(b"checkpoint-request"));
    Ok(())
}
