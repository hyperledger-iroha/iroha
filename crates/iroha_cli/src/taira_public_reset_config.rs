//! Native custody for validator configuration and dedicated operator signing keys.

use super::*;
use iroha::data_model::NetworkId;
use zeroize::Zeroizing;

const MAX_CONFIG_BYTES: u64 = 1024 * 1024;
// Checked NetworkId display (74 ASCII bytes) plus exactly one LF.
const MAX_CLIENT_NETWORK_ID_BYTES: u64 = 75;

/// Request budgets that can be amended independently without changing authentication policy.
#[derive(clap::ValueEnum, Clone, Copy, Debug, PartialEq, Eq)]
enum ToriiRateSection {
    Torii,
    Mcp,
    Push,
    Content,
    Connect,
    Gateway,
    OperatorAuth,
    PrivacyIngest,
    RecipientLookup,
}

/// Amend only named HTTP request budgets through native private-file custody.
#[derive(clap::Args, Debug)]
pub(super) struct ToriiRateConfigAmend {
    /// Inherited read-only owner-private regular configuration descriptor.
    #[arg(long, value_name = "FD", value_parser = clap::value_parser!(u32).range(3..=65535))]
    config_fd: u32,
    /// Absolute original path; checked against the held descriptor without reopening its body.
    #[arg(long, value_name = "PATH")]
    config_source_path: PathBuf,
    /// Positive steady request budget; minute budgets are derived with checked arithmetic.
    #[arg(long, value_name = "REQUESTS", value_parser = clap::value_parser!(u32).range(1..))]
    rate_per_second: u32,
    /// Positive request-token burst capacity.
    #[arg(long, value_name = "REQUESTS", value_parser = clap::value_parser!(u32).range(1..))]
    burst: u32,
    /// Repeat to select budgets; omission selects all. Torii includes query/tx/deploy/preauth/proof/Soracloud.
    #[arg(long, value_enum)]
    section: Vec<ToriiRateSection>,
    /// Fresh 0600 runtime file beside the source, outside repositories; never overwritten.
    #[arg(long, value_name = "PATH")]
    output: PathBuf,
}

/// Materialize and report an offline amendment. Activation still requires daemon check-config
/// and a supervised restart: existing Torii token buckets do not reload from disk.
pub(super) fn torii_rate_config_amend(
    args: &ToriiRateConfigAmend,
    writer: &mut impl Write,
) -> Result<()> {
    #[cfg(unix)]
    {
        torii_rate_config_amend_unix(args, writer)
    }
    #[cfg(not(unix))]
    {
        let _ = (args, writer);
        Err(eyre!(
            "Torii rate amendment requires Unix private-file custody"
        ))
    }
}

#[cfg(unix)]
fn torii_rate_config_amend_unix(
    args: &ToriiRateConfigAmend,
    writer: &mut impl Write,
) -> Result<()> {
    use std::os::fd::AsRawFd as _;
    validate_absolute_normal_path(&args.config_source_path, "validator source path")?;
    validate_absolute_normal_path(&args.output, "amended validator output")?;
    let source_path = args
        .config_source_path
        .to_str()
        .ok_or_else(|| eyre!("validator source path must be UTF-8"))?;
    let output_path = args
        .output
        .to_str()
        .ok_or_else(|| eyre!("amended validator output must be UTF-8"))?;
    if args.output == args.config_source_path
        || args.output.parent() != args.config_source_path.parent()
    {
        return Err(eyre!(
            "rate amendment requires a fresh output beside its source"
        ));
    }
    let mut directories = retain_client_input_directories(&args.config_source_path, true)?;
    // Publication changes the shared private parent timestamps, but not its core identity.
    for directory in &mut directories {
        if Some(directory.path.as_path()) == args.output.parent() {
            directory.private = false;
        }
    }
    let retained = crate::client_config::duplicate_inherited_descriptor(args.config_fd)?;
    let before = retained.metadata()?;
    let check = || -> Result<()> {
        check_client_source_directories(&directories)?;
        let opened = retained.metadata()?;
        let named = fs::symlink_metadata(&args.config_source_path)?;
        if private_client_snapshot(&opened) != private_client_snapshot(&before)
            || private_client_snapshot(&named) != private_client_snapshot(&before)
            || !named.is_file()
            || named.file_type().is_symlink()
        {
            return Err(eyre!(
                "validator source custody changed during rate amendment"
            ));
        }
        Ok(())
    };
    check()?;
    let source = inherited_config(retained.as_raw_fd() as u32)?;
    check()?;
    let text = std::str::from_utf8(&source).map_err(|_| eyre!("validator config is not UTF-8"))?;
    let mut table: toml::Table =
        toml::from_str(text).map_err(|_| eyre!("validator config is not valid TOML"))?;
    let result = (|| -> Result<()> {
        if table.contains_key("extends") || table.contains_key("profile") {
            return Err(eyre!(
                "rate amendment requires a self-contained flat validator config"
            ));
        }
        let original = validate_rate_node_config(&table, &args.config_source_path)?;
        let gateway_window = original.torii.sorafs_gateway.rate_limit.window;
        drop(original);
        let rates = amend_request_rates(
            &mut table,
            &args.section,
            args.rate_per_second,
            args.burst,
            gateway_window,
        )?;
        let recipient_unchanged = (args.section.is_empty()
            || args.section.contains(&ToriiRateSection::RecipientLookup))
            && !rates.contains_key("torii.recipient_lookup.requests_per_minute");
        drop(validate_rate_node_config(&table, &args.output)?);
        let rendered = Zeroizing::new(
            toml::to_string_pretty(&table)
                .map_err(|_| eyre!("cannot materialize amended validator config"))?,
        );
        if rendered.len() as u64 > MAX_CONFIG_BYTES {
            return Err(eyre!("amended validator config exceeds its bound"));
        }
        check()?;
        write_projected_client_private(&args.output, rendered.as_bytes(), check)?;
        let metadata = fs::symlink_metadata(&args.output)?;
        let mut report = Map::new();
        report.insert(
            "schema".into(),
            Value::String("iroha.taira.torii-rate-config-amend.v1".into()),
        );
        report.insert("source_path".into(), Value::String(source_path.into()));
        report.insert(
            "source_config_blake3".into(),
            Value::String(blake3::hash(&source).to_hex().to_string()),
        );
        report.insert("output_path".into(), Value::String(output_path.into()));
        report.insert(
            "output_config_blake3".into(),
            Value::String(blake3::hash(rendered.as_bytes()).to_hex().to_string()),
        );
        report.insert("request_budgets".into(), Value::Object(rates));
        report.insert(
            "unchanged_optional_sections".into(),
            if recipient_unchanged {
                norito::json!(["recipient-lookup"])
            } else {
                norito::json!([])
            },
        );
        let mut output_metadata = Map::new();
        for (field, value) in [
            ("device", metadata.dev()),
            ("inode", metadata.ino()),
            ("uid", u64::from(metadata.uid())),
            ("mode", u64::from(metadata.mode() & 0o7777)),
            ("links", metadata.nlink()),
            ("bytes", metadata.len()),
        ] {
            output_metadata.insert(field.into(), Value::from(value));
        }
        report.insert("output_metadata".into(), Value::Object(output_metadata));
        writeln!(writer, "{}", json::to_json(&Value::Object(report))?)?;
        Ok(())
    })();
    crate::soracloud::zeroize_taira_toml_table(&mut table);
    result
}

/// Use the daemon's environment-free loader and actual schema; diagnostics never include values.
fn validate_rate_node_config(
    table: &toml::Table,
    path: &Path,
) -> Result<iroha_config::parameters::actual::Root> {
    use iroha_config::node_config::{NodeConfigOptions, NodeFile, open_node_config};
    let (user, _) = open_node_config(
        NodeFile::Verified {
            path: path.to_owned(),
            table: table.clone(),
        },
        NodeConfigOptions { sora: true },
    )
    .map_err(|_| eyre!("validator configuration cannot be loaded by the native node schema"))?
    .read()
    .map_err(|_| eyre!("validator configuration does not match the native node schema"))?;
    user.parse()
        .map_err(|_| eyre!("validator configuration fails native runtime validation"))
}

/// Preserve every non-budget value, including feature switches, byte limits and window durations.
fn amend_request_rates(
    table: &mut toml::Table,
    sections: &[ToriiRateSection],
    rate: u32,
    burst: u32,
    gateway_window: std::time::Duration,
) -> Result<Map> {
    use ToriiRateSection::*;
    if rate == 0 || burst == 0 {
        return Err(eyre!("request rate and burst must be positive"));
    }
    let selected = |section| sections.is_empty() || sections.contains(&section);
    let recipient_present = table
        .get("torii")
        .and_then(toml::Value::as_table)
        .is_some_and(|torii| torii.contains_key("recipient_lookup"));
    let minute = if [Torii, Mcp, Push, Connect, OperatorAuth]
        .into_iter()
        .any(selected)
        || (selected(RecipientLookup) && recipient_present)
    {
        rate.checked_mul(60)
            .ok_or_else(|| eyre!("request rate per minute exceeds u32"))?
    } else {
        1 // Unselected minute fields are never written.
    };
    let gateway = if selected(Gateway) {
        let tokens = u128::from(rate)
            .checked_mul(gateway_window.as_nanos())
            .and_then(|value| value.checked_add(999_999_999))
            .map(|value| value / 1_000_000_000)
            .and_then(|value| u32::try_from(value).ok())
            .filter(|value| *value > 0)
            .ok_or_else(|| eyre!("gateway request budget cannot represent its retained window"))?;
        Some(tokens)
    } else {
        None
    };
    let mut budgets = Map::new();
    for (section, path, value) in [
        (
            Torii,
            &["torii", "query_rate_per_authority_per_sec"][..],
            rate,
        ),
        (Torii, &["torii", "query_burst_per_authority"][..], burst),
        (Torii, &["torii", "tx_rate_per_authority_per_sec"][..], rate),
        (Torii, &["torii", "tx_burst_per_authority"][..], burst),
        (
            Torii,
            &["torii", "deploy_rate_per_origin_per_sec"][..],
            rate,
        ),
        (Torii, &["torii", "deploy_burst_per_origin"][..], burst),
        (Torii, &["torii", "preauth_rate_per_ip_per_sec"][..], rate),
        (Torii, &["torii", "preauth_burst_per_ip"][..], burst),
        (
            Torii,
            &["torii", "soracloud_public_rate_per_ip_per_sec"][..],
            rate,
        ),
        (
            Torii,
            &["torii", "soracloud_public_burst_per_ip"][..],
            burst,
        ),
        (
            Torii,
            &[
                "torii",
                "soracloud_mutation_rate_per_account_origin_per_sec",
            ][..],
            rate,
        ),
        (
            Torii,
            &["torii", "soracloud_mutation_burst_per_account_origin"][..],
            burst,
        ),
        (Torii, &["torii", "proof_rate_per_minute"][..], minute),
        (Torii, &["torii", "proof_burst"][..], burst),
        (Mcp, &["torii", "mcp", "rate_per_minute"][..], minute),
        (Mcp, &["torii", "mcp", "burst"][..], burst),
        (Push, &["torii", "push", "rate_per_minute"][..], minute),
        (Push, &["torii", "push", "burst"][..], burst),
        (Content, &["content", "max_requests_per_second"][..], rate),
        (Content, &["content", "request_burst"][..], burst),
        (
            Connect,
            &["torii", "connect", "ws_rate_per_ip_per_min"][..],
            minute,
        ),
        (
            Gateway,
            &["sorafs", "gateway", "rate_limit", "max_requests"][..],
            gateway.unwrap_or(1),
        ),
        (
            OperatorAuth,
            &["torii", "operator_auth", "rate_per_minute"][..],
            minute,
        ),
        (
            OperatorAuth,
            &["torii", "operator_auth", "burst"][..],
            burst,
        ),
        (
            PrivacyIngest,
            &["torii", "soranet_privacy_ingest", "rate_per_sec"][..],
            rate,
        ),
        (
            PrivacyIngest,
            &["torii", "soranet_privacy_ingest", "burst"][..],
            burst,
        ),
        (
            RecipientLookup,
            &["torii", "recipient_lookup", "requests_per_minute"][..],
            minute,
        ),
    ] {
        if !selected(section) {
            continue;
        }
        // Recipient lookup is an optional whole typed leaf. Preserve its absence and inherited
        // runtime defaults rather than creating a partial configuration or inventing route policy.
        if section == RecipientLookup && !recipient_present {
            continue;
        }
        let (field, parents) = path
            .split_last()
            .ok_or_else(|| eyre!("empty budget path"))?;
        let mut cursor = &mut *table;
        for parent in parents {
            cursor = cursor
                .entry((*parent).to_owned())
                .or_insert_with(|| toml::Value::Table(toml::Table::new()))
                .as_table_mut()
                .ok_or_else(|| eyre!("request budget parent must be a table"))?;
        }
        cursor.insert((*field).to_owned(), toml::Value::Integer(i64::from(value)));
        budgets.insert(path.join("."), Value::from(value));
    }
    Ok(budgets)
}

#[derive(clap::Args, Debug)]
pub(super) struct ConfigRebase {
    /// Inherited owner-controlled regular config descriptor; contents never enter argv/stdout.
    #[arg(long, value_name = "FD", value_parser = clap::value_parser!(u32).range(3..=65535))]
    config_fd: u32,
    /// Exact current genesis.file value; drift fails before creating output.
    #[arg(long, value_name = "PATH")]
    expected_genesis_file: PathBuf,
    /// Replace genesis.file with this absolute path.
    #[arg(long, value_name = "PATH")]
    genesis_file: PathBuf,
    /// Exact current inline checked genesis identity; paired with --network-id.
    #[arg(long, value_name = "NETWORK_ID", requires = "network_id", value_parser = canonical_network_id)]
    expected_network_id: Option<NetworkId>,
    /// Explicit checked identity of the new genesis; paired with --expected-network-id.
    #[arg(long, value_name = "NETWORK_ID", requires = "expected_network_id", value_parser = canonical_network_id)]
    network_id: Option<NetworkId>,
    /// Enable operator signatures with exactly this dedicated canonical Ed25519 public key.
    #[arg(long, value_name = "PUBLIC_KEY", value_parser = canonical_operator_public_key)]
    operator_public_key: Option<PublicKey>,
    /// Fresh 0600 config in an existing owner-only directory; never overwritten.
    #[arg(long, value_name = "PATH")]
    output: PathBuf,
}

/// Rebase an inline client NetworkId, or project a checked Torii root without changing it.
///
/// Route projection consumes an inherited read-only descriptor and checks its original path.
/// The paired routes must come from explicit owner-approved public metadata. Use the same
/// checked NetworkId twice and a fresh file in a separate existing 0700 directory:
///
/// ```text
/// iroha --machine taira public-reset client-config-rebase --config-fd 3 \
///   --expected-network-id <CURRENT_CHECKED_NETWORK_ID> \
///   --network-id <CURRENT_CHECKED_NETWORK_ID> \
///   --expected-torii-url http://127.0.0.1:8080 --torii-url http://127.0.0.1:18080 \
///   --config-source-path /private/runtime/original/client.toml \
///   --output /private/runtime/projected/client.toml
/// ```
///
/// A native launcher passes descriptor 3; profile contents, keys and passwords never enter
/// argv or stdout. The original file and owned private ancestors stay unchanged. Native
/// source-relative key, namespace-proof, queue and witness references retain their source
/// base when the new profile is written elsewhere. A public `network_id_file` is accepted
/// only by this route-only operation: native code retains its owner-controlled descriptor,
/// checks exactly one canonical NetworkId plus LF against the explicit unchanged identity,
/// and anchors its original source path absolutely. That public identity file remains a live
/// dependency and must stay under custody during later use. Extends and competing inline/file
/// sources are refused. Inline network-only rebinding retains its separate existing policy.
/// No environment route override is used.
#[derive(clap::Args, Debug)]
pub(super) struct ClientConfigRebase {
    /// Inherited owner-controlled regular config descriptor; contents never enter argv/stdout.
    #[arg(long, value_name = "FD", value_parser = clap::value_parser!(u32).range(3..=65535))]
    config_fd: u32,
    /// Exact current checked network identity; route projection also admits a held public file.
    #[arg(long, value_name = "NETWORK_ID", value_parser = canonical_network_id)]
    expected_network_id: NetworkId,
    /// Explicit checked identity of the new genesis.
    #[arg(long, value_name = "NETWORK_ID", value_parser = canonical_network_id)]
    network_id: NetworkId,
    /// Exact current loopback HTTP or public HTTPS DNS root; paired with --torii-url and original provenance.
    #[arg(long, value_name = "URL", requires = "torii_url", value_parser = canonical_client_torii_url)]
    expected_torii_url: Option<String>,
    /// Project only the Torii route while keeping the exact current NetworkId.
    #[arg(long, value_name = "URL", requires_all = ["expected_torii_url", "config_source_path"], value_parser = canonical_client_torii_url)]
    torii_url: Option<String>,
    /// Absolute original profile path; metadata/provenance only, never reopened for its body.
    #[arg(long, value_name = "ABSOLUTE_PATH", requires = "torii_url")]
    config_source_path: Option<PathBuf>,
    /// Fresh 0600 config in an existing owner-only directory; never overwritten.
    #[arg(long, value_name = "PATH")]
    output: PathBuf,
}

#[derive(clap::Args, Debug)]
pub(super) struct OperatorKeygen {
    /// Fresh absolute runtime key file in a direct owner-only directory outside repositories.
    #[arg(long, value_name = "PATH")]
    private_key_file: PathBuf,
}

/// Admit an explicit loopback HTTP or public HTTPS DNS root without credentials or a path prefix.
fn canonical_client_torii_url(value: &str) -> Result<String, String> {
    let failure = || {
        "client Torii route must be a normalized loopback HTTP or public HTTPS DNS root".to_owned()
    };
    if value.starts_with("https://") {
        let root = value.strip_suffix('/').unwrap_or(value);
        validate_validator_public_origin(&format!("{root}/")).map_err(|_| failure())?;
        return Ok(root.to_owned());
    }
    let digits = value
        .strip_prefix("http://127.0.0.1:")
        .ok_or_else(failure)?;
    let digits = digits.strip_suffix('/').unwrap_or(digits);
    let port = digits.parse::<u16>().map_err(|_| failure())?;
    if port == 0 || digits != port.to_string() {
        return Err(failure());
    }
    Ok(format!("http://127.0.0.1:{port}"))
}

#[derive(Clone, Copy)]
struct ClientRouteProjection<'a> {
    expected: &'a str,
    replacement: &'a str,
    source_path: &'a Path,
    output_path: &'a Path,
}

fn canonical_operator_public_key(value: &str) -> Result<PublicKey, String> {
    let key = value
        .parse::<PublicKey>()
        .map_err(|_| "operator public key must be canonical Ed25519".to_owned())?;
    if key.try_algorithm().ok() != Some(Algorithm::Ed25519) || key.to_string() != value {
        return Err("operator public key must be canonical Ed25519".to_owned());
    }
    Ok(key)
}

fn canonical_network_id(value: &str) -> Result<NetworkId, String> {
    let network = value
        .parse::<NetworkId>()
        .map_err(|_| "network identity must be canonical checked NetworkId".to_owned())?;
    if network.to_string() != value {
        return Err("network identity must be canonical checked NetworkId".to_owned());
    }
    Ok(network)
}

/// Generate one runtime credential and print only its public identity and path.
pub(super) fn operator_keygen(args: &OperatorKeygen, writer: &mut impl Write) -> Result<()> {
    #[cfg(unix)]
    {
        operator_keygen_unix(args, writer)
    }
    #[cfg(not(unix))]
    {
        let _ = (args, writer);
        Err(eyre!(
            "operator key generation requires Unix private-file custody"
        ))
    }
}

#[cfg(unix)]
fn operator_keygen_unix(args: &OperatorKeygen, writer: &mut impl Write) -> Result<()> {
    use iroha_crypto::{ExposedPrivateKey, KeyPair};
    use rustix::fs::{Mode, OFlags};

    let path = &args.private_key_file;
    validate_absolute_normal_path(path, "operator private-key path")?;
    let path_text = path
        .to_str()
        .ok_or_else(|| eyre!("operator private-key path must be UTF-8"))?;
    let parent = path
        .parent()
        .ok_or_else(|| eyre!("operator private-key path has no parent"))?;
    validate_owner_private_dir(parent, "operator private-key directory")?;
    for ancestor in parent.ancestors() {
        match fs::symlink_metadata(ancestor.join(".git")) {
            Ok(_) => {
                return Err(eyre!(
                    "operator private-key path must be outside repositories"
                ));
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(_) => return Err(eyre!("cannot establish operator key repository exclusion")),
        }
    }
    let directory = File::from(rustix::fs::open(
        parent,
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    )?);
    let held_parent = directory.metadata()?;
    let named_parent = fs::symlink_metadata(parent)?;
    if held_parent.dev() != named_parent.dev()
        || held_parent.ino() != named_parent.ino()
        || held_parent.uid() != rustix::process::geteuid().as_raw()
        || held_parent.mode() & 0o7777 != 0o700
    {
        return Err(eyre!("operator private-key directory changed during open"));
    }
    let name = path
        .file_name()
        .ok_or_else(|| eyre!("operator private-key path has no file name"))?;
    let mut file = File::from(
        rustix::fs::openat(
            &directory,
            name,
            OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::from_raw_mode(0o600),
        )
        .map_err(|_| eyre!("cannot create fresh operator private-key file"))?,
    );
    file.set_permissions(fs::Permissions::from_mode(0o600))?;
    let key = KeyPair::try_random_with_algorithm(Algorithm::Ed25519)
        .map_err(|_| eyre!("native operator key generation failed"))?;
    let encoded = Zeroizing::new(
        ExposedPrivateKey(key.private_key().clone())
            .try_to_multihash_string()
            .map_err(|_| eyre!("cannot encode operator private key"))?,
    );
    file.write_all(encoded.as_bytes())
        .wrap_err("cannot write operator private-key file")?;
    file.write_all(b"\n")
        .wrap_err("cannot terminate operator private-key file")?;
    file.sync_all()
        .wrap_err("cannot synchronize operator private-key file")?;
    directory.sync_all()?;
    validate_owner_private_dir(parent, "operator private-key directory")?;
    let named_parent = fs::symlink_metadata(parent)?;
    let named = fs::symlink_metadata(path)?;
    let held = file.metadata()?;
    if held_parent.dev() != named_parent.dev()
        || held_parent.ino() != named_parent.ino()
        || named.dev() != held.dev()
        || named.ino() != held.ino()
        || !named.is_file()
        || named.file_type().is_symlink()
        || named.uid() != rustix::process::geteuid().as_raw()
        || named.mode() & 0o7777 != 0o600
        || named.nlink() != 1
        || named.len() != encoded.len() as u64 + 1
    {
        return Err(eyre!(
            "operator private-key custody changed during publication"
        ));
    }
    let mut report = Map::new();
    report.insert(
        "schema".into(),
        Value::String("iroha.taira.public-reset.operator-key.v1".into()),
    );
    report.insert(
        "public_key".into(),
        Value::String(key.public_key().to_string()),
    );
    report.insert("path".into(), Value::String(path_text.into()));
    writeln!(writer, "{}", json::to_json(&Value::Object(report))?)?;
    Ok(())
}

pub(super) fn config_rebase(args: &ConfigRebase) -> Result<()> {
    validate_absolute_normal_path(&args.expected_genesis_file, "expected genesis path")?;
    validate_absolute_normal_path(&args.genesis_file, "new genesis path")?;
    let network_rebind = match (&args.expected_network_id, &args.network_id) {
        (None, None) => None,
        (Some(expected), Some(replacement)) => Some((expected, replacement)),
        _ => {
            return Err(eyre!(
                "validator identity rebind requires paired expected and new NetworkId"
            ));
        }
    };
    let source = inherited_config(args.config_fd)?;
    let output = rebase_genesis_file(
        &source,
        &args.expected_genesis_file,
        &args.genesis_file,
        network_rebind,
        args.operator_public_key.as_ref(),
    )?;
    super::inputs::write_new_private(&args.output, &output)
}

pub(super) fn client_config_rebase(args: &ClientConfigRebase) -> Result<()> {
    let route = match (
        args.expected_torii_url.as_deref(),
        args.torii_url.as_deref(),
        args.config_source_path.as_deref(),
    ) {
        (None, None, None) => None,
        (Some(expected), Some(replacement), Some(source_path)) => Some(ClientRouteProjection {
            expected,
            replacement,
            source_path,
            output_path: &args.output,
        }),
        _ => {
            return Err(eyre!(
                "client route projection requires paired roots and original provenance"
            ));
        }
    };
    if let Some(route) = route {
        return project_client_config(args, route);
    }
    let source = crate::client_config::read_inherited_private_file(
        args.config_fd,
        MAX_CONFIG_BYTES,
        "client config",
    )?;
    let output =
        rebase_client_network_id(&source, &args.expected_network_id, &args.network_id, None)?;
    super::inputs::write_new_private(&args.output, &output)
}

#[cfg(unix)]
fn private_client_snapshot(
    metadata: &std::fs::Metadata,
) -> (u64, u64, u32, u32, u32, u64, u64, i64, i64, i64, i64) {
    use std::os::unix::fs::MetadataExt as _;
    (
        metadata.dev(),
        metadata.ino(),
        metadata.mode(),
        metadata.uid(),
        metadata.gid(),
        metadata.nlink(),
        metadata.len(),
        metadata.mtime(),
        metadata.mtime_nsec(),
        metadata.ctime(),
        metadata.ctime_nsec(),
    )
}

#[cfg(unix)]
struct RetainedClientSourceDirectory {
    path: PathBuf,
    directory: File,
    before: std::fs::Metadata,
    private: bool,
}

#[cfg(unix)]
fn client_directory_core(metadata: &std::fs::Metadata) -> (u64, u64, u32, u32, u32) {
    use std::os::unix::fs::MetadataExt as _;
    (
        metadata.dev(),
        metadata.ino(),
        metadata.mode(),
        metadata.uid(),
        metadata.gid(),
    )
}

/// Compare private ancestors in full and shared ancestors by stable core identity.
#[cfg(unix)]
fn client_directory_unchanged(
    before: &std::fs::Metadata,
    observed: &std::fs::Metadata,
    private: bool,
) -> bool {
    if private {
        private_client_snapshot(before) == private_client_snapshot(observed)
    } else {
        client_directory_core(before) == client_directory_core(observed)
    }
}

/// Freeze all owned private input ancestors, retaining shared ancestors by stable identity.
#[cfg(unix)]
fn retain_client_source_directories(source: &Path) -> Result<Vec<RetainedClientSourceDirectory>> {
    retain_client_input_directories(source, true)
}

/// Public NetworkId files may use safe shared parents; private profiles require mode0700.
#[cfg(unix)]
fn retain_client_input_directories(
    source: &Path,
    private_parent: bool,
) -> Result<Vec<RetainedClientSourceDirectory>> {
    use rustix::fs::{Mode, OFlags};
    use std::os::unix::fs::MetadataExt as _;
    validate_absolute_normal_path(source, "original client provenance")?;
    validate_no_symlink_ancestors(source, "original client provenance")?;
    let parent = source
        .parent()
        .ok_or_else(|| eyre!("original client provenance has no parent"))?;
    if private_parent {
        validate_owner_private_dir(parent, "original client directory")?;
    }
    let owner = rustix::process::geteuid().as_raw();
    let mut retained = Vec::new();
    for path in parent.ancestors() {
        let named = fs::symlink_metadata(path)?;
        if !named.is_dir()
            || named.file_type().is_symlink()
            || (named.uid() != 0 && named.uid() != owner)
            || named.mode() & 0o022 != 0
        {
            return Err(eyre!("original client ancestor has unsafe custody"));
        }
        let private = named.uid() == owner && named.mode() & 0o7777 == 0o700;
        let directory = File::from(rustix::fs::open(
            path,
            OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::empty(),
        )?);
        let before = directory.metadata()?;
        if !client_directory_unchanged(&named, &before, private) {
            return Err(eyre!("original client ancestor changed during retention"));
        }
        retained.push(RetainedClientSourceDirectory {
            path: path.to_owned(),
            private,
            directory,
            before,
        });
    }
    check_client_source_directories(&retained)?;
    Ok(retained)
}

#[cfg(unix)]
fn check_client_source_directories(retained: &[RetainedClientSourceDirectory]) -> Result<()> {
    for ancestor in retained {
        let opened = ancestor.directory.metadata()?;
        let named = fs::symlink_metadata(&ancestor.path)?;
        let changed = !client_directory_unchanged(&ancestor.before, &opened, ancestor.private)
            || !client_directory_unchanged(&ancestor.before, &named, ancestor.private);
        if changed || !named.is_dir() || named.file_type().is_symlink() {
            return Err(eyre!(
                "original client ancestor custody changed during route projection"
            ));
        }
    }
    Ok(())
}

/// Retain an explicitly selected public identity source through final route publication.
#[cfg(unix)]
struct RetainedClientNetworkFile {
    path: PathBuf,
    file: File,
    before: std::fs::Metadata,
    directories: Vec<RetainedClientSourceDirectory>,
}

#[cfg(unix)]
impl RetainedClientNetworkFile {
    fn check(&self) -> Result<()> {
        check_client_source_directories(&self.directories)?;
        let opened = self.file.metadata()?;
        let named = fs::symlink_metadata(&self.path)?;
        if private_client_snapshot(&opened) != private_client_snapshot(&self.before)
            || private_client_snapshot(&named) != private_client_snapshot(&self.before)
            || !named.is_file()
            || named.file_type().is_symlink()
        {
            return Err(eyre!("public client NetworkId file custody changed"));
        }
        Ok(())
    }
}

/// Parse only in native code; no profile values or identity file contents enter diagnostics.
#[cfg(unix)]
fn retain_client_network_file(
    bytes: &[u8],
    source_path: &Path,
    expected: &NetworkId,
) -> Result<Option<RetainedClientNetworkFile>> {
    use rustix::fs::{Mode, OFlags};
    use std::io::Read as _;
    use std::os::unix::fs::MetadataExt as _;
    let text = std::str::from_utf8(bytes).map_err(|_| eyre!("client config is not UTF-8"))?;
    let mut table: toml::Table =
        toml::from_str(text).map_err(|_| eyre!("client config is not valid TOML"))?;
    let result = (|| {
        if table.contains_key("extends") {
            return Err(eyre!("client route projection cannot use extends"));
        }
        let Some(value) = table.get("network_id_file") else {
            return Ok(None);
        };
        if table.contains_key("network_id") {
            return Err(eyre!(
                "client config has competing inline and file network identities"
            ));
        }
        let literal = value
            .as_str()
            .filter(|value| !value.is_empty() && value.trim() == *value && !value.contains('\0'))
            .ok_or_else(|| eyre!("client NetworkId file must be an explicit unambiguous path"))?;
        let selected = Path::new(literal);
        let path = if selected.is_absolute() {
            selected.to_owned()
        } else {
            source_path
                .parent()
                .ok_or_else(|| eyre!("original client provenance has no parent"))?
                .join(selected)
        };
        validate_absolute_normal_path(&path, "public client NetworkId file")?;
        let directories = retain_client_input_directories(&path, false)
            .map_err(|_| eyre!("public client NetworkId file has unsafe ancestor custody"))?;
        let named = fs::symlink_metadata(&path)?;
        if !named.is_file()
            || named.file_type().is_symlink()
            || named.uid() != rustix::process::geteuid().as_raw()
            || !matches!(named.mode() & 0o7777, 0o600 | 0o644)
            || named.nlink() != 1
            || named.len() == 0
            || named.len() > MAX_CLIENT_NETWORK_ID_BYTES
        {
            return Err(eyre!(
                "public client NetworkId file has unsafe custody or size"
            ));
        }
        let file = File::from(rustix::fs::open(
            &path,
            OFlags::RDONLY | OFlags::NONBLOCK | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::empty(),
        )?);
        let before = file.metadata()?;
        if private_client_snapshot(&named) != private_client_snapshot(&before) {
            return Err(eyre!(
                "public client NetworkId file changed during retention"
            ));
        }
        let retained = RetainedClientNetworkFile {
            path,
            file,
            before,
            directories,
        };
        retained.check()?;
        let mut body = Zeroizing::new(Vec::new());
        (&retained.file)
            .take(MAX_CLIENT_NETWORK_ID_BYTES + 1)
            .read_to_end(&mut body)?;
        retained.check()?;
        let literal = std::str::from_utf8(&body)
            .map_err(|_| eyre!("public client NetworkId file is not canonical UTF-8"))?
            .strip_suffix('\n')
            .ok_or_else(|| {
                eyre!("public client NetworkId file requires exactly one canonical LF")
            })?;
        let actual = canonical_network_id(literal)
            .map_err(|_| eyre!("public client NetworkId file is not canonical checked identity"))?;
        if body.len() as u64 != retained.before.len()
            || body.as_slice() != format!("{actual}\n").as_bytes()
            || &actual != expected
        {
            return Err(eyre!(
                "public client NetworkId file differs from the explicit retained identity"
            ));
        }
        retained.check()?;
        Ok(Some(retained))
    })();
    crate::soracloud::zeroize_taira_toml_table(&mut table);
    result
}

#[cfg(unix)]
fn project_client_config(
    args: &ClientConfigRebase,
    route: ClientRouteProjection<'_>,
) -> Result<()> {
    use std::os::fd::AsRawFd as _;
    use std::os::unix::fs::MetadataExt as _;
    validate_absolute_normal_path(&args.output, "projected client output")?;
    let source_directories = retain_client_source_directories(route.source_path)?;
    let output_parent = args
        .output
        .parent()
        .ok_or_else(|| eyre!("projected client output has no parent"))?;
    if source_directories
        .iter()
        .any(|ancestor| ancestor.private && ancestor.path == output_parent)
    {
        // Creating a file there would change an owned private input ancestor snapshot.
        return Err(eyre!(
            "client route projection requires a separate private output directory"
        ));
    }
    if args.expected_network_id != args.network_id {
        return Err(eyre!(
            "client route projection must preserve the exact current NetworkId"
        ));
    }
    let retained = crate::client_config::duplicate_inherited_descriptor(args.config_fd)?;
    let before = retained
        .metadata()
        .map_err(|_| eyre!("cannot inspect retained client descriptor"))?;
    if !before.is_file()
        || before.uid() != rustix::process::geteuid().as_raw()
        || before.mode() & 0o7777 != 0o600
        || before.nlink() != 1
    {
        return Err(eyre!(
            "projected client source must be an owner-private single-link regular file"
        ));
    }
    let check = || -> Result<()> {
        check_client_source_directories(&source_directories)?;
        let opened = retained
            .metadata()
            .map_err(|_| eyre!("cannot revalidate retained client descriptor"))?;
        let named = fs::symlink_metadata(route.source_path)
            .map_err(|_| eyre!("cannot inspect original client provenance"))?;
        if private_client_snapshot(&opened) != private_client_snapshot(&before)
            || private_client_snapshot(&named) != private_client_snapshot(&before)
            || !named.is_file()
            || named.file_type().is_symlink()
        {
            return Err(eyre!(
                "original client custody changed during route projection"
            ));
        }
        Ok(())
    };
    check()?;
    // Read through the retained descriptor, not the caller's mutable descriptor number.
    let source = crate::client_config::read_inherited_private_file(
        retained.as_raw_fd() as u32,
        MAX_CONFIG_BYTES,
        "client config",
    )?;
    check()?;
    let network_file =
        retain_client_network_file(&source, route.source_path, &args.expected_network_id)?;
    if network_file.as_ref().is_some_and(|retained| {
        retained
            .directories
            .iter()
            .any(|ancestor| ancestor.private && ancestor.path == output_parent)
    }) {
        // Publication must not change any retained private identity-file ancestor.
        return Err(eyre!(
            "client route projection requires a separate private output directory"
        ));
    }
    let check_all = || -> Result<()> {
        check()?;
        if let Some(retained) = &network_file {
            retained.check()?;
        }
        Ok(())
    };
    check_all()?;
    let output = materialize_client_network_source(
        &source,
        &args.expected_network_id,
        &args.network_id,
        Some(route),
        network_file
            .as_ref()
            .map(|retained| retained.path.as_path()),
    )?;
    check_all()?;
    write_projected_client_private(&args.output, &output, check_all)
}

#[cfg(not(unix))]
fn project_client_config(_: &ClientConfigRebase, _: ClientRouteProjection<'_>) -> Result<()> {
    Err(eyre!(
        "client route projection requires Unix private-file custody"
    ))
}

/// Retain the fresh output through final source checks and refuse to remove a foreign replacement.
///
/// The output directory is owner-private and must not be concurrently modified by another owner
/// process. The metadata check followed by unlink is not an atomic conditional unlink; a same-owner
/// actor racing between those operations is outside that private-directory concurrency contract.
#[cfg(unix)]
fn write_projected_client_private(
    path: &Path,
    bytes: &[u8],
    check_source: impl Fn() -> Result<()>,
) -> Result<()> {
    use rustix::fs::{AtFlags, Mode, OFlags};
    use std::os::unix::fs::MetadataExt as _;
    validate_absolute_normal_path(path, "projected client output")?;
    let parent = path
        .parent()
        .ok_or_else(|| eyre!("projected client output has no parent"))?;
    validate_owner_private_dir(parent, "projected client output directory")?;
    for ancestor in parent.ancestors() {
        match fs::symlink_metadata(ancestor.join(".git")) {
            Ok(_) => {
                return Err(eyre!(
                    "projected client output must be outside repositories"
                ));
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(_) => {
                return Err(eyre!(
                    "cannot establish projected client repository exclusion"
                ));
            }
        }
    }
    let directory = File::from(rustix::fs::open(
        parent,
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    )?);
    let before = directory.metadata()?;
    let core = |m: &std::fs::Metadata| (m.dev(), m.ino(), m.mode(), m.uid(), m.gid());
    let check_parent = || -> Result<()> {
        validate_owner_private_dir(parent, "projected client output directory")?;
        let named = fs::symlink_metadata(parent)?;
        if core(&directory.metadata()?) != core(&before) || core(&named) != core(&before) {
            return Err(eyre!("projected client output directory changed"));
        }
        Ok(())
    };
    check_source()?;
    check_parent()?;
    let name = path
        .file_name()
        .ok_or_else(|| eyre!("projected client output has no filename"))?;
    let mut file = File::from(
        rustix::fs::openat(
            &directory,
            name,
            OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::from_raw_mode(0o600),
        )
        .map_err(|_| eyre!("cannot create fresh private client projection"))?,
    );
    let result = (|| -> Result<()> {
        file.set_permissions(fs::Permissions::from_mode(0o600))?;
        file.write_all(bytes)?;
        file.sync_all()?;
        check_source()?;
        check_parent()?;
        let published = file.metadata()?;
        let check_output = || -> Result<()> {
            let named = File::from(rustix::fs::openat(
                &directory,
                name,
                OFlags::RDONLY | OFlags::NONBLOCK | OFlags::NOFOLLOW | OFlags::CLOEXEC,
                Mode::empty(),
            )?);
            let opened = file.metadata()?;
            let named_metadata = named.metadata()?;
            if private_client_snapshot(&opened) != private_client_snapshot(&published)
                || private_client_snapshot(&named_metadata) != private_client_snapshot(&published)
                || !opened.is_file()
                || opened.uid() != rustix::process::geteuid().as_raw()
                || opened.mode() & 0o7777 != 0o600
                || opened.nlink() != 1
                || opened.len() != bytes.len() as u64
            {
                return Err(eyre!(
                    "private client projection custody changed during publication"
                ));
            }
            Ok(())
        };
        check_output()?;
        directory.sync_all()?;
        check_source()?;
        check_parent()?;
        check_output()?;
        Ok(())
    })();
    if result.is_err() {
        if let Ok(named) = rustix::fs::openat(
            &directory,
            name,
            OFlags::RDONLY | OFlags::NONBLOCK | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::empty(),
        ) {
            let named = File::from(named);
            let ours = file.metadata()?;
            let current = named.metadata()?;
            if current.is_file() && (current.dev(), current.ino()) == (ours.dev(), ours.ino()) {
                rustix::fs::unlinkat(&directory, name, AtFlags::empty())
                    .map_err(|_| eyre!("cannot remove refused owned client projection"))?;
                directory.sync_all()?;
            }
        }
    }
    result
}

fn inherited_config(fd: u32) -> Result<Zeroizing<Vec<u8>>> {
    crate::client_config::read_inherited_private_file(fd, MAX_CONFIG_BYTES, "validator config")
}

fn rebase_genesis_file(
    bytes: &[u8],
    expected: &Path,
    replacement: &Path,
    network_rebind: Option<(&NetworkId, &NetworkId)>,
    operator_public_key: Option<&PublicKey>,
) -> Result<Zeroizing<Vec<u8>>> {
    if bytes.is_empty() || bytes.len() as u64 > MAX_CONFIG_BYTES {
        return Err(eyre!("validator config exceeds its materialization bound"));
    }
    let text = std::str::from_utf8(bytes).map_err(|_| eyre!("validator config is not UTF-8"))?;
    let mut table: toml::Table =
        toml::from_str(text).map_err(|_| eyre!("validator config is not valid TOML"))?;
    let result = (|| {
        if table.contains_key("extends") {
            return Err(eyre!(
                "validator config cannot inherit an unbound TOML source"
            ));
        }
        let genesis = table
            .get_mut("genesis")
            .and_then(toml::Value::as_table_mut)
            .ok_or_else(|| eyre!("validator config omits its genesis table"))?;
        let expected = expected
            .to_str()
            .ok_or_else(|| eyre!("expected genesis path is not UTF-8"))?;
        if genesis.get("file").and_then(toml::Value::as_str) != Some(expected) {
            return Err(eyre!(
                "validator genesis.file differs from the explicitly retained path"
            ));
        }
        if let Some((expected, replacement)) = network_rebind {
            if genesis.contains_key("expected_hash_file") {
                return Err(eyre!(
                    "validator identity rebind cannot use genesis.expected_hash_file"
                ));
            }
            rebind_inline_network_id(genesis, "expected_hash", expected, replacement)?;
        }
        let replacement = replacement
            .to_str()
            .ok_or_else(|| eyre!("new genesis path is not UTF-8"))?;
        genesis.insert(
            "file".to_owned(),
            toml::Value::String(replacement.to_owned()),
        );
        if let Some(key) = operator_public_key {
            canonical_operator_public_key(&key.to_string()).map_err(|error| eyre!(error))?;
            let torii = table
                .entry("torii")
                .or_insert_with(|| toml::Value::Table(toml::Table::new()))
                .as_table_mut()
                .ok_or_else(|| eyre!("validator torii configuration must be a table"))?;
            let signatures = torii
                .entry("operator_signatures")
                .or_insert_with(|| toml::Value::Table(toml::Table::new()))
                .as_table_mut()
                .ok_or_else(|| eyre!("validator operator_signatures must be a table"))?;
            signatures.insert("enabled".to_owned(), toml::Value::Boolean(true));
            signatures.insert(
                "allowed_public_keys".to_owned(),
                toml::Value::Array(vec![toml::Value::String(key.to_string())]),
            );
        }
        let rendered = Zeroizing::new(
            toml::to_string_pretty(&table)
                .map_err(|_| eyre!("cannot materialize validator config"))?,
        );
        if rendered.len() as u64 > MAX_CONFIG_BYTES {
            return Err(eyre!("materialized validator config exceeds its bound"));
        }
        Ok(Zeroizing::new(rendered.as_bytes().to_vec()))
    })();
    crate::soracloud::zeroize_taira_toml_table(&mut table);
    result
}

fn rebind_inline_network_id(
    table: &mut toml::Table,
    key: &str,
    expected: &NetworkId,
    replacement: &NetworkId,
) -> Result<()> {
    let literal = table
        .get(key)
        .and_then(toml::Value::as_str)
        .ok_or_else(|| eyre!("config requires its current inline checked network identity"))?;
    let actual = canonical_network_id(literal)
        .map_err(|_| eyre!("config inline network identity is not canonical checked NetworkId"))?;
    if &actual != expected {
        return Err(eyre!(
            "config network identity differs from the explicitly retained identity"
        ));
    }
    table.insert(key.to_owned(), toml::Value::String(replacement.to_string()));
    Ok(())
}

fn rebase_client_network_id(
    bytes: &[u8],
    expected: &NetworkId,
    replacement: &NetworkId,
    route: Option<ClientRouteProjection<'_>>,
) -> Result<Zeroizing<Vec<u8>>> {
    materialize_client_network_source(bytes, expected, replacement, route, None)
}

/// A file source is admitted only after native FD custody, never by network-only rebinding.
fn materialize_client_network_source(
    bytes: &[u8],
    expected: &NetworkId,
    replacement: &NetworkId,
    route: Option<ClientRouteProjection<'_>>,
    retained_network_file: Option<&Path>,
) -> Result<Zeroizing<Vec<u8>>> {
    if bytes.is_empty() || bytes.len() as u64 > MAX_CONFIG_BYTES {
        return Err(eyre!("client config exceeds its materialization bound"));
    }
    let text = std::str::from_utf8(bytes).map_err(|_| eyre!("client config is not UTF-8"))?;
    let mut table: toml::Table =
        toml::from_str(text).map_err(|_| eyre!("client config is not valid TOML"))?;
    let result = (|| {
        if table.contains_key("extends") {
            return Err(eyre!("client identity rebind cannot use extends"));
        }
        if table.contains_key("network_id_file") {
            let (Some(route), Some(retained_path)) = (route, retained_network_file) else {
                return Err(eyre!(
                    "client network-only rebind cannot use network_id_file"
                ));
            };
            if table.contains_key("network_id") || expected != replacement {
                return Err(eyre!(
                    "file network source requires an unchanged single selected identity"
                ));
            }
            let literal = table
                .get("network_id_file")
                .and_then(toml::Value::as_str)
                .ok_or_else(|| eyre!("client NetworkId file must be an explicit path"))?;
            let selected = Path::new(literal);
            let selected = if selected.is_absolute() {
                selected.to_owned()
            } else {
                route
                    .source_path
                    .parent()
                    .ok_or_else(|| eyre!("original client provenance has no parent"))?
                    .join(selected)
            };
            if selected != retained_path {
                return Err(eyre!(
                    "client NetworkId file differs from its retained source"
                ));
            }
            let anchored = retained_path
                .to_str()
                .ok_or_else(|| eyre!("public client NetworkId path is not UTF-8"))?;
            table.insert(
                "network_id_file".to_owned(),
                toml::Value::String(anchored.to_owned()),
            );
        } else {
            if retained_network_file.is_some() {
                return Err(eyre!(
                    "client inline network identity cannot use a retained file"
                ));
            }
            rebind_inline_network_id(&mut table, "network_id", expected, replacement)?;
        }
        if let Some(route) = route {
            if expected != replacement {
                return Err(eyre!(
                    "client route projection must preserve the exact current NetworkId"
                ));
            }
            project_client_route(&mut table, route)?;
        }
        let rendered = Zeroizing::new(
            toml::to_string_pretty(&table)
                .map_err(|_| eyre!("cannot materialize client config"))?,
        );
        if rendered.len() as u64 > MAX_CONFIG_BYTES {
            return Err(eyre!("materialized client config exceeds its bound"));
        }
        Ok(Zeroizing::new(rendered.as_bytes().to_vec()))
    })();
    crate::soracloud::zeroize_taira_toml_table(&mut table);
    result
}

/// Preserve the source path base explicitly when the route projection is written elsewhere.
fn anchor_client_reference(
    table: &mut toml::Table,
    keys: &[&str],
    source_parent: &Path,
) -> Result<()> {
    let mut value = table;
    for key in &keys[..keys.len() - 1] {
        let Some(child) = value.get_mut(*key) else {
            return Ok(());
        };
        value = child
            .as_table_mut()
            .ok_or_else(|| eyre!("client path reference has an invalid parent table"))?;
    }
    let Some(reference) = value.get_mut(keys[keys.len() - 1]) else {
        return Ok(());
    };
    let text = reference
        .as_str()
        .filter(|text| !text.is_empty() && !text.contains('\0'))
        .ok_or_else(|| eyre!("client path reference must be a nonempty path"))?;
    if keys == &["musubi", "publication", "namespace_delegation_file"][..] && text.trim() != text {
        return Err(eyre!("client namespace proof path must be unambiguous"));
    }
    let path = Path::new(text);
    if !path.is_absolute() {
        // Preserve filesystem resolution (including any .. component) rather than canonicalizing or following it.
        let anchored = source_parent.join(path);
        let text = anchored
            .to_str()
            .ok_or_else(|| eyre!("client path reference is not UTF-8"))?;
        *reference = toml::Value::String(text.to_owned());
    }
    Ok(())
}

fn project_client_route(table: &mut toml::Table, route: ClientRouteProjection<'_>) -> Result<()> {
    validate_absolute_normal_path(route.source_path, "original client provenance")?;
    validate_absolute_normal_path(route.output_path, "projected client output")?;
    let expected = canonical_client_torii_url(route.expected).map_err(|error| eyre!(error))?;
    let replacement =
        canonical_client_torii_url(route.replacement).map_err(|error| eyre!(error))?;
    let current = table
        .get("torii_url")
        .and_then(toml::Value::as_str)
        .ok_or_else(|| eyre!("client config requires its current explicit Torii root"))?;
    let current = canonical_client_torii_url(current)
        .map_err(|_| eyre!("client config has no admissible Torii root"))?;
    if current != expected {
        return Err(eyre!(
            "client Torii root differs from the explicitly retained route"
        ));
    }
    let source_parent = route
        .source_path
        .parent()
        .ok_or_else(|| eyre!("original client provenance has no parent"))?;
    if route.output_path.parent() != Some(source_parent) {
        for keys in [
            &["account", "private_key_file"][..],
            &["musubi", "publication", "namespace_delegation_file"][..],
            &["connect", "queue_root"][..],
            &["soracloud", "http_witness_file"][..],
        ] {
            anchor_client_reference(table, keys, source_parent)?;
        }
    }
    table.insert("torii_url".to_owned(), toml::Value::String(replacement));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE: &[u8] = b"private_key = 'fixture-secret-not-runtime'\nsoranet_transport_private_key = 'transport-fixture'\n[genesis]\nfile = '/retained/genesis.nrt'\nexpected_hash = 'public-hash'\n[streaming]\nidentity_private_key = 'streaming-fixture'\n";

    #[test]
    fn torii_rate_amendment_preserves_non_budget_values_and_optional_service_absence() {
        let mut table: toml::Table = toml::from_str(
            "private_key='fixture-secret-not-runtime'\n[torii]\npreauth_cooldown_ms=250\napi_tokens=['fixture-token']\nproof_egress_bytes_per_sec=2048\n[torii.push]\nrate_limit_enabled=false\n[torii.operator_auth]\nenabled=false\n[content]\nmax_egress_bytes_per_second=1024\n[sorafs.gateway.rate_limit]\nwindow='2s'\nban='1s'\n",
        ).unwrap();
        let retained = table.clone();
        let budgets = amend_request_rates(
            &mut table,
            &[],
            1000,
            10000,
            std::time::Duration::from_secs(2),
        )
        .unwrap();
        assert_eq!(
            table["torii"]["query_rate_per_authority_per_sec"].as_integer(),
            Some(1000)
        );
        assert_eq!(
            table["torii"]["mcp"]["rate_per_minute"].as_integer(),
            Some(60000)
        );
        assert_eq!(table["content"]["request_burst"].as_integer(), Some(10000));
        assert_eq!(
            table["torii"]["connect"]["ws_rate_per_ip_per_min"].as_integer(),
            Some(60000)
        );
        assert_eq!(
            table["sorafs"]["gateway"]["rate_limit"]["max_requests"].as_integer(),
            Some(2000)
        );
        assert!(
            !table["torii"]
                .as_table()
                .unwrap()
                .contains_key("recipient_lookup")
        );
        assert!(!budgets.contains_key("torii.recipient_lookup.requests_per_minute"));
        // Every preexisting leaf except an explicit request budget remains identical.
        fn compare(old: &toml::Table, new: &toml::Table, prefix: &str, budgets: &Map) {
            for (field, value) in old {
                let path = if prefix.is_empty() {
                    field.clone()
                } else {
                    format!("{prefix}.{field}")
                };
                if let Some(nested) = value.as_table() {
                    compare(nested, new[field].as_table().unwrap(), &path, budgets);
                } else if !budgets.contains_key(&path) {
                    assert_eq!(value, &new[field], "{path}");
                }
            }
        }
        compare(&retained, &table, "", &budgets);
        let mut optional: toml::Table = toml::from_str("[torii.recipient_lookup]\nrequests_per_minute=30\npolicy_id='cbuae_aed_sbp_pkr'\nrequest_timeout_ms=4000\nroutes=[]\n").unwrap();
        amend_request_rates(
            &mut optional,
            &[ToriiRateSection::RecipientLookup],
            1000,
            10000,
            std::time::Duration::from_secs(60),
        )
        .unwrap();
        assert_eq!(
            optional["torii"]["recipient_lookup"]["requests_per_minute"].as_integer(),
            Some(60000)
        );
        assert_eq!(
            optional["torii"]["recipient_lookup"]["policy_id"].as_str(),
            Some("cbuae_aed_sbp_pkr")
        );
    }

    #[test]
    fn torii_rate_amendment_sections_and_checked_arithmetic_are_exact() {
        let mut table = toml::Table::new();
        let budgets = amend_request_rates(
            &mut table,
            &[ToriiRateSection::Mcp],
            5,
            10,
            std::time::Duration::ZERO,
        )
        .unwrap();
        assert_eq!(budgets.len(), 2);
        assert_eq!(table.len(), 1);
        assert_eq!(table["torii"].as_table().unwrap().len(), 1);
        assert_eq!(
            table["torii"]["mcp"]["rate_per_minute"].as_integer(),
            Some(300)
        );
        for (rate, burst, window) in [
            (0, 1, 60),
            (1, 0, 60),
            (u32::MAX, 1, 60),
            (1, 1, 0),
            (1_000_000, 1, u64::MAX),
        ] {
            let mut empty = toml::Table::new();
            assert!(
                amend_request_rates(
                    &mut empty,
                    &[],
                    rate,
                    burst,
                    std::time::Duration::from_secs(window)
                )
                .is_err()
            );
            assert!(empty.is_empty());
        }
        let mut partial = toml::Table::new();
        let budget = amend_request_rates(
            &mut partial,
            &[ToriiRateSection::Gateway],
            3,
            4,
            std::time::Duration::from_millis(500),
        )
        .unwrap();
        assert_eq!(
            budget["sorafs.gateway.rate_limit.max_requests"],
            Value::from(2_u32)
        );
        let mut seconds_only = toml::Table::new();
        let budgets = amend_request_rates(
            &mut seconds_only,
            &[ToriiRateSection::Content, ToriiRateSection::PrivacyIngest],
            100_000_000,
            1,
            std::time::Duration::ZERO,
        )
        .unwrap();
        assert_eq!(budgets.len(), 4);
        assert_eq!(
            budgets["content.max_requests_per_second"],
            Value::from(100_000_000_u32)
        );
    }

    #[test]
    fn torii_rate_amendment_cli_requires_explicit_positive_values_and_provenance() {
        use clap::Parser as _;
        let arguments = [
            "iroha",
            "--machine",
            "taira",
            "public-reset",
            "torii-rate-config-amend",
            "--config-fd",
            "3",
            "--config-source-path",
            "/private/runtime/validator/config.toml",
            "--rate-per-second",
            "1000000",
            "--burst",
            "10000000",
            "--output",
            "/private/runtime/validator/config.next.toml",
        ];
        assert!(crate::Args::try_parse_from(arguments).is_ok());
        for (index, invalid) in [(6, "2"), (10, "0"), (12, "0")] {
            let mut args = arguments;
            args[index] = invalid;
            assert!(crate::Args::try_parse_from(args).is_err());
        }
        let mut scoped = arguments.to_vec();
        scoped.extend(["--section", "torii", "--section", "mcp"]);
        assert!(crate::Args::try_parse_from(scoped).is_ok());
    }

    #[cfg(unix)]
    #[test]
    fn torii_rate_amendment_native_fd_custody_emits_only_public_receipt() {
        use std::os::fd::AsRawFd as _;
        let directory = operator_runtime_fixture();
        let root = directory.path().canonicalize().unwrap();
        let source = root.join("config.toml");
        let bytes = include_bytes!("../../iroha_config/tests/fixtures/base.toml");
        fs::write(&source, bytes).unwrap();
        fs::set_permissions(&source, fs::Permissions::from_mode(0o600)).unwrap();
        let file = File::open(&source).unwrap();
        let args = ToriiRateConfigAmend {
            config_fd: file.as_raw_fd() as u32,
            config_source_path: source.clone(),
            rate_per_second: 1_000_000,
            burst: 10_000_000,
            section: Vec::new(),
            output: root.join("config.next.toml"),
        };
        let mut stdout = Vec::new();
        torii_rate_config_amend(&args, &mut stdout).unwrap();
        assert_eq!(fs::read(&source).unwrap(), bytes);
        assert_eq!(args.output.metadata().unwrap().mode() & 0o7777, 0o600);
        let rendered = fs::read(&args.output).unwrap();
        let report: Value = json::from_slice(&stdout).unwrap();
        assert_eq!(report["source_path"].as_str(), source.to_str());
        assert_eq!(
            report["source_config_blake3"].as_str(),
            Some(blake3::hash(bytes).to_hex().as_str())
        );
        assert_eq!(
            report["output_config_blake3"].as_str(),
            Some(blake3::hash(&rendered).to_hex().as_str())
        );
        assert_eq!(
            report["request_budgets"]["torii.query_rate_per_authority_per_sec"],
            Value::from(1_000_000_u32)
        );
        let text = String::from_utf8(stdout).unwrap();
        for secret in ["892620", "802620", "private_key", "api_tokens"] {
            assert!(!text.contains(secret));
        }
        assert!(torii_rate_config_amend(&args, &mut Vec::new()).is_err());
        assert_eq!(fs::read(&args.output).unwrap(), rendered);
    }

    #[cfg(unix)]
    #[test]
    fn torii_rate_amendment_refuses_unbound_invalid_or_unsafe_inputs_before_write() {
        use std::os::fd::AsRawFd as _;
        let directory = operator_runtime_fixture();
        let root = directory.path().canonicalize().unwrap();
        let source = root.join("config.toml");
        let output = root.join("config.next.toml");
        for bytes in [
            FIXTURE.to_vec(),
            [
                b"extends='fixture-secret-not-runtime'\n".as_slice(),
                FIXTURE,
            ]
            .concat(),
            b"private_key = 'fixture-secret-not-runtime\n".to_vec(),
        ] {
            fs::write(&source, bytes).unwrap();
            fs::set_permissions(&source, fs::Permissions::from_mode(0o600)).unwrap();
            let file = File::open(&source).unwrap();
            let args = ToriiRateConfigAmend {
                config_fd: file.as_raw_fd() as u32,
                config_source_path: source.clone(),
                rate_per_second: 1_000_000,
                burst: 10_000_000,
                section: Vec::new(),
                output: output.clone(),
            };
            let error = torii_rate_config_amend(&args, &mut Vec::new()).unwrap_err();
            assert!(!format!("{error:#}").contains("fixture-secret"));
            assert!(!output.exists());
        }
        fs::write(
            &source,
            include_bytes!("../../iroha_config/tests/fixtures/base.toml"),
        )
        .unwrap();
        let file = File::open(&source).unwrap();
        let mut args = ToriiRateConfigAmend {
            config_fd: file.as_raw_fd() as u32,
            config_source_path: source.clone(),
            rate_per_second: 1_000_000,
            burst: 10_000_000,
            section: Vec::new(),
            output: output.clone(),
        };
        fs::set_permissions(&source, fs::Permissions::from_mode(0o644)).unwrap();
        assert!(torii_rate_config_amend(&args, &mut Vec::new()).is_err());
        fs::set_permissions(&source, fs::Permissions::from_mode(0o600)).unwrap();
        fs::hard_link(&source, root.join("linked.toml")).unwrap();
        assert!(torii_rate_config_amend(&args, &mut Vec::new()).is_err());
        fs::remove_file(root.join("linked.toml")).unwrap();
        args.config_source_path = root.join("different.toml");
        assert!(torii_rate_config_amend(&args, &mut Vec::new()).is_err());
        args.config_source_path = source.clone();
        args.output = root.join("elsewhere").join("next.toml");
        assert!(torii_rate_config_amend(&args, &mut Vec::new()).is_err());
        assert!(!output.exists());
    }

    fn network_fixture(seed: &[u8]) -> NetworkId {
        NetworkId::from_genesis_hash(iroha_crypto::HashOf::from_untyped_unchecked(Hash::new(
            seed,
        )))
    }

    fn validator_network_fixture(network: &NetworkId) -> Vec<u8> {
        String::from_utf8(FIXTURE.to_vec())
            .unwrap()
            .replace("public-hash", &network.to_string())
            .into_bytes()
    }

    fn client_network_fixture(network: &NetworkId) -> Vec<u8> {
        format!("network_id = '{network}'\ntorii_url = 'https://taira.sora.org'\n[account]\nprofile = 'taira'\nprivate_key = 'fixture-secret-not-runtime'\npublic_key = 'retained-public-fixture'\n").into_bytes()
    }

    fn route_fixture(network: &NetworkId) -> Vec<u8> {
        format!(
            r#"chain = "fc56984b-2be7-431d-840e-21514d1883f0"
network_id = "{network}"
torii_url = "http://127.0.0.1:8080/"
api_token = "fixture-token-not-runtime"
[account]
profile = "taira"
public_key = "ed0120CE7FA46C9DCE7EA4B125E2E36BDB63EA33073E7590AC92816AE1E861B7048B03"
private_key = "802620CCF31D85E3B32A4BEA59987CE0C78E3B8E2DB93881468AB2435FE45D5C9DCD53"
[basic_auth]
web_login = "fixture-reader"
password = "fixture-password-not-runtime"
"#
        )
        .into_bytes()
    }

    fn route_projection<'a>(source: &'a Path, output: &'a Path) -> ClientRouteProjection<'a> {
        ClientRouteProjection {
            expected: "http://127.0.0.1:8080",
            replacement: "http://127.0.0.1:18080",
            source_path: source,
            output_path: output,
        }
    }

    #[test]
    fn client_route_roots_are_normalized_and_never_credentials_or_paths() {
        for port in [1, 80, 8080, 18080, 65535] {
            let root = format!("http://127.0.0.1:{port}");
            assert_eq!(canonical_client_torii_url(&root).unwrap(), root);
            assert_eq!(
                canonical_client_torii_url(&format!("{root}/")).unwrap(),
                root
            );
        }
        for root in [
            "https://taira.sora.org",
            "https://taira.sora.org:8443",
            "https://taira.sora.org:8446",
        ] {
            assert_eq!(canonical_client_torii_url(root).unwrap(), root);
            assert_eq!(
                canonical_client_torii_url(&format!("{root}/")).unwrap(),
                root
            );
        }
        for invalid in [
            "http://127.0.0.1:0",
            "http://127.0.0.1:65536",
            "http://127.0.0.1:018080",
            "http://127.0.0.1:+18080",
            "http://127.0.0.1:18080 ",
            "http://127.0.0.1:18080\n",
            "http://127.0.0.1",
            "http://localhost:18080",
            "http://0.0.0.0:18080",
            "http://127.0.0.2:18080",
            "http://[::1]:18080",
            "https://127.0.0.1:18080",
            "https://localhost:8443",
            "https://taira.local:8443",
            "https://taira.sora.org:0",
            "https://taira.sora.org:08443",
            "https://taira.sora.org:8443/path",
            "https://taira.sora.org:8443?query=1",
            "https://taira.sora.org:8443#fragment",
            "https://user:secret@taira.sora.org:8443",
            "https://taira.sora.org:8443//",
            "https://taira.sora.org:8443\n",
            "http://user:secret@127.0.0.1:18080",
            "http://127.0.0.1:18080/path",
            "http://127.0.0.1:18080?query=1",
            "http://127.0.0.1:18080#fragment",
            "http://127.0.0.1:18080//",
            "http://127.0.0.1:18080\\path",
        ] {
            let error = canonical_client_torii_url(invalid).unwrap_err();
            assert!(!error.contains("secret"));
        }
    }

    #[test]
    fn client_route_projection_preserves_identity_and_custody_for_public_tls() {
        let network = network_fixture(b"public TLS projection keeps original network");
        let source = Path::new("/private/runtime/original/client.toml");
        let output = Path::new("/private/runtime/projected/client.toml");
        let mut table: toml::Table =
            toml::from_str(std::str::from_utf8(&route_fixture(&network)).unwrap()).unwrap();
        let original = table.clone();
        project_client_route(
            &mut table,
            ClientRouteProjection {
                expected: "http://127.0.0.1:8080",
                replacement: "https://taira.sora.org:8443/",
                source_path: source,
                output_path: output,
            },
        )
        .unwrap();
        assert_eq!(
            table["torii_url"].as_str(),
            Some("https://taira.sora.org:8443")
        );
        table.insert("torii_url".into(), original["torii_url"].clone());
        assert!(table == original);
    }

    #[test]
    fn client_route_cli_requires_both_roots_and_original_provenance() {
        use clap::Parser as _;
        let network = network_fixture(b"route CLI unchanged network").to_string();
        let base = [
            "iroha",
            "--machine",
            "taira",
            "public-reset",
            "client-config-rebase",
            "--config-fd",
            "3",
            "--expected-network-id",
            network.as_str(),
            "--network-id",
            network.as_str(),
            "--output",
            "/private/runtime/projected/client.toml",
        ];
        let mut complete = base.to_vec();
        complete.extend([
            "--expected-torii-url",
            "http://127.0.0.1:8080",
            "--torii-url",
            "http://127.0.0.1:18080",
            "--config-source-path",
            "/private/runtime/original/client.toml",
        ]);
        assert!(crate::Args::try_parse_from(complete).is_ok());
        let mut public_tls = base.to_vec();
        public_tls.extend([
            "--expected-torii-url",
            "http://127.0.0.1:8080",
            "--torii-url",
            "https://taira.sora.org:8443/",
            "--config-source-path",
            "/private/runtime/original/client.toml",
        ]);
        assert!(crate::Args::try_parse_from(public_tls).is_ok());
        for partial in [
            vec!["--expected-torii-url", "http://127.0.0.1:8080"],
            vec!["--torii-url", "http://127.0.0.1:18080"],
            vec![
                "--config-source-path",
                "/private/runtime/original/client.toml",
            ],
            vec![
                "--expected-torii-url",
                "http://127.0.0.1:8080",
                "--torii-url",
                "http://127.0.0.1:18080",
            ],
            vec![
                "--torii-url",
                "http://127.0.0.1:18080",
                "--config-source-path",
                "/private/runtime/original/client.toml",
            ],
        ] {
            let mut arguments = base.to_vec();
            arguments.extend(partial);
            assert!(crate::Args::try_parse_from(arguments).is_err());
        }
    }

    fn file_route_fixture(network: &NetworkId, file: &str) -> Vec<u8> {
        String::from_utf8(route_fixture(network))
            .unwrap()
            .replace(
                &format!("network_id = \"{network}\""),
                &format!("network_id_file = '{file}'"),
            )
            .into_bytes()
    }

    #[cfg(unix)]
    #[test]
    fn client_directory_opening_preserves_shared_core_and_private_full_metadata() {
        let directory = operator_runtime_fixture();
        let root = directory.path().canonicalize().unwrap();
        for (mode, private) in [(0o755, false), (0o700, true)] {
            let parent = root.join(format!("directory-{mode:o}"));
            fs::create_dir(&parent).unwrap();
            fs::set_permissions(&parent, fs::Permissions::from_mode(mode)).unwrap();
            let before = fs::symlink_metadata(&parent).unwrap();
            fs::write(
                parent.join("unrelated-public-child"),
                b"public metadata fixture",
            )
            .unwrap();
            let after = fs::symlink_metadata(&parent).unwrap();
            assert_ne!(
                private_client_snapshot(&before),
                private_client_snapshot(&after)
            );
            assert_eq!(
                client_directory_unchanged(&before, &after, private),
                !private
            );
            fs::set_permissions(&parent, fs::Permissions::from_mode(0o750)).unwrap();
            assert!(!client_directory_unchanged(
                &before,
                &fs::symlink_metadata(&parent).unwrap(),
                private
            ));
            fs::rename(&parent, root.join(format!("displaced-{mode:o}"))).unwrap();
            fs::create_dir(&parent).unwrap();
            fs::set_permissions(&parent, fs::Permissions::from_mode(mode)).unwrap();
            assert!(!client_directory_unchanged(
                &before,
                &fs::symlink_metadata(&parent).unwrap(),
                private
            ));
        }
    }

    #[cfg(unix)]
    fn file_route_args(
        fd: &File,
        source: &Path,
        output: &Path,
        network: NetworkId,
    ) -> ClientConfigRebase {
        use std::os::fd::AsRawFd as _;
        ClientConfigRebase {
            config_fd: fd.as_raw_fd() as u32,
            expected_network_id: network,
            network_id: network,
            expected_torii_url: Some("http://127.0.0.1:8080".into()),
            torii_url: Some("http://127.0.0.1:18080".into()),
            config_source_path: Some(source.to_owned()),
            output: output.to_owned(),
        }
    }

    #[cfg(unix)]
    #[test]
    fn client_route_network_file_roundtrip_preserves_source_selection_and_native_identity() {
        for mode in [0o600, 0o644] {
            let directory = operator_runtime_fixture();
            let root = directory.path().canonicalize().unwrap();
            let original = root.join("original");
            let projected = root.join("projected");
            for parent in [&original, &projected] {
                fs::create_dir(parent).unwrap();
                fs::set_permissions(parent, fs::Permissions::from_mode(0o700)).unwrap();
            }
            let network = network_fixture(b"file route unchanged network");
            let identity = original.join("genesis.expected_hash");
            fs::write(&identity, format!("{network}\n")).unwrap();
            fs::set_permissions(&identity, fs::Permissions::from_mode(mode)).unwrap();
            let source = file_route_fixture(&network, "genesis.expected_hash");
            let source_path = original.join("client.toml");
            fs::write(&source_path, &source).unwrap();
            fs::set_permissions(&source_path, fs::Permissions::from_mode(0o600)).unwrap();
            let source_before = fs::symlink_metadata(&source_path).unwrap();
            let identity_before = fs::symlink_metadata(&identity).unwrap();
            let mut fd = File::open(&source_path).unwrap();
            fd.seek(std::io::SeekFrom::Start(7)).unwrap();
            let output = projected.join("client.toml");
            let command = super::super::PublicReset {
                command: super::super::PublicResetCommand::ClientConfigRebase(file_route_args(
                    &fd,
                    &source_path,
                    &output,
                    network,
                )),
            };
            let mut stdout = Vec::new();
            command.run_without_client_config(&mut stdout).unwrap();
            assert!(stdout.is_empty());
            assert_eq!(fd.stream_position().unwrap(), 7);
            let result = Zeroizing::new(fs::read(&output).unwrap());
            let table: toml::Table = toml::from_str(std::str::from_utf8(&result).unwrap()).unwrap();
            assert!(!table.contains_key("network_id"));
            assert_eq!(table["network_id_file"].as_str(), identity.to_str());
            let (before, _) =
                iroha::config::Config::load_bytes_with_musubi_publication(&source_path, &source)
                    .unwrap();
            let (after, _) =
                iroha::config::Config::load_bytes_with_musubi_publication(&output, &result)
                    .unwrap();
            assert!(
                before.network_id == after.network_id
                    && before.account == after.account
                    && before.chain == after.chain
            );
            assert_eq!(before.key_pair.public_key(), after.key_pair.public_key());
            assert_eq!(after.torii_api_url.as_str(), "http://127.0.0.1:18080/");
            assert_eq!(
                private_client_snapshot(&source_before),
                private_client_snapshot(&fs::symlink_metadata(&source_path).unwrap())
            );
            assert_eq!(
                private_client_snapshot(&identity_before),
                private_client_snapshot(&fs::symlink_metadata(&identity).unwrap())
            );
            assert_eq!(fs::read(&source_path).unwrap(), source);
        }
    }

    #[cfg(unix)]
    #[test]
    fn client_route_network_file_wrong_or_noncanonical_identity_refuses_before_output() {
        let directory = operator_runtime_fixture();
        let root = directory.path().canonicalize().unwrap();
        let original = root.join("original");
        let projected = root.join("projected");
        for parent in [&original, &projected] {
            fs::create_dir(parent).unwrap();
            fs::set_permissions(parent, fs::Permissions::from_mode(0o700)).unwrap();
        }
        let network = network_fixture(b"expected file identity");
        let other = network_fixture(b"wrong file identity");
        let source = original.join("client.toml");
        fs::write(
            &source,
            file_route_fixture(&network, "genesis.expected_hash"),
        )
        .unwrap();
        fs::set_permissions(&source, fs::Permissions::from_mode(0o600)).unwrap();
        let fd = File::open(&source).unwrap();
        let output = projected.join("client.toml");
        let args = file_route_args(&fd, &source, &output, network);
        for body in [
            format!("{other}\n").into_bytes(),
            network.to_string().into_bytes(),
            format!("{network}\n\n").into_bytes(),
            format!("{network}\r\n").into_bytes(),
            format!(" {network}\n").into_bytes(),
            b"malformed-public-fixture\n".to_vec(),
            vec![0xff; 75],
            vec![b'x'; 76],
        ] {
            let identity = original.join("genesis.expected_hash");
            fs::write(&identity, body).unwrap();
            fs::set_permissions(&identity, fs::Permissions::from_mode(0o600)).unwrap();
            let error = client_config_rebase(&args).unwrap_err();
            assert!(!format!("{error:#}").contains("fixture-token-not-runtime"));
            assert!(!output.exists());
        }
    }

    #[cfg(unix)]
    #[test]
    fn client_route_network_file_competing_inherited_or_unbound_sources_refuse() {
        let directory = operator_runtime_fixture();
        let root = directory.path().canonicalize().unwrap();
        let network = network_fixture(b"single file network selection");
        for source in [
            [
                b"extends = 'fixture-secret-not-runtime'\n".as_slice(),
                file_route_fixture(&network, "missing").as_slice(),
            ]
            .concat(),
            [
                format!("network_id = '{network}'\n").as_bytes(),
                file_route_fixture(&network, "missing").as_slice(),
            ]
            .concat(),
            file_route_fixture(&network, "../identity"),
            file_route_fixture(&network, " identity "),
        ] {
            let error = retain_client_network_file(&source, &root.join("client.toml"), &network)
                .err()
                .unwrap();
            assert!(!format!("{error:#}").contains("fixture-secret-not-runtime"));
        }
        let source = file_route_fixture(&network, "genesis.expected_hash");
        // Network-only rebinding never opens or silently replaces a file source.
        assert!(rebase_client_network_id(&source, &network, &network, None).is_err());
        assert!(
            rebase_client_network_id(
                &source,
                &network,
                &network,
                Some(route_projection(
                    &root.join("client.toml"),
                    &root.join("out.toml")
                ))
            )
            .is_err()
        );
    }

    #[cfg(unix)]
    #[test]
    fn client_route_network_file_unsafe_symlink_hardlink_and_nonregular_sources_refuse() {
        let directory = operator_runtime_fixture();
        let root = directory.path().canonicalize().unwrap();
        let network = network_fixture(b"safe file custody");
        let identity = root.join("genesis.expected_hash");
        let target = root.join("target");
        fs::write(&target, format!("{network}\n")).unwrap();
        fs::set_permissions(&target, fs::Permissions::from_mode(0o600)).unwrap();
        let source = file_route_fixture(&network, "genesis.expected_hash");
        std::os::unix::fs::symlink(&target, &identity).unwrap();
        assert!(retain_client_network_file(&source, &root.join("client.toml"), &network).is_err());
        fs::remove_file(&identity).unwrap();
        fs::hard_link(&target, &identity).unwrap();
        assert!(retain_client_network_file(&source, &root.join("client.toml"), &network).is_err());
        fs::remove_file(&identity).unwrap();
        fs::write(&identity, format!("{network}\n")).unwrap();
        for mode in [0o666, 0o622, 0o400] {
            fs::set_permissions(&identity, fs::Permissions::from_mode(mode)).unwrap();
            assert!(
                retain_client_network_file(&source, &root.join("client.toml"), &network).is_err()
            );
        }
        fs::remove_file(&identity).unwrap();
        fs::create_dir(&identity).unwrap();
        assert!(retain_client_network_file(&source, &root.join("client.toml"), &network).is_err());
        fs::remove_dir(&identity).unwrap();
        fs::write(&identity, format!("{network}\n")).unwrap();
        fs::set_permissions(&identity, fs::Permissions::from_mode(0o600)).unwrap();
        let alias = root.join("alias");
        std::os::unix::fs::symlink(&root, &alias).unwrap();
        let aliased = file_route_fixture(&network, "alias/genesis.expected_hash");
        assert!(retain_client_network_file(&aliased, &root.join("client.toml"), &network).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn client_route_network_file_inode_and_restored_body_mtime_drift_refuse() {
        let directory = operator_runtime_fixture();
        let root = directory.path().canonicalize().unwrap();
        let network = network_fixture(b"retained file identity");
        let identity = root.join("genesis.expected_hash");
        let body = format!("{network}\n");
        fs::write(&identity, &body).unwrap();
        fs::set_permissions(&identity, fs::Permissions::from_mode(0o600)).unwrap();
        let source = file_route_fixture(&network, "genesis.expected_hash");
        let retained = retain_client_network_file(&source, &root.join("client.toml"), &network)
            .unwrap()
            .unwrap();
        let replacement = root.join("replacement");
        fs::write(&replacement, &body).unwrap();
        fs::set_permissions(&replacement, fs::Permissions::from_mode(0o600)).unwrap();
        fs::rename(&replacement, &identity).unwrap();
        assert!(retained.check().is_err());
        let retained = retain_client_network_file(&source, &root.join("client.toml"), &network)
            .unwrap()
            .unwrap();
        let before = fs::metadata(&identity).unwrap();
        fs::write(&identity, vec![b'x'; body.len()]).unwrap();
        fs::write(&identity, &body).unwrap();
        File::open(&identity)
            .unwrap()
            .set_times(std::fs::FileTimes::new().set_modified(before.modified().unwrap()))
            .unwrap();
        assert!(retained.check().is_err());
    }

    #[cfg(unix)]
    #[test]
    fn client_route_network_file_restored_ancestor_refuses_even_when_leaf_is_unchanged() {
        let directory = operator_runtime_fixture();
        let root = directory.path().canonicalize().unwrap();
        let parent = root.join("original");
        fs::create_dir(&parent).unwrap();
        fs::set_permissions(&parent, fs::Permissions::from_mode(0o700)).unwrap();
        let network = network_fixture(b"retained ancestor file identity");
        let identity = parent.join("genesis.expected_hash");
        fs::write(&identity, format!("{network}\n")).unwrap();
        fs::set_permissions(&identity, fs::Permissions::from_mode(0o600)).unwrap();
        let source = file_route_fixture(&network, "genesis.expected_hash");
        let retained = retain_client_network_file(&source, &parent.join("client.toml"), &network)
            .unwrap()
            .unwrap();
        let before = private_client_snapshot(&fs::metadata(&identity).unwrap());
        let displaced = root.join("displaced");
        fs::rename(&parent, &displaced).unwrap();
        fs::set_permissions(&displaced, fs::Permissions::from_mode(0o755)).unwrap();
        fs::set_permissions(&displaced, fs::Permissions::from_mode(0o700)).unwrap();
        fs::rename(&displaced, &parent).unwrap();
        assert_eq!(
            before,
            private_client_snapshot(&fs::metadata(&identity).unwrap())
        );
        assert!(retained.check().is_err());
    }

    #[cfg(unix)]
    #[test]
    fn client_route_network_file_final_publication_drift_removes_only_owned_output() {
        use std::cell::Cell;
        let directory = operator_runtime_fixture();
        let root = directory.path().canonicalize().unwrap();
        let original = root.join("original");
        let projected = root.join("projected");
        for parent in [&original, &projected] {
            fs::create_dir(parent).unwrap();
            fs::set_permissions(parent, fs::Permissions::from_mode(0o700)).unwrap();
        }
        let network = network_fixture(b"final file publication identity");
        let identity = original.join("genesis.expected_hash");
        fs::write(&identity, format!("{network}\n")).unwrap();
        fs::set_permissions(&identity, fs::Permissions::from_mode(0o600)).unwrap();
        let source = file_route_fixture(&network, "genesis.expected_hash");
        let retained = retain_client_network_file(&source, &original.join("client.toml"), &network)
            .unwrap()
            .unwrap();
        let output = projected.join("client.toml");
        let calls = Cell::new(0);
        let check = || {
            calls.set(calls.get() + 1);
            if calls.get() == 3 {
                fs::write(&identity, vec![b'x'; 75])?;
            }
            retained.check()
        };
        assert!(
            write_projected_client_private(&output, b"opaque projected unit fixture", check)
                .is_err()
        );
        assert_eq!(calls.get(), 3);
        assert!(!output.exists());
    }

    #[cfg(unix)]
    #[test]
    fn client_route_network_file_private_output_ancestor_refuses_before_publication() {
        for nested_identity in [false, true] {
            let directory = operator_runtime_fixture();
            let root = directory.path().canonicalize().unwrap();
            let original = root.join("original");
            let projected = root.join("projected");
            for parent in [&original, &projected] {
                fs::create_dir(parent).unwrap();
                fs::set_permissions(parent, fs::Permissions::from_mode(0o700)).unwrap();
            }
            let identity_parent = if nested_identity {
                let parent = projected.join("network");
                fs::create_dir(&parent).unwrap();
                fs::set_permissions(&parent, fs::Permissions::from_mode(0o700)).unwrap();
                parent
            } else {
                projected.clone()
            };
            let network = network_fixture(b"separate identity-file output ancestor");
            let identity = identity_parent.join("genesis.expected_hash");
            let identity_body = format!("{network}\n");
            fs::write(&identity, &identity_body).unwrap();
            fs::set_permissions(&identity, fs::Permissions::from_mode(0o600)).unwrap();
            let source = file_route_fixture(&network, identity.to_str().unwrap());
            let source_path = original.join("client.toml");
            fs::write(&source_path, &source).unwrap();
            fs::set_permissions(&source_path, fs::Permissions::from_mode(0o600)).unwrap();
            let paths = [
                &original,
                &source_path,
                &identity,
                &identity_parent,
                &projected,
            ];
            let before =
                paths.map(|path| private_client_snapshot(&fs::symlink_metadata(path).unwrap()));
            let fd = File::open(&source_path).unwrap();
            let output = projected.join("client.toml");
            let command = super::super::PublicReset {
                command: super::super::PublicResetCommand::ClientConfigRebase(file_route_args(
                    &fd,
                    &source_path,
                    &output,
                    network,
                )),
            };
            let mut stdout = Vec::new();
            let error = command.run_without_client_config(&mut stdout).unwrap_err();
            assert!(format!("{error:#}").contains("separate private output directory"));
            assert!(stdout.is_empty());
            assert!(!output.exists());
            for (path, snapshot) in paths.into_iter().zip(before) {
                assert_eq!(
                    snapshot,
                    private_client_snapshot(&fs::symlink_metadata(path).unwrap())
                );
            }
            assert_eq!(fs::read(&source_path).unwrap(), source);
            assert_eq!(fs::read(&identity).unwrap(), identity_body.as_bytes());
        }
    }

    #[cfg(unix)]
    #[test]
    fn client_route_network_file_private_selected_path_never_enters_errors() {
        const MARKER: &str = "fixture-private-path-marker-not-runtime";
        for case in ["missing", "unsafe", "noncanonical"] {
            let directory = operator_runtime_fixture();
            let root = directory.path().canonicalize().unwrap();
            let original = root.join("original");
            let projected = root.join("projected");
            for parent in [&original, &projected] {
                fs::create_dir(parent).unwrap();
                fs::set_permissions(parent, fs::Permissions::from_mode(0o700)).unwrap();
            }
            let selected_parent = root.join(MARKER);
            if case == "unsafe" {
                fs::create_dir(&selected_parent).unwrap();
                fs::set_permissions(&selected_parent, fs::Permissions::from_mode(0o777)).unwrap();
            }
            let selected = if case == "noncanonical" {
                selected_parent.join("..").join("genesis.expected_hash")
            } else {
                selected_parent.join("genesis.expected_hash")
            };
            let network = network_fixture(b"private path stays native");
            let source = file_route_fixture(&network, selected.to_str().unwrap());
            let source_path = original.join("client.toml");
            fs::write(&source_path, &source).unwrap();
            fs::set_permissions(&source_path, fs::Permissions::from_mode(0o600)).unwrap();
            let before = [&original, &source_path, &projected]
                .map(|path| private_client_snapshot(&fs::symlink_metadata(path).unwrap()));
            let fd = File::open(&source_path).unwrap();
            let output = projected.join("client.toml");
            let command = super::super::PublicReset {
                command: super::super::PublicResetCommand::ClientConfigRebase(file_route_args(
                    &fd,
                    &source_path,
                    &output,
                    network,
                )),
            };
            let mut stdout = Vec::new();
            let error = command.run_without_client_config(&mut stdout).unwrap_err();
            let diagnostic = format!("{error:#}");
            assert!(diagnostic.contains("NetworkId"));
            assert!(!diagnostic.contains(MARKER));
            assert!(stdout.is_empty());
            assert!(!output.exists());
            for (path, snapshot) in [&original, &source_path, &projected]
                .into_iter()
                .zip(before)
            {
                assert_eq!(
                    snapshot,
                    private_client_snapshot(&fs::symlink_metadata(path).unwrap())
                );
            }
            assert_eq!(fs::read(&source_path).unwrap(), source);
        }
    }

    #[test]
    fn client_route_roundtrip_preserves_account_network_and_signing_identity() {
        let network = network_fixture(b"route unchanged network");
        let source_path = Path::new("/private/runtime/original/client.toml");
        let output_path = Path::new("/private/runtime/projected/client.toml");
        let source = route_fixture(&network);
        let projected = rebase_client_network_id(
            &source,
            &network,
            &network,
            Some(route_projection(source_path, output_path)),
        )
        .unwrap();
        let (before, _) =
            iroha::config::Config::load_bytes_with_musubi_publication(source_path, &source)
                .unwrap();
        let (after, _) =
            iroha::config::Config::load_bytes_with_musubi_publication(output_path, &projected)
                .unwrap();
        assert!(before.chain == after.chain && before.network_id == after.network_id);
        assert!(
            before.account == after.account
                && before.account_chain_discriminant == after.account_chain_discriminant
        );
        assert!(before.key_pair.public_key() == after.key_pair.public_key());
        assert_eq!(after.torii_api_url.as_str(), "http://127.0.0.1:18080/");
        let mut original: toml::Table =
            toml::from_str(std::str::from_utf8(&source).unwrap()).unwrap();
        let mut actual: toml::Table =
            toml::from_str(std::str::from_utf8(&projected).unwrap()).unwrap();
        // Boolean assertions do not dump either secret-bearing table on failure.
        assert!(original["account"]["private_key"] == actual["account"]["private_key"]);
        original.remove("torii_url");
        actual.remove("torii_url");
        assert!(original == actual);
    }

    #[test]
    fn client_route_refuses_wrong_route_identity_or_unbound_sources_without_secret_errors() {
        let network = network_fixture(b"route unchanged network");
        let other = network_fixture(b"unapproved changed network");
        let source = route_fixture(&network);
        let original = Path::new("/private/runtime/original/client.toml");
        let output = Path::new("/private/runtime/projected/client.toml");
        let mut route = route_projection(original, output);
        route.expected = "http://127.0.0.1:8081";
        assert!(rebase_client_network_id(&source, &network, &network, Some(route)).is_err());
        route.expected = "http://127.0.0.1:8080";
        assert!(rebase_client_network_id(&source, &network, &other, Some(route)).is_err());
        assert!(rebase_client_network_id(&source, &other, &other, Some(route)).is_err());
        for invalid in [
            [
                b"extends = '/unbound/client.toml'\n".as_slice(),
                source.as_slice(),
            ]
            .concat(),
            [
                b"network_id_file = '/unbound/network-id'\n".as_slice(),
                source.as_slice(),
            ]
            .concat(),
            b"private_key = 'fixture-secret-not-runtime\n".to_vec(),
            b"network_id = 'fixture-secret-not-runtime'\n".to_vec(),
        ] {
            let error =
                rebase_client_network_id(&invalid, &network, &network, Some(route)).unwrap_err();
            assert!(!format!("{error:#}").contains("fixture-secret"));
        }
    }

    #[test]
    fn client_route_preserves_relative_key_proof_and_filesystem_source_semantics() {
        let network = network_fixture(b"relative route unchanged network");
        let original = Path::new("/private/runtime/original/client.toml");
        let output = Path::new("/private/runtime/projected/client.toml");
        let mut table: toml::Table =
            toml::from_str(std::str::from_utf8(&route_fixture(&network)).unwrap()).unwrap();
        let account = table.get_mut("account").unwrap().as_table_mut().unwrap();
        account.remove("private_key");
        account.insert(
            "private_key_file".into(),
            toml::Value::String("keys/account.key".into()),
        );
        table.insert(
            "connect".into(),
            toml::Value::Table(toml::Table::from_iter([(
                "queue_root".into(),
                toml::Value::String("../queues".into()),
            )])),
        );
        table.insert(
            "soracloud".into(),
            toml::Value::Table(toml::Table::from_iter([(
                "http_witness_file".into(),
                toml::Value::String("proofs/witness.json".into()),
            )])),
        );
        table.insert(
            "musubi".into(),
            toml::Value::Table(toml::toml! {
                [publication]
                namespace_delegation_file = "proofs/delegation.json"
            }),
        );
        let source = Zeroizing::new(toml::to_string(&table).unwrap());
        let projected = rebase_client_network_id(
            source.as_bytes(),
            &network,
            &network,
            Some(route_projection(original, output)),
        )
        .unwrap();
        let actual: toml::Table = toml::from_str(std::str::from_utf8(&projected).unwrap()).unwrap();
        let parent = original.parent().unwrap();
        assert_eq!(
            actual["account"]["private_key_file"].as_str(),
            parent.join("keys/account.key").to_str()
        );
        assert_eq!(
            actual["connect"]["queue_root"].as_str(),
            parent.join("../queues").to_str()
        );
        assert_eq!(
            actual["soracloud"]["http_witness_file"].as_str(),
            parent.join("proofs/witness.json").to_str()
        );
        assert_eq!(
            actual["musubi"]["publication"]["namespace_delegation_file"].as_str(),
            parent.join("proofs/delegation.json").to_str()
        );
        let same_parent = Path::new("/private/runtime/original/projected.toml");
        let same = rebase_client_network_id(
            source.as_bytes(),
            &network,
            &network,
            Some(route_projection(original, same_parent)),
        )
        .unwrap();
        let same: toml::Table = toml::from_str(std::str::from_utf8(&same).unwrap()).unwrap();
        assert!(same["account"] == table["account"] && same["musubi"] == table["musubi"]);
    }

    #[test]
    fn client_route_relative_public_proof_refuses_whitespace_instead_of_readmitting_it() {
        let network = network_fixture(b"route public proof validation");
        let original = Path::new("/private/runtime/original/client.toml");
        let output = Path::new("/private/runtime/projected/client.toml");
        for proof in [
            " delegation.json",
            "delegation.json ",
            "delegation.json\n",
            "",
        ] {
            let mut table: toml::Table =
                toml::from_str(std::str::from_utf8(&route_fixture(&network)).unwrap()).unwrap();
            table.insert(
                "musubi".into(),
                toml::Value::Table(toml::Table::from_iter([(
                    "publication".into(),
                    toml::Value::Table(toml::Table::from_iter([(
                        "namespace_delegation_file".into(),
                        toml::Value::String(proof.into()),
                    )])),
                )])),
            );
            let source = Zeroizing::new(toml::to_string(&table).unwrap());
            assert!(
                rebase_client_network_id(
                    source.as_bytes(),
                    &network,
                    &network,
                    Some(route_projection(original, output))
                )
                .is_err()
            );
        }
    }

    #[cfg(unix)]
    #[test]
    fn client_route_native_fd_public_tls_retains_offset_source_and_public_proof() {
        use std::os::fd::AsRawFd as _;
        let directory = operator_runtime_fixture();
        let root = directory.path().canonicalize().unwrap();
        let original_dir = root.join("original");
        let projected_dir = root.join("projected");
        for parent in [&original_dir, &projected_dir] {
            fs::create_dir(parent).unwrap();
            fs::set_permissions(parent, fs::Permissions::from_mode(0o700)).unwrap();
        }
        let public_proof = original_dir.join("delegation.json");
        fs::write(&public_proof, b"{\"public_fixture\":true}\n").unwrap();
        let network = network_fixture(b"native route unchanged network");
        let mut source = route_fixture(&network);
        source.extend_from_slice(
            b"\n[musubi.publication]\nnamespace_delegation_file = 'delegation.json'\n",
        );
        let source_path = original_dir.join("client.toml");
        fs::write(&source_path, &source).unwrap();
        fs::set_permissions(&source_path, fs::Permissions::from_mode(0o600)).unwrap();
        let mut fd = File::open(&source_path).unwrap();
        fd.seek(std::io::SeekFrom::Start(7)).unwrap();
        let output_path = projected_dir.join("client.toml");
        let command = super::super::PublicReset {
            command: super::super::PublicResetCommand::ClientConfigRebase(ClientConfigRebase {
                config_fd: fd.as_raw_fd() as u32,
                expected_network_id: network,
                network_id: network,
                expected_torii_url: Some("http://127.0.0.1:8080".into()),
                torii_url: Some("https://taira.sora.org:8443/".into()),
                config_source_path: Some(source_path.clone()),
                output: output_path.clone(),
            }),
        };
        let mut stdout = Vec::new();
        command.run_without_client_config(&mut stdout).unwrap();
        assert!(stdout.is_empty());
        assert_eq!(fd.stream_position().unwrap(), 7);
        assert!(fs::read(&source_path).unwrap() == source);
        let projected = Zeroizing::new(fs::read(&output_path).unwrap());
        let (before, _) =
            iroha::config::Config::load_bytes_with_musubi_publication(&source_path, &source)
                .unwrap();
        let (after, publication) =
            iroha::config::Config::load_bytes_with_musubi_publication(&output_path, &projected)
                .unwrap();
        assert!(
            before.account == after.account
                && before.chain == after.chain
                && before.network_id == after.network_id
        );
        assert!(before.key_pair.public_key() == after.key_pair.public_key());
        assert_eq!(after.torii_api_url.as_str(), "https://taira.sora.org:8443/");
        let retained_proof = Path::new(publication.namespace_delegation_file.as_deref().unwrap());
        assert_eq!(retained_proof, public_proof);
        assert_eq!(
            fs::metadata(retained_proof).unwrap().ino(),
            fs::metadata(&public_proof).unwrap().ino()
        );
        let published = fs::symlink_metadata(&output_path).unwrap();
        assert_eq!(published.mode() & 0o7777, 0o600);
        assert_eq!(published.nlink(), 1);
        assert!(command.run_without_client_config(&mut stdout).is_err());
        assert!(stdout.is_empty());
        assert!(fs::read(&output_path).unwrap() == projected.as_slice());
    }

    #[cfg(unix)]
    #[test]
    fn client_route_native_refuses_wrong_provenance_and_unsafe_or_existing_output() {
        use std::os::fd::AsRawFd as _;
        let directory = operator_runtime_fixture();
        let root = directory.path().canonicalize().unwrap();
        let network = network_fixture(b"route custody unchanged network");
        let source = root.join("source.toml");
        let identical = root.join("identical.toml");
        for path in [&source, &identical] {
            fs::write(path, route_fixture(&network)).unwrap();
            fs::set_permissions(path, fs::Permissions::from_mode(0o600)).unwrap();
        }
        let projected_dir = root.join("projected");
        fs::create_dir(&projected_dir).unwrap();
        fs::set_permissions(&projected_dir, fs::Permissions::from_mode(0o700)).unwrap();
        let fd = File::open(&source).unwrap();
        let output = projected_dir.join("projected.toml");
        let mut args = ClientConfigRebase {
            config_fd: fd.as_raw_fd() as u32,
            expected_network_id: network,
            network_id: network,
            expected_torii_url: Some("http://127.0.0.1:8080".into()),
            torii_url: Some("http://127.0.0.1:18080".into()),
            config_source_path: Some(identical),
            output: output.clone(),
        };
        assert!(client_config_rebase(&args).is_err());
        assert!(!output.exists());
        args.config_source_path = Some(source.clone());
        fs::write(&output, b"existing fixture must remain").unwrap();
        assert!(client_config_rebase(&args).is_err());
        assert_eq!(fs::read(&output).unwrap(), b"existing fixture must remain");
        fs::remove_file(&output).unwrap();
        std::os::unix::fs::symlink(&source, &output).unwrap();
        assert!(client_config_rebase(&args).is_err());
        assert!(
            fs::symlink_metadata(&output)
                .unwrap()
                .file_type()
                .is_symlink()
        );
        fs::remove_file(&output).unwrap();
        let unsafe_parent = root.join("unsafe");
        fs::create_dir(&unsafe_parent).unwrap();
        fs::set_permissions(&unsafe_parent, fs::Permissions::from_mode(0o755)).unwrap();
        args.output = unsafe_parent.join("projected.toml");
        assert!(client_config_rebase(&args).is_err());
        assert!(!args.output.exists());
        let repository = root.join("repository");
        fs::create_dir(&repository).unwrap();
        fs::set_permissions(&repository, fs::Permissions::from_mode(0o700)).unwrap();
        fs::create_dir(repository.join(".git")).unwrap();
        args.output = repository.join("projected.toml");
        assert!(client_config_rebase(&args).is_err());
        assert!(!args.output.exists());
        fs::hard_link(&source, root.join("source-alias.toml")).unwrap();
        args.output = output;
        assert!(client_config_rebase(&args).is_err());
        assert!(!args.output.exists());
    }

    #[cfg(unix)]
    #[test]
    fn client_route_final_source_check_refusal_removes_only_owned_fresh_output() {
        use std::cell::Cell;
        let directory = operator_runtime_fixture();
        let root = directory.path().canonicalize().unwrap();
        let output = root.join("projected.toml");
        let calls = Cell::new(0);
        let check = || {
            calls.set(calls.get() + 1);
            if calls.get() == 3 {
                Err(eyre!("injected final source refusal"))
            } else {
                Ok(())
            }
        };
        assert!(write_projected_client_private(&output, b"opaque fixture only", check).is_err());
        assert_eq!(calls.get(), 3);
        assert!(!output.exists());
        assert!(!output.is_symlink());
        fs::write(&output, b"preexisting fixture").unwrap();
        assert!(
            write_projected_client_private(&output, b"replacement fixture", || Ok(())).is_err()
        );
        assert_eq!(fs::read(&output).unwrap(), b"preexisting fixture");
    }

    #[cfg(unix)]
    #[test]
    fn client_route_retained_source_refuses_restored_private_grandparent() {
        let directory = operator_runtime_fixture();
        let root = directory.path().canonicalize().unwrap();
        let grandparent = root.join("original");
        let parent = grandparent.join("network");
        fs::create_dir(&grandparent).unwrap();
        fs::create_dir(&parent).unwrap();
        for path in [&grandparent, &parent] {
            fs::set_permissions(path, fs::Permissions::from_mode(0o700)).unwrap();
        }
        let source = parent.join("client.toml");
        fs::write(&source, b"opaque fixture only").unwrap();
        fs::set_permissions(&source, fs::Permissions::from_mode(0o600)).unwrap();
        let leaf_before = fs::symlink_metadata(&source).unwrap();
        let retained = retain_client_source_directories(&source).unwrap();
        let before = fs::symlink_metadata(&grandparent).unwrap();
        let displaced = root.join("displaced");
        fs::rename(&grandparent, &displaced).unwrap();
        fs::set_permissions(&displaced, fs::Permissions::from_mode(0o755)).unwrap();
        fs::set_permissions(&displaced, fs::Permissions::from_mode(0o700)).unwrap();
        fs::rename(&displaced, &grandparent).unwrap();
        let restored = fs::symlink_metadata(&grandparent).unwrap();
        assert_eq!(
            client_directory_core(&before),
            client_directory_core(&restored)
        );
        assert_ne!(
            private_client_snapshot(&before),
            private_client_snapshot(&restored)
        );
        assert_eq!(
            private_client_snapshot(&leaf_before),
            private_client_snapshot(&fs::symlink_metadata(&source).unwrap())
        );
        assert!(check_client_source_directories(&retained).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn client_route_native_refuses_same_parent_to_keep_source_ancestors_exact() {
        use std::os::fd::AsRawFd as _;
        let directory = operator_runtime_fixture();
        let root = directory.path().canonicalize().unwrap();
        let network = network_fixture(b"route same parent unchanged network");
        let source = root.join("client.toml");
        fs::write(&source, route_fixture(&network)).unwrap();
        fs::set_permissions(&source, fs::Permissions::from_mode(0o600)).unwrap();
        let fd = File::open(&source).unwrap();
        let output = root.join("projected.toml");
        let parent_before = fs::symlink_metadata(&root).unwrap();
        let args = ClientConfigRebase {
            config_fd: fd.as_raw_fd() as u32,
            expected_network_id: network,
            network_id: network,
            expected_torii_url: Some("http://127.0.0.1:8080".into()),
            torii_url: Some("http://127.0.0.1:18080".into()),
            config_source_path: Some(source),
            output: output.clone(),
        };
        let error = client_config_rebase(&args).unwrap_err();
        assert!(format!("{error:#}").contains("separate private output directory"));
        assert!(!output.exists());
        assert_eq!(
            private_client_snapshot(&parent_before),
            private_client_snapshot(&fs::symlink_metadata(&root).unwrap())
        );
    }

    #[cfg(unix)]
    #[test]
    fn client_route_final_refusal_leaves_unrelated_output_replacement_untouched() {
        use std::cell::Cell;
        let directory = operator_runtime_fixture();
        let root = directory.path().canonicalize().unwrap();
        let output = root.join("projected.toml");
        let calls = Cell::new(0);
        let check = || {
            calls.set(calls.get() + 1);
            if calls.get() == 3 {
                fs::remove_file(&output).unwrap();
                fs::write(&output, b"unrelated owner fixture must remain").unwrap();
                fs::set_permissions(&output, fs::Permissions::from_mode(0o600)).unwrap();
                Err(eyre!("injected final source refusal after replacement"))
            } else {
                Ok(())
            }
        };
        assert!(write_projected_client_private(&output, b"owned opaque fixture", check).is_err());
        assert_eq!(calls.get(), 3);
        assert_eq!(
            fs::read(&output).unwrap(),
            b"unrelated owner fixture must remain"
        );
    }

    #[test]
    fn config_rebase_network_identity_uses_exact_cas_and_preserves_other_config() {
        let old = network_fixture(b"retained test genesis");
        let next = network_fixture(b"replacement test genesis");
        let source = validator_network_fixture(&old);
        let output = rebase_genesis_file(
            &source,
            Path::new("/retained/genesis.nrt"),
            Path::new("/installed/genesis.nrt"),
            Some((&old, &next)),
            None,
        )
        .unwrap();
        let mut expected: toml::Table =
            toml::from_str(std::str::from_utf8(&source).unwrap()).unwrap();
        expected
            .get_mut("genesis")
            .unwrap()
            .as_table_mut()
            .unwrap()
            .insert(
                "file".to_owned(),
                toml::Value::String("/installed/genesis.nrt".to_owned()),
            );
        expected
            .get_mut("genesis")
            .unwrap()
            .as_table_mut()
            .unwrap()
            .insert(
                "expected_hash".to_owned(),
                toml::Value::String(next.to_string()),
            );
        let actual: toml::Table = toml::from_str(std::str::from_utf8(&output).unwrap()).unwrap();
        assert_eq!(actual, expected);
        assert!(
            rebase_genesis_file(
                &source,
                Path::new("/retained/genesis.nrt"),
                Path::new("/installed/genesis.nrt"),
                Some((&next, &old)),
                None
            )
            .is_err()
        );
        assert!(
            rebase_genesis_file(
                &output,
                Path::new("/installed/genesis.nrt"),
                Path::new("/other/genesis.nrt"),
                Some((&old, &next)),
                None
            )
            .is_err()
        );
    }

    #[test]
    fn config_rebase_network_identity_rejects_competing_and_noncanonical_sources() {
        let old = network_fixture(b"retained test genesis");
        let next = network_fixture(b"replacement test genesis");
        let source = String::from_utf8(validator_network_fixture(&old)).unwrap();
        for invalid in [
            format!("extends = '/unbound/fixture.toml'\n{source}"),
            source.replace(
                "[genesis]",
                "[genesis]\nexpected_hash_file = '/unbound/network-id'",
            ),
            source.replace(&format!("expected_hash = '{old}'"), ""),
            source.replace(&old.to_string(), "fixture-secret-not-runtime"),
            source.replace(&old.to_string(), &format!(" {old}")),
        ] {
            let error = rebase_genesis_file(
                invalid.as_bytes(),
                Path::new("/retained/genesis.nrt"),
                Path::new("/installed/genesis.nrt"),
                Some((&old, &next)),
                None,
            )
            .unwrap_err();
            assert!(!format!("{error:#}").contains("fixture-secret"));
        }
    }

    #[test]
    fn config_rebase_network_identity_cli_requires_paired_checked_values() {
        use clap::Parser as _;
        let old = network_fixture(b"retained test genesis").to_string();
        let next = network_fixture(b"replacement test genesis").to_string();
        let base = [
            "iroha",
            "taira",
            "public-reset",
            "config-rebase",
            "--config-fd",
            "3",
            "--expected-genesis-file",
            "/retained/genesis.nrt",
            "--genesis-file",
            "/installed/genesis.nrt",
            "--output",
            "/private/runtime/validator.toml",
        ];
        assert!(crate::Args::try_parse_from(base).is_ok());
        let mut paired = base.to_vec();
        paired.extend(["--expected-network-id", &old, "--network-id", &next]);
        assert!(crate::Args::try_parse_from(paired).is_ok());
        for tail in [
            vec!["--expected-network-id", old.as_str()],
            vec!["--network-id", next.as_str()],
            vec![
                "--expected-network-id",
                old.as_str(),
                "--network-id",
                "raw-unchecked-hash",
            ],
        ] {
            let mut args = base.to_vec();
            args.extend(tail);
            assert!(crate::Args::try_parse_from(args).is_err());
        }
        for invalid in [
            format!(" {old}"),
            format!("{old}\n"),
            "raw-unchecked-hash".to_owned(),
        ] {
            assert!(canonical_network_id(&invalid).is_err());
        }
        assert!(
            crate::Args::try_parse_from([
                "iroha",
                "taira",
                "public-reset",
                "client-config-rebase",
                "--config-fd",
                "3",
                "--expected-network-id",
                &old,
                "--network-id",
                &next,
                "--output",
                "/private/runtime/client.toml"
            ])
            .is_ok()
        );
    }

    #[test]
    fn client_config_rebase_network_identity_uses_exact_cas_and_preserves_other_config() {
        let old = network_fixture(b"retained test genesis");
        let next = network_fixture(b"replacement test genesis");
        let source = client_network_fixture(&old);
        let output = rebase_client_network_id(&source, &old, &next, None).unwrap();
        let mut expected: toml::Table =
            toml::from_str(std::str::from_utf8(&source).unwrap()).unwrap();
        expected.insert(
            "network_id".to_owned(),
            toml::Value::String(next.to_string()),
        );
        let actual: toml::Table = toml::from_str(std::str::from_utf8(&output).unwrap()).unwrap();
        assert_eq!(actual, expected);
        assert!(rebase_client_network_id(&source, &next, &old, None).is_err());
        assert!(rebase_client_network_id(&output, &old, &next, None).is_err());
        for invalid in [
            [
                b"extends = '/unbound/config.toml'\n".as_slice(),
                source.as_slice(),
            ]
            .concat(),
            [
                b"network_id_file = '/unbound/network-id'\n".as_slice(),
                source.as_slice(),
            ]
            .concat(),
            b"network_id = 'fixture-secret-not-runtime'\n".to_vec(),
            b"private_key = 'fixture-secret-not-runtime'\n".to_vec(),
            b"private_key = 'fixture-secret-not-runtime\n".to_vec(),
        ] {
            let error = rebase_client_network_id(&invalid, &old, &next, None).unwrap_err();
            assert!(!format!("{error:#}").contains("fixture-secret"));
        }
    }

    #[cfg(unix)]
    #[test]
    fn client_config_rebase_inherited_fd_preserves_custody_and_has_no_stdout() {
        use std::os::fd::AsRawFd as _;
        let directory = private_custody_test_dir("client-config-rebase-");
        let root = directory.path().canonicalize().unwrap();
        let old = network_fixture(b"retained test genesis");
        let next = network_fixture(b"replacement test genesis");
        let source = root.join("source.toml");
        let bytes = client_network_fixture(&old);
        fs::write(&source, &bytes).unwrap();
        fs::set_permissions(&source, fs::Permissions::from_mode(0o600)).unwrap();
        let mut file = File::open(&source).unwrap();
        file.seek(std::io::SeekFrom::Start(7)).unwrap();
        let output = root.join("rebound.toml");
        let command = super::super::PublicReset {
            command: super::super::PublicResetCommand::ClientConfigRebase(ClientConfigRebase {
                config_fd: file.as_raw_fd() as u32,
                expected_network_id: old,
                network_id: next,
                expected_torii_url: None,
                torii_url: None,
                config_source_path: None,
                output: output.clone(),
            }),
        };
        let mut stdout = Vec::new();
        command.run_without_client_config(&mut stdout).unwrap();
        assert!(stdout.is_empty());
        assert_eq!(file.stream_position().unwrap(), 7);
        assert_eq!(fs::read(&source).unwrap(), bytes);
        let published = fs::read(&output).unwrap();
        let table: toml::Table = toml::from_str(std::str::from_utf8(&published).unwrap()).unwrap();
        assert_eq!(
            table["network_id"].as_str(),
            Some(next.to_string().as_str())
        );
        let metadata = fs::symlink_metadata(&output).unwrap();
        assert_eq!(metadata.mode() & 0o7777, 0o600);
        assert_eq!(metadata.nlink(), 1);
        assert_eq!(metadata.uid(), rustix::process::geteuid().as_raw());
        assert!(command.run_without_client_config(&mut stdout).is_err());
        assert_eq!(fs::read(&output).unwrap(), published);
        assert!(stdout.is_empty());
    }

    #[cfg(unix)]
    #[test]
    fn client_config_rebase_rejects_drift_and_unsafe_custody_before_output() {
        use std::os::fd::AsRawFd as _;
        let directory = private_custody_test_dir("client-config-rebase-refusal-");
        let root = directory.path().canonicalize().unwrap();
        let old = network_fixture(b"retained test genesis");
        let next = network_fixture(b"replacement test genesis");
        let source = root.join("source.toml");
        fs::write(&source, client_network_fixture(&old)).unwrap();
        fs::set_permissions(&source, fs::Permissions::from_mode(0o600)).unwrap();
        let file = File::open(&source).unwrap();
        let output = root.join("rebound.toml");
        let mut args = ClientConfigRebase {
            config_fd: file.as_raw_fd() as u32,
            expected_network_id: next,
            network_id: old,
            expected_torii_url: None,
            torii_url: None,
            config_source_path: None,
            output: output.clone(),
        };
        assert!(client_config_rebase(&args).is_err());
        assert!(!output.exists());
        args.expected_network_id = old;
        args.network_id = next;
        fs::set_permissions(&source, fs::Permissions::from_mode(0o644)).unwrap();
        assert!(client_config_rebase(&args).is_err());
        fs::set_permissions(&source, fs::Permissions::from_mode(0o600)).unwrap();
        fs::hard_link(&source, root.join("linked.toml")).unwrap();
        assert!(client_config_rebase(&args).is_err());
        assert!(!output.exists());
        fs::remove_file(root.join("linked.toml")).unwrap();
        let unsafe_parent = root.join("unsafe");
        fs::create_dir(&unsafe_parent).unwrap();
        fs::set_permissions(&unsafe_parent, fs::Permissions::from_mode(0o755)).unwrap();
        args.output = unsafe_parent.join("output.toml");
        assert!(client_config_rebase(&args).is_err());
        assert!(!args.output.exists());
        std::os::unix::fs::symlink(&source, &output).unwrap();
        args.output = output;
        assert!(client_config_rebase(&args).is_err());
        assert_eq!(fs::read(&source).unwrap(), client_network_fixture(&old));
    }

    #[test]
    fn config_rebase_operator_key_is_canonical_and_changes_only_explicit_operator_fields() {
        let key = iroha_crypto::KeyPair::try_from_seed(vec![0x53; 32], Algorithm::Ed25519).unwrap();
        let canonical = key.public_key().to_string();
        assert_eq!(
            canonical_operator_public_key(&canonical).unwrap(),
            *key.public_key()
        );
        for invalid in [
            format!(" {canonical}"),
            format!("{canonical}\n"),
            "private-fixture".into(),
        ] {
            let error = canonical_operator_public_key(&invalid).unwrap_err();
            assert!(!error.contains(&invalid));
        }
        let source = [
            FIXTURE,
            b"[torii]\naddress = '127.0.0.1:8080'\n[torii.operator_signatures]\nenabled = false\nallowed_public_keys = ['retired-public-fixture']\nallow_node_key = false\nnonce_ttl_secs = 120\n",
        ]
        .concat();
        for source in [FIXTURE, source.as_slice()] {
            let output = rebase_genesis_file(
                source,
                Path::new("/retained/genesis.nrt"),
                Path::new("/installed/genesis.nrt"),
                None,
                Some(key.public_key()),
            )
            .unwrap();
            let mut actual: toml::Table =
                toml::from_str(std::str::from_utf8(&output).unwrap()).unwrap();
            let mut original: toml::Table =
                toml::from_str(std::str::from_utf8(source).unwrap()).unwrap();
            let operators = actual
                .get_mut("torii")
                .unwrap()
                .as_table_mut()
                .unwrap()
                .get_mut("operator_signatures")
                .unwrap()
                .as_table_mut()
                .unwrap();
            assert_eq!(
                operators.remove("enabled"),
                Some(toml::Value::Boolean(true))
            );
            assert_eq!(
                operators.remove("allowed_public_keys"),
                Some(toml::Value::Array(vec![toml::Value::String(
                    canonical.clone()
                )]))
            );
            if let Some(torii) = original.get_mut("torii") {
                let operators = torii
                    .as_table_mut()
                    .unwrap()
                    .get_mut("operator_signatures")
                    .unwrap()
                    .as_table_mut()
                    .unwrap();
                operators.remove("enabled");
                operators.remove("allowed_public_keys");
            } else {
                actual.remove("torii");
            }
            actual
                .get_mut("genesis")
                .unwrap()
                .as_table_mut()
                .unwrap()
                .insert(
                    "file".into(),
                    toml::Value::String("/retained/genesis.nrt".into()),
                );
            assert_eq!(actual, original);
        }
        for malformed in [
            [
                b"torii = 'fixture-secret-not-runtime'\n".as_slice(),
                FIXTURE,
            ]
            .concat(),
            [
                FIXTURE,
                b"[torii]\noperator_signatures = 'fixture-secret-not-runtime'\n",
            ]
            .concat(),
        ] {
            let error = rebase_genesis_file(
                &malformed,
                Path::new("/retained/genesis.nrt"),
                Path::new("/installed/genesis.nrt"),
                None,
                Some(key.public_key()),
            )
            .unwrap_err();
            assert!(!format!("{error:#}").contains("fixture-secret"));
        }
    }

    #[cfg(unix)]
    fn operator_runtime_fixture() -> tempfile::TempDir {
        // Real key generation must pass the production repository exclusion, including in tests.
        let home = std::env::var_os("HOME").expect("native operator test home");
        let root = Path::new(&home).canonicalize().unwrap();
        let directory = tempfile::Builder::new()
            .prefix("iroha-operator-key-test-")
            .tempdir_in(root)
            .unwrap();
        fs::set_permissions(directory.path(), fs::Permissions::from_mode(0o700)).unwrap();
        directory
    }

    #[cfg(unix)]
    #[test]
    fn operator_keygen_publishes_canonical_private_key_and_only_public_report() {
        let directory = operator_runtime_fixture();
        let mut public_keys = Vec::new();
        for name in ["first.key", "second.key"] {
            let path = directory.path().join(name);
            let mut stdout = Vec::new();
            operator_keygen(
                &OperatorKeygen {
                    private_key_file: path.clone(),
                },
                &mut stdout,
            )
            .unwrap();
            let report: Value = json::from_slice(&stdout).unwrap();
            let object = report.as_object().unwrap();
            assert_eq!(object.len(), 3);
            assert_eq!(
                report.get("schema").and_then(Value::as_str),
                Some("iroha.taira.public-reset.operator-key.v1")
            );
            assert_eq!(report.get("path").and_then(Value::as_str), path.to_str());
            let loaded = crate::operator_key::load_operator_key_pair(&path).unwrap();
            assert_eq!(
                loaded.public_key().try_algorithm().unwrap(),
                Algorithm::Ed25519
            );
            assert_eq!(
                report.get("public_key").and_then(Value::as_str),
                Some(loaded.public_key().to_string().as_str())
            );
            let encoded = Zeroizing::new(fs::read(&path).unwrap());
            assert_eq!(encoded.last(), Some(&b'\n'));
            assert!(
                !stdout
                    .windows(encoded.len() - 1)
                    .any(|window| window == &encoded[..encoded.len() - 1])
            );
            let metadata = fs::symlink_metadata(&path).unwrap();
            assert!(metadata.is_file());
            assert_eq!(metadata.uid(), rustix::process::geteuid().as_raw());
            assert_eq!(metadata.mode() & 0o7777, 0o600);
            assert_eq!(metadata.nlink(), 1);
            public_keys.push(loaded.public_key().clone());
            let mut refused_stdout = Vec::new();
            assert!(
                operator_keygen(
                    &OperatorKeygen {
                        private_key_file: path.clone()
                    },
                    &mut refused_stdout
                )
                .is_err()
            );
            assert!(refused_stdout.is_empty());
            assert!(Zeroizing::new(fs::read(path).unwrap()).as_slice() == encoded.as_slice());
        }
        assert_ne!(public_keys[0], public_keys[1]);
    }

    #[cfg(unix)]
    #[test]
    fn operator_keygen_rejects_repository_existing_symlink_and_unsafe_parent_paths() {
        use std::os::unix::fs::symlink;
        let directory = operator_runtime_fixture();
        let root = directory.path();
        let existing = root.join("existing.key");
        fs::write(&existing, b"fixture-must-remain").unwrap();
        fs::set_permissions(&existing, fs::Permissions::from_mode(0o600)).unwrap();
        let link = root.join("link.key");
        symlink(&existing, &link).unwrap();
        let unsafe_parent = root.join("unsafe");
        fs::create_dir(&unsafe_parent).unwrap();
        fs::set_permissions(&unsafe_parent, fs::Permissions::from_mode(0o755)).unwrap();
        let indirect_parent = root.join("indirect");
        symlink(root, &indirect_parent).unwrap();
        let repository = root.join("repository");
        fs::create_dir(&repository).unwrap();
        fs::set_permissions(&repository, fs::Permissions::from_mode(0o700)).unwrap();
        fs::create_dir(repository.join(".git")).unwrap();
        for path in [
            PathBuf::from("relative.key"),
            existing.clone(),
            link,
            unsafe_parent.join("operator.key"),
            indirect_parent.join("operator.key"),
            repository.join("operator.key"),
        ] {
            let mut stdout = Vec::new();
            assert!(
                operator_keygen(
                    &OperatorKeygen {
                        private_key_file: path.clone()
                    },
                    &mut stdout
                )
                .is_err()
            );
            assert!(stdout.is_empty());
            if path != existing && !path.is_symlink() {
                assert!(!path.exists());
            }
        }
        assert_eq!(fs::read(existing).unwrap(), b"fixture-must-remain");
        // A worktree .git file is a repository boundary too, not only a .git directory.
        fs::write(root.join(".git"), b"gitdir: fixture-only").unwrap();
        let path = root.join("worktree.key");
        assert!(
            operator_keygen(
                &OperatorKeygen {
                    private_key_file: path.clone()
                },
                &mut Vec::new()
            )
            .is_err()
        );
        assert!(!path.exists());
    }

    #[test]
    fn config_rebase_changes_only_genesis_file_and_keeps_errors_secret_free() {
        let output = rebase_genesis_file(
            FIXTURE,
            Path::new("/retained/genesis.nrt"),
            Path::new("/installed/genesis.nrt"),
            None,
            None,
        )
        .unwrap();
        let mut expected: toml::Table =
            toml::from_str(std::str::from_utf8(FIXTURE).unwrap()).unwrap();
        expected
            .get_mut("genesis")
            .unwrap()
            .as_table_mut()
            .unwrap()
            .insert(
                "file".to_owned(),
                toml::Value::String("/installed/genesis.nrt".to_owned()),
            );
        let actual: toml::Table = toml::from_str(std::str::from_utf8(&output).unwrap()).unwrap();
        assert_eq!(actual, expected);
        for bytes in [
            FIXTURE.to_vec(),
            [b"extends='/unbound'\n".as_slice(), FIXTURE].concat(),
            b"private_key = 'fixture-secret-not-runtime\n".to_vec(),
        ] {
            let error = rebase_genesis_file(
                &bytes,
                Path::new("/wrong/path"),
                Path::new("/installed/genesis.nrt"),
                None,
                None,
            )
            .unwrap_err();
            assert!(!format!("{error:#}").contains("fixture-secret"));
            assert!(!format!("{error:#}").contains("transport-fixture"));
        }
    }

    #[cfg(unix)]
    #[test]
    fn config_rebase_inherited_fd_publishes_private_file_without_stdout_or_overwrite() {
        use std::os::fd::AsRawFd as _;
        let directory = private_custody_test_dir("config-rebase-");
        let root = directory.path().canonicalize().unwrap();
        fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
        let source = root.join("source.toml");
        fs::write(&source, FIXTURE).unwrap();
        fs::set_permissions(&source, fs::Permissions::from_mode(0o600)).unwrap();
        let file = File::open(&source).unwrap();
        let destination = root.join("materialized.toml");
        let key = iroha_crypto::KeyPair::try_from_seed(vec![0x54; 32], Algorithm::Ed25519).unwrap();
        let command = super::super::PublicReset {
            command: super::super::PublicResetCommand::ConfigRebase(ConfigRebase {
                config_fd: file.as_raw_fd() as u32,
                expected_genesis_file: PathBuf::from("/retained/genesis.nrt"),
                genesis_file: PathBuf::from("/installed/genesis.nrt"),
                expected_network_id: None,
                network_id: None,
                operator_public_key: Some(key.public_key().clone()),
                output: destination.clone(),
            }),
        };
        let mut stdout = Vec::new();
        command.run_without_client_config(&mut stdout).unwrap();
        assert!(stdout.is_empty());
        assert_eq!(destination.metadata().unwrap().mode() & 0o7777, 0o600);
        assert_eq!(fs::read(&source).unwrap(), FIXTURE);
        let before = fs::read(&destination).unwrap();
        let materialized: toml::Table =
            toml::from_str(std::str::from_utf8(&before).unwrap()).unwrap();
        let signatures = &materialized["torii"]["operator_signatures"];
        assert_eq!(signatures["enabled"].as_bool(), Some(true));
        assert_eq!(
            signatures["allowed_public_keys"].as_array().unwrap(),
            &vec![toml::Value::String(key.public_key().to_string())]
        );
        assert!(command.run_without_client_config(&mut stdout).is_err());
        assert_eq!(fs::read(destination).unwrap(), before);
        assert!(stdout.is_empty());
    }

    #[cfg(unix)]
    #[test]
    fn config_rebase_rejects_drift_malformed_input_and_unsafe_descriptors_before_output() {
        use std::os::fd::AsRawFd as _;
        let directory = private_custody_test_dir("config-rebase-refusal-");
        let source = directory.path().join("source.toml");
        let output = directory.path().join("output.toml");
        for bytes in [FIXTURE, b"private_key = 'fixture-secret-not-runtime\n"] {
            fs::write(&source, bytes).unwrap();
            fs::set_permissions(&source, fs::Permissions::from_mode(0o600)).unwrap();
            let file = File::open(&source).unwrap();
            let error = config_rebase(&ConfigRebase {
                config_fd: file.as_raw_fd() as u32,
                expected_genesis_file: PathBuf::from("/wrong/path"),
                genesis_file: PathBuf::from("/installed/genesis.nrt"),
                expected_network_id: None,
                network_id: None,
                operator_public_key: None,
                output: output.clone(),
            })
            .unwrap_err();
            assert!(!format!("{error:#}").contains("fixture-secret"));
            assert!(!output.exists());
        }
        fs::write(&source, FIXTURE).unwrap();
        fs::set_permissions(&source, fs::Permissions::from_mode(0o644)).unwrap();
        let file = File::open(&source).unwrap();
        assert!(inherited_config(file.as_raw_fd() as u32).is_err());
        fs::set_permissions(&source, fs::Permissions::from_mode(0o600)).unwrap();
        fs::hard_link(&source, directory.path().join("linked.toml")).unwrap();
        assert!(inherited_config(file.as_raw_fd() as u32).is_err());
        let directory_fd = File::open(directory.path()).unwrap();
        assert!(inherited_config(directory_fd.as_raw_fd() as u32).is_err());
        let (reader, _writer) = std::os::unix::net::UnixStream::pair().unwrap();
        assert!(inherited_config(reader.as_raw_fd() as u32).is_err());
        assert!(inherited_config(2).is_err());
        assert!(!output.exists());
    }

    #[cfg(unix)]
    #[test]
    fn config_rebase_preserves_caller_offset_and_rejects_writable_or_unlinked_descriptors() {
        use std::os::fd::AsRawFd as _;
        let directory = private_custody_test_dir("config-rebase-fd-");
        let path = directory.path().join("source.toml");
        fs::write(&path, FIXTURE).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
        let mut source = File::open(&path).unwrap();
        source.seek(std::io::SeekFrom::Start(9)).unwrap();
        rustix::io::fcntl_setfd(&source, rustix::io::FdFlags::empty()).unwrap();
        let fd = source.as_raw_fd() as u32;
        assert_eq!(inherited_config(fd).unwrap().as_slice(), FIXTURE);
        assert_eq!(source.stream_position().unwrap(), 9);
        assert!(
            rustix::io::fcntl_getfd(&source)
                .unwrap()
                .contains(rustix::io::FdFlags::CLOEXEC)
        );
        for read in [true, false] {
            let writable = fs::OpenOptions::new()
                .read(read)
                .write(true)
                .open(&path)
                .unwrap();
            let error = inherited_config(writable.as_raw_fd() as u32).unwrap_err();
            assert!(error.to_string().contains("read-only"));
        }
        fs::set_permissions(&path, fs::Permissions::from_mode(0o640)).unwrap();
        assert!(inherited_config(fd).is_err());
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
        fs::remove_file(&path).unwrap();
        assert!(inherited_config(fd).is_err());
    }
}
