# Changelog

All notable changes to `IrohaSwift` are documented in this file.

## [Unreleased]

- Replaced Explorer instruction, transaction, and transfer-history page-number
  pagination with snapshot-bound `cursor`/`limit` APIs and strict continuation
  metadata. The async, completion-handler, `IrohaSDK`, and Combine surfaces now
  share the same first-release contract; instruction boxes also expose the
  server-provided framed SHA-256 digest. Explorer lists, details, streams, and
  contract activity/event reads plus the generic event SSE feed now use the
  client's default canonical request signer when configured, while remaining
  anonymous for public dataspaces.
- Added the aggregate-balance `KagemushaV1` wire, `kgm1:` text, device-lifecycle
  surface, and fail-closed wallet orchestration. The wallet supports concurrent
  head-independent requests, durable idempotent staging and acknowledgements,
  unbounded inbox-prefix folding, immediately usable send successors,
  byte-identical retries, partial/full redemption, and Kagemusha epoch-local counter
  rollover without a software fallback.
- Replaced the governance mutation boundary with closed public-only request
  types. Deploy proposals no longer expose ignored limits and now use typed
  manifest provenance; ZK public inputs are exact and shared across legacy,
  v1-envelope, and nested BallotProof routes; Parliament ballots use canonical
  enums; and plain-ballot durations encode as canonical decimal JSON strings.
  Added Swift client and `IrohaSDK` helpers for the v1-envelope, BallotProof,
  and Parliament ballot endpoints. Governance windows are ordered across REST
  and local transaction builders, ZK backends are exact tokens, enactment
  requests expose only the authoritative proposal id, deploy builders require
  ABI V1, and locally signed ZK ballots use the same closed typed public-input
  model with recursive private-key-alias defence.
- Bound the `CancelAssetLock` lock-ID preimage to the public V1 limit of 4,096
  UTF-8 bytes while preserving the fixed 32-byte `EscrowId` wire field.
- Added strict typed `CancelAssetLock` V1 parity. Swift now derives the native
  marked Blake2b-256 escrow id from a clean lock id, requires an exact positive
  `expected_remaining_amount`, emits a transaction-ready schema-bound Norito
  frame, and rejects the retired one-field JSON/Norito layout, aliases, extras,
  malformed identifiers, noncanonical quantities, and trailing bytes.
