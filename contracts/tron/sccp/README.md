# SCCP TRON contracts

This directory contains the exact V1 XOR route between SORA Taira and TRON.
There is no generic owner-controlled emitter and no contract in this directory
accepts an arbitrary SCCP digest.

## Contracts

- `TairaXOR.sol` is the wrapped TRC20-compatible XOR token. Exactly one route
  contract may mint or burn it, and that route is immutable from the token
  constructor. There is no owner, bridge setter, lock flag, or upgrade hook.
- `SccpTronGroth16Bn254MessageVerifier.sol` is an immutable, stateless TVM
  wrapper around the shared BN254 verifier. It pins the verifying key, backend,
  network, and source/target domains. It deliberately has no
  direct proof-submission or replay state.
- `TairaXorSccpBridge.sol` is the value-moving route. It validates exact
  payloads, owns replay protection, burns wrapped XOR for TRON-to-Taira
  transfers, verifies Taira finality proofs, and mints wrapped XOR for
  Taira-to-TRON transfers.

## Exact source event

`transferToTaira(bytes,uint256,uint64)` accepts a canonical Taira account
payload, a raw token amount, and the caller's exact expected transfer nonce,
then constructs the complete SCCP Transfer payload in the contract. The call
requires `expectedNonce == transferNonce`, so the successful native TRON
transaction authenticates the same nonce used by the emitted payload and
message id. Wrapped XOR has 18 decimals while the Taira payload uses 9, so raw
token amounts must be divisible by `10^9` and the payload commits the quotient.
It burns before emitting:

```text
SccpTransfer(
  indexed laneHash,
  indexed messageId,
  indexed sourceEventDigest,
  payloadHash,
  routeConfigHash,
  canonicalPayload
)
```

The event digest is
`keccak256("sccp:source:event:v1" || 0x01 || laneHash || messageId || payloadHash)`.
The event cannot be produced without a successful token burn, and the route
rejects zero amounts, stale or future expected nonces, nonce exhaustion,
malformed recipients, changed token code, and replayed message ids. Every
payload commits the constructor-bound,
nonzero governed route revision immediately after the nonce, preventing a new
route whose nonce restarts at zero from colliding with an earlier revision.
The irreversible recipient is specifically the exact discriminant-`369`
`test...` I105 spelling of a canonical, non-weak single-key Ed25519 account.
This is intentionally narrower than the proof-authenticated Taira sender on the
reverse path, which may use a canonical Ed25519/secp256k1 single or multisig
controller. Unsupported controller algorithms are rejected on Taira before an
outbound lock is created.

## Exact destination binding

`finalizeFromTaira(bytes,bytes32[6],bytes32,bytes)` parses the canonical SCCP
payload and requires the fixed `xor` asset, `taira_tron_xor` route, SORA-to-TRON
domains, TRON address codec, nonzero recipient, exact payload hash, and exact
lane-derived message id before verifier dispatch. A verified payload amount is
multiplied by `10^9`, with an explicit overflow check, before minting.

The Groth16 public statement commits a route-specific destination binding:

```text
keccak256(abi.encode(
  keccak256("iroha:sccp:tron-destination-binding:v1"),
  keccak256("tron-groth16-bn254-v1"),
  networkId,
  sourceDomain,
  targetDomain,
  tronAddressWord(verifier),
  tronAddressWord(routeBridge),
  verifierRuntimeCodeHash,
  verifyingKeyHash,
  semanticProofProfileHash,
  soraFinalityAnchorHash
))
```

Including both contract addresses prevents a proof accepted for one token route
from being replayed through another route that shares the same verifier. The
route pins the verifier runtime code and key, supplies its own immutable binding
to the stateless verifier, and records consumed destination message ids before
minting. The verifier contract therefore remains safely shareable without
pretending that a verifier-only hash identifies a value-moving route.
The route's separate `routeConfigHash` is public signal 9 (the tenth signal), so
the proof directly commits the governed token, code hashes, lane hashes,
profile, destination binding, and route revision instead of relying only on a
post-proof contract check. Public signal 10 (the eleventh signal) separately
binds the governed SORA finality anchor. Both policy hashes are immutable
verifier getters, are pinned by the route's typed `VerifierPolicyV1` constructor
tuple, and are rechecked before every proof dispatch.

Governed route state must store typed TRON addresses and fixed-width hashes and
derive this binding; clients must never select deployment identity with request
fields or an operator-provided “expected binding” alias.

## Deployment requirements

Before activating a route, independently verify all of the following:

1. The token, verifier, and route are distinct nonzero deployed contracts.
2. The verifier network is the intended TRON profile and its domains are
   SORA-to-TRON.
3. `verifierCodeHash`, `verifierKeyHash`, `semanticProofProfileHash`,
   `soraFinalityAnchorHash`, `tokenCodeHash`,
   `destinationBindingHash`, both lane hashes, and `routeConfigHash` match the
   governed typed deployment record.
4. Precompute the route address, deploy the token with that exact immutable
   bridge, then deploy the route at the precomputed address with the exact
   token address. Verify `token.bridge()`, `route.token()`, token code, and the
   governed token code hash. Neither contract exposes an owner or
   bridge-mutation entrypoint.
5. The Groth16 verifying key belongs to an independently audited,
   reproducibly generated circuit and witness generator that prove canonical
   payload semantics, message inclusion, block commitment, commit-QC finality,
   and validator continuity from the governed Taira anchor. The checked-in
   labeled-signal fixture is diagnostic-only: production validators reject its
   identifier, classification, and exact published digest. Neither that
   fixture, a syntactically valid key, nor a pairing test is evidence of those
   semantics.
6. A finalized adversarial canary proves route-specific binding, correct
   mint/burn accounting, and replay rejection.
7. The optimized route runtime remains within the deployment-size ceiling and
   its software BLAKE2b-256 implementation matches the EIP-152 reference across
   compression-block boundaries. TVM cannot use EVM precompile address `0x09`
   because TRON assigns that address to `BatchValidateSign`.

Deployment, signing, and broadcasting are intentionally out of process. Do not
store deployer keys, bearer tokens, or wallet secrets in repository artifacts,
route manifests, evidence bundles, or documentation.

Production sources require exact Solidity `0.7.4`. The governed TVM artifact
is built with authenticated `0.7.4+commit.3f05b770`, optimizer run count `200`,
and the `istanbul` opcode target. Its exact compiler, complete
standard-json input, ABI, creation/runtime bytes, immutable-runtime patch
ranges, and hashes are release-policy inputs and must match the governed route
exactly; ordinary EVM execution is never accepted as TVM evidence.

## Testing

Run the shared exact-contract suite from the repository root:

```bash
bash scripts/sccp_evm_contract_smoke.sh
```

The suite verifies every production artifact against the target-specific
source-map and artifact locks. It executes the exact reviewed TRON creation
code under an EVM compatibility runtime, while still treating the real-TRE
phase as the only TVM deployment evidence. It covers
canonical cross-language vectors, zero and mismatched revisions, malformed
payloads and codecs, hash-role separation, wrong networks/routes/assets, token
failures, reentrancy, changed code/key identities, deployment-size and
BLAKE2b-parity checks, Groth16 point and public-input failures, replay, and the
cross-route TRON proof attack described above.

Release TVM evidence is produced only by `scripts/contract_tvm_runner.sh` on
the immutable official TRE image. The runner snapshots the authenticated
manifest and checked-in Rust native-transfer vectors once into a private,
read-only temporary directory, verifies that snapshot against both artifact
locks and the current standard-JSON source inputs, and passes only the snapshot
to static and live execution. The live test recomputes lane, payload, message,
source-event, destination-binding, and route-configuration hashes independently;
checks BLAKE2b-256 boundaries around 128- and 256-byte compression blocks; and
decodes the emitted `SccpTransfer` log independently. A negative canary counts
only when TRE returns an identity-matched, included receipt with a non-success
TVM result. Transport, ABI, broadcast, and receipt-timeout errors fail the gate
and are never reported as contract rejection evidence.
