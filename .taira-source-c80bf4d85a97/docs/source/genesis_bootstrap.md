# Genesis Bootstrap from Trusted Peers

Iroha peers without a local `genesis.file` can fetch a signed genesis block from trusted peers
using the Norito-encoded bootstrap protocol.

- **Protocol:** peers exchange `GenesisRequest` (`Preflight` for metadata, `Fetch` for payload) and
  `GenesisResponse` frames keyed by `request_id`. Responders include the chain id, signer pubkey,
  hash, and an optional size hint; payloads are returned only on `Fetch`. A requester keeps the
  same id registered and retransmits it until the request deadline. Responders rebuild the same
  idempotent response, even inside the per-peer throttle window, so actor backpressure or a lost
  first response cannot poison that id. The responder's throttle history is capped by the bounded
  response queue and expires with the throttle window; peer-identity churn cannot grow it without
  bound.
- **Guards:** responders enforce an allowlist (`genesis.bootstrap_allowlist` or the trusted peers
  set), chain-id/pubkey/hash matching, rate limits (`genesis.bootstrap_response_throttle`), and a
  size cap (`genesis.bootstrap_max_bytes`). Requests outside the allowlist receive `NotAllowed`, and
  payloads signed by the wrong key receive `MismatchedPubkey`.
- **Requester flow:** when storage is empty and `genesis.file` is unset (and
  `genesis.bootstrap_enabled=true`), the node preflights trusted peers with the optional
  `genesis.expected_hash`, then fetches the payload, validates signatures via `validate_genesis_block`,
  and persists `genesis.bootstrap.nrt` alongside Kura before applying the block. Bootstrap retries
  honor `genesis.bootstrap_request_timeout` and `genesis.bootstrap_retry_interval`. Payload fetches
  try preflight responders first but retain every configured peer as a recovery source, because a
  responder may fail after advertising metadata while another peer becomes responsive after GST.
  `genesis.bootstrap_max_attempts` bounds one diagnostic/backoff cycle: after that many unanswered
  windows the node logs, resets the bounded backoff, and continues. It does not turn a pre-GST
  partition into a permanent startup failure while bootstrap remains enabled.
- **Failure modes:** requests are rejected for allowlist misses, chain/pubkey/hash mismatches, size
  cap violations, rate limits, or missing local genesis. Conflicting hashes and permanent
  validation errors abort the fetch. A missing responsive peer/quorum remains pending and is
  surfaced through periodic warnings; this is intentional under the partial-synchrony liveness
  contract.
- **Operator steps:** ensure at least one trusted peer is reachable with a valid genesis, configure
  `bootstrap_allowlist`/`bootstrap_max_bytes`/`bootstrap_response_throttle` and the retry knobs, and
  optionally pin `expected_hash` to avoid accepting mismatched payloads. Persisted payloads can be
  reused on subsequent boots by pointing `genesis.file` to `genesis.bootstrap.nrt`.
