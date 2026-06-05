# SCCP EVM Reference Contracts

This directory contains the reference EVM-side SCCP message-proof wrapper
contracts that correspond to the SCCP `submission_template` values emitted by
`crates/iroha_sccp`.

The current ETH and BSC lanes intentionally share the same wrapper contract
shape and verifier entrypoint:

- `submitSccpMessageProof(bytes proof_bytes, bytes32[6] public_inputs, bytes32 statement_hash)`

The fixed-width `bytes32[6]` argument is chosen specifically so EVM-like lanes
can stick to ABI-native words and `keccak256` instead of shipping the full SCCP
bundle through calldata.

Files:

- `ISccpMessageVerifier.sol`: external verifier interface expected by the
  wrapper.
- `SccpMessageBridge.sol`: replay-protected SCCP proof submission wrapper.
- `SccpMessageBridgeDeployer.sol`: small deployer for the wrapper contract.
- `SccpGroth16Bn254MessageVerifier.sol`: production-style verifier for the
  `evm-groth16-bn254-v1` backend using the EVM BN254 pairing precompile and an
  immutable constructor-supplied verifying key.
- `SccpSecp256k1MessageVerifier.sol`: reference-only verifier that checks
  secp256k1 attestation quorum with `keccak256` and `ecrecover`; it is not a
  production SCCP verifier.
- `test/sccp_message_bridge_smoke.js`: focused Ganache smoke test for the EVM
  wrapper plus the TRON/TVM destination-verifier and governed source-bridge
  entrypoint paths.

Quick verification:

```bash
scripts/sccp_evm_contract_smoke.sh
```

The smoke installs pinned `solc`, `ganache`, and `ethers` versions into a
temporary directory, compiles the EVM and TRON SCCP contracts, and runs the
deterministic BN254 acceptance/replay checks without relying on repository-local
Node dependencies.

The current reference path keeps the native SCCP proof artifact as a canonical
`OpenVerifyEnvelope` that wraps the FASTPQ proof and bound public inputs, while
the EVM submission package carries an attestation envelope over
`native_proof_hash + public_inputs_hash + statement_hash +
destination_binding_hash`, where `destination_binding_hash` binds the
attestation to one SCCP lane on one deployed wrapper and its immutable verifier
(`source_domain + target_domain + verifier_backend_hash + proof_family_hash +
network_id + verifier_address + wrapper_address + verifier_code_hash +
verifier_key_hash`). The wrapper constructor rejects missing or mismatched
verifier bytecode hashes, any backend other than `evm-groth16-bn254-v1`, and
any proof family other than `stark-fri-v1`. It requires the verifier contract
to expose the expected immutable verifying-key hash via `verifyingKeyHash()`
and rejects empty verifier-backend/proof-family labels, a zero network id, a
non-SORA source domain, a target domain outside ETH/BSC, and same-domain
deployments. ETH-targeted deployments must use the bytes32 EIP-155 chain-id
word for Ethereum mainnet (`1`), and BSC-targeted deployments must use the
bytes32 EIP-155 chain-id word for BNB Smart Chain mainnet (`56`).
The wrapper enforces the expected source/target domains, rejects zero SCCP
statement/public-input fields before calling the verifier, and checks the
returned `messageId` and `commitmentRoot` against the supplied public inputs.
The `destinationBindingHash()` view exposes the deployment binding that the
wrapper passes to the verifier, and accepted proof events include both the
statement hash and destination binding hash so canary logs can be matched to the
exact governed proof statement and wrapper/verifier deployment. A Groth16 proof
accepted for one wrapper is not portable to another wrapper address because the
destination binding hash is one of the verifier's public signals; the smoke test
covers both direct wrong-binding verification and cross-wrapper replay failure.
The reference secp256k1 verifier contract checks canonical attestation ABI
encoding, non-zero SCCP statement/public-input/native-proof fields, signer
authorization, and destination binding on-chain, but it remains a
non-production fixture and cannot be bound through `SccpMessageBridge`.
Production admission for live EVM lanes is gated on a governed immutable
Groth16 verifier deployment, matching verifying-key and runtime-code hashes,
route allowlist material, and externally supplied proof tuples.

Production EVM-family manifests use `evm-groth16-bn254-v1`. The
`SccpGroth16Bn254MessageVerifier` contract verifies ABI-encoded Groth16 proof
points against the constructor-supplied BN254 verifying key and derives nine
field public signals from the six SCCP public-input words plus source domain,
statement hash, and destination binding hash. Proof bytes must be the canonical
12-word static ABI tuple; trailing bytes or truncated tuples fail before point
validation. The verifier has no signer set or update function; replacing
verifier material requires deploying a new verifier/bridge binding with the new
verifier bytecode hash and verifying-key hash.

Relay packages for `evm-groth16-bn254-v1` carry the Groth16 proof bytes
directly. They do not contain an attestor list, signature vector, quorum, or
other committee authorization field. The only deployment-specific metadata in
the package is the destination binding used by tooling to target the intended
wrapper/verifier deployment; the wrapper supplies that binding hash to the
verifier as a public input.
Read-only live evidence summaries only emit Torii artifact/job destination
query fields after the operator-supplied expected destination binding hash
matches the wrapper's `destinationBindingHash()` view, and they mark those
fields as still requiring `proof_bytes_hex`. The Rust, Python, and JavaScript
typed clients plus the bridge-feature CLI enforce the same relationship in both
directions before sending package fetch requests: deployment fields require
`proof_bytes_hex`, and `proof_bytes_hex` requires deployment fields.
Direct and live EVM rollout TOML now also carries bridge/verifier runtime
bytecode, verifier-backend hash comments, and proof-family hash comments.
All-lanes preflight rejects missing or drifting values before ETH/BSC
activation.

The Groth16 verifier constructor takes:

- `uint256[2] alpha1`
- `uint256[4] beta2`
- `uint256[4] gamma2`
- `uint256[4] delta2`
- `uint256[] ic`, flattened as `(x, y)` G1 pairs, exactly ten points for the
  constant term plus the nine SCCP public signals

G1 points use `(x, y)`. G2 points use `(x_0, x_1, y_0, y_1)` as supplied by the
verifying-key exporter; the verifier repacks them into the EVM pairing
precompile's expected coordinate order internally. Proof calldata is:

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

The nine public signals are `uint256(keccak256(abi.encode(label, value))) mod r`
in this order: `message_id`, `payload_hash`, `target_domain`,
`commitment_root`, `finality_height`, `finality_block_hash`, `source_domain`,
`statement_hash`, and `destination_binding_hash`.
The verifier fails closed on zero statement or destination-binding hashes,
zero payload/commitment/finality fields, zero or overflowing target domains,
and same source/target domain bindings before it evaluates the pairing check.
Proof and verifying-key point validation rejects zero G1/G2 points and uses the
EIP-197 pairing precompile's G2 group-order validation before accepting G2
inputs.
The Ganache smoke test constructs a deterministic self-consistent BN254 proof
for the test verifying key and submits it through the EVM wrapper, so the
positive pairing path and replay guard are covered in addition to malformed
proof rejection, zero proof-point rejection, off-curve G2 rejection, and
non-prime-subgroup G2 rejection. It also pins the verifier's pre-pairing proof
word policy for overflowing source/target domains and same source/target domain
bindings.
