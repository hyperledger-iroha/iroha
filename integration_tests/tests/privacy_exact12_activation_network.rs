//! Four-validator canonical privacy catalog and ordinary resource-policy coverage.
//! Available native profiles require no protocol registration. Unavailable implementations
//! retain their exact diagnostics. Authorized limit decreases and increases become effective
//! at committed block boundaries, survive cold restart, and preserve the native profiles.
//!
//! This scenario does not construct valid privacy proofs. Native proof/action scenarios own
//! that coverage; the generic X509 wire is explicitly rejected in favor of signed attachments.
//!
//! Run with `TEST_NETWORK_IROHAD_FEATURES=zk-stark IROHA_TEST_REQUIRE_NETWORK=1
//! IROHA_TEST_SERIALIZE_NETWORKS=1 cargo test --locked -p integration_tests
//! --test network_functional --features zk-stark
//! privacy_exact12_activation_network::canonical_exact12_policy_updates_survive_four_peer_replay_and_restart
//! -- --exact --nocapture --test-threads=1`.
use super::privacy_exact12_network_support::{privacy_capabilities, wait_for_transaction_on_peers};
use eyre::{Result, WrapErr as _, ensure, eyre};
use integration_tests::sandbox;
use iroha::{blocking::Client, client::FeeQuoteRequest};
use iroha_core_privacy::privacy_profiles::{
    CompiledPrivacyProfileErrorV1, CompiledPrivacyProfileV1,
    compiled_privacy_profile_snapshot_result_v1, compiled_privacy_profile_v1,
};
use iroha_data_model::{
    Level,
    isi::{
        Grant, InstructionBox, Log,
        privacy::{SchedulePrivacyProtocolLimitsUpdateV1, SubmitPrivacyProofV1},
    },
    permission::Permission,
    privacy::{
        JindoLimitsV1, PrivacyCompiledProfileResultV1, PrivacyCompiledProfileSnapshotV1,
        PrivacyCompiledProfileUnavailableReasonV1, PrivacyExact12CapabilityManifestV1,
        PrivacyProofEnvelopeV1, PrivacyProtocolIdV1, PrivacyProtocolLimitsUpdateV1,
        PrivacyProtocolLimitsV1, PrivacyProtocolPolicyV1, privacy_exact12_fixture_bundle_v1,
    },
    transaction::{
        FeePaymentIntent, SignedTransaction, TransactionBuilder,
        signed::PrivacyTransactionIntentErrorV1,
    },
};
use iroha_executor_data_model::permission::governance::CanEnactGovernance;
use iroha_model_base::metadata::Metadata;
use iroha_test_network::{NetworkBuilder, init_instruction_registry};
use iroha_test_samples::{ALICE_ID, ALICE_KEYPAIR, BOB_ID, BOB_KEYPAIR};
use std::time::Duration;
use tokio::time::{Instant, sleep, timeout};
const TEST_NAME: &str = "canonical_exact12_policy_updates_survive_four_peer_replay_and_restart";
const REQUIRED_DAEMON_FEATURE: &str = "zk-stark";
const SUBMISSION_TIMEOUT: Duration = Duration::from_secs(60);
const PEER_CONVERGENCE_TIMEOUT: Duration = Duration::from_secs(90);
const RESTART_TIMEOUT: Duration = Duration::from_secs(90);
const TEST_BLOCK_CADENCE: Duration = Duration::from_millis(100);
const TEST_NEXUS_LOCAL_STORAGE_BUDGET_BYTES: i64 = 1024 * 1024 * 1024;
const POLL_INTERVAL: Duration = Duration::from_millis(200);
// Each isolated ordinary submission executes in the next native certified global carrier.
const EXECUTION_CARRIER_BLOCKS: u64 = 1;
const ZK_AMS_PROTOCOL: PrivacyProtocolIdV1 = PrivacyProtocolIdV1::IrohaZkAmsV1;
const ZK_ACE_PROTOCOL: PrivacyProtocolIdV1 = PrivacyProtocolIdV1::ZkAcePqAuthorizationV1;
const VEGA_PROTOCOL: PrivacyProtocolIdV1 = PrivacyProtocolIdV1::VegaExistingCredentialZkV1;
const ZK_X509_PROTOCOL: PrivacyProtocolIdV1 = PrivacyProtocolIdV1::IrohaZkX509StarkP256V1;
const UNAVAILABLE_PROTOCOLS: [PrivacyProtocolIdV1; 4] = [
    ZK_ACE_PROTOCOL,
    ZK_AMS_PROTOCOL,
    VEGA_PROTOCOL,
    ZK_X509_PROTOCOL,
];
#[derive(Clone, Copy)]
struct ExpectedProtocolState {
    protocol_id: PrivacyProtocolIdV1,
    compiled_profile: PrivacyCompiledProfileResultV1,
    policy: Option<PrivacyProtocolPolicyV1>,
}
fn is_expected_unavailable(protocol_id: PrivacyProtocolIdV1) -> bool {
    UNAVAILABLE_PROTOCOLS.contains(&protocol_id)
}
// Pin actual implementation diagnostics; no resource policy can complete a missing relation.
fn expected_unavailable_reason(
    protocol_id: PrivacyProtocolIdV1,
) -> Option<PrivacyCompiledProfileUnavailableReasonV1> {
    match protocol_id {
        ZK_X509_PROTOCOL => {
            Some(PrivacyCompiledProfileUnavailableReasonV1::JointSecurityReductionIncomplete)
        }
        ZK_ACE_PROTOCOL | ZK_AMS_PROTOCOL | VEGA_PROTOCOL => {
            Some(PrivacyCompiledProfileUnavailableReasonV1::EngineUnavailable)
        }
        _ => None,
    }
}
fn expected_unavailable_error(
    protocol_id: PrivacyProtocolIdV1,
) -> Option<CompiledPrivacyProfileErrorV1> {
    match protocol_id {
        ZK_X509_PROTOCOL => {
            Some(CompiledPrivacyProfileErrorV1::JointSecurityReductionIncomplete { protocol_id })
        }
        ZK_ACE_PROTOCOL | ZK_AMS_PROTOCOL | VEGA_PROTOCOL => {
            Some(CompiledPrivacyProfileErrorV1::EngineUnavailable { protocol_id })
        }
        _ => None,
    }
}
#[test]
fn unavailable_profile_diagnostics_match_the_exact_compiled_catalog() {
    for protocol_id in PrivacyProtocolIdV1::ALL {
        let expected_error = expected_unavailable_error(protocol_id);
        assert_eq!(
            expected_error.is_some(),
            is_expected_unavailable(protocol_id)
        );
        assert_eq!(
            expected_unavailable_reason(protocol_id).is_some(),
            is_expected_unavailable(protocol_id)
        );
        if let Some(error) = expected_error {
            assert_eq!(compiled_privacy_profile_v1(protocol_id), Err(error));
            assert_eq!(
                compiled_privacy_profile_snapshot_result_v1(protocol_id),
                PrivacyCompiledProfileResultV1::Unavailable(
                    expected_unavailable_reason(protocol_id).unwrap()
                ),
            );
        }
    }
}
fn require_test_network_feature(feature: &str) -> Result<()> {
    let enabled = std::env::var("TEST_NETWORK_IROHAD_FEATURES")
        .ok()
        .is_some_and(|value| {
            value
                .split([',', ' ', '\t', '\n'])
                .any(|item| item.trim() == feature)
        });
    ensure!(
        enabled,
        "{TEST_NAME}: TEST_NETWORK_IROHAD_FEATURES must include `{feature}` so the four real \
         validators execute the same exact-12 availability catalog as the test harness"
    );
    Ok(())
}
fn bounded_client(client: Client) -> Client {
    integration_tests::sync::rebind_blocking_client(&client, |client| {
        client.transaction_status_timeout = SUBMISSION_TIMEOUT;
        client.torii_request_timeout = Duration::from_secs(20);
        client.transaction_ttl = Some(Duration::from_secs(3_600));
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
fn compiled_available_profiles() -> Result<Vec<CompiledPrivacyProfileV1>> {
    ensure!(
        PrivacyProtocolIdV1::COUNT == 12,
        "first-release exact-12 registry cardinality drifted to {}",
        PrivacyProtocolIdV1::COUNT
    );
    let mut profiles = Vec::new();
    for protocol_id in PrivacyProtocolIdV1::ALL {
        match compiled_privacy_profile_v1(protocol_id) {
            Ok(profile) => {
                ensure!(
                    !is_expected_unavailable(protocol_id),
                    "fail-closed protocol `{}` unexpectedly became executable",
                    protocol_id.canonical_label()
                );
                profiles.push(profile);
            }
            Err(error) if Some(error) == expected_unavailable_error(protocol_id) => {
                ensure!(
                    compiled_privacy_profile_snapshot_result_v1(protocol_id)
                        == PrivacyCompiledProfileResultV1::Unavailable(
                            expected_unavailable_reason(protocol_id)
                                .expect("pinned unavailable protocol"),
                        ),
                    "fail-closed capability result drifted for `{}`",
                    protocol_id.canonical_label()
                );
            }
            Err(error) => {
                return Err(eyre!(
                    "canonical exact-12 profile `{}` has unexpected compiled status: {error:?}",
                    protocol_id.canonical_label()
                ));
            }
        }
    }
    ensure!(
        profiles.len() == PrivacyProtocolIdV1::COUNT - UNAVAILABLE_PROTOCOLS.len(),
        "available exact-12 profile count drifted: expected {}, got {}",
        PrivacyProtocolIdV1::COUNT - UNAVAILABLE_PROTOCOLS.len(),
        profiles.len()
    );
    for pair in profiles.windows(2) {
        let left = PrivacyProtocolIdV1::ALL
            .iter()
            .position(|protocol_id| *protocol_id == pair[0].protocol_id)
            .expect("compiled protocol belongs to exact-12 registry");
        let right = PrivacyProtocolIdV1::ALL
            .iter()
            .position(|protocol_id| *protocol_id == pair[1].protocol_id)
            .expect("compiled protocol belongs to exact-12 registry");
        ensure!(
            left < right,
            "available exact-12 profiles are not in canonical order"
        );
    }
    Ok(profiles)
}
fn expected_states(
    profiles: &[CompiledPrivacyProfileV1],
    policies: impl IntoIterator<Item = Option<PrivacyProtocolPolicyV1>>,
) -> Result<Vec<ExpectedProtocolState>> {
    let policies = policies.into_iter().collect::<Vec<_>>();
    ensure!(
        profiles.len() == PrivacyProtocolIdV1::COUNT - UNAVAILABLE_PROTOCOLS.len()
            && policies.len() == profiles.len(),
        "expected-state construction requires exactly {} available profiles and policies",
        PrivacyProtocolIdV1::COUNT - UNAVAILABLE_PROTOCOLS.len()
    );
    let mut available = profiles.iter().copied().zip(policies).peekable();
    let mut expected = Vec::with_capacity(PrivacyProtocolIdV1::COUNT);
    for protocol_id in PrivacyProtocolIdV1::ALL {
        let compiled_profile = compiled_privacy_profile_snapshot_result_v1(protocol_id);
        let policy = match compiled_profile {
            PrivacyCompiledProfileResultV1::Available(snapshot) => {
                let Some((compiled, policy)) = available.next() else {
                    return Err(eyre!(
                        "compiled profile list omitted available protocol `{}`",
                        protocol_id.canonical_label()
                    ));
                };
                ensure!(
                    compiled.protocol_id == protocol_id
                        && snapshot == PrivacyCompiledProfileSnapshotV1::from(compiled),
                    "compiled binding/order drifted for available protocol `{}`",
                    protocol_id.canonical_label()
                );
                policy
            }
            PrivacyCompiledProfileResultV1::Unavailable(reason)
                if Some(reason) == expected_unavailable_reason(protocol_id) =>
            {
                None
            }
            status => {
                return Err(eyre!(
                    "unexpected compiled status for `{}`: {status:?}",
                    protocol_id.canonical_label()
                ));
            }
        };
        expected.push(ExpectedProtocolState {
            protocol_id,
            compiled_profile,
            policy,
        });
    }
    ensure!(
        available.next().is_none(),
        "compiled profile list contains a non-canonical trailing entry"
    );
    Ok(expected)
}
fn assert_exact12_snapshot(
    snapshot: &PrivacyExact12CapabilityManifestV1,
    minimum_height: u64,
    expected: &[ExpectedProtocolState],
    context: &str,
) -> Result<()> {
    snapshot
        .validate()
        .wrap_err_with(|| format!("{context}: invalid privacy capability snapshot"))?;
    ensure!(
        snapshot.committed_height >= minimum_height,
        "{context}: committed height {} is below {minimum_height}",
        snapshot.committed_height
    );
    ensure!(
        expected.len() == PrivacyProtocolIdV1::COUNT
            && snapshot.protocols.len() == PrivacyProtocolIdV1::COUNT,
        "{context}: exact-12 row count drifted"
    );
    for (index, ((row, state), protocol_id)) in snapshot
        .protocols
        .iter()
        .zip(expected)
        .zip(PrivacyProtocolIdV1::ALL)
        .enumerate()
    {
        ensure!(
            row.protocol_id == protocol_id && state.protocol_id == protocol_id,
            "{context}: canonical protocol order drifted at row {index}: expected \
             {protocol_id:?}, snapshot={:?}, expected={:?}",
            row.protocol_id,
            state.protocol_id
        );
        ensure!(
            row.compiled_profile == state.compiled_profile,
            "{context}: immutable compiled binding drifted for `{}`: {:?}",
            protocol_id.canonical_label(),
            row.compiled_profile
        );
        ensure!(
            row.policy == state.policy,
            "{context}: committed resource policy drifted for `{}`: expected {:?}, got {:?}",
            protocol_id.canonical_label(),
            state.policy,
            row.policy
        );
    }
    Ok(())
}
async fn wait_for_identical_exact12_snapshots(
    clients: &[Client],
    minimum_height: u64,
    expected: &[ExpectedProtocolState],
    context: &str,
) -> Result<Vec<PrivacyExact12CapabilityManifestV1>> {
    ensure!(
        !clients.is_empty() && clients.len() <= 4,
        "{context}: exact-12 snapshot polling requires between one and four validator clients"
    );
    let deadline = Instant::now() + PEER_CONVERGENCE_TIMEOUT;
    let mut last_observed = Vec::new();
    loop {
        let mut snapshots = Vec::with_capacity(clients.len());
        last_observed.clear();
        for (index, client) in clients.iter().enumerate() {
            match privacy_capabilities(&client).await {
                Ok(snapshot) => {
                    match assert_exact12_snapshot(&snapshot, minimum_height, expected, context) {
                        Ok(()) => {
                            last_observed.push(format!(
                                "peer {index}: exact snapshot at height {}",
                                snapshot.committed_height
                            ));
                            snapshots.push(snapshot);
                        }
                        Err(error) => last_observed.push(format!(
                            "peer {index}: snapshot mismatch at height {}: {error}",
                            snapshot.committed_height
                        )),
                    }
                }
                Err(error) => last_observed.push(format!("peer {index}: query failed: {error}")),
            }
        }
        if snapshots.len() == clients.len() {
            let canonical = &snapshots[0];
            if snapshots
                .iter()
                .skip(1)
                .all(|snapshot| snapshot == canonical)
            {
                return Ok(snapshots);
            }
            last_observed.push(format!(
                "valid exact-12 snapshots are not yet byte-for-byte identical; heights={:?}",
                snapshots
                    .iter()
                    .map(|snapshot| snapshot.committed_height)
                    .collect::<Vec<_>>()
            ));
        }
        if Instant::now() >= deadline {
            return Err(eyre!(
                "{context}: {} validators did not converge within \
                 {PEER_CONVERGENCE_TIMEOUT:?}; {}",
                clients.len(),
                last_observed.join("; ")
            ));
        }
        sleep(POLL_INTERVAL).await;
    }
}
async fn next_execution_height(client: &Client) -> Result<u64> {
    privacy_capabilities(&client)
        .await
        .wrap_err("query committed height before native transaction governed transaction")?
        .committed_height
        .checked_add(EXECUTION_CARRIER_BLOCKS)
        .ok_or_else(|| eyre!("native transaction privacy-governance execution height overflowed"))
}
async fn instruction_transaction(
    client: &Client,
    instruction: impl Into<InstructionBox>,
) -> Result<SignedTransaction> {
    instructions_transaction(client, vec![instruction.into()]).await
}
async fn instructions_transaction(
    client: &Client,
    instructions: Vec<InstructionBox>,
) -> Result<SignedTransaction> {
    let account = client.account_client();
    let mut payload = account.prepare_transaction(iroha::client::AccountTransactionDraft::new(
        instructions,
        no_fee(),
        Metadata::default(),
    ))?;
    // The default genesis charges ordinary instructions. Quote the same payload before
    // signing so negative policy probes reach their intended validator, not a missing fee cap.
    let quote = account
        .quote_fees(FeeQuoteRequest::AccountSignature { payload: &payload })
        .await
        .wrap_err("quote exact network-test payload")?;
    quote
        .validate_for_draft(&payload)
        .map_err(|error| eyre!(error))?;
    payload.fee_payment = quote.intent.clone();
    quote
        .validate_for_signed_payload(&payload)
        .map_err(|error| eyre!(error))?;
    account.sign_transaction(payload).map_err(Into::into)
}
async fn unavailable_protocol_transaction(
    client: &Client,
    mut envelope: PrivacyProofEnvelopeV1,
) -> Result<SignedTransaction> {
    // Keep one exact client-owned payload while deriving the self-referential fields. Building a
    // second client transaction would change its time/nonce and stale the resulting intent.
    envelope.statement.context_mut().network_id = *client.client().network_id();
    envelope.statement_digest = envelope
        .statement
        .digest()
        .wrap_err("bind unavailable-protocol fixture to the actual network")?;
    let draft =
        instruction_transaction(client, SubmitPrivacyProofV1::new(envelope.clone())).await?;
    bind_unavailable_fixture_transaction(draft, envelope, client.client().key_pair().private_key())
}

fn bind_unavailable_fixture_transaction(
    draft: SignedTransaction,
    mut envelope: PrivacyProofEnvelopeV1,
    private_key: &iroha_crypto::PrivateKey,
) -> Result<SignedTransaction> {
    let payload = draft.payload().clone();
    if envelope.protocol_id == ZK_X509_PROTOCOL {
        // This deliberate invalid-wire probe must reach the node. Generic X509 has no
        // valid generic intent: the signed payload is rejected before native ISI execution.
        ensure!(
            matches!(
                payload.privacy_transaction_intent_binding_if_present_v1(),
                Err(PrivacyTransactionIntentErrorV1::X509RequiresSignedAttachment)
            ),
            "generic X509 fixture lost its dedicated signed-attachment refusal"
        );
        return Ok(draft);
    }
    let intent = payload
        .privacy_transaction_intent_digest_v1()
        .wrap_err("derive unavailable-protocol transaction intent")?;
    envelope.statement.context_mut().transaction_intent_digest = intent;
    envelope.statement_digest = envelope
        .statement
        .digest()
        .wrap_err("derive unavailable-protocol statement digest")?;
    let transaction = TransactionBuilder::from_payload(payload)
        .wrap_err("reopen unavailable-protocol payload")?
        .with_instructions([SubmitPrivacyProofV1::new(envelope)])
        .try_sign(private_key)
        .wrap_err("sign unavailable-protocol transaction")?;
    ensure!(
        transaction
            .payload()
            .validate_privacy_transaction_intent_binding_v1()
            .wrap_err("validate unavailable-protocol transaction intent")?
            == intent,
        "unavailable-protocol transaction retained a foreign intent"
    );
    Ok(transaction)
}
#[test]
fn unavailable_fixture_transactions_preserve_native_admission_boundaries() -> Result<()> {
    let bundle = privacy_exact12_fixture_bundle_v1()?;
    for protocol_id in UNAVAILABLE_PROTOCOLS {
        let row = bundle
            .rows
            .iter()
            .find(|row| row.protocol_id == protocol_id)
            .expect("exact12 fixture includes every protocol");
        let envelope: PrivacyProofEnvelopeV1 = norito::decode_from_bytes(&row.envelope_norito)?;
        let network_id = envelope.statement.context().network_id;
        let mut builder = TransactionBuilder::new(network_id, ALICE_ID.clone(), no_fee())
            .with_instructions([SubmitPrivacyProofV1::new(envelope.clone())]);
        builder.set_ttl(Duration::from_secs(3_600));
        let draft = builder.try_sign(ALICE_KEYPAIR.private_key())?;
        let draft_hash = draft.hash();
        let draft_fee = draft.fee_payment_intent().clone();
        let transaction =
            bind_unavailable_fixture_transaction(draft, envelope, ALICE_KEYPAIR.private_key())?;
        transaction.verify_signature()?;
        assert_eq!(transaction.fee_payment_intent(), &draft_fee);
        assert_eq!(transaction.payload().network_id(), Some(&network_id));
        if protocol_id == ZK_X509_PROTOCOL {
            assert_eq!(
                transaction.hash(),
                draft_hash,
                "retain the exact invalid wire probe"
            );
            assert!(matches!(
                transaction
                    .payload()
                    .privacy_transaction_intent_binding_if_present_v1(),
                Err(PrivacyTransactionIntentErrorV1::X509RequiresSignedAttachment)
            ));
        } else {
            transaction
                .payload()
                .validate_privacy_transaction_intent_binding_v1()?;
            assert_ne!(
                transaction.hash(),
                draft_hash,
                "bind the fixture to its signed intent"
            );
        }
    }
    Ok(())
}

async fn submit_signed_transaction(
    client: &Client,
    transaction: &SignedTransaction,
    context: &str,
) -> Result<iroha_crypto::HashOf<SignedTransaction>> {
    timeout(
        SUBMISSION_TIMEOUT,
        client
            .account_client()
            .submit_transaction_and_wait(transaction),
    )
    .await
    .map_err(|_| eyre!("{context}: signed transaction exceeded {SUBMISSION_TIMEOUT:?}"))?
    .wrap_err_with(|| context.to_owned())
}
async fn submit_instruction(
    client: &Client,
    instruction: impl Into<InstructionBox>,
    context: &str,
) -> Result<iroha_crypto::HashOf<SignedTransaction>> {
    let transaction = instruction_transaction(client, instruction).await?;
    submit_signed_transaction(client, &transaction, context).await
}
fn assert_unreleased_profiles_unavailable(
    snapshot: &PrivacyExact12CapabilityManifestV1,
    minimum_height: u64,
    context: &str,
) -> Result<()> {
    snapshot
        .validate()
        .wrap_err_with(|| format!("{context}: invalid capability snapshot"))?;
    ensure!(
        snapshot.committed_height >= minimum_height,
        "{context}: committed height {} is below {minimum_height}",
        snapshot.committed_height
    );
    for protocol_id in UNAVAILABLE_PROTOCOLS {
        let row = snapshot
            .protocols
            .iter()
            .find(|row| row.protocol_id == protocol_id)
            .ok_or_else(|| {
                eyre!(
                    "{context}: capability snapshot omitted unavailable protocol `{}`",
                    protocol_id.canonical_label()
                )
            })?;
        ensure!(
            row.compiled_profile
                == PrivacyCompiledProfileResultV1::Unavailable(
                    expected_unavailable_reason(protocol_id).expect("pinned unavailable protocol"),
                ),
            "{context}: compiled status unexpectedly changed for `{}`: {:?}",
            protocol_id.canonical_label(),
            row.compiled_profile
        );
        ensure!(
            row.policy.is_none(),
            "{context}: unavailable protocol `{}` unexpectedly has a policy: {:?}",
            protocol_id.canonical_label(),
            row.policy
        );
    }
    Ok(())
}
async fn wait_for_identical_unreleased_profiles(
    clients: &[Client],
    minimum_height: u64,
    context: &str,
) -> Result<Vec<PrivacyExact12CapabilityManifestV1>> {
    let deadline = Instant::now() + PEER_CONVERGENCE_TIMEOUT;
    let mut last_observed = Vec::new();
    loop {
        let mut snapshots = Vec::with_capacity(clients.len());
        last_observed.clear();
        for (index, client) in clients.iter().enumerate() {
            match privacy_capabilities(&client).await {
                Ok(snapshot) => {
                    match assert_unreleased_profiles_unavailable(&snapshot, minimum_height, context)
                    {
                        Ok(()) => {
                            last_observed.push(format!(
                                "peer {index}: unavailable at height {}",
                                snapshot.committed_height
                            ));
                            snapshots.push(snapshot);
                        }
                        Err(error) => last_observed.push(format!("peer {index}: {error}")),
                    }
                }
                Err(error) => {
                    last_observed.push(format!("peer {index}: query failed: {error}"));
                }
            }
        }
        if snapshots.len() == clients.len()
            && snapshots
                .iter()
                .skip(1)
                .all(|snapshot| snapshot == &snapshots[0])
        {
            return Ok(snapshots);
        }
        if Instant::now() >= deadline {
            return Err(eyre!(
                "{context}: unavailable snapshots did not converge within \
                 {PEER_CONVERGENCE_TIMEOUT:?}; {}",
                last_observed.join("; ")
            ));
        }
        sleep(POLL_INTERVAL).await;
    }
}
#[test]
fn all_unreleased_profiles_fail_closed_across_four_peer_restart() -> Result<()> {
    super::privacy_exact12_network_support::run_network_case(
        "all_unreleased_profiles_fail_closed_across_four_peer_restart",
        all_unreleased_profiles_fail_closed_across_four_peer_restart_impl,
    )
}
async fn all_unreleased_profiles_fail_closed_across_four_peer_restart_impl() -> Result<()> {
    require_test_network_feature(REQUIRED_DAEMON_FEATURE)?;
    init_instruction_registry();
    for protocol_id in UNAVAILABLE_PROTOCOLS {
        let unavailable =
            expected_unavailable_error(protocol_id).expect("pinned unavailable protocol");
        let actual = compiled_privacy_profile_v1(protocol_id);
        ensure!(
            actual == Err(unavailable),
            "closed-profile diagnostic changed for `{}`: {actual:?}",
            protocol_id.canonical_label()
        );
        ensure!(
            compiled_privacy_profile_snapshot_result_v1(protocol_id)
                == PrivacyCompiledProfileResultV1::Unavailable(
                    expected_unavailable_reason(protocol_id).expect("pinned unavailable protocol"),
                ),
            "local capability result for `{}` is not the exact fail-closed status",
            protocol_id.canonical_label()
        );
    }
    let builder = NetworkBuilder::new()
        .with_peers(4)
        .with_auto_populated_trusted_peers()
        .with_block_cadence(TEST_BLOCK_CADENCE)
        .with_permissioned_consensus()
        .with_config_layer(|layer| {
            // Keep the production handshake enabled while minimizing its supported
            // puzzle cost so this four-peer gate isolates consensus progress.
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
    let context = stringify!(all_unreleased_profiles_fail_closed_across_four_peer_restart);
    let network = sandbox::start_network_async_or_skip(builder, context)
        .await?
        .ok_or_else(|| {
            eyre!(
                "unavailable-profile coverage requires four real validators; sandbox did not start"
            )
        })?;
    let result: Result<()> = async {
        ensure!(
            network.peers().len() == 4,
            "unreleased-profile test requires exactly four validators"
        );
        let all_clients = network
            .peers()
            .iter()
            .map(|peer| bounded_client(peer.client()))
            .collect::<Vec<_>>();
        let client = all_clients[0].clone();
        let initial_height = privacy_capabilities(&client).await
            .wrap_err("query initial unavailable-profile capability state")?
            .committed_height;
        wait_for_identical_unreleased_profiles(
            &all_clients,
            initial_height,
            "ZK-ACE, ZK-AMS, Vega, and ZK-X509 must begin unavailable and unregistered",
        )
        .await?;
        let bundle = privacy_exact12_fixture_bundle_v1()
            .wrap_err("construct canonical Exact12 fixture bundle")?;
        let mut unavailable_actions = Vec::with_capacity(UNAVAILABLE_PROTOCOLS.len());
        for protocol_id in UNAVAILABLE_PROTOCOLS {
            let fixture_row = bundle
                .rows
                .iter()
                .find(|row| row.protocol_id == protocol_id)
                .ok_or_else(|| {
                    eyre!(
                        "Exact12 fixture bundle omitted `{}`",
                        protocol_id.canonical_label()
                    )
                })?;
            let envelope: PrivacyProofEnvelopeV1 =
                norito::decode_from_bytes(&fixture_row.envelope_norito).wrap_err_with(|| {
                    format!(
                        "decode canonical `{}` fixture envelope",
                        protocol_id.canonical_label()
                    )
                })?;
            let action = unavailable_protocol_transaction(&client, envelope).await?;
            let action_error = submit_signed_transaction(
                &client,
                &action,
                &format!(
                    "unreleased `{}` production action must reject",
                    protocol_id.canonical_label()
                ),
            )
            .await
            .expect_err("unreleased privacy production action was accepted");
            ensure!(
                error_chain_contains(
                    &action_error,
                    &if protocol_id == ZK_X509_PROTOCOL {
                        PrivacyTransactionIntentErrorV1::X509RequiresSignedAttachment.to_string()
                    } else {
                        expected_unavailable_error(protocol_id).expect("pinned reason").to_string()
                    }
                ),
                "production action for `{}` rejected for wrong reason: {action_error:?}",
                protocol_id.canonical_label()
            );
            unavailable_actions.push((protocol_id, action));
        }
        let pre_restart_height = privacy_capabilities(&client).await
            .wrap_err("query unavailable-profile state before restart")?
            .committed_height;
        wait_for_identical_unreleased_profiles(
            &all_clients,
            pre_restart_height,
            "unavailable-profile policy and action rejections must preserve closed state",
        )
        .await?;
        let restart_index = all_clients.len() - 1;
        let restart_peer = network.peers()[restart_index].clone();
        let config_layers = network.config_layers().collect::<Vec<_>>();
        ensure!(
            restart_peer.shutdown_if_started().await,
            "selected unavailable-profile validator was not running before restart"
        );
        let sentinel = instruction_transaction(
            &client,
            Log::new(
                Level::INFO,
                "unavailable-profile restart sentinel".to_owned(),
            ),
        ).await?;
        submit_signed_transaction(
            &client,
            &sentinel,
            "commit unavailable-profile restart sentinel",
        )
        .await?;
        wait_for_transaction_on_peers(
            &all_clients[..restart_index],
            &sentinel,
            "healthy-validator unavailable-profile sentinel convergence",
            PEER_CONVERGENCE_TIMEOUT,
        )
        .await?;
        timeout(
            RESTART_TIMEOUT,
            restart_peer.start_checked(config_layers.iter(), None),
        )
        .await
        .map_err(|_| eyre!("unavailable-profile validator restart exceeded {RESTART_TIMEOUT:?}"))?
        .wrap_err("restart unavailable-profile validator")?;
        wait_for_transaction_on_peers(
            &all_clients,
            &sentinel,
            "post-restart unavailable-profile sentinel convergence",
            PEER_CONVERGENCE_TIMEOUT,
        )
        .await?;
        let final_height = privacy_capabilities(&client).await
            .wrap_err("query final unavailable-profile capability state")?
            .committed_height;
        wait_for_identical_unreleased_profiles(
            &all_clients,
            final_height,
            "ZK-ACE, ZK-AMS, Vega, and ZK-X509 closed status must survive validator restart",
        )
        .await?;
        let restarted_client = bounded_client(restart_peer.client());
        for (protocol_id, action) in &unavailable_actions {
            let replay_error = submit_signed_transaction(
                &restarted_client,
                action,
                &format!(
                    "post-restart unreleased `{}` action must reject",
                    protocol_id.canonical_label()
                ),
            )
            .await
            .expect_err("post-restart unreleased privacy action was accepted");
            ensure!(
                error_chain_contains(
                    &replay_error,
                    &if *protocol_id == ZK_X509_PROTOCOL {
                        PrivacyTransactionIntentErrorV1::X509RequiresSignedAttachment.to_string()
                    } else {
                        expected_unavailable_error(*protocol_id).expect("pinned reason").to_string()
                    }
                ),
                "post-restart action for `{}` rejected for wrong reason: {replay_error:?}",
                protocol_id.canonical_label()
            );
        }
        println!(
            "TAIRA_PRIVACY_PROTOCOL_FOUR_PEER_CASE_V1:privacy_exact12_activation_network::all_unreleased_profiles_fail_closed_across_four_peer_restart:passed"
        );
        Ok(())
    }
    .await;
    network.shutdown().await;
    result
}

fn jindo_limits(count: u32) -> PrivacyProtocolLimitsV1 {
    PrivacyProtocolLimitsV1::IrohaJindoPolynomialCommitmentV1(JindoLimitsV1 {
        max_polynomial_count: count,
    })
}

fn expected_with_jindo_policy(
    profiles: &[CompiledPrivacyProfileV1],
    policy: Option<PrivacyProtocolPolicyV1>,
) -> Result<Vec<ExpectedProtocolState>> {
    expected_states(
        profiles,
        profiles.iter().map(|profile| {
            if profile.protocol_id == PrivacyProtocolIdV1::IrohaJindoPolynomialCommitmentV1 {
                policy
            } else {
                None
            }
        }),
    )
}

async fn advance_policy_height(client: &Client, target: u64) -> Result<()> {
    // Idle chains make no blocks: every step is a real signed ordinary transaction.
    for _ in 0..16 {
        let height = privacy_capabilities(client).await?.committed_height;
        if height == target {
            return Ok(());
        }
        ensure!(
            height < target,
            "policy update overshot exact target height {target}"
        );
        submit_instruction(
            client,
            Log::new(
                Level::INFO,
                format!("privacy-policy carrier {} for {target}", height + 1),
            ),
            "commit ordinary policy-boundary carrier",
        )
        .await?;
    }
    Err(eyre!(
        "policy boundary did not reach {target} within 16 signed carriers"
    ))
}

#[test]
fn canonical_policy_expectations_preserve_profiles_and_future_limits() {
    let profiles = compiled_available_profiles().expect("exact native catalog");
    let protocol_id = PrivacyProtocolIdV1::IrohaJindoPolynomialCommitmentV1;
    let defaults = PrivacyProtocolLimitsV1::canonical_default_v1(protocol_id);
    let policy = PrivacyProtocolPolicyV1 {
        protocol_id,
        current_limits: defaults,
        pending_update: Some(PrivacyProtocolLimitsUpdateV1 {
            private_settlement_drain_blocks: None,
            scheduled_at_height: 2,
            effective_at_height: 10,
            next_limits: jindo_limits(1),
        }),
    };
    policy
        .validate_at_committed_height(2)
        .expect("ordinary future decrease");
    let expected = expected_with_jindo_policy(&profiles, Some(policy)).unwrap();
    assert_eq!(expected.len(), PrivacyProtocolIdV1::COUNT);
    for state in expected {
        assert_eq!(
            state.compiled_profile,
            compiled_privacy_profile_snapshot_result_v1(state.protocol_id)
        );
        assert_eq!(
            state.policy,
            (state.protocol_id == protocol_id).then_some(policy)
        );
    }
    let increased = PrivacyProtocolPolicyV1 {
        current_limits: jindo_limits(1),
        pending_update: Some(PrivacyProtocolLimitsUpdateV1 {
            private_settlement_drain_blocks: None,
            scheduled_at_height: 10,
            effective_at_height: 13,
            next_limits: defaults,
        }),
        ..policy
    };
    increased
        .validate_at_committed_height(10)
        .expect("ordinary future increase");
}

#[test]
fn canonical_exact12_policy_updates_survive_four_peer_replay_and_restart() -> Result<()> {
    super::privacy_exact12_network_support::run_network_case(
        TEST_NAME,
        canonical_exact12_policy_updates_survive_four_peer_replay_and_restart_impl,
    )
}

async fn canonical_exact12_policy_updates_survive_four_peer_replay_and_restart_impl() -> Result<()>
{
    require_test_network_feature(REQUIRED_DAEMON_FEATURE)?;
    init_instruction_registry();
    let profiles = compiled_available_profiles()?;
    let builder = NetworkBuilder::new()
        .with_peers(4)
        .with_auto_populated_trusted_peers()
        .with_block_cadence(TEST_BLOCK_CADENCE)
        .with_permissioned_consensus()
        // Alice remains unauthorized until an existing holder delegates this exact token.
        .with_genesis_instruction(Grant::account_permission(
            Permission::from(CanEnactGovernance),
            BOB_ID.clone(),
        ))
        .with_config_layer(|layer| {
            layer.write(
                ["nexus", "storage", "local_budget_bytes"],
                TEST_NEXUS_LOCAL_STORAGE_BUDGET_BYTES,
            );
        });
    let network = sandbox::start_network_async_or_skip(builder, TEST_NAME)
        .await?
        .ok_or_else(|| {
            eyre!("policy coverage requires four real validators; sandbox did not start")
        })?;
    let result: Result<()> = async {
        ensure!(
            network.peers().len() == 4,
            "policy test requires exactly four validators"
        );
        let all_clients = network
            .peers()
            .iter()
            .map(|peer| bounded_client(peer.client()))
            .collect::<Vec<_>>();
        let client = all_clients[0].clone();
        let protocol_id = PrivacyProtocolIdV1::IrohaJindoPolynomialCommitmentV1;
        let defaults = PrivacyProtocolLimitsV1::canonical_default_v1(protocol_id);
        ensure!(
            defaults != jindo_limits(1),
            "Jindo fixture requires a nontrivial safe decrease"
        );
        let absent = expected_with_jindo_policy(&profiles, None)?;
        let initial_height = privacy_capabilities(&client).await?.committed_height;
        let initial = wait_for_identical_exact12_snapshots(
            &all_clients,
            initial_height,
            &absent,
            "canonical profiles are available without registered protocol policies",
        )
        .await?;
        let immutable_consensus_policy = initial[0].consensus_policy;
        for snapshot in &initial {
            for row in &snapshot.protocols {
                ensure!(
                    row.is_network_available()
                        == matches!(
                            row.compiled_profile,
                            PrivacyCompiledProfileResultV1::Available(_)
                        ),
                    "a documentary prerequisite changed native availability"
                );
            }
        }
        let unauthorized = submit_instruction(
            &client,
            SchedulePrivacyProtocolLimitsUpdateV1::new(
                protocol_id,
                next_execution_height(&client).await? + 4,
                jindo_limits(1),
            ),
            "ordinary resource update still requires CanEnactGovernance",
        )
        .await
        .expect_err("unauthorized policy update was accepted");
        ensure!(
            error_chain_contains(&unauthorized, "not permitted: CanEnactGovernance"),
            "unauthorized update failed for wrong reason: {unauthorized:?}"
        );
        wait_for_identical_exact12_snapshots(
            &all_clients,
            initial_height,
            &absent,
            "unauthorized update must not create a policy",
        )
        .await?;
        let governor = bounded_client(
            network.peers()[0].client_for(&BOB_ID, BOB_KEYPAIR.private_key().clone()),
        );
        submit_instruction(
            &governor,
            Grant::account_permission(
                Permission::from(CanEnactGovernance),
                client.client().account().clone(),
            ),
            "grant ordinary resource-policy permission",
        )
        .await?;

        // Each invalid update is its own signed transaction. No test relies on exceeding
        // the unchanged per-transaction privacy action limit to simulate rollback.
        for (limits, future) in [
            (jindo_limits(0), true),
            (jindo_limits(u32::MAX), true),
            (defaults, true),
            (
                PrivacyProtocolLimitsV1::canonical_default_v1(
                    PrivacyProtocolIdV1::VeRangeTransparentRangeV1,
                ),
                true,
            ),
            (jindo_limits(1), false),
        ] {
            let incoming = next_execution_height(&client).await?;
            let effective = if future { incoming + 4 } else { incoming };
            let error = submit_instruction(
                &client,
                SchedulePrivacyProtocolLimitsUpdateV1::new(protocol_id, effective, limits),
                "invalid limits, no-op, foreign tag or current-height schedule must reject",
            )
            .await
            .expect_err("invalid policy update was accepted");
            ensure!(
                error_chain_contains(&error, "privacy protocol-limit update rejected"),
                "policy failed outside its semantic validator: {error:?}"
            );
            let after_rejection = privacy_capabilities(&client).await?.committed_height;
            wait_for_identical_exact12_snapshots(
                &all_clients,
                after_rejection,
                &absent,
                "invalid update must leave every profile and absent policy unchanged",
            )
            .await?;
        }

        let scheduled = next_execution_height(&client).await?;
        let effective = scheduled
            .checked_add(8)
            .ok_or_else(|| eyre!("policy height overflow"))?;
        let decrease = PrivacyProtocolPolicyV1 {
            protocol_id,
            current_limits: defaults,
            pending_update: Some(PrivacyProtocolLimitsUpdateV1 {
                private_settlement_drain_blocks: None,
                scheduled_at_height: scheduled,
                effective_at_height: effective,
                next_limits: jindo_limits(1),
            }),
        };
        let schedule_transaction = instruction_transaction(
            &client,
            SchedulePrivacyProtocolLimitsUpdateV1::new(protocol_id, effective, jindo_limits(1)),
        )
        .await?;
        let accepted_hash = submit_signed_transaction(
            &client,
            &schedule_transaction,
            "schedule bounded limit decrease without registration",
        )
        .await?;
        ensure!(
            *accepted_hash.as_ref() == *schedule_transaction.hash().as_ref(),
            "signed update hash changed"
        );
        let pending = expected_with_jindo_policy(&profiles, Some(decrease))?;
        wait_for_identical_exact12_snapshots(
            &all_clients,
            scheduled,
            &pending,
            "future update must preserve current default limits",
        )
        .await?;
        let duplicate = instruction_transaction(
            &client,
            SchedulePrivacyProtocolLimitsUpdateV1::new(protocol_id, effective + 1, jindo_limits(1)),
        )
        .await?;
        ensure!(
            duplicate.hash() != schedule_transaction.hash(),
            "fresh duplicate must have a distinct intent"
        );
        let duplicate_error =
            submit_signed_transaction(&client, &duplicate, "second pending update must reject")
                .await
                .expect_err("pending policy overwritten");
        ensure!(
            error_chain_contains(&duplicate_error, "already has a pending limit update"),
            "pending collision failed for wrong reason: {duplicate_error:?}"
        );
        let stopped_height = privacy_capabilities(&client).await?.committed_height;
        ensure!(
            stopped_height < effective,
            "pending fixture reached its boundary too early"
        );
        let stopped = wait_for_identical_exact12_snapshots(
            &all_clients,
            stopped_height,
            &pending,
            "pending-update rejection must preserve complete state",
        )
        .await?;
        ensure!(
            stopped
                .iter()
                .all(|s| s.consensus_policy == immutable_consensus_policy),
            "protocol scheduling changed global consensus limits"
        );
        // Torii acknowledges an authenticated committed replay idempotently; the
        // same hash and unchanged finalized state prove it was not executed again.
        let replay_hash = submit_signed_transaction(
            &client,
            &schedule_transaction,
            "exact committed schedule replay must acknowledge its existing result",
        )
        .await?;
        ensure!(
            replay_hash == accepted_hash,
            "replay changed the signed transaction hash"
        );
        wait_for_transaction_on_peers(
            &all_clients,
            &schedule_transaction,
            "exact committed schedule replay identity",
            PEER_CONVERGENCE_TIMEOUT,
        )
        .await?;
        let after_replay = wait_for_identical_exact12_snapshots(
            &all_clients,
            stopped_height,
            &pending,
            "idempotent replay preserves pending state",
        )
        .await?;
        ensure!(
            after_replay == stopped,
            "exact replay changed finalized capability state"
        );
        ensure!(
            privacy_capabilities(&client).await?.committed_height == stopped_height,
            "exact transaction replay created a block"
        );

        let restart_index = all_clients.len() - 1;
        let restart_peer = network.peers()[restart_index].clone();
        let config_layers = network.config_layers().collect::<Vec<_>>();
        ensure!(
            restart_peer.shutdown_if_started().await,
            "pending-policy validator was not running"
        );
        timeout(
            RESTART_TIMEOUT,
            restart_peer.start_checked(config_layers.iter(), None),
        )
        .await
        .map_err(|_| eyre!("pending-policy cold restart timed out"))??;
        let persisted = wait_for_identical_exact12_snapshots(
            std::slice::from_ref(&all_clients[restart_index]),
            stopped_height,
            &pending,
            "cold restart must recover the exact future schedule",
        )
        .await?;
        ensure!(
            persisted[0].committed_height == stopped_height
                && persisted[0].consensus_policy == immutable_consensus_policy,
            "pending-policy cold restore changed committed state"
        );
        ensure!(
            restart_peer.shutdown_if_started().await,
            "restarted validator was not running"
        );
        let healthy = &all_clients[..restart_index];
        advance_policy_height(&client, effective - 1).await?;
        wait_for_identical_exact12_snapshots(
            healthy,
            effective - 1,
            &pending,
            "pending decrease must remain unapplied through the preceding block",
        )
        .await?;
        advance_policy_height(&client, effective).await?;
        let reduced = PrivacyProtocolPolicyV1 {
            current_limits: jindo_limits(1),
            pending_update: None,
            ..decrease
        };
        let reduced_expectation = expected_with_jindo_policy(&profiles, Some(reduced))?;
        let reduced_snapshots = wait_for_identical_exact12_snapshots(
            healthy,
            effective,
            &reduced_expectation,
            "three-validator quorum applies decrease only at the exact future boundary",
        )
        .await?;
        ensure!(
            reduced_snapshots
                .iter()
                .all(|s| s.committed_height == effective),
            "decrease boundary differs across peers"
        );

        let increase_scheduled = next_execution_height(&client).await?;
        let increase_effective = increase_scheduled + 3;
        let increase_transaction = instruction_transaction(
            &client,
            SchedulePrivacyProtocolLimitsUpdateV1::new(protocol_id, increase_effective, defaults),
        )
        .await?;
        submit_signed_transaction(
            &client,
            &increase_transaction,
            "schedule restoration to canonical limits",
        )
        .await?;
        let increased_pending = PrivacyProtocolPolicyV1 {
            pending_update: Some(PrivacyProtocolLimitsUpdateV1 {
                private_settlement_drain_blocks: None,
                scheduled_at_height: increase_scheduled,
                effective_at_height: increase_effective,
                next_limits: defaults,
            }),
            ..reduced
        };
        wait_for_identical_exact12_snapshots(
            healthy,
            increase_scheduled,
            &expected_with_jindo_policy(&profiles, Some(increased_pending))?,
            "future increase preserves reduced limits until its boundary",
        )
        .await?;
        advance_policy_height(&client, increase_effective - 1).await?;
        wait_for_identical_exact12_snapshots(
            healthy,
            increase_effective - 1,
            &expected_with_jindo_policy(&profiles, Some(increased_pending))?,
            "pending increase must remain unapplied through the preceding block",
        )
        .await?;
        advance_policy_height(&client, increase_effective).await?;
        let restored = PrivacyProtocolPolicyV1 {
            current_limits: defaults,
            pending_update: None,
            ..reduced
        };
        let final_expected = expected_with_jindo_policy(&profiles, Some(restored))?;
        wait_for_identical_exact12_snapshots(
            healthy,
            increase_effective,
            &final_expected,
            "ordinary authorized increase returns to canonical bounds",
        )
        .await?;
        timeout(
            RESTART_TIMEOUT,
            restart_peer.start_checked(config_layers.iter(), None),
        )
        .await
        .map_err(|_| eyre!("policy catch-up restart timed out"))??;
        wait_for_transaction_on_peers(
            &all_clients,
            &increase_transaction,
            "post-restart signed update finality",
            PEER_CONVERGENCE_TIMEOUT,
        )
        .await?;
        let recovered = wait_for_identical_exact12_snapshots(
            &all_clients,
            increase_effective,
            &final_expected,
            "fourth validator replays both policy boundaries and recovers identical state",
        )
        .await?;
        ensure!(
            recovered
                .iter()
                .all(|s| s.committed_height == increase_effective
                    && s.consensus_policy == immutable_consensus_policy),
            "policy catch-up changed height or consensus policy"
        );
        let restarted = bounded_client(restart_peer.client());
        let replay = submit_signed_transaction(
            &restarted,
            &schedule_transaction,
            "cold-restored peer must acknowledge the original committed result without execution",
        )
        .await?;
        ensure!(
            replay == accepted_hash,
            "cold-restored replay changed the signed transaction hash"
        );
        wait_for_transaction_on_peers(
            &all_clients,
            &schedule_transaction,
            "cold-restored original schedule identity",
            PEER_CONVERGENCE_TIMEOUT,
        )
        .await?;
        let after_replay = wait_for_identical_exact12_snapshots(
            &all_clients,
            increase_effective,
            &final_expected,
            "cold-restored idempotent replay preserves complete state",
        )
        .await?;
        ensure!(
            after_replay == recovered,
            "cold-restored replay changed finalized state"
        );
        ensure!(
            privacy_capabilities(&restarted).await? == recovered[restart_index],
            "idempotent replay mutated cold-restored capability state"
        );
        Ok(())
    }
    .await;
    network.shutdown().await;
    result
}
