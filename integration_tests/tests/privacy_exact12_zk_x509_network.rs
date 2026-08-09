#![cfg(feature = "privacy-release-evidence")]
#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! Four-peer DA/RBC fail-closed coverage for governed ZK-X509 state while the
//! native profile remains deliberately unavailable to production governance.
//!
//! The test registers a complete active trust-anchor, certificate-policy, and
//! signed-CRL lineage through production instructions. It then proves that an
//! unpinned release candidate cannot be activated or used for certificate
//! actions, that candidate statements with substituted governance references
//! cannot bypass that outer boundary, and that the governance lineage survives
//! a validator restart. Reference-specific proof verification remains out of
//! scope until the canonical native action builder is released.
//!
//! Release gate:
//! ```text
//! TEST_NETWORK_IROHAD_FEATURES=zk-stark IROHA_TEST_REQUIRE_NETWORK=1 \
//! IROHA_TEST_SERIALIZE_NETWORKS=1 \
//! cargo test --locked -p integration_tests --test network_functional \
//! --features 'zk-stark privacy-release-evidence' \
//! privacy_exact12_zk_x509_network::zk_x509_governance_and_unreleased_actions_fail_closed_across_four_peer_restart \
//! -- --exact --nocapture --test-threads=1
//! ```

use std::{
    num::NonZeroU32,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use eyre::{Result, WrapErr as _, ensure, eyre};
use integration_tests::sandbox;
use iroha::{
    client::Client,
    data_model::{
        Level,
        isi::{
            Grant, InstructionBox, Log,
            privacy::{
                RegisterPrivacyProtocolActivationV1, RegisterPrivacyZkX509CertificatePolicyV1,
                RegisterPrivacyZkX509CrlV1, RegisterPrivacyZkX509TrustAnchorV1,
                RotatePrivacyZkX509CrlV1, SubmitPrivacyProofV1,
            },
        },
        metadata::Metadata,
        permission::Permission,
        prelude::{Name, QueryBuilderExt},
        privacy::{
            IrohaZkX509StarkP256StatementV1, PrivacyAttributeDigestV1, PrivacyCapabilitySnapshotV1,
            PrivacyCertificateKeyDigestV1, PrivacyChallengeV1, PrivacyCompiledProfileResultV1,
            PrivacyCompiledProfileUnavailableReasonV1, PrivacyConsensusLimitsV1, PrivacyIssuerIdV1,
            PrivacyNullifierV1, PrivacyPolicyDigestV1, PrivacyPolicyIdV1, PrivacyProofBytesV1,
            PrivacyProofEnvelopeV1, PrivacyProofV1, PrivacyProposedLifecycleV1,
            PrivacyProtocolIdV1, PrivacyProtocolLifecycleV1, PrivacyRootV1,
            PrivacyStatementContextV1, PrivacyStatementDigestV1, PrivacyStatementV1,
            PrivacyTransactionIntentDigestV1, PrivacyX509CrlDerDigestV1,
            PrivacyX509CrlIssuerSpkiDigestV1, PrivacyX509ExtendedKeyUsageV1,
            PrivacyX509KeyUsageRequirementV1, PrivacyX509KeyUsageV1, PrivacyX509TrustStoreDigestV1,
            PrivacyZkX509CertificatePolicyRecordV1, PrivacyZkX509CrlRecordDigestV1,
            PrivacyZkX509CrlRecordV1, PrivacyZkX509DisclosedAttributeV1,
            PrivacyZkX509RecordLifecycleV1, PrivacyZkX509TrustAnchorRecordV1,
        },
        query::{block::prelude::FindBlocks, transaction::prelude::FindTransactions},
        transaction::{
            FeePaymentIntent, SignedTransaction, TransactionBuilder, TransactionEntrypoint,
            TransactionPayload,
        },
    },
};
use iroha_core::{
    privacy::PRIVACY_MIN_ACTIVATION_DELAY_BLOCKS_V1,
    privacy_profiles::{
        CompiledPrivacyProfileErrorV1, CompiledPrivacyProfileV1,
        compiled_privacy_profile_snapshot_result_v1, compiled_privacy_profile_v1,
        zk_x509_release_candidate_profile_material_v1,
    },
};
use iroha_executor_data_model::permission::governance::CanEnactGovernance;
use iroha_test_network::{NetworkBuilder, init_instruction_registry};
use tokio::time::{Instant, sleep, timeout};

const TEST_NAME: &str =
    "zk_x509_governance_and_unreleased_actions_fail_closed_across_four_peer_restart";
const REQUIRED_DAEMON_FEATURE: &str = "zk-stark";
const ZK_X509_PROTOCOL: PrivacyProtocolIdV1 = PrivacyProtocolIdV1::IrohaZkX509StarkP256V0;
const SUBMISSION_TIMEOUT: Duration = Duration::from_secs(120);
const PEER_CONVERGENCE_TIMEOUT: Duration = Duration::from_secs(180);
const RESTART_TIMEOUT: Duration = Duration::from_secs(120);
const TEST_BLOCK_CADENCE: Duration = Duration::from_millis(100);
const POLL_INTERVAL: Duration = Duration::from_millis(250);
const ACTION_TTL: Duration = Duration::from_secs(3_600);

#[derive(Clone)]
struct ZkX509GovernanceFixtureV1 {
    trust_anchor: PrivacyZkX509TrustAnchorRecordV1,
    certificate_policy: PrivacyZkX509CertificatePolicyRecordV1,
    crl: PrivacyZkX509CrlRecordV1,
}

#[derive(Clone, Copy, Debug)]
enum CandidateStatementKindV1 {
    ExactGovernance,
    WrongTrustAnchor,
    WrongCertificatePolicy,
    WrongCrl,
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
         validators expose the STARK-capable production surface required by this gate"
    );
    Ok(())
}

fn bounded_client(mut client: Client) -> Client {
    client.transaction_status_timeout = SUBMISSION_TIMEOUT;
    client.torii_request_timeout = Duration::from_secs(30);
    client.transaction_ttl = Some(ACTION_TTL);
    client
}

fn no_fee() -> FeePaymentIntent {
    FeePaymentIntent::authority(Vec::new(), None)
}

fn now_duration() -> Result<Duration> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .wrap_err("system clock is before the Unix epoch")
}

fn latest_committed_block_unix_seconds(client: &Client) -> Result<u64> {
    let blocks = client
        .query(FindBlocks)
        .execute_all()
        .wrap_err("query trusted block timestamp for ZK-X509 fixture")?;
    let latest = blocks
        .first()
        .ok_or_else(|| eyre!("ZK-X509 fixture requires a committed genesis block"))?;
    Ok(latest.header().creation_time().as_secs())
}

fn error_chain_contains(error: &eyre::Report, needle: &str) -> bool {
    let needle = needle.to_ascii_lowercase();
    error
        .chain()
        .any(|cause| cause.to_string().to_ascii_lowercase().contains(&needle))
}

fn tagged_metadata(tag: u64) -> Result<Metadata> {
    let mut metadata = Metadata::default();
    metadata.insert("zk_x509_network_probe".parse::<Name>()?, tag);
    Ok(metadata)
}

fn instruction_transaction(
    client: &Client,
    instruction: impl Into<InstructionBox>,
    tag: u64,
) -> Result<SignedTransaction> {
    Ok(client.build_transaction([instruction.into()], no_fee(), tagged_metadata(tag)?))
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

async fn submit_expecting_rejection(
    client: &Client,
    transaction: &SignedTransaction,
    context: &str,
    unexpected_acceptance: &str,
) -> Result<eyre::Report> {
    match submit_signed_transaction(client, transaction, context).await {
        Ok(_) => Err(eyre!("{unexpected_acceptance}")),
        Err(error) => Ok(error),
    }
}

async fn submit_instruction(
    client: &Client,
    instruction: impl Into<InstructionBox>,
    tag: u64,
    context: &str,
) -> Result<(SignedTransaction, iroha_crypto::HashOf<SignedTransaction>)> {
    let transaction = instruction_transaction(client, instruction, tag)?;
    let hash = submit_signed_transaction(client, &transaction, context).await?;
    ensure!(
        *hash.as_ref() == *transaction.hash().as_ref(),
        "{context}: submitted hash differs from the signed transaction"
    );
    Ok((transaction, hash))
}

fn exact_transaction_result(
    client: &Client,
    transaction: &SignedTransaction,
) -> Result<Option<bool>> {
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
        return Ok(None);
    };
    ensure!(
        committed.entrypoint() == &expected_entrypoint,
        "entrypoint hash matched different transaction bytes"
    );
    Ok(Some(committed.result().0.is_ok()))
}

fn exact_applied_transaction_visible(
    client: &Client,
    transaction: &SignedTransaction,
) -> Result<bool> {
    match exact_transaction_result(client, transaction)? {
        Some(true) => Ok(true),
        Some(false) => Err(eyre!("expected applied transaction finalized as rejected")),
        None => Ok(false),
    }
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
                "{context}: transaction did not converge within {PEER_CONVERGENCE_TIMEOUT:?}; {}",
                last_observed.join("; ")
            ));
        }
        sleep(POLL_INTERVAL).await;
    }
}

async fn wait_for_transaction_result_on_peers(
    clients: &[Client],
    transaction: &SignedTransaction,
    expected_success: bool,
    context: &str,
) -> Result<()> {
    let deadline = Instant::now() + PEER_CONVERGENCE_TIMEOUT;
    let mut last_observed = Vec::new();
    loop {
        let mut matching = 0_usize;
        last_observed.clear();
        for (index, client) in clients.iter().enumerate() {
            match exact_transaction_result(client, transaction) {
                Ok(Some(success)) if success == expected_success => {
                    matching += 1;
                    last_observed.push(format!("peer {index}: expected result visible"));
                }
                Ok(Some(success)) => last_observed.push(format!(
                    "peer {index}: result success={success}, expected {expected_success}"
                )),
                Ok(None) => last_observed.push(format!("peer {index}: transaction absent")),
                Err(error) => last_observed.push(format!("peer {index}: {error}")),
            }
        }
        if matching == clients.len() {
            return Ok(());
        }
        if Instant::now() >= deadline {
            return Err(eyre!(
                "{context}: transaction result did not converge within \
                 {PEER_CONVERGENCE_TIMEOUT:?}; {}",
                last_observed.join("; ")
            ));
        }
        sleep(POLL_INTERVAL).await;
    }
}

fn assert_zk_x509_unavailable(
    snapshot: &PrivacyCapabilitySnapshotV1,
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
    let row = snapshot
        .protocols
        .iter()
        .find(|row| row.protocol_id == ZK_X509_PROTOCOL)
        .ok_or_else(|| eyre!("{context}: capability snapshot omitted ZK-X509"))?;
    ensure!(
        row.compiled_profile
            == PrivacyCompiledProfileResultV1::Unavailable(
                PrivacyCompiledProfileUnavailableReasonV1::EngineUnavailable,
            ),
        "{context}: ZK-X509 compiled status unexpectedly changed: {:?}",
        row.compiled_profile
    );
    ensure!(
        row.activation.is_none(),
        "{context}: unavailable ZK-X509 unexpectedly has a governance activation: {:?}",
        row.activation
    );
    Ok(())
}

async fn wait_for_identical_unavailable_snapshots(
    clients: &[Client],
    minimum_height: u64,
    context: &str,
) -> Result<Vec<PrivacyCapabilitySnapshotV1>> {
    let deadline = Instant::now() + PEER_CONVERGENCE_TIMEOUT;
    let mut last_observed = Vec::new();
    loop {
        let mut snapshots = Vec::with_capacity(clients.len());
        last_observed.clear();
        for (index, client) in clients.iter().enumerate() {
            match client.get_privacy_capabilities() {
                Ok(snapshot) => {
                    match assert_zk_x509_unavailable(&snapshot, minimum_height, context) {
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
                Err(error) => last_observed.push(format!("peer {index}: query failed: {error}")),
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

fn next_incoming_height(client: &Client) -> Result<u64> {
    client
        .get_privacy_capabilities()
        .wrap_err("query committed height before ZK-X509 governance transaction")?
        .committed_height
        .checked_add(1)
        .ok_or_else(|| eyre!("incoming ZK-X509 governance height overflowed"))
}

fn governance_fixture(now_unix_seconds: u64) -> Result<ZkX509GovernanceFixtureV1> {
    let trust_anchor_id = PrivacyIssuerIdV1::new([0x91; 32]);
    let policy_id = PrivacyPolicyIdV1::new([0x92; 32]);
    let trust_anchor = PrivacyZkX509TrustAnchorRecordV1::new(
        trust_anchor_id,
        1,
        PrivacyX509TrustStoreDigestV1::new([0xA1; 32]),
        PrivacyRootV1::new([0xA2; 32]),
        1,
        None,
        PrivacyZkX509RecordLifecycleV1::Active,
    )
    .wrap_err("construct canonical ZK-X509 trust-anchor origin")?;
    let key_usage = PrivacyX509KeyUsageV1 {
        digital_signature: PrivacyX509KeyUsageRequirementV1::new(true),
        content_commitment: PrivacyX509KeyUsageRequirementV1::new(false),
        key_encipherment: PrivacyX509KeyUsageRequirementV1::new(false),
        key_agreement: PrivacyX509KeyUsageRequirementV1::new(false),
    };
    let certificate_policy = PrivacyZkX509CertificatePolicyRecordV1::new(
        trust_anchor_id,
        policy_id,
        1,
        PrivacyPolicyDigestV1::new([0xA3; 32]),
        key_usage,
        vec![
            PrivacyX509ExtendedKeyUsageV1::ClientAuthentication,
            PrivacyX509ExtendedKeyUsageV1::WalletIdentity,
        ],
        vec![0, 3],
        None,
        PrivacyZkX509RecordLifecycleV1::Active,
    )
    .wrap_err("construct canonical ZK-X509 certificate-policy origin")?;
    let next_update_unix_seconds = now_unix_seconds
        .checked_add(3_600)
        .ok_or_else(|| eyre!("ZK-X509 CRL validity window overflowed"))?;
    let crl = PrivacyZkX509CrlRecordV1::new(
        trust_anchor_id,
        policy_id,
        1,
        1,
        PrivacyX509CrlDerDigestV1::new([0xA4; 32]),
        PrivacyX509CrlIssuerSpkiDigestV1::new([0xA5; 32]),
        now_unix_seconds,
        next_update_unix_seconds,
        None,
        PrivacyZkX509RecordLifecycleV1::Active,
    )
    .wrap_err("construct canonical ZK-X509 signed-CRL origin")?;
    Ok(ZkX509GovernanceFixtureV1 {
        trust_anchor,
        certificate_policy,
        crl,
    })
}

fn refresh_governance_fixture_crl(
    fixture: &mut ZkX509GovernanceFixtureV1,
    now_unix_seconds: u64,
) -> Result<()> {
    let refreshed = governance_fixture(now_unix_seconds)?;
    ensure!(
        refreshed.trust_anchor == fixture.trust_anchor,
        "refreshing the ZK-X509 CRL changed deterministic trust-anchor content"
    );
    ensure!(
        refreshed.certificate_policy == fixture.certificate_policy,
        "refreshing the ZK-X509 CRL changed deterministic certificate-policy content"
    );
    fixture.crl = refreshed.crl;
    Ok(())
}

fn missing_anchor_policy(
    fixture: &ZkX509GovernanceFixtureV1,
) -> Result<PrivacyZkX509CertificatePolicyRecordV1> {
    PrivacyZkX509CertificatePolicyRecordV1::new(
        PrivacyIssuerIdV1::new([0xE1; 32]),
        PrivacyPolicyIdV1::new([0xE2; 32]),
        1,
        PrivacyPolicyDigestV1::new([0xE3; 32]),
        fixture.certificate_policy.required_key_usage,
        fixture
            .certificate_policy
            .required_extended_key_usages
            .clone(),
        fixture
            .certificate_policy
            .required_disclosed_attribute_indices
            .clone(),
        None,
        PrivacyZkX509RecordLifecycleV1::Active,
    )
    .wrap_err("construct missing-anchor certificate-policy probe")
}

fn missing_policy_crl(
    fixture: &ZkX509GovernanceFixtureV1,
    now_unix_seconds: u64,
) -> Result<PrivacyZkX509CrlRecordV1> {
    PrivacyZkX509CrlRecordV1::new(
        fixture.trust_anchor.trust_anchor_id,
        PrivacyPolicyIdV1::new([0xE4; 32]),
        1,
        1,
        PrivacyX509CrlDerDigestV1::new([0xE5; 32]),
        PrivacyX509CrlIssuerSpkiDigestV1::new([0xE6; 32]),
        now_unix_seconds,
        now_unix_seconds
            .checked_add(3_600)
            .ok_or_else(|| eyre!("missing-policy CRL validity window overflowed"))?,
        None,
        PrivacyZkX509RecordLifecycleV1::Active,
    )
    .wrap_err("construct missing-policy signed-CRL probe")
}

fn candidate_statement(
    client: &Client,
    profile: CompiledPrivacyProfileV1,
    fixture: &ZkX509GovernanceFixtureV1,
    now_unix_seconds: u64,
    kind: CandidateStatementKindV1,
) -> Result<IrohaZkX509StarkP256StatementV1> {
    let mut statement = IrohaZkX509StarkP256StatementV1 {
        context: PrivacyStatementContextV1 {
            chain_id: client.chain.clone(),
            action_index: 0,
            transaction_intent_digest: PrivacyTransactionIntentDigestV1::new([0; 32]),
            parameter_id: profile.parameter_id,
            parameter_digest: profile.parameter_digest,
            verifier_digest: profile.verifier_digest,
            statement_schema_digest: profile.statement_schema_digest,
            engine_manifest_digest: profile.engine_manifest_digest,
        },
        trust_anchor_id: fixture.trust_anchor.trust_anchor_id,
        certificate_policy_id: fixture.certificate_policy.policy_id,
        trust_anchor_record_digest: fixture.trust_anchor.record_digest,
        trust_anchor_record_epoch: fixture.trust_anchor.record_epoch,
        certificate_policy_record_digest: fixture.certificate_policy.record_digest,
        certificate_policy_record_epoch: fixture.certificate_policy.record_epoch,
        crl_record_digest: fixture.crl.record_digest,
        crl_record_epoch: fixture.crl.record_epoch,
        subject_public_key_digest: PrivacyCertificateKeyDigestV1::new([0xB1; 32]),
        ca_membership_root: fixture.trust_anchor.ca_membership_root,
        ca_membership_root_epoch: fixture.trust_anchor.ca_membership_root_epoch,
        key_usage: fixture.certificate_policy.required_key_usage,
        extended_key_usages: fixture
            .certificate_policy
            .required_extended_key_usages
            .clone(),
        disclosed_attributes: vec![
            PrivacyZkX509DisclosedAttributeV1 {
                index: 0,
                attribute_digest: PrivacyAttributeDigestV1::new([0xB2; 32]),
            },
            PrivacyZkX509DisclosedAttributeV1 {
                index: 3,
                attribute_digest: PrivacyAttributeDigestV1::new([0xB3; 32]),
            },
        ],
        presentation_not_before_unix_seconds: now_unix_seconds,
        presentation_not_after_unix_seconds: now_unix_seconds
            .checked_add(120)
            .ok_or_else(|| eyre!("ZK-X509 presentation window overflowed"))?,
        wallet_account: client.account.clone(),
        wallet_challenge: PrivacyChallengeV1::new([0xB4; 32]),
        certificate_nullifier: PrivacyNullifierV1::new([0xB5; 32]),
    };
    match kind {
        CandidateStatementKindV1::ExactGovernance => {}
        CandidateStatementKindV1::WrongTrustAnchor => {
            statement.trust_anchor_id = PrivacyIssuerIdV1::new([0xC1; 32]);
            statement.trust_anchor_record_digest =
                iroha::data_model::privacy::PrivacyZkX509TrustAnchorRecordDigestV1::new([0xC2; 32]);
        }
        CandidateStatementKindV1::WrongCertificatePolicy => {
            statement.certificate_policy_id = PrivacyPolicyIdV1::new([0xC3; 32]);
            statement.certificate_policy_record_digest =
                iroha::data_model::privacy::PrivacyZkX509CertificatePolicyRecordDigestV1::new(
                    [0xC4; 32],
                );
        }
        CandidateStatementKindV1::WrongCrl => {
            statement.crl_record_digest =
                iroha::data_model::privacy::PrivacyZkX509CrlRecordDigestV1::new([0xC5; 32]);
        }
    }
    let anchor_is_exact = statement.trust_anchor_id == fixture.trust_anchor.trust_anchor_id
        && statement.trust_anchor_record_digest == fixture.trust_anchor.record_digest
        && statement.trust_anchor_record_epoch == fixture.trust_anchor.record_epoch;
    let policy_is_exact = statement.certificate_policy_id == fixture.certificate_policy.policy_id
        && statement.certificate_policy_record_digest == fixture.certificate_policy.record_digest
        && statement.certificate_policy_record_epoch == fixture.certificate_policy.record_epoch;
    let crl_is_exact = statement.crl_record_digest == fixture.crl.record_digest
        && statement.crl_record_epoch == fixture.crl.record_epoch;
    let intended_references = match kind {
        CandidateStatementKindV1::ExactGovernance => {
            anchor_is_exact && policy_is_exact && crl_is_exact
        }
        CandidateStatementKindV1::WrongTrustAnchor => {
            !anchor_is_exact
                && statement.trust_anchor_id != fixture.trust_anchor.trust_anchor_id
                && statement.trust_anchor_record_digest != fixture.trust_anchor.record_digest
                && policy_is_exact
                && crl_is_exact
        }
        CandidateStatementKindV1::WrongCertificatePolicy => {
            anchor_is_exact
                && !policy_is_exact
                && statement.certificate_policy_id != fixture.certificate_policy.policy_id
                && statement.certificate_policy_record_digest
                    != fixture.certificate_policy.record_digest
                && crl_is_exact
        }
        CandidateStatementKindV1::WrongCrl => {
            anchor_is_exact
                && policy_is_exact
                && !crl_is_exact
                && statement.crl_record_digest != fixture.crl.record_digest
        }
    };
    ensure!(
        intended_references,
        "{kind:?} candidate did not carry the intended exact/substituted governance references"
    );
    Ok(statement)
}

fn candidate_action_transaction(
    client: &Client,
    profile: CompiledPrivacyProfileV1,
    fixture: &ZkX509GovernanceFixtureV1,
    now_unix_seconds: u64,
    kind: CandidateStatementKindV1,
    nonce: u32,
) -> Result<SignedTransaction> {
    let mut statement = candidate_statement(client, profile, fixture, now_unix_seconds, kind)?;
    let draft_envelope = PrivacyProofEnvelopeV1 {
        protocol_id: profile.protocol_id,
        proof_system_id: profile.proof_system_id,
        engine_id: profile.engine_id,
        parameter_id: profile.parameter_id,
        parameter_digest: profile.parameter_digest,
        verifier_digest: profile.verifier_digest,
        statement_schema_digest: profile.statement_schema_digest,
        engine_manifest_digest: profile.engine_manifest_digest,
        statement_digest: PrivacyStatementDigestV1::new([0; 32]),
        statement: PrivacyStatementV1::IrohaZkX509StarkP256V0(statement.clone()),
        proof: PrivacyProofV1::IrohaZkX509StarkP256V0(PrivacyProofBytesV1::new(Vec::new())),
    };
    let creation_time = now_duration()?;
    let nonce = NonZeroU32::new(nonce).ok_or_else(|| eyre!("probe nonce must be non-zero"))?;
    let build_payload = |envelope| -> Result<TransactionPayload> {
        let mut builder =
            TransactionBuilder::new(client.chain.clone(), client.account.clone(), no_fee())
                .with_instructions([SubmitPrivacyProofV1::new(envelope)])
                .with_metadata(Metadata::default());
        builder.set_creation_time(creation_time);
        builder.set_ttl(ACTION_TTL);
        builder.set_nonce(nonce);
        builder
            .into_payload()
            .wrap_err("construct ZK-X509 candidate action payload")
    };
    let intent = build_payload(draft_envelope)?
        .privacy_transaction_intent_digest_v1()
        .wrap_err("derive ZK-X509 candidate transaction intent")?;
    ensure!(
        !intent.is_zero(),
        "ZK-X509 candidate intent must be nonzero"
    );
    statement.context.transaction_intent_digest = intent;
    let typed_statement = PrivacyStatementV1::IrohaZkX509StarkP256V0(statement);
    let envelope = PrivacyProofEnvelopeV1 {
        protocol_id: profile.protocol_id,
        proof_system_id: profile.proof_system_id,
        engine_id: profile.engine_id,
        parameter_id: profile.parameter_id,
        parameter_digest: profile.parameter_digest,
        verifier_digest: profile.verifier_digest,
        statement_schema_digest: profile.statement_schema_digest,
        engine_manifest_digest: profile.engine_manifest_digest,
        statement_digest: typed_statement
            .digest()
            .wrap_err("digest final ZK-X509 candidate statement")?,
        statement: typed_statement,
        proof: PrivacyProofV1::IrohaZkX509StarkP256V0(PrivacyProofBytesV1::new(
            b"X5S1-unreleased-network-probe".to_vec(),
        )),
    };
    envelope
        .validate_with_limits(&PrivacyConsensusLimitsV1::taira_default())
        .wrap_err("validate intrinsic ZK-X509 candidate envelope")?;
    let payload = build_payload(envelope)?;
    let validated_intent = payload
        .validate_privacy_transaction_intent_binding_v1()
        .wrap_err("validate final ZK-X509 transaction intent")?;
    ensure!(
        validated_intent == intent,
        "final ZK-X509 payload validated a different transaction intent"
    );
    let transaction = TransactionBuilder::from_payload(payload)
        .wrap_err("re-open final ZK-X509 candidate payload")?
        .try_sign(client.key_pair.private_key())
        .wrap_err("sign ZK-X509 candidate action")?;
    transaction
        .verify_signature()
        .wrap_err("verify ZK-X509 candidate action signature")?;
    Ok(transaction)
}

fn proposed_candidate_activation(
    profile: CompiledPrivacyProfileV1,
    proposed_at_height: u64,
) -> Result<iroha::data_model::privacy::PrivacyProtocolActivationRecordV1> {
    let activate_at_height = proposed_at_height
        .checked_add(PRIVACY_MIN_ACTIVATION_DELAY_BLOCKS_V1)
        .ok_or_else(|| eyre!("ZK-X509 candidate activation height overflowed"))?;
    Ok(
        profile.activation_record(PrivacyProtocolLifecycleV1::Proposed(
            PrivacyProposedLifecycleV1 {
                proposed_at_height,
                activate_at_height,
            },
        )),
    )
}

// TODO(privacy-zk-x509-release): after real KAT/resource evidence opens the
// readiness gate, add the non-shipping
// `build_privacy_release_zk_x509_network_actions_v1` API. It must prove against
// the actual chain id, canonical genesis hash, transaction intent, action
// index, trusted block timestamp, and registered anchor/policy/CRL revisions.
// Replace the unavailable-action probes below with accepted canonical action,
// wrong-anchor/policy/CRL proof rejection, stable-nullifier replay, and a fresh
// replay submitted through the restarted validator.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn zk_x509_governance_and_unreleased_actions_fail_closed_across_four_peer_restart()
-> Result<()> {
    require_test_network_feature(REQUIRED_DAEMON_FEATURE)?;
    init_instruction_registry();
    let unavailable = CompiledPrivacyProfileErrorV1::EngineUnavailable {
        protocol_id: ZK_X509_PROTOCOL,
    };
    ensure!(
        compiled_privacy_profile_v1(ZK_X509_PROTOCOL) == Err(unavailable),
        "this pre-release gate must be replaced once ZK-X509 becomes governance-available"
    );
    ensure!(
        compiled_privacy_profile_snapshot_result_v1(ZK_X509_PROTOCOL)
            == PrivacyCompiledProfileResultV1::Unavailable(
                PrivacyCompiledProfileUnavailableReasonV1::EngineUnavailable,
            ),
        "local ZK-X509 capability result is not the exact fail-closed status"
    );
    let candidate = zk_x509_release_candidate_profile_material_v1()
        .wrap_err("derive deterministic but non-activatable ZK-X509 candidate profile")?;
    ensure!(
        candidate.protocol_id == ZK_X509_PROTOCOL,
        "candidate profile selected a different protocol"
    );

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
            "ZK-X509 persistence gate requires exactly four trusted validators"
        );
        let all_clients = network
            .peers()
            .iter()
            .map(|peer| bounded_client(peer.client()))
            .collect::<Vec<_>>();
        let client = all_clients[0].clone();
        let initial_height = client
            .get_privacy_capabilities()
            .wrap_err("query initial ZK-X509 capability state")?
            .committed_height;
        wait_for_identical_unavailable_snapshots(
            &all_clients,
            initial_height,
            "ZK-X509 must begin unavailable and unregistered",
        )
        .await?;

        let initial_fixture_time = latest_committed_block_unix_seconds(&client)?;
        let mut fixture = governance_fixture(initial_fixture_time)?;
        let unauthorized_anchor = instruction_transaction(
            &client,
            RegisterPrivacyZkX509TrustAnchorV1::new(fixture.trust_anchor),
            1,
        )?;
        let unauthorized_error = submit_expecting_rejection(
            &client,
            &unauthorized_anchor,
            "unauthorized ZK-X509 trust-anchor registration must reject",
            "unauthorized ZK-X509 trust-anchor registration was accepted",
        )
        .await?;
        ensure!(
            error_chain_contains(&unauthorized_error, "CanEnactGovernance"),
            "unauthorized trust-anchor registration rejected for wrong reason: \
             {unauthorized_error:?}"
        );
        wait_for_transaction_result_on_peers(
            &all_clients,
            &unauthorized_anchor,
            false,
            "unauthorized ZK-X509 trust-anchor rejection convergence",
        )
        .await?;

        let (grant_transaction, _) = submit_instruction(
            &client,
            Grant::account_permission(Permission::from(CanEnactGovernance), client.account.clone()),
            2,
            "grant CanEnactGovernance for ZK-X509 state",
        )
        .await?;
        wait_for_transaction_on_peers(
            &all_clients,
            &grant_transaction,
            "ZK-X509 governance permission convergence",
        )
        .await?;

        let wrong_anchor_policy = instruction_transaction(
            &client,
            RegisterPrivacyZkX509CertificatePolicyV1::new(missing_anchor_policy(&fixture)?),
            3,
        )?;
        let wrong_anchor_error = submit_expecting_rejection(
            &client,
            &wrong_anchor_policy,
            "certificate policy with an absent trust anchor must reject",
            "certificate policy with an absent trust anchor was accepted",
        )
        .await?;
        ensure!(
            error_chain_contains(&wrong_anchor_error, "requires a trust anchor"),
            "missing-anchor policy rejected for wrong reason: {wrong_anchor_error:?}"
        );
        wait_for_transaction_result_on_peers(
            &all_clients,
            &wrong_anchor_policy,
            false,
            "missing-anchor certificate-policy rejection convergence",
        )
        .await?;

        let (anchor_transaction, _) = submit_instruction(
            &client,
            RegisterPrivacyZkX509TrustAnchorV1::new(fixture.trust_anchor),
            4,
            "register canonical ZK-X509 trust-anchor origin",
        )
        .await?;

        let wrong_policy_crl = instruction_transaction(
            &client,
            RegisterPrivacyZkX509CrlV1::new(missing_policy_crl(&fixture, initial_fixture_time)?),
            5,
        )?;
        let wrong_policy_error = submit_expecting_rejection(
            &client,
            &wrong_policy_crl,
            "signed CRL with an absent certificate policy must reject",
            "signed CRL with an absent certificate policy was accepted",
        )
        .await?;
        ensure!(
            error_chain_contains(&wrong_policy_error, "requires a certificate policy"),
            "missing-policy CRL rejected for wrong reason: {wrong_policy_error:?}"
        );
        wait_for_transaction_result_on_peers(
            &all_clients,
            &wrong_policy_crl,
            false,
            "missing-policy signed-CRL rejection convergence",
        )
        .await?;

        let (policy_transaction, _) = submit_instruction(
            &client,
            RegisterPrivacyZkX509CertificatePolicyV1::new(fixture.certificate_policy.clone()),
            6,
            "register canonical ZK-X509 certificate-policy origin",
        )
        .await?;

        let tampered_crl_time = latest_committed_block_unix_seconds(&client)?;
        refresh_governance_fixture_crl(&mut fixture, tampered_crl_time)?;
        let mut tampered_crl = fixture.crl;
        tampered_crl.crl_der_digest = PrivacyX509CrlDerDigestV1::new([0xEF; 32]);
        let tampered_crl_transaction =
            instruction_transaction(&client, RegisterPrivacyZkX509CrlV1::new(tampered_crl), 7)?;
        let tampered_crl_error = submit_expecting_rejection(
            &client,
            &tampered_crl_transaction,
            "self-digest-substituted signed CRL must reject",
            "self-digest-substituted signed CRL was accepted",
        )
        .await?;
        ensure!(
            error_chain_contains(&tampered_crl_error, "digest"),
            "tampered CRL rejected for wrong reason: {tampered_crl_error:?}"
        );
        wait_for_transaction_result_on_peers(
            &all_clients,
            &tampered_crl_transaction,
            false,
            "self-digest-substituted signed-CRL rejection convergence",
        )
        .await?;

        let fixture_time = latest_committed_block_unix_seconds(&client)?;
        refresh_governance_fixture_crl(&mut fixture, fixture_time)?;
        let (crl_transaction, _) = submit_instruction(
            &client,
            RegisterPrivacyZkX509CrlV1::new(fixture.crl),
            8,
            "register canonical ZK-X509 signed-CRL origin",
        )
        .await?;
        for (transaction, context) in [
            (&anchor_transaction, "trust-anchor governance convergence"),
            (
                &policy_transaction,
                "certificate-policy governance convergence",
            ),
            (&crl_transaction, "signed-CRL governance convergence"),
        ] {
            wait_for_transaction_on_peers(&all_clients, transaction, context).await?;
        }

        let proposal_height = next_incoming_height(&client)?;
        let candidate_activation = proposed_candidate_activation(candidate, proposal_height)?;
        let activation_transaction = instruction_transaction(
            &client,
            RegisterPrivacyProtocolActivationV1::new(candidate_activation),
            9,
        )?;
        let activation_error = submit_expecting_rejection(
            &client,
            &activation_transaction,
            "unpinned ZK-X509 candidate activation must reject",
            "unpinned ZK-X509 candidate activation was accepted",
        )
        .await?;
        ensure!(
            error_chain_contains(&activation_error, "does not match compiled native profile")
                && error_chain_contains(&activation_error, "not governance-available"),
            "ZK-X509 candidate activation rejected for wrong reason: {activation_error:?}"
        );
        wait_for_transaction_result_on_peers(
            &all_clients,
            &activation_transaction,
            false,
            "unpinned ZK-X509 activation rejection convergence",
        )
        .await?;

        let mut candidate_actions = Vec::new();
        for (index, kind) in [
            CandidateStatementKindV1::ExactGovernance,
            CandidateStatementKindV1::WrongTrustAnchor,
            CandidateStatementKindV1::WrongCertificatePolicy,
            CandidateStatementKindV1::WrongCrl,
        ]
        .into_iter()
        .enumerate()
        {
            let nonce = u32::try_from(index + 10).expect("four probe nonces fit u32");
            let transaction = candidate_action_transaction(
                &client,
                candidate,
                &fixture,
                fixture_time,
                kind,
                nonce,
            )?;
            let rejection = submit_expecting_rejection(
                &client,
                &transaction,
                &format!("unreleased {kind:?} ZK-X509 candidate action must reject"),
                "unreleased ZK-X509 candidate action was accepted",
            )
            .await?;
            ensure!(
                error_chain_contains(&rejection, "privacy protocol")
                    && error_chain_contains(&rejection, "is not registered"),
                "unreleased {kind:?} action rejected for wrong reason: {rejection:?}"
            );
            wait_for_transaction_result_on_peers(
                &all_clients,
                &transaction,
                false,
                &format!("unreleased {kind:?} candidate-action rejection convergence"),
            )
            .await?;
            candidate_actions.push(transaction);
        }
        ensure!(
            candidate_actions
                .iter()
                .enumerate()
                .all(|(index, transaction)| candidate_actions
                    .iter()
                    .enumerate()
                    .all(|(other_index, other)| index == other_index
                        || transaction.hash() != other.hash())),
            "candidate action probes must have distinct signed transaction hashes"
        );

        let pre_restart_height = client
            .get_privacy_capabilities()
            .wrap_err("query height before ZK-X509 persistence restart")?
            .committed_height;
        wait_for_identical_unavailable_snapshots(
            &all_clients,
            pre_restart_height,
            "governance and candidate rejections preserve unavailable capability state",
        )
        .await?;

        let restart_index = all_clients.len() - 1;
        let restart_peer = network.peers()[restart_index].clone();
        let config_layers = network.config_layers().collect::<Vec<_>>();
        ensure!(
            restart_peer.shutdown_if_started().await,
            "selected ZK-X509 validator was not running before restart coverage"
        );
        let healthy_clients = all_clients[..restart_index].to_vec();
        let (catch_up_transaction, _) = submit_instruction(
            &client,
            Log::new(
                Level::INFO,
                "ZK-X509 governance restart catch-up sentinel".to_owned(),
            ),
            20,
            "commit ZK-X509 restart catch-up sentinel with three validators",
        )
        .await?;
        wait_for_transaction_on_peers(
            &healthy_clients,
            &catch_up_transaction,
            "three-validator ZK-X509 catch-up sentinel finality",
        )
        .await?;

        timeout(
            RESTART_TIMEOUT,
            restart_peer.start_checked(config_layers.iter(), None),
        )
        .await
        .map_err(|_| eyre!("ZK-X509 validator restart exceeded {RESTART_TIMEOUT:?}"))?
        .wrap_err("restart ZK-X509 validator from persisted state")?;
        wait_for_transaction_on_peers(
            &all_clients,
            &catch_up_transaction,
            "restarted validator ZK-X509 catch-up sentinel visibility",
        )
        .await?;
        let recovered_height = bounded_client(restart_peer.client())
            .get_privacy_capabilities()
            .wrap_err("query restarted validator ZK-X509 capability state")?
            .committed_height;
        wait_for_identical_unavailable_snapshots(
            &all_clients,
            recovered_height,
            "restarted validator recovers unavailable ZK-X509 capability state",
        )
        .await?;

        let restarted_client = bounded_client(restart_peer.client());
        let duplicate_instructions: [(InstructionBox, &str, &str); 2] = [
            (
                RegisterPrivacyZkX509TrustAnchorV1::new(fixture.trust_anchor).into(),
                "trust-anchor lineage is already registered",
                "fresh trust-anchor duplicate through restarted validator",
            ),
            (
                RegisterPrivacyZkX509CertificatePolicyV1::new(fixture.certificate_policy.clone())
                    .into(),
                "certificate-policy lineage is already registered",
                "fresh certificate-policy duplicate through restarted validator",
            ),
        ];
        for (index, (instruction, expected, context)) in
            duplicate_instructions.into_iter().enumerate()
        {
            let transaction = instruction_transaction(
                &restarted_client,
                instruction,
                u64::try_from(index + 30).expect("two duplicate tags fit u64"),
            )?;
            let rejection = submit_expecting_rejection(
                &restarted_client,
                &transaction,
                context,
                "restarted validator accepted a duplicate ZK-X509 lineage",
            )
            .await?;
            ensure!(
                error_chain_contains(&rejection, expected),
                "{context} rejected for wrong reason: {rejection:?}"
            );
            wait_for_transaction_result_on_peers(
                &all_clients,
                &transaction,
                false,
                &format!("{context} rejection convergence"),
            )
            .await?;
        }

        let post_restart_time = latest_committed_block_unix_seconds(&restarted_client)?;
        let crl_successor = PrivacyZkX509CrlRecordV1::new(
            fixture.crl.trust_anchor_id,
            fixture.crl.certificate_policy_id,
            2,
            2,
            PrivacyX509CrlDerDigestV1::new([0xD1; 32]),
            PrivacyX509CrlIssuerSpkiDigestV1::new([0xD2; 32]),
            post_restart_time,
            post_restart_time
                .checked_add(3_600)
                .ok_or_else(|| eyre!("post-restart CRL validity window overflowed"))?,
            Some(fixture.crl.record_digest),
            PrivacyZkX509RecordLifecycleV1::Active,
        )
        .wrap_err("construct post-restart signed-CRL compare-and-swap probe")?;
        let stale_crl_rotation = instruction_transaction(
            &restarted_client,
            RotatePrivacyZkX509CrlV1::new(
                PrivacyZkX509CrlRecordDigestV1::new([0xD3; 32]),
                crl_successor,
            ),
            32,
        )?;
        let stale_crl_error = submit_expecting_rejection(
            &restarted_client,
            &stale_crl_rotation,
            "stale signed-CRL compare-and-swap through restarted validator must reject",
            "restarted validator lost the authoritative signed-CRL head",
        )
        .await?;
        ensure!(
            error_chain_contains(&stale_crl_error, "stale or substituted current revision"),
            "post-restart signed-CRL compare-and-swap rejected for wrong reason: \
             {stale_crl_error:?}"
        );
        wait_for_transaction_result_on_peers(
            &all_clients,
            &stale_crl_rotation,
            false,
            "post-restart stale signed-CRL rejection convergence",
        )
        .await?;

        let post_restart_proposal_height = next_incoming_height(&restarted_client)?;
        let post_restart_activation =
            proposed_candidate_activation(candidate, post_restart_proposal_height)?;
        let post_restart_activation_transaction = instruction_transaction(
            &restarted_client,
            RegisterPrivacyProtocolActivationV1::new(post_restart_activation),
            40,
        )?;
        let post_restart_activation_error = submit_expecting_rejection(
            &restarted_client,
            &post_restart_activation_transaction,
            "restarted validator must still reject unpinned ZK-X509 activation",
            "restarted validator accepted unpinned ZK-X509 activation",
        )
        .await?;
        ensure!(
            error_chain_contains(&post_restart_activation_error, "not governance-available"),
            "post-restart activation rejected for wrong reason: \
             {post_restart_activation_error:?}"
        );
        wait_for_transaction_result_on_peers(
            &all_clients,
            &post_restart_activation_transaction,
            false,
            "post-restart unpinned activation rejection convergence",
        )
        .await?;

        let post_restart_candidate = candidate_action_transaction(
            &restarted_client,
            candidate,
            &fixture,
            fixture_time,
            CandidateStatementKindV1::ExactGovernance,
            41,
        )?;
        let post_restart_action_error = submit_expecting_rejection(
            &restarted_client,
            &post_restart_candidate,
            "fresh ZK-X509 candidate action through restarted validator must reject",
            "restarted validator accepted unreleased ZK-X509 action",
        )
        .await?;
        ensure!(
            error_chain_contains(&post_restart_action_error, "privacy protocol")
                && error_chain_contains(&post_restart_action_error, "is not registered"),
            "post-restart candidate action rejected for wrong reason: \
             {post_restart_action_error:?}"
        );
        wait_for_transaction_result_on_peers(
            &all_clients,
            &post_restart_candidate,
            false,
            "post-restart unreleased candidate-action rejection convergence",
        )
        .await?;

        let final_height = restarted_client
            .get_privacy_capabilities()
            .wrap_err("query final ZK-X509 capability state")?
            .committed_height;
        wait_for_identical_unavailable_snapshots(
            &all_clients,
            final_height,
            "all validators retain fail-closed ZK-X509 state after restart probes",
        )
        .await?;
        Ok(())
    }
    .await;

    network.shutdown().await;
    result
}
