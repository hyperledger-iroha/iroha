//! Bundle files around the peer configurations: protective ignore file, bundled rANS table,
//! runtime operator signer, POSIX-shell lifecycle scripts, guide and command summary.

use super::*;
use std::fs;

/// Deterministic rANS table loaded by the mandatory streaming codec at node startup.
const RANS_SEED0_TABLE: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../codec/rans/tables/rans_seed0.toml"
));

/// Write the files every generation needs before configurations are rendered.
pub(super) fn write_scaffold(out_dir: &Path, operator: &Identity) -> Result<()> {
    let ignore = out_dir.join(".gitignore");
    custody::write(
        &ignore,
        concat!(
            "# Kagami localnets contain private signing material.\n",
            "*\n",
            "!.gitignore\n",
        ),
    )
    .wrap_err_with(|| format!("write protective ignore file {}", ignore.display()))?;
    let table = out_dir.join(config::RANS_TABLE_RELATIVE_PATH);
    custody::ensure_directory(
        table
            .parent()
            .expect("the rANS table path has a parent directory"),
    )?;
    custody::write(&table, RANS_SEED0_TABLE).wrap_err("write bundled rANS table")?;
    let runtime = custody::prepare_empty_private_directory(&out_dir.join(RUNTIME_DIRECTORY))
        .wrap_err("prepare localnet runtime credential directory")?;
    write_private_key_sidecar(
        &runtime.join(OPERATOR_SIGNER_KEY_FILE),
        &operator.private_key,
    )
}

/// Write `start.sh`, `stop.sh` and `README.md`, then validate the private artifact tree.
pub(super) fn write_lifecycle(out_dir: &Path, request: &Request<'_>) -> Result<()> {
    let peers = request.opts.peers.get();
    let start = out_dir.join("start.sh");
    let stop = out_dir.join("stop.sh");
    custody::write(&start, start_script(peers)).wrap_err("write start.sh")?;
    custody::write(&stop, stop_script(peers)).wrap_err("write stop.sh")?;
    #[cfg(unix)]
    for script in [&start, &stop] {
        use std::os::unix::fs::PermissionsExt as _;
        fs::set_permissions(script, fs::Permissions::from_mode(0o700))
            .wrap_err_with(|| format!("failed to mark {} executable", script.display()))?;
    }
    let shell_out_dir = crate::shell::absolute_quote_path(out_dir)
        .wrap_err("validate localnet output path for shell handoff commands")?;
    let torii_url = request.public_host.torii_url(request.opts.base_api_port);
    let readme = format!(
        concat!(
            "# Kagami localnet\n\n",
            "A disposable developer network: {peers} validators, `{consensus}` consensus, ",
            "chain `{chain}`.\n\n",
            "- Torii: {torii_url}\n",
            "- `client.toml`: the funded client signer; it pays fees in XOR (`xor#universal`), ",
            "deploys contracts and registers accounts.\n",
            "- `runtime/{operator_key}`: operator signer for node diagnostics.\n",
            "- `genesis.json`, `genesis.signed.nrt`, `genesis.expected_hash`, ",
            "`genesis.public_key`: the signed genesis the peers load.\n",
            "- `genesis.private_key`: the genesis signing key; keep it offline and never ",
            "commit it.\n",
            "- `peerN.toml`: validator configs; state lives in `storage/peerN` and ",
            "`state/peerN`, logs in `peerN.log`.\n\n",
            "## Run\n\n",
            "```bash\n",
            "cd {out_dir}\n",
            "bash ./start.sh\n",
            "curl -sf {torii_url}health\n",
            "bash ./stop.sh\n",
            "```\n\n",
            "`bash ./start.sh --peer-index N` starts one validator. Set `IROHAD_BIN` to choose ",
            "the `iroha3d` binary. `kagami docker --config-dir .` renders Docker Compose from ",
            "this bundle.\n",
        ),
        peers = peers,
        consensus = consensus_mode_label(request.opts.consensus_mode),
        chain = request.chain_id,
        torii_url = torii_url,
        operator_key = OPERATOR_SIGNER_KEY_FILE,
        out_dir = shell_out_dir,
    );
    custody::write(out_dir.join("README.md"), readme).wrap_err("write localnet guide")?;
    custody::validate_private_tree(out_dir, &[start.as_path(), stop.as_path()])
        .wrap_err("validate fresh localnet private artifact tree")
}

/// Print where everything is and the next commands.
pub(super) fn write_summary<T: Write>(
    writer: &mut BufWriter<T>,
    request: &Request<'_>,
    generated: &Generated,
) -> Result<()> {
    let out_dir = &generated.out_dir;
    let torii_url = request.public_host.torii_url(request.opts.base_api_port);
    let shell_out_dir = crate::shell::absolute_quote_path(out_dir)?;
    writeln!(writer, "out_dir: {}", out_dir.display())?;
    writeln!(writer, "chain_id: {}", request.chain_id)?;
    writeln!(
        writer,
        "consensus_mode: {}",
        consensus_mode_label(request.opts.consensus_mode)
    )?;
    writeln!(writer, "peers: {}", request.opts.peers)?;
    writeln!(writer, "torii_url: {torii_url}")?;
    writeln!(writer, "client_account: {}", generated.client.account_id)?;
    for (label, file) in [
        ("genesis_json", "genesis.json"),
        ("genesis_signed", "genesis.signed.nrt"),
        ("genesis_expected_hash", GENESIS_EXPECTED_HASH_FILE),
        ("genesis_public_key", GENESIS_PUBLIC_KEY_FILE),
        ("genesis_private_key", GENESIS_PRIVATE_KEY_FILE),
        ("client_config", "client.toml"),
        ("start_script", "start.sh"),
        ("stop_script", "stop.sh"),
        ("guide", "README.md"),
    ] {
        writeln!(writer, "{label}: {}", out_dir.join(file).display())?;
    }
    writeln!(
        writer,
        "operator_signer_key: {}",
        out_dir
            .join(RUNTIME_DIRECTORY)
            .join(OPERATOR_SIGNER_KEY_FILE)
            .display()
    )?;
    writeln!(writer, "next_start: cd {shell_out_dir} && bash ./start.sh")?;
    writeln!(writer, "next_health: curl -sf {torii_url}health")?;
    writeln!(writer, "next_stop: cd {shell_out_dir} && bash ./stop.sh")?;
    Ok(())
}

/// Default `iroha3d` locations: this checkout's debug and release builds.
fn default_daemon_paths() -> (PathBuf, PathBuf) {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let repo_root = manifest
        .parent()
        .and_then(Path::parent)
        .map_or_else(|| manifest.clone(), Path::to_path_buf);
    let repo_root = repo_root.canonicalize().unwrap_or(repo_root);
    let target = std::env::var_os("CARGO_TARGET_DIR").map_or_else(
        || repo_root.join("target"),
        |path| {
            let path = PathBuf::from(path);
            if path.is_absolute() {
                path
            } else {
                repo_root.join(path)
            }
        },
    );
    (
        target.join("debug").join("iroha3d"),
        target.join("release").join("iroha3d"),
    )
}

fn shell_literal(path: &Path) -> String {
    format!("'{}'", path.display().to_string().replace('\'', r"'\''"))
}

const PEER_SELECTION: &str = r#"SELECTED_PEERS="$(seq 0 "$((PEER_COUNT - 1))")"
if [ "$#" -ne 0 ]; then
  if [ "$#" -ne 2 ] || [ "$1" != "--peer-index" ]; then
    echo "usage: $0 [--peer-index INDEX]" >&2
    exit 2
  fi
  if [[ ! $2 =~ ^(0|[1-9][0-9]{0,4})$ ]]; then
    echo "invalid peer index: $2" >&2
    exit 2
  fi
  peer_index=$((10#$2))
  if (( peer_index >= PEER_COUNT )); then
    echo "invalid peer index: $2" >&2
    exit 2
  fi
  SELECTED_PEERS="$peer_index"
fi
"#;

fn start_script(peers: u16) -> String {
    let (debug, release) = default_daemon_paths();
    format!(
        r#"#!/usr/bin/env bash
set -euo pipefail
umask 077
DIR=$(cd "$(dirname "$0")" && pwd)
PEER_COUNT={peers}
{PEER_SELECTION}pid_is_running() {{
  pid="$1"
  case "$pid" in ''|*[!0-9]*) return 1 ;; esac
  command -v ps >/dev/null 2>&1 || return 0
  ps -p "$pid" -o pid= >/dev/null 2>&1
}}
DEFAULT_IROHAD_BIN_DEBUG={debug}
DEFAULT_IROHAD_BIN_RELEASE={release}
if [ -z "${{IROHAD_BIN:-}}" ]; then
  if [ -x "$DEFAULT_IROHAD_BIN_DEBUG" ]; then
    IROHAD_BIN="$DEFAULT_IROHAD_BIN_DEBUG"
  elif [ -x "$DEFAULT_IROHAD_BIN_RELEASE" ]; then
    IROHAD_BIN="$DEFAULT_IROHAD_BIN_RELEASE"
  else
    echo "IROHAD_BIN not set and neither $DEFAULT_IROHAD_BIN_DEBUG nor $DEFAULT_IROHAD_BIN_RELEASE exists; build iroha3d or set IROHAD_BIN" >&2
    exit 1
  fi
fi
# Resolve a relative IROHAD_BIN from the caller's directory before validators change into theirs.
IROHAD_BIN_RESOLVED="$(command -v "$IROHAD_BIN" 2>/dev/null || true)"
case "$IROHAD_BIN_RESOLVED" in
  ''|/*) ;;
  *) IROHAD_BIN_RESOLVED="$PWD/$IROHAD_BIN_RESOLVED" ;;
esac
if [ -z "$IROHAD_BIN_RESOLVED" ] || [ ! -x "$IROHAD_BIN_RESOLVED" ]; then
  echo "iroha3d binary not executable: $IROHAD_BIN" >&2
  exit 1
fi
echo "Using IROHAD_BIN=$IROHAD_BIN_RESOLVED" >&2
for i in $SELECTED_PEERS; do
  PIDFILE="$DIR/peer${{i}}.pid"
  if [ -f "$PIDFILE" ]; then
    existing_pid="$(cat "$PIDFILE" 2>/dev/null || true)"
    if [ -n "$existing_pid" ] && pid_is_running "$existing_pid"; then
      echo "peer$i already running with pid $existing_pid" >&2
      exit 1
    fi
    rm -f "$PIDFILE"
  fi
  set -- "$IROHAD_BIN_RESOLVED" --config "$DIR/peer${{i}}.toml"
  # A fresh key is asserted only on the first start, before safety records exist.
  if [ ! -d "$DIR/state/peer${{i}}/sumeragi-records" ]; then
    set -- "$@" --sumeragi-assert-fresh-key
  fi
  # Each validator runs from its own state directory, so node defaults that name relative
  # paths stay isolated per peer.
  mkdir -p "$DIR/state/peer${{i}}"
  (cd "$DIR/state/peer${{i}}" && exec nohup "$@" >>"$DIR/peer${{i}}.log" 2>&1 </dev/null) &
  echo "$!" > "$PIDFILE"
  echo "peer$i pid $(cat "$PIDFILE")"
done
"#,
        debug = shell_literal(&debug),
        release = shell_literal(&release),
    )
}

fn stop_script(peers: u16) -> String {
    format!(
        r#"#!/usr/bin/env bash
set -euo pipefail
umask 077
DIR=$(cd "$(dirname "$0")" && pwd)
PEER_COUNT={peers}
{PEER_SELECTION}pid_is_running() {{
  pid="$1"
  case "$pid" in ''|*[!0-9]*) return 1 ;; esac
  command -v ps >/dev/null 2>&1 || return 1
  ps -p "$pid" -o pid= >/dev/null 2>&1
}}
pid_matches_peer() {{
  pid="$1"
  config="$2"
  case "$pid" in ''|*[!0-9]*) return 1 ;; esac
  command -v ps >/dev/null 2>&1 || return 0
  command_line="$(ps -p "$pid" -o command= 2>/dev/null || true)"
  [ -n "$command_line" ] || return 1
  printf '%s' "$command_line" | grep -F -- "--config $config" >/dev/null \
    || printf '%s' "$command_line" | grep -F -- "--config=$config" >/dev/null
}}
for i in $SELECTED_PEERS; do
  pidfile="$DIR/peer${{i}}.pid"
  [ -f "$pidfile" ] || continue
  pid="$(cat "$pidfile" 2>/dev/null || true)"
  if [ -z "$pid" ]; then
    rm -f "$pidfile"
    continue
  fi
  case "$pid" in
    *[!0-9]*)
      echo "removing malformed pidfile $pidfile (pid=$pid)" >&2
      rm -f "$pidfile"
      continue
      ;;
  esac
  if ! pid_is_running "$pid"; then
    rm -f "$pidfile"
    continue
  fi
  config="$DIR/peer${{i}}.toml"
  if ! pid_matches_peer "$pid" "$config"; then
    echo "leaving $pidfile in place: live pid $pid does not match $config" >&2
    continue
  fi
  kill "$pid" 2>/dev/null || true
  for _ in $(seq 1 40); do
    if pid_is_running "$pid"; then
      sleep 0.25
    else
      break
    fi
  done
  if pid_is_running "$pid"; then
    echo "leaving $pidfile in place: localnet peer$i pid $pid is still running" >&2
    continue
  fi
  rm -f "$pidfile"
done
"#,
    )
}
