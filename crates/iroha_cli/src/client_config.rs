//! CLI-owned filesystem configuration layered beside the reusable SDK configuration.

use eyre::{Result, eyre};
use std::{
    env,
    path::{Path, PathBuf},
};

/// Filesystem paths used only by CLI commands.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct FilesystemConfig {
    /// Root directory containing Connect queue state.
    pub(crate) connect_queue_root: PathBuf,
    /// Optional canonical request witness used for multisig Soracloud HTTP mutations.
    pub(crate) soracloud_http_witness_file: Option<PathBuf>,
}

impl Default for FilesystemConfig {
    fn default() -> Self {
        Self {
            connect_queue_root: default_connect_queue_root(),
            soracloud_http_witness_file: None,
        }
    }
}

/// Return the CLI default Connect queue root.
pub(crate) fn default_connect_queue_root() -> PathBuf {
    let mut base = if cfg!(windows) {
        env::var_os("USERPROFILE").map(PathBuf::from)
    } else {
        env::var_os("HOME").map(PathBuf::from)
    }
    .unwrap_or_else(|| PathBuf::from("."));
    base.push(".iroha");
    base.push("connect");
    base
}

impl FilesystemConfig {
    /// Remove and validate the CLI-owned sections from a complete client TOML table.
    pub(crate) fn take_from(table: &mut toml::Table, source_path: &Path) -> Result<Self> {
        let mut config = Self::default();
        if let Some(mut connect) = take_section(table, "connect")? {
            if let Some(value) = connect.remove("queue_root") {
                config.connect_queue_root =
                    take_nonempty_path(value, "connect.queue_root", source_path)?;
            }
            reject_unknown_keys("connect", &connect)?;
        }
        if let Some(mut soracloud) = take_section(table, "soracloud")? {
            if let Some(value) = soracloud.remove("http_witness_file") {
                config.soracloud_http_witness_file = Some(take_nonempty_path(
                    value,
                    "soracloud.http_witness_file",
                    source_path,
                )?);
            }
            reject_unknown_keys("soracloud", &soracloud)?;
        }
        Ok(config)
    }
}

fn take_section(table: &mut toml::Table, name: &str) -> Result<Option<toml::Table>> {
    table
        .remove(name)
        .map(|value| {
            value
                .try_into()
                .map_err(|_| eyre!("`{name}` must be a TOML table"))
        })
        .transpose()
}

fn take_nonempty_path(value: toml::Value, parameter: &str, source_path: &Path) -> Result<PathBuf> {
    let raw = value
        .as_str()
        .ok_or_else(|| eyre!("`{parameter}` must be a string path"))?;
    if raw.is_empty() {
        return Err(eyre!("`{parameter}` must not be empty"));
    }
    let path = PathBuf::from(raw);
    if path.is_absolute() {
        return Ok(path);
    }
    let source_dir = source_path.parent().unwrap_or_else(|| Path::new("."));
    if source_dir.is_absolute() {
        Ok(source_dir.join(path))
    } else {
        Ok(env::current_dir()?.join(source_dir).join(path))
    }
}

fn reject_unknown_keys(section: &str, table: &toml::Table) -> Result<()> {
    if table.is_empty() {
        return Ok(());
    }
    let keys = table.keys().cloned().collect::<Vec<_>>().join(", ");
    Err(eyre!(
        "unknown `{section}` configuration parameter(s): {keys}"
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_cli_paths_and_removes_owned_sections() {
        let mut table = toml::toml! {
            chain = "test"
            [connect]
            queue_root = "/var/lib/iroha/connect"
            [soracloud]
            http_witness_file = "/run/iroha/witness.json"
        };
        let config = FilesystemConfig::take_from(&mut table, Path::new("/etc/iroha/client.toml"))
            .expect("valid CLI paths");
        assert_eq!(
            config.connect_queue_root,
            PathBuf::from("/var/lib/iroha/connect")
        );
        assert_eq!(
            config.soracloud_http_witness_file,
            Some(PathBuf::from("/run/iroha/witness.json"))
        );
        assert!(!table.contains_key("connect"));
        assert!(!table.contains_key("soracloud"));
        assert!(table.contains_key("chain"));
    }

    #[test]
    fn rejects_unknown_cli_keys() {
        let mut table = toml::toml! {
            [connect]
            root = "/retired/alias"
        };
        let error = FilesystemConfig::take_from(&mut table, Path::new("/etc/iroha/client.toml"))
            .expect_err("retired CLI aliases must fail closed");
        assert!(error.to_string().contains("unknown `connect`"));
    }

    #[test]
    fn rejects_invalid_cli_paths_and_unknown_keys() {
        let cases = [
            ("[connect]\nqueue_root = \"\"", "must not be empty"),
            ("[connect]\nqueue_root = 7", "must be a string path"),
            ("[soracloud]\nhttp_witness_file = \"\"", "must not be empty"),
            (
                "[soracloud]\nhttp_witness_file = 7",
                "must be a string path",
            ),
            (
                "[soracloud]\nwitness_file = \"retired\"",
                "unknown `soracloud`",
            ),
        ];
        for (source, expected) in cases {
            let mut table = source.parse::<toml::Table>().expect("test TOML");
            let error =
                FilesystemConfig::take_from(&mut table, Path::new("/etc/iroha/client.toml"))
                    .expect_err("invalid CLI filesystem configuration must fail closed");
            assert!(
                error.to_string().contains(expected),
                "expected `{expected}` in `{error}`"
            );
        }
    }

    #[test]
    fn resolves_relative_paths_from_the_toml_source_directory() {
        let mut table = toml::toml! {
            [connect]
            queue_root = "state/connect"
            [soracloud]
            http_witness_file = "auth/witness.norito"
        };
        let config =
            FilesystemConfig::take_from(&mut table, Path::new("/etc/iroha/profiles/operator.toml"))
                .expect("relative CLI paths");
        assert_eq!(
            config.connect_queue_root,
            PathBuf::from("/etc/iroha/profiles/state/connect")
        );
        assert_eq!(
            config.soracloud_http_witness_file,
            Some(PathBuf::from("/etc/iroha/profiles/auth/witness.norito"))
        );
    }
}
