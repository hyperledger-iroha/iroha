//! Taira-shaped coverage for the ZK-ACE public-pin gate and non-shipping canary.
//!
//! The committed candidate canary must be invoked with an explicitly
//! feature-matched daemon, for example:
//! `TEST_NETWORK_IROHAD_FEATURES=zk-stark,privacy-release-evidence` plus the
//! integration test features `zk-stark,privacy-release-evidence`. An optional
//! absolute `IROHA_ZK_ACE_NETWORK_SEMANTIC_RECEIPT_OUT` publishes the validated
//! canonical receipt once, without replacing any existing filesystem entry.
#![cfg(feature = "zk-stark")]
use eyre::{Result, WrapErr as _, ensure, eyre};
use integration_tests::{
    privacy_exact12_controller::{
        require_privacy_action_receipt_on_peer_v1, submit_signed_privacy_action_and_wait_v1,
    },
    sandbox,
};
#[cfg(feature = "privacy-release-evidence")]
use iroha::data_model::{
    Level, ValidationFail,
    asset::{AssetBalancePolicy, AssetDefinition},
    isi::{
        Grant, InstructionBox, Log, Mint, Register,
        error::{InstructionExecutionError, InvalidParameterError},
        privacy::{
            RegisterPrivacyProtocolActivationV1, RegisterPrivacyZkAcePolicyV1, SubmitPrivacyProofV1,
        },
    },
    permission::Permission,
    prelude::{AssetId, FindAssets, Identifiable, Quantity},
    privacy::{
        PrivacyActionLocalStateV1, PrivacyActionTerminalChainStateV1, PrivacyActiveLifecycleV1,
        PrivacyCompiledProfileSnapshotV1, PrivacyExact12CapabilityManifestV1,
        PrivacyLedgerEffectKindV1, PrivacyOperationSchemaV1, PrivacyProposedLifecycleV1,
        PrivacyProtocolActivationRecordV1, PrivacyProtocolLifecycleV1,
        PrivacyZkAceReplayNullifierProvenanceV1,
    },
    query::transaction::prelude::FindTransactions,
    transaction::{
        SignedTransaction, TransactionEntrypoint, TransactionResult,
        error::TransactionRejectionReason,
    },
};
use iroha::{
    client::Client,
    data_model::{
        asset::AssetBalanceScope,
        metadata::Metadata,
        prelude::{AssetDefinitionId, DomainId, QueryBuilderExt},
        privacy::{
            PRIVACY_ZK_ACE_POLICY_INITIAL_EPOCH_V1, PrivacyCompiledProfileResultV1,
            PrivacyCompiledProfileUnavailableReasonV1, PrivacyPolicyDigestV1, PrivacyPolicyIdV1,
            PrivacyProtocolIdV1, PrivacyZkAcePolicyLifecycleV1, PrivacyZkAcePolicyRecordV1,
        },
        query::block::prelude::FindBlocks,
        transaction::FeePaymentIntent,
    },
};
use iroha_core::privacy_engines::zk_ace::ZK_ACE_FULL_ENGINE_AVAILABLE_V1;
#[cfg(feature = "privacy-release-evidence")]
use iroha_core::privacy_engines::zk_ace::{
    ZK_ACE_RELEASE_STAGE_EVIDENCE_SHA256_V2, zk_ace_compiled_profile_digest_v1,
    zk_ace_nonshipping_release_candidate_available_v2, zk_ace_public_release_pins_complete_v2,
};
use iroha_core::privacy_profiles::{
    CompiledPrivacyProfileErrorV1, compiled_privacy_profile_snapshot_result_v1,
    compiled_privacy_profile_v1,
};
#[cfg(feature = "privacy-release-evidence")]
use iroha_core::{
    privacy::PRIVACY_MIN_ACTIVATION_DELAY_BLOCKS_V1,
    privacy_profiles::{CompiledPrivacyProfileV1, nonshipping_zk_ace_release_candidate_profile_v2},
    privacy_release_evidence::{
        PRIVACY_ZK_ACE_NETWORK_SEMANTIC_RECEIPT_MAX_BYTES_V1, PrivacyReleaseTransactionContextV1,
        PrivacyReleaseZkAceNetworkActionV1, PrivacyZkAceActivationReceiptV1,
        PrivacyZkAceAppliedTransferReceiptV1, PrivacyZkAceCanonicalTransactionAnchorV1,
        PrivacyZkAceNetworkSemanticCorridorV1, PrivacyZkAceNetworkSemanticReceiptV1,
        PrivacyZkAceRejectedReplayReceiptV1, PrivacyZkAceReplayRejectionKindV1,
        PrivacyZkAceValidatorReplayObservationV1, build_privacy_release_zk_ace_network_action_v1,
    },
};
#[cfg(feature = "privacy-release-evidence")]
use iroha_executor_data_model::permission::governance::CanEnactGovernance;
use iroha_test_network::{NetworkBuilder, init_instruction_registry};
use iroha_test_samples::{ALICE_ID, ALICE_KEYPAIR, BOB_ID};
#[cfg(feature = "privacy-release-evidence")]
use sha2::{Digest as _, Sha256};
#[cfg(all(feature = "privacy-release-evidence", unix))]
use std::os::unix::fs::OpenOptionsExt as _;
#[cfg(feature = "privacy-release-evidence")]
use std::{
    fs::{self, File, OpenOptions},
    io::Write as _,
    path::{Component, Path, PathBuf},
};
use std::{
    num::NonZeroU32,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use zk_ace_prover::{
    ZkAcePrivacyActionBuildErrorV1, ZkAcePrivacyActionTransactionContextV1, ZkAcePrivacyTransferV1,
    ZkAcePrivacyWitnessV1, build_signed_zk_ace_privacy_transfer_v1,
};

const TEST_NAME: &str = "zk_ace_privacy_transfer_fails_closed_taira_localnet";
const PROTOCOL: PrivacyProtocolIdV1 = PrivacyProtocolIdV1::ZkAcePqAuthorizationV0;
const SUBMISSION_TIMEOUT: Duration = Duration::from_secs(180);
#[cfg(feature = "privacy-release-evidence")]
const PEER_CONVERGENCE_TIMEOUT: Duration = Duration::from_secs(180);
#[cfg(feature = "privacy-release-evidence")]
const POLL_INTERVAL: Duration = Duration::from_millis(250);
#[cfg(feature = "privacy-release-evidence")]
const ACTION_TTL: Duration = Duration::from_secs(7_200);
#[cfg(feature = "privacy-release-evidence")]
const CONSUMED_REPLAY_NULLIFIER_MESSAGE: &str = "ZK-ACE replay nullifier was already consumed";
#[cfg(feature = "privacy-release-evidence")]
const RECEIPT_OUTPUT_ENV: &str = "IROHA_ZK_ACE_NETWORK_SEMANTIC_RECEIPT_OUT";

fn no_fee() -> FeePaymentIntent {
    FeePaymentIntent::authority(Vec::new(), None)
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
        "{TEST_NAME}: TEST_NETWORK_IROHAD_FEATURES must include `{feature}`"
    );
    Ok(())
}

#[cfg(feature = "privacy-release-evidence")]
fn selected_zk_ace_release_profile() -> Result<(
    CompiledPrivacyProfileV1,
    PrivacyZkAceNetworkSemanticCorridorV1,
)> {
    if ZK_ACE_FULL_ENGINE_AVAILABLE_V1 {
        ensure!(
            zk_ace_public_release_pins_complete_v2(),
            "public ZK-ACE availability disagrees with its complete release-pin gate"
        );
        return Ok((
            compiled_privacy_profile_v1(PROTOCOL)
                .wrap_err("load public post-pin ZK-ACE profile")?,
            PrivacyZkAceNetworkSemanticCorridorV1::PublicPostPin,
        ));
    }
    ensure!(
        zk_ace_nonshipping_release_candidate_available_v2(),
        "the non-shipping ZK-ACE candidate requires four complete distinct stage pins and a still-open public semantic pin"
    );
    ensure!(
        compiled_privacy_profile_v1(PROTOCOL)
            == Err(CompiledPrivacyProfileErrorV1::EngineUnavailable {
                protocol_id: PROTOCOL,
            }),
        "ordinary compiled ZK-ACE profile became available in the non-shipping candidate corridor"
    );
    Ok((
        nonshipping_zk_ace_release_candidate_profile_v2()
            .wrap_err("load bounded non-shipping ZK-ACE candidate profile")?,
        PrivacyZkAceNetworkSemanticCorridorV1::NonshippingPrivacyReleaseEvidenceCandidate,
    ))
}

#[cfg(feature = "privacy-release-evidence")]
fn requested_receipt_output_path() -> Result<Option<PathBuf>> {
    let Some(raw) = std::env::var_os(RECEIPT_OUTPUT_ENV) else {
        return Ok(None);
    };
    ensure!(
        !raw.is_empty(),
        "{RECEIPT_OUTPUT_ENV} was explicitly supplied but empty"
    );
    let path = PathBuf::from(raw);
    validate_new_receipt_output_path(&path)?;
    Ok(Some(path))
}

#[cfg(feature = "privacy-release-evidence")]
fn validate_new_receipt_output_path(path: &Path) -> Result<()> {
    ensure!(
        path.is_absolute(),
        "requested ZK-ACE receipt output must be an absolute path"
    );
    ensure!(
        path.components()
            .all(|component| matches!(component, Component::RootDir | Component::Normal(_))),
        "requested ZK-ACE receipt output contains a non-canonical path component"
    );
    let parent = path
        .parent()
        .ok_or_else(|| eyre!("requested ZK-ACE receipt output has no parent directory"))?;
    let canonical_parent = parent
        .canonicalize()
        .wrap_err("canonicalize requested ZK-ACE receipt output parent")?;
    ensure!(
        canonical_parent == parent,
        "requested ZK-ACE receipt output parent traverses a symlink or alias"
    );
    ensure!(
        path.file_name().and_then(|name| name.to_str()).is_some(),
        "requested ZK-ACE receipt output has no canonical UTF-8 file name"
    );
    match fs::symlink_metadata(path) {
        Ok(_) => return Err(eyre!("requested ZK-ACE receipt output already exists")),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => {
            return Err(error).wrap_err("inspect requested ZK-ACE receipt output");
        }
    }
    Ok(())
}

#[cfg(feature = "privacy-release-evidence")]
fn write_new_receipt_atomic(path: &Path, bytes: &[u8]) -> Result<()> {
    validate_new_receipt_output_path(path)?;
    ensure!(
        !bytes.is_empty() && bytes.len() <= PRIVACY_ZK_ACE_NETWORK_SEMANTIC_RECEIPT_MAX_BYTES_V1,
        "refusing to persist an empty or oversized ZK-ACE receipt"
    );
    let parent = path
        .parent()
        .ok_or_else(|| eyre!("requested ZK-ACE receipt output has no parent directory"))?;
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| eyre!("requested ZK-ACE receipt output file name is invalid"))?;
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .wrap_err("system clock before Unix epoch while staging receipt")?
        .as_nanos();
    let temporary = parent.join(format!(
        ".{file_name}.{}.{}.partial",
        std::process::id(),
        nonce
    ));
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    options.mode(0o600);
    let mut file = options
        .open(&temporary)
        .wrap_err("create private ZK-ACE receipt staging file")?;
    let staged = (|| -> Result<()> {
        file.write_all(bytes)
            .wrap_err("write complete canonical ZK-ACE receipt")?;
        file.sync_all()
            .wrap_err("sync complete canonical ZK-ACE receipt")?;
        drop(file);
        fs::hard_link(&temporary, path)
            .wrap_err("atomically publish new no-clobber ZK-ACE receipt")?;
        File::open(parent)
            .and_then(|directory| directory.sync_all())
            .wrap_err("sync ZK-ACE receipt output directory")?;
        Ok(())
    })();
    let cleanup = fs::remove_file(&temporary);
    staged?;
    cleanup.wrap_err("remove published ZK-ACE receipt staging link")?;
    Ok(())
}

fn asset_definition_id() -> AssetDefinitionId {
    AssetDefinitionId::derive_from_components(
        DomainId::try_new("wonderland", "universal").expect("default domain"),
        "zkace_typed".parse().expect("asset name"),
    )
}

fn canonical_genesis_hash(client: &Client) -> Result<[u8; 32]> {
    let blocks = client
        .query(FindBlocks)
        .execute_all()
        .wrap_err("query canonical genesis block")?;
    let genesis = blocks
        .iter()
        .filter(|block| block.header().prev_block_hash().is_none())
        .collect::<Vec<_>>();
    ensure!(
        genesis.len() == 1,
        "expected exactly one genesis block, got {}",
        genesis.len()
    );
    let hash = *genesis[0].header().hash().as_ref();
    ensure!(hash != [0; 32], "canonical genesis hash is zero");
    Ok(hash)
}

fn witness(seed: u8) -> ZkAcePrivacyWitnessV1 {
    ZkAcePrivacyWitnessV1::try_new(
        [seed; 32],
        [seed.wrapping_add(1); 32],
        [seed.wrapping_add(2); 32],
    )
    .expect("valid localnet witness")
}

fn policy(witness: &ZkAcePrivacyWitnessV1) -> PrivacyZkAcePolicyRecordV1 {
    PrivacyZkAcePolicyRecordV1::new(
        PrivacyPolicyIdV1::new([0x41; 32]),
        witness.identity_commitment_v1(),
        PrivacyPolicyDigestV1::new([0x42; 32]),
        PRIVACY_ZK_ACE_POLICY_INITIAL_EPOCH_V1,
        asset_definition_id(),
        vec![ALICE_ID.clone()],
        PrivacyZkAcePolicyLifecycleV1::Active,
    )
    .expect("valid governed policy")
}

fn bounded_client(mut client: Client) -> Client {
    client.transaction_status_timeout = SUBMISSION_TIMEOUT;
    client.torii_request_timeout = Duration::from_secs(45);
    client
}

#[cfg(feature = "privacy-release-evidence")]
fn rejection_chain_contains(reason: &TransactionRejectionReason, expected: &str) -> bool {
    let expected = expected.to_ascii_lowercase();
    let mut current: &(dyn std::error::Error + 'static) = reason;
    loop {
        if current.to_string().to_ascii_lowercase().contains(&expected) {
            return true;
        }
        let Some(source) = current.source() else {
            return false;
        };
        current = source;
    }
}

#[cfg(feature = "privacy-release-evidence")]
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

#[cfg(feature = "privacy-release-evidence")]
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

#[cfg(feature = "privacy-release-evidence")]
fn assert_exact_zk_ace_capability(
    snapshot: &PrivacyExact12CapabilityManifestV1,
    compiled: PrivacyCompiledProfileSnapshotV1,
    activation: Option<PrivacyProtocolActivationRecordV1>,
    context: &str,
) -> Result<()> {
    snapshot
        .validate()
        .wrap_err_with(|| format!("{context}: invalid capability manifest"))?;
    let row = snapshot
        .protocols
        .iter()
        .copied()
        .find(|row| row.protocol_id == PROTOCOL)
        .ok_or_else(|| eyre!("{context}: ZK-ACE capability row missing"))?;
    ensure!(
        row.compiled_profile == PrivacyCompiledProfileResultV1::Available(compiled),
        "{context}: exact compiled ZK-ACE profile drifted: {:?}",
        row.compiled_profile
    );
    ensure!(
        row.activation == activation,
        "{context}: exact ZK-ACE activation drifted: expected {activation:?}, got {:?}",
        row.activation
    );
    Ok(())
}

#[cfg(feature = "privacy-release-evidence")]
fn wait_for_exact_zk_ace_capability_on_peers(
    network: &sandbox::SerializedNetwork,
    minimum_height: u64,
    compiled: PrivacyCompiledProfileSnapshotV1,
    activation: Option<PrivacyProtocolActivationRecordV1>,
    context: &str,
) -> Result<()> {
    let deadline = std::time::Instant::now() + PEER_CONVERGENCE_TIMEOUT;
    let mut last_observed = Vec::new();
    loop {
        let mut matching = 0_usize;
        last_observed.clear();
        for (index, peer) in network.peers().iter().enumerate() {
            let client = bounded_client(peer.client());
            match client.get_privacy_capabilities() {
                Ok(snapshot) if snapshot.committed_height >= minimum_height => {
                    match assert_exact_zk_ace_capability(&snapshot, compiled, activation, context) {
                        Ok(()) => {
                            matching += 1;
                            last_observed.push(format!(
                                "peer {index}: exact row at height {}",
                                snapshot.committed_height
                            ));
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
        if matching == network.peers().len() {
            return Ok(());
        }
        if std::time::Instant::now() >= deadline {
            return Err(eyre!(
                "{context}: peers did not converge within {PEER_CONVERGENCE_TIMEOUT:?}; {}",
                last_observed.join("; ")
            ));
        }
        std::thread::sleep(POLL_INTERVAL);
    }
}

#[cfg(feature = "privacy-release-evidence")]
fn advance_to_exact_height(client: &Client, target_height: u64) -> Result<()> {
    let start = client
        .get_privacy_capabilities()
        .wrap_err("query height before exact ZK-ACE activation advance")?
        .committed_height;
    ensure!(
        start <= target_height,
        "cannot advance backwards from height {start} to {target_height}"
    );
    for incoming_height in start.saturating_add(1)..=target_height {
        client
            .submit_all_blocking(
                vec![Log::new(
                    Level::INFO,
                    format!("exact ZK-ACE activation advance {incoming_height}"),
                )],
                no_fee(),
            )
            .wrap_err_with(|| format!("commit exact activation block {incoming_height}"))?;
        let observed = client
            .get_privacy_capabilities()
            .wrap_err_with(|| format!("query exact activation block {incoming_height}"))?
            .committed_height;
        ensure!(
            observed == incoming_height,
            "activation advance landed at height {observed}, expected {incoming_height}"
        );
    }
    Ok(())
}

#[cfg(feature = "privacy-release-evidence")]
fn exact_transaction_result(
    client: &Client,
    transaction: &SignedTransaction,
) -> Result<Option<bool>> {
    let expected_hash = transaction.hash_as_entrypoint();
    let expected_entrypoint = TransactionEntrypoint::External(transaction.clone());
    let transactions = client
        .query(FindTransactions::new())
        .execute_all()
        .wrap_err("query finalized ZK-ACE transactions")?;
    let Some(committed) = transactions
        .iter()
        .find(|committed| committed.entrypoint_hash() == &expected_hash)
    else {
        return Ok(None);
    };
    ensure!(
        committed.entrypoint() == &expected_entrypoint,
        "entrypoint hash matched different ZK-ACE transaction bytes"
    );
    Ok(Some(committed.result().0.is_ok()))
}

#[cfg(feature = "privacy-release-evidence")]
fn exact_transaction_anchor(
    client: &Client,
    transaction: &SignedTransaction,
    expected_success: bool,
    context: &str,
) -> Result<PrivacyZkAceCanonicalTransactionAnchorV1> {
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
    ensure!(
        committed.result().0.is_ok() == expected_success,
        "{context}: terminal result success={} instead of {expected_success}",
        committed.result().0.is_ok()
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
                "{context}: carrier block {} is absent",
                committed.block_hash()
            )
        })?;
    ensure!(
        committed.verify_inclusion_in_block(block),
        "{context}: transaction inclusion proof does not match its carrier block"
    );
    let canonical_transaction = norito::to_bytes(transaction)
        .wrap_err_with(|| format!("{context}: encode canonical signed transaction"))?;
    Ok(PrivacyZkAceCanonicalTransactionAnchorV1 {
        signed_transaction_hash: *transaction.hash().as_ref(),
        entrypoint_hash: *expected_hash.as_ref(),
        canonical_transaction_sha256: Sha256::digest(canonical_transaction).into(),
        carrier_height: block.header().height().get(),
        carrier_block_hash: *block.header().hash().as_ref(),
    })
}

#[cfg(feature = "privacy-release-evidence")]
fn privacy_submission_digests(
    transaction: &SignedTransaction,
    context: &str,
) -> Result<(
    [u8; 32],
    [u8; 32],
    iroha::data_model::privacy::PrivacyStatementDigestV1,
)> {
    let submissions = transaction
        .instructions()
        .explicit_instructions()
        .filter_map(|instruction| instruction.as_any().downcast_ref::<SubmitPrivacyProofV1>())
        .collect::<Vec<_>>();
    ensure!(
        submissions.len() == 1,
        "{context}: expected exactly one direct SubmitPrivacyProofV1"
    );
    let envelope = &submissions[0].envelope;
    let statement = norito::to_bytes(&envelope.statement)
        .wrap_err_with(|| format!("{context}: encode canonical typed statement"))?;
    Ok((
        Sha256::digest(envelope.proof.bytes().as_bytes()).into(),
        Sha256::digest(statement).into(),
        envelope.statement_digest,
    ))
}

#[cfg(feature = "privacy-release-evidence")]
fn require_exact_committed_rejection_reason(
    client: &Client,
    transaction: &SignedTransaction,
    context: &str,
) -> Result<TransactionRejectionReason> {
    let expected_hash = transaction.hash_as_entrypoint();
    let expected_entrypoint = TransactionEntrypoint::External(transaction.clone());
    let transactions = client
        .query(FindTransactions::new())
        .execute_all()
        .wrap_err_with(|| format!("{context}: query finalized rejected transaction"))?;
    let committed = transactions
        .iter()
        .find(|committed| committed.entrypoint_hash() == &expected_hash)
        .ok_or_else(|| eyre!("{context}: exact rejected transaction is absent"))?;
    ensure!(
        committed.entrypoint() == &expected_entrypoint,
        "{context}: entrypoint hash matched different rejected transaction bytes"
    );
    match committed.result() {
        TransactionResult(
            Err(
                rejection @ TransactionRejectionReason::Validation(
                    ValidationFail::InstructionFailed(InstructionExecutionError::InvalidParameter(
                        InvalidParameterError::SmartContract(message),
                    )),
                ),
            ),
            batch_outcomes,
        ) => {
            ensure!(
                message.as_str() == CONSUMED_REPLAY_NULLIFIER_MESSAGE,
                "{context}: committed replay rejection message drifted: {message:?}"
            );
            ensure!(
                batch_outcomes.is_empty(),
                "{context}: committed replay rejection attached unexpected batch outcomes"
            );
            Ok(rejection.clone())
        }
        actual => Err(eyre!(
            "{context}: committed replay rejection has the wrong typed terminal result: {actual:?}"
        )),
    }
}

#[cfg(feature = "privacy-release-evidence")]
fn wait_for_transaction_result_on_peers(
    clients: &[Client],
    transaction: &SignedTransaction,
    expected_success: bool,
    context: &str,
) -> Result<()> {
    let deadline = std::time::Instant::now() + PEER_CONVERGENCE_TIMEOUT;
    let mut last_observed = Vec::new();
    loop {
        let mut matching = 0_usize;
        last_observed.clear();
        for (index, client) in clients.iter().enumerate() {
            match exact_transaction_result(client, transaction) {
                Ok(Some(success)) if success == expected_success => {
                    matching += 1;
                    last_observed.push(format!("peer {index}: expected terminal result"));
                }
                Ok(Some(success)) => last_observed.push(format!(
                    "peer {index}: success={success}, expected {expected_success}"
                )),
                Ok(None) => last_observed.push(format!("peer {index}: transaction absent")),
                Err(error) => last_observed.push(format!("peer {index}: {error}")),
            }
        }
        if matching == clients.len() {
            return Ok(());
        }
        if std::time::Instant::now() >= deadline {
            return Err(eyre!(
                "{context}: terminal result did not converge within \
                 {PEER_CONVERGENCE_TIMEOUT:?}; {}",
                last_observed.join("; ")
            ));
        }
        std::thread::sleep(POLL_INTERVAL);
    }
}

#[cfg(feature = "privacy-release-evidence")]
fn asset_quantities(client: &Client, asset_ids: &[AssetId]) -> Result<Vec<Option<Quantity>>> {
    let assets = client
        .query(FindAssets::new())
        .execute_all()
        .wrap_err("query exact ZK-ACE asset snapshot")?;
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

#[cfg(feature = "privacy-release-evidence")]
fn wait_for_asset_quantities(
    clients: &[Client],
    asset_ids: &[AssetId],
    expected: &[Option<Quantity>],
    context: &str,
) -> Result<()> {
    let deadline = std::time::Instant::now() + PEER_CONVERGENCE_TIMEOUT;
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
        if std::time::Instant::now() >= deadline {
            return Err(eyre!(
                "{context}: asset state did not converge within \
                 {PEER_CONVERGENCE_TIMEOUT:?}; {}",
                last_observed.join("; ")
            ));
        }
        std::thread::sleep(POLL_INTERVAL);
    }
}

#[cfg(feature = "privacy-release-evidence")]
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
        .wrap_err_with(|| format!("{context}: validate direct privacy binding"))?
        .ok_or_else(|| eyre!("{context}: direct privacy submission was not detected"))?;
    Ok(())
}

#[cfg(feature = "privacy-release-evidence")]
fn validate_finalized_replay_provenance(
    client: &Client,
    action: &PrivacyReleaseZkAceNetworkActionV1,
    expected_admitted_at_height: u64,
    context: &str,
) -> Result<PrivacyZkAceReplayNullifierProvenanceV1> {
    let provenance = client
        .query_single(action.replay_nullifier_query.clone())
        .wrap_err_with(|| format!("{context}: query finalized replay provenance"))?;
    action
        .validate_finalized_replay_provenance_v1(&provenance)
        .map_err(|error| eyre!("{context}: replay provenance mismatch: {error:?}"))?;
    ensure!(
        provenance.admitted_at_height == expected_admitted_at_height,
        "{context}: replay provenance admitted at height {}, expected transaction carrier height {expected_admitted_at_height}",
        provenance.admitted_at_height
    );
    let blocks = client
        .query(FindBlocks)
        .execute_all()
        .wrap_err_with(|| format!("{context}: query finalized replay anchor"))?;
    ensure!(
        blocks.iter().any(|block| {
            block.header().height().get() == provenance.finalized_height
                && block.header().hash() == provenance.finalized_block_hash
        }),
        "{context}: finalized replay anchor ({}, {}) is not a canonical block",
        provenance.finalized_height,
        provenance.finalized_block_hash
    );
    Ok(provenance)
}

#[cfg(feature = "privacy-release-evidence")]
fn execute_zk_ace_network_semantic_flow(
    network: &sandbox::SerializedNetwork,
    client: &Client,
    compiled: CompiledPrivacyProfileV1,
    corridor: PrivacyZkAceNetworkSemanticCorridorV1,
    receipt_output: Option<&Path>,
) -> Result<()> {
    ensure!(
        network.peers().len() == 4,
        "ZK-ACE semantic gate requires exactly four peers"
    );
    ensure!(
        PRIVACY_MIN_ACTIVATION_DELAY_BLOCKS_V1 == 300,
        "ZK-ACE release gate requires the exact 300-block activation notice, got {}",
        PRIVACY_MIN_ACTIVATION_DELAY_BLOCKS_V1
    );
    let compiled_snapshot = PrivacyCompiledProfileSnapshotV1::from(compiled);
    let all_clients = network
        .peers()
        .iter()
        .map(|peer| bounded_client(peer.client()))
        .collect::<Vec<_>>();

    client
        .submit_all_blocking(
            vec![Grant::account_permission(
                Permission::from(CanEnactGovernance),
                client.account.clone(),
            )],
            no_fee(),
        )
        .wrap_err("grant CanEnactGovernance for ZK-ACE release flow")?;
    let registration_height = client
        .get_privacy_capabilities()
        .wrap_err("query height before ZK-ACE activation registration")?
        .committed_height
        .checked_add(1)
        .ok_or_else(|| eyre!("ZK-ACE registration height overflowed"))?;
    let activation_height = registration_height
        .checked_add(PRIVACY_MIN_ACTIVATION_DELAY_BLOCKS_V1)
        .ok_or_else(|| eyre!("ZK-ACE activation height overflowed"))?;
    ensure!(
        activation_height - registration_height == 300,
        "ZK-ACE activation notice is not exactly 300 blocks"
    );
    let proposed = proposed_activation(compiled, registration_height, activation_height);
    let registration_transaction = client.build_transaction(
        vec![InstructionBox::from(
            RegisterPrivacyProtocolActivationV1::new(proposed),
        )],
        no_fee(),
        Metadata::default(),
    );
    client
        .submit_transaction_blocking(&registration_transaction)
        .wrap_err("register exact proposed ZK-ACE activation")?;
    wait_for_exact_zk_ace_capability_on_peers(
        network,
        registration_height,
        compiled_snapshot,
        Some(proposed),
        "proposed ZK-ACE lifecycle",
    )?;
    let registration_anchor = exact_transaction_anchor(
        client,
        &registration_transaction,
        true,
        "ZK-ACE activation registration",
    )?;
    ensure!(
        registration_anchor.carrier_height == registration_height,
        "ZK-ACE activation registration committed at {}, expected {registration_height}",
        registration_anchor.carrier_height
    );

    advance_to_exact_height(client, activation_height)?;
    let active = active_activation(compiled, registration_height, activation_height);
    wait_for_exact_zk_ace_capability_on_peers(
        network,
        activation_height,
        compiled_snapshot,
        Some(active),
        "active ZK-ACE lifecycle",
    )?;

    let creation_time = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .wrap_err("system clock before Unix epoch")?;
    let genesis_hash = canonical_genesis_hash(client)?;
    let transaction_context = PrivacyReleaseTransactionContextV1 {
        network_id: client.network_id,
        authority: client.account.clone(),
        creation_time,
        time_to_live: Some(ACTION_TTL),
        nonce: NonZeroU32::new(101),
        fee_payment: no_fee(),
        metadata: Metadata::default(),
        genesis_hash,
    };
    let canonical = build_privacy_release_zk_ace_network_action_v1(
        transaction_context.clone(),
        ALICE_ID.clone(),
        BOB_ID.clone(),
        asset_definition_id(),
        19,
        [0x51; 32],
        [0x52; 32],
        ALICE_KEYPAIR.private_key(),
    )
    .map_err(|error| eyre!("build canonical native ZK-ACE action: {error:?}"))?;
    let replay = build_privacy_release_zk_ace_network_action_v1(
        transaction_context,
        ALICE_ID.clone(),
        BOB_ID.clone(),
        asset_definition_id(),
        19,
        [0x51; 32],
        [0x53; 32],
        ALICE_KEYPAIR.private_key(),
    )
    .map_err(|error| eyre!("build fresh native ZK-ACE replay carrier: {error:?}"))?;
    ensure!(
        canonical.policy == replay.policy
            && canonical.statement == replay.statement
            && canonical.replay_nullifier_query == replay.replay_nullifier_query,
        "fresh replay carrier changed the governed policy, statement, or replay nullifier"
    );
    ensure!(
        canonical.transaction.hash_as_entrypoint() != replay.transaction.hash_as_entrypoint(),
        "independent proof randomness did not produce a fresh signed replay carrier"
    );
    ensure!(
        canonical.statement.source == *ALICE_ID
            && canonical.statement.destination == *BOB_ID
            && canonical.statement.asset_definition_id == asset_definition_id()
            && canonical.statement.public_balance_scope == AssetBalanceScope::Global
            && canonical.statement.amount == 19,
        "generated ZK-ACE statement does not describe the exact expected public transfer"
    );
    for (label, action) in [("canonical", &canonical), ("replay", &replay)] {
        action
            .transaction
            .verify_signature()
            .wrap_err_with(|| format!("verify {label} ZK-ACE transaction signature"))?;
        assert_exactly_one_direct_privacy_submission(
            &action.transaction,
            &format!("{label} ZK-ACE action"),
        )?;
    }

    client
        .submit_all_blocking(
            vec![RegisterPrivacyZkAcePolicyV1::new(canonical.policy.clone())],
            no_fee(),
        )
        .wrap_err("register generated authoritative ZK-ACE policy")?;
    let asset_ids = [
        AssetId::new(asset_definition_id(), ALICE_ID.clone()),
        AssetId::new(asset_definition_id(), BOB_ID.clone()),
    ];
    wait_for_asset_quantities(
        &all_clients,
        &asset_ids,
        &[Some(Quantity::from(100_u32)), None],
        "pre-transfer ZK-ACE balances",
    )?;
    ensure!(
        exact_transaction_result(client, &canonical.transaction)?.is_none(),
        "canonical ZK-ACE action appeared before submission"
    );
    let canonical_handle = submit_signed_privacy_action_and_wait_v1(
        client,
        PrivacyOperationSchemaV1::ZkAceAuthorizationActionV1,
        &canonical.transaction,
        SUBMISSION_TIMEOUT,
        POLL_INTERVAL,
    )
    .wrap_err("submit canonical signed native ZK-ACE transfer through authenticated controller")?;
    let canonical_view = canonical_handle.view();
    ensure!(
        canonical_view.local_state() == PrivacyActionLocalStateV1::Terminal
            && canonical_view.terminal_chain_state()
                == Some(PrivacyActionTerminalChainStateV1::Applied)
            && canonical_view.ledger_effect_kind()
                == PrivacyLedgerEffectKindV1::ZkAceTransparentTransfer
            && canonical_view.rejection_reason().is_none()
            && canonical_handle.typed_rejection_reason().is_none(),
        "canonical ZK-ACE controller projection is not an authenticated applied transfer: {canonical_view:?}"
    );
    ensure!(
        canonical_view
            .execution_capability_manifest_digest()
            .is_some()
            && canonical_view
                .execution_capability_committed_height()
                .is_some()
            && canonical_view
                .execution_receipt_finalized_height()
                .is_some()
            && canonical_view
                .execution_receipt_finalized_block_hash()
                .is_some(),
        "canonical ZK-ACE applied projection omitted execution-time capability or finalized receipt evidence"
    );
    wait_for_transaction_result_on_peers(
        &all_clients,
        &canonical.transaction,
        true,
        "canonical ZK-ACE committed/applied state",
    )?;
    let canonical_anchor = exact_transaction_anchor(
        client,
        &canonical.transaction,
        true,
        "canonical ZK-ACE carrier",
    )?;
    let admitted_at_height = canonical_anchor.carrier_height;
    ensure!(
        canonical_view.committed_height() == Some(admitted_at_height),
        "canonical ZK-ACE controller height {:?} differs from exact carrier height {admitted_at_height}",
        canonical_view.committed_height()
    );
    wait_for_asset_quantities(
        &all_clients,
        &asset_ids,
        &[Some(Quantity::from(81_u32)), Some(Quantity::from(19_u32))],
        "canonical ZK-ACE exact public balance effect",
    )?;
    for (peer_index, peer) in all_clients.iter().enumerate() {
        require_privacy_action_receipt_on_peer_v1(peer, &canonical_handle).wrap_err_with(|| {
            format!("peer {peer_index} finalized canonical ZK-ACE execution receipt")
        })?;
        validate_finalized_replay_provenance(
            peer,
            &canonical,
            admitted_at_height,
            &format!("peer {peer_index} canonical replay marker"),
        )?;
    }

    let replay_handle = submit_signed_privacy_action_and_wait_v1(
        client,
        PrivacyOperationSchemaV1::ZkAceAuthorizationActionV1,
        &replay.transaction,
        SUBMISSION_TIMEOUT,
        POLL_INTERVAL,
    )
    .wrap_err(
        "submit freshly signed consumed-nullifier carrier through authenticated controller",
    )?;
    let replay_view = replay_handle.view();
    ensure!(
        replay_view.local_state() == PrivacyActionLocalStateV1::Terminal
            && replay_view.terminal_chain_state()
                == Some(PrivacyActionTerminalChainStateV1::Rejected)
            && replay_view.ledger_effect_kind()
                == PrivacyLedgerEffectKindV1::ZkAceTransparentTransfer
            && replay_view
                .rejection_reason()
                .is_some_and(|reason| reason.contains(CONSUMED_REPLAY_NULLIFIER_MESSAGE))
            && replay_view.execution_capability_manifest_digest().is_none()
            && replay_view
                .execution_capability_committed_height()
                .is_none()
            && replay_view.execution_receipt_finalized_height().is_none()
            && replay_view
                .execution_receipt_finalized_block_hash()
                .is_none(),
        "fresh ZK-ACE replay controller projection is not the exact authenticated rejection: {replay_view:?}"
    );
    let typed_replay_rejection = replay_handle
        .typed_rejection_reason()
        .ok_or_else(|| eyre!("rejected ZK-ACE replay omitted its typed committed reason"))?;
    ensure!(
        rejection_chain_contains(typed_replay_rejection, CONSUMED_REPLAY_NULLIFIER_MESSAGE),
        "fresh ZK-ACE replay carrier rejected for the wrong typed reason: {typed_replay_rejection:?}"
    );
    wait_for_transaction_result_on_peers(
        &all_clients,
        &replay.transaction,
        false,
        "consumed ZK-ACE replay rejection",
    )?;
    let mut canonical_rejection = None;
    let mut peer_provenance = Vec::with_capacity(all_clients.len());
    for (peer_index, peer) in all_clients.iter().enumerate() {
        let rejection = require_exact_committed_rejection_reason(
            peer,
            &replay.transaction,
            &format!("peer {peer_index} consumed-nullifier terminal state"),
        )?;
        if let Some(expected) = &canonical_rejection {
            ensure!(
                &rejection == expected,
                "peer {peer_index}: committed replay rejection differs from the canonical peer"
            );
        } else {
            canonical_rejection = Some(rejection);
        }
        let provenance = validate_finalized_replay_provenance(
            peer,
            &canonical,
            admitted_at_height,
            &format!("peer {peer_index} persisted replay marker after rejection"),
        )?;
        ensure!(
            provenance.policy_id == canonical.statement.policy_id
                && provenance.replay_nullifier == canonical.statement.replay_nullifier,
            "peer {peer_index}: rejected replay changed the consumed marker identity"
        );
        peer_provenance.push((network.peers()[peer_index].id(), provenance));
    }
    wait_for_asset_quantities(
        &all_clients,
        &asset_ids,
        &[Some(Quantity::from(81_u32)), Some(Quantity::from(19_u32))],
        "rejected ZK-ACE replay must preserve exact balances",
    )?;
    let replay_anchor = exact_transaction_anchor(
        client,
        &replay.transaction,
        false,
        "rejected replay ZK-ACE carrier",
    )?;
    let (transfer_proof_sha256, transfer_statement_sha256, transfer_statement_digest) =
        privacy_submission_digests(&canonical.transaction, "canonical ZK-ACE transfer")?;
    let (replay_proof_sha256, replay_statement_sha256, replay_statement_digest) =
        privacy_submission_digests(
            &replay.transaction,
            "independently randomized ZK-ACE replay",
        )?;
    ensure!(
        transfer_statement_sha256 == replay_statement_sha256
            && transfer_statement_digest == replay_statement_digest,
        "independently randomized ZK-ACE replay changed its canonical typed statement"
    );
    peer_provenance.sort_by(|left, right| left.0.cmp(&right.0));
    let replay_nullifier_finality: [PrivacyZkAceValidatorReplayObservationV1; 4] = peer_provenance
        .into_iter()
        .map(
            |(validator, provenance)| PrivacyZkAceValidatorReplayObservationV1 {
                validator,
                provenance,
            },
        )
        .collect::<Vec<_>>()
        .try_into()
        .map_err(|_| eyre!("ZK-ACE receipt requires exactly four validator observations"))?;
    let canonical_rejection = canonical_rejection
        .ok_or_else(|| eyre!("ZK-ACE replay rejection was not observed on any validator"))?;
    let canonical_rejection_bytes = norito::to_bytes(&canonical_rejection)
        .wrap_err("encode exact typed ZK-ACE replay rejection")?;
    let receipt = PrivacyZkAceNetworkSemanticReceiptV1 {
        version:
            iroha_core::privacy_release_evidence::PRIVACY_ZK_ACE_NETWORK_SEMANTIC_RECEIPT_VERSION_V1,
        corridor,
        candidate_profile_digest: zk_ace_compiled_profile_digest_v1(),
        release_stage_evidence_sha256: ZK_ACE_RELEASE_STAGE_EVIDENCE_SHA256_V2,
        network_id: client.network_id,
        genesis_block_hash: genesis_hash,
        activation: PrivacyZkAceActivationReceiptV1 {
            registration: registration_anchor,
            registration_height,
            activation_notice_blocks: PRIVACY_MIN_ACTIVATION_DELAY_BLOCKS_V1,
            activation_height,
        },
        transfer: PrivacyZkAceAppliedTransferReceiptV1 {
            transaction: canonical_anchor,
            proof_sha256: transfer_proof_sha256,
            canonical_statement_sha256: transfer_statement_sha256,
            statement_digest: transfer_statement_digest,
            policy_id: canonical.statement.policy_id,
            replay_nullifier: canonical.statement.replay_nullifier,
            source: canonical.statement.source.clone(),
            destination: canonical.statement.destination.clone(),
            asset_definition_id: canonical.statement.asset_definition_id.clone(),
            amount: canonical.statement.amount,
            source_balance_before: 100,
            destination_balance_before: 0,
            source_balance_after: 81,
            destination_balance_after: 19,
        },
        replay_nullifier_finality,
        replay: PrivacyZkAceRejectedReplayReceiptV1 {
            transaction: replay_anchor,
            proof_sha256: replay_proof_sha256,
            rejection_kind: PrivacyZkAceReplayRejectionKindV1::ConsumedReplayNullifier,
            canonical_typed_rejection_sha256: Sha256::digest(canonical_rejection_bytes).into(),
            source_balance_after_replay: 81,
            destination_balance_after_replay: 19,
        },
    };
    receipt
        .validate()
        .map_err(|error| eyre!("validate canonical ZK-ACE network-semantic receipt: {error}"))?;
    let receipt_bytes = receipt
        .canonical_norito_bytes()
        .map_err(|error| eyre!("encode canonical ZK-ACE network-semantic receipt: {error}"))?;
    let receipt_sha256 = receipt
        .canonical_norito_sha256()
        .map_err(|error| eyre!("digest canonical ZK-ACE network-semantic receipt: {error}"))?;
    ensure!(
        receipt_sha256 == Sha256::digest(&receipt_bytes).into(),
        "ZK-ACE receipt method disagrees with direct SHA-256 over canonical Norito"
    );
    if let Some(output) = receipt_output {
        write_new_receipt_atomic(output, &receipt_bytes)?;
    }
    Ok(())
}

fn assert_ordinary_wallet_builder_unavailable(client: &Client) -> Result<()> {
    let witness = witness(0x11);
    let transfer = ZkAcePrivacyTransferV1::try_new(
        policy(&witness),
        ALICE_ID.clone(),
        BOB_ID.clone(),
        AssetBalanceScope::Global,
        19,
    )
    .wrap_err("construct governed ZK-ACE transfer")?;
    let creation_time = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .wrap_err("system clock before Unix epoch")?;
    let genesis_hash = canonical_genesis_hash(client)?;
    let build_error = match build_signed_zk_ace_privacy_transfer_v1(
        ZkAcePrivacyActionTransactionContextV1 {
            network_id: client.network_id,
            authority: ALICE_ID.clone(),
            creation_time,
            time_to_live: Some(Duration::from_secs(3_600)),
            nonce: NonZeroU32::new(1),
            fee_payment: no_fee(),
            metadata: Metadata::default(),
        },
        transfer,
        witness,
        genesis_hash,
        ALICE_KEYPAIR.private_key(),
    ) {
        Ok(_) => {
            return Err(eyre!(
                "release-gated ordinary ZK-ACE wallet builder admitted a transfer"
            ));
        }
        Err(error) => error,
    };
    ensure!(
        matches!(
            &build_error,
            ZkAcePrivacyActionBuildErrorV1::CompiledProfileUnavailable
        ),
        "ordinary ZK-ACE wallet builder rejected for the wrong reason: {build_error:?}"
    );
    Ok(())
}

#[cfg(feature = "privacy-release-evidence")]
#[test]
fn zk_ace_receipt_output_is_explicit_canonical_and_no_clobber() -> Result<()> {
    let directory = tempfile::tempdir().wrap_err("create receipt-output test directory")?;
    let canonical_parent = directory
        .path()
        .canonicalize()
        .wrap_err("canonicalize receipt-output test directory")?;
    let output = canonical_parent.join("zk-ace-network-semantic.norito");
    let bytes = [0x51_u8; 32];

    write_new_receipt_atomic(&output, &bytes)?;
    ensure!(
        fs::read(&output).wrap_err("read first published receipt")? == bytes,
        "published receipt bytes drifted"
    );
    ensure!(
        write_new_receipt_atomic(&output, &[0x52; 32]).is_err(),
        "receipt output silently replaced an existing file"
    );
    ensure!(
        fs::read(&output).wrap_err("read no-clobber receipt")? == bytes,
        "failed no-clobber attempt changed the published receipt"
    );
    ensure!(
        validate_new_receipt_output_path(Path::new("relative-receipt.norito")).is_err(),
        "relative receipt output was accepted"
    );
    ensure!(
        validate_new_receipt_output_path(
            &canonical_parent
                .join("not-created")
                .join("..")
                .join("aliased-receipt.norito"),
        )
        .is_err(),
        "receipt output with a parent-directory component was accepted"
    );
    Ok(())
}

#[test]
fn zk_ace_privacy_transfer_fails_closed_taira_localnet() -> Result<()> {
    require_test_network_feature("zk-stark")?;
    #[cfg(feature = "privacy-release-evidence")]
    require_test_network_feature("privacy-release-evidence")?;
    init_instruction_registry();

    #[cfg(feature = "privacy-release-evidence")]
    let (release_profile, release_corridor) = selected_zk_ace_release_profile()?;
    #[cfg(feature = "privacy-release-evidence")]
    let receipt_output = requested_receipt_output_path()?;

    if ZK_ACE_FULL_ENGINE_AVAILABLE_V1 {
        ensure!(
            compiled_privacy_profile_v1(PROTOCOL).is_ok(),
            "complete ZK-ACE public-pin gate did not expose its compiled profile"
        );
    } else {
        ensure!(
            compiled_privacy_profile_v1(PROTOCOL)
                == Err(CompiledPrivacyProfileErrorV1::EngineUnavailable {
                    protocol_id: PROTOCOL,
                }),
            "open ZK-ACE public-pin gate did not fail closed"
        );
        ensure!(
            compiled_privacy_profile_snapshot_result_v1(PROTOCOL)
                == PrivacyCompiledProfileResultV1::Unavailable(
                    PrivacyCompiledProfileUnavailableReasonV1::EngineUnavailable,
                ),
            "local ZK-ACE capability result is not the exact fail-closed status"
        );
    }

    let builder = NetworkBuilder::new()
        .with_peers(4)
        .with_auto_populated_trusted_peers()
        .with_block_cadence(Duration::from_millis(100))
        .with_permissioned_consensus()
        .with_config_layer(|layer| {
            layer.write(["zk", "stark", "enabled"], true);
        });
    #[cfg(feature = "privacy-release-evidence")]
    let builder = builder
        .with_genesis_instruction(Register::asset_definition(AssetDefinition::numeric(
            asset_definition_id(),
            "zkace_typed".to_owned(),
            AssetBalancePolicy::Global,
            None,
        )))
        .with_genesis_instruction(Mint::asset_quantity(
            100_u32,
            AssetId::new(asset_definition_id(), ALICE_ID.clone()),
        ));
    let (network, _runtime) = sandbox::start_network_blocking_or_skip(builder, TEST_NAME)?
        .ok_or_else(|| {
            eyre!(
                "{TEST_NAME}: the committed ZK-ACE semantic gate cannot pass by skipping its four-peer network"
            )
        })?;
    let mut client = bounded_client(network.client());
    client.add_transaction_nonce = true;

    #[cfg(feature = "privacy-release-evidence")]
    let expected_compiled = PrivacyCompiledProfileResultV1::Available(
        PrivacyCompiledProfileSnapshotV1::from(release_profile),
    );
    #[cfg(not(feature = "privacy-release-evidence"))]
    let expected_compiled = compiled_privacy_profile_snapshot_result_v1(PROTOCOL);
    let row = client
        .get_privacy_capabilities()
        .wrap_err("query release-gated ZK-ACE capability")?
        .protocols
        .into_iter()
        .find(|row| row.protocol_id == PROTOCOL)
        .ok_or_else(|| eyre!("ZK-ACE capability row missing"))?;
    ensure!(
        row.compiled_profile == expected_compiled,
        "network ZK-ACE compiled profile differs from the exact local release gate: network={:?}, local={expected_compiled:?}",
        row.compiled_profile
    );
    ensure!(
        row.activation.is_none(),
        "fresh ZK-ACE localnet unexpectedly has an activation: {:?}",
        row.activation
    );

    if !ZK_ACE_FULL_ENGINE_AVAILABLE_V1 {
        assert_ordinary_wallet_builder_unavailable(&client)?;
    }

    #[cfg(feature = "privacy-release-evidence")]
    return execute_zk_ace_network_semantic_flow(
        &network,
        &client,
        release_profile,
        release_corridor,
        receipt_output.as_deref(),
    );

    #[cfg(not(feature = "privacy-release-evidence"))]
    Ok(())
}
