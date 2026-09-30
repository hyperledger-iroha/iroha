test_items! {
    fn moderation_ballots_list_prints_payload() {
        let args = ModerationBallotsListArgs { limit: Some(8) };
        let mut ctx = TestContext::new();
        args.run_with(&mut ctx, |_client, filter| {
            assert_eq!(filter.limit, Some(8));
json_response_fixture!(StatusCode::OK, &norito::json!({
                    "ballots": [
                        { "case_id": "case-401", "round_id": "round-7" }
                    ]
                }))
        })
        .expect("run should succeed");
        assert_eq!(ctx.printed.len(), 1);
        assert!(ctx.printed[0].contains("\"ballots\""));
    }
    fn moderation_ballots_get_trims_identifiers() {
        let args = ModerationBallotsGetArgs {
            case_id: " case-401 ".to_string(),
            round_id: " round-7 ".to_string(),
            limit: Some(3),
        };
        let mut ctx = TestContext::new();
        args.run_with(&mut ctx, |_client, case_id, round_id, filter| {
            assert_eq!(case_id, "case-401");
            assert_eq!(round_id, "round-7");
            assert_eq!(filter.limit, Some(3));
json_response_fixture!(StatusCode::OK, &norito::json!({
                    "case_id": "case-401",
                    "round_id": "round-7"
                }))
        })
        .expect("run should succeed");
        assert_eq!(ctx.printed.len(), 1);
        assert!(ctx.printed[0].contains("\"case-401\""));
    }
    fn moderation_ballots_no_show_plan_trims_identifiers_and_prints_payload() {
        let args = ModerationBallotsNoShowPlanArgs {
            case_id: " case-401 ".to_string(),
            round_id: " round-7 ".to_string(),
        };
        let mut ctx = TestContext::new();
        args.run_with(&mut ctx, |_client, case_id, round_id| {
            assert_eq!(case_id, "case-401");
            assert_eq!(round_id, "round-7");
json_response_fixture!(StatusCode::OK, &norito::json!({
                    "schema": "sorafs.moderation.ballot.no_show_plan.v1",
                    "case_id": "case-401",
                    "round_id": "round-7",
                    "no_show_count": 2,
                    "penalty_plan_digest_hex": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
                }))
        })
        .expect("run should succeed");
        assert_eq!(ctx.printed.len(), 1);
        assert!(ctx.printed[0].contains("\"no_show_count\""));
        assert!(ctx.printed[0].contains("\"penalty_plan_digest_hex\""));
    }
    fn moderation_ballots_events_prints_payload() {
        let args = ModerationBallotsEventsArgs {
            since: Some(12),
            limit: Some(4),
        };
        let mut ctx = TestContext::new();
        args.run_with(&mut ctx, |_client, filter| {
            assert_eq!(filter.since, Some(12));
            assert_eq!(filter.limit, Some(4));
json_response_fixture!(StatusCode::OK, &norito::json!({
                    "events": [
                        { "sequence": 13, "kind": "commit_accepted" }
                    ]
                }))
        })
        .expect("run should succeed");
        assert_eq!(ctx.printed.len(), 1);
        assert!(ctx.printed[0].contains("\"events\""));
    }
    fn moderation_ballots_commit_reads_json_payload() {
        let mut ctx = TestContext::new();
        let commit = moderation_ballot_commit_fixture_for_juror(&ctx.cfg.account.to_string());
        let mut file = NamedTempFile::new().expect("commit file");
        file.write_all(
            norito::json::to_json_pretty(&commit)
                .expect("render commit json")
                .as_bytes(),
        )
        .expect("write commit json");
        let args = ModerationBallotsCommitArgs {
            payload: file.path().to_path_buf(),
            format: "json".to_string(),
        };
        args.run_with(&mut ctx, |_client, transaction| {
            transaction.verify_signature().expect("canonical signed public transaction");
            assert_eq!(moderation_commit_from_transaction(transaction), commit);
            Ok(transaction.hash())
        })
        .expect("run should succeed");
        assert_eq!(ctx.printed.len(), 1);
        assert!(ctx.printed[0].contains("\"transaction_hash_hex\""));
    }
    fn moderation_ballots_reveal_reads_norito_payload() {
        let mut ctx = TestContext::new();
        let reveal = moderation_ballot_reveal_fixture_for_juror(&ctx.cfg.account.to_string());
        let encoded = to_bytes(&reveal).expect("encode reveal");
        let mut file = NamedTempFile::new().expect("reveal file");
        file.write_all(&encoded).expect("write reveal norito");
        let args = ModerationBallotsRevealArgs {
            payload: file.path().to_path_buf(),
            format: "norito".to_string(),
        };
        args.run_with(&mut ctx, |_client, transaction| {
            assert_eq!(moderation_reveal_from_transaction(transaction), reveal);
            Ok(transaction.hash())
        })
        .expect("run should succeed");
        assert_eq!(ctx.printed.len(), 1);
        assert!(ctx.printed[0].contains("\"transaction_hash_hex\""));
    }
    fn moderation_ballots_tally_builds_request() {
        let args = ModerationBallotsTallyArgs {
            case_id: " case-401 ".to_string(),
            round_id: " round-7 ".to_string(),
        };
        let mut ctx = TestContext::new();
        args.run_with(&mut ctx, |_client, transaction| {
            let instruction = moderation_finalization_from_transaction(transaction);
            assert_eq!(instruction.case_id(), "case-401");
            assert_eq!(instruction.round_id(), "round-7");
            Ok(transaction.hash())
        })
        .expect("run should succeed");
        assert_eq!(ctx.printed.len(), 1);
        assert!(ctx.printed[0].contains("\"transaction_hash_hex\""));
    }
    fn moderation_ballots_commit_rejects_invalid_format() {
        let file = NamedTempFile::new().expect("commit file");
        let args = ModerationBallotsCommitArgs {
            payload: file.path().to_path_buf(),
            format: "yaml".to_string(),
        };
        let mut ctx = TestContext::new();
        let err = args
            .run_with(&mut ctx, |_client, _| unreachable!("submit must not run"))
            .expect_err("invalid format must be rejected");
        assert!(err.to_string().contains("--format"));
        assert!(ctx.printed.is_empty());
    }
    fn moderation_native_action_and_coordination_inputs_are_bounded() {
        let action = NamedTempFile::new().expect("action payload file");
        action
            .as_file()
            .set_len((MODERATION_NATIVE_ACTION_INPUT_MAX_BYTES_V1 + 1) as u64)
            .expect("extend action payload");
        let action_err = read_moderation_ballot_payload_file(action.path())
            .expect_err("oversized native action input must be rejected");
        assert!(action_err.to_string().contains("between 1 and"));
        let status = NamedTempFile::new().expect("coordination status file");
        status
            .as_file()
            .set_len((MODERATION_COORDINATION_STATUS_MAX_BYTES_V1 + 1) as u64)
            .expect("extend coordination status");
        let status_err = load_moderation_commit_reveal_status_payload(status.path())
            .expect_err("oversized coordination input must be rejected");
        assert!(status_err.to_string().contains("between 1 and"));
    }
    fn moderation_native_juror_actions_reject_caller_timestamps() {
        let ctx = TestContext::new();
        let client = ctx.client_from_config().expect("valid moderation context");
        let juror_id = client.account().to_string();
        let mut commit = moderation_ballot_commit_fixture_for_juror(&juror_id);
        commit.committed_at_unix_ms = 1;
        let commit_err = build_moderation_commit_transaction(&client, &commit)
            .expect_err("caller-supplied commit timestamp must be rejected");
        assert!(commit_err.to_string().contains("must be zero"));
        let mut reveal = moderation_ballot_reveal_fixture_for_juror(&juror_id);
        reveal.revealed_at_unix_ms = 1;
        let reveal_err = build_moderation_reveal_transaction(&client, &reveal)
            .expect_err("caller-supplied reveal timestamp must be rejected");
        assert!(reveal_err.to_string().contains("must be zero"));
    }
    fn moderation_native_juror_actions_require_transaction_authority() {
        let ctx = TestContext::new();
        let client = ctx.client_from_config().expect("valid moderation context");
        let commit = moderation_ballot_commit_fixture_for_juror("other-juror@moderation");
        let commit_err = build_moderation_commit_transaction(&client, &commit)
            .expect_err("substituted commit juror must be rejected");
        assert!(commit_err.to_string().contains("transaction authority"));
        let reveal = moderation_ballot_reveal_fixture_for_juror("other-juror@moderation");
        let reveal_err = build_moderation_reveal_transaction(&client, &reveal)
            .expect_err("substituted reveal juror must be rejected");
        assert!(reveal_err.to_string().contains("transaction authority"));
    }
    fn moderation_ballots_execute_submits_pending_actions_payload_free() {
        let mut ctx = TestContext::new();
        let juror_id = ctx.cfg.account.to_string();
        let commit = moderation_ballot_commit_fixture_for_juror(&juror_id);
        let reveal = moderation_ballot_reveal_fixture_for_juror(&juror_id);
        let mut commit_file = NamedTempFile::new().expect("commit file");
        commit_file
            .write_all(
                norito::json::to_json_pretty(&commit)
                    .expect("render commit json")
                    .as_bytes(),
            )
            .expect("write commit json");
        let mut reveal_file = NamedTempFile::new().expect("reveal file");
        reveal_file
            .write_all(&to_bytes(&reveal).expect("encode reveal"))
            .expect("write reveal norito");
        let status_file =
            write_commit_reveal_status_file(&[juror_id.as_str()], &[juror_id.as_str()], true);
        let args = ModerationBallotsExecuteArgs {
            status: status_file.path().to_path_buf(),
            commit_payloads: vec![commit_file.path().to_path_buf()],
            reveal_payloads: vec![reveal_file.path().to_path_buf()],
            commit_format: "json".to_string(),
            reveal_format: "norito".to_string(),
            submit_tally: true,
        };
        let mut committed = Vec::new();
        let mut revealed = Vec::new();
        let mut tallied = Vec::new();
        args.run_with(
            &mut ctx,
            |_client, transaction| {
                committed.push(moderation_commit_from_transaction(transaction).juror_id);
                Ok(transaction.hash())
            },
            |_client, transaction| {
                revealed.push(moderation_reveal_from_transaction(transaction).juror_id);
                Ok(transaction.hash())
            },
            |_client, transaction| {
                let instruction = moderation_finalization_from_transaction(transaction);
                tallied.push((
                    instruction.case_id().to_string(),
                    instruction.round_id().to_string(),
                ));
                Ok(transaction.hash())
            },
        )
        .expect("execution should succeed");
        assert_eq!(committed, vec![juror_id.clone()]);
        assert_eq!(revealed, vec![juror_id]);
        assert_eq_compact! { tallied => vec![("case-401".to_string(), "round-7".to_string())] };
        assert_eq!(ctx.printed.len(), 1);
        let summary: Value =
            norito::json::from_str(&ctx.printed[0]).expect("execution summary JSON");
        assert_eq_compact! { summary.get("schema").and_then(Value::as_str) => Some("sorafs.moderation.ballots.execution.v1") };
        assert_eq!(summary.get("action_count").and_then(Value::as_u64), Some(3));
        assert_eq_compact! { summary.get("payload_bytes_included").and_then(Value::as_bool) => Some(false) };
        assert_eq_compact! { summary.get("private_payloads_included").and_then(Value::as_bool) => Some(false) };
        assert_compact! { !ctx.printed[0].contains("nonce"); "execution summary must not print reveal payload internals" };
    }
    fn moderation_ballots_execute_rejects_non_pending_commit() {
        let commit = moderation_ballot_commit_fixture_for_juror("juror-1@moderation");
        let mut commit_file = NamedTempFile::new().expect("commit file");
        commit_file
            .write_all(
                norito::json::to_json_pretty(&commit)
                    .expect("render commit json")
                    .as_bytes(),
            )
            .expect("write commit json");
        let status_file = write_commit_reveal_status_file(&["juror-other@moderation"], &[], false);
        let args = ModerationBallotsExecuteArgs {
            status: status_file.path().to_path_buf(),
            commit_payloads: vec![commit_file.path().to_path_buf()],
            reveal_payloads: Vec::new(),
            commit_format: "json".to_string(),
            reveal_format: "json".to_string(),
            submit_tally: false,
        };
        let mut ctx = TestContext::new();
        let err = args
            .run_with(
                &mut ctx,
                |_client, _| unreachable!("commit submit must not run"),
                |_client, _| unreachable!("reveal submit must not run"),
                |_client, _| unreachable!("tally submit must not run"),
            )
            .expect_err("non-pending commit must be rejected");
        assert!(err.to_string().contains("not pending in --status"));
        assert!(ctx.printed.is_empty());
    }
    fn moderation_ballots_executor_bundle_writes_supervised_job_payload_free() {
        let temp = TempDir::new().expect("executor bundle temp dir");
        let bundle_dir = temp.path().join("executor-bundle");
        let status_path = temp.path().join("runtime/commit-reveal-status.json");
        let commit_path = temp.path().join("private/commit.json");
        let reveal_path = temp.path().join("private/reveal.to");
        let args = ModerationBallotsExecutorBundleArgs {
            status: status_path.clone(),
            bundle_out: bundle_dir.clone(),
            commit_payloads: vec![commit_path.clone()],
            reveal_payloads: vec![reveal_path.clone()],
            commit_format: "json".to_string(),
            reveal_format: "norito".to_string(),
            submit_tally: true,
            iroha_bin: "/usr/local/bin/iroha".to_string(),
            service_name: "org.sora.sorafs.ballots-executor-test".to_string(),
            service_user: "sorafs-exec".to_string(),
            service_group: "sorafs-exec".to_string(),
            interval_secs: 30,
        };
        let mut ctx = TestContext::new();
        args.run_with(&mut ctx)
            .expect("executor bundle should be written");
        assert_eq!(ctx.printed.len(), 1);
        let summary: Value =
            norito::json::from_str(&ctx.printed[0]).expect("executor bundle summary JSON");
        assert_eq_compact! { summary.get("schema").and_then(Value::as_str) => Some("sorafs.moderation.ballots.executor_bundle.v1") };
        assert_eq_compact! { summary.get("commit_payload_count").and_then(Value::as_u64) => Some(1) };
        assert_eq_compact! { summary.get("reveal_payload_count").and_then(Value::as_u64) => Some(1) };
        assert_eq_compact! { summary.get("submit_tally").and_then(Value::as_bool) => Some(true) };
        assert_eq_compact! { summary.get("payload_bytes_included").and_then(Value::as_bool) => Some(false) };
        assert_eq_compact! { summary.get("private_payloads_included").and_then(Value::as_bool) => Some(false) };
        assert_eq_compact! { summary.get("private_payload_files_copied").and_then(Value::as_bool) => Some(false) };
        let run_script = fs::read_to_string(bundle_dir.join("run.sh")).expect("read run script");
        assert!(run_script.contains("sorafs moderation ballots execute"));
        assert!(run_script.contains("--submit-tally"));
        assert!(run_script.contains(&format!("--commit-payload='{}'", commit_path.display())));
        assert!(run_script.contains(&format!("--reveal-payload='{}'", reveal_path.display())));
        assert!(!run_script.contains("nonce"));
        let env = fs::read_to_string(bundle_dir.join("executor.env")).expect("read env");
        assert!(env.contains("IROHA_BIN='/usr/local/bin/iroha'"));
        assert_compact! { env.contains(&format!( "SORAFS_BALLOTS_EXECUTOR_STATUS_PATH='{}'", status_path.display() )) };
        assert!(!env.contains("commitment_blake2b_256"));
        let systemd =
            fs::read_to_string(bundle_dir.join("org.sora.sorafs.ballots-executor-test.service"))
                .expect("read systemd unit");
        assert!(systemd.contains("Type=oneshot"));
        assert!(systemd.contains("NoNewPrivileges=true"));
        let timer =
            fs::read_to_string(bundle_dir.join("org.sora.sorafs.ballots-executor-test.timer"))
                .expect("read systemd timer");
        assert!(timer.contains("OnUnitActiveSec=30s"));
        let launchd =
            fs::read_to_string(bundle_dir.join("org.sora.sorafs.ballots-executor-test.plist"))
                .expect("read launchd plist");
        assert!(launchd.contains("<key>StartInterval</key>"));
        assert!(launchd.contains("<integer>30</integer>"));
        let metadata: Value =
            norito::json::from_slice(&fs::read(bundle_dir.join("bundle.json")).expect("metadata"))
                .expect("metadata JSON");
        assert_eq_compact! { metadata.get("schema").and_then(Value::as_str) => Some("sorafs.moderation.ballots.executor_bundle.v1") };
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            let mode = fs::metadata(bundle_dir.join("run.sh"))
                .expect("run script metadata")
                .permissions()
                .mode();
            assert_ne!(mode & 0o111, 0, "run.sh should be executable");
        }
    }
    fn moderation_ballots_executor_bundle_rejects_empty_action_set() {
        let temp = TempDir::new().expect("executor bundle temp dir");
        let args = ModerationBallotsExecutorBundleArgs {
            status: temp.path().join("status.json"),
            bundle_out: temp.path().join("executor-bundle"),
            commit_payloads: Vec::new(),
            reveal_payloads: Vec::new(),
            commit_format: "json".to_string(),
            reveal_format: "json".to_string(),
            submit_tally: false,
            iroha_bin: "iroha".to_string(),
            service_name: "org.sora.sorafs.ballots-executor-test".to_string(),
            service_user: "sorafs-exec".to_string(),
            service_group: "sorafs-exec".to_string(),
            interval_secs: 60,
        };
        let mut ctx = TestContext::new();
        let err = args
            .run_with(&mut ctx)
            .expect_err("empty executor bundle action set must be rejected");
        assert!(err.to_string().contains("at least one --commit-payload"));
        assert!(ctx.printed.is_empty());
        assert_compact! { !temp.path().join("executor-bundle").exists(); "bundle directory must not be created on validation failure" };
    }
    fn moderation_ballots_executor_canary_writes_payload_free_evidence() {
        let temp = TempDir::new().expect("executor canary temp dir");
        let bundle_dir = temp.path().join("executor-bundle");
        let bundle_args = ModerationBallotsExecutorBundleArgs {
            status: temp.path().join("runtime/commit-reveal-status.json"),
            bundle_out: bundle_dir.clone(),
            commit_payloads: vec![temp.path().join("private/commit.json")],
            reveal_payloads: vec![temp.path().join("private/reveal.to")],
            commit_format: "json".to_string(),
            reveal_format: "norito".to_string(),
            submit_tally: true,
            iroha_bin: "/usr/local/bin/iroha".to_string(),
            service_name: "org.sora.sorafs.ballots-executor-test".to_string(),
            service_user: "sorafs-exec".to_string(),
            service_group: "sorafs-exec".to_string(),
            interval_secs: 30,
        };
        let mut setup_ctx = TestContext::new();
        bundle_args
            .run_with(&mut setup_ctx)
            .expect("executor bundle should be written");
        let execution_summary = write_json_file(&norito::json!({
            "schema": "sorafs.moderation.ballots.execution.v1",
            "source": "commit-reveal-status",
            "status": "executed",
            "action_count": 2_u64,
            "commit_action_count": 1_u64,
            "reveal_action_count": 0_u64,
            "tally_action_count": 1_u64,
            "payload_bytes_included": false,
            "private_payloads_included": false,
            "actions": [{
                "action": "commit",
                "case_id": "case-401",
                "round_id": "round-7",
                "juror_id": "juror-1@moderation",
                "transaction_hash_hex": ("ab".repeat(32)),
                "payload_bytes_included": false,
                "private_payloads_included": false
            }, {
                "action": "tally",
                "case_id": "case-401",
                "round_id": "round-7",
                "juror_id": null,
                "transaction_hash_hex": ("cd".repeat(32)),
                "payload_bytes_included": false,
                "private_payloads_included": false
            }]
        }));
        let out = temp.path().join("nested/executor-canary.json");
        let args = ModerationBallotsExecutorCanaryArgs {
            bundle: bundle_dir.clone(),
            execution_summary: Some(execution_summary.path().to_path_buf()),
            out: Some(out.clone()),
        };
        let mut ctx = TestContext::new();
        args.run_with(&mut ctx)
            .expect("executor canary should emit evidence");
        assert_eq!(ctx.printed.len(), 1);
        assert!(out.exists(), "executor canary evidence should be written");
        let evidence: Value =
            norito::json::from_str(&ctx.printed[0]).expect("executor canary evidence JSON");
        assert_eq_compact! { evidence.get("schema").and_then(Value::as_str) => Some("sorafs.moderation.ballots.executor_canary.v1") };
        assert_eq_compact! { evidence.get("status").and_then(Value::as_str) => Some("passed") };
        assert_eq_compact! { evidence.get("artifact_count").and_then(Value::as_u64) => Some(7) };
        assert_eq_compact! { evidence.get("passed_artifact_count").and_then(Value::as_u64) => Some(7) };
        assert_eq_compact! { evidence.get("execution_summary_present").and_then(Value::as_bool) => Some(true) };
        assert_eq_compact! { evidence.get("payload_bytes_included").and_then(Value::as_bool) => Some(false) };
        assert_eq_compact! { evidence.get("private_payloads_included").and_then(Value::as_bool) => Some(false) };
        assert_compact! { !ctx.printed[0].contains("payload_b64"); "canary evidence must not include payload bytes" };
        assert_compact! { !ctx.printed[0].contains("nonce"); "canary evidence must not include reveal internals" };
        let artifacts = evidence
            .get("artifacts")
            .and_then(Value::as_array)
            .expect("artifact probes");
        assert_compact! { artifacts.iter().any(|artifact| artifact.get("kind").and_then(Value::as_str) == Some("run_script")) };
    }
    fn moderation_ballots_executor_canary_rejects_payload_bearing_summary() {
        let temp = TempDir::new().expect("executor canary temp dir");
        let bundle_dir = temp.path().join("executor-bundle");
        let bundle_args = ModerationBallotsExecutorBundleArgs {
            status: temp.path().join("runtime/commit-reveal-status.json"),
            bundle_out: bundle_dir.clone(),
            commit_payloads: vec![temp.path().join("private/commit.json")],
            reveal_payloads: Vec::new(),
            commit_format: "json".to_string(),
            reveal_format: "json".to_string(),
            submit_tally: false,
            iroha_bin: "iroha".to_string(),
            service_name: "org.sora.sorafs.ballots-executor-test".to_string(),
            service_user: "sorafs-exec".to_string(),
            service_group: "sorafs-exec".to_string(),
            interval_secs: 60,
        };
        let mut setup_ctx = TestContext::new();
        bundle_args
            .run_with(&mut setup_ctx)
            .expect("executor bundle should be written");
        let execution_summary = write_json_file(&norito::json!({
            "schema": "sorafs.moderation.ballots.execution.v1",
            "payload_bytes_included": false,
            "private_payloads_included": false,
            "payload_b64": "AAAA",
            "actions": []
        }));
        let args = ModerationBallotsExecutorCanaryArgs {
            bundle: bundle_dir,
            execution_summary: Some(execution_summary.path().to_path_buf()),
            out: None,
        };
        let mut ctx = TestContext::new();
        let err = args
            .run_with(&mut ctx)
            .expect_err("payload-bearing execution summary must be rejected");
        assert!(err.to_string().contains("payload bytes"));
        assert!(ctx.printed.is_empty());
    }
    fn moderation_quarantine_notifications_deliver_writes_outbox_and_webhook_summary() {
        let manifest_file = write_json_file(&juror_notifications_manifest_fixture(false));
        let out_dir = TempDir::new().expect("notification outbox");
        let args = ModerationQuarantineNotificationsDeliverArgs {
            manifest: manifest_file.path().to_path_buf(),
            out_dir: Some(out_dir.path().to_path_buf()),
            webhook_url: Some("https://moderation.example.test/webhook".to_string()),
            timeout_secs: 5,
        };
        let mut ctx = TestContext::new();
        let mut posts = Vec::new();
        args.run_with(&mut ctx, |url, body| {
        assert_eq!(url, "https://moderation.example.test/webhook");
        posts.push(body.to_vec());
        Ok(Response::builder()
            .status(StatusCode::ACCEPTED)
            .header("Content-Type", "application/json")
            .body(br#"{"status":"accepted"}"#.to_vec())
            .unwrap())
    })
        .expect("notification delivery should succeed");
        assert_eq!(posts.len(), 1);
        let posted: Value = norito::json::from_slice(&posts[0]).expect("posted notification JSON");
        assert_eq!(posted["delivery_id"].as_str(), Some("notify-1"));
        let outbox_file = out_dir.path().join("notify-1.json");
        assert!(outbox_file.exists(), "outbox file should be written");
        let outbox_body = fs::read_to_string(outbox_file).expect("read outbox file");
        assert!(!outbox_body.contains("payload_b64"));
        assert_eq!(ctx.printed.len(), 1);
        let summary: Value =
            norito::json::from_str(&ctx.printed[0]).expect("delivery summary JSON");
        assert_eq_compact! { summary["schema"].as_str() => Some("sorafs.moderation.juror_notifications.delivery.v1") };
        assert_eq!(summary["delivery_count"].as_u64(), Some(1));
        assert_eq!(summary["payload_bytes_included"].as_bool(), Some(false));
        assert_eq!(summary["private_payloads_included"].as_bool(), Some(false));
        assert_compact! { !ctx.printed[0].contains("Build the private commit payload locally."); "delivery summary must not repeat notification body text" };
    }
    fn moderation_quarantine_notifications_deliver_rejects_private_payload_flags() {
        let manifest_file = write_json_file(&juror_notifications_manifest_fixture(true));
        let out_dir = TempDir::new().expect("notification outbox");
        let args = ModerationQuarantineNotificationsDeliverArgs {
            manifest: manifest_file.path().to_path_buf(),
            out_dir: Some(out_dir.path().to_path_buf()),
            webhook_url: None,
            timeout_secs: 5,
        };
        let mut ctx = TestContext::new();
        let err = args
            .run_with(&mut ctx, |_url, _body| {
                unreachable!("webhook delivery must not run")
            })
            .expect_err("private payload flag must be rejected");
        assert!(err.to_string().contains("private_payload_included"));
        assert!(ctx.printed.is_empty());
        assert_compact! { fs::read_dir(out_dir.path()).expect("read outbox dir").next().is_none(); "outbox must stay empty on validation failure" };
    }
    fn moderation_quarantine_notifications_canary_writes_payload_free_evidence() {
        let manifest_file = write_json_file(&juror_notifications_manifest_fixture(false));
        let out_dir = TempDir::new().expect("canary evidence dir");
        let out = out_dir.path().join("nested/evidence.json");
        let args = ModerationQuarantineNotificationsCanaryArgs {
            manifest: manifest_file.path().to_path_buf(),
            webhook_url: "https://moderation.example.test/webhook".to_string(),
            out: Some(out.clone()),
            timeout_secs: 5,
        };
        let mut ctx = TestContext::new();
        let mut posts = Vec::new();
        args.run_with(&mut ctx, |url, body| {
        assert_eq!(url, "https://moderation.example.test/webhook");
        posts.push(body.to_vec());
        Ok(Response::builder()
            .status(StatusCode::ACCEPTED)
            .header("Content-Type", "application/json")
            .body(br#"{"status":"accepted"}"#.to_vec())
            .unwrap())
    })
        .expect("canary should succeed");
        assert_eq!(posts.len(), 1);
        assert!(out.exists(), "canary evidence file should be written");
        assert_eq!(ctx.printed.len(), 1);
        let evidence: Value =
            norito::json::from_str(&ctx.printed[0]).expect("canary evidence JSON");
        assert_eq_compact! { evidence["schema"].as_str() => Some("sorafs.moderation.juror_notifications.transport_canary.v1") };
        assert_eq!(evidence["status"].as_str(), Some("passed"));
        assert_eq!(evidence["probe_count"].as_u64(), Some(1));
        assert_eq!(evidence["accepted_count"].as_u64(), Some(1));
        assert_compact! { evidence["manifest_body_blake3_hex"].as_str().is_some(); "canary evidence should expose the typed manifest digest key" };
        assert_compact! { evidence.get("manifest_body_blake3").is_none(); "canary evidence must not emit the ambiguous manifest digest key" };
        assert_eq!(evidence["payload_bytes_included"].as_bool(), Some(false));
        assert_eq!(evidence["private_payloads_included"].as_bool(), Some(false));
        assert_compact! { !ctx.printed[0].contains("Build the private commit payload locally."); "canary evidence must not repeat notification body text" };
    }
    fn moderation_quarantine_notifications_canary_records_failed_probe_without_body() {
        let manifest_file = write_json_file(&juror_notifications_manifest_fixture(false));
        let args = ModerationQuarantineNotificationsCanaryArgs {
            manifest: manifest_file.path().to_path_buf(),
            webhook_url: "https://moderation.example.test/webhook".to_string(),
            out: None,
            timeout_secs: 5,
    };
    let mut ctx = TestContext::new();
    args.run_with(&mut ctx, |_url, _body| {
        Ok(Response::builder()
            .status(StatusCode::BAD_GATEWAY)
            .header("Content-Type", "application/json")
            .body(br#"{"error":"transport unavailable"}"#.to_vec())
            .unwrap())
    })
        .expect("canary should emit failed evidence instead of hiding probe failure");
        let evidence: Value =
            norito::json::from_str(&ctx.printed[0]).expect("canary evidence JSON");
        assert_eq!(evidence["status"].as_str(), Some("failed"));
        assert_eq!(evidence["accepted_count"].as_u64(), Some(0));
        assert_compact! { !ctx.printed[0].contains("transport unavailable"); "canary evidence must hash response bodies instead of archiving them" };
    }
    fn moderation_quarantine_notifications_run_does_not_follow_cross_origin_redirects() {
        for canary in [false, true] {
            let manifest = write_json_file(&juror_notifications_manifest_fixture(false));
            let origin = TcpListener::bind("127.0.0.1:0").expect("bind redirect origin");
            let origin_addr = origin.local_addr().expect("redirect origin address");
            let target = TcpListener::bind("127.0.0.1:0").expect("bind redirect target");
            let target_addr = target.local_addr().expect("redirect target address");
            let server = thread::spawn(move || {
                let (mut stream, _) = origin.accept().expect("accept webhook request");
                let mut request = [0_u8; 8192];
                let len = stream.read(&mut request).expect("read webhook request");
                assert!(request[..len].starts_with(b"POST "));
                write!(
                    stream,
                    "HTTP/1.1 307 Temporary Redirect\r\nLocation: http://{target_addr}/stolen\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                )
                .expect("write redirect response");
            });
            let webhook_url = format!("http://{origin_addr}/webhook");
            let mut ctx = TestContext::new();
            if canary {
                ModerationQuarantineNotificationsCanaryArgs {
                    manifest: manifest.path().to_path_buf(),
                    webhook_url,
                    out: None,
                    timeout_secs: 1,
                }
                .run(&mut ctx)
                .expect("redirect must produce failed canary evidence");
                let evidence: Value =
                    norito::json::from_str(&ctx.printed[0]).expect("redirect canary evidence JSON");
                assert_eq!(evidence["status"].as_str(), Some("failed"));
                assert_eq!(evidence["accepted_count"].as_u64(), Some(0));
                assert_eq!(evidence["probes"][0]["response_status"].as_u64(), Some(307));
            } else {
                let err = ModerationQuarantineNotificationsDeliverArgs {
                    manifest: manifest.path().to_path_buf(),
                    out_dir: None,
                    webhook_url: Some(webhook_url),
                    timeout_secs: 1,
                }
                .run(&mut ctx)
                .expect_err("redirected delivery must fail");
                assert!(err.to_string().contains("status 307"));
                assert!(ctx.printed.is_empty());
            }
            server.join().expect("redirect server finished");
            target
                .set_nonblocking(true)
                .expect("set target nonblocking");
            assert_compact! { matches!(target.accept(), Err(error) if error.kind() == io::ErrorKind::WouldBlock); "cross-origin redirect target must receive no connection" };
        }
    }
    fn sorafs_get_canary_runs_do_not_follow_cross_origin_redirects() {
        for command in 0_u8..3 {
            let origin = TcpListener::bind("127.0.0.1:0").expect("bind redirect origin");
            let origin_addr = origin.local_addr().expect("redirect origin address");
            let target = TcpListener::bind("127.0.0.1:0").expect("bind redirect target");
            let target_addr = target.local_addr().expect("redirect target address");
            let server = thread::spawn(move || {
                let (mut stream, _) = origin.accept().expect("accept GET canary request");
                let mut request = [0_u8; 8192];
                let len = stream.read(&mut request).expect("read GET canary request");
                assert!(request[..len].starts_with(b"GET "));
                write!(
                    stream,
                    "HTTP/1.1 307 Temporary Redirect\r\nLocation: http://{target_addr}/substitute\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                )
                .expect("write redirect response");
            });
            let base_url = format!("http://{origin_addr}/root");
            let mut ctx = TestContext::new();
            match command {
                0 => {
                    let error = TransparencyExplorerCanaryArgs {
                        torii_url: Some(base_url),
                        limit: None,
                        timeout_secs: 1,
                        out: None,
                    }
                    .run(&mut ctx)
                    .expect_err("explorer redirect must fail");
                    assert!(error.to_string().contains("307"));
                    assert!(ctx.printed.is_empty());
                }
                1 => {
                    TransparencyPublicationCanaryArgs {
                        torii_url: Some(base_url),
                        cycle_ids: Vec::new(),
                        limit: None,
                        timeout_secs: 1,
                        out: None,
                    }
                    .run(&mut ctx)
                    .expect("publication redirect must emit failed evidence");
                    let evidence: Value = norito::json::from_str(&ctx.printed[0])
                        .expect("publication redirect evidence JSON");
                    assert_eq!(evidence["status"].as_str(), Some("failed"));
                    assert_eq!(evidence["routes"][0]["status_code"].as_u64(), Some(307));
                    assert_eq!(evidence["routes"][0]["passed"].as_bool(), Some(false));
                }
                2 => {
                    let error = ModerationQuarantineOperatorCanaryArgs {
                        operator_url: base_url,
                        quarantine_id: "ba".repeat(16),
                        limit: None,
                        timeout_secs: 1,
                        out: None,
                    }
                    .run(&mut ctx)
                    .expect_err("operator redirect must fail");
                    assert!(error.to_string().contains("307"));
                    assert!(ctx.printed.is_empty());
                }
                _ => unreachable!("bounded GET canary selector"),
            }
            server.join().expect("redirect server finished");
            target.set_nonblocking(true).expect("nonblocking target");
            assert_compact! { matches!(target.accept(), Err(error) if error.kind() == io::ErrorKind::WouldBlock); "cross-origin redirect target must receive no GET connection" };
        }
    }
    fn moderation_registry_list_with_prints_payload() {
        let args = ModerationRegistryListArgs { limit: Some(5) };
        let mut ctx = TestContext::new();
        args.run_with(&mut ctx, |_client, filter| {
            assert_eq!(filter.limit, Some(5));
json_response_fixture!(StatusCode::OK, &norito::json!({
                    "repro_manifests": [
                        { "manifest_id_hex": "aa", "model_count": 1 }
                    ],
                    "adversarial_corpora": []
                }))
        })
        .expect("run should succeed");
        assert_eq!(ctx.printed.len(), 1);
        assert!(ctx.printed[0].contains("\"repro_manifests\""));
    }
    fn moderation_registry_submit_repro_reads_json_manifest() {
        let manifest = signed_moderation_repro_manifest_fixture();
        let expected_bytes = to_bytes(&manifest).expect("encode canonical repro manifest");
        let mut file = NamedTempFile::new().expect("repro manifest file");
        file.write_all(
            norito::json::to_json_pretty(&manifest)
                .expect("render repro json")
                .as_bytes(),
        )
        .expect("write repro json");
        let args = ModerationRegistrySubmitReproArgs {
            manifest: file.path().to_path_buf(),
            format: "json".to_string(),
        };
        let mut ctx = TestContext::new();
        args.run_with(&mut ctx, |_client, manifest_bytes| {
            assert_eq!(manifest_bytes, expected_bytes.as_slice());
json_response_fixture!(StatusCode::ACCEPTED,
                    &norito::json!({ "status": "admitted" }),
                )
        })
        .expect("run should succeed");
        assert_eq!(ctx.printed.len(), 1);
        assert!(ctx.printed[0].contains("\"admitted\""));
    }
    fn moderation_registry_submit_corpus_reads_norito_manifest() {
        let manifest = adversarial_corpus_manifest_fixture();
        let expected_bytes = to_bytes(&manifest).expect("encode canonical corpus manifest");
        let mut file = NamedTempFile::new().expect("corpus manifest file");
        file.write_all(&expected_bytes)
            .expect("write corpus norito");
        let args = ModerationRegistrySubmitCorpusArgs {
            manifest: file.path().to_path_buf(),
            format: "norito".to_string(),
        };
        let mut ctx = TestContext::new();
        args.run_with(&mut ctx, |_client, manifest_bytes| {
            assert_eq!(manifest_bytes, expected_bytes.as_slice());
json_response_fixture!(StatusCode::ACCEPTED,
                    &norito::json!({ "status": "admitted" }),
                )
        })
        .expect("run should succeed");
        assert_eq!(ctx.printed.len(), 1);
        assert!(ctx.printed[0].contains("\"admitted\""));
    }
    fn moderation_registry_submit_repro_rejects_invalid_format() {
        let file = NamedTempFile::new().expect("manifest file");
        let args = ModerationRegistrySubmitReproArgs {
            manifest: file.path().to_path_buf(),
            format: "yaml".to_string(),
        };
        let mut ctx = TestContext::new();
        let err = args
            .run_with(&mut ctx, |_client, _| unreachable!("submit must not run"))
            .expect_err("invalid format must be rejected");
        assert!(err.to_string().contains("--format"));
        assert!(ctx.printed.is_empty());
    }
    fn moderation_screening_list_with_prints_payload() {
        let args = ModerationScreeningListArgs { limit: Some(6) };
        let mut ctx = TestContext::new();
        args.run_with(&mut ctx, |_client, filter| {
            assert_eq!(filter.limit, Some(6));
json_response_fixture!(StatusCode::OK, &norito::json!({
                    "screening_records": [
                        { "record_id_hex": "aa", "verdict": "quarantine" }
                    ]
                }))
        })
        .expect("run should succeed");
        assert_eq!(ctx.printed.len(), 1);
        assert!(ctx.printed[0].contains("\"screening_records\""));
    }
    fn moderation_screening_submit_reads_authenticated_authority_json() {
        let idempotency_key = [0xA1_u8; 32];
        let expected_idempotency_key = encode(idempotency_key);
        let uppercase_idempotency_key =
            format!("0x{}", encode(idempotency_key).to_ascii_uppercase());
        let authority_b64 = STANDARD.encode(b"canonical committee aggregate");
        let member_one_b64 = STANDARD.encode(b"canonical signed member one");
        let member_two_b64 = STANDARD.encode(b"canonical signed member two");
        let mut file = NamedTempFile::new().expect("screening result file");
        let mut screening_result = Map::new();
        screening_result.insert(
            "idempotency_key_hex".to_owned(),
            Value::String(uppercase_idempotency_key),
        );
        screening_result.insert(
            "evidence_kind".to_owned(),
            Value::String("committee_aggregate".to_owned()),
        );
        screening_result.insert(
            "authority_b64".to_owned(),
            Value::String(authority_b64.clone()),
        );
        screening_result.insert(
            "committee_member_results_b64".to_owned(),
            Value::Array(vec![
                Value::String(member_one_b64.clone()),
                Value::String(member_two_b64.clone()),
            ]),
        );
        file.write_all(
            &norito::json::to_vec(&Value::Object(screening_result))
                .expect("serialize screening JSON"),
        )
        .expect("write screening JSON");
        let args = ModerationScreeningSubmitArgs {
            input: file.path().to_path_buf(),
        };
        let mut ctx = TestContext::new();
        args.run_with(&mut ctx, |_client, request| {
            assert_eq!(request.idempotency_key_hex, expected_idempotency_key);
            assert_eq!(request.evidence_kind, "committee_aggregate");
            assert_eq!(request.authority_b64, authority_b64);
            assert_eq_compact! { request.committee_member_results_b64 =>[member_one_b64.clone(), member_two_b64.clone()] };
json_response_fixture!(StatusCode::ACCEPTED,
                    &norito::json!({ "status": "accepted" }),
                )
        })
        .expect("run should succeed");
        assert_eq!(ctx.printed.len(), 1);
        assert!(ctx.printed[0].contains("\"accepted\""));
    }
    fn moderation_screening_submit_rejects_missing_field() {
        let mut file = NamedTempFile::new().expect("screening result file");
        file.write_all(
            &norito::json::to_vec(&norito::json!({
                "idempotency_key_hex": (encode([0x11_u8; 32])),
                "evidence_kind": "signed_result",
                "committee_member_results_b64": [],
            }))
            .expect("serialize screening JSON"),
        )
        .expect("write screening JSON");
        let args = ModerationScreeningSubmitArgs {
            input: file.path().to_path_buf(),
        };
        let mut ctx = TestContext::new();
        let err = args
            .run_with(&mut ctx, |_client, _| unreachable!("submit must not run"))
            .expect_err("missing authority must be rejected");
        assert!(err.to_string().contains("authority_b64"));
        assert!(ctx.printed.is_empty());
    }
    fn moderation_quarantine_list_with_prints_payload() {
        let args = ModerationQuarantineListArgs { limit: Some(4) };
        let mut ctx = TestContext::new();
        args.run_with(&mut ctx, |_client, filter| {
            assert_eq!(filter.limit, Some(4));
json_response_fixture!(StatusCode::OK, &norito::json!({
                    "quarantine_records": [
                        { "quarantine_id_hex": "aa", "state": "pending_review" }
                    ]
                }))
        })
        .expect("run should succeed");
        assert_eq!(ctx.printed.len(), 1);
        assert!(ctx.printed[0].contains("\"quarantine_records\""));
    }
    fn moderation_quarantine_object_store_reads_payload_file() {
        let quarantine_id = [0xA7_u8; 16];
        let mut file = NamedTempFile::new().expect("temp payload");
        file.write_all(b"quarantine payload bytes")
            .expect("write payload");
        let args = ModerationQuarantineObjectStoreArgs {
            quarantine_id: format!("0x{}", encode(quarantine_id).to_ascii_uppercase()),
            payload_file: file.path().to_path_buf(),
            captured_at: Some("@1800000310".to_string()),
            content_type: Some(" application/octet-stream ".to_string()),
            notes: Some(" sealed via cli ".to_string()),
        };
        let mut ctx = TestContext::new();
        args.run_with(&mut ctx, |_client, id, request| {
            assert_eq!(id, encode(quarantine_id));
            assert_eq!(request.payload, b"quarantine payload bytes");
            assert_eq!(request.captured_at_unix, Some(1_800_000_310));
            assert_eq!(request.content_type, Some("application/octet-stream"));
            assert_eq!(request.notes, Some("sealed via cli"));
json_response_fixture!(StatusCode::ACCEPTED,
                    &norito::json!({ "status": "stored" }),
                )
        })
        .expect("run should succeed");
        assert_eq!(ctx.printed.len(), 1);
        assert!(ctx.printed[0].contains("\"stored\""));
    }
    fn moderation_quarantine_object_read_prints_payload_json() {
        let quarantine_id = [0xB8_u8; 16];
        let args = ModerationQuarantineObjectReadArgs {
            quarantine_id: encode(quarantine_id),
        };
        let mut ctx = TestContext::new();
        args.run_with(&mut ctx, |_client, id| {
            assert_eq!(id, encode(quarantine_id));
json_response_fixture!(StatusCode::OK, &norito::json!({
                    "status": "read",
                    "payload_b64": "cGF5bG9hZA=="
                }))
        })
        .expect("run should succeed");
        assert_eq!(ctx.printed.len(), 1);
        assert!(ctx.printed[0].contains("\"payload_b64\""));
    }
    fn moderation_quarantine_object_store_rejects_empty_payload_file() {
        let file = NamedTempFile::new().expect("empty payload");
        let args = ModerationQuarantineObjectStoreArgs {
            quarantine_id: encode([0xC9_u8; 16]),
            payload_file: file.path().to_path_buf(),
            captured_at: Some("@1800000310".to_string()),
            content_type: None,
            notes: None,
        };
        let mut ctx = TestContext::new();
        let err = args
            .run_with(&mut ctx, |_client, _, _| {
                unreachable!("submit must not run")
            })
            .expect_err("empty payload must be rejected");
        assert!(err.to_string().contains("--payload-file"));
        assert!(ctx.printed.is_empty());
    }
    fn moderation_quarantine_review_builds_request() {
        let quarantine_id = [0xAB_u8; 16];
        let args = ModerationQuarantineReviewArgs {
            quarantine_id: format!("0x{}", encode(quarantine_id).to_ascii_uppercase()),
            reviewed_by: Some(" operator@moderation ".to_string()),
            reviewed_at: Some("@1800000210".to_string()),
            notes: Some(" reviewed locally ".to_string()),
        };
        let mut ctx = TestContext::new();
        args.run_with(&mut ctx, |_client, id, request| {
            assert_eq!(id, encode(quarantine_id));
            assert_eq!(request.reviewed_by, "operator@moderation");
            assert_eq!(request.reviewed_at_unix, Some(1_800_000_210));
            assert_eq!(request.notes, Some("reviewed locally"));
json_response_fixture!(StatusCode::ACCEPTED,
                    &norito::json!({ "state": "reviewed" }),
                )
        })
        .expect("run should succeed");
        assert_eq!(ctx.printed.len(), 1);
        assert!(ctx.printed[0].contains("\"reviewed\""));
    }
    fn moderation_quarantine_release_defaults_authority_to_cli_account() {
        let quarantine_id = [0xCD_u8; 16];
        let args = ModerationQuarantineReleaseArgs {
            quarantine_id: encode(quarantine_id),
            release_authority: None,
            released_at: Some("@1800000220".to_string()),
            notes: None,
        };
        let mut ctx = TestContext::new();
        let expected_authority = ctx.config().account.to_string();
        args.run_with(&mut ctx, |_client, id, request| {
            assert_eq!(id, encode(quarantine_id));
            assert_eq!(request.release_authority, expected_authority);
            assert_eq!(request.released_at_unix, Some(1_800_000_220));
            assert_eq!(request.notes, None);
json_response_fixture!(StatusCode::ACCEPTED,
                    &norito::json!({ "state": "released" }),
                )
        })
        .expect("run should succeed");
        assert_eq!(ctx.printed.len(), 1);
        assert!(ctx.printed[0].contains("\"released\""));
    }
    fn moderation_quarantine_appeal_handoff_reads_json_payload() {
        let quarantine_id = [0xA4_u8; 16];
        let input = write_json_file(&norito::json!({
            "class": "content",
            "backlog": 2_u64,
            "evidence_size_mb": 8_u64,
            "payer_account": "payer",
            "destination_account": "treasury",
            "asset_definition_id": "xor#wonderland"
        }));
        let args = ModerationQuarantineAppealHandoffArgs {
            quarantine_id: format!("0x{}", encode(quarantine_id).to_ascii_uppercase()),
            input: input.path().to_path_buf(),
        };
        let mut ctx = TestContext::new();
        args.run_with(&mut ctx, |_client, id, payload| {
            assert_eq!(id, encode(quarantine_id));
            let value: Value = norito::json::from_slice(payload)?;
            assert_eq!(value.get("class").and_then(Value::as_str), Some("content"));
json_response_fixture!(StatusCode::OK, &norito::json!({
                    "schema": "sorafs.moderation.quarantine.appeal_handoff.v1",
                    "status": "ready_for_deposit"
                }))
        })
        .expect("run should succeed");
        assert_eq!(ctx.printed.len(), 1);
        assert!(ctx.printed[0].contains("ready_for_deposit"));
    }
    fn moderation_quarantine_appeal_handoff_rejects_empty_payload() {
        let input = NamedTempFile::new().expect("empty appeal handoff payload");
        let args = ModerationQuarantineAppealHandoffArgs {
            quarantine_id: encode([0xA5_u8; 16]),
            input: input.path().to_path_buf(),
        };
        let mut ctx = TestContext::new();
        let err = args
            .run_with(&mut ctx, |_client, _, _| {
                unreachable!("submit must not run")
            })
            .expect_err("empty appeal handoff payload must be rejected");
        assert_compact! { err.to_string().contains("moderation quarantine appeal handoff payload") };
        assert!(ctx.printed.is_empty());
    }
    fn moderation_quarantine_operator_panel_reads_workflow_view() {
        let quarantine_id = [0xA8_u8; 16];
        let args = ModerationQuarantineOperatorPanelArgs {
            quarantine_id: format!("0x{}", encode(quarantine_id).to_ascii_uppercase()),
            limit: Some(4),
        };
        let mut ctx = TestContext::new();
        args.run_with(&mut ctx, |_client, id, filter| {
            assert_eq!(id, encode(quarantine_id));
            assert_eq!(filter.limit, Some(4));
json_response_fixture!(StatusCode::OK, &norito::json!({
                    "schema": "sorafs.moderation.quarantine.operator_panel.v1",
                    "status": "ready"
                }))
        })
        .expect("run should succeed");
        assert_eq!(ctx.printed.len(), 1);
        assert!(ctx.printed[0].contains("operator_panel"));
    }
    fn moderation_quarantine_operator_panel_rejects_bad_quarantine_id() {
        let args = ModerationQuarantineOperatorPanelArgs {
            quarantine_id: "abcd".to_owned(),
            limit: None,
        };
        let mut ctx = TestContext::new();
        let err = args
            .run_with(&mut ctx, |_client, _, _| unreachable!("get must not run"))
            .expect_err("bad quarantine id must be rejected");
        assert!(err.to_string().contains("--quarantine-id"));
        assert!(ctx.printed.is_empty());
    }
    fn moderation_quarantine_bridge_plan_derives_workflow_actions() {
        let quarantine_id = [0xA9_u8; 16];
        let args = ModerationQuarantineBridgePlanArgs {
            quarantine_id: format!("0x{}", encode(quarantine_id).to_ascii_uppercase()),
            limit: Some(5),
        };
        let mut ctx = TestContext::new();
        args.run_with(&mut ctx, |_client, id, filter| {
            assert_eq!(id, encode(quarantine_id));
            assert_eq!(filter.limit, Some(5));
json_response_fixture!(StatusCode::OK, &norito::json!({
                    "schema": "sorafs.moderation.quarantine.operator_panel.v1",
                    "status": "ready",
                    "record": {
                        "quarantine_id_hex": (encode(quarantine_id)),
                        "state": "reviewed"
                    },
                    "object_status": "stored",
                    "case_count": 1_u64,
                    "returned_case_count": 1_u64,
                    "cases": [(fixture_finalized_moderation_case(&[], &[], &[]))],
                    "operator_routes": {
                        "object": "/v1/sorafs/moderation/quarantine/object"
                    },
                    "next_actions": [
                        {
                            "action": "read_object",
                            "route": "/v1/sorafs/moderation/quarantine/object",
                            "required": false
                        },
                        {
                            "action": "submit_native_case_actions",
                            "route": "/v1/sorafs/moderation/ballots",
                            "required": true
                        }
                    ]
                }))
        })
        .expect("bridge plan should render");
        assert_eq!(ctx.printed.len(), 1);
        let value: Value = norito::json::from_str(&ctx.printed[0]).expect("bridge plan json");
        assert_eq_compact! { value.get("schema").and_then(Value::as_str) => Some("sorafs.moderation.quarantine.bridge_plan.v1") };
        assert_eq_compact! { value.get("payload_bytes_included").and_then(Value::as_bool) => Some(false) };
        let actions = value
            .get("actions")
            .and_then(Value::as_array)
            .expect("actions");
        assert_eq!(actions.len(), 2);
        assert_eq_compact! { actions[1].get("automation_status").and_then(Value::as_str) => Some("waiting_for_native_commit_reveal_finalization") };
        let cli = actions[1]
            .get("cli")
            .and_then(Value::as_array)
            .expect("cli");
        assert_compact! { cli.iter().any(|part| part.as_str() == Some("quarantine-case")) };
        assert!(!ctx.printed[0].contains("payload_b64"));
    }
    fn moderation_quarantine_bridge_plan_rejects_payload_bytes() {
        let quarantine_id = [0xAA_u8; 16];
        let args = ModerationQuarantineBridgePlanArgs {
            quarantine_id: encode(quarantine_id),
            limit: None,
        };
        let mut ctx = TestContext::new();
        let err = args
            .run_with(&mut ctx, |_client, _id, _filter| {
json_response_fixture!(StatusCode::OK, &norito::json!({
                        "schema": "sorafs.moderation.quarantine.operator_panel.v1",
                        "record": {
                            "state": "reviewed"
                        },
                        "object_status": "stored",
                        "payload_b64": "c2hvdWxkLW5vdC1iZS1oZXJl",
                        "next_actions": []
                    }))
            })
            .expect_err("payload bytes must be rejected");
        assert!(err.to_string().contains("payload bytes"));
        assert!(ctx.printed.is_empty());
    }
    fn moderation_quarantine_bridge_plan_rejects_bad_quarantine_id() {
        let args = ModerationQuarantineBridgePlanArgs {
            quarantine_id: "abcd".to_owned(),
            limit: None,
        };
        let mut ctx = TestContext::new();
        let err = args
            .run_with(&mut ctx, |_client, _, _| unreachable!("get must not run"))
            .expect_err("bad quarantine id must be rejected");
        assert!(err.to_string().contains("--quarantine-id"));
        assert!(ctx.printed.is_empty());
    }
    }
fn moderation_operator_canary_fixture_json(
    value: Value,
) -> Result<ModerationOperatorCanaryHttpResponse> {
    Ok(ModerationOperatorCanaryHttpResponse {
        status: StatusCode::OK,
        content_type: Some("application/json".to_string()),
        body: norito::json::to_vec(&value)?,
    })
}
fn moderation_operator_canary_fixture_response(
    url: &str,
    quarantine_id_hex: &str,
    include_payload_b64: bool,
    bridge_schema: &str,
) -> Result<ModerationOperatorCanaryHttpResponse> {
    let parsed = Url::parse(url).expect("canary URL should parse");
    let path = parsed.path();
    if path.contains("/quarantine/") {
        assert_compact! { path.contains(&format!("/quarantine/{quarantine_id_hex}/")); "unexpected canary quarantine route: {path}" };
    }
    if path.ends_with("/healthz") || path.ends_with("/v1/sorafs/moderation/operator-panel/status") {
        return moderation_operator_canary_fixture_json(norito::json!({
            "schema": "sorafs.moderation.quarantine.operator_service.status.v1",
            "status": "ready"
        }));
    }
    if path.ends_with("/v1/sorafs/moderation/operator-panel/ui") {
        return Ok(ModerationOperatorCanaryHttpResponse {
            status: StatusCode::OK,
            content_type: Some("text/html; charset=utf-8".to_string()),
            body: b"<main><h1>SoraFS Moderation Operator</h1></main>".to_vec(),
        });
    }
    if path.ends_with("/operator-panel") {
        let value = if include_payload_b64 {
            norito::json!({
                "schema": "sorafs.moderation.quarantine.operator_panel.v1",
                "status": "ready",
                "payload_b64": "c2hvdWxkLW5vdC1iZS1oZXJl",
                "payload_bytes_included": false,
                "record": {
                    "quarantine_id_hex": (quarantine_id_hex),
                    "state": "reviewed"
                },
                "object_status": "stored",
                "next_actions": []
            })
        } else {
            norito::json!({
                "schema": "sorafs.moderation.quarantine.operator_panel.v1",
                "status": "ready",
                "payload_bytes_included": false,
                "record": {
                    "quarantine_id_hex": (quarantine_id_hex),
                    "state": "reviewed"
                },
                "object_status": "stored",
                "next_actions": []
            })
        };
        return moderation_operator_canary_fixture_json(value);
    }
    if path.ends_with("/bridge-plan") {
        return moderation_operator_canary_fixture_json(norito::json!({
            "schema": (bridge_schema),
            "payload_bytes_included": false,
            "private_payloads_included": false,
            "actions": []
        }));
    }
    if path.ends_with("/juror-plan") {
        return moderation_operator_canary_fixture_json(norito::json!({
            "schema": "sorafs.moderation.quarantine.juror_plan.v1",
            "payload_bytes_included": false,
            "private_payloads_included": false,
            "ballots": []
        }));
    }
    if path.ends_with("/juror-notifications") {
        return moderation_operator_canary_fixture_json(norito::json!({
            "schema": "sorafs.moderation.quarantine.juror_notifications.v1",
            "payload_bytes_included": false,
            "private_payloads_included": false,
            "notifications": []
        }));
    }
    if path.ends_with("/commit-reveal-status") {
        return moderation_operator_canary_fixture_json(norito::json!({
            "schema": "sorafs.moderation.quarantine.commit_reveal_status.v1",
            "payload_bytes_included": false,
            "private_payloads_included": false,
            "ballots": []
        }));
    }
    panic!("unexpected canary route: {url}");
}
test_items! {
fn moderation_quarantine_operator_canary_builds_payload_free_evidence() {
    let quarantine_id = [0xBA_u8; 16];
    let quarantine_id_hex = encode(quarantine_id);
    let out_dir = TempDir::new().expect("canary evidence dir");
    let out = out_dir.path().join("nested/evidence.json");
    let args = ModerationQuarantineOperatorCanaryArgs {
        operator_url: " https://operator.test/root ".to_string(),
        quarantine_id: format!("0x{}", quarantine_id_hex.to_ascii_uppercase()),
        limit: Some(4),
        timeout_secs: 1,
        out: Some(out.clone()),
    };
    let mut ctx = TestContext::new();
    let mut requested = Vec::new();
    args.run_with_fetch(&mut ctx, |url| {
        requested.push(url.to_string());
        moderation_operator_canary_fixture_response(
            url,
            &quarantine_id_hex,
            false,
            "sorafs.moderation.quarantine.bridge_plan.v1",
        )
    })
    .expect("operator canary should render evidence");
    assert_eq!(requested.len(), 8);
    assert_eq!(ctx.printed.len(), 1);
    assert!(!ctx.printed[0].contains("payload_b64"));
    let value: Value = norito::json::from_str(&ctx.printed[0]).expect("canary evidence JSON");
    let schema = value["schema"].as_str();
    assert_eq_compact! { schema => Some("sorafs.moderation.quarantine.operator_canary.v1") };
    assert_eq!(value["status"].as_str(), Some("passed"));
    assert_eq!(value["limit"].as_u64(), Some(4));
    assert_eq!(value["route_count"].as_u64(), Some(8));
    assert_eq!(value["payload_bytes_included"].as_bool(), Some(false));
    let routes = value["routes"].as_array().expect("canary routes");
    assert_eq!(routes.len(), 8);
    let has_commit_reveal = routes
        .iter()
        .any(|route| route["name"].as_str() == Some("commit_reveal_status"));
    assert!(has_commit_reveal);
    let operator_panel = routes
        .iter()
        .find(|route| route["name"].as_str() == Some("operator_panel"))
        .expect("operator-panel route evidence");
    let operator_panel_url = operator_panel["url"].as_str().expect("operator-panel URL");
    assert!(operator_panel_url.contains("/root/v1/sorafs/moderation/quarantine/"));
    assert!(operator_panel_url.contains("limit=4"));
    assert_compact! { routes.iter().all(|route| { route.get("payload_bytes_included").and_then(Value::as_bool) == Some(false) }) };
    let bytes = fs::read(out).expect("written canary evidence");
    let written: Value = norito::json::from_slice(&bytes).expect("written evidence JSON");
    assert_eq!(written["schema"], value["schema"]);
}
fn moderation_quarantine_operator_canary_rejects_payload_bytes() {
    let quarantine_id = [0xBB_u8; 16];
    let quarantine_id_hex = encode(quarantine_id);
    let args = ModerationQuarantineOperatorCanaryArgs {
        operator_url: "https://operator.test/root".to_string(),
        quarantine_id: quarantine_id_hex.clone(),
        limit: Some(4),
        timeout_secs: 1,
        out: None,
    };
    let mut ctx = TestContext::new();
    let err = args
        .run_with_fetch(&mut ctx, |url| {
            moderation_operator_canary_fixture_response(
                url,
                &quarantine_id_hex,
                true,
                "sorafs.moderation.quarantine.bridge_plan.v1",
            )
        })
        .expect_err("operator canary must reject payload bytes");
    assert!(err.to_string().contains("payload bytes"));
    assert!(ctx.printed.is_empty());
}
fn moderation_quarantine_operator_canary_rejects_schema_drift() {
    let quarantine_id = [0xBC_u8; 16];
    let quarantine_id_hex = encode(quarantine_id);
    let args = ModerationQuarantineOperatorCanaryArgs {
        operator_url: "https://operator.test/root".to_string(),
        quarantine_id: quarantine_id_hex.clone(),
        limit: None,
        timeout_secs: 1,
        out: None,
    };
    let mut ctx = TestContext::new();
    let err = args
        .run_with_fetch(&mut ctx, |url| {
            moderation_operator_canary_fixture_response(
                url,
                &quarantine_id_hex,
                false,
                "sorafs.moderation.quarantine.unexpected_bridge_plan.v1",
            )
        })
        .expect_err("operator canary must reject schema drift");
    assert!(err.to_string().contains("schema"));
    assert!(ctx.printed.is_empty());
}
}
struct FixtureModerationOperatorPanelSource {
    expected_quarantine_id_hex: String,
    expected_limit: Option<u32>,
    status: StatusCode,
    body: Vec<u8>,
}
impl ModerationOperatorWorkflowSource for FixtureModerationOperatorPanelSource {
    fn get_operator_panel(
        &self,
        quarantine_id_hex: &str,
        filter: SorafsModerationQuarantineFilter,
    ) -> Result<Response<Vec<u8>>> {
        assert_eq!(quarantine_id_hex, self.expected_quarantine_id_hex);
        assert_eq!(filter.limit, self.expected_limit);
        Ok(Response::builder()
            .status(self.status)
            .header("Content-Type", "application/json")
            .body(self.body.clone())
            .unwrap())
    }
}
macro_rules! moderation_operator_panel_fixture {
    ($quarantine_id_hex:ident; $($extra:tt)*) => {
        norito::json!({
            "schema": "sorafs.moderation.quarantine.operator_panel.v1",
            "status": "ready",
            "record": {
                "quarantine_id_hex": ($quarantine_id_hex.clone()),
                "state": "reviewed"
            },
            "object_status": "stored",
            $($extra)*
        })
    };
}
fn fixture_moderation_operator_service(
    quarantine_id: [u8; 16],
    expected_limit: Option<u32>,
    body: Value,
) -> ModerationOperatorService {
    let args = ModerationQuarantineOperatorServeArgs {
        listen: "127.0.0.1:0".to_string(),
        limit: expected_limit,
        max_body_bytes: 1024,
    };
    args.service(
        Arc::new(FixtureModerationOperatorPanelSource {
            expected_quarantine_id_hex: encode(quarantine_id),
            expected_limit,
            status: StatusCode::OK,
            body: norito::json::to_vec(&body).expect("fixture operator-panel JSON"),
        }),
        "http://torii.test/".to_string(),
        "operator@moderation".to_string(),
    )
    .expect("operator service")
}
fn fixture_finalized_moderation_case(
    jurors: &[&str],
    committed_jurors: &[&str],
    revealed_jurors: &[&str],
) -> Value {
    let jurors = jurors.iter().copied().map(Value::from).collect::<Vec<_>>();
    let commits = committed_jurors
        .iter()
        .map(|juror| norito::json!({ "juror": (*juror) }))
        .collect::<Vec<_>>();
    let reveals = revealed_jurors
        .iter()
        .map(|juror| norito::json!({ "juror": (*juror) }))
        .collect::<Vec<_>>();
    norito::json!({
        "case": {
            "spec": {
                "context": {
                    "case_id": "quarantine-case",
                    "evidence_uri": "sorafs://moderation/quarantine"
                },
                "round_id": "round-7",
                "jurors": (jurors),
                "quorum": 2_u64,
                "commit_deadline_unix_ms": 1_800_000_200_000_u64,
                "challenge_submission_deadline_unix_ms": 1_800_000_300_000_u64,
                "challenge_resolution_deadline_unix_ms": 1_800_086_700_000_u64,
                "reveal_deadline_unix_ms": 1_800_086_800_000_u64
            },
            "opened_at_unix_ms": 1_800_000_100_000_u64
        },
        "commits": (commits),
        "reveals": (reveals),
        "challenges": [],
        "outcome": null,
        "no_shows": []
    })
}
fn fixture_payload_bearing_finalized_moderation_case() -> Value {
    let mut case = fixture_finalized_moderation_case(&["juror-a@moderation"], &[], &[]);
    case.as_object_mut()
        .expect("finalized case fixture object")
        .insert(
            "payload_b64".to_owned(),
            Value::from("c2hvdWxkLW5vdC1sZWFr"),
        );
    case
}
struct FixtureModerationOperatorMutationSource {
    expected_quarantine_id_hex: String,
    expected_kind: &'static str,
    status: StatusCode,
    body: Vec<u8>,
}
impl FixtureModerationOperatorMutationSource {
    fn response(&self) -> Result<Response<Vec<u8>>> {
        Ok(Response::builder()
            .status(self.status)
            .header("Content-Type", "application/json")
            .body(self.body.clone())
            .unwrap())
    }
}
impl ModerationOperatorWorkflowSource for FixtureModerationOperatorMutationSource {
    fn get_operator_panel(
        &self,
        _quarantine_id_hex: &str,
        _filter: SorafsModerationQuarantineFilter,
    ) -> Result<Response<Vec<u8>>> {
        unreachable!("mutation fixture does not serve operator-panel reads")
    }
    fn post_review(
        &self,
        quarantine_id_hex: &str,
        request: &SorafsModerationQuarantineReviewRequest<'_>,
    ) -> Result<Response<Vec<u8>>> {
        assert_eq!(self.expected_kind, "review");
        assert_eq!(quarantine_id_hex, self.expected_quarantine_id_hex);
        assert_eq!(request.reviewed_by, "operator@moderation");
        assert_eq!(request.reviewed_at_unix, Some(1_800_000_310));
        assert_eq!(request.notes, Some("reviewed through service"));
        self.response()
    }
    fn post_release(
        &self,
        quarantine_id_hex: &str,
        request: &SorafsModerationQuarantineReleaseRequest<'_>,
    ) -> Result<Response<Vec<u8>>> {
        assert_eq!(self.expected_kind, "release");
        assert_eq!(quarantine_id_hex, self.expected_quarantine_id_hex);
        assert_eq!(request.release_authority, "release@moderation");
        assert_eq!(request.released_at_unix, Some(1_800_000_320));
        assert_eq!(request.notes, Some("released through service"));
        self.response()
    }
    fn post_appeal_handoff(
        &self,
        quarantine_id_hex: &str,
        payload: &[u8],
    ) -> Result<Response<Vec<u8>>> {
        assert_eq!(self.expected_kind, "appeal-handoff");
        assert_eq!(quarantine_id_hex, self.expected_quarantine_id_hex);
        let value: Value = norito::json::from_slice(payload).expect("handoff payload JSON");
        assert_eq!(value.get("class").and_then(Value::as_str), Some("content"));
        assert!(!String::from_utf8_lossy(payload).contains("payload_b64"));
        self.response()
    }
}
fn fixture_moderation_operator_mutation_service(
    quarantine_id: [u8; 16],
    expected_kind: &'static str,
    status: StatusCode,
    body: Value,
) -> ModerationOperatorService {
    let args = ModerationQuarantineOperatorServeArgs {
        listen: "127.0.0.1:0".to_string(),
        limit: None,
        max_body_bytes: 2048,
    };
    args.service(
        Arc::new(FixtureModerationOperatorMutationSource {
            expected_quarantine_id_hex: encode(quarantine_id),
            expected_kind,
            status,
            body: norito::json::to_vec(&body).expect("fixture mutation response JSON"),
        }),
        "http://torii.test/".to_string(),
        "operator@moderation".to_string(),
    )
    .expect("operator mutation service")
}
fn handle_moderation_operator_raw_request(
    service: &ModerationOperatorService,
    raw: String,
) -> ModerationOperatorHttpResponse {
    handle_moderation_operator_raw_request_with_csrf(service, raw, true)
}
fn handle_moderation_operator_raw_request_without_csrf(
    service: &ModerationOperatorService,
    raw: String,
) -> ModerationOperatorHttpResponse {
    handle_moderation_operator_raw_request_with_csrf(service, raw, false)
}
fn handle_moderation_operator_raw_request_with_csrf(
    service: &ModerationOperatorService,
    mut raw: String,
    include_csrf: bool,
) -> ModerationOperatorHttpResponse {
    if include_csrf && raw.starts_with("POST ") {
        raw = raw.replacen(
            "\r\n\r\n",
            &format!(
                "\r\n{MODERATION_OPERATOR_CSRF_HEADER}: {}\r\n\r\n",
                service.csrf_token
            ),
            1,
        );
    }
    let raw = raw.into_bytes();
    let request = moderation_operator_parse_http_request(&raw, 1024).expect("HTTP request");
    service.handle_request(&request)
}
fn assert_moderation_operator_payload_rejected(
    quarantine_id: [u8; 16],
    request: String,
    quarantine_id_hex: String,
) {
    let service = fixture_moderation_operator_service(
        quarantine_id,
        None,
        moderation_operator_panel_fixture! {
            quarantine_id_hex;
            "case_count": 1_u64,
            "returned_case_count": 1_u64,
            "cases": [(fixture_payload_bearing_finalized_moderation_case())],
            "next_actions": []
        },
    );
    let response = handle_moderation_operator_raw_request(&service, request);
    assert_eq!(response.status, StatusCode::BAD_GATEWAY);
    let body = String::from_utf8(response.body).expect("error JSON is UTF-8");
    assert!(body.contains("payload bytes"));
}
test_items! {
fn moderation_operator_service_routes_operator_panel_with_query_limit() {
    let quarantine_id = [0xAB_u8; 16];
    let quarantine_id_hex = encode(quarantine_id);
    let service = fixture_moderation_operator_service(
        quarantine_id,
        Some(7),
        moderation_operator_panel_fixture! {
            quarantine_id_hex;
            "next_actions": []
        },
    );
    let response = handle_moderation_operator_raw_request(
        &service,
        format!(
            "GET /v1/sorafs/moderation/quarantine/{quarantine_id_hex}/operator-panel?limit=7 HTTP/1.1\r\nHost: local\r\n\r\n"
        ),
    );
    assert_eq!(response.status, StatusCode::OK);
    let value: Value = norito::json::from_slice(&response.body).expect("operator panel JSON");
    assert_eq_compact! { value.get("schema").and_then(Value::as_str) => Some("sorafs.moderation.quarantine.operator_panel.v1") };
    assert!(!String::from_utf8_lossy(&response.body).contains("payload_b64"));
}
fn moderation_operator_service_builds_bridge_plan_without_payload() {
    let quarantine_id = [0xAC_u8; 16];
    let quarantine_id_hex = encode(quarantine_id);
    let service = fixture_moderation_operator_service(
        quarantine_id,
        Some(5),
        moderation_operator_panel_fixture! {
            quarantine_id_hex;
            "case_count": 0_u64,
            "returned_case_count": 0_u64,
            "cases": [],
            "next_actions": [{
                "action": "review",
                "route": "/v1/sorafs/moderation/quarantine/review",
                "required": true
            }]
        },
    );
    let response = handle_moderation_operator_raw_request(
        &service,
        format!(
            "GET /v1/sorafs/moderation/quarantine/{quarantine_id_hex}/bridge-plan HTTP/1.1\r\nHost: local\r\n\r\n"
        ),
    );
    assert_eq!(response.status, StatusCode::OK);
    let value: Value = norito::json::from_slice(&response.body).expect("bridge plan JSON");
    assert_eq_compact! { value.get("schema").and_then(Value::as_str) => Some("sorafs.moderation.quarantine.bridge_plan.v1") };
    assert_eq_compact! { value.get("payload_bytes_included").and_then(Value::as_bool) => Some(false) };
    let actions = value
        .get("actions")
        .and_then(Value::as_array)
        .expect("planned actions");
    assert_eq_compact! { actions[0].get("automation_status").and_then(Value::as_str) => Some("operator_review_required") };
}
fn moderation_operator_service_builds_juror_plan_without_payload() {
    let quarantine_id = [0xA4_u8; 16];
    let quarantine_id_hex = encode(quarantine_id);
    let service = fixture_moderation_operator_service(
        quarantine_id,
        Some(2),
        moderation_operator_panel_fixture! {
            quarantine_id_hex;
            "case_count": 1_u64,
            "returned_case_count": 1_u64,
            "truncated_cases": false,
            "cases": [(
                fixture_finalized_moderation_case(
                    &["juror-a@moderation", "juror-b@moderation"],
                    &["juror-a@moderation"],
                    &[],
                )
            )],
            "next_actions": []
        },
    );
    let response = handle_moderation_operator_raw_request(
        &service,
        format!(
            "GET /v1/sorafs/moderation/quarantine/{quarantine_id_hex}/juror-plan HTTP/1.1\r\nHost: local\r\n\r\n"
        ),
    );
    assert_eq!(response.status, StatusCode::OK);
    let body = String::from_utf8(response.body.clone()).expect("juror plan body UTF-8");
    assert!(!body.contains("payload_b64"));
    let value: Value = norito::json::from_slice(&response.body).expect("juror plan JSON");
    assert_eq_compact! { value.get("schema").and_then(Value::as_str) => Some("sorafs.moderation.quarantine.juror_plan.v1") };
    assert_eq_compact! { value.get("payload_bytes_included").and_then(Value::as_bool) => Some(false) };
    assert_eq_compact! { value.get("notification_count").and_then(Value::as_u64) => Some(2) };
    assert_eq_compact! { value.get("pending_commit_count").and_then(Value::as_u64) => Some(1) };
    assert_eq_compact! { value.get("pending_reveal_count").and_then(Value::as_u64) => Some(1) };
    let ballots = value
        .get("ballots")
        .and_then(Value::as_array)
        .expect("planned ballots");
    let jurors = ballots[0]
        .get("jurors")
        .and_then(Value::as_array)
        .expect("planned jurors");
    assert_eq_compact! { jurors[0].get("notification_status").and_then(Value::as_str) => Some("reveal_required") };
    assert_eq_compact! { jurors[0].get("signed_by").and_then(Value::as_str) => Some("juror-a@moderation") };
    assert_eq_compact! { jurors[1].get("notification_status").and_then(Value::as_str) => Some("commit_required") };
    assert_eq_compact! { jurors[1].get("routes").and_then(Value::as_object).and_then(|routes| routes.get("commit")).and_then(Value::as_str) => Some("/v1/sorafs/moderation/ballots/commits") };
}
fn moderation_operator_service_builds_juror_notifications_without_payload() {
    let quarantine_id = [0xA6_u8; 16];
    let quarantine_id_hex = encode(quarantine_id);
    let service = fixture_moderation_operator_service(
        quarantine_id,
        Some(3),
        moderation_operator_panel_fixture! {
            quarantine_id_hex;
            "case_count": 1_u64,
            "returned_case_count": 1_u64,
            "truncated_cases": false,
            "cases": [(
                fixture_finalized_moderation_case(
                    &[
                        "juror-a@moderation",
                        "juror-b@moderation",
                        "juror-c@moderation",
                    ],
                    &["juror-a@moderation", "juror-c@moderation"],
                    &["juror-c@moderation"],
                )
            )],
            "next_actions": []
        },
    );
    let response = handle_moderation_operator_raw_request(
        &service,
        format!(
            "GET /v1/sorafs/moderation/quarantine/{quarantine_id_hex}/juror-notifications HTTP/1.1\r\nHost: local\r\n\r\n"
        ),
    );
    assert_eq!(response.status, StatusCode::OK);
    let body = String::from_utf8(response.body.clone()).expect("notification body UTF-8");
    assert!(!body.contains("payload_b64"));
    let value: Value =
        norito::json::from_slice(&response.body).expect("juror notifications JSON");
    assert_eq_compact! { value.get("schema").and_then(Value::as_str) => Some("sorafs.moderation.quarantine.juror_notifications.v1") };
    assert_eq_compact! { value.get("payload_bytes_included").and_then(Value::as_bool) => Some(false) };
    assert_eq_compact! { value.get("private_payloads_included").and_then(Value::as_bool) => Some(false) };
    assert_eq_compact! { value.get("planned_juror_count").and_then(Value::as_u64) => Some(3) };
    assert_eq_compact! { value.get("notification_count").and_then(Value::as_u64) => Some(2) };
    assert_eq_compact! { value.get("skipped_complete_count").and_then(Value::as_u64) => Some(1) };
    let notifications = value
        .get("notifications")
        .and_then(Value::as_array)
        .expect("notifications");
    assert_eq!(notifications.len(), 2);
    assert_eq_compact! { notifications[0].get("action").and_then(Value::as_str) => Some("submit_reveal") };
    assert_eq_compact! { notifications[0].get("route").and_then(Value::as_str) => Some("/v1/sorafs/moderation/ballots/reveals") };
    assert_eq_compact! { notifications[1].get("action").and_then(Value::as_str) => Some("submit_commit") };
    assert_eq_compact! { notifications[1].get("route").and_then(Value::as_str) => Some("/v1/sorafs/moderation/ballots/commits") };
    assert_eq_compact! { notifications[1].get("private_payload_included").and_then(Value::as_bool) => Some(false) };
    assert_eq_compact! { notifications[1].get("delivery_id").and_then(Value::as_str).map(str::len) => Some(64) };
    assert_compact! { notifications[1].get("body").and_then(Value::as_str).expect("notification body").contains("carries no payload bytes") };
}
fn moderation_operator_service_builds_commit_reveal_status_without_payload() {
    let quarantine_id = [0xA8_u8; 16];
    let quarantine_id_hex = encode(quarantine_id);
    let service = fixture_moderation_operator_service(
        quarantine_id,
        Some(3),
        moderation_operator_panel_fixture! {
            quarantine_id_hex;
            "case_count": 1_u64,
            "returned_case_count": 1_u64,
            "truncated_cases": false,
            "cases": [(
                fixture_finalized_moderation_case(
                    &[
                        "juror-a@moderation",
                        "juror-b@moderation",
                        "juror-c@moderation",
                    ],
                    &["juror-a@moderation", "juror-c@moderation"],
                    &["juror-a@moderation", "juror-c@moderation"],
                )
            )],
            "next_actions": []
        },
    );
    let response = handle_moderation_operator_raw_request(
        &service,
        format!(
            "GET /v1/sorafs/moderation/quarantine/{quarantine_id_hex}/commit-reveal-status HTTP/1.1\r\nHost: local\r\n\r\n"
        ),
    );
    assert_eq!(response.status, StatusCode::OK);
    let body = String::from_utf8(response.body.clone()).expect("status body UTF-8");
    assert!(!body.contains("payload_b64"));
    let value: Value =
        norito::json::from_slice(&response.body).expect("commit/reveal status JSON");
    assert_eq_compact! { value.get("schema").and_then(Value::as_str) => Some("sorafs.moderation.quarantine.commit_reveal_status.v1") };
    assert_eq_compact! { value.get("payload_bytes_included").and_then(Value::as_bool) => Some(false) };
    assert_eq_compact! { value.get("private_payloads_included").and_then(Value::as_bool) => Some(false) };
    assert_eq_compact! { value.get("tally_ready_count").and_then(Value::as_u64) => Some(1) };
    assert_eq_compact! { value.get("pending_commit_count").and_then(Value::as_u64) => Some(1) };
    assert_eq_compact! { value.get("pending_reveal_count").and_then(Value::as_u64) => Some(0) };
    let ballots = value
        .get("ballots")
        .and_then(Value::as_array)
        .expect("ballot statuses");
    assert_eq_compact! { ballots[0].get("next_action").and_then(Value::as_str) => Some("submit_tally") };
    assert_eq_compact! { ballots[0].get("ready_to_tally").and_then(Value::as_bool) => Some(true) };
    let missing_commit = ballots[0]
        .get("missing_commit_jurors")
        .and_then(Value::as_array)
        .expect("missing commit jurors");
    assert_eq!(missing_commit[0].as_str(), Some("juror-b@moderation"));
    assert_eq_compact! { ballots[0].get("tally_request").and_then(Value::as_object).and_then(|request| request.get("route")).and_then(Value::as_str) => Some("/v1/sorafs/moderation/ballots/tally") };
    assert_eq_compact! { ballots[0].get("tally_request").and_then(Value::as_object).and_then(|request| request.get("submission")).and_then(Value::as_str) => Some("caller-signed-native-transaction") };
    assert_compact! { ballots[0].get("tally_request").and_then(Value::as_object).is_some_and(|request| !request.contains_key("body")) };
}
fn moderation_operator_service_rejects_juror_plan_payload_bytes() {
    let quarantine_id = [0xA5_u8; 16];
    let quarantine_id_hex = encode(quarantine_id);
    assert_moderation_operator_payload_rejected(
        quarantine_id,
        format!(
            "GET /v1/sorafs/moderation/quarantine/{quarantine_id_hex}/juror-plan HTTP/1.1\r\nHost: local\r\n\r\n"
        ),
        quarantine_id_hex,
    );
}
fn moderation_operator_service_rejects_juror_notifications_payload_bytes() {
    let quarantine_id = [0xA7_u8; 16];
    let quarantine_id_hex = encode(quarantine_id);
    assert_moderation_operator_payload_rejected(
        quarantine_id,
        format!(
            "GET /v1/sorafs/moderation/quarantine/{quarantine_id_hex}/juror-notifications HTTP/1.1\r\nHost: local\r\n\r\n"
        ),
        quarantine_id_hex,
    );
}
fn moderation_operator_service_rejects_commit_reveal_status_payload_bytes() {
    let quarantine_id = [0xA9_u8; 16];
    let quarantine_id_hex = encode(quarantine_id);
    assert_moderation_operator_payload_rejected(
        quarantine_id,
        format!(
            "GET /v1/sorafs/moderation/quarantine/{quarantine_id_hex}/commit-reveal-status HTTP/1.1\r\nHost: local\r\n\r\n"
        ),
        quarantine_id_hex,
    );
}
fn moderation_operator_service_rejects_payload_b64_from_upstream() {
    let quarantine_id = [0xAD_u8; 16];
    let quarantine_id_hex = encode(quarantine_id);
    let service = fixture_moderation_operator_service(
        quarantine_id,
        None,
        moderation_operator_panel_fixture! {
            quarantine_id_hex;
            "payload_b64": "c2hvdWxkLW5vdC1sZWFr",
            "next_actions": []
        },
    );
    let response = handle_moderation_operator_raw_request(
        &service,
        format!(
            "GET /v1/sorafs/moderation/quarantine/{quarantine_id_hex}/operator-panel HTTP/1.1\r\nHost: local\r\n\r\n"
        ),
    );
    assert_eq!(response.status, StatusCode::BAD_GATEWAY);
    let body = String::from_utf8(response.body).expect("error JSON is UTF-8");
    assert!(body.contains("payload bytes"));
}
fn moderation_operator_service_rejects_request_body() {
    let request = b"GET /healthz HTTP/1.1\r\nHost: local\r\nContent-Length: 2\r\n\r\n{}";
    let parsed =
        moderation_operator_parse_http_request(request, 1024).expect("parse request body");
    let args = ModerationQuarantineOperatorServeArgs {
        listen: "127.0.0.1:0".to_string(),
        limit: None,
        max_body_bytes: 1024,
    };
    let service = args
        .service(
            Arc::new(FixtureModerationOperatorPanelSource {
                expected_quarantine_id_hex: encode([0_u8; 16]),
                expected_limit: None,
                status: StatusCode::OK,
                body: Vec::new(),
            }),
            "http://torii.test/".to_string(),
            "operator@moderation".to_string(),
        )
        .expect("operator service");
    let response = service.handle_request(&parsed);
    assert_eq!(response.status, StatusCode::BAD_REQUEST);
}
fn moderation_operator_parse_rejects_post_without_content_length() {
    let quarantine_id_hex = encode([0x42_u8; 16]);
    let request = format!(
        "POST /v1/sorafs/moderation/quarantine/{quarantine_id_hex}/review HTTP/1.1\r\nHost: local\r\n\r\n{{}}"
    );
    let error = moderation_operator_parse_http_request(request.as_bytes(), 1024)
        .expect_err("POST bodies must declare Content-Length");
    assert_eq!(error.status, StatusCode::BAD_REQUEST);
    assert!(error.message.contains("requires Content-Length"));
}
fn moderation_operator_parse_rejects_body_without_content_length() {
    let request = b"GET /healthz HTTP/1.1\r\nHost: local\r\n\r\n{}";
    let error = moderation_operator_parse_http_request(request, 1024)
        .expect_err("undeclared body bytes must be rejected");
    assert_eq!(error.status, StatusCode::BAD_REQUEST);
    assert!(error.message.contains("requires Content-Length"));
}
fn moderation_operator_parse_rejects_trailing_bytes_after_declared_body() {
    let request = b"GET /healthz HTTP/1.1\r\nHost: local\r\nContent-Length: 0\r\n\r\nGET / HTTP/1.1\r\n\r\n";
    let error = moderation_operator_parse_http_request(request, 1024)
        .expect_err("trailing bytes after declared body must be rejected");
    assert_eq!(error.status, StatusCode::BAD_REQUEST);
    assert!(error.message.contains("trailing bytes"));
}
fn moderation_operator_read_rejects_trailing_bytes_after_declared_body() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind test listener");
    let addr = listener.local_addr().expect("listener address");
    let client = thread::spawn(move || {
        let mut stream = TcpStream::connect(addr).expect("connect test client");
        stream
            .write_all(
                b"GET /healthz HTTP/1.1\r\nHost: local\r\nContent-Length: 0\r\n\r\nGET / HTTP/1.1\r\n\r\n",
            )
            .expect("write trailing request bytes");
    });
    let (mut stream, _) = listener.accept().expect("accept test client");
    let error = moderation_operator_read_http_request(&mut stream, 1024)
        .expect_err("socket reader must reject trailing request bytes");
    assert_eq!(error.status, StatusCode::BAD_REQUEST);
    assert!(error.message.contains("trailing bytes"));
    client.join().expect("client thread finished");
}
fn moderation_operator_service_serves_browser_ui() {
    let quarantine_id = [0xA1_u8; 16];
    let service = fixture_moderation_operator_service(quarantine_id, None, norito::json!({}));
    let response = handle_moderation_operator_raw_request(
        &service,
        "GET / HTTP/1.1\r\nHost: local\r\n\r\n".to_string(),
    );
    assert_eq!(response.status, StatusCode::OK);
    assert_eq_compact! { response.content_type => ModerationOperatorService::HTML_CONTENT_TYPE };
    let body = String::from_utf8(response.body.clone()).expect("UI body UTF-8");
    assert!(body.contains("SoraFS Moderation Operator"));
    assert!(body.contains("juror-plan"));
    assert!(body.contains("juror-notifications"));
    assert!(body.contains("commit-reveal-status"));
    assert!(!body.contains(&["ballot", "tally"].join("-")));
    assert!(body.contains(MODERATION_OPERATOR_CSRF_HEADER));
    assert!(body.contains(&service.csrf_token));
    let http = String::from_utf8(response.to_http_bytes()).expect("HTTP response UTF-8");
    assert!(http.contains("Content-Type: text/html; charset=utf-8"));
    assert!(http.contains("X-Content-Type-Options: nosniff"));
}
fn moderation_operator_service_status_lists_browser_ui_route() {
    let quarantine_id = [0xA2_u8; 16];
    let service = fixture_moderation_operator_service(quarantine_id, None, norito::json!({}));
    let response = handle_moderation_operator_raw_request(
        &service,
        "GET /v1/sorafs/moderation/operator-panel/status HTTP/1.1\r\nHost: local\r\n\r\n"
            .to_string(),
    );
    assert_eq!(response.status, StatusCode::OK);
    let value: Value = norito::json::from_slice(&response.body).expect("status JSON");
    let routes = value
        .get("routes")
        .and_then(Value::as_array)
        .expect("status routes");
    assert_eq_compact! { value.get("csrf_header").and_then(Value::as_str) => Some(MODERATION_OPERATOR_CSRF_HEADER) };
    assert_eq_compact! { value.get("csrf_token").and_then(Value::as_str) => Some(service.csrf_token.as_str()) };
    assert_compact! { routes.iter().any(|route| { route.as_str() == Some("/v1/sorafs/moderation/operator-panel/ui") }) };
    assert_compact! { routes.iter().any(|route| { route.as_str() == Some("/v1/sorafs/moderation/quarantine/{quarantine_id_hex}/juror-plan") }) };
    assert_compact! { routes.iter().any(|route| { route.as_str() == Some("/v1/sorafs/moderation/quarantine/{quarantine_id_hex}/juror-notifications") }) };
    assert_compact! { routes.iter().any(|route| { route.as_str() == Some("/v1/sorafs/moderation/quarantine/{quarantine_id_hex}/commit-reveal-status") }) };
}
fn moderation_operator_service_rejects_browser_ui_request_body() {
    let quarantine_id = [0xA3_u8; 16];
    let service = fixture_moderation_operator_service(quarantine_id, None, norito::json!({}));
    let response = handle_moderation_operator_raw_request(
        &service,
        "GET /v1/sorafs/moderation/operator-panel/ui HTTP/1.1\r\nHost: local\r\nContent-Length: 2\r\n\r\n{}"
            .to_string(),
    );
    assert_eq!(response.status, StatusCode::BAD_REQUEST);
    assert_eq_compact! { response.content_type => ModerationOperatorService::JSON_CONTENT_TYPE };
}
fn moderation_operator_service_rejects_mutation_without_csrf_token() {
    let quarantine_id = [0xA9_u8; 16];
    let quarantine_id_hex = encode(quarantine_id);
    let service = fixture_moderation_operator_mutation_service(
        quarantine_id,
        "review",
        StatusCode::ACCEPTED,
        norito::json!({ "status": "must_not_be_called" }),
    );
    let body = r#"{"notes":"missing token"}"#;
    let response = handle_moderation_operator_raw_request_without_csrf(
        &service,
        format!(
            "POST /v1/sorafs/moderation/quarantine/{quarantine_id_hex}/review HTTP/1.1\r\nHost: local\r\nContent-Length: {}\r\n\r\n{body}",
            body.len()
        ),
    );
    assert_eq!(response.status, StatusCode::FORBIDDEN);
    let body = String::from_utf8(response.body).expect("error body UTF-8");
    assert!(body.contains(MODERATION_OPERATOR_CSRF_HEADER));
}
fn moderation_operator_service_rejects_mutation_with_wrong_csrf_token() {
    let quarantine_id = [0xAA_u8; 16];
    let quarantine_id_hex = encode(quarantine_id);
    let service = fixture_moderation_operator_mutation_service(
        quarantine_id,
        "review",
        StatusCode::ACCEPTED,
        norito::json!({ "status": "must_not_be_called" }),
    );
    let body = r#"{"notes":"wrong token"}"#;
    let response = handle_moderation_operator_raw_request_without_csrf(
        &service,
        format!(
            "POST /v1/sorafs/moderation/quarantine/{quarantine_id_hex}/review HTTP/1.1\r\nHost: local\r\n{MODERATION_OPERATOR_CSRF_HEADER}: wrong\r\nContent-Length: {}\r\n\r\n{body}",
            body.len()
        ),
    );
    assert_eq!(response.status, StatusCode::FORBIDDEN);
    let body = String::from_utf8(response.body).expect("error body UTF-8");
    assert!(body.contains("CSRF"));
}
fn moderation_operator_service_forwards_review_with_default_actor() {
    let quarantine_id = [0xAE_u8; 16];
    let quarantine_id_hex = encode(quarantine_id);
    let service = fixture_moderation_operator_mutation_service(
        quarantine_id,
        "review",
        StatusCode::ACCEPTED,
        norito::json!({
            "schema": "sorafs.moderation.quarantine.review.v1",
            "status": "reviewed"
        }),
    );
    let body = r#"{"reviewed_at_unix":1800000310,"notes":"reviewed through service"}"#;
    let response = handle_moderation_operator_raw_request(
        &service,
        format!(
            "POST /v1/sorafs/moderation/quarantine/{quarantine_id_hex}/review HTTP/1.1\r\nHost: local\r\nContent-Length: {}\r\n\r\n{body}",
            body.len()
        ),
    );
    assert_eq!(response.status, StatusCode::ACCEPTED);
    let value: Value = norito::json::from_slice(&response.body).expect("review response JSON");
    assert_eq_compact! { value.get("status").and_then(Value::as_str) => Some("reviewed") };
}
fn moderation_operator_service_forwards_release_with_explicit_authority() {
    let quarantine_id = [0xAF_u8; 16];
    let quarantine_id_hex = encode(quarantine_id);
    let service = fixture_moderation_operator_mutation_service(
        quarantine_id,
        "release",
        StatusCode::ACCEPTED,
        norito::json!({
            "schema": "sorafs.moderation.quarantine.release.v1",
            "status": "released"
        }),
    );
    let body = r#"{"release_authority":"release@moderation","released_at_unix":1800000320,"notes":"released through service"}"#;
    let response = handle_moderation_operator_raw_request(
        &service,
        format!(
            "POST /v1/sorafs/moderation/quarantine/{quarantine_id_hex}/release HTTP/1.1\r\nHost: local\r\nContent-Length: {}\r\n\r\n{body}",
            body.len()
        ),
    );
    assert_eq!(response.status, StatusCode::ACCEPTED);
    let value: Value = norito::json::from_slice(&response.body).expect("release response JSON");
    assert_eq_compact! { value.get("status").and_then(Value::as_str) => Some("released") };
}
fn moderation_operator_service_forwards_appeal_handoff_payload() {
    let quarantine_id = [0xB0_u8; 16];
    let quarantine_id_hex = encode(quarantine_id);
    let service = fixture_moderation_operator_mutation_service(
        quarantine_id,
        "appeal-handoff",
        StatusCode::OK,
        norito::json!({
            "schema": "sorafs.moderation.quarantine.appeal_handoff.v1",
            "status": "handoff_ready"
        }),
    );
    let body = r#"{"class":"content","backlog":3,"evidence_size_mb":8}"#;
    let response = handle_moderation_operator_raw_request(
        &service,
        format!(
            "POST /v1/sorafs/moderation/quarantine/{quarantine_id_hex}/appeal-handoff HTTP/1.1\r\nHost: local\r\nContent-Length: {}\r\n\r\n{body}",
            body.len()
        ),
    );
    assert_eq!(response.status, StatusCode::OK);
    let value: Value = norito::json::from_slice(&response.body).expect("handoff response JSON");
    assert_eq_compact! { value.get("status").and_then(Value::as_str) => Some("handoff_ready") };
}
fn moderation_operator_service_rejects_mutation_payload_bytes() {
    let quarantine_id = [0xB2_u8; 16];
    let quarantine_id_hex = encode(quarantine_id);
    let service = fixture_moderation_operator_mutation_service(
        quarantine_id,
        "appeal-handoff",
        StatusCode::OK,
        norito::json!({ "status": "must_not_be_called" }),
    );
    let body = r#"{"class":"content","payload_b64":"c2hvdWxkLW5vdC1sZWFr"}"#;
    let response = handle_moderation_operator_raw_request(
        &service,
        format!(
            "POST /v1/sorafs/moderation/quarantine/{quarantine_id_hex}/appeal-handoff HTTP/1.1\r\nHost: local\r\nContent-Length: {}\r\n\r\n{body}",
            body.len()
        ),
    );
    assert_eq!(response.status, StatusCode::BAD_REQUEST);
    let body = String::from_utf8(response.body).expect("error body UTF-8");
    assert!(body.contains("payload bytes"));
}
fn moderation_quarantine_review_rejects_blank_notes() {
    let args = ModerationQuarantineReviewArgs {
        quarantine_id: encode([0xEF_u8; 16]),
        reviewed_by: Some("operator@moderation".to_string()),
        reviewed_at: Some("@1800000210".to_string()),
        notes: Some("   ".to_string()),
    };
    let mut ctx = TestContext::new();
    let err = args
        .run_with(&mut ctx, |_client, _, _| {
            unreachable!("submit must not run")
        })
        .expect_err("blank notes must be rejected");
    assert!(err.to_string().contains("--notes"));
    assert!(ctx.printed.is_empty());
}
fn repair_ticket_id_rejects_lowercase() {
    let result = parse_repair_ticket_id("rep-1", "--ticket-id");
    assert!(result.is_err(), "lowercase ticket id should fail");
}
}
fn single_repair_action(transaction: &SignedTransaction) -> &ApplySorafsRepairTaskAction {
    let iroha_data_model::transaction::Executable::Instructions(instructions) =
        transaction.instructions()
    else {
        panic!("repair transaction must contain native instructions");
    };
    assert_eq!(instructions.len(), 1);
    instructions[0]
        .as_any()
        .downcast_ref::<ApplySorafsRepairTaskAction>()
        .expect("repair transaction contains ApplySorafsRepairTaskAction")
}
test_items! {
    fn repair_list_uses_finalized_task_cursor() {
        let args = RepairListArgs {
            ticket_id: None,
            limit: Some(25),
            expected_finalized_height: Some(7),
            expected_finalized_block_hash: Some(format!("0x{}", "AB".repeat(32))),
            after_task_id: Some("CD".repeat(32)),
        };
        let mut ctx = TestContext::new();
        args.run_with(
            &mut ctx,
            |_client, filter| {
                assert_eq!(filter.limit, Some(25));
                assert_eq!(filter.finalized.expected_finalized_height, Some(7));
                assert_eq_compact! { filter.finalized.expected_finalized_block_hash_hex => Some("ab".repeat(32).as_str()) };
                assert_eq!(filter.after_task_id_hex, Some("cd".repeat(32).as_str()));
json_response_fixture!(StatusCode::OK, &norito::json!({
                        "tasks": [ { "ticket_id": "REP-1" } ]
                    }))
            },
            |_client, _, _| unreachable!("single-task lookup should not be called"),
        )
        .expect("run should succeed");
        assert_eq!(ctx.printed.len(), 1);
        assert!(ctx.printed[0].contains("\"tasks\""));
    }
    fn repair_claim_builds_native_signed_transaction() {
        let args = RepairClaimArgs {
            ticket_id: "REP-501".to_string(),
            expected_revision: 2,
            lease_duration_ms: 60_000,
            idempotency_key: Some("claim-501".to_string()),
        };
        let mut ctx = TestContext::new();
        args.run_with(&mut ctx, |_client, transaction| {
            transaction.verify_signature().expect("canonical signed public transaction");
            let apply = single_repair_action(transaction);
            assert_eq!(apply.ticket_id, "REP-501");
            assert_eq!(apply.expected_revision, 2);
            let SorafsRepairTaskActionV1::Claim(action) = &apply.action else {
                panic!("expected claim action");
            };
            assert_eq!(action.lease_duration_ms, 60_000);
            assert_eq!(action.idempotency_key, "claim-501");
            Ok(transaction.hash())
        })
        .expect("run should succeed");
        assert_eq!(ctx.printed.len(), 1);
        assert!(ctx.printed[0].contains("transaction_hash_hex"));
    }
    fn repair_renew_builds_native_signed_transaction() {
        let args = RepairRenewArgs {
            ticket_id: "REP-502".to_string(),
            expected_revision: 3,
            lease_generation: 2,
            lease_duration_ms: 90_000,
            idempotency_key: Some("renew-502".to_string()),
        };
        let mut ctx = TestContext::new();
        args.run_with(&mut ctx, |_client, transaction| {
            let apply = single_repair_action(transaction);
            assert_eq!(apply.ticket_id, "REP-502");
            assert_eq!(apply.expected_revision, 3);
            let SorafsRepairTaskActionV1::Renew(action) = &apply.action else {
                panic!("expected renew action");
            };
            assert_eq!(action.lease_generation, 2);
            assert_eq!(action.lease_duration_ms, 90_000);
            assert_eq!(action.idempotency_key, "renew-502");
            Ok(transaction.hash())
        })
        .expect("run should succeed");
        assert_eq!(ctx.printed.len(), 1);
    }
    fn repair_complete_builds_native_signed_transaction() {
        let evidence_digest = [0x33_u8; 32];
        let args = RepairCompleteArgs {
            ticket_id: "REP-503".to_string(),
            expected_revision: 4,
            lease_generation: 2,
            evidence_digest: encode(evidence_digest),
            idempotency_key: Some("complete-503".to_string()),
        };
        let mut ctx = TestContext::new();
        args.run_with(&mut ctx, |_client, transaction| {
            let apply = single_repair_action(transaction);
            assert_eq!(apply.ticket_id, "REP-503");
            assert_eq!(apply.expected_revision, 4);
            let SorafsRepairTaskActionV1::Complete(action) = &apply.action else {
                panic!("expected complete action");
            };
            assert_eq!(action.lease_generation, 2);
            assert_eq!(action.evidence_digest, evidence_digest);
            assert_eq!(action.idempotency_key, "complete-503");
            Ok(transaction.hash())
        })
        .expect("run should succeed");
        assert_eq!(ctx.printed.len(), 1);
    }
    fn repair_fail_builds_native_signed_transaction() {
        let failure_digest = [0x55_u8; 32];
        let args = RepairFailArgs {
            ticket_id: "REP-504".to_string(),
            expected_revision: 5,
            lease_generation: 3,
            failure_digest: encode(failure_digest),
            idempotency_key: Some("fail-504".to_string()),
        };
        let mut ctx = TestContext::new();
        args.run_with(&mut ctx, |_client, transaction| {
            let apply = single_repair_action(transaction);
            assert_eq!(apply.ticket_id, "REP-504");
            assert_eq!(apply.expected_revision, 5);
            let SorafsRepairTaskActionV1::Fail(action) = &apply.action else {
                panic!("expected fail action");
            };
            assert_eq!(action.lease_generation, 3);
            assert_eq!(action.failure_digest, failure_digest);
            assert_eq!(action.idempotency_key, "fail-504");
            Ok(transaction.hash())
        })
        .expect("run should succeed");
        assert_eq!(ctx.printed.len(), 1);
    }
    fn repair_escalate_builds_native_atomic_slash_transaction() {
        let manifest_digest = [0x77_u8; 32];
        let provider_id = [0x88_u8; 32];
        let args = RepairEscalateArgs {
            ticket_id: "REP-505".to_string(),
            expected_revision: 6,
            lease_generation: 4,
            manifest_digest: encode(manifest_digest),
            provider_id: encode(provider_id),
            penalty: "0.0000009".to_owned(),
            rationale: "sla_missed".to_string(),
            auditor: None,
            submitted_at: Some("@1700000504".to_string()),
            idempotency_key: Some("escalate-505".to_string()),
        };
        let mut ctx = TestContext::new();
        let expected_auditor = ctx.config().account.to_string();
        args.run_with(&mut ctx, |_client, transaction| {
            let apply = single_repair_action(transaction);
            assert_eq!(apply.ticket_id, "REP-505");
            assert_eq!(apply.expected_revision, 6);
            let SorafsRepairTaskActionV1::Escalate(action) = &apply.action else {
                panic!("expected escalate action");
            };
            assert_eq!(action.lease_generation, 4);
            assert_eq!(action.idempotency_key, "escalate-505");
            let proposal: RepairSlashProposalV1 =
                norito::decode_from_bytes(&action.slash_proposal_payload)
                    .expect("decode canonical slash proposal");
            assert_eq!(proposal.ticket_id.0, "REP-505");
            assert_eq!(proposal.provider_id, provider_id);
            assert_eq!(proposal.manifest_digest, manifest_digest);
            assert_eq!(proposal.auditor_account, expected_auditor);
            assert_eq_compact! { proposal.proposed_penalty => "0.0000009".parse::<XorQuantity>().expect("valid quantity") };
            assert_eq!(proposal.submitted_at_unix, 1_700_000_504);
            assert!(proposal.approval.is_none());
            Ok(transaction.hash())
        })
        .expect("run should succeed");
        assert_eq!(ctx.printed.len(), 1);
    }
    fn repair_action_rejects_zero_compare_and_set_revision() {
        let args = RepairClaimArgs {
            ticket_id: "REP-506".to_string(),
            expected_revision: 0,
            lease_duration_ms: 60_000,
            idempotency_key: Some("claim-506".to_string()),
        };
        let mut ctx = TestContext::new();
        let error = args
            .run_with(&mut ctx, |_client, _| {
                unreachable!("zero revision must fail before submission")
            })
            .expect_err("zero compare-and-set revision must fail");
        assert!(error.to_string().contains("--expected-revision"));
        assert!(ctx.printed.is_empty());
    }
    fn gc_inspect_reports_expiry_state() {
        let dir = TempDir::new().expect("temp dir");
        write_gc_manifest(
            dir.path(),
            "alpha",
            1_000,
            ManifestStorageClass::Hot,
            100,
            10,
        );
        write_gc_manifest(
            dir.path(),
            "beta",
            2_000,
            ManifestStorageClass::Warm,
            200,
            20,
        );
        write_gc_manifest(dir.path(), "gamma", 0, ManifestStorageClass::Cold, 300, 30);
        let report = build_gc_report("inspect", Some(dir.path()), Some("@1500"), Some(100), false)
            .expect("report");
        assert_eq!(report.mode, "inspect");
        assert_eq!(report.total_manifests, 3);
        assert_eq!(report.total_payload_bytes, 600);
        assert_eq!(report.total_car_bytes, 60);
        assert_eq!(report.expired_count, 1);
        assert_eq!(report.expired_payload_bytes, 100);
        assert_eq!(report.expired_car_bytes, 10);
        assert_eq!(report.entries.len(), 3);
        assert_eq!(report.now_unix, 1_500);
        assert_eq!(report.grace_secs, 100);
        let first = &report.entries[0];
        assert_eq!(first.manifest_id, "alpha");
        assert_eq!(first.storage_class, "hot");
        assert_eq!(first.expires_at_unix, Some(1_100));
        assert!(first.expired);
        assert_eq!(first.payload_bytes, 100);
        assert_eq!(first.car_bytes, 10);
        assert_eq!(first.manifest_digest_hex.len(), 64);
        let last = &report.entries[2];
        assert_eq!(last.manifest_id, "gamma");
        assert_eq!(last.storage_class, "cold");
        assert_eq!(last.expires_at_unix, None);
        assert!(!last.expired);
    }
    fn gc_dry_run_filters_expired() {
        let dir = TempDir::new().expect("temp dir");
        write_gc_manifest(
            dir.path(),
            "alpha",
            1_000,
            ManifestStorageClass::Hot,
            100,
            10,
        );
        write_gc_manifest(
            dir.path(),
            "beta",
            2_000,
            ManifestStorageClass::Warm,
            200,
            20,
        );
        let report = build_gc_report("dry_run", Some(dir.path()), Some("@1500"), Some(100), true)
            .expect("report");
        assert_eq!(report.mode, "dry_run");
        assert_eq!(report.total_manifests, 2);
        assert_eq!(report.expired_count, 1);
        assert_eq!(report.entries.len(), 1);
        assert_eq!(report.entries[0].manifest_id, "alpha");
        assert!(report.entries[0].expired);
    }
    fn gc_inspect_command_prints_json_report() {
        let dir = TempDir::new().expect("temp dir");
        write_gc_manifest(dir.path(), "alpha", 0, ManifestStorageClass::Hot, 50, 5);
        let args = GcInspectArgs {
            data_dir: Some(dir.path().to_path_buf()),
            now: Some("@1500".to_string()),
            grace_secs: Some(100),
        };
        let mut ctx = TestContext::new();
        GcCommand::Inspect(args).run(&mut ctx).expect("inspect run");
        let output = ctx.outputs().last().expect("output");
        let json: Value = norito::json::from_str(output).expect("json");
        assert_eq!(json["mode"], Value::from("inspect"));
        assert_eq!(json["total_manifests"], Value::from(1u64));
        assert_eq!(json["entries"].as_array().map(Vec::len), Some(1));
    }
    fn gc_dry_run_command_filters_json_entries() {
        let dir = TempDir::new().expect("temp dir");
        write_gc_manifest(
            dir.path(),
            "alpha",
            1_000,
            ManifestStorageClass::Warm,
            10,
            1,
        );
        write_gc_manifest(dir.path(), "beta", 2_000, ManifestStorageClass::Cold, 20, 2);
        let args = GcDryRunArgs {
            data_dir: Some(dir.path().to_path_buf()),
            now: Some("@1500".to_string()),
            grace_secs: Some(100),
        };
        let mut ctx = TestContext::new();
        GcCommand::DryRun(args).run(&mut ctx).expect("dry run");
        let output = ctx.outputs().last().expect("output");
        let json: Value = norito::json::from_str(output).expect("json");
        assert_eq!(json["mode"], Value::from("dry_run"));
        assert_eq!(json["total_manifests"], Value::from(2u64));
        assert_eq!(json["entries"].as_array().map(Vec::len), Some(1));
    }
    fn gc_manifest_entries_require_manifest_dir() {
        let dir = TempDir::new().expect("temp dir");
        let err = load_gc_manifest_entries(dir.path()).expect_err("missing manifests");
        assert_compact! { err.to_string().contains("SoraFS manifests directory"); "unexpected error: {err}" };
    }
    fn gc_retention_deadline_respects_zero_epoch() {
        assert_eq!(retention_deadline(0, 5), None);
        assert_eq!(retention_deadline(10, 5), Some(15));
    }
    fn gc_storage_class_labels_match_expected_values() {
        assert_eq_compact! { manifest_storage_class_label(ManifestStorageClass::Hot) => "hot" };
        assert_eq_compact! { manifest_storage_class_label(ManifestStorageClass::Warm) => "warm" };
        assert_eq_compact! { manifest_storage_class_label(ManifestStorageClass::Cold) => "cold" };
    }
    fn storage_token_issue_passes_arguments_and_prints_nonce() {
        use std::cell::RefCell;
        let args = StorageTokenIssueArgs {
            manifest_id: "aa".repeat(32),
            provider_id: "bb".repeat(32),
            client_id: "gateway-alpha".into(),
            nonce: None,
            ttl_secs: Some(600),
            max_streams: Some(5),
            rate_limit_bytes: Some(256_000),
            requests_per_minute: Some(90),
        };
        let mut ctx = TestContext::new();
        let captured = RefCell::new(None);
        args.run_with(
            &mut ctx,
            |_, manifest, provider, client_id, nonce, overrides| {
                *captured.borrow_mut() = Some((
                    manifest.to_owned(),
                    provider.to_owned(),
                    client_id.to_owned(),
                    nonce.to_owned(),
                    *overrides,
            ));
            let body = norito::json::to_vec(&norito::json!({ "token": { "body": {} } }))?;
            Ok(Response::builder()
                .status(StatusCode::OK)
                .header("Content-Type", "application/json")
                .body(body)
                .unwrap())
        },
        )
        .expect("token issue succeeds");
        let (manifest, provider, client_id, nonce, overrides) =
            captured.borrow().clone().expect("captured arguments");
        assert_eq!(manifest, "aa".repeat(32));
        assert_eq!(provider, "bb".repeat(32));
        assert_eq!(client_id, "gateway-alpha");
        assert_eq!(overrides.ttl_secs, Some(600));
        assert_eq!(overrides.max_streams, Some(5));
        assert_eq!(overrides.rate_limit_bytes, Some(256_000));
        assert_eq!(overrides.requests_per_minute, Some(90));
        assert_eq!(ctx.printed.len(), 2);
        assert_compact! { ctx.printed[0].starts_with("nonce: "); "expected nonce println, got {}", ctx.printed[0] };
        assert_eq_compact! { ctx.printed[1] => "{\"token\":{\"body\":{}}}"; "expected JSON payload output" };
        assert_eq_compact! { nonce.len() => 24; "nonce should be 12 random bytes hex encoded" };
    }
    fn direct_mode_plan_generates_summary() {
        let manifest = ManifestBuilder::new()
            .root_cid(vec![0x01, 0x02, 0x03])
            .dag_codec(DagCodecId(0x71))
            .chunking_profile(ChunkingProfileV1 {
                profile_id: ProfileId(7),
                namespace: "sorafs".into(),
                name: "sf1".into(),
                semver: "1.0.0".into(),
                min_size: 4096,
                target_size: 262_144,
                max_size: 524_288,
                break_mask: 0,
                multihash_code: BLAKE3_256_MULTIHASH_CODE,
                aliases: vec!["sf1".into()],
            })
            .chunk_digest_sha3_256([0xCD; 32])
            .por_root([0xCE; 32])
            .content_length(1_048_576)
            .car_digest([0xAB; 32])
            .car_size(1_111_111)
            .pin_policy(PinPolicy {
                min_replicas: 3,
                storage_class: ManifestStorageClass::Hot,
                retention_epoch: 0,
            })
            .add_metadata("manifest.requires_envelope", "true")
            .add_metadata("capability.direct_car", "true")
            .build()
            .expect("build manifest");
        let bytes = to_bytes(&manifest).expect("encode manifest");
        let mut temp_manifest = NamedTempFile::new().expect("temp manifest");
        temp_manifest
            .write_all(&bytes)
            .expect("write manifest bytes");
        let provider = [0xAA; 32];
        let args = GatewayDirectModePlanArgs {
            manifest: temp_manifest.path().to_path_buf(),
            admission_envelope: None,
            provider_id: Some(hex::encode(provider)),
            chain_id: Some("nexus".to_owned()),
            scheme: "https".to_owned(),
        };
        let mut ctx = TestContext::new();
        args.run(&mut ctx).expect("plan command runs");
        assert_eq!(ctx.outputs().len(), 1);
        let plan: DirectModePlanOutput =
            norito::json::from_str(&ctx.outputs()[0]).expect("parse plan");
        assert_eq!(plan.provider_id_hex, hex::encode(provider));
        assert_eq!(plan.chain_id, "nexus");
        assert_compact! { plan.direct_car.canonical_url.contains("/direct/v1/car/"); "direct car locator should reference the manifest digest" };
        assert!(plan.capabilities.direct_car_supported);
    }
    }
fn direct_mode_enable_capabilities() -> ManifestCapabilitySummary {
    ManifestCapabilitySummary {
        direct_car_supported: true,
        ..ManifestCapabilitySummary::default()
    }
}
fn direct_mode_enable_test_plan(capabilities: ManifestCapabilitySummary) -> DirectModePlanOutput {
    let chain_id = "nexus";
    let provider = [0x33; 32];
    let manifest_digest_hex = "fe".repeat(32);
    let host_input = HostMappingInput {
        chain_id,
        provider_id: &provider,
    };
    let hosts = host_input.to_summary();
    let direct_car = host_input
        .direct_car_locator("https", &manifest_digest_hex)
        .expect("direct CAR locator");
    DirectModePlanOutput::from_components(
        chain_id,
        provider,
        manifest_digest_hex,
        hosts,
        direct_car,
        capabilities,
    )
}
fn write_direct_mode_plan(plan: &DirectModePlanOutput) -> NamedTempFile {
    let mut plan_file = NamedTempFile::new().expect("temp plan");
    plan_file
        .write_all(&norito::json::to_vec(plan).expect("serialize plan"))
        .expect("write plan");
    plan_file
}
pub(super) fn assert_sorafs_config_snippet_is_schema_valid(snippet: &str) -> UserSorafsConfig {
    let mut root: toml::Table =
        toml::from_str(snippet).expect("generated snippet must parse as TOML");
    let sorafs = root
        .remove("sorafs")
        .expect("generated snippet must use the top-level `sorafs` table");
    assert_compact! { root.is_empty(); "generated snippet contains unexpected top-level keys: {root:?}" };
    let sorafs = sorafs
        .as_table()
        .expect("top-level `sorafs` value must be a table")
        .clone();
    ConfigReader::new()
        .with_toml_source(TomlSource::inline(sorafs))
        .read_and_complete::<UserSorafsConfig>()
        .expect("generated snippet must satisfy the iroha_config SoraFS schema")
}
