# Genesis Bootstrap from Trusted Peers

Iroha peers without a local `genesis.file` can fetch a signed genesis block from trusted peers
using the Norito-encoded bootstrap protocol.

- **Protocol:** peers exchange `GenesisRequest` (`Preflight` for metadata, `Fetch` for payload) and
  `GenesisResponse` frames keyed by `request_id`. Responders include the chain id, signer pubkey,
  hash, and an optional size hint; payloads are returned only on `Fetch`. A requester keeps the
  same id registered and retransmits it until the request deadline. Responders rebuild the same
  idempotent response, even inside the per-source throttle window, so actor backpressure or a lost
  first response cannot poison that id. Rate ownership uses the authenticated transport peer, not
  the semantic origin that a trusted relay may preserve. An admitted response moves its payload
  allocation into the bounded requester queue and retains the P2P byte-ownership lease until that
  queue entry is consumed or dropped; the listener and payload validator do not clone a second
  max-sized payload. Reply-queue byte admission computes the exact Norito length without
  materializing a second encoded frame.
  The responder's throttle history is capped by the bounded response queue and expires with the
  throttle window; source-identity churn cannot grow it without bound.
- **Guards:** responders enforce an allowlist (`genesis.bootstrap_allowlist` or the trusted peers
  set), chain-id/pubkey/hash matching, rate limits (`genesis.bootstrap_response_throttle`), and a
  size cap (`genesis.bootstrap_max_bytes`). Authorization uses the end-to-end semantic origin after
  the P2P relay signature check; an empty allowlist and empty trusted-peer fallback deny every
  request. Requests outside the allowlist receive `NotAllowed`, and payloads signed by the wrong
  key receive `MismatchedPubkey`.
- **Requester flow:** when storage is empty and `genesis.file` is unset (and
  `genesis.bootstrap_enabled=true`), `genesis.expected_hash` must pin the exact signed genesis
  block. The node sends that pin in every preflight and payload request, validates the returned
  hash and signatures via `validate_genesis_block`, and persists `genesis.bootstrap.nrt` alongside
  Kura before applying the block. Bootstrap retries
  honor `genesis.bootstrap_request_timeout` and `genesis.bootstrap_retry_interval`. Payload fetches
  try preflight responders first but retain every configured peer as a recovery source, because a
  responder may fail after advertising metadata while another peer becomes responsive after GST.
  `genesis.bootstrap_max_attempts` bounds one diagnostic/backoff cycle: after that many unanswered
  windows the node logs, resets the bounded backoff, and continues. It does not turn a pre-GST
  partition into a permanent startup failure while bootstrap remains enabled.
- **Failure modes:** enabling remote bootstrap with neither a local `genesis.file` nor
  `genesis.expected_hash` is a startup configuration error, before any request is sent. Requests
  are rejected for allowlist misses, chain/pubkey/hash mismatches, size cap violations, rate
  limits, or missing local genesis. A hash outside the configured pin and every other permanent
  validation error abort the fetch. A missing responsive peer/quorum remains pending and is
  surfaced through periodic warnings; this is intentional under the partial-synchrony liveness
  contract. Framed genesis decoding applies payload-derived Norito sequence, allocation, and
  nesting budgets before signature validation. The listener is a supervised daemon child; losing
  its subscription or authenticated reply path triggers normal node shutdown instead of silently
  leaving bootstrap unavailable.
- **Operator steps:** ensure at least one trusted peer is reachable with a valid genesis, configure
  `bootstrap_allowlist`/`bootstrap_max_bytes`/`bootstrap_response_throttle` and the retry knobs, and
  pin the exact block in `expected_hash` before enabling remote bootstrap. A local signed
  `genesis.file` is already an explicit artifact and does not require an additional hash pin.
  Persisted payloads can be reused on subsequent boots by pointing `genesis.file` to
  `genesis.bootstrap.nrt`.
