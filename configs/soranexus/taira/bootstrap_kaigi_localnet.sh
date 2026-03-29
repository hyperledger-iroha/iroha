#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
LOCALNET_DIR="${IROHA_TAIRA_LOCALNET_DIR:-$ROOT_DIR/dist/taira-localnet}"
SCREEN_SESSION="${IROHA_TAIRA_SCREEN_SESSION:-taira-localnet}"
GENESIS_JSON="${IROHA_TAIRA_GENESIS_JSON:-$LOCALNET_DIR/genesis.json}"
GENESIS_SIGNED="${IROHA_TAIRA_GENESIS_SIGNED:-$LOCALNET_DIR/genesis.signed.nrt}"
GENESIS_SEED="${IROHA_TAIRA_GENESIS_SEED:-taira-localgenesis}"
CALL_DOMAIN="${IROHA_TAIRA_KAIGI_CALL_DOMAIN:-wonderland}"
CALL_NAME="${IROHA_TAIRA_KAIGI_CALL_NAME:-taira-relay-bootstrap}"
REPORTED_AT_MS="${IROHA_TAIRA_KAIGI_REPORTED_AT_MS:-1890864000000}"
RELAY_DOMAIN="${IROHA_TAIRA_KAIGI_RELAY_DOMAIN:-nexus}"
KAIGI_HELPER_BIN="${IROHA_TAIRA_KAIGI_HELPER_BIN:-}"

RELAY_HPKE_KEYS=(
  "K4NiAXqV5L1V3aD+/9NItPlFhEtm3qD4Q4K/1M8jewQ="
  "i4v17uBA5sK6YeK1+f3jvHfgvX4QAZp8ktPSVgJiccc="
  "aB9ehhc+zl8pKrjIY2g+it2e6G3I8gGxev5dCwSMQ9E="
)
RELAY_BANDWIDTH_CLASSES=(3 2 1)

need_file() {
  if [[ ! -f "$1" ]]; then
    echo "missing required file: $1" >&2
    exit 1
  fi
}

need_cmd() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "missing required command: $1" >&2
    exit 1
  fi
}

ensure_launchd_runner() {
  local runner="$LOCALNET_DIR/launchd-run.sh"
  if [[ -f "$runner" ]]; then
    return 0
  fi
  if [[ ! -f "$LOCALNET_DIR/start.sh" ]]; then
    echo "missing required file: $runner" >&2
    exit 1
  fi
  cat >"$runner" <<EOF
#!/bin/zsh
set -euo pipefail
setopt null_glob

DIR=\$(cd "\$(dirname "\$0")" && pwd)
IROHAD_BIN="\${IROHAD_BIN:-$ROOT_DIR/target/release/irohad}"

if [[ ! -x "\$IROHAD_BIN" ]]; then
  echo "irohad binary not executable: \$IROHAD_BIN" >&2
  exit 1
fi

cleanup() {
  local pidfile pid
  for pidfile in "\$DIR"/peer*.pid; do
    [[ -f "\$pidfile" ]] || continue
    pid=\$(<"\$pidfile")
    [[ -n "\$pid" ]] || continue
    kill "\$pid" 2>/dev/null || true
  done
}

for pid in \${(f)"\$(pgrep -f "\$DIR/peer[0-3]\\\\.toml" 2>/dev/null || true)"}; do
  [[ -n "\$pid" ]] || continue
  kill "\$pid" 2>/dev/null || true
done

rm -f "\$DIR"/peer*.pid
trap cleanup HUP INT TERM EXIT

for i in 0 1 2 3; do
  snapshot_dir="\$DIR/storage/peer\${i}/snapshot"
  mkdir -p "\$snapshot_dir"
  env SNAPSHOT_STORE_DIR="\$snapshot_dir" RUST_LOG="\${RUST_LOG:-info}" \\
    "\$IROHAD_BIN" --sora --config "\$DIR/peer\${i}.toml" >> "\$DIR/peer\${i}.log" 2>&1 &
  pid=\$!
  print -r -- "\$pid" > "\$DIR/peer\${i}.pid"
  echo "peer\$i pid \$pid"
done

while true; do
  for pidfile in "\$DIR"/peer*.pid; do
    [[ -f "\$pidfile" ]] || continue
    pid=\$(<"\$pidfile")
    if ! kill -0 "\$pid" 2>/dev/null; then
      echo "peer from \${pidfile:t} exited" >&2
      exit 1
    fi
  done
  sleep 1
done
EOF
  chmod +x "$runner"
}

extract_toml_string() {
  local key="$1"
  local file="$2"
  awk -F'"' -v wanted="$key" '$1 ~ "^[[:space:]]*" wanted "[[:space:]]*=" { print $2; exit }' "$file"
}

helper_supports_cli() {
  local candidate="$1"
  [[ -x "$candidate" ]] || return 1
  "$candidate" --help 2>&1 | grep -q -- '--genesis'
}

discover_helper_bin() {
  local candidate=""
  if [[ -n "$KAIGI_HELPER_BIN" ]]; then
    if helper_supports_cli "$KAIGI_HELPER_BIN"; then
      printf '%s\n' "$KAIGI_HELPER_BIN"
      return 0
    fi
    echo "configured Kaigi helper binary does not expose the genesis overlay CLI: $KAIGI_HELPER_BIN" >&2
    exit 1
  fi
  for candidate in \
    "$ROOT_DIR/target/release/examples/taira_kaigi_localnet" \
    "$ROOT_DIR/target/debug/examples/taira_kaigi_localnet"
  do
    if helper_supports_cli "$candidate"; then
      printf '%s\n' "$candidate"
      return 0
    fi
  done
  candidate="$(ls -1t "$ROOT_DIR"/target/release/examples/taira_kaigi_localnet-* 2>/dev/null | head -n1 || true)"
  if [[ -n "$candidate" ]] && helper_supports_cli "$candidate"; then
    printf '%s\n' "$candidate"
    return 0
  fi
  candidate="$(ls -1t "$ROOT_DIR"/target/debug/examples/taira_kaigi_localnet-* 2>/dev/null | head -n1 || true)"
  if [[ -n "$candidate" ]] && helper_supports_cli "$candidate"; then
    printf '%s\n' "$candidate"
    return 0
  fi
  return 1
}

need_cmd cargo
need_cmd curl
need_cmd jq
need_cmd screen
need_file "$GENESIS_JSON"
need_file "$LOCALNET_DIR/client.toml"
need_file "$LOCALNET_DIR/peer0.toml"
need_file "$LOCALNET_DIR/peer1.toml"
need_file "$LOCALNET_DIR/peer2.toml"
ensure_launchd_runner

HOST_PUBLIC_KEY="$(extract_toml_string public_key "$LOCALNET_DIR/client.toml")"
PEER0_PUBLIC_KEY="$(extract_toml_string public_key "$LOCALNET_DIR/peer0.toml")"
PEER1_PUBLIC_KEY="$(extract_toml_string public_key "$LOCALNET_DIR/peer1.toml")"
PEER2_PUBLIC_KEY="$(extract_toml_string public_key "$LOCALNET_DIR/peer2.toml")"

if [[ -z "$HOST_PUBLIC_KEY" || -z "$PEER0_PUBLIC_KEY" || -z "$PEER1_PUBLIC_KEY" || -z "$PEER2_PUBLIC_KEY" ]]; then
  echo "failed to extract host/relay public keys from localnet configs" >&2
  exit 1
fi

helper_bin="$(discover_helper_bin || true)"
echo "building signed Kaigi overlay genesis"
if [[ -n "$helper_bin" ]]; then
  "$helper_bin" \
    --genesis "$GENESIS_JSON" \
    --out-file "$GENESIS_SIGNED" \
    --seed "$GENESIS_SEED" \
    --host-public-key "$HOST_PUBLIC_KEY" \
    --relay-domain "$RELAY_DOMAIN" \
    --call-domain "$CALL_DOMAIN" \
    --call-name "$CALL_NAME" \
    --reported-at-ms "$REPORTED_AT_MS" \
    --relay-spec "${PEER0_PUBLIC_KEY}:${RELAY_HPKE_KEYS[0]}:${RELAY_BANDWIDTH_CLASSES[0]}" \
    --relay-spec "${PEER1_PUBLIC_KEY}:${RELAY_HPKE_KEYS[1]}:${RELAY_BANDWIDTH_CLASSES[1]}" \
    --relay-spec "${PEER2_PUBLIC_KEY}:${RELAY_HPKE_KEYS[2]}:${RELAY_BANDWIDTH_CLASSES[2]}"
else
  cargo run -p iroha_kagami --example taira_kaigi_localnet --release -- \
    --genesis "$GENESIS_JSON" \
    --out-file "$GENESIS_SIGNED" \
    --seed "$GENESIS_SEED" \
    --host-public-key "$HOST_PUBLIC_KEY" \
    --relay-domain "$RELAY_DOMAIN" \
    --call-domain "$CALL_DOMAIN" \
    --call-name "$CALL_NAME" \
    --reported-at-ms "$REPORTED_AT_MS" \
    --relay-spec "${PEER0_PUBLIC_KEY}:${RELAY_HPKE_KEYS[0]}:${RELAY_BANDWIDTH_CLASSES[0]}" \
    --relay-spec "${PEER1_PUBLIC_KEY}:${RELAY_HPKE_KEYS[1]}:${RELAY_BANDWIDTH_CLASSES[1]}" \
    --relay-spec "${PEER2_PUBLIC_KEY}:${RELAY_HPKE_KEYS[2]}:${RELAY_BANDWIDTH_CLASSES[2]}"
fi

echo "stopping existing taira localnet session"
"$LOCALNET_DIR/stop.sh" >/dev/null 2>&1 || true
screen -S "$SCREEN_SESSION" -X quit >/dev/null 2>&1 || true

if [[ -d "$LOCALNET_DIR/storage" ]]; then
  timestamp="$(date -u +%Y%m%d-%H%M%S)"
  mv "$LOCALNET_DIR/storage" "$LOCALNET_DIR/storage.prev-$timestamp"
fi
mkdir -p "$LOCALNET_DIR/storage"

echo "starting taira localnet"
screen -dmS "$SCREEN_SESSION" zsh -lc "cd '$LOCALNET_DIR' && ./launchd-run.sh"

echo "waiting for local status"
for _ in {1..60}; do
  if curl -sf "http://127.0.0.1:29080/status" >/dev/null; then
    break
  fi
  sleep 2
done

echo "waiting for Kaigi relay metadata"
for _ in {1..60}; do
  relay_json="$(curl -sf 'http://127.0.0.1:29080/v1/kaigi/relays' || true)"
  if [[ -n "$relay_json" ]] && [[ "$(jq -r '.total // 0' <<<"$relay_json")" -ge 3 ]]; then
    break
  fi
  sleep 2
done

echo "kaigi relays:"
curl -sf "http://127.0.0.1:29080/v1/kaigi/relays" | jq .

echo "kaigi health snapshot:"
curl -sf "http://127.0.0.1:29080/v1/kaigi/relays/health" | jq .
