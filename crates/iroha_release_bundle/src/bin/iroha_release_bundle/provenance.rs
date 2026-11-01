//! Closed publisher evidence emitted by Rust and admitted by the independent pipeline.
//!
//! These records never authorize their own producer. The entry pipeline must already
//! authenticate the exact record and executable independently before launching it.

use std::{collections::BTreeMap, io};

/// Maximum complete encoded record, including the final newline.
pub const MAX_BYTES: usize = 512 * 1024;

/// Exact original tool bytes used by the bootstrap.
#[derive(Clone, norito::json::JsonSerialize, norito::json::JsonDeserialize)]
#[norito(deny_unknown_fields)]
pub struct Tool {
    /// Bounded original executable extent.
    pub bytes: u64,
    /// Digest read through the held original.
    pub sha256: String,
}

/// Complete file-presence evidence for one Cargo configuration search directory.
#[derive(norito::json::JsonSerialize, norito::json::JsonDeserialize)]
#[norito(deny_unknown_fields)]
pub struct Configuration {
    /// Exact absolute search path in Cargo's discovery order.
    pub path: String,
    /// Whether that directory was present throughout the bootstrap.
    pub present: bool,
    /// Original `config` digest, or initial absence retained through all commands.
    pub config_sha256: Option<String>,
    /// Original `config.toml` digest, or initial absence retained through all commands.
    pub config_toml_sha256: Option<String>,
}

/// One first-release publisher evidence schema shared by emitter and consumer.
#[derive(norito::json::JsonSerialize, norito::json::JsonDeserialize)]
#[norito(deny_unknown_fields)]
pub struct Publisher {
    /// Exact first-release schema discriminator.
    pub schema: String,
    /// Existing canonical whole-workspace aggregate supplied by its retained owner.
    pub source_manifest_sha256: String,
    /// Digest of the independent original lockfile.
    pub cargo_lock_sha256: String,
    /// Exact native target of this executable and compiler host.
    pub native_target: String,
    /// Cargo's exact package identifier selected from authenticated workspace source.
    pub package_id: String,
    /// Exact native filename, including the Windows suffix.
    pub executable_filename: String,
    /// Digest copied from the retained Cargo output into the sealed executable.
    pub executable_sha256: String,
    /// Complete original executable extent.
    pub executable_bytes: u64,
    /// Exact bootstrap Cargo original.
    pub cargo_tool: Tool,
    /// Exact bootstrap rustc original.
    pub rustc_tool: Tool,
    /// Successful bounded Cargo version transcript.
    pub cargo_version: String,
    /// Successful bounded rustc verbose version transcript.
    pub rustc_version: String,
    /// Every retained Cargo configuration search location, including absences.
    pub configurations: Vec<Configuration>,
    /// Reviewed policy bytes embedded into the native bootstrap owner.
    pub build_environment_policy_sha256: String,
    /// Exact cleared-and-configured command environment; no unrelated ambient values.
    pub bootstrap_build_environment: BTreeMap<String, String>,
}

impl Publisher {
    /// Encode the only accepted record schema, retaining its explicit total bound.
    pub fn encode(&self) -> io::Result<Vec<u8>> {
        if self.schema != "iroha.publisher.v1" {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "unknown publisher schema",
            ));
        }
        let mut bytes = norito::json::to_vec(self).map_err(|_| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "cannot encode publisher evidence",
            )
        })?;
        bytes.push(b'\n');
        if bytes.len() > MAX_BYTES {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "publisher evidence exceeds its bound",
            ));
        }
        Ok(bytes)
    }
}
