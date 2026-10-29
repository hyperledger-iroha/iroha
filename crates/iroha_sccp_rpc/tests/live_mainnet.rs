//! Live mainnet checks of the light-client builders (`specs/sccp.md` §4.13).
//!
//! Each test builds a bootstrap from the compiled default public endpoints, installs it into an
//! in-memory light client exactly as core applies `InitializeLightClient`, then builds the next
//! advance and applies it with the compiled verifier, as the zero-touch keeper does. Where no
//! SCCP deployment exists yet, a source proof of an ordinary mainnet transaction must pass every
//! anchor, header and inclusion check and fail only at the SCCP event. They read public mainnet
//! endpoints, so they are ignored by default. To retain the exact verifier inputs and clocks:
//!
//! ```text
//! SCCP_LIVE_CAPTURE_DIR=target/qualification/sccp-blockers/live-captures \
//!   cargo test -p iroha_sccp_rpc --test live_mainnet bsc_bootstrap_and_advance_verify -- --ignored --nocapture
//! SCCP_LIVE_CAPTURE_DIR=target/qualification/sccp-blockers/live-captures \
//!   cargo test -p iroha_sccp_rpc --test live_mainnet ton_bootstrap_and_advance_verify -- --ignored --nocapture
//! SCCP_LIVE_REPLAY_FILE=<printed-capture-path> \
//!   cargo test -p iroha_sccp_rpc --test live_mainnet replay_captured_verifier_inputs -- --ignored --nocapture
//! ```
//!
//! Captures are local Norito JSON artifacts containing canonical proof frames, not raw RPC
//! transcripts. Replay checks the recorded profile, default limits and exact verifier clocks.
//! The TON key-block hop uses an explicitly recorded historical Taira clock; the newest
//! bootstrap is separately checked at capture time. Neither check authenticates Parliament's
//! bootstrap selection, a deployed SCCP route or a monetary operation. Generated captures must
//! stay untracked. The environment variables above control only these developer tests.

use std::{
    fs::{self, OpenOptions},
    io::{Read as _, Write as _},
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use iroha_config::parameters::defaults::sccp::endpoints;
use iroha_data_model::{
    bridge::SccpNetworkV1,
    sccp::{
        inbound::SccpSourceProofBytesV1,
        light_client::{
            SccpLcAdvanceBytesV1, SccpLcBootstrapV1, SccpLcHeadV1, SccpLcInitExpectationV1,
            SccpLightClientParamsV1,
        },
    },
};
use iroha_sccp::light_client::{
    self, SccpLcBootstrapDataV1, SccpLcError, SccpLcStateView,
    bsc::{BscLcBootstrapV1, BscLcError},
    ethereum::EthereumLcError,
    profile::{SccpChainProfilesV1, SccpLcProfileCatalogV1, SccpLcProfileRefV1},
    state::SccpLcMemoryStateV1,
};
use iroha_sccp_rpc::{
    BeaconClient, EndpointSet, EvmClient, FailoverPolicy, HttpConfig, HttpTransport,
    builders::{
        AdvanceBudgetV1, BuildError, LightClientReplayV1, SourceChainBuilder, SourceEventRefV1,
        SourceEvidenceV1, TairaLightClientView,
        bsc::{BscBuilder, BscSource},
        ethereum::EthereumBuilder,
        ethereum::EthereumEventV1,
        ton::TonBuilder,
    },
    evm::{BlockId, BlockTag},
    ton::{LiteClient, LiteClientConfig, LiteServerSet},
};

#[derive(Clone, Debug, norito::derive::JsonSerialize, norito::derive::JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct CapturedEvidence {
    taira_now_ms: u64,
    backfills: Vec<SccpLcAdvanceBytesV1>,
    proof: SccpSourceProofBytesV1,
}

#[derive(Clone, Debug, norito::derive::JsonSerialize, norito::derive::JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct CapturedFreshness {
    bootstrap: SccpLcBootstrapV1,
    taira_now_ms: u64,
}

#[derive(Clone, Debug, norito::derive::JsonSerialize, norito::derive::JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct CapturedRun {
    format_version: u32,
    captured_at_ms: u64,
    profile_version: u32,
    profile_hash: [u8; 32],
    params: SccpLightClientParamsV1,
    bootstrap: SccpLcBootstrapV1,
    bootstrap_taira_now_ms: u64,
    advance: SccpLcAdvanceBytesV1,
    advance_taira_now_ms: u64,
    minimum_finalized_height: Option<u64>,
    final_head: SccpLcHeadV1,
    freshness_check: Option<CapturedFreshness>,
    evidence: Option<CapturedEvidence>,
}

impl CapturedRun {
    /// Replay the recorded proof frames under their compiled profile and unmodified defaults.
    fn replay(&self) -> Result<LightClientReplayV1, String> {
        if self.format_version != 1 {
            return Err("unsupported live capture format".into());
        }
        let network = self.bootstrap.network;
        if Some(self.params) != SccpLightClientParamsV1::defaults_for(network) {
            return Err("capture changed the default verifier limits".into());
        }
        let catalog = SccpLcProfileCatalogV1::compiled();
        let profiles = catalog
            .resolve(&catalog.genesis().with(
                network,
                SccpLcProfileRefV1 {
                    version: self.profile_version,
                    profile_hash: self.profile_hash,
                },
            ))
            .map_err(|error| format!("capture profile: {error:?}"))?;
        if let Some(freshness) = &self.freshness_check {
            light_client::verify_bootstrap_with_profiles(
                &profiles,
                network,
                &self.params,
                &freshness.bootstrap,
                freshness.taira_now_ms,
            )
            .map_err(|error| format!("capture newest-bootstrap freshness: {error}"))?;
        }
        let initial = light_client::initialize_light_client_with_profiles(
            &profiles,
            &SccpLcMemoryStateV1::new(),
            network,
            SccpLcInitExpectationV1::Absent,
            &self.params,
            &self.bootstrap,
            self.bootstrap_taira_now_ms,
        )
        .map_err(|error| format!("capture bootstrap: {error}"))?;
        let initial_height = initial.light_client.head.latest_finalized.source_height;
        let mut replay = LightClientReplayV1::installed(&profiles, network, &initial);
        replay
            .advance(&self.advance, self.advance_taira_now_ms)
            .map_err(|error| format!("capture advance: {error}"))?;
        let head = replay
            .light_client()
            .map_err(|error| error.to_string())?
            .head;
        if head != self.final_head {
            return Err("capture replay produced another finalized head".into());
        }
        if let Some(minimum) = self.minimum_finalized_height
            && (head.latest_finalized.source_height <= initial_height
                || head.latest_finalized.source_height < minimum)
        {
            return Err("capture did not advance to the required finalized checkpoint".into());
        }
        if let Some(evidence) = &self.evidence {
            let evidence_bytes = SourceEvidenceV1 {
                backfills: evidence.backfills.clone(),
                proof: evidence.proof.clone(),
            };
            if !matches!(
                replay.verify_evidence(&evidence_bytes, evidence.taira_now_ms),
                Err(SccpLcError::Ethereum(EthereumLcError::Event(_)))
                    | Err(SccpLcError::Bsc(BscLcError::Receipt(
                        EthereumLcError::Event(_)
                    )))
            ) {
                return Err(
                    "capture must pass inclusion and fail only at its non-SCCP event".into(),
                );
            }
        }
        Ok(replay)
    }

    /// Persist only a run that can be independently replayed; never replace an earlier capture.
    fn write_to(&self, directory: &Path) -> PathBuf {
        self.replay().expect("capture replays before writing");
        fs::create_dir_all(directory).expect("create capture directory");
        let path = directory.join(format!(
            "{}-{}-{}.json",
            self.bootstrap.network.profile_key(),
            self.captured_at_ms,
            std::process::id()
        ));
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .expect("create a new capture file");
        file.write_all(
            norito::json::to_json(self)
                .expect("encode capture")
                .as_bytes(),
        )
        .expect("write capture");
        file.sync_all().expect("persist capture");
        path
    }

    fn capture_if_requested(&self) {
        if let Some(directory) = std::env::var_os("SCCP_LIVE_CAPTURE_DIR") {
            let path = self.write_to(Path::new(&directory));
            println!("verified input capture: {}", path.display());
        }
    }
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| {
            u64::try_from(elapsed.as_millis()).unwrap_or(u64::MAX)
        })
}

fn transport(urls: &[&str]) -> HttpTransport {
    let endpoints = EndpointSet::parse(urls, &[]).expect("compiled endpoints parse");
    HttpTransport::new(endpoints, HttpConfig::default(), FailoverPolicy::default())
        .expect("transport")
}

/// Install `bootstrap` and require one accepted advance under the keeper's default budget.
/// When given a target height, the finalized head must move forward and reach that checkpoint.
fn bootstrap_then_advance(
    network: SccpNetworkV1,
    bootstrap: &SccpLcBootstrapV1,
    minimum_finalized_height: Option<u64>,
    now: impl Fn() -> u64,
    advance: impl FnOnce(u64, AdvanceBudgetV1) -> Result<SccpLcAdvanceBytesV1, BuildError>,
) -> (LightClientReplayV1, CapturedRun) {
    let params = SccpLightClientParamsV1::defaults_for(network).expect("defaults");
    let profiles = SccpChainProfilesV1::latest();
    let bootstrap_taira_now_ms = now();
    let initial = light_client::initialize_light_client_with_profiles(
        profiles,
        &SccpLcMemoryStateV1::new(),
        network,
        SccpLcInitExpectationV1::Absent,
        &params,
        bootstrap,
        bootstrap_taira_now_ms,
    )
    .expect("the live bootstrap verifies");
    let mut replay = LightClientReplayV1::installed(profiles, network, &initial);
    let installed = replay.light_client().expect("installed");
    println!(
        "{}: bootstrap {} bytes, set {}, head {:?}",
        network.profile_key(),
        bootstrap.bytes.len(),
        installed.head.latest_set_id,
        installed.head
    );
    let budget = AdvanceBudgetV1::for_params(&params, 262_144);
    let advance = advance(installed.head.latest_set_id, budget)
        .expect("the live test requires an advance; unavailable is not a pass");
    let advance_taira_now_ms = now();
    let delta = replay
        .advance(&advance, advance_taira_now_ms)
        .expect("the live advance verifies");
    let final_head = replay.light_client().expect("installed").head;
    println!(
        "{}: advance {} bytes, {} new sets, head {final_head:?}",
        network.profile_key(),
        advance.len(),
        delta.new_sets.len()
    );
    let catalog = SccpLcProfileCatalogV1::compiled();
    let version = catalog.latest_version(network);
    let captured = CapturedRun {
        format_version: 1,
        captured_at_ms: now_ms(),
        profile_version: version,
        profile_hash: catalog
            .profile_hash(network, version)
            .expect("compiled profile"),
        params,
        bootstrap: bootstrap.clone(),
        bootstrap_taira_now_ms,
        advance,
        advance_taira_now_ms,
        minimum_finalized_height,
        final_head,
        freshness_check: None,
        evidence: None,
    };
    captured
        .replay()
        .expect("verified advance reaches the required checkpoint");
    (replay, captured)
}

/// Bootstrap one completed epoch behind the latest finalized epoch, so an advance already exists.
fn bsc_bootstrap_checkpoints(finalized: u64, epoch: u64) -> Option<[u64; 2]> {
    let latest = finalized.checked_div(epoch)?.checked_mul(epoch)?;
    let checkpoint = latest.checked_sub(epoch)?;
    Some([checkpoint.checked_sub(epoch)?, checkpoint])
}

/// The first successful receipt with a log in EVM block `number`.
fn receipt_with_log(client: &EvmClient, number: u64) -> [u8; 32] {
    client
        .block_receipts(BlockId::Tag(BlockTag::Number(number)))
        .expect("receipts")
        .expect("served")
        .into_iter()
        .find(|receipt| receipt.status == Some(1) && !receipt.logs.is_empty())
        .expect("a successful receipt with a log")
        .transaction_hash
}

#[test]
#[ignore = "reads public Ethereum mainnet endpoints"]
fn ethereum_bootstrap_and_advance_verify() {
    let execution = EvmClient::new(transport(endpoints::ETHEREUM_EXECUTION));
    let beacon = BeaconClient::new(transport(endpoints::ETHEREUM_BEACON));
    // The finalized state's own finalized checkpoint is earlier and has a retained
    // sync-committee branch; an arbitrary parent block need not have a bootstrap.
    let checkpoints = beacon
        .transport()
        .get_json("/eth/v1/beacon/states/finalized/finality_checkpoints")
        .expect("previous finalized checkpoint");
    let earlier_root = iroha_sccp_rpc::evm::parse_data_array::<32>(
        checkpoints
            .get("data")
            .and_then(|data| data.get("finalized"))
            .and_then(|checkpoint| checkpoint.get("root"))
            .and_then(norito::json::Value::as_str)
            .expect("finalized checkpoint root"),
    )
    .expect("canonical checkpoint root");
    let builder = EthereumBuilder::new(
        iroha_sccp::light_client::profile::SccpChainProfilesV1::latest().ethereum,
        beacon,
        execution,
    );
    // The checkpoint is already finalized, so a successful no-op refresh cannot substitute
    // for the finalized progress this live test is meant to verify.
    let bootstrap = builder
        .bootstrap_at(&earlier_root)
        .expect("earlier bootstrap");
    let params = SccpLightClientParamsV1::defaults_for(SccpNetworkV1::EthereumMainnet).unwrap();
    let minimum = light_client::verify_bootstrap(
        SccpNetworkV1::EthereumMainnet,
        &params,
        &bootstrap,
        now_ms(),
    )
    .expect("earlier bootstrap verifies")
    .light_client
    .head
    .latest_finalized
    .source_height
    .checked_add(1)
    .expect("next finalized execution height");
    let (replay, mut captured) = bootstrap_then_advance(
        SccpNetworkV1::EthereumMainnet,
        &bootstrap,
        Some(minimum),
        now_ms,
        |latest, budget| builder.advance(latest, budget),
    );
    let execution = EvmClient::new(transport(endpoints::ETHEREUM_EXECUTION));
    let finalized = execution
        .block_by_number(BlockTag::Finalized)
        .expect("finalized block")
        .expect("served")
        .header
        .number;
    let tx = receipt_with_log(&execution, finalized - 8);
    let event = SourceEventRefV1::Evm {
        tx_hash: tx,
        event: EthereumEventV1::TransferToTaira { log_index: 0 },
    };
    let evidence = builder
        .evidence(&event, &replay, now_ms())
        .expect("the proof builds");
    let evidence_taira_now_ms = now_ms();
    let error = replay
        .verify_evidence(&evidence, evidence_taira_now_ms)
        .expect_err("an ordinary log is not an SCCP event");
    println!(
        "ethereum-mainnet: proof {} bytes, {error}",
        evidence.proof.len()
    );
    assert!(
        matches!(error, SccpLcError::Ethereum(EthereumLcError::Event(_))),
        "the proof must fail only at the event: {error:?}"
    );
    captured.evidence = Some(CapturedEvidence {
        taira_now_ms: evidence_taira_now_ms,
        backfills: evidence.backfills,
        proof: evidence.proof,
    });
    captured.capture_if_requested();
}

#[test]
#[ignore = "reads public BNB Smart Chain mainnet endpoints"]
fn bsc_bootstrap_and_advance_verify() {
    let profile = SccpChainProfilesV1::latest().bsc;
    let builder = BscBuilder::new(profile, EvmClient::new(transport(endpoints::BSC)));
    let checkpoints = bsc_bootstrap_checkpoints(
        builder.finalized_number().expect("finalized height"),
        profile.epoch_length,
    )
    .expect("two earlier finalized checkpoints exist");
    let target = checkpoints[1] + profile.epoch_length;
    let mut headers = builder
        .headers_at(&checkpoints)
        .expect("checkpoint headers")
        .into_iter();
    let bootstrap = SccpLcBootstrapDataV1::Bsc(BscLcBootstrapV1 {
        previous_checkpoint_header: headers.next().expect("previous checkpoint"),
        checkpoint_header: headers.next().expect("bootstrap checkpoint"),
    })
    .to_bootstrap()
    .expect("bootstrap frame");
    let (replay, mut captured) = bootstrap_then_advance(
        SccpNetworkV1::BscMainnet,
        &bootstrap,
        Some(target),
        now_ms,
        |latest, budget| builder.advance(latest, budget),
    );
    let head = replay.light_client().expect("installed").head;
    assert!(
        replay
            .state()
            .consensus_set(SccpNetworkV1::BscMainnet, head.latest_set_id)
            .is_some(),
        "the newest set is stored"
    );
    let rpc = EvmClient::new(transport(endpoints::BSC));
    let tx = receipt_with_log(&rpc, head.latest_finalized.source_height - 4);
    let event = SourceEventRefV1::Evm {
        tx_hash: tx,
        event: EthereumEventV1::TransferToTaira { log_index: 0 },
    };
    let evidence = builder
        .evidence(&event, &replay, now_ms())
        .expect("the proof builds");
    let evidence_taira_now_ms = now_ms();
    let error = replay
        .verify_evidence(&evidence, evidence_taira_now_ms)
        .expect_err("an ordinary log is not an SCCP event");
    println!("bsc-mainnet: proof {} bytes, {error}", evidence.proof.len());
    assert!(
        matches!(
            error,
            SccpLcError::Bsc(BscLcError::Receipt(EthereumLcError::Event(_)))
        ),
        "the proof must fail only at the event: {error:?}"
    );
    captured.evidence = Some(CapturedEvidence {
        taira_now_ms: evidence_taira_now_ms,
        backfills: evidence.backfills,
        proof: evidence.proof,
    });
    captured.capture_if_requested();
}

#[test]
#[ignore = "reads public TON mainnet liteservers"]
fn ton_bootstrap_and_advance_verify() {
    let builder = TonBuilder::new(
        iroha_sccp::light_client::profile::SccpChainProfilesV1::latest().ton,
        LiteClient::new(
            LiteServerSet::compiled_defaults(),
            LiteClientConfig::default(),
            FailoverPolicy::default(),
        ),
    );
    let newest = builder.newest_key_block().expect("newest key block");
    // Replay the hop into the newest key block at a Taira time just after it, while the earlier
    // epoch was still fresh, so the hop verifier runs on real signatures and config proofs.
    let params =
        SccpLightClientParamsV1::defaults_for(SccpNetworkV1::TonMainnet).expect("defaults");
    let newest_bootstrap = builder.bootstrap_at(newest).expect("newest bootstrap");
    let freshness_taira_now_ms = now_ms();
    let newest_time = light_client::verify_bootstrap_with_profiles(
        SccpChainProfilesV1::latest(),
        SccpNetworkV1::TonMainnet,
        &params,
        &newest_bootstrap,
        freshness_taira_now_ms,
    )
    .expect("fresh")
    .light_client
    .head
    .latest_finalized
    .source_time_ms;
    let previous = builder
        .previous_key_block(newest)
        .expect("previous key block");
    let (replay, mut captured) = bootstrap_then_advance(
        SccpNetworkV1::TonMainnet,
        &builder.bootstrap_at(previous).expect("earlier bootstrap"),
        Some(u64::from(newest)),
        || newest_time + 60_000,
        |latest, budget| builder.advance(latest, budget),
    );
    let reached = replay.light_client().expect("installed").head.latest_set_id;
    assert!(
        u64::from(newest) <= reached,
        "the hop reaches the newest key block"
    );
    captured.freshness_check = Some(CapturedFreshness {
        bootstrap: newest_bootstrap,
        taira_now_ms: freshness_taira_now_ms,
    });
    captured.capture_if_requested();
}

#[test]
#[ignore = "reads a locally retained verifier-input capture; no network access"]
fn replay_captured_verifier_inputs() {
    let path = std::env::var_os("SCCP_LIVE_REPLAY_FILE").expect("set SCCP_LIVE_REPLAY_FILE");
    let mut json = String::new();
    // Bound this developer input too; a larger capture needs explicit review before replay.
    const MAX_CAPTURE_BYTES: u64 = 16 * 1024 * 1024;
    fs::File::open(&path)
        .expect("open capture")
        .take(MAX_CAPTURE_BYTES + 1)
        .read_to_string(&mut json)
        .expect("read capture");
    assert!(
        json.len() as u64 <= MAX_CAPTURE_BYTES,
        "capture is too large"
    );
    let captured: CapturedRun = norito::json::from_json(&json).expect("decode capture");
    captured.replay().expect("captured verifier inputs verify");
}

#[test]
fn earlier_bsc_bootstrap_always_leaves_a_finalized_epoch_to_advance() {
    assert_eq!(bsc_bootstrap_checkpoints(1_999, 1_000), None);
    assert_eq!(bsc_bootstrap_checkpoints(2_000, 1_000), Some([0, 1_000]));
    assert_eq!(
        bsc_bootstrap_checkpoints(3_999, 1_000),
        Some([1_000, 2_000])
    );
    assert_eq!(bsc_bootstrap_checkpoints(u64::MAX, 0), None);
    for finalized in [4_000, 4_001, 4_999, u64::MAX] {
        let pair = bsc_bootstrap_checkpoints(finalized, 1_000).expect("earlier epochs");
        assert_eq!(pair[0] + 1_000, pair[1]);
        assert_eq!(pair[1] + 1_000, finalized / 1_000 * 1_000);
    }
}

#[test]
fn captured_advance_roundtrips_and_rejects_altered_verifier_inputs() {
    use iroha_sccp::{
        light_client::{SccpLcAdvanceV1, bsc::BscLcAdvanceV1},
        test_support::bsc::SyntheticParliaChainV1,
    };

    let chain = SyntheticParliaChainV1::new([0x79; 32], 4, 16);
    let advance = SccpLcAdvanceV1::Bsc(BscLcAdvanceV1 {
        steps: vec![chain.step(3_000, 3_001, 2_000, None)],
    })
    .to_bytes()
    .expect("synthetic advance");
    let (_, captured) = bootstrap_then_advance(
        SccpNetworkV1::BscMainnet,
        &chain.bootstrap(2),
        Some(3_000),
        || SyntheticParliaChainV1::time_ms(3_002),
        |_, _| Ok(advance),
    );
    let json = norito::json::to_json(&captured).expect("encode capture");
    let decoded: CapturedRun = norito::json::from_json(&json).expect("decode capture");
    assert_eq!(
        decoded
            .replay()
            .expect("offline replay")
            .light_client()
            .expect("installed")
            .head,
        captured.final_head
    );

    // Re-encode a canonical outer frame after changing an aggregate-signature byte: replay
    // must run source verification, rather than trusting a captured success marker or head.
    let mut altered = captured.clone();
    let SccpLcAdvanceV1::Bsc(mut advance) =
        SccpLcAdvanceV1::from_frame(altered.advance.as_bytes()).expect("decode advance")
    else {
        panic!("BSC advance");
    };
    advance.steps[0].finality.attestation[20] ^= 1;
    altered.advance = SccpLcAdvanceV1::Bsc(advance)
        .to_bytes()
        .expect("canonical frame");
    assert!(altered.replay().is_err(), "altered signature must fail");

    let mut altered = captured.clone();
    altered.profile_hash[0] ^= 1;
    assert!(altered.replay().is_err(), "profile identity must match");
    let mut altered = captured.clone();
    altered.params.ws_bound_ms *= 2;
    assert!(altered.replay().is_err(), "capture cannot widen validity");
    let mut altered = captured.clone();
    altered.advance_taira_now_ms += altered.params.ws_bound_ms + 1;
    assert!(altered.replay().is_err(), "the recorded clock is enforced");
    let mut altered = captured.clone();
    altered.minimum_finalized_height = Some(4_000);
    assert!(altered.replay().is_err(), "an unreached target cannot pass");

    let directory = std::env::temp_dir().join(format!(
        "iroha-sccp-live-capture-test-{}-{}",
        std::process::id(),
        now_ms()
    ));
    let path = captured.write_to(&directory);
    let saved: CapturedRun =
        norito::json::from_json(&fs::read_to_string(&path).expect("read capture"))
            .expect("decode saved capture");
    saved.replay().expect("saved verifier inputs replay");
    fs::remove_dir_all(directory).expect("remove temporary capture");
}

#[test]
fn unavailable_advance_cannot_pass_the_live_gate() {
    use iroha_sccp::test_support::bsc::SyntheticParliaChainV1;

    let chain = SyntheticParliaChainV1::new([0x7a; 32], 4, 16);
    let bootstrap = chain.bootstrap(2);
    let result = std::panic::catch_unwind(|| {
        bootstrap_then_advance(
            SccpNetworkV1::BscMainnet,
            &bootstrap,
            Some(3_000),
            || SyntheticParliaChainV1::time_ms(3_002),
            |_, _| Err(BuildError::Unavailable("no later checkpoint".into())),
        )
    });
    assert!(
        result.is_err(),
        "unavailability must fail the qualification test"
    );
}
