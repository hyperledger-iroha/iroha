use clap::Subcommand;
use eyre::Result;
use iroha::{
    client::{SccpCapabilities, SccpMessageProofQueryParams, SccpProofManifestSet},
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
    /// Fetch a normalized SCCP counterparty proof job by message id
    Job(ArtifactArgs),
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
    /// Destination network id (hex, 32 bytes)
    #[arg(long, value_name = "HEX")]
    network_id_hex: Option<String>,
    /// EVM verifier contract address (hex, 20 bytes)
    #[arg(long, value_name = "HEX")]
    verifier_address_hex: Option<String>,
    /// EVM bridge contract address (hex, 20 bytes)
    #[arg(long, value_name = "HEX")]
    bridge_address_hex: Option<String>,
    /// Destination verifier contract code hash (hex, 32 bytes)
    #[arg(long, value_name = "HEX")]
    verifier_code_hash_hex: Option<String>,
    /// Destination verifier key hash (hex, 32 bytes)
    #[arg(long, value_name = "HEX")]
    verifier_key_hash_hex: Option<String>,
    /// Expected canonical destination binding hash (hex, 32 bytes)
    #[arg(long, value_name = "HEX")]
    expected_destination_binding_hash_hex: Option<String>,
    /// TRON verifier contract address
    #[arg(long, value_name = "ADDRESS")]
    tron_verifier_address: Option<String>,
    /// Externally generated 384-byte Groth16 ABI proof tuple (hex)
    #[arg(long, value_name = "HEX")]
    proof_bytes_hex: Option<String>,
}

impl ArtifactArgs {
    fn proof_query_params(&self) -> SccpMessageProofQueryParams {
        SccpMessageProofQueryParams {
            network_id_hex: self.network_id_hex.clone(),
            verifier_address_hex: self.verifier_address_hex.clone(),
            bridge_address_hex: self.bridge_address_hex.clone(),
            verifier_code_hash_hex: self.verifier_code_hash_hex.clone(),
            verifier_key_hash_hex: self.verifier_key_hash_hex.clone(),
            expected_destination_binding_hash_hex: self
                .expected_destination_binding_hash_hex
                .clone(),
            tron_verifier_address: self.tron_verifier_address.clone(),
            proof_bytes_hex: self.proof_bytes_hex.clone(),
        }
    }
}

impl Run for Command {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        match self {
            Command::EmitReceipt(args) => emit_receipt(context, args),
            Command::Sccp(cmd) => match cmd {
                SccpCommand::Capabilities => sccp_capabilities(context),
                SccpCommand::Manifests => sccp_manifests(context),
                SccpCommand::Artifact(args) => sccp_artifact(context, args),
                SccpCommand::Job(args) => sccp_job(context, args),
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
    let query_params = args.proof_query_params();
    match ctx.output_format() {
        CliOutputFormat::Text => {
            let artifact = ctx
                .client_from_config()
                .get_sccp_message_proof_artifact_with_params(&args.message_id, &query_params)?;
            ctx.println(render_sccp_artifact_summary(&artifact))
        }
        CliOutputFormat::Json => {
            let artifact = ctx
                .client_from_config()
                .get_sccp_message_proof_artifact_json_with_params(
                    &args.message_id,
                    &query_params,
                )?;
            ctx.print_data(&artifact)
        }
    }
}

fn sccp_job(ctx: &mut impl RunContext, args: ArtifactArgs) -> Result<()> {
    let query_params = args.proof_query_params();
    match ctx.output_format() {
        CliOutputFormat::Text => {
            let job = ctx
                .client_from_config()
                .get_sccp_message_proof_job_with_params(&args.message_id, &query_params)?;
            ctx.println(render_sccp_job_summary(&job))
        }
        CliOutputFormat::Json => {
            let job = ctx
                .client_from_config()
                .get_sccp_message_proof_job_json_with_params(&args.message_id, &query_params)?;
            ctx.print_data(&job)
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
                "{}({}:{}:{}:{}:{})",
                counterparty.chain,
                counterparty.domain,
                counterparty.counterparty_account_codec_key,
                counterparty.verifier_backend.key.as_str(),
                render_sccp_destination_rollout_summary(&counterparty.destination_rollout),
                if counterparty.production_ready {
                    "ready"
                } else {
                    "disabled"
                }
            )
        })
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        "sccp capabilities: local={}({}) proof_family={} payloads={} codecs={} artifact={} job={} manifests={} counterparties=[{}]",
        capabilities.local_chain,
        capabilities.local_domain,
        capabilities.proof_family,
        payloads,
        codecs,
        capabilities.message_proof_path,
        capabilities.message_job_path,
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
            "chain={} domain={} backend={} verifier_backend={} registry={} security={:?} anchors={:?} binding={} finality={:?} verifier={:?} codec={} rollout={} ready={} submit={}",
            manifest.chain,
            manifest.counterparty_domain,
            manifest.message_backend,
            manifest.verifier_backend.key.as_str(),
            manifest.registry_backend,
            manifest.security_model,
            manifest.anchor_governance,
            manifest.destination_binding.key,
            manifest.finality_model,
            manifest.verifier_target,
            manifest.counterparty_account_codec_key,
            render_sccp_destination_rollout_summary(&manifest.destination_rollout),
            manifest.production_ready,
            render_sccp_submission_template_summary(&manifest.submission_template)
        )
    }));
    lines.join("\n")
}

fn render_sccp_destination_rollout_summary(
    rollout: &iroha_sccp::SccpDestinationRolloutV1,
) -> String {
    format!(
        "{:?}/verifier_live={}/anchors_live={}",
        rollout.verifier_plan, rollout.immutable_verifier_ready, rollout.anchors_ready
    )
}

fn render_sccp_artifact_summary(
    artifact: &iroha_sccp::NexusSccpMessageTransparentProofV1,
) -> String {
    let projection_summary =
        match iroha_sccp::build_sccp_counterparty_proof_job_from_artifact(artifact) {
            Some(job) => format!(
                " projection={} submit={}",
                render_sccp_payload_projection_summary(&job.payload_projection),
                render_sccp_submission_template_summary(&job.submission_template)
            ),
            None => String::new(),
        };
    let inner_summary = match iroha_sccp::build_sccp_message_transparent_inner_proof_from_artifact(
        artifact,
    ) {
        Some(inner) => format!(
            " inner_family={:?} inner_payload={} statement_hash={} verifier_backend={} proof_parameter=fastpq-lane-balanced",
            inner.chain_family,
            inner.payload_kind,
            hex::encode(inner.statement_hash),
            inner.verifier_backend.key.as_str()
        ),
        None => format!(" proof_bytes_len={}", artifact.proof_bytes.len()),
    };
    let open_verify_summary =
        iroha_sccp::summarize_sccp_message_transparent_open_verify_proof_from_artifact(artifact)
            .map(render_sccp_open_verify_summary)
            .unwrap_or_default();
    format!(
        "sccp artifact: message_id={} payload={} chain={}({}) backend={} verifier_backend={} security={:?} anchors={:?} binding={} proof_family={} finality_height={} commitment_root={}{}{}{} package={}/{}",
        hex::encode(artifact.public_inputs.message_id),
        iroha_sccp::sccp_message_payload_kind_key(&artifact.bundle.payload),
        iroha_sccp::sccp_chain_key_for_domain(artifact.counterparty_domain).unwrap_or("unknown"),
        artifact.counterparty_domain,
        artifact.message_backend,
        artifact.verifier_backend.key.as_str(),
        artifact.security_model,
        artifact.anchor_governance,
        artifact.destination_binding.key,
        artifact.proof_family,
        artifact.public_inputs.finality_height,
        hex::encode(artifact.public_inputs.commitment_root),
        projection_summary,
        inner_summary,
        open_verify_summary,
        artifact.submission_package.submission_kind,
        artifact.submission_package.envelope_encoding
    )
}

fn render_sccp_job_summary(job: &iroha_sccp::SccpCounterpartyProofJobV1) -> String {
    let open_verify_summary =
        iroha_sccp::build_sccp_message_transparent_open_verify_summary_from_bundle(&job.bundle)
            .map(render_sccp_open_verify_summary)
            .unwrap_or_default();
    format!(
        "sccp job: message_id={} payload={} chain={}({}) backend={} verifier_backend={} registry={} security={:?} anchors={:?} binding={} finality={:?} verifier={:?} projection={} submit={}{} package={}/{}",
        hex::encode(job.public_inputs.message_id),
        job.payload_kind,
        job.chain,
        job.counterparty_domain,
        job.message_backend,
        job.verifier_backend.key.as_str(),
        job.registry_backend,
        job.security_model,
        job.anchor_governance,
        job.destination_binding.key,
        job.finality_model,
        job.verifier_target,
        render_sccp_payload_projection_summary(&job.payload_projection),
        render_sccp_submission_template_summary(&job.submission_template),
        open_verify_summary,
        job.submission_package.submission_kind,
        job.submission_package.envelope_encoding
    )
}

fn render_sccp_open_verify_summary(summary: iroha_sccp::SccpOpenVerifyEnvelopeSummaryV1) -> String {
    format!(
        " open_verify={}/{} vk_hash={} schema_hash={} columns={} words={} backend_proof_len={} aux_len={}",
        summary.backend,
        summary.circuit_id,
        hex::encode(summary.vk_hash),
        hex::encode(summary.public_inputs_schema_hash),
        summary.public_input_column_count,
        summary.public_input_word_count,
        summary.backend_proof_len_bytes,
        summary.aux_len_bytes
    )
}

fn render_sccp_submission_template_summary(
    template: &iroha_sccp::SccpCounterpartySubmissionTemplateV1,
) -> String {
    let arguments = template
        .required_arguments
        .iter()
        .map(|argument| argument.key.as_str())
        .collect::<Vec<_>>()
        .join(",");
    format!(
        "{}/{}/{} args=[{}]",
        template.submission_kind, template.encoding, template.verifier_entrypoint, arguments
    )
}

fn render_sccp_payload_projection_summary(
    projection: &iroha_sccp::SccpPayloadProjectionV1,
) -> String {
    match projection {
        iroha_sccp::SccpPayloadProjectionV1::AssetRegister(asset) => format!(
            "asset_register asset_id={} home_domain={} decimals={}",
            render_sccp_normalized_codec_value(&asset.asset_id),
            asset.home_domain,
            asset.decimals
        ),
        iroha_sccp::SccpPayloadProjectionV1::RouteActivate(route) => format!(
            "route_activate asset_id={} route_id={}",
            render_sccp_normalized_codec_value(&route.asset_id),
            render_sccp_normalized_codec_value(&route.route_id)
        ),
        iroha_sccp::SccpPayloadProjectionV1::Transfer(transfer) => format!(
            "transfer asset_id={} amount={} sender={} recipient={} route_id={}",
            render_sccp_normalized_codec_value(&transfer.asset_id),
            transfer.amount,
            render_sccp_normalized_codec_value(&transfer.sender),
            render_sccp_normalized_codec_value(&transfer.recipient),
            render_sccp_normalized_codec_value(&transfer.route_id)
        ),
        iroha_sccp::SccpPayloadProjectionV1::TokenAdd(token) => format!(
            "token_add sora_asset_id={} decimals={} name={} symbol={}",
            hex::encode(token.sora_asset_id),
            token.decimals,
            hex::encode(token.name),
            hex::encode(token.symbol)
        ),
        iroha_sccp::SccpPayloadProjectionV1::TokenPause(token) => {
            format!(
                "token_pause sora_asset_id={}",
                hex::encode(token.sora_asset_id)
            )
        }
        iroha_sccp::SccpPayloadProjectionV1::TokenResume(token) => {
            format!(
                "token_resume sora_asset_id={}",
                hex::encode(token.sora_asset_id)
            )
        }
    }
}

fn render_sccp_normalized_codec_value(value: &iroha_sccp::SccpNormalizedCodecValueV1) -> String {
    match value {
        iroha_sccp::SccpNormalizedCodecValueV1::TextUtf8 { value } => format!("text:{value}"),
        iroha_sccp::SccpNormalizedCodecValueV1::EvmHex { bytes } => {
            format!("evm:0x{}", hex::encode(bytes))
        }
        iroha_sccp::SccpNormalizedCodecValueV1::SolanaBase58 { bytes } => {
            format!("solana:{}", bs58::encode(bytes).into_string())
        }
        iroha_sccp::SccpNormalizedCodecValueV1::TonRaw { workchain, account } => {
            format!("ton:{workchain}:{}", hex::encode(account))
        }
        iroha_sccp::SccpNormalizedCodecValueV1::TronBase58Check { payload } => {
            format!("tron:{}", hex::encode(payload))
        }
        iroha_sccp::SccpNormalizedCodecValueV1::SoraAssetId { bytes } => {
            format!("sora_asset_id:0x{}", hex::encode(bytes))
        }
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
        net::{Shutdown, TcpListener, TcpStream},
        sync::{
            Arc, Mutex, MutexGuard, OnceLock,
            atomic::{AtomicBool, Ordering},
        },
        thread,
        time::Duration,
    };
    use url::Url;

    fn mock_http_server_guard() -> MutexGuard<'static, ()> {
        static GUARD: OnceLock<Mutex<()>> = OnceLock::new();
        GUARD
            .get_or_init(|| Mutex::new(()))
            .lock()
            .expect("mock HTTP server test guard")
    }

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
                            stream.flush().expect("flush mock response");
                            let _ = stream.shutdown(Shutdown::Write);
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
            message_bundle_path: "/v1/sccp/proofs/message/{message_id}".to_owned(),
            message_proof_path: "/v1/sccp/artifacts/message/{message_id}".to_owned(),
            message_job_path: "/v1/sccp/jobs/message/{message_id}".to_owned(),
            proof_manifest_path: "/v1/sccp/manifests".to_owned(),
            legacy_burn_registry_backend: "bridge/sccp/burn-v1".to_owned(),
            proof_submit_path: Some("/v1/bridge/proofs/submit".to_owned()),
            message_submit_path: Some("/v1/bridge/messages".to_owned()),
            production_policy: iroha_sccp::sccp_production_policy_v1(),
            launch_ready: false,
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
                    description: "0x-prefixed canonical EIP-55 EVM account addresses.".to_owned(),
                },
            ],
            counterparties: vec![
                SccpCounterpartyCapability {
                    domain: iroha_sccp::SCCP_DOMAIN_TON,
                    chain: "ton".to_owned(),
                    verifier_backend: iroha_sccp::sccp_verifier_backend_for_domain(
                        iroha_sccp::SCCP_DOMAIN_TON,
                    )
                    .expect("ton verifier backend"),
                    message_backend: "bridge/sccp/stark-fri-v1/ton".to_owned(),
                    registry_backend: "bridge/sccp/registry-v1/ton".to_owned(),
                    counterparty_account_codec: iroha_sccp::SCCP_CODEC_TON_RAW,
                    counterparty_account_codec_key: "ton_raw".to_owned(),
                    destination_rollout: iroha_sccp::sccp_destination_rollout_for_domain(
                        iroha_sccp::SCCP_DOMAIN_TON,
                    )
                    .expect("ton destination rollout"),
                    production_ready: false,
                    disabled_reason: Some(
                        iroha_sccp::sccp_lane_disabled_reason_for_domain(
                            iroha_sccp::SCCP_DOMAIN_TON,
                        )
                        .expect("ton disabled reason")
                        .to_owned(),
                    ),
                    production_readiness: iroha_sccp::sccp_lane_production_readiness_for_domain(
                        iroha_sccp::SCCP_DOMAIN_TON,
                    )
                    .expect("ton production readiness"),
                },
                SccpCounterpartyCapability {
                    domain: iroha_sccp::SCCP_DOMAIN_ETH,
                    chain: "eth".to_owned(),
                    verifier_backend: iroha_sccp::sccp_verifier_backend_for_domain(
                        iroha_sccp::SCCP_DOMAIN_ETH,
                    )
                    .expect("eth verifier backend"),
                    message_backend: "bridge/sccp/stark-fri-v1/eth".to_owned(),
                    registry_backend: "bridge/sccp/registry-v1/eth".to_owned(),
                    counterparty_account_codec: iroha_sccp::SCCP_CODEC_EVM_HEX,
                    counterparty_account_codec_key: "evm_hex".to_owned(),
                    destination_rollout: iroha_sccp::sccp_destination_rollout_for_domain(
                        iroha_sccp::SCCP_DOMAIN_ETH,
                    )
                    .expect("eth destination rollout"),
                    production_ready: false,
                    disabled_reason: Some(
                        iroha_sccp::sccp_lane_disabled_reason_for_domain(
                            iroha_sccp::SCCP_DOMAIN_ETH,
                        )
                        .expect("eth disabled reason")
                        .to_owned(),
                    ),
                    production_readiness: iroha_sccp::sccp_lane_production_readiness_for_domain(
                        iroha_sccp::SCCP_DOMAIN_ETH,
                    )
                    .expect("eth production readiness"),
                },
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
            NexusSccpMessageProofV1, NexusSccpMessageTransparentProofV1,
            SccpCounterpartySubmissionPackageV1, SccpHubCommitmentV1, SccpHubMessageKind,
            SccpMerkleProofV1, SccpPayloadV1, SccpPlatformSubmissionPayloadV1, TransferPayloadV1,
            build_sccp_message_transparent_inner_proof,
            build_sccp_ton_internal_message_submission_payload, canonical_sccp_payload_bytes,
            merkle_root_from_commitment, payload_hash, sccp_message_id,
            sccp_message_transparent_public_inputs, sccp_proof_manifest_for_domain,
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
                parent_state_root: [0u8; 32],
                post_state_root: [0u8; 32],
                chain_order_hash: [0u8; 32],
                rechain_seq: 0,
                highest_qc: None,
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
        let manifest =
            sccp_proof_manifest_for_domain(iroha_sccp::SCCP_DOMAIN_TON).expect("ton manifest");
        let public_inputs =
            sccp_message_transparent_public_inputs(&bundle).expect("message public inputs");
        let open = iroha::data_model::zk::StarkFriOpenProofV1 {
            version: 1,
            public_inputs: vec![vec![[0x55; 32]]],
            envelope_bytes: vec![0xAA, 0xBB, 0xCC],
        };
        let proof_bytes = norito::to_bytes(&iroha::data_model::zk::OpenVerifyEnvelope {
            backend: iroha::data_model::zk::BackendTag::Stark,
            circuit_id: "sccp-message-transparent-v1".to_owned(),
            vk_hash: [0x66; 32],
            public_inputs: vec![0x77, 0x88, 0x99],
            proof_bytes: norito::to_bytes(&open).expect("encode open proof"),
            aux: vec![0xDE, 0xAD],
        })
        .expect("encode open verify envelope");
        let inner =
            build_sccp_message_transparent_inner_proof(&bundle, &manifest).expect("inner proof");
        let platform_payload = SccpPlatformSubmissionPayloadV1::TonInternalMessage(
            build_sccp_ton_internal_message_submission_payload(
                &manifest,
                &proof_bytes,
                &public_inputs,
                &bundle,
                inner.statement_hash,
                &manifest.destination_binding,
            )
            .expect("ton payload"),
        );
        NexusSccpMessageTransparentProofV1 {
            version: 1,
            local_domain: manifest.local_domain,
            counterparty_domain: manifest.counterparty_domain,
            security_model: manifest.security_model,
            anchor_governance: manifest.anchor_governance,
            destination_binding: manifest.destination_binding.clone(),
            proof_family: manifest.proof_family.clone(),
            verifier_backend: manifest.verifier_backend.clone(),
            message_backend: manifest.message_backend.clone(),
            registry_backend: manifest.registry_backend.clone(),
            manifest_seed: manifest.manifest_seed.clone(),
            finality_model: manifest.finality_model,
            verifier_target: manifest.verifier_target,
            public_inputs,
            proof_bytes: proof_bytes.clone(),
            submission_package: SccpCounterpartySubmissionPackageV1 {
                version: 1,
                proof_family: manifest.proof_family.clone(),
                verifier_backend: manifest.verifier_backend.clone(),
                envelope_encoding: "ton_message_body_boc_v1".to_owned(),
                submission_kind: manifest.submission_template.submission_kind.clone(),
                verifier_entrypoint: manifest.submission_template.verifier_entrypoint.clone(),
                platform_payload,
                arguments: Vec::new(),
                envelope_bytes: vec![0xCC],
            },
            bundle,
        }
    }

    fn sample_sccp_message_job() -> iroha_sccp::SccpCounterpartyProofJobV1 {
        let artifact = sample_sccp_message_proof_artifact();
        let manifest = iroha_sccp::sccp_proof_manifest_for_domain(iroha_sccp::SCCP_DOMAIN_TON)
            .expect("ton manifest");
        iroha_sccp::SccpCounterpartyProofJobV1 {
            version: 1,
            chain_family: iroha_sccp::SccpTransparentChainFamilyV1::Ton,
            chain: "ton".to_owned(),
            local_domain: artifact.local_domain,
            counterparty_domain: artifact.counterparty_domain,
            security_model: artifact.security_model,
            anchor_governance: artifact.anchor_governance,
            destination_binding: artifact.destination_binding.clone(),
            proof_family: artifact.proof_family.clone(),
            verifier_backend: artifact.verifier_backend.clone(),
            message_backend: artifact.message_backend.clone(),
            registry_backend: artifact.registry_backend.clone(),
            manifest_seed: artifact.manifest_seed.clone(),
            finality_model: artifact.finality_model,
            verifier_target: artifact.verifier_target,
            public_inputs: artifact.public_inputs.clone(),
            payload_kind: "transfer".to_owned(),
            payload_projection: iroha_sccp::sccp_payload_projection(&artifact.bundle.payload)
                .expect("payload projection"),
            submission_template: manifest.submission_template,
            submission_package: artifact.submission_package.clone(),
            bundle: artifact.bundle,
        }
    }

    fn artifact_args(message_id: String) -> ArtifactArgs {
        ArtifactArgs {
            message_id,
            network_id_hex: None,
            verifier_address_hex: None,
            bridge_address_hex: None,
            verifier_code_hash_hex: None,
            verifier_key_hash_hex: None,
            expected_destination_binding_hash_hex: None,
            tron_verifier_address: None,
            proof_bytes_hex: None,
        }
    }

    fn sccp_groth16_abi_word_hex(value: u32) -> String {
        format!("{value:064x}")
    }

    fn sample_sccp_groth16_proof_hex() -> String {
        [
            sccp_groth16_abi_word_hex(1),
            "11".repeat(32),
            sccp_groth16_abi_word_hex(0),
            "33".repeat(32),
            sccp_groth16_abi_word_hex(1),
            sccp_groth16_abi_word_hex(2),
            "1800deef121f1e76426a00665e5c4479674322d4f75edadd46debd5cd992f6ed".to_owned(),
            "198e9393920d483a7260bfb731fb5d25f1aa493335a9e71297e485b7aef312c2".to_owned(),
            "12c85ea5db8c6deb4aab71808dcb408fe3d1e7690c43d37b4ce6cc0166fa7daa".to_owned(),
            "090689d0585ff075ec9e99ad690c3395bc4b313370b38ef355acdadcd122975b".to_owned(),
            sccp_groth16_abi_word_hex(1),
            sccp_groth16_abi_word_hex(2),
        ]
        .concat()
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
            recipient: "sorauﾛ1NﾗhBUd2BﾂｦﾄiﾔﾆﾂﾇKSﾃaﾘﾒﾓQﾗrﾒoﾘﾅnｳﾘbQｳQJﾆLJ5HSE".to_string(),
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
            "sorauﾛ1NﾗhBUd2BﾂｦﾄiﾔﾆﾂﾇKSﾃaﾘﾒﾓQﾗrﾒoﾘﾅnｳﾘbQｳQJﾆLJ5HSE"
                .as_bytes()
                .to_vec()
        );
    }

    #[test]
    fn sccp_capabilities_text_command_prints_summary() {
        let _guard = mock_http_server_guard();
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
        assert!(!rendered.contains("runtime_family="));
        assert!(!rendered.contains("runtime_backend="));
        assert!(!rendered.contains("runtime_message="));
        assert!(rendered.contains(
            "ton(4:ton_raw:ton-contract-v1:TonContractNativeRecursive/verifier_live=false/anchors_live=false:disabled)"
        ));
        assert!(rendered.contains(
            "eth(1:evm_hex:evm-groth16-bn254-v1:EvmGroth16Bn254Adapter/verifier_live=false/anchors_live=false:disabled)"
        ));
    }

    #[test]
    fn sccp_manifests_json_command_prints_typed_payload() {
        let _guard = mock_http_server_guard();
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
        let _guard = mock_http_server_guard();
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

        Command::Sccp(SccpCommand::Artifact(artifact_args(format!(
            "0x{message_id_hex}"
        ))))
        .run(&mut ctx)
        .expect("run artifact command");

        let rendered = ctx.printed_lines.join("\n");
        assert!(rendered.contains("sccp artifact:"));
        assert!(rendered.contains(&message_id_hex));
        assert!(rendered.contains("payload=transfer"));
        assert!(rendered.contains("chain=ton(4)"));
        assert!(rendered.contains("verifier_backend=ton-contract-v1"));
        assert!(rendered.contains("finality_height=19"));
        assert!(rendered.contains("inner_family=Ton"));
        assert!(rendered.contains("inner_payload=transfer"));
        assert!(rendered.contains("open_verify=stark/sccp-message-transparent-v1"));
        assert!(rendered.contains(&format!("vk_hash={}", "66".repeat(32))));
        assert!(rendered.contains("package=internal_message/ton_message_body_boc_v1"));
    }

    #[test]
    fn sccp_artifact_text_command_rejects_invalid_tron_verifier_address_before_request() {
        for invalid_address in [
            "TJRabPrwbZy45sbavfcjinPJC18kjpRTv9",
            "T9yD14Nj9j7xAB4dbGeiX9h8unkKHxuWwb",
        ] {
            let mut ctx = TestContext::with_base_url(
                CliOutputFormat::Text,
                Url::parse("http://127.0.0.1:1").expect("test base url"),
            );
            let mut args = artifact_args("11".repeat(32));
            args.tron_verifier_address = Some(invalid_address.to_owned());
            args.proof_bytes_hex = Some(format!("0x{}", sample_sccp_groth16_proof_hex()));

            let err = Command::Sccp(SccpCommand::Artifact(args))
                .run(&mut ctx)
                .expect_err("invalid TRON verifier address must fail before request");

            assert!(
                err.to_string().contains("tron_verifier_address"),
                "unexpected error: {err:?}"
            );
            assert!(
                ctx.printed_lines.is_empty(),
                "invalid query params must not print an artifact summary"
            );
        }
    }

    #[test]
    fn sccp_job_text_command_prints_summary() {
        let _guard = mock_http_server_guard();
        let job = sample_sccp_message_job();
        let message_id_hex = hex::encode(job.public_inputs.message_id);
        let server = MockHttpServer::start(BTreeMap::from([(
            format!("/v1/sccp/jobs/message/{message_id_hex}"),
            MockHttpResponse {
                content_type: "application/x-norito",
                body: norito::to_bytes(&job).expect("encode job"),
            },
        )]));
        let mut ctx = TestContext::with_base_url(CliOutputFormat::Text, server.base_url.clone());

        Command::Sccp(SccpCommand::Job(artifact_args(format!(
            "0x{message_id_hex}"
        ))))
        .run(&mut ctx)
        .expect("run job command");

        let rendered = ctx.printed_lines.join("\n");
        assert!(rendered.contains("sccp job:"));
        assert!(rendered.contains(&message_id_hex));
        assert!(rendered.contains("payload=transfer"));
        assert!(rendered.contains("chain=ton(4)"));
        assert!(rendered.contains("verifier_backend=ton-contract-v1"));
        assert!(rendered.contains("projection=transfer"));
        assert!(rendered.contains(
            "recipient=ton:0:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
        ));
        assert!(rendered.contains(
            "submit=internal_message/ton_message_body_boc_v1/op::submit_sccp_message_proof args=[message_body_boc]"
        ));
        assert!(rendered.contains("open_verify=stark/sccp-message-transparent-v1"));
        assert!(rendered.contains("package=internal_message/ton_message_body_boc_v1"));
    }

    #[test]
    fn sccp_job_text_command_forwards_tron_query_material() {
        let _guard = mock_http_server_guard();
        let job = sample_sccp_message_job();
        let message_id_hex = hex::encode(job.public_inputs.message_id);
        let network_id_hex = "11".repeat(32);
        let verifier_code_hash_hex = "ab".repeat(32);
        let verifier_key_hash_hex = "cd".repeat(32);
        let expected_destination_binding_hash_hex = "ef".repeat(32);
        let proof_bytes_hex = sample_sccp_groth16_proof_hex();
        let query = format!(
            "network_id_hex={network_id_hex}&verifier_code_hash_hex={verifier_code_hash_hex}&verifier_key_hash_hex={verifier_key_hash_hex}&expected_destination_binding_hash_hex={expected_destination_binding_hash_hex}&tron_verifier_address=TJRabPrwbZy45sbavfcjinPJC18kjpRTv8&proof_bytes_hex={proof_bytes_hex}"
        );
        let server = MockHttpServer::start(BTreeMap::from([(
            format!("/v1/sccp/jobs/message/{message_id_hex}?{query}"),
            MockHttpResponse {
                content_type: "application/x-norito",
                body: norito::to_bytes(&job).expect("encode job"),
            },
        )]));
        let mut ctx = TestContext::with_base_url(CliOutputFormat::Text, server.base_url.clone());
        let mut args = artifact_args(format!("0x{message_id_hex}"));
        args.network_id_hex = Some(format!("0x{}", network_id_hex.to_uppercase()));
        args.verifier_code_hash_hex = Some(format!("0x{}", verifier_code_hash_hex.to_uppercase()));
        args.verifier_key_hash_hex = Some(format!("0x{}", verifier_key_hash_hex.to_uppercase()));
        args.expected_destination_binding_hash_hex = Some(format!(
            "0x{}",
            expected_destination_binding_hash_hex.to_uppercase()
        ));
        args.tron_verifier_address = Some("  TJRabPrwbZy45sbavfcjinPJC18kjpRTv8  ".to_owned());
        args.proof_bytes_hex = Some(format!("0x{}", proof_bytes_hex.to_uppercase()));

        Command::Sccp(SccpCommand::Job(args))
            .run(&mut ctx)
            .expect("run job command");

        let rendered = ctx.printed_lines.join("\n");
        assert!(rendered.contains("sccp job:"));
        assert!(rendered.contains(&message_id_hex));
    }

    #[test]
    fn sccp_job_text_command_rejects_all_zero_proof_bytes_before_request() {
        let mut ctx = TestContext::with_base_url(
            CliOutputFormat::Text,
            Url::parse("http://127.0.0.1:1").expect("test base url"),
        );
        let mut args = artifact_args("11".repeat(32));
        args.proof_bytes_hex = Some("0x0000".to_owned());

        let err = Command::Sccp(SccpCommand::Job(args))
            .run(&mut ctx)
            .expect_err("all-zero proof bytes must fail before request");

        assert!(
            err.to_string().contains("proof_bytes_hex"),
            "unexpected error: {err:?}"
        );
        assert!(
            err.to_string().contains("all zero"),
            "unexpected error: {err:?}"
        );
        assert!(
            ctx.printed_lines.is_empty(),
            "invalid query params must not print a job summary"
        );
    }

    #[test]
    fn sccp_job_text_command_rejects_destination_material_without_proof_bytes() {
        let mut ctx = TestContext::with_base_url(
            CliOutputFormat::Text,
            Url::parse("http://127.0.0.1:1").expect("test base url"),
        );
        let mut args = artifact_args("11".repeat(32));
        args.network_id_hex = Some("22".repeat(32));
        args.verifier_code_hash_hex = Some("33".repeat(32));
        args.verifier_key_hash_hex = Some("44".repeat(32));
        args.expected_destination_binding_hash_hex = Some("55".repeat(32));
        args.tron_verifier_address = Some("TJRabPrwbZy45sbavfcjinPJC18kjpRTv8".to_owned());

        let err = Command::Sccp(SccpCommand::Job(args))
            .run(&mut ctx)
            .expect_err("destination proof material without proof bytes must fail before request");

        assert!(
            err.to_string().contains("proof_bytes_hex is required"),
            "unexpected error: {err:?}"
        );
        assert!(
            ctx.printed_lines.is_empty(),
            "invalid query params must not print a job summary"
        );
    }

    #[test]
    fn sccp_job_text_command_rejects_partial_or_mixed_destination_tuple() {
        let proof_bytes_hex = sample_sccp_groth16_proof_hex();
        let mut partial_tron = artifact_args("11".repeat(32));
        partial_tron.network_id_hex = Some("22".repeat(32));
        partial_tron.verifier_code_hash_hex = Some("33".repeat(32));
        partial_tron.verifier_key_hash_hex = Some("44".repeat(32));
        partial_tron.tron_verifier_address = Some("TJRabPrwbZy45sbavfcjinPJC18kjpRTv8".to_owned());
        partial_tron.proof_bytes_hex = Some(format!("0x{proof_bytes_hex}"));

        let mut mixed_evm_tron = artifact_args("11".repeat(32));
        mixed_evm_tron.network_id_hex = Some("22".repeat(32));
        mixed_evm_tron.verifier_address_hex = Some("66".repeat(20));
        mixed_evm_tron.bridge_address_hex = Some("77".repeat(20));
        mixed_evm_tron.verifier_code_hash_hex = Some("33".repeat(32));
        mixed_evm_tron.verifier_key_hash_hex = Some("44".repeat(32));
        mixed_evm_tron.expected_destination_binding_hash_hex = Some("55".repeat(32));
        mixed_evm_tron.tron_verifier_address =
            Some("TJRabPrwbZy45sbavfcjinPJC18kjpRTv8".to_owned());
        mixed_evm_tron.proof_bytes_hex = Some(format!("0x{proof_bytes_hex}"));

        for (args, expected_error) in [
            (
                partial_tron,
                "complete TRON SCCP deployment destination fields are required",
            ),
            (
                mixed_evm_tron,
                "EVM and TRON SCCP destination fields cannot be mixed",
            ),
        ] {
            let mut ctx = TestContext::with_base_url(
                CliOutputFormat::Text,
                Url::parse("http://127.0.0.1:1").expect("test base url"),
            );

            let err = Command::Sccp(SccpCommand::Job(args))
                .run(&mut ctx)
                .expect_err("partial or mixed destination tuple must fail before request");

            assert!(
                err.to_string().contains(expected_error),
                "unexpected error: {err:?}"
            );
            assert!(
                ctx.printed_lines.is_empty(),
                "invalid query params must not print a job summary"
            );
        }
    }

    #[test]
    fn sccp_job_text_command_rejects_invalid_tron_verifier_address_before_request() {
        for invalid_address in [
            "TJRabPrwbZy45sbavfcjinPJC18kjpRTv9",
            "T9yD14Nj9j7xAB4dbGeiX9h8unkKHxuWwb",
        ] {
            let mut ctx = TestContext::with_base_url(
                CliOutputFormat::Text,
                Url::parse("http://127.0.0.1:1").expect("test base url"),
            );
            let mut args = artifact_args("11".repeat(32));
            args.tron_verifier_address = Some(invalid_address.to_owned());
            args.proof_bytes_hex = Some(format!("0x{}", sample_sccp_groth16_proof_hex()));

            let err = Command::Sccp(SccpCommand::Job(args))
                .run(&mut ctx)
                .expect_err("invalid TRON verifier address must fail before request");

            assert!(
                err.to_string().contains("tron_verifier_address"),
                "unexpected error: {err:?}"
            );
            assert!(
                ctx.printed_lines.is_empty(),
                "invalid query params must not print a job summary"
            );
        }
    }

    #[test]
    fn sccp_job_text_command_rejects_proof_bytes_without_destination_material() {
        let mut ctx = TestContext::with_base_url(
            CliOutputFormat::Text,
            Url::parse("http://127.0.0.1:1").expect("test base url"),
        );
        let mut args = artifact_args("11".repeat(32));
        args.proof_bytes_hex = Some(format!("0x{}", sample_sccp_groth16_proof_hex()));

        let err = Command::Sccp(SccpCommand::Job(args))
            .run(&mut ctx)
            .expect_err("proof bytes without destination material must fail before request");

        assert!(
            err.to_string()
                .contains("deployment destination fields are required"),
            "unexpected error: {err:?}"
        );
        assert!(
            ctx.printed_lines.is_empty(),
            "invalid query params must not print a job summary"
        );
    }

    #[test]
    fn sccp_job_text_command_rejects_short_proof_bytes_before_request() {
        let mut ctx = TestContext::with_base_url(
            CliOutputFormat::Text,
            Url::parse("http://127.0.0.1:1").expect("test base url"),
        );
        let mut args = artifact_args("11".repeat(32));
        args.proof_bytes_hex = Some("0x0102AB".to_owned());

        let err = Command::Sccp(SccpCommand::Job(args))
            .run(&mut ctx)
            .expect_err("short proof bytes must fail before request");

        assert!(
            err.to_string().contains("proof_bytes_hex"),
            "unexpected error: {err:?}"
        );
        assert!(
            err.to_string().contains("384-byte"),
            "unexpected error: {err:?}"
        );
        assert!(
            ctx.printed_lines.is_empty(),
            "invalid query params must not print a job summary"
        );
    }
}
