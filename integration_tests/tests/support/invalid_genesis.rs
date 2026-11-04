//! Signed adversarial genesis fixtures whose retained outputs cannot authorize invalid inputs.

use std::{borrow::Cow, time::Duration};

use eyre::{Result, ensure};
use futures_util::future::try_join_all;
use integration_tests::sandbox::SerializedNetwork;
use iroha_config_base::toml::WriteExt as _;
use iroha_data_model::{
    block::{builder::BlockBuilder, output_budget::ExecutionOutputLimits},
    prelude::*,
};
use iroha_genesis::GenesisBlock;
use iroha_test_network::PeerLifecycleEvent;
use iroha_test_samples::SAMPLE_GENESIS_ACCOUNT_KEYPAIR;

/// Require every validator to reject a signed instruction mutation for its exact semantic cause.
pub(super) async fn assert_rejected_by_all_peers(
    network: &SerializedNetwork,
    placeholder: &InstructionBox,
    invalid_instruction: InstructionBox,
    expected_cause: &str,
    startup_timeout: Duration,
) -> Result<()> {
    let original = network.genesis();
    let invalid = replace_instruction(&original, placeholder, invalid_instruction)?;
    let authority = original
        .0
        .external_transactions()
        .next()
        .expect("genesis has external transactions")
        .authority()
        .clone();
    // The intended refusal must come from execution, not malformed signatures,
    // missing results, mismatched commitments, or an unrelated startup setting.
    iroha_core::validate_genesis_block(&invalid.0, &authority)?;
    let override_layer = toml::Table::new().write(
        ["genesis", "expected_hash"],
        norito::literal::format("hash", &invalid.0.hash().to_string().to_ascii_uppercase()),
    );
    assert_eq!(network.peers().len(), 4);
    let exits = network
        .peers()
        .iter()
        .map(|peer| {
            let mut events = peer.events();
            async move {
                loop {
                    match events.recv().await? {
                        PeerLifecycleEvent::Terminated { status } => {
                            ensure!(
                                !status.success(),
                                "invalid genesis must exit unsuccessfully"
                            );
                            return Ok::<_, eyre::Report>(());
                        }
                        PeerLifecycleEvent::ServerStarted => {
                            eyre::bail!("invalid genesis must not start a validator server");
                        }
                        PeerLifecycleEvent::Killed => {
                            eyre::bail!("validator must reject genesis without being killed");
                        }
                        _ => {}
                    }
                }
            }
        })
        .collect::<Vec<_>>();
    tokio::time::timeout(startup_timeout, async {
        for peer in network.peers() {
            peer.start(
                network
                    .config_layers_for_peer(peer)
                    .chain(std::iter::once(Cow::Borrowed(&override_layer))),
                Some(&invalid),
            )
            .await?;
        }
        try_join_all(exits).await
    })
    .await??;
    for peer in network.peers() {
        let stderr_path = peer
            .latest_stderr_log_path()
            .expect("failed validator startup must retain stderr");
        let stderr = tokio::fs::read_to_string(&stderr_path).await?;
        let stdout_path = peer
            .latest_stdout_log_path()
            .expect("failed validator startup must retain stdout");
        let stdout = tokio::fs::read_to_string(&stdout_path).await?;
        let diagnostic = format!("{stderr}\n{stdout}");
        ensure!(
            diagnostic.contains("genesis validation failed: Invalid genesis block: Genesis execution output rejected")
                && diagnostic.contains(expected_cause),
            "validator {} must reject the exact invalid genesis instruction: {diagnostic}",
            peer.mnemonic(),
        );
    }
    Ok(())
}

fn replace_instruction(
    original: &GenesisBlock,
    placeholder: &InstructionBox,
    invalid_instruction: InstructionBox,
) -> Result<GenesisBlock> {
    let original = &original.0;
    let mut builder = BlockBuilder::new(original.header());
    builder.set_da_commitments(original.da_commitments().cloned());
    builder.set_da_proof_policies(original.da_proof_policies().cloned());
    builder.set_da_pin_intents(original.da_pin_intents().cloned());
    builder.set_npos_consensus_effects(original.npos_consensus_effects().cloned());
    builder.set_global_beacon_pulse(original.global_beacon_pulse().cloned());
    builder.set_execution_context(original.execution_context().cloned());
    let mut replacements = 0;
    for transaction in original.external_transactions() {
        let Executable::Instructions(instructions) = transaction.instructions() else {
            eyre::bail!("genesis fixture must contain instruction transactions");
        };
        if instructions
            .iter()
            .any(|instruction| instruction == placeholder)
        {
            let mut payload = transaction.payload().clone();
            payload.instructions = instructions
                .iter()
                .map(|instruction| {
                    if instruction == placeholder {
                        replacements += 1;
                        invalid_instruction.clone()
                    } else {
                        instruction.clone()
                    }
                })
                .collect::<Vec<_>>()
                .into();
            builder.push_transaction(
                TransactionBuilder::from_genesis_payload(payload)?
                    .try_sign(SAMPLE_GENESIS_ACCOUNT_KEYPAIR.private_key())?,
            );
        } else {
            builder.push_transaction(transaction.clone());
        }
    }
    ensure!(
        replacements == 1,
        "fixture must replace exactly one placeholder"
    );
    let mut invalid =
        builder.try_build_with_signature(0, SAMPLE_GENESIS_ACCOUNT_KEYPAIR.private_key())?;
    // Retain actual outputs from the valid original, deliberately stale for the
    // mutated input. Structural validity must not substitute for re-execution.
    let byte_limit = u64::try_from(iroha_genesis::SIGNED_GENESIS_MAX_BYTES_V1)?;
    invalid.set_execution_outputs(
        original.execution_outputs().to_vec(),
        original
            .committed_fragment_count()
            .expect("executed genesis"),
        original.fastpq_transcripts().clone(),
        original.axt_envelopes().expect("executed genesis").to_vec(),
        original
            .axt_policy_snapshot()
            .expect("executed genesis")
            .clone(),
        original
            .axt_transitioned_dataspaces()
            .expect("executed genesis")
            .clone(),
        &ExecutionOutputLimits {
            max_outputs: u32::try_from(original.execution_outputs().len())?,
            max_output_bytes: byte_limit,
            max_total_output_bytes: byte_limit,
            max_executed_wire_bytes: byte_limit,
        },
    )?;
    Ok(GenesisBlock(invalid))
}
