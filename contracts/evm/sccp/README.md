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
- `SccpSecp256k1MessageVerifier.sol`: production verifier that checks
  secp256k1 attestation quorum with `keccak256` and `ecrecover`.
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
checks signer authorization, destination binding, and replay safety on-chain.
That path is still treated as reference-only. SCCP production use is disabled
until an immutable destination verifier can validate a recursive SCCP proof
under governed trust anchors without challenge windows, watcher assumptions, or
attestation-only shortcuts.
