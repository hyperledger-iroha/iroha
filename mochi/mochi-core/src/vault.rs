//! Signing authority vault management for MOCHI.
//!
//! The vault keeps user-provided signing authorities on disk so the composer
//! can sign transactions with real account keys instead of the bundled
//! development fixtures.

use std::{
    collections::BTreeSet,
    fs::{self, File},
    io::{self, Write},
    path::{Path, PathBuf},
    str::FromStr,
};

use iroha_crypto::{ExposedPrivateKey, KeyPair, PrivateKey};
use iroha_data_model::{account::AccountId, role::RoleId};
use norito::json::{self, Map, Value};

use crate::{
    compose::{InstructionPermission, SigningAuthority, development_signing_authorities},
    config::NetworkPaths,
};

/// Canonical filename storing signer metadata beneath a network root.
pub const SIGNERS_FILE_NAME: &str = "signers.json";

/// Errors emitted when reading or writing signing authorities.
#[derive(Debug, thiserror::Error)]
pub enum SignerVaultError {
    /// Wrapper for filesystem failures.
    #[error("io error: {0}")]
    Io(#[from] io::Error),
    /// Wrapper for JSON encoding/decoding failures.
    #[error("json error: {0}")]
    Json(#[from] norito::json::Error),
    /// Invalid or unsupported signer entry detected.
    #[error("invalid signer entry: {0}")]
    InvalidEntry(String),
}

/// Helper to expose the on-disk `signers.json` layout.
#[derive(Debug, Clone)]
pub struct SignerVault {
    path: PathBuf,
}

impl SignerVault {
    /// Create a vault handle rooted under the provided network paths.
    #[must_use]
    pub fn new(paths: &NetworkPaths) -> Self {
        Self {
            path: paths.root().join(SIGNERS_FILE_NAME),
        }
    }

    /// Create a vault handle for an explicit file path.
    #[must_use]
    pub fn from_path(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    /// Path to the underlying `signers.json`.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Whether the vault file exists on disk.
    #[must_use]
    pub fn exists(&self) -> bool {
        self.path.exists()
    }

    /// Load signing authorities from disk without applying fallbacks.
    ///
    /// Returns an empty list when the vault file is absent.
    pub fn load(&self) -> Result<Vec<SigningAuthority>, SignerVaultError> {
        let bytes = match fs::read(&self.path) {
            Ok(bytes) => bytes,
            Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(err) => return Err(err.into()),
        };
        let value: Value = json::from_slice(&bytes)?;
        let entries = value.as_array().ok_or_else(|| {
            SignerVaultError::InvalidEntry("vault payload must be a JSON array".to_owned())
        })?;

        let mut signers = Vec::with_capacity(entries.len());
        for entry in entries {
            match parse_entry(entry) {
                Ok(signer) => signers.push(signer),
                Err(err) => return Err(err),
            }
        }
        Ok(signers)
    }

    /// Load signing authorities, falling back to development fixtures when unavailable.
    #[must_use]
    pub fn load_with_fallback(&self) -> Vec<SigningAuthority> {
        match self.load() {
            Ok(signers) if !signers.is_empty() => signers,
            Ok(_) => development_signing_authorities().to_vec(),
            Err(err) => {
                eprintln!(
                    "MOCHI: failed to load signing vault {}: {err}",
                    self.path.display()
                );
                development_signing_authorities().to_vec()
            }
        }
    }

    /// Persist the provided signing authorities to disk, replacing the existing vault.
    pub fn save(&self, signers: &[SigningAuthority]) -> Result<(), SignerVaultError> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)?;
        }
        let serialized = Value::Array(
            signers
                .iter()
                .map(encode_entry)
                .collect::<Result<Vec<_>, _>>()?,
        );
        let text = json::to_string_pretty(&serialized)?;
        let tmp_path = self.path.with_extension("json.tmp");
        if tmp_path.exists() {
            fs::remove_file(&tmp_path)?;
        }
        {
            let mut file = File::create(&tmp_path)?;
            file.write_all(text.as_bytes())?;
            file.sync_all()?;
        }
        if let Err(err) = fs::rename(&tmp_path, &self.path) {
            let _ = fs::remove_file(&tmp_path);
            return Err(err.into());
        }
        Ok(())
    }
}

fn parse_entry(entry: &Value) -> Result<SigningAuthority, SignerVaultError> {
    let object = entry.as_object().ok_or_else(|| {
        SignerVaultError::InvalidEntry("signer entry must be a JSON object".to_owned())
    })?;

    let label = extract_string(object, "label")?;
    let account_str = extract_string(object, "account")?;
    let account = AccountId::from_str(&account_str).map_err(|err| {
        SignerVaultError::InvalidEntry(format!("invalid account id `{account_str}`: {err}"))
    })?;

    let key_field = object
        .get("private_key")
        .or_else(|| object.get("privateKey"))
        .or_else(|| object.get("private_key_hex"))
        .ok_or_else(|| SignerVaultError::InvalidEntry("missing `private_key` field".to_owned()))?;
    let key_str = key_field.as_str().ok_or_else(|| {
        SignerVaultError::InvalidEntry("`private_key` must be a string".to_owned())
    })?;
    let private_key = PrivateKey::from_str(key_str).map_err(|err| {
        SignerVaultError::InvalidEntry(format!("failed to parse private key: {err}"))
    })?;
    let key_pair = KeyPair::from_private_key(private_key).map_err(|err| {
        SignerVaultError::InvalidEntry(format!("failed to construct key pair: {err}"))
    })?;

    let permissions = parse_permissions(object)?;
    let roles = parse_roles(object)?;
    Ok(SigningAuthority::with_permissions_and_roles(
        label,
        account,
        key_pair,
        permissions,
        roles,
    ))
}

fn parse_permissions(object: &Map) -> Result<BTreeSet<InstructionPermission>, SignerVaultError> {
    let Some(raw) = object.get("permissions") else {
        return Ok(InstructionPermission::all().into_iter().collect());
    };
    let array = raw.as_array().ok_or_else(|| {
        SignerVaultError::InvalidEntry("`permissions` must be an array".to_owned())
    })?;
    let mut set = BTreeSet::new();
    for value in array {
        let item = value.as_str().ok_or_else(|| {
            SignerVaultError::InvalidEntry(
                "`permissions` entries must be permission keys".to_owned(),
            )
        })?;
        let Some(permission) = InstructionPermission::from_key(item) else {
            return Err(SignerVaultError::InvalidEntry(format!(
                "unknown permission `{item}`"
            )));
        };
        set.insert(permission);
    }
    if set.is_empty() {
        return Err(SignerVaultError::InvalidEntry(
            "`permissions` list must not be empty".to_owned(),
        ));
    }
    Ok(set)
}

fn parse_roles(object: &Map) -> Result<BTreeSet<RoleId>, SignerVaultError> {
    let Some(raw) = object.get("roles") else {
        return Ok(BTreeSet::new());
    };
    let array = raw
        .as_array()
        .ok_or_else(|| SignerVaultError::InvalidEntry("`roles` must be an array".to_owned()))?;
    let mut set = BTreeSet::new();
    for value in array {
        let item = value.as_str().ok_or_else(|| {
            SignerVaultError::InvalidEntry("`roles` entries must be role ids".to_owned())
        })?;
        let role = RoleId::from_str(item).map_err(|err| {
            SignerVaultError::InvalidEntry(format!("invalid role id `{item}`: {err}"))
        })?;
        set.insert(role);
    }
    Ok(set)
}

fn encode_entry(signer: &SigningAuthority) -> Result<Value, SignerVaultError> {
    let mut object = Map::new();
    object.insert("label".into(), Value::from(signer.label().to_owned()));
    object.insert(
        "account".into(),
        Value::from(account_literal(signer.account_id())),
    );
    let private_key = signer.key_pair().private_key().clone();
    let exposed = ExposedPrivateKey(private_key);
    object.insert("private_key".into(), Value::from(exposed.to_string()));
    let permissions: Vec<_> = signer
        .permissions()
        .map(|permission| Value::from(permission.key()))
        .collect();
    object.insert("permissions".into(), Value::Array(permissions));
    let roles: Vec<_> = signer
        .roles()
        .map(|role| Value::from(role.to_string()))
        .collect();
    object.insert("roles".into(), Value::Array(roles));
    Ok(Value::Object(object))
}

fn account_literal(account_id: &AccountId) -> String {
    format!("{account_id}@{}", account_id.domain())
}

fn extract_string(object: &Map, key: &str) -> Result<String, SignerVaultError> {
    object
        .get(key)
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
        .ok_or_else(|| SignerVaultError::InvalidEntry(format!("missing or invalid `{key}` field")))
}

#[cfg(test)]
mod tests {
    use iroha_test_samples::{ALICE_ID, ALICE_KEYPAIR};
    use tempfile::tempdir;

    use super::*;
    use crate::config::{NetworkProfile, ProfilePreset};

    fn dummy_paths(root: &Path) -> NetworkPaths {
        NetworkPaths::from_root(
            root,
            &NetworkProfile::from_preset(ProfilePreset::SinglePeer),
        )
    }

    #[test]
    fn vault_roundtrip_preserves_signers() {
        let dir = tempdir().expect("temp dir");
        let paths = dummy_paths(dir.path());
        paths.ensure().expect("ensure directories");
        let vault = SignerVault::new(&paths);

        let role: RoleId = "basic_user".parse().expect("role id");
        let custom_signer = SigningAuthority::with_permissions_and_roles(
            "Alice custom",
            ALICE_ID.clone(),
            ALICE_KEYPAIR.clone(),
            [InstructionPermission::MintAsset],
            [role.clone()],
        );

        let signers = vec![custom_signer];
        vault.save(&signers).expect("save vault");

        let loaded = vault.load().expect("load vault");
        assert_eq!(loaded.len(), 1);
        let loaded_signer = &loaded[0];
        assert_eq!(loaded_signer.label(), "Alice custom");
        assert_eq!(
            loaded_signer.account_id(),
            signers[0].account_id(),
            "account id should persist"
        );
        let expected_key =
            ExposedPrivateKey(signers[0].key_pair().private_key().clone()).to_string();
        let actual_key =
            ExposedPrivateKey(loaded_signer.key_pair().private_key().clone()).to_string();
        assert_eq!(expected_key, actual_key, "private key should roundtrip");
        let permissions: Vec<_> = loaded_signer.permissions().collect();
        assert_eq!(
            permissions,
            vec![InstructionPermission::MintAsset],
            "permission set should persist"
        );
        let roles: Vec<_> = loaded_signer.roles().collect();
        assert_eq!(roles, vec![&role], "role list should persist");
    }

    #[test]
    fn load_missing_vault_produces_empty_list() {
        let dir = tempdir().expect("temp dir");
        let paths = dummy_paths(dir.path());
        paths.ensure().expect("ensure directories");
        let vault = SignerVault::new(&paths);
        let loaded = vault.load().expect("load missing vault returns Ok");
        assert!(loaded.is_empty(), "missing vault should return empty set");
    }

    #[test]
    fn save_replaces_existing_vault_atomically() {
        let dir = tempdir().expect("temp dir");
        let paths = dummy_paths(dir.path());
        paths.ensure().expect("ensure directories");
        let vault = SignerVault::new(&paths);

        let first = SigningAuthority::with_permissions(
            "first signer",
            ALICE_ID.clone(),
            ALICE_KEYPAIR.clone(),
            [InstructionPermission::MintAsset],
        );
        vault.save(&[first]).expect("save initial vault");
        let initial = vault.load().expect("load initial vault");
        assert_eq!(initial.len(), 1);
        assert_eq!(initial[0].label(), "first signer");

        let second = SigningAuthority::with_permissions(
            "second signer",
            ALICE_ID.clone(),
            ALICE_KEYPAIR.clone(),
            [InstructionPermission::TransferAsset],
        );
        vault.save(&[second]).expect("replace vault contents");
        let replaced = vault.load().expect("load replaced vault");
        assert_eq!(replaced.len(), 1);
        assert_eq!(replaced[0].label(), "second signer");
        let permissions: Vec<_> = replaced[0].permissions().collect();
        assert_eq!(
            permissions,
            vec![InstructionPermission::TransferAsset],
            "updated permissions should persist"
        );
        let tmp_path = vault.path().with_extension("json.tmp");
        assert!(
            !tmp_path.exists(),
            "temporary vault file should be removed after rename"
        );
    }
}
