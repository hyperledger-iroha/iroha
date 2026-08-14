//! Shared test fixtures for the desktop supervisor shell.
use iroha_data_model::{
    block::consensus_v2::{PROTOCOL_VERSION, SumeragiV2GenesisContextParameters},
    parameter::{
        Parameter, Parameters,
        system::{SumeragiConsensusMode, SumeragiNposParameters},
    },
};
use norito::json::{self, Map, Value};
use std::{
    env, fs,
    path::{Path, PathBuf},
    sync::{Mutex, OnceLock},
};
pub(super) struct TestEnvGuard {
    key: &'static str,
    prev: Option<String>,
}
impl TestEnvGuard {
    pub(super) fn set(key: &'static str, value: &Path) -> Self {
        let prev = env::var(key).ok();
        // SAFETY: Callers serialize these tests with `env_lock`.
        unsafe { env::set_var(key, value) };
        Self { key, prev }
    }
}
impl Drop for TestEnvGuard {
    fn drop(&mut self) {
        if let Some(prev) = self.prev.as_ref() {
            unsafe { env::set_var(self.key, prev) };
        } else {
            unsafe { env::remove_var(self.key) };
        }
    }
}
pub(super) fn env_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}
pub(super) fn genesis_invocation_count(path: &Path) -> usize {
    invocation_count(path, "genesis")
}
pub(super) fn kagami_sign_invocation_count(path: &Path) -> usize {
    invocation_count(path, "sign")
}
fn invocation_count(path: &Path, expected: &str) -> usize {
    if !path.exists() {
        return 0;
    }
    let contents = fs::read_to_string(path).unwrap_or_else(|err| panic!("read kagami log: {err}"));
    contents.lines().filter(|line| *line == expected).count()
}
pub(super) fn install_kagami_stub(root: &Path) -> (PathBuf, TestEnvGuard) {
    let permissioned_manifest = fixture_manifest(SumeragiConsensusMode::Permissioned);
    let npos_manifest = fixture_manifest(SumeragiConsensusMode::Npos);
    let script = format!(
        r#"#!/bin/sh
set -e
if [ "$1" = "--version" ]; then
  echo "kagami-stub iroha3"
  exit 0
fi
if [ "$1" = "verify" ]; then
  exit 0
fi
if [ "$1" = "genesis" ] && [ "$2" = "generate" ]; then
  LOG_FILE="${{MOCHI_TEST_KAGAMI_LOG:-}}"
  if [ -n "$LOG_FILE" ]; then
    printf '%s\n' "$@" >> "$LOG_FILE"
  fi
  requested_mode=permissioned
  previous=
  for argument in "$@"; do
    if [ "$previous" = "--consensus-mode" ]; then
      requested_mode="$argument"
    fi
    previous="$argument"
  done
  if [ "$requested_mode" = "npos" ]; then
    cat <<'JSON'
{npos_manifest}
JSON
  else
    cat <<'JSON'
{permissioned_manifest}
JSON
  fi
  exit 0
fi
if [ "$1" = "genesis" ] && [ "$2" = "sign" ]; then
  LOG_FILE="${{MOCHI_TEST_KAGAMI_LOG:-}}"
  if [ -n "$LOG_FILE" ]; then
    printf 'sign\n' >> "$LOG_FILE"
  fi
  manifest_path="$3"
  shift 3
  while [ "$#" -gt 0 ]; do
    case "$1" in
      --out-file)
        out_file="$2"
        shift 2
        ;;
      --bound-manifest-out)
        bound_manifest_out="$2"
        shift 2
        ;;
      --expected-hash-out)
        expected_hash_out="$2"
        shift 2
        ;;
      --private-key-file)
        private_key_file="$2"
        shift 2
        ;;
      --config)
        config_file="$2"
        shift 2
        ;;
      *)
        shift
        ;;
    esac
  done
  test -s "$private_key_file"
  test -s "$config_file"
  printf 'stub-signed-genesis' > "$out_file"
  printf '0000000000000000000000000000000000000000000000000000000000000001\n' > "$expected_hash_out"
  if [ "$bound_manifest_out" != "$manifest_path" ]; then
    cp "$manifest_path" "$bound_manifest_out"
  fi
  exit 0
fi
printf 'unexpected invocation: %s\n' "$0 $*" >&2
exit 1
"#,
    );
    let path = install_stub_script(root, "kagami_stub.sh", &script);
    let signature_guard =
        TestEnvGuard::set("MOCHI_TEST_FINALIZE_KAGAMI_STUB_SIGNATURE", Path::new("1"));
    (path, signature_guard)
}
fn fixture_manifest(consensus_mode: SumeragiConsensusMode) -> String {
    let mut transaction = Map::new();
    if consensus_mode == SumeragiConsensusMode::Npos {
        let mut parameters = Parameters::default();
        parameters.set_parameter(Parameter::Custom(
            SumeragiNposParameters::default().into_custom_parameter(),
        ));
        transaction.insert(
            "parameters".to_owned(),
            json::value::to_value(&parameters).expect("encode NPoS parameters"),
        );
    }
    let mut manifest = Map::new();
    manifest.insert(
        "chain".to_owned(),
        Value::String("mochi-fixture".to_owned()),
    );
    manifest.insert(
        "chain_discriminant".to_owned(),
        Value::Number(u64::from(iroha_data_model::account::address::chain_discriminant()).into()),
    );
    manifest.insert("ivm_dir".to_owned(), Value::String(".".to_owned()));
    manifest.insert(
        "consensus_mode".to_owned(),
        Value::String(consensus_mode.to_string()),
    );
    manifest.insert(
        "wire_protocol_version".to_owned(),
        Value::Number(u64::from(PROTOCOL_VERSION).into()),
    );
    manifest.insert(
        "sumeragi_v2".to_owned(),
        json::value::to_value(&SumeragiV2GenesisContextParameters::recommended())
            .expect("encode signed Sumeragi v2 context"),
    );
    manifest.insert(
        "transactions".to_owned(),
        Value::Array(vec![Value::Object(transaction)]),
    );
    json::to_string(&Value::Object(manifest)).expect("encode fixture manifest")
}
pub(super) fn install_noop_stub(root: &Path, name: &str) -> PathBuf {
    install_stub_script(
        root,
        name,
        r#"#!/bin/sh
exit 0
"#,
    )
}
fn install_stub_script(root: &Path, name: &str, contents: &str) -> PathBuf {
    let path = root.join(name);
    fs::write(&path, contents).expect("write stub");
    make_executable(&path);
    path
}
#[cfg(unix)]
fn make_executable(path: &Path) {
    use std::os::unix::fs::PermissionsExt;
    let mut perms = fs::metadata(path).expect("metadata").permissions();
    perms.set_mode(0o755);
    fs::set_permissions(path, perms).expect("set perms");
}
#[cfg(not(unix))]
fn make_executable(_path: &Path) {}
