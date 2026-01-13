#!/usr/bin/env bash
set -euo pipefail

# Bare-metal 4-node swarm (no Docker). Run from repo root.

BASE="${BASE:-/tmp/iroha-bare}"
GEN_PUB_OVERRIDE="${GEN_PUB_OVERRIDE:-}"
GENESIS_PRIVATE_KEY="${GENESIS_PRIVATE_KEY:-82B3BDE54AEBECA4146257DA0DE8D59D8E46D5FE34887DCD8072866792FCB3AD}"

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

mkdir -p "$BASE"
rm -f "$BASE"/peer*.pid
if [ "${RESET_STORAGE:-1}" -ne 0 ]; then
  rm -rf "$BASE/storage"
fi

export NORITO_SKIP_BINDINGS_SYNC=1
export IROHA_BUILD_LINE=iroha3
if [ "${SKIP_BUILD:-0}" -ne 1 ]; then
  cargo build --release --bin irohad --bin iroha --bin kagami
fi
IROHAD=target/release/irohad
IROHA=target/release/iroha
KAGAMI=target/release/kagami

echo "[1/6] Generate genesis"
$KAGAMI genesis generate --ivm-dir . --genesis-public-key ed01204164BF554923ECE1FD412D241036D863A6AE430476C898248B8237D77534CFC4 default > "$BASE/genesis.json"

echo "[2/6] Inject topology"
inject_topology

echo "[3/6] Sign genesis"
GEN_LOG=$(
  $KAGAMI genesis sign "$BASE/genesis.json" \
    -o "$BASE/genesis.signed.nrt" \
    --private-key "$GENESIS_PRIVATE_KEY" \
    2>&1 | tee "$BASE/gen.sign.log"
)
GEN_PUB=${GEN_PUB_OVERRIDE:-$(echo "$GEN_LOG" | awk '/Genesis public key:/ {print $4; exit}')}
if [ -z "$GEN_PUB" ]; then
  echo "Failed to capture genesis public key; check $BASE/gen.sign.log" >&2
  exit 1
fi
echo "Genesis public key: $GEN_PUB"

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
[kura]
init_mode = "fast"
store_dir = "$store_dir"
[snapshot]
store_dir = "$store_dir/snapshot"
[nexus]
enabled = false
[confidential]
enabled = true
[sumeragi]
consensus_mode = "npos"
enable_bls = true
da_enabled = true
[logger]
format = "compact"
level = "info"
EOF
}

write_client_config() {
  cat > "$BASE/client.toml" <<EOF
chain = "00000000-0000-0000-0000-000000000000"
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
SENDER="34mSYnDgbaJM58rbLoif4Tkp7G7pptR1KNF52GyuvUNd2XGP5NJ7ERtfk7Pbj5Fhtv2BW74vs@wonderland"
RECIP="34mSYmj74pAZ6nNwwEsmGKGrHErCPMWtqeBwWhHkT9WcsndXbe2FjCNWFYCn5FiW5fdUcsbQD@garden_of_live_flowers"

CLIENT_CONFIG="$BASE/client.toml"
$IROHA --config "$CLIENT_CONFIG" asset get --id "$ASSET#$SENDER"
$IROHA --config "$CLIENT_CONFIG" asset transfer --id "$ASSET#$SENDER" --to "$RECIP" --quantity 1
$IROHA --config "$CLIENT_CONFIG" asset get --id "$ASSET#$RECIP"

echo "Logs: $BASE/peer*.log  PIDs: $BASE/peer*.pid"
echo "To stop: xargs kill < $BASE/peer0.pid $BASE/peer1.pid $BASE/peer2.pid $BASE/peer3.pid 2>/dev/null || true"
