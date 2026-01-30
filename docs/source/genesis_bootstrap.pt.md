---
lang: pt
direction: ltr
source: docs/source/genesis_bootstrap.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 6feb2b03bd8f6a41de693a0c3f3c4ffc058072bc7942e2bc50b3fd9770aa56d4
source_last_modified: "2026-01-03T18:08:01.368173+00:00"
translation_last_reviewed: 2026-01-30
---

# Genesis Bootstrap from Trusted Peers

Iroha peers without a local `genesis.file` can fetch a signed genesis block from trusted peers
using the Norito-encoded bootstrap protocol.

- **Protocol:** peers exchange `GenesisRequest` (`Preflight` for metadata, `Fetch` for payload) and
  `GenesisResponse` frames keyed by `request_id`. Responders include the chain id, signer pubkey,
  hash, and an optional size hint; payloads are returned only on `Fetch`, and duplicate request ids
  receive `DuplicateRequest`.
- **Guards:** responders enforce an allowlist (`genesis.bootstrap_allowlist` or the trusted peers
  set), chain-id/pubkey/hash matching, rate limits (`genesis.bootstrap_response_throttle`), and a
  size cap (`genesis.bootstrap_max_bytes`). Requests outside the allowlist receive `NotAllowed`, and
  payloads signed by the wrong key receive `MismatchedPubkey`.
- **Requester flow:** when storage is empty and `genesis.file` is unset (and
  `genesis.bootstrap_enabled=true`), the node preflights trusted peers with the optional
  `genesis.expected_hash`, then fetches the payload, validates signatures via `validate_genesis_block`,
  and persists `genesis.bootstrap.nrt` alongside Kura before applying the block. Bootstrap retries
  honor `genesis.bootstrap_request_timeout`, `genesis.bootstrap_retry_interval`, and
  `genesis.bootstrap_max_attempts`.
- **Failure modes:** requests are rejected for allowlist misses, chain/pubkey/hash mismatches, size
  cap violations, rate limits, missing local genesis, or duplicate request ids. Conflicting hashes
  across peers abort the fetch; no responders/timeouts fall back to local configuration.
- **Operator steps:** ensure at least one trusted peer is reachable with a valid genesis, configure
  `bootstrap_allowlist`/`bootstrap_max_bytes`/`bootstrap_response_throttle` and the retry knobs, and
  optionally pin `expected_hash` to avoid accepting mismatched payloads. Persisted payloads can be
  reused on subsequent boots by pointing `genesis.file` to `genesis.bootstrap.nrt`.
