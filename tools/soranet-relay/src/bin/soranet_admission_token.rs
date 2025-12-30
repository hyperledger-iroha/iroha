use std::{
    fs,
    path::{Path, PathBuf},
    process,
    time::{Duration, SystemTime},
};

use clap::{ArgGroup, Args, Parser, Subcommand, ValueEnum};
use norito::json::{self, Value};
use rand::{RngCore, SeedableRng, rng, rngs::StdRng};
use soranet_pq::MlDsaSuite;
use soranet_relay::token_tool::{
    MintRequest, RevocationList, TokenBundle, decode_token_string, encode_token_base64,
    encode_token_hex, inspect_token, mint_token, parse_hex_array, parse_hex_bytes, parse_rfc3339,
    read_revocation_file,
};

const DEFAULT_TTL_SECS: u64 = 900;

#[derive(Parser, Debug)]
#[command(
    name = "soranet-admission-token",
    version,
    about = "Mint, inspect, and manage SoraNet admission tokens"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Mint a new admission token signed with an ML-DSA issuer key.
    Mint(MintArgs),
    /// Decode an admission token and print its metadata.
    Inspect(InspectArgs),
    /// Append a token identifier to a Norito JSON revocation list.
    Revoke(RevokeArgs),
    /// Print all token identifiers present in a revocation list.
    ListRevocations(ListArgs),
}

#[derive(Args, Debug)]
struct MintArgs {
    #[arg(long, conflicts_with = "issuer_public_file")]
    issuer_public_hex: Option<String>,
    #[arg(long)]
    issuer_public_file: Option<PathBuf>,
    #[arg(long, conflicts_with = "issuer_secret_file")]
    issuer_secret_hex: Option<String>,
    #[arg(long)]
    issuer_secret_file: Option<PathBuf>,
    #[arg(long)]
    relay_id_hex: String,
    #[arg(long)]
    transcript_hash_hex: String,
    #[arg(long)]
    issued_at: Option<String>,
    #[arg(long, conflicts_with = "ttl_secs")]
    expires_at: Option<String>,
    #[arg(long, conflicts_with = "expires_at")]
    ttl_secs: Option<u64>,
    #[arg(long, default_value_t = 0)]
    flags: u8,
    #[arg(long, value_enum, default_value_t = OutputFormat::Json)]
    format: OutputFormat,
    #[arg(long)]
    output: Option<PathBuf>,
    #[arg(long, value_enum, default_value_t = SuiteArg::MlDsa44)]
    suite: SuiteArg,
}

#[derive(Args, Debug)]
#[command(group(
    ArgGroup::new("inspect_source")
        .required(true)
        .args(&["token", "input"])
))]
struct InspectArgs {
    #[arg(long)]
    token: Option<String>,
    #[arg(long)]
    input: Option<PathBuf>,
    #[arg(long, value_enum, default_value_t = OutputFormat::Json)]
    format: OutputFormat,
}

#[derive(Args, Debug)]
#[command(group(
    ArgGroup::new("revoke_source")
        .required(true)
        .args(&["token", "token_file", "token_id_hex"])
))]
struct RevokeArgs {
    #[arg(long)]
    list: PathBuf,
    #[arg(long)]
    token: Option<String>,
    #[arg(long)]
    token_file: Option<PathBuf>,
    #[arg(long)]
    token_id_hex: Option<String>,
    #[arg(long)]
    dry_run: bool,
}

#[derive(Args, Debug)]
struct ListArgs {
    #[arg(long)]
    list: PathBuf,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum OutputFormat {
    Json,
    Base64,
    Hex,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum SuiteArg {
    MlDsa44,
    MlDsa65,
    MlDsa87,
}

impl From<SuiteArg> for MlDsaSuite {
    fn from(value: SuiteArg) -> Self {
        match value {
            SuiteArg::MlDsa44 => MlDsaSuite::MlDsa44,
            SuiteArg::MlDsa65 => MlDsaSuite::MlDsa65,
            SuiteArg::MlDsa87 => MlDsaSuite::MlDsa87,
        }
    }
}

fn main() {
    if let Err(err) = run() {
        eprintln!("soranet-admission-token: {err}");
        process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let cli = Cli::parse();
    match cli.command {
        Command::Mint(args) => command_mint(args),
        Command::Inspect(args) => command_inspect(args),
        Command::Revoke(args) => command_revoke(args),
        Command::ListRevocations(args) => command_list(args),
    }
}

fn command_mint(args: MintArgs) -> Result<(), String> {
    let issuer_public = load_hex_source(
        args.issuer_public_hex,
        args.issuer_public_file.as_deref(),
        "issuer_public_key",
    )?;
    let issuer_secret = load_hex_source(
        args.issuer_secret_hex,
        args.issuer_secret_file.as_deref(),
        "issuer_secret_key",
    )?;
    let relay_id =
        parse_hex_array::<32>(&args.relay_id_hex, "relay_id_hex").map_err(|err| err.to_string())?;
    let transcript_hash = parse_hex_array::<32>(&args.transcript_hash_hex, "transcript_hash_hex")
        .map_err(|err| err.to_string())?;

    let issued_at = match args.issued_at {
        Some(ref value) => parse_rfc3339(value, "issued_at").map_err(|err| err.to_string())?,
        None => SystemTime::now(),
    };

    let expires_at = if let Some(ref value) = args.expires_at {
        parse_rfc3339(value, "expires_at").map_err(|err| err.to_string())?
    } else {
        let ttl = args.ttl_secs.unwrap_or(DEFAULT_TTL_SECS);
        issued_at
            .checked_add(Duration::from_secs(ttl))
            .ok_or_else(|| "expiry timestamp overflowed SystemTime".to_owned())?
    };

    let suite: MlDsaSuite = args.suite.into();
    let request = MintRequest {
        suite,
        issuer_public_key: &issuer_public,
        issuer_secret_key: &issuer_secret,
        relay_id,
        transcript_hash,
        issued_at,
        expires_at,
        flags: args.flags,
    };

    let mut seed = [0u8; 32];
    rng().fill_bytes(&mut seed);
    let mut rng = StdRng::from_seed(seed);
    let bundle = mint_token(&request, &mut rng).map_err(|err| err.to_string())?;
    let output = render_bundle(&bundle, args.format)?;
    write_output(args.output.as_deref(), &output)
}

fn command_inspect(args: InspectArgs) -> Result<(), String> {
    let bytes = if let Some(token_str) = args.token {
        decode_token_string(&token_str).map_err(|err| err.to_string())?
    } else if let Some(path) = args.input {
        fs::read(path).map_err(|err| err.to_string())?
    } else {
        unreachable!("clap enforces inspect source group");
    };
    let bundle = inspect_token(&bytes).map_err(|err| err.to_string())?;
    let output = render_bundle(&bundle, args.format)?;
    write_output(None, &output)
}

fn command_revoke(args: RevokeArgs) -> Result<(), String> {
    let token_id = if let Some(hex) = args.token_id_hex {
        parse_hex_array::<32>(&hex, "token_id_hex").map_err(|err| err.to_string())?
    } else {
        let bytes = if let Some(token_str) = args.token {
            decode_token_string(&token_str).map_err(|err| err.to_string())?
        } else if let Some(path) = args.token_file {
            fs::read(path).map_err(|err| err.to_string())?
        } else {
            unreachable!("clap enforces revoke source group");
        };
        let bundle = inspect_token(&bytes).map_err(|err| err.to_string())?;
        bundle.metadata.token_id
    };

    let mut list = RevocationList::load_or_default(&args.list)
        .map_err(|err| format!("failed to load {}: {err}", args.list.display()))?;
    let inserted = list.insert(token_id);

    if !args.dry_run && inserted {
        list.write(&args.list)
            .map_err(|err| format!("failed to write {}: {err}", args.list.display()))?;
    }

    let mut payload = json::Map::new();
    payload.insert("token_id_hex".into(), Value::from(hex::encode(token_id)));
    payload.insert("inserted".into(), Value::from(inserted));
    payload.insert("dry_run".into(), Value::from(args.dry_run));
    payload.insert(
        "revocation_list_path".into(),
        Value::from(args.list.display().to_string()),
    );
    let mut text = json::to_string_pretty(&Value::Object(payload))
        .map_err(|err| format!("failed to serialise revoke output: {err}"))?;
    text.push('\n');
    write_output(None, &text)
}

fn command_list(args: ListArgs) -> Result<(), String> {
    let ids = read_revocation_file(&args.list)
        .map_err(|err| format!("failed to load {}: {err}", args.list.display()))?;
    let values: Vec<Value> = ids
        .into_iter()
        .map(|id| Value::from(hex::encode(id)))
        .collect();
    let mut text = json::to_string_pretty(&Value::Array(values))
        .map_err(|err| format!("failed to serialise revocation list: {err}"))?;
    text.push('\n');
    write_output(None, &text)
}

fn render_bundle(bundle: &TokenBundle, format: OutputFormat) -> Result<String, String> {
    match format {
        OutputFormat::Json => {
            let mut text = json::to_string_pretty(&bundle.to_json())
                .map_err(|err| format!("failed to serialise JSON: {err}"))?;
            text.push('\n');
            Ok(text)
        }
        OutputFormat::Base64 => {
            let mut text = encode_token_base64(&bundle.token);
            text.push('\n');
            Ok(text)
        }
        OutputFormat::Hex => {
            let mut text = encode_token_hex(&bundle.token);
            text.push('\n');
            Ok(text)
        }
    }
}

fn write_output(path: Option<&Path>, data: &str) -> Result<(), String> {
    if let Some(path) = path {
        if let Some(dir) = path.parent()
            && !dir.as_os_str().is_empty()
        {
            fs::create_dir_all(dir)
                .map_err(|err| format!("failed to create {}: {err}", dir.display()))?;
        }
        fs::write(path, data).map_err(|err| format!("failed to write {}: {err}", path.display()))
    } else {
        print!("{data}");
        Ok(())
    }
}

fn load_hex_source(
    inline: Option<String>,
    path: Option<&Path>,
    field: &'static str,
) -> Result<Vec<u8>, String> {
    match (inline, path) {
        (Some(value), None) => parse_hex_bytes(value.trim(), field).map_err(|err| err.to_string()),
        (None, Some(path)) => {
            let text = fs::read_to_string(path)
                .map_err(|err| format!("failed to read {}: {err}", path.display()))?;
            parse_hex_bytes(text.trim(), field).map_err(|err| err.to_string())
        }
        (Some(_), Some(_)) => Err(format!(
            "--{field}-hex and --{field}-file are mutually exclusive"
        )),
        (None, None) => Err(format!("--{field}-hex or --{field}-file must be provided")),
    }
}
