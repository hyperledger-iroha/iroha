# SCCP TRON Contracts

This directory contains TRON/TVM deployment entrypoints for SCCP source and
destination verifier flows.

Files:

- `SccpTronSourceBridge.sol`: owner-governed source emitter for the proven
  `submitSccpSourceEvent(uint32,uint32,bytes32)` transaction-call path.
- `SccpTronGroth16Bn254MessageVerifier.sol`: destination-side BN254 Groth16
  verifier entrypoint for TRON/TVM deployments.
- `TairaXOR.sol`: TRC20-compatible bridged XOR token for the
  `taira_tron_xor` route.
- `TairaXorSccpBridge.sol`: route-bound mint/burn bridge that verifies
  TAIRA-origin proofs and emits TRON-origin SCCP source events.

The deployment helper `scripts/sccp_tron_taira_xor_deploy.mjs` creates and uses
a separate TRON deployer account artifact under ignored `artifacts/sccp-tron/`.
This deployer is only for contract deployment and evidence collection; end-user
bridging must continue through WalletConnect-connected TRON wallets.

Minimal operator sequence:

```bash
node scripts/sccp_tron_taira_xor_deploy.mjs generate-deployer
# Fund the printed address with the TRX/Energy budget approved for deployment.
node scripts/sccp_tron_taira_xor_deploy.mjs account-status
NODE_PATH=/tmp/iroha-sccp-smoke-node/node_modules \
  node scripts/sccp_tron_taira_xor_deploy.mjs compile
NODE_PATH=/tmp/iroha-sccp-smoke-node/node_modules \
  node scripts/sccp_tron_taira_xor_deploy.mjs deploy \
    --verifier artifacts/sccp-tron/production-verifier-key.json \
    --broadcast true \
    --confirm-mainnet taira_tron_xor
```

`deploy` compiles the TRON artifacts, creates java-tron
`/wallet/deploycontract` transactions, signs them locally with the deployer key,
broadcasts them only when `--confirm-mainnet taira_tron_xor` is present, waits
for transaction info, then configures the deployment by calling
`TairaXOR.setBridge`, `TairaXOR.lockBridge`,
`SccpTronSourceBridge.transferOwnership`, and
`SccpTronGroth16Bn254MessageVerifier.emitDestinationBindingConfigured`.
The helper also exposes `sign-transaction` and `broadcast` commands for
operator-reviewed unsigned transaction JSON. The signed output stores the
recoverable secp256k1 signature and verifies that it recovers to the deployer
before writing the artifact. Broadcast is deliberately unavailable without the
explicit mainnet confirmation flag.

## Source bridge

`SccpTronSourceBridge` stores immutable lane metadata (`networkId`,
`sourceDomain`, and `targetDomain`) and inherits the shared `Ownable` governance
surface. For the production TRON -> SORA lane the source domain is TRON and the
target domain is SORA, so `targetDomain = 0` is required; the constructor
rejects any non-TRON source domain, any non-SORA target-domain id, and
same-source/target pairs. Only the owner can
call:

```solidity
submitSccpSourceEvent(
    uint32 eventSourceDomain,
    uint32 eventTargetDomain,
    bytes32 sourceEventDigest
)
```

The call rejects source/target domain arguments that do not match the immutable
lane metadata, rejects a zero source-event digest, and rejects replay of a digest
that has already been submitted. A successful call emits exactly
`SccpSourceEvent(bytes32 indexed sourceEventDigest)`, so `topic0` is
`keccak256("SccpSourceEvent(bytes32)")`, `topic1` is the SCCP source event
digest, and event data is empty. The Rust TRON source adapter proves the
successful transaction call under the signed-header `txTrieRoot`; the legacy
receipt diagnostic path can still recognize this event shape when a bounded
`TransactionInfo` value is supplied. Production source-call proofs also require
exactly one recovered transaction signer, and it must match the trigger
`owner_address` recorded as the governed owner in SCCP source material.

The `sourceBridgeConfigHash()` view returns
`keccak256(abi.encode(keccak256("iroha:sccp:tron-source-bridge-config:v1"),
address(this), networkId, sourceDomain, targetDomain, owner))`. Operators can
record this value with the deployed address, runtime bytecode hash, and
deployment receipt so governed source-bridge rollout evidence binds the exact
lane metadata and current owner. Deployment emits `SourceBridgeConfigured` with
the same config hash. Ownership transfers automatically emit
`SourceBridgeConfigHash` for the new owner, and the owner can call
`emitSourceBridgeConfigHash()` later for rollout audits.

Production source material must record the deployed TVM contract address as the
governed source bridge emitter address, the bytes32 `networkId`, the current
owner address, and the resulting config hash. Because `networkId`,
`sourceDomain`, and `targetDomain` are Solidity immutables, the deployed
runtime bytecode hash is also lane-specific and must be captured in the source
material plus matching source-adapter deployment record before production
admission can open the lane. The Rust verifier checks production transaction
source-call proofs against the configured owner in
`TriggerSmartContract.owner_address`.

`SccpTronGroth16Bn254MessageVerifier.sol` inherits the audited
`SccpGroth16Bn254MessageVerifier` implementation used by EVM-family lanes. The
wrapper exists so TRON deployments have a stable artifact path and contract name
while preserving the same immutable verifying-key hash, nine BN254 public
signals, and proof tuple. The TRON entrypoint adds
  `submitSccpMessageProof(bytes,bytes32[6],bytes32)`, preflights the cleartext
  Groth16 envelope version, message id, source-domain word, and commitment root
  against the supplied public-input lane before verifier dispatch, recomputes the
  deployment-specific TRON destination binding from `address(this)`, the target
  network id, expected source/target domains, verifier backend, proof family,
  the actual deployed runtime bytecode hash exposed by `verifierCodeHash()`, and
  the immutable verifying-key hash, then marks accepted message ids as used.
The `destinationBindingHash()` view exposes the same value for canary tooling and
operator audits before any proof is submitted. After deployment, anyone can call
`emitDestinationBindingConfigured()` to emit `DestinationBindingConfigured`, so
rollout evidence can use either that event or the view as the observed value
passed to `--expected-destination-binding-hash`.

The TRON deployment flow must record the deployed TVM bytecode hash and
`verifyingKeyHash()` value in SCCP destination binding material. The Rust helper
`build_sccp_tron_destination_binding(...)` rejects missing or zero code/key
hashes, malformed base58check verifier addresses, non-SORA source-domain
material, non-TRON target-domain material, and non-`stark-fri-v1` proof
families. Rust TRON package construction and proof-structure verification also
parse the deployment binding key and recompute this hash, so a forged binding
hash is rejected even if the ABI envelope is rebuilt around it. The constructor
stores only the key hash, network id, source/target
domains, and proof family; it does not accept a self code-hash parameter because
that would make the runtime bytecode hash self-referential. The constructor
rejects a missing key hash, a mismatched verifying-key hash, an empty proof
family, any proof family other than `stark-fri-v1`, a zero network id,
any non-SORA source-domain id, a zero or non-TRON target domain, and
same-source/target domain pairs.

The offline helper `scripts/sccp_tron_source_bridge_evidence.py` can recompute
the TRON -> SORA source-bridge config hash and the TRON destination binding hash
from the base58 wrapper address, network id, source/target domains, proof
family, deployed bytecode hash, and verifier-key hash. Production TOML rendering
requires `--expected-config-hash`; the source-adapter `adapter_verifier_vk_hash`
is derived from the canonical TRON -> SORA FastPQ/OpenVerify verifier profile
and any manually supplied value must match it. Full TOML rendering also requires
`--expected-source-verifier-material-hash`,
`--expected-source-adapter-engine-deployment-hash`, and
`--expected-tron-dpos-source-gate-hash` before source/source-adapter records are
treated as production-pinned. The DPoS source gate is the canonical
`sccp:tron:dpos-source-gate:v1` transcript over the governed source material,
source-adapter deployment, adapter verifier key, TRON verifier role hashes,
source bridge config, verifier transcript prefixes, and bounded proof shapes.
Full TOML also requires `--expected-destination-binding-hash`, plus the
governed `--route-allowlist-hash` and complete `--route-canary-transaction-*`
metadata.
That metadata includes `--route-canary-call-data-sha256`,
`--route-canary-payload-hash`, `--route-canary-target-domain`,
`--route-canary-finality-height`, `--route-canary-finality-block-hash`,
`--route-canary-proof-version`, `--route-canary-proof-source-domain`,
`--route-canary-transaction-owner-address`, and the
`--route-canary-raw-data-owner-matches-transaction` assertion from live
transaction readback, so direct full TOML cannot skip the owner binding between
visible transaction JSON, hashed `raw_data_hex`, the exact submitted
`submitSccpMessageProof(...)` calldata, and the recovered transaction
signature signer. Rendering production TOML from saved live JSON also reparses
the carried `raw_data_hex` and raw signature before accepting the owner,
selector, public-input, proof-header, and signature-recovery metadata.
When `--route-canary-evidence-hash` is also supplied it must match the
transaction-derived value, so rollout audits must compare every source,
source-adapter, destination, route, and post-deploy canary hash with governed
live values before enabling routes. Compact JSON and
full TOML output also include the canonical
`SccpDestinationBindingV1.key` string derived from the same inputs, matching the
key/hash pair accepted by the Rust TRON relay verifier. When complete source
material is supplied, compact JSON also includes the canonical source material
and source-adapter deployment record hashes plus the canonical TRON DPoS source
gate hash. `toml_ready` remains false until all three expected source pins
match. Both TOML modes emit those hashes as comments next to the records they
digest, matching Rust's
`sccp_source_verifier_material_hash(...)` and
`sccp_source_adapter_engine_deployment_hash(...)` helpers. The compact JSON,
source TOML, and full rollout TOML
output modes are mutually exclusive, so a single command cannot silently choose
a broader rollout artifact than requested.
Full rollout TOML is locked to the paired production lane: TRON -> SORA source
material plus SORA -> TRON destination binding under the `stark-fri-v1` proof
family. Compact JSON dry-runs use the same lane checks before hashing, so source
config hashing is limited to the TRON -> SORA lane accepted by
`SccpTronSourceBridge` and destination binding hashing is limited to the
SORA -> TRON lane accepted by `SccpTronGroth16Bn254MessageVerifier`. Full TOML
now writes `destination_network_id`, `destination_binding_key`, and
`destination_binding_hash` into the `zk.sccp_destination_rollouts` record; the
same binding hash/key comments remain next to the record as audit hints.
It also recomputes the governed route allowlist hash from the canonical TRON
source material record hash, source-adapter deployment record hash, and
SORA -> TRON destination binding hash, then rejects a supplied
`--route-allowlist-hash` that does not match that exact evidence tuple.
For operator source-event rollouts, compact JSON also accepts
`--source-event-digest` and emits the exact
`submitSccpSourceEvent(uint32,uint32,bytes32)` calldata plus the unsigned
`/wallet/triggersmartcontract` request body expected from the owner transaction.
TOML modes reject that one-off call payload so deployment evidence and
transaction execution material stay separate. The live collector exposes the
same JSON-only source-event calldata for a queried source bridge and emits
`offline_source_event_args` for replaying the derivation through the direct
helper while keeping the live `full_toml_ready` readiness flag in JSON output.
Those replay arguments are rebuilt from the source bridge domains, owner,
digest, and canonical `submitSccpSourceEvent` calldata, and post-submit JSON
must also replay the saved raw transaction/signature summary, including a
canonical successful source-proof transaction `Result` payload, before the
helper emits `offline_source_event_args`.
It also reads `submittedSourceEvents(bytes32)`: pre-submit mode refuses to emit
an unsigned trigger request for an already submitted digest, while post-submit
mode with `--source-event-transaction-id` requires that mapping to be true. The
live JSON includes an unsigned `source_event_call.trigger_request` body for
`/wallet/triggersmartcontract`; operators still sign and broadcast that
transaction outside the read-only helper. After broadcast,
`--source-event-transaction-id` verifies the successful transaction readback and
the emitted exact two-topic `SccpSourceEvent(bytes32)` log against the same
digest. It also verifies the raw transaction body is a single
`TriggerSmartContract` call whose `contractRet = SUCCESS`; an explicitly
present top-level `ret` must be java-tron's default `SUCESS = 0`, while an
omitted default `ret` is accepted. The owner, contract, and calldata must match
the governed source bridge owner, source bridge address, and digest. The same
raw readback must include
`raw_data_hex` that hashes to the requested `txID` and exactly one canonical
65-byte low-S TRON recoverable secp256k1 signature that recovers to the source
bridge owner. The visible log address/data and `TriggerSmartContract.data`
readback must be lowercase exact hex, so uppercase or `0X` aliases cannot be
normalized into accepted evidence; any supported result-extension byte fields
used while reconstructing block transaction bytes follow the same rule. The
helper parses the signed `raw_data_hex` protobuf and requires the embedded
`TriggerSmartContract` owner, contract, calldata, ref-block, expiration,
timestamp, and fee-limit fields to match the production transaction-source
verifier's source-call profile. JSON readback also emits the
canonical transaction protobuf bytes, SHA-256 transaction hash, and transaction
Merkle branch used by the transaction-source proof builder. When operators
provide `--receipt-root` and repeated `--source-inclusion-branch-hex` values,
the helper derives the canonical `sccp:tron:transaction-source-proof:v1`
bytes/hash and compares `--receipt-proof-hash` when present. It then fetches
the containing block, rebuilds
the canonical block-header `raw_data` hash and TRON block id, fetches the
immediate parent block, verifies the child `parentHash` and monotonic
timestamp, recovers both child and parent header signatures to their declared
TRON witness addresses, and recomputes the java-tron transaction Merkle root
from the block's canonical transaction protobuf bytes to match `txTrieRoot`. The
JSON also emits the canonical `solid_block_header_proof` bytes/hash when the
child and parent headers carry the non-zero account-state roots required by the
shared SCCP transcript helper; otherwise it reports the proof blocker. The
helper also accepts a canonical `--witness-schedule-payload-hex` or
`--witness-schedule-payload-file`, derives the
`sccp:tron:witness-schedule:v1` hash, optionally checks
`--expected-witness-schedule-hash`, and requires the child and parent block
witnesses to be schedule members. When operators also provide `--receipt-root`,
`--receipt-proof-hash`, `--witness-seal-signers-bitmap-hex`, and repeated
`--witness-seal-signature-hex` values, JSON derives the canonical
`sccp:tron:solid-block-message:v1` bytes/hash and the
`sccp:tron:witness-seal:v1` bytes/hash, verifies the signatures recover to the
selected schedule witnesses before canonical seal serialization, enforces
strict `> 2/3` signed weight, and optionally checks
`--expected-witness-seal-hash`; missing seal inputs stay visible as a proof
blocker. Operators can set
`--solid-block-ancestor-depth` and `--solid-block-confirmation-depth` to fetch
the bounded signed headers required by non-placeholder TRON material; JSON
verifies backward ancestor linkage, forward confirmation linkage, active
witness membership, monotonic timestamps, and strict `> 2/3` unique
confirmation weight, or reports the missing header evidence as blockers. If
the active schedule differs from the governed source trust-anchor hash,
repeated `--witness-schedule-transition-json` values can supply the
parent/next schedule payloads, transition block, signer bitmap, and signatures;
JSON anchors each transition block to the solid, parent, or ancestor headers
requires transition signatures to recover to the selected parent-schedule
witnesses with strict `> 2/3` signed weight, and marks production readiness only
when the canonical transition chain reaches the active schedule. With
`--solid`, those readbacks use
`/walletsolidity/gettransactioninfobyid`,
`/walletsolidity/gettransactionbyid`, and `/walletsolidity/getblockbynum`.
The helper can also derive
`source_bridge_emitter_code_hash` and `destination_verifier_code_hash` from
hex-encoded deployed runtime bytecode, failing if a manually supplied hash does
not match the derived value. Its reusable hash functions reject zero or
wrong-width addresses, network ids, code hashes, and verifier-key hashes before
computing production evidence values.

For deployed contracts, `scripts/sccp_tron_live_evidence.py` performs the
read-only collection step against a TRON full-node or TronGrid-compatible HTTP
API. It calls the source bridge and destination verifier view functions through
`/wallet/triggerconstantcontract`, optionally reads `/wallet/getcontract`
bytecode metadata, recomputes `sourceBridgeConfigHash()` and
`destinationBindingHash()`, and prints the matching arguments for
`scripts/sccp_tron_source_bridge_evidence.py` plus the `network_id_hex`,
`tron_verifier_address`, `verifier_code_hash_hex`, `verifier_key_hash_hex`, and
`expected_destination_binding_hash_hex` query fields expected by Torii SCCP
artifact/job requests only after the operator-supplied
`--expected-destination-binding-hash` matches the deployed verifier view. Torii
recomputes the destination binding from the live deployment fields and rejects
the request if it does not match the expected hash read from the deployed
verifier. SDK helpers and the bridge-feature CLI validate
`tron_verifier_address` as a checksummed non-zero TRON Base58Check address
before sending proof-job or bridge-submit requests. The JSON also sets
`torii_destination_query_proof_bytes_hex_required = true` because those
deployment fields still need the prover's `proof_bytes_hex` tuple before they
form a complete Torii artifact/job request. The Rust, Python, and JavaScript
typed clients plus the bridge-feature CLI enforce the same two-way rule before
sending artifact/job queries: deployment fields require `proof_bytes_hex`, and
`proof_bytes_hex` requires deployment fields. The helper never
signs, broadcasts, deploys, or mutates chain state;
a mismatch in the observed source config hash, destination binding hash,
source/destination `networkId()`, or production lane fails before any rollout
evidence is rendered. If the governed source component hashes,
source-adapter deployment receipt hash, and expected source record hashes are
supplied, the live helper also recomputes the canonical source verifier
material, source-adapter deployment record, and TRON DPoS source gate hashes
before printing offline rollout arguments. Live full-TOML output requires all
three expected source pins to match. Operators can also pass
`--expected-source-bridge-config-hash` with the deployment or governed
`SourceBridgeConfigured` value; owner or lane drift then fails before rollout
arguments are emitted, and this pin is required for direct full-TOML output.
The direct TOML renderers apply the same runtime-bytecode hash derivation and
mismatch checks as the CLI before emitting governance records, so programmatic
rollout tooling cannot bypass the bytecode pin by calling them directly.
Inline runtime bytecode, runtime bytecode files, fixed-width deployment hashes,
and hex-form TRON addresses must be lowercase hex with a lowercase `0x` prefix
when a prefix is present.
When that source-config pin, those source records, destination
evidence, `--expected-destination-binding-hash`, and
`--route-allowlist-hash` are all present, the JSON includes
`offline_full_toml_args`, the exact offline renderer argument list ending in
`--full-toml`, plus `offline_full_toml_sha256` for the internally rendered
governance TOML. The expected destination binding hash must match
`destinationBindingHash()`, and the supplied route hash must also match the
canonical source-material, source-adapter-deployment, and destination-binding
tuple before those fields are emitted. Supplying
`--route-canary-transaction-id <txid>` makes the live helper read the
destination verifier transaction receipt, verify the `MessageProofAccepted`
log against the deployed binding/backend/family/network views, require exactly
one matching accepted-proof log, fetch the raw `TriggerSmartContract`
transaction, parse the hashed `raw_data_hex`, require
the visible transaction `owner_address` to match the owner encoded in that raw
transaction body, require the canonical low-S recoverable secp256k1 signature
to recover to that transaction owner, and verify that its
`submitSccpMessageProof(bytes,bytes32[6],bytes32)` selector, ABI public inputs,
statement hash, 384-byte proof tuple, and proof header match the accepted event
and the deployed verifier domains. The raw transaction `data` readback is also
lowercase exact hex, matching the canonical event/log parsing used for the
source-event path. It also queries
`usedMessageProofs(messageId)` on the same verifier and requires the accepted
message id to be marked consumed in current contract state. It then derives the
route canary evidence hash bound to the validated route allowlist. The
`iroha:sccp:tron-route-canary-evidence:v3` transcript commits the exact
`submitSccpMessageProof(...)` calldata SHA-256, decoded payload hash, target
domain, finality height, finality block hash, proof version, proof source
domain, transaction owner address, transaction block number/timestamp,
raw-data owner binding flag, signature SHA-256, recovered signer address, and
owner-recovery flag alongside the
accepted event tuple and governed verifier/backend/network pins. Live
full-TOML output requires this verified transaction-derived canary evidence;
supplying both `--route-canary-transaction-id` and `--route-canary-evidence-hash`
requires the manual hash to match that transaction-derived value. The all-lanes
preflight requires the emitted transaction metadata comments for TRON and
recomputes the same canary evidence hash, so hand-edited live TOML cannot drift
from the accepted proof event, the submitted verifier call, or the
`usedMessageProofs` state check, and must carry
`sccp_tron_route_canary_transaction_owner_address`,
`sccp_tron_route_canary_raw_data_owner_matches_transaction = "true"` plus the
signature hash, recovered address, and
`sccp_tron_route_canary_signature_recovers_to_owner = "true"` audit comments. The
release-bundle verifier also rejects readiness/all-lanes JSON where either TRON
owner address is zero, any route-canary binding hash or transaction transcript
word is zero, any distinct TRON canary hash role is reused, or the recovered
TRON address drifts from the transaction owner. The replayed JSON-to-TOML gate
also revalidates the submitted selector, proof tuple
length/version/source domain, public-input message id, target domain, commitment
root, statement hash, event source domain, destination binding, backend, proof
family, network id, and recomputed canary hash before emitting production TOML.
Source-event transaction readback uses the same single-governed-log policy for
`SccpSourceEvent(bytes32)`, so ambiguous TRON receipts fail closed instead of
choosing the first matching log.
The offline direct renderer also requires the same `--route-canary-transaction-*`
metadata plus `--route-canary-used-message-proof` and
`--route-canary-raw-data-owner-matches-transaction`, plus the
`--route-canary-signature-*` recovery metadata for full TOML, requires the
recovered address to match the transaction owner address, can derive the canary
evidence hash from it, and rejects a manually supplied canary hash that does
not match. Passing
`--full-toml` to the live
collector prints that verified TOML directly instead of JSON only after the
source config, source-record, DPoS source-gate, destination binding, route
allowlist, and route canary checks are all pinned and verified. A supplied
`--source-bridge-emitter-code-hash` must match
`/wallet/getcontract` runtime bytecode when metadata lookup is enabled; if a
node omits source-bridge bytecode, source-record preflight fails unless
operators explicitly pass `--no-getcontract` and supply an independently audited
code hash. Destination verifier metadata must include bytecode and match the
deployed contract's `verifierCodeHash()` view. The live collector also checks
the verifier's `verifierBackendHash()` and `proofFamilyHash()` views against
`tron-groth16-bn254-v1` and `stark-fri-v1`, so a deployment cannot look
rollout-ready under a different backend or proof family. Full-TOML output
records those two hashes next to the destination rollout, and the all-lanes
preflight requires the metadata to match the canonical profile. Pass `--solid`
to read those view functions from
`/walletsolidity/triggerconstantcontract` when rollout evidence should be
collected from TRON's confirmed state. For TronGrid production endpoints, pass
`--tron-pro-api-key-file <runtime-secret-file>` or `--tron-pro-api-key` to send
`TRON-PRO-API-KEY`; the key is not included in the JSON evidence output.
The repository all-lanes preflight consumes that full-TOML output as the TRON
slice of a complete SCCP rollout bundle and recomputes the TRON source
material, source-adapter deployment, DPoS source-gate, route allowlist, and
route canary records from lowercase exact fixed-width hex metadata before
marking the lane ready.

```bash
# Diagnostic live readback. This prints JSON and is useful for discovering
# governed hashes, but it is not enough for production full-TOML cutover.
python3 scripts/sccp_tron_live_evidence.py \
  --tron-node-url https://api.trongrid.io \
  --solid \
  --tron-pro-api-key-file <runtime-secret-file> \
  --source-bridge-address <deployed-source-bridge> \
  --destination-verifier-address <deployed-destination-verifier> \
  --expected-destination-binding-hash <sccp-tron-destination-binding-hash>
```

```bash
# Production cutover evidence. The helper derives source-bridge and verifier
# runtime code hashes from /wallet/getcontract and prints verified full TOML
# only after every expected governed pin and route canary check matches.
python3 scripts/sccp_tron_live_evidence.py \
  --tron-node-url https://api.trongrid.io \
  --solid \
  --tron-pro-api-key-file <runtime-secret-file> \
  --source-bridge-address <deployed-source-bridge> \
  --destination-verifier-address <deployed-destination-verifier> \
  --expected-source-bridge-config-hash <source-bridge-config-hash> \
  --source-trust-anchor-hash <tron-witness-schedule-hash> \
  --consensus-verifier-hash <tron-dpos-consensus-verifier-hash> \
  --message-inclusion-verifier-hash <tron-transaction-source-verifier-hash> \
  --finality-policy-hash <tron-finality-policy-hash> \
  --deployment-receipt-hash <source-adapter-deployment-receipt-hash> \
  --expected-source-verifier-material-hash <source-material-record-hash> \
  --expected-source-adapter-engine-deployment-hash <source-deployment-record-hash> \
  --expected-tron-dpos-source-gate-hash <tron-dpos-source-gate-hash> \
  --expected-destination-binding-hash <sccp-tron-destination-binding-hash> \
  --route-allowlist-hash <sora-tron-route-allowlist-hash> \
  --route-canary-transaction-id <accepted-message-proof-txid> \
  --route-canary-transaction-owner-address <0x41-prefixed-transaction-owner> \
  --route-canary-call-data-sha256 <submit-message-proof-calldata-sha256> \
  --route-canary-payload-hash <submitted-public-input-payload-hash> \
  --route-canary-target-domain 5 \
  --route-canary-finality-height <submitted-finality-height-word> \
  --route-canary-finality-block-hash <submitted-finality-block-hash> \
  --route-canary-proof-version 1 \
  --route-canary-proof-source-domain 0 \
  --full-toml
```

`submitSccpMessageProof(...)` fails closed before verifier dispatch on a zero
statement hash, zero message id, zero payload hash, zero commitment root, zero
finality height, zero finality block hash, or a target-domain word that does not
match the configured TRON lane. It also requires `proofBytes` to be the exact
12-word static Groth16 ABI tuple, decodes the envelope's cleartext source-domain
word before verifier dispatch, and rejects values wider than `uint32` or values
that do not match the configured source lane; the proof header's message id and
commitment root must also match `publicInputs[0]` and `publicInputs[3]`.
  Accepted proof events include the SCCP statement hash and destination binding
  hash alongside the message id, source domain, commitment root, verifier
  backend, proof family, and network id, so live canary logs can be matched to
  the exact governed statement and deployed binding. The inherited BN254 verifier
  then enforces the Groth16 proof tuple, public-signal derivation, and pairing
  check.
The shared contract smoke constructs a deterministic self-consistent BN254 proof
for the test verifying key and submits it through this TRON wrapper, asserting
the public-input preflight negatives, source-domain overflow rejection, zero
proof-point, off-curve G2, non-prime-subgroup G2, wrong-statement, and
post-generation payload/finality-height/finality-block signal drift,
wrong-deployment-binding negatives, the `MessageProofAccepted` event fields, and
replay rejection after acceptance.

Reference TRON sources:

- TRON TVM docs describe TVM as EVM-compatible for Solidity contracts and note
  the TRON-specific address/resource differences.
- TRON TIP-176 is final and covers altbn128 operation energy changes in TVM.
