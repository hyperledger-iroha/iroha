use clap::Subcommand;
use eyre::Result;
use iroha::{
    client::{SccpCapabilities, SccpProofManifestSet},
    data_model::prelude::*,
};

use crate::{CliOutputFormat, Run, RunContext};

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Emit a bridge receipt as a typed event
    EmitReceipt(EmitReceiptArgs),
    /// Inspect generic SCCP proof-discovery and artifact surfaces
    #[command(subcommand)]
    Sccp(SccpCommand),
}

#[derive(Subcommand, Debug)]
pub enum SccpCommand {
    /// Fetch the public SCCP capability snapshot
    Capabilities,
    /// Fetch SCCP chain-specific proof manifests
    Manifests,
    /// Fetch a typed SCCP message proof artifact by message id
    Artifact(ArtifactArgs),
}

#[derive(clap::Args, Debug)]
pub struct EmitReceiptArgs {
    /// Bridge lane id (numeric)
    #[arg(long)]
    lane: u32,
    /// Direction: lock|mint|burn|release
    #[arg(long)]
    direction: String,
    /// Source tx hash (hex, 32 bytes)
    #[arg(long)]
    source_tx: String,
    /// Amount (integer units)
    #[arg(long)]
    amount: u128,
    /// Asset id (Iroha canonical), e.g., "wBTC#btc"
    #[arg(long)]
    asset_id: String,
    /// Recipient (Iroha account id or external address payload)
    #[arg(long)]
    recipient: String,
    /// Optional destination tx hash (hex, 32 bytes)
    #[arg(long)]
    dest_tx: Option<String>,
    /// Proof hash (hex, 32 bytes)
    #[arg(long)]
    proof_hash: Option<String>,
}

#[derive(clap::Args, Debug)]
pub struct ArtifactArgs {
    /// SCCP message id (hex, 32 bytes)
    #[arg(long, value_name = "HEX")]
    message_id: String,
}

impl Run for Command {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        match self {
            Command::EmitReceipt(args) => emit_receipt(context, args),
            Command::Sccp(cmd) => match cmd {
                SccpCommand::Capabilities => sccp_capabilities(context),
                SccpCommand::Manifests => sccp_manifests(context),
                SccpCommand::Artifact(args) => sccp_artifact(context, args),
            },
        }
    }
}

fn hex32(s: &str) -> Result<[u8; 32]> {
    let bytes = hex::decode(s.trim_start_matches("0x"))?;
    let mut out = [0u8; 32];
    if bytes.len() != 32 {
        return Err(eyre::eyre!("expected 32 bytes, got {}", bytes.len()));
    }
    out.copy_from_slice(&bytes);
    Ok(out)
}

fn emit_receipt(ctx: &mut impl RunContext, a: EmitReceiptArgs) -> Result<()> {
    let source_tx = hex32(&a.source_tx)?;
    let dest_tx = match &a.dest_tx {
        Some(h) => Some(hex32(h)?),
        None => None,
    };
    let proof_hash = match &a.proof_hash {
        Some(h) => hex32(h)?,
        None => [0u8; 32],
    };
    let receipt = BridgeReceipt {
        lane: LaneId::new(a.lane),
        direction: a.direction.into_bytes(),
        source_tx,
        dest_tx,
        proof_hash,
        amount: a.amount,
        asset_id: a.asset_id.into_bytes(),
        recipient: a.recipient.into_bytes(),
    };
    let isi = RecordBridgeReceipt::new(receipt);
    ctx.finish(vec![InstructionBox::from(isi)])
}

fn sccp_capabilities(ctx: &mut impl RunContext) -> Result<()> {
    let capabilities = ctx.client_from_config().get_sccp_capabilities()?;
    match ctx.output_format() {
        CliOutputFormat::Text => ctx.println(render_sccp_capabilities_summary(&capabilities)),
        CliOutputFormat::Json => ctx.print_data(&capabilities),
    }
}

fn sccp_manifests(ctx: &mut impl RunContext) -> Result<()> {
    match ctx.output_format() {
        CliOutputFormat::Text => {
            let manifests = ctx.client_from_config().get_sccp_proof_manifests()?;
            ctx.println(render_sccp_manifests_summary(&manifests))
        }
        CliOutputFormat::Json => {
            let manifests = ctx.client_from_config().get_sccp_proof_manifests_json()?;
            ctx.print_data(&manifests)
        }
    }
}

fn sccp_artifact(ctx: &mut impl RunContext, args: ArtifactArgs) -> Result<()> {
    match ctx.output_format() {
        CliOutputFormat::Text => {
            let artifact = ctx
                .client_from_config()
                .get_sccp_message_proof_artifact(&args.message_id)?;
            ctx.println(render_sccp_artifact_summary(&artifact))
        }
        CliOutputFormat::Json => {
            let artifact = ctx
                .client_from_config()
                .get_sccp_message_proof_artifact_json(&args.message_id)?;
            ctx.print_data(&artifact)
        }
    }
}

fn render_sccp_capabilities_summary(capabilities: &SccpCapabilities) -> String {
    let payloads = capabilities.message_payload_kinds.join(",");
    let codecs = capabilities
        .codecs
        .iter()
        .map(|codec| codec.key.as_str())
        .collect::<Vec<_>>()
        .join(",");
    let counterparties = capabilities
        .counterparties
        .iter()
        .map(|counterparty| {
            format!(
                "{}({}:{})",
                counterparty.chain,
                counterparty.domain,
                counterparty.counterparty_account_codec_key
            )
        })
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        "sccp capabilities: local={}({}) proof_family={} payloads={} codecs={} artifact={} manifests={} counterparties=[{}]",
        capabilities.local_chain,
        capabilities.local_domain,
        capabilities.proof_family,
        payloads,
        codecs,
        capabilities.message_proof_path,
        capabilities.proof_manifest_path,
        counterparties
    )
}

fn render_sccp_manifests_summary(manifests: &SccpProofManifestSet) -> String {
    let mut lines = vec![format!(
        "sccp manifests: local={}({}) proof_family={} count={}",
        manifests.local_chain,
        manifests.local_domain,
        manifests.proof_family,
        manifests.manifests.len()
    )];
    lines.extend(manifests.manifests.iter().map(|manifest| {
        format!(
            "chain={} domain={} backend={} registry={} finality={:?} verifier={:?} codec={}",
            manifest.chain,
            manifest.counterparty_domain,
            manifest.message_backend,
            manifest.registry_backend,
            manifest.finality_model,
            manifest.verifier_target,
            manifest.counterparty_account_codec_key
        )
    }));
    lines.join("\n")
}

fn render_sccp_artifact_summary(
    artifact: &iroha_sccp::NexusSccpMessageTransparentProofV1,
) -> String {
    format!(
        "sccp artifact: message_id={} payload={} chain={}({}) backend={} proof_family={} finality_height={} commitment_root={}",
        hex::encode(artifact.public_inputs.message_id),
        sccp_payload_kind_key(&artifact.bundle.payload),
        iroha_sccp::sccp_chain_key_for_domain(artifact.counterparty_domain).unwrap_or("unknown"),
        artifact.counterparty_domain,
        artifact.message_backend,
        artifact.proof_family,
        artifact.public_inputs.finality_height,
        hex::encode(artifact.public_inputs.commitment_root)
    )
}

fn sccp_payload_kind_key(payload: &iroha_sccp::SccpPayloadV1) -> &'static str {
    match payload {
        iroha_sccp::SccpPayloadV1::AssetRegister(_) => "asset_register",
        iroha_sccp::SccpPayloadV1::RouteActivate(_) => "route_activate",
        iroha_sccp::SccpPayloadV1::Transfer(_) => "transfer",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use iroha::client::{SccpCodecCapability, SccpCounterpartyCapability};
    use iroha_i18n::{Bundle, Language, Localizer};
    use norito::json::{JsonSerialize, Value as JsonValue};
    use std::{
        collections::BTreeMap,
        io::Write,
        net::{TcpListener, TcpStream},
        sync::{
            Arc,
            atomic::{AtomicBool, Ordering},
        },
        thread,
        time::Duration,
    };
    use url::Url;

    struct TestContext {
        cfg: iroha::config::Config,
        i18n: Localizer,
        captured: Option<Executable>,
        output_format: CliOutputFormat,
        printed_json: Option<JsonValue>,
        printed_lines: Vec<String>,
    }

    impl TestContext {
        fn new() -> Self {
            Self::with_base_url(
                CliOutputFormat::Json,
                Url::parse("http://127.0.0.1/").unwrap(),
            )
        }

        fn with_base_url(output_format: CliOutputFormat, torii_api_url: Url) -> Self {
            let key_pair = KeyPair::random_with_algorithm(Algorithm::Ed25519);
            let account_id = AccountId::new(key_pair.public_key().clone());
            let cfg = iroha::config::Config {
                chain: ChainId::from("00000000-0000-0000-0000-000000000000"),
                account: account_id,
                account_chain_discriminant:
                    iroha_config::parameters::defaults::common::chain_discriminant(),
                key_pair,
                basic_auth: None,
                torii_api_url,
                torii_api_version: iroha::config::default_torii_api_version(),
                torii_api_min_proof_version: iroha::config::DEFAULT_TORII_API_MIN_PROOF_VERSION
                    .to_string(),
                torii_request_timeout: iroha::config::DEFAULT_TORII_REQUEST_TIMEOUT,
                transaction_ttl: iroha::config::DEFAULT_TRANSACTION_TIME_TO_LIVE,
                transaction_status_timeout: iroha::config::DEFAULT_TRANSACTION_STATUS_TIMEOUT,
                transaction_add_nonce: iroha::config::DEFAULT_TRANSACTION_NONCE,
                connect_queue_root: iroha::config::default_connect_queue_root(),
                soracloud_http_witness_file: None,
                sorafs_alias_cache: crate::config_utils::default_alias_cache_policy(),
                sorafs_anonymity_policy: crate::config_utils::default_anonymity_policy(),
                sorafs_rollout_phase: crate::config_utils::default_rollout_phase(),
            };
            Self {
                cfg,
                i18n: Localizer::new(Bundle::Cli, Language::English),
                captured: None,
                output_format,
                printed_json: None,
                printed_lines: Vec::new(),
            }
        }
    }

    impl RunContext for TestContext {
        fn config(&self) -> &iroha::config::Config {
            &self.cfg
        }

        fn transaction_metadata(&self) -> Option<&Metadata> {
            None
        }

        fn input_instructions(&self) -> bool {
            false
        }

        fn output_instructions(&self) -> bool {
            false
        }

        fn i18n(&self) -> &Localizer {
            &self.i18n
        }

        fn output_format(&self) -> CliOutputFormat {
            self.output_format
        }

        fn print_data<T>(&mut self, data: &T) -> Result<()>
        where
            T: JsonSerialize + ?Sized,
        {
            self.printed_json = Some(norito::json::to_value(data)?);
            Ok(())
        }

        fn println(&mut self, data: impl std::fmt::Display) -> Result<()> {
            self.printed_lines.push(data.to_string());
            Ok(())
        }

        fn finish_with_mode(
            &mut self,
            instructions: impl Into<Executable>,
            _wait_for_confirmation: bool,
        ) -> Result<()> {
            self.captured = Some(instructions.into());
            Ok(())
        }
    }

    #[derive(Clone)]
    struct MockHttpResponse {
        content_type: &'static str,
        body: Vec<u8>,
    }

    struct MockHttpServer {
        base_url: Url,
        address: String,
        stop: Arc<AtomicBool>,
        handle: Option<thread::JoinHandle<()>>,
    }

    impl MockHttpServer {
        fn start(routes: BTreeMap<String, MockHttpResponse>) -> Self {
            let listener = TcpListener::bind("127.0.0.1:0").expect("bind mock http server");
            listener
                .set_nonblocking(true)
                .expect("set mock listener nonblocking");
            let address = listener
                .local_addr()
                .expect("mock listener address")
                .to_string();
            let base_url = Url::parse(&format!("http://{address}")).expect("mock base url");
            let stop = Arc::new(AtomicBool::new(false));
            let stop_flag = Arc::clone(&stop);
            let handle = thread::spawn(move || {
                while !stop_flag.load(Ordering::SeqCst) {
                    match listener.accept() {
                        Ok((mut stream, _)) => {
                            let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
                            let path = read_mock_http_request_path(&mut stream);
                            if stop_flag.load(Ordering::SeqCst) && path.is_empty() {
                                continue;
                            }
                            let response = routes.get(&path).cloned().unwrap_or(MockHttpResponse {
                                content_type: "text/plain",
                                body: b"not found".to_vec(),
                            });
                            let status = if routes.contains_key(&path) {
                                "200 OK"
                            } else {
                                "404 Not Found"
                            };
                            write!(
                                stream,
                                "HTTP/1.1 {status}\r\nContent-Length: {}\r\nContent-Type: {}\r\nConnection: close\r\n\r\n",
                                response.body.len(),
                                response.content_type
                            )
                            .expect("write mock headers");
                            stream
                                .write_all(&response.body)
                                .expect("write mock response body");
                        }
                        Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                            thread::sleep(Duration::from_millis(10));
                        }
                        Err(error) => panic!("mock http server accept failed: {error}"),
                    }
                }
            });
            Self {
                base_url,
                address,
                stop,
                handle: Some(handle),
            }
        }
    }

    impl Drop for MockHttpServer {
        fn drop(&mut self) {
            self.stop.store(true, Ordering::SeqCst);
            let _ = TcpStream::connect(&self.address);
            if let Some(handle) = self.handle.take() {
                handle.join().expect("join mock http server");
            }
        }
    }

    fn read_mock_http_request_path(stream: &mut TcpStream) -> String {
        let mut request = Vec::new();
        let mut buffer = [0_u8; 1024];
        loop {
            match std::io::Read::read(stream, &mut buffer) {
                Ok(0) => break,
                Ok(read) => {
                    request.extend_from_slice(&buffer[..read]);
                    if request.windows(4).any(|window| window == b"\r\n\r\n") {
                        break;
                    }
                }
                Err(error)
                    if matches!(
                        error.kind(),
                        std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                    ) =>
                {
                    break;
                }
                Err(error) => panic!("read mock http request failed: {error}"),
            }
        }

        String::from_utf8_lossy(&request)
            .lines()
            .next()
            .and_then(|line| line.split_whitespace().nth(1))
            .unwrap_or_default()
            .to_owned()
    }

    fn sample_sccp_capabilities() -> SccpCapabilities {
        SccpCapabilities {
            local_domain: iroha_sccp::SCCP_DOMAIN_SORA,
            local_chain: "sora".to_owned(),
            proof_family: iroha_sccp::SCCP_STARK_FRI_PROOF_FAMILY_V1.to_owned(),
            burn_bundle_path: "/v1/sccp/proofs/burn/{message_id}".to_owned(),
            governance_bundle_path: "/v1/sccp/proofs/governance/{message_id}".to_owned(),
            message_bundle_path: "/v1/sccp/proofs/message/{message_id}".to_owned(),
            message_proof_path: "/v1/sccp/artifacts/message/{message_id}".to_owned(),
            proof_manifest_path: "/v1/sccp/manifests".to_owned(),
            legacy_burn_registry_backend: "bridge/sccp/burn-v1".to_owned(),
            legacy_governance_registry_backend: "bridge/sccp/governance-v1".to_owned(),
            proof_submit_path: Some("/v1/bridge/proofs/submit".to_owned()),
            message_submit_path: Some("/v1/bridge/messages".to_owned()),
            message_payload_kinds: vec![
                "asset_register".to_owned(),
                "route_activate".to_owned(),
                "transfer".to_owned(),
            ],
            codecs: vec![
                SccpCodecCapability {
                    id: iroha_sccp::SCCP_CODEC_TEXT_UTF8,
                    key: "text_utf8".to_owned(),
                    description: "Logical UTF-8 identifiers for SORA and route-local names."
                        .to_owned(),
                },
                SccpCodecCapability {
                    id: iroha_sccp::SCCP_CODEC_EVM_HEX,
                    key: "evm_hex".to_owned(),
                    description: "0x-prefixed 20-byte EVM account addresses.".to_owned(),
                },
            ],
            counterparties: vec![
                SccpCounterpartyCapability {
                    domain: iroha_sccp::SCCP_DOMAIN_TON,
                    chain: "ton".to_owned(),
                    message_backend: "bridge/sccp/stark-fri-v1/ton".to_owned(),
                    registry_backend: "bridge/sccp/registry-v1/ton".to_owned(),
                    counterparty_account_codec: iroha_sccp::SCCP_CODEC_TON_RAW,
                    counterparty_account_codec_key: "ton_raw".to_owned(),
                },
                SccpCounterpartyCapability {
                    domain: iroha_sccp::SCCP_DOMAIN_ETH,
                    chain: "eth".to_owned(),
                    message_backend: "bridge/sccp/stark-fri-v1/eth".to_owned(),
                    registry_backend: "bridge/sccp/registry-v1/eth".to_owned(),
                    counterparty_account_codec: iroha_sccp::SCCP_CODEC_EVM_HEX,
                    counterparty_account_codec_key: "evm_hex".to_owned(),
                },
            ],
        }
    }

    fn sample_sccp_proof_manifests() -> SccpProofManifestSet {
        SccpProofManifestSet {
            local_domain: iroha_sccp::SCCP_DOMAIN_SORA,
            local_chain: "sora".to_owned(),
            proof_family: iroha_sccp::SCCP_STARK_FRI_PROOF_FAMILY_V1.to_owned(),
            manifests: vec![
                iroha_sccp::sccp_proof_manifest_for_domain(iroha_sccp::SCCP_DOMAIN_TON)
                    .expect("ton manifest"),
                iroha_sccp::sccp_proof_manifest_for_domain(iroha_sccp::SCCP_DOMAIN_TRON)
                    .expect("tron manifest"),
            ],
        }
    }

    fn sample_sccp_proof_manifests_json() -> JsonValue {
        JsonValue::Object(norito::json::Map::from_iter([
            (
                "local_domain".into(),
                JsonValue::from(iroha_sccp::SCCP_DOMAIN_SORA),
            ),
            ("local_chain".into(), JsonValue::from("sora")),
            (
                "proof_family".into(),
                JsonValue::from(iroha_sccp::SCCP_STARK_FRI_PROOF_FAMILY_V1),
            ),
            (
                "manifests".into(),
                JsonValue::Array(vec![
                    JsonValue::Object(norito::json::Map::from_iter([
                        ("chain".into(), JsonValue::from("ton")),
                        (
                            "counterparty_domain".into(),
                            JsonValue::from(iroha_sccp::SCCP_DOMAIN_TON),
                        ),
                    ])),
                    JsonValue::Object(norito::json::Map::from_iter([
                        ("chain".into(), JsonValue::from("tron")),
                        (
                            "counterparty_domain".into(),
                            JsonValue::from(iroha_sccp::SCCP_DOMAIN_TRON),
                        ),
                    ])),
                ]),
            ),
        ]))
    }

    fn sample_sccp_message_proof_artifact() -> iroha_sccp::NexusSccpMessageTransparentProofV1 {
        use iroha_sccp::{
            NexusBridgeFinalityProofV1, NexusCommitQcV1, NexusConsensusPhaseV1,
            NexusSccpMessageProofV1, SccpHubCommitmentV1, SccpHubMessageKind, SccpMerkleProofV1,
            SccpPayloadV1, TransferPayloadV1, build_nexus_sccp_message_transparent_proof,
            canonical_sccp_payload_bytes, merkle_root_from_commitment, payload_hash,
            sccp_message_id,
        };

        let payload = SccpPayloadV1::Transfer(TransferPayloadV1 {
            version: 1,
            source_domain: iroha_sccp::SCCP_DOMAIN_SORA,
            dest_domain: iroha_sccp::SCCP_DOMAIN_TON,
            nonce: 21,
            asset_home_domain: iroha_sccp::SCCP_DOMAIN_SORA,
            asset_id_codec: iroha_sccp::SCCP_CODEC_TEXT_UTF8,
            asset_id: b"xor#universal".to_vec(),
            amount: 77,
            sender_codec: iroha_sccp::SCCP_CODEC_TEXT_UTF8,
            sender: b"nexus:soraswap".to_vec(),
            recipient_codec: iroha_sccp::SCCP_CODEC_TON_RAW,
            recipient: b"0:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
                .to_vec(),
            route_id_codec: iroha_sccp::SCCP_CODEC_TEXT_UTF8,
            route_id: b"nexus:ton:xor".to_vec(),
        });
        let commitment = SccpHubCommitmentV1 {
            version: 1,
            kind: SccpHubMessageKind::Transfer,
            target_domain: iroha_sccp::SCCP_DOMAIN_TON,
            message_id: sccp_message_id(&payload),
            payload_hash: payload_hash(&canonical_sccp_payload_bytes(&payload)),
            parliament_certificate_hash: None,
        };
        let merkle_proof = SccpMerkleProofV1 { steps: Vec::new() };
        let commitment_root = merkle_root_from_commitment(&commitment, &merkle_proof);
        let finality_proof = NexusBridgeFinalityProofV1 {
            version: 1,
            chain_id: "taira".to_owned(),
            height: 19,
            block_hash: [0x44; 32],
            commitment_root,
            block_header_bytes: vec![0x01, 0x02, 0x03],
            commit_qc: NexusCommitQcV1 {
                version: 1,
                phase: NexusConsensusPhaseV1::Commit,
                height: 19,
                view: 1,
                epoch: 1,
                mode_tag: "normal".to_owned(),
                subject_block_hash: [0x44; 32],
                validator_set_hash_version: 1,
                validator_public_keys: vec!["validator-1".to_owned()],
                validator_set_pops: vec![vec![0xAA]],
                signers_bitmap: vec![0x01],
                bls_aggregate_signature: vec![0xBB],
            },
        };
        let bundle = NexusSccpMessageProofV1 {
            version: 1,
            commitment_root,
            commitment,
            merkle_proof,
            payload,
            finality_proof: norito::to_bytes(&finality_proof).expect("encode finality proof"),
        };
        build_nexus_sccp_message_transparent_proof(&bundle).expect("build SCCP message artifact")
    }

    #[test]
    fn emit_receipt_builds_record_bridge_receipt() {
        let mut ctx = TestContext::new();
        let args = EmitReceiptArgs {
            lane: 3,
            direction: "mint".to_string(),
            source_tx: "11".repeat(32),
            amount: 5,
            asset_id: "62Fk4FPcMuLvW5QjDGNF2a4jAmjM".to_string(),
            recipient: "sorauロ1NラhBUd2BツヲトiヤニツヌKSテaリメモQラrメoリナnウリbQウQJニLJ5HSE"
                .to_string(),
            dest_tx: Some("22".repeat(32)),
            proof_hash: Some("33".repeat(32)),
        };

        emit_receipt(&mut ctx, args).expect("emit receipt");

        let executable = ctx.captured.expect("captured executable");
        let Executable::Instructions(instructions) = executable else {
            panic!("expected instruction executable");
        };
        let instructions = instructions.into_vec();
        assert_eq!(instructions.len(), 1, "expected one instruction");

        let record = instructions[0]
            .as_any()
            .downcast_ref::<RecordBridgeReceipt>()
            .expect("record bridge receipt instruction");
        let receipt = &record.receipt;
        assert_eq!(receipt.lane, LaneId::new(3));
        assert_eq!(receipt.direction, b"mint".to_vec());
        assert_eq!(receipt.source_tx, [0x11; 32]);
        assert_eq!(receipt.dest_tx, Some([0x22; 32]));
        assert_eq!(receipt.proof_hash, [0x33; 32]);
        assert_eq!(receipt.amount, 5);
        assert_eq!(receipt.asset_id, b"62Fk4FPcMuLvW5QjDGNF2a4jAmjM".to_vec());
        assert_eq!(
            receipt.recipient,
            "sorauロ1NラhBUd2BツヲトiヤニツヌKSテaリメモQラrメoリナnウリbQウQJニLJ5HSE"
                .as_bytes()
                .to_vec()
        );
    }

    #[test]
    fn sccp_capabilities_text_command_prints_summary() {
        let capabilities = sample_sccp_capabilities();
        let server = MockHttpServer::start(BTreeMap::from([(
            "/v1/sccp/capabilities".to_owned(),
            MockHttpResponse {
                content_type: "application/x-norito",
                body: norito::to_bytes(&capabilities).expect("encode capabilities"),
            },
        )]));
        let mut ctx = TestContext::with_base_url(CliOutputFormat::Text, server.base_url.clone());

        Command::Sccp(SccpCommand::Capabilities)
            .run(&mut ctx)
            .expect("run capabilities command");

        let rendered = ctx.printed_lines.join("\n");
        assert!(rendered.contains("sccp capabilities:"));
        assert!(rendered.contains("proof_family=stark-fri-v1"));
        assert!(rendered.contains("ton(4:ton_raw)"));
        assert!(rendered.contains("eth(1:evm_hex)"));
    }

    #[test]
    fn sccp_manifests_json_command_prints_typed_payload() {
        let manifests = sample_sccp_proof_manifests_json();
        let server = MockHttpServer::start(BTreeMap::from([(
            "/v1/sccp/manifests".to_owned(),
            MockHttpResponse {
                content_type: "application/json",
                body: norito::json::to_vec(&manifests).expect("encode manifests json"),
            },
        )]));
        let mut ctx = TestContext::with_base_url(CliOutputFormat::Json, server.base_url.clone());

        Command::Sccp(SccpCommand::Manifests)
            .run(&mut ctx)
            .expect("run manifests command");

        let payload = ctx.printed_json.expect("printed json");
        assert_eq!(
            payload.get("local_chain").and_then(JsonValue::as_str),
            Some("sora")
        );
        assert_eq!(
            payload
                .get("manifests")
                .and_then(JsonValue::as_array)
                .map(Vec::len),
            Some(2)
        );
    }

    #[test]
    fn sccp_artifact_text_command_prints_summary() {
        let artifact = sample_sccp_message_proof_artifact();
        let message_id_hex = hex::encode(artifact.public_inputs.message_id);
        let server = MockHttpServer::start(BTreeMap::from([(
            format!("/v1/sccp/artifacts/message/{message_id_hex}"),
            MockHttpResponse {
                content_type: "application/x-norito",
                body: norito::to_bytes(&artifact).expect("encode artifact"),
            },
        )]));
        let mut ctx = TestContext::with_base_url(CliOutputFormat::Text, server.base_url.clone());

        Command::Sccp(SccpCommand::Artifact(ArtifactArgs {
            message_id: format!("0x{message_id_hex}"),
        }))
        .run(&mut ctx)
        .expect("run artifact command");

        let rendered = ctx.printed_lines.join("\n");
        assert!(rendered.contains("sccp artifact:"));
        assert!(rendered.contains(&message_id_hex));
        assert!(rendered.contains("payload=transfer"));
        assert!(rendered.contains("chain=ton(4)"));
        assert!(rendered.contains("finality_height=19"));
    }
}
