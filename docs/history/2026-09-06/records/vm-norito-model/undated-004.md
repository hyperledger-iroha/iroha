# Historical project evidence

This page is historical evidence, not current release readiness.
See [the archive index](../../index.md) for provenance and reconstruction.


<a id="record-6095c41747dc5b390200c5a9da0b568dd4befe6639c29d05cdc6f94fdef6ad75"></a>

<!-- Original context: Roadmap / Release and Stabilization -->
- SFM-4c transparency ledger V1 data-model payloads are now shipped:
  `iroha_data_model::sorafs::transparency` defines
  `ModerationLedgerEntryV1`, `ModerationLedgerBlockV1`, and
  `ModerationLedgerProofV1` with Norito schemas, domain-separated BLAKE3
  entry/block hashing, deterministic cycle entry sorting, Merkle root/proof
  verification helpers, and focused roundtrip/tamper/ordering coverage. It now
  also defines `ModerationLedgerCyclePublicationV1`, and `sorafs_node` can
  publish validated publication bundles through the local Governance DAG
  filesystem sink, publish index, digest sidecars, and CAR queue under the
  `transparency_ledger_publication` payload kind. `sorafs_manifest` now also
  provides `GovernanceExternalPayloadV1`, and the local node signs canonical
  transparency publication bytes into the optional runtime Governance DAG when
  a signer is configured. Torii now also exposes local readback endpoints for
  published transparency cycles and entry inclusion proofs, with artifact path,
  BLAKE3, Norito decode, and Merkle-proof verification against the Governance
  DAG publish-index, and cycle detail readback bounds returned publication
  proofs with full verification counts preserved. Torii also exposes local
  SFGT proof-token verification for
  caller-supplied gateway public keys and optional runtime-only evidence-binding
  material, now honoring configured Torii API-token enforcement and the shared
  proof API rate limiter with `429`/`Retry-After` throttle responses. The
  proof-token issuance index foundation is now also shipped:
  `ProofTokenIssuanceV1` records privacy-safe issued-token summaries with
  canonical Norito hashing and `ProofTokenIssuance` ledger-entry conversion,
  `sorafs_node::NodeHandle::publish_proof_token_issuance(...)` writes validated
  issuance records through the local Governance DAG filesystem sink under the
  `proof_token_issuance` payload kind, `sorafs_node` can derive and publish
  those issuance records from signed `SFGT` frames via
  `proof_token_issuance_from_base64(...)` /
  `NodeHandle::publish_proof_token_base64_issuance(...)` after verifying the
  raw Ed25519 signer key, and Torii lists those local issuance entries at
  `/v1/sorafs/transparency/tokens` with action-code, signer, token,
  bound-entry, expiry, evidence-digest summaries, total/returned counts, and a
  bounded `limit` query for returned entries. Torii SoraFS publish-index
  coverage now pins no-query governance DAG and proof-token issuance readback
  responses to the shared bounded default list limit while preserving explicit
  missing-digest negative controls. `sorafs_node` also now
  exposes a generic local transparency source-entry worker via
  `TransparencyLedgerSourceEntry`,
  `record_transparency_ledger_source_entry(...)`, and
  `publish_transparency_ledger_cycle_from_source_entries(...)`, covering
  privacy-safe GAR/moderation/appeal/legal-hold/redaction/evidence-access style
  entries with duplicate-id rejection, deterministic cycle-window sorting,
  stable ledger entry ids, cycle publication construction, and Governance DAG
  publication. Concrete local source-entry adapters are now shipped for
  `GarEnforcementReceiptV1`, `SoraFsModerationBallotGovernanceEventV1`,
  `SoraFsAppealFinanceReportV1`, and
  `SoraFsAppealFinanceSettlementReceiptV1`; they derive canonical Norito
  payload digests plus sorted public metadata, and the local moderation/appeal
  paths expose trusted in-process adapter boundaries. Torii deliberately has no
  generic source-kind-selected public write route. The externally reachable
  token, privacy-aggregate, and appeal-finance Governance DAG writes require
  canonical account signatures plus exact publisher roles, and bind the
  server-verified account and stable origin into durable checkpoint/outbox
  records and publish-index labels. The same provenance is part of the signed
  public DAG node CID/signature preimages, with payload/origin validation and
  identity-aware idempotency. Finance publication requires provenance at the
  outbox and publisher type boundaries; proof-token and transparency payloads
  preserve it for authenticated ingress while trusted in-process producers are
  explicitly node-attested without caller provenance. Pre-change DAGs must be
  reseeded. The standalone public mirror cross-checks nullable runtime-index
  attribution against each signed node and publishes only the signed values,
  so its lookup index cannot relabel an attestation. Rollout collection accepts only
  pre-collected `sorafs.transparency.source_entry.producer_evidence.v1` from
  trusted internal producers; the gate requires internal producer routes,
  durable provenance digests, verified checkpoints, and
  `generic_public_ingress_absent=true`. The remaining SFM-4c
  production work is
  deployed producers for GAR, moderation, appeal, legal-hold, redaction, and
  evidence-viewer events plus captured trusted-producer evidence and
  the rollout evidence verifier,
  deployed anchoring/publisher identities, deployed proof API hardening beyond
  the local verifier throttle and bounded readback arrays, deployed proof-token
  issuance producers/explorer-linking rollout evidence, deployed public receipt
  explorer rollout evidence capture, and live privacy-safe moderation aggregate
  publisher. The rollout-gate static contract keeps the generic
  `/v1/transparency/*` route family out of Torii/OpenAPI and warning-only in
  SoraFS docs until the deployed transparency builder and public explorer
  exist. SFM-4b3 separately exposes the authenticated `/v1/evidence/*` family
  after finalized authorization, WebAuthn, rotating grants, signed receipts,
  and legal-hold-aware retention/erasure were implemented. The transparency
  public-route scanner uses segment-aware family/stem matching, so route labels
  such as `/v1/transparency-canary/*` stay local without weakening the generic
  public-route block. The warning-only public
  route scan now shares the broad SoraFS docs path inventory used by reserved
  operator-command checks, covering top-level SoraFS plans under `specs/` and
  nested `specs/sorafs/**` docs before repository-local public evidence routes
  can be published outside the reviewed warning context. Public and localized
  mirrors remain sibling `iroha-docs` inputs. The same static contract now also
  pins the
  deployed-only SFM-4c source-entry producer, GAR/moderation/appeal/legal-hold/
  redaction/evidence-viewer producer, publisher-identity/anchoring, public
  receipt explorer, proof-token issuance producer/explorer-linking,
  privacy-aggregate scheduler service, moderation-ledger service, and
  transparency promotion route/CLI names as unshipped while negative controls
  preserve the shipped local cycles, explorer, source-entry, privacy-aggregate,
  token issuance, and proof-token verification surfaces. The deployed route
  scanner now uses a reusable segment-aware matcher, with `/v1/transparency/*`
  treated as a blocked generic family while SoraFS deployed producer,
  explorer, proof-token, scheduler, ledger, and promotion route stems reject
  exact deployed routes without catching canary/evidence suffixes. It also
  scans CLI sources for nested deployed-only `transparency source-entry
  producer-service`, `transparency public-explorer`, `transparency
  proof-token-issuance explorer-linking`, and `transparency promote` spellings
  without blocking shipped local readback, canary, or producer-submission
  helpers. A rollout-contract self-audit now maps every
	  `UNSHIPPED_*_ROUTE_PATTERNS` constant to exactly one
	  `unshipped_*_route_matches` helper and fails if a helper stops using
	  segment-aware `re.search(re.escape(route), ...)` matching, drops the shared
	  left-boundary guard, or reintroduces raw `route in source` substring scans.
	  A runtime adversarial route-matcher test now feeds every reserved route
	  helper `x...`, `/internal...`, and `prefix-...` fake paths so prefixed
	  internal or diagnostic strings cannot satisfy public exposure checks. The
	  same self-audit now covers every
	  `UNSHIPPED_*_CLI_SUBCOMMANDS` family plus its paired
	  `UNSHIPPED_*_NESTED_CLI_COMMANDS` family, requiring quoted hyphenated-command
	  checks and boundary-aware nested-command regex matching instead of direct
	  `command in source` substring scans. Nested CLI command matching now treats
	  slash-prefixed fragments as invalid command starts too, and a runtime
	  adversarial CLI test feeds every helper quoted/backticked `x...`, `not-...`,
	  `/...`, `/internal/...`, and `...-canary` fragments so prefixed or suffixed
	  fake commands cannot satisfy the exact reserved command under test. A paired exposure self-audit now also
	  requires every unshipped route/CLI matcher helper to feed a fail-closed
  exposure test that scans real Torii/OpenAPI or CLI source and asserts the
  exposed surface inventory remains empty. The matcher inventory now also
  requires every unshipped route/CLI matcher helper to be exercised by an
  adversarial negative-control test before new unshipped surface families can
  be added.
  Torii now also exposes
  `/v1/sorafs/transparency/tokens/issuances` as a canonical-authenticated local
  proof-token issuance feed; it accepts one URL-safe base64 `SFGT` frame, the
  Ed25519 signer public key, optional evidence/policy digests, and sorted public
  metadata, then verifies and publishes the derived `ProofTokenIssuanceV1`
  through the local Governance DAG publisher when configured, without accepting
  blinded-digest keys. `iroha::Client` and `iroha sorafs transparency
  token-issuance submit --payload PATH` now wrap that signed feed for deployed
  producer automation, and `iroha sorafs transparency token-issuance canary
  --issuance PATH [--issuance PATH...] [--out PATH]` emits payload-free
  `sorafs.transparency.proof_token_issuance.canary.v1` rollout evidence with
  request/response sizes, status, and BLAKE3 hashes, without archiving
  proof-token frames, private digest-key material, or response bodies. Torii now
  also exposes
  `/v1/sorafs/transparency/explorer` as a local read-only explorer snapshot over
  the Governance DAG publish-index, returning cycle summaries, proof-token
  issuance summaries, payload-kind counts, source paths, index digests, cache
  validators, total/returned counts, and `limit`-bounded arrays for local UI
  integration without exposing private proof-token digest keys. Torii now also
  exposes `/v1/sorafs/transparency/explorer/ui` as a static local browser
  explorer that fetches that payload-free snapshot, renders cycle and
  proof-token issuance summaries, ships `no-store`/`nosniff`/CSP headers, and
  does not embed ledger payload bodies or private proof-token digest keys.
  `iroha sorafs transparency publication-canary [--cycle-id HEX...]
  [--limit N] [--torii-url URL] [--out PATH]` now probes deployed/public cycle
  list and optional cycle-detail readback, requires publisher identity fields
  unconditionally, checks anchor metadata plus verification flags, and emits
  payload-free `sorafs.transparency.publication_canary.v1` evidence with
  response sizes and BLAKE3 hashes without archiving publication bodies. The
  former missing-publisher-identity CLI waiver is closed, and the rollout
  static contract pins that production canary boundary. The
  CLI canary and transparency collection runner now reject non-lowercase,
  wrong-length, or otherwise malformed `--cycle-id` values before dry-run
  command plans or deployed cycle-detail probes are emitted.
  `iroha sorafs transparency explorer-canary` now probes deployed/public
  explorer snapshot, browser UI, and proof-token issuance index routes, verifies
  expected schemas/HTML markers, rejects ledger payload bodies and private
  proof-token digest-key material, and emits payload-free rollout evidence with
  response body hashes. `scripts/check_sorafs_transparency_rollout_evidence.py`
  now verifies the collected SFM-4c source-entry, publication, privacy
  aggregate, proof-token issuance, and explorer canary artifacts before
  promotion, emits `sorafs.transparency.rollout_evidence_gate.v1` summaries,
  and fails closed when any required artifact is missing, failed, missing
  supported source-entry producer kind coverage, missing publication
  cycle-detail coverage, missing publisher/anchor/verification signals,
  missing source-event or publish-due aggregate coverage, missing explorer
  snapshot/UI/proof-token index route coverage, or carrying raw
  payload, request/response body, bearer-token, signed-transaction,
  proof-token frame, private-key, or private digest-key fields, and binds
  publication and explorer `route_count` to the unique canonical
  `routes[].name` inventories so duplicate route rows cannot inflate readiness,
  binding publication `cycle_detail_probe_count` to the unique canonical
  `cycle_detail_probes[].name` inventory using reviewed
  `transparency-cycle-detail-*` probe labels without non-production markers so
  duplicate or placeholder cycle-detail rows cannot inflate readiness, while
  keeping probe-based `probe_count` values equal to the `probes[]` inventory
  length and requiring source-entry, source-event,
  publish-due, and proof-token issuance sub-counts to match the corresponding
  `probes[]` role inventory, and binding privacy aggregate plus proof-token issuance
  `probe_count` values to the unique canonical `probes[].action` inventory so
  duplicate action rows cannot inflate readiness. The gate now also rejects
  unknown values outside the reviewed source-kind, route, cycle-detail-probe,
  privacy-action, and proof-token action inventories.
  The gate also
  requires publication evidence to bind back to a valid source-entry
  `source_batch_digest_hex`, and requires privacy aggregate, proof-token
  issuance, and explorer evidence to bind back to a source-bound publication
  `cycle_digest_hex` from the same rollout bundle; publication cycles that
  fail source-entry binding do not anchor downstream rollout evidence, and
  source-batch/cycle binding failures are recorded through the shared scalar
  binding error recorder. The transparency checker now exports
  `valid_publication_bindings`, and the aggregate production-readiness gate
  requires those source-batch/cycle binding tuples to match publication
  fingerprints while requiring every ready cycle digest to come from a
  source-bound publication, and now rechecks source-bound and cycle-bound
  artifact fingerprints against `valid_source_batch_digests` and
  `valid_cycle_digests`. The gate now also requires exactly one active source
  batch digest, publication cycle digest, and publication binding, clearing
  mixed `valid_source_batch_digests`, `valid_cycle_digests`, or
  `valid_publication_bindings` before bound artifact or aggregate metadata can
  promote. The lane checker also has direct adversarial
  coverage that forges publication `source_batch_digest_hex` and every
  cycle-bound downstream `cycle_digest_hex`, so publication, privacy aggregate,
  proof-token issuance, and explorer evidence fail against detached source or
  publication anchors before promotion. The checker also exports those required
  top-level evidence payload fields as `EVIDENCE_REQUIRED_FIELDS`, and the
  collection harness includes the checker-backed `evidence_contract` map in
  dry-run output so operators can review the exact SFM-4c artifact contract
  before contacting live services.
  `scripts/run_sorafs_transparency_rollout_evidence.py` now provides the
  operator collection harness for those gates: it requires every supported
  source-entry kind, rejects duplicate or unsupported `--source-entry` kinds,
  and requires privacy source-event/publish-due payloads, proof-token issuance
  payloads, and publication cycle ids before running the canaries, then invokes
  the verifier over the collected artifact directory. Repeated
  `--iroha-arg ARG` values pass runtime-only client config/signing options
  before `sorafs`, shell-style `@ARGFILE` response files keep reviewed
  operator inputs reproducible without embedding secrets, and `--dry-run` emits
  the exact command plan for rollout review without contacting live services.
  `scripts/examples/sorafs_transparency_rollout_collection.args.example`
  captures the required source-entry, aggregate, proof-token issuance,
  publication cycle, and Torii URL inputs while pointing signing material at
  runtime-only client config, and
  `scripts/examples/sorafs_transparency_rollout_evidence.args.example` remains
  the direct verifier argfile for captured payload-free artifacts.
	  `scripts/build_sorafs_transparency_canary.py` now provides the fail-closed
	  payload-free SFM-4c canary builder for reviewed source-entry, publication,
	  privacy-aggregate, proof-token issuance, and explorer rollout artifacts,
	  requires complete source-kind, publication-route, cycle-detail-probe,
	  privacy-action, and explorer-route inventories, rejects duplicate, unknown,
	  or non-production cycle-detail probe labels before writing, defaults
	  `--cycle-detail-probe-count` from the reviewed cycle-detail probe
	  inventory, and includes
	  response-file examples for source-entry and publication canary generation
	  plus checker-backed validation before atomic JSON writes.
  The rollout-gate static contract now pins deployed source-entry producers,
  deployed publisher identities/anchoring, deployed proof API hardening,
  public receipt explorer rollout, deployed proof-token producer/explorer
  linking, deployed moderation ledger publication service, generic
  `/v1/transparency/*` routes, and matching production-service CLI commands as
  unshipped while preserving the local `/v1/sorafs/transparency/*` readback,
  source-entry ingest, privacy aggregate, proof-token, explorer, and canary
  surfaces, including exact nested deployed-service CLI spellings.
  The remaining rollout gap is captured live deployed evidence that passes that
  gate. The
  data-model foundation for that publisher is now shipped as
  `ModerationPrivacyAggregateV1` plus explicit
  `ModerationPrivacyParametersV1` epsilon/delta/suppression metadata,
  deterministic aggregate hashing, sorted metric/metadata validation, and
  conversion into `PrivacyAggregate` ledger entries. `sorafs_node` exposes
  local aggregate source-event ingestion via
  `record_privacy_aggregate_source_event(...)` and the single production
  publication path
  `publish_due_configured_privacy_aggregate_cycle_from_source_events(...)`,
  including duplicate source-event rejection, cycle-window filtering,
  distinct-subject suppression, per-subject clipping, exact integer
  discrete-Laplace sampling, source-payload digest binding, and atomic
  composition-budget/cycle/outbox persistence. `PrivacyAggregateScheduleConfig`
  now derives due
  publication windows, deterministic cycle ids, stale-window catch-up for the
  oldest due unpublished window with retained source events, and structured
  skip outcomes for not-due, already-published, empty, and fully suppressed
  cycles while publishing each due cycle at most once per node runtime.
  `iroha_config` now also exposes the dormant-by-default
  `[sorafs.storage.privacy_aggregates]` cadence, canonical rational privacy
  policy, per-subject cap, suppression threshold, governed digest, and durable
  composition budget. It also requires an exact non-secret release-anchor
  handle/revision/policy digest whenever enabled and the same exact
  threshold-PRF binding for DP modes, while rejecting dormant provider fields.
  `sorafs_node::StorageConfig` projects that single production policy and both
  provider pins. Standard Node, Torii, and `irohad` launcher paths now accept
  only production-qualified provider traits and construct the rotation-aware
  wrappers before persistence; missing, unexpected, substituted, stale,
  unavailable, test-marked, zero, and config-mismatched qualifications fail
  closed without provider diagnostics. A prebuilt Node is checked against the
  exact Torii config bindings and cannot also receive ambiguous raw providers.
  The
  `publish_due_configured_privacy_aggregate_cycle_from_source_events(...)`
  runs due-cycle publication while accepting only runtime threshold-PRF output
  and predecessor hash material. Torii now also exposes
  `/v1/sorafs/transparency/privacy-aggregates/source-events` as a
  canonical-authenticated local feed boundary for privacy aggregate source
  events, routing accepted events into the duplicate-checked aggregate worker
  and returning only event ids, digests, and counts rather than raw metric
  values. Torii now also exposes
  `/v1/sorafs/transparency/privacy-aggregates/publish-due` as a
  canonical-authenticated local trigger for configured due aggregate
  publication, with stale due event-backed window catch-up, config-authoritative
  privacy policy, atomic composition-budget/outbox persistence, and structured
  published/skipped/already-published outcomes. `iroha::Client` and
  `iroha sorafs transparency privacy-aggregate source-event|publish-due
	  --payload PATH` now wrap those signed routes for producer and scheduler
	  automation. `iroha sorafs transparency privacy-aggregate canary
	  --source-event PATH [--source-event PATH...] [--publish-due PATH...]
	  [--out PATH]` now submits canary source-event and publish-due payloads
	  through the signed routes, records request/response sizes, status, and BLAKE3
	  hashes, and emits `sorafs.transparency.privacy_aggregate.canary.v1`
	  evidence without archiving raw metric arrays, metric names, or response
	  bodies. The SFM-4c rollout gate now also requires the privacy aggregate
	  canary probe array to include both action labels, `source_event` and
	  `publish_due`, binds privacy aggregate `probe_count` to the unique
	  canonical `probes[].action` inventory with duplicate action rejection, so
	  top-level probe counts cannot stand in for deployed producer and scheduler
	  evidence, and source-entry, privacy-aggregate, and
	  proof-token issuance probes now require request/response BLAKE3 hashes so
	  replay evidence stays payload-free while publication and explorer route
	  evidence also requires response BLAKE3 hashes to bind exact deployed
	  exchanges. The transparency rollout gate now also requires reviewed
  `deployment_id`/`environment` context on every artifact, and the collection
  runner stamps that context onto generated canary artifacts and now validates
  the schema-closed collection-plan envelope, reviewed deployment context,
  evidence contract, and command steps before dry-run output or live canaries.
  The local stock registry/broker client/server transport for transparency
  slots 2–10 is complete: sanitized config projection, exact fixed-endpoint
  handshake, bounded canonical operations, fail-closed scope and live
  requalification, CAS readback, fencing, and standard-launcher injection.
  Remaining work is the supervised deployment-owned broker executable with
  genuine independently administered threshold-PRF, finalized-anchor,
  sealed-CAS leader-lease, fused writer/authoritative-head-reader, and an
  authenticated deployment-selected Governance DAG signer backend, plus every
  finalized source producer, public replicas/proofs/pagination/ETags, hardened
  explorer delivery, deployed scheduler jobs, and captured rollout evidence
  using those backends and the canary.


<a id="record-82dafc573ce44144b9acfde94ffe2a381f8de9449354a5ee679e71093b94dfeb"></a>

- Solana live account/program imports reject non-string verifier program ids,
  ProgramData addresses, verifier code hashes, ProgramData metadata hashes, and
  copied base64 account/program byte fields before parser dispatch, with the
  same hostile-object regression pinned in strict/readiness inventories.


<a id="record-8a6d74392e5dacc12638443da1d20f4902c658a2ba98c903697bcebe5c552311"></a>

- EVM live destination copied metadata now rejects non-string destination,
  source-record, route allowlist, route-canary, and Torii query hashes or
  addresses before generated TOML argument emission, with hostile-object
  coverage pinned in strict/readiness inventories.


<a id="record-cb30229b30781c64db1d2c2d2aa158147d60fb53dec5c6dae9387ba154851736"></a>

- EVM source-live copied metadata now rejects non-string source bridge,
  deployment receipt, expected bridge-code, and source-record hashes before TOML
  prerequisites or generated source-material output can mask malformed copied
  evidence.


<a id="record-86411cf71e9ce3646cc4e0a9d6b41f270567e70a70711e0ad66bf6af5386ad73"></a>

- Keep the direct-Serde migration closed: `scripts/serde_allowlist.txt` is
  empty, and `make guards` keeps new direct `serde_json` usage and retired
  non-Norito codec dependencies, including renamed retired-codec package
  aliases, out of the workspace. `tools/soranet-relay`, `crates/iroha_core`,
  `crates/iroha_cli`, `crates/iroha_torii`, and `crates/iroha_sccp` have been
  removed from that allowlist after their relay JSON paths, STARK/FRI envelope
  types, contract-app TOML manifest decoder, Torii SCCP query DTOs, Torii
  tx-history/push JWT JSON paths, and SCCP public payload/proof DTOs were
  verified to build without direct Serde dependencies.
  The stale workspace-level `bincode` dependency is removed; remaining Solana
  bincode-layout wording refers to hand-validated external protocol bytes, not
  a production codec dependency.

