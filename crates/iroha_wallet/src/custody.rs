//! Canonical wallet identities and native client configuration backed by private immutable files.

use crate::custody_fs::{PrivateDirectory, read_external, resolved_target};
use eyre::{Result, bail, eyre};
use iroha::{
    config::Config,
    data_model::{
        NetworkId,
        account::{AccountId, address::ChainDiscriminantGuard},
    },
};
use iroha_crypto::{ExposedPrivateKey, KeyPair, PrivateKey, PublicKey};
use iroha_model_base::chain::ChainId;
use norito::json;
use std::{
    fs,
    path::{Path, PathBuf},
    str::FromStr,
};
use url::Url;
use zeroize::Zeroizing;

const TAIRA_CHAIN_ID: &str = "fc56984b-2be7-431d-840e-21514d1883f0";
const MAX_KEY_BYTES: usize = 4096;
const MAX_CONFIG_BYTES: usize = 64 * 1024;
const MAX_INFO_BYTES: usize = 8192;
const MAX_WALLETS: usize = 1024;
const WALLET_SCHEMA: &str = "iroha.wallet.v1";

/// Public, exact network context retained independently of project configuration.
#[derive(
    Clone, Debug, PartialEq, Eq, norito::derive::JsonSerialize, norito::derive::JsonDeserialize,
)]
#[norito(deny_unknown_fields)]
pub struct WalletNetwork {
    /// Genesis-derived transaction and query signing domain.
    pub network_id: NetworkId,
    /// Canonical chain identity, never used as a substitute for `network_id`.
    pub chain_id: String,
    /// Public HTTP(S) Torii root without embedded credentials or query parameters.
    pub torii_url: String,
    /// Explicit account-address profile.
    pub chain_discriminant: u16,
}

impl WalletNetwork {
    /// Construct and validate public network context.
    ///
    /// # Errors
    /// Rejects invalid public endpoints, profile zero, and mismatched Taira chain/profile pairs.
    pub fn new(
        network_id: NetworkId,
        chain_id: ChainId,
        torii_url: Url,
        chain_discriminant: u16,
    ) -> Result<Self> {
        let network = Self {
            network_id,
            chain_id: chain_id.to_string(),
            torii_url: torii_url.to_string(),
            chain_discriminant,
        };
        network.validate()?;
        Ok(network)
    }

    /// Revalidate public fields received from configuration or stored JSON.
    ///
    /// # Errors
    /// Rejects noncanonical or inconsistent network context.
    pub fn validate(&self) -> Result<()> {
        let url =
            Url::parse(&self.torii_url).map_err(|_| eyre!("wallet Torii endpoint is invalid"))?;
        iroha::account_bootstrap::validate_endpoint(&url)?;
        if url.as_str() != self.torii_url {
            bail!("wallet Torii endpoint must use its canonical URL spelling");
        }
        if self.chain_id.is_empty()
            || self.chain_id.len() > 128
            || self.chain_id.trim() != self.chain_id
            || self.chain_id.chars().any(char::is_control)
            || self.chain_id.parse::<ChainId>().is_err()
        {
            bail!("wallet chain identity must be canonical and bounded");
        }
        iroha::config::resolve_account_chain_discriminant(None, Some(self.chain_discriminant))
            .map_err(|_| eyre!("wallet address profile must be nonzero"))?;
        let is_taira = self.chain_id == TAIRA_CHAIN_ID;
        if is_taira != (self.chain_discriminant == iroha_torii_shared::TAIRA_CHAIN_DISCRIMINANT) {
            bail!("Taira wallet context requires its exact chain identity and address profile 369");
        }
        Ok(())
    }
}

/// Public wallet information; this type has no signing material.
#[derive(
    Clone, Debug, PartialEq, Eq, norito::derive::JsonSerialize, norito::derive::JsonDeserialize,
)]
#[norito(deny_unknown_fields)]
pub struct WalletInfo {
    /// Canonical local wallet name.
    pub name: String,
    /// Domainless account rendered using the wallet's exact address profile.
    pub account_id: String,
    /// Canonical public signing key.
    pub public_key: String,
    /// Exact public endpoint and signing-domain binding.
    pub network: WalletNetwork,
    /// Absolute path to the reusable native client configuration, containing only a key reference.
    pub config_path: PathBuf,
}

#[derive(norito::derive::JsonSerialize, norito::derive::JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct WalletRecord {
    schema: String,
    name: String,
    public_key: String,
    network: WalletNetwork,
}

/// A private wallet collection whose contents never reside in the caller's project.
pub struct WalletStore {
    directory: PrivateDirectory,
}

/// Derive the platform's persistent user-data wallet directory without consulting a project.
///
/// macOS uses `~/Library/Application Support/Iroha/wallets`; other Unix platforms use
/// `$XDG_DATA_HOME/iroha/wallets` or `~/.local/share/iroha/wallets`.
///
/// # Errors
/// Returns an error when the platform has no convention or its user-data base is absent/relative.
pub fn default_wallet_dir() -> Result<PathBuf> {
    #[cfg(target_os = "macos")]
    {
        data_root(
            std::env::var_os("HOME").map(PathBuf::from),
            &["Library", "Application Support", "Iroha", "wallets"],
        )
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        if let Some(base) = std::env::var_os("XDG_DATA_HOME") {
            return data_root(Some(PathBuf::from(base)), &["iroha", "wallets"]);
        }
        data_root(
            std::env::var_os("HOME").map(PathBuf::from),
            &[".local", "share", "iroha", "wallets"],
        )
    }
    #[cfg(not(unix))]
    {
        bail!("private wallet custody requires a supported Unix user-data directory")
    }
}

fn data_root(base: Option<PathBuf>, components: &[&str]) -> Result<PathBuf> {
    let mut root = base.ok_or_else(|| eyre!("platform user-data directory is unavailable"))?;
    if !root.is_absolute() {
        bail!("platform user-data directory must be absolute");
    }
    for component in components {
        root.push(component);
    }
    Ok(root)
}

fn validate_name(name: &str) -> Result<()> {
    if name.is_empty()
        || name.len() > 64
        || !name.as_bytes()[0].is_ascii_lowercase()
        || name.ends_with('-')
        || !name
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
    {
        bail!(
            "wallet name must be 1–64 lowercase letters, digits, or hyphens, starting with a letter and ending with a letter or digit"
        );
    }
    Ok(())
}

impl WalletStore {
    /// Open or privately create the wallet collection outside an existing project root.
    ///
    /// # Errors
    /// Rejects project-overlapping paths, nonprivate storage, symlinks, and unsupported filesystems.
    pub fn open(root: &Path, excluded_project_root: Option<&Path>) -> Result<Self> {
        let root = resolved_target(root)?;
        if let Some(project) = excluded_project_root {
            let project = project.canonicalize()?;
            if root.starts_with(&project) || project.starts_with(&root) {
                bail!("wallet custody must be outside and disjoint from the project directory");
            }
        }
        for ancestor in root.ancestors() {
            if ancestor.join(".git").try_exists()? || ancestor.join("Musubi.toml").try_exists()? {
                bail!("wallet custody cannot reside inside a Git repository or Musubi package");
            }
        }
        Ok(Self {
            directory: PrivateDirectory::open_or_create(&root)?,
        })
    }

    /// Return the absolute persistent wallet collection directory.
    pub fn root(&self) -> &Path {
        &self.directory.path
    }

    /// Return one validated wallet's reusable native client file.
    ///
    /// # Errors
    /// Returns the same public-binding/storage failures as [`Self::show`].
    pub fn config_path(&self, name: &str) -> Result<PathBuf> {
        Ok(self.show(name)?.config_path)
    }

    /// Allocate an absent operation-journal path under this wallet's private operations directory.
    ///
    /// The operation service creates the journal exclusively before retaining a signed envelope.
    /// # Errors
    /// Rejects unsafe wallet storage, invalid operation names, or directory creation failure.
    pub fn operation_path(&self, name: &str, kind: &str) -> Result<PathBuf> {
        validate_name(kind)?;
        let (_, wallet) = self.read_record(name)?;
        let operations = wallet.ensure_child("operations")?;
        for _ in 0..16 {
            let path = operations.path.join(format!(
                "{kind}-{}",
                hex::encode(rand::random::<[u8; 16]>())
            ));
            match fs::symlink_metadata(&path) {
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                    operations.revalidate()?;
                    wallet.revalidate()?;
                    self.directory.revalidate()?;
                    return Ok(path);
                }
                Ok(_) => continue,
                Err(error) => return Err(error.into()),
            }
        }
        bail!("could not allocate a fresh wallet operation path")
    }

    /// Generate a native random signer and atomically publish a new immutable named wallet.
    ///
    /// # Errors
    /// Rejects invalid context, existing names, random-generation failure, or unsafe storage.
    pub fn create(&self, name: &str, network: &WalletNetwork) -> Result<WalletInfo> {
        validate_name(name)?;
        network.validate()?;
        let key =
            KeyPair::try_random().map_err(|_| eyre!("native wallet key generation failed"))?;
        self.install(name, network, &key)
    }

    /// Import one canonical native private key from an owner-private regular file.
    ///
    /// The source is read with no-follow, single-link and size checks. It is never printed or moved.
    /// # Errors
    /// Rejects unsafe or malformed input, conflicting names, or invalid exact network context.
    pub fn import_key_file(
        &self,
        name: &str,
        network: &WalletNetwork,
        path: &Path,
    ) -> Result<WalletInfo> {
        validate_name(name)?;
        network.validate()?;
        let bytes = read_external(&absolute_input(path)?, MAX_KEY_BYTES, true)?;
        let key = parse_key(&bytes)?;
        self.install(name, network, &key)
    }

    /// Import the exact account and network from a private native client file.
    ///
    /// Native key and network identity file sources are read securely before parsing. The new
    /// wallet uses canonical native defaults rather than copying unrelated publication sidecars.
    /// # Errors
    /// Rejects unsafe inputs, inconsistent keys/identity, or separate Basic Auth credentials.
    pub fn import_client_file(&self, name: &str, path: &Path) -> Result<WalletInfo> {
        validate_name(name)?;
        let path = absolute_input(path)?;
        let bytes = read_external(&path, MAX_CONFIG_BYTES, true)?;
        let mut table = std::str::from_utf8(&bytes)
            .ok()
            .and_then(|source| source.parse::<toml::Table>().ok())
            .ok_or_else(|| eyre!("native wallet import configuration is not valid UTF-8 TOML"))?;
        if table.contains_key("basic_auth") {
            bail!("wallet client import does not accept separate Basic Auth credentials");
        }
        resolve_import_sources(&mut table, &path)?;
        let encoded = Zeroizing::new(
            toml::to_string(&table).map_err(|_| eyre!("cannot encode native wallet import"))?,
        );
        let (config, _) = Config::load_bytes_with_musubi_publication(&path, encoded.as_bytes())
            .map_err(|_| eyre!("native wallet import configuration or signer is invalid"))?;
        let network = WalletNetwork::new(
            config.network_id,
            config.chain,
            config.torii_api_url,
            config.account_chain_discriminant,
        )?;
        self.install(name, &network, &config.key_pair)
    }

    /// Inspect one public wallet identity without reading its private key.
    ///
    /// # Errors
    /// Rejects an absent, malformed, substituted, or unsafe wallet directory/configuration.
    pub fn show(&self, name: &str) -> Result<WalletInfo> {
        let (record, _) = self.read_record(name)?;
        record.info(self.directory.path.join(name).join("client.toml"))
    }

    /// List public identities in canonical name order without reading any private keys.
    ///
    /// # Errors
    /// Rejects malformed published wallets, changed paths, or collections exceeding 1,024 entries.
    pub fn list(&self) -> Result<Vec<WalletInfo>> {
        self.directory.revalidate()?;
        let mut names = Vec::new();
        for (index, entry) in fs::read_dir(&self.directory.path)?.enumerate() {
            if index >= MAX_WALLETS {
                bail!("wallet collection exceeds its fixed entry bound");
            }
            let entry = entry?;
            let name = entry
                .file_name()
                .into_string()
                .map_err(|_| eyre!("wallet name is not UTF-8"))?;
            if name.starts_with(".pending-") {
                continue;
            }
            validate_name(&name)?;
            names.push(name);
        }
        names.sort_unstable();
        let result = names
            .iter()
            .map(|name| self.show(name))
            .collect::<Result<Vec<_>>>()?;
        self.directory.revalidate()?;
        Ok(result)
    }

    /// Load the wallet's exact native signing configuration from retained no-follow descriptors.
    ///
    /// The persisted configuration references `private.key`; the native configuration reader
    /// enforces the same no-follow, owner-private, single-link bounded key-file policy.
    /// # Errors
    /// Rejects unsafe or changed storage and key/account/network substitutions.
    pub fn load_config(&self, name: &str) -> Result<Config> {
        let (record, directory) = self.read_record(name)?;
        let source = render_config(&record, None)?;
        let (config, _) = Config::load_bytes_with_musubi_publication(
            directory.path.join("client.toml"),
            source.as_bytes(),
        )
        .map_err(|_| eyre!("retained native wallet configuration is invalid"))?;
        if config.network_id != record.network.network_id
            || config.chain.to_string() != record.network.chain_id
            || config.torii_api_url.as_str() != record.network.torii_url
            || config.account_chain_discriminant != record.network.chain_discriminant
            || config.key_pair.public_key().to_string() != record.public_key
        {
            bail!("native wallet configuration differs from its exact public binding");
        }
        directory.revalidate()?;
        self.directory.revalidate()?;
        Ok(config)
    }

    fn read_record(&self, name: &str) -> Result<(WalletRecord, PrivateDirectory)> {
        validate_name(name)?;
        let directory = self.directory.child(name, false)?;
        let bytes = directory.read("wallet.json", MAX_INFO_BYTES)?;
        let record: WalletRecord =
            json::from_slice(&bytes).map_err(|_| eyre!("wallet public record is invalid"))?;
        if record.schema != WALLET_SCHEMA
            || record.name != name
            || json::to_vec(&record).map_err(|_| eyre!("wallet record cannot be encoded"))?
                != *bytes
        {
            bail!("wallet record must retain its canonical schema, name and encoding");
        }
        record.network.validate()?;
        let config = directory.read("client.toml", MAX_CONFIG_BYTES)?;
        if config.as_slice() != render_config(&record, None)?.as_bytes() {
            bail!("wallet client configuration differs from its immutable public binding");
        }
        self.directory.revalidate()?;
        Ok((record, directory))
    }

    fn install(&self, name: &str, network: &WalletNetwork, key: &KeyPair) -> Result<WalletInfo> {
        let record = WalletRecord {
            schema: WALLET_SCHEMA.to_owned(),
            name: name.to_owned(),
            public_key: key.public_key().to_string(),
            network: network.clone(),
        };
        let public_bytes =
            json::to_vec(&record).map_err(|_| eyre!("cannot encode wallet identity"))?;
        let client = render_config(&record, None)?;
        let pending = format!(".pending-{}", hex::encode(rand::random::<[u8; 16]>()));
        let directory = self.directory.child(&pending, true)?;
        let result = (|| -> Result<()> {
            let secret = Zeroizing::new(format!(
                "{}\n",
                ExposedPrivateKey(key.private_key().clone())
            ));
            directory.write_new("private.key", secret.as_bytes())?;
            directory.write_new("client.toml", client.as_bytes())?;
            directory.write_new("wallet.json", &public_bytes)?;
            self.directory.publish(&directory, name)
        })();
        if let Err(error) = result {
            // Remove only this still-unpublished private directory. A completed rename is never
            // rolled back merely because a later directory sync reported an error.
            if directory.revalidate().is_ok() {
                self.directory.remove_pending(&directory)?;
            }
            return Err(error);
        }
        self.show(name)
    }
}

impl WalletRecord {
    fn info(&self, config_path: PathBuf) -> Result<WalletInfo> {
        let key = self
            .public_key
            .parse::<PublicKey>()
            .map_err(|_| eyre!("wallet public key is invalid"))?;
        if key.to_string() != self.public_key {
            bail!("wallet public key is not canonical");
        }
        let _profile = ChainDiscriminantGuard::enter(self.network.chain_discriminant);
        Ok(WalletInfo {
            name: self.name.clone(),
            account_id: AccountId::of(key).to_string(),
            public_key: self.public_key.clone(),
            network: self.network.clone(),
            config_path,
        })
    }
}

fn render_config(record: &WalletRecord, key: Option<&str>) -> Result<String> {
    let quote = |value: &str| toml::Value::String(value.to_owned()).to_string();
    let mut config = format!(
        "chain = {}\nnetwork_id = {}\ntorii_url = {}\n\n[account]\ndomain = \"universal\"\nchain_discriminant = {}\npublic_key = {}\n",
        quote(&record.network.chain_id),
        quote(&record.network.network_id.to_string()),
        quote(&record.network.torii_url),
        record.network.chain_discriminant,
        quote(&record.public_key),
    );
    match key {
        Some(key) => config.push_str(&format!("private_key = {}\n", quote(key))),
        None => config.push_str("private_key_file = \"private.key\"\n"),
    }
    if config.len() > MAX_CONFIG_BYTES {
        bail!("wallet configuration exceeds its size bound");
    }
    Ok(config)
}

fn parse_key(bytes: &[u8]) -> Result<KeyPair> {
    let source = std::str::from_utf8(bytes)
        .map_err(|_| eyre!("private key input is not canonical UTF-8"))?;
    let source = source.strip_suffix('\n').unwrap_or(source);
    if source.is_empty() || source.contains(['\n', '\r']) {
        bail!("private key input must contain exactly one canonical key");
    }
    let key = PrivateKey::from_str(source).map_err(|_| eyre!("private key input is invalid"))?;
    if ExposedPrivateKey(key.clone()).to_string() != source {
        bail!("private key input is not canonical");
    }
    KeyPair::from_private_key(key).map_err(|_| eyre!("private key cannot derive a native signer"))
}

fn absolute_input(path: &Path) -> Result<PathBuf> {
    Ok(if path.is_absolute() {
        path.to_owned()
    } else {
        std::env::current_dir()?.join(path)
    })
}

fn resolve_import_sources(table: &mut toml::Table, path: &Path) -> Result<()> {
    let relative = |value: &str| -> Result<PathBuf> {
        let source = Path::new(value);
        Ok(if source.is_absolute() {
            source.to_owned()
        } else {
            path.parent()
                .ok_or_else(|| eyre!("invalid native client path"))?
                .join(source)
        })
    };
    if let Some(value) = table.remove("network_id_file") {
        if table.contains_key("network_id") {
            bail!("native wallet import has conflicting network identity sources");
        }
        let file = value
            .as_str()
            .ok_or_else(|| eyre!("network identity file must be a path"))?;
        let bytes = read_external(&relative(file)?, 512, false)?;
        let source = std::str::from_utf8(&bytes)
            .ok()
            .and_then(|source| source.strip_suffix('\n'))
            .ok_or_else(|| {
                eyre!("network identity file must contain one LF-terminated identity")
            })?;
        let id = source
            .parse::<NetworkId>()
            .map_err(|_| eyre!("network identity file is invalid"))?;
        if id.to_string() != source {
            bail!("network identity file is not canonical");
        }
        table.insert(
            "network_id".to_owned(),
            toml::Value::String(source.to_owned()),
        );
    }
    let account = table
        .get_mut("account")
        .and_then(toml::Value::as_table_mut)
        .ok_or_else(|| eyre!("native client account configuration is missing"))?;
    if let Some(value) = account.remove("private_key_file") {
        if account.contains_key("private_key") {
            bail!("native wallet import has conflicting private key sources");
        }
        let file = value
            .as_str()
            .ok_or_else(|| eyre!("private key file must be a path"))?;
        let bytes = read_external(&relative(file)?, MAX_KEY_BYTES, true)?;
        let key = parse_key(&bytes)?;
        account.insert(
            "private_key".to_owned(),
            toml::Value::String(ExposedPrivateKey(key.private_key().clone()).to_string()),
        );
    }
    Ok(())
}

#[cfg(all(test, unix))]
#[path = "custody_tests.rs"]
mod tests;
