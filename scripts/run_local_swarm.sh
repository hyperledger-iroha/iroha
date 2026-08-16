#!/usr/bin/env bash
set -euo pipefail

# Bare-metal 4-node swarm (no Docker). Run from repo root.

BASE="${BASE:-/tmp/iroha-bare}"
GENESIS_PUBLIC_KEY_FILE="${GENESIS_PUBLIC_KEY_FILE:-}"
GENESIS_PRIVATE_KEY_FILE="${GENESIS_PRIVATE_KEY_FILE:-}"

PKS=(
  "ea01308A230D5DA9DE92163DDE5F662CD859985ADC53040D9BFE1FA4A091CA7E1D8E88914535FC790CACC077DCC2F2D06FE106"
  "ea0130A3EBC016AF760F9B5FD20B45D6FDC0A6EC420DC08BC6D56EAF01F211DC23E4E0FB77C1A2E6984265ACB4450CFF4F2E3D"
  "ea0130904BB61D85D9AD526E4711D8B5A734B2335B71511BB66D0FBF2AE7EEE6C18A57BE44B1B9B1ACECA2C5C66384DC4F62D7"
  "ea0130AAB578FED3452B1629FF416025BF3D646D89C86DAAE0184A49C2A15D1D59A5DC39B38C606309FC38E3D5DF7ED2120B2E"
)
SKS=(
  "8926205F6436289BE34CE48DD72C52597B13B15D1F5EF078548AAE3AAEE1A3EDE6FA36"
  "892620DC0938B53B52D0C33B6FE5757EEB9CD6B535C55DFF0A5642E27A80F39330A753"
  "892620093156CF77B49FC14185247B8FE1539C288D56E63B83BDB06EBD9E0F8A5F4D56"
  "892620AF64535BCE4B6775610F0E660C021A4165E1629ED160369C1174FAAB06301909"
)
POPS=(
  "850859fa719dd75be661bc1a429e4d30a2151429e2f9a4c189347f9b0e1116219454f5d27a8834e40d9ae076aaea6285043586b4fb5c963df6b03b4e5363597a615af0915edef8ed6fa1cd268a31c437bd72c8a7f6666c0b7194df8739c1c3d8"
  "ac3a32b36684a7860eb5864d3f21806766f1faafc02a7b2c5fc6d687b6658e01edcb8aa18f8ee8b56763822a5bf6a3cb0e502cbf9ab04498e37f74b7cfd8f3d3509c1240ea899390d84f6826bd58bf053f8ef9b564df2e366aa3e786f5b1b24d"
  "927f73fd6614147584ad7886bb14761140dee5317bdca188be4e903d92335d8c704e5049de8ad56321a4a76794902a9309f507b4abc8d3639486a9500f7eb34fb89a2b1c311204ef30b1ea7d957ecae154df823f55e8a1d8af6c24ad68bd0b97"
  "b2b098c400391fd43cc643a235e6a408d748b654a024f9997109ca02e918dcdb78a3b2aa089d1fa12ed692217578965f01097d7749b5c0464b8f1f46a1b6853f7f7ef6f1f280a9b184602f7d8566e65b5983621c6e7a6e93de6ae932f522745f"
)
STREAM_PKS=(
  "ed0120A98BAFB0663CE08D75EBD506FEC38A84E576A7C9B0897693ED4B04FD9EF2D18D"
  "ed01209897952D14BDFAEA780087C38FF3EB800CB20B882748FC95A575ADB9CD2CB21D"
  "ed01204EE2FCD53E1730AF142D1E23951198678295047F9314B4006B0CB61850B1DB10"
  "ed0120CACF3A84B8DC8710CE9D6B968EE95EC7EE4C93C85858F026F3B4417F569592CE"
)
STREAM_SKS=(
  "802620A4DFC16789FBF9A588525E4AC7F791AC51B12AEE8919EACC03EB2FC31D32C692"
  "8026203ECA64ADC23DC106C9D703233375EA6AC345AD7299FF3AD45F355DE6CD1B5510"
  "8026207B1C78F733EDAFD6AF9BAC3A0D6C5A494557DD031609A4FDD9796EEF471D928C"
  "8026206C7FF4CA09D395C7B7332C654099406E929C6238942E3CE85155CC1A5E2CF519"
)
# Dedicated Ed25519 SoraNet transport identities derived from the SHA-256 seeds
# `iroha-run-local-swarm-soranet-transport-v1-peer-{0,1,2,3}`.
SORANET_TRANSPORT_PKS=(
  "ed01208FB5364C3275F840CFF3DA7B2585D0EF5DA34E6EA47D742A9F2AF0ACAE90D51E"
  "ed012006DA31B9676E61B33B8160415145CC28943ED419402F7412FBF9D7D1E7EDF56E"
  "ed01208F6F75539C6420238C08C07311C3BF594866B8F867E9FA032C533F9603134DE7"
  "ed01206CBFE275EB73969EB8A7C5842E1CB89AC6D2DEE9C6AB2E71B7A42D6E5938A3A2"
)
SORANET_TRANSPORT_SKS=(
  "802620A64266B9BC7DFA8DB70EAB99A1C9E7470858A87D850CF48F5A1F81950ADBF36C"
  "802620A1D97AD9C6106A50E82BED39CF0A4CD1AEF6DAF5AE3299E09D3D9EBB387522D5"
  "802620FFBF7F577A39A1ADB7E043EE9B7D24C2158F8E4FACEB432A76011B01959C2E01"
  "8026200177E60BE138D5F97520629FCF270EE9B4C9EB69870B1F67A5DC86C8E8E6FC41"
)
BASE_API_PORT="${BASE_API_PORT:-8080}"
BASE_P2P_PORT="${BASE_P2P_PORT:-1337}"
APIS=(
  "$BASE_API_PORT"
  "$((BASE_API_PORT + 1))"
  "$((BASE_API_PORT + 2))"
  "$((BASE_API_PORT + 3))"
)
P2PS=(
  "$BASE_P2P_PORT"
  "$((BASE_P2P_PORT + 1))"
  "$((BASE_P2P_PORT + 2))"
  "$((BASE_P2P_PORT + 3))"
)

pid_matches_local_swarm_peer() {
  local pid="$1"
  local config_path="$2"
  local command_line

  [[ "$pid" =~ ^[0-9]+$ ]] || return 1
  command -v ps >/dev/null 2>&1 || return 1
  command_line="$(ps -p "$pid" -o command= 2>/dev/null || true)"
  [[ -n "$command_line" ]] || return 1
  printf '%s' "$command_line" | grep -F -- "--config $config_path" >/dev/null \
    || printf '%s' "$command_line" | grep -F -- "--config=$config_path" >/dev/null
}

pid_is_running() {
  local pid="$1"

  [[ "$pid" =~ ^[0-9]+$ ]] || return 1
  command -v ps >/dev/null 2>&1 || return 0
  ps -p "$pid" -o pid= >/dev/null 2>&1
}

write_stop_script() {
  cat > "$BASE/stop.sh" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail

DIR="$(cd "$(dirname "$0")" && pwd)"

pid_matches_peer() {
  local pid="$1"
  local config="$2"
  local command_line

  [[ "$pid" =~ ^[0-9]+$ ]] || return 1
  command -v ps >/dev/null 2>&1 || return 1
  command_line="$(ps -p "$pid" -o command= 2>/dev/null || true)"
  [[ -n "$command_line" ]] || return 1
  printf '%s' "$command_line" | grep -F -- "--config $config" >/dev/null \
    || printf '%s' "$command_line" | grep -F -- "--config=$config" >/dev/null
}

pid_is_running() {
  local pid="$1"

  [[ "$pid" =~ ^[0-9]+$ ]] || return 1
  command -v ps >/dev/null 2>&1 || return 0
  ps -p "$pid" -o pid= >/dev/null 2>&1
}

for pidfile in "$DIR"/peer*.pid; do
  [[ -f "$pidfile" ]] || continue
  pid="$(cat "$pidfile" 2>/dev/null || true)"
  if [[ -z "$pid" ]]; then
    rm -f "$pidfile"
    continue
  fi
  if [[ ! "$pid" =~ ^[0-9]+$ ]]; then
    echo "removing malformed pidfile $pidfile (pid=$pid)" >&2
    rm -f "$pidfile"
    continue
  fi
  if ! pid_is_running "$pid"; then
    rm -f "$pidfile"
    continue
  fi
  peer_name="$(basename "$pidfile" .pid)"
  config="$DIR/${peer_name}.toml"
  if ! pid_matches_peer "$pid" "$config"; then
    echo "leaving $pidfile in place: live pid $pid does not match $config" >&2
    continue
  fi
  kill "$pid" 2>/dev/null || true
  for _ in {1..40}; do
    if pid_is_running "$pid"; then
      sleep 0.25
    else
      break
    fi
  done
  if pid_is_running "$pid"; then
    echo "leaving $pidfile in place: local-swarm peer $peer_name pid $pid is still running" >&2
    continue
  fi
  rm -f "$pidfile"
done
EOF
  chmod 700 "$BASE/stop.sh"
}

preflight_existing_pidfiles() {
  local found_live=0
  local pidfile pid peer_name config_path

  for pidfile in "$BASE"/peer*.pid; do
    [[ -f "$pidfile" ]] || continue
    pid="$(cat "$pidfile" 2>/dev/null || true)"
    if [[ -z "$pid" ]]; then
      rm -f "$pidfile"
      continue
    fi
    if [[ ! "$pid" =~ ^[0-9]+$ ]]; then
      echo "Removing malformed pidfile $pidfile (pid=$pid)" >&2
      rm -f "$pidfile"
      continue
    fi
    if ! pid_is_running "$pid"; then
      rm -f "$pidfile"
      continue
    fi
    peer_name="$(basename "$pidfile" .pid)"
    config_path="$BASE/${peer_name}.toml"
    if pid_matches_local_swarm_peer "$pid" "$config_path"; then
      echo "Refusing to overwrite live local-swarm peer $peer_name pid $pid; run $BASE/stop.sh first." >&2
    else
      echo "Leaving $pidfile in place: live pid $pid does not match $config_path." >&2
    fi
    found_live=1
  done

  if [[ "$found_live" -ne 0 ]]; then
    exit 1
  fi
}

addr_literal() {
  python3 - <<'PY' "$1"
import sys

tag = "addr"
body = sys.argv[1]
crc = 0xFFFF
for byte in (tag + ":" + body).encode():
    crc ^= byte << 8
    for _ in range(8):
        if crc & 0x8000:
            crc = ((crc << 1) ^ 0x1021) & 0xFFFF
        else:
            crc = (crc << 1) & 0xFFFF
print(f"{tag}:{body}#{crc:04X}")
PY
}

inject_topology() {
  python3 - "$BASE/genesis.json" \
    "${PKS[0]}" "${POPS[0]}" \
    "${PKS[1]}" "${POPS[1]}" \
    "${PKS[2]}" "${POPS[2]}" \
    "${PKS[3]}" "${POPS[3]}" <<'PY'
import json
import sys

path = sys.argv[1]
items = sys.argv[2:]
if len(items) % 2 != 0:
    raise SystemExit("expected peer/pop pairs")
topology = []
for i in range(0, len(items), 2):
    topology.append({"peer": items[i], "pop_hex": items[i + 1]})
with open(path) as fh:
    data = json.load(fh)
txs = data.get("transactions")
if not isinstance(txs, list):
    raise SystemExit("genesis missing transactions array")
txs.append({"instructions": [], "ivm_triggers": [], "topology": topology})
with open(path, "w") as fh:
    json.dump(data, fh, indent=2)
PY
}

validate_transport_identities() {
  local i j
  for i in 0 1 2 3; do
    if [[ "${SORANET_TRANSPORT_PKS[$i]}" == "${PKS[$i]}" ]] \
      || [[ "${SORANET_TRANSPORT_PKS[$i]}" == "${STREAM_PKS[$i]}" ]] \
      || [[ "${SORANET_TRANSPORT_SKS[$i]}" == "${SKS[$i]}" ]] \
      || [[ "${SORANET_TRANSPORT_SKS[$i]}" == "${STREAM_SKS[$i]}" ]]; then
      echo "Peer $i reuses a node or streaming key for SoraNet transport." >&2
      exit 1
    fi
    for ((j = 0; j < i; j++)); do
      if [[ "${SORANET_TRANSPORT_PKS[$i]}" == "${SORANET_TRANSPORT_PKS[$j]}" ]] \
        || [[ "${SORANET_TRANSPORT_SKS[$i]}" == "${SORANET_TRANSPORT_SKS[$j]}" ]]; then
        echo "Peers $j and $i share a SoraNet transport identity." >&2
        exit 1
      fi
    done
  done
}

validate_transport_identities
mkdir -p "$BASE"
BASE="$(cd "$BASE" && pwd)"
write_stop_script
preflight_existing_pidfiles
if [ "${RESET_STORAGE:-1}" -ne 0 ]; then
  rm -rf "$BASE/storage"
fi

export NORITO_SKIP_BINDINGS_SYNC=1
export IROHA_BUILD_LINE=iroha3
if [ "${SKIP_BUILD:-0}" -ne 1 ]; then
  cargo build --release --bin iroha3d --bin iroha --bin kagami
fi
IROHAD=target/release/iroha3d
IROHA=target/release/iroha
KAGAMI=target/release/kagami

if [[ -n "$GENESIS_PUBLIC_KEY_FILE" || -n "$GENESIS_PRIVATE_KEY_FILE" ]]; then
  if [[ -z "$GENESIS_PUBLIC_KEY_FILE" || -z "$GENESIS_PRIVATE_KEY_FILE" ]]; then
    echo "Set both GENESIS_PUBLIC_KEY_FILE and GENESIS_PRIVATE_KEY_FILE, or neither." >&2
    exit 1
  fi
  [[ -s "$GENESIS_PUBLIC_KEY_FILE" ]] || {
    echo "Genesis public-key file is missing or empty: $GENESIS_PUBLIC_KEY_FILE" >&2
    exit 1
  }
  [[ -s "$GENESIS_PRIVATE_KEY_FILE" ]] || {
    echo "Genesis private-key file is missing or empty: $GENESIS_PRIVATE_KEY_FILE" >&2
    exit 1
  }
else
  GENESIS_KEY_DIR="$BASE/genesis-key-custody"
  GENESIS_PUBLIC_KEY_FILE="$GENESIS_KEY_DIR/public.key"
  GENESIS_PRIVATE_KEY_FILE="$GENESIS_KEY_DIR/private.key"
  if [[ -e "$GENESIS_KEY_DIR" ]]; then
    echo "Refusing to overwrite existing genesis key custody under $BASE." >&2
    echo "Remove the old local swarm directory or supply both key-file variables explicitly." >&2
    exit 1
  fi
  $KAGAMI keys --out-dir "$GENESIS_KEY_DIR" > "$BASE/genesis-key-custody.log"
fi

if [[ "$(wc -l < "$GENESIS_PUBLIC_KEY_FILE" | tr -d ' ')" -ne 1 ]] \
  || [[ -n "$(tail -c 1 < "$GENESIS_PUBLIC_KEY_FILE")" ]]; then
  echo "Genesis public-key file must contain exactly one newline-terminated record." >&2
  exit 1
fi
GEN_PUB="$(cat "$GENESIS_PUBLIC_KEY_FILE")"
[[ -n "$GEN_PUB" ]] || {
  echo "Genesis public-key file is empty." >&2
  exit 1
}

echo "[1/6] Generate genesis"
$KAGAMI genesis generate --ivm-dir . --genesis-public-key "$GEN_PUB" default > "$BASE/genesis.json"

echo "[2/6] Inject topology"
inject_topology

echo "[3/6] Sign genesis"
$KAGAMI genesis sign "$BASE/genesis.json" \
  -o "$BASE/genesis.signed.nrt" \
  --expected-hash-out "$BASE/genesis.expected_hash" \
  --private-key-file "$GENESIS_PRIVATE_KEY_FILE" \
  --expected-public-key "$GEN_PUB" \
  2>&1 | tee "$BASE/gen.sign.log"
echo "Genesis public key: $GEN_PUB"
if [[ "$(wc -l < "$BASE/genesis.expected_hash" | tr -d ' ')" -ne 1 ]] \
  || [[ -n "$(tail -c 1 < "$BASE/genesis.expected_hash")" ]]; then
  echo "Genesis expected-hash file must contain exactly one newline-terminated record." >&2
  exit 1
fi
GENESIS_EXPECTED_HASH="$(cat "$BASE/genesis.expected_hash")"
if [[ ! "$GENESIS_EXPECTED_HASH" =~ ^[0-9a-f]{63}[13579bdf]$ ]]; then
  echo "Genesis expected hash is not one canonical marked lowercase hash." >&2
  exit 1
fi

trusted_peers_literal() {
  printf '["%s@%s","%s@%s","%s@%s","%s@%s"]' \
    "${PKS[0]}" "$(addr_literal "127.0.0.1:${P2PS[0]}")" \
    "${PKS[1]}" "$(addr_literal "127.0.0.1:${P2PS[1]}")" \
    "${PKS[2]}" "$(addr_literal "127.0.0.1:${P2PS[2]}")" \
    "${PKS[3]}" "$(addr_literal "127.0.0.1:${P2PS[3]}")"
}

write_config() {
  local idx=$1
  local store_dir="$BASE/storage/peer${idx}"
  mkdir -p "$store_dir"
  cat > "$BASE/peer${idx}.toml" <<EOF
chain = "00000000-0000-0000-0000-000000000000"
public_key = "${PKS[$idx]}"
private_key = "${SKS[$idx]}"
soranet_transport_public_key = "${SORANET_TRANSPORT_PKS[$idx]}"
soranet_transport_private_key = "${SORANET_TRANSPORT_SKS[$idx]}"
trusted_peers = $(trusted_peers_literal)
[[trusted_peers_pop]]
public_key = "${PKS[0]}"
pop_hex = "${POPS[0]}"

[[trusted_peers_pop]]
public_key = "${PKS[1]}"
pop_hex = "${POPS[1]}"

[[trusted_peers_pop]]
public_key = "${PKS[2]}"
pop_hex = "${POPS[2]}"

[[trusted_peers_pop]]
public_key = "${PKS[3]}"
pop_hex = "${POPS[3]}"
[network]
address = "$(addr_literal "0.0.0.0:${P2PS[$idx]}")"
public_address = "$(addr_literal "127.0.0.1:${P2PS[$idx]}")"
[torii]
address = "$(addr_literal "0.0.0.0:${APIS[$idx]}")"
[torii.transport.norito_rpc]
allowed_clients = ["*"]
enabled = true
require_mtls = false
stage = "ga"
[streaming]
identity_public_key = "${STREAM_PKS[$idx]}"
identity_private_key = "${STREAM_SKS[$idx]}"
[genesis]
public_key = "$GEN_PUB"
file = "$BASE/genesis.signed.nrt"
expected_hash = "$GENESIS_EXPECTED_HASH"
[kura]
init_mode = "fast"
store_dir = "$store_dir"
[snapshot]
store_dir = "$store_dir/snapshot"
[nexus]
enabled = false
[confidential]
enabled = true
# Consensus mode, validator set, and DA geometry come from the signed genesis.
[logger]
format = "compact"
level = "info"
EOF
}

write_client_config() {
  cat > "$BASE/client.toml" <<EOF
chain = "00000000-0000-0000-0000-000000000000"
network_id = "$GENESIS_EXPECTED_HASH"
torii_url = "http://127.0.0.1:${APIS[0]}/"

[basic_auth]
web_login = "mad_hatter"
password = "ilovetea"

[account]
domain = "wonderland"
public_key = "ed0120CE7FA46C9DCE7EA4B125E2E36BDB63EA33073E7590AC92816AE1E861B7048B03"
private_key = "802620CCF31D85E3B32A4BEA59987CE0C78E3B8E2DB93881468AB2435FE45D5C9DCD53"

[connect]
# Root directory for Connect queue diagnostics and evidence export. Defaults to ~/.iroha/connect.
# queue_root = "/home/alice/.iroha/connect"
EOF
}

echo "[4/6] Write configs"
for i in 0 1 2 3; do write_config "$i"; done
write_client_config

echo "[5/6] Start peers"
for i in 0 1 2 3; do
  RUST_LOG=info "$IROHAD" --config "$BASE/peer${i}.toml" > "$BASE/peer${i}.log" 2>&1 &
  echo $! > "$BASE/peer${i}.pid"
  echo "peer$i pid $(cat "$BASE/peer${i}.pid") api ${APIS[$i]} p2p ${P2PS[$i]}"
done

echo "Waiting 5s for peers to settle..."
sleep 5

echo "[6/6] Asset flow (transfer, query)"
ASSET="rose#wonderland"
SENDER="sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV"
RECIP="34mSYmj74pAZ6nNwwEsmGKGrHErCPMWtqeBwWhHkT9WcsndXbe2FjCNWFYCn5FiW5fdUcsbQD"

CLIENT_CONFIG="$BASE/client.toml"
$IROHA --config "$CLIENT_CONFIG" asset get --id "$ASSET#$SENDER"
$IROHA --config "$CLIENT_CONFIG" asset transfer --id "$ASSET#$SENDER" --to "$RECIP" --quantity 1
$IROHA --config "$CLIENT_CONFIG" asset get --id "$ASSET#$RECIP"

echo "Logs: $BASE/peer*.log  PIDs: $BASE/peer*.pid"
echo "To stop safely: cd $BASE && ./stop.sh"
