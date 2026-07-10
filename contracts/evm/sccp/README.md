# SCCP EVM contracts

These contracts provide the shared exact V1 implementation used by Ethereum
and BNB Smart Chain routes.

## Components

- `SccpExactTransferCodec.sol` implements the canonical network, lane, Transfer
  payload, message-id, payload-hash, and source-event-digest encodings. EVM/BSC
  hashing uses the EIP-152 BLAKE2F precompile so the route remains below the
  24,576-byte runtime ceiling; the TRON route uses the deterministic software
  compressor because TVM assigns precompile address `0x09` to a different
  operation.
- `TairaXorEvmToken.sol` is the constructor-bound wrapped-token implementation;
  its sole route is immutable and it has no owner or bridge mutator.
- `TairaXorExactEvmSccpBridge.sol` is the concrete value-moving base route. The
  Ethereum and BSC wrappers fix their profiles and route identifiers.
- `ISccpMessageVerifier.sol` and `SccpGroth16Bn254MessageVerifier.sol` define
  and implement immutable BN254 verification for the ten exact SCCP signals.

Generic owner emitters and the secp256k1 attestation verifier are intentionally
absent. Generic proof-only message wrappers are also absent: accepting a proof
without executing the value-moving route is not settlement. A production
source event must be coupled to the concrete token burn, and a production
destination proof must call `finalizeFromTaira` on the immutable typed route.
The token must be deployed with the precomputed route address immediately
before the route deployment. The route constructor checks that immutable
back-reference and a nonzero governed route revision, eliminating a privileged
initialization window. The revision is encoded immediately after the Transfer
nonce and is included in `routeConfigHash`, so nonce reuse by a replacement
route cannot collide with an older route's message identity.

## Groth16 statement

Proof bytes are the exact static ABI tuple:

```text
abi.encode(
  uint256 version = 1,
  bytes32 message_id,
  uint256 source_domain,
  bytes32 commitment_root,
  uint256[2] a,
  uint256[4] b,
  uint256[2] c
)
```

The verifying key contains eleven G1 input-coefficient points: one constant plus
ten signals. Each signal is
`uint256(keccak256(abi.encode(label, value))) mod r`, in this order:

1. message id
2. payload hash
3. target domain
4. commitment root
5. finality height
6. finality block hash
7. source domain
8. statement hash
9. destination binding hash
10. route-configuration hash

The verifier rejects wrong tuple lengths, zero required words, domain overflow
or equality, noncanonical/zero/off-curve/non-subgroup points, and failed
pairings. It has no mutable signer set or update function. Route rotation
creates a new immutable route revision and destination binding.

The concrete destination binding commits the exact network, domains, backend,
proof family, verifier address, value-moving route address, verifier runtime
code hash, and verifying-key hash. The separate route-configuration signal also
commits the governed token identity, token runtime code hash, both lane hashes,
profile, and route revision. Proofs are therefore not portable between two
route contracts or route revisions even if the verifier is shared.

## Release requirements

A successful pairing only establishes the statement encoded by its circuit.
Production activation additionally requires an audited, reproducibly generated
circuit and verifying key that prove the complete SCCP finality semantics. The
governed registry stores typed deployment identities and derives bindings;
request callers never supply deployment material or expected-binding aliases.

Run:

```bash
bash scripts/sccp_evm_contract_smoke.sh
```

The suite compiles the shared, Ethereum, BSC, and TRON contracts in a temporary
environment, enforces the deployed-code ceiling, cross-checks precompiled and
software BLAKE2b results, and exercises positive accounting plus malformed
payloads, zero or mismatched route revisions, wrong routes/networks/codecs,
replay, cross-route proof attacks, code/key drift, reentrancy, token failures,
and adversarial BN254 inputs.
