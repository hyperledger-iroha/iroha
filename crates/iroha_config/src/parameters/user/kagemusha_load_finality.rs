//! Bounded terminal Load proof-service configuration; parsing performs no custody I/O.

use super::*;

/// Optional complete proof installation selection; absence disables serving.
/// No private signing key or request-supplied finality source is accepted.
#[derive(Clone, ReadConfig, norito::JsonDeserialize)]
#[norito(deny_unknown_fields)]
pub struct KagemushaLoadFinality {
    /// Exact nonzero Scheme identity as 64 lowercase hexadecimal characters.
    pub scheme_id_hex: String,
    /// Exact nonzero manifest identity as 64 lowercase hexadecimal characters.
    pub manifest_digest_hex: String,
    /// Absolute exact verifier-pack original path.
    pub verifier_pack: PathBuf,
    /// Absolute exact producer-inventory original path.
    pub producer_inventory: PathBuf,
    /// Absolute existing private directory of complete server originals.
    pub server_originals: PathBuf,
    /// Absolute existing private retention journal directory.
    pub journal_dir: PathBuf,
    /// Maximum queued, active and unread results; 1..=64.
    #[config(default = "defaults::torii::kagemusha_load_finality::MAX_PENDING_REQUESTS")]
    #[norito(default = "default_max_pending_requests")]
    pub max_pending_requests: usize,
    /// Maximum receipt height to replay; 2..=u32::MAX.
    #[config(default = "defaults::torii::kagemusha_load_finality::MAXIMUM_RECEIPT_HEIGHT")]
    #[norito(default = "default_maximum_receipt_height")]
    pub maximum_receipt_height: u64,
    /// Maximum one key original; 1 byte through 1 GiB.
    #[config(default = "defaults::torii::kagemusha_load_finality::MAXIMUM_KEY_BYTES")]
    #[norito(default = "default_maximum_key_bytes")]
    pub maximum_key_bytes: usize,
    /// Positive finite aggregate graph extent, excluding usize::MAX.
    #[config(default = "defaults::torii::kagemusha_load_finality::MAXIMUM_ORIGINAL_BYTES")]
    #[norito(default = "default_maximum_original_bytes")]
    pub maximum_original_bytes: usize,
    /// Maximum graph entries; 1..=65_536.
    #[config(default = "defaults::torii::kagemusha_load_finality::MAXIMUM_ARTIFACTS")]
    #[norito(default = "default_maximum_artifacts")]
    pub maximum_artifacts: usize,
    /// Per-proof scratch budget; 1 MiB through 1 GiB.
    #[config(default = "defaults::torii::kagemusha_load_finality::MSM_BYTES")]
    #[norito(default = "default_msm_bytes")]
    pub msm_bytes: usize,
    /// Maximum journal entries including partials and locks; 3..=1_000_000.
    #[config(default = "defaults::torii::kagemusha_load_finality::MAXIMUM_JOURNAL_ENTRIES")]
    #[norito(default = "default_maximum_journal_entries")]
    pub maximum_journal_entries: usize,
    /// Positive finite retained byte extent, excluding u64::MAX.
    #[config(default = "defaults::torii::kagemusha_load_finality::MAXIMUM_JOURNAL_BYTES")]
    #[norito(default = "default_maximum_journal_bytes")]
    pub maximum_journal_bytes: u64,
    /// Native history allocation budget; 1 byte through 1 GiB.
    #[config(default = "defaults::torii::kagemusha_load_finality::NATIVE_WORKING_SET_BYTES")]
    #[norito(default = "default_native_working_set_bytes")]
    pub native_working_set_bytes: usize,
    /// Deadline for each native block acquisition; 1..=60_000 milliseconds.
    #[config(default = "defaults::torii::kagemusha_load_finality::NATIVE_STEP_TIMEOUT_MS")]
    #[norito(default = "default_native_step_timeout_ms")]
    pub native_step_timeout_ms: u64,
}

// Optional Torii sections use JSON deserialization; retain the same defaults as direct ReadConfig.
fn default_max_pending_requests() -> usize {
    defaults::torii::kagemusha_load_finality::MAX_PENDING_REQUESTS
}
fn default_maximum_receipt_height() -> u64 {
    defaults::torii::kagemusha_load_finality::MAXIMUM_RECEIPT_HEIGHT
}
fn default_maximum_key_bytes() -> usize {
    defaults::torii::kagemusha_load_finality::MAXIMUM_KEY_BYTES
}
fn default_maximum_original_bytes() -> usize {
    defaults::torii::kagemusha_load_finality::MAXIMUM_ORIGINAL_BYTES
}
fn default_maximum_artifacts() -> usize {
    defaults::torii::kagemusha_load_finality::MAXIMUM_ARTIFACTS
}
fn default_msm_bytes() -> usize {
    defaults::torii::kagemusha_load_finality::MSM_BYTES
}
fn default_maximum_journal_entries() -> usize {
    defaults::torii::kagemusha_load_finality::MAXIMUM_JOURNAL_ENTRIES
}
fn default_maximum_journal_bytes() -> u64 {
    defaults::torii::kagemusha_load_finality::MAXIMUM_JOURNAL_BYTES
}
fn default_native_working_set_bytes() -> usize {
    defaults::torii::kagemusha_load_finality::NATIVE_WORKING_SET_BYTES
}
fn default_native_step_timeout_ms() -> u64 {
    defaults::torii::kagemusha_load_finality::NATIVE_STEP_TIMEOUT_MS
}

impl std::fmt::Debug for KagemushaLoadFinality {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("KagemushaLoadFinality")
            .field("max_pending_requests", &self.max_pending_requests)
            .field("maximum_receipt_height", &self.maximum_receipt_height)
            .finish_non_exhaustive()
    }
}

fn digest(text: &str) -> std::result::Result<[u8; 32], &'static str> {
    if text.len() != 64
        || !text
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return Err("identities must be exactly 64 lowercase hexadecimal characters");
    }
    let mut value = [0; 32];
    hex::decode_to_slice(text, &mut value).map_err(|_| "invalid identity hex")?;
    if value == [0; 32] {
        return Err("identities must be nonzero");
    }
    Ok(value)
}

fn exact_path(path: &Path) -> std::result::Result<(), &'static str> {
    let text = path.to_str().ok_or("custody paths must be UTF-8")?;
    if !path.is_absolute()
        || path.parent().is_none()
        || text.len() > 4096
        || text.contains('\0')
        || path
            .components()
            .any(|c| matches!(c, std::path::Component::ParentDir))
        || path.components().collect::<PathBuf>().as_os_str() != path.as_os_str()
    {
        return Err(
            "custody paths must be bounded exact absolute paths without aliases or traversal",
        );
    }
    Ok(())
}

impl KagemushaLoadFinality {
    fn checked(self) -> std::result::Result<actual::KagemushaLoadFinality, &'static str> {
        let scheme_id = digest(&self.scheme_id_hex)?;
        let manifest_digest = digest(&self.manifest_digest_hex)?;
        for path in [
            &self.verifier_pack,
            &self.producer_inventory,
            &self.server_originals,
            &self.journal_dir,
        ] {
            exact_path(path)?;
        }
        if !(1..=64).contains(&self.max_pending_requests)
            || !(2..=u64::from(u32::MAX)).contains(&self.maximum_receipt_height)
            || !(1..=1 << 30).contains(&self.maximum_key_bytes)
            || self.maximum_original_bytes == 0
            || self.maximum_original_bytes == usize::MAX
            || !(1..=65_536).contains(&self.maximum_artifacts)
            || !(1 << 20..=1 << 30).contains(&self.msm_bytes)
            || !(3..=1_000_000).contains(&self.maximum_journal_entries)
            || self.maximum_journal_bytes == 0
            || self.maximum_journal_bytes == u64::MAX
            || !(1..=1 << 30).contains(&self.native_working_set_bytes)
            || !(1..=60_000).contains(&self.native_step_timeout_ms)
        {
            return Err("proof service resource limits exceed their finite bounds");
        }
        Ok(actual::KagemushaLoadFinality {
            scheme_id,
            manifest_digest,
            verifier_pack: self.verifier_pack,
            producer_inventory: self.producer_inventory,
            server_originals: self.server_originals,
            journal_dir: self.journal_dir,
            max_pending_requests: self.max_pending_requests,
            maximum_receipt_height: self.maximum_receipt_height,
            maximum_key_bytes: self.maximum_key_bytes,
            maximum_original_bytes: self.maximum_original_bytes,
            maximum_artifacts: self.maximum_artifacts,
            msm_bytes: self.msm_bytes,
            maximum_journal_entries: self.maximum_journal_entries,
            maximum_journal_bytes: self.maximum_journal_bytes,
            native_working_set_bytes: self.native_working_set_bytes,
            native_step_timeout: Duration::from_millis(self.native_step_timeout_ms),
        })
    }

    pub(super) fn parse(
        self,
        emitter: &mut Emitter<ParseError>,
    ) -> Option<actual::KagemushaLoadFinality> {
        match self.checked() {
            Ok(config) => Some(config),
            Err(error) => {
                emit_torii_config_error(emitter, format!("torii.kagemusha_load_finality: {error}"));
                None
            }
        }
    }
}

#[cfg(test)]
mod tests;
