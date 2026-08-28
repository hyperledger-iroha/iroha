#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! Canonical Sumeragi v2 data-availability integration coverage.
//!
//! The first-release v2 runtime has no global RBC status endpoint or persisted
//! global RBC session store. Availability is committed by the signed genesis
//! context and observed through the authoritative v2 status and committed
//! subject.
use eyre::{Result, WrapErr, ensure, eyre};
use futures_util::future::try_join_all;
use integration_tests::sandbox;
use iroha::{
    client::{Client, TransactionWaitOptions},
    crypto::{Algorithm, Hash, HashOf, KeyPair},
    data_model::{
        Level, NetworkId,
        account::{Account, AccountId},
        asset::{AssetBalancePolicy, AssetDefinition, AssetDefinitionId, AssetId},
        block::consensus_v2::{
            BlockSubject, ConsensusRound, ExecutionCommitment, GlobalPhase, HeightContextId,
            PayloadManifest, SumeragiV2Status, encode_payload_chunks,
        },
        block::{SignedBlock, decode_framed_signed_block},
        bridge::{BridgeFinalityProof, verify_bridge_finality_proof},
        domain::{Domain, DomainId},
        isi::{
            Log, Mint, Register, SetParameter,
            staking::{ActivatePublicLaneValidator, RegisterPublicLaneValidator},
        },
        metadata::Metadata,
        nexus::LaneId,
        parameter::{CustomParameter, CustomParameterId, Parameter, TransactionParameter},
        peer::PeerId,
        transaction::{Executable, SignedTransaction},
    },
};
use iroha_config::parameters::actual::LaneConfig;
use iroha_primitives::{json::Json, numeric::Quantity};
use iroha_test_network::{
    ConsensusMessageControlAck, ConsensusMessageControlAction, ConsensusMessageControlKind,
    ConsensusMessageControlRule, Network, NetworkBuilder, genesis_factory_with_post_topology,
    init_instruction_registry,
};
use iroha_test_samples::ALICE_ID;
use norito::codec::DecodeAll as _;
use std::{
    io::{Read as _, Seek as _, SeekFrom},
    mem::size_of,
    num::NonZeroU64,
    path::{Path, PathBuf},
    sync::Arc,
    time::{Duration, Instant},
};
const LARGE_PAYLOAD_BYTES: usize = 1024 * 1024;
const PACKET_LOSS_PAYLOAD_BYTES: usize = 10 * 1024 * 1024;
const PACKET_LOSS_ADMISSION_HEIGHT: u64 = 2;
const PACKET_LOSS_HEIGHT: u64 = 3;
const PACKET_LOSS_ADMISSION_VIEW: u64 = 0;
const PACKET_LOSS_CARRIER_VIEWS: [u64; 4] = [0, 1, 2, 3];
const PACKET_LOSS_CAPTURE_VIEWS: [u64; 6] = [0, 1, 2, 3, 4, 5];
const PACKET_LOSS_CHUNK_INDICES: [u32; 3] = [57, 58, 59];
const PACKET_LOSS_QUEUE_CAPACITY: usize = 16;
const PACKET_LOSS_CAPTURE_QUEUE_CAPACITY: usize = 512;
const PACKET_LOSS_CONTROL_TIMEOUT: Duration = Duration::from_secs(360);
const PACKET_LOSS_BLOCK_CADENCE: Duration = Duration::from_secs(8);
const PACKET_LOSS_QUEUE_REPLICATION_TIMEOUT: Duration = Duration::from_secs(30);
const TORII_CONTENT_HEADROOM_BYTES: usize = 2 * 1024 * 1024;
const BLOCK_GAS_HEADROOM: u64 = 2 * 1024 * 1024;
const TORII_MAX_CONTENT_LEN_BYTES: i64 = 64_000_000;
const NETWORK_FRAME_BUDGET_BYTES: i64 = 128 * 1024 * 1024;
const NETWORK_TOPIC_FRAME_BUDGET_BYTES: i64 = NETWORK_FRAME_BUDGET_BYTES - 28;
const NETWORK_STREAM_FRAME_BUDGET_BYTES: i64 = NETWORK_FRAME_BUDGET_BYTES + 4;
const NETWORK_DEFERRED_SEND_BUDGET_BYTES: i64 = 2 * NETWORK_STREAM_FRAME_BUDGET_BYTES;
const TEST_NEXUS_LOCAL_STORAGE_BUDGET_BYTES: i64 = 1024 * 1024 * 1024;
const COMMIT_WAIT_BUDGET: Duration = Duration::from_secs(480);
const ROUTE_BINDING_POLL: Duration = Duration::from_secs(1);
const DA_VALIDATOR_STAKE: u64 = 1;
const V2_BODY_STORE_MAGIC: &[u8; 8] = b"SUM2BODY";
const V2_BODY_VALIDATION_MAGIC: &[u8; 8] = b"SUM2VALD";
const V2_BODY_STORE_VERSION: u16 = 1;
const V2_BODY_VALIDATION_VERSION: u16 = 1;
const V2_BODY_FRAME_HEADER_BYTES: usize = 8 + size_of::<u16>() + size_of::<u64>();
const V2_BODY_FRAME_CHECKSUM_BYTES: usize = Hash::LENGTH;
const V2_BODY_FRAME_MAX_PAYLOAD_BYTES: usize = 64 * 1024 * 1024;

/// Test-side mirror of the private, versioned durable body-store envelope.
///
/// The integration corridor reads this only to authenticate that the exact
/// held manifest crossed the production body-store boundary after healing.
#[derive(Debug, norito::Decode, norito::Encode)]
struct StoredDaBodyEnvelope {
    version: u16,
    context_id: HeightContextId,
    round: ConsensusRound,
    subject: BlockSubject,
    manifest: PayloadManifest,
    canonical_wire: Vec<u8>,
}

/// Test-side mirror of the closed durable body-validation outcome.
#[derive(Debug, norito::Decode, norito::Encode)]
enum StoredDaBodyValidationOutcome {
    /// Deterministic validation succeeded with this exact commitment.
    Validated(ExecutionCommitment),
    /// Deterministic validation rejected with its closed reason code.
    Rejected(u8),
}

/// Test-side mirror of the private, versioned validation marker.
#[derive(Debug, norito::Decode, norito::Encode)]
struct StoredDaBodyValidationMarker {
    version: u16,
    context_id: HeightContextId,
    round: ConsensusRound,
    subject: BlockSubject,
    manifest_hash: HashOf<PayloadManifest>,
    body_frame_hash: Hash,
    outcome: StoredDaBodyValidationOutcome,
}

/// Exact durable evidence recovered for the authenticated held manifest.
#[derive(Debug)]
struct HeldDaBodyEvidence {
    manifest: PayloadManifest,
    subject: BlockSubject,
    canonical_wire: Vec<u8>,
    execution_commitment: ExecutionCommitment,
}
fn torii_max_content_len_for_payload(payload_bytes: usize) -> i64 {
    let inflated = payload_bytes.saturating_mul(12);
    let with_headroom = payload_bytes.saturating_add(TORII_CONTENT_HEADROOM_BYTES);
    i64::try_from(inflated.max(with_headroom))
        .unwrap_or(i64::MAX)
        .min(TORII_MAX_CONTENT_LEN_BYTES)
}
fn tx_limit_for_payload(payload_bytes: usize) -> NonZeroU64 {
    NonZeroU64::new(
        u64::try_from(torii_max_content_len_for_payload(payload_bytes)).unwrap_or(u64::MAX),
    )
    .expect("payload-driven transaction limit must be non-zero")
}
fn block_gas_limit_for_payload(payload_bytes: usize) -> u64 {
    u64::try_from(payload_bytes)
        .unwrap_or(u64::MAX)
        .saturating_add(BLOCK_GAS_HEADROOM)
}
fn block_gas_parameter_for_payload(payload_bytes: usize) -> Parameter {
    Parameter::Custom(CustomParameter::new(
        CustomParameterId::new(
            "ivm_gas_limit_per_block"
                .parse()
                .expect("canonical block gas parameter name"),
        ),
        Json::new(block_gas_limit_for_payload(payload_bytes)),
    ))
}
fn da_stake_asset_definition_id() -> AssetDefinitionId {
    AssetDefinitionId::derive_from_components(
        DomainId::try_new("nexus", "universal").expect("DA stake domain"),
        "xor".parse().expect("DA stake asset name"),
    )
}
fn da_validator_account_id(index: usize) -> AccountId {
    let key_pair = KeyPair::try_from_seed(
        format!("integration_tests::sumeragi_da::route-validator::{index}").into_bytes(),
        Algorithm::Ed25519,
    )
    .expect("derive checked DA route-validator key");
    AccountId::new(key_pair.public_key().clone())
}
fn da_route_authority_genesis_transactions(
    topology: &[PeerId],
) -> Vec<Vec<iroha::data_model::isi::InstructionBox>> {
    let stake_asset_id = da_stake_asset_definition_id();
    let mut bootstrap = vec![
        Register::domain(Domain::new(
            DomainId::try_new("nexus", "universal").expect("DA stake domain"),
        ))
        .into(),
        Register::asset_definition(
            AssetDefinition::numeric(
                stake_asset_id.clone(),
                "DA route stake".to_owned(),
                AssetBalancePolicy::Global,
                None,
            )
            .with_metadata(Metadata::default()),
        )
        .into(),
    ];
    bootstrap.reserve(topology.len().saturating_mul(2));
    let mut validators = Vec::with_capacity(topology.len().saturating_mul(2));
    for (index, peer_id) in topology.iter().enumerate() {
        let validator_id = da_validator_account_id(index);
        bootstrap.push(Register::account(Account::new(validator_id.clone())).into());
        bootstrap.push(
            Mint::asset_quantity(
                DA_VALIDATOR_STAKE,
                AssetId::new(stake_asset_id.clone(), validator_id.clone()),
            )
            .into(),
        );
        validators.push(
            RegisterPublicLaneValidator::new(
                LaneId::SINGLE,
                validator_id.clone(),
                peer_id.clone(),
                validator_id.clone(),
                Quantity::from(DA_VALIDATOR_STAKE),
                Metadata::default(),
            )
            .into(),
        );
        validators.push(ActivatePublicLaneValidator::new(LaneId::SINGLE, validator_id).into());
    }
    vec![bootstrap, validators]
}
fn large_da_network_builder(peers: usize, payload_bytes: usize) -> NetworkBuilder {
    let tx_limit = tx_limit_for_payload(payload_bytes);
    NetworkBuilder::new()
        .with_peers(peers)
        .with_auto_populated_trusted_peers()
        .with_permissioned_consensus()
        .with_genesis_block(|topology, topology_entries| {
            let post_topology = da_route_authority_genesis_transactions(topology.as_ref());
            genesis_factory_with_post_topology(
                Vec::new(),
                post_topology,
                topology,
                topology_entries,
            )
        })
        .with_config_layer(|layer| {
            let gas_account = ALICE_ID.to_string();
            layer
                .write("telemetry_profile", "full")
                .write(
                    ["torii", "max_content_len"],
                    torii_max_content_len_for_payload(payload_bytes),
                )
                // The fixture exercises DA transport, not fee funding. A Log's
                // gas scales with its message length, so make the isolated
                // test network fee-free instead of coupling a 10 MiB carrier
                // to the default authority seed balance.
                .write(["nexus", "fees", "base_fee"], "0")
                .write(["nexus", "fees", "per_byte_fee"], "0")
                .write(["nexus", "fees", "per_instruction_fee"], "0")
                .write(["nexus", "fees", "per_gas_unit_fee"], "0")
                // QueuePlan admission is intentionally fail-closed without a
                // lane authority. Bind the permissioned global fixture to an
                // explicit public-lane stake pool instead of treating the
                // global consensus roster as implicit lane authority.
                .write(
                    ["nexus", "staking", "stake_asset_id"],
                    da_stake_asset_definition_id().to_string(),
                )
                .write(
                    ["nexus", "staking", "stake_escrow_account_id"],
                    gas_account.clone(),
                )
                .write(["nexus", "staking", "slash_sink_account_id"], gas_account)
                .write(["network", "max_frame_bytes"], NETWORK_FRAME_BUDGET_BYTES)
                .write(
                    ["network", "p2p_outbound_frame_queue_max_high_bytes"],
                    NETWORK_STREAM_FRAME_BUDGET_BYTES,
                )
                .write(
                    ["network", "p2p_outbound_frame_queue_max_low_bytes"],
                    NETWORK_STREAM_FRAME_BUDGET_BYTES,
                )
                .write(
                    ["network", "deferred_send_max_bytes_per_peer"],
                    NETWORK_DEFERRED_SEND_BUDGET_BYTES,
                )
                .write(
                    ["network", "deferred_send_max_bytes_total"],
                    NETWORK_DEFERRED_SEND_BUDGET_BYTES,
                )
                .write(
                    ["network", "max_frame_bytes_consensus"],
                    NETWORK_TOPIC_FRAME_BUDGET_BYTES,
                )
                .write(
                    ["network", "max_frame_bytes_control"],
                    NETWORK_TOPIC_FRAME_BUDGET_BYTES,
                )
                .write(
                    ["network", "max_frame_bytes_block_sync"],
                    NETWORK_TOPIC_FRAME_BUDGET_BYTES,
                )
                .write(
                    ["network", "max_frame_bytes_other"],
                    NETWORK_TOPIC_FRAME_BUDGET_BYTES,
                )
                .write(
                    ["network", "max_frame_bytes_tx_gossip"],
                    NETWORK_TOPIC_FRAME_BUDGET_BYTES,
                )
                .write(
                    ["nexus", "storage", "local_budget_bytes"],
                    TEST_NEXUS_LOCAL_STORAGE_BUDGET_BYTES,
                );
        })
        .with_genesis_instruction(SetParameter::new(Parameter::Transaction(
            TransactionParameter::MaxTxBytes(tx_limit),
        )))
        .with_genesis_instruction(SetParameter::new(Parameter::Transaction(
            TransactionParameter::MaxDecompressedBytes(tx_limit),
        )))
        // A native Log costs one gas per message byte plus fixed overhead.
        // Keep this isolated DA fixture's block budget above its exact carrier
        // size so execution success does not depend on the production default.
        .with_genesis_instruction(SetParameter::new(block_gas_parameter_for_payload(
            payload_bytes,
        )))
}
fn hold_bounded_view_payload_chunks(
    receiver_index: usize,
    peer_ids: &[PeerId],
) -> Vec<ConsensusMessageControlRule> {
    let mut rules = Vec::with_capacity(
        peer_ids
            .len()
            .saturating_sub(1)
            .saturating_mul(PACKET_LOSS_CARRIER_VIEWS.len())
            .saturating_mul(PACKET_LOSS_CHUNK_INDICES.len()),
    );
    for (sender_index, sender) in peer_ids.iter().enumerate() {
        if sender_index == receiver_index {
            continue;
        }
        for view in PACKET_LOSS_CARRIER_VIEWS {
            for index in PACKET_LOSS_CHUNK_INDICES {
                rules.push(ConsensusMessageControlRule::payload_chunk_from_proposal(
                    sender.clone(),
                    PACKET_LOSS_HEIGHT,
                    view,
                    index,
                    ConsensusMessageControlAction::Hold,
                ));
            }
        }
    }
    rules
}
fn hold_bounded_view_finality_traffic(
    receiver_index: usize,
    peer_ids: &[PeerId],
) -> Vec<ConsensusMessageControlRule> {
    let kinds = [
        ConsensusMessageControlKind::CommitVote,
        ConsensusMessageControlKind::CommitCertificate,
        ConsensusMessageControlKind::CommitCertificateResponse,
        ConsensusMessageControlKind::TimeoutVote,
        ConsensusMessageControlKind::TimeoutCertificate,
    ];
    let mut rules = Vec::with_capacity(
        peer_ids
            .len()
            .saturating_sub(1)
            .saturating_mul(PACKET_LOSS_CAPTURE_VIEWS.len())
            .saturating_mul(kinds.len()),
    );
    for (sender_index, sender) in peer_ids.iter().enumerate() {
        if sender_index == receiver_index {
            continue;
        }
        for view in PACKET_LOSS_CAPTURE_VIEWS {
            for kind in kinds {
                rules.push(ConsensusMessageControlRule::exact(
                    sender.clone(),
                    kind,
                    PACKET_LOSS_HEIGHT,
                    view,
                    ConsensusMessageControlAction::Hold,
                ));
            }
        }
    }
    rules
}
fn hold_exact_manifest_chunks_and_finality_traffic(
    receiver_index: usize,
    peer_ids: &[PeerId],
    manifest_hash: HashOf<PayloadManifest>,
    proposal_view: u64,
) -> Vec<ConsensusMessageControlRule> {
    let mut rules = Vec::new();
    for (sender_index, sender) in peer_ids.iter().enumerate() {
        if sender_index == receiver_index {
            continue;
        }
        for index in PACKET_LOSS_CHUNK_INDICES {
            rules.push(ConsensusMessageControlRule::payload_chunk(
                sender.clone(),
                manifest_hash,
                index,
                ConsensusMessageControlAction::Hold,
            ));
        }
        rules.push(ConsensusMessageControlRule::exact(
            sender.clone(),
            ConsensusMessageControlKind::CertifiedBodyResponse,
            PACKET_LOSS_HEIGHT,
            proposal_view,
            ConsensusMessageControlAction::Hold,
        ));
    }
    rules.extend(hold_bounded_view_finality_traffic(receiver_index, peer_ids));
    rules
}
type HeldPayloadChunkMatch = (u64, HashOf<PayloadManifest>, u32, u64);

fn proposal_bound_payload_chunk_matches(
    ack: &ConsensusMessageControlAck,
) -> Result<Vec<HeldPayloadChunkMatch>> {
    let mut matched = Vec::new();
    for held in &ack.held {
        if held.kind != ConsensusMessageControlKind::PayloadChunk
            || held.height.is_some()
            || held.view.is_some()
            || held.block_hash.is_some()
            || held.subject.is_some()
            || held.execution_commitment.is_some()
            || held.sender != held.authenticated_via
        {
            continue;
        }
        let (Some(manifest_hash), Some(index)) = (held.manifest_hash, held.chunk_index) else {
            continue;
        };
        let Some(exact_rule) = ack.rules.iter().find(|rule| {
            rule.kind == ConsensusMessageControlKind::PayloadChunk
                && rule.sender == held.sender
                && rule.authenticated_via == held.authenticated_via
                && rule.height == 0
                && rule.view == 0
                && rule.block_hash.is_none()
                && rule.chunk_index == Some(index)
                && rule.proposal_height == Some(PACKET_LOSS_HEIGHT)
                && rule
                    .proposal_view
                    .is_some_and(|view| PACKET_LOSS_CARRIER_VIEWS.contains(&view))
                && rule.manifest_hash == Some(manifest_hash)
                && rule.action == ConsensusMessageControlAction::Hold
        }) else {
            continue;
        };
        let Some(resolved_manifest_hash) = exact_rule.manifest_hash else {
            // A chunk may arrive before its Proposal. The Hold is already
            // effective, but it is not evidence for this exact experiment
            // until Proposal resolution binds the rule to the manifest.
            continue;
        };
        ensure!(
            resolved_manifest_hash == manifest_hash && PACKET_LOSS_CHUNK_INDICES.contains(&index),
            "retained occurrence disagreed with its exact manifest/index selector"
        );
        let proposal_view = exact_rule
            .proposal_view
            .expect("bounded carrier rule must select a proposal view");
        if !matched
            .iter()
            .any(|(sequence, _, _, _)| *sequence == held.sequence)
        {
            matched.push((held.sequence, manifest_hash, index, proposal_view));
        }
    }
    Ok(matched)
}
fn validate_committed_da_status(status: &SumeragiV2Status, expected_height: u64) -> Result<()> {
    status
        .validate()
        .map_err(|err| eyre!("invalid canonical Sumeragi v2 status: {err}"))?;
    ensure!(
        status.last_committed_height >= expected_height,
        "peer committed height {} is below expected DA height {expected_height}",
        status.last_committed_height
    );
    ensure!(
        status.last_committed_subject.is_some(),
        "peer omitted the committed v2 subject"
    );
    Ok(())
}
fn fetch_v2_status(client: Client) -> Result<SumeragiV2Status> {
    client
        .get_sumeragi_status()
        .wrap_err("fetch canonical Sumeragi v2 status")
}
async fn wait_for_bridge_finality_proof(
    client: Client,
    peer_name: &str,
    height: u64,
    timeout: Duration,
) -> Result<BridgeFinalityProof> {
    let url = client
        .torii_url
        .join(&format!("v1/bridge/finality/{height}"))
        .wrap_err("construct bridge-finality URL")?;
    let request_timeout = timeout.min(Duration::from_secs(5));
    let http = reqwest::Client::builder()
        .timeout(request_timeout)
        .build()
        .wrap_err("build bridge-finality HTTP client")?;
    let deadline = Instant::now() + timeout;
    let mut observation = "not requested".to_owned();
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        ensure!(
            !remaining.is_zero(),
            "{peer_name} did not expose height-{height} bridge finality within {timeout:?}: {observation}"
        );
        match http
            .get(url.clone())
            .header(reqwest::header::ACCEPT, "application/json")
            .send()
            .await
        {
            Ok(response) => {
                let status = response.status();
                match response.bytes().await {
                    Ok(bytes) if status.is_success() => {
                        return norito::json::from_slice::<BridgeFinalityProof>(&bytes)
                            .wrap_err_with(|| {
                                format!(
                                    "{peer_name} returned malformed height-{height} bridge finality JSON"
                                )
                            });
                    }
                    Ok(bytes) => {
                        observation = format!("HTTP {status}: {}", String::from_utf8_lossy(&bytes));
                    }
                    Err(error) => observation = format!("read response: {error}"),
                }
            }
            Err(error) => observation = error.to_string(),
        }
        tokio::time::sleep(
            Duration::from_millis(100).min(deadline.saturating_duration_since(Instant::now())),
        )
        .await;
    }
}
fn read_kura_block_at_height(
    store_dir: &Path,
    expected_height: u64,
) -> Result<Option<SignedBlock>> {
    let candidate_blocks_dir = LaneConfig::default().primary().blocks_dir(store_dir);
    let blocks_dir = if candidate_blocks_dir.join("blocks.index").exists() {
        candidate_blocks_dir
    } else {
        store_dir.to_path_buf()
    };
    let index_path = blocks_dir.join("blocks.index");
    let data_path = blocks_dir.join("blocks.data");
    if !index_path.exists() || !data_path.exists() {
        return Ok(None);
    }
    let index_offset = expected_height
        .checked_sub(1)
        .and_then(|index| index.checked_mul(16))
        .ok_or_else(|| eyre!("invalid Kura height {expected_height}"))?;
    if std::fs::metadata(&index_path)?.len() < index_offset.saturating_add(16) {
        return Ok(None);
    }
    let mut index_file = std::fs::File::open(&index_path)?;
    index_file.seek(SeekFrom::Start(index_offset))?;
    let mut word = [0_u8; 8];
    index_file.read_exact(&mut word)?;
    let data_offset = u64::from_le_bytes(word);
    index_file.read_exact(&mut word)?;
    let data_len = u64::from_le_bytes(word);
    if data_len == 0 || std::fs::metadata(&data_path)?.len() < data_offset.saturating_add(data_len)
    {
        return Ok(None);
    }
    let mut data_file = std::fs::File::open(&data_path)?;
    data_file.seek(SeekFrom::Start(data_offset))?;
    let mut bytes =
        vec![0_u8; usize::try_from(data_len).wrap_err("Kura block length does not fit usize")?];
    data_file.read_exact(&mut bytes)?;
    let block = decode_framed_signed_block(&bytes).wrap_err("decode exact Kura block wire")?;
    ensure!(
        block.header().height().get() == expected_height,
        "Kura index for height {expected_height} decoded h{}",
        block.header().height()
    );
    Ok(Some(block))
}
fn wait_for_kura_block_at_height(
    store_dir: &Path,
    expected_height: u64,
    timeout: Duration,
) -> Result<SignedBlock> {
    let deadline = Instant::now() + timeout;
    loop {
        if let Some(block) = read_kura_block_at_height(store_dir, expected_height)? {
            return Ok(block);
        }
        ensure!(
            Instant::now() < deadline,
            "Kura did not flush exact block height {expected_height} within {timeout:?}"
        );
        std::thread::sleep(
            Duration::from_millis(50).min(deadline.saturating_duration_since(Instant::now())),
        );
    }
}
fn decode_checked_v2_body_frame<'frame>(
    frame: &'frame [u8],
    expected_magic: &[u8; 8],
) -> Result<(&'frame [u8], Hash)> {
    let minimum_len = V2_BODY_FRAME_HEADER_BYTES
        .checked_add(V2_BODY_FRAME_CHECKSUM_BYTES)
        .ok_or_else(|| eyre!("v2 body frame overhead overflow"))?;
    ensure!(
        frame.len() >= minimum_len,
        "v2 body frame is shorter than its header and checksum"
    );
    ensure!(
        &frame[..expected_magic.len()] == expected_magic,
        "v2 body frame has the wrong magic"
    );
    let version_offset = expected_magic.len();
    let version = u16::from_le_bytes(
        frame[version_offset..version_offset + size_of::<u16>()]
            .try_into()
            .map_err(|_| eyre!("v2 body frame version is truncated"))?,
    );
    ensure!(
        version == V2_BODY_STORE_VERSION,
        "v2 body frame version {version} is unsupported"
    );
    let length_offset = version_offset + size_of::<u16>();
    let payload_len = u64::from_le_bytes(
        frame[length_offset..length_offset + size_of::<u64>()]
            .try_into()
            .map_err(|_| eyre!("v2 body frame payload length is truncated"))?,
    );
    let payload_len =
        usize::try_from(payload_len).wrap_err("v2 body frame payload length does not fit usize")?;
    ensure!(
        payload_len <= V2_BODY_FRAME_MAX_PAYLOAD_BYTES,
        "v2 body frame payload exceeds the integration-test read bound"
    );
    let payload_end = V2_BODY_FRAME_HEADER_BYTES
        .checked_add(payload_len)
        .ok_or_else(|| eyre!("v2 body frame payload end overflow"))?;
    let expected_len = payload_end
        .checked_add(V2_BODY_FRAME_CHECKSUM_BYTES)
        .ok_or_else(|| eyre!("v2 body frame length overflow"))?;
    ensure!(
        frame.len() == expected_len,
        "v2 body frame length does not match its header"
    );
    let payload = &frame[V2_BODY_FRAME_HEADER_BYTES..payload_end];
    ensure!(
        Hash::new(payload).as_ref() == &frame[payload_end..expected_len],
        "v2 body frame checksum mismatch"
    );
    Ok((payload, Hash::new(frame)))
}
fn read_checked_v2_body_frame(path: &Path, expected_magic: &[u8; 8]) -> Result<(Vec<u8>, Hash)> {
    let frame = std::fs::read(path)
        .wrap_err_with(|| format!("read durable v2 body frame {}", path.display()))?;
    let (payload, frame_hash) = decode_checked_v2_body_frame(&frame, expected_magic)
        .wrap_err_with(|| format!("validate durable v2 body frame {}", path.display()))?;
    Ok((payload.to_vec(), frame_hash))
}
fn validate_exact_da_transaction(transaction: &SignedTransaction) -> Result<()> {
    let Executable::Instructions(instructions) = transaction.instructions() else {
        return Err(eyre!(
            "submitted DA carrier entrypoint is not native instructions"
        ));
    };
    ensure!(
        instructions.len() == 1,
        "submitted DA carrier changed its one-instruction entrypoint"
    );
    let log = instructions[0]
        .as_any()
        .downcast_ref::<Log>()
        .ok_or_else(|| eyre!("submitted DA carrier entrypoint is not Log"))?;
    ensure!(
        log.level == Level::INFO
            && log.msg.len() == PACKET_LOSS_PAYLOAD_BYTES
            && log.msg.as_bytes().iter().all(|byte| *byte == b'P'),
        "submitted DA carrier did not preserve the exact 10 MiB P payload"
    );
    Ok(())
}
fn validate_exact_da_block_transactions(
    block: &SignedBlock,
    submitted_hash: HashOf<SignedTransaction>,
    carrier: &str,
) -> Result<()> {
    let transactions = block.external_transactions().collect::<Vec<_>>();
    ensure!(
        transactions.len() == 1 && transactions[0].hash() == submitted_hash,
        "{carrier} does not contain exactly the submitted 10 MiB transaction: observed={:?}",
        transactions
            .iter()
            .map(|transaction| transaction.hash())
            .collect::<Vec<_>>()
    );
    validate_exact_da_transaction(transactions[0])
}
fn try_read_exact_held_da_body(
    store_dir: &Path,
    expected_height: u64,
    expected_view: u64,
    held_manifest_hash: HashOf<PayloadManifest>,
    submitted_hash: HashOf<SignedTransaction>,
) -> Result<Option<HeldDaBodyEvidence>> {
    let bodies_root = store_dir.join("sumeragi_v2").join("bodies");
    let filename_prefix = format!("{expected_height:020}-{expected_view:020}-");
    let mut matched = None;
    let context_entries = match std::fs::read_dir(&bodies_root) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(error).wrap_err_with(|| {
                format!(
                    "enumerate durable body contexts in {}",
                    bodies_root.display()
                )
            });
        }
    };
    for context_entry in context_entries {
        let context_entry = context_entry.wrap_err("read durable body context entry")?;
        if !context_entry
            .file_type()
            .wrap_err("read durable body context entry type")?
            .is_dir()
        {
            continue;
        }
        for body_entry in
            std::fs::read_dir(context_entry.path()).wrap_err("enumerate durable body frames")?
        {
            let body_entry = body_entry.wrap_err("read durable body frame entry")?;
            if !body_entry
                .file_type()
                .wrap_err("read durable body frame entry type")?
                .is_file()
            {
                continue;
            }
            let body_path = body_entry.path();
            let Some(filename) = body_path.file_name().and_then(|name| name.to_str()) else {
                continue;
            };
            if !filename.starts_with(&filename_prefix)
                || body_path
                    .extension()
                    .and_then(|extension| extension.to_str())
                    != Some("norito")
            {
                continue;
            }
            let (payload, body_frame_hash) =
                read_checked_v2_body_frame(&body_path, V2_BODY_STORE_MAGIC)?;
            let mut cursor = payload.as_slice();
            let envelope = StoredDaBodyEnvelope::decode_all(&mut cursor)
                .wrap_err("decode durable v2 body envelope")?;
            if HashOf::<PayloadManifest>::new(&envelope.manifest) != held_manifest_hash {
                continue;
            }
            ensure!(
                envelope.version == V2_BODY_STORE_VERSION
                    && envelope.context_id == envelope.round.context_id
                    && envelope.round.height == expected_height
                    && envelope.round.view == expected_view
                    && envelope.manifest.round == envelope.round
                    && envelope.manifest.subject == envelope.subject
                    && envelope.manifest.payload_size_bytes
                        == u64::try_from(envelope.canonical_wire.len())
                            .wrap_err("held durable body length does not fit u64")?,
                "held manifest's durable body envelope changed its exact context/round/subject binding"
            );
            ensure!(
                Hash::new(&envelope.canonical_wire) == envelope.subject.payload_hash,
                "held manifest's durable body bytes changed their payload subject"
            );
            let body = decode_framed_signed_block(&envelope.canonical_wire)
                .wrap_err("decode exact held durable body wire")?;
            let reencoded = body
                .encode_wire()
                .wrap_err("re-encode exact held durable body wire")?;
            ensure!(
                body.is_resultless_proposal()
                    && reencoded == envelope.canonical_wire
                    && body.header().height().get() == expected_height
                    && body.header().view_change_index() <= expected_view
                    && body.hash() == envelope.subject.block_hash
                    && body.header().prev_block_hash() == envelope.subject.parent_block_hash,
                "held manifest's durable body changed its exact block subject"
            );
            validate_exact_da_block_transactions(&body, submitted_hash, "held durable DA body")?;

            let marker_path = body_path.with_extension("validated");
            if !marker_path
                .try_exists()
                .wrap_err("inspect held durable body validation marker")?
            {
                continue;
            }
            let (marker_payload, _) =
                read_checked_v2_body_frame(&marker_path, V2_BODY_VALIDATION_MAGIC)?;
            let mut marker_cursor = marker_payload.as_slice();
            let marker = StoredDaBodyValidationMarker::decode_all(&mut marker_cursor)
                .wrap_err("decode held durable body validation marker")?;
            ensure!(
                marker.version == V2_BODY_VALIDATION_VERSION
                    && marker.context_id == envelope.context_id
                    && marker.round == envelope.round
                    && marker.subject == envelope.subject
                    && marker.manifest_hash == held_manifest_hash
                    && marker.body_frame_hash == body_frame_hash,
                "held manifest's durable validation marker changed its exact frame binding"
            );
            let execution_commitment = match marker.outcome {
                StoredDaBodyValidationOutcome::Validated(commitment) => commitment,
                StoredDaBodyValidationOutcome::Rejected(code) => {
                    return Err(eyre!(
                        "held manifest's durable body was rejected with code {code}"
                    ));
                }
            };
            execution_commitment
                .validate()
                .wrap_err("validate held durable body execution commitment")?;
            ensure!(
                matched
                    .replace(HeldDaBodyEvidence {
                        manifest: envelope.manifest,
                        subject: envelope.subject,
                        canonical_wire: envelope.canonical_wire,
                        execution_commitment,
                    })
                    .is_none(),
                "held manifest matched multiple durable body frames"
            );
        }
    }
    Ok(matched)
}
fn has_four_peer_held_durable_quorum_intersection(
    held_receivers: &[bool],
    durable_receivers: &[bool],
) -> bool {
    held_receivers.len() == 4
        && durable_receivers.len() == 4
        && held_receivers.iter().filter(|held| **held).count() >= 3
        && durable_receivers.iter().filter(|durable| **durable).count() >= 3
        && held_receivers
            .iter()
            .zip(durable_receivers)
            .filter(|(held, durable)| **held && **durable)
            .count()
            >= 2
}
fn validate_held_final_manifest_relation(
    held_view: u64,
    finalized_view: u64,
    held_subject: BlockSubject,
    finalized_subject: BlockSubject,
    held_manifest_hash: HashOf<PayloadManifest>,
    finalized_manifest_hash: HashOf<PayloadManifest>,
) -> Result<bool> {
    ensure!(
        finalized_view >= held_view,
        "finalized DA carrier regressed behind the authenticated held view"
    );
    let same_subject = held_subject == finalized_subject;
    if same_subject {
        ensure!(
            (finalized_view == held_view) == (finalized_manifest_hash == held_manifest_hash),
            "unchanged DA subject did not preserve the exact same-view manifest or rotate it at a later view"
        );
    } else {
        ensure!(
            finalized_view > held_view && finalized_manifest_hash != held_manifest_hash,
            "fresh finalized subject did not advance beyond the held manifest's proposal round"
        );
    }
    Ok(same_subject)
}
fn validate_exact_applied_payload_carrier(
    client: Client,
    kura_store_dir: PathBuf,
    submitted_hash: HashOf<SignedTransaction>,
    expected_height: u64,
    held_view: u64,
    finality_proof: BridgeFinalityProof,
    network_id: NetworkId,
    held_manifest_hash: HashOf<PayloadManifest>,
    held: Arc<HeldDaBodyEvidence>,
) -> Result<BlockSubject> {
    let status = fetch_v2_status(client.clone())?;
    validate_committed_da_status(&status, expected_height)?;
    ensure!(
        status.last_committed_height == expected_height,
        "peer advanced past exact DA carrier height {expected_height}: committed={}",
        status.last_committed_height
    );
    let subject = status
        .last_committed_subject
        .ok_or_else(|| eyre!("peer omitted exact DA carrier subject"))?;
    let commit_qc = status
        .last_commit_qc
        .as_ref()
        .ok_or_else(|| eyre!("peer omitted exact DA carrier CommitQC"))?;
    let committed_view = commit_qc.certificate.round.view;
    ensure!(
        commit_qc.certificate.phase == GlobalPhase::Commit
            && commit_qc.certificate.round.height == expected_height
            && committed_view >= held_view
            && commit_qc.certificate.proposal_round == commit_qc.certificate.round
            && commit_qc.certificate.subject == subject,
        "peer status CommitQC does not authenticate a height-{expected_height} decision at or after held view {held_view}"
    );
    let pipeline_status = client
        .get_transaction_status_response_local(submitted_hash)
        .wrap_err("query exact local DA transaction status")?
        .ok_or_else(|| eyre!("peer omitted exact local DA transaction status"))?;
    ensure!(
        pipeline_status.hash == submitted_hash.to_string()
            && pipeline_status.status.kind == "Applied"
            && pipeline_status.status.block_height == Some(expected_height),
        "peer did not locally resolve the exact submitted hash as Applied at height {expected_height}: {pipeline_status:?}"
    );
    let carrier = wait_for_kura_block_at_height(
        &kura_store_dir,
        expected_height,
        PACKET_LOSS_CONTROL_TIMEOUT,
    )?;
    ensure!(
        carrier.header().height().get() == expected_height
            && carrier.header().view_change_index() <= committed_view,
        "submitted 10 MiB transaction used h{}/v{}, outside the h{expected_height}/v<={committed_view} finality decision",
        carrier.header().height(),
        carrier.header().view_change_index()
    );
    ensure!(
        carrier.hash() == subject.block_hash,
        "status/QC subject does not identify the exact submitted payload carrier"
    );
    validate_exact_da_block_transactions(&carrier, submitted_hash, "finalized local Kura carrier")?;
    verify_bridge_finality_proof(&finality_proof, &network_id)
        .wrap_err("verify exact DA bridge finality proof")?;
    let artifact = &finality_proof.finality_artifact;
    ensure!(
        artifact.height == expected_height
            && artifact.subject == subject
            && artifact.block_hash == carrier.hash()
            && artifact.commit_qc.as_ref() == commit_qc.certificate
            && finality_proof.block_header == carrier.header(),
        "bridge finality evidence does not authenticate the exact local Kura carrier"
    );
    let proposal = carrier.canonical_resultless_proposal();
    ensure!(
        proposal.hash() == artifact.subject.block_hash,
        "canonical resultless proposal changed the finalized block subject"
    );
    let payload = proposal
        .encode_wire()
        .wrap_err("encode canonical resultless DA proposal")?;
    ensure!(
        Hash::new(&payload) == artifact.subject.payload_hash,
        "canonical resultless proposal wire does not match the finalized payload hash"
    );
    let chunks = encode_payload_chunks(artifact.height_context.da_layout, &payload)
        .wrap_err("encode exact finalized RS16 payload chunks")?;
    let manifest = PayloadManifest::derive(
        &artifact.height_context,
        artifact.commit_qc.proposal_round,
        artifact.subject,
        u64::try_from(payload.len()).wrap_err("exact DA payload length does not fit u64")?,
        &chunks,
    )
    .wrap_err("derive exact finalized payload manifest")?;
    let finalized_manifest_hash = HashOf::<PayloadManifest>::new(&manifest);
    ensure!(
        held.manifest.round.context_id == artifact.context_id()
            && held.manifest.round.height == expected_height
            && held.manifest.round.view == held_view
            && held.manifest.subject == held.subject
            && HashOf::<PayloadManifest>::new(&held.manifest) == held_manifest_hash,
        "captured held body evidence does not bind the finalized height context and exact held manifest"
    );
    held.manifest
        .validate(&artifact.height_context)
        .wrap_err("validate held manifest against finalized height context")?;
    let held_chunks =
        encode_payload_chunks(artifact.height_context.da_layout, &held.canonical_wire)
            .wrap_err("encode captured held RS16 payload chunks")?;
    let rederived_held_manifest = PayloadManifest::derive(
        &artifact.height_context,
        held.manifest.round,
        held.subject,
        u64::try_from(held.canonical_wire.len())
            .wrap_err("captured held DA payload length does not fit u64")?,
        &held_chunks,
    )
    .wrap_err("rederive captured held payload manifest")?;
    ensure!(
        rederived_held_manifest == held.manifest
            && HashOf::<PayloadManifest>::new(&rederived_held_manifest) == held_manifest_hash,
        "captured held body bytes do not rederive the exact authenticated RS16 manifest"
    );
    let same_subject = validate_held_final_manifest_relation(
        held_view,
        committed_view,
        held.subject,
        artifact.subject,
        held_manifest_hash,
        finalized_manifest_hash,
    )?;
    if same_subject {
        let held_round = ConsensusRound {
            context_id: artifact.context_id(),
            height: expected_height,
            view: held_view,
        };
        let held_manifest_from_final_body = PayloadManifest::derive(
            &artifact.height_context,
            held_round,
            held.subject,
            u64::try_from(payload.len()).wrap_err("held DA payload length does not fit u64")?,
            &chunks,
        )
        .wrap_err("rederive unchanged held payload manifest")?;
        ensure!(
            held.canonical_wire.as_slice() == payload.as_slice()
                && held.execution_commitment == artifact.commit_qc.execution_commitment
                && held.manifest == held_manifest_from_final_body
                && HashOf::<PayloadManifest>::new(&held_manifest_from_final_body)
                    == held_manifest_hash,
            "unchanged held subject lost exact body, execution, or held-round manifest identity during certification"
        );
    }
    Ok(subject)
}
async fn wait_for_applied_v2_height(
    network: &Network,
    minimum_height: u64,
    timeout: Duration,
) -> Result<()> {
    let deadline = Instant::now() + timeout;
    let mut observations = Vec::new();
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        ensure!(
            !remaining.is_zero(),
            "validators did not expose applied v2 height {minimum_height} within {timeout:?}: {}",
            observations.join(" | ")
        );
        observations.clear();
        let mut all_applied = true;
        let status_fetches = network.peers().iter().map(|peer| {
            let mnemonic = peer.mnemonic().to_owned();
            let mut client = peer.client();
            client.torii_request_timeout = remaining;
            async move {
                let status = tokio::task::spawn_blocking(move || client.get_sumeragi_status())
                    .await
                    .wrap_err_with(|| format!("join applied-height status fetch for {mnemonic}"))?;
                Ok::<_, eyre::Report>((mnemonic, status))
            }
        });
        let fetched = tokio::time::timeout(remaining, try_join_all(status_fetches))
            .await
            .wrap_err_with(|| {
                format!(
                    "applied v2 height {minimum_height} status fetch exceeded the {timeout:?} convergence budget"
                )
            })??;
        for (mnemonic, result) in fetched {
            match result {
                Ok(status) => {
                    let valid = status.validate().is_ok();
                    all_applied &= valid && status.last_committed_height >= minimum_height;
                    observations.push(format!(
                        "{}=h{}/committed{}/valid{}",
                        mnemonic, status.height, status.last_committed_height, valid
                    ));
                }
                Err(error) => {
                    all_applied = false;
                    observations.push(format!("{mnemonic}={error}"));
                }
            }
        }
        if all_applied {
            return Ok(());
        }
        if Instant::now() >= deadline {
            return Err(eyre!(
                "validators did not expose applied v2 height {minimum_height} within {timeout:?}: {}",
                observations.join(" | ")
            ));
        }
        tokio::time::sleep(
            Duration::from_millis(100).min(deadline.saturating_duration_since(Instant::now())),
        )
        .await;
    }
}
async fn wait_for_exact_round_leader(
    network: &Network,
    expected_height: u64,
    expected_view: u64,
    timeout: Duration,
) -> Result<usize> {
    let deadline = Instant::now() + timeout;
    let mut observations = Vec::new();
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        ensure!(
            !remaining.is_zero(),
            "validators did not converge on exact h{expected_height}/v{expected_view} within {timeout:?}: {}",
            observations.join(" | ")
        );
        observations.clear();
        let mut statuses = Vec::with_capacity(network.peers().len());
        let status_fetches = network.peers().iter().map(|peer| {
            let mnemonic = peer.mnemonic().to_owned();
            let mut client = peer.client();
            client.torii_request_timeout = remaining;
            async move {
                let status = tokio::task::spawn_blocking(move || fetch_v2_status(client))
                    .await
                    .wrap_err_with(|| format!("join exact-round status fetch for {mnemonic}"))?;
                Ok::<_, eyre::Report>((mnemonic, status))
            }
        });
        let fetched = tokio::time::timeout(remaining, try_join_all(status_fetches))
            .await
            .wrap_err_with(|| {
                format!(
                    "exact h{expected_height}/v{expected_view} status fetch exceeded the {timeout:?} convergence budget"
                )
            })??;
        for (mnemonic, result) in fetched {
            match result {
                Ok(status) => {
                    status.validate().map_err(|error| {
                        eyre!(
                            "{} exposed invalid Sumeragi v2 status while selecting the exact leader: {error}",
                            mnemonic
                        )
                    })?;
                    if status.height > expected_height
                        || (status.height == expected_height && status.view > expected_view)
                    {
                        return Err(eyre!(
                            "{} advanced past exact h{expected_height}/v{expected_view} before submission: h{}/v{}",
                            mnemonic,
                            status.height,
                            status.view
                        ));
                    }
                    observations.push(format!(
                        "{}=h{}/v{}/leader{}",
                        mnemonic, status.height, status.view, status.leader
                    ));
                    statuses.push(status);
                }
                Err(error) => {
                    observations.push(format!("{mnemonic}={error}"));
                }
            }
        }
        if statuses.len() == network.peers().len()
            && statuses
                .iter()
                .all(|status| status.height == expected_height && status.view == expected_view)
        {
            let leader = statuses[0].leader;
            let context = statuses[0].height_context_id;
            ensure!(
                statuses.iter().all(|status| {
                    status.leader == leader && status.height_context_id == context
                }),
                "validators disagreed on the exact h{expected_height}/v{expected_view} leader: {}",
                observations.join(" | ")
            );
            let leader =
                usize::try_from(leader).wrap_err("exact leader index does not fit usize")?;
            ensure!(
                leader < network.peers().len(),
                "exact leader index {leader} is outside the {}-peer fixture",
                network.peers().len()
            );
            return Ok(leader);
        }
        if Instant::now() >= deadline {
            return Err(eyre!(
                "validators did not converge on exact h{expected_height}/v{expected_view} within {timeout:?}: {}",
                observations.join(" | ")
            ));
        }
        tokio::time::sleep(
            Duration::from_millis(100).min(deadline.saturating_duration_since(Instant::now())),
        )
        .await;
    }
}
fn is_route_unavailable_submission(error: &eyre::Report) -> bool {
    error.to_string().contains("reject code: route_unavailable")
}
async fn submit_prepared_with_route_retry(
    mut client: Client,
    transaction: SignedTransaction,
    timeout: Duration,
) -> Result<HashOf<SignedTransaction>> {
    let prepared = Client::prepare_transaction_payload(&transaction);
    let deadline = Instant::now() + timeout;
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        ensure!(
            !remaining.is_zero(),
            "authoritative lane route did not become available within the {timeout:?} submission budget"
        );
        client.torii_request_timeout = remaining;
        let attempt = prepared.clone();
        let result = tokio::time::timeout(
            remaining,
            client.submit_prepared_transaction_payload_async(&attempt),
        )
        .await
        .wrap_err_with(|| {
            format!("exact prepared DA submission exceeded its {timeout:?} route budget")
        })?;
        match result {
            Ok(hash) => return Ok(hash),
            Err(error) if is_route_unavailable_submission(&error) => {
                let remaining = deadline.saturating_duration_since(Instant::now());
                if remaining.is_zero() {
                    return Err(error).wrap_err(
                        "authoritative lane route did not become available within the submission budget",
                    );
                }
                tokio::time::sleep(ROUTE_BINDING_POLL.min(remaining)).await;
            }
            Err(error) => return Err(error).wrap_err("submit exact prepared DA payload"),
        }
    }
}
async fn wait_for_exact_local_queue_replication(
    network: &Network,
    submitted_hash: HashOf<SignedTransaction>,
    carrier_height: u64,
    timeout: Duration,
) -> Result<()> {
    let deadline = Instant::now() + timeout;
    let expected_hash = submitted_hash.to_string();
    let mut witnessed = vec![false; network.peers().len()];
    let mut observations = vec![String::new(); network.peers().len()];
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        ensure!(
            !remaining.is_zero(),
            "exact signed transaction did not reach every peer-local queue within {timeout:?}: {}",
            observations.join(" | ")
        );
        let request_timeout = remaining.min(Duration::from_secs(5));
        let fetches = network
            .peers()
            .iter()
            .enumerate()
            .map(|(peer_index, peer)| {
                let mnemonic = peer.mnemonic().to_owned();
                let mut client = peer.client();
                client.torii_request_timeout = request_timeout;
                let hash = submitted_hash;
                async move {
                    let result = tokio::task::spawn_blocking(move || {
                        let blocks = client.get_status()?.blocks;
                        let pipeline = client.get_transaction_status_response_local(hash)?;
                        Ok::<_, eyre::Report>((blocks, pipeline))
                    })
                    .await
                    .wrap_err_with(|| {
                        format!("join exact local queue observation for {mnemonic}")
                    })?;
                    Ok::<_, eyre::Report>((peer_index, mnemonic, result))
                }
            });
        let fetched = tokio::time::timeout(remaining, try_join_all(fetches))
            .await
            .wrap_err_with(|| {
                format!("exact local queue replication exceeded the {timeout:?} budget")
            })??;
        for (peer_index, mnemonic, result) in fetched {
            match result {
                Ok((blocks, pipeline)) => {
                    ensure!(
                        blocks < carrier_height,
                        "{mnemonic} committed height {carrier_height} before peer-local queue replication was witnessed"
                    );
                    match pipeline {
                        Some(status)
                            if status.hash == expected_hash
                                && status.scope == "local"
                                && status.status.kind == "Queued"
                                && status.status.block_height.is_none() =>
                        {
                            witnessed[peer_index] = true;
                            observations[peer_index] = format!("{mnemonic}=Queued/local");
                        }
                        Some(status) => {
                            observations[peer_index] = format!("{mnemonic}={status:?}");
                        }
                        None => {
                            observations[peer_index] = format!("{mnemonic}=missing");
                        }
                    }
                }
                Err(error) => {
                    observations[peer_index] = format!("{mnemonic}={error}");
                }
            }
        }
        if witnessed.iter().all(|seen| *seen) {
            return Ok(());
        }
        tokio::time::sleep(
            Duration::from_millis(100).min(deadline.saturating_duration_since(Instant::now())),
        )
        .await;
    }
}
async fn large_da_payload_commits_with_consistent_v2_subject_for_committee(
    peers: usize,
    scenario: &str,
) -> Result<()> {
    init_instruction_registry();
    let Some(network) = sandbox::start_network_async_or_skip(
        large_da_network_builder(peers, LARGE_PAYLOAD_BYTES),
        scenario,
    )
    .await?
    else {
        return Ok(());
    };
    network
        .ensure_blocks_with(|height| height.total >= 1)
        .await?;
    wait_for_applied_v2_height(&network, 1, COMMIT_WAIT_BUDGET).await?;
    let client = network.client();
    let expected_height = client.get_status()?.blocks.saturating_add(1);
    let payload = "D".repeat(LARGE_PAYLOAD_BYTES);
    let submit_client = client.clone();
    tokio::task::spawn_blocking(move || {
        submit_client.submit(
            Log::new(Level::INFO, payload),
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
    })
    .await
    .wrap_err("join large DA payload submission")??;
    tokio::time::timeout(
        COMMIT_WAIT_BUDGET,
        network.ensure_blocks_with(|height| height.total >= expected_height),
    )
    .await
    .wrap_err("large DA payload did not commit within the budget")??;
    let required = network.peers().len().saturating_sub(1).max(1);
    let mut committed_subjects = Vec::new();
    for peer in network.peers() {
        let status_client = peer.client();
        let status = tokio::task::spawn_blocking(move || fetch_v2_status(status_client))
            .await
            .wrap_err("join canonical v2 status request")??;
        if validate_committed_da_status(&status, expected_height).is_ok() {
            committed_subjects.push(status.last_committed_subject);
        }
    }
    ensure!(
        committed_subjects.len() >= required,
        "expected canonical DA commit evidence on {required} peers, observed {}",
        committed_subjects.len()
    );
    let expected_subject = committed_subjects[0];
    ensure!(
        committed_subjects
            .iter()
            .all(|subject| *subject == expected_subject),
        "quorum peers must agree on the committed DA subject: {committed_subjects:?}"
    );
    network.shutdown().await;
    Ok(())
}
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn large_da_payload_commits_with_consistent_v2_subject_four_peers() -> Result<()> {
    large_da_payload_commits_with_consistent_v2_subject_for_committee(
        4,
        stringify!(large_da_payload_commits_with_consistent_v2_subject_four_peers),
    )
    .await
}
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn large_da_payload_commits_with_consistent_v2_subject_seven_peers() -> Result<()> {
    large_da_payload_commits_with_consistent_v2_subject_for_committee(
        7,
        stringify!(large_da_payload_commits_with_consistent_v2_subject_seven_peers),
    )
    .await
}
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn authenticated_payload_chunk_hold_heals_and_converges_four_peers() -> Result<()> {
    init_instruction_registry();
    let builder = large_da_network_builder(4, PACKET_LOSS_PAYLOAD_BYTES)
        .with_base_seed(stringify!(
            authenticated_payload_chunk_hold_heals_and_converges_four_peers
        ))
        // QueuePlan binds transactions admitted during height two to the
        // successor proposal. Keep that admission interval comfortably open,
        // then exercise the exact height-three carrier selected by production.
        .with_block_cadence(PACKET_LOSS_BLOCK_CADENCE)
        .with_sync_timeout(COMMIT_WAIT_BUDGET)
        .with_peer_startup_timeout(COMMIT_WAIT_BUDGET)
        .with_consensus_message_control();
    let scenario = stringify!(authenticated_payload_chunk_hold_heals_and_converges_four_peers);
    let Some(network) = sandbox::start_network_async_or_skip(builder, scenario).await? else {
        return Ok(());
    };
    let result = async {
        ensure!(network.peers().len() == 4, "DA loss test requires four peers");
        network.ensure_blocks_with(|height| height.total >= 1).await?;
        wait_for_applied_v2_height(&network, 1, PACKET_LOSS_CONTROL_TIMEOUT).await?;
        let peers = network.peers().to_vec();
        let peer_ids = peers.iter().map(|peer| peer.id()).collect::<Vec<_>>();
        let expected_initial_rules = peers
            .iter()
            .enumerate()
            .map(|(receiver_index, _)| hold_bounded_view_payload_chunks(receiver_index, &peer_ids))
            .collect::<Vec<_>>();
        try_join_all(peers.iter().zip(&expected_initial_rules).map(
            |(peer, expected_rules)| async move {
                let control = peer
                    .consensus_message_control()
                    .ok_or_else(|| eyre!("{} lacks message control", peer.mnemonic()))?;
                let initial = control
                    .wait_until_ready(PACKET_LOSS_CONTROL_TIMEOUT)
                    .await?;
                ensure!(
                    initial.revision == 1
                        && initial.rules.is_empty()
                        && !initial.draining
                        && !initial.fatal
                        && initial.dropped == 0
                        && initial.overflowed == 0,
                    "{} did not acknowledge its empty genesis-safe controller revision",
                    peer.mnemonic()
                );
                let ack = control
                    .apply(
                        expected_rules,
                        &[],
                        PACKET_LOSS_QUEUE_CAPACITY,
                        PACKET_LOSS_CONTROL_TIMEOUT,
                    )
                    .await?;
                ensure!(
                    ack.revision == 2
                        && ack.rules.as_slice() == expected_rules.as_slice()
                        && ack.queue_capacity == PACKET_LOSS_QUEUE_CAPACITY
                        && !ack.draining
                        && !ack.fatal
                        && ack.dropped == 0
                        && ack.overflowed == 0,
                    "{} did not acknowledge its authenticated Proposal-bound chunk Hold rules",
                    peer.mnemonic()
                );
                Ok::<(), eyre::Report>(())
            },
        ))
        .await?;

        let client = network.client();
        let admission_height = client.get_status()?.blocks.saturating_add(1);
        ensure!(
            admission_height == PACKET_LOSS_ADMISSION_HEIGHT,
            "packet-loss admission expected active height {PACKET_LOSS_ADMISSION_HEIGHT}, but the network opened {admission_height}"
        );
        let expected_height = admission_height.saturating_add(1);
        ensure!(
            expected_height == PACKET_LOSS_HEIGHT,
            "packet-loss rules target successor height {PACKET_LOSS_HEIGHT}, but admission selected {expected_height}"
        );
        let prepare_client = client.clone();
        let transaction = tokio::task::spawn_blocking(move || {
            let payload = prepare_client.try_build_transaction_payload(
                vec![Log::new(
                    Level::INFO,
                    "P".repeat(PACKET_LOSS_PAYLOAD_BYTES),
                )],
                iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
                Metadata::default(),
            )?;
            prepare_client.quote_and_sign_transaction_payload(payload)
        })
        .await
        .wrap_err("join packet-loss payload preparation")??;
        validate_exact_da_transaction(&transaction)?;
        // Submit directly to the unanimously reported height-two/view-zero
        // admission leader.
        // Production QueuePlan binds this height-two admission to the
        // height-three proposal; ordinary transaction gossip ensures whichever
        // validator owns that successor turn has the exact signed transaction.
        let leader_index = wait_for_exact_round_leader(
            &network,
            admission_height,
            PACKET_LOSS_ADMISSION_VIEW,
            PACKET_LOSS_CONTROL_TIMEOUT,
        )
        .await?;
        // Permissioned height contexts retain the signed genesis roster's
        // canonical PeerId order; the builder's process order is not authority.
        let mut roster_peers = peers.iter().collect::<Vec<_>>();
        roster_peers.sort_by(|left, right| left.id().cmp(&right.id()));
        let submitted_hash = submit_prepared_with_route_retry(
            roster_peers[leader_index].client(),
            transaction,
            PACKET_LOSS_CONTROL_TIMEOUT,
        )
        .await?;
        // Prove that normal transaction gossip has placed the exact signed
        // carrier in every local queue before DA transport begins. The later
        // no-commit assertion therefore also proves that a mempool copy cannot
        // bypass the authenticated manifest/chunk reconstruction barrier.
        wait_for_exact_local_queue_replication(
            &network,
            submitted_hash,
            expected_height,
            PACKET_LOSS_QUEUE_REPLICATION_TIMEOUT,
        )
        .await?;

        let proposal_match_deadline = Instant::now() + PACKET_LOSS_CONTROL_TIMEOUT;
        // Arm the finality fence as soon as one common authenticated Proposal
        // occurrence reaches three receivers. Waiting for every selected
        // chunk first leaves a legitimate timeout already delivered to fair
        // ingress; that timeout can advance the view ahead of the released
        // chunks and correctly retire their now-unprotected stale pipeline.
        let (pre_fence_matches, held_manifest_hash, held_proposal_view) = loop {
            let mut matched_sequences = vec![Vec::new(); peers.len()];
            for (receiver_index, peer) in peers.iter().enumerate() {
                let ack = peer
                    .consensus_message_control()
                    .ok_or_else(|| eyre!("{} lacks message control", peer.mnemonic()))?
                    .read_ack()?;
                ensure!(
                    ack.revision == 2
                        && ack.queue_capacity == PACKET_LOSS_QUEUE_CAPACITY
                        && !ack.draining
                        && !ack.fatal
                        && ack.dropped == 0
                        && ack.overflowed == 0,
                    "{} drifted from its acknowledged deferred chunk selector command",
                    peer.mnemonic()
                );
                matched_sequences[receiver_index] = proposal_bound_payload_chunk_matches(&ack)?;
            }
            let common_manifest_and_view = matched_sequences
                .iter()
                .flat_map(|matches| {
                    matches
                        .iter()
                        .map(|(_, manifest, _, view)| (*manifest, *view))
                })
                .find(|(manifest, view)| {
                    matched_sequences
                        .iter()
                        .filter(|matches| {
                            matches.iter().any(
                                |(_, matched_manifest, _, matched_view)| {
                                    matched_manifest == manifest && matched_view == view
                                },
                            )
                        })
                        .count()
                        >= 3
                });
            if let Some((manifest_hash, proposal_view)) = common_manifest_and_view {
                break (matched_sequences, manifest_hash, proposal_view);
            }
            ensure!(
                Instant::now() < proposal_match_deadline,
                "fewer than three receivers observed one common authenticated RS16 manifest/view before the capture fence"
            );
            tokio::time::sleep(Duration::from_millis(10)).await;
        };
        // Keep the original chunk selectors active while arming the finality
        // fence on every receiver. A non-drain command changes each selector
        // atomically while leaving old retained occurrences queued; combining
        // both rule sets closes the cross-peer cutover window as well.
        let capture_arm_rules = peers
            .iter()
            .enumerate()
            .map(|(receiver_index, _)| {
                let mut rules = expected_initial_rules[receiver_index].clone();
                rules.extend(hold_bounded_view_finality_traffic(
                    receiver_index,
                    &peer_ids,
                ));
                rules
            })
            .collect::<Vec<_>>();
        let capture_arm_acknowledgements = try_join_all((0..peers.len()).map(|peer_index| {
            let peer = &peers[peer_index];
            let rules = &capture_arm_rules[peer_index];
            async move {
                peer.consensus_message_control()
                    .ok_or_else(|| eyre!("{} lacks message control", peer.mnemonic()))?
                    .apply(
                        rules,
                        &[],
                        PACKET_LOSS_CAPTURE_QUEUE_CAPACITY,
                        PACKET_LOSS_CONTROL_TIMEOUT,
                    )
                    .await
            }
        }))
        .await?;
        for (((peer, ack), matched), rules) in peers
            .iter()
            .zip(&capture_arm_acknowledgements)
            .zip(&pre_fence_matches)
            .zip(&capture_arm_rules)
        {
            ensure!(
                ack.revision == 3
                    && ack.rules.len() == rules.len()
                    && ack.queue_capacity == PACKET_LOSS_CAPTURE_QUEUE_CAPACITY
                    && !ack.draining
                    && ack.release_pending.is_empty()
                    && ack.in_flight.is_none()
                    && !ack.fatal
                    && ack.dropped == 0
                    && ack.overflowed == 0,
                "{} did not acknowledge the pre-release finality capture fence",
                peer.mnemonic()
            );
            ensure!(
                matched
                    .iter()
                    .all(|(sequence, _, _, _)| ack.held.iter().any(|held| held.sequence == *sequence)),
                "{} lost an exactly matched chunk while arming the finality capture fence: matched={matched:?}, held={:?}",
                peer.mnemonic(),
                ack.held
            );
        }
        // With timeout and Commit traffic fenced, wait for the complete exact
        // loss set on a receiver quorum. The acknowledgement snapshot returned
        // here is also the sole release source, so chunks arriving during the
        // cross-peer arm cannot be omitted from the healed sequence set.
        let complete_match_deadline = Instant::now() + PACKET_LOSS_CONTROL_TIMEOUT;
        let (matched_sequences, capture_armed) = loop {
            let mut matched_sequences = vec![Vec::new(); peers.len()];
            let mut acknowledgements = Vec::with_capacity(peers.len());
            for (receiver_index, peer) in peers.iter().enumerate() {
                let ack = peer
                    .consensus_message_control()
                    .ok_or_else(|| eyre!("{} lacks message control", peer.mnemonic()))?
                    .read_ack()?;
                let expected_ack = &capture_arm_acknowledgements[receiver_index];
                ensure!(
                    ack.revision == 3
                        && ack.command_digest == expected_ack.command_digest
                        && ack.rules.as_slice() == expected_ack.rules.as_slice()
                        && ack.queue_capacity == PACKET_LOSS_CAPTURE_QUEUE_CAPACITY
                        && !ack.draining
                        && !ack.fatal
                        && ack.dropped == 0
                        && ack.overflowed == 0,
                    "{} drifted from the pre-release finality capture fence",
                    peer.mnemonic()
                );
                matched_sequences[receiver_index] = proposal_bound_payload_chunk_matches(&ack)?;
                acknowledgements.push(ack);
            }
            let complete_receivers = matched_sequences
                .iter()
                .filter(|matches| {
                    PACKET_LOSS_CHUNK_INDICES.iter().all(|index| {
                        matches.iter().any(
                            |(_, matched_manifest, matched_index, matched_view)| {
                                *matched_manifest == held_manifest_hash
                                    && matched_index == index
                                    && *matched_view == held_proposal_view
                            },
                        )
                    })
                })
                .count();
            if complete_receivers >= 3 {
                break (matched_sequences, acknowledgements);
            }
            ensure!(
                Instant::now() < complete_match_deadline,
                "the armed finality fence retained fewer than three complete exact chunk loss sets for the common authenticated RS16 manifest/view"
            );
            tokio::time::sleep(Duration::from_millis(10)).await;
        };
        let matched_receivers = matched_sequences
            .iter()
            .map(|matches| {
                PACKET_LOSS_CHUNK_INDICES.iter().all(|index| {
                    matches
                        .iter()
                        .any(|(_, manifest, matched_index, proposal_view)| {
                            *manifest == held_manifest_hash
                                && matched_index == index
                                && *proposal_view == held_proposal_view
                        })
                })
            })
            .collect::<Vec<_>>();
        ensure!(
            matched_receivers.iter().filter(|matched| **matched).count() >= 3,
            "healing evidence lost the common authenticated manifest/view witness"
        );
        for peer in &peers {
            ensure!(
                peer.client().get_status()?.blocks < expected_height,
                "{} committed before the three-of-six RS16 loss was healed",
                peer.mnemonic()
            );
            let status = fetch_v2_status(peer.client())?;
            status
                .validate()
                .map_err(|error| eyre!("invalid armed-fence v2 status: {error}"))?;
            ensure!(
                status.height == expected_height
                    && status.view <= held_proposal_view
                    && status.last_committed_height < expected_height,
                "{} advanced past the fenced h{expected_height}/v{held_proposal_view} proposal during the all-peer cutover: active=h{}/v{}, committed={}",
                peer.mnemonic(),
                status.height,
                status.view,
                status.last_committed_height
            );
        }

        let chunk_releases = capture_armed
            .iter()
            .zip(&matched_receivers)
            .map(|(ack, matched_receiver)| {
                let mut sequences = ack
                    .held
                    .iter()
                    .filter(|held| {
                        *matched_receiver
                            && held.kind == ConsensusMessageControlKind::PayloadChunk
                            && held.manifest_hash == Some(held_manifest_hash)
                            && held
                                .chunk_index
                                .is_some_and(|index| PACKET_LOSS_CHUNK_INDICES.contains(&index))
                    })
                    .map(|held| held.sequence)
                    .collect::<Vec<_>>();
                sequences.sort_unstable();
                sequences
            })
            .collect::<Vec<_>>();
        for ((ack, releases), matched_receiver) in capture_armed
            .iter()
            .zip(&chunk_releases)
            .zip(&matched_receivers)
        {
            ensure!(
                !*matched_receiver
                    || (!releases.is_empty()
                        && PACKET_LOSS_CHUNK_INDICES.iter().all(|index| {
                            ack.held.iter().any(|held| {
                                releases.contains(&held.sequence)
                                    && held.manifest_hash == Some(held_manifest_hash)
                                    && held.chunk_index == Some(*index)
                            })
                        })),
                "matched receiver release set did not cover all exact selected manifest chunks"
            );
        }
        let capture_release_rules = peers
            .iter()
            .enumerate()
            .map(|(receiver_index, _)| {
                hold_exact_manifest_chunks_and_finality_traffic(
                    receiver_index,
                    &peer_ids,
                    held_manifest_hash,
                    held_proposal_view,
                )
            })
            .collect::<Vec<_>>();
        let chunks_released = try_join_all((0..peers.len()).map(|peer_index| {
            let peer = &peers[peer_index];
            let rules = &capture_release_rules[peer_index];
            let releases = &chunk_releases[peer_index];
            async move {
                peer.consensus_message_control()
                    .ok_or_else(|| eyre!("{} lacks message control", peer.mnemonic()))?
                    .apply(
                        rules,
                        releases,
                        PACKET_LOSS_CAPTURE_QUEUE_CAPACITY,
                        PACKET_LOSS_CONTROL_TIMEOUT,
                    )
                    .await
            }
        }))
        .await?;
        for (((((peer, ack), matched), rules), releases), matched_receiver) in peers
            .iter()
            .zip(&chunks_released)
            .zip(&matched_sequences)
            .zip(&capture_release_rules)
            .zip(&chunk_releases)
            .zip(&matched_receivers)
        {
            ensure!(
                ack.revision == 4
                    && ack.rules.len() == rules.len()
                    && ack.queue_capacity == PACKET_LOSS_CAPTURE_QUEUE_CAPACITY
                    && !ack.draining
                    && ack.release_pending.is_empty()
                    && ack.in_flight.is_none()
                    && !ack.fatal
                    && ack.dropped == 0
                    && ack.overflowed == 0,
                "{} did not release the selected payload chunks under the finality capture rules",
                peer.mnemonic()
            );
            ensure!(
                (!*matched_receiver
                    || matched
                        .iter()
                        .filter(|(_, manifest, index, proposal_view)| {
                            *manifest == held_manifest_hash
                                && PACKET_LOSS_CHUNK_INDICES.contains(index)
                                && *proposal_view == held_proposal_view
                        })
                        .all(|(sequence, _, _, _)| ack.delivered.contains(sequence)))
                    && releases
                        .iter()
                        .all(|sequence| ack.delivered.contains(sequence)),
                "{} did not deliver every retained payload chunk under the finality capture fence: matched={matched:?}, released={releases:?}, delivered={:?}, retired={:?}",
                peer.mnemonic(),
                ack.delivered,
                ack.retired
            );
        }
        for peer in &peers {
            let status = fetch_v2_status(peer.client())?;
            status
                .validate()
                .map_err(|error| eyre!("invalid released-fence v2 status: {error}"))?;
            ensure!(
                status.height == expected_height
                    && status.view <= held_proposal_view
                    && status.last_committed_height < expected_height,
                "{} advanced past fenced h{expected_height}/v{held_proposal_view} while the selected chunks crossed ingress: active=h{}/v{}, committed={}",
                peer.mnemonic(),
                status.height,
                status.view,
                status.last_committed_height
            );
        }

        // The finality fence keeps the height-three body-store namespace alive
        // while the released chunks reconstruct and validate the exact held
        // manifest. Capture an exact held-round durable quorum before healing
        // Commit traffic. In a four-validator committee it intersects the held
        // receiver quorum in at least two peers; prior-round markers never
        // count as authority for this check.
        let store_dirs = peers
            .iter()
            .map(|peer| peer.kura_store_dir())
            .collect::<Vec<_>>();
        let mut held_evidence_by_peer = vec![None; peers.len()];
        let evidence_deadline = Instant::now() + PACKET_LOSS_CONTROL_TIMEOUT;
        loop {
            let captured = try_join_all(
                store_dirs
                    .iter()
                    .enumerate()
                    .filter(|(peer_index, _)| held_evidence_by_peer[*peer_index].is_none())
                    .map(|(peer_index, store_dir)| {
                        let store_dir = store_dir.clone();
                        async move {
                            let evidence = tokio::task::spawn_blocking(move || {
                                try_read_exact_held_da_body(
                                    &store_dir,
                                    expected_height,
                                    held_proposal_view,
                                    held_manifest_hash,
                                    submitted_hash,
                                )
                            })
                            .await
                            .wrap_err("join held DA body evidence probe")??;
                            Ok::<_, eyre::Report>((peer_index, evidence.map(Arc::new)))
                        }
                    }),
            )
            .await?;
            for (peer_index, evidence) in captured {
                if let Some(evidence) = evidence {
                    held_evidence_by_peer[peer_index] = Some(evidence);
                }
            }
            if held_evidence_by_peer.iter().flatten().count() >= 3 {
                break;
            }
            let durable_receivers = held_evidence_by_peer
                .iter()
                .map(Option::is_some)
                .collect::<Vec<_>>();
            ensure!(
                Instant::now() < evidence_deadline,
                "held authenticated h{expected_height}/v{held_proposal_view} manifest has no exact validated durable quorum within {PACKET_LOSS_CONTROL_TIMEOUT:?}: held_receivers={matched_receivers:?}, durable_receivers={durable_receivers:?}"
            );
            tokio::time::sleep(
                Duration::from_millis(20)
                    .min(evidence_deadline.saturating_duration_since(Instant::now())),
            )
            .await;
        }
        let held_evidence = held_evidence_by_peer
            .iter()
            .flatten()
            .next()
            .cloned()
            .ok_or_else(|| eyre!("held-round durable quorum produced no body evidence"))?;
        let durable_receivers = held_evidence_by_peer
            .iter()
            .map(Option::is_some)
            .collect::<Vec<_>>();
        ensure!(
            has_four_peer_held_durable_quorum_intersection(
                &matched_receivers,
                &durable_receivers,
            )
                && held_evidence_by_peer.iter().flatten().all(|evidence| {
                    evidence.manifest == held_evidence.manifest
                        && evidence.subject == held_evidence.subject
                        && evidence.canonical_wire == held_evidence.canonical_wire
                        && evidence.execution_commitment == held_evidence.execution_commitment
                        && HashOf::<PayloadManifest>::new(&evidence.manifest)
                            == held_manifest_hash
                }),
            "held receiver and exact durable quorums did not intersect in two peers with one byte-identical validated held-round body: held_receivers={matched_receivers:?}, durable_receivers={durable_receivers:?}"
        );
        for peer in &peers {
            ensure!(
                peer.client().get_status()?.blocks < expected_height,
                "{} committed before the held body evidence capture fence was healed",
                peer.mnemonic()
            );
        }

        let fenced_sequences = chunks_released
            .iter()
            .map(|ack| ack.held.iter().map(|held| held.sequence).collect::<Vec<_>>())
            .collect::<Vec<_>>();
        let healed = try_join_all(peers.iter().map(|peer| async move {
            peer.consensus_message_control()
                .ok_or_else(|| eyre!("{} lacks message control", peer.mnemonic()))?
                .heal_and_release_all(PACKET_LOSS_CONTROL_TIMEOUT)
                .await
        }))
        .await?;
        for ((peer, ack), fenced) in peers.iter().zip(&healed).zip(&fenced_sequences) {
            ensure!(
                ack.revision == 5
                    && !ack.draining
                    && ack.drain_fence == Some(ack.revision)
                    && ack.rules.is_empty()
                    && ack.held.is_empty()
                    && ack.release_pending.is_empty()
                    && ack.in_flight.is_none()
                    && !ack.fatal
                    && ack.dropped == 0
                    && ack.overflowed == 0,
                "{} did not acknowledge a complete finality healing drain fence",
                peer.mnemonic()
            );
            ensure!(
                fenced.iter().all(|sequence| {
                    ack.delivered.contains(sequence) || ack.retired.contains(sequence)
                }),
                "{} did not settle every finality-fence occurrence during healing: fenced={fenced:?}, delivered={:?}, retired={:?}",
                peer.mnemonic(),
                ack.delivered,
                ack.retired
            );
        }

        tokio::time::timeout(
            COMMIT_WAIT_BUDGET,
            network.ensure_blocks_with(|height| height.total >= expected_height),
        )
        .await
        .wrap_err("healed four-peer DA payload did not commit")??;
        let wait_client = network.client();
        let wait_hash = submitted_hash;
        let applied = tokio::task::spawn_blocking(move || {
            wait_client.wait_for_transaction_applied(
                wait_hash,
                TransactionWaitOptions {
                    timeout: COMMIT_WAIT_BUDGET,
                    poll_interval: Duration::from_millis(100),
                },
            )
        })
        .await
        .wrap_err("join exact DA payload Applied wait")??;
        ensure!(
            applied.terminal_kind == "Applied"
                && applied.scope == "global"
                && applied.resolved_from == "state"
                && applied.block_height == Some(expected_height),
            "submitted DA payload did not reach Applied at exact height {expected_height}: {applied:?}"
        );
        // Kura persistence precedes applied-state publication. Wait until every
        // peer's canonical status endpoint has crossed that boundary before
        // inspecting its local transaction result and finality sidecar.
        wait_for_applied_v2_height(&network, expected_height, COMMIT_WAIT_BUDGET).await?;
        let mut committed_subjects = Vec::with_capacity(peers.len());
        let network_id = network.network_id();
        for peer in &peers {
            let carrier_client = peer.client();
            let kura_store_dir = peer.kura_store_dir();
            let carrier_hash = submitted_hash;
            let finality_proof = wait_for_bridge_finality_proof(
                peer.client(),
                peer.mnemonic(),
                expected_height,
                PACKET_LOSS_CONTROL_TIMEOUT,
            )
            .await?;
            let carrier_network_id = network_id.clone();
            let manifest_hash = held_manifest_hash;
            let captured_held = Arc::clone(&held_evidence);
            let subject = tokio::task::spawn_blocking(move || {
                validate_exact_applied_payload_carrier(
                    carrier_client,
                    kura_store_dir,
                    carrier_hash,
                    expected_height,
                    held_proposal_view,
                    finality_proof,
                    carrier_network_id,
                    manifest_hash,
                    captured_held,
                )
            })
                .await
                .wrap_err("join exact healed DA carrier query")??;
            committed_subjects.push(subject);
        }
        ensure!(
            committed_subjects
                .iter()
                .all(|subject| *subject == committed_subjects[0]),
            "all four peers must converge on one committed subject after chunk healing: observed={committed_subjects:?}"
        );
        Ok(())
    }
    .await;
    network.shutdown().await;
    result
}
#[test]
fn checked_v2_body_frame_rejects_length_and_checksum_drift() {
    let payload = b"exact held body fixture";
    let mut frame = Vec::new();
    frame.extend_from_slice(V2_BODY_STORE_MAGIC);
    frame.extend_from_slice(&V2_BODY_STORE_VERSION.to_le_bytes());
    frame.extend_from_slice(
        &u64::try_from(payload.len())
            .expect("fixture payload length fits u64")
            .to_le_bytes(),
    );
    frame.extend_from_slice(payload);
    frame.extend_from_slice(Hash::new(payload).as_ref());
    let (decoded, frame_hash) =
        decode_checked_v2_body_frame(&frame, V2_BODY_STORE_MAGIC).expect("valid body frame");
    assert_eq!(decoded, payload);
    assert_eq!(frame_hash, Hash::new(&frame));

    let mut bad_length = frame.clone();
    bad_length[V2_BODY_STORE_MAGIC.len() + size_of::<u16>()] ^= 1;
    assert!(decode_checked_v2_body_frame(&bad_length, V2_BODY_STORE_MAGIC).is_err());
    let mut bad_checksum = frame;
    *bad_checksum.last_mut().expect("fixture has checksum") ^= 1;
    assert!(decode_checked_v2_body_frame(&bad_checksum, V2_BODY_STORE_MAGIC).is_err());
}
#[test]
fn four_peer_held_and_durable_quorums_intersect_in_two_receivers() {
    assert!(has_four_peer_held_durable_quorum_intersection(
        &[true, true, true, false],
        &[true, false, true, true],
    ));
    assert!(!has_four_peer_held_durable_quorum_intersection(
        &[true, true, true, false],
        &[true, false, true, false],
    ));
    assert!(!has_four_peer_held_durable_quorum_intersection(
        &[true, true, false, false],
        &[true, true, true, false],
    ));
    assert!(!has_four_peer_held_durable_quorum_intersection(
        &[true, true, true, false, false],
        &[true, true, true, false, false],
    ));
}
#[test]
fn held_final_manifest_relation_covers_same_and_later_view_carriers() {
    let subject = |seed: u8| BlockSubject {
        parent_block_hash: Some(HashOf::from_untyped_unchecked(Hash::new([seed, 0]))),
        block_hash: HashOf::from_untyped_unchecked(Hash::new([seed, 1])),
        payload_hash: Hash::new([seed, 2]),
    };
    let held_subject = subject(1);
    let fresh_subject = subject(2);
    let held_manifest = HashOf::from_untyped_unchecked(Hash::new(b"held manifest"));
    let later_manifest = HashOf::from_untyped_unchecked(Hash::new(b"later manifest"));

    assert!(
        validate_held_final_manifest_relation(
            1,
            1,
            held_subject,
            held_subject,
            held_manifest,
            held_manifest,
        )
        .expect("same-view carrier keeps its manifest")
    );
    assert!(
        validate_held_final_manifest_relation(
            1,
            2,
            held_subject,
            held_subject,
            held_manifest,
            later_manifest,
        )
        .expect("later-view reproposal rotates its manifest")
    );
    assert!(
        !validate_held_final_manifest_relation(
            1,
            2,
            held_subject,
            fresh_subject,
            held_manifest,
            later_manifest,
        )
        .expect("later view may finalize a fresh subject")
    );
    assert!(
        validate_held_final_manifest_relation(
            1,
            2,
            held_subject,
            held_subject,
            held_manifest,
            held_manifest,
        )
        .is_err()
    );
    assert!(
        validate_held_final_manifest_relation(
            1,
            1,
            held_subject,
            fresh_subject,
            held_manifest,
            later_manifest,
        )
        .is_err()
    );
}
#[test]
fn large_payload_limits_cover_transport_overhead() {
    let content_limit = torii_max_content_len_for_payload(LARGE_PAYLOAD_BYTES);
    assert!(content_limit > i64::try_from(LARGE_PAYLOAD_BYTES).expect("fixture fits i64"));
    assert_eq!(
        tx_limit_for_payload(LARGE_PAYLOAD_BYTES).get(),
        u64::try_from(content_limit).expect("positive content limit")
    );
    assert!(
        block_gas_limit_for_payload(PACKET_LOSS_PAYLOAD_BYTES)
            > u64::try_from(PACKET_LOSS_PAYLOAD_BYTES).expect("packet-loss fixture fits u64")
    );
}
#[test]
fn route_retry_only_matches_explicit_pre_admission_unavailability() {
    assert!(is_route_unavailable_submission(&eyre!(
        "503 Service Unavailable; reject code: route_unavailable"
    )));
    assert!(!is_route_unavailable_submission(&eyre!(
        "422 Unprocessable Entity; reject code: fee_quote_rejected"
    )));
    assert!(!is_route_unavailable_submission(&eyre!(
        "connection reset before a transaction response"
    )));
}
#[test]
fn da_route_authority_genesis_binds_every_peer_once() {
    let peers = (0..4)
        .map(|index| {
            let key_pair = KeyPair::try_from_seed(
                format!("integration_tests::sumeragi_da::test-peer::{index}").into_bytes(),
                Algorithm::BlsNormal,
            )
            .expect("derive checked DA test-peer key");
            PeerId::new(key_pair.public_key().clone())
        })
        .collect::<Vec<_>>();
    let transactions = da_route_authority_genesis_transactions(&peers);
    assert_eq!(transactions.len(), 2);
    assert_eq!(transactions[0].len(), peers.len() * 2 + 2);
    assert_eq!(transactions[1].len(), peers.len() * 2);
    assert_eq!(
        da_stake_asset_definition_id(),
        AssetDefinitionId::derive_from_components(
            DomainId::try_new("nexus", "universal").expect("DA stake domain"),
            "xor".parse().expect("DA stake asset name"),
        )
    );

    let registrations = transactions[1]
        .iter()
        .filter_map(|instruction| {
            instruction
                .as_any()
                .downcast_ref::<RegisterPublicLaneValidator>()
        })
        .collect::<Vec<_>>();
    let activations = transactions[1]
        .iter()
        .filter_map(|instruction| {
            instruction
                .as_any()
                .downcast_ref::<ActivatePublicLaneValidator>()
        })
        .collect::<Vec<_>>();
    assert_eq!(registrations.len(), peers.len());
    assert_eq!(activations.len(), peers.len());
    for (index, (registration, peer_id)) in registrations.iter().zip(&peers).enumerate() {
        let validator_id = da_validator_account_id(index);
        assert_eq!(registration.lane_id, LaneId::SINGLE);
        assert_eq!(registration.validator, validator_id);
        assert_eq!(registration.stake_account, validator_id);
        assert_eq!(&registration.peer_id, peer_id);
        assert_eq!(
            registration.initial_stake,
            Quantity::from(DA_VALIDATOR_STAKE)
        );
    }
}
