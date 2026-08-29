# SCCP EVM contracts

These contracts provide the shared exact V1 implementation used by Ethereum
and BNB Smart Chain routes.

Production sources require exact Solidity `0.7.4`. EVM and TVM artifacts use
the authenticated `0.7.4+commit.3f05b770` compiler, optimizer run count `200`,
and the `istanbul` target over distinct reviewed source maps. Their target
identities, complete standard-json inputs,
ABIs, creation/runtime bytes, immutable-runtime patch ranges, and hashes are
reviewed release-policy inputs. The fixed opcode target avoids instructions
that are not shared by every first-release destination. The runtime smoke loads
production artifacts only from that verified manifest, compiles test-only
harnesses separately with the same locked EVM compiler, rejects compiler,
artifact, and source-staleness mutation before creating a provider, and checks
deployed runtime bytes outside compiler-declared immutable slots. The BLAKE2b
compressor's intentional modulo-2^64 additions are isolated in the documented
`_add64` helper with an explicit 64-bit mask; every value-moving overflow remains
explicitly checked.

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
- `SccpSha256ReplayForest.sol` implements the canonical sharded SHA-256 sparse
  Merkle replay forest and witness verifier shared by admission and outbound
  burn paths.
- `ISccpMessageVerifier.sol` and `SccpGroth16Bn254MessageVerifier.sol` define
  and implement immutable BN254 verification for the eleven exact SCCP signals.

Account roles are deliberately asymmetric. An external-to-Taira burn accepts
only the exact `test...` I105 spelling for discriminant `369` and a single,
canonical Ed25519 controller in the prime-order subgroup, matching Taira
settlement admission and rejecting both small-order and mixed-torsion points.
A proof-authenticated Taira-to-external sender may instead be a single-key or
canonical multisig `AccountId` composed from Ed25519 and compressed secp256k1
keys. The parser checks the complete V1 AccountAddress tags, big-endian policy
fields, Rust algorithm-name member order, I105 round trip and checksum, weak
Ed25519 encodings, and off-curve/noncanonical secp256k1 encodings. Taira rejects
all other controller algorithms before locking assets, so no accepted outbound
message can be unfinalizable merely because its sender controller is outside
the immutable destination parser.

Generic owner emitters and the secp256k1 attestation verifier are intentionally
absent. Generic proof-only message wrappers are also absent: accepting a proof
without executing the value-moving route is not settlement. A production
source event must be coupled to the concrete token burn, and a production
destination proof must call `finalizeFromTaira` on the immutable typed route.
Each concrete production route accepts one exact predeployed token address.
Deployment tooling first precomputes the route address, deploys the token with
that address as its immutable `bridge`, and then deploys the route at the exact
precomputed address with the exact token address. The route constructor rejects
token/route readback, code, policy, or role drift before storing any binding.
There is no privileged initialization window or mutable bridge setter. The
constructor also requires one positive u128-sized `maxWrappedSupply`, stores it
immutably, commits it to `routeConfigHash`, and rejects any mint that would make
the wrapped token's total supply exceed that ceiling. The revision is encoded
immediately after the Transfer nonce and is included in `routeConfigHash`, so
nonce reuse by a replacement route cannot collide with an older route's
message identity. Each route deploys an immutable five-guardian, three-vote,
one-way mint breaker. The route stores the breaker's deployment-time runtime
hash, exposes it through `mintBreakerCodeHash()`, and checks the live runtime
hash plus the disabled latch before every new mint admission. Breaker votes can
never re-enable minting, withdraw assets, or block outbound burns.

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

The verifying key contains twelve G1 input-coefficient points: one constant plus
eleven signals. Those points serialize with alpha, beta, gamma, and delta to the
canonical 38-ABI-word verifying-key preimage. Ten-signal, eleven-IC-point, and
36-word key representations are invalid. Each signal is
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
11. governed SORA finality-anchor hash

The verifier rejects wrong tuple lengths, zero required words, domain overflow
or equality, noncanonical/zero/off-curve/non-subgroup points, and failed
pairings. It has no mutable signer set or update function. Route rotation
creates a new immutable route revision and destination binding.

The concrete destination binding commits the exact network, domains, Groth16
backend, verifier address, value-moving route address, verifier runtime
code hash, verifying-key hash, audited semantic-profile hash, and governed SORA
finality-anchor hash, followed by the replay-verifier address and runtime hash
and the mint-breaker address and runtime hash. The separate route-configuration
signal also commits all of those policy roles plus the governed token identity,
token runtime code hash, both lane hashes, network profile, route revision,
maximum wrapped supply, and the same replay/breaker quartet. Proofs are
therefore not portable between supply caps, policy revisions, route contracts,
breaker deployments, or route revisions even if the verifier is shared.
Route constructors take one typed
`VerifierPolicyV1` tuple and reject zero, aliased, or getter-mismatched roles.
Governed destination deployment records carry the same tuple as required
`outbound_proof_policy`; policy-less JSON and Norito records are invalid.

## Release requirements

A successful pairing only establishes the statement encoded by its circuit.
The checked-in labeled-signal circuit is a diagnostic-only artifact and is not
production evidence; production validators reject its identifier,
classification, and exact published digest. Production activation requires a
signed independent audit and reproducible circuit, witness-generator,
proving-key, and verifying-key
commitments proving canonical payload semantics, message-leaf derivation,
Merkle inclusion, the block-header commitment root, commit-QC finality, exact
chain identity, and validator-set continuity rooted in the governed SORA
anchor. The governed registry stores those typed commitments and derives all
bindings; request callers never supply deployment material or
expected-binding aliases.

Run:

```bash
bash scripts/sccp_evm_contract_smoke.sh
```

The suite verifies the authenticated EVM/TVM manifest against the current
sources and deploys the exact reviewed EVM and TRON artifacts in an EVM
runtime. Test harnesses are compiled separately with the same locked compiler,
and their TRON output must reproduce the reviewed TRON creation code exactly.
The suite enforces runtime, initcode, and deployment-gas
ceilings, cross-checks precompiled and
software BLAKE2b results, and exercises positive accounting plus malformed
payloads, zero or mismatched route revisions, wrong routes/networks/codecs,
replay, stale sparse-Merkle witnesses, cross-route proof attacks, token,
replay-verifier and mint-breaker runtime drift, reentrancy, token failures,
substituted/zero/aliased semantic-profile or finality-anchor commitments, and
adversarial BN254 inputs.

EVM execution of the reviewed TRON bytecode is compatibility coverage only. It
is never accepted as TVM deployment evidence; the production corridor's pinned
real-TRE phase remains mandatory.
