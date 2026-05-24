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
- `test/sccp_message_bridge_smoke.js`: focused Ganache smoke test for the
  wrapper path.

Quick verification:

```bash
scripts/sccp_evm_contract_smoke.sh
```

The current reference path keeps the native SCCP proof artifact as a canonical
`OpenVerifyEnvelope` that wraps the FASTPQ proof and bound public inputs, while
the EVM submission package carries an attestation envelope over
`native_proof_hash + public_inputs_hash + statement_hash +
destination_binding_hash`, where `destination_binding_hash` binds the
attestation to one SCCP lane on one deployed wrapper and its immutable verifier
(`source_domain + target_domain + verifier_backend_hash + proof_family_hash +
network_id + verifier_address + wrapper_address`). The wrapper also enforces
the expected source/target domains and checks the returned `messageId` and
`commitmentRoot` against the supplied public inputs. The verifier contract
checks signer authorization, destination binding, and replay safety on-chain,
which is not enough for production. SCCP production use is disabled until an
immutable destination verifier can validate a recursive SCCP proof under
governed trust anchors without challenge windows, watcher assumptions, or
attestation-only shortcuts.

Production EVM-family manifests use `evm-groth16-bn254-v1`. The
`SccpGroth16Bn254MessageVerifier` contract verifies ABI-encoded Groth16 proof
points against the constructor-supplied BN254 verifying key and derives nine
field public signals from the six SCCP public-input words plus source domain,
statement hash, and destination binding hash. The verifier has no signer set
or update function; replacing verifier material requires deploying a new
verifier/bridge binding.

Relay packages for `evm-groth16-bn254-v1` carry the Groth16 proof bytes
directly. They do not contain an attestor list, signature vector, quorum, or
other committee authorization field. The only deployment-specific metadata in
the package is the destination binding used by tooling to target the intended
wrapper/verifier deployment; the wrapper supplies that binding hash to the
verifier as a public input.

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
