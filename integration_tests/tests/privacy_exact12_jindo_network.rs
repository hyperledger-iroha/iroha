#![cfg(feature = "privacy-release-evidence")]

//! Four-peer native Jindo actions with default and scheduled ordinary policies.
//! Exercises profile binding, malformed proofs, atomic policy rollback, replay and restart.
use super::privacy_exact12_network_support::{privacy_capabilities, wait_for_transaction_on_peers};
use eyre::{Result, WrapErr as _, ensure, eyre};
use futures_util::TryStreamExt as _;
use integration_tests::sandbox;
use iroha::{blocking::Client, client::FeeQuoteRequest};
use iroha_core_privacy::{
    privacy_engines::jindo::{
        JindoPrivacyActionEffectV1, JindoPrivacyActionTransactionContextV1,
        JindoPrivacyActionWitnessV1, prepare_jindo_privacy_action_v1,
        sign_prepared_jindo_privacy_action_v1,
    },
    privacy_profiles::compiled_privacy_profile_v1,
};
use iroha_data_model::{
    Level,
    isi::{
        Grant, InstructionBox, Log,
        privacy::{SchedulePrivacyProtocolLimitsUpdateV1, SubmitPrivacyProofV1},
    },
    permission::Permission,
    privacy::{
        JindoLimitsV1, PrivacyCapabilityReadinessV1, PrivacyCompiledProfileResultV1,
        PrivacyCompiledProfileSnapshotV1, PrivacyExact12CapabilityManifestV1,
        PrivacyExact12CapabilityRowV1, PrivacyParameterDigestV1, PrivacyProofBytesV1,
        PrivacyProofV1, PrivacyProtocolIdV1, PrivacyProtocolLimitsUpdateV1,
        PrivacyProtocolLimitsV1, PrivacyProtocolPolicyV1, PrivacyStatementDigestV1,
        PrivacyTransactionIntentDigestV1,
    },
    transaction::{Executable, FeePaymentIntent, SignedTransaction, TransactionBuilder},
};
use iroha_executor_data_model::permission::governance::CanEnactGovernance;
use iroha_model_base::metadata::Metadata;
use iroha_test_network::{NetworkBuilder, init_instruction_registry};
use std::{
    num::{NonZeroU32, NonZeroU64},
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tokio::time::{Instant, sleep, timeout};
const JINDO_PROTOCOL: PrivacyProtocolIdV1 = PrivacyProtocolIdV1::IrohaJindoPolynomialCommitmentV1;
const TEST_NEXUS_LOCAL_STORAGE_BUDGET_BYTES: i64 = 1024 * 1024 * 1024;
// An ordinary transaction executes in one certified native global carrier.
// Leave deterministic headroom for healthy view churn.
const SUBMISSION_TIMEOUT: Duration = Duration::from_secs(300);
// Let the client's detailed terminal status/error win over the Tokio wrapper.
const SUBMISSION_TASK_TIMEOUT: Duration = Duration::from_secs(360);
const TRANSACTION_TTL: Duration = Duration::from_secs(600);
const EXECUTION_CARRIER_BLOCKS: u64 = 1;
const PEER_CONVERGENCE_TIMEOUT: Duration = Duration::from_secs(180);
const RESTART_TIMEOUT: Duration = Duration::from_secs(180);
// This release-evidence fixture exercises instrumented four-validator body
// reconstruction, validation, replay, and restart rather than throughput. Use
// the released one-second signed cadence so the deterministic Sumeragi view
// backoff can cover that work without changing the production timer policy.
const TEST_BLOCK_CADENCE: Duration = Duration::from_secs(1);
const POLL_INTERVAL: Duration = Duration::from_millis(200);
const CANONICAL_GENESIS_FETCH_TIMEOUT: Duration = Duration::from_secs(30);
fn bounded_client(client: Client) -> Client {
    integration_tests::sync::rebind_blocking_client(&client, |client| {
        client.transaction_status_timeout = SUBMISSION_TIMEOUT;
        client.transaction_ttl = Some(TRANSACTION_TTL);
        client.torii_request_timeout = Duration::from_secs(20);
    })
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
    policy: Option<PrivacyProtocolPolicyV1>,
    context: &str,
) -> Result<()> {
    snapshot
        .validate()
        .wrap_err_with(|| format!("{context}: invalid capability manifest"))?;
    let row = jindo_row(snapshot)?;
    ensure!(
        row.compiled_profile == PrivacyCompiledProfileResultV1::Available(compiled),
        "{context}: Jindo compiled binding drifted: {:?}",
        row.compiled_profile
    );
    ensure!(
        row.policy == policy,
        "{context}: Jindo policy mismatch: expected {policy:?}, got {:?}",
        row.policy
    );
    ensure!(
        row.readiness == PrivacyCapabilityReadinessV1::Available && row.is_network_available(),
        "{context}: actual native Jindo should be available: {:?}",
        row.readiness
    );
    ensure!(
        row.effective_protocol_limits_v1()
            == Some(policy.map_or(compiled.protocol_limits, |record| record.current_limits)),
        "{context}: effective Jindo protocol limits drifted"
    );
    Ok(())
}
async fn canonical_genesis_hash(client: &Client) -> Result<[u8; 32]> {
    let genesis = timeout(CANONICAL_GENESIS_FETCH_TIMEOUT, async {
        let mut blocks = client
            .account_client()
            .blocks()
            .subscribe(NonZeroU64::MIN)
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
async fn committed_height(client: &Client, context: &str) -> Result<u64> {
    Ok(privacy_capabilities(&client)
        .await
        .wrap_err_with(|| format!("{context}: query committed height"))?
        .committed_height)
}
async fn next_execution_height(client: &Client) -> Result<u64> {
    committed_height(client, "predict native transaction execution")
        .await?
        .checked_add(EXECUTION_CARRIER_BLOCKS)
        .ok_or_else(|| eyre!("native transaction privacy-governance height overflowed"))
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
async fn build_jindo_action(
    client: &Client,
    canonical_genesis_hash: [u8; 32],
    nonce: u32,
) -> Result<SignedTransaction> {
    let creation_time = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .wrap_err("system clock is before the Unix epoch")?;
    let mut context = JindoPrivacyActionTransactionContextV1 {
        network_id: *client.client().network_id(),
        authority: client.client().account().clone(),
        creation_time,
        time_to_live: Some(Duration::from_secs(3_600)),
        nonce: NonZeroU32::new(nonce),
        fee_payment: no_fee(),
        metadata: Metadata::default(),
    };
    let provisional =
        prepare_jindo_privacy_action_v1(context.clone(), jindo_witness()?, canonical_genesis_hash)
            .wrap_err("prepare provisional native Jindo action for fee quoting")?;
    let quote = client
        .account_client()
        .quote_fees(FeeQuoteRequest::AccountSignature {
            payload: provisional.transaction_payload_for_fee_quote_v1(),
        })
        .await
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
    quote
        .validate_for_signed_payload(prepared.transaction_payload_for_fee_quote_v1())
        .map_err(|error| eyre!(error))
        .wrap_err("validate quoted intent against final Jindo payload")?;
    let final_quote = client
        .account_client()
        .quote_fees(FeeQuoteRequest::AccountSignature {
            payload: prepared.transaction_payload_for_fee_quote_v1(),
        })
        .await
        .wrap_err("re-quote final canonical Jindo action")?;
    ensure!(
        final_quote.intent == quote.intent,
        "Jindo fee quote changed after fixed-size proof regeneration"
    );
    let signed =
        sign_prepared_jindo_privacy_action_v1(prepared, client.client().key_pair().private_key())
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
async fn submit_instruction(
    client: &Client,
    instruction: impl Into<InstructionBox>,
    context: &str,
) -> Result<SignedTransaction> {
    submit_instructions(client, vec![instruction.into()], context).await
}
async fn submit_instructions(
    client: &Client,
    instructions: Vec<InstructionBox>,
    context: &str,
) -> Result<SignedTransaction> {
    let mut payload = client
        .account_client()
        .prepare_transaction(iroha::client::AccountTransactionDraft::new(
            instructions,
            no_fee(),
            Metadata::default(),
        ))
        .wrap_err_with(|| format!("{context}: build native transaction instruction payload"))?;
    let quote = client
        .account_client()
        .quote_fees(FeeQuoteRequest::AccountSignature { payload: &payload })
        .await
        .wrap_err_with(|| format!("{context}: quote native transaction instruction"))?;
    ensure!(
        payload
            .fee_payment
            .has_same_payer_and_gas_bound(&quote.intent),
        "{context}: fee quote changed the selected payer, sponsor revision, or gas bound"
    );
    payload.fee_payment = quote.intent;
    let transaction = client
        .account_client()
        .sign_transaction(payload)
        .wrap_err_with(|| format!("{context}: sign native transaction instruction"))?;
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
    timeout(
        SUBMISSION_TASK_TIMEOUT,
        client
            .account_client()
            .submit_transaction_and_wait(transaction),
    )
    .await
    .map_err(|_| eyre!("{context}: signed transaction exceeded {SUBMISSION_TASK_TIMEOUT:?}"))?
    .wrap_err_with(|| context.to_owned())
}
async fn wait_for_all_peer_policies(
    network: &sandbox::SerializedNetwork,
    minimum_height: u64,
    compiled: PrivacyCompiledProfileSnapshotV1,
    policy: Option<PrivacyProtocolPolicyV1>,
    context: &str,
) -> Result<Vec<PrivacyExact12CapabilityManifestV1>> {
    let deadline = Instant::now() + PEER_CONVERGENCE_TIMEOUT;
    let mut last_observed = Vec::new();
    loop {
        let mut snapshots = Vec::with_capacity(network.peers().len());
        last_observed.clear();
        for (index, peer) in network.peers().iter().enumerate() {
            let client = bounded_client(peer.client());
            match privacy_capabilities(&client).await {
                Ok(snapshot) => {
                    let row = jindo_row(&snapshot)?;
                    if snapshot.committed_height < minimum_height {
                        last_observed.push(format!(
                            "peer {index}: height={} below {minimum_height}, policy={:?}",
                            snapshot.committed_height, row.policy
                        ));
                    } else {
                        match assert_exact_jindo_row(&snapshot, compiled, policy, context) {
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
#[derive(Clone, Copy)]
enum JindoTamper {
    ProofMagic,
    ParameterDigest,
}
fn independently_resigned_jindo_tamper(
    transaction: &SignedTransaction,
    tamper: JindoTamper,
    client: &Client,
) -> Result<SignedTransaction> {
    let (original_intent, submission) = transaction
        .privacy_transaction_intent_binding_if_present_v1()?
        .ok_or_else(|| eyre!("canonical Jindo transaction omitted its direct submission"))?;
    let mut envelope = submission.envelope.clone();
    match tamper {
        JindoTamper::ProofMagic => {
            let PrivacyProofV1::IrohaJindoPolynomialCommitmentV1(proof) = &mut envelope.proof
            else {
                return Err(eyre!("Jindo tamper reached a different native proof"));
            };
            let mut bytes = proof.as_bytes().to_vec();
            *bytes
                .first_mut()
                .ok_or_else(|| eyre!("canonical Jindo proof is empty"))? ^= 1;
            *proof = PrivacyProofBytesV1::new(bytes);
        }
        JindoTamper::ParameterDigest => {
            let mut digest = *envelope.parameter_digest.as_bytes();
            digest[0] ^= 0x80;
            envelope.parameter_digest = PrivacyParameterDigestV1::new(digest);
            envelope.statement.context_mut().parameter_digest = envelope.parameter_digest;
            envelope.statement.context_mut().transaction_intent_digest =
                PrivacyTransactionIntentDigestV1::new([0; 32]);
            envelope.statement_digest = PrivacyStatementDigestV1::new([0; 32]);
        }
    }
    let mut payload = transaction.payload().clone();
    let executable = |value| {
        Executable::Instructions(
            vec![InstructionBox::from(SubmitPrivacyProofV1::new(value))].into(),
        )
    };
    payload.instructions = executable(envelope.clone());
    if matches!(tamper, JindoTamper::ParameterDigest) {
        let intent = payload.privacy_transaction_intent_digest_v1()?;
        envelope.statement.context_mut().transaction_intent_digest = intent;
        envelope.statement_digest = envelope.statement.digest()?;
        payload.instructions = executable(envelope);
    } else {
        ensure!(
            payload.validate_privacy_transaction_intent_binding_v1()? == original_intent,
            "proof-only corruption changed the proof-independent transaction intent"
        );
    }
    payload.validate_privacy_transaction_intent_binding_v1()?;
    let signed = TransactionBuilder::from_payload(payload)?
        .try_sign(client.client().key_pair().private_key())?;
    signed.verify_signature()?;
    signed
        .privacy_transaction_intent_binding_if_present_v1()?
        .ok_or_else(|| eyre!("independently signed Jindo tamper lost its submission"))?;
    Ok(signed)
}
async fn assert_exact_jindo_replay_rejected(
    client: &Client,
    transaction: &SignedTransaction,
) -> Result<()> {
    let error = timeout(
        SUBMISSION_TASK_TIMEOUT,
        client.account_client().submit_transaction(transaction),
    )
    .await
    .map_err(|_| eyre!("exact Jindo replay submission timed out"))?
    .expect_err("exact finalized Jindo transaction was accepted again");
    ensure!(
        ["already_committed", "transaction already committed"]
            .iter()
            .any(|reason| error_chain_contains(&error, reason)),
        "exact Jindo replay rejected for the wrong reason: {error:?}"
    );
    Ok(())
}
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn canonical_jindo_actions_survive_four_peer_policy_replay_and_restart() -> Result<()> {
    init_instruction_registry();
    let context = stringify!(canonical_jindo_actions_survive_four_peer_policy_replay_and_restart);
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
    let network = sandbox::start_network_async_or_skip(builder, context)
        .await?
        .ok_or_else(|| eyre!("{context}: four-peer delivery requires a running network"))?;
    let result: Result<()> = async {
        ensure!(
            network.peers().len() == 4,
            "Jindo policy test requires exactly four trusted peers"
        );
        let client = bounded_client(network.client());
        let genesis_hash = canonical_genesis_hash(&client).await?;
        let compiled = compiled_privacy_profile_v1(JINDO_PROTOCOL)
            .wrap_err("load canonical compiled Jindo profile")?;
        let compiled_snapshot: PrivacyCompiledProfileSnapshotV1 = compiled.into();
        let submission_start_height =
            committed_height(&client, "begin native transaction policy preflight").await?;
        let preflight_transaction = submit_instruction(
            &client,
            Log::new(
                Level::INFO,
                "Jindo native transaction policy preflight".to_owned(),
            ),
            "complete Jindo native transaction policy preflight",
        )
        .await?;
        let preflight_clients = network
            .peers()
            .iter()
            .map(|peer| bounded_client(peer.client()))
            .collect::<Vec<_>>();
        wait_for_transaction_on_peers(
            &preflight_clients,
            &preflight_transaction,
            "native transaction preflight terminal visibility",
            PEER_CONVERGENCE_TIMEOUT,
        )
        .await?;
        let submission_end_height =
            committed_height(&client, "finish native transaction policy preflight").await?;
        let expected_submission_end_height = submission_start_height
            .checked_add(EXECUTION_CARRIER_BLOCKS)
            .ok_or_else(|| eyre!("native transaction policy preflight height overflowed"))?;
        ensure!(
            submission_end_height == expected_submission_end_height,
            "native transaction policy preflight ended at height {submission_end_height}, expected \
             exact admission, autonomous payload, and merge carriers through height \
             {expected_submission_end_height}"
        );
        wait_for_all_peer_policies(
            &network,
            submission_end_height,
            compiled_snapshot,
            None,
            "native transaction preflight preserves default Jindo policy",
        )
        .await?;
        let default_action = build_jindo_action(&client, genesis_hash, 1).await?;
        for (tamper, expected, label) in [
            (JindoTamper::ProofMagic, "native Jindo verification failed", "malformed native Jindo proof"),
            (JindoTamper::ParameterDigest, "privacy profile parameter digest differs from envelope", "substituted native Jindo profile"),
        ] {
            let transaction = independently_resigned_jindo_tamper(&default_action, tamper, &client)?;
            let incoming = next_execution_height(&client).await?;
            let error = submit_signed_transaction(&client, &transaction, label).await
                .expect_err("independently signed invalid Jindo action was accepted");
            ensure!(error_chain_contains(&error, expected), "{label}: wrong rejection: {error:?}");
            wait_for_all_peer_policies(&network, incoming, compiled_snapshot, None,
                "invalid native action preserves absent optional policy").await?;
        }
        submit_signed_transaction(&client, &default_action,
            "native Jindo action succeeds without a protocol policy row").await?;
        wait_for_transaction_on_peers(&preflight_clients, &default_action,
            "all-four native Jindo default-policy finality", PEER_CONVERGENCE_TIMEOUT).await?;
        let default_height = committed_height(&client, "default-policy finality").await?;
        wait_for_all_peer_policies(&network, default_height, compiled_snapshot, None,
            "successful native proof leaves optional policy absent").await?;
        assert_exact_jindo_replay_rejected(&client, &default_action).await?;
        ensure!(committed_height(&client, "exact replay height").await? == default_height,
            "exact Jindo replay committed another block");

        let restricted_limits = PrivacyProtocolLimitsV1::IrohaJindoPolynomialCommitmentV1(
            JindoLimitsV1 { max_polynomial_count: 3 });
        let incoming = next_execution_height(&client).await?;
        let effective = incoming.checked_add(1).ok_or_else(|| eyre!("policy height overflowed"))?;
        let atomic_error = submit_instructions(&client, vec![
            SchedulePrivacyProtocolLimitsUpdateV1::new(JINDO_PROTOCOL, effective, restricted_limits).into(),
            SchedulePrivacyProtocolLimitsUpdateV1::new(PrivacyProtocolIdV1::VegaExistingCredentialZkV1,
                incoming, PrivacyProtocolLimitsV1::VegaExistingCredentialZkV1).into(),
        ], "invalid second policy schedule rolls back the first").await
            .expect_err("non-future atomic policy schedule was accepted");
        ensure!(error_chain_contains(&atomic_error, "must be later than scheduled height"),
            "atomic policy schedule failed for wrong reason: {atomic_error:?}");
        wait_for_all_peer_policies(&network, incoming, compiled_snapshot, None,
            "failed atomic schedule leaves no Jindo policy on any peer").await?;
        let bad_limits = PrivacyProtocolLimitsV1::IrohaJindoPolynomialCommitmentV1(
            JindoLimitsV1 { max_polynomial_count: 5 });
        let incoming = next_execution_height(&client).await?;
        let invalid_limit_error = submit_instruction(&client,
            SchedulePrivacyProtocolLimitsUpdateV1::new(JINDO_PROTOCOL,
                incoming.checked_add(1).ok_or_else(|| eyre!("policy height overflowed"))?, bad_limits),
            "protocol policy cannot exceed the canonical hard ceiling").await
            .expect_err("oversized Jindo policy was accepted");
        ensure!(error_chain_contains(&invalid_limit_error, "privacy protocol-limit update rejected"),
            "oversized Jindo policy failed for wrong reason: {invalid_limit_error:?}");
        wait_for_all_peer_policies(&network, incoming, compiled_snapshot, None,
            "invalid limits preserve absent optional policy").await?;

        let scheduled_at_height = next_execution_height(&client).await?;
        let effective_at_height = scheduled_at_height.checked_add(1)
            .ok_or_else(|| eyre!("policy height overflowed"))?;
        submit_instruction(&client, SchedulePrivacyProtocolLimitsUpdateV1::new(
            JINDO_PROTOCOL, effective_at_height, restricted_limits), "schedule ordinary Jindo limits").await?;
        let pending_policy = PrivacyProtocolPolicyV1 { protocol_id: JINDO_PROTOCOL,
            current_limits: compiled_snapshot.protocol_limits,
            pending_update: Some(PrivacyProtocolLimitsUpdateV1 {
                scheduled_at_height, effective_at_height, next_limits: restricted_limits }) };
        wait_for_all_peer_policies(&network, scheduled_at_height, compiled_snapshot, Some(pending_policy),
            "exact pending ordinary Jindo policy on all peers").await?;
        submit_instruction(&client, Log::new(Level::INFO, "apply scheduled Jindo limits".to_owned()),
            "execute exact Jindo policy boundary").await?;
        let restricted_policy = PrivacyProtocolPolicyV1 { protocol_id: JINDO_PROTOCOL,
            current_limits: restricted_limits, pending_update: None };
        wait_for_all_peer_policies(&network, effective_at_height, compiled_snapshot, Some(restricted_policy),
            "scheduled Jindo limits apply at the exact boundary").await?;
        let restricted_action = build_jindo_action(&client, genesis_hash, 2).await?;
        let limit_error = submit_signed_transaction(&client, &restricted_action,
            "four-polynomial native action exceeds ordinary limit three").await
            .expect_err("native Jindo action exceeded committed protocol limits");
        ensure!(error_chain_contains(&limit_error, "privacy statement violates active protocol limits"),
            "restricted action failed for wrong reason: {limit_error:?}");

        let scheduled_at_height = next_execution_height(&client).await?;
        let effective_at_height = scheduled_at_height.checked_add(1)
            .ok_or_else(|| eyre!("policy height overflowed"))?;
        submit_instruction(&client, SchedulePrivacyProtocolLimitsUpdateV1::new(JINDO_PROTOCOL,
            effective_at_height, compiled_snapshot.protocol_limits),
            "restore ordinary Jindo limits within the same canonical ceiling").await?;
        let pending_restore = PrivacyProtocolPolicyV1 { pending_update: Some(PrivacyProtocolLimitsUpdateV1 {
            scheduled_at_height, effective_at_height, next_limits: compiled_snapshot.protocol_limits }),
            ..restricted_policy };
        wait_for_all_peer_policies(&network, scheduled_at_height, compiled_snapshot, Some(pending_restore),
            "authorized ordinary increase remains pending until its exact block").await?;
        submit_instruction(&client, Log::new(Level::INFO, "restore scheduled Jindo limits".to_owned()),
            "execute ordinary Jindo increase boundary").await?;
        let restored_policy = PrivacyProtocolPolicyV1 { protocol_id: JINDO_PROTOCOL,
            current_limits: compiled_snapshot.protocol_limits, pending_update: None };
        wait_for_all_peer_policies(&network, effective_at_height, compiled_snapshot, Some(restored_policy),
            "ordinary Jindo increase applies without profile rebinding").await?;
        let final_action = build_jindo_action(&client, genesis_hash, 3).await?;
        submit_signed_transaction(&client, &final_action, "native Jindo succeeds under restored limits").await?;
        wait_for_transaction_on_peers(&preflight_clients, &final_action,
            "native Jindo finality before restart", PEER_CONVERGENCE_TIMEOUT).await?;
        let final_height = committed_height(&client, "native Jindo finality before restart").await?;
        let restart_index = network.peers().len() - 1;
        let restart_peer = network.peers()[restart_index].clone();
        let config_layers = network.config_layers().collect::<Vec<_>>();
        ensure!(
            restart_peer.shutdown_if_started().await,
            "selected Jindo peer was not running before restart coverage"
        );
        timeout(
            RESTART_TIMEOUT,
            restart_peer.start_checked(config_layers.iter(), None),
        )
        .await
        .map_err(|_| eyre!("Jindo peer restart exceeded {RESTART_TIMEOUT:?}"))?
        .wrap_err("restart Jindo peer")?;
        wait_for_all_peer_policies(
            &network,
            final_height,
            compiled_snapshot,
            Some(restored_policy),
            "post-restart native Jindo binding and ordinary policy",
        )
        .await?;
        ensure!(
            canonical_genesis_hash(&bounded_client(restart_peer.client())).await? == genesis_hash,
            "restarted peer derived a different canonical genesis hash"
        );
        let restarted_client = bounded_client(restart_peer.client());
        wait_for_transaction_on_peers(&[restarted_client.clone()], &default_action,
            "restarted peer restores default-policy native Jindo transaction", RESTART_TIMEOUT).await?;
        wait_for_transaction_on_peers(&[restarted_client.clone()], &final_action,
            "restarted peer restores governed native Jindo transaction", RESTART_TIMEOUT).await?;
        assert_exact_jindo_replay_rejected(&restarted_client, &default_action).await?;
        assert_exact_jindo_replay_rejected(&restarted_client, &final_action).await?;
        ensure!(committed_height(&restarted_client, "post-restart replay height").await? == final_height,
            "post-restart exact Jindo replay committed another block");
        println!(
            "TAIRA_PRIVACY_PROTOCOL_FOUR_PEER_CASE_V1:privacy_exact12_jindo_network::canonical_jindo_actions_survive_four_peer_policy_replay_and_restart:passed"
        );
        Ok(())
    }
    .await;
    network.shutdown().await;
    result
}
