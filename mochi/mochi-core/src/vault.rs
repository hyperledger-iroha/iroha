//! Signing authority vault management for MOCHI.
//!
//! The vault keeps user-provided signing authorities on disk so the composer
//! can sign transactions with real account keys instead of the bundled
//! development fixtures. First-release byte, signer, field, and JSON-graph
//! ceilings are enforced before owned decoding or filesystem mutation.
use std::{
    collections::BTreeSet,
    fs::{self, File},
    io::{self, Read, Write},
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
/// Maximum complete first-release signer-vault JSON file.
const SIGNER_VAULT_MAX_BYTES_V1: usize = 1024 * 1024;
/// Maximum signing authorities retained by one first-release MOCHI network.
const SIGNER_VAULT_MAX_SIGNERS_V1: usize = 256;
const SIGNER_VAULT_MAX_LABEL_BYTES_V1: usize = 256;
const SIGNER_VAULT_MAX_ACCOUNT_BYTES_V1: usize = 16 * 1024;
const SIGNER_VAULT_MAX_PRIVATE_KEY_BYTES_V1: usize = 16 * 1024;
const SIGNER_VAULT_MAX_ROLES_PER_SIGNER_V1: usize = 256;
const SIGNER_VAULT_MAX_ROLE_BYTES_V1: usize = 4 * 1024;
const SIGNER_VAULT_MAX_JSON_VALUES_V1: usize =
    SIGNER_VAULT_MAX_SIGNERS_V1 * (SIGNER_VAULT_MAX_ROLES_PER_SIGNER_V1 + 12 + 16);
const SIGNER_VAULT_MAX_JSON_ARRAY_ENTRIES_V1: usize =
    SIGNER_VAULT_MAX_SIGNERS_V1 * (SIGNER_VAULT_MAX_ROLES_PER_SIGNER_V1 + 12 + 1);
const SIGNER_VAULT_MAX_JSON_OBJECT_ENTRIES_V1: usize = SIGNER_VAULT_MAX_SIGNERS_V1 * 8;
const SIGNER_VAULT_MAX_JSON_DEPTH_V1: usize = 8;
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
fn read_vault_file(path: &Path) -> io::Result<Option<Vec<u8>>> {
    let mut file = match File::open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error),
    };
    let metadata = file.metadata()?;
    if !metadata.is_file() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "signer vault must be a regular file",
        ));
    }
    if metadata.len() > u64::try_from(SIGNER_VAULT_MAX_BYTES_V1).unwrap_or(u64::MAX) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("signer vault exceeds the {SIGNER_VAULT_MAX_BYTES_V1}-byte V1 limit"),
        ));
    }
    let length = usize::try_from(metadata.len()).map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "signer vault length does not fit this platform",
        )
    })?;
    let reserve = length.checked_add(1).ok_or_else(|| {
        io::Error::new(io::ErrorKind::InvalidData, "signer vault length overflow")
    })?;
    let mut bytes = Vec::new();
    bytes.try_reserve_exact(reserve).map_err(|_| {
        io::Error::new(
            io::ErrorKind::OutOfMemory,
            "failed to reserve bounded signer vault buffer",
        )
    })?;
    bytes.resize(length, 0);
    file.read_exact(&mut bytes)?;
    let mut growth_probe = [0_u8; 1];
    if file.read(&mut growth_probe)? != 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("signer vault grew beyond its metadata-approved {length}-byte length"),
        ));
    }
    Ok(Some(bytes))
}
fn signer_vault_limit(message: impl Into<String>) -> SignerVaultError {
    SignerVaultError::InvalidEntry(message.into())
}
fn ensure_string_limit(value: &str, label: &str, maximum: usize) -> Result<(), SignerVaultError> {
    if value.len() > maximum {
        return Err(signer_vault_limit(format!(
            "{label} is {} bytes (maximum {maximum})",
            value.len()
        )));
    }
    Ok(())
}
fn json_string_encoded_len(value: &str) -> Result<usize, SignerVaultError> {
    let mut encoded = 2usize;
    for character in value.chars() {
        let additional = match character {
            '"' | '\\' | '\n' | '\r' | '\t' | '\u{08}' | '\u{0C}' => 2,
            control if (control as u32) < 0x20 => 6,
            ordinary => ordinary.len_utf8(),
        };
        encoded = encoded
            .checked_add(additional)
            .ok_or_else(|| signer_vault_limit("signer vault string charge overflow"))?;
    }
    Ok(encoded)
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
        let bytes = match read_vault_file(&self.path)? {
            Some(bytes) => bytes,
            None => return Ok(Vec::new()),
        };
        json::preflight_slice(
            &bytes,
            json::JsonPreflightLimits::new(
                SIGNER_VAULT_MAX_BYTES_V1,
                SIGNER_VAULT_MAX_JSON_VALUES_V1,
                SIGNER_VAULT_MAX_PRIVATE_KEY_BYTES_V1,
                SIGNER_VAULT_MAX_PRIVATE_KEY_BYTES_V1,
                SIGNER_VAULT_MAX_BYTES_V1,
                SIGNER_VAULT_MAX_ROLES_PER_SIGNER_V1,
                SIGNER_VAULT_MAX_JSON_ARRAY_ENTRIES_V1,
                SIGNER_VAULT_MAX_JSON_OBJECT_ENTRIES_V1,
                SIGNER_VAULT_MAX_JSON_VALUES_V1,
                SIGNER_VAULT_MAX_JSON_DEPTH_V1,
            ),
        )
        .map_err(|error| signer_vault_limit(format!("vault JSON admission failed: {error}")))?;
        let value: Value = json::from_slice(&bytes)?;
        let entries = value.as_array().ok_or_else(|| {
            SignerVaultError::InvalidEntry("vault payload must be a JSON array".to_owned())
        })?;
        if entries.len() > SIGNER_VAULT_MAX_SIGNERS_V1 {
            return Err(signer_vault_limit(format!(
                "vault contains {} signers (maximum {SIGNER_VAULT_MAX_SIGNERS_V1})",
                entries.len()
            )));
        }
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
        if signers.len() > SIGNER_VAULT_MAX_SIGNERS_V1 {
            return Err(signer_vault_limit(format!(
                "cannot persist {} signers (maximum {SIGNER_VAULT_MAX_SIGNERS_V1})",
                signers.len()
            )));
        }
        let mut worst_case_json_bytes = 2usize;
        for signer in signers {
            worst_case_json_bytes = worst_case_json_bytes
                .checked_add(signer_worst_case_json_bytes(signer)?)
                .ok_or_else(|| signer_vault_limit("signer vault byte accounting overflow"))?;
            if worst_case_json_bytes > SIGNER_VAULT_MAX_BYTES_V1 {
                return Err(signer_vault_limit(format!(
                    "signer vault exceeds the {SIGNER_VAULT_MAX_BYTES_V1}-byte V1 limit"
                )));
            }
        }
        let serialized = Value::Array(
            signers
                .iter()
                .map(encode_entry)
                .collect::<Result<Vec<_>, _>>()?,
        );
        let text =
            json::to_json_bounded(&serialized, SIGNER_VAULT_MAX_BYTES_V1).map_err(|error| {
                signer_vault_limit(format!("bounded vault encoding failed: {error}"))
            })?;
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)?;
        }
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
fn signer_worst_case_json_bytes(signer: &SigningAuthority) -> Result<usize, SignerVaultError> {
    fn charge_string(
        total: &mut usize,
        value: &str,
        label: &str,
        maximum: usize,
    ) -> Result<(), SignerVaultError> {
        ensure_string_limit(value, label, maximum)?;
        let escaped = json_string_encoded_len(value)?;
        *total = total
            .checked_add(escaped)
            .ok_or_else(|| signer_vault_limit("signer vault byte accounting overflow"))?;
        Ok(())
    }
    let mut total = 512usize;
    charge_string(
        &mut total,
        signer.label(),
        "signer label",
        SIGNER_VAULT_MAX_LABEL_BYTES_V1,
    )?;
    let account = account_literal(signer.account_id());
    charge_string(
        &mut total,
        account.as_str(),
        "signer account",
        SIGNER_VAULT_MAX_ACCOUNT_BYTES_V1,
    )?;
    let private_key = ExposedPrivateKey(signer.key_pair().private_key().clone()).to_string();
    charge_string(
        &mut total,
        private_key.as_str(),
        "signer private key",
        SIGNER_VAULT_MAX_PRIVATE_KEY_BYTES_V1,
    )?;
    let permission_count = signer.permissions().count();
    if permission_count > InstructionPermission::all().len() {
        return Err(signer_vault_limit(format!(
            "signer contains {permission_count} permissions (maximum {})",
            InstructionPermission::all().len()
        )));
    }
    total = total
        .checked_add(permission_count.saturating_mul(64))
        .ok_or_else(|| signer_vault_limit("signer permission byte accounting overflow"))?;
    let mut role_count = 0usize;
    for role in signer.roles() {
        if role_count == SIGNER_VAULT_MAX_ROLES_PER_SIGNER_V1 {
            return Err(signer_vault_limit(format!(
                "signer contains more than {SIGNER_VAULT_MAX_ROLES_PER_SIGNER_V1} roles"
            )));
        }
        let role = role.to_string();
        charge_string(
            &mut total,
            role.as_str(),
            "signer role",
            SIGNER_VAULT_MAX_ROLE_BYTES_V1,
        )?;
        role_count += 1;
    }
    Ok(total)
}
fn parse_entry(entry: &Value) -> Result<SigningAuthority, SignerVaultError> {
    let object = entry.as_object().ok_or_else(|| {
        SignerVaultError::InvalidEntry("signer entry must be a JSON object".to_owned())
    })?;
    let label = extract_string(
        object,
        "label",
        "signer label",
        SIGNER_VAULT_MAX_LABEL_BYTES_V1,
    )?;
    let account_str = extract_string(
        object,
        "account",
        "signer account",
        SIGNER_VAULT_MAX_ACCOUNT_BYTES_V1,
    )?;
    let account = AccountId::parse_encoded(&account_str)
        .map(|parsed| parsed.into_account_id())
        .map_err(|err| {
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
    ensure_string_limit(
        key_str,
        "signer private key",
        SIGNER_VAULT_MAX_PRIVATE_KEY_BYTES_V1,
    )?;
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
    if array.len() > InstructionPermission::all().len() {
        return Err(signer_vault_limit(format!(
            "`permissions` contains {} entries (maximum {})",
            array.len(),
            InstructionPermission::all().len()
        )));
    }
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
    if array.len() > SIGNER_VAULT_MAX_ROLES_PER_SIGNER_V1 {
        return Err(signer_vault_limit(format!(
            "`roles` contains {} entries (maximum {SIGNER_VAULT_MAX_ROLES_PER_SIGNER_V1})",
            array.len()
        )));
    }
    let mut set = BTreeSet::new();
    for value in array {
        let item = value.as_str().ok_or_else(|| {
            SignerVaultError::InvalidEntry("`roles` entries must be role ids".to_owned())
        })?;
        ensure_string_limit(item, "signer role", SIGNER_VAULT_MAX_ROLE_BYTES_V1)?;
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
    account_id.to_string()
}
fn extract_string(
    object: &Map,
    key: &str,
    label: &str,
    maximum: usize,
) -> Result<String, SignerVaultError> {
    let value = object.get(key).and_then(Value::as_str).ok_or_else(|| {
        SignerVaultError::InvalidEntry(format!("missing or invalid `{key}` field"))
    })?;
    ensure_string_limit(value, label, maximum)?;
    Ok(value.to_owned())
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
            &NetworkProfile::from_preset(ProfilePreset::FourPeerBft),
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
    fn vault_json_string_charge_matches_canonical_escaping() {
        let value = "plain\nquote\"slash\\tab\tcontrol\u{1f}雪";
        let mut encoded = String::new();
        json::write_json_string(value, &mut encoded);
        assert_eq!(
            json_string_encoded_len(value).expect("measure bounded JSON string"),
            encoded.len()
        );
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
    fn bounded_vault_reader_accepts_exact_limit_and_rejects_first_overflow() {
        let dir = tempdir().expect("temp dir");
        let path = dir.path().join(SIGNERS_FILE_NAME);
        File::create(&path)
            .expect("create sparse exact-limit vault")
            .set_len(SIGNER_VAULT_MAX_BYTES_V1 as u64)
            .expect("size sparse exact-limit vault");
        assert_eq!(
            read_vault_file(&path)
                .expect("read exact-limit vault")
                .expect("vault exists")
                .len(),
            SIGNER_VAULT_MAX_BYTES_V1
        );
        File::create(&path)
            .expect("replace sparse vault")
            .set_len((SIGNER_VAULT_MAX_BYTES_V1 + 1) as u64)
            .expect("size sparse overflow vault");
        let error = read_vault_file(&path).expect_err("first overflow byte must fail");
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
    }
    #[test]
    fn load_rejects_signer_count_before_owned_decode() {
        let dir = tempdir().expect("temp dir");
        let path = dir.path().join(SIGNERS_FILE_NAME);
        let payload = format!(
            "[{}]",
            std::iter::repeat_n("null", SIGNER_VAULT_MAX_SIGNERS_V1 + 1)
                .collect::<Vec<_>>()
                .join(",")
        );
        fs::write(&path, payload).expect("write over-count vault");
        let error = SignerVault::from_path(path)
            .load()
            .expect_err("first signer beyond the V1 count must fail");
        assert!(error.to_string().contains("vault JSON admission failed"));
    }
    #[test]
    fn save_rejects_oversized_label_before_filesystem_mutation() {
        let dir = tempdir().expect("temp dir");
        let parent = dir.path().join("not-created");
        let vault = SignerVault::from_path(parent.join(SIGNERS_FILE_NAME));
        let signer = SigningAuthority::with_permissions(
            "x".repeat(SIGNER_VAULT_MAX_LABEL_BYTES_V1 + 1),
            ALICE_ID.clone(),
            ALICE_KEYPAIR.clone(),
            [InstructionPermission::MintAsset],
        );
        let error = vault
            .save(&[signer])
            .expect_err("oversized label must fail before persistence");
        assert!(error.to_string().contains("signer label"));
        assert!(
            !parent.exists(),
            "validation must precede directory creation"
        );
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
