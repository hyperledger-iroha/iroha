//! Authoritative SoraCloud agent-apartment ledger transition tests.
use super::*;

#[test]
fn agent_text_helpers_preserve_free_form_bytes_and_reject_aliases() {
    assert_eq!(
        validate_agent_reason(" reason with intentional padding ")
            .expect("free-form reason must be preserved"),
        " reason with intentional padding "
    );
    assert!(validate_optional_agent_reason(Some(" \t ")).is_err());
    assert_eq!(
        parse_agent_capability_name("agent.autonomy.run").expect("canonical capability"),
        "agent.autonomy.run"
    );
    assert!(parse_agent_capability_name(" agent.autonomy.run").is_err());
    assert!(parse_agent_capability_name("cafe\u{301}").is_err());
    assert_eq!(
        parse_agent_mailbox_channel("ops.sync").expect("canonical channel"),
        "ops.sync"
    );
    assert!(parse_agent_mailbox_channel(" ops.sync").is_err());
    assert_eq!(
        validate_agent_mailbox_payload(" payload bytes ")
            .expect("free-form mailbox payload must be preserved"),
        " payload bytes "
    );
    assert!(validate_agent_mailbox_payload(" \n ").is_err());
    assert!(parse_agent_record_id("message_id", "worker:mail:1 ").is_err());
    assert_eq!(
        parse_agent_hash_like("artifact_hash", "hash:artifact#1").expect("canonical artifact hash"),
        "hash:artifact#1"
    );
    assert!(parse_agent_hash_like("artifact_hash", " hash:artifact#1").is_err());
    assert_eq!(
        parse_agent_run_label("nightly batch").expect("canonical run label"),
        "nightly batch"
    );
    assert!(parse_agent_run_label(" nightly batch").is_err());
    let canonical_json = "{\"a\":1,\"b\":2}";
    assert_eq!(
        parse_optional_agent_workflow_input_json(Some(canonical_json))
            .expect("canonical workflow JSON"),
        Some(canonical_json.to_owned())
    );
    for noncanonical_json in [
        " {\"a\":1,\"b\":2}",
        "{\"b\":2,\"a\":1}",
        "{\"a\": 1,\"b\":2}",
    ] {
        let error = parse_optional_agent_workflow_input_json(Some(noncanonical_json))
            .expect_err("workflow JSON aliases must fail closed");
        assert!(
            error
                .to_string()
                .contains("canonical Norito JSON serialization"),
            "unexpected workflow JSON rejection: {error}"
        );
    }
}

#[test]
fn agent_execute_paths_reject_pre_v1_text_rewrites_before_state_lookup() -> Result<(), eyre::Report>
{
    let kura = Kura::blank_kura_for_testing();
    let state = state_with_soracloud_permission(&kura)?;
    let block_header = ValidBlock::new_dummy(&checked_keypair().into_parts().1)
        .as_ref()
        .header();
    let mut state_block = state.block(block_header);
    let mut stx = state_block.transaction();
    let apartment_name: iroha_data_model::name::Name =
        "missing_agent".parse().expect("valid apartment name");
    let provenance_for = |payload: Vec<u8>| ManifestProvenance {
        signer: ALICE_KEYPAIR.public_key().clone(),
        signature: checked_signature(ALICE_KEYPAIR.private_key(), &payload),
    };

    let policy_capability_payload = encode_agent_policy_revoke_provenance_payload(
        apartment_name.as_ref(),
        "agent.autonomy.run",
        None,
    )?;
    let error = iroha_data_model::isi::InstructionBox::from(isi::RevokeSoracloudAgentPolicy {
        apartment_name: apartment_name.clone(),
        capability: " agent.autonomy.run ".to_owned(),
        reason: None,
        provenance: provenance_for(policy_capability_payload),
    })
    .execute(&ALICE_ID, &mut stx)
    .expect_err("padded capability must fail instead of verifying as its trimmed alias");
    assert!(error.to_string().contains("invalid capability"));

    let policy_reason_payload = encode_agent_policy_revoke_provenance_payload(
        apartment_name.as_ref(),
        "agent.autonomy.run",
        None,
    )?;
    let error = iroha_data_model::isi::InstructionBox::from(isi::RevokeSoracloudAgentPolicy {
        apartment_name: apartment_name.clone(),
        capability: "agent.autonomy.run".to_owned(),
        reason: Some(" \t ".to_owned()),
        provenance: provenance_for(policy_reason_payload),
    })
    .execute(&ALICE_ID, &mut stx)
    .expect_err("blank optional reason must fail instead of becoming None");
    assert!(error.to_string().contains("reason must not be empty"));

    let mailbox_payload = encode_agent_message_send_provenance_payload(
        apartment_name.as_ref(),
        apartment_name.as_ref(),
        "ops.sync",
        "body",
    )?;
    let error = iroha_data_model::isi::InstructionBox::from(isi::EnqueueSoracloudAgentMessage {
        from_apartment: apartment_name.clone(),
        to_apartment: apartment_name.clone(),
        channel: " ops.sync ".to_owned(),
        payload: " body ".to_owned(),
        provenance: provenance_for(mailbox_payload),
    })
    .execute(&ALICE_ID, &mut stx)
    .expect_err("padded channel must fail instead of verifying as its trimmed alias");
    assert!(error.to_string().contains("surrounding whitespace"));

    let mailbox_payload = encode_agent_message_send_provenance_payload(
        apartment_name.as_ref(),
        apartment_name.as_ref(),
        "ops.sync",
        "body",
    )?;
    let error = iroha_data_model::isi::InstructionBox::from(isi::EnqueueSoracloudAgentMessage {
        from_apartment: apartment_name.clone(),
        to_apartment: apartment_name.clone(),
        channel: "ops.sync".to_owned(),
        payload: " body ".to_owned(),
        provenance: provenance_for(mailbox_payload),
    })
    .execute(&ALICE_ID, &mut stx)
    .expect_err("free-form payload bytes must not be trimmed before verification");
    assert!(error.to_string().contains("signature verification failed"));

    let ack_payload = encode_agent_message_ack_provenance_payload(
        apartment_name.as_ref(),
        "missing_agent:mail:1",
    )?;
    let error =
        iroha_data_model::isi::InstructionBox::from(isi::AcknowledgeSoracloudAgentMessage {
            apartment_name: apartment_name.clone(),
            message_id: " missing_agent:mail:1 ".to_owned(),
            provenance: provenance_for(ack_payload),
        })
        .execute(&ALICE_ID, &mut stx)
        .expect_err("padded message id must fail instead of looking up its trimmed alias");
    assert!(error.to_string().contains("whitespace"));

    let artifact_payload = encode_agent_artifact_allow_provenance_payload(
        apartment_name.as_ref(),
        "hash:artifact#1",
        None,
    )?;
    let error =
        iroha_data_model::isi::InstructionBox::from(isi::AllowSoracloudAgentAutonomyArtifact {
            apartment_name: apartment_name.clone(),
            artifact_hash: " hash:artifact#1 ".to_owned(),
            provenance_hash: None,
            provenance: provenance_for(artifact_payload),
        })
        .execute(&ALICE_ID, &mut stx)
        .expect_err("padded artifact hash must fail instead of verifying as its trimmed alias");
    assert!(error.to_string().contains("whitespace"));

    let autonomy_payload = encode_agent_autonomy_run_provenance_payload(
        apartment_name.as_ref(),
        "hash:artifact#1",
        None,
        1,
        "nightly",
        None,
    )?;
    let error = iroha_data_model::isi::InstructionBox::from(isi::RunSoracloudAgentAutonomy {
        apartment_name: apartment_name.clone(),
        artifact_hash: "hash:artifact#1".to_owned(),
        provenance_hash: None,
        budget_units: 1,
        run_label: " nightly ".to_owned(),
        workflow_input_json: None,
        provenance: provenance_for(autonomy_payload),
    })
    .execute(&ALICE_ID, &mut stx)
    .expect_err("padded run label must fail instead of verifying as its trimmed alias");
    assert!(error.to_string().contains("surrounding whitespace"));

    let noncanonical_workflow_json = "{ \"b\": 2, \"a\": 1 }";
    let canonical_workflow_json = "{\"a\":1,\"b\":2}";
    let autonomy_payload = encode_agent_autonomy_run_provenance_payload(
        apartment_name.as_ref(),
        "hash:artifact#1",
        None,
        1,
        "nightly",
        Some(canonical_workflow_json),
    )?;
    let error = iroha_data_model::isi::InstructionBox::from(isi::RunSoracloudAgentAutonomy {
        apartment_name: apartment_name.clone(),
        artifact_hash: "hash:artifact#1".to_owned(),
        provenance_hash: None,
        budget_units: 1,
        run_label: "nightly".to_owned(),
        workflow_input_json: Some(noncanonical_workflow_json.to_owned()),
        provenance: provenance_for(autonomy_payload),
    })
    .execute(&ALICE_ID, &mut stx)
    .expect_err("noncanonical workflow JSON must fail even when its canonical form was signed");
    assert!(
        error
            .to_string()
            .contains("canonical Norito JSON serialization")
    );

    let error =
        iroha_data_model::isi::InstructionBox::from(isi::RecordSoracloudAgentAutonomyExecution {
            apartment_name,
            run_id: " missing_agent:autonomy:1 ".to_owned(),
            process_generation: 1,
            succeeded: true,
            result_commitment: Hash::new(b"unused-result"),
            service_name: None,
            service_version: None,
            handler_name: None,
            runtime_receipt_id: None,
            journal_artifact_hash: None,
            checkpoint_artifact_hash: None,
            error: None,
        })
        .execute(&ALICE_ID, &mut stx)
        .expect_err("padded run id must fail instead of looking up its trimmed alias");
    assert!(error.to_string().contains("whitespace"));
    assert!(stx.world.soracloud_agent_apartments.iter().next().is_none());
    assert!(
        stx.world
            .soracloud_agent_apartment_audit_events
            .iter()
            .next()
            .is_none()
    );
    Ok(())
}

#[test]
fn agent_apartment_lifecycle_instructions_record_authoritative_state() -> Result<(), eyre::Report> {
    let kura = Kura::blank_kura_for_testing();
    let state = state_with_soracloud_permission(&kura)?;
    let manifest = sample_agent_manifest_with_capabilities("ops_agent", &["agent.autonomy.run"]);
    let block_header = ValidBlock::new_dummy(&checked_keypair().into_parts().1)
        .as_ref()
        .header();
    let mut state_block = state.block(block_header);
    let mut stx = state_block.transaction();
    iroha_data_model::isi::InstructionBox::from(isi::DeploySoracloudAgentApartment {
        manifest: manifest.clone(),
        lease_ticks: 120,
        autonomy_budget_units: 500,
        provenance: agent_deploy_provenance(manifest, 120, 500),
    })
    .execute(&ALICE_ID, &mut stx)?;
    let apartment_name: iroha_data_model::name::Name = "ops_agent".parse().expect("valid");
    let renew_payload = encode_agent_lease_renew_provenance_payload(apartment_name.as_ref(), 60)
        .expect("renew payload");
    iroha_data_model::isi::InstructionBox::from(isi::RenewSoracloudAgentLease {
        apartment_name: apartment_name.clone(),
        lease_ticks: 60,
        provenance: ManifestProvenance {
            signer: ALICE_KEYPAIR.public_key().clone(),
            signature: checked_signature(ALICE_KEYPAIR.private_key(), &renew_payload),
        },
    })
    .execute(&ALICE_ID, &mut stx)?;
    let restart_reason = " manual-restart ";
    let restart_payload =
        encode_agent_restart_provenance_payload(apartment_name.as_ref(), restart_reason)
            .expect("restart payload");
    iroha_data_model::isi::InstructionBox::from(isi::RestartSoracloudAgentApartment {
        apartment_name: apartment_name.clone(),
        reason: restart_reason.to_string(),
        provenance: ManifestProvenance {
            signer: ALICE_KEYPAIR.public_key().clone(),
            signature: checked_signature(ALICE_KEYPAIR.private_key(), &restart_payload),
        },
    })
    .execute(&ALICE_ID, &mut stx)?;
    let revoke_reason = " manual-review ";
    let revoke_payload = encode_agent_policy_revoke_provenance_payload(
        apartment_name.as_ref(),
        "agent.autonomy.run",
        Some(revoke_reason),
    )
    .expect("revoke payload");
    iroha_data_model::isi::InstructionBox::from(isi::RevokeSoracloudAgentPolicy {
        apartment_name: apartment_name.clone(),
        capability: "agent.autonomy.run".to_string(),
        reason: Some(revoke_reason.to_string()),
        provenance: ManifestProvenance {
            signer: ALICE_KEYPAIR.public_key().clone(),
            signature: checked_signature(ALICE_KEYPAIR.private_key(), &revoke_payload),
        },
    })
    .execute(&ALICE_ID, &mut stx)?;
    stx.apply();
    state_block.commit()?;
    let view = state.view();
    let world = view.world();
    let record = world
        .soracloud_agent_apartments()
        .get("ops_agent")
        .expect("apartment record");
    assert_eq!(record.restart_count, 1);
    assert_eq!(record.process_generation, 2);
    assert_eq!(record.last_restart_reason.as_deref(), Some(restart_reason));
    assert!(
        record
            .revoked_policy_capabilities
            .contains("agent.autonomy.run"),
        "policy capability should be revoked"
    );
    let audit_actions = world
        .soracloud_agent_apartment_audit_events()
        .iter()
        .map(|(_sequence, event)| event.action)
        .collect::<Vec<_>>();
    assert_eq!(
        audit_actions,
        vec![
            SoraAgentApartmentActionV1::Deploy,
            SoraAgentApartmentActionV1::LeaseRenew,
            SoraAgentApartmentActionV1::Restart,
            SoraAgentApartmentActionV1::PolicyRevoked,
        ]
    );
    let policy_event = world
        .soracloud_agent_apartment_audit_events()
        .iter()
        .map(|(_sequence, event)| event)
        .find(|event| event.action == SoraAgentApartmentActionV1::PolicyRevoked)
        .expect("policy revoke audit event");
    assert_eq!(policy_event.reason.as_deref(), Some(revoke_reason));
    Ok(())
}
#[test]
fn agent_wallet_mailbox_and_autonomy_instructions_record_authoritative_state()
-> Result<(), eyre::Report> {
    let kura = Kura::blank_kura_for_testing();
    let state = state_with_soracloud_permission(&kura)?;
    let ops_manifest = sample_agent_manifest_with_capabilities(
        "ops_agent",
        &[
            "wallet.sign",
            "agent.mailbox.send",
            "agent.autonomy.allow",
            "agent.autonomy.run",
        ],
    );
    let worker_manifest =
        sample_agent_manifest_with_capabilities("worker_agent", &["agent.mailbox.receive"]);
    let block_header = ValidBlock::new_dummy(&checked_keypair().into_parts().1)
        .as_ref()
        .header();
    let mut state_block = state.block(block_header);
    let mut stx = state_block.transaction();
    let wallet_asset_definition_id: AssetDefinitionId = "61CtjvNd9T3THAR65GsMVHr82Bjc"
        .parse()
        .expect("canonical wallet asset definition");
    Register::asset_definition(AssetDefinition::numeric(
        wallet_asset_definition_id.clone(),
        "xor".to_string(),
        iroha_data_model::asset::AssetBalancePolicy::Global,
        None,
    ))
    .execute(&SAMPLE_GENESIS_ACCOUNT_ID, &mut stx)?;
    iroha_data_model::isi::InstructionBox::from(isi::DeploySoracloudAgentApartment {
        manifest: ops_manifest.clone(),
        lease_ticks: 120,
        autonomy_budget_units: 500,
        provenance: agent_deploy_provenance(ops_manifest, 120, 500),
    })
    .execute(&ALICE_ID, &mut stx)?;
    iroha_data_model::isi::InstructionBox::from(isi::DeploySoracloudAgentApartment {
        manifest: worker_manifest.clone(),
        lease_ticks: 120,
        autonomy_budget_units: 250,
        provenance: agent_deploy_provenance(worker_manifest, 120, 250),
    })
    .execute(&ALICE_ID, &mut stx)?;
    let ops_name: iroha_data_model::name::Name = "ops_agent".parse().expect("valid");
    let worker_name: iroha_data_model::name::Name = "worker_agent".parse().expect("valid");
    let wallet_amount: Quantity = "0.001".parse().expect("wallet amount");
    let wallet_request_id = "ops-wallet-request-1";
    let wallet_spend_payload = encode_agent_wallet_spend_provenance_payload(
        ops_name.as_ref(),
        wallet_request_id,
        "61CtjvNd9T3THAR65GsMVHr82Bjc",
        &wallet_amount,
    )
    .expect("wallet spend payload");
    let wallet_spend_instruction =
        iroha_data_model::isi::InstructionBox::from(isi::RequestSoracloudAgentWalletSpend {
            apartment_name: ops_name.clone(),
            request_id: wallet_request_id.to_owned(),
            asset_definition: "61CtjvNd9T3THAR65GsMVHr82Bjc".to_string(),
            amount: wallet_amount.clone(),
            provenance: ManifestProvenance {
                signer: ALICE_KEYPAIR.public_key().clone(),
                signature: checked_signature(ALICE_KEYPAIR.private_key(), &wallet_spend_payload),
            },
        });
    wallet_spend_instruction
        .clone()
        .execute(&ALICE_ID, &mut stx)?;
    let audit_count_after_request = stx
        .world
        .soracloud_agent_apartment_audit_events
        .iter()
        .count();
    let pending_replay_error = wallet_spend_instruction
        .clone()
        .execute(&ALICE_ID, &mut stx)
        .expect_err("pending wallet request ID replay must fail closed");
    assert!(
        pending_replay_error
            .to_string()
            .contains("has already been used"),
        "unexpected pending replay rejection: {pending_replay_error}"
    );
    assert_eq!(
        stx.world
            .soracloud_agent_apartment_audit_events
            .iter()
            .count(),
        audit_count_after_request,
        "rejected pending replay must not append an audit event"
    );
    let wallet_approve_payload =
        encode_agent_wallet_approve_provenance_payload(ops_name.as_ref(), wallet_request_id)
            .expect("wallet approve payload");
    iroha_data_model::isi::InstructionBox::from(isi::ApproveSoracloudAgentWalletSpend {
        apartment_name: ops_name.clone(),
        request_id: wallet_request_id.to_owned(),
        provenance: ManifestProvenance {
            signer: ALICE_KEYPAIR.public_key().clone(),
            signature: checked_signature(ALICE_KEYPAIR.private_key(), &wallet_approve_payload),
        },
    })
    .execute(&ALICE_ID, &mut stx)?;
    let audit_count_after_approval = stx
        .world
        .soracloud_agent_apartment_audit_events
        .iter()
        .count();
    let historical_replay_error = wallet_spend_instruction
        .execute(&ALICE_ID, &mut stx)
        .expect_err("approved wallet request ID replay must fail closed");
    assert!(
        historical_replay_error
            .to_string()
            .contains("has already been used"),
        "unexpected historical replay rejection: {historical_replay_error}"
    );
    assert_eq!(
        stx.world
            .soracloud_agent_apartment_audit_events
            .iter()
            .count(),
        audit_count_after_approval,
        "rejected historical replay must not append an audit event"
    );
    let mailbox_payload = " rotate-key-42 ";
    let message_send_payload = encode_agent_message_send_provenance_payload(
        ops_name.as_ref(),
        worker_name.as_ref(),
        "ops.sync",
        mailbox_payload,
    )
    .expect("message send payload");
    iroha_data_model::isi::InstructionBox::from(isi::EnqueueSoracloudAgentMessage {
        from_apartment: ops_name.clone(),
        to_apartment: worker_name.clone(),
        channel: "ops.sync".to_string(),
        payload: mailbox_payload.to_string(),
        provenance: ManifestProvenance {
            signer: ALICE_KEYPAIR.public_key().clone(),
            signature: checked_signature(ALICE_KEYPAIR.private_key(), &message_send_payload),
        },
    })
    .execute(&ALICE_ID, &mut stx)?;
    let message_ack_payload =
        encode_agent_message_ack_provenance_payload(worker_name.as_ref(), "worker_agent:mail:5")
            .expect("message ack payload");
    iroha_data_model::isi::InstructionBox::from(isi::AcknowledgeSoracloudAgentMessage {
        apartment_name: worker_name.clone(),
        message_id: "worker_agent:mail:5".to_string(),
        provenance: ManifestProvenance {
            signer: ALICE_KEYPAIR.public_key().clone(),
            signature: checked_signature(ALICE_KEYPAIR.private_key(), &message_ack_payload),
        },
    })
    .execute(&ALICE_ID, &mut stx)?;
    let artifact_allow_payload = encode_agent_artifact_allow_provenance_payload(
        ops_name.as_ref(),
        "hash:artifact#1",
        Some("hash:prov#1"),
    )
    .expect("artifact allow payload");
    iroha_data_model::isi::InstructionBox::from(isi::AllowSoracloudAgentAutonomyArtifact {
        apartment_name: ops_name.clone(),
        artifact_hash: "hash:artifact#1".to_string(),
        provenance_hash: Some("hash:prov#1".to_string()),
        provenance: ManifestProvenance {
            signer: ALICE_KEYPAIR.public_key().clone(),
            signature: checked_signature(ALICE_KEYPAIR.private_key(), &artifact_allow_payload),
        },
    })
    .execute(&ALICE_ID, &mut stx)?;
    let canonical_workflow_input_json =
        "{\"inputs\":{\"messages\":[{\"content\":\"nightly-batch-1\",\"role\":\"user\"}]}}";
    let autonomy_run_payload = encode_agent_autonomy_run_provenance_payload(
        ops_name.as_ref(),
        "hash:artifact#1",
        Some("hash:prov#1"),
        120,
        "nightly-batch-1",
        Some(canonical_workflow_input_json),
    )
    .expect("autonomy run payload");
    iroha_data_model::isi::InstructionBox::from(isi::RunSoracloudAgentAutonomy {
        apartment_name: ops_name,
        artifact_hash: "hash:artifact#1".to_string(),
        provenance_hash: Some("hash:prov#1".to_string()),
        budget_units: 120,
        run_label: "nightly-batch-1".to_string(),
        workflow_input_json: Some(canonical_workflow_input_json.to_string()),
        provenance: ManifestProvenance {
            signer: ALICE_KEYPAIR.public_key().clone(),
            signature: checked_signature(ALICE_KEYPAIR.private_key(), &autonomy_run_payload),
        },
    })
    .execute(&ALICE_ID, &mut stx)?;
    stx.apply();
    state_block.commit()?;
    let view = state.view();
    let world = view.world();
    let ops_record = world
        .soracloud_agent_apartments()
        .get("ops_agent")
        .expect("ops apartment");
    assert!(ops_record.pending_wallet_requests.is_empty());
    assert_eq!(
        ops_record
            .wallet_daily_spend
            .get("61CtjvNd9T3THAR65GsMVHr82Bjc:0")
            .expect("wallet day aggregate")
            .spent,
        wallet_amount
    );
    assert_eq!(ops_record.autonomy_budget_remaining_units, 380);
    assert_eq!(ops_record.autonomy_run_history.len(), 1);
    assert_eq!(
        ops_record.autonomy_run_history[0]
            .workflow_input_json
            .as_deref(),
        Some(canonical_workflow_input_json)
    );
    let autonomy_event = world
        .soracloud_agent_apartment_audit_events()
        .get(&ops_record.autonomy_run_history[0].approved_sequence)
        .expect("autonomy audit event");
    assert_eq!(
        autonomy_event.payload_hash,
        Some(Hash::new(canonical_workflow_input_json.as_bytes()))
    );
    assert_eq!(ops_record.checkpoint_count, 1);
    assert_eq!(ops_record.last_checkpoint_sequence, Some(8));
    assert_eq!(ops_record.artifact_allowlist.len(), 1);
    let mailbox_event = world
        .soracloud_agent_apartment_audit_events()
        .iter()
        .map(|(_sequence, event)| event)
        .find(|event| event.action == SoraAgentApartmentActionV1::MessageEnqueued)
        .expect("mailbox enqueue audit event");
    assert_eq!(
        mailbox_event.payload_hash,
        Some(Hash::new(mailbox_payload.as_bytes()))
    );
    let worker_record = world
        .soracloud_agent_apartments()
        .get("worker_agent")
        .expect("worker apartment");
    assert!(worker_record.mailbox_queue.is_empty());
    assert_eq!(
        world
            .soracloud_agent_apartment_audit_events()
            .iter()
            .count(),
        8
    );
    Ok(())
}
#[test]
fn auto_approved_agent_wallet_request_id_cannot_be_replayed() -> Result<(), eyre::Report> {
    let kura = Kura::blank_kura_for_testing();
    let state = state_with_soracloud_permission(&kura)?;
    let manifest = sample_agent_manifest_with_capabilities(
        "auto_wallet_agent",
        &["wallet.sign", "wallet.auto_approve"],
    );
    let block_header = ValidBlock::new_dummy(&checked_keypair().into_parts().1)
        .as_ref()
        .header();
    let mut state_block = state.block(block_header);
    let mut stx = state_block.transaction();
    let asset_definition_id: AssetDefinitionId = "61CtjvNd9T3THAR65GsMVHr82Bjc"
        .parse()
        .expect("canonical wallet asset definition");
    Register::asset_definition(AssetDefinition::numeric(
        asset_definition_id,
        "xor".to_string(),
        iroha_data_model::asset::AssetBalancePolicy::Global,
        None,
    ))
    .execute(&SAMPLE_GENESIS_ACCOUNT_ID, &mut stx)?;
    iroha_data_model::isi::InstructionBox::from(isi::DeploySoracloudAgentApartment {
        manifest: manifest.clone(),
        lease_ticks: 120,
        autonomy_budget_units: 500,
        provenance: agent_deploy_provenance(manifest, 120, 500),
    })
    .execute(&ALICE_ID, &mut stx)?;
    let apartment_name: iroha_data_model::name::Name =
        "auto_wallet_agent".parse().expect("valid apartment name");
    let noncanonical_asset_definition = " 61CtjvNd9T3THAR65GsMVHr82Bjc";
    let noncanonical_request_id = "auto-wallet-request-whitespace";
    let noncanonical_amount: Quantity = "0.001".parse().expect("wallet amount");
    let noncanonical_payload = encode_agent_wallet_spend_provenance_payload(
        apartment_name.as_ref(),
        noncanonical_request_id,
        noncanonical_asset_definition,
        &noncanonical_amount,
    )
    .expect("wallet spend payload");
    let noncanonical_instruction =
        iroha_data_model::isi::InstructionBox::from(isi::RequestSoracloudAgentWalletSpend {
            apartment_name: apartment_name.clone(),
            request_id: noncanonical_request_id.to_owned(),
            asset_definition: noncanonical_asset_definition.to_owned(),
            amount: noncanonical_amount,
            provenance: ManifestProvenance {
                signer: ALICE_KEYPAIR.public_key().clone(),
                signature: checked_signature(ALICE_KEYPAIR.private_key(), &noncanonical_payload),
            },
        });
    let error = noncanonical_instruction
        .execute(&ALICE_ID, &mut stx)
        .expect_err("surrounding asset-definition whitespace must fail closed");
    assert!(
        error.to_string().contains("surrounding whitespace"),
        "unexpected asset-definition rejection: {error}"
    );
    let request_id = "auto-wallet-request-1";
    let amount: Quantity = "0.001".parse().expect("wallet amount");
    let payload = encode_agent_wallet_spend_provenance_payload(
        apartment_name.as_ref(),
        request_id,
        "61CtjvNd9T3THAR65GsMVHr82Bjc",
        &amount,
    )
    .expect("wallet spend payload");
    let instruction =
        iroha_data_model::isi::InstructionBox::from(isi::RequestSoracloudAgentWalletSpend {
            apartment_name,
            request_id: request_id.to_owned(),
            asset_definition: "61CtjvNd9T3THAR65GsMVHr82Bjc".to_owned(),
            amount,
            provenance: ManifestProvenance {
                signer: ALICE_KEYPAIR.public_key().clone(),
                signature: checked_signature(ALICE_KEYPAIR.private_key(), &payload),
            },
        });
    instruction.clone().execute(&ALICE_ID, &mut stx)?;
    let audit_count_after_approval = stx
        .world
        .soracloud_agent_apartment_audit_events
        .iter()
        .count();
    let error = instruction
        .execute(&ALICE_ID, &mut stx)
        .expect_err("auto-approved wallet request ID replay must fail closed");
    assert!(
        error.to_string().contains("has already been used"),
        "unexpected auto-approval replay rejection: {error}"
    );
    assert_eq!(
        stx.world
            .soracloud_agent_apartment_audit_events
            .iter()
            .count(),
        audit_count_after_approval,
        "rejected auto-approval replay must not append an audit event"
    );
    Ok(())
}
#[test]
fn record_agent_autonomy_execution_records_authoritative_audit_state() -> Result<(), eyre::Report> {
    let kura = Kura::blank_kura_for_testing();
    let state = state_with_soracloud_permission(&kura)?;
    let ops_manifest = sample_agent_manifest_with_capabilities(
        "ops_agent",
        &["agent.autonomy.allow", "agent.autonomy.run"],
    );
    let block_header = ValidBlock::new_dummy(&checked_keypair().into_parts().1)
        .as_ref()
        .header();
    let mut state_block = state.block(block_header);
    let mut stx = state_block.transaction();
    iroha_data_model::isi::InstructionBox::from(isi::DeploySoracloudAgentApartment {
        manifest: ops_manifest.clone(),
        lease_ticks: 120,
        autonomy_budget_units: 500,
        provenance: agent_deploy_provenance(ops_manifest, 120, 500),
    })
    .execute(&ALICE_ID, &mut stx)?;
    let apartment_name: iroha_data_model::name::Name = "ops_agent".parse().expect("valid");
    let artifact_allow_payload = encode_agent_artifact_allow_provenance_payload(
        apartment_name.as_ref(),
        "hash:artifact#1",
        Some("hash:prov#1"),
    )
    .expect("artifact allow payload");
    iroha_data_model::isi::InstructionBox::from(isi::AllowSoracloudAgentAutonomyArtifact {
        apartment_name: apartment_name.clone(),
        artifact_hash: "hash:artifact#1".to_string(),
        provenance_hash: Some("hash:prov#1".to_string()),
        provenance: ManifestProvenance {
            signer: ALICE_KEYPAIR.public_key().clone(),
            signature: checked_signature(ALICE_KEYPAIR.private_key(), &artifact_allow_payload),
        },
    })
    .execute(&ALICE_ID, &mut stx)?;
    let workflow_input_json = "{\"inputs\":\"nightly\"}";
    let autonomy_run_payload = encode_agent_autonomy_run_provenance_payload(
        apartment_name.as_ref(),
        "hash:artifact#1",
        Some("hash:prov#1"),
        120,
        "nightly",
        Some(workflow_input_json),
    )
    .expect("autonomy run payload");
    iroha_data_model::isi::InstructionBox::from(isi::RunSoracloudAgentAutonomy {
        apartment_name: apartment_name.clone(),
        artifact_hash: "hash:artifact#1".to_string(),
        provenance_hash: Some("hash:prov#1".to_string()),
        budget_units: 120,
        run_label: "nightly".to_string(),
        workflow_input_json: Some(workflow_input_json.to_string()),
        provenance: ManifestProvenance {
            signer: ALICE_KEYPAIR.public_key().clone(),
            signature: checked_signature(ALICE_KEYPAIR.private_key(), &autonomy_run_payload),
        },
    })
    .execute(&ALICE_ID, &mut stx)?;
    let approved_run = stx
        .world
        .soracloud_agent_apartments
        .get("ops_agent")
        .expect("ops apartment in transaction")
        .autonomy_run_history
        .last()
        .cloned()
        .expect("approved run");
    let result_commitment = Hash::new(b"ops-agent-runtime-result");
    let runtime_receipt_id = Hash::new(b"ops-agent-runtime-receipt");
    let journal_artifact_hash = Hash::new(b"ops-agent-runtime-journal");
    let checkpoint_artifact_hash = Hash::new(b"ops-agent-runtime-checkpoint");
    let service_name: iroha_data_model::name::Name =
        "hf_agent_service".parse().expect("valid service name");
    let handler_name: iroha_data_model::name::Name = "infer".parse().expect("valid handler");
    iroha_data_model::isi::InstructionBox::from(isi::RecordSoracloudAgentAutonomyExecution {
        apartment_name,
        run_id: approved_run.run_id.clone(),
        process_generation: approved_run.approved_process_generation,
        succeeded: true,
        result_commitment,
        service_name: Some(service_name),
        service_version: Some("hf.generated.v1".to_string()),
        handler_name: Some(handler_name),
        runtime_receipt_id: Some(runtime_receipt_id),
        journal_artifact_hash: Some(journal_artifact_hash),
        checkpoint_artifact_hash: Some(checkpoint_artifact_hash),
        error: None,
    })
    .execute(&ALICE_ID, &mut stx)?;
    stx.apply();
    state_block.commit()?;
    let view = state.view();
    let world = view.world();
    let record = world
        .soracloud_agent_apartments()
        .get("ops_agent")
        .expect("ops apartment");
    let event = world
        .soracloud_agent_apartment_audit_events()
        .get(&record.last_active_sequence)
        .expect("execution audit event");
    assert_eq!(
        event.schema_version,
        iroha_data_model::soracloud::SORA_AGENT_APARTMENT_AUDIT_EVENT_VERSION_V1
    );
    assert_eq!(
        event.action,
        iroha_data_model::soracloud::SoraAgentApartmentActionV1::AutonomyRunExecuted
    );
    assert_eq!(event.run_id.as_deref(), Some(approved_run.run_id.as_str()));
    assert_eq!(
        event.request_id.as_deref(),
        Some(approved_run.run_id.as_str())
    );
    assert_eq!(event.result_commitment, Some(result_commitment));
    assert_eq!(event.runtime_receipt_id, Some(runtime_receipt_id));
    assert_eq!(event.journal_artifact_hash, Some(journal_artifact_hash));
    assert_eq!(
        event.checkpoint_artifact_hash,
        Some(checkpoint_artifact_hash)
    );
    assert_eq!(event.succeeded, Some(true));
    assert_eq!(event.service_name.as_deref(), Some("hf_agent_service"));
    assert_eq!(event.service_version.as_deref(), Some("hf.generated.v1"));
    assert_eq!(event.handler_name.as_deref(), Some("infer"));
    assert_eq!(
        world
            .soracloud_agent_apartment_audit_events()
            .iter()
            .count(),
        4
    );
    Ok(())
}
