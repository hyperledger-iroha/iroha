#![cfg(feature = "privacy-release-evidence")]
#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! Enforced four-peer DA/RBC, activation, adversarial-proof, state-replay,
//! exact-finality, and restart gate for the six retained exact-12 native
//! engines: ZK-ACE, Anonymous PGC, VeRange, Bootle/Lantern, FCMP++, and the
//! private-IVM note protocol.

use std::{
    num::{NonZeroU32, NonZeroU64},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use eyre::{Result, WrapErr as _, ensure, eyre};
use integration_tests::sandbox;
use iroha::{
    client::Client,
    data_model::{
        Level,
        account::Account,
        asset::AssetDefinition,
        block::consensus_v2::BlockSubject,
        domain::Domain,
        isi::{
            Grant, InstructionBox, Log, Mint, Register, SetParameter,
            privacy::{
                BootstrapPrivacyPgcAccountsV1, BootstrapPrivacyProofManagedPoolV1,
                RegisterPrivacyBootleLanternIssuerPolicyV1, RegisterPrivacyProtocolActivationV1,
                RegisterPrivacyZkAcePolicyV1, RotatePrivacyBootleLanternIssuerPolicyV1,
                SubmitPrivacyProofV1,
            },
        },
        metadata::Metadata,
        parameter::{Parameter, TransactionParameter},
        permission::Permission,
        prelude::{
            AccountId, AssetDefinitionId, AssetId, DomainId, FindAssets, Identifiable, Name,
            Quantity, QueryBuilderExt,
        },
        privacy::{
            PrivacyActiveLifecycleV1, PrivacyCompiledProfileResultV1,
            PrivacyCompiledProfileSnapshotV1, PrivacyConsensusLimitsV1,
            PrivacyExact12CapabilityManifestV1, PrivacyExact12CapabilityRowV1, PrivacyPolicyIdV1,
            PrivacyPoolIdV1, PrivacyProposedLifecycleV1, PrivacyProtocolActivationRecordV1,
            PrivacyProtocolIdV1, PrivacyProtocolLifecycleV1, PrivacyStatementDigestV1,
        },
        query::{block::prelude::FindBlocks, transaction::prelude::FindTransactions},
        transaction::{
            FeePaymentIntent, SignedTransaction, TransactionBuilder, TransactionEntrypoint,
        },
    },
};
use iroha_core::{
    privacy::PRIVACY_MIN_ACTIVATION_DELAY_BLOCKS_V1,
    privacy_profiles::{CompiledPrivacyProfileV1, compiled_privacy_profile_v1},
    privacy_release_evidence::{
        PrivacyReleaseTransactionContextV1, build_privacy_release_anonymous_pgc_network_action_v1,
        build_privacy_release_bootle_lantern_network_action_v1,
        build_privacy_release_fcmp_network_action_v1,
        build_privacy_release_ivm_private_note_network_action_v1,
        build_privacy_release_verange_network_action_v1,
        build_privacy_release_zk_ace_network_action_v1,
    },
};
use iroha_executor_data_model::permission::governance::CanEnactGovernance;
use iroha_test_network::{NetworkBuilder, init_instruction_registry};
use iroha_test_samples::{ALICE_ID, gen_account_in};
use tokio::time::{Instant, sleep, timeout};

const ZK_ACE_PROTOCOL: PrivacyProtocolIdV1 = PrivacyProtocolIdV1::ZkAcePqAuthorizationV0;
const PGC_PROTOCOL: PrivacyProtocolIdV1 = PrivacyProtocolIdV1::AnonymousPgcKOutOfNV1;
const VERANGE_PROTOCOL: PrivacyProtocolIdV1 = PrivacyProtocolIdV1::VeRangeTransparentRangeV1;
const BOOTLE_PROTOCOL: PrivacyProtocolIdV1 = PrivacyProtocolIdV1::IrohaBootleLanternAnoncredV1;
const FCMP_PROTOCOL: PrivacyProtocolIdV1 = PrivacyProtocolIdV1::MoneroFcmpPlusPlusV1;
const IVM_PROTOCOL: PrivacyProtocolIdV1 = PrivacyProtocolIdV1::IrohaIvmPrivateNoteStarkV1;
const PROTOCOLS: [PrivacyProtocolIdV1; 6] = [
    ZK_ACE_PROTOCOL,
    PGC_PROTOCOL,
    VERANGE_PROTOCOL,
    BOOTLE_PROTOCOL,
    FCMP_PROTOCOL,
    IVM_PROTOCOL,
];

const SUBMISSION_TIMEOUT: Duration = Duration::from_secs(180);
const PROVER_TIMEOUT: Duration = Duration::from_secs(1_800);
const PEER_CONVERGENCE_TIMEOUT: Duration = Duration::from_secs(180);
const RESTART_TIMEOUT: Duration = Duration::from_secs(120);
const ACTIVATION_ADVANCE_TIMEOUT: Duration = Duration::from_secs(240);
const TEST_BLOCK_CADENCE: Duration = Duration::from_millis(100);
const POLL_INTERVAL: Duration = Duration::from_millis(250);
const ACTION_TTL: Duration = Duration::from_secs(7_200);
const TRANSACTION_BUDGET_BYTES: u64 = 32 * 1024 * 1024;
const TORII_CONTENT_BUDGET_BYTES: i64 = 128 * 1024 * 1024;
const NETWORK_FRAME_BUDGET_BYTES: i64 = 128 * 1024 * 1024;

#[derive(Clone, Copy)]
struct ProtocolExpectationV1 {
    protocol: PrivacyProtocolIdV1,
    compiled: PrivacyCompiledProfileSnapshotV1,
    activation: Option<PrivacyProtocolActivationRecordV1>,
}

fn bounded_client(mut client: Client) -> Client {
    client.transaction_status_timeout = SUBMISSION_TIMEOUT;
    client.torii_request_timeout = Duration::from_secs(45);
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

fn error_chain_contains_any(error: &eyre::Report, needles: &[&str]) -> bool {
    needles
        .iter()
        .any(|needle| error_chain_contains(error, needle))
}

fn is_exact_transaction_replay(error: &eyre::Report) -> bool {
    error_chain_contains_any(
        error,
        &[
            "prtry:already_committed",
            "prtry:already_enqueued",
            "already_committed",
            "already_enqueued",
            "transaction already committed",
            "transaction already present in the queue",
        ],
    )
}

fn protocol_row(
    snapshot: &PrivacyExact12CapabilityManifestV1,
    protocol: PrivacyProtocolIdV1,
) -> Result<PrivacyExact12CapabilityRowV1> {
    snapshot
        .protocols
        .iter()
        .copied()
        .find(|row| row.protocol_id == protocol)
        .ok_or_else(|| eyre!("canonical capability snapshot omitted {protocol:?}"))
}

fn assert_protocol_expectations(
    snapshot: &PrivacyExact12CapabilityManifestV1,
    expectations: &[ProtocolExpectationV1],
    context: &str,
) -> Result<()> {
    snapshot
        .validate()
        .wrap_err_with(|| format!("{context}: invalid capability snapshot"))?;
    for expected in expectations {
        let row = protocol_row(snapshot, expected.protocol)?;
        ensure!(
            row.compiled_profile == PrivacyCompiledProfileResultV1::Available(expected.compiled),
            "{context}: {:?} compiled profile drifted: {:?}",
            expected.protocol,
            row.compiled_profile
        );
        ensure!(
            row.activation == expected.activation,
            "{context}: {:?} lifecycle mismatch: expected {:?}, got {:?}",
            expected.protocol,
            expected.activation,
            row.activation
        );
    }
    Ok(())
}

async fn wait_for_all_peer_protocols(
    network: &sandbox::SerializedNetwork,
    minimum_height: u64,
    expectations: &[ProtocolExpectationV1],
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
                Ok(snapshot) if snapshot.committed_height >= minimum_height => {
                    match assert_protocol_expectations(&snapshot, expectations, context) {
                        Ok(()) => {
                            snapshots.push(snapshot);
                            last_observed.push(format!("peer {index}: exact rows"));
                        }
                        Err(error) => last_observed.push(format!(
                            "peer {index}: height={}, row mismatch: {error}",
                            snapshot.committed_height
                        )),
                    }
                }
                Ok(snapshot) => last_observed.push(format!(
                    "peer {index}: height={} below {minimum_height}",
                    snapshot.committed_height
                )),
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

fn canonical_genesis_hash(client: &Client) -> Result<[u8; 32]> {
    let blocks = client
        .query(FindBlocks)
        .execute_all()
        .wrap_err("query committed blocks for canonical genesis binding")?;
    let genesis = blocks
        .iter()
        .filter(|block| block.header().prev_block_hash().is_none())
        .collect::<Vec<_>>();
    ensure!(
        genesis.len() == 1,
        "FindBlocks must contain exactly one canonical genesis block, got {}",
        genesis.len()
    );
    let hash = *genesis[0].header().hash().as_ref();
    ensure!(hash != [0; 32], "canonical genesis hash must be nonzero");
    Ok(hash)
}

fn next_incoming_height(client: &Client) -> Result<u64> {
    client
        .get_privacy_capabilities()
        .wrap_err("query committed height before governed transaction")?
        .committed_height
        .checked_add(1)
        .ok_or_else(|| eyre!("incoming privacy height overflowed"))
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

fn active_activation(
    compiled: CompiledPrivacyProfileV1,
    proposed_at_height: u64,
    activated_at_height: u64,
) -> PrivacyProtocolActivationRecordV1 {
    compiled.activation_record(PrivacyProtocolLifecycleV1::Active(
        PrivacyActiveLifecycleV1 {
            proposed_at_height,
            activated_at_height,
            state_since_height: activated_at_height,
        },
    ))
}

async fn submit_instructions(
    client: &Client,
    instructions: Vec<InstructionBox>,
    context: &str,
) -> Result<iroha_crypto::HashOf<SignedTransaction>> {
    let client = client.clone();
    timeout(
        SUBMISSION_TIMEOUT,
        tokio::task::spawn_blocking(move || client.submit_all_blocking(instructions, no_fee())),
    )
    .await
    .map_err(|_| eyre!("{context}: submission exceeded {SUBMISSION_TIMEOUT:?}"))?
    .map_err(|error| eyre!("{context}: submission task failed: {error}"))?
    .wrap_err_with(|| context.to_owned())
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

async fn advance_to_exact_height(client: &Client, target_height: u64) -> Result<()> {
    let start = client
        .get_privacy_capabilities()
        .wrap_err("query height before deterministic activation advance")?
        .committed_height;
    ensure!(
        start <= target_height,
        "cannot advance backwards from height {start} to {target_height}"
    );
    for incoming_height in start.saturating_add(1)..=target_height {
        submit_instructions(
            client,
            vec![
                Log::new(
                    Level::INFO,
                    format!("retained exact-12 activation advance {incoming_height}"),
                )
                .into(),
            ],
            "advance retained exact-12 activation height",
        )
        .await?;
    }
    let observed = client
        .get_privacy_capabilities()
        .wrap_err("query height after deterministic activation advance")?
        .committed_height;
    ensure!(
        observed == target_height,
        "activation advance landed at {observed}, expected {target_height}"
    );
    Ok(())
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

fn exact_transaction_block_subject(
    client: &Client,
    transaction: &SignedTransaction,
    context: &str,
) -> Result<(u64, BlockSubject)> {
    let expected_hash = transaction.hash_as_entrypoint();
    let expected_entrypoint = TransactionEntrypoint::External(transaction.clone());
    let transactions = client
        .query(FindTransactions::new())
        .execute_all()
        .wrap_err_with(|| format!("{context}: query finalized transactions"))?;
    let committed = transactions
        .iter()
        .find(|committed| committed.entrypoint_hash() == &expected_hash)
        .ok_or_else(|| eyre!("{context}: exact transaction is absent"))?;
    ensure!(
        committed.entrypoint() == &expected_entrypoint,
        "{context}: entrypoint hash matched different transaction bytes"
    );
    let blocks = client
        .query(FindBlocks)
        .execute_all()
        .wrap_err_with(|| format!("{context}: query carrier block"))?;
    let block = blocks
        .iter()
        .find(|block| block.header().hash() == *committed.block_hash())
        .ok_or_else(|| {
            eyre!(
                "{context}: transaction carrier block {} is absent",
                committed.block_hash()
            )
        })?;
    ensure!(
        committed.verify_inclusion_in_block(block),
        "{context}: transaction inclusion proofs do not match its carrier block"
    );
    let header = block.header();
    let subject = BlockSubject {
        parent_block_hash: header.prev_block_hash(),
        block_hash: header.hash(),
        payload_hash: block
            .canonical_proposal_wire_hash()
            .wrap_err_with(|| format!("{context}: hash canonical proposal wire"))?,
    };
    Ok((header.height().get(), subject))
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

fn asset_quantities(client: &Client, asset_ids: &[AssetId]) -> Result<Vec<Option<Quantity>>> {
    let assets = client
        .query(FindAssets::new())
        .execute_all()
        .wrap_err("query exact asset snapshot")?;
    Ok(asset_ids
        .iter()
        .map(|asset_id| {
            assets
                .iter()
                .find(|asset| asset.id() == asset_id)
                .map(|asset| asset.value().clone())
        })
        .collect())
}

async fn wait_for_asset_quantities(
    clients: &[Client],
    asset_ids: &[AssetId],
    expected: &[Option<Quantity>],
    context: &str,
) -> Result<()> {
    let deadline = Instant::now() + PEER_CONVERGENCE_TIMEOUT;
    let mut last_observed = Vec::new();
    loop {
        let mut matching = 0_usize;
        last_observed.clear();
        for (index, client) in clients.iter().enumerate() {
            match asset_quantities(client, asset_ids) {
                Ok(observed) if observed == expected => {
                    matching += 1;
                    last_observed.push(format!("peer {index}: {observed:?}"));
                }
                Ok(observed) => last_observed.push(format!(
                    "peer {index}: observed {observed:?}, expected {expected:?}"
                )),
                Err(error) => last_observed.push(format!("peer {index}: {error}")),
            }
        }
        if matching == clients.len() {
            return Ok(());
        }
        if Instant::now() >= deadline {
            return Err(eyre!(
                "{context}: asset state did not converge within \
                 {PEER_CONVERGENCE_TIMEOUT:?}; {}",
                last_observed.join("; ")
            ));
        }
        sleep(POLL_INTERVAL).await;
    }
}

fn resign_replaced_envelope(
    client: &Client,
    valid: &SignedTransaction,
    envelope: iroha::data_model::privacy::PrivacyProofEnvelopeV1,
    context: &str,
) -> Result<SignedTransaction> {
    let (valid_intent, _) = valid
        .privacy_transaction_intent_binding_if_present_v1()
        .wrap_err_with(|| format!("{context}: scan canonical privacy action"))?
        .ok_or_else(|| eyre!("{context}: canonical action omitted privacy submission"))?;
    let adversarial = TransactionBuilder::from_payload(valid.payload().clone())
        .wrap_err_with(|| format!("{context}: reopen canonical payload"))?
        .with_instructions([SubmitPrivacyProofV1::new(envelope)])
        .try_sign(client.key_pair.private_key())
        .wrap_err_with(|| format!("{context}: independently sign adversarial transaction"))?;
    adversarial
        .verify_signature()
        .wrap_err_with(|| format!("{context}: verify adversarial signature"))?;
    let adversarial_intent = adversarial
        .privacy_transaction_intent_digest_v1()
        .wrap_err_with(|| format!("{context}: derive adversarial transaction intent"))?;
    ensure!(
        adversarial_intent == valid_intent,
        "{context}: proof/digest-only mutation changed transaction intent"
    );
    ensure!(
        adversarial.hash() != valid.hash(),
        "{context}: mutation did not change the signed transaction hash"
    );
    Ok(adversarial)
}

fn assert_exactly_one_direct_privacy_submission(
    transaction: &SignedTransaction,
    context: &str,
) -> Result<()> {
    let direct_submission_count = transaction
        .instructions()
        .explicit_instructions()
        .filter(|instruction| {
            instruction
                .as_any()
                .downcast_ref::<SubmitPrivacyProofV1>()
                .is_some()
        })
        .count();
    ensure!(
        direct_submission_count == 1,
        "{context}: expected exactly one direct typed privacy submission, got \
         {direct_submission_count}"
    );
    transaction
        .privacy_transaction_intent_binding_if_present_v1()
        .wrap_err_with(|| format!("{context}: validate exact direct privacy binding"))?
        .ok_or_else(|| eyre!("{context}: direct privacy submission was not detected"))?;
    Ok(())
}

fn independently_sign_two_submit_transaction(
    client: &Client,
    canonical: &SignedTransaction,
) -> Result<SignedTransaction> {
    let (_, submission) = canonical
        .privacy_transaction_intent_binding_if_present_v1()
        .wrap_err("scan canonical action before two-submit adversary")?
        .ok_or_else(|| eyre!("canonical action omitted privacy submission"))?;
    let adversarial = TransactionBuilder::from_payload(canonical.payload().clone())
        .wrap_err("reopen canonical payload for two-submit adversary")?
        .with_instructions([submission.clone(), submission.clone()])
        .try_sign(client.key_pair.private_key())
        .wrap_err("independently sign two-submit adversary")?;
    adversarial
        .verify_signature()
        .wrap_err("verify two-submit adversary signature")?;
    let binding_error = adversarial
        .privacy_transaction_intent_binding_if_present_v1()
        .expect_err("two direct submissions passed the local closed projection");
    ensure!(
        binding_error.to_string()
            == "privacy transaction intent contains 2 direct privacy submissions",
        "two-submit local rejection drifted: {binding_error}"
    );
    Ok(adversarial)
}

fn independently_resign_corrupted_proof(
    client: &Client,
    valid: &SignedTransaction,
    context: &str,
) -> Result<SignedTransaction> {
    let (_, submission) = valid
        .privacy_transaction_intent_binding_if_present_v1()
        .wrap_err_with(|| format!("{context}: scan canonical action before corruption"))?
        .ok_or_else(|| eyre!("{context}: canonical action omitted privacy submission"))?;
    let mut envelope = submission.envelope.clone();
    let proof = envelope.proof.bytes_mut();
    ensure!(
        proof.bytes.len() > 8,
        "{context}: proof is too short to corrupt"
    );
    let interior = proof.bytes.len() / 2;
    proof.bytes[interior] ^= 0x01;
    envelope
        .validate_with_limits(&PrivacyConsensusLimitsV1::taira_default())
        .wrap_err_with(|| format!("{context}: corruption broke generic envelope shape"))?;
    resign_replaced_envelope(client, valid, envelope, context)
}

fn independently_resign_cross_profile_proof(
    client: &Client,
    target: &SignedTransaction,
    donor: &SignedTransaction,
) -> Result<SignedTransaction> {
    let (_, target_submission) = target
        .privacy_transaction_intent_binding_if_present_v1()
        .wrap_err("scan target action before cross-profile substitution")?
        .ok_or_else(|| eyre!("target action omitted privacy submission"))?;
    let (_, donor_submission) = donor
        .privacy_transaction_intent_binding_if_present_v1()
        .wrap_err("scan donor action before cross-profile substitution")?
        .ok_or_else(|| eyre!("donor action omitted privacy submission"))?;
    ensure!(
        target_submission.envelope.protocol_id != donor_submission.envelope.protocol_id,
        "cross-profile substitution requires different protocols"
    );
    let mut envelope = target_submission.envelope.clone();
    envelope.proof.bytes_mut().bytes = donor_submission.envelope.proof.bytes().bytes.clone();
    envelope
        .validate_with_limits(&PrivacyConsensusLimitsV1::taira_default())
        .wrap_err("cross-profile bytes must preserve the generic envelope contract")?;
    resign_replaced_envelope(client, target, envelope, "cross-profile proof substitution")
}

fn independently_resign_wrong_statement_digest(
    client: &Client,
    valid: &SignedTransaction,
) -> Result<SignedTransaction> {
    let (_, submission) = valid
        .privacy_transaction_intent_binding_if_present_v1()
        .wrap_err("scan canonical action before statement-digest substitution")?
        .ok_or_else(|| eyre!("canonical action omitted privacy submission"))?;
    let mut envelope = submission.envelope.clone();
    let mut wrong = *envelope.statement_digest.as_bytes();
    wrong[0] ^= 0x80;
    envelope.statement_digest = PrivacyStatementDigestV1::new(wrong);
    envelope
        .validate_with_limits(&PrivacyConsensusLimitsV1::taira_default())
        .wrap_err("wrong statement digest must preserve the generic envelope contract")?;
    resign_replaced_envelope(client, valid, envelope, "wrong statement-digest binding")
}

async fn wait_for_exact_v2_commit_subject(
    clients: &[Client],
    expected_height: u64,
    expected_subject: BlockSubject,
    context: &str,
) -> Result<()> {
    let deadline = Instant::now() + PEER_CONVERGENCE_TIMEOUT;
    let mut last_observed = Vec::new();
    loop {
        let mut matching = 0_usize;
        last_observed.clear();
        for (index, client) in clients.iter().enumerate() {
            match client.get_sumeragi_status() {
                Ok(status) => {
                    if let Err(error) = status.validate() {
                        last_observed.push(format!("peer {index}: invalid v2 status: {error}"));
                    } else if status.last_committed_height != expected_height {
                        last_observed.push(format!(
                            "peer {index}: v2 height {}, expected exact {expected_height}",
                            status.last_committed_height
                        ));
                    } else if status.last_committed_subject != Some(expected_subject) {
                        last_observed.push(format!(
                            "peer {index}: wrong exact committed subject: {:?}",
                            status.last_committed_subject
                        ));
                    } else if let Some(certificate) = status.last_commit_qc {
                        let exact_certificate = certificate.certificate.subject == expected_subject
                            && certificate.certificate.round.height == expected_height
                            && certificate.certificate.proposal_round.height == expected_height;
                        let exact_quorum = certificate.validator_count > 0
                            && certificate.min_signers > 0
                            && certificate.signer_count >= certificate.min_signers
                            && certificate.signer_count <= certificate.validator_count
                            && certificate.signed_power <= certificate.total_power
                            && u128::from(certificate.signed_power) * 3
                                > u128::from(certificate.total_power) * 2;
                        if exact_certificate && exact_quorum {
                            matching += 1;
                            last_observed.push(format!(
                                "peer {index}: exact block subject and authenticated quorum"
                            ));
                        } else {
                            last_observed.push(format!(
                                "peer {index}: wrong exact CommitQC/quorum: {certificate:?}"
                            ));
                        }
                    } else {
                        last_observed.push(format!("peer {index}: missing durable CommitQC"));
                    }
                }
                Err(error) => last_observed.push(format!("peer {index}: status failed: {error}")),
            }
        }
        if matching == clients.len() {
            return Ok(());
        }
        if Instant::now() >= deadline {
            return Err(eyre!(
                "{context}: exact v2 DA/RBC block subject and CommitQC did not converge within \
                 {PEER_CONVERGENCE_TIMEOUT:?}; {}",
                last_observed.join("; ")
            ));
        }
        sleep(POLL_INTERVAL).await;
    }
}

fn action_context(
    client: &Client,
    genesis_hash: [u8; 32],
    creation_time: Duration,
    nonce: u32,
) -> PrivacyReleaseTransactionContextV1 {
    PrivacyReleaseTransactionContextV1 {
        network_id: client.network_id,
        authority: client.account.clone(),
        creation_time,
        time_to_live: Some(ACTION_TTL),
        nonce: NonZeroU32::new(nonce),
        fee_payment: no_fee(),
        metadata: Metadata::default(),
        genesis_hash,
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn canonical_retained_exact12_actions_survive_four_peer_adversarial_replay_and_restart()
-> Result<()> {
    init_instruction_registry();
    let context = stringify!(
        canonical_retained_exact12_actions_survive_four_peer_adversarial_replay_and_restart
    );
    let privacy_domain = DomainId::try_new("privacy", "universal")?;
    let (reserve_account, _) = gen_account_in("privacy");
    let zk_asset = AssetDefinitionId::derive_from_components(
        privacy_domain.clone(),
        "zk_ace_coin".parse::<Name>()?,
    );
    let pgc_asset = AssetDefinitionId::derive_from_components(
        privacy_domain.clone(),
        "pgc_note".parse::<Name>()?,
    );
    let verange_asset = AssetDefinitionId::derive_from_components(
        privacy_domain.clone(),
        "verange_value".parse::<Name>()?,
    );
    let fcmp_asset = AssetDefinitionId::derive_from_components(
        privacy_domain.clone(),
        "fcmp_note".parse::<Name>()?,
    );
    let ivm_asset = AssetDefinitionId::derive_from_components(
        privacy_domain.clone(),
        "ivm_note".parse::<Name>()?,
    );
    let zk_asset_at_alice = AssetId::new(zk_asset.clone(), ALICE_ID.clone());
    let zk_asset_at_reserve = AssetId::new(zk_asset.clone(), reserve_account.clone());
    let transaction_budget =
        NonZeroU64::new(TRANSACTION_BUDGET_BYTES).expect("fixed transaction budget is nonzero");

    let builder = NetworkBuilder::new()
        .with_peers(4)
        .with_auto_populated_trusted_peers()
        .with_block_cadence(TEST_BLOCK_CADENCE)
        .with_permissioned_consensus()
        .with_config_layer(|layer| {
            layer
                .write(["torii", "max_content_len"], TORII_CONTENT_BUDGET_BYTES)
                .write(["network", "max_frame_bytes"], NETWORK_FRAME_BUDGET_BYTES)
                .write(
                    ["network", "max_frame_bytes_consensus"],
                    NETWORK_FRAME_BUDGET_BYTES,
                )
                .write(
                    ["network", "max_frame_bytes_control"],
                    NETWORK_FRAME_BUDGET_BYTES,
                )
                .write(
                    ["network", "max_frame_bytes_block_sync"],
                    NETWORK_FRAME_BUDGET_BYTES,
                )
                .write(
                    ["network", "max_frame_bytes_other"],
                    NETWORK_FRAME_BUDGET_BYTES,
                )
                .write(
                    ["network", "max_frame_bytes_tx_gossip"],
                    NETWORK_FRAME_BUDGET_BYTES,
                );
        })
        .with_genesis_instruction(SetParameter::new(Parameter::Transaction(
            TransactionParameter::MaxTxBytes(transaction_budget),
        )))
        .with_genesis_instruction(SetParameter::new(Parameter::Transaction(
            TransactionParameter::MaxDecompressedBytes(transaction_budget),
        )))
        .with_genesis_instruction(Register::domain(Domain::new(privacy_domain.clone())))
        .with_genesis_instruction(Register::account(Account::new(reserve_account.clone())))
        .with_genesis_instruction(Register::asset_definition(AssetDefinition::numeric(
            zk_asset.clone(),
            "zk_ace_coin".to_owned(),
            iroha_data_model::asset::AssetBalancePolicy::Global,
            None,
        )))
        .with_genesis_instruction(Register::asset_definition(AssetDefinition::numeric(
            pgc_asset.clone(),
            "pgc_note".to_owned(),
            iroha_data_model::asset::AssetBalancePolicy::Global,
            None,
        )))
        .with_genesis_instruction(Register::asset_definition(AssetDefinition::numeric(
            verange_asset.clone(),
            "verange_value".to_owned(),
            iroha_data_model::asset::AssetBalancePolicy::Global,
            None,
        )))
        .with_genesis_instruction(Register::asset_definition(AssetDefinition::numeric(
            fcmp_asset.clone(),
            "fcmp_note".to_owned(),
            iroha_data_model::asset::AssetBalancePolicy::Global,
            None,
        )))
        .with_genesis_instruction(Register::asset_definition(AssetDefinition::numeric(
            ivm_asset.clone(),
            "ivm_note".to_owned(),
            iroha_data_model::asset::AssetBalancePolicy::Global,
            None,
        )))
        .with_genesis_instruction(Mint::asset_quantity(100_u32, zk_asset_at_alice.clone()));
    let Some(network) = sandbox::start_network_async_or_skip(builder, context).await? else {
        return Ok(());
    };

    let result: Result<()> = async {
        ensure!(
            network.peers().len() == 4,
            "retained exact-12 DA gate requires exactly four peers"
        );
        network
            .ensure_blocks_with(|height| height.total >= 1)
            .await?;
        let client = bounded_client(network.client());
        let all_clients = network
            .peers()
            .iter()
            .map(|peer| bounded_client(peer.client()))
            .collect::<Vec<_>>();
        let genesis_hash = canonical_genesis_hash(&client)?;
        let compiled_profiles = PROTOCOLS
            .iter()
            .copied()
            .map(|protocol| {
                compiled_privacy_profile_v1(protocol)
                    .wrap_err_with(|| format!("load canonical {protocol:?} compiled profile"))
            })
            .collect::<Result<Vec<_>>>()?;

        submit_instructions(
            &client,
            vec![
                Grant::account_permission(
                    Permission::from(CanEnactGovernance),
                    client.account.clone(),
                )
                .into(),
            ],
            "grant CanEnactGovernance",
        )
        .await?;

        let registration_height = next_incoming_height(&client)?;
        let activation_height = registration_height
            .checked_add(PRIVACY_MIN_ACTIVATION_DELAY_BLOCKS_V1)
            .ok_or_else(|| eyre!("retained exact-12 activation height overflowed"))?;
        let proposed_records = compiled_profiles
            .iter()
            .copied()
            .map(|compiled| proposed_activation(compiled, registration_height, activation_height))
            .collect::<Vec<_>>();
        submit_instructions(
            &client,
            proposed_records
                .iter()
                .copied()
                .map(RegisterPrivacyProtocolActivationV1::new)
                .map(InstructionBox::from)
                .collect(),
            "register exact retained exact-12 activations",
        )
        .await?;
        let proposed_expectations = PROTOCOLS
            .iter()
            .copied()
            .zip(compiled_profiles.iter().copied())
            .zip(proposed_records.iter().copied())
            .map(|((protocol, compiled), activation)| ProtocolExpectationV1 {
                protocol,
                compiled: compiled.into(),
                activation: Some(activation),
            })
            .collect::<Vec<_>>();
        wait_for_all_peer_protocols(
            &network,
            registration_height,
            &proposed_expectations,
            "exact proposed retained exact-12 lifecycles",
        )
        .await?;

        let creation_time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .wrap_err("system clock is before the Unix epoch")?;
        let signing_key = client.key_pair.private_key().clone();
        let zk_context = action_context(
            &client,
            genesis_hash,
            creation_time + Duration::from_millis(1),
            201,
        );
        let pgc_context = action_context(
            &client,
            genesis_hash,
            creation_time + Duration::from_millis(2),
            202,
        );
        let pgc_replay_context = action_context(
            &client,
            genesis_hash,
            creation_time + Duration::from_millis(3),
            203,
        );
        let pre_verange_context = action_context(
            &client,
            genesis_hash,
            creation_time + Duration::from_millis(4),
            204,
        );
        let verange_context = action_context(
            &client,
            genesis_hash,
            creation_time + Duration::from_millis(5),
            205,
        );
        let bootle_context = action_context(
            &client,
            genesis_hash,
            creation_time + Duration::from_millis(6),
            206,
        );
        let stale_bootle_context = action_context(
            &client,
            genesis_hash,
            creation_time + Duration::from_millis(7),
            207,
        );
        let fcmp_context = action_context(
            &client,
            genesis_hash,
            creation_time + Duration::from_millis(8),
            208,
        );
        let fcmp_replay_context = action_context(
            &client,
            genesis_hash,
            creation_time + Duration::from_millis(9),
            209,
        );
        let ivm_context = action_context(
            &client,
            genesis_hash,
            creation_time + Duration::from_millis(10),
            210,
        );
        let ivm_replay_context = action_context(
            &client,
            genesis_hash,
            creation_time + Duration::from_millis(11),
            211,
        );
        let pgc_pool = PrivacyPoolIdV1::new([0x21; 32]);
        let fcmp_pool = PrivacyPoolIdV1::new([0x22; 32]);
        let ivm_pool = PrivacyPoolIdV1::new([0x23; 32]);
        let verange_policy = PrivacyPolicyIdV1::new([0x24; 32]);
        let reserve_for_builder = reserve_account.clone();
        let zk_asset_for_builder = zk_asset.clone();
        let pgc_asset_for_builder = pgc_asset.clone();
        let verange_asset_for_builder = verange_asset.clone();
        let fcmp_asset_for_builder = fcmp_asset.clone();
        let ivm_asset_for_builder = ivm_asset.clone();
        let build_actions = tokio::task::spawn_blocking(move || {
            let zk = build_privacy_release_zk_ace_network_action_v1(
                zk_context.clone(),
                ALICE_ID.clone(),
                reserve_for_builder.clone(),
                zk_asset_for_builder.clone(),
                19,
                [0x31; 32],
                [0x32; 32],
                &signing_key,
            )
            .map_err(|error| eyre!("build canonical ZK-ACE action: {error:?}"))?;
            let zk_replay = build_privacy_release_zk_ace_network_action_v1(
                zk_context,
                ALICE_ID.clone(),
                reserve_for_builder.clone(),
                zk_asset_for_builder,
                19,
                [0x31; 32],
                [0x33; 32],
                &signing_key,
            )
            .map_err(|error| eyre!("build fresh ZK-ACE replay action: {error:?}"))?;
            let pgc = build_privacy_release_anonymous_pgc_network_action_v1(
                pgc_context,
                pgc_asset_for_builder.clone(),
                pgc_pool,
                0,
                [0x41; 32],
                &signing_key,
            )
            .map_err(|error| eyre!("build canonical Anonymous-PGC action: {error:?}"))?;
            let pgc_replay = build_privacy_release_anonymous_pgc_network_action_v1(
                pgc_replay_context,
                pgc_asset_for_builder,
                pgc_pool,
                0,
                [0x41; 32],
                &signing_key,
            )
            .map_err(|error| eyre!("build stale-head Anonymous-PGC action: {error:?}"))?;
            let pre_verange = build_privacy_release_verange_network_action_v1(
                pre_verange_context,
                verange_asset_for_builder.clone(),
                verange_policy,
                vec![1, 2, 3, 5, 8, 13, 21, u64::from(u32::MAX)],
                [0x51; 32],
                &signing_key,
            )
            .map_err(|error| eyre!("build pre-activation VeRange action: {error:?}"))?;
            let verange = build_privacy_release_verange_network_action_v1(
                verange_context,
                verange_asset_for_builder,
                verange_policy,
                vec![0, 1, 17, u64::from(u32::MAX)],
                [0x52; 32],
                &signing_key,
            )
            .map_err(|error| eyre!("build canonical VeRange action: {error:?}"))?;
            let bootle = build_privacy_release_bootle_lantern_network_action_v1(
                bootle_context,
                [0x61; 32],
                &signing_key,
            )
            .map_err(|error| eyre!("build canonical Bootle/Lantern action: {error:?}"))?;
            let stale_bootle = build_privacy_release_bootle_lantern_network_action_v1(
                stale_bootle_context,
                [0x61; 32],
                &signing_key,
            )
            .map_err(|error| eyre!("build stale-policy Bootle/Lantern action: {error:?}"))?;
            ensure!(
                bootle.policy == stale_bootle.policy
                    && bootle.successor_policy == stale_bootle.successor_policy,
                "same Bootle/Lantern fixture seed did not reproduce both governed policy revisions"
            );
            ensure!(
                bootle.policy.issuer_public_matrix != bootle.successor_policy.issuer_public_matrix,
                "Bootle/Lantern successor reused the initial issuer public key"
            );
            let fcmp = build_privacy_release_fcmp_network_action_v1(
                fcmp_context,
                fcmp_asset_for_builder.clone(),
                fcmp_pool,
                [0x71; 32],
                &signing_key,
            )
            .map_err(|error| eyre!("build canonical FCMP++ action: {error:?}"))?;
            let fcmp_replay = build_privacy_release_fcmp_network_action_v1(
                fcmp_replay_context,
                fcmp_asset_for_builder,
                fcmp_pool,
                [0x71; 32],
                &signing_key,
            )
            .map_err(|error| eyre!("build key-image replay FCMP++ action: {error:?}"))?;
            let ivm = build_privacy_release_ivm_private_note_network_action_v1(
                ivm_context,
                ivm_asset_for_builder.clone(),
                ivm_pool,
                reserve_for_builder.clone(),
                [0x81; 32],
                &signing_key,
            )
            .map_err(|error| eyre!("build canonical private-IVM action: {error:?}"))?;
            let ivm_replay = build_privacy_release_ivm_private_note_network_action_v1(
                ivm_replay_context,
                ivm_asset_for_builder,
                ivm_pool,
                reserve_for_builder,
                [0x81; 32],
                &signing_key,
            )
            .map_err(|error| eyre!("build nullifier replay private-IVM action: {error:?}"))?;
            Ok::<_, eyre::Report>((
                zk,
                zk_replay,
                pgc,
                pgc_replay,
                pre_verange,
                verange,
                bootle,
                stale_bootle,
                fcmp,
                fcmp_replay,
                ivm,
                ivm_replay,
            ))
        });
        let (
            zk,
            zk_replay,
            pgc,
            pgc_replay,
            pre_verange,
            verange,
            bootle,
            stale_bootle,
            fcmp,
            fcmp_replay,
            ivm,
            ivm_replay,
        ) = timeout(PROVER_TIMEOUT, build_actions)
            .await
            .map_err(|_| eyre!("retained exact-12 proving exceeded {PROVER_TIMEOUT:?}"))?
            .map_err(|error| eyre!("retained exact-12 prover task failed: {error}"))??;

        ensure!(
            zk.policy == zk_replay.policy
                && zk.statement.replay_nullifier == zk_replay.statement.replay_nullifier,
            "fresh ZK-ACE replay did not preserve policy and replay nullifier"
        );
        ensure!(
            zk.transaction.hash() != zk_replay.transaction.hash(),
            "independently randomized ZK-ACE proofs produced the same transaction"
        );
        ensure!(
            pgc.bootstrap == pgc_replay.bootstrap
                && pgc.bootstrap_proof == pgc_replay.bootstrap_proof
                && pgc.statement.account_state_root == pgc_replay.statement.account_state_root
                && pgc.statement.next_account_state_root
                    == pgc_replay.statement.next_account_state_root,
            "Anonymous-PGC replay fixture drifted from its authoritative origin"
        );
        ensure!(
            bootle.policy == stale_bootle.policy,
            "Bootle/Lantern stale-policy fixture changed the initial policy"
        );
        ensure!(
            fcmp.bootstrap == fcmp_replay.bootstrap
                && fcmp.statement.inputs[0].key_image == fcmp_replay.statement.inputs[0].key_image,
            "FCMP++ replay fixture did not preserve bootstrap and key image"
        );
        ensure!(
            ivm.bootstrap == ivm_replay.bootstrap
                && ivm.statement.nullifiers == ivm_replay.statement.nullifiers,
            "private-IVM replay fixture did not preserve bootstrap and nullifiers"
        );
        for (label, canonical, replay) in [
            ("PGC", &pgc.transaction, &pgc_replay.transaction),
            (
                "Bootle/Lantern",
                &bootle.transaction,
                &stale_bootle.transaction,
            ),
            ("FCMP++", &fcmp.transaction, &fcmp_replay.transaction),
            ("private-IVM", &ivm.transaction, &ivm_replay.transaction),
        ] {
            ensure!(
                canonical.hash() != replay.hash(),
                "{label} canonical and replay transactions must be distinct"
            );
        }
        for (label, transaction) in [
            ("canonical ZK-ACE", &zk.transaction),
            ("fresh ZK-ACE replay", &zk_replay.transaction),
            ("canonical Anonymous-PGC", &pgc.transaction),
            ("stale-head Anonymous-PGC", &pgc_replay.transaction),
            ("pre-activation VeRange", &pre_verange.transaction),
            ("canonical VeRange", &verange.transaction),
            ("canonical Bootle/Lantern", &bootle.transaction),
            ("stale-policy Bootle/Lantern", &stale_bootle.transaction),
            ("canonical FCMP++", &fcmp.transaction),
            ("key-image replay FCMP++", &fcmp_replay.transaction),
            ("canonical private-IVM", &ivm.transaction),
            ("nullifier replay private-IVM", &ivm_replay.transaction),
        ] {
            assert_exactly_one_direct_privacy_submission(transaction, label)?;
        }

        let advance_target = activation_height
            .checked_sub(2)
            .ok_or_else(|| eyre!("activation height lacks a pre-activation predecessor"))?;
        timeout(
            ACTIVATION_ADVANCE_TIMEOUT,
            advance_to_exact_height(&client, advance_target),
        )
        .await
        .map_err(|_| {
            eyre!("advancing through activation lead exceeded {ACTIVATION_ADVANCE_TIMEOUT:?}")
        })??;
        let preactivation_error = submit_signed_transaction(
            &client,
            &pre_verange.transaction,
            "valid VeRange action before activation must reject",
        )
        .await
        .expect_err("valid VeRange action was admitted before activation");
        ensure!(
            error_chain_contains(&preactivation_error, "activation is not active"),
            "pre-activation VeRange rejection had wrong reason: {preactivation_error:?}"
        );
        wait_for_transaction_result_on_peers(
            &all_clients,
            &pre_verange.transaction,
            false,
            "pre-activation VeRange rejection convergence",
        )
        .await?;
        wait_for_asset_quantities(
            &all_clients,
            &[zk_asset_at_alice.clone(), zk_asset_at_reserve.clone()],
            &[Some(Quantity::from(100_u32)), None],
            "pre-activation rejection must preserve ZK-ACE public balances",
        )
        .await?;

        submit_instructions(
            &client,
            vec![
                Log::new(
                    Level::INFO,
                    format!("exact retained exact-12 activation block {activation_height}"),
                )
                .into(),
            ],
            "commit exact retained exact-12 activation block",
        )
        .await?;
        let active_records = compiled_profiles
            .iter()
            .copied()
            .map(|compiled| active_activation(compiled, registration_height, activation_height))
            .collect::<Vec<_>>();
        let active_expectations = PROTOCOLS
            .iter()
            .copied()
            .zip(compiled_profiles.iter().copied())
            .zip(active_records.iter().copied())
            .map(|((protocol, compiled), activation)| ProtocolExpectationV1 {
                protocol,
                compiled: compiled.into(),
                activation: Some(activation),
            })
            .collect::<Vec<_>>();
        wait_for_all_peer_protocols(
            &network,
            activation_height,
            &active_expectations,
            "exact active retained exact-12 lifecycles",
        )
        .await?;

        // Taira permits one budgeted privacy action per transaction. Keep
        // every governed policy/bootstrap in its own consensus action so the
        // PGC proof is bound to the exact action index zero and no setup path
        // relies on a local-only budget bypass.
        submit_instructions(
            &client,
            vec![RegisterPrivacyZkAcePolicyV1::new(zk.policy.clone()).into()],
            "register authoritative ZK-ACE policy",
        )
        .await?;
        submit_instructions(
            &client,
            vec![RegisterPrivacyBootleLanternIssuerPolicyV1::new(bootle.policy.clone()).into()],
            "register authoritative Bootle/Lantern policy",
        )
        .await?;
        submit_instructions(
            &client,
            vec![
                BootstrapPrivacyPgcAccountsV1::new(
                    pgc.bootstrap.clone(),
                    pgc.bootstrap_proof.clone(),
                )
                .into(),
            ],
            "bootstrap authoritative Anonymous-PGC account table",
        )
        .await?;
        submit_instructions(
            &client,
            vec![BootstrapPrivacyProofManagedPoolV1::new(fcmp.bootstrap.clone()).into()],
            "bootstrap authoritative FCMP++ pool",
        )
        .await?;
        submit_instructions(
            &client,
            vec![BootstrapPrivacyProofManagedPoolV1::new(ivm.bootstrap.clone()).into()],
            "bootstrap authoritative private-IVM pool",
        )
        .await?;

        let corrupted_zk = independently_resign_corrupted_proof(
            &client,
            &zk.transaction,
            "corrupted ZK-ACE proof",
        )?;
        let corrupted_pgc = independently_resign_corrupted_proof(
            &client,
            &pgc.transaction,
            "corrupted Anonymous-PGC proof",
        )?;
        let corrupted_verange = independently_resign_corrupted_proof(
            &client,
            &verange.transaction,
            "corrupted VeRange proof",
        )?;
        let corrupted_bootle = independently_resign_corrupted_proof(
            &client,
            &bootle.transaction,
            "corrupted Bootle/Lantern proof",
        )?;
        let corrupted_fcmp = independently_resign_corrupted_proof(
            &client,
            &fcmp.transaction,
            "corrupted FCMP++ proof",
        )?;
        let corrupted_ivm = independently_resign_corrupted_proof(
            &client,
            &ivm.transaction,
            "corrupted private-IVM proof",
        )?;
        let cross_profile_adversaries = [
            (
                "ZK-ACE target with Anonymous-PGC proof",
                independently_resign_cross_profile_proof(
                    &client,
                    &zk.transaction,
                    &pgc.transaction,
                )?,
                "native ZK-ACE verification failed",
            ),
            (
                "Anonymous-PGC target with VeRange proof",
                independently_resign_cross_profile_proof(
                    &client,
                    &pgc.transaction,
                    &verange.transaction,
                )?,
                "native Anonymous-PGC verification failed",
            ),
            (
                "VeRange target with Bootle/Lantern proof",
                independently_resign_cross_profile_proof(
                    &client,
                    &verange.transaction,
                    &bootle.transaction,
                )?,
                "native VeRange verification failed",
            ),
            (
                "Bootle/Lantern target with FCMP++ proof",
                independently_resign_cross_profile_proof(
                    &client,
                    &bootle.transaction,
                    &fcmp.transaction,
                )?,
                "native Bootle/Lantern verification failed",
            ),
            (
                "FCMP++ target with private-IVM proof",
                independently_resign_cross_profile_proof(
                    &client,
                    &fcmp.transaction,
                    &ivm.transaction,
                )?,
                "native FCMP++ verification failed",
            ),
            (
                "private-IVM target with ZK-ACE proof",
                independently_resign_cross_profile_proof(
                    &client,
                    &ivm.transaction,
                    &zk.transaction,
                )?,
                "native private-IVM verification failed",
            ),
        ];
        let wrong_statement_digest_adversaries = [
            (
                "ZK-ACE wrong statement digest",
                independently_resign_wrong_statement_digest(&client, &zk.transaction)?,
            ),
            (
                "Anonymous-PGC wrong statement digest",
                independently_resign_wrong_statement_digest(&client, &pgc.transaction)?,
            ),
            (
                "VeRange wrong statement digest",
                independently_resign_wrong_statement_digest(&client, &verange.transaction)?,
            ),
            (
                "Bootle/Lantern wrong statement digest",
                independently_resign_wrong_statement_digest(&client, &bootle.transaction)?,
            ),
            (
                "FCMP++ wrong statement digest",
                independently_resign_wrong_statement_digest(&client, &fcmp.transaction)?,
            ),
            (
                "private-IVM wrong statement digest",
                independently_resign_wrong_statement_digest(&client, &ivm.transaction)?,
            ),
        ];
        let two_submit = independently_sign_two_submit_transaction(&client, &zk.transaction)?;
        let adversarial_transactions = [
            (
                "corrupted ZK-ACE",
                &corrupted_zk,
                "native ZK-ACE verification failed",
            ),
            (
                "corrupted Anonymous-PGC",
                &corrupted_pgc,
                "native Anonymous-PGC verification failed",
            ),
            (
                "corrupted VeRange",
                &corrupted_verange,
                "native VeRange verification failed",
            ),
            (
                "corrupted Bootle/Lantern",
                &corrupted_bootle,
                "native Bootle/Lantern verification failed",
            ),
            (
                "corrupted FCMP++",
                &corrupted_fcmp,
                "native FCMP++ verification failed",
            ),
            (
                "corrupted private-IVM",
                &corrupted_ivm,
                "native private-IVM verification failed",
            ),
        ];
        for (label, transaction, expected_reason) in adversarial_transactions {
            let error = submit_signed_transaction(
                &client,
                transaction,
                &format!("{label} transaction must reject"),
            )
            .await
            .expect_err("independently signed corrupted proof was accepted");
            ensure!(
                error_chain_contains(&error, expected_reason),
                "{label} rejected for wrong reason: {error:?}"
            );
            wait_for_transaction_result_on_peers(
                &all_clients,
                transaction,
                false,
                &format!("{label} rejection convergence"),
            )
            .await?;
        }
        for (label, transaction, expected_reason) in &cross_profile_adversaries {
            let error =
                submit_signed_transaction(&client, transaction, &format!("{label} must reject"))
                    .await
                    .expect_err("cross-profile proof substitution was accepted");
            ensure!(
                error_chain_contains(&error, expected_reason),
                "{label} rejected for wrong reason: {error:?}"
            );
            wait_for_transaction_result_on_peers(
                &all_clients,
                transaction,
                false,
                &format!("{label} rejection convergence"),
            )
            .await?;
        }
        for (label, transaction) in &wrong_statement_digest_adversaries {
            let error =
                submit_signed_transaction(&client, transaction, &format!("{label} must reject"))
                    .await
                    .expect_err("wrong statement digest was accepted");
            ensure!(
                error_chain_contains(
                    &error,
                    "privacy envelope statement digest differs from the canonical statement"
                ),
                "{label} rejected for wrong reason: {error:?}"
            );
            wait_for_transaction_result_on_peers(
                &all_clients,
                transaction,
                false,
                &format!("{label} rejection convergence"),
            )
            .await?;
        }
        let two_submit_error = submit_signed_transaction(
            &client,
            &two_submit,
            "two direct privacy submissions must reject at admission",
        )
        .await
        .expect_err("two direct privacy submissions were accepted");
        ensure!(
            error_chain_contains(
                &two_submit_error,
                "privacy transaction intent contains 2 direct privacy submissions"
            ),
            "two-submit transaction rejected for wrong reason: {two_submit_error:?}"
        );
        wait_for_transaction_result_on_peers(
            &all_clients,
            &two_submit,
            false,
            "two-submit admission rejection convergence",
        )
        .await?;
        wait_for_asset_quantities(
            &all_clients,
            &[zk_asset_at_alice.clone(), zk_asset_at_reserve.clone()],
            &[Some(Quantity::from(100_u32)), None],
            "all adversarial failures must preserve public balances",
        )
        .await?;
        for (label, transaction) in [
            ("ZK-ACE", &zk.transaction),
            ("Anonymous-PGC", &pgc.transaction),
            ("VeRange", &verange.transaction),
            ("Bootle/Lantern", &bootle.transaction),
            ("FCMP++", &fcmp.transaction),
            ("private-IVM", &ivm.transaction),
        ] {
            ensure!(
                exact_transaction_result(&client, transaction)?.is_none(),
                "{label} canonical action appeared before canonical submission"
            );
        }

        let restart_index = network.peers().len() - 1;
        let restart_peer = network.peers()[restart_index].clone();
        let config_layers = network.config_layers().collect::<Vec<_>>();
        ensure!(
            restart_peer.shutdown_if_started().await,
            "selected retained exact-12 peer was not running before restart coverage"
        );
        let healthy_clients = network
            .peers()
            .iter()
            .enumerate()
            .filter(|(index, _)| *index != restart_index)
            .map(|(_, peer)| bounded_client(peer.client()))
            .collect::<Vec<_>>();

        for (label, transaction) in [
            ("ZK-ACE", &zk.transaction),
            ("Anonymous-PGC", &pgc.transaction),
            ("VeRange", &verange.transaction),
            ("Bootle/Lantern", &bootle.transaction),
            ("FCMP++", &fcmp.transaction),
            ("private-IVM", &ivm.transaction),
        ] {
            submit_signed_transaction(
                &client,
                transaction,
                &format!("submit canonical {label} action through DA/RBC"),
            )
            .await?;
            wait_for_transaction_result_on_peers(
                &healthy_clients,
                transaction,
                true,
                &format!("healthy-peer canonical {label} finality"),
            )
            .await?;
        }
        wait_for_asset_quantities(
            &healthy_clients,
            &[zk_asset_at_alice.clone(), zk_asset_at_reserve.clone()],
            &[Some(Quantity::from(81_u32)), Some(Quantity::from(19_u32))],
            "canonical ZK-ACE transfer must apply atomically",
        )
        .await?;

        let canonical_height = client
            .get_privacy_capabilities()
            .wrap_err("query height before exact retained exact-12 replays")?
            .committed_height;
        let (canonical_subject_height, canonical_subject) = exact_transaction_block_subject(
            &client,
            &ivm.transaction,
            "derive final canonical privacy carrier subject",
        )?;
        ensure!(
            canonical_subject_height == canonical_height,
            "final canonical privacy carrier height {canonical_subject_height} differs from \
             committed height {canonical_height}"
        );
        for (label, transaction) in [
            ("ZK-ACE", &zk.transaction),
            ("Anonymous-PGC", &pgc.transaction),
            ("VeRange", &verange.transaction),
            ("Bootle/Lantern", &bootle.transaction),
            ("FCMP++", &fcmp.transaction),
            ("private-IVM", &ivm.transaction),
        ] {
            let replay_error = client
                .submit_transaction(transaction)
                .expect_err("exact retained exact-12 transaction replay was accepted");
            ensure!(
                is_exact_transaction_replay(&replay_error),
                "exact {label} replay rejected for wrong reason: {replay_error:?}"
            );
            let observed_height = client
                .get_privacy_capabilities()
                .wrap_err_with(|| format!("query height after exact {label} replay"))?
                .committed_height;
            ensure!(
                observed_height == canonical_height,
                "exact {label} replay changed height from {canonical_height} to {observed_height}"
            );
        }
        wait_for_exact_v2_commit_subject(
            &healthy_clients,
            canonical_height,
            canonical_subject,
            "healthy-peer exact retained exact-12 carrier subject",
        )
        .await?;

        timeout(
            RESTART_TIMEOUT,
            restart_peer.start_checked(config_layers.iter(), None),
        )
        .await
        .map_err(|_| eyre!("retained exact-12 peer restart exceeded {RESTART_TIMEOUT:?}"))?
        .wrap_err("restart retained exact-12 peer")?;
        wait_for_all_peer_protocols(
            &network,
            canonical_height,
            &active_expectations,
            "post-restart active retained exact-12 lifecycles",
        )
        .await?;
        let recovered_clients = network
            .peers()
            .iter()
            .map(|peer| bounded_client(peer.client()))
            .collect::<Vec<_>>();
        let restarted_client = bounded_client(restart_peer.client());
        wait_for_exact_v2_commit_subject(
            &recovered_clients,
            canonical_height,
            canonical_subject,
            "restarted peer exact retained exact-12 carrier subject before later writes",
        )
        .await?;
        for (label, transaction) in [
            ("ZK-ACE", &zk.transaction),
            ("Anonymous-PGC", &pgc.transaction),
            ("VeRange", &verange.transaction),
            ("Bootle/Lantern", &bootle.transaction),
            ("FCMP++", &fcmp.transaction),
            ("private-IVM", &ivm.transaction),
        ] {
            wait_for_transaction_result_on_peers(
                &recovered_clients,
                transaction,
                true,
                &format!("post-restart canonical {label} byte-for-byte visibility"),
            )
            .await?;
        }
        ensure!(
            canonical_genesis_hash(&restarted_client)? == genesis_hash,
            "restarted peer derived a different canonical genesis hash"
        );

        for (label, transaction, expected_reasons) in [
            (
                "ZK-ACE stable-nullifier replay",
                &zk_replay.transaction,
                &["ZK-ACE replay nullifier was already consumed"][..],
            ),
            (
                "Anonymous-PGC stale-head replay",
                &pgc_replay.transaction,
                &["StaleHead", "stale head"][..],
            ),
            (
                "FCMP++ stable-key-image replay",
                &fcmp_replay.transaction,
                &["FCMP++ key image was already consumed"][..],
            ),
            (
                "private-IVM stable-nullifier replay",
                &ivm_replay.transaction,
                &["proof-managed nullifier was already consumed"][..],
            ),
        ] {
            let replay_error = submit_signed_transaction(
                &restarted_client,
                transaction,
                &format!("{label} through restarted peer must reject"),
            )
            .await
            .expect_err("fresh protocol-level replay was accepted");
            ensure!(
                error_chain_contains_any(&replay_error, expected_reasons),
                "{label} rejected for wrong reason: {replay_error:?}"
            );
            wait_for_transaction_result_on_peers(
                &recovered_clients,
                transaction,
                false,
                &format!("{label} rejection convergence"),
            )
            .await?;
        }
        wait_for_asset_quantities(
            &recovered_clients,
            &[zk_asset_at_alice.clone(), zk_asset_at_reserve.clone()],
            &[Some(Quantity::from(81_u32)), Some(Quantity::from(19_u32))],
            "fresh state replays must preserve post-ZK-ACE public balances",
        )
        .await?;

        submit_instructions(
            &restarted_client,
            vec![
                RotatePrivacyBootleLanternIssuerPolicyV1::new(
                    bootle.policy.record_digest,
                    bootle.successor_policy.clone(),
                )
                .into(),
            ],
            "rotate authoritative Bootle/Lantern policy after restart",
        )
        .await?;
        let stale_policy_error = submit_signed_transaction(
            &restarted_client,
            &stale_bootle.transaction,
            "valid proof against stale Bootle/Lantern policy must reject",
        )
        .await
        .expect_err("stale-policy Bootle/Lantern proof was accepted");
        ensure!(
            error_chain_contains(
                &stale_policy_error,
                "statement does not exactly match authoritative issuer-policy state"
            ),
            "stale Bootle/Lantern proof rejected for wrong reason: {stale_policy_error:?}"
        );
        wait_for_transaction_result_on_peers(
            &recovered_clients,
            &stale_bootle.transaction,
            false,
            "stale Bootle/Lantern policy rejection convergence",
        )
        .await?;

        for (label, transaction) in [
            ("pre-activation VeRange", &pre_verange.transaction),
            ("corrupted ZK-ACE", &corrupted_zk),
            ("corrupted Anonymous-PGC", &corrupted_pgc),
            ("corrupted VeRange", &corrupted_verange),
            ("corrupted Bootle/Lantern", &corrupted_bootle),
            ("corrupted FCMP++", &corrupted_fcmp),
            ("corrupted private-IVM", &corrupted_ivm),
            ("two-submit admission", &two_submit),
        ] {
            wait_for_transaction_result_on_peers(
                &recovered_clients,
                transaction,
                false,
                &format!("post-restart {label} rejection visibility"),
            )
            .await?;
        }
        for (label, transaction, _) in &cross_profile_adversaries {
            wait_for_transaction_result_on_peers(
                &recovered_clients,
                transaction,
                false,
                &format!("post-restart {label} rejection visibility"),
            )
            .await?;
        }
        for (label, transaction) in &wrong_statement_digest_adversaries {
            wait_for_transaction_result_on_peers(
                &recovered_clients,
                transaction,
                false,
                &format!("post-restart {label} rejection visibility"),
            )
            .await?;
        }
        wait_for_asset_quantities(
            &recovered_clients,
            &[zk_asset_at_alice, zk_asset_at_reserve],
            &[Some(Quantity::from(81_u32)), Some(Quantity::from(19_u32))],
            "post-restart adversarial and stale-policy failures remain atomic",
        )
        .await?;
        let (final_height, final_subject) = exact_transaction_block_subject(
            &restarted_client,
            &stale_bootle.transaction,
            "derive final stale-policy rejection carrier subject",
        )?;
        ensure!(
            final_height > canonical_height,
            "fresh state/policy rejections did not reach canonical finality"
        );
        wait_for_exact_v2_commit_subject(
            &recovered_clients,
            final_height,
            final_subject,
            "post-restart exact stale-policy carrier subject",
        )
        .await?;
        println!(
            "TAIRA_PRIVACY_PROTOCOL_FOUR_PEER_CASE_V1:privacy_exact12_retained_network::canonical_retained_exact12_actions_survive_four_peer_adversarial_replay_and_restart:passed"
        );
        Ok(())
    }
    .await;

    network.shutdown().await;
    result
}
