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
  proof family, network, and source/target domains. It deliberately has no
  direct proof-submission or replay state.
- `TairaXorSccpBridge.sol` is the value-moving route. It validates exact
  payloads, owns replay protection, burns wrapped XOR for TRON-to-Taira
  transfers, verifies Taira finality proofs, and mints wrapped XOR for
  Taira-to-TRON transfers.

## Exact source event

`transferToTaira(bytes,uint256)` accepts a canonical Taira account payload and
constructs the complete SCCP Transfer payload in the contract. Wrapped XOR has
18 decimals while the Taira payload uses 9, so raw token amounts must be
divisible by `10^9` and the payload commits the quotient. It burns before
emitting:

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
rejects zero amounts, nonce exhaustion, malformed recipients, changed token
code, and replayed message ids. Every payload commits the constructor-bound,
nonzero governed route revision immediately after the nonce, preventing a new
route whose nonce restarts at zero from colliding with an earlier revision.

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
  keccak256("stark-fri-v1"),
  networkId,
  sourceDomain,
  targetDomain,
  tronAddressWord(verifier),
  tronAddressWord(routeBridge),
  verifierRuntimeCodeHash,
  verifyingKeyHash
))
```

Including both contract addresses prevents a proof accepted for one token route
from being replayed through another route that shares the same verifier. The
route pins the verifier runtime code and key, supplies its own immutable binding
to the stateless verifier, and records consumed destination message ids before
minting. The verifier contract therefore remains safely shareable without
pretending that a verifier-only hash identifies a value-moving route.
The route's separate `routeConfigHash` is the tenth Groth16 public signal, so
the proof directly commits the governed token, code hashes, lane hashes,
profile, destination binding, and route revision instead of relying only on a
post-proof contract check.

Governed route state must store typed TRON addresses and fixed-width hashes and
derive this binding; clients must never select deployment identity with request
fields or an operator-provided “expected binding” alias.

## Deployment requirements

Before activating a route, independently verify all of the following:

1. The token, verifier, and route are distinct nonzero deployed contracts.
2. The verifier network is the intended TRON profile and its domains are
   SORA-to-TRON.
3. `verifierCodeHash`, `verifierKeyHash`, `tokenCodeHash`,
   `destinationBindingHash`, both lane hashes, and `routeConfigHash` match the
   governed typed deployment record.
4. The token's immutable bridge equals the route and its ABI/runtime exposes
   no owner or bridge-mutation entrypoint. Precompute the route address, deploy
   the token with it, then deploy that exact route next.
5. The Groth16 verifying key belongs to an audited circuit that proves the
   complete SCCP statement semantics. A syntactically valid key or pairing test
   is not evidence of those semantics.
6. A finalized adversarial canary proves route-specific binding, correct
   mint/burn accounting, and replay rejection.
7. The optimized route runtime remains within the deployment-size ceiling and
   its software BLAKE2b-256 implementation matches the EIP-152 reference across
   compression-block boundaries. TVM cannot use EVM precompile address `0x09`
   because TRON assigns that address to `BatchValidateSign`.

Deployment, signing, and broadcasting are intentionally out of process. Do not
store deployer keys, bearer tokens, or wallet secrets in repository artifacts,
route manifests, evidence bundles, or documentation.

## Testing

Run the shared exact-contract suite from the repository root:

```bash
bash scripts/sccp_evm_contract_smoke.sh
```

The suite compiles every shared/Ethereum/BSC/TRON exact contract and covers
canonical cross-language vectors, zero and mismatched revisions, malformed
payloads and codecs, hash-role separation, wrong networks/routes/assets, token
failures, reentrancy, changed code/key identities, deployment-size and
BLAKE2b-parity checks, Groth16 point and public-input failures, replay, and the
cross-route TRON proof attack described above.
