# Changelog

All notable changes to `@iroha/iroha-js` are documented in this file.

## [Unreleased]

- `ToriiClient.callContract` now requires a `gasLimit` in the request payload so
  callers always supply the on-chain gas cap; typings, README docs, and test
  coverage reflect the stricter contract.【javascript/iroha_js/src/toriiClient.js:15360】【javascript/iroha_js/index.d.ts:4477】【javascript/iroha_js/test/toriiClient.test.js:13919】【javascript/iroha_js/test/integrationTorii.test.js:2701】【javascript/iroha_js/README.md:1909】
- Constrained the JS SDK to the first-release surface: Connect WebSocket URLs no longer accept token
  query parameters, Torii health snapshots now only parse JSON responses, the `X-Iroha-API-Token`
  alias is no longer emitted, offline summary counter aliases are dropped, and account address
  decoding rejects extension-flag headers. Tests and docs now reflect the first-release surface.
- Added `ToriiClient.iterateVerifyingKeys` and `iterateProverReports` plus
  iterator option whitelists so SoraFS/registry/prover paginators accept their
  filter fields alongside paging knobs; typings, README snippets, and Jest
  coverage close the remaining JS-04/JS-07 pagination gaps.【javascript/iroha_js/src/toriiClient.js:1181】【javascript/iroha_js/src/toriiClient.js:4671】【javascript/iroha_js/src/toriiClient.js:6949】【javascript/iroha_js/index.d.ts:5470】【javascript/iroha_js/test/toriiClient.test.js:761】【javascript/iroha_js/test/toriiClient.test.js:11493】【javascript/iroha_js/README.md:106】
- Added `ToriiClient.iterateSnsGovernanceCases`, which follows the
  `/v1/sns/governance/cases` `next_since` cursor automatically so JS-04/ADDR-6
  automation can stream arbitration cases without bespoke pagination.
  TypeScript declarations, README docs, and Jest coverage demonstrate the
  iterator usage and confirm the `limit`/`status` filters stay intact across
  pages.【javascript/iroha_js/src/toriiClient.js:3131】【javascript/iroha_js/index.d.ts:5673】【javascript/iroha_js/test/toriiClient.test.js:14354】【javascript/iroha_js/README.md:2564】
- ISO bridge status normalization now constrains Torii responses to the
  expected `Pending`/`Accepted`/`Rejected` labels and validates `pacs002_code`
  against the standard `ACTC`/`ACSP`/`ACSC`/`ACWC`/`PDNG`/`RJCT` set so JS-06
  callers get deterministic errors when the bridge returns an unexpected
  state. Typings, README/docs snippets, and Jest coverage exercise the new
  validation paths.【javascript/iroha_js/src/toriiClient.js:7168】【javascript/iroha_js/index.d.ts:3600】【javascript/iroha_js/test/toriiClient.test.js:940】【javascript/iroha_js/README.md:1232】【docs/source/sdk/js/governance_iso_examples.md:79】
- `decodeCompressedAccountAddress` now enforces string inputs and surfaces a
  clear `TypeError` for non-string values, keeping JS-04 validation parity for
  compressed address helpers and preventing accidental coercion when decoding
  selectors. Jest coverage guards the new behaviour.【javascript/iroha_js/src/address.js:1635】【javascript/iroha_js/test/address.test.js:482】
- Added optional SNS integration smoke coverage gated by
  `IROHA_TORII_INTEGRATION_SNS_SUFFIX`/`IROHA_TORII_INTEGRATION_SNS_SELECTOR`
  so JS-04/ADDR-5 adopters can validate suffix policies and registration
  payloads against live Torii deployments without bespoke scripts. README
  environment docs and integration assertions cover the new toggles.【javascript/iroha_js/test/integrationTorii.test.js:2988】【javascript/iroha_js/README.md:2015】
- Added `ToriiClient.submitIsoMessage`, which builds pacs.008/pacs.009 payloads
  from structured fields, applies pacs-specific `Content-Type` defaults, reuses
  a single `AbortSignal` across submission and polling, and optionally waits
  for a terminal bridge status. Typings, README/docs snippets, and Jest
  coverage keep the JS-06 advanced ISO bridge flow deterministic for CI and
  operators.【javascript/iroha_js/src/toriiClient.js:493】【javascript/iroha_js/index.d.ts:5420】【javascript/iroha_js/test/toriiClient.test.js:2876】【javascript/iroha_js/README.md:1243】【docs/source/sdk/js/governance_iso_examples.md:92】
- Hardened `ToriiClient.waitForTransactionStatus{,Typed}` by validating the
  polling options up front: the helper now requires the options payload to be a
  plain object, enforces non-negative `intervalMs`/`timeoutMs`, positive
  `maxAttempts`, and a functional `onStatus` callback while reusing the same
  guards inside `submitTransactionAndWait`. README snippets and Jest coverage
  document the stricter JS-04 validation so callers receive actionable
  `TypeError`s before any Torii request is issued.【javascript/iroha_js/src/toriiClient.js:1756】【javascript/iroha_js/test/toriiClient.test.js:2598】【javascript/iroha_js/README.md:148】
- Added `ToriiClient.submitDaBlob` together with the DA ingest builder, typings, README
  snippet, and Jest coverage so JS-04/DA-8 callers can mirror the
  `iroha da submit` payload (BLAKE3 digest, typed metadata, retention policy) directly from
  Node without shelling out to the CLI.【javascript/iroha_js/src/toriiClient.js:1163】【javascript/iroha_js/src/dataAvailability.js:22】【javascript/iroha_js/index.d.ts:4030】【javascript/iroha_js/README.md:770】【javascript/iroha_js/test/toriiClient.test.js:1408】
- Added offline verdict revocation helpers (`listOfflineRevocations`,
  `queryOfflineRevocations`, iterator variants, DTO normalisers, TypeScript definitions,
  README docs, and Jest coverage) so JS-04 validation covers the OA7 revocation surfaces and
  SDK consumers can inspect `/v1/offline/revocations{,/query}` without bespoke parsing.【javascript/iroha_js/src/toriiClient.js:408】【javascript/iroha_js/index.d.ts:2005】【javascript/iroha_js/README.md:1857】【javascript/iroha_js/test/toriiClient.test.js:8095】
- Added `buildDaProofSummaryArtifact` and `emitDaProofSummaryArtifact` so DA-8 proof
  workflows can serialise PoR summaries into the same Norito JSON emitted by
  `iroha da prove-availability`, with README usage, typings, and Jest coverage to keep the
  CLI-compatible artefacts reproducible from JS automation.【javascript/iroha_js/src/dataAvailability.js:111】【javascript/iroha_js/index.d.ts:3273】【javascript/iroha_js/README.md:820】【javascript/iroha_js/test/dataAvailability.proof.test.js:1】
- Hardened the Torii iterable/list/query helpers to reject non-object options,
  raising a `TypeError` before any HTTP call and documenting the stricter
  contract with README + Jest coverage so JS-04 validation stays aligned with
  the Rust/Python SDKs.【javascript/iroha_js/src/toriiClient.js:4391】【javascript/iroha_js/test/toriiClient.test.js:4452】【javascript/iroha_js/README.md:1689】
- Hardened the SoraFS, data availability, and UAID ToriiClient helpers by
  routing them through a shared `_normalizeOptionsWithSignal` guard so malformed
  `options` payloads are rejected before hitting Torii, and added Jest coverage
  to exercise the new JS-04 validation paths across the registry, PoR, storage,
  DA ingest, and Space Directory surfaces.【javascript/iroha_js/src/toriiClient.js:889】【javascript/iroha_js/src/toriiClient.js:5320】【javascript/iroha_js/test/toriiClient.test.js:1872】
- `ToriiClient.getSorafsPinManifest` now treats `404 Not Found` as `null`
  so callers can distinguish between "missing" and "malformed" responses, and
  `getSorafsPinManifestTyped` raises an explicit error when Torii cannot locate
  the requested digest. README guidance and Jest coverage document the stricter
  behaviour to keep JS-04 validation aligned with the SoraFS rollout
  requirements.【javascript/iroha_js/src/toriiClient.js:1438】【javascript/iroha_js/test/toriiClient.test.js:1515】【javascript/iroha_js/README.md:903】
- Added `ToriiClient.getUaidPortfolio`, `getUaidBindings`, and `getUaidManifests`
  plus TypeScript definitions, README docs, and Jest coverage so the JS SDK
  mirrors the Nexus NX-16 UAID portfolio and Space Directory manifest APIs
  without bespoke JSON parsing. The helpers validate UAID literals, lifecycle
  metadata, and dataspace filters, keeping the new universal-account surfaces in
  lockstep with the Torii reference docs.【javascript/iroha_js/src/toriiClient.js:1234】【javascript/iroha_js/index.d.ts:3255】【javascript/iroha_js/README.md:818】【javascript/iroha_js/test/toriiClient.test.js:964】【docs/source/torii/portfolio_api.md:1】
- Added `ToriiClient.callContract` with typed request/response normalisation so
  Node.js clients can invoke `/v1/contracts/call` without hand-crafting JSON,
  keeping contract execution coverage aligned with the roadmap’s JS-04/JS-06
  goals.
- Added `ToriiClient.iterateConnectApps` along with cursor-aware pagination
  helpers, TypeScript definitions, README usage, and Jest coverage so Connect
  registry admins can stream `/v1/connect/app/apps` listings without manual
  `cursor` bookkeeping, advancing the roadmap’s JS-04 complex pagination
  deliverable.
- Added `ToriiClient.getExplorerAccountQr` with typed DTOs, TypeScript
  definitions, README usage, and Jest coverage so wallets and explorers can
  fetch share-ready QR payloads (IH58 preferred or compressed (`snx1`, second-best)) directly from Torii
  instead of reimplementing the renderer, progressing ADDR-6b’s SDK coverage
  goals.【javascript/iroha_js/src/toriiClient.js:1440】【javascript/iroha_js/index.d.ts:3513】【javascript/iroha_js/README.md:1538】【javascript/iroha_js/test/toriiClient.test.js:6650】
- Broadened the Dockerised integration smoke suite to cover asset re-mint
  flows, iterator-based queries, and an optional ISO `pacs.008` submission; the
  README now documents the new environment toggles and the CI workflow emits
  runtime/cache telemetry so JS-10’s “docs + tests + metrics” gate exercises
  more real-world scenarios.【javascript/iroha_js/test/integrationTorii.test.js:1】【javascript/iroha_js/README.md:1325】【.github/workflows/javascript-sdk.yml:56】
- Added an optional RBC sampling integration test driven by
  `IROHA_TORII_INTEGRATION_RBC_SAMPLE`; when set, the suite now calls
  `ToriiClient.sampleRbcChunks()` against the live node and validates the typed
  chunk proofs/audit paths so JS-10 coverage includes the RBC observability
  surface, and the README explains the new behaviour.【javascript/iroha_js/test/integrationTorii.test.js:1】【javascript/iroha_js/README.md:1325】
- Hardened `ToriiClient.createConnectSession`/`deleteConnectSession` by
  normalising `sid`, enforcing the 32-byte base64url/hex requirement, surfacing
  `extra` metadata, and rejecting malformed Torii responses so the Connect
  overlay helper satisfies the roadmap’s JS-04 validation goals. The README
  example now calls out the sid constraints plus returned URIs/tokens, and the
  TypeScript definitions/tests document the stricter behaviour.【javascript/iroha_js/src/toriiClient.js:2532】【javascript/iroha_js/README.md:1147】【javascript/iroha_js/test/toriiClient.test.js:5731】
- Added `generateConnectSid` and `createConnectSessionPreview` with README usage,
  TypeScript definitions, and Jest coverage so the JS SDK can mint session ids,
  derive deeplink URIs, and expose the Connect preview workflow called out in
  the roadmap’s JS-04 Connect deliverable.【javascript/iroha_js/src/connectSession.js:1】【javascript/iroha_js/index.d.ts:1295】【javascript/iroha_js/README.md:1140】【javascript/iroha_js/test/connectSession.test.js:1】
- Added `bootstrapConnectPreviewSession` to bundle the preview + Torii
  registration flow in one helper, with README docs, TypeScript declarations,
  and tests so JS SDK consumers can script the Connect preview setup without
  rewriting the session orchestration logic.【javascript/iroha_js/src/connectPreviewFlow.js:1】【javascript/iroha_js/src/index.js:1】【javascript/iroha_js/index.d.ts:1473】【javascript/iroha_js/README.md:1367】【javascript/iroha_js/test/connectPreviewFlow.test.js:1】
- Added normalised pipeline status helpers
  (`getTransactionStatus`, `getTransactionStatusTyped`,
  `waitForTransactionStatusTyped`, `submitTransactionAndWaitTyped`) with
  TypeScript definitions, README usage, and Jest coverage so JS-04 validation
  also covers the `/v1/pipeline/*` surfaces: the raw helper now enforces the
  canonical `kind`/`content.hash`/`status.kind` layout while the typed
  wrappers expose DTOs instead of forcing consumers to inspect Torii's JSON
  blobs.
- Added `getNetworkTimeNow` and `getNetworkTimeStatus` helpers (with TypeScript
  definitions, README usage, and tests) so the JS SDK can query the `/v1/time/*`
  endpoints and keep the NRPC/AND7 network-time diagnostics in lockstep with
  the Rust/Python clients.
- Normalized `ToriiClient.getHealth()` so it returns a typed `{status: string}`
  snapshot when Torii replies with JSON payloads, keeping the health telemetry
  surface aligned with JS-04 validation coverage.
- Normalised `ToriiClient.getBlock`/`listBlocks` responses into the typed
  `ToriiExplorerBlock` and `ToriiExplorerBlocksPage` DTOs (404 now yields `null`
  for `getBlock`), added TypeScript definitions/README docs, and extended the
  Jest suite so the explorer block endpoints enjoy the same JS-04 validation
  guarantees as the rest of the Torii query surface.
- Added `getSumeragiTelemetry`/`getSumeragiTelemetryTyped` to `ToriiClient`
  with typed availability/RBC/VRF snapshots, README guidance, and TypeScript
  definitions so JS SDK users can replay `/v1/sumeragi/telemetry` data as part
  of the JS-04/JS-07 roadmap telemetry coverage without bespoke parsing, and
  tightened `sampleRbcChunks` validation so block/hash/proof fields must be
  hex-encoded like the Rust/Python clients.
- Added `ToriiClient.listTelemetryPeersInfo` plus the corresponding DTOs,
  TypeScript declarations, README usage example, and unit tests so the JS SDK
  exposes the `/v1/telemetry/peers-info` surface that Rust/Python clients rely
  on for telemetry replay/peer analytics.
- Added `NoritoRpcClient` and the accompanying `NoritoRpcError`, TypeScript
  definitions, and tests so Node consumers can call the binary Norito-RPC
  surface with first-class helpers instead of wiring bespoke fetch logic.
- Added the missing `ToriiClient.submitTransactionAndWait` helper so the
  runtime now matches the published TypeScript definitions, validating
  `hashHex` inputs and reusing the existing pipeline polling logic.
- Added `ToriiClient` helpers for Sumeragi telemetry endpoints
  (`getSumeragiPacemaker`, `getSumeragiQc`, `getSumeragiPhases`,
  `getSumeragiBlsKeys`, `getSumeragiLeader`, `getSumeragiCollectors`,
  `getSumeragiParams`) with README examples, TypeScript definitions, and tests
  so JS SDK consumers can inspect the same `/v1/sumeragi/*` diagnostics that
  Rust tooling relies on for roadmap JS-08 coverage.
- Hardened `listGovernanceInstances`/`listContractInstances` validation so
  `hashPrefix` must be hexadecimal and `order` is clamped to the Torii-supported
  values (`cid_asc`, `cid_desc`, `hash_asc`, `hash_desc`). The TypeScript
  definitions now encode the same order enum to keep JS-04 validation/typedef
  parity green.
- Added governance HTTP helpers (`governanceProposeDeployContract`,
  `governanceSubmitPlainBallot`, `governanceSubmitZkBallot`,
  `governanceSubmitZkBallotV1`, `governanceSubmitZkBallotProofV1`) with input
  validation, README snippets, and TypeScript definitions so the JS SDK covers
  the `/v1/gov/proposals/deploy-contract` and ballot DTOs described in
  `docs/source/governance_api.md`.
- Added `addressFormat` support to `ToriiClient.listAccounts`/`queryAccounts`
  (plus TypeScript definitions, README guidance, and tests) so SDK consumers
  can request compressed `snx1` literals when fulfilling ADDR-5 roadmap items.
- Added runtime capability helpers to `ToriiClient`
  (`getNodeCapabilities`, `getRuntimeAbiActive`, `getRuntimeAbiHash`,
  `getRuntimeMetrics`, `listRuntimeUpgrades`) with README snippets, TypeScript
  definitions, and unit tests so JS-07 advanced endpoint coverage now includes
  the `/v1/node/capabilities` and `/v1/runtime/*` surfaces exposed by Torii.
- Added runtime upgrade transaction helpers
  (`proposeRuntimeUpgrade`, `activateRuntimeUpgrade`, `cancelRuntimeUpgrade`)
  so rollout automation can post manifests and fetch transaction skeletons
  directly from the JS SDK, complete with typed inputs, README examples, and
  Jest coverage.
- Added the missing `iterate*Query` TypeScript declarations
  (`iterateAccountsQuery`, `iterateDomainsQuery`,
  `iterateAssetDefinitionsQuery`, `iterateNftsQuery`,
  `iterateAccountAssetsQuery`, `iterateAccountTransactionsQuery`,
  `iterateAssetHoldersQuery`) so the typings now match the runtime
  implementation and close the remaining JS-04 validation/type gaps.
- Added shared Torii governance/query fixtures
  (`javascript/iroha_js/test/fixtures/torii_responses.json`) and migrated the
  `toriiClient` governance + iterable tests to those payloads so JS-04 parity
  checks rely on the same deterministic responses that Rust/Python SDKs use
  for ballot/proposal/council validation.
- Hardened `ToriiClient.listSumeragiEvidence` so responses are validated and
  normalised into the typed `SumeragiEvidenceRecord` structures, rejecting
  malformed fields and keeping runtime behaviour aligned with the published
  TypeScript definitions/tests.
- Validated alias resolution responses for `ToriiClient.resolveAlias` and
  `resolveAliasByIndex`, normalising payloads into the published
  `AliasResolutionDto` shape and rejecting malformed fields so JS-04 query
  parity remains strict across runtimes. Tests now assert both happy-path
  normalisation and failure diagnostics.
- Added `listSorafsPinManifests` to `ToriiClient` plus typed DTOs for the pin
  registry so SDK consumers can page `/v1/sorafs/pin` with status filters and
  attestation metadata, completing the registry listing coverage called out in
  the pin-registry plan.
- Added `listSorafsAliases` and `listSorafsReplicationOrders` to `ToriiClient`
  so the JS SDK can page the `/v1/sorafs/aliases` and `/v1/sorafs/replication`
  endpoints with typed attestation metadata, fulfilling the remaining JS-07
  coverage for the pin registry's observability APIs.
- Added `ToriiClient.iterateSorafsPinManifests`,
  `iterateSorafsAliases`, and `iterateSorafsReplicationOrders` with README
  snippets, TypeScript declarations, and Jest coverage so the offset iterator
  helper can stream the SoraFS registry endpoints without bespoke pagination,
  extending the JS-07 query wrapper work to the storage APIs.【javascript/iroha_js/src/toriiClient.js:681】【javascript/iroha_js/index.d.ts:3408】【javascript/iroha_js/test/toriiClient.test.js:760】【javascript/iroha_js/README.md:732】
- Tightened the Sumeragi telemetry helpers by normalising the
  `getSumeragiPacemaker`, `getSumeragiQc`, `getSumeragiPhases`,
  `getSumeragiBlsKeys`, `getSumeragiLeader`, `getSumeragiCollectors`, and
  `getSumeragiParams` responses (raising type errors on malformed telemetry)
  and corrected the `getSumeragiStatus` TypeScript declaration to reflect that
  it returns the raw Torii payload while `getSumeragiStatusTyped` provides the
  validated snapshot.

## [0.0.2] - 2026-01-27

- Added governance instruction support to native Norito helpers so
  `buildCastZkBallotInstruction`, `buildCastPlainBallotInstruction`,
  `buildEnactReferendumInstruction`, `buildFinalizeReferendumInstruction`, and
  `buildPersistCouncilForEpochInstruction` now round-trip through
  `noritoEncodeInstruction`.
- Updated the native build script to try an offline cargo build first and
  automatically retry online when dependencies are missing.
- Added release documentation automation script covering changelog/status/roadmap updates.
- Added opt-in Torii integration smoke tests and documentation for exercising ISO bridge, RBC sampling, and Connect endpoints from the JS SDK.
- Added `submitIsoPacs009` helper mirroring the new Torii `/v1/iso20022/pacs009` endpoint,
  updated recipes/README, and extended TypeScript definitions/tests so PvP funding legs can be
  submitted from the SDK alongside pacs.008 flows.

## [0.0.1] - 2024-01-01

- Initial preview release of the Norito/Torii JavaScript SDK.

<!--
Maintainer note: keep this changelog aligned with the versions published to npm.
Continue to follow the Keep a Changelog format with ISO-8601 dates.
-->
