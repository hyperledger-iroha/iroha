use clap::Subcommand;
use eyre::Result;
use iroha::{
    client::{
        SccpCapabilities, SccpMessageProofQueryParams, SccpProofManifestSet, SccpRecentMessages,
        SccpRecentMessagesQuery,
    },
    data_model::prelude::*,
};

use crate::{CliOutputFormat, Run, RunContext};

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Emit a bridge receipt as a typed event
    EmitReceipt(EmitReceiptArgs),
    /// Inspect exact-lane SCCP proof-discovery and outbound artifact surfaces
    #[command(subcommand)]
    Sccp(SccpCommand),
}

#[derive(Subcommand, Debug)]
pub enum SccpCommand {
    /// Fetch the public SCCP capability snapshot
    Capabilities,
    /// Fetch exact inbound native-admission and outbound destination manifests
    Manifests,
    /// Discover newest-first SORA-origin messages with exact lane and binding context
    Recent(RecentArgs),
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
    /// TRON verifier contract address
    #[arg(long, value_name = "ADDRESS")]
    tron_verifier_address: Option<String>,
    /// TRON route bridge contract address
    #[arg(long, value_name = "ADDRESS")]
    tron_bridge_address: Option<String>,
    /// Externally generated 384-byte Groth16 ABI proof tuple (hex)
    #[arg(long, value_name = "HEX")]
    proof_bytes_hex: Option<String>,
}

#[derive(clap::Args, Debug)]
pub struct RecentArgs {
    /// Inclusive block height from which to scan backwards.
    #[arg(long)]
    from: Option<u64>,
    /// Maximum number of messages to return; the node caps this at 50.
    #[arg(long)]
    limit: Option<u32>,
}

impl RecentArgs {
    fn query(&self) -> SccpRecentMessagesQuery {
        SccpRecentMessagesQuery {
            from: self.from,
            limit: self.limit,
        }
    }
}

impl ArtifactArgs {
    fn proof_query_params(&self) -> SccpMessageProofQueryParams {
        SccpMessageProofQueryParams {
            network_id_hex: self.network_id_hex.clone(),
            verifier_address_hex: self.verifier_address_hex.clone(),
            bridge_address_hex: self.bridge_address_hex.clone(),
            verifier_code_hash_hex: self.verifier_code_hash_hex.clone(),
            verifier_key_hash_hex: self.verifier_key_hash_hex.clone(),
            tron_verifier_address: self.tron_verifier_address.clone(),
            tron_bridge_address: self.tron_bridge_address.clone(),
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
                SccpCommand::Recent(args) => sccp_recent(context, args),
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

fn sccp_recent(ctx: &mut impl RunContext, args: RecentArgs) -> Result<()> {
    let query = args.query();
    match ctx.output_format() {
        CliOutputFormat::Text => {
            let messages = ctx
                .client_from_config()
                .get_sccp_recent_messages_with_query(query)?;
            ctx.println(render_sccp_recent_messages_summary(&messages))
        }
        CliOutputFormat::Json => {
            let messages = ctx
                .client_from_config()
                .get_sccp_recent_messages_json_with_query(query)?;
            ctx.print_data(&messages)
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
    let inbound_lanes = capabilities
        .inbound_lanes
        .iter()
        .map(|lane| {
            let admission = lane.native_admission.as_ref().map_or_else(
                || "staged/no-native-anchor".to_owned(),
                |native| {
                    format!(
                        "{}/anchor={}",
                        native.backend_label, native.trust_anchor_hash
                    )
                },
            );
            format!(
                "{}({})->{}({}) identity={} admission={}:{}",
                lane.source_profile,
                lane.source_domain,
                lane.target_profile,
                lane.target_domain,
                lane.source_identity_hash,
                lane.admission_enabled,
                admission,
            )
        })
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        "sccp capabilities: version={} registry_revision={} payloads={} codecs={} bundle={} artifact={} job={} recent={} manifests={} native_message_submit={} inbound_lanes=[{}]",
        capabilities.version,
        capabilities.registry_revision,
        payloads,
        codecs,
        capabilities.outbound.message_bundle_path,
        capabilities.outbound.proof_artifact_path,
        capabilities.outbound.proof_job_path,
        capabilities.outbound.recent_messages_path,
        capabilities.outbound.manifest_path,
        capabilities
            .native_message_submit_path
            .as_deref()
            .unwrap_or("disabled"),
        inbound_lanes
    )
}

fn render_sccp_manifests_summary(manifests: &SccpProofManifestSet) -> String {
    let mut lines = vec![format!(
        "sccp manifests: version={} registry_revision={} inbound={} outbound={}",
        manifests.version,
        manifests.registry_revision,
        manifests.inbound_native_lanes.len(),
        manifests.outbound_destination_routes.len()
    )];
    lines.extend(manifests.inbound_native_lanes.iter().map(|lane| {
        let native = lane.native_admission.as_ref().map_or_else(
            || "none".to_owned(),
            |admission| {
                format!(
                    "{}/anchor={}",
                    admission.backend_label, admission.trust_anchor_hash
                )
            },
        );
        format!(
            "inbound {}({})->{}({}) identity={} enabled={} native={} builder={}",
            lane.source_profile,
            lane.source_domain,
            lane.target_profile,
            lane.target_domain,
            lane.source_identity_hash,
            lane.admission_enabled,
            native,
            lane.native_proof_builder
                .as_ref()
                .map_or("none", |builder| builder.module_url.as_str())
        )
    }));
    lines.extend(manifests.outbound_destination_routes.iter().map(|route| {
        format!(
            "outbound {}({})->{}({}) route={}/{} verifier={:?}:{} code_hash={} key_hash={} binding={}:{} prover={}",
            route.source_profile,
            route.source_domain,
            route.target_profile,
            route.target_domain,
            route.route_id,
            route.asset_key,
            route.verifier_plan,
            route.verifier_identity,
            route.verifier_code_hash,
            route.verifier_key_hash.as_deref().unwrap_or("none"),
            route.destination_binding_key,
            route.destination_binding_hash,
            route
                .browser_prover
                .as_ref()
                .map_or("none", |builder| builder.module_url.as_str())
        )
    }));
    lines.join("\n")
}

fn render_sccp_recent_messages_summary(messages: &SccpRecentMessages) -> String {
    let mut lines = vec![format!(
        "sccp recent messages: count={}",
        messages.items.len()
    )];
    lines.extend(messages.items.iter().map(|message| {
        format!(
            "height={} id={} kind={} {}->{} target_domain={} binding={} route={} asset={} amount={} bundle={}",
            message.height,
            message.message_id_hex,
            message.kind,
            message.source_profile,
            message.target_profile,
            message.target_domain,
            message.destination_binding_hash,
            message.route_id.as_deref().unwrap_or("none"),
            message.asset_id.as_deref().unwrap_or("none"),
            message.amount.as_deref().unwrap_or("none"),
            message.links.bundle_path,
        )
    }));
    lines.join("\n")
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
        "sccp job: message_id={} payload={} chain={}({}) backend={} verifier_backend={} registry={} security={:?} anchors={:?} binding={} verifier={:?} projection={} submit={}{} package={}/{}",
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
        iroha_sccp::SccpNormalizedCodecValueV1::CanonicalText { value } => {
            format!("canonical_text:{value}")
        }
        iroha_sccp::SccpNormalizedCodecValueV1::EvmAddress20 { bytes } => {
            format!("evm:0x{}", hex::encode(bytes))
        }
        iroha_sccp::SccpNormalizedCodecValueV1::SolanaPubkey32 { bytes } => {
            format!("solana:{}", bs58::encode(bytes).into_string())
        }
        iroha_sccp::SccpNormalizedCodecValueV1::TonAccount36 { workchain, account } => {
            format!("ton:{workchain}:{}", hex::encode(account))
        }
        iroha_sccp::SccpNormalizedCodecValueV1::TronAddress21 { bytes } => {
            format!("tron:{}", hex::encode(bytes))
        }
        iroha_sccp::SccpNormalizedCodecValueV1::SoraAssetId { bytes } => {
            format!("sora_asset_id:0x{}", hex::encode(bytes))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use iroha::client::{
        SccpCodecCapability, SccpExactInboundLaneCapability, SccpNativeAdmissionCapability,
        SccpOutboundProofCapability, SccpRecentMessage, SccpRecentMessageLinks,
    };
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

    fn checked_bridge_cli_ed25519_key_fixture() -> KeyPair {
        KeyPair::try_random_with_algorithm(Algorithm::Ed25519)
            .expect("generate checked bridge CLI Ed25519 key fixture")
    }

    fn checked_bridge_cli_seeded_ed25519_key_fixture(seed_byte: u8) -> KeyPair {
        KeyPair::try_from_seed(vec![seed_byte; 32], Algorithm::Ed25519)
            .expect("derive checked bridge CLI Ed25519 key fixture")
    }

    #[test]
    fn bridge_cli_fixture_uses_checked_ed25519_key_generation() {
        let key_pair = checked_bridge_cli_ed25519_key_fixture();
        let algorithm = key_pair
            .public_key()
            .try_algorithm()
            .expect("bridge CLI fixture key advertises a valid algorithm");

        assert_eq!(algorithm, Algorithm::Ed25519);
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
            let key_pair = checked_bridge_cli_ed25519_key_fixture();
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
        use iroha::data_model::bridge::{
            BridgeNativeProofBackendV1, SccpEvmSourceEmitterV1, SccpLaneIdV1, SccpNetworkV1,
            SccpSourceEmitterV1, SccpSourceIdentityV1,
        };

        let lane = SccpLaneIdV1 {
            source: SccpNetworkV1::EthereumSepolia,
            target: SccpNetworkV1::SoraTaira,
        };
        SccpCapabilities {
            version: 1,
            registry_revision: format!("0x{}", "91".repeat(32)),
            native_message_submit_path: Some("/v1/bridge/messages".to_owned()),
            outbound: SccpOutboundProofCapability {
                message_bundle_path: "/v1/sccp/proofs/message/{message_id}".to_owned(),
                proof_artifact_path: "/v1/sccp/artifacts/message/{message_id}".to_owned(),
                proof_job_path: "/v1/sccp/jobs/message/{message_id}".to_owned(),
                recent_messages_path: "/v1/sccp/messages/recent".to_owned(),
                manifest_path: "/v1/sccp/manifests".to_owned(),
            },
            message_payload_kinds: vec![
                "asset_register".to_owned(),
                "route_activate".to_owned(),
                "transfer".to_owned(),
            ],
            codecs: vec![
                SccpCodecCapability {
                    id: iroha_sccp::SCCP_CODEC_CANONICAL_TEXT,
                    key: "canonical_text".to_owned(),
                    description:
                        "Non-empty printable ASCII bytes for canonical SORA accounts and route-local names."
                            .to_owned(),
                },
                SccpCodecCapability {
                    id: iroha_sccp::SCCP_CODEC_EVM_ADDRESS20,
                    key: "evm_address20".to_owned(),
                    description: "Raw nonzero 20-byte EVM account addresses.".to_owned(),
                },
            ],
            inbound_lanes: vec![SccpExactInboundLaneCapability {
                source_profile: lane.source.profile_key().to_owned(),
                target_profile: lane.target.profile_key().to_owned(),
                source_domain: lane.source.domain_id(),
                target_domain: lane.target.domain_id(),
                source_identity_hash: format!("0x{}", "a1".repeat(32)),
                source_identity: SccpSourceIdentityV1 {
                    lane,
                    emitter: SccpSourceEmitterV1::Evm(SccpEvmSourceEmitterV1 {
                        address: [0x11; 20],
                        runtime_code_hash: [0x22; 32],
                        route_config_hash: [0x33; 32],
                    }),
                },
                admission_enabled: true,
                native_admission: Some(SccpNativeAdmissionCapability {
                    backend: BridgeNativeProofBackendV1::EthereumBeacon,
                    backend_label: BridgeNativeProofBackendV1::EthereumBeacon
                        .backend_label()
                        .to_owned(),
                    trust_anchor_hash: format!("0x{}", "b2".repeat(32)),
                }),
                native_proof_builder: None,
            }],
        }
    }

    fn sample_sccp_proof_manifests_json() -> JsonValue {
        JsonValue::Object(norito::json::Map::from_iter([
            ("version".into(), JsonValue::from(1_u8)),
            (
                "registry_revision".into(),
                JsonValue::from(format!("0x{}", "91".repeat(32))),
            ),
            ("inbound_native_lanes".into(), JsonValue::Array(Vec::new())),
            (
                "outbound_destination_routes".into(),
                JsonValue::Array(vec![JsonValue::Object(norito::json::Map::from_iter([
                    ("source_profile".into(), JsonValue::from("sora-taira")),
                    ("target_profile".into(), JsonValue::from("tron-nile")),
                    ("route_id".into(), JsonValue::from("taira-tron-nile")),
                ]))]),
            ),
        ]))
    }

    fn sample_sccp_recent_messages() -> SccpRecentMessages {
        SccpRecentMessages {
            items: vec![SccpRecentMessage {
                height: 42,
                message_id_hex: format!("0x{}", "67".repeat(32)),
                kind: "transfer".to_owned(),
                source_profile: "sora-taira".to_owned(),
                target_profile: "tron-nile".to_owned(),
                destination_binding_hash: format!("0x{}", "56".repeat(32)),
                target_domain: iroha_sccp::SCCP_DOMAIN_TRON,
                counterparty_domain: iroha_sccp::SCCP_DOMAIN_TRON,
                asset_id: Some("xor#universal".to_owned()),
                route_id: Some("taira-tron-nile".to_owned()),
                recipient: Some("TJRabPrwbZy45sbavfcjinPJC18kjpRTv8".to_owned()),
                amount: Some("77".to_owned()),
                payload_projection: None,
                links: SccpRecentMessageLinks {
                    bundle_path: format!("/v1/sccp/proofs/message/{}", "67".repeat(32)),
                    artifact_path: format!("/v1/sccp/artifacts/message/{}", "67".repeat(32)),
                    job_path: format!("/v1/sccp/jobs/message/{}", "67".repeat(32)),
                },
            }],
        }
    }

    fn sample_sccp_message_proof_artifact() -> iroha_sccp::NexusSccpMessageTransparentProofV1 {
        use iroha_sccp::{
            NexusBridgeFinalityProofV1, NexusCommitQcV1, NexusConsensusPhaseV1,
            NexusSccpMessageProofV1, SccpLaneIdV1, SccpMerkleProofV1, SccpNetworkV1,
            SccpOutboundMessageContextV1, SccpPayloadV1, TransferPayloadV1,
            hub_commitment_from_sccp_payload, merkle_root_from_commitment,
        };

        let validator_public_keys = vec![
            checked_bridge_cli_seeded_ed25519_key_fixture(0x5A)
                .public_key()
                .to_string(),
        ];
        let validator_set = validator_public_keys
            .iter()
            .map(|key| {
                key.parse::<iroha_crypto::PublicKey>()
                    .expect("sample validator public key should parse")
            })
            .map(PeerId::from)
            .collect::<Vec<_>>();
        let validator_set_hash = iroha_crypto::HashOf::<Vec<PeerId>>::new(&validator_set);
        let mut validator_set_hash_bytes = [0u8; 32];
        validator_set_hash_bytes.copy_from_slice(validator_set_hash.as_ref().as_ref());

        let manifest = iroha_sccp::sccp_proof_manifest_for_domain(iroha_sccp::SCCP_DOMAIN_SOL)
            .expect("solana manifest");
        let payload = SccpPayloadV1::Transfer(TransferPayloadV1 {
            version: 1,
            source_domain: iroha_sccp::SCCP_DOMAIN_SORA,
            dest_domain: iroha_sccp::SCCP_DOMAIN_SOL,
            nonce: 21,
            asset_home_domain: iroha_sccp::SCCP_DOMAIN_SORA,
            asset_id_codec: iroha_sccp::SCCP_CODEC_CANONICAL_TEXT,
            asset_id: b"xor#universal".to_vec(),
            amount: 77,
            sender_codec: iroha_sccp::SCCP_CODEC_CANONICAL_TEXT,
            sender: b"nexus:soraswap".to_vec(),
            recipient_codec: iroha_sccp::SCCP_CODEC_SOLANA_PUBKEY32,
            recipient: vec![0x11; 32],
            route_id_codec: iroha_sccp::SCCP_CODEC_CANONICAL_TEXT,
            route_id: b"nexus:sol:xor".to_vec(),
        });
        let context = SccpOutboundMessageContextV1::new(
            SccpLaneIdV1 {
                source: SccpNetworkV1::SoraNexus,
                target: SccpNetworkV1::SolanaMainnetBeta,
            },
            manifest.destination_binding.binding_hash,
        )
        .expect("valid exact outbound fixture context");
        let commitment = hub_commitment_from_sccp_payload(context, &payload)
            .expect("valid exact outbound fixture commitment");
        let merkle_proof = SccpMerkleProofV1 { steps: Vec::new() };
        let commitment_root = merkle_root_from_commitment(&commitment, &merkle_proof);
        let mut block_header = iroha::data_model::block::BlockHeader::new(
            core::num::NonZeroU64::new(19).expect("non-zero finality height"),
            None,
            None,
            None,
            0,
            0,
        );
        block_header.set_sccp_commitment_root(Some(commitment_root));
        let mut block_hash = [0u8; 32];
        block_hash.copy_from_slice(block_header.hash().as_ref().as_ref());
        let block_header_bytes =
            norito::to_bytes(&block_header).expect("encode finality block header");
        let finality_proof = NexusBridgeFinalityProofV1 {
            version: 1,
            chain_id: iroha_sccp::SCCP_NEXUS_FINALITY_CHAIN_ID_V1.to_owned(),
            height: 19,
            block_hash,
            commitment_root,
            block_header_bytes,
            commit_qc: NexusCommitQcV1 {
                version: 1,
                phase: NexusConsensusPhaseV1::Commit,
                height: 19,
                view: 1,
                epoch: 1,
                mode_tag: "normal".to_owned(),
                subject_block_hash: block_hash,
                parent_state_root: [0u8; 32],
                post_state_root: [0u8; 32],
                chain_order_hash: [0u8; 32],
                rechain_seq: 0,
                highest_qc: None,
                validator_set_hash: validator_set_hash_bytes,
                validator_set_hash_version: 1,
                validator_public_keys,
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
        assert!(
            iroha_sccp::verify_message_bundle_structure(&bundle),
            "sample SCCP message bundle must satisfy current structure gates"
        );
        assert!(
            iroha_sccp::sccp_message_transparent_public_inputs(&bundle).is_some(),
            "sample SCCP message bundle must derive transparent public inputs"
        );
        assert!(
            iroha_sccp::build_sccp_message_transparent_open_verify_summary_from_bundle(&bundle)
                .is_some(),
            "sample SCCP message bundle must build transparent OpenVerify proof bytes"
        );

        let public_inputs = iroha_sccp::sccp_message_transparent_public_inputs(&bundle)
            .expect("sample SCCP message bundle public inputs");
        let public_inputs_bytes =
            iroha_sccp::canonical_sccp_message_transparent_public_inputs_bytes(&public_inputs);
        let bundle_bytes = iroha_sccp::canonical_nexus_sccp_message_bundle_bytes_checked(&bundle)
            .expect("sample SCCP message bundle bytes");
        let inner = iroha_sccp::build_sccp_message_transparent_inner_proof_from_bundle(&bundle)
            .expect("sample SCCP message transparent inner proof");
        let proof_bytes = vec![0xA5, 0x5A, 0xC3, 0x3C];
        let destination_binding = manifest.destination_binding.clone();
        let destination_binding_hash = destination_binding.binding_hash;
        let proof_context_hash = iroha_sccp::sccp_solana_proof_context_hash(
            inner.statement_hash,
            destination_binding_hash,
        );
        let platform_payload =
            iroha_sccp::SccpPlatformSubmissionPayloadV1::SolanaProgramInstruction(
                iroha_sccp::SccpSolanaProgramSubmissionPayloadV1 {
                    proof_bytes: proof_bytes.clone(),
                    public_inputs_bytes: public_inputs_bytes.clone(),
                    bundle_bytes: bundle_bytes.clone(),
                    destination_binding: destination_binding.clone(),
                    destination_binding_hash,
                    statement_hash: inner.statement_hash,
                    proof_context_hash,
                },
            );
        let arguments = vec![
            iroha_sccp::SccpSubmissionArgumentValueV1 {
                key: "proof_bytes".to_owned(),
                encoding: "raw_bytes".to_owned(),
                bytes: proof_bytes.clone(),
            },
            iroha_sccp::SccpSubmissionArgumentValueV1 {
                key: "public_inputs".to_owned(),
                encoding: "raw_bytes".to_owned(),
                bytes: public_inputs_bytes,
            },
            iroha_sccp::SccpSubmissionArgumentValueV1 {
                key: "bundle_bytes".to_owned(),
                encoding: "raw_bytes".to_owned(),
                bytes: bundle_bytes,
            },
            iroha_sccp::SccpSubmissionArgumentValueV1 {
                key: "statement_hash".to_owned(),
                encoding: "raw_bytes".to_owned(),
                bytes: inner.statement_hash.to_vec(),
            },
            iroha_sccp::SccpSubmissionArgumentValueV1 {
                key: "destination_binding_hash".to_owned(),
                encoding: "raw_bytes".to_owned(),
                bytes: destination_binding_hash.to_vec(),
            },
            iroha_sccp::SccpSubmissionArgumentValueV1 {
                key: "proof_context_hash".to_owned(),
                encoding: "raw_bytes".to_owned(),
                bytes: proof_context_hash.to_vec(),
            },
        ];
        let submission_package = iroha_sccp::SccpCounterpartySubmissionPackageV1 {
            version: 1,
            proof_family: manifest.proof_family.clone(),
            verifier_backend: manifest.verifier_backend.clone(),
            envelope_encoding: manifest.submission_template.encoding.clone(),
            submission_kind: manifest.submission_template.submission_kind.clone(),
            verifier_entrypoint: manifest.submission_template.verifier_entrypoint.clone(),
            platform_payload,
            arguments,
            envelope_bytes: b"submit_sccp_message_proof".to_vec(),
        };
        iroha_sccp::NexusSccpMessageTransparentProofV1 {
            version: 1,
            local_domain: manifest.local_domain,
            counterparty_domain: manifest.counterparty_domain,
            security_model: manifest.security_model,
            anchor_governance: manifest.anchor_governance,
            destination_binding,
            proof_family: manifest.proof_family,
            verifier_backend: manifest.verifier_backend,
            message_backend: manifest.message_backend,
            registry_backend: manifest.registry_backend,
            manifest_seed: manifest.manifest_seed,
            verifier_target: manifest.verifier_target,
            public_inputs,
            proof_bytes,
            submission_package,
            bundle,
        }
    }

    fn sample_sccp_message_job() -> iroha_sccp::SccpCounterpartyProofJobV1 {
        let artifact = sample_sccp_message_proof_artifact();
        let manifest = iroha_sccp::sccp_proof_manifest_for_domain(iroha_sccp::SCCP_DOMAIN_SOL)
            .expect("solana manifest");
        iroha_sccp::SccpCounterpartyProofJobV1 {
            version: 1,
            chain_family: iroha_sccp::SccpTransparentChainFamilyV1::Solana,
            chain: "sol".to_owned(),
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
            tron_verifier_address: None,
            tron_bridge_address: None,
            proof_bytes_hex: None,
        }
    }

    fn sccp_groth16_abi_word_hex(value: u32) -> String {
        format!("{value:064x}")
    }

    fn sample_sccp_groth16_proof_hex_for_message_id(message_id_hex: &str) -> String {
        [
            sccp_groth16_abi_word_hex(1),
            message_id_hex.trim_start_matches("0x").to_ascii_lowercase(),
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

    fn sample_sccp_groth16_proof_hex() -> String {
        sample_sccp_groth16_proof_hex_for_message_id(&"11".repeat(32))
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
        assert!(rendered.contains("version=1"));
        assert!(rendered.contains("registry_revision=0x9191"));
        assert!(rendered.contains("ethereum-sepolia(1)->sora-taira(0)"));
        assert!(rendered.contains("bridge/sccp/native/ethereum-beacon-v1"));
        assert!(!rendered.contains("proof_family="));
        assert!(!rendered.contains("burn"));
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
        assert_eq!(payload.get("version").and_then(JsonValue::as_u64), Some(1));
        assert_eq!(
            payload
                .get("outbound_destination_routes")
                .and_then(JsonValue::as_array)
                .map(Vec::len),
            Some(1)
        );
        assert!(payload.get("local_chain").is_none());
        assert!(payload.get("proof_family").is_none());
    }

    #[test]
    fn sccp_recent_text_command_prints_exact_context() {
        let _guard = mock_http_server_guard();
        let messages = sample_sccp_recent_messages();
        let server = MockHttpServer::start(BTreeMap::from([(
            "/v1/sccp/messages/recent?from=42&limit=7".to_owned(),
            MockHttpResponse {
                content_type: "application/x-norito",
                body: norito::to_bytes(&messages).expect("encode recent SCCP messages"),
            },
        )]));
        let mut ctx = TestContext::with_base_url(CliOutputFormat::Text, server.base_url.clone());

        Command::Sccp(SccpCommand::Recent(RecentArgs {
            from: Some(42),
            limit: Some(7),
        }))
        .run(&mut ctx)
        .expect("run recent SCCP command");

        let rendered = ctx.printed_lines.join("\n");
        assert!(rendered.contains("sccp recent messages: count=1"));
        assert!(rendered.contains("sora-taira->tron-nile"));
        assert!(rendered.contains(&format!("binding=0x{}", "56".repeat(32))));
        assert!(rendered.contains("route=taira-tron-nile"));
    }

    #[test]
    fn sccp_recent_json_command_preserves_exact_context() {
        let _guard = mock_http_server_guard();
        let messages = sample_sccp_recent_messages();
        let json = norito::json::value::to_value(&messages).expect("recent SCCP JSON");
        let server = MockHttpServer::start(BTreeMap::from([(
            "/v1/sccp/messages/recent".to_owned(),
            MockHttpResponse {
                content_type: "application/json",
                body: norito::json::to_vec(&json).expect("encode recent SCCP JSON"),
            },
        )]));
        let mut ctx = TestContext::with_base_url(CliOutputFormat::Json, server.base_url.clone());

        Command::Sccp(SccpCommand::Recent(RecentArgs {
            from: None,
            limit: None,
        }))
        .run(&mut ctx)
        .expect("run recent SCCP JSON command");

        let rendered = ctx.printed_json.expect("printed recent SCCP JSON");
        let item = rendered
            .get("items")
            .and_then(JsonValue::as_array)
            .and_then(|items| items.first())
            .expect("one recent SCCP item");
        assert_eq!(
            item.get("source_profile").and_then(JsonValue::as_str),
            Some("sora-taira")
        );
        assert_eq!(
            item.get("target_profile").and_then(JsonValue::as_str),
            Some("tron-nile")
        );
    }

    #[test]
    fn sccp_artifact_text_command_prints_typed_snapshot_summary() {
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
        assert!(rendered.contains("chain=sol(3)"));
        assert!(rendered.contains("verifier_backend=solana-program-v1"));
        assert!(rendered.contains("finality_height=19"));
        assert!(rendered.contains("inner_family=Solana"));
        assert!(rendered.contains("inner_payload=transfer"));
        assert!(!rendered.contains("open_verify=stark/sccp-message-transparent-v1"));
        assert!(rendered.contains("package=program_instruction/borsh_instruction_v1"));
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
            args.tron_bridge_address = Some("TJk5a8Y1bWkUxqLeBEKiyLEJD2ytoBrsa9".to_owned());
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
    fn sccp_job_text_command_prints_typed_snapshot_summary() {
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
        assert!(rendered.contains("chain=sol(3)"));
        assert!(rendered.contains("verifier_backend=solana-program-v1"));
        assert!(rendered.contains("projection=transfer"));
        assert!(rendered.contains("recipient=solana:11111111111111111111111111111111"));
        assert!(rendered.contains(
            "submit=program_instruction/borsh_instruction_v1/submit_sccp_message_proof args=[proof_bytes,public_inputs,bundle_bytes,statement_hash,destination_binding_hash,proof_context_hash]"
        ));
        assert!(rendered.contains("open_verify=stark/sccp-message-transparent-v1"));
        assert!(rendered.contains("package=program_instruction/borsh_instruction_v1"));
    }

    #[test]
    fn sccp_job_text_command_forwards_tron_query_material() {
        let _guard = mock_http_server_guard();
        let job = sample_sccp_message_job();
        let message_id_hex = hex::encode(job.public_inputs.message_id);
        let network_id_hex = "11".repeat(32);
        let verifier_code_hash_hex = "ab".repeat(32);
        let verifier_key_hash_hex = "cd".repeat(32);
        let proof_bytes_hex = sample_sccp_groth16_proof_hex_for_message_id(&message_id_hex);
        let query = format!(
            "network_id_hex={network_id_hex}&verifier_code_hash_hex={verifier_code_hash_hex}&verifier_key_hash_hex={verifier_key_hash_hex}&tron_verifier_address=TJRabPrwbZy45sbavfcjinPJC18kjpRTv8&tron_bridge_address=TJk5a8Y1bWkUxqLeBEKiyLEJD2ytoBrsa9&proof_bytes_hex={proof_bytes_hex}"
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
        args.tron_verifier_address = Some("  TJRabPrwbZy45sbavfcjinPJC18kjpRTv8  ".to_owned());
        args.tron_bridge_address = Some("  TJk5a8Y1bWkUxqLeBEKiyLEJD2ytoBrsa9  ".to_owned());
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
        args.tron_verifier_address = Some("TJRabPrwbZy45sbavfcjinPJC18kjpRTv8".to_owned());
        args.tron_bridge_address = Some("TJk5a8Y1bWkUxqLeBEKiyLEJD2ytoBrsa9".to_owned());

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
        mixed_evm_tron.tron_verifier_address =
            Some("TJRabPrwbZy45sbavfcjinPJC18kjpRTv8".to_owned());
        mixed_evm_tron.tron_bridge_address = Some("TJk5a8Y1bWkUxqLeBEKiyLEJD2ytoBrsa9".to_owned());
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
            args.tron_bridge_address = Some("TJk5a8Y1bWkUxqLeBEKiyLEJD2ytoBrsa9".to_owned());
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
