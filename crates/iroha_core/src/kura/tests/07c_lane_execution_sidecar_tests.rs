    #[test]
    fn lane_execution_evidence_overrides_batched_fsync_and_reissues_failed_barriers() {
        let temp_dir = TempDir::new().expect("create temp dir");
        let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
        assert_eq!(
            config.fsync_mode,
            FsyncMode::Batched,
            "fixture must exercise the shipped batched fsync mode"
        );
        let lane_config = two_lane_runtime_config();
        let lane_id = LaneId::from(1);
        let lane_entry = lane_config.entry(lane_id).expect("lane entry");
        let lane_block_height = 1;
        let block = dummy_block_with_lane_payload_ownership(
            lane_id,
            lane_entry.dataspace_id,
            lane_block_height,
        );
        let ownership = block
            .execution_context()
            .expect("execution context")
            .lane_payload_ownerships
            .first()
            .expect("lane ownership")
            .clone();
        let proposal = lane_block_proposal_from_ownership(&ownership);
        let (kura, _) = test_kura_with_default_lane_markers(&config, &lane_config);
        kura.store_block(block)
            .expect("store canonical lane payload source");
        let recovered = kura
            .recover_lane_block_payload(&proposal)
            .expect("recover exact execution input");

        for (label, inject_failure) in strict_indexed_sidecar_failure_modes() {
            inject_failure();
            assert!(
                kura.persist_lane_block_execution_input(&recovered).is_err(),
                "injected {label} execution-input barrier failure must be reported"
            );
        }
        kura.persist_lane_block_execution_input(&recovered)
            .expect("execution-input retry must reissue every strict barrier");
        let input = kura
            .read_lane_block_execution_input(lane_id, lane_block_height)
            .expect("strict execution input");

        let state_hash = Some(HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(
            b"strict lane execution preflight state",
        )));
        let results = vec![TransactionResult::new(TransactionResultInner::Ok(
            DataTriggerSequence::new(),
        ))];
        for (label, inject_failure) in strict_indexed_sidecar_failure_modes() {
            inject_failure();
            assert!(
                kura.persist_lane_block_execution_preflight(
                    &input,
                    7,
                    state_hash.clone(),
                    results.clone(),
                )
                .is_err(),
                "injected {label} execution-preflight barrier failure must be reported"
            );
        }
        kura.persist_lane_block_execution_preflight(&input, 7, state_hash, results.clone())
            .expect("execution-preflight retry must reissue every strict barrier");
        let preflight = kura
            .read_lane_block_execution_preflight(lane_id, lane_block_height)
            .expect("strict execution preflight");
        assert_eq!(preflight.results, results);
    }

    #[test]
    fn lane_block_execution_input_persists_recovered_payload_and_reloads() {
        let temp_dir = TempDir::new().expect("create temp dir");
        let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
        let lane_config = two_lane_runtime_config();
        let lane_id = LaneId::from(1);
        let lane_entry = lane_config.entry(lane_id).expect("lane entry");
        let lane_block_height = 1;
        let block = dummy_block_with_lane_payload_ownership(
            lane_id,
            lane_entry.dataspace_id,
            lane_block_height,
        );
        let ownership = block
            .execution_context()
            .expect("execution context")
            .lane_payload_ownerships
            .first()
            .expect("lane ownership")
            .clone();
        let proposal = lane_block_proposal_from_ownership(&ownership);

        let (kura, _) = test_kura_with_default_lane_markers(&config, &lane_config);
        kura.store_block(block)
            .expect("store block with lane artifact");
        let recovered = kura
            .recover_lane_block_payload(&proposal)
            .expect("recover executable lane payload");
        kura.persist_lane_block_execution_input(&recovered)
            .expect("persist lane execution input");
        kura.persist_lane_block_execution_input(&recovered)
            .expect("duplicate lane execution input persistence is idempotent");

        let input = kura
            .read_lane_block_execution_input(lane_id, lane_block_height)
            .expect("lane execution input");
        assert_eq!(input.format_label(), "lane.execution_input");
        assert_eq!(input.proposal, proposal);
        assert_eq!(input.artifact, recovered.artifact);
        assert_eq!(
            input.entrypoint_hashes,
            proposal.descriptor.accepted_transaction_hashes
        );
        assert_eq!(input.entrypoints, recovered.entrypoints);
        assert!(kura.lane_block_execution_input_available(&proposal));

        let (data_path, index_path) =
            Kura::lane_block_execution_input_paths_for_entry(lane_entry, temp_dir.path());
        assert!(
            data_path.is_file(),
            "lane execution input data file missing"
        );
        assert!(
            index_path.is_file(),
            "lane execution input index file missing"
        );

        drop(kura);
        let (reloaded, _) = Kura::new(&config, &lane_config).expect("reopen kura");
        assert_eq!(
            reloaded.read_lane_block_execution_input(lane_id, lane_block_height),
            Some(input)
        );
        assert!(reloaded.lane_block_execution_input_available(&proposal));
    }

    #[test]
    fn lane_execution_sidecars_validate_without_recursive_prune_repair() {
        let temp_dir = TempDir::new().expect("create temp dir");
        let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
        let lane_config = two_lane_runtime_config();
        let lane_id = LaneId::from(1);
        let lane_entry = lane_config.entry(lane_id).expect("lane entry");
        let lane_block_height = 1;
        let block = dummy_block_with_lane_payload_ownership(
            lane_id,
            lane_entry.dataspace_id,
            lane_block_height,
        );
        let ownership = block
            .execution_context()
            .expect("execution context")
            .lane_payload_ownerships
            .first()
            .expect("lane ownership")
            .clone();
        let proposal = lane_block_proposal_from_ownership(&ownership);

        let (kura, _) = test_kura_with_default_lane_markers(&config, &lane_config);
        kura.store_block(block)
            .expect("store block with lane artifact");
        let recovered = kura
            .recover_lane_block_payload(&proposal)
            .expect("recover executable lane payload");
        let (artifact_data_path, artifact_index_path) =
            Kura::lane_artifact_paths_for_entry(lane_entry, temp_dir.path());
        std::fs::remove_file(&artifact_data_path).expect("remove lane artifact data sidecar");
        std::fs::remove_file(&artifact_index_path).expect("remove lane artifact index sidecar");
        assert!(
            kura.read_lane_block_artifact(lane_id, lane_block_height)
                .is_none(),
            "test setup must remove the repairable lane artifact sidecar",
        );

        let worker_kura = Arc::clone(&kura);
        let (done_tx, done_rx) = std::sync::mpsc::sync_channel(1);
        let worker = thread::spawn(move || {
            let outcome = (|| -> std::result::Result<(), String> {
                worker_kura
                    .persist_lane_block_execution_input(&recovered)
                    .map_err(|error| format!("persist execution input: {error:?}"))?;
                let input = worker_kura
                    .read_lane_block_execution_input_with_repair_policy(
                        lane_id,
                        lane_block_height,
                        false,
                    )
                    .ok_or_else(|| "read execution input after persistence".to_owned())?;
                let state_hash = Some(HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(
                    b"missing lane artifact direct application state",
                )));
                let result =
                    TransactionResult::new(TransactionResultInner::Ok(DataTriggerSequence::new()));
                worker_kura
                    .persist_lane_block_execution_preflight(&input, 7, state_hash, vec![result])
                    .map_err(|error| format!("persist execution preflight: {error:?}"))?;
                let preflight = worker_kura
                    .read_lane_block_execution_preflight_with_repair_policy(
                        lane_id,
                        lane_block_height,
                        false,
                    )
                    .ok_or_else(|| "read execution preflight after persistence".to_owned())?;
                worker_kura
                    .persist_direct_lane_block_application_receipt(&input, &preflight)
                    .map_err(|error| format!("persist direct receipt: {error:?}"))?;
                Ok(())
            })();
            done_tx.send(outcome).expect("report sidecar outcome");
        });
        done_rx
            .recv_timeout(Duration::from_secs(5))
            .expect("lane sidecar validation must not recursively lock prune_lock")
            .unwrap_or_else(|error| panic!("lane sidecar validation failed: {error}"));
        worker.join().expect("lane sidecar validation worker");

        assert!(
            kura.read_lane_block_artifact(lane_id, lane_block_height)
                .is_none(),
            "validation under prune_lock must not repair the missing lane artifact sidecar",
        );
        let receipt = kura
            .read_active_lane_block_application_receipt_structural(
                lane_id,
                lane_block_height,
                false,
            )
            .expect("read direct receipt without sidecar repair");
        assert!(
            kura.lane_block_application_receipt_matches_available_evidence(&receipt, false),
            "execution input, preflight, and direct receipt must remain usable without repair",
        );
        assert!(
            kura.read_lane_block_artifact(lane_id, lane_block_height)
                .is_none(),
            "nonrepair evidence validation must leave the missing lane artifact absent",
        );
        assert_eq!(
            kura.read_lane_block_application_receipt(lane_id, lane_block_height),
            Some(receipt),
            "the public repair-enabled receipt reader must retain valid evidence",
        );
        assert!(
            kura.read_lane_block_artifact(lane_id, lane_block_height)
                .is_some(),
            "the public repair-enabled reader must recover the missing lane artifact",
        );
    }
