//! Bridge and exact first-release SCCP commands.

use std::{
    fs,
    path::{Path, PathBuf},
};

use base64::Engine as _;
use clap::Subcommand;
use eyre::{Result, WrapErr as _, eyre};
use iroha::{
    client::{
        SccpBridgeSubmitResponse, SccpCapabilities, SccpDestinationProofSubmitRequest,
        SccpNativeMessageSubmitRequest, SccpRecentMessages, SccpRecentMessagesQuery,
        SccpRegistryLimits, SccpResourceLimits,
    },
    data_model::{bridge::SccpRegistryV1, prelude::*},
};

use crate::{CliOutputFormat, Run, RunContext};

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Emit a bridge receipt as a typed event.
    EmitReceipt(EmitReceiptArgs),
    /// Inspect the exact transfer-only SCCP registry and proof inputs.
    #[command(subcommand)]
    Sccp(SccpCommand),
}
#[derive(Subcommand, Debug)]
pub enum SccpCommand {
    /// Fetch the closed first-release SCCP HTTP surface.
    Capabilities,
    /// Fetch the authoritative typed SCCP route registry.
    Registry,
    /// Discover newest-first finalized SORA-origin messages.
    Recent(RecentArgs),
    /// Fetch one finalized canonical SCCP message bundle.
    Bundle(MessageArgs),
    /// Fetch the exact state-derived Groth16 prover request for one message.
    ProofRequest(MessageArgs),
    /// Prepare or directly submit one closed destination-proof artifact.
    SubmitDestinationProof(SubmitDestinationProofArgs),
    /// Prepare or directly submit one protocol-native inbound proof.
    SubmitNativeMessage(SubmitNativeMessageArgs),
}
#[derive(clap::Args, Debug, Clone)]
pub struct DetachedSubmitArgs {
    /// File containing the exact canonical padded-base64 transaction payload returned by prepare.
    #[arg(
        long,
        value_name = "PATH",
        requires_all = ["signature_b64_file", "creation_time_ms"]
    )]
    transaction_payload_b64_file: Option<PathBuf>,
    /// File containing one canonical padded-base64 detached signature over the prepared payload hash.
    #[arg(
        long,
        value_name = "PATH",
        requires_all = ["transaction_payload_b64_file", "creation_time_ms"]
    )]
    signature_b64_file: Option<PathBuf>,
    /// Positive transaction creation timestamp in Unix milliseconds.
    ///
    /// Direct submission must repeat the value returned by preparation.
    #[arg(long)]
    creation_time_ms: Option<u64>,
}

#[derive(clap::Args, Debug)]
pub struct SubmitDestinationProofArgs {
    /// File containing one canonical Norito SCCP Groth16 destination artifact.
    #[arg(long, value_name = "PATH")]
    artifact: PathBuf,
    #[command(flatten)]
    detached: DetachedSubmitArgs,
}

#[derive(clap::Args, Debug)]
pub struct SubmitNativeMessageArgs {
    /// File containing one canonical Norito protocol-native SCCP inbound proof.
    #[arg(long, value_name = "PATH")]
    proof: PathBuf,
    #[command(flatten)]
    detached: DetachedSubmitArgs,
}

#[derive(clap::Args, Debug)]
pub struct EmitReceiptArgs {
    /// Bridge lane id (numeric).
    #[arg(long)]
    lane: u32,
    /// Direction: lock|mint|burn|release.
    #[arg(long)]
    direction: String,
    /// Source transaction hash (hex, 32 bytes).
    #[arg(long)]
    source_tx: String,
    /// Amount in integer asset units.
    #[arg(long)]
    amount: u128,
    /// Canonical Iroha asset id.
    #[arg(long)]
    asset_id: String,
    /// Iroha account id or external address payload.
    #[arg(long)]
    recipient: String,
    /// Optional destination transaction hash (hex, 32 bytes).
    #[arg(long)]
    dest_tx: Option<String>,
    /// Proof hash (hex, 32 bytes).
    #[arg(long)]
    proof_hash: Option<String>,
}

#[derive(clap::Args, Debug)]
pub struct MessageArgs {
    /// Nonzero SCCP message id (hex, 32 bytes).
    #[arg(long, value_name = "HEX")]
    message_id: String,
}

#[derive(clap::Args, Debug)]
pub struct RecentArgs {
    /// Inclusive block height through which to scan backwards.
    #[arg(long)]
    from: Option<u64>,
    /// Last commitment index already consumed at `--from` (inclusive range `0..=511`).
    #[arg(long, requires = "from")]
    after_index: Option<u32>,
    /// Maximum number of messages to return (inclusive range `1..=50`).
    #[arg(long)]
    limit: Option<u64>,
}

impl RecentArgs {
    fn query(&self) -> SccpRecentMessagesQuery {
        SccpRecentMessagesQuery {
            from: self.from,
            after_index: self.after_index,
            limit: self.limit,
        }
    }
}

impl Run for Command {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        match self {
            Self::EmitReceipt(args) => emit_receipt(context, args),
            Self::Sccp(command) => match command {
                SccpCommand::Capabilities => sccp_capabilities(context),
                SccpCommand::Registry => sccp_registry(context),
                SccpCommand::Recent(args) => sccp_recent(context, args),
                SccpCommand::Bundle(args) => sccp_bundle(context, args),
                SccpCommand::ProofRequest(args) => sccp_proof_request(context, args),
                SccpCommand::SubmitDestinationProof(args) => {
                    sccp_submit_destination_proof(context, args)
                }
                SccpCommand::SubmitNativeMessage(args) => sccp_submit_native_message(context, args),
            },
        }
    }
}

fn hex32(value: &str) -> Result<[u8; 32]> {
    let value = value
        .strip_prefix("0x")
        .or_else(|| value.strip_prefix("0X"))
        .unwrap_or(value);
    if value.len() != 64 {
        return Err(eyre::eyre!(
            "expected exactly 32 hexadecimal bytes, got {} characters",
            value.len()
        ));
    }
    let mut out = [0_u8; 32];
    hex::decode_to_slice(value, &mut out)?;
    Ok(out)
}

fn emit_receipt(ctx: &mut impl RunContext, args: EmitReceiptArgs) -> Result<()> {
    let source_tx = hex32(&args.source_tx)?;
    let dest_tx = args.dest_tx.as_deref().map(hex32).transpose()?;
    let proof_hash = args
        .proof_hash
        .as_deref()
        .map(hex32)
        .transpose()?
        .unwrap_or([0; 32]);
    let receipt = BridgeReceipt {
        lane: LaneId::new(args.lane),
        direction: args.direction.into_bytes(),
        source_tx,
        dest_tx,
        proof_hash,
        amount: args.amount,
        asset_id: args.asset_id.into_bytes(),
        recipient: args.recipient.into_bytes(),
    };
    ctx.finish(vec![InstructionBox::from(RecordBridgeReceipt::new(
        receipt,
    ))])
}

fn sccp_capabilities(ctx: &mut impl RunContext) -> Result<()> {
    let capabilities = ctx.client_from_config().get_sccp_capabilities()?;
    match ctx.output_format() {
        CliOutputFormat::Text => ctx.println(render_sccp_capabilities_summary(&capabilities)),
        CliOutputFormat::Json => ctx.print_data(&capabilities),
    }
}

fn sccp_registry(ctx: &mut impl RunContext) -> Result<()> {
    let registry = ctx.client_from_config().get_sccp_registry()?;
    match ctx.output_format() {
        CliOutputFormat::Text => ctx.println(render_sccp_registry_summary(&registry)),
        CliOutputFormat::Json => ctx.print_data(&registry),
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

fn sccp_bundle(ctx: &mut impl RunContext, args: MessageArgs) -> Result<()> {
    match ctx.output_format() {
        CliOutputFormat::Text => {
            let bundle = ctx
                .client_from_config()
                .get_sccp_message_bundle(&args.message_id)?;
            ctx.println(render_sccp_message_bundle_summary(&bundle))
        }
        CliOutputFormat::Json => {
            let bundle = ctx
                .client_from_config()
                .get_sccp_message_bundle_json(&args.message_id)?;
            ctx.print_data(&bundle)
        }
    }
}

fn sccp_proof_request(ctx: &mut impl RunContext, args: MessageArgs) -> Result<()> {
    match ctx.output_format() {
        CliOutputFormat::Text => {
            let request = ctx
                .client_from_config()
                .get_sccp_groth16_proof_request(&args.message_id)?;
            ctx.println(render_sccp_proof_request_summary(&request))
        }
        CliOutputFormat::Json => {
            let request = ctx
                .client_from_config()
                .get_sccp_groth16_proof_request_json(&args.message_id)?;
            ctx.print_data(&request)
        }
    }
}

const MAX_SCCP_TRANSACTION_PAYLOAD_BYTES: usize = 16 * 1024 * 1024;
const MAX_SCCP_DETACHED_SIGNATURE_BYTES: usize = 16 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
struct DetachedSubmitMaterial {
    transaction_payload_b64: Option<String>,
    signature_b64: Option<String>,
    creation_time_ms: Option<u64>,
}

fn read_bounded_binary_artifact(path: &Path, maximum: usize, label: &str) -> Result<Vec<u8>> {
    let metadata = fs::metadata(path)
        .wrap_err_with(|| format!("failed to inspect {label} file `{}`", path.display()))?;
    if metadata.len() == 0 || metadata.len() > maximum as u64 {
        return Err(eyre!(
            "{label} file must contain between 1 and {maximum} bytes"
        ));
    }
    let bytes = fs::read(path)
        .wrap_err_with(|| format!("failed to read {label} file `{}`", path.display()))?;
    if bytes.is_empty() || bytes.len() > maximum {
        return Err(eyre!(
            "{label} file must contain between 1 and {maximum} bytes"
        ));
    }
    Ok(bytes)
}

fn read_canonical_base64_file(path: &Path, maximum: usize, label: &str) -> Result<String> {
    let maximum_encoded = 4 * maximum.div_ceil(3);
    let metadata = fs::metadata(path)
        .wrap_err_with(|| format!("failed to inspect {label} file `{}`", path.display()))?;
    if metadata.len() == 0 || metadata.len() > (maximum_encoded + 2) as u64 {
        return Err(eyre!(
            "{label} file exceeds the {maximum}-byte decoded protocol bound"
        ));
    }
    let mut bytes = fs::read(path)
        .wrap_err_with(|| format!("failed to read {label} file `{}`", path.display()))?;
    if bytes.ends_with(b"\r\n") {
        bytes.truncate(bytes.len() - 2);
    } else if bytes.ends_with(b"\n") {
        bytes.truncate(bytes.len() - 1);
    }
    if bytes.is_empty() || bytes.iter().any(|byte| byte.is_ascii_whitespace()) {
        return Err(eyre!(
            "{label} file must contain exactly one canonical padded-base64 value"
        ));
    }
    let value = String::from_utf8(bytes)
        .map_err(|_| eyre!("{label} file must contain ASCII padded base64"))?;
    let decoded = base64::engine::general_purpose::STANDARD
        .decode(value.as_bytes())
        .map_err(|_| eyre!("{label} file is not valid padded base64"))?;
    if decoded.is_empty()
        || decoded.len() > maximum
        || base64::engine::general_purpose::STANDARD.encode(&decoded) != value
    {
        return Err(eyre!(
            "{label} file must contain one nonempty canonical padded-base64 value within the {maximum}-byte bound"
        ));
    }
    Ok(value)
}

fn validate_detached_signature_matches_payload(
    authority: &AccountId,
    transaction_payload_b64: &str,
    signature_b64: &str,
) -> Result<()> {
    let payload = base64::engine::general_purpose::STANDARD
        .decode(transaction_payload_b64)
        .map_err(|_| eyre!("transaction payload file is not canonical padded base64"))?;
    let signature_bytes = base64::engine::general_purpose::STANDARD
        .decode(signature_b64)
        .map_err(|_| eyre!("signature file is not canonical padded base64"))?;
    let signature = iroha_crypto::Signature::try_from_bytes(&signature_bytes)
        .map_err(|error| eyre!("detached signature failed admission: {error}"))?;
    let signatory = authority.try_signatory().ok_or_else(|| {
        eyre!(
            "direct SCCP submission requires a single-key authority; use the prepared payload with the multisig workflow instead"
        )
    })?;
    let signing_hash = iroha_crypto::Hash::new(&payload);
    signature
        .verify(signatory, signing_hash.as_ref())
        .map_err(|_| {
            eyre!("detached signature does not verify the exact prepared transaction payload")
        })
}

fn load_detached_submit_material(
    args: &DetachedSubmitArgs,
    authority: &AccountId,
) -> Result<DetachedSubmitMaterial> {
    if args.creation_time_ms == Some(0) {
        return Err(eyre!("creation_time_ms must be a positive integer"));
    }
    match (
        args.transaction_payload_b64_file.as_deref(),
        args.signature_b64_file.as_deref(),
    ) {
        (None, None) => Ok(DetachedSubmitMaterial {
            transaction_payload_b64: None,
            signature_b64: None,
            creation_time_ms: args.creation_time_ms,
        }),
        (Some(payload_path), Some(signature_path)) => {
            if args.creation_time_ms.is_none() {
                return Err(eyre!(
                    "direct SCCP submission requires --creation-time-ms from the preparation response"
                ));
            }
            let transaction_payload_b64 = read_canonical_base64_file(
                payload_path,
                MAX_SCCP_TRANSACTION_PAYLOAD_BYTES,
                "transaction payload",
            )?;
            let signature_b64 = read_canonical_base64_file(
                signature_path,
                MAX_SCCP_DETACHED_SIGNATURE_BYTES,
                "detached signature",
            )?;
            validate_detached_signature_matches_payload(
                authority,
                &transaction_payload_b64,
                &signature_b64,
            )?;
            Ok(DetachedSubmitMaterial {
                transaction_payload_b64: Some(transaction_payload_b64),
                signature_b64: Some(signature_b64),
                creation_time_ms: args.creation_time_ms,
            })
        }
        _ => Err(eyre!(
            "preparation requires neither signing file; direct submission requires both --transaction-payload-b64-file and --signature-b64-file"
        )),
    }
}

fn submit_sccp_once<T>(description: &str, submit: impl FnOnce() -> Result<T>) -> Result<T> {
    submit().wrap_err_with(|| format!("{description} failed without retrying or rebuilding"))
}

fn is_nonzero_lower_hex32(value: &str) -> bool {
    value.len() == 64
        && value.bytes().any(|byte| byte != b'0')
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
}

fn validate_sccp_submit_response_state(response: &SccpBridgeSubmitResponse) -> Result<()> {
    if response.payload_kind != "transfer" {
        return Err(eyre!(
            "SCCP submit response payload_kind must be the closed first-release transfer kind"
        ));
    }
    if !is_nonzero_lower_hex32(&response.message_id_hex) {
        return Err(eyre!(
            "SCCP submit response message_id_hex must be a nonzero lowercase 32-byte hash"
        ));
    }
    if response.creation_time_ms == 0 {
        return Err(eyre!(
            "SCCP submit response creation_time_ms must be positive"
        ));
    }
    if response.range_start_height == 0 || response.range_end_height < response.range_start_height {
        return Err(eyre!(
            "SCCP submit response proof height range must be positive and ordered"
        ));
    }
    let route_hash = &response.route_configuration_hash_hex;
    if !is_nonzero_lower_hex32(route_hash) {
        return Err(eyre!(
            "SCCP submit response route_configuration_hash_hex must be a nonzero lowercase 32-byte hash"
        ));
    }
    match (
        response.submitted,
        response.tx_hash_hex.as_ref(),
        response.transaction_payload_b64.as_ref(),
        response.signing_message_b64.as_ref(),
    ) {
        (true, Some(tx_hash), None, None) if is_nonzero_lower_hex32(tx_hash) => Ok(()),
        (false, None, Some(payload_b64), Some(signing_message_b64)) => {
            let payload = base64::engine::general_purpose::STANDARD
                .decode(payload_b64)
                .map_err(|_| eyre!("prepared response transaction_payload_b64 is malformed"))?;
            let signing_message = base64::engine::general_purpose::STANDARD
                .decode(signing_message_b64)
                .map_err(|_| eyre!("prepared response signing_message_b64 is malformed"))?;
            if payload.is_empty()
                || payload.len() > MAX_SCCP_TRANSACTION_PAYLOAD_BYTES
                || signing_message.len() != iroha_crypto::Hash::LENGTH
                || base64::engine::general_purpose::STANDARD.encode(&payload)
                    != payload_b64.as_str()
                || base64::engine::general_purpose::STANDARD.encode(&signing_message)
                    != signing_message_b64.as_str()
                || signing_message.as_slice() != iroha_crypto::Hash::new(&payload).as_ref()
            {
                return Err(eyre!(
                    "prepared SCCP response signing message does not match the exact transaction payload"
                ));
            }
            Ok(())
        }
        _ => Err(eyre!(
            "SCCP submit response has an inconsistent prepared/submitted field state"
        )),
    }
}

fn render_sccp_submit_response(response: &SccpBridgeSubmitResponse) -> Result<String> {
    validate_sccp_submit_response_state(response)?;
    let prefix = format!(
        "sccp submit: submitted={} message_id_hex={} backend={} counterparty_chain={} counterparty_domain={} route_configuration_hash_hex={} range_start_height={} range_end_height={} creation_time_ms={}",
        response.submitted,
        response.message_id_hex,
        response.backend,
        response.counterparty_chain,
        response.counterparty_domain,
        response.route_configuration_hash_hex,
        response.range_start_height,
        response.range_end_height,
        response.creation_time_ms,
    );
    if response.submitted {
        return Ok(format!(
            "{prefix}\ntx_hash_hex={}",
            response
                .tx_hash_hex
                .as_deref()
                .expect("validated submitted response has a transaction hash")
        ));
    }
    Ok(format!(
        "{prefix}\ntransaction_payload_b64={}\nsigning_message_b64={}",
        response
            .transaction_payload_b64
            .as_deref()
            .expect("validated prepared response has a transaction payload"),
        response
            .signing_message_b64
            .as_deref()
            .expect("validated prepared response has a signing message"),
    ))
}

fn print_sccp_submit_response(
    ctx: &mut impl RunContext,
    response: &SccpBridgeSubmitResponse,
) -> Result<()> {
    validate_sccp_submit_response_state(response)?;
    match ctx.output_format() {
        CliOutputFormat::Text => ctx.println(render_sccp_submit_response(response)?),
        CliOutputFormat::Json => ctx.print_data(response),
    }
}

fn sccp_submit_destination_proof(
    ctx: &mut impl RunContext,
    args: SubmitDestinationProofArgs,
) -> Result<()> {
    let authority = ctx.config().account.clone();
    let artifact = read_bounded_binary_artifact(
        &args.artifact,
        iroha_sccp::SCCP_GROTH16_BN254_MAX_ENCODED_ARTIFACT_BYTES_V1,
        "SCCP destination artifact",
    )?;
    let detached = load_detached_submit_material(&args.detached, &authority)?;
    let request = SccpDestinationProofSubmitRequest {
        authority,
        signature_b64: detached.signature_b64,
        transaction_payload_b64: detached.transaction_payload_b64,
        destination_proof_b64: base64::engine::general_purpose::STANDARD.encode(artifact),
        creation_time_ms: detached.creation_time_ms,
    };
    let client = ctx.client_from_config();
    let response = submit_sccp_once("SCCP destination-proof submission", || {
        client.post_sccp_destination_proof(&request)
    })?;
    print_sccp_submit_response(ctx, &response)
}

fn sccp_submit_native_message(
    ctx: &mut impl RunContext,
    args: SubmitNativeMessageArgs,
) -> Result<()> {
    let authority = ctx.config().account.clone();
    let proof = read_bounded_binary_artifact(
        &args.proof,
        iroha_sccp::SCCP_NATIVE_ADMISSION_MAX_ENCODED_BYTES_V1,
        "SCCP native proof",
    )?;
    let detached = load_detached_submit_material(&args.detached, &authority)?;
    let request = SccpNativeMessageSubmitRequest {
        authority,
        signature_b64: detached.signature_b64,
        transaction_payload_b64: detached.transaction_payload_b64,
        native_proof_b64: base64::engine::general_purpose::STANDARD.encode(proof),
        creation_time_ms: detached.creation_time_ms,
    };
    let client = ctx.client_from_config();
    let response = submit_sccp_once("SCCP native-message submission", || {
        client.post_sccp_native_message(&request)
    })?;
    print_sccp_submit_response(ctx, &response)
}

fn render_sccp_capabilities_summary(capabilities: &SccpCapabilities) -> String {
    format!(
        "sccp capabilities: version={} registry_revision={} registry={} bundle={} proof_request={} recent={} proof_submit={} native_submit={}\nregistry_limits: lanes={} live_total={} live_per_lane={} retained_routes_per_lane={} retained_anchors_per_lane={}\nresource_limits: outbound_messages_block={} outbound_payload_bytes={} pending_messages/pending_bytes={}/{} proofs_tx/block={}/{} proof_bytes_each/tx/block={}/{}/{} native_headers_tx/block={}/{} eth_updates_tx/block={}/{} native_bytes_tx/block={}/{} secp_tx/block={}/{} bls_checks_tx/block={}/{} bls_contributions_tx/block={}/{} bn254_tx/block={}/{}",
        capabilities.version,
        capabilities.registry_revision,
        capabilities.registry_path,
        capabilities.message_bundle_path,
        capabilities.proof_request_path,
        capabilities.recent_messages_path,
        capabilities
            .proof_submit_path
            .as_deref()
            .unwrap_or("disabled"),
        capabilities
            .native_message_submit_path
            .as_deref()
            .unwrap_or("disabled"),
        capabilities.registry_limits.max_governed_lanes,
        capabilities.registry_limits.max_live_governed_routes,
        capabilities.registry_limits.max_live_routes_per_lane,
        capabilities.registry_limits.max_retained_routes_per_lane,
        capabilities
            .registry_limits
            .max_retained_native_trust_anchors_per_lane,
        capabilities.resource_limits.max_outbound_messages_per_block,
        capabilities
            .resource_limits
            .max_outbound_message_payload_bytes,
        capabilities.resource_limits.max_pending_outbound_messages,
        capabilities
            .resource_limits
            .max_pending_outbound_payload_bytes,
        capabilities.resource_limits.max_proofs_per_transaction,
        capabilities.resource_limits.max_proofs_per_block,
        capabilities.resource_limits.max_proof_bytes_per_proof,
        capabilities.resource_limits.max_proof_bytes_per_transaction,
        capabilities.resource_limits.max_proof_bytes_per_block,
        capabilities
            .resource_limits
            .max_native_headers_per_transaction,
        capabilities.resource_limits.max_native_headers_per_block,
        capabilities
            .resource_limits
            .max_ethereum_light_client_updates_per_transaction,
        capabilities
            .resource_limits
            .max_ethereum_light_client_updates_per_block,
        capabilities
            .resource_limits
            .max_native_header_bytes_per_transaction,
        capabilities
            .resource_limits
            .max_native_header_bytes_per_block,
        capabilities
            .resource_limits
            .max_secp256k1_recoveries_per_transaction,
        capabilities
            .resource_limits
            .max_secp256k1_recoveries_per_block,
        capabilities
            .resource_limits
            .max_bls_aggregate_checks_per_transaction,
        capabilities
            .resource_limits
            .max_bls_aggregate_checks_per_block,
        capabilities
            .resource_limits
            .max_bls_signer_contributions_per_transaction,
        capabilities
            .resource_limits
            .max_bls_signer_contributions_per_block,
        capabilities
            .resource_limits
            .max_bn254_pairing_checks_per_transaction,
        capabilities
            .resource_limits
            .max_bn254_pairing_checks_per_block,
    )
}

fn render_sccp_registry_summary(registry: &SccpRegistryV1) -> String {
    let route_count = registry
        .lanes
        .iter()
        .map(|lane| lane.routes.len())
        .sum::<usize>();
    let mut lines = vec![format!(
        "sccp registry: version={} lanes={} routes={}",
        registry.version,
        registry.lanes.len(),
        route_count
    )];
    for lane in &registry.lanes {
        let anchor = lane.current_native_trust_anchor().map_or_else(
            || "none".to_owned(),
            |anchor| {
                format!(
                    "{}@{}:{}",
                    anchor.backend.backend_label(),
                    anchor.checkpoint_height,
                    hex::encode(anchor.anchor_hash)
                )
            },
        );
        lines.push(format!(
            "lane {}->{} current_anchor={} retained_anchors={} routes={}",
            lane.lane_id.source.profile_key(),
            lane.lane_id.target.profile_key(),
            anchor,
            lane.native_trust_anchors.len(),
            lane.routes.len()
        ));
        for route in &lane.routes {
            let configuration = route
                .route_configuration_hash()
                .map(hex::encode)
                .unwrap_or_else(|error| format!("invalid:{error}"));
            lines.push(format!(
                "  route={}/{} revision={} activation={:?} configuration={}",
                route.route_id, route.asset_key, route.revision, route.activation, configuration
            ));
        }
    }
    lines.join("\n")
}

fn render_sccp_recent_messages_summary(messages: &SccpRecentMessages) -> String {
    let mut lines = vec![format!(
        "sccp recent messages: count={}",
        messages.items.len()
    )];
    lines.extend(messages.items.iter().map(|message| {
        format!(
            "height={} commitment_index={} id={} kind={} {}->{} target_domain={} binding={} configuration={} route={} asset={} amount={} bundle={} proof_request={}",
            message.height,
            message.commitment_index,
            message.message_id_hex,
            message.kind,
            message.source_profile,
            message.target_profile,
            message.target_domain,
            message.destination_binding_hash,
            message.route_configuration_hash,
            message.route_id.as_deref().unwrap_or("none"),
            message.asset_id.as_deref().unwrap_or("none"),
            message.amount,
            message.links.bundle_path,
            message.links.proof_request_path,
        )
    }));
    if let Some(next) = messages.next {
        lines.push(format!(
            "next: --from {} --after-index {}",
            next.from, next.after_index
        ));
    }
    lines.join("\n")
}

fn render_sccp_message_bundle_summary(bundle: &iroha_sccp::TairaSccpMessageProofV1) -> String {
    let projection = iroha_sccp::sccp_payload_projection(&bundle.payload)
        .map(|projection| render_sccp_payload_projection_summary(&projection))
        .unwrap_or_else(|| "invalid-transfer".to_owned());
    let finality = iroha_sccp::sccp_message_public_inputs(bundle)
        .map(|inputs| {
            format!(
                "height={} block_hash={}",
                inputs.finality_height,
                hex::encode(inputs.finality_block_hash)
            )
        })
        .unwrap_or_else(|| "invalid-finality".to_owned());
    format!(
        "sccp bundle: id={} lane={}->{} binding={} configuration={} root={} {} payload={}",
        hex::encode(bundle.commitment.message_id),
        bundle.commitment.context.lane.source.profile_key(),
        bundle.commitment.context.lane.target.profile_key(),
        hex::encode(bundle.commitment.context.destination_binding_hash),
        hex::encode(bundle.commitment.context.route_configuration_hash),
        hex::encode(bundle.commitment_root),
        finality,
        projection,
    )
}

fn render_sccp_proof_request_summary(
    request: &iroha_sccp::SccpGroth16Bn254ProofRequestV1,
) -> String {
    format!(
        "sccp proof request: id={} backend={} {}->{} finality_height={} request_hash={} statement_hash={} binding={} configuration={} verifier_key={} semantic_profile={} finality_anchor={}",
        hex::encode(request.public_inputs.message_id),
        request.backend.backend_label(),
        request.source_network.profile_key(),
        request.target_network.profile_key(),
        request.public_inputs.finality_height,
        hex::encode(request.request_hash),
        hex::encode(request.statement_hash),
        hex::encode(request.destination_binding_hash),
        hex::encode(request.route_configuration_hash),
        hex::encode(request.verifier_key_hash),
        hex::encode(request.semantic_proof_profile_hash),
        hex::encode(request.sora_finality_anchor_hash),
    )
}

fn render_sccp_payload_projection_summary(
    projection: &iroha_sccp::SccpPayloadProjectionV1,
) -> String {
    let iroha_sccp::SccpPayloadProjectionV1::Transfer(transfer) = projection;
    format!(
        "transfer revision={} asset_id={} amount={} sender={} recipient={} route_id={}",
        transfer.route_revision,
        render_sccp_normalized_codec_value(&transfer.asset_id),
        transfer.amount,
        render_sccp_normalized_codec_value(&transfer.sender),
        render_sccp_normalized_codec_value(&transfer.recipient),
        render_sccp_normalized_codec_value(&transfer.route_id)
    )
}

fn render_sccp_normalized_codec_value(value: &iroha_sccp::SccpNormalizedCodecValueV1) -> String {
    match value {
        iroha_sccp::SccpNormalizedCodecValueV1::CanonicalText { value } => {
            format!("canonical_text:{value}")
        }
        iroha_sccp::SccpNormalizedCodecValueV1::EvmAddress20 { bytes } => {
            format!("evm_address20:0x{}", hex::encode(bytes))
        }
        iroha_sccp::SccpNormalizedCodecValueV1::TronAddress21 { bytes } => {
            format!("tron_address21:0x{}", hex::encode(bytes))
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{cell::Cell, path::PathBuf};

    use clap::Parser as _;
    use iroha_crypto::{Algorithm, KeyPair, Signature};
    use tempfile::tempdir;

    use super::*;

    #[derive(clap::Parser)]
    struct TestCli {
        #[command(subcommand)]
        command: Command,
    }

    fn detached_authority() -> (AccountId, KeyPair) {
        let key_pair = KeyPair::try_from_seed(vec![0x57; 32], Algorithm::Ed25519)
            .expect("detached CLI test key");
        (AccountId::new(key_pair.public_key().clone()), key_pair)
    }

    fn submit_response(
        submitted: bool,
        tx_hash_hex: Option<String>,
        transaction_payload_b64: Option<String>,
        signing_message_b64: Option<String>,
    ) -> SccpBridgeSubmitResponse {
        SccpBridgeSubmitResponse {
            submitted,
            payload_kind: "transfer".to_owned(),
            message_id_hex: "11".repeat(32),
            backend: "bridge/sccp/native/ethereum-beacon-v1".to_owned(),
            counterparty_domain: iroha_sccp::SCCP_DOMAIN_ETH,
            counterparty_chain: "ethereum-mainnet".to_owned(),
            route_configuration_hash_hex: "22".repeat(32),
            range_start_height: 7,
            range_end_height: 7,
            creation_time_ms: 9,
            tx_hash_hex,
            transaction_payload_b64,
            signing_message_b64,
        }
    }

    #[test]
    fn sccp_submit_cli_rejects_mixed_and_legacy_signing_flags() {
        let prepared = TestCli::try_parse_from([
            "iroha",
            "sccp",
            "submit-native-message",
            "--proof",
            "proof.norito",
        ])
        .expect("unsigned preparation must remain directly expressible");
        assert!(matches!(
            prepared.command,
            Command::Sccp(SccpCommand::SubmitNativeMessage(_))
        ));

        let direct = TestCli::try_parse_from([
            "iroha",
            "sccp",
            "submit-destination-proof",
            "--artifact",
            "proof.norito",
            "--transaction-payload-b64-file",
            "payload.b64",
            "--signature-b64-file",
            "signature.b64",
            "--creation-time-ms",
            "7",
        ])
        .expect("complete direct submission grammar");
        assert!(matches!(
            direct.command,
            Command::Sccp(SccpCommand::SubmitDestinationProof(_))
        ));
        for hostile in [
            vec!["iroha", "sccp", "submit-native-message"],
            vec!["iroha", "sccp", "submit-destination-proof"],
            vec![
                "iroha",
                "sccp",
                "submit-native-message",
                "--proof",
                "proof.norito",
                "--transaction-payload-b64-file",
                "payload.b64",
                "--creation-time-ms",
                "7",
            ],
            vec![
                "iroha",
                "sccp",
                "submit-native-message",
                "--proof",
                "proof.norito",
                "--signature-b64-file",
                "signature.b64",
                "--creation-time-ms",
                "7",
            ],
            vec![
                "iroha",
                "sccp",
                "submit-destination-proof",
                "--artifact",
                "proof.norito",
                "--manifest-hash",
                "11",
            ],
            vec![
                "iroha",
                "sccp",
                "submit-destination-proof",
                "--artifact",
                "proof.norito",
                "--transaction-payload-b64",
                "secret-value",
            ],
            vec![
                "iroha",
                "sccp",
                "submit-destination-proof",
                "--artifact",
                "proof.norito",
                "--network-id-hex",
                "11",
            ],
            vec![
                "iroha",
                "sccp",
                "submit-native-message",
                "--proof",
                "proof.norito",
                "--creation-time-ms=not-a-number",
            ],
            vec![
                "iroha",
                "sccp",
                "submit-native-message",
                "--proof",
                "proof.norito",
                "--transaction-payload-b64-file",
                "payload.b64",
                "--signature-b64-file",
                "signature.b64",
                "--creation-time-ms=-1",
            ],
        ] {
            assert!(
                TestCli::try_parse_from(hostile).is_err(),
                "accepted mixed or retired SCCP submit grammar"
            );
        }
    }

    #[test]
    fn bounded_sccp_artifact_reader_rejects_missing_empty_and_oversized_inputs() {
        let directory = tempdir().expect("temporary SCCP artifact directory");
        let path = directory.path().join("artifact.norito");

        assert!(read_bounded_binary_artifact(&path, 4, "SCCP test artifact").is_err());
        fs::write(&path, []).expect("write empty artifact");
        assert!(read_bounded_binary_artifact(&path, 4, "SCCP test artifact").is_err());
        fs::write(&path, [1_u8; 5]).expect("write oversized artifact");
        assert!(read_bounded_binary_artifact(&path, 4, "SCCP test artifact").is_err());
        fs::write(&path, [1_u8; 4]).expect("write bounded artifact");
        assert_eq!(
            read_bounded_binary_artifact(&path, 4, "SCCP test artifact")
                .expect("maximum-size artifact"),
            vec![1_u8; 4]
        );
    }

    #[test]
    fn detached_submit_material_rejects_zero_missing_and_mixed_state() {
        let (authority, _) = detached_authority();
        let none = || DetachedSubmitArgs {
            transaction_payload_b64_file: None,
            signature_b64_file: None,
            creation_time_ms: None,
        };
        assert_eq!(
            load_detached_submit_material(&none(), &authority).expect("preparation state"),
            DetachedSubmitMaterial {
                transaction_payload_b64: None,
                signature_b64: None,
                creation_time_ms: None,
            }
        );
        let mut zero = none();
        zero.creation_time_ms = Some(0);
        assert!(
            load_detached_submit_material(&zero, &authority)
                .expect_err("zero creation time")
                .to_string()
                .contains("positive")
        );
        let mut mixed = none();
        mixed.transaction_payload_b64_file = Some(PathBuf::from("payload.b64"));
        assert!(
            load_detached_submit_material(&mixed, &authority)
                .expect_err("mixed signing state")
                .to_string()
                .contains("requires both")
        );
        mixed.signature_b64_file = Some(PathBuf::from("signature.b64"));
        assert!(
            load_detached_submit_material(&mixed, &authority)
                .expect_err("missing direct creation time")
                .to_string()
                .contains("creation-time-ms")
        );
    }

    #[test]
    fn detached_submit_files_bind_signature_to_exact_payload() {
        let directory = tempdir().expect("temporary SCCP signing directory");
        let payload_path = directory.path().join("payload.b64");
        let signature_path = directory.path().join("signature.b64");
        let (authority, key_pair) = detached_authority();
        let payload = b"exact prepared transaction payload";
        let payload_b64 = base64::engine::general_purpose::STANDARD.encode(payload);
        let signing_hash = iroha_crypto::Hash::new(payload);
        let signature = Signature::try_new(key_pair.private_key(), signing_hash.as_ref())
            .expect("sign exact payload hash");
        let signature_b64 = base64::engine::general_purpose::STANDARD.encode(signature.payload());
        fs::write(&payload_path, format!("{payload_b64}\n")).expect("write payload fixture");
        fs::write(&signature_path, format!("{signature_b64}\r\n"))
            .expect("write signature fixture");

        let args = DetachedSubmitArgs {
            transaction_payload_b64_file: Some(payload_path.clone()),
            signature_b64_file: Some(signature_path.clone()),
            creation_time_ms: Some(7),
        };
        let material =
            load_detached_submit_material(&args, &authority).expect("exact detached signing files");
        assert_eq!(
            material.transaction_payload_b64.as_deref(),
            Some(payload_b64.as_str())
        );
        assert_eq!(
            material.signature_b64.as_deref(),
            Some(signature_b64.as_str())
        );

        let other_hash = iroha_crypto::Hash::new(b"different payload");
        let wrong_signature = Signature::try_new(key_pair.private_key(), other_hash.as_ref())
            .expect("sign different payload hash");
        fs::write(
            &signature_path,
            base64::engine::general_purpose::STANDARD.encode(wrong_signature.payload()),
        )
        .expect("replace signature fixture");
        let error = load_detached_submit_material(&args, &authority)
            .expect_err("payload/signature mismatch must reject");
        assert!(error.to_string().contains("does not verify"));

        fs::write(&payload_path, format!(" {payload_b64}"))
            .expect("replace payload fixture with whitespace");
        assert!(load_detached_submit_material(&args, &authority).is_err());
    }

    #[test]
    fn sccp_submit_is_attempted_once_even_for_ambiguous_errors() {
        let calls = Cell::new(0_u8);
        let error = submit_sccp_once::<()>("SCCP test submit", || {
            calls.set(calls.get() + 1);
            Err(eyre!("length mismatch from remote"))
        })
        .expect_err("ambiguous remote error must propagate");
        assert_eq!(calls.get(), 1);
        assert!(error.to_string().contains("without retrying or rebuilding"));
    }

    #[test]
    fn submit_response_renderer_exposes_exact_prepared_fields_and_rejects_mixed_state() {
        let payload = b"prepared payload";
        let payload_b64 = base64::engine::general_purpose::STANDARD.encode(payload);
        let signing_message_b64 = base64::engine::general_purpose::STANDARD
            .encode(iroha_crypto::Hash::new(payload).as_ref());
        let prepared = submit_response(
            false,
            None,
            Some(payload_b64.clone()),
            Some(signing_message_b64.clone()),
        );
        let rendered = render_sccp_submit_response(&prepared).expect("prepared response");
        assert!(rendered.contains("route_configuration_hash_hex="));
        assert!(rendered.contains("creation_time_ms=9"));
        assert!(rendered.contains(&format!("transaction_payload_b64={payload_b64}")));
        assert!(rendered.contains(&format!("signing_message_b64={signing_message_b64}")));
        assert!(!rendered.contains("manifest_hash"));

        let submitted = submit_response(true, Some("33".repeat(32)), None, None);
        let rendered = render_sccp_submit_response(&submitted).expect("submitted response");
        assert!(rendered.contains("tx_hash_hex="));
        assert!(!rendered.contains("transaction_payload_b64="));

        for invalid in [
            submit_response(true, None, None, None),
            submit_response(true, Some("33".repeat(32)), Some(payload_b64.clone()), None),
            submit_response(false, None, Some(payload_b64.clone()), None),
            submit_response(
                false,
                None,
                Some(payload_b64),
                Some(base64::engine::general_purpose::STANDARD.encode([0_u8; 32])),
            ),
        ] {
            assert!(validate_sccp_submit_response_state(&invalid).is_err());
        }

        let mut zero_creation = prepared.clone();
        zero_creation.creation_time_ms = 0;
        assert!(validate_sccp_submit_response_state(&zero_creation).is_err());
        let mut zero_route = prepared.clone();
        zero_route.route_configuration_hash_hex = "00".repeat(32);
        assert!(validate_sccp_submit_response_state(&zero_route).is_err());

        for invalid_tx_hash in ["00".repeat(32), "AA".repeat(32), "11".repeat(31)] {
            let invalid = submit_response(true, Some(invalid_tx_hash), None, None);
            assert!(validate_sccp_submit_response_state(&invalid).is_err());
        }
        let mut zero_message = prepared.clone();
        zero_message.message_id_hex = "00".repeat(32);
        assert!(validate_sccp_submit_response_state(&zero_message).is_err());
        let mut reversed_range = prepared.clone();
        reversed_range.range_start_height = 8;
        reversed_range.range_end_height = 7;
        assert!(validate_sccp_submit_response_state(&reversed_range).is_err());
        let mut retired_payload_kind = prepared;
        retired_payload_kind.payload_kind = "generic".to_owned();
        assert!(validate_sccp_submit_response_state(&retired_payload_kind).is_err());
    }

    #[test]
    fn capabilities_summary_names_only_exact_first_release_paths() {
        let summary = render_sccp_capabilities_summary(&SccpCapabilities {
            version: 1,
            registry_revision: format!("0x{}", "11".repeat(32)),
            registry_path: "/v1/sccp/registry".to_owned(),
            message_bundle_path: "/v1/sccp/proofs/message/{message_id}".to_owned(),
            proof_request_path: "/v1/sccp/proof-requests/{message_id}".to_owned(),
            recent_messages_path: "/v1/sccp/messages/recent".to_owned(),
            registry_limits: SccpRegistryLimits {
                max_governed_lanes: 16,
                max_live_governed_routes: 64,
                max_live_routes_per_lane: 8,
                max_retained_routes_per_lane: 64,
                max_retained_native_trust_anchors_per_lane: 4_096,
            },
            resource_limits: SccpResourceLimits {
                max_outbound_messages_per_block: 512,
                max_outbound_message_payload_bytes: 4_096,
                max_pending_outbound_messages: 65_536,
                max_pending_outbound_payload_bytes: 256 * 1024 * 1024,
                max_proofs_per_transaction: 1,
                max_proofs_per_block: 4,
                max_proof_bytes_per_proof: 8 * 1024 * 1024,
                max_proof_bytes_per_transaction: 8 * 1024 * 1024,
                max_proof_bytes_per_block: 32 * 1024 * 1024,
                max_native_headers_per_transaction: 1_004,
                max_native_headers_per_block: 4_016,
                max_ethereum_light_client_updates_per_transaction: 128,
                max_ethereum_light_client_updates_per_block: 512,
                max_native_header_bytes_per_transaction: 8 * 1024 * 1024,
                max_native_header_bytes_per_block: 32 * 1024 * 1024,
                max_secp256k1_recoveries_per_transaction: 1_005,
                max_secp256k1_recoveries_per_block: 4_020,
                max_bls_aggregate_checks_per_transaction: 1_004,
                max_bls_aggregate_checks_per_block: 4_016,
                max_bls_signer_contributions_per_transaction: 131_713,
                max_bls_signer_contributions_per_block: 526_852,
                max_bn254_pairing_checks_per_transaction: 1,
                max_bn254_pairing_checks_per_block: 4,
            },
            proof_submit_path: Some("/v1/bridge/proofs/submit".to_owned()),
            native_message_submit_path: Some("/v1/bridge/messages".to_owned()),
        });
        for required in ["registry=", "bundle=", "proof_request=", "proof_submit="] {
            assert!(summary.contains(required));
        }
        for retired in ["manifest", "artifact", "job", "solana", "ton-"] {
            assert!(!summary.contains(retired));
        }
    }

    #[test]
    fn recent_query_enforces_closed_first_release_bounds() {
        let query = RecentArgs {
            from: Some(1),
            after_index: Some(0),
            limit: Some(50),
        }
        .query();
        query.validate().expect("maximum valid SCCP window");
        for query in [
            RecentArgs {
                from: Some(0),
                after_index: None,
                limit: Some(1),
            }
            .query(),
            RecentArgs {
                from: Some(1),
                after_index: None,
                limit: Some(0),
            }
            .query(),
            RecentArgs {
                from: Some(1),
                after_index: None,
                limit: Some(51),
            }
            .query(),
        ] {
            assert!(query.validate().is_err());
        }
    }

    #[test]
    fn hex32_rejects_ambiguous_or_wrong_width_values() {
        for hostile in [
            "",
            "00",
            &format!(" {}", "11".repeat(32)),
            &format!("0x0X{}", "11".repeat(32)),
            &"gg".repeat(32),
        ] {
            assert!(hex32(hostile).is_err(), "accepted hostile hex: {hostile:?}");
        }
        assert_eq!(
            hex32(&format!("0X{}", "AB".repeat(32))).expect("valid hash"),
            [0xAB; 32]
        );
    }

    #[test]
    fn recent_summary_includes_both_governed_hash_roles() {
        let summary = render_sccp_recent_messages_summary(&SccpRecentMessages {
            items: vec![iroha::client::SccpRecentMessage {
                height: 9,
                commitment_index: 0,
                message_id_hex: "11".repeat(32),
                kind: "transfer".to_owned(),
                source_profile: "sora-taira".to_owned(),
                target_profile: "ethereum-sepolia".to_owned(),
                destination_binding_hash: format!("0x{}", "22".repeat(32)),
                route_configuration_hash: format!("0x{}", "33".repeat(32)),
                target_domain: 1,
                asset_id: Some("xor".to_owned()),
                route_id: Some("taira_eth_xor".to_owned()),
                recipient: None,
                amount: "5".to_owned(),
                payload_projection: iroha_sccp::SccpPayloadProjectionV1::Transfer(
                    iroha_sccp::SccpTransferProjectionV1 {
                        version: 1,
                        source_domain: iroha_sccp::SCCP_DOMAIN_SORA,
                        dest_domain: iroha_sccp::SCCP_DOMAIN_ETH,
                        nonce: 1,
                        route_revision: 1,
                        asset_home_domain: iroha_sccp::SCCP_DOMAIN_SORA,
                        asset_id: iroha_sccp::SccpNormalizedCodecValueV1::CanonicalText {
                            value: "xor".to_owned(),
                        },
                        amount: 5,
                        sender: iroha_sccp::SccpNormalizedCodecValueV1::CanonicalText {
                            value: "alice".to_owned(),
                        },
                        recipient: iroha_sccp::SccpNormalizedCodecValueV1::EvmAddress20 {
                            bytes: [0x44; 20],
                        },
                        route_id: iroha_sccp::SccpNormalizedCodecValueV1::CanonicalText {
                            value: "taira_eth_xor".to_owned(),
                        },
                    },
                ),
                links: iroha::client::SccpRecentMessageLinks {
                    bundle_path: "/v1/sccp/proofs/message/id".to_owned(),
                    proof_request_path: "/v1/sccp/proof-requests/id".to_owned(),
                },
            }],
            next: None,
        });
        assert!(summary.contains(&"22".repeat(32)));
        assert!(summary.contains(&"33".repeat(32)));
        assert!(summary.contains("proof_request="));
    }
}
