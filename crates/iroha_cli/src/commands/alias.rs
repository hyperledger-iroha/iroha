//! Alias resolution helpers.
//!
//! The underlying API surface is not yet stable; these commands focus on basic
//! input validation, forwarding requests to the alias Torii endpoints while
//! handling not-yet-implemented responses gracefully.

use crate::cli_output::print_with_optional_text;
use crate::{Run, RunContext};
use eyre::{Result, eyre};
use iroha::data_model::alias::AliasIndex;
use iroha::{client::Client, http::Response, http::StatusCode};
use std::fmt::Write as _;

#[cfg(test)]
use iroha_i18n::{Bundle, Language, Localizer};

#[derive(clap::Subcommand, Debug)]
pub enum Command {
    /// Evaluate a blinded element using the alias VOPRF service (placeholder).
    VoprfEvaluate(VoprfEvaluateArgs),
    /// Resolve an alias by its canonical name (placeholder).
    Resolve(ResolveArgs),
    /// Resolve an alias by Merkle index (placeholder).
    ResolveIndex(ResolveIndexArgs),
}

impl Run for Command {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        match self {
            Command::VoprfEvaluate(args) => args.run(context),
            Command::Resolve(args) => args.run(context),
            Command::ResolveIndex(args) => args.run(context),
        }
    }
}

#[derive(clap::Args, Debug)]
pub struct VoprfEvaluateArgs {
    /// Blinded element in hex encoding.
    #[arg(long, value_name = "HEX")]
    pub blinded_element_hex: String,
}

impl Run for VoprfEvaluateArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        alias_voprf_evaluate_with(
            context,
            &self.blinded_element_hex,
            Client::post_alias_voprf_hex,
        )
    }
}

#[derive(clap::Args, Debug)]
pub struct ResolveArgs {
    /// Alias name to resolve.
    #[arg(long)]
    pub alias: String,
    /// Print only validation result (skip future network call).
    #[arg(long, default_value_t = false)]
    pub dry_run: bool,
}

impl Run for ResolveArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        alias_resolve_with(
            context,
            &self.alias,
            self.dry_run,
            Client::post_alias_resolve,
        )
    }
}

#[derive(clap::Args, Debug)]
pub struct ResolveIndexArgs {
    /// Alias Merkle index to resolve.
    #[arg(long)]
    pub index: u64,
}

impl Run for ResolveIndexArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        alias_resolve_index_with(context, self.index, Client::post_alias_resolve_index)
    }
}

fn alias_voprf_evaluate_with<C, F>(context: &mut C, blinded_hex: &str, call: F) -> Result<()>
where
    C: RunContext,
    F: FnOnce(&Client, &str) -> Result<Response<Vec<u8>>>,
{
    hex::decode(blinded_hex.trim_start_matches("0x"))
        .map_err(|err| eyre!("invalid blinded element hex: {err}"))?;
    let client = context.client_from_config();
    let response = call(&client, blinded_hex)?;
    let status = response.status();
    let body = response.into_body();

    match status {
        StatusCode::OK => {
            let value: norito::json::Value = norito::json::from_slice(&body)?;
            let text = render_voprf_evaluate_text(&value);
            print_with_optional_text(context, Some(text), &value)
        }
        StatusCode::NOT_IMPLEMENTED => Err(eyre!(
            "{}",
            format_alias_error("alias VOPRF evaluation is not available", &body)
        )),
        status => Err(eyre!(
            "alias VOPRF evaluation failed with status {status}: {}",
            format_alias_error("server response", &body)
        )),
    }
}

fn alias_resolve_with<C, F>(context: &mut C, alias: &str, dry_run: bool, call: F) -> Result<()>
where
    C: RunContext,
    F: FnOnce(&Client, &str) -> Result<Response<Vec<u8>>>,
{
    validate_account_alias_literal(alias)?;
    if dry_run {
        let output = norito::json!({
            "alias": alias,
            "dry_run": true,
        });
        let text = "alias resolve dry-run completed".to_string();
        return print_with_optional_text(context, Some(text), &output);
    }

    let client = context.client_from_config();
    let response = call(&client, alias)?;
    let status = response.status();
    let body = response.into_body();

    match status {
        StatusCode::OK => {
            let dto: AliasResolveResponse = norito::json::from_slice(&body)?;
            let text = render_alias_resolve_text(&dto);
            print_with_optional_text(context, Some(text), &dto)
        }
        StatusCode::NOT_FOUND => Err(eyre!(
            "{}",
            format_alias_error(&format!("alias `{alias}` not found"), &body)
        )),
        StatusCode::SERVICE_UNAVAILABLE => Err(eyre!(
            "{}",
            format_alias_error("alias service is unavailable on the target node", &body)
        )),
        status => Err(eyre!(
            "alias resolve failed with status {status}: {}",
            format_alias_error("server response", &body)
        )),
    }
}

fn validate_account_alias_literal(alias: &str) -> Result<()> {
    let trimmed = alias.trim();
    if trimmed.is_empty() {
        return Err(eyre!(
            "invalid alias: account alias must use `name@domain.dataspace` or `name@dataspace` format"
        ));
    }
    if trimmed != alias {
        return Err(eyre!(
            "invalid alias: account alias must not contain leading or trailing whitespace"
        ));
    }

    let (name, right) = trimmed.split_once('@').ok_or_else(|| {
        eyre!("invalid alias: account alias must use `name@domain.dataspace` or `name@dataspace` format")
    })?;
    if right.contains('@') {
        return Err(eyre!(
            "invalid alias: account alias must contain exactly one `@` separator"
        ));
    }
    if name.is_empty() || right.is_empty() {
        return Err(eyre!(
            "invalid alias: account alias segments must not be empty"
        ));
    }

    let dot_count = right.bytes().filter(|byte| *byte == b'.').count();
    match dot_count {
        0 => Ok(()),
        1 => {
            let (domain, dataspace) = right.split_once('.').expect("counted dot");
            if domain.is_empty() || dataspace.is_empty() {
                return Err(eyre!(
                    "invalid alias: account alias domain and dataspace segments must not be empty"
                ));
            }
            Ok(())
        }
        _ => Err(eyre!(
            "invalid alias: account alias must use `name@domain.dataspace` or `name@dataspace` format"
        )),
    }
}

fn alias_resolve_index_with<C, F>(context: &mut C, index: u64, call: F) -> Result<()>
where
    C: RunContext,
    F: FnOnce(&Client, u64) -> Result<Response<Vec<u8>>>,
{
    let _ = AliasIndex(index);
    let client = context.client_from_config();
    let response = call(&client, index)?;
    let status = response.status();
    let body = response.into_body();

    match status {
        StatusCode::OK => {
            let dto: AliasResolveIndexResponse = norito::json::from_slice(&body)?;
            let text = render_alias_resolve_index_text(&dto);
            print_with_optional_text(context, Some(text), &dto)
        }
        StatusCode::NOT_IMPLEMENTED => Err(eyre!(
            "{}",
            format_alias_error("alias index resolution is not supported", &body)
        )),
        status => Err(eyre!(
            "alias resolve-index failed with status {status}: {}",
            format_alias_error("server response", &body)
        )),
    }
}

fn format_alias_error(fallback: &str, body: &[u8]) -> String {
    if let Some(message) = parse_alias_error_message(body) {
        return format!("{fallback}: {message}");
    }
    let trimmed = String::from_utf8_lossy(body).trim().to_string();
    if trimmed.is_empty() {
        fallback.to_string()
    } else {
        format!("{fallback}: {trimmed}")
    }
}

fn parse_alias_error_message(body: &[u8]) -> Option<String> {
    norito::json::from_slice::<AliasErrorResponse>(body)
        .ok()
        .map(|err| err.error)
}

fn render_voprf_evaluate_text(value: &norito::json::Value) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "alias VOPRF evaluation completed");
    if let Some(obj) = value.as_object() {
        if let Some(evaluated) = obj
            .get("evaluated_element_hex")
            .and_then(norito::json::Value::as_str)
        {
            let _ = writeln!(out, "evaluated_element_hex: {evaluated}");
        }
        if let Some(backend) = obj.get("backend").and_then(norito::json::Value::as_str) {
            let _ = writeln!(out, "backend: {backend}");
        }
    }
    out
}

fn render_alias_resolve_text(dto: &AliasResolveResponse) -> String {
    let mut out = String::new();
    let _ = writeln!(
        out,
        "alias `{}` resolved to `{}`",
        dto.alias, dto.account_id
    );
    if let Some(source) = dto.source.as_deref() {
        let _ = writeln!(out, "source: {source}");
    }
    out
}

fn render_alias_resolve_index_text(dto: &AliasResolveIndexResponse) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "alias index {} resolved to `{}`", dto.index, dto.alias);
    let _ = writeln!(out, "account_id: {}", dto.account_id);
    if let Some(source) = dto.source.as_deref() {
        let _ = writeln!(out, "source: {source}");
    }
    out
}

#[derive(norito::json::JsonSerialize, norito::json::JsonDeserialize)]
struct AliasResolveResponse {
    alias: String,
    account_id: String,
    #[norito(default)]
    source: Option<String>,
}

#[derive(norito::json::JsonSerialize, norito::json::JsonDeserialize)]
struct AliasResolveIndexResponse {
    index: u64,
    alias: String,
    account_id: String,
    #[norito(default)]
    source: Option<String>,
}

#[derive(norito::json::JsonDeserialize)]
struct AliasErrorResponse {
    error: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::CliOutputFormat;
    use clap::Parser;
    use iroha::{
        config::{self, Config},
        crypto::KeyPair,
        data_model::{
            Metadata,
            prelude::{AccountId, ChainId},
        },
    };
    use norito::json::JsonSerialize;
    use std::fmt::Display;
    use url::Url;

    #[derive(Parser, Debug)]
    struct Wrapper {
        #[command(subcommand)]
        command: Command,
    }

    const SAMPLE_ACCOUNT_ID: &str = "sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB";

    #[test]
    fn parse_voprf_args() {
        let wrapper =
            Wrapper::parse_from(["iroha", "voprf-evaluate", "--blinded-element-hex", "00"]);
        match wrapper.command {
            Command::VoprfEvaluate(args) => {
                assert_eq!(args.blinded_element_hex, "00");
            }
            _ => panic!("unexpected command"),
        }
    }

    struct TestContext {
        cfg: Config,
        printed: Vec<String>,
        i18n: Localizer,
        output_format: CliOutputFormat,
    }

    impl TestContext {
        fn new(output_format: CliOutputFormat) -> Self {
            let kp = KeyPair::random();
            let account = AccountId::new(kp.public_key().clone());
            let cfg = Config {
                chain: ChainId::from("test-chain"),
                account,
                key_pair: kp,
                basic_auth: None,
                torii_api_url: Url::parse("http://localhost/").unwrap(),
                torii_api_version: config::default_torii_api_version(),
                torii_api_min_proof_version: config::DEFAULT_TORII_API_MIN_PROOF_VERSION
                    .to_string(),
                torii_request_timeout: config::DEFAULT_TORII_REQUEST_TIMEOUT,
                transaction_ttl: config::DEFAULT_TRANSACTION_TIME_TO_LIVE,
                transaction_status_timeout: config::DEFAULT_TRANSACTION_STATUS_TIMEOUT,
                transaction_add_nonce: config::DEFAULT_TRANSACTION_NONCE,
                connect_queue_root: config::default_connect_queue_root(),
                soracloud_http_witness_file: None,
                sorafs_alias_cache: crate::config_utils::default_alias_cache_policy(),
                sorafs_anonymity_policy: crate::config_utils::default_anonymity_policy(),
                sorafs_rollout_phase: crate::config_utils::default_rollout_phase(),
            };
            Self {
                cfg,
                printed: Vec::new(),
                i18n: Localizer::new(Bundle::Cli, Language::English),
                output_format,
            }
        }
    }

    impl RunContext for TestContext {
        fn config(&self) -> &Config {
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

        fn output_format(&self) -> crate::CliOutputFormat {
            self.output_format
        }

        fn print_data<T>(&mut self, data: &T) -> Result<()>
        where
            T: JsonSerialize + ?Sized,
        {
            let bytes = norito::json::to_vec(data)?;
            let out = String::from_utf8(bytes).map_err(|err| eyre!(err.to_string()))?;
            self.printed.push(out);
            Ok(())
        }

        fn println(&mut self, data: impl Display) -> Result<()> {
            self.printed.push(data.to_string());
            Ok(())
        }
    }

    fn not_implemented_response() -> Result<Response<Vec<u8>>> {
        let body = norito::json::to_vec(&norito::json!({ "error": "not ready" }))
            .map_err(|err| eyre!(err.to_string()))?;
        Ok(Response::builder()
            .status(StatusCode::NOT_IMPLEMENTED)
            .header("Content-Type", "application/json")
            .body(body)
            .unwrap())
    }

    #[test]
    fn voprf_helper_prints_ok_payload() {
        let mut ctx = TestContext::new(CliOutputFormat::Json);
        alias_voprf_evaluate_with(&mut ctx, "deadbeef", |_, _| {
            Ok(Response::builder()
                .status(StatusCode::OK)
                .header("Content-Type", "application/json")
                .body(norito::json::to_vec(&norito::json!({
                    "evaluated_element_hex": "aa",
                    "backend": "mock"
                }))?)
                .unwrap())
        })
        .expect("helper should succeed");
        assert_eq!(ctx.printed.len(), 1);
        assert!(ctx.printed[0].contains("\"backend\":\"mock\""));
    }

    #[test]
    fn voprf_text_includes_backend() {
        let payload = norito::json!({
            "evaluated_element_hex": "aa",
            "backend": "mock"
        });
        let text = render_voprf_evaluate_text(&payload);
        assert!(text.contains("backend: mock"));
    }

    #[test]
    fn voprf_helper_handles_not_implemented() {
        let mut ctx = TestContext::new(CliOutputFormat::Json);
        let err =
            alias_voprf_evaluate_with(&mut ctx, "deadbeef", |_, _| not_implemented_response())
                .expect_err("expected error");
        assert!(err.to_string().contains("not ready"));
    }

    #[test]
    fn voprf_helper_validates_hex() {
        let mut ctx = TestContext::new(CliOutputFormat::Json);
        let err = alias_voprf_evaluate_with(&mut ctx, "zz", |_, _| unreachable!());
        assert!(err.is_err());
    }

    #[test]
    fn resolve_helper_prints_result() {
        let mut ctx = TestContext::new(CliOutputFormat::Json);
        alias_resolve_with(&mut ctx, "alice", false, |_, _| {
            Ok(Response::builder()
                .status(StatusCode::OK)
                .header("Content-Type", "application/json")
                .body(norito::json::to_vec(&norito::json!({
                    "alias": "alice",
                    "account_id": SAMPLE_ACCOUNT_ID,
                    "source": "iso_bridge"
                }))?)
                .unwrap())
        })
        .expect("helper should succeed");
        assert_eq!(ctx.printed.len(), 1);
        assert!(ctx.printed[0].contains(SAMPLE_ACCOUNT_ID));
    }

    #[test]
    fn resolve_text_includes_source() {
        let dto = AliasResolveResponse {
            alias: "alice".to_string(),
            account_id: SAMPLE_ACCOUNT_ID.to_string(),
            source: Some("iso_bridge".to_string()),
        };
        let text = render_alias_resolve_text(&dto);
        assert!(text.contains("source: iso_bridge"));
    }

    #[test]
    fn resolve_helper_handles_not_found() {
        let mut ctx = TestContext::new(CliOutputFormat::Json);
        let err = alias_resolve_with(&mut ctx, "alice", false, |_, _| {
            Ok(Response::builder()
                .status(StatusCode::NOT_FOUND)
                .body(Vec::new())
                .unwrap())
        })
        .expect_err("expected error");
        assert!(err.to_string().contains("alias `alice` not found"));
    }

    #[test]
    fn resolve_index_helper_handles_not_implemented() {
        let mut ctx = TestContext::new(CliOutputFormat::Json);
        let err = alias_resolve_index_with(&mut ctx, 0, |_, _| not_implemented_response())
            .expect_err("expected error");
        assert!(err.to_string().contains("not ready"));
    }

    #[test]
    fn resolve_index_helper_prints_result() {
        let mut ctx = TestContext::new(CliOutputFormat::Json);
        alias_resolve_index_with(&mut ctx, 0, |_, _| {
            Ok(Response::builder()
                .status(StatusCode::OK)
                .header("Content-Type", "application/json")
                .body(norito::json::to_vec(&norito::json!({
                    "index": 0,
                    "alias": "GB82WEST12345698765432",
                    "account_id": SAMPLE_ACCOUNT_ID,
                    "source": "iso_bridge"
                }))?)
                .unwrap())
        })
        .expect("helper should succeed");
        assert_eq!(ctx.printed.len(), 1);
        assert!(ctx.printed[0].contains("GB82WEST12345698765432"));
    }

    #[test]
    fn resolve_index_text_mentions_account() {
        let dto = AliasResolveIndexResponse {
            index: 0,
            alias: "GB82WEST12345698765432".to_string(),
            account_id: SAMPLE_ACCOUNT_ID.to_string(),
            source: None,
        };
        let text = render_alias_resolve_index_text(&dto);
        assert!(text.contains(&format!("account_id: {SAMPLE_ACCOUNT_ID}")));
    }

    #[test]
    fn alias_error_prefers_json_message() {
        let body =
            norito::json::to_vec(&norito::json!({ "error": "not ready" })).expect("encode error");
        let rendered = format_alias_error("fallback", &body);
        assert_eq!(rendered, "fallback: not ready");
    }
}
