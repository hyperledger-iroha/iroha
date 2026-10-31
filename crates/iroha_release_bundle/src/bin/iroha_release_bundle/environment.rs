//! Closed, owned child environment; unrelated runtime secrets never enter build provenance.
//!
//! These names are reviewed build/OS inputs, not authentication of the programs or
//! directories they mention. The enclosing pipeline still owns tool/configuration
//! trust. Unsupported compiler and linker overrides refuse before allocation.

use super::invalid;
use iroha_fs::ProcessCommand as Command;
use std::{collections::BTreeMap, ffi::OsString, io};

#[derive(Debug, Clone)]
pub(super) struct BuildEnvironment {
    values: BTreeMap<String, String>,
}

#[derive(norito::json::JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct Policy {
    schema: String,
    allowed: Vec<String>,
    owned: Vec<String>,
    ignored_names: Vec<String>,
    ignored_prefixes: Vec<String>,
    refused_names: Vec<String>,
    refused_prefixes: Vec<String>,
}

fn policy() -> &'static Policy {
    static VALUE: std::sync::OnceLock<Policy> = std::sync::OnceLock::new();
    VALUE.get_or_init(|| {
        let value: Policy = norito::json::from_str(include_str!("environment-policy.json"))
            .expect("reviewed embedded environment policy");
        assert_eq!(value.schema, "iroha.release-build-environment.v1");
        value
    })
}

fn contains(names: &[String], name: &str) -> bool {
    names.iter().any(|candidate| candidate == name)
}

fn unsupported(name: &str) -> bool {
    let policy = policy();
    if contains(&policy.owned, name)
        || contains(&policy.allowed, name)
        || contains(&policy.ignored_names, name)
        || policy
            .ignored_prefixes
            .iter()
            .any(|prefix| name.starts_with(prefix))
    {
        return false;
    }
    contains(&policy.refused_names, name)
        || policy
            .refused_prefixes
            .iter()
            .any(|prefix| name.starts_with(prefix))
}

impl BuildEnvironment {
    pub(super) fn capture() -> io::Result<Self> {
        Self::from_pairs(std::env::vars_os())
    }

    #[cfg(test)]
    pub(super) fn for_fixture() -> io::Result<Self> {
        // The fixture owns its child inputs independently of Cargo's environment.
        // Production capture still rejects inherited overrides, including empty ones.
        let inherited = std::env::vars_os().filter(|(name, _)| {
            name.to_str().is_some_and(|name| {
                let name = if cfg!(windows) {
                    name.to_ascii_uppercase()
                } else {
                    name.to_owned()
                };
                name != "RUSTUP_AUTO_INSTALL" && contains(&policy().allowed, &name)
            })
        });
        Self::from_pairs(inherited.chain([("RUSTUP_AUTO_INSTALL".into(), "0".into())]))
    }

    fn from_pairs(values: impl IntoIterator<Item = (OsString, OsString)>) -> io::Result<Self> {
        let mut approved = BTreeMap::new();
        let mut size = 0_usize;
        for (key, value) in values {
            let Some(key) = key.to_str() else {
                continue;
            };
            let name = if cfg!(windows) {
                key.to_ascii_uppercase()
            } else {
                key.to_owned()
            };
            if unsupported(&name) {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("unsupported inherited compiler, linker or Cargo override: {name}"),
                ));
            }
            if name == "RUSTUP_AUTO_INSTALL" && value != "0" {
                return Err(invalid("caller rustup auto-install policy is not disabled"));
            }
            if !contains(&policy().allowed, &name) {
                continue;
            }
            let value = value
                .into_string()
                .map_err(|_| invalid("reviewed build environment value is not UTF-8"))?;
            if value.len() > 64 * 1024 || value.contains('\0') {
                return Err(invalid(
                    "reviewed build environment value exceeds its bound",
                ));
            }
            if name == "SOURCE_DATE_EPOCH"
                && (value.is_empty()
                    || value.len() > 20
                    || !value.bytes().all(|byte| byte.is_ascii_digit())
                    || value.parse::<u64>().is_err())
            {
                return Err(invalid(
                    "SOURCE_DATE_EPOCH is not a bounded unsigned decimal",
                ));
            }
            size = size
                .checked_add(name.len() + value.len())
                .ok_or_else(|| invalid("build environment size overflow"))?;
            if size > 256 * 1024 || approved.insert(name, value).is_some() {
                return Err(invalid(
                    "build environment is excessive or repeats a native name",
                ));
            }
        }
        Ok(Self { values: approved })
    }

    pub(super) fn apply(&self, command: &mut Command) {
        command
            .env_clear()
            .envs(&self.values)
            .env("CARGO_BUILD_JOBS", "1")
            .env("CARGO_INCREMENTAL", "0");
    }

    #[cfg(test)]
    pub(super) fn provenance(&self) -> &BTreeMap<String, String> {
        &self.values
    }

    pub(super) fn configured(command: &Command) -> io::Result<BTreeMap<String, String>> {
        command
            .get_envs()
            .map(|(name, value)| {
                let name = name
                    .to_str()
                    .ok_or_else(|| invalid("configured environment name is not UTF-8"))?;
                let value = value.and_then(std::ffi::OsStr::to_str).ok_or_else(|| {
                    invalid("configured environment value is absent or not UTF-8")
                })?;
                Ok((name.to_owned(), value.to_owned()))
            })
            .collect()
    }

    pub(super) fn policy_digest() -> String {
        use sha2::Digest as _;
        hex::encode(sha2::Sha256::digest(include_bytes!(
            "environment-policy.json"
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn pairs(values: &[(&str, &str)]) -> Vec<(OsString, OsString)> {
        values
            .iter()
            .map(|(name, value)| ((*name).into(), (*value).into()))
            .collect()
    }

    #[test]
    fn unsupported_overrides_refuse_including_empty_values() {
        for name in [
            "RUSTC_WRAPPER",
            "RUSTC_WORKSPACE_WRAPPER",
            "RUSTFLAGS",
            "CARGO_ENCODED_RUSTFLAGS",
            "CARGO_BUILD_RUSTFLAGS",
            "CARGO_TARGET_X86_64_PC_WINDOWS_MSVC_LINKER",
            "RUSTUP_TOOLCHAIN",
            "CC",
            "CC_x86_64_unknown_linux_gnu",
            "LD_PRELOAD",
            "BINDGEN_EXTRA_CLANG_ARGS",
        ] {
            for value in ["", "unapproved"] {
                assert!(
                    BuildEnvironment::from_pairs(pairs(&[(name, value)])).is_err(),
                    "{name}"
                );
            }
        }
    }

    #[test]
    fn public_build_commands_refuse_ambient_toolchain_before_allocation() {
        const TEST: &str = "cli::environment::tests::public_build_commands_refuse_ambient_toolchain_before_allocation";
        const ROOT: &str = "IROHA_RELEASE_ENVIRONMENT_FIXTURE_ROOT";
        const SUBCOMMAND: &str = "IROHA_RELEASE_ENVIRONMENT_FIXTURE_COMMAND";
        const CHILD_MARKER: &str = "actual-public-entry-refused-ambient-toolchain";
        const DIRECTORIES: [&str; 6] =
            ["source", "cargo", "artifacts", "home", "controls", "tools"];

        if let Some(root) = std::env::var_os(ROOT) {
            let root = std::path::PathBuf::from(root);
            let subcommand = std::env::var(SUBCOMMAND).unwrap();
            assert!(matches!(
                subcommand.as_str(),
                "build-publish" | "bootstrap-build-publish" | "build-bridge-publish"
            ));
            assert!(std::env::var_os("RUSTUP_TOOLCHAIN").is_some());
            let mut values = vec![OsString::from(subcommand)];
            for (name, path) in [
                ("--repo-root", root.join("source")),
                ("--cargo-target-dir", root.join("cargo")),
                ("--artifact-root", root.join("artifacts")),
                ("--cargo-home", root.join("home")),
                ("--cargo-bin", root.join("tools").join("cargo")),
                ("--rustc-bin", root.join("tools").join("rustc")),
                ("--cancel-request", root.join("controls").join("request")),
            ] {
                values.extend([name.into(), path.into_os_string()]);
            }
            values.extend([
                "--source-sha256".into(),
                "a".repeat(64).into(),
                "--name".into(),
                "invocation.abc123".into(),
            ]);
            let mut output = Vec::new();
            // This is the public production entry, with its real ambient capture.
            // The missing tool files cannot be consulted before this refusal.
            let error = super::super::run(values.into_iter(), &mut output).unwrap_err();
            assert_eq!(error.exit_code(), 2);
            assert_eq!(
                error.to_string(),
                "unsupported inherited compiler, linker or Cargo override: RUSTUP_TOOLCHAIN"
            );
            assert!(output.is_empty());
            for name in DIRECTORIES {
                assert_eq!(
                    std::fs::read_dir(root.join(name)).unwrap().count(),
                    0,
                    "{name}"
                );
            }
            println!("{CHILD_MARKER}");
            return;
        }

        let subcommands: &[&str] = if cfg!(any(target_os = "linux", windows)) {
            &[
                "build-publish",
                "bootstrap-build-publish",
                "build-bridge-publish",
            ]
        } else {
            &["build-publish", "bootstrap-build-publish"]
        };
        for &subcommand in subcommands {
            for value in ["fixture-selected-toolchain", ""] {
                let temporary = tempfile::tempdir().unwrap();
                let root =
                    iroha_fs::PrivateDirectory::open_or_create(temporary.path().join("private"))
                        .unwrap();
                for name in DIRECTORIES {
                    root.create_child(name).unwrap();
                }
                let mut command = Command::new(std::env::current_exe().unwrap()).unwrap();
                BuildEnvironment::for_fixture().unwrap().apply(&mut command);
                command
                    .args(["--exact", TEST, "--nocapture", "--test-threads=1"])
                    .env(ROOT, root.path())
                    .env(SUBCOMMAND, subcommand)
                    .env("RUSTUP_TOOLCHAIN", value)
                    .kill_on_drop(true);
                let output = command.output().unwrap();
                assert!(
                    output.status.success(),
                    "{subcommand}, {value:?}: {output:?}"
                );
                let stdout = String::from_utf8(output.stdout).unwrap();
                assert!(
                    stdout.contains("1 passed; 0 failed; 0 ignored;"),
                    "{stdout}"
                );
                assert_eq!(stdout.matches(CHILD_MARKER).count(), 1, "{stdout}");
                root.revalidate().unwrap();
                assert_eq!(
                    std::fs::read_dir(root.path()).unwrap().count(),
                    DIRECTORIES.len()
                );
                for name in DIRECTORIES {
                    assert_eq!(
                        std::fs::read_dir(root.path().join(name)).unwrap().count(),
                        0,
                        "{name}"
                    );
                }
            }
        }
    }

    #[test]
    fn os_and_msvc_inputs_survive_without_forwarding_or_recording_runtime_secrets() {
        let environment = BuildEnvironment::from_pairs(pairs(&[
            ("PATH", "reviewed-path"),
            ("INCLUDE", "reviewed-include"),
            ("LIB", "reviewed-lib"),
            ("LIBPATH", "reviewed-libpath"),
            ("TEMP", "original-native-temp"),
            ("IROHA_RUNTIME_SIGNING_KEY", "fixture-only-secret"),
            ("GITHUB_TOKEN", "fixture-only-token"),
            ("CARGO_PKG_VERSION", "test-metadata"),
            ("CARGO_TARGET_DIR", "ambient-untrusted-target"),
        ]))
        .unwrap();
        assert_eq!(environment.provenance().len(), 5);
        assert!(!format!("{environment:?}").contains("fixture-only"));
        let mut command = Command::new(
            std::env::current_dir()
                .unwrap()
                .join("observed-without-spawning"),
        )
        .unwrap();
        command.env("IROHA_RUNTIME_SIGNING_KEY", "must-be-cleared");
        environment.apply(&mut command);
        let values: BTreeMap<_, _> = command
            .get_envs()
            .map(|(name, value)| (name.to_owned(), value.map(std::ffi::OsStr::to_owned)))
            .collect();
        assert!(!values.contains_key(std::ffi::OsStr::new("IROHA_RUNTIME_SIGNING_KEY")));
        assert!(!values.contains_key(std::ffi::OsStr::new("CARGO_TARGET_DIR")));
        assert_eq!(
            values[std::ffi::OsStr::new("CARGO_BUILD_JOBS")],
            Some("1".into())
        );
        assert_eq!(
            values[std::ffi::OsStr::new("TEMP")],
            Some("original-native-temp".into())
        );
    }

    #[test]
    fn auto_install_policy_and_timestamp_are_validated_before_allocation() {
        assert!(BuildEnvironment::from_pairs(pairs(&[("RUSTUP_AUTO_INSTALL", "1")])).is_err());
        for value in ["", "-1", "not-a-time", "18446744073709551616"] {
            assert!(BuildEnvironment::from_pairs(pairs(&[("SOURCE_DATE_EPOCH", value)])).is_err());
        }
        assert!(
            BuildEnvironment::from_pairs(pairs(&[
                ("RUSTUP_AUTO_INSTALL", "0"),
                ("SOURCE_DATE_EPOCH", "0")
            ]))
            .is_ok()
        );
    }

    #[test]
    #[ignore = "subprocess entry executed by actual_child_receives_only_owned_environment"]
    fn original_environment_child() {
        assert!(std::env::var_os("IROHA_RUNTIME_SIGNING_KEY").is_none());
        assert!(std::env::var_os("GITHUB_TOKEN").is_none());
        assert_eq!(std::env::var("SOURCE_DATE_EPOCH").unwrap(), "123456789");
        assert_eq!(std::env::var("CARGO_BUILD_JOBS").unwrap(), "1");
        println!("owned-build-environment-observed");
    }

    #[test]
    fn actual_child_receives_only_owned_environment() {
        let inherited = std::env::vars_os().filter(|(name, _)| {
            name.to_str().is_some_and(|name| {
                policy().allowed.iter().any(|candidate| {
                    candidate
                        == if cfg!(windows) {
                            name.to_ascii_uppercase()
                        } else {
                            name.to_owned()
                        }
                        .as_str()
                })
            })
        });
        let mut environment = BuildEnvironment::from_pairs(inherited).unwrap();
        environment
            .values
            .insert("SOURCE_DATE_EPOCH".into(), "123456789".into());
        let mut command = Command::new(std::env::current_exe().unwrap()).unwrap();
        command
            .args([
                "--ignored",
                "--exact",
                "cli::environment::tests::original_environment_child",
                "--nocapture",
            ])
            .env("IROHA_RUNTIME_SIGNING_KEY", "fixture-only-secret")
            .env("GITHUB_TOKEN", "fixture-only-token");
        environment.apply(&mut command);
        let output = command.output().unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(
            String::from_utf8(output.stdout)
                .unwrap()
                .contains("owned-build-environment-observed")
        );
    }
}
