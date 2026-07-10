# SCCP BSC contracts

This directory contains the concrete exact V1 XOR route for BNB Smart Chain.
There is no owner-controlled source emitter: a BSC-origin SCCP event can only be
emitted by the value-moving route after wrapped XOR is burned.

## Contracts

- `TairaXOR.sol` is the wrapped BEP20/ERC20 XOR token built from the shared
  exact EVM token. Its sole mint/burn route is fixed in the constructor; there
  is no owner, setter, lock flag, or upgrade hook.
- `TairaXorBscSccpBridge.sol` specializes the shared exact EVM route for BSC
  mainnet or testnet and the fixed `taira_bsc_xor` route.

The route constructor pins the token code hash, verifier address/runtime code
hash/verifying-key hash, BSC profile and chain id, lane hashes, destination
binding, nonzero route revision, and route-config hash. The revision is encoded
after the Transfer nonce and prevents message-id reuse when a replacement route
restarts its local nonce. The destination binding includes both verifier and
route addresses, so a proof for one deployment cannot be replayed through
another route that shares its verifier.

`transferToTaira(bytes,uint256)` constructs the complete canonical Transfer
payload, burns wrapped XOR, and emits the exact six-field `SccpTransfer` event.
`finalizeFromTaira(bytes,bytes32[6],bytes32,bytes)` parses the canonical payload,
checks the fixed asset/route/domains/address codec and lane-derived message id,
verifies the route-bound Groth16 statement, records replay state, and mints the
scaled wrapped amount. All checks and replay writes precede external token
state changes, and both paths are non-reentrant.

Governed deployment state must use fixed-width typed addresses and hashes and
derive the destination binding and route-config hash. Clients do not supply
deployment identities, aliases, readiness flags, browser provers, or an
operator-selected expected binding.

Before activation, independently verify:

1. Token, verifier, and route are distinct nonzero contracts with the exact
   governed runtime code hashes.
2. The route profile and chain id are BSC mainnet (`56`) or BSC testnet (`97`)
   as intended.
3. Verifier key, both lane hashes, destination binding, and route-config hash
   match the typed route revision.
4. `TairaXOR.bridge()` is the route, and the token ABI/runtime exposes no
   owner or bridge-mutation entrypoint. Precompute the route address, deploy
   the token with it, then deploy that exact route next.
5. The Groth16 key belongs to an audited circuit proving the complete SCCP
   statement, not merely the public-signal wiring.
6. The optimized route runtime is at most 24,576 bytes and its BLAKE2b-256
   results match the software reference across compression-block boundaries;
   BSC uses the EIP-152 BLAKE2F precompile.

Deployment and signing are out of process. Never persist deployer keys or
wallet secrets in manifests, evidence, scripts, or repository files.

Run the exact shared suite with:

```bash
bash scripts/sccp_evm_contract_smoke.sh
```

It covers exact cross-language vectors, mint/burn accounting, zero and
mismatched revisions, malformed and cross-network payloads, replay,
reentrancy, token failure, immutable code/key drift, deployment-size and
BLAKE2F parity, route-binding separation, and adversarial BN254 inputs.
