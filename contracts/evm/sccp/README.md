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

- `Ownable.sol`: minimal owner-gated helper used by the wrapper/deployer.
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

The EVM lane is now production-targeted: the native SCCP FASTPQ proof stays in
the artifact, while the EVM submission package carries an attestation envelope
over `native_proof_hash + public_inputs_hash + statement_hash`, and the
verifier contract checks signer authorization and replay safety on-chain.
