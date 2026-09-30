# Changelog

All notable changes to `IrohaSwift` are documented in this file.

## [Unreleased]

- `ToriiPipelinePreflight` decodes exactly the served `GET /v1/pipeline/preflight`
  body, checked against the Rust-generated `fixtures/torii/pipeline_preflight.json`:
  `ToriiPipelinePreflightSumeragi` carries only the positive `blockCadenceMs`,
  every object rejects fields Torii does not serve, unsigned limits reject
  negative values, fee amounts are `String`s and `settlementMode` is `direct` or
  `lane_relay_burn`. Torii serves no stall threshold, so
  `ToriiPipelinePreflight.stallThresholdMs` (`stallBlockCadences` = 20 times the
  block cadence) now drives `isStatusStalled(_:)`; `blockTimeMs`,
  `commitTimeMs` and `stallThresholdMs` were removed from the sumeragi section.
- Added `ToriiClient.getSumeragiLanes()` (async and completion-handler forms)
  for the operator-signed `GET /v1/sumeragi/lanes` list. Responses decode
  fail-closed into `ToriiSumeragiLaneStatus` (exact fields, canonical
  BLS-normal committee keys, 96-byte proofs of possession, uppercase 32-byte
  hex lane hashes) and are tested against the shared Rust-generated
  `fixtures/sumeragi/native_lanes_v1.tsv` corpus.
- Pinned Norito v1 headers to the fixed-width (`0x00`) and compact
  (`COMPACT_LEN`, `0x02`) layouts. `NoritoHeader.packedSeq`, `packedStruct`,
  `fieldBitset`, `varintOffsets`, and `compactSeqLen` were removed, and headers
  carrying any flag other than `COMPACT_LEN` (or none) are rejected.
  `ConfidentialEncryptedPayload.noritoEnvelope()` now defaults to
  `COMPACT_LEN` (payload bytes are unchanged), and the Connect queue journal
  rejects records whose header flag byte is non-zero.
- Replaced Explorer instruction, transaction, and transfer-history page-number
  pagination with snapshot-bound `cursor`/`limit` APIs and strict continuation
  metadata. The async, completion-handler, `IrohaSDK`, and Combine surfaces now
  share the same first-release contract; instruction boxes also expose the
  server-provided framed SHA-256 digest. Explorer lists, details, streams, and
  contract activity/event reads plus the generic event SSE feed now use the
  client's default canonical request signer when configured, while remaining
  anonymous for public dataspaces.
- Added the aggregate-balance KAGEMUSHA wire codec (`KagemushaNoritoV1`), `kgm1:`
  text transport, device-lifecycle surface, and fail-closed wallet orchestration.
  The wallet supports concurrent head-independent requests, durable idempotent
  staging and acknowledgements, unbounded inbox-prefix folding, immediately
  usable send successors,
  byte-identical retries, partial/full redemption, and KAGEMUSHA epoch-local counter
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
