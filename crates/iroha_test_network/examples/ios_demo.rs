use std::{
    collections::HashSet,
    env,
    fs::{self, File},
    io::{self, Write},
    path::{Path, PathBuf},
    str::FromStr,
    thread,
    time::Duration,
};

use color_eyre::{
    Result,
    eyre::{Context, eyre},
};
use iroha::{client::Client, data_model::prelude::*};
use iroha_primitives::{json::Json, numeric::Numeric};
use iroha_test_network::{NetworkBuilder, NetworkPeer, init_instruction_registry};
use norito::json::{JsonDeserialize, JsonSerialize};
use url::Url;

const DEFAULT_CONFIG_RELATIVE: &str = "examples/ios/NoritoDemoXcode/Configs/SampleAccounts.json";
const DEFAULT_TELEMETRY_PROFILE: &str = "full";

#[derive(Debug, JsonDeserialize)]
struct AccountConfig {
    name: Option<String>,
    public_key: String,
    #[norito(default)]
    private_key: Option<String>,
    #[norito(default)]
    domain: Option<String>,
    asset_id: String,
    initial_balance: String,
}

#[derive(Debug, JsonSerialize)]
struct AccountState {
    account_id: String,
    public_key: String,
    #[norito(skip_serializing_if = "Option::is_none")]
    private_key: Option<String>,
    asset_id: String,
    initial_balance: String,
}

#[derive(Debug, JsonSerialize)]
struct StateOutput {
    torii_url: String,
    metrics_url: String,
    #[norito(skip_serializing_if = "Option::is_none")]
    stdout_log: Option<String>,
    accounts: Vec<AccountState>,
}

#[derive(Debug)]
struct Args {
    config: PathBuf,
    state_path: Option<PathBuf>,
    telemetry_profile: String,
    exit_after_ready: bool,
}

impl Args {
    fn parse() -> Result<Self> {
        let mut config: Option<PathBuf> = None;
        let mut state_path: Option<PathBuf> = None;
        let mut telemetry_profile: Option<String> = None;
        let mut exit_after_ready = false;

        let mut args = env::args().skip(1);
        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--config" => {
                    let value = args
                        .next()
                        .ok_or_else(|| eyre!("`--config` expects a value"))?;
                    config = Some(PathBuf::from(value));
                }
                "--state" | "--state-file" => {
                    let value = args
                        .next()
                        .ok_or_else(|| eyre!("`--state-file` expects a value"))?;
                    state_path = Some(PathBuf::from(value));
                }
                "--telemetry-profile" => {
                    let value = args
                        .next()
                        .ok_or_else(|| eyre!("`--telemetry-profile` expects a value"))?;
                    telemetry_profile = Some(value);
                }
                "--exit-after-ready" => {
                    exit_after_ready = true;
                }
                "--help" | "-h" => {
                    print_usage();
                    std::process::exit(0);
                }
                other => {
                    return Err(eyre!("Unknown argument `{other}`"));
                }
            }
        }

        let config = config.unwrap_or_else(|| PathBuf::from(DEFAULT_CONFIG_RELATIVE));
        let telemetry_profile =
            telemetry_profile.unwrap_or_else(|| DEFAULT_TELEMETRY_PROFILE.to_string());

        Ok(Self {
            config,
            state_path,
            telemetry_profile,
            exit_after_ready,
        })
    }
}

fn print_usage() {
    eprintln!(
        "Usage: cargo run -p iroha_test_network --example ios_demo -- [options]\n\
Options:\n  --config PATH             Path to SampleAccounts.json (default: {DEFAULT_CONFIG_RELATIVE})\n  --state PATH              Write readiness JSON to this file\n  --telemetry-profile NAME  Override telemetry profile (default: {DEFAULT_TELEMETRY_PROFILE})\n  --exit-after-ready        Exit after provisioning instead of waiting for Ctrl+C\n  --help                    Show this message"
    );
}

fn ensure_absolute(path: &Path) -> Result<PathBuf> {
    if path.is_absolute() {
        Ok(path.to_path_buf())
    } else {
        let cwd = env::current_dir().wrap_err("Failed to resolve current directory")?;
        Ok(cwd.join(path))
    }
}

fn load_config(path: &Path) -> Result<Vec<AccountConfig>> {
    let data = fs::read_to_string(path)
        .wrap_err_with(|| eyre!("Failed to read accounts config from {}", path.display()))?;
    let accounts: Vec<AccountConfig> = norito::json::from_json(&data).wrap_err_with(|| {
        eyre!(
            "Failed to parse accounts config JSON from {}",
            path.display()
        )
    })?;
    if accounts.is_empty() {
        return Err(eyre!("Accounts config `{}` is empty", path.display()));
    }
    Ok(accounts)
}

fn parse_numeric(value: &str, context: &str) -> Result<Numeric> {
    Numeric::from_str(value)
        .wrap_err_with(|| eyre!("Failed to parse numeric amount `{value}` for {context}"))
}

fn account_id_from_parts(public_key: &str, domain: &DomainId) -> Result<AccountId> {
    let parsed_key = PublicKey::from_str(public_key)
        .wrap_err_with(|| eyre!("Failed to parse public key `{public_key}`"))?;
    Ok(AccountId::new(domain.clone(), parsed_key))
}

fn ensure_parent_dir(path: &Path) -> Result<()> {
    if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
        fs::create_dir_all(parent)
            .wrap_err_with(|| eyre!("Failed to create directory {}", parent.display()))?;
    }
    Ok(())
}

fn find_existing_domains(client: &Client) -> Result<HashSet<DomainId>> {
    let domains = client
        .query(FindDomains::new())
        .execute_all()
        .wrap_err("Failed to query existing domains")?;
    Ok(domains
        .into_iter()
        .map(|domain| domain.id().clone())
        .collect())
}

fn find_existing_asset_defs(client: &Client) -> Result<HashSet<AssetDefinitionId>> {
    let definitions = client
        .query(FindAssetsDefinitions::new())
        .execute_all()
        .wrap_err("Failed to query existing asset definitions")?;
    Ok(definitions
        .into_iter()
        .map(|definition| definition.id().clone())
        .collect())
}

fn find_existing_accounts(client: &Client) -> Result<HashSet<AccountId>> {
    let accounts = client
        .query(FindAccounts::new())
        .execute_all()
        .wrap_err("Failed to query existing accounts")?;
    Ok(accounts
        .into_iter()
        .map(|account| account.id().clone())
        .collect())
}

fn derive_metrics_url(torii_url: &str) -> Result<String> {
    let mut url =
        Url::parse(torii_url).wrap_err_with(|| eyre!("Invalid Torii URL `{torii_url}`"))?;
    let mut path = url.path().trim_end_matches('/').to_string();
    if path.ends_with("/api") {
        path.push_str("/metrics");
    } else if !path.ends_with("/metrics") {
        if !path.is_empty() {
            path.push('/');
        }
        path.push_str("metrics");
    }
    if path.is_empty() {
        path = "/metrics".into();
    }
    url.set_path(&path);
    Ok(url.to_string())
}

fn wait_for_peer_log(peer: &NetworkPeer) -> Option<String> {
    for _ in 0..40 {
        if let Some(path) = peer.latest_stdout_log_path() {
            return Some(path.to_string_lossy().into_owned());
        }
        thread::sleep(Duration::from_millis(100));
    }
    None
}

fn main() -> Result<()> {
    color_eyre::install()?;

    let args = Args::parse()?;
    let config_path = ensure_absolute(&args.config)?;
    let state_path = match &args.state_path {
        Some(path) => Some(ensure_absolute(path)?),
        None => None,
    };

    let accounts_cfg = load_config(&config_path)?;

    init_instruction_registry();

    let telemetry_profile = args.telemetry_profile.clone();

    let (network, rt) = NetworkBuilder::new()
        .with_peers(1)
        .with_auto_populated_trusted_peers()
        .with_config_layer(move |layer| {
            layer
                .write("telemetry_enabled", true)
                .write("telemetry_profile", telemetry_profile.as_str())
                .write(["logger", "level"], "INFO");
        })
        .start_blocking()
        .wrap_err("Failed to start test network")?;

    let peer = network
        .peers()
        .first()
        .ok_or_else(|| eyre!("Expected at least one peer"))?;
    let torii_url = peer.torii_url();
    let metrics_url = derive_metrics_url(&torii_url)?;
    let stdout_log = wait_for_peer_log(peer);

    let client = network.client();
    let mut known_domains = find_existing_domains(&client)?;
    let mut known_asset_defs = find_existing_asset_defs(&client)?;
    let mut known_accounts = find_existing_accounts(&client)?;

    let mut state_accounts = Vec::new();

    for entry in accounts_cfg {
        let AccountConfig {
            name,
            public_key,
            private_key,
            domain,
            asset_id,
            initial_balance,
        } = entry;

        let asset_def_id: AssetDefinitionId = asset_id
            .parse()
            .wrap_err_with(|| eyre!("Failed to parse asset id `{asset_id}`"))?;
        let domain_id: DomainId = if let Some(domain_str) = domain {
            domain_str
                .parse()
                .wrap_err_with(|| eyre!("Failed to parse domain `{domain_str}`"))?
        } else {
            asset_def_id.domain.clone()
        };

        if known_domains.insert(domain_id.clone()) {
            let new_domain = Domain::new(domain_id.clone());
            client
                .submit_blocking(Register::domain(new_domain))
                .wrap_err_with(|| eyre!("Failed to register domain `{domain_id}`"))?;
        }

        if known_asset_defs.insert(asset_def_id.clone()) {
            let definition = AssetDefinition::numeric(asset_def_id.clone());
            client
                .submit_blocking(Register::asset_definition(definition))
                .wrap_err_with(|| eyre!("Failed to register asset definition `{asset_def_id}`"))?;
        }

        let account_id = account_id_from_parts(&public_key, &domain_id)?;

        if known_accounts.insert(account_id.clone()) {
            let mut account_builder = Account::new(account_id.clone());
            if let Some(alias) = &name {
                let mut metadata = Metadata::default();
                let alias_key = Name::from_str("alias").expect("static alias key");
                metadata.insert(alias_key, Json::from(alias.as_str()));
                account_builder = account_builder.with_metadata(metadata);
            }
            client
                .submit_blocking(Register::account(account_builder))
                .wrap_err_with(|| eyre!("Failed to register account `{account_id}`"))?;
        }

        let amount = parse_numeric(&initial_balance, &account_id.to_string())?;
        let asset_instance = AssetId::new(asset_def_id.clone(), account_id.clone());
        client
            .submit_blocking(Mint::asset_numeric(amount, asset_instance.clone()))
            .wrap_err_with(|| {
                eyre!(
                    "Failed to mint {} into asset `{}`",
                    initial_balance,
                    asset_instance
                )
            })?;

        state_accounts.push(AccountState {
            account_id: account_id.to_string(),
            public_key: public_key.clone(),
            private_key,
            asset_id: asset_def_id.to_string(),
            initial_balance: initial_balance.clone(),
        });
    }

    let state = StateOutput {
        torii_url: torii_url.clone(),
        metrics_url: metrics_url.clone(),
        stdout_log,
        accounts: state_accounts,
    };

    if let Some(path) = state_path.as_ref() {
        ensure_parent_dir(path)?;
        let mut file = File::create(path)
            .wrap_err_with(|| eyre!("Failed to create state file {}", path.display()))?;
        norito::json::to_writer_pretty(&mut file, &state)?;
        file.write_all(b"\n")?;
    }

    {
        let stdout = io::stdout();
        let mut handle = stdout.lock();
        norito::json::to_writer_pretty(&mut handle, &state)?;
        handle.write_all(b"\n")?;
        handle.flush()?;
    }

    if args.exit_after_ready {
        rt.block_on(network.shutdown());
        return Ok(());
    }

    println!("[ios-demo] Torii ready at {torii_url}. Press Ctrl+C to stop the demo network.");
    rt.block_on(async {
        tokio::signal::ctrl_c()
            .await
            .expect("Failed to install Ctrl+C handler");
    });
    println!("[ios-demo] Shutting down...");
    rt.block_on(network.shutdown());
    println!("[ios-demo] Done.");

    Ok(())
}
