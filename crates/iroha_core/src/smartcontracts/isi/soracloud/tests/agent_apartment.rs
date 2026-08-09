//! Authoritative SoraCloud agent-apartment ledger transition tests.

use super::*;
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

    let restart_payload =
        encode_agent_restart_provenance_payload(apartment_name.as_ref(), "manual-restart")
            .expect("restart payload");
    iroha_data_model::isi::InstructionBox::from(isi::RestartSoracloudAgentApartment {
        apartment_name: apartment_name.clone(),
        reason: "manual-restart".to_string(),
        provenance: ManifestProvenance {
            signer: ALICE_KEYPAIR.public_key().clone(),
            signature: checked_signature(ALICE_KEYPAIR.private_key(), &restart_payload),
        },
    })
    .execute(&ALICE_ID, &mut stx)?;

    let revoke_payload = encode_agent_policy_revoke_provenance_payload(
        apartment_name.as_ref(),
        "agent.autonomy.run",
        Some("manual-review"),
    )
    .expect("revoke payload");
    iroha_data_model::isi::InstructionBox::from(isi::RevokeSoracloudAgentPolicy {
        apartment_name: apartment_name.clone(),
        capability: "agent.autonomy.run".to_string(),
        reason: Some("manual-review".to_string()),
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
    assert_eq!(
        record.last_restart_reason.as_deref(),
        Some("manual-restart")
    );
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

    let wallet_spend_payload = encode_agent_wallet_spend_provenance_payload(
        ops_name.as_ref(),
        "61CtjvNd9T3THAR65GsMVHr82Bjc",
        &wallet_amount,
    )
    .expect("wallet spend payload");
    iroha_data_model::isi::InstructionBox::from(isi::RequestSoracloudAgentWalletSpend {
        apartment_name: ops_name.clone(),
        asset_definition: "61CtjvNd9T3THAR65GsMVHr82Bjc".to_string(),
        amount: wallet_amount.clone(),
        provenance: ManifestProvenance {
            signer: ALICE_KEYPAIR.public_key().clone(),
            signature: checked_signature(ALICE_KEYPAIR.private_key(), &wallet_spend_payload),
        },
    })
    .execute(&ALICE_ID, &mut stx)?;

    let wallet_approve_payload =
        encode_agent_wallet_approve_provenance_payload(ops_name.as_ref(), "ops_agent:wallet:3")
            .expect("wallet approve payload");
    iroha_data_model::isi::InstructionBox::from(isi::ApproveSoracloudAgentWalletSpend {
        apartment_name: ops_name.clone(),
        request_id: "ops_agent:wallet:3".to_string(),
        provenance: ManifestProvenance {
            signer: ALICE_KEYPAIR.public_key().clone(),
            signature: checked_signature(ALICE_KEYPAIR.private_key(), &wallet_approve_payload),
        },
    })
    .execute(&ALICE_ID, &mut stx)?;

    let message_send_payload = encode_agent_message_send_provenance_payload(
        ops_name.as_ref(),
        worker_name.as_ref(),
        "ops.sync",
        "rotate-key-42",
    )
    .expect("message send payload");
    iroha_data_model::isi::InstructionBox::from(isi::EnqueueSoracloudAgentMessage {
        from_apartment: ops_name.clone(),
        to_apartment: worker_name.clone(),
        channel: "ops.sync".to_string(),
        payload: "rotate-key-42".to_string(),
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

    let autonomy_run_payload = encode_agent_autonomy_run_provenance_payload(
        ops_name.as_ref(),
        "hash:artifact#1",
        Some("hash:prov#1"),
        120,
        "nightly-batch-1",
        Some("{\"inputs\":{\"messages\":[{\"role\":\"user\",\"content\":\"nightly-batch-1\"}]}}"),
    )
    .expect("autonomy run payload");
    iroha_data_model::isi::InstructionBox::from(isi::RunSoracloudAgentAutonomy {
        apartment_name: ops_name,
        artifact_hash: "hash:artifact#1".to_string(),
        provenance_hash: Some("hash:prov#1".to_string()),
        budget_units: 120,
        run_label: "nightly-batch-1".to_string(),
        workflow_input_json: Some(
            "{\"inputs\":{\"messages\":[{\"role\":\"user\",\"content\":\"nightly-batch-1\"}]}}"
                .to_string(),
        ),
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
    let canonical_workflow_input_json =
        "{\"inputs\":{\"messages\":[{\"content\":\"nightly-batch-1\",\"role\":\"user\"}]}}";
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
