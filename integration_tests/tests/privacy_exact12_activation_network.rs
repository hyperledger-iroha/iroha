#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! Four-validator governance lifecycle coverage for the complete canonical
//! first-release exact-12 privacy registry. Eight executable profiles retain
//! positive governance coverage; ZK-ACE, ZK-AMS, Vega, and ZK-X509 remain
//! explicitly unavailable and fail closed on every validator.
//!
//! This scenario deliberately does not construct privacy proofs. The isolated
//! release-evidence runner owns native-engine proof coverage; this test proves
//! that consensus persists, activates, replays, and recovers the exact
//! compiled governance bindings identically on all validators.
//!
//! Release gate:
//! ```text
//! TEST_NETWORK_IROHAD_FEATURES=zk-stark IROHA_TEST_REQUIRE_NETWORK=1 \
//! IROHA_TEST_SERIALIZE_NETWORKS=1 cargo test --locked -p integration_tests \
//! --test network_functional --features zk-stark \
//! privacy_exact12_activation_network::canonical_exact12_governance_survives_four_peer_activation_replay_and_restart \
//! -- --exact --nocapture --test-threads=1
//! ```
use eyre::{Result, WrapErr as _, ensure, eyre};
use integration_tests::sandbox;
use iroha::client::Client;
use iroha_core::{
    privacy::PRIVACY_MIN_ACTIVATION_DELAY_BLOCKS_V1,
    privacy_profiles::{
        CompiledPrivacyProfileErrorV1, CompiledPrivacyProfileV1,
        compiled_privacy_profile_snapshot_result_v1, compiled_privacy_profile_v1,
        zk_ams_release_candidate_profile_material_v1,
        zk_x509_release_candidate_profile_material_v1,
    },
};
use iroha_data_model::{
    Level,
    isi::{
        Grant, InstructionBox, Log,
        privacy::{RegisterPrivacyProtocolActivationV1, SubmitPrivacyProofV1},
    },
    metadata::Metadata,
    permission::Permission,
    prelude::QueryBuilderExt,
    privacy::{
        PrivacyActiveLifecycleV1, PrivacyCompiledProfileResultV1, PrivacyCompiledProfileSnapshotV1,
        PrivacyCompiledProfileUnavailableReasonV1, PrivacyEngineIdV1,
        PrivacyExact12CapabilityManifestV1, PrivacyProofEnvelopeV1, PrivacyProofSystemIdV1,
        PrivacyProposedLifecycleV1, PrivacyProtocolActivationLimitsV1,
        PrivacyProtocolActivationRecordV1, PrivacyProtocolIdV1, PrivacyProtocolLifecycleV1,
        privacy_exact12_fixture_bundle_v1,
    },
    query::transaction::prelude::FindTransactions,
    transaction::{FeePaymentIntent, SignedTransaction, TransactionBuilder, TransactionEntrypoint},
};
use iroha_executor_data_model::permission::governance::CanEnactGovernance;
use iroha_test_network::{NetworkBuilder, init_instruction_registry};
use std::time::Duration;
use tokio::time::{Instant, sleep, timeout};
const TEST_NAME: &str =
    "canonical_exact12_governance_survives_four_peer_activation_replay_and_restart";
const REQUIRED_DAEMON_FEATURE: &str = "zk-stark";
const SUBMISSION_TIMEOUT: Duration = Duration::from_secs(60);
const PEER_CONVERGENCE_TIMEOUT: Duration = Duration::from_secs(90);
const RESTART_TIMEOUT: Duration = Duration::from_secs(90);
const ACTIVATION_ADVANCE_TIMEOUT: Duration = Duration::from_secs(300);
const TEST_BLOCK_CADENCE: Duration = Duration::from_millis(100);
const POLL_INTERVAL: Duration = Duration::from_millis(200);
const ZK_AMS_PROTOCOL: PrivacyProtocolIdV1 = PrivacyProtocolIdV1::IrohaZkAmsV1;
const ZK_ACE_PROTOCOL: PrivacyProtocolIdV1 = PrivacyProtocolIdV1::ZkAcePqAuthorizationV0;
const VEGA_PROTOCOL: PrivacyProtocolIdV1 = PrivacyProtocolIdV1::VegaExistingCredentialZkV0;
const ZK_X509_PROTOCOL: PrivacyProtocolIdV1 = PrivacyProtocolIdV1::IrohaZkX509StarkP256V0;
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
    activation: Option<PrivacyProtocolActivationRecordV1>,
}
fn is_expected_unavailable(protocol_id: PrivacyProtocolIdV1) -> bool {
    UNAVAILABLE_PROTOCOLS.contains(&protocol_id)
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
fn bounded_client(mut client: Client) -> Client {
    client.transaction_status_timeout = SUBMISSION_TIMEOUT;
    client.torii_request_timeout = Duration::from_secs(20);
    client.transaction_ttl = Some(Duration::from_secs(3_600));
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
            Err(CompiledPrivacyProfileErrorV1::EngineUnavailable {
                protocol_id: unavailable,
            }) if unavailable == protocol_id && is_expected_unavailable(protocol_id) => {
                ensure!(
                    compiled_privacy_profile_snapshot_result_v1(protocol_id)
                        == PrivacyCompiledProfileResultV1::Unavailable(
                            PrivacyCompiledProfileUnavailableReasonV1::EngineUnavailable,
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
    activations: impl IntoIterator<Item = Option<PrivacyProtocolActivationRecordV1>>,
) -> Result<Vec<ExpectedProtocolState>> {
    let activations = activations.into_iter().collect::<Vec<_>>();
    ensure!(
        profiles.len() == PrivacyProtocolIdV1::COUNT - UNAVAILABLE_PROTOCOLS.len()
            && activations.len() == profiles.len(),
        "expected-state construction requires exactly {} available profiles and activations",
        PrivacyProtocolIdV1::COUNT - UNAVAILABLE_PROTOCOLS.len()
    );
    let mut available = profiles.iter().copied().zip(activations).peekable();
    let mut expected = Vec::with_capacity(PrivacyProtocolIdV1::COUNT);
    for protocol_id in PrivacyProtocolIdV1::ALL {
        let compiled_profile = compiled_privacy_profile_snapshot_result_v1(protocol_id);
        let activation = match compiled_profile {
            PrivacyCompiledProfileResultV1::Available(snapshot) => {
                let Some((compiled, activation)) = available.next() else {
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
                activation
            }
            PrivacyCompiledProfileResultV1::Unavailable(
                PrivacyCompiledProfileUnavailableReasonV1::EngineUnavailable,
            ) if is_expected_unavailable(protocol_id) => None,
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
            activation,
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
            row.activation == state.activation,
            "{context}: governed lifecycle/binding drifted for `{}`: expected {:?}, got {:?}",
            protocol_id.canonical_label(),
            state.activation,
            row.activation
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
            match client.get_privacy_capabilities() {
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
fn next_incoming_height(client: &Client) -> Result<u64> {
    client
        .get_privacy_capabilities()
        .wrap_err("query committed height before governed transaction")?
        .committed_height
        .checked_add(1)
        .ok_or_else(|| eyre!("incoming privacy-governance height overflowed"))
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
fn instruction_transaction(
    client: &Client,
    instruction: impl Into<InstructionBox>,
) -> SignedTransaction {
    client.build_transaction([instruction.into()], no_fee(), Metadata::default())
}
fn intent_bound_privacy_transaction(
    client: &Client,
    mut envelope: PrivacyProofEnvelopeV1,
) -> Result<SignedTransaction> {
    // Keep one exact client-owned payload while deriving the self-referential fields. Building a
    // second client transaction would change its time/nonce and stale the resulting intent.
    let draft = instruction_transaction(client, SubmitPrivacyProofV1::new(envelope.clone()));
    let payload = draft.payload().clone();
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
        .try_sign(client.key_pair.private_key())
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
async fn submit_signed_transaction(
    client: &Client,
    transaction: &SignedTransaction,
    context: &str,
) -> Result<iroha_crypto::HashOf<SignedTransaction>> {
    let client = client.clone();
    let transaction = transaction.clone();
    timeout(
        SUBMISSION_TIMEOUT,
        tokio::task::spawn_blocking(move || client.submit_transaction_blocking(&transaction)),
    )
    .await
    .map_err(|_| eyre!("{context}: signed transaction exceeded {SUBMISSION_TIMEOUT:?}"))?
    .map_err(|error| eyre!("{context}: submission task failed: {error}"))?
    .wrap_err_with(|| context.to_owned())
}
async fn submit_instruction(
    client: &Client,
    instruction: impl Into<InstructionBox>,
    context: &str,
) -> Result<iroha_crypto::HashOf<SignedTransaction>> {
    let transaction = instruction_transaction(client, instruction);
    submit_signed_transaction(client, &transaction, context).await
}
async fn advance_to_exact_height(client: &Client, target_height: u64) -> Result<()> {
    let start = client
        .get_privacy_capabilities()
        .wrap_err("query height before deterministic exact-12 activation advance")?
        .committed_height;
    ensure!(
        start <= target_height,
        "cannot advance backwards from committed height {start} to {target_height}"
    );
    if start < target_height {
        let first_incoming_height = start
            .checked_add(1)
            .ok_or_else(|| eyre!("exact-12 activation advance height overflowed"))?;
        for incoming_height in first_incoming_height..=target_height {
            submit_instruction(
                client,
                Log::new(
                    Level::INFO,
                    format!("exact-12 activation advance block {incoming_height}"),
                ),
                "advance exact-12 activation height",
            )
            .await?;
        }
    }
    let observed = client
        .get_privacy_capabilities()
        .wrap_err("query height after deterministic exact-12 activation advance")?
        .committed_height;
    ensure!(
        observed == target_height,
        "deterministic exact-12 activation advance landed at height {observed}, expected \
         {target_height}"
    );
    Ok(())
}
fn exact_applied_transaction_visible(
    client: &Client,
    transaction: &SignedTransaction,
) -> Result<bool> {
    let expected_hash = transaction.hash_as_entrypoint();
    let expected_entrypoint = TransactionEntrypoint::External(transaction.clone());
    let transactions = client
        .query(FindTransactions::new())
        .execute_all()
        .wrap_err("query finalized transactions")?;
    let Some(committed) = transactions
        .iter()
        .find(|committed| committed.entrypoint_hash() == &expected_hash)
    else {
        return Ok(false);
    };
    ensure!(
        committed.entrypoint() == &expected_entrypoint,
        "entrypoint hash matched different transaction bytes"
    );
    ensure!(
        committed.result().0.is_ok(),
        "exact-12 catch-up sentinel is visible but finalized as rejected"
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
                    last_observed.push(format!("peer {index}: exact transaction visible"));
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
                "{context}: finalized transaction did not converge within \
                 {PEER_CONVERGENCE_TIMEOUT:?}; {}",
                last_observed.join("; ")
            ));
        }
        sleep(POLL_INTERVAL).await;
    }
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
                    PrivacyCompiledProfileUnavailableReasonV1::EngineUnavailable,
                ),
            "{context}: compiled status unexpectedly changed for `{}`: {:?}",
            protocol_id.canonical_label(),
            row.compiled_profile
        );
        ensure!(
            row.activation.is_none(),
            "{context}: unavailable protocol `{}` unexpectedly has an activation: {:?}",
            protocol_id.canonical_label(),
            row.activation
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
            match client.get_privacy_capabilities() {
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
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn all_unreleased_profiles_fail_closed_across_four_peer_restart() -> Result<()> {
    require_test_network_feature(REQUIRED_DAEMON_FEATURE)?;
    init_instruction_registry();
    for protocol_id in UNAVAILABLE_PROTOCOLS {
        let unavailable = CompiledPrivacyProfileErrorV1::EngineUnavailable { protocol_id };
        ensure!(
            compiled_privacy_profile_v1(protocol_id) == Err(unavailable),
            "this closed-profile test must be replaced when `{}` becomes governance-available",
            protocol_id.canonical_label()
        );
        ensure!(
            compiled_privacy_profile_snapshot_result_v1(protocol_id)
                == PrivacyCompiledProfileResultV1::Unavailable(
                    PrivacyCompiledProfileUnavailableReasonV1::EngineUnavailable,
                ),
            "local capability result for `{}` is not the exact fail-closed status",
            protocol_id.canonical_label()
        );
    }
    let zk_ams_candidate = zk_ams_release_candidate_profile_material_v1()
        .wrap_err("derive deterministic but non-activatable ZK-AMS candidate profile")?;
    let zk_x509_candidate = zk_x509_release_candidate_profile_material_v1()
        .wrap_err("derive deterministic but non-activatable ZK-X509 candidate profile")?;
    let mut zk_ace_candidate = compiled_available_profiles()?
        .into_iter()
        .next()
        .ok_or_else(|| eyre!("exact-12 registry contains no executable control profile"))?;
    zk_ace_candidate.protocol_id = ZK_ACE_PROTOCOL;
    zk_ace_candidate.proof_system_id = PrivacyProofSystemIdV1::StarkFriSha256Goldilocks;
    zk_ace_candidate.engine_id = PrivacyEngineIdV1::NativeGoldilocksStarkFri;
    zk_ace_candidate.protocol_limits = PrivacyProtocolActivationLimitsV1::ZkAcePqAuthorizationV0;
    // This is a structurally valid activation probe, not a Vega profile candidate. The real
    // candidate stays sealed inside the release-evidence boundary until its authenticated keys
    // and independent vectors exist.
    let mut vega_candidate = zk_ace_candidate;
    vega_candidate.protocol_id = VEGA_PROTOCOL;
    vega_candidate.proof_system_id = PrivacyProofSystemIdV1::VegaNeutronNovaSpartanHyraxT256;
    vega_candidate.engine_id = PrivacyEngineIdV1::NativeVega;
    vega_candidate.protocol_limits = PrivacyProtocolActivationLimitsV1::VegaExistingCredentialZkV0;
    let builder = NetworkBuilder::new()
        .with_peers(4)
        .with_auto_populated_trusted_peers()
        .with_block_cadence(TEST_BLOCK_CADENCE)
        .with_permissioned_consensus()
        .with_config_layer(|layer| {
            layer.write(["zk", "stark", "enabled"], true);
        });
    let context = stringify!(all_unreleased_profiles_fail_closed_across_four_peer_restart);
    let Some(network) = sandbox::start_network_async_or_skip(builder, context).await? else {
        return Ok(());
    };
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
        let initial_height = client
            .get_privacy_capabilities()
            .wrap_err("query initial unavailable-profile capability state")?
            .committed_height;
        wait_for_identical_unreleased_profiles(
            &all_clients,
            initial_height,
            "ZK-ACE, ZK-AMS, Vega, and ZK-X509 must begin unavailable and unregistered",
        )
        .await?;
        let grant = instruction_transaction(
            &client,
            Grant::account_permission(
                Permission::from(CanEnactGovernance),
                client.account.clone(),
            ),
        );
        submit_signed_transaction(&client, &grant, "grant unavailable-profile governance permission")
            .await?;
        wait_for_transaction_on_peers(
            &all_clients,
            &grant,
            "unavailable-profile governance grant convergence",
        )
        .await?;
        for candidate in [
            zk_ace_candidate,
            zk_ams_candidate,
            vega_candidate,
            zk_x509_candidate,
        ] {
            let proposal_height = next_incoming_height(&client)?;
            let activation_height = proposal_height
                .checked_add(PRIVACY_MIN_ACTIVATION_DELAY_BLOCKS_V1)
                .ok_or_else(|| eyre!("unavailable-profile activation height overflowed"))?;
            let protocol_id = candidate.protocol_id;
            let proposed = proposed_activation(candidate, proposal_height, activation_height);
            proposed.validate().wrap_err_with(|| {
                format!(
                    "construct structurally valid `{}` activation probe",
                    protocol_id.canonical_label()
                )
            })?;
            let activation = instruction_transaction(
                &client,
                RegisterPrivacyProtocolActivationV1::new(proposed),
            );
            let activation_error = submit_signed_transaction(
                &client,
                &activation,
                &format!(
                    "unreleased `{}` candidate activation must reject",
                    protocol_id.canonical_label()
                ),
            )
            .await
            .expect_err("unreleased privacy candidate activation was accepted");
            ensure!(
                error_chain_contains(&activation_error, "does not match compiled native profile")
                    && error_chain_contains(&activation_error, "not governance-available"),
                "candidate activation for `{}` rejected for wrong reason: {activation_error:?}",
                protocol_id.canonical_label()
            );
        }
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
            let action = intent_bound_privacy_transaction(&client, envelope)?;
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
                error_chain_contains(&action_error, "privacy protocol")
                    && error_chain_contains(&action_error, "is not registered"),
                "production action for `{}` rejected for wrong reason: {action_error:?}",
                protocol_id.canonical_label()
            );
            unavailable_actions.push((protocol_id, action));
        }
        let pre_restart_height = client
            .get_privacy_capabilities()
            .wrap_err("query unavailable-profile state before restart")?
            .committed_height;
        wait_for_identical_unreleased_profiles(
            &all_clients,
            pre_restart_height,
            "unavailable-profile activation and action rejections must preserve closed state",
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
        );
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
        )
        .await?;
        let final_height = client
            .get_privacy_capabilities()
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
                error_chain_contains(&replay_error, "privacy protocol")
                    && error_chain_contains(&replay_error, "is not registered"),
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
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn canonical_exact12_governance_survives_four_peer_activation_replay_and_restart()
-> Result<()> {
    require_test_network_feature(REQUIRED_DAEMON_FEATURE)?;
    init_instruction_registry();
    let profiles = compiled_available_profiles()?;
    let builder = NetworkBuilder::new()
        .with_peers(4)
        .with_auto_populated_trusted_peers()
        .with_block_cadence(TEST_BLOCK_CADENCE)
        .with_permissioned_consensus()
        .with_config_layer(|layer| {
            layer.write(["zk", "stark", "enabled"], true);
        });
    let Some(network) = sandbox::start_network_async_or_skip(builder, TEST_NAME).await? else {
        return Ok(());
    };
    let result: Result<()> = async {
        ensure!(
            network.peers().len() == 4,
            "exact-12 lifecycle test requires exactly four trusted validators"
        );
        let all_clients = network
            .peers()
            .iter()
            .map(|peer| bounded_client(peer.client()))
            .collect::<Vec<_>>();
        let client = all_clients[0].clone();
        let absent = expected_states(&profiles, std::iter::repeat_n(None, profiles.len()))?;
        let initial_height = client
            .get_privacy_capabilities()
            .wrap_err("query initial exact-12 governance state")?
            .committed_height;
        let initial_snapshots = wait_for_identical_exact12_snapshots(
            &all_clients,
            initial_height,
            &absent,
            "available profiles must begin inactive while ZK-ACE, ZK-AMS, Vega, and ZK-X509 remain unavailable",
        )
        .await?;
        let immutable_consensus_policy = initial_snapshots[0].consensus_policy;
        let unauthorized_height = next_incoming_height(&client)?;
        let unauthorized_activation_height = unauthorized_height
            .checked_add(PRIVACY_MIN_ACTIVATION_DELAY_BLOCKS_V1)
            .ok_or_else(|| eyre!("unauthorized activation height overflowed"))?;
        let unauthorized_error = submit_instruction(
            &client,
            RegisterPrivacyProtocolActivationV1::new(proposed_activation(
                profiles[0],
                unauthorized_height,
                unauthorized_activation_height,
            )),
            "privacy activation registration without CanEnactGovernance must reject",
        )
        .await
        .expect_err("unauthorized exact-12 activation registration was accepted");
        ensure!(
            error_chain_contains(&unauthorized_error, "not permitted: CanEnactGovernance"),
            "unauthorized activation registration rejected for wrong reason: \
             {unauthorized_error:?}"
        );
        let post_unauthorized_height = client
            .get_privacy_capabilities()
            .wrap_err("query exact-12 state after unauthorized registration rejection")?
            .committed_height;
        wait_for_identical_exact12_snapshots(
            &all_clients,
            post_unauthorized_height,
            &absent,
            "unauthorized registration rejection must not register any available activation",
        )
        .await?;
        submit_instruction(
            &client,
            Grant::account_permission(Permission::from(CanEnactGovernance), client.account.clone()),
            "grant CanEnactGovernance for exact-12 activation",
        )
        .await?;
        let early_height = next_incoming_height(&client)?;
        let insufficient_activation_delay =
            PRIVACY_MIN_ACTIVATION_DELAY_BLOCKS_V1
                .checked_sub(1)
                .ok_or_else(|| eyre!("privacy activation delay must be positive"))?;
        let early_activation_height = early_height
            .checked_add(insufficient_activation_delay)
            .ok_or_else(|| eyre!("one-block-early activation height overflowed"))?;
        let early_error = submit_instruction(
            &client,
            RegisterPrivacyProtocolActivationV1::new(proposed_activation(
                profiles[0],
                early_height,
                early_activation_height,
            )),
            "one-block-early exact-12 activation must reject",
        )
        .await
        .expect_err("one-block-early exact-12 activation was accepted");
        ensure!(
            error_chain_contains(&early_error, "is too early"),
            "one-block-early activation rejected for wrong reason: {early_error:?}"
        );
        let post_early_height = client
            .get_privacy_capabilities()
            .wrap_err("query exact-12 state after one-block-early rejection")?
            .committed_height;
        wait_for_identical_exact12_snapshots(
            &all_clients,
            post_early_height,
            &absent,
            "one-block-early rejection must not register any available activation",
        )
        .await?;
        let tampered_height = next_incoming_height(&client)?;
        let tampered_activation_height = tampered_height
            .checked_add(PRIVACY_MIN_ACTIVATION_DELAY_BLOCKS_V1)
            .ok_or_else(|| eyre!("tampered activation height overflowed"))?;
        let mut substituted =
            proposed_activation(profiles[0], tampered_height, tampered_activation_height);
        ensure!(
            profiles[0].verifier_digest != profiles[1].verifier_digest,
            "cross-profile substitution fixture requires distinct verifier digests"
        );
        substituted.verifier_digest = profiles[1].verifier_digest;
        let tampered_error = submit_instruction(
            &client,
            RegisterPrivacyProtocolActivationV1::new(substituted),
            "cross-profile verifier binding substitution must reject",
        )
        .await
        .expect_err("cross-profile exact-12 activation substitution was accepted");
        ensure!(
            error_chain_contains(&tampered_error, "does not match compiled native profile"),
            "cross-profile activation substitution rejected for wrong reason: \
             {tampered_error:?}"
        );
        let post_tamper_height = client
            .get_privacy_capabilities()
            .wrap_err("query exact-12 state after substitution rejection")?
            .committed_height;
        wait_for_identical_exact12_snapshots(
            &all_clients,
            post_tamper_height,
            &absent,
            "rejected profile substitution must not register any activation",
        )
        .await?;
        let first_registration_height = next_incoming_height(&client)?;
        let final_registration_height = first_registration_height
            .checked_add(
                u64::try_from(profiles.len() - 1).expect("available profile count fits u64"),
            )
            .ok_or_else(|| eyre!("final available-profile registration height overflowed"))?;
        let activation_height = final_registration_height
            .checked_add(PRIVACY_MIN_ACTIVATION_DELAY_BLOCKS_V1)
            .ok_or_else(|| eyre!("shared exact-12 activation height overflowed"))?;
        let mut proposed_records = Vec::with_capacity(profiles.len());
        let mut first_proposal_transaction = None;
        for (index, compiled) in profiles.iter().copied().enumerate() {
            let expected_height = first_registration_height
                .checked_add(u64::try_from(index).expect("exact-12 index fits u64"))
                .ok_or_else(|| eyre!("exact-12 proposal height overflowed"))?;
            let observed_height = next_incoming_height(&client)?;
            ensure!(
                observed_height == expected_height,
                "proposal `{}` would land at height {observed_height}, expected deterministic \
                 height {expected_height}",
                compiled.protocol_id.canonical_label()
            );
            let proposed = proposed_activation(compiled, expected_height, activation_height);
            let transaction = instruction_transaction(
                &client,
                RegisterPrivacyProtocolActivationV1::new(proposed),
            );
            let submitted_hash = submit_signed_transaction(
                &client,
                &transaction,
                &format!(
                    "register exact compiled activation for `{}`",
                    compiled.protocol_id.canonical_label()
                ),
            )
            .await?;
            ensure!(
                *submitted_hash.as_ref() == *transaction.hash().as_ref(),
                "submitted proposal hash drifted for `{}`",
                compiled.protocol_id.canonical_label()
            );
            if index == 0 {
                first_proposal_transaction = Some(transaction);
            }
            proposed_records.push(proposed);
        }
        ensure!(
            client
                .get_privacy_capabilities()
                .wrap_err("query height after exact-12 proposals")?
                .committed_height
                == final_registration_height,
            "the available-profile proposals did not occupy their deterministic consecutive heights"
        );
        let proposed = expected_states(&profiles, proposed_records.iter().copied().map(Some))?;
        let proposed_snapshots = wait_for_identical_exact12_snapshots(
            &all_clients,
            final_registration_height,
            &proposed,
            "all available records are Proposed while unavailable rows remain fail-closed",
        )
        .await?;
        ensure!(
            proposed_snapshots.iter().all(|snapshot| snapshot
                .protocols
                .iter()
                .filter(|row| !is_expected_unavailable(row.protocol_id))
                .all(|row| row
                    .activation
                    .is_some_and(|record| !record.lifecycle.is_active()))),
            "an available pre-activation proposal was incorrectly exposed as Active"
        );
        let last_pre_activation_height = activation_height
            .checked_sub(1)
            .ok_or_else(|| eyre!("shared exact-12 activation height has no predecessor"))?;
        timeout(
            ACTIVATION_ADVANCE_TIMEOUT,
            advance_to_exact_height(&client, last_pre_activation_height),
        )
        .await
        .map_err(|_| {
            eyre!(
                "advancing through the exact {}-block activation lead exceeded \
                 {ACTIVATION_ADVANCE_TIMEOUT:?}",
                PRIVACY_MIN_ACTIVATION_DELAY_BLOCKS_V1
            )
        })??;
        let last_proposed_snapshots = wait_for_identical_exact12_snapshots(
            &all_clients,
            last_pre_activation_height,
            &proposed,
            "every available lifecycle remains Proposed through activation height minus one",
        )
        .await?;
        ensure!(
            last_proposed_snapshots.iter().all(|snapshot| snapshot
                .protocols
                .iter()
                .filter(|row| !is_expected_unavailable(row.protocol_id))
                .all(|row| row
                    .activation
                    .is_some_and(|record| !record.lifecycle.is_active()))),
            "an available lifecycle became Active before its governed height"
        );
        let restart_index = all_clients.len() - 1;
        let restart_peer = network.peers()[restart_index].clone();
        let config_layers = network.config_layers().collect::<Vec<_>>();
        ensure!(
            restart_peer.shutdown_if_started().await,
            "selected Proposed exact-12 validator was not running before activation catch-up \
             coverage"
        );
        timeout(
            RESTART_TIMEOUT,
            restart_peer.start_checked(config_layers.iter(), None),
        )
        .await
        .map_err(|_| {
            eyre!("pre-activation exact-12 persistence restart exceeded {RESTART_TIMEOUT:?}")
        })?
        .wrap_err("restart Proposed exact-12 validator from its persisted state")?;
        let persisted_proposed = wait_for_identical_exact12_snapshots(
            std::slice::from_ref(&all_clients[restart_index]),
            last_pre_activation_height,
            &proposed,
            "cold-restarted validator recovers available proposals and closed unavailable rows",
        )
        .await?;
        ensure!(
            persisted_proposed[0].committed_height == last_pre_activation_height,
            "cold-restarted validator recovered Proposed exact-12 state at height {}, expected \
             its exact stopped height {last_pre_activation_height}",
            persisted_proposed[0].committed_height
        );
        ensure!(
            persisted_proposed[0].consensus_policy == immutable_consensus_policy,
            "cold-restarted validator recovered a mutated privacy consensus policy"
        );
        ensure!(
            restart_peer.shutdown_if_started().await,
            "cold-restarted Proposed exact-12 validator was not running before the activation \
             quorum probe"
        );
        let healthy_clients = all_clients[..restart_index].to_vec();
        submit_instruction(
            &client,
            Log::new(
                Level::INFO,
                format!("exact-12 shared activation block {activation_height}"),
            ),
            "commit exact-12 shared activation block",
        )
        .await?;
        let active_records = profiles
            .iter()
            .copied()
            .zip(proposed_records.iter().copied())
            .map(|(compiled, proposed)| {
                let PrivacyProtocolLifecycleV1::Proposed(proposed_lifecycle) = proposed.lifecycle
                else {
                    unreachable!("locally constructed proposal has Proposed lifecycle");
                };
                compiled.activation_record(PrivacyProtocolLifecycleV1::Active(
                    PrivacyActiveLifecycleV1 {
                        proposed_at_height: proposed_lifecycle.proposed_at_height,
                        activated_at_height: activation_height,
                        state_since_height: activation_height,
                    },
                ))
            })
            .collect::<Vec<_>>();
        let active = expected_states(&profiles, active_records.iter().copied().map(Some))?;
        let active_snapshots = wait_for_identical_exact12_snapshots(
            &healthy_clients,
            activation_height,
            &active,
            "the three-validator quorum promotes every available profile while validator four \
             remains offline with Proposed state and unavailable rows stay closed",
        )
        .await?;
        ensure!(
            active_snapshots.iter().all(|snapshot| snapshot
                .protocols
                .iter()
                .filter(|row| !is_expected_unavailable(row.protocol_id))
                .all(|row| row
                    .activation
                    .is_some_and(|record| record.lifecycle.is_active()))),
            "an available lifecycle failed to become Active at the shared governed height"
        );
        ensure!(
            active_snapshots
                .iter()
                .all(|snapshot| snapshot.consensus_policy == immutable_consensus_policy),
            "exact-12 registration or activation mutated the privacy consensus policy"
        );
        let immutable_active_rows = active_snapshots[0].protocols.clone();
        let first_proposal_transaction = first_proposal_transaction
            .as_ref()
            .ok_or_else(|| eyre!("first exact-12 proposal transaction was not retained"))?;
        let mut duplicate_metadata = Metadata::default();
        duplicate_metadata.insert(
            "privacy_exact12_fresh_duplicate"
                .parse()
                .expect("static exact-12 duplicate metadata key is valid"),
            activation_height,
        );
        let fresh_duplicate_registration = client.build_transaction(
            [RegisterPrivacyProtocolActivationV1::new(
                proposed_records[0],
            )],
            no_fee(),
            duplicate_metadata,
        );
        ensure!(
            fresh_duplicate_registration.hash() != first_proposal_transaction.hash(),
            "fresh duplicate registration must have a distinct signed transaction hash"
        );
        let duplicate_error = submit_signed_transaction(
            &client,
            &fresh_duplicate_registration,
            "freshly signed duplicate exact-12 registration must reach governance and reject",
        )
        .await
        .expect_err("freshly signed duplicate exact-12 registration was accepted");
        ensure!(
            error_chain_contains(&duplicate_error, "is already registered"),
            "fresh duplicate registration rejected for wrong reason: {duplicate_error:?}"
        );
        let height_after_fresh_duplicate = client
            .get_privacy_capabilities()
            .wrap_err("query exact-12 height after fresh duplicate registration rejection")?
            .committed_height;
        let post_duplicate_snapshots = wait_for_identical_exact12_snapshots(
            &healthy_clients,
            height_after_fresh_duplicate,
            &active,
            "fresh duplicate rejection preserves active profiles and closed unavailable rows",
        )
        .await?;
        ensure!(
            post_duplicate_snapshots
                .iter()
                .all(|snapshot| snapshot.protocols == immutable_active_rows),
            "fresh duplicate registration mutated an exact-12 capability row"
        );
        ensure!(
            post_duplicate_snapshots
                .iter()
                .all(|snapshot| snapshot.consensus_policy == immutable_consensus_policy),
            "fresh duplicate registration mutated the privacy consensus policy"
        );
        let height_before_replay = client
            .get_privacy_capabilities()
            .wrap_err("query exact-12 height before proposal replay")?
            .committed_height;
        let replay_error = submit_signed_transaction(
            &client,
            first_proposal_transaction,
            "exact committed activation proposal replay must reject",
        )
        .await
        .expect_err("exact committed activation proposal replay was accepted");
        ensure!(
            is_exact_replay_error(&replay_error),
            "exact activation proposal replay rejected for wrong reason: {replay_error:?}"
        );
        ensure!(
            client
                .get_privacy_capabilities()
                .wrap_err("query exact-12 height after proposal replay")?
                .committed_height
                == height_before_replay,
            "exact activation proposal replay unexpectedly committed another block"
        );
        let expected_catch_up_height = height_before_replay
            .checked_add(1)
            .ok_or_else(|| eyre!("exact-12 catch-up height overflowed"))?;
        let catch_up_transaction = instruction_transaction(
            &client,
            Log::new(
                Level::INFO,
                format!("exact-12 restart catch-up block {expected_catch_up_height}"),
            ),
        );
        let catch_up_hash = submit_signed_transaction(
            &client,
            &catch_up_transaction,
            "commit exact-12 restart catch-up sentinel",
        )
        .await?;
        ensure!(
            *catch_up_hash.as_ref() == *catch_up_transaction.hash().as_ref(),
            "submitted catch-up sentinel hash differs from its signed transaction"
        );
        wait_for_identical_exact12_snapshots(
            &healthy_clients,
            expected_catch_up_height,
            &active,
            "healthy validators preserve active profiles and closed unavailable rows",
        )
        .await?;
        wait_for_transaction_on_peers(
            &healthy_clients,
            &catch_up_transaction,
            "healthy-validator exact-12 catch-up sentinel finality",
        )
        .await?;
        timeout(
            RESTART_TIMEOUT,
            restart_peer.start_checked(config_layers.iter(), None),
        )
        .await
        .map_err(|_| eyre!("exact-12 validator restart exceeded {RESTART_TIMEOUT:?}"))?
        .wrap_err("restart exact-12 validator")?;
        let recovered = wait_for_identical_exact12_snapshots(
            &all_clients,
            expected_catch_up_height,
            &active,
            "restarted validator catches up active profiles and closed unavailable rows",
        )
        .await?;
        wait_for_transaction_on_peers(
            &all_clients,
            &catch_up_transaction,
            "post-restart exact-12 catch-up sentinel visibility",
        )
        .await?;
        ensure!(
            recovered[restart_index].protocols == immutable_active_rows,
            "restarted validator recovered different exact-12 bindings or lifecycles"
        );
        ensure!(
            recovered[restart_index].consensus_policy == immutable_consensus_policy,
            "restarted validator recovered a different privacy consensus policy"
        );
        ensure!(
            recovered
                .iter()
                .all(|snapshot| snapshot.committed_height == expected_catch_up_height),
            "four validators did not converge at the exact catch-up height"
        );
        Ok(())
    }
    .await;
    network.shutdown().await;
    result
}
