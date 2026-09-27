//! Native private client bundle for one freshly generated four-validator Taira network.
//!
//! All signing material stays in the approved guest's owner-only runtime tree. The
//! generated public-input bundle is the independent authority for genesis and canary
//! identity; Kagami's private localnet files supply only the matching signer keys.

use super::*;
use iroha::data_model::NetworkId;
use iroha_crypto::{ExposedPrivateKey, KeyPair};
use zeroize::Zeroizing;

const RUNTIME_ROOT: &str = "/private/runtime/taira-public-reset";
const MAX_CLIENT_INPUT_BYTES: u64 = 1024 * 1024;
const MAX_KEY_BYTES: u64 = 128;

/// Produce one complete owner-private client bundle without contacting validators.
#[derive(clap::Args, Debug)]
pub(super) struct PrepareRuntimeClients {
    /// Exact new Kagami localnet directory on the approved guest.
    #[arg(long, value_name = "DIR")]
    localnet_dir: PathBuf,
    /// Complete native prepare-public-inputs bundle for that genesis.
    #[arg(long, value_name = "DIR")]
    public_inputs_dir: PathBuf,
    /// Independent canary public.key and private.key directory.
    #[arg(long, value_name = "DIR")]
    canary_key_dir: PathBuf,
    /// Fresh bundle directory under an existing owner-only runtime directory.
    #[arg(long, value_name = "DIR")]
    output_dir: PathBuf,
}

#[derive(JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct LaneManifestValidator {
    validator: String,
    peer_id: String,
}

#[derive(JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct LaneManifest {
    lane: String,
    governance: String,
    version: u32,
    validators: Vec<LaneManifestValidator>,
    quorum: u32,
}

#[derive(JsonSerialize)]
struct ClientBundleReceipt {
    schema: String,
    output_dir: String,
    validator_clients: u8,
    genesis_hash: String,
}

fn runtime_path(path: &Path, label: &str) -> Result<()> {
    validate_absolute_normal_path(path, label)?;
    if !path.starts_with(RUNTIME_ROOT) || path == Path::new(RUNTIME_ROOT) {
        return Err(eyre!(
            "{label} must be beneath the approved private runtime root"
        ));
    }
    Ok(())
}

fn private_bytes(path: &Path, maximum: u64, label: &str) -> Result<Zeroizing<Vec<u8>>> {
    let pinned = pin_owner_private_file(path, label)?;
    if pinned.snapshot.len == 0 {
        return Err(eyre!("{label} is empty"));
    }
    Ok(Zeroizing::new(read_pinned_bytes(
        path,
        label,
        pinned.file.try_clone()?,
        &pinned.snapshot,
        maximum,
    )?))
}

fn public_manifest(path: &Path) -> Result<LaneManifest> {
    let (file, snapshot) = open_pinned_regular(path, "generated lane manifest")?;
    #[cfg(unix)]
    if snapshot.uid != rustix::process::geteuid().as_raw()
        || !matches!(snapshot.mode & 0o7777, 0o600 | 0o644)
    {
        return Err(eyre!("generated lane manifest custody differs"));
    }
    let bytes = read_pinned_bytes(
        path,
        "generated lane manifest",
        file,
        &snapshot,
        MAX_CLIENT_INPUT_BYTES,
    )?;
    let manifest: LaneManifest = json::from_slice(&bytes)
        .map_err(|_| eyre!("generated lane manifest is not exact V1 JSON"))?;
    validate_manifest(&manifest)?;
    Ok(manifest)
}

fn validate_manifest(manifest: &LaneManifest) -> Result<()> {
    if manifest.lane != "is"
        || manifest.governance != "parliament"
        || manifest.version != 1
        || manifest.quorum != 3
        || manifest.validators.len() != VALIDATOR_SLUGS.len()
    {
        return Err(eyre!(
            "generated lane manifest is not the four-validator IS committee"
        ));
    }
    Ok(())
}

fn toml_table<'a>(root: &'a toml::Table, path: &[&str]) -> Result<&'a toml::Table> {
    let mut table = root;
    for segment in path {
        table = table
            .get(*segment)
            .and_then(toml::Value::as_table)
            .ok_or_else(|| eyre!("generated config omits a required table"))?;
    }
    Ok(table)
}

fn toml_text<'a>(table: &'a toml::Table, key: &str) -> Result<&'a str> {
    table
        .get(key)
        .and_then(toml::Value::as_str)
        .ok_or_else(|| eyre!("generated config omits a required string"))
}

fn parse_private_text(bytes: &[u8], label: &str) -> Result<Zeroizing<String>> {
    let text = std::str::from_utf8(bytes).map_err(|_| eyre!("{label} is not UTF-8"))?;
    let key = text.strip_suffix('\n').unwrap_or(text);
    if key.is_empty() || key.chars().any(char::is_whitespace) {
        return Err(eyre!("{label} is not one canonical key line"));
    }
    Ok(Zeroizing::new(key.to_owned()))
}

fn checked_key_pair(public: &str, private: &str, label: &str) -> Result<PublicKey> {
    let public_key: PublicKey = public
        .parse()
        .map_err(|_| eyre!("{label} public key is invalid"))?;
    if public_key.to_string() != public || public_key.try_algorithm()? != Algorithm::Ed25519 {
        return Err(eyre!("{label} public key must be canonical Ed25519"));
    }
    let private_key: ExposedPrivateKey = private
        .parse()
        .map_err(|_| eyre!("{label} private key is invalid"))?;
    if private_key.try_to_multihash_string()? != private {
        return Err(eyre!("{label} private key is not canonical"));
    }
    KeyPair::new(public_key.clone(), private_key.0)
        .map_err(|_| eyre!("{label} public and private keys differ"))?;
    Ok(public_key)
}

fn parse_base(bytes: &[u8]) -> Result<toml::Table> {
    let text =
        std::str::from_utf8(bytes).map_err(|_| eyre!("generated client config is not UTF-8"))?;
    let mut table: toml::Table =
        toml::from_str(text).map_err(|_| eyre!("generated client config is not TOML"))?;
    let result = (|| {
        if table.len() != 6
            || table.get("chain").and_then(toml::Value::as_str) != Some(CHAIN_ID)
            || table.get("network_id_file").and_then(toml::Value::as_str)
                != Some("genesis.expected_hash")
            || table.get("torii_url").and_then(toml::Value::as_str)
                != Some("http://127.0.0.1:8080/")
        {
            return Err(eyre!(
                "generated base client is not the exact selected localnet"
            ));
        }
        let tx = toml_table(&table, &["transaction"])?;
        if tx.len() != 3
            || tx.get("nonce").and_then(toml::Value::as_bool) != Some(false)
            || ["time_to_live_ms", "status_timeout_ms"]
                .iter()
                .any(|field| {
                    tx.get(*field)
                        .and_then(toml::Value::as_integer)
                        .is_none_or(|value| value <= 0)
                })
        {
            return Err(eyre!("generated base transaction policy differs"));
        }
        let account = toml_table(&table, &["account"])?;
        if account.len() != 4
            || account
                .get("chain_discriminant")
                .and_then(toml::Value::as_integer)
                != Some(i64::from(CHAIN_DISCRIMINANT))
            || toml_text(account, "domain")? != "wonderland.universal"
        {
            return Err(eyre!("generated base account context differs"));
        }
        let admin_public = toml_text(account, "public_key")?;
        let admin_private = toml_text(account, "private_key")?;
        checked_key_pair(admin_public, admin_private, "generated maintenance account")?;
        let auth = toml_table(&table, &["basic_auth"])?;
        if auth.len() != 2 || !auth.contains_key("web_login") || !auth.contains_key("password") {
            return Err(eyre!("generated basic auth shape differs"));
        }
        toml_text(auth, "web_login")?;
        toml_text(auth, "password")?;
        Ok(())
    })();
    if let Err(error) = result {
        crate::soracloud::zeroize_taira_toml_table(&mut table);
        return Err(error);
    }
    Ok(table)
}

fn render_client(
    base: &toml::Table,
    network: &NetworkId,
    origin: &str,
    public: &PublicKey,
    private: &str,
) -> Result<Zeroizing<Vec<u8>>> {
    let tx = toml_table(base, &["transaction"])?;
    let mut root = toml::Table::new();
    root.insert("chain".into(), toml::Value::String(CHAIN_ID.into()));
    root.insert(
        "network_id".into(),
        toml::Value::String(network.to_string()),
    );
    root.insert("torii_url".into(), toml::Value::String(origin.into()));
    let mut transaction = toml::Table::new();
    for field in ["time_to_live_ms", "status_timeout_ms", "nonce"] {
        transaction.insert(
            field.into(),
            tx.get(field)
                .ok_or_else(|| eyre!("generated transaction field absent"))?
                .clone(),
        );
    }
    root.insert("transaction".into(), toml::Value::Table(transaction));
    let mut account = toml::Table::new();
    account.insert(
        "domain".into(),
        toml::Value::String("wonderland.universal".into()),
    );
    account.insert(
        "chain_discriminant".into(),
        toml::Value::Integer(i64::from(CHAIN_DISCRIMINANT)),
    );
    account.insert("public_key".into(), toml::Value::String(public.to_string()));
    account.insert(
        "private_key".into(),
        toml::Value::String(private.to_owned()),
    );
    root.insert("account".into(), toml::Value::Table(account));
    if let Some(auth) = base.get("basic_auth") {
        root.insert("basic_auth".into(), auth.clone());
    }
    let rendered =
        toml::to_string(&root).map_err(|_| eyre!("cannot render private runtime client config"));
    crate::soracloud::zeroize_taira_toml_table(&mut root);
    let rendered = Zeroizing::new(rendered?);
    if rendered.len() as u64 > MAX_CLIENT_INPUT_BYTES {
        return Err(eyre!("runtime client config exceeds its byte bound"));
    }
    Ok(Zeroizing::new(rendered.as_bytes().to_vec()))
}

fn canonical_account(text: &str, public: &PublicKey, label: &str) -> Result<()> {
    let account =
        AccountId::parse_encoded(text).map_err(|_| eyre!("{label} account is not canonical"))?;
    if account.to_string() != text || account != AccountId::new(public.clone()) {
        return Err(eyre!("{label} account differs from its signer"));
    }
    Ok(())
}

fn peer_config(
    config: &[u8],
    row: &LaneManifestValidator,
    index: usize,
) -> Result<(PublicKey, FaucetPolicyV1)> {
    let text = std::str::from_utf8(config)
        .map_err(|_| eyre!("generated validator config is not UTF-8"))?;
    let mut table: toml::Table =
        toml::from_str(text).map_err(|_| eyre!("generated validator config is not TOML"))?;
    let result = (|| {
        if toml_text(&table, "chain")? != CHAIN_ID
            || table
                .get("chain_discriminant")
                .and_then(toml::Value::as_integer)
                != Some(i64::from(CHAIN_DISCRIMINANT))
        {
            return Err(eyre!("generated validator chain context differs"));
        }
        let peer_public: PublicKey = toml_text(&table, "public_key")?
            .parse()
            .map_err(|_| eyre!("generated peer public key is invalid"))?;
        let peer_id: PeerId = row
            .peer_id
            .parse()
            .map_err(|_| eyre!("generated manifest peer identity is invalid"))?;
        if peer_id.to_string() != row.peer_id
            || PeerId::from(peer_public).to_string() != row.peer_id
        {
            return Err(eyre!(
                "generated manifest peer identity differs from peer config"
            ));
        }
        let signer = toml_table(&table, &["soracloud_runtime", "submission", "signer"])?;
        let hex = toml_text(signer, "public_key_hex")?;
        if hex.len() != 64
            || !hex
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
        {
            return Err(eyre!("generated runtime signer hex is invalid"));
        }
        let public: PublicKey = format!("ed0120{}", hex.to_ascii_uppercase())
            .parse()
            .map_err(|_| eyre!("generated runtime signer public key is invalid"))?;
        canonical_account(
            toml_text(signer, "authority")?,
            &public,
            "generated validator",
        )?;
        if row.validator != toml_text(signer, "authority")? {
            return Err(eyre!("generated runtime signer differs from lane manifest"));
        }
        let faucet = toml_table(&table, &["torii", "faucet"])?;
        if faucet.get("enabled").and_then(toml::Value::as_bool) != Some(true) {
            return Err(eyre!("generated validator faucet is disabled"));
        }
        let amount_text = toml_text(faucet, "amount")?;
        if amount_text.is_empty() || !amount_text.bytes().all(|byte| byte.is_ascii_digit()) {
            return Err(eyre!("generated faucet amount is not a positive integer"));
        }
        let policy = FaucetPolicyV1 {
            authority: toml_text(faucet, "authority")?.to_owned(),
            asset_definition_id: toml_text(faucet, "asset_definition_id")?.to_owned(),
            amount: amount_text
                .parse()
                .map_err(|_| eyre!("generated faucet amount is invalid"))?,
        };
        validate_faucet_policy(&policy)?;
        if policy.authority == row.validator {
            return Err(eyre!("faucet authority must differ from validator signer"));
        }
        let expected_port = 8080_u16 + u16::try_from(index)?;
        let address = toml_table(&table, &["torii"])?;
        let address: iroha_primitives::addr::SocketAddr =
            json::from_slice(&json::to_vec(toml_text(address, "address")?)?)
                .map_err(|_| eyre!("generated validator Torii binding is not canonical"))?;
        if address != iroha_primitives::addr::SocketAddr::from(([127, 0, 0, 1], expected_port)) {
            return Err(eyre!("generated validator Torii binding differs from slot"));
        }
        Ok((public, policy))
    })();
    crate::soracloud::zeroize_taira_toml_table(&mut table);
    result
}

fn json_line<T: JsonSerialize>(value: &T) -> Result<Vec<u8>> {
    let mut bytes = json::to_vec(value)?;
    bytes.push(b'\n');
    Ok(bytes)
}

#[cfg(unix)]
fn existing_matches(path: &Path, files: &[(&str, Zeroizing<Vec<u8>>)]) -> Result<()> {
    validate_owner_private_dir(path, "published runtime client directory")?;
    let actual = fs::read_dir(path)?
        .map(|entry| {
            entry?
                .file_name()
                .into_string()
                .map_err(|_| eyre!("published runtime client name is not UTF-8"))
        })
        .collect::<Result<BTreeSet<_>>>()?;
    let expected = files
        .iter()
        .map(|(name, _)| (*name).to_owned())
        .collect::<BTreeSet<_>>();
    if actual != expected {
        return Err(eyre!(
            "published runtime client set is incomplete or foreign"
        ));
    }
    for (name, bytes) in files {
        let file_path = path.join(name);
        let (file, snapshot) = open_pinned_regular(&file_path, "published runtime client")?;
        if snapshot.uid != rustix::process::geteuid().as_raw()
            || snapshot.mode & 0o7777 != 0o600
            || snapshot.nlink != 1
            || snapshot.len != bytes.len() as u64
            || read_pinned_bytes(
                &file_path,
                "published runtime client",
                file,
                &snapshot,
                MAX_CLIENT_INPUT_BYTES,
            )?
            .as_slice()
                != bytes.as_slice()
        {
            return Err(eyre!(
                "published runtime client differs from the exact generated bundle"
            ));
        }
    }
    Ok(())
}

/// Stage the complete private bundle and publish it without replacing another run.
#[cfg(unix)]
pub(super) fn prepare(args: &PrepareRuntimeClients, output: &mut impl Write) -> Result<()> {
    use std::os::unix::fs::PermissionsExt as _;
    let _guard = ChainDiscriminantGuard::enter(CHAIN_DISCRIMINANT);
    for (path, label) in [
        (&args.localnet_dir, "generated network directory"),
        (&args.public_inputs_dir, "native public input directory"),
        (&args.canary_key_dir, "canary key directory"),
        (&args.output_dir, "runtime client output directory"),
    ] {
        runtime_path(path, label)?;
    }
    let runtime_dir = args.localnet_dir.join("runtime");
    let signer_dir = runtime_dir.join("taira-runtime-signers");
    for (path, label) in [
        (&args.localnet_dir, "generated network directory"),
        (&args.public_inputs_dir, "native public input directory"),
        (&args.canary_key_dir, "canary key directory"),
        (&runtime_dir, "generated runtime directory"),
        (&signer_dir, "generated runtime signer directory"),
    ] {
        validate_owner_private_dir(path, label)?;
    }
    let parent = args
        .output_dir
        .parent()
        .ok_or_else(|| eyre!("runtime client output has no parent"))?;
    validate_owner_private_dir(parent, "runtime client output parent")?;
    let public = public_inputs::load(&args.public_inputs_dir)?;
    let network_path = args.localnet_dir.join("genesis.expected_hash");
    let network_line = private_bytes(&network_path, MAX_KEY_BYTES, "generated genesis identity")?;
    if network_line.as_slice() != format!("{}\n", public.network_id).as_bytes()
        || public.genesis_hash != Hash::from(public.network_id.into_genesis_hash()).to_string()
    {
        return Err(eyre!(
            "generated and native public genesis identities differ"
        ));
    }
    let base_bytes = private_bytes(
        &args.localnet_dir.join("client.toml"),
        MAX_CLIENT_INPUT_BYTES,
        "generated base client",
    )?;
    let mut base = parse_base(&base_bytes)?;
    let result = (|| {
        let admin = toml_text(toml_table(&base, &["account"])?, "public_key")?;
        let canary_public_text = parse_private_text(
            &private_bytes(
                &args.canary_key_dir.join("public.key"),
                MAX_KEY_BYTES,
                "canary public key",
            )?,
            "canary public key",
        )?;
        let canary_private = parse_private_text(
            &private_bytes(
                &args.canary_key_dir.join("private.key"),
                MAX_KEY_BYTES,
                "canary private key",
            )?,
            "canary private key",
        )?;
        let canary_public = checked_key_pair(&canary_public_text, &canary_private, "canary")?;
        if canary_public != public.canary_public_key || canary_public.to_string() == admin {
            return Err(eyre!(
                "canary identity differs from native public inputs or maintenance admin"
            ));
        }
        let manifest = public_manifest(&args.localnet_dir.join("lane-manifests/is.manifest.json"))?;
        let mut files: Vec<(&'static str, Zeroizing<Vec<u8>>)> = vec![(
            "runtime-client.toml",
            render_client(
                &base,
                &public.network_id,
                "https://taira.sora.org/",
                &canary_public,
                &canary_private,
            )?,
        )];
        let mut clients = Vec::with_capacity(4);
        let mut faucet: Option<FaucetPolicyV1> = None;
        let mut accounts = BTreeSet::new();
        let mut peers = BTreeSet::new();
        for (index, row) in manifest.validators.iter().enumerate() {
            let path = args.localnet_dir.join(format!("peer{index}.toml"));
            let config =
                private_bytes(&path, MAX_CLIENT_INPUT_BYTES, "generated validator config")?;
            let (signer_public, policy) = peer_config(&config, row, index)?;
            let private = parse_private_text(
                &private_bytes(
                    &args.localnet_dir.join(format!(
                        "runtime/taira-runtime-signers/peer{index}.private_key"
                    )),
                    MAX_KEY_BYTES,
                    "validator runtime signer key",
                )?,
                "validator runtime signer key",
            )?;
            checked_key_pair(
                &signer_public.to_string(),
                &private,
                "validator runtime signer",
            )?;
            if signer_public == canary_public
                || signer_public.to_string() == admin
                || !accounts.insert(row.validator.clone())
                || !peers.insert(row.peer_id.clone())
            {
                return Err(eyre!(
                    "generated validator signer identities are not four distinct peers"
                ));
            }
            if faucet.as_ref().is_some_and(|prior| prior != &policy) {
                return Err(eyre!("generated validator faucet policies differ"));
            }
            faucet = Some(policy);
            let origin = format!("https://{}.sora.org/", VALIDATOR_SLUGS[index]);
            files.push((
                [
                    "validator-client-1.toml",
                    "validator-client-2.toml",
                    "validator-client-3.toml",
                    "validator-client-4.toml",
                ][index],
                render_client(&base, &public.network_id, &origin, &signer_public, &private)?,
            ));
            clients.push(ValidatorClientV1 {
                slug: VALIDATOR_SLUGS[index].into(),
                torii_origin: origin,
                probe_origin: format!("http://127.0.0.1:{}/", 8080 + index),
                account_id: row.validator.clone(),
                peer_id: row.peer_id.clone(),
            });
        }
        let faucet = faucet.ok_or_else(|| eyre!("generated faucet policy is absent"))?;
        if accounts.contains(&faucet.authority) || accounts.len() != 4 || peers.len() != 4 {
            return Err(eyre!("generated validator and faucet identities overlap"));
        }
        files.push((
            "validator-clients.json",
            Zeroizing::new(json_line(&clients)?),
        ));
        files.push(("faucet-policy.json", Zeroizing::new(json_line(&faucet)?)));
        for (_, bytes) in &files {
            if bytes.is_empty() || bytes.len() as u64 > MAX_CLIENT_INPUT_BYTES {
                return Err(eyre!("runtime client output exceeds its byte bound"));
            }
        }
        if args.output_dir.try_exists()? {
            existing_matches(&args.output_dir, &files)?;
        } else {
            let temporary = tempfile::Builder::new()
                .prefix(".runtime-clients-")
                .tempdir_in(parent)?;
            fs::set_permissions(temporary.path(), fs::Permissions::from_mode(0o700))?;
            validate_owner_private_dir(temporary.path(), "runtime client staging directory")?;
            for (name, bytes) in &files {
                inputs::write_new_private(&temporary.path().join(name), bytes)?;
            }
            File::open(temporary.path())?.sync_all()?;
            validate_owner_private_dir(parent, "runtime client output parent")?;
            match rustix::fs::renameat_with(
                rustix::fs::CWD,
                temporary.path(),
                rustix::fs::CWD,
                &args.output_dir,
                rustix::fs::RenameFlags::NOREPLACE,
            ) {
                Ok(()) => {}
                Err(rustix::io::Errno::EXIST) => existing_matches(&args.output_dir, &files)?,
                Err(error) => {
                    return Err(error)
                        .wrap_err("publish fresh runtime client bundle without replacement");
                }
            }
        }
        File::open(parent)?.sync_all()?;
        existing_matches(&args.output_dir, &files)?;
        let receipt = ClientBundleReceipt {
            schema: "iroha.taira.public-reset.runtime-clients.v1".into(),
            output_dir: args
                .output_dir
                .to_str()
                .ok_or_else(|| eyre!("runtime output path is not UTF-8"))?
                .into(),
            validator_clients: 4,
            genesis_hash: public.genesis_hash,
        };
        writeln!(output, "{}", json::to_json(&receipt)?)?;
        Ok(())
    })();
    crate::soracloud::zeroize_taira_toml_table(&mut base);
    result
}

/// Guest-only producer requires Unix private-file custody.
#[cfg(not(unix))]
pub(super) fn prepare(_args: &PrepareRuntimeClients, _output: &mut impl Write) -> Result<()> {
    Err(eyre!(
        "runtime client preparation requires Unix private-file custody"
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parser_requires_the_complete_native_input_bundle() {
        use clap::Parser as _;
        #[derive(clap::Parser)]
        struct Command {
            #[command(flatten)]
            clients: PrepareRuntimeClients,
        }
        let arguments = [
            "iroha",
            "--localnet-dir",
            "/private/runtime/taira-public-reset/run/network",
            "--public-inputs-dir",
            "/private/runtime/taira-public-reset/run/public-inputs",
            "--canary-key-dir",
            "/private/runtime/taira-public-reset/run/canary-key",
            "--output-dir",
            "/private/runtime/taira-public-reset/run/clients",
        ];
        let parsed = Command::try_parse_from(arguments).unwrap();
        assert_eq!(parsed.clients.output_dir, PathBuf::from(arguments[8]));
        assert!(Command::try_parse_from(arguments[..7].iter().copied()).is_err());
    }

    #[test]
    fn runtime_paths_are_normalized_and_beneath_private_root() {
        assert!(
            runtime_path(
                Path::new("/private/runtime/taira-public-reset/run/clients"),
                "output"
            )
            .is_ok()
        );
        for path in [
            "/private/runtime/taira-public-reset",
            "/private/runtime/taira-public-reset-foreign/run",
            "/private/runtime/taira-public-reset/run/../clients",
            "relative/clients",
        ] {
            assert!(runtime_path(Path::new(path), "output").is_err(), "{path}");
        }
    }

    #[test]
    fn manifest_requires_exact_four_validator_is_committee() {
        let correct = r#"{"lane":"is","governance":"parliament","version":1,"validators":[{"validator":"one","peer_id":"peer1"},{"validator":"two","peer_id":"peer2"},{"validator":"three","peer_id":"peer3"},{"validator":"four","peer_id":"peer4"}],"quorum":3}"#;
        let decoded: LaneManifest = json::from_str(correct).unwrap();
        validate_manifest(&decoded).unwrap();
        for wrong in [
            correct.replace("\"quorum\":3", "\"quorum\":1"),
            correct.replace("\"lane\":\"is\"", "\"lane\":\"paynet\""),
        ] {
            let parsed: LaneManifest = json::from_str(&wrong).unwrap();
            assert!(validate_manifest(&parsed).is_err());
        }
        let unknown = correct.replace("\"peer_id\":\"peer4\"", "\"peer_id\":\"peer4\",\"extra\":1");
        assert!(json::from_str::<LaneManifest>(&unknown).is_err());
    }

    #[test]
    fn peer_parser_binds_manifest_signer_socket_and_faucet() {
        let _guard = ChainDiscriminantGuard::enter(CHAIN_DISCRIMINANT);
        let peer = KeyPair::from_seed(b"peer-fixture".to_vec(), Algorithm::BlsNormal);
        let signer = KeyPair::from_seed(b"runtime-signer-fixture".to_vec(), Algorithm::Ed25519);
        let faucet = KeyPair::from_seed(b"faucet-fixture".to_vec(), Algorithm::Ed25519);
        let (_, bytes) = signer.public_key().try_to_bytes().unwrap();
        let account = AccountId::new(signer.public_key().clone()).to_string();
        let row = LaneManifestValidator {
            validator: account.clone(),
            peer_id: PeerId::from(peer.public_key().clone()).to_string(),
        };
        let address = json::to_json(&iroha_primitives::addr::SocketAddr::from((
            [127, 0, 0, 1],
            8080,
        )))
        .unwrap();
        let config = format!(
            "chain = {CHAIN_ID:?}\nchain_discriminant = 369\npublic_key = {:?}\n[soracloud_runtime.submission.signer]\npublic_key_hex = {:?}\nauthority = {account:?}\n[torii]\naddress = {address}\n[torii.faucet]\nenabled = true\nauthority = {:?}\nasset_definition_id = {:?}\namount = \"100\"\n",
            peer.public_key().to_string(),
            hex::encode(bytes),
            AccountId::new(faucet.public_key().clone()).to_string(),
            crate::taira::DEFAULT_GAS_ASSET_ID,
        );
        let (actual_signer, policy) = peer_config(config.as_bytes(), &row, 0).unwrap();
        assert_eq!(actual_signer, *signer.public_key());
        assert_eq!(policy.amount, Quantity::from(100_u32));
        let wrong_address = json::to_json(&iroha_primitives::addr::SocketAddr::from((
            [127, 0, 0, 1],
            8081,
        )))
        .unwrap();
        let wrong_port = config.replace(&address, &wrong_address);
        assert!(peer_config(wrong_port.as_bytes(), &row, 0).is_err());
        let wrong_account = LaneManifestValidator {
            validator: AccountId::new(faucet.public_key().clone()).to_string(),
            peer_id: row.peer_id.clone(),
        };
        assert!(peer_config(config.as_bytes(), &wrong_account, 0).is_err());
    }

    #[test]
    fn rendered_clients_bind_exact_identity_and_hide_input_secret_on_error() {
        let kp = KeyPair::from_seed(b"runtime-client-fixture".to_vec(), Algorithm::Ed25519);
        let network = NetworkId::from_genesis_hash(iroha_crypto::HashOf::from_untyped_unchecked(
            Hash::new(b"runtime-client-test"),
        ));
        let base = format!(
            "chain = {CHAIN_ID:?}\nnetwork_id_file = \"genesis.expected_hash\"\ntorii_url = \"http://127.0.0.1:8080/\"\n[transaction]\ntime_to_live_ms = 30000\nstatus_timeout_ms = 30000\nnonce = false\n[account]\ndomain = \"wonderland.universal\"\nchain_discriminant = 369\npublic_key = {:?}\nprivate_key = \"fixture-secret-invalid\"\n[basic_auth]\nweb_login = \"fixture-user\"\npassword = \"fixture-password\"\n",
            kp.public_key().to_string(),
        );
        let error = parse_base(base.as_bytes()).unwrap_err();
        assert!(!format!("{error:#}").contains("fixture-secret"));
        let valid = base.replace(
            "fixture-secret-invalid",
            &iroha_crypto::ExposedPrivateKey(kp.private_key().clone())
                .try_to_multihash_string()
                .unwrap(),
        );
        let mut parsed = parse_base(valid.as_bytes()).unwrap();
        for foreign in [
            valid.replace(
                "network_id_file = \"genesis.expected_hash\"",
                &format!("network_id = {:?}", network.to_string()),
            ),
            valid.replace("genesis.expected_hash", "../foreign.expected_hash"),
        ] {
            assert!(parse_base(foreign.as_bytes()).is_err());
        }
        let output = render_client(
            &parsed,
            &network,
            "https://taira.sora.org/",
            kp.public_key(),
            "fixture-secret-marker",
        )
        .unwrap();
        let rendered: toml::Table = toml::from_str(std::str::from_utf8(&output).unwrap()).unwrap();
        let network_text = network.to_string();
        assert_eq!(
            rendered.get("network_id").and_then(toml::Value::as_str),
            Some(network_text.as_str())
        );
        assert_eq!(
            rendered.get("torii_url").and_then(toml::Value::as_str),
            Some("https://taira.sora.org/")
        );
        assert_eq!(
            toml_table(&rendered, &["account"])
                .unwrap()
                .get("private_key")
                .and_then(toml::Value::as_str),
            Some("fixture-secret-marker")
        );
        assert!(!rendered.contains_key("network_id_file"));
        crate::soracloud::zeroize_taira_toml_table(&mut parsed);
    }

    #[cfg(unix)]
    #[test]
    fn published_client_bundle_replays_only_exact_private_complete_files() {
        use std::os::unix::fs::PermissionsExt as _;

        let cwd = std::env::current_dir().unwrap();
        let root = tempfile::Builder::new()
            .prefix("runtime-client-replay-test-")
            .tempdir_in(cwd)
            .unwrap();
        let output = root.path().join("clients");
        fs::create_dir(&output).unwrap();
        fs::set_permissions(&output, fs::Permissions::from_mode(0o700)).unwrap();
        let files = [
            (
                "runtime-client.toml",
                Zeroizing::new(b"private client\n".to_vec()),
            ),
            (
                "validator-clients.json",
                Zeroizing::new(b"public census\n".to_vec()),
            ),
        ];
        for (name, bytes) in &files {
            inputs::write_new_private(&output.join(name), bytes).unwrap();
        }
        existing_matches(&output, &files).unwrap();
        fs::write(output.join(files[0].0), b"changed client\n").unwrap();
        assert!(existing_matches(&output, &files).is_err());
        fs::write(output.join(files[0].0), files[0].1.as_slice()).unwrap();
        fs::remove_file(output.join(files[1].0)).unwrap();
        assert!(existing_matches(&output, &files).is_err());
    }
}
