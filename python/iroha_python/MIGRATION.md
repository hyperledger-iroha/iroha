<!--
Copyright 2024 Hyperledger Iroha Contributors
SPDX-License-Identifier: Apache-2.0
-->

# Migrating to `iroha_python`

The `iroha_python` package consolidates the functionality that previously lived
in the `norito_py` and `iroha_torii_client` helper projects. This guide outlines
the expected migration steps and highlights compatibility shims that remain
available during the transition.

## Quick Reference

| Deprecated import | Replacement | Notes |
|---------------|-------------|-------|
| `import norito_py` | `import iroha_python as iroha_python` | Norito codec re-exported as `iroha_python.norito`. |
| `from norito_py import norito` | `from iroha_python import norito` | Identical API, now bundled with the SDK. |
| `from iroha_torii_client.client import ToriiClient` | `from iroha_python import ToriiClient` | Wrapper subclass preserves existing behaviour plus transaction helpers. |
| `from iroha_torii_client.client import create_torii_client` | `from iroha_python import create_torii_client` | Returns the richer `ToriiClient`. |
| `from iroha_torii_client.mock import MockTorii` | `from iroha_python._compat import compat_mock_torii` | Helper returns the deprecated mock while emitting a deprecation warning. |

## Recommended Changes

1. Replace imports of `norito_py` and `iroha_torii_client` with `iroha_python`.
2. Adjust typing annotations to use the SDK exports (`ToriiClient`,
   `SignedTransactionEnvelope`, `Instruction`, and the new
   `DomainId`/`AccountId`/`AssetDefinitionId`/`AssetId` wrappers).
3. Adopt the high-level helpers exposed by `iroha_python.crypto` and
   `iroha_python.client` to avoid hand-crafted Norito payloads; fall back to
   `Instruction.from_json`/`Instruction.to_json` for specialised ISIs.
4. Update `create_torii_client` call sites to take advantage of the new
   configuration knobs (`auth_token`, `api_token`, retry/backoff tuning, default headers)
   instead of custom wrappers around `requests`.
5. Call the dedicated governance helpers (`governance_*`) and Connect helpers
   (`create_connect_session`, `connect_websocket`) instead of hand-rolling HTTP
   requests or WebSocket URLs.
6. Use the trigger helpers (`Instruction.register_time_trigger`, `Instruction.register_precommit_trigger`,
   `Instruction.execute_trigger`, `Instruction.mint_trigger_repetitions`, `Instruction.burn_trigger_repetitions`, `Instruction.unregister_trigger`)
   instead of emitting raw JSON for trigger automation flows. Pair them with the new event filter builders (`VerifyingKeyFilter`,
   `ProofEventFilter`, `DataEventFilter`) when wiring Torii SSE subscriptions.
7. Prefer `ToriiClient.stream_verifying_key_events`, `ToriiClient.stream_proof_events`, and
   `ToriiClient.stream_trigger_events` over hand-crafted SSE filters; each accepts the new builders and reuses the client retry/auth settings.
8. If you rely on the old mock helpers, switch to the return value of
   `iroha_python._compat.compat_mock_torii()`; tests can gradually migrate to
   higher-level fixtures once available.

## Compatibility Shims

The SDK keeps lightweight wrappers so existing code can upgrade incrementally:

- Importing `norito_py` emits a `DeprecationWarning` and re-exports the
  canonical `iroha_python.norito` module.
- Importing `iroha_torii_client.client.ToriiClient` or
  `create_torii_client` returns the new implementations while warning about the
  impending deprecation.
- The `_compat` module exposes helpers for projects that need explicit shims.

See `python/iroha_python/tests/test_back_compat.py` for examples that cover
both the new and deprecated entry points.

## Deprecation Timeline

- `0.1.x`: Deprecated modules continue to work but warn on import.
- `0.2.0`: Deprecated modules will depend directly on `iroha_python` and forward all
  APIs without re-implementing logic.
- `0.3.0`: Deprecated packages may be yanked once downstream consumers confirm the
  migration. Applications should not rely on them long-term.

For questions or migration blockers, file an issue referencing this guide so
we can document additional shims if required.
