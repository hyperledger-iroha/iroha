//! Generate per-client Iroha CLI configs from a base client.toml.
use crate::{Outcome, RunArgs, tui};
use clap::Args as ClapArgs;
use color_eyre::eyre::{Result, WrapErr as _, eyre};
use iroha_crypto::{Algorithm, ExposedPrivateKey, KeyPair};
use iroha_data_model::{
    NetworkId,
    domain::DomainId,
    name::{Name, canonicalize_domain_label},
};
use std::{
    collections::BTreeSet,
    fs,
    io::{BufWriter, Write},
    path::{Path, PathBuf},
    str::FromStr as _,
};
#[cfg(unix)]
use std::{fs::OpenOptions, io::Read as _};
use zeroize::{Zeroize as _, Zeroizing};
const DEFAULT_TTL_MS: u64 = 120_000;
const DEFAULT_STATUS_TIMEOUT_MS: u64 = 120_000;
#[cfg(unix)]
const MAX_BASE_CONFIG_BYTES: u64 = 64 * 1024;
const CLIENT_CONFIG_DERIVATION_DOMAIN: &[u8] = b"iroha:kagami:client-config:v1";
struct BaseConfig {
    chain: String,
    network_id: NetworkId,
    torii_url: String,
    basic_auth: Option<BasicAuth>,
}
struct BasicAuth {
    web_login: String,
    password: Zeroizing<String>,
}
/// Generate per-client CLI configs from a base client.toml.
#[derive(ClapArgs)]
pub struct Args {
    /// Base client config to copy `chain`, `torii_url`, and `basic_auth` from.
    #[arg(long, value_name = "PATH")]
    base_config: PathBuf,
    /// Output directory for generated client configs (default: <base-config-dir>/clients).
    #[arg(long, value_name = "DIR")]
    out_dir: Option<PathBuf>,
    /// Account scope for generated client configs (`dataspace` or `domain.dataspace`).
    #[arg(long, default_value = "acme.universal", value_name = "SCOPE")]
    domain: String,
    /// A 32-byte secret master seed encoded as 64 hexadecimal characters.
    ///
    /// Per-client keys are derived with an explicit domain and client name.
    /// Omit this option for independent operating-system-random keys.
    #[arg(long, value_name = "HEX")]
    seed_hex: Option<String>,
    /// Comma-separated list of client names.
    #[arg(long, value_delimiter = ',', required = true, value_name = "NAME")]
    names: Vec<String>,
}
impl<T: Write> RunArgs<T> for Args {
    fn run(self, writer: &mut BufWriter<T>) -> Outcome {
        let Self {
            base_config,
            out_dir,
            domain,
            seed_hex,
            names,
        } = self;
        let seed_hex = seed_hex.map(Zeroizing::new);
        let master_seed = seed_hex
            .as_ref()
            .map(|seed_hex| crate::crypto::parse_keygen_seed_hex(seed_hex.as_str()))
            .transpose()?;
        let base = load_base_config(&base_config)?;
        let out_dir = resolve_out_dir(&base_config, out_dir)?;
        let names = normalize_names(names)?;
        validate_account_scope(&domain)?;
        let out_dir = crate::secure_fs::prepare_empty_private_directory(&out_dir)
            .wrap_err("prepare client-config private output directory")?;
        tui::status(format!(
            "Generating {} client configs in {}",
            names.len(),
            out_dir.display()
        ));
        for name in names {
            let key_pair = if let Some(master_seed) = master_seed.as_ref() {
                derive_client_key_pair(master_seed.as_slice(), &name)?
            } else {
                KeyPair::try_random_with_algorithm(Algorithm::Ed25519)
                    .wrap_err("failed to generate an OS-random client key pair")?
            };
            let rendered = render_client_config(&base, &domain, &key_pair)?;
            let path = out_dir.join(format!("{name}.toml"));
            crate::secure_fs::write_private_file_atomic(&path, rendered.as_bytes())
                .wrap_err_with(|| format!("failed to write {}", path.display()))?;
            writeln!(writer, "wrote {}", path.display())?;
        }
        tui::success("Client configs ready");
        Ok(())
    }
}
fn derive_client_key_pair(master_seed: &[u8], name: &str) -> Result<KeyPair> {
    if master_seed.len() != 32 {
        return Err(eyre!("client-config master seed must be exactly 32 bytes"));
    }
    let name_len =
        u64::try_from(name.len()).map_err(|_| eyre!("client name length exceeds u64"))?;
    let mut seed_material = Zeroizing::new(Vec::with_capacity(
        CLIENT_CONFIG_DERIVATION_DOMAIN.len()
            + master_seed.len()
            + core::mem::size_of::<u64>()
            + name.len(),
    ));
    seed_material.extend_from_slice(CLIENT_CONFIG_DERIVATION_DOMAIN);
    seed_material.extend_from_slice(master_seed);
    seed_material.extend_from_slice(&name_len.to_le_bytes());
    seed_material.extend_from_slice(name.as_bytes());
    KeyPair::try_from_seed(std::mem::take(&mut *seed_material), Algorithm::Ed25519)
        .wrap_err_with(|| format!("failed to derive key pair for client `{name}`"))
}
fn load_base_config(path: &Path) -> Result<BaseConfig> {
    let raw = read_base_config(path)?;
    let description = format!("base client config {}", path.display());
    let value = crate::secret_toml::Value::new(toml::Value::Table(
        crate::secret_toml::parse_table(&raw, &description)?,
    ));
    let chain = value
        .get("chain")
        .and_then(toml::Value::as_str)
        .ok_or_else(|| eyre!("base config is missing `chain`"))?
        .to_owned();
    let network_id = value
        .get("network_id")
        .and_then(toml::Value::as_str)
        .ok_or_else(|| eyre!("base config is missing `network_id`"))?
        .parse::<NetworkId>()
        .wrap_err("base config `network_id` is not an exact genesis hash")?;
    let torii_url = value
        .get("torii_url")
        .and_then(toml::Value::as_str)
        .ok_or_else(|| eyre!("base config is missing `torii_url`"))?
        .to_owned();
    let basic_auth = match value.get("basic_auth") {
        Some(toml::Value::Table(table)) => {
            let web_login = table
                .get("web_login")
                .and_then(toml::Value::as_str)
                .ok_or_else(|| eyre!("base config `basic_auth.web_login` is missing"))?
                .to_owned();
            let password = table
                .get("password")
                .and_then(toml::Value::as_str)
                .ok_or_else(|| eyre!("base config `basic_auth.password` is missing"))?
                .to_owned();
            Some(BasicAuth {
                web_login,
                password: Zeroizing::new(password),
            })
        }
        Some(_) => return Err(eyre!("base config `basic_auth` must be a TOML table")),
        None => None,
    };
    Ok(BaseConfig {
        chain,
        network_id,
        torii_url,
        basic_auth,
    })
}
#[cfg(unix)]
fn same_base_config_snapshot(left: &fs::Metadata, right: &fs::Metadata) -> bool {
    use std::os::unix::fs::MetadataExt as _;
    left.dev() == right.dev()
        && left.ino() == right.ino()
        && left.mode() == right.mode()
        && left.uid() == right.uid()
        && left.gid() == right.gid()
        && left.nlink() == right.nlink()
        && left.size() == right.size()
        && left.mtime() == right.mtime()
        && left.mtime_nsec() == right.mtime_nsec()
        && left.ctime() == right.ctime()
        && left.ctime_nsec() == right.ctime_nsec()
}
fn read_base_config(path: &Path) -> Result<Zeroizing<String>> {
    #[cfg(not(unix))]
    return Err(eyre!(
        "base client config requires owner-only file custody, which Kagami cannot verify on this platform"
    ));
    #[cfg(unix)]
    {
        let lexical = fs::symlink_metadata(path)
            .wrap_err_with(|| format!("failed to inspect {}", path.display()))?;
        if !lexical.is_file()
            || lexical.file_type().is_symlink()
            || lexical.len() > MAX_BASE_CONFIG_BYTES
        {
            return Err(eyre!(
                "base config must be a non-symlink regular file within the {MAX_BASE_CONFIG_BYTES}-byte input limit"
            ));
        }
        let mut options = OpenOptions::new();
        options.read(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt as _;
            options.custom_flags(libc::O_CLOEXEC | libc::O_NOFOLLOW | libc::O_NONBLOCK);
        }
        let mut file = options
            .open(path)
            .wrap_err_with(|| format!("failed to open {}", path.display()))?;
        let before = file
            .metadata()
            .wrap_err_with(|| format!("failed to inspect opened {}", path.display()))?;
        if !before.is_file() || !same_base_config_snapshot(&lexical, &before) {
            return Err(eyre!("base config changed while it was opened"));
        }
        {
            use std::os::unix::fs::MetadataExt as _;
            if before.uid() != rustix::process::geteuid().as_raw()
                || before.nlink() != 1
                || before.mode() & 0o777 != 0o600
            {
                return Err(eyre!(
                    "base client config must be owner-held, single-link, and have exact mode 0600"
                ));
            }
        }
        let capacity = usize::try_from(before.len())
            .map_err(|_| eyre!("base config length cannot be addressed on this platform"))?;
        let mut raw = Zeroizing::new(Vec::new());
        raw.try_reserve_exact(capacity.saturating_add(1))
            .wrap_err("reserve base config buffer")?;
        std::io::Read::by_ref(&mut file)
            .take(MAX_BASE_CONFIG_BYTES + 1)
            .read_to_end(&mut raw)
            .wrap_err_with(|| format!("failed to read TOML from {}", path.display()))?;
        let after = file
            .metadata()
            .wrap_err_with(|| format!("failed to reinspect {}", path.display()))?;
        if raw.len() as u64 > MAX_BASE_CONFIG_BYTES
            || raw.len() as u64 != before.len()
            || !same_base_config_snapshot(&before, &after)
        {
            return Err(eyre!(
                "base config changed while it was read or exceeded its input limit"
            ));
        }
        match String::from_utf8(std::mem::take(&mut *raw)) {
            Ok(raw) => Ok(Zeroizing::new(raw)),
            Err(error) => {
                let utf8_error = error.utf8_error();
                let mut bytes = error.into_bytes();
                bytes.zeroize();
                Err(eyre!("base config is not UTF-8: {utf8_error}"))
            }
        }
    }
}
fn resolve_out_dir(base_config: &Path, out_dir: Option<PathBuf>) -> Result<PathBuf> {
    if let Some(out_dir) = out_dir {
        return Ok(out_dir);
    }
    let parent = base_config
        .parent()
        .ok_or_else(|| eyre!("base config has no parent directory"))?;
    Ok(parent.join("clients"))
}
fn normalize_names(raw: Vec<String>) -> Result<Vec<String>> {
    let mut names = Vec::new();
    let mut seen = BTreeSet::new();
    for name in raw {
        let trimmed = name.trim();
        if trimmed.is_empty() {
            return Err(eyre!("client name cannot be empty"));
        }
        if trimmed.contains('/') || trimmed.contains('\\') {
            return Err(eyre!(
                "client name `{}` must not contain path separators",
                trimmed
            ));
        }
        Name::from_str(trimmed)
            .wrap_err_with(|| format!("client name `{trimmed}` is not canonical"))?;
        if !trimmed
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
        {
            return Err(eyre!(
                "client name `{trimmed}` must contain only ASCII letters, digits, `-`, or `_`"
            ));
        }
        if !seen.insert(trimmed.to_owned()) {
            return Err(eyre!("duplicate client name `{}`", trimmed));
        }
        names.push(trimmed.to_owned());
    }
    if names.is_empty() {
        return Err(eyre!("no client names provided"));
    }
    Ok(names)
}
fn render_client_config(
    base: &BaseConfig,
    account_scope: &str,
    key_pair: &KeyPair,
) -> Result<Zeroizing<String>> {
    validate_account_scope(account_scope)?;
    let public_key = key_pair
        .public_key()
        .try_to_multihash_string()
        .wrap_err("encode generated public key")?;
    let private_key = Zeroizing::new(
        ExposedPrivateKey(key_pair.private_key().clone())
            .try_to_multihash_string()
            .wrap_err("encode generated private key")?,
    );
    let mut root = crate::secret_toml::Table::default();
    root.insert("chain".into(), toml::Value::String(base.chain.clone()));
    root.insert(
        "network_id".into(),
        toml::Value::String(base.network_id.to_string()),
    );
    root.insert(
        "torii_url".into(),
        toml::Value::String(base.torii_url.clone()),
    );

    let mut transaction = toml::Table::new();
    transaction.insert(
        "time_to_live_ms".into(),
        toml::Value::Integer(
            i64::try_from(DEFAULT_TTL_MS).wrap_err("default TTL does not fit TOML integer")?,
        ),
    );
    transaction.insert(
        "status_timeout_ms".into(),
        toml::Value::Integer(
            i64::try_from(DEFAULT_STATUS_TIMEOUT_MS)
                .wrap_err("default status timeout does not fit TOML integer")?,
        ),
    );
    transaction.insert("nonce".into(), toml::Value::Boolean(false));
    root.insert("transaction".into(), toml::Value::Table(transaction));

    let mut account = toml::Table::new();
    account.insert(
        "domain".into(),
        toml::Value::String(account_scope.to_owned()),
    );
    account.insert(
        "private_key".into(),
        toml::Value::String(private_key.as_str().to_owned()),
    );
    account.insert("public_key".into(), toml::Value::String(public_key));
    root.insert("account".into(), toml::Value::Table(account));

    if let Some(auth) = &base.basic_auth {
        let mut basic_auth = toml::Table::new();
        basic_auth.insert(
            "password".into(),
            toml::Value::String(auth.password.as_str().to_owned()),
        );
        basic_auth.insert(
            "web_login".into(),
            toml::Value::String(auth.web_login.clone()),
        );
        root.insert("basic_auth".into(), toml::Value::Table(basic_auth));
    }
    toml::to_string(&*root)
        .map(Zeroizing::new)
        .wrap_err("serialize generated client TOML")
}
fn validate_account_scope(value: &str) -> Result<()> {
    if value.trim().is_empty() || value.trim() != value {
        return Err(eyre!(
            "account scope must use canonical `dataspace` or `domain.dataspace` form"
        ));
    }
    let valid = if value.contains('.') {
        DomainId::parse_fully_qualified(value).is_ok_and(|scope| scope.to_string() == value)
    } else {
        canonicalize_domain_label(value).is_ok_and(|scope| scope == value)
    };
    if !valid {
        return Err(eyre!(
            "account scope must use canonical `dataspace` or `domain.dataspace` form"
        ));
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::{fs, io::BufWriter};
    fn write_base_config(path: &Path) {
        let payload = r#"
chain = "demo-chain"
network_id = "hash:32C903E5B3497E34C2B844EBFE8A39C19E6CF8F95D44C1FFB8BA9DCB42F91149#A2F0"
torii_url = "http://127.0.0.1:8080/"

[basic_auth]
password = "secret"
web_login = "demo"
"#;
        fs::write(path, payload).expect("write base config");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            fs::set_permissions(path, fs::Permissions::from_mode(0o600))
                .expect("protect base config");
        }
    }
    #[cfg(unix)]
    #[test]
    fn load_base_config_reads_fields() {
        let temp = tempfile::tempdir().expect("temp dir");
        let path = temp.path().join("client.toml");
        write_base_config(&path);
        let base = load_base_config(&path).expect("load base config");
        assert_eq!(base.chain, "demo-chain");
        assert_eq!(
            base.network_id.to_string(),
            "hash:32C903E5B3497E34C2B844EBFE8A39C19E6CF8F95D44C1FFB8BA9DCB42F91149#A2F0"
        );
        assert_eq!(base.torii_url, "http://127.0.0.1:8080/");
        let auth = base.basic_auth.expect("basic auth present");
        assert_eq!(auth.web_login, "demo");
        assert_eq!(auth.password.as_str(), "secret");
    }
    #[cfg(unix)]
    #[test]
    fn load_base_config_rejects_oversized_inputs_and_malformed_auth() {
        let temp = tempfile::tempdir().expect("temp dir");
        let path = temp.path().join("client.toml");
        let oversized_len = usize::try_from(MAX_BASE_CONFIG_BYTES).expect("limit fits usize") + 1;
        fs::write(&path, vec![b'x'; oversized_len]).expect("write oversized config");
        let error = load_base_config(&path)
            .err()
            .expect("oversized config must be rejected");
        assert!(error.to_string().contains("input limit"));

        let malformed = r#"
chain = "demo-chain"
network_id = "hash:32C903E5B3497E34C2B844EBFE8A39C19E6CF8F95D44C1FFB8BA9DCB42F91149#A2F0"
torii_url = "http://127.0.0.1:8080/"
basic_auth = "not a table"
"#;
        fs::write(&path, malformed).expect("write malformed config");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            fs::set_permissions(&path, fs::Permissions::from_mode(0o600))
                .expect("protect malformed config fixture");
        }
        let error = load_base_config(&path)
            .err()
            .expect("malformed auth must be rejected");
        assert!(error.to_string().contains("must be a TOML table"));
    }
    #[cfg(unix)]
    #[test]
    fn base_config_reader_rejects_symlinks_and_special_files() {
        use std::os::unix::fs::symlink;

        let temp = tempfile::tempdir().expect("temp dir");
        let target = temp.path().join("client.toml");
        write_base_config(&target);
        let linked = temp.path().join("linked.toml");
        symlink(&target, &linked).expect("create base-config symlink");
        assert!(read_base_config(&linked).is_err());

        let fifo = temp.path().join("client.fifo");
        crate::secure_fs::create_fifo_for_test(&fifo, 0o600).expect("create base-config FIFO");
        assert!(read_base_config(&fifo).is_err());
    }
    #[cfg(unix)]
    #[test]
    fn base_config_reader_requires_exact_owner_only_mode() {
        use std::os::unix::fs::PermissionsExt as _;

        let temp = tempfile::tempdir().expect("temp dir");
        let path = temp.path().join("client.toml");
        write_base_config(&path);
        fs::set_permissions(&path, fs::Permissions::from_mode(0o644))
            .expect("make base config public-readable");
        let error = read_base_config(&path)
            .expect_err("a client config containing secrets must not be public-readable");
        assert!(error.to_string().contains("exact mode 0600"));
    }
    #[test]
    fn resolve_out_dir_defaults_to_clients_dir() {
        let temp = tempfile::tempdir().expect("temp dir");
        let path = temp.path().join("client.toml");
        let out_dir = resolve_out_dir(&path, None).expect("resolve out dir");
        assert_eq!(out_dir, temp.path().join("clients"));
    }
    #[test]
    fn normalize_names_trims_and_rejects_duplicates() {
        let names = normalize_names(vec![" admin1 ".into(), "admin2".into()]).expect("names ok");
        assert_eq!(names, vec!["admin1".to_owned(), "admin2".to_owned()]);
        let err = normalize_names(vec!["admin1".into(), "admin1".into()])
            .expect_err("duplicate rejected");
        assert!(format!("{err}").contains("duplicate client name"));
    }
    #[test]
    fn normalize_names_rejects_path_components() {
        for name in [
            "../escape",
            "nested/client",
            r"nested\client",
            ".",
            "client.toml",
        ] {
            let error = normalize_names(vec![name.to_owned()])
                .expect_err("client filename stem must not contain path syntax");
            assert!(error.to_string().contains("client name"));
        }
    }
    #[test]
    fn deterministic_client_derivation_requires_secret_master_and_separates_names() {
        let master = [0xA5; 32];
        let alice = derive_client_key_pair(&master, "alice").expect("derive Alice");
        let alice_again = derive_client_key_pair(&master, "alice").expect("derive Alice again");
        let bob = derive_client_key_pair(&master, "bob").expect("derive Bob");
        assert_eq!(alice.public_key(), alice_again.public_key());
        assert_ne!(alice.public_key(), bob.public_key());
        assert!(
            derive_client_key_pair(b"human password", "alice").is_err(),
            "low-entropy, wrong-length master material must be rejected"
        );
    }
    #[test]
    fn render_client_config_contains_expected_fields() {
        let base = BaseConfig {
            chain: "demo-chain".to_owned(),
            network_id:
                "hash:32C903E5B3497E34C2B844EBFE8A39C19E6CF8F95D44C1FFB8BA9DCB42F91149#A2F0"
                    .parse()
                    .expect("network id"),
            torii_url: "http://127.0.0.1:8080/".to_owned(),
            basic_auth: Some(BasicAuth {
                web_login: "demo".to_owned(),
                password: Zeroizing::new("secret".to_owned()),
            }),
        };
        let key_pair = KeyPair::try_from_seed(b"demo-admin1".to_vec(), Algorithm::Ed25519)
            .expect("seeded client key should derive");
        let rendered =
            render_client_config(&base, "acme.universal", &key_pair).expect("render config");
        let value: toml::Value = toml::from_str(&rendered).expect("parse rendered config");
        assert_eq!(
            value.get("chain").and_then(toml::Value::as_str),
            Some("demo-chain")
        );
        assert_eq!(
            value.get("network_id").and_then(toml::Value::as_str),
            Some("hash:32C903E5B3497E34C2B844EBFE8A39C19E6CF8F95D44C1FFB8BA9DCB42F91149#A2F0")
        );
        assert_eq!(
            value.get("torii_url").and_then(toml::Value::as_str),
            Some("http://127.0.0.1:8080/")
        );
        let account = value
            .get("account")
            .and_then(toml::Value::as_table)
            .expect("account");
        assert_eq!(
            account.get("domain").and_then(toml::Value::as_str),
            Some("acme.universal")
        );
        let expected_public = key_pair.public_key().to_string();
        let expected_private = ExposedPrivateKey(key_pair.private_key().clone()).to_string();
        assert_eq!(
            account.get("public_key").and_then(toml::Value::as_str),
            Some(expected_public.as_str())
        );
        assert_eq!(
            account.get("private_key").and_then(toml::Value::as_str),
            Some(expected_private.as_str())
        );
        let transaction = value
            .get("transaction")
            .and_then(toml::Value::as_table)
            .expect("transaction");
        assert_eq!(
            transaction
                .get("time_to_live_ms")
                .and_then(toml::Value::as_integer),
            Some(i64::try_from(DEFAULT_TTL_MS).expect("DEFAULT_TTL_MS fits i64"))
        );
        assert_eq!(
            transaction
                .get("status_timeout_ms")
                .and_then(toml::Value::as_integer),
            Some(
                i64::try_from(DEFAULT_STATUS_TIMEOUT_MS)
                    .expect("DEFAULT_STATUS_TIMEOUT_MS fits i64"),
            )
        );
        assert_eq!(
            transaction.get("nonce").and_then(toml::Value::as_bool),
            Some(false)
        );
        let basic_auth = value
            .get("basic_auth")
            .and_then(toml::Value::as_table)
            .expect("basic_auth");
        assert_eq!(
            basic_auth.get("web_login").and_then(toml::Value::as_str),
            Some("demo")
        );
        assert_eq!(
            basic_auth.get("password").and_then(toml::Value::as_str),
            Some("secret")
        );
    }
    #[test]
    fn render_client_config_accepts_dataspace_account_scope() {
        let base = BaseConfig {
            chain: "demo-chain".to_owned(),
            network_id:
                "hash:32C903E5B3497E34C2B844EBFE8A39C19E6CF8F95D44C1FFB8BA9DCB42F91149#A2F0"
                    .parse()
                    .expect("network id"),
            torii_url: "http://127.0.0.1:8080/".to_owned(),
            basic_auth: None,
        };
        let key_pair = KeyPair::try_from_seed(b"demo-sender".to_vec(), Algorithm::Ed25519)
            .expect("seeded client key should derive");
        let rendered = render_client_config(&base, "cbuae", &key_pair).expect("render config");
        let value: toml::Value = toml::from_str(&rendered).expect("parse rendered config");
        let account = value
            .get("account")
            .and_then(toml::Value::as_table)
            .expect("account");
        assert_eq!(
            account.get("domain").and_then(toml::Value::as_str),
            Some("cbuae")
        );
    }
    #[test]
    fn render_client_config_escapes_values_and_rejects_noncanonical_scope() {
        let base = BaseConfig {
            chain: "demo\"chain\nnext".to_owned(),
            network_id:
                "hash:32C903E5B3497E34C2B844EBFE8A39C19E6CF8F95D44C1FFB8BA9DCB42F91149#A2F0"
                    .parse()
                    .expect("network id"),
            torii_url: "https://example.test/path?value=\"quoted\"".to_owned(),
            basic_auth: Some(BasicAuth {
                web_login: "operator\"name".to_owned(),
                password: Zeroizing::new("line one\nline \"two\"".to_owned()),
            }),
        };
        let key_pair = KeyPair::try_from_seed(b"escaping-client".to_vec(), Algorithm::Ed25519)
            .expect("seeded client key should derive");
        let rendered =
            render_client_config(&base, "acme.universal", &key_pair).expect("render config");
        let value: toml::Value = toml::from_str(rendered.as_str()).expect("parse rendered config");
        assert_eq!(
            value.get("chain").and_then(toml::Value::as_str),
            Some(base.chain.as_str())
        );
        let auth = value
            .get("basic_auth")
            .and_then(toml::Value::as_table)
            .expect("basic auth");
        assert_eq!(
            auth.get("password").and_then(toml::Value::as_str),
            Some(base.basic_auth.as_ref().expect("auth").password.as_str())
        );
        assert!(render_client_config(&base, "acme.universal\n[evil]", &key_pair).is_err());
        assert!(render_client_config(&base, "ACME.universal", &key_pair).is_err());
    }
    #[test]
    fn run_writes_client_configs() {
        let temp = tempfile::tempdir().expect("temp dir");
        let root = fs::canonicalize(temp.path()).expect("canonical temp dir");
        let base_path = root.join("client.toml");
        write_base_config(&base_path);
        let out_dir = root.join("clients");
        let args = Args {
            base_config: base_path.clone(),
            out_dir: Some(out_dir.clone()),
            domain: "acme.universal".to_owned(),
            seed_hex: Some("11".repeat(32)),
            names: vec!["admin1".to_owned()],
        };
        let mut writer = BufWriter::new(Vec::new());
        args.run(&mut writer).expect("run client configs");
        let config_path = out_dir.join("admin1.toml");
        assert!(config_path.exists());
    }
    #[test]
    fn invalid_account_scope_does_not_create_output_directory() {
        let temp = tempfile::tempdir().expect("temp dir");
        let root = fs::canonicalize(temp.path()).expect("canonical temp dir");
        let base_path = root.join("client.toml");
        write_base_config(&base_path);
        let out_dir = root.join("clients");
        let args = Args {
            base_config: base_path,
            out_dir: Some(out_dir.clone()),
            domain: "ACME.universal".to_owned(),
            seed_hex: Some("11".repeat(32)),
            names: vec!["admin1".to_owned()],
        };

        let _error = args
            .run(&mut BufWriter::new(Vec::new()))
            .expect_err("noncanonical account scope must fail");
        assert!(
            !out_dir.exists(),
            "validation must finish before creating the fresh custody directory"
        );
    }
    #[cfg(unix)]
    #[test]
    fn random_default_uses_distinct_keys_and_owner_only_atomic_outputs() {
        use std::os::unix::fs::PermissionsExt as _;
        let temp = tempfile::tempdir().expect("temp dir");
        let root = fs::canonicalize(temp.path()).expect("canonical temp dir");
        let base_path = root.join("client.toml");
        write_base_config(&base_path);
        let out_dir = root.join("fresh-clients");
        let args = Args {
            base_config: base_path,
            out_dir: Some(out_dir.clone()),
            domain: "cbuae".to_owned(),
            seed_hex: None,
            names: vec!["sender".to_owned(), "sponsor".to_owned()],
        };
        let mut writer = BufWriter::new(Vec::new());
        args.run(&mut writer).expect("generate fresh clients");
        assert_eq!(
            fs::metadata(&out_dir)
                .expect("out dir")
                .permissions()
                .mode()
                & 0o777,
            0o700
        );
        let sender_path = out_dir.join("sender.toml");
        let sponsor_path = out_dir.join("sponsor.toml");
        for path in [&sender_path, &sponsor_path] {
            assert_eq!(
                fs::metadata(path)
                    .expect("client config")
                    .permissions()
                    .mode()
                    & 0o777,
                0o600
            );
        }
        let sender = fs::read_to_string(&sender_path).expect("sender config");
        let sponsor = fs::read_to_string(&sponsor_path).expect("sponsor config");
        let sender_toml: toml::Value = toml::from_str(&sender).expect("sender TOML");
        let sponsor_toml: toml::Value = toml::from_str(&sponsor).expect("sponsor TOML");
        let sender_public = sender_toml["account"]["public_key"]
            .as_str()
            .expect("sender public key");
        let sponsor_public = sponsor_toml["account"]["public_key"]
            .as_str()
            .expect("sponsor public key");
        assert_ne!(sender_public, sponsor_public);
        let command_output = String::from_utf8(writer.into_inner().expect("writer bytes"))
            .expect("UTF-8 command output");
        assert!(
            !command_output.contains(
                sender_toml["account"]["private_key"]
                    .as_str()
                    .expect("sender private key")
            )
        );
        assert!(
            !command_output.contains(
                sponsor_toml["account"]["private_key"]
                    .as_str()
                    .expect("sponsor private key")
            )
        );
        assert!(!command_output.contains("secret"));
        assert_eq!(
            fs::read_dir(&out_dir)
                .expect("fresh output inventory")
                .filter_map(std::result::Result::ok)
                .count(),
            2
        );
    }
}
