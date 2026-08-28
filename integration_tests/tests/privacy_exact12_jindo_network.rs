#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! Four-peer lifecycle, admission, replay, and restart coverage for the
//! canonical native Jindo direct action.
use eyre::{Result, WrapErr as _, ensure, eyre};
use futures_util::TryStreamExt as _;
use integration_tests::sandbox;
use iroha::client::Client;
use iroha_core::{
    privacy::PRIVACY_MIN_ACTIVATION_DELAY_BLOCKS_V1,
    privacy_engines::jindo::{
        JindoPrivacyActionEffectV1, JindoPrivacyActionTransactionContextV1,
        JindoPrivacyActionWitnessV1, prepare_jindo_privacy_action_v1,
        sign_prepared_jindo_privacy_action_v1,
    },
    privacy_profiles::{CompiledPrivacyProfileV1, compiled_privacy_profile_v1},
};
use iroha_data_model::{
    Level,
    isi::{
        Grant, InstructionBox, Log,
        privacy::{RegisterPrivacyProtocolActivationV1, SubmitPrivacyProofV1},
    },
    metadata::Metadata,
    permission::Permission,
    privacy::{
        PrivacyCapabilityLimitationV1, PrivacyCapabilityReadinessV1,
        PrivacyCompiledProfileResultV1, PrivacyCompiledProfileSnapshotV1, PrivacyConsensusLimitsV1,
        PrivacyExact12CapabilityManifestV1, PrivacyExact12CapabilityRowV1,
        PrivacyParameterDigestV1, PrivacyProofV1, PrivacyProposedLifecycleV1,
        PrivacyProtocolActivationRecordV1, PrivacyProtocolIdV1, PrivacyProtocolLifecycleV1,
    },
    transaction::{
        FeePaymentIntent, SignedTransaction, TransactionAdmissionIntent, TransactionBuilder,
        TransactionEntrypoint,
    },
};
use iroha_executor_data_model::permission::governance::CanEnactGovernance;
use iroha_test_network::{NetworkBuilder, init_instruction_registry};
use std::{
    num::{NonZeroU32, NonZeroU64},
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tokio::time::{Instant, sleep, timeout};
const JINDO_PROTOCOL: PrivacyProtocolIdV1 = PrivacyProtocolIdV1::IrohaJindoPolynomialCommitmentV0;
const TEST_NEXUS_LOCAL_STORAGE_BUDGET_BYTES: i64 = 1024 * 1024 * 1024;
// A public transaction reaches terminal state through three separately signed
// QueuePlan carriers. Leave deterministic headroom for healthy view churn.
const SUBMISSION_TIMEOUT: Duration = Duration::from_secs(300);
// Let the client's detailed terminal status/error win over the Tokio wrapper.
const SUBMISSION_TASK_TIMEOUT: Duration = Duration::from_secs(360);
const TRANSACTION_TTL: Duration = Duration::from_secs(600);
const QUEUE_PLAN_LIFECYCLE_BLOCKS: u64 = 3;
const PEER_CONVERGENCE_TIMEOUT: Duration = Duration::from_secs(180);
const RESTART_TIMEOUT: Duration = Duration::from_secs(180);
// The signed cadence below advances 98 sequential three-carrier transactions;
// leave deterministic headroom for healthy one-second production and view churn.
const ACTIVATION_ADVANCE_TIMEOUT: Duration = Duration::from_secs(900);
// This release-evidence fixture exercises instrumented four-validator body
// reconstruction, validation, replay, and restart rather than throughput. Use
// the released one-second signed cadence so the deterministic Sumeragi view
// backoff can cover that work without changing the production timer policy.
const TEST_BLOCK_CADENCE: Duration = Duration::from_secs(1);
const POLL_INTERVAL: Duration = Duration::from_millis(200);
const CANONICAL_GENESIS_FETCH_TIMEOUT: Duration = Duration::from_secs(30);
fn bounded_client(mut client: Client) -> Client {
    client.transaction_status_timeout = SUBMISSION_TIMEOUT;
    client.transaction_ttl = Some(TRANSACTION_TTL);
    client.torii_request_timeout = Duration::from_secs(20);
    client
}
fn no_fee() -> FeePaymentIntent {
    FeePaymentIntent::authority(Vec::new(), None)
}
fn error_chain_contains(error: &eyre::Report, needle: &str) -> bool {
    let needle = needle.to_ascii_lowercase();
    error
        .chain()
        .any(|cause| cause.to_string().to_ascii_lowercase().contains(&needle))
}
fn is_exact_replay_error(error: &eyre::Report) -> bool {
    [
        "prtry:already_committed",
        "prtry:already_enqueued",
        "already_committed",
        "already_enqueued",
        "transaction already committed",
        "transaction already present in the queue",
    ]
    .iter()
    .any(|needle| error_chain_contains(error, needle))
}
fn jindo_row(
    snapshot: &PrivacyExact12CapabilityManifestV1,
) -> Result<PrivacyExact12CapabilityRowV1> {
    snapshot
        .protocols
        .iter()
        .copied()
        .find(|row| row.protocol_id == JINDO_PROTOCOL)
        .ok_or_else(|| eyre!("canonical capability snapshot omitted the Jindo row"))
}
fn assert_exact_jindo_row(
    snapshot: &PrivacyExact12CapabilityManifestV1,
    compiled: PrivacyCompiledProfileSnapshotV1,
    activation: Option<PrivacyProtocolActivationRecordV1>,
    context: &str,
) -> Result<()> {
    snapshot
        .validate()
        .wrap_err_with(|| format!("{context}: invalid capability snapshot"))?;
    let row = jindo_row(snapshot)?;
    ensure!(
        row.compiled_profile == PrivacyCompiledProfileResultV1::Available(compiled),
        "{context}: Jindo compiled binding drifted: {:?}",
        row.compiled_profile
    );
    ensure!(
        row.activation == activation,
        "{context}: Jindo activation mismatch: expected {activation:?}, got {:?}",
        row.activation
    );
    ensure!(
        row.readiness == PrivacyCapabilityReadinessV1::AvailableExperimental,
        "{context}: revised Jindo must remain explicitly available-experimental"
    );
    ensure!(
        row.limitation
            == Some(
                PrivacyCapabilityLimitationV1::MissingDistributionWideKnowledgeSoundnessEvidence,
            ),
        "{context}: revised Jindo omitted its missing knowledge-soundness evidence"
    );
    Ok(())
}
async fn canonical_genesis_hash(client: &Client) -> Result<[u8; 32]> {
    let genesis = timeout(CANONICAL_GENESIS_FETCH_TIMEOUT, async {
        let mut blocks = client
            .listen_for_blocks_async(NonZeroU64::MIN)
            .await
            .wrap_err("subscribe to canonical block replay from genesis")?;
        blocks
            .try_next()
            .await
            .wrap_err("read canonical genesis block replay")?
            .ok_or_else(|| eyre!("canonical block replay ended before genesis"))
    })
    .await
    .map_err(|_| {
        eyre!("canonical genesis block replay exceeded {CANONICAL_GENESIS_FETCH_TIMEOUT:?}")
    })??;
    ensure!(
        genesis.header().height().get() == 1 && genesis.header().prev_block_hash().is_none(),
        "canonical block replay returned a non-genesis block at height {}",
        genesis.header().height()
    );
    let hash = *genesis.header().hash().as_ref();
    ensure!(hash != [0; 32], "canonical genesis hash must be non-zero");
    Ok(hash)
}
fn committed_height(client: &Client, context: &str) -> Result<u64> {
    Ok(client
        .get_privacy_capabilities()
        .wrap_err_with(|| format!("{context}: query committed height"))?
        .committed_height)
}
fn next_queue_plan_execution_height(client: &Client) -> Result<u64> {
    committed_height(client, "predict QueuePlan execution")?
        .checked_add(QUEUE_PLAN_LIFECYCLE_BLOCKS)
        .ok_or_else(|| eyre!("QueuePlan privacy-governance height overflowed"))
}
fn proposed_activation(
    compiled: CompiledPrivacyProfileV1,
    proposed_at_height: u64,
    activate_at_height: u64,
) -> PrivacyProtocolActivationRecordV1 {
    compiled.activation_record(PrivacyProtocolLifecycleV1::Proposed(
        PrivacyProposedLifecycleV1 {
            proposed_at_height,
            activate_at_height,
        },
    ))
}
fn jindo_field(value: u64) -> iroha_data_model::privacy::PrivacyJindoFieldElementV1 {
    let mut encoding = [0_u8; 32];
    encoding[..8].copy_from_slice(&value.to_le_bytes());
    iroha_data_model::privacy::PrivacyJindoFieldElementV1::new(encoding)
}
fn jindo_witness() -> Result<JindoPrivacyActionWitnessV1> {
    JindoPrivacyActionWitnessV1::try_new(
        vec![
            vec![
                jindo_field(3),
                jindo_field(5),
                jindo_field(7),
                jindo_field(11),
            ],
            vec![jindo_field(13), jindo_field(17)],
            vec![jindo_field(19), jindo_field(23)],
            vec![jindo_field(29), jindo_field(31)],
        ],
        jindo_field(37),
    )
    .map_err(|error| eyre!("construct canonical Jindo witness: {error}"))
}
fn build_jindo_action(
    client: &Client,
    canonical_genesis_hash: [u8; 32],
    nonce: u32,
) -> Result<SignedTransaction> {
    let creation_time = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .wrap_err("system clock is before the Unix epoch")?;
    let mut context = JindoPrivacyActionTransactionContextV1 {
        network_id: client.network_id,
        authority: client.account.clone(),
        creation_time,
        time_to_live: Some(Duration::from_secs(3_600)),
        nonce: NonZeroU32::new(nonce),
        fee_payment: no_fee(),
        metadata: Metadata::default(),
    };
    let provisional =
        prepare_jindo_privacy_action_v1(context.clone(), jindo_witness()?, canonical_genesis_hash)
            .wrap_err("prepare provisional native Jindo action for fee quoting")?;
    ensure!(
        provisional
            .transaction_payload_for_fee_quote_v1()
            .admission_intent()
            == TransactionAdmissionIntent::QueuePlanSynced,
        "provisional Jindo action did not bind QueuePlanSynced admission"
    );
    let quote = client
        .quote_fees(provisional.transaction_payload_for_fee_quote_v1())
        .wrap_err("quote canonical Jindo action fee")?;
    ensure!(
        context
            .fee_payment
            .has_same_payer_and_gas_bound(&quote.intent),
        "Jindo fee quote changed the selected payer or gas bound"
    );
    drop(provisional);
    context.fee_payment = quote.intent.clone();
    let prepared =
        prepare_jindo_privacy_action_v1(context, jindo_witness()?, canonical_genesis_hash)
            .wrap_err("prepare canonical native Jindo action with quoted fees")?;
    ensure!(
        prepared
            .transaction_payload_for_fee_quote_v1()
            .admission_intent()
            == TransactionAdmissionIntent::QueuePlanSynced,
        "canonical Jindo action did not bind QueuePlanSynced admission"
    );
    quote
        .validate_for_signed_payload(prepared.transaction_payload_for_fee_quote_v1())
        .map_err(|error| eyre!(error))
        .wrap_err("validate quoted intent against final Jindo payload")?;
    let final_quote = client
        .quote_fees(prepared.transaction_payload_for_fee_quote_v1())
        .wrap_err("re-quote final canonical Jindo action")?;
    ensure!(
        final_quote.intent == quote.intent,
        "Jindo fee quote changed after fixed-size proof regeneration"
    );
    let signed = sign_prepared_jindo_privacy_action_v1(prepared, client.key_pair.private_key())
        .wrap_err("sign canonical native Jindo action")?;
    ensure!(
        signed.effect() == JindoPrivacyActionEffectV1::ActionVerificationAndFinalityOnly,
        "first-release Jindo action unexpectedly inferred a ledger mutation"
    );
    ensure!(
        signed.transaction_hash() == *signed.signed_transaction().hash().as_ref(),
        "Jindo builder-reported hash differs from the canonical signed transaction hash"
    );
    Ok(signed.into_signed_transaction())
}
fn independently_resign_corrupted_jindo_proof(
    client: &Client,
    valid: &SignedTransaction,
) -> Result<SignedTransaction> {
    let (valid_intent, submission) = valid
        .privacy_transaction_intent_binding_if_present_v1()
        .wrap_err("scan canonical Jindo action before proof corruption")?
        .ok_or_else(|| eyre!("canonical Jindo action omitted its direct privacy submission"))?;
    let mut envelope = submission.envelope.clone();
    let PrivacyProofV1::IrohaJindoPolynomialCommitmentV0(proof) = &mut envelope.proof else {
        return Err(eyre!(
            "canonical Jindo action carried a different proof variant"
        ));
    };
    ensure!(
        !proof.bytes.is_empty(),
        "canonical Jindo proof unexpectedly had no magic byte"
    );
    proof.bytes[0] ^= 0x80;
    envelope
        .validate_with_limits(&PrivacyConsensusLimitsV1::taira_default())
        .wrap_err("proof corruption must preserve the generic envelope contract")?;
    let corrupted = TransactionBuilder::from_payload(valid.payload().clone())
        .wrap_err("re-open canonical Jindo payload for proof corruption")?
        .with_instructions([SubmitPrivacyProofV1::new(envelope)])
        .try_sign(client.key_pair.private_key())
        .wrap_err("independently sign corrupted Jindo proof payload")?;
    corrupted
        .verify_signature()
        .wrap_err("verify independently signed corrupted Jindo transaction")?;
    let (corrupted_intent, _) = corrupted
        .privacy_transaction_intent_binding_if_present_v1()
        .wrap_err("corrupted Jindo proof lost its valid transaction-intent binding")?
        .ok_or_else(|| eyre!("corrupted Jindo proof omitted its direct privacy submission"))?;
    ensure!(
        corrupted_intent == valid_intent,
        "proof-only corruption changed the proof-independent Jindo transaction intent"
    );
    ensure!(
        corrupted.hash() != valid.hash(),
        "proof corruption did not change the canonical signed transaction hash"
    );
    Ok(corrupted)
}
async fn submit_instruction(
    client: &Client,
    instruction: impl Into<InstructionBox>,
    context: &str,
) -> Result<SignedTransaction> {
    let instruction = instruction.into();
    let payload = client
        .try_build_transaction_payload([instruction], no_fee(), Metadata::default())
        .wrap_err_with(|| format!("{context}: build QueuePlanSynced instruction payload"))?;
    let transaction = client
        .quote_and_sign_transaction_payload(payload)
        .wrap_err_with(|| format!("{context}: quote and sign QueuePlanSynced instruction"))?;
    ensure!(
        transaction.admission_intent() == TransactionAdmissionIntent::QueuePlanSynced,
        "{context}: client builder did not bind QueuePlanSynced admission"
    );
    let submitted_hash = submit_signed_transaction(client, &transaction, context).await?;
    ensure!(
        submitted_hash == transaction.hash(),
        "{context}: submitted hash differs from the exact signed transaction"
    );
    Ok(transaction)
}
async fn submit_signed_transaction(
    client: &Client,
    transaction: &SignedTransaction,
    context: &str,
) -> Result<iroha_crypto::HashOf<SignedTransaction>> {
    let client = client.clone();
    let transaction = transaction.clone();
    timeout(
        SUBMISSION_TASK_TIMEOUT,
        tokio::task::spawn_blocking(move || client.submit_transaction_blocking(&transaction)),
    )
    .await
    .map_err(|_| eyre!("{context}: signed transaction exceeded {SUBMISSION_TASK_TIMEOUT:?}"))?
    .map_err(|error| eyre!("{context}: submission task failed: {error}"))?
    .wrap_err_with(|| context.to_owned())
}
async fn wait_for_all_peer_activations(
    network: &sandbox::SerializedNetwork,
    minimum_height: u64,
    compiled: PrivacyCompiledProfileSnapshotV1,
    activation: Option<PrivacyProtocolActivationRecordV1>,
    context: &str,
) -> Result<Vec<PrivacyExact12CapabilityManifestV1>> {
    let deadline = Instant::now() + PEER_CONVERGENCE_TIMEOUT;
    let mut last_observed = Vec::new();
    loop {
        let mut snapshots = Vec::with_capacity(network.peers().len());
        last_observed.clear();
        for (index, peer) in network.peers().iter().enumerate() {
            let client = bounded_client(peer.client());
            match client.get_privacy_capabilities() {
                Ok(snapshot) => {
                    let row = jindo_row(&snapshot)?;
                    if snapshot.committed_height < minimum_height {
                        last_observed.push(format!(
                            "peer {index}: height={} below {minimum_height}, lifecycle={:?}",
                            snapshot.committed_height,
                            row.activation.map(|record| record.lifecycle)
                        ));
                    } else {
                        match assert_exact_jindo_row(&snapshot, compiled, activation, context) {
                            Ok(()) => {
                                last_observed.push(format!(
                                    "peer {index}: exact row at height {}",
                                    snapshot.committed_height
                                ));
                                snapshots.push(snapshot);
                            }
                            Err(error) => {
                                last_observed.push(format!(
                                    "peer {index}: height={}, exact-row mismatch: {error}",
                                    snapshot.committed_height
                                ));
                            }
                        }
                    }
                }
                Err(error) => last_observed.push(format!("peer {index}: query failed: {error}")),
            }
        }
        if snapshots.len() == network.peers().len() {
            return Ok(snapshots);
        }
        if Instant::now() >= deadline {
            return Err(eyre!(
                "{context}: peers did not converge within {PEER_CONVERGENCE_TIMEOUT:?}; {}",
                last_observed.join("; ")
            ));
        }
        sleep(POLL_INTERVAL).await;
    }
}
async fn advance_to_exact_height(client: &Client, target_height: u64) -> Result<()> {
    let start = committed_height(client, "begin deterministic activation advance")?;
    ensure!(
        start <= target_height,
        "cannot advance backwards from committed height {start} to {target_height}"
    );
    let distance = target_height - start;
    ensure!(
        distance % QUEUE_PLAN_LIFECYCLE_BLOCKS == 0,
        "QueuePlan activation advance from {start} to {target_height} is not divisible by \
         {QUEUE_PLAN_LIFECYCLE_BLOCKS} lifecycle blocks"
    );
    let transaction_count = distance / QUEUE_PLAN_LIFECYCLE_BLOCKS;
    let mut observed = start;
    for transaction_index in 1..=transaction_count {
        let expected = observed
            .checked_add(QUEUE_PLAN_LIFECYCLE_BLOCKS)
            .ok_or_else(|| eyre!("deterministic activation advance height overflowed"))?;
        submit_instruction(
            client,
            Log::new(
                Level::INFO,
                format!(
                    "Jindo activation advance QueuePlan transaction {transaction_index}/{transaction_count}"
                ),
            ),
            "advance Jindo activation height",
        )
        .await?;
        observed = committed_height(client, "observe QueuePlan activation advance")?;
        ensure!(
            observed == expected,
            "QueuePlan activation advance landed at height {observed}, expected {expected}"
        );
    }
    ensure!(
        observed == target_height,
        "deterministic activation advance landed at height {observed}, expected {target_height}"
    );
    Ok(())
}
fn exact_applied_transaction_visible(
    client: &Client,
    transaction: &SignedTransaction,
) -> Result<bool> {
    let signed_hash = transaction.hash();
    let Some(status) = client
        .get_transaction_status_response_local(signed_hash)
        .wrap_err("query exact peer-local transaction status")?
    else {
        return Ok(false);
    };
    match (status.status.kind.as_str(), status.resolved_from.as_str()) {
        ("Applied", "state") => {}
        ("Rejected" | "Expired", "state") => {
            return Err(eyre!(
                "canonical privacy transaction reached terminal {} status",
                status.status.kind
            ));
        }
        _ => return Ok(false),
    }
    let expected_hash = transaction.hash_as_entrypoint();
    let expected_entrypoint = TransactionEntrypoint::External(transaction.clone());
    let details = client
        .get_successful_transaction_details(expected_hash)
        .wrap_err("query exact successful transaction details")?;
    let committed = &details.transaction;
    ensure!(
        committed.entrypoint() == &expected_entrypoint,
        "entrypoint hash matched different transaction bytes"
    );
    ensure!(
        committed.result().0.is_ok(),
        "canonical Jindo transaction is visible but finalized as rejected"
    );
    Ok(true)
}
async fn wait_for_transaction_on_peers(
    clients: &[Client],
    transaction: &SignedTransaction,
    context: &str,
) -> Result<()> {
    let deadline = Instant::now() + PEER_CONVERGENCE_TIMEOUT;
    let mut last_observed = Vec::new();
    loop {
        let mut visible = 0_usize;
        last_observed.clear();
        for (index, client) in clients.iter().enumerate() {
            match exact_applied_transaction_visible(client, transaction) {
                Ok(true) => {
                    visible += 1;
                    last_observed.push(format!("peer {index}: exact applied transaction visible"));
                }
                Ok(false) => last_observed.push(format!("peer {index}: transaction absent")),
                Err(error) => last_observed.push(format!("peer {index}: {error}")),
            }
        }
        if visible == clients.len() {
            return Ok(());
        }
        if Instant::now() >= deadline {
            return Err(eyre!(
                "{context}: finalized transaction did not converge within {PEER_CONVERGENCE_TIMEOUT:?}; {}",
                last_observed.join("; ")
            ));
        }
        sleep(POLL_INTERVAL).await;
    }
}
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn canonical_jindo_direct_action_survives_four_peer_activation_replay_and_restart()
-> Result<()> {
    init_instruction_registry();
    let context =
        stringify!(canonical_jindo_direct_action_survives_four_peer_activation_replay_and_restart);
    let builder = NetworkBuilder::new()
        .with_peers(4)
        .with_auto_populated_trusted_peers()
        .with_block_cadence(TEST_BLOCK_CADENCE)
        .with_permissioned_consensus()
        .with_genesis_instruction(Grant::account_permission(
            Permission::from(CanEnactGovernance),
            iroha_test_samples::ALICE_ID.clone(),
        ))
        .with_config_layer(|layer| {
            // Bound disk allocation explicitly for test filesystems and keep
            // the production handshake at its minimum supported puzzle cost.
            layer
                .write(
                    ["nexus", "storage", "local_budget_bytes"],
                    TEST_NEXUS_LOCAL_STORAGE_BUDGET_BYTES,
                )
                .write(
                    [
                        "network",
                        "soranet_handshake",
                        "pow",
                        "puzzle",
                        "memory_kib",
                    ],
                    i64::from(iroha_crypto::soranet::puzzle::MIN_MEMORY_KIB),
                )
                .write(
                    ["network", "soranet_handshake", "pow", "puzzle", "time_cost"],
                    1_i64,
                )
                .write(
                    ["network", "soranet_handshake", "pow", "puzzle", "lanes"],
                    1_i64,
                );
        });
    let Some(network) = sandbox::start_network_async_or_skip(builder, context).await? else {
        return Ok(());
    };
    let result: Result<()> = async {
        ensure!(
            network.peers().len() == 4,
            "Jindo lifecycle test requires exactly four trusted peers"
        );
        let client = bounded_client(network.client());
        let genesis_hash = canonical_genesis_hash(&client).await?;
        let compiled = compiled_privacy_profile_v1(JINDO_PROTOCOL)
            .wrap_err("load canonical compiled Jindo profile")?;
        let compiled_snapshot: PrivacyCompiledProfileSnapshotV1 = compiled.into();
        let queue_plan_start_height =
            committed_height(&client, "begin QueuePlanSynced lifecycle preflight")?;
        let queue_plan_transaction = submit_instruction(
            &client,
            Log::new(
                Level::INFO,
                "Jindo QueuePlanSynced lifecycle preflight".to_owned(),
            ),
            "complete Jindo QueuePlanSynced lifecycle preflight",
        )
        .await?;
        let preflight_clients = network
            .peers()
            .iter()
            .map(|peer| bounded_client(peer.client()))
            .collect::<Vec<_>>();
        wait_for_transaction_on_peers(
            &preflight_clients,
            &queue_plan_transaction,
            "QueuePlanSynced preflight terminal visibility",
        )
        .await?;
        let queue_plan_end_height =
            committed_height(&client, "finish QueuePlanSynced lifecycle preflight")?;
        let expected_queue_plan_end_height = queue_plan_start_height
            .checked_add(QUEUE_PLAN_LIFECYCLE_BLOCKS)
            .ok_or_else(|| eyre!("QueuePlanSynced lifecycle preflight height overflowed"))?;
        ensure!(
            queue_plan_end_height == expected_queue_plan_end_height,
            "QueuePlanSynced lifecycle preflight ended at height {queue_plan_end_height}, expected \
             exact admission, autonomous payload, and merge carriers through height \
             {expected_queue_plan_end_height}"
        );
        wait_for_all_peer_activations(
            &network,
            queue_plan_end_height,
            compiled_snapshot,
            None,
            "QueuePlanSynced preflight preserves unregistered Jindo state",
        )
        .await?;
        let early_execution_height = next_queue_plan_execution_height(&client)?;
        let early = proposed_activation(
            compiled,
            early_execution_height,
            early_execution_height
                .checked_add(PRIVACY_MIN_ACTIVATION_DELAY_BLOCKS_V1 - 1)
                .ok_or_else(|| eyre!("early activation height overflowed"))?,
        );
        let early_error = submit_instruction(
            &client,
            RegisterPrivacyProtocolActivationV1::new(early),
            "one-height-early Jindo activation must reject",
        )
        .await
        .expect_err("one-height-early Jindo activation was accepted");
        ensure!(
            error_chain_contains(&early_error, "is too early"),
            "one-height-early rejection had the wrong reason: {early_error:?}"
        );
        let early_terminal_height =
            committed_height(&client, "observe one-height-early rejection")?;
        ensure!(
            early_terminal_height == early_execution_height,
            "one-height-early QueuePlan rejection landed at height {early_terminal_height}, \
             expected {early_execution_height}"
        );
        wait_for_all_peer_activations(
            &network,
            early_terminal_height,
            compiled_snapshot,
            None,
            "one-height-early rejection must not register state",
        )
        .await?;
        let mismatch_execution_height = next_queue_plan_execution_height(&client)?;
        let mut mismatched = proposed_activation(
            compiled,
            mismatch_execution_height,
            mismatch_execution_height
                .checked_add(PRIVACY_MIN_ACTIVATION_DELAY_BLOCKS_V1)
                .ok_or_else(|| eyre!("mismatched activation height overflowed"))?,
        );
        mismatched.parameter_digest = PrivacyParameterDigestV1::new([0xA5; 32]);
        ensure!(
            mismatched.parameter_digest != compiled.parameter_digest,
            "mismatched digest fixture accidentally equals the compiled digest"
        );
        let mismatch_error = submit_instruction(
            &client,
            RegisterPrivacyProtocolActivationV1::new(mismatched),
            "mismatched Jindo compiled digest must reject",
        )
        .await
        .expect_err("mismatched Jindo compiled digest was accepted");
        ensure!(
            error_chain_contains(&mismatch_error, "does not match compiled native profile"),
            "compiled-digest rejection had the wrong reason: {mismatch_error:?}"
        );
        let mismatch_terminal_height =
            committed_height(&client, "observe compiled-digest rejection")?;
        ensure!(
            mismatch_terminal_height == mismatch_execution_height,
            "compiled-digest QueuePlan rejection landed at height {mismatch_terminal_height}, \
             expected {mismatch_execution_height}"
        );
        wait_for_all_peer_activations(
            &network,
            mismatch_terminal_height,
            compiled_snapshot,
            None,
            "compiled-digest rejection must not register state",
        )
        .await?;
        let forged_execution_height = next_queue_plan_execution_height(&client)?;
        let forged_proposal_height = forged_execution_height
            .checked_add(1)
            .ok_or_else(|| eyre!("forged proposal height overflowed"))?;
        let forged = proposed_activation(
            compiled,
            forged_proposal_height,
            forged_proposal_height
                .checked_add(PRIVACY_MIN_ACTIVATION_DELAY_BLOCKS_V1)
                .ok_or_else(|| eyre!("forged activation height overflowed"))?,
        );
        let forged_error = submit_instruction(
            &client,
            RegisterPrivacyProtocolActivationV1::new(forged),
            "forged Jindo proposal height must reject",
        )
        .await
        .expect_err("forged Jindo proposal height was accepted");
        ensure!(
            error_chain_contains(&forged_error, "differs from current height"),
            "forged-height rejection had the wrong reason: {forged_error:?}"
        );
        let forged_terminal_height =
            committed_height(&client, "observe forged-height rejection")?;
        ensure!(
            forged_terminal_height == forged_execution_height,
            "forged-height QueuePlan rejection landed at height {forged_terminal_height}, \
             expected {forged_execution_height}"
        );
        wait_for_all_peer_activations(
            &network,
            forged_terminal_height,
            compiled_snapshot,
            None,
            "forged-height rejection must not register state",
        )
        .await?;
        let registration_height = next_queue_plan_execution_height(&client)?;
        let activation_height = registration_height
            .checked_add(PRIVACY_MIN_ACTIVATION_DELAY_BLOCKS_V1)
            .ok_or_else(|| eyre!("canonical activation height overflowed"))?;
        let proposed = proposed_activation(compiled, registration_height, activation_height);
        submit_instruction(
            &client,
            RegisterPrivacyProtocolActivationV1::new(proposed),
            "register exact compiled Jindo activation",
        )
        .await?;
        let observed_registration_height =
            committed_height(&client, "observe exact Jindo registration")?;
        ensure!(
            observed_registration_height == registration_height,
            "Jindo registration landed at height {observed_registration_height}, expected \
             {registration_height}"
        );
        wait_for_all_peer_activations(
            &network,
            registration_height,
            compiled_snapshot,
            Some(proposed),
            "exact proposed Jindo activation",
        )
        .await?;
        let preactivation_probe_start_height = activation_height
            .checked_sub(2 * QUEUE_PLAN_LIFECYCLE_BLOCKS)
            .ok_or_else(|| eyre!("activation height has no QueuePlan pre-probe window"))?;
        timeout(
            ACTIVATION_ADVANCE_TIMEOUT,
            advance_to_exact_height(&client, preactivation_probe_start_height),
        )
        .await
        .map_err(|_| {
            eyre!(
                "advancing Jindo through the exact 300-block activation lead exceeded \
                 {ACTIVATION_ADVANCE_TIMEOUT:?}"
            )
        })??;
        wait_for_all_peer_activations(
            &network,
            preactivation_probe_start_height,
            compiled_snapshot,
            Some(proposed),
            "Jindo remains proposed before its final pre-activation QueuePlan opportunity",
        )
        .await?;
        let preactivation_probe = build_jindo_action(&client, genesis_hash, 1)?;
        let probe_error = submit_signed_transaction(
            &client,
            &preactivation_probe,
            "Jindo action at the last QueuePlan opportunity before activation must reject",
        )
        .await
        .expect_err("Jindo action was admitted while lifecycle was Proposed");
        ensure!(
            error_chain_contains(&probe_error, "activation is not active"),
            "pre-activation action rejected for the wrong reason: {probe_error:?}"
        );
        let expected_probe_terminal_height = preactivation_probe_start_height
            .checked_add(QUEUE_PLAN_LIFECYCLE_BLOCKS)
            .ok_or_else(|| eyre!("pre-activation probe height overflowed"))?;
        let probe_terminal_height =
            committed_height(&client, "observe pre-activation Jindo rejection")?;
        ensure!(
            probe_terminal_height == expected_probe_terminal_height,
            "pre-activation Jindo rejection landed at height {probe_terminal_height}, expected \
             {expected_probe_terminal_height}"
        );
        wait_for_all_peer_activations(
            &network,
            probe_terminal_height,
            compiled_snapshot,
            Some(proposed),
            "Jindo remains Proposed after its final pre-activation QueuePlan opportunity",
        )
        .await?;
        submit_instruction(
            &client,
            Log::new(
                Level::INFO,
                format!("Jindo exact activation block {activation_height}"),
            ),
            "commit exact Jindo activation block",
        )
        .await?;
        let observed_activation_height =
            committed_height(&client, "observe exact Jindo activation")?;
        ensure!(
            observed_activation_height == activation_height,
            "Jindo activation transaction landed at height {observed_activation_height}, \
             expected {activation_height}"
        );
        let active = compiled.activation_record(PrivacyProtocolLifecycleV1::Active(
            iroha_data_model::privacy::PrivacyActiveLifecycleV1 {
                proposed_at_height: registration_height,
                activated_at_height: activation_height,
                state_since_height: activation_height,
            },
        ));
        wait_for_all_peer_activations(
            &network,
            activation_height,
            compiled_snapshot,
            Some(active),
            "exact active Jindo state on all peers",
        )
        .await?;
        let final_action = build_jindo_action(&client, genesis_hash, 2)?;
        let corrupted_action = independently_resign_corrupted_jindo_proof(&client, &final_action)?;
        let corruption_start_height =
            committed_height(&client, "begin corrupted Jindo QueuePlan lifecycle")?;
        let corruption_error = submit_signed_transaction(
            &client,
            &corrupted_action,
            "post-activation corrupted Jindo proof must reject",
        )
        .await
        .expect_err("post-activation corrupted Jindo proof was accepted");
        ensure!(
            error_chain_contains(&corruption_error, "native Jindo verification failed"),
            "corrupted Jindo proof bypassed native verification: {corruption_error:?}"
        );
        ensure!(
            error_chain_contains(&corruption_error, "proof magic is invalid"),
            "corrupted Jindo proof rejected for the wrong native reason: {corruption_error:?}"
        );
        let post_corruption_height =
            committed_height(&client, "observe corrupted Jindo rejection")?;
        let expected_post_corruption_height = corruption_start_height
            .checked_add(QUEUE_PLAN_LIFECYCLE_BLOCKS)
            .ok_or_else(|| eyre!("corrupted Jindo terminal height overflowed"))?;
        ensure!(
            post_corruption_height == expected_post_corruption_height,
            "corrupted Jindo rejection landed at height {post_corruption_height}, expected \
             {expected_post_corruption_height}"
        );
        wait_for_all_peer_activations(
            &network,
            post_corruption_height,
            compiled_snapshot,
            Some(active),
            "corrupted Jindo rejection must preserve exact Active state",
        )
        .await?;
        ensure!(
            !exact_applied_transaction_visible(&client, &final_action)?,
            "canonical Jindo action was visible before its own submission"
        );
        let restart_index = network.peers().len() - 1;
        let restart_peer = network.peers()[restart_index].clone();
        let config_layers = network.config_layers().collect::<Vec<_>>();
        ensure!(
            restart_peer.shutdown_if_started().await,
            "selected Active Jindo peer was not running before restart coverage"
        );
        let submitted_hash = submit_signed_transaction(
            &client,
            &final_action,
            "submit canonical active Jindo direct action",
        )
        .await?;
        ensure!(
            *submitted_hash.as_ref() == *final_action.hash().as_ref(),
            "submitted Jindo transaction hash differs from the signed action"
        );
        let finalized_height = committed_height(&client, "observe canonical Jindo finality")?;
        let expected_finalized_height = post_corruption_height
            .checked_add(QUEUE_PLAN_LIFECYCLE_BLOCKS)
            .ok_or_else(|| eyre!("Jindo finality height overflowed"))?;
        ensure!(
            finalized_height == expected_finalized_height,
            "Jindo finality landed at height {finalized_height}, expected \
             {expected_finalized_height}"
        );
        let healthy_clients = network
            .peers()
            .iter()
            .enumerate()
            .filter(|(index, _)| *index != restart_index)
            .map(|(_, peer)| bounded_client(peer.client()))
            .collect::<Vec<_>>();
        wait_for_transaction_on_peers(
            &healthy_clients,
            &final_action,
            "healthy-peer Jindo finality",
        )
        .await?;
        let replay_error = client
            .submit_transaction(&final_action)
            .expect_err("exact Jindo transaction replay was accepted");
        ensure!(
            is_exact_replay_error(&replay_error),
            "exact Jindo replay rejected for the wrong reason: {replay_error:?}"
        );
        ensure!(
            client
                .get_privacy_capabilities()
                .wrap_err("query height after exact replay rejection")?
                .committed_height
                == finalized_height,
            "exact transaction replay unexpectedly committed another block"
        );
        timeout(
            RESTART_TIMEOUT,
            restart_peer.start_checked(config_layers.iter(), None),
        )
        .await
        .map_err(|_| eyre!("Jindo peer restart exceeded {RESTART_TIMEOUT:?}"))?
        .wrap_err("restart Jindo peer")?;
        wait_for_all_peer_activations(
            &network,
            finalized_height,
            compiled_snapshot,
            Some(active),
            "post-restart Active Jindo binding and state",
        )
        .await?;
        let all_clients = network
            .peers()
            .iter()
            .map(|peer| bounded_client(peer.client()))
            .collect::<Vec<_>>();
        wait_for_transaction_on_peers(
            &all_clients,
            &final_action,
            "post-restart finalized Jindo transaction visibility",
        )
        .await?;
        ensure!(
            canonical_genesis_hash(&bounded_client(restart_peer.client())).await? == genesis_hash,
            "restarted peer derived a different canonical genesis hash"
        );
        println!(
            "TAIRA_PRIVACY_PROTOCOL_FOUR_PEER_CASE_V1:privacy_exact12_jindo_network::canonical_jindo_direct_action_survives_four_peer_activation_replay_and_restart:passed"
        );
        Ok(())
    }
    .await;
    network.shutdown().await;
    result
}
