//! Signing authority vault management for MOCHI.
//!
//! The vault keeps user-provided signing authorities on disk so the composer can sign transactions
//! with real account keys instead of the bundled development fixtures. First-release byte, signer,
//! field, and JSON-graph ceilings are enforced before owned decoding or filesystem mutation.
use crate::{
    compose::{InstructionPermission, SigningAuthority, development_signing_authorities},
    config::NetworkPaths,
};
use iroha_crypto::{ExposedPrivateKey, KeyPair, PrivateKey};
use iroha_data_model::{account::AccountId, role::RoleId};
use norito::json::{self, Map, Value};
use zeroize::Zeroizing;
#[cfg(unix)]
use std::os::unix::fs::{MetadataExt as _, OpenOptionsExt as _};
use std::{
    collections::BTreeSet,
    fs::{self, File, OpenOptions},
    io::{self, Read, Write},
    path::{Path, PathBuf},
    str::FromStr,
    sync::atomic::{AtomicU64, Ordering},
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
const SIGNER_VAULT_ENTRY_FIELDS_V1: [&str; 5] =
    ["label", "account", "private_key", "permissions", "roles"];
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
    let named_metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error),
    };
    if named_metadata.file_type().is_symlink() || !named_metadata.is_file() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "signer vault must be a regular file, not a symlink",
        ));
    }
    #[cfg(unix)]
    validate_vault_metadata(path, &named_metadata)?;
    let mut file = match File::open(path) {
        Ok(file) => file,
        Err(error) => return Err(error),
    };
    let metadata = file.metadata()?;
    if !metadata.is_file() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "signer vault must be a regular file",
        ));
    }
    #[cfg(unix)]
    {
        validate_vault_metadata(path, &metadata)?;
        if named_metadata.dev() != metadata.dev() || named_metadata.ino() != metadata.ino() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "signer vault changed while it was being opened",
            ));
        }
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
#[cfg(unix)]
fn validate_vault_metadata(path: &Path, metadata: &fs::Metadata) -> io::Result<()> {
    if metadata.nlink() != 1 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "signer vault must have exactly one filesystem link",
        ));
    }
    let mode = metadata.mode() & 0o777;
    if mode & 0o077 != 0 || mode & 0o400 == 0 {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            format!(
                "signer vault `{}` must be owner-readable and inaccessible to group/other users",
                path.display()
            ),
        ));
    }
    if let Some(parent) = path.parent() {
        let parent_metadata = fs::metadata(parent)?;
        if metadata.uid() != parent_metadata.uid() {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "signer vault owner must match its parent directory owner",
            ));
        }
    }
    Ok(())
}
fn create_vault_temp(path: &Path) -> io::Result<(PathBuf, File)> {
    static NEXT_TEMP_ID: AtomicU64 = AtomicU64::new(0);
    for _ in 0..32 {
        let id = NEXT_TEMP_ID.fetch_add(1, Ordering::Relaxed);
        let tmp_path = path.with_extension(format!("json.tmp.{}.{id}", std::process::id()));
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        options.mode(0o600);
        match options.open(&tmp_path) {
            Ok(file) => return Ok((tmp_path, file)),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error),
        }
    }
    Err(io::Error::new(
        io::ErrorKind::AlreadyExists,
        "could not allocate a unique signer-vault temporary file",
    ))
}
fn prepare_vault_parent(path: &Path) -> io::Result<PathBuf> {
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent)?;
    let metadata = fs::symlink_metadata(parent)?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "signer vault parent `{}` must be a real directory",
                parent.display()
            ),
        ));
    }
    #[cfg(unix)]
    if metadata.mode() & 0o022 != 0 {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            format!(
                "signer vault parent `{}` must not be writable by group or other users",
                parent.display()
            ),
        ));
    }
    Ok(parent.to_path_buf())
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
    /// Load persisted signing authorities, using development fixtures only when no vault exists.
    pub fn load_or_development(&self) -> Result<Vec<SigningAuthority>, SignerVaultError> {
        match fs::symlink_metadata(&self.path) {
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                Ok(development_signing_authorities().to_vec())
            }
            Err(error) => Err(error.into()),
            Ok(_) => {
                let signers = self.load()?;
                if signers.is_empty() {
                    return Err(SignerVaultError::InvalidEntry(
                        "persisted signer vault must contain at least one signer".to_owned(),
                    ));
                }
                Ok(signers)
            }
        }
    }
    /// Persist the provided signing authorities to disk, replacing the existing vault.
    pub fn save(&self, signers: &[SigningAuthority]) -> Result<(), SignerVaultError> {
        if signers.is_empty() {
            return Err(signer_vault_limit(
                "cannot persist an empty signer vault; remove the file to use development signers",
            ));
        }
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
        let parent = prepare_vault_parent(&self.path)?;
        let serialized = Value::Array(
            signers
                .iter()
                .map(encode_entry)
                .collect::<Result<Vec<_>, _>>()?,
        );
        let text = Zeroizing::new(
            json::to_json_bounded(&serialized, SIGNER_VAULT_MAX_BYTES_V1).map_err(|error| {
                signer_vault_limit(format!("bounded vault encoding failed: {error}"))
            })?,
        );
        let (tmp_path, mut file) = create_vault_temp(&self.path)?;
        let write_result = (|| -> io::Result<()> {
            file.write_all(text.as_bytes())?;
            file.sync_all()
        })();
        if let Err(error) = write_result {
            let _ = fs::remove_file(&tmp_path);
            return Err(error.into());
        }
        if let Err(err) = fs::rename(&tmp_path, &self.path) {
            let _ = fs::remove_file(&tmp_path);
            return Err(err.into());
        }
        #[cfg(unix)]
        File::open(parent)?.sync_all()?;
        #[cfg(not(unix))]
        let _ = parent;
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
    let private_key = Zeroizing::new(
        ExposedPrivateKey(signer.key_pair().private_key().clone()).to_string(),
    );
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
    for (role_count, role) in signer.roles().enumerate() {
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
    }
    Ok(total)
}
fn parse_entry(entry: &Value) -> Result<SigningAuthority, SignerVaultError> {
    let object = entry.as_object().ok_or_else(|| {
        SignerVaultError::InvalidEntry("signer entry must be a JSON object".to_owned())
    })?;
    if let Some(field) = object
        .keys()
        .find(|field| !SIGNER_VAULT_ENTRY_FIELDS_V1.contains(&field.as_str()))
    {
        return Err(SignerVaultError::InvalidEntry(format!(
            "unknown signer field `{field}`"
        )));
    }
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
    let account = AccountId::parse_encoded(&account_str).map_err(|err| {
        SignerVaultError::InvalidEntry(format!("invalid account id `{account_str}`: {err}"))
    })?;
    let key_field = object
        .get("private_key")
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
    let raw = object
        .get("permissions")
        .ok_or_else(|| SignerVaultError::InvalidEntry("missing `permissions` field".to_owned()))?;
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
        if !set.insert(permission) {
            return Err(SignerVaultError::InvalidEntry(format!(
                "duplicate permission `{item}`"
            )));
        }
    }
    if set.is_empty() {
        return Err(SignerVaultError::InvalidEntry(
            "`permissions` list must not be empty".to_owned(),
        ));
    }
    Ok(set)
}
fn parse_roles(object: &Map) -> Result<BTreeSet<RoleId>, SignerVaultError> {
    let raw = object
        .get("roles")
        .ok_or_else(|| SignerVaultError::InvalidEntry("missing `roles` field".to_owned()))?;
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
        if !set.insert(role) {
            return Err(SignerVaultError::InvalidEntry(format!(
                "duplicate role `{item}`"
            )));
        }
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
    use super::*;
    use crate::config::{NetworkProfile, ProfilePreset};
    use iroha_test_samples::{ALICE_ID, ALICE_KEYPAIR};
    use tempfile::tempdir;
    fn dummy_paths(root: &Path) -> NetworkPaths {
        NetworkPaths::from_root(
            root,
            &NetworkProfile::from_preset(ProfilePreset::FourPeerBft),
        )
    }
    fn create_private_test_file(path: &Path) -> File {
        let mut options = OpenOptions::new();
        options.write(true).create(true).truncate(true);
        #[cfg(unix)]
        options.mode(0o600);
        options.open(path).expect("create private test vault")
    }
    fn write_private_test_file(path: &Path, bytes: impl AsRef<[u8]>) {
        let mut file = create_private_test_file(path);
        file.write_all(bytes.as_ref())
            .expect("write private test vault");
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
    fn parser_requires_the_exact_canonical_field_set() {
        let signer = &development_signing_authorities()[0];
        let Value::Object(mut object) = encode_entry(signer).expect("encode development signer")
        else {
            panic!("encoded signer must be an object");
        };
        let private_key = object
            .remove("private_key")
            .expect("canonical private-key field");
        object.insert("privateKey".to_owned(), private_key.clone());
        let error = parse_entry(&Value::Object(object.clone()))
            .expect_err("legacy private-key aliases must be rejected");
        assert!(
            error
                .to_string()
                .contains("unknown signer field `privateKey`")
        );

        object.remove("privateKey");
        object.insert("private_key".to_owned(), private_key);
        object.remove("permissions");
        let error =
            parse_entry(&Value::Object(object.clone())).expect_err("permissions must be explicit");
        assert!(error.to_string().contains("missing `permissions`"));

        object.insert(
            "permissions".to_owned(),
            Value::Array(vec![Value::from(InstructionPermission::MintAsset.key())]),
        );
        object.remove("roles");
        let error = parse_entry(&Value::Object(object)).expect_err("roles must be explicit");
        assert!(error.to_string().contains("missing `roles`"));
    }
    #[test]
    fn parser_rejects_duplicate_permissions_and_roles() {
        let signer = &development_signing_authorities()[0];
        let Value::Object(mut object) = encode_entry(signer).expect("encode development signer")
        else {
            panic!("encoded signer must be an object");
        };
        let permission = Value::from(InstructionPermission::MintAsset.key());
        object.insert(
            "permissions".to_owned(),
            Value::Array(vec![permission.clone(), permission]),
        );
        let error = parse_entry(&Value::Object(object.clone()))
            .expect_err("duplicate permissions must not be normalized");
        assert!(error.to_string().contains("duplicate permission"));

        object.insert(
            "permissions".to_owned(),
            Value::Array(vec![Value::from(InstructionPermission::MintAsset.key())]),
        );
        object.insert(
            "roles".to_owned(),
            Value::Array(vec![Value::from("basic_user"), Value::from("basic_user")]),
        );
        let error = parse_entry(&Value::Object(object))
            .expect_err("duplicate roles must not be normalized");
        assert!(error.to_string().contains("duplicate role"));
    }
    #[test]
    fn load_missing_vault_produces_empty_list() {
        let dir = tempdir().expect("temp dir");
        let paths = dummy_paths(dir.path());
        paths.ensure().expect("ensure directories");
        let vault = SignerVault::new(&paths);
        let loaded = vault.load().expect("load missing vault returns Ok");
        assert!(loaded.is_empty(), "missing vault should return empty set");
        assert_eq!(
            vault
                .load_or_development()
                .expect("missing vault uses development signers")
                .len(),
            development_signing_authorities().len()
        );
    }
    #[test]
    fn bounded_vault_reader_accepts_exact_limit_and_rejects_first_overflow() {
        let dir = tempdir().expect("temp dir");
        let path = dir.path().join(SIGNERS_FILE_NAME);
        create_private_test_file(&path)
            .set_len(SIGNER_VAULT_MAX_BYTES_V1 as u64)
            .expect("size sparse exact-limit vault");
        assert_eq!(
            read_vault_file(&path)
                .expect("read exact-limit vault")
                .expect("vault exists")
                .len(),
            SIGNER_VAULT_MAX_BYTES_V1
        );
        create_private_test_file(&path)
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
        write_private_test_file(&path, payload);
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
        let file_name = vault
            .path()
            .file_name()
            .and_then(|name| name.to_str())
            .expect("UTF-8 vault filename");
        assert!(
            fs::read_dir(vault.path().parent().expect("vault parent"))
                .expect("list vault parent")
                .filter_map(Result::ok)
                .all(|entry| !entry
                    .file_name()
                    .to_string_lossy()
                    .starts_with(&format!("{file_name}.tmp."))),
            "temporary vault file should be removed after rename"
        );
    }
    #[test]
    fn existing_empty_vault_does_not_fall_back_to_development_keys() {
        let dir = tempdir().expect("temp dir");
        let path = dir.path().join(SIGNERS_FILE_NAME);
        write_private_test_file(&path, b"[]");
        let error = SignerVault::from_path(path)
            .load_or_development()
            .expect_err("an existing empty vault must fail closed");
        assert!(error.to_string().contains("at least one signer"));
    }
    #[test]
    fn save_rejects_empty_vault() {
        let dir = tempdir().expect("temp dir");
        let path = dir.path().join(SIGNERS_FILE_NAME);
        let error = SignerVault::from_path(&path)
            .save(&[])
            .expect_err("empty signer vault must be rejected");
        assert!(error.to_string().contains("empty signer vault"));
        assert!(!path.exists());
    }
    #[cfg(unix)]
    #[test]
    fn vault_is_owner_only_and_symlinks_are_rejected() {
        use std::os::unix::fs::symlink;

        let dir = tempdir().expect("temp dir");
        let path = dir.path().join(SIGNERS_FILE_NAME);
        let vault = SignerVault::from_path(&path);
        vault
            .save(development_signing_authorities())
            .expect("save private vault");
        let mode = fs::metadata(&path).expect("vault metadata").mode() & 0o777;
        assert_eq!(mode & 0o077, 0, "group and other bits must be clear");

        let link_path = dir.path().join("linked-signers.json");
        symlink(&path, &link_path).expect("create vault symlink");
        let error = SignerVault::from_path(link_path)
            .load()
            .expect_err("vault symlinks must be rejected");
        assert!(error.to_string().contains("not a symlink"));
    }
    #[cfg(unix)]
    #[test]
    fn save_rejects_a_symlinked_parent_directory() {
        use std::os::unix::fs::symlink;

        let root = tempdir().expect("vault root");
        let outside = tempdir().expect("outside target");
        let linked_parent = root.path().join("linked-parent");
        symlink(outside.path(), &linked_parent).expect("create parent symlink");
        let vault = SignerVault::from_path(linked_parent.join(SIGNERS_FILE_NAME));
        let error = vault
            .save(development_signing_authorities())
            .expect_err("a vault parent symlink must fail closed");
        assert!(error.to_string().contains("must be a real directory"));
        assert!(!outside.path().join(SIGNERS_FILE_NAME).exists());
    }
}
