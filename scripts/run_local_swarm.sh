#!/usr/bin/env bash
set -euo pipefail

# Bare-metal 4-node swarm (no Docker). Run from repo root.

BASE="${BASE:-/tmp/iroha-bare}"
GEN_PUB_OVERRIDE="${GEN_PUB_OVERRIDE:-}"

PKS=(
  "ed0120A98BAFB0663CE08D75EBD506FEC38A84E576A7C9B0897693ED4B04FD9EF2D18D"
  "ed01209897952D14BDFAEA780087C38FF3EB800CB20B882748FC95A575ADB9CD2CB21D"
  "ed01204EE2FCD53E1730AF142D1E23951198678295047F9314B4006B0CB61850B1DB10"
  "ed0120CACF3A84B8DC8710CE9D6B968EE95EC7EE4C93C85858F026F3B4417F569592CE"
)
SKS=(
  "802620A4DFC16789FBF9A588525E4AC7F791AC51B12AEE8919EACC03EB2FC31D32C692"
  "8026203ECA64ADC23DC106C9D703233375EA6AC345AD7299FF3AD45F355DE6CD1B5510"
  "8026207B1C78F733EDAFD6AF9BAC3A0D6C5A494557DD031609A4FDD9796EEF471D928C"
  "8026206C7FF4CA09D395C7B7332C654099406E929C6238942E3CE85155CC1A5E2CF519"
)
APIS=(8080 8081 8082 8083)
P2PS=(1337 1338 1339 1340)

mkdir -p "$BASE"
rm -f "$BASE"/peer*.pid

export NORITO_SKIP_BINDINGS_SYNC=1
cargo build --release --bin irohad --bin iroha --bin kagami
IROHAD=target/release/irohad
IROHA=target/release/iroha
KAGAMI=target/release/kagami

echo "[1/5] Generate genesis"
$KAGAMI genesis generate --ivm-dir . --genesis-public-key ed01204164BF554923ECE1FD412D241036D863A6AE430476C898248B8237D77534CFC4 default > "$BASE/genesis.json"

echo "[2/5] Sign genesis"
GEN_LOG=$($KAGAMI genesis sign "$BASE/genesis.json" -o "$BASE/genesis.signed.nrt" 2>&1 | tee "$BASE/gen.sign.log")
GEN_PUB=${GEN_PUB_OVERRIDE:-$(echo "$GEN_LOG" | awk '/Genesis public key:/ {print $4; exit}')}
if [ -z "$GEN_PUB" ]; then
  echo "Failed to capture genesis public key; check $BASE/gen.sign.log" >&2
  exit 1
fi
echo "Genesis public key: $GEN_PUB"

trusted_peers_literal() {
  printf '["%s@127.0.0.1:%s","%s@127.0.0.1:%s","%s@127.0.0.1:%s","%s@127.0.0.1:%s"]' \
    "${PKS[0]}" "${P2PS[0]}" "${PKS[1]}" "${P2PS[1]}" "${PKS[2]}" "${P2PS[2]}" "${PKS[3]}" "${P2PS[3]}"
}

write_config() {
  local idx=$1
  cat > "$BASE/peer${idx}.toml" <<EOF
chain = "00000000-0000-0000-0000-000000000000"
public_key = "${PKS[$idx]}"
private_key = "${SKS[$idx]}"
trusted_peers = $(trusted_peers_literal)
[network]
address = "0.0.0.0:${P2PS[$idx]}"
public_address = "127.0.0.1:${P2PS[$idx]}"
[torii]
address = "0.0.0.0:${APIS[$idx]}"
[genesis]
public_key = "$GEN_PUB"
file = "$BASE/genesis.signed.nrt"
[logger]
format = "compact"
level = "info"
EOF
}

echo "[3/5] Write configs"
for i in 0 1 2 3; do write_config "$i"; done

echo "[4/5] Start peers"
for i in 0 1 2 3; do
  RUST_LOG=info "$IROHAD" --config "$BASE/peer${i}.toml" > "$BASE/peer${i}.log" 2>&1 &
  echo $! > "$BASE/peer${i}.pid"
  echo "peer$i pid $(cat "$BASE/peer${i}.pid") api ${APIS[$i]} p2p ${P2PS[$i]}"
done

echo "Waiting 5s for peers to settle..."
sleep 5

echo "[5/5] Asset flow (register, mint, transfer, query)"
ASSET="testasset#wonderland"
SENDER="ed0120CE7FA46C9DCE7EA4B125E2E36BDB63EA33073E7590AC92816AE1E861B7048B03@wonderland"
RECIP="ed012004FF5B81046DDCCF19E2E451C45DFB6F53759D4EB30FA2EFA807284D1CC33016@wonderland"

$IROHA --config defaults/client.toml asset definition register --id "$ASSET" -t Numeric || true
$IROHA --config defaults/client.toml --torii-url "http://127.0.0.1:${APIS[0]}/" asset mint --id "$ASSET#$SENDER" --quantity 100
$IROHA --config defaults/client.toml --torii-url "http://127.0.0.1:${APIS[0]}/" asset transfer --id "$ASSET#$SENDER" --to "$RECIP" --quantity 25
$IROHA --config defaults/client.toml --torii-url "http://127.0.0.1:${APIS[1]}/" asset get --id "$ASSET#$RECIP"

echo "Logs: $BASE/peer*.log  PIDs: $BASE/peer*.pid"
echo "To stop: xargs kill < $BASE/peer0.pid $BASE/peer1.pid $BASE/peer2.pid $BASE/peer3.pid 2>/dev/null || true"
