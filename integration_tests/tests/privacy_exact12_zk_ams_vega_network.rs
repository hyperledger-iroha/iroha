#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! Four-validator activation, adversarial intent/governance, replay, and
//! restart coverage for the canonical native ZK-AMS and Vega action APIs.
//!
//! This release gate intentionally leaves the network's DA/RBC configuration
//! untouched. A validator is stopped only after all negative probes converge,
//! then both exact proof transactions are finalized by the remaining quorum
//! and recovered byte-for-byte by the restarted validator.

use std::{
    num::NonZeroU32,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use eyre::{Result, WrapErr as _, ensure, eyre};
use integration_tests::sandbox;
use iroha::client::Client;
use iroha_core::{
    privacy::PRIVACY_MIN_ACTIVATION_DELAY_BLOCKS_V1,
    privacy_engines::{
        p256::TranscriptBindingV1,
        vega::{
            VegaPrivacyActionPublicInputV1, VegaPrivacyActionTransactionContextV1,
            VegaPrivacyActionWitnessMaterialV1, build_signed_vega_privacy_action_with_rng_v1,
        },
        zk_ams::{
            ZkAmsBatchCredentialWitnessV1, ZkAmsMaskedProverConfigV1,
            ZkAmsPrivacyActionGovernanceV1, ZkAmsPrivacyActionTransactionContextV1,
            ZkAmsSeedSecretV1, prepare_zk_ams_batch_admission_transaction_intent_v1,
            prove_zk_ams_batch_admission_v1, validate_zk_ams_privacy_action_transaction_intent_v1,
            verify_zk_ams_batch_admission_v1, zk_ams_generator_digest_v1,
            zk_ams_registry_transition_root_v1, zk_ams_seed_public_key_v1,
        },
    },
    privacy_profiles::{CompiledPrivacyProfileV1, compiled_privacy_profile_v1},
};
use iroha_data_model::{
    Level,
    isi::{
        Grant, InstructionBox, Log,
        privacy::{
            BootstrapPrivacyZkAmsRegistryV1, RegisterPrivacyProtocolActivationV1,
            RegisterPrivacyVegaIssuerV1, SubmitPrivacyProofV1,
        },
    },
    metadata::Metadata,
    permission::Permission,
    privacy::{
        IrohaZkAmsProofV1, PrivacyActiveLifecycleV1, PrivacyCapabilityRowV1,
        PrivacyCapabilitySnapshotV1, PrivacyChallengeV1, PrivacyCompiledProfileResultV1,
        PrivacyCompiledProfileSnapshotV1, PrivacyCredentialDocumentTypeV1, PrivacyIssuerIdV1,
        PrivacyP256PointV1, PrivacyParameterDigestV1, PrivacyPolicyDigestV1, PrivacyPolicyIdV1,
        PrivacyProofBytesV1, PrivacyProofEnvelopeV1, PrivacyProofV1,
        PrivacyProposedLifecycleV1, PrivacyProtocolActivationRecordV1, PrivacyProtocolIdV1,
        PrivacyProtocolLifecycleV1, PrivacyRootV1, PrivacySessionTranscriptDigestV1,
        PrivacyStatementDigestV1, PrivacyStatementV1, PrivacyTransactionIntentDigestV1,
        PrivacyVegaIssuerRecordLifecycleV1, PrivacyVegaIssuerRecordV1,
        PrivacyVegaMdlDateV1, PrivacyVegaMdlDigestAlgorithmV1,
        PrivacyVegaMdlNamespaceV1, PrivacyVegaMdlSignatureAlgorithmV1,
        PrivacyZkAmsActionV1, PrivacyZkAmsAdmissionAnchorV1,
        PrivacyZkAmsBatchAdmissionV1, PrivacyZkAmsCredentialNonceV1,
        PrivacyZkAmsPersonhoodCredentialV1, PrivacyZkAmsRegistryBootstrapV1,
        PrivacyZkAmsRegistryRecordDigestV1, PrivacyZkAmsSeedPublicKeyV1,
        PrivacyZkAmsSubjectCommitmentV1, VEGA_MDL_BIRTH_DATE_ISSUER_SIGNED_ITEM_BYTES_V1,
        VEGA_MDL_ISSUER_AUTHENTICATION_SIG_STRUCTURE_BYTES_V1, VEGA_MDL_MSO_PAYLOAD_BYTES_V1,
        ZK_AMS_PHC_VERSION_V1, ZK_AMS_REGISTRY_BOOTSTRAP_INITIAL_EPOCH_V1,
    },
    query::{block::prelude::FindBlocks, transaction::prelude::FindTransactions},
    transaction::{
        Executable, FeePaymentIntent, SignedTransaction, TransactionBuilder,
        TransactionEntrypoint, TransactionPayload,
    },
};
use iroha_executor_data_model::permission::governance::CanEnactGovernance;
use iroha_test_network::{NetworkBuilder, init_instruction_registry};
use p256::{
    ecdsa::{
        Signature as P256Signature, SigningKey as P256SigningKey,
        signature::hazmat::PrehashSigner as _,
    },
    elliptic_curve::sec1::ToEncodedPoint as _,
};
use rand_core_06::{CryptoRng, Error as RngError, RngCore};
use sha2::{Digest, Sha256};
use tokio::time::{Instant, sleep, timeout};

const ZK_AMS_PROTOCOL: PrivacyProtocolIdV1 = PrivacyProtocolIdV1::IrohaZkAmsV1;
const VEGA_PROTOCOL: PrivacyProtocolIdV1 = PrivacyProtocolIdV1::VegaExistingCredentialZkV0;
const SUBMISSION_TIMEOUT: Duration = Duration::from_secs(120);
const PEER_CONVERGENCE_TIMEOUT: Duration = Duration::from_secs(90);
const RESTART_TIMEOUT: Duration = Duration::from_secs(90);
const ACTIVATION_ADVANCE_TIMEOUT: Duration = Duration::from_secs(180);
const TEST_BLOCK_CADENCE: Duration = Duration::from_millis(100);
const POLL_INTERVAL: Duration = Duration::from_millis(200);
const ACTION_TTL: Duration = Duration::from_secs(7_200);

struct DeterministicCryptoRng {
    seed: [u8; 32],
    counter: u64,
    buffered: [u8; 32],
    offset: usize,
}

impl DeterministicCryptoRng {
    fn new(seed: [u8; 32]) -> Self {
        Self {
            seed,
            counter: 0,
            buffered: [0; 32],
            offset: 32,
        }
    }

    fn refill(&mut self) {
        let mut hash = Sha256::new();
        hash.update(b"iroha.integration.privacy.zk-ams-vega.rng.v1");
        hash.update(self.seed);
        hash.update(self.counter.to_be_bytes());
        self.buffered = hash.finalize().into();
        self.counter = self
            .counter
            .checked_add(1)
            .expect("test RNG counter cannot exhaust");
        self.offset = 0;
    }
}

impl RngCore for DeterministicCryptoRng {
    fn next_u32(&mut self) -> u32 {
        let mut bytes = [0_u8; 4];
        self.fill_bytes(&mut bytes);
        u32::from_le_bytes(bytes)
    }

    fn next_u64(&mut self) -> u64 {
        let mut bytes = [0_u8; 8];
        self.fill_bytes(&mut bytes);
        u64::from_le_bytes(bytes)
    }

    fn fill_bytes(&mut self, destination: &mut [u8]) {
        let mut written = 0;
        while written < destination.len() {
            if self.offset == self.buffered.len() {
                self.refill();
            }
            let available = self.buffered.len() - self.offset;
            let take = available.min(destination.len() - written);
            destination[written..written + take]
                .copy_from_slice(&self.buffered[self.offset..self.offset + take]);
            self.offset += take;
            written += take;
        }
    }

    fn try_fill_bytes(&mut self, destination: &mut [u8]) -> Result<(), RngError> {
        self.fill_bytes(destination);
        Ok(())
    }
}

impl CryptoRng for DeterministicCryptoRng {}

#[derive(Clone)]
struct ZkAmsFixture {
    bootstrap: PrivacyZkAmsRegistryBootstrapV1,
    credential: PrivacyZkAmsPersonhoodCredentialV1,
    issuer_signature: [u8; 64],
    seed_secret_bytes: [u8; 32],
}

struct VegaFixture {
    issuer_record: PrivacyVegaIssuerRecordV1,
    public_input: VegaPrivacyActionPublicInputV1,
    witness_material: VegaPrivacyActionWitnessMaterialV1,
    device_signing_key: P256SigningKey,
}

fn bounded_client(mut client: Client) -> Client {
    client.transaction_status_timeout = SUBMISSION_TIMEOUT;
    client.torii_request_timeout = Duration::from_secs(30);
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

fn is_exact_replay_error(error: &eyre::Report) -> bool {
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
    snapshot: &PrivacyCapabilitySnapshotV1,
    protocol: PrivacyProtocolIdV1,
) -> Result<PrivacyCapabilityRowV1> {
    snapshot
        .protocols
        .iter()
        .copied()
        .find(|row| row.protocol_id == protocol)
        .ok_or_else(|| eyre!("canonical capability snapshot omitted {protocol:?}"))
}

fn assert_exact_protocol_row(
    snapshot: &PrivacyCapabilitySnapshotV1,
    protocol: PrivacyProtocolIdV1,
    compiled: PrivacyCompiledProfileSnapshotV1,
    activation: Option<PrivacyProtocolActivationRecordV1>,
    context: &str,
) -> Result<()> {
    snapshot
        .validate()
        .wrap_err_with(|| format!("{context}: invalid capability snapshot"))?;
    let row = protocol_row(snapshot, protocol)?;
    ensure!(
        row.compiled_profile == PrivacyCompiledProfileResultV1::Available(compiled),
        "{context}: {protocol:?} compiled binding drifted: {:?}",
        row.compiled_profile
    );
    ensure!(
        row.activation == activation,
        "{context}: {protocol:?} activation mismatch: expected {activation:?}, got {:?}",
        row.activation
    );
    Ok(())
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
    ensure!(hash != [0; 32], "canonical genesis hash must be non-zero");
    Ok(hash)
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

async fn submit_instruction(
    client: &Client,
    instruction: impl Into<InstructionBox>,
    context: &str,
) -> Result<iroha_crypto::HashOf<SignedTransaction>> {
    let client = client.clone();
    let instruction = instruction.into();
    timeout(
        SUBMISSION_TIMEOUT,
        tokio::task::spawn_blocking(move || client.submit_blocking(instruction, no_fee())),
    )
    .await
    .map_err(|_| eyre!("{context}: instruction submission exceeded {SUBMISSION_TIMEOUT:?}"))?
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

async fn wait_for_all_peer_activations(
    network: &sandbox::SerializedNetwork,
    minimum_height: u64,
    rows: &[(
        PrivacyProtocolIdV1,
        PrivacyCompiledProfileSnapshotV1,
        Option<PrivacyProtocolActivationRecordV1>,
    )],
    context: &str,
) -> Result<Vec<PrivacyCapabilitySnapshotV1>> {
    let deadline = Instant::now() + PEER_CONVERGENCE_TIMEOUT;
    let mut last_observed = Vec::new();
    loop {
        let mut snapshots = Vec::with_capacity(network.peers().len());
        last_observed.clear();
        for (index, peer) in network.peers().iter().enumerate() {
            let client = bounded_client(peer.client());
            match client.get_privacy_capabilities() {
                Ok(snapshot) => {
                    if snapshot.committed_height < minimum_height {
                        last_observed.push(format!(
                            "peer {index}: height={} below {minimum_height}",
                            snapshot.committed_height
                        ));
                        continue;
                    }
                    let exact = rows.iter().try_for_each(
                        |(protocol, compiled, activation)| {
                            assert_exact_protocol_row(
                                &snapshot,
                                *protocol,
                                *compiled,
                                *activation,
                                context,
                            )
                        },
                    );
                    match exact {
                        Ok(()) => {
                            last_observed.push(format!(
                                "peer {index}: exact rows at height {}",
                                snapshot.committed_height
                            ));
                            snapshots.push(snapshot);
                        }
                        Err(error) => last_observed.push(format!(
                            "peer {index}: height={}, exact-row mismatch: {error}",
                            snapshot.committed_height
                        )),
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

async fn advance_to_exact_height(
    client: &Client,
    target_height: u64,
    label: &str,
) -> Result<()> {
    let start = client
        .get_privacy_capabilities()
        .wrap_err("query height before deterministic activation advance")?
        .committed_height;
    ensure!(
        start <= target_height,
        "cannot advance backwards from committed height {start} to {target_height}"
    );
    if start < target_height {
        let first_incoming_height = start
            .checked_add(1)
            .ok_or_else(|| eyre!("deterministic activation advance height overflowed"))?;
        for incoming_height in first_incoming_height..=target_height {
            submit_instruction(
                client,
                Log::new(
                    Level::INFO,
                    format!("{label} activation advance block {incoming_height}"),
                ),
                "advance joint privacy activation height",
            )
            .await?;
        }
    }
    let observed = client
        .get_privacy_capabilities()
        .wrap_err("query height after deterministic activation advance")?
        .committed_height;
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
        "canonical privacy transaction is visible but finalized as rejected"
    );
    Ok(true)
}

async fn wait_for_transactions_on_peers(
    clients: &[Client],
    transactions: &[(&str, &SignedTransaction)],
    context: &str,
) -> Result<()> {
    let deadline = Instant::now() + PEER_CONVERGENCE_TIMEOUT;
    let mut last_observed = Vec::new();
    loop {
        let mut visible = 0_usize;
        last_observed.clear();
        for (peer_index, client) in clients.iter().enumerate() {
            for (label, transaction) in transactions {
                match exact_applied_transaction_visible(client, transaction) {
                    Ok(true) => {
                        visible += 1;
                        last_observed.push(format!(
                            "peer {peer_index}: exact applied {label} transaction visible"
                        ));
                    }
                    Ok(false) => last_observed.push(format!(
                        "peer {peer_index}: {label} transaction absent"
                    )),
                    Err(error) => {
                        last_observed.push(format!("peer {peer_index}: {label}: {error}"))
                    }
                }
            }
        }
        if visible == clients.len() * transactions.len() {
            return Ok(());
        }
        if Instant::now() >= deadline {
            return Err(eyre!(
                "{context}: finalized transactions did not converge within \
                 {PEER_CONVERGENCE_TIMEOUT:?}; {}",
                last_observed.join("; ")
            ));
        }
        sleep(POLL_INTERVAL).await;
    }
}

fn p256_public_key(signing_key: &P256SigningKey) -> Result<PrivacyP256PointV1> {
    let encoded = signing_key.verifying_key().to_encoded_point(true);
    let bytes: [u8; 33] = encoded
        .as_bytes()
        .try_into()
        .map_err(|_| eyre!("compressed P-256 point did not contain 33 bytes"))?;
    Ok(PrivacyP256PointV1::new(bytes))
}

fn zk_ams_seed_secret(seed: u64) -> Result<ZkAmsSeedSecretV1> {
    let mut bytes = [0_u8; 32];
    bytes[..8].copy_from_slice(&seed.to_le_bytes());
    ZkAmsSeedSecretV1::from_bytes(bytes)
        .map_err(|error| eyre!("construct canonical ZK-AMS seed secret: {error}"))
}

fn zk_ams_fixture() -> Result<ZkAmsFixture> {
    let issuer_signing_key = P256SigningKey::from_bytes((&[7_u8; 32]).into())
        .map_err(|_| eyre!("construct fixed ZK-AMS issuer key"))?;
    let issuer_id = PrivacyIssuerIdV1::new([0x31; 32]);
    let registry_id = iroha_data_model::privacy::PrivacyZkAmsRegistryIdV1::new([0x33; 32]);
    let policy_id = PrivacyPolicyIdV1::new([0x35; 32]);
    let bootstrap = PrivacyZkAmsRegistryBootstrapV1 {
        issuer_id,
        registry_id,
        policy_id,
        issuer_public_key: p256_public_key(&issuer_signing_key)?,
        policy_digest: PrivacyPolicyDigestV1::new([0x36; 32]),
        initial_registry_root: PrivacyRootV1::new([0x37; 32]),
        initial_registry_epoch: ZK_AMS_REGISTRY_BOOTSTRAP_INITIAL_EPOCH_V1,
    };
    bootstrap
        .validate()
        .map_err(|error| eyre!("validate ZK-AMS bootstrap fixture: {error}"))?;

    let seed_secret = zk_ams_seed_secret(41)?;
    let mut seed_secret_bytes = [0_u8; 32];
    seed_secret_bytes[..8].copy_from_slice(&41_u64.to_le_bytes());
    let credential = PrivacyZkAmsPersonhoodCredentialV1 {
        version: ZK_AMS_PHC_VERSION_V1,
        issuer_id,
        policy_id,
        subject_commitment: PrivacyZkAmsSubjectCommitmentV1::new([0x41; 32]),
        seed_public_key: PrivacyZkAmsSeedPublicKeyV1::new(zk_ams_seed_public_key_v1(&seed_secret)),
        credential_nonce: PrivacyZkAmsCredentialNonceV1::new([0x51; 32]),
    };
    let signature: P256Signature = issuer_signing_key
        .sign_prehash(credential.digest().as_bytes())
        .map_err(|_| eyre!("sign fixed ZK-AMS credential"))?;
    let signature = signature.normalize_s().unwrap_or(signature);
    Ok(ZkAmsFixture {
        bootstrap,
        credential,
        issuer_signature: signature.to_bytes().into(),
        seed_secret_bytes,
    })
}

fn zk_ams_transaction_context(
    client: &Client,
    creation_time: Duration,
    nonce: u32,
) -> ZkAmsPrivacyActionTransactionContextV1 {
    ZkAmsPrivacyActionTransactionContextV1 {
        chain_id: client.chain.clone(),
        authority: client.account.clone(),
        creation_time,
        time_to_live: Some(ACTION_TTL),
        nonce: NonZeroU32::new(nonce),
        fee_payment: no_fee(),
        metadata: Metadata::default(),
    }
}

fn build_transaction_from_envelope(
    context: &ZkAmsPrivacyActionTransactionContextV1,
    envelope: PrivacyProofEnvelopeV1,
    client: &Client,
) -> Result<SignedTransaction> {
    let mut builder = TransactionBuilder::new(
        context.chain_id.clone(),
        context.authority.clone(),
        context.fee_payment.clone(),
    )
    .with_instructions([SubmitPrivacyProofV1::new(envelope)])
    .with_metadata(context.metadata.clone());
    builder.set_creation_time(context.creation_time);
    if let Some(ttl) = context.time_to_live {
        builder.set_ttl(ttl);
    }
    if let Some(nonce) = context.nonce {
        builder.set_nonce(nonce);
    }
    let payload = builder
        .into_payload()
        .wrap_err("construct final ZK-AMS transaction payload")?;
    payload
        .validate_privacy_transaction_intent_binding_v1()
        .wrap_err("validate final ZK-AMS transaction intent")?;
    let signed = TransactionBuilder::from_payload(payload)
        .wrap_err("re-open final ZK-AMS payload for signing")?
        .try_sign(client.key_pair.private_key())
        .wrap_err("sign final ZK-AMS transaction")?;
    signed
        .verify_signature()
        .wrap_err("verify final ZK-AMS transaction signature")?;
    Ok(signed)
}

fn build_zk_ams_action(
    client: &Client,
    fixture: &ZkAmsFixture,
    canonical_genesis_hash: [u8; 32],
    nonce: u32,
    proof_seed: [u8; 32],
) -> Result<SignedTransaction> {
    let context = zk_ams_transaction_context(client, now_duration()?, nonce);
    let anchor = PrivacyZkAmsAdmissionAnchorV1 {
        phc_hash: fixture.credential.digest(),
        seed_public_key: fixture.credential.seed_public_key,
    };
    let current_epoch = fixture.bootstrap.initial_registry_epoch;
    let next_epoch = current_epoch
        .checked_add(1)
        .ok_or_else(|| eyre!("ZK-AMS registry epoch overflowed"))?;
    let next_root = zk_ams_registry_transition_root_v1(
        fixture.bootstrap.registry_id,
        fixture.bootstrap.initial_registry_root,
        current_epoch,
        next_epoch,
        1,
        0,
        anchor,
    );
    let action = PrivacyZkAmsBatchAdmissionV1 {
        account_registry_root: fixture.bootstrap.initial_registry_root,
        account_registry_root_epoch: current_epoch,
        next_account_registry_root: next_root,
        next_account_registry_root_epoch: next_epoch,
        anchors: vec![anchor],
    };
    let governance = ZkAmsPrivacyActionGovernanceV1 {
        issuer_id: fixture.bootstrap.issuer_id,
        issuer_public_key: fixture.bootstrap.issuer_public_key,
        issuer_policy_record_digest: fixture.bootstrap.issuer_policy_record_digest(),
        registry_id: fixture.bootstrap.registry_id,
        registry_record_digest: fixture.bootstrap.registry_record_digest(),
        policy_id: fixture.bootstrap.policy_id,
        policy_digest: fixture.bootstrap.policy_digest,
    };
    let statement = prepare_zk_ams_batch_admission_transaction_intent_v1(
        &context,
        governance,
        action,
    )
    .map_err(|error| eyre!("prepare canonical ZK-AMS action intent: {error}"))?;
    let intent = validate_zk_ams_privacy_action_transaction_intent_v1(&context, &statement)
        .map_err(|error| eyre!("revalidate canonical ZK-AMS action intent: {error}"))?;
    ensure!(
        intent == statement.context.transaction_intent_digest,
        "ZK-AMS prepared intent differs from the statement"
    );
    let typed_statement = PrivacyStatementV1::IrohaZkAmsV1(statement.clone());
    let statement_digest = typed_statement
        .digest()
        .wrap_err("derive canonical ZK-AMS statement digest")?;
    let binding = TranscriptBindingV1 {
        chain_id: statement.context.chain_id.as_str().as_bytes(),
        genesis_hash: canonical_genesis_hash,
        action_index: statement.context.action_index,
        statement_digest: *statement_digest.as_bytes(),
        parameter_id: *statement.context.parameter_id.as_bytes(),
        parameter_digest: *statement.context.parameter_digest.as_bytes(),
        verifier_digest: *statement.context.verifier_digest.as_bytes(),
        statement_schema_digest: *statement.context.statement_schema_digest.as_bytes(),
        engine_manifest_digest: *statement.context.engine_manifest_digest.as_bytes(),
        generator_digest: zk_ams_generator_digest_v1(),
    };
    let seed_secret = ZkAmsSeedSecretV1::from_bytes(fixture.seed_secret_bytes)
        .map_err(|error| eyre!("restore canonical ZK-AMS seed secret: {error}"))?;
    let witnesses = [ZkAmsBatchCredentialWitnessV1::new(
        &fixture.credential,
        &fixture.issuer_signature,
        &seed_secret,
    )];
    let mut rng = DeterministicCryptoRng::new(proof_seed);
    let proof = prove_zk_ams_batch_admission_v1(
        &statement,
        &binding,
        &witnesses,
        ZkAmsMaskedProverConfigV1::new(1)
            .map_err(|error| eyre!("construct deterministic ZK-AMS prover config: {error}"))?,
        &mut rng,
    )
    .map_err(|error| eyre!("prove canonical ZK-AMS batch admission: {error}"))?;
    let effect = verify_zk_ams_batch_admission_v1(&statement, &binding, &proof)
        .map_err(|error| eyre!("verify locally produced ZK-AMS proof: {error}"))?;
    ensure!(
        effect.next_root == next_root
            && effect.next_epoch == next_epoch
            && effect.anchors == vec![anchor],
        "locally verified ZK-AMS state effect drifted"
    );

    let profile = compiled_privacy_profile_v1(ZK_AMS_PROTOCOL)
        .wrap_err("load canonical compiled ZK-AMS profile for envelope")?;
    let envelope = PrivacyProofEnvelopeV1 {
        protocol_id: profile.protocol_id,
        proof_system_id: profile.proof_system_id,
        engine_id: profile.engine_id,
        parameter_id: profile.parameter_id,
        parameter_digest: profile.parameter_digest,
        verifier_digest: profile.verifier_digest,
        statement_schema_digest: profile.statement_schema_digest,
        engine_manifest_digest: profile.engine_manifest_digest,
        statement_digest,
        statement: typed_statement,
        proof: PrivacyProofV1::IrohaZkAmsV1(
            IrohaZkAmsProofV1::MaskedRelaxedSpartanBatchAdmission(
                PrivacyProofBytesV1::new(proof),
            ),
        ),
    };
    build_transaction_from_envelope(&context, envelope, client)
}
