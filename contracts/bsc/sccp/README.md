# SCCP BSC contracts

This directory contains the concrete exact V1 XOR route for BNB Smart Chain.
There is no owner-controlled source emitter: a BSC-origin SCCP event can only be
emitted by the value-moving route after wrapped XOR is burned.

## Contracts

- `TairaXOR.sol` is the wrapped BEP20/ERC20 XOR token built from the shared
  exact EVM token. Its sole mint/burn route is fixed in the constructor; there
  is no owner, setter, lock flag, or upgrade hook.
- `TairaXorBscSccpBridge.sol` specializes the shared exact EVM route for BSC
  mainnet (profile `0x42`, chain id `56`) and the fixed `taira_bsc_xor` route.

The route constructor pins the token code hash, verifier address/runtime code
hash/verifying-key hash, semantic-proof-profile hash, Taira-finality-anchor
hash, BSC profile and chain id, lane hashes, destination binding, nonzero route
revision, positive u128-sized maximum wrapped supply, and route-config hash. The
immutable cap is committed as the final asset-route word and every mint rejects
`totalSupply + amount` above it. The revision is encoded after the Transfer
nonce and prevents message-id reuse when a replacement route restarts its local
nonce. The destination binding includes both policy hashes and both verifier
and route identities, followed by the replay-verifier address/runtime hash and
mint-breaker address/runtime hash. The route configuration commits the same
quartet. A proof for one deployment therefore cannot be replayed through
another route that shares its verifier or substitutes either helper contract.
The route exposes both helper runtime hashes and rechecks the immutable
mint-breaker hash and one-way disabled latch before each new mint admission.
Three of the constructor-bound five guardians may permanently disable minting;
the breaker cannot re-enable minting or block outbound burns.
The governed destination deployment requires the corresponding typed
`outbound_proof_policy`; policy-less JSON and Norito records are invalid.

`transferToTaira(bytes,uint256,bytes)` constructs the complete canonical
Transfer payload, verifies and occupies the caller's sparse-Merkle replay leaf,
burns wrapped XOR, and emits the exact six-field `SccpTransfer` event.
`finalizeFromTaira(bytes,bytes32[6],bytes32,bytes,bytes)` parses the canonical
payload, checks the fixed asset/route/domains/address codec and lane-derived
message id, verifies the route-bound Groth16 statement and canonical replay
witness, and mints the scaled wrapped amount subject to the immutable supply
cap. All checks and replay-root writes precede external token state changes,
and both paths are non-reentrant.

The burn recipient must be the exact discriminant-`369` `test...` I105 spelling
of a canonical, non-weak single-key Ed25519 account. A proof-authenticated Taira
sender may be a canonical single or multisig AccountId composed from Ed25519
and compressed secp256k1 keys. Taira applies the same closed controller policy
before locking funds; other valid Rust controller algorithms cannot create an
outbound message that this immutable destination parser would reject.

Governed deployment state must use fixed-width typed addresses and hashes and
derive the destination binding and route-config hash. Clients do not supply
deployment identities, aliases, readiness flags, browser provers, or an
operator-selected expected binding.

Before activation, independently verify:

1. Token, verifier, replay verifier, mint breaker, and route are distinct
   nonzero contracts with the exact governed runtime code hashes.
2. The route profile and chain id are exactly BSC mainnet (`0x42`, `56`). No
   testnet profile is accepted by the first-release contract.
3. Verifier key, `semanticProofProfileHash`, `soraFinalityAnchorHash`, both lane
   hashes, destination binding, route-config hash, `replayVerifierCodeHash`,
   `mintBreakerCodeHash`, guardian set, and maximum wrapped supply match the
   typed deployment and route revision.
4. Precompute the route address, deploy `TairaXOR` with that exact immutable
   bridge, then deploy the route at the precomputed address with the exact
   token address. Verify `TairaXOR.bridge()`, `route.token()`, token code, and
   token code hash before activation. Neither contract exposes an owner or
   bridge-mutation entrypoint.
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
mismatched revisions, malformed and cross-network payloads, sparse-Merkle
replay witnesses, reentrancy, token failure, immutable token/verifier/breaker
code drift, the one-way breaker with outbound burns still open, deployment-size
and BLAKE2F parity, route-binding separation, and adversarial BN254 inputs.
