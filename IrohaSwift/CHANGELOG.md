# Changelog

All notable changes to `IrohaSwift` are documented in this file.

## [Unreleased]

- 2026-10-30: `proposeRuntimeUpgrade(changeSet:provenance:)` posts `{change_set, provenance}`:
  the change set and the trusted release signers' `ToriiRuntimeChangeSetEndorsement`s the
  committed `runtime_provenance_policy_v1` requires (none for a root without that policy).

- 2026-10-30: Runtime upgrades follow the governed change-set lifecycle
  (`specs/runtime_upgrades.md`); the manifest API is removed without compatibility.
  `listRuntimeUpgrades()` returns `ToriiRuntimeUpgradesSnapshot` (selection, committee, committee
  digest and change-set records), `proposeRuntimeUpgrade(changeSet:)` and
  `cancelRuntimeUpgrade(idHex:)` draft the SORA Parliament proposal, and all three are
  operator-signed. `activateRuntimeUpgrade` and the manifest, record and status types are deleted;
  `ToriiRuntimeUpgradeCounters` counts `scheduled`, `activated`, `lapsed` and `canceled`, and the
  governance `RuntimeUpgrade` payload is `proposalOperator`, `action`, `notice` and `proposedMs`.

- 2026-10-26: `PetalScanSession` reads a capture's turn lanes after its bands when it has time
  to spare: when the capture's work and the session's last turn try fit a 30 fps camera's frame
  time (`captureBudgetMilliseconds`), timed on the monotonic clock. A slow phone core never has
  the time, so it still leaves them to the shown frame's next capture; a fast decoder whose
  camera gives it few frames a second gets its turn lanes back. `init(limits:workCost:)` takes a
  model's costs (`PetalWork`) in place of the measured times, and
  `PetalScanStats.spareTurns` counts the captures that had the time.

- 2026-10-26: Petal Stream format 5 (no format 3 compatibility: beacon version `0x50`; `0x30`,
  `0x20` and `0x10` are refused). A frame is a portrait card of 1024 × 1668 units: the format 3
  square with a band of 306 katakana tiles above and below it (`PetalLayout.cardHeight`,
  `squareTop`, `kanaTiles`, `kanaTileAt`, `kanaInside`, `kanaInked`; `PetalGlyphs` holds the
  sixteen katakana). Eight tiles of each band are pilots; the other 298 carry lanes `P0`/`P1`
  (tile polarity, RS(37, 20), one atom) and `K0`/`K1` (tile katakana, RS(149, 116), seven atoms),
  each bound by a CRC-32C to the stream tag, frame and lane: up to 53 atoms a frame.
  `PetalKana` holds the lane codec and the band reader (`readKana`, `observeBand`,
  `templateProbe`, `coarseTemplateProbe`); `PetalFrameCells.kanaGlyphs`/`kanaLight`/`setKana`,
  `PetalLaneData.kana`, `PetalScanOutcome.kana` and `PetalScanStats.kanaTries`/`kanaLanes` are
  new. `PetalScanSession` reads each band once per shown frame with its own back-off, and a
  capture whose bands read a kana lane tries no turn lane (both on one capture overran a 12 fps
  frame on a slow phone core). `PetalRenderer.render` draws the card (`cardHeightPixels`;
  `PetalPalette.ink`), `PetalDrawList.kanaTiles`/`kanaGlyphStrokes` describe the tiles, and
  `PetalCoreGraphicsRenderer`/`PetalFrameView` draw the portrait card. Fixtures:
  `petal_stream_v5.json` and `petal_captures_v5.json`.

- 2026-10-26: Petal Stream format 3 (no format 2 compatibility: beacon version `0x30`; `0x20`
  and `0x10` are refused). Every petal is narrower (half-length 6.0, half-width 3.73, notch
  radius 1.12 at 6.24) and turn lanes `T0`–`T3` ride on the field lanes: the first 360 lit
  petals of a sector, in a fixed shuffled order, are turned by 0°, 45°, 90° or 135° to show two
  more bits each, one RS(90, 68) codeword of four repair atoms and a CRC-32C check bound to the
  field lane's header; a frame carries up to 37 atoms. `PetalTurns` holds the carrier order, the
  turn lane codec and the reader (`readTurns`, `observedTurns`, `turnTemplate`);
  `PetalFrameCells.turns`/`setTurns`/`turnWord`, `PetalLaneData.turns`,
  `PetalScanOutcome.turned` and `PetalScanStats.turnTries`/`turnLanes` are new, and
  `PetalScanSession` tries each turn lane once per shown frame and, after tries that read
  nothing, passes over up to eight captures before it tries again (reading turns costs a slow
  phone core several times a frame's decode). `PetalDrawList` petal angles include the turn, and
  `PetalCoreGraphicsRenderer` draws the petal notch, antialiased. Fixtures:
  `petal_stream_v3.json` and `petal_captures_v3.json`.

- 2026-10-25: Petal Stream format 2 (no format 1 compatibility: beacon version `0x20`,
  `0x10` is refused). The katakana tile field and lanes `P`/`K` are replaced by 3,376 petals
  on 34 bands in four sectors (`PetalLane.field(0...3)`, 105-byte codewords) beside lane `D`
  on rings at radii 405/440/475; a frame carries 21 atoms. The decoder adds the local read, a
  lens-distortion search (`PetalPose`) and `PetalDecoder.fieldEvidence`/`mergeFields`;
  `PetalScanStats` counts `laneD`, `fieldLanes` and `mergedLanes`; `PetalDrawList` lists the
  `天` backdrop and lit petals. `PetalGlyphs` and the tile APIs are removed. Fixtures:
  `petal_stream_v2.json` and `petal_captures_v2.json`. `PetalStreamView.defaultFramesPerSecond`
  is 12.

- 2026-10-18: Bridge ABI 30 (no ABI 29 compatibility). Removed the Parliament timed-OVN/TLE
  wallet and client APIs: the native casting-proof, registration and ballot wallet functions
  with their seed-handle, trust-anchor and Keychain seed types, the `ToriiClient`
  casting-context, casting-proof, release-context and partial-release methods, and the
  matching response types and constants. `ToriiParliamentLifecycleTransitionV1` keeps its 13
  public transitions, attempt-read body states carry exactly the 8 fixture fields, and an
  attempt-read certificate that contains a private-jury body is rejected until the
  anonymous-ballot result binding is supported.

- 2026-10-17: KAGEMUSHA blacklists are local-only. Added
  `KagemushaWalletExchangeCallV1.installBlacklist(list:certificates:)`, which stores a signed list on the
  phone (a replacement's version must increase; it has no retry identity); Native then refuses
  to pay a listed receiver and to request or accept money from a listed payer with
  `KagemushaWalletErrorV1.blockedStatus` (-15). Nothing is sent to peers or proved:
  `KagemushaWalletRefreshKindV1.blacklist` is removed, and Requests drop the receiver blacklist
  version and root (17-field body, 418-byte transcript, 24 `credit_id` elements).

- Bridge ABI 29 (no ABI 28 compatibility). `KagemushaWalletEnrollmentV1.abandon()` returns
  `KagemushaWalletEnrollmentAbandonmentV1`: `localAbandoned` (Native result kind 58, nothing
  to submit) or `abandonment(frame)` (kind 28, the exact retained signed frame) instead of
  frame bytes. Added the closed E1 refusal table `KagemushaWalletEnrollmentRefusalV1` with
  `KagemushaWalletEnrollmentRefusalClassV1`, admitted only through Native's table, and
  `KagemushaWalletEnrollmentOutcomeV1`, which keeps a withheld E6 delivery distinct from a
  terminal refusal. The Apple key profile has only `secureElement`; tag 2 now fails closed.
  The native export retention table retains the refusal export and is sorted.

- KAGEMUSHA wallet V1 follows the single-design G1 layout: 20 SHA-256 roles,
  including the NEW unsigned app/enrollment policy identities,
  59 Poseidon domains and 16 signing domains. Every signature signs its exact
  32-byte Poseidon message. The artifact manifest alone has a SHA signed-object
  digest (`artifactManifestDigest(message:signature:)`); circuit-visible object,
  package, statement, operation and nullifier digests are opaque native Poseidon
  values. Quota usage uses a depth-6 array. Only this layout is accepted.
- Added `KagemushaWalletApplePlatformV1`, the iPhone platform adapter behind the
  Rust KAGEMUSHA wallet Advance provider: the Secure Enclave payment key, which
  signs exactly the 32-byte message the Rust signer passes with
  `kSecKeyAlgorithmECDSASignatureMessageX962SHA256` and refuses any other length
  (`KagemushaWalletAppleStatusV1.invalidSigningMessage`), the keychain rollback
  anchor, the Complete-class protected-data canary, the custody root under
  Application Support and the boot session identity. Both custody
  keychain items are `kSecAttrAccessibleWhenPasscodeSetThisDeviceOnly` in the
  app's own access group, which `init(appAttest:applicationIdentifierPrefix:)`
  derives from the App ID prefix. The adapter is constructible only for iPhone
  and iPad apps on their own device. `attestEnrollment(slot:paymentPublicKey:challengeDigest:)`
  produces the App Attest evidence of enrollment step E5
  (`KagemushaWalletAppleEnrollmentEvidenceV1`), with
  `KagemushaWalletAppleEnrollmentAttestationErrorV1` and
  `KagemushaWalletAppleAppAttestStageV1` for failures. The slot, profile,
  unavailable-reason, status and configuration-error types are public.
- Removed the aggregate-balance KAGEMUSHA wallet and its consumers, superseded by
  the KAGEMUSHA wallet V1 design (`KagemushaWalletWireV1` and
  `KagemushaWalletApplePlatformV1`): the wire codec and models (`KagemushaNoritoV1`,
  `KagemushaWireV1`, `KagemushaModelsV1`), `KagemushaWalletV1` orchestration, the
  hardware-provider, device-lifecycle and Core coordinator bridges, the App Attest
  ordinary enrollment and approval providers, reserve finality, top-up and
  redemption. `ToriiClient` no longer exposes `getKagemushaCapability`,
  `submitKagemushaTopUp`, `submitKagemushaRedemption`, `getKagemushaOperation`,
  `ToriiKagemushaStatus`, the `ToriiKagemushaOperation*` types or
  `ToriiTopUpOwnershipV1`; `TxBuilder.buildSignedKagemushaTopUp` and
  `KagemushaTopUpTransactionInputError` are gone. `ParticipantEnrollmentHttpCodecV1`,
  `AndroidProvisionedProof` and the `IrohaSwiftTransferUI` KAGEMUSHA flow views
  (`IrohaKagemushaFlowView` and its widgets) were removed with them. The native
  bridge loader no longer requires any `connect_norito_kagemusha_*` export, since
  the bridge exports none.
- Removed the signature-only KAGEMUSHA wallet suite (`Sources/IrohaSwift/KagemushaAttested/`):
  `KagemushaAmount`, `KagemushaConfig`, `KagemushaAccountProof`,
  `KagemushaLedgerPort`, `KagemushaPlatform`, `KagemushaStatus`,
  `KagemushaReadyState`, `KagemushaLimit`, `KagemushaError`, `KagemushaRefusal`,
  `KagemushaUnsupportedReason`, `KagemushaFrozenReason`,
  `KagemushaRevocationReason`, `KagemushaLoadResult`, `KagemushaOutgoingPayment`,
  `KagemushaReceiveResult`, `KagemushaRedemptionStatus`, `KagemushaRedeemResult`,
  `KagemushaDeliveredPayment`, `KagemushaSyncResult`, `KagemushaPeerMessage`,
  `KagemushaIssuerTransport`, `KagemushaURLSessionTransport` and the DEBUG-only
  `KagemushaTestKeyStore` and `KagemushaTestAttestationProvider`. Its Secure
  Enclave key handling moved into `KagemushaWalletApplePlatformV1`; the Rust
  provider owns custody storage and enrollment parsing.
- Collection queries follow `specs/torii/collection_queries.md`: `ToriiFilter`
  (builder operators, result builder, canonical text `description` and JSON
  `jsonData()`, client-side limits), `ToriiSortKey`, `ToriiAggregate`,
  `ToriiListQuery` (canonical `POST /query` body and `GET` parameters),
  `ToriiPage` and the on-demand `ToriiPageSequence`/`ToriiItemSequence`, checked
  against `fixtures/torii/list_query/vectors.json`. `ToriiClient` exposes
  `domains`, `accounts` (with `get(_:)`), `assetDefinitions`, `nfts`, `rwas`,
  `repoAgreements`, `accountAssets(of:)`, `assetHolders(of:)`, the history
  collections `transactions` and `accountTransactions(of:)` (rows
  `ToriiTransaction`, newest first; `sort`, `includeTotal` and `aggregate`
  rejected locally; pagers follow `nextCursor` across short and empty pages)
  with typed rows whose non-identity fields are optional. Filters with object or
  array literals are sent only in the JSON form (`queryItems()` and event
  streams reject them). Removed `listDomains`,
  `iterateDomains`, `listRwas`, `queryRwas`, `iterateRwas`, `getAssets`,
  `getTransactions`, their completion/Combine/`IrohaSDK` twins and
  `ToriiListOptions`/`ToriiListFilter`/`ToriiListSort`/`ToriiQueryEnvelope`
  and the offset/`total`-based page types.
- Torii error responses surface as `ToriiClientError.api(ToriiAPIError)` with
  `status`, envelope `code`, `message`, `details` and the `X-Iroha-Reject-Code`
  header; `ToriiClientError.invalidQuery` reports locally rejected controls with
  the same codes. `ToriiClientError.httpStatus` was removed.
- Event streams decode the specified `/v1/events/sse` payloads into `ToriiEvent`
  (pipeline transaction, block, warning and witness events; proof verified,
  rejected and pruned events; `.data`/`.other` notices for every other kind, so
  unknown events never fail a stream), wrapped in `ToriiEventMessage`.
  Transaction statuses, block statuses and `ToriiTransactionRejectionCode` are
  typed from their names. New `streamEvents(filter:)`/`streamEvents(filterText:)`
  take the collection text grammar, and built filters are checked against the
  event-stream subset before connecting. The transaction-status stream sends
  `tx_hash = "…"` and checks every event's hash (including the trailing one);
  proof streams send `proof_backend`/`proof_call_hash`/`proof_envelope_hash`
  filters and match pruning events too; `streamVerifyingKeyEvents()` and
  `streamTriggerEvents()` recognise their events by kind. The retired
  `{"VerifyingKey": …}`, `{"Trigger": …}` and `{"Proof": …}` shapes, their
  typed payloads and filters (`ToriiVerifyingKeyEventFilter`,
  `ToriiTriggerEventFilter`, `ToriiVerifyingKeyEvent`, `ToriiTriggerEvent`) were
  removed. Event streams follow the same transport policy as other requests
  (HTTPS for credentials, no redirects) and surface rejections with the parsed
  error envelope.
- `+` in query values is percent-encoded; `getStatusSnapshot()` no longer races
  on its sample state; `AccountId.make(publicKey:)` throws instead of trapping;
  building a transfer preserves non-empty `TransferRequest.description` exactly
  in signed `memo` metadata; unsigned fee-quote payloads use the same native
  instruction and payload encoder as signed transfers; the always-failing `deployContractInstance` and
  `activateContractInstance` were removed.
- `ToriiCanonicalRequestAuth` is now only the account credential
  (`accountId`, `privateKey`); its `timestampMs`/`nonce` were removed because a
  reused credential replayed one nonce. `ToriiClient`, `MusubiToriiClientV1` and
  `AtomicPrivateSettlementToriiClientV1` draw a fresh timestamp and nonce for
  every signed request from `ToriiCanonicalRequestFreshness` (injectable for
  tests), so pinned-freshness errors and their both-or-neither checks are gone.

- Added the Petal Stream optical transport (`Sources/IrohaSwift/Petal/`), a
  function-by-function port of `crates/iroha_petal`: CRC-32C bound streams,
  whitened GF(256) Reed–Solomon lanes with errors-and-erasures decoding,
  the GF(2) fountain code (`PetalStreamEncoder`, `PetalStreamAssembler`),
  the reference software renderer and `PetalDrawList`, the camera decoder
  (`PetalDecoder`: adaptive threshold, blossom finders, Hartley-normalised
  homographies, joint polarity/katakana tile matching read against the finder
  levels and, for a lane that stays unreadable, again with every patch and
  template normalised by its own contrast so over-exposure, veiling light,
  glare and shadows cancel; rotation and mirror hypotheses ranked by the ring
  gates and the `天` silhouette; a blossom hidden by a thumb, a glare or the
  frame edge is inferred from the other three and reported as
  `PetalDecodedFrame.inferredCorner`) and `PetalScanSession`, which follows
  the code from its last pose (`PetalDecoder.track`, within 500 ms) instead
  of searching every frame and counts `tracked` and `inferred` frames in
  `PetalScanStats`. The suites check every section of
  `fixtures/petal/petal_stream_v1.json`, decode all eleven golden captures in
  `fixtures/petal/petal_captures_v1.json` with exactly the reference lanes and
  inferred corners, follow both tracking pairs and reproduce the clean capture
  pixel for pixel. `IrohaSwiftTransferUI` adds
  `PetalCoreGraphicsRenderer`, `PetalFrameView` and the animated
  `PetalStreamView`; `IrohaSwiftMobileTransports` adds the AVFoundation
  `PetalCameraAnalyzer` and `PetalCameraFrame` pixel-buffer conversion.
- `ToriiPipelinePreflight` decodes exactly the served `GET /v1/pipeline/preflight`
  body, checked against the Rust-generated `fixtures/torii/pipeline_preflight.json`:
  `ToriiPipelinePreflightSumeragi` carries only the positive `blockCadenceMs`,
  every object rejects fields Torii does not serve, unsigned limits reject
  negative values, fee amounts are `String`s from the committed root policy. Torii serves no stall threshold, so
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
