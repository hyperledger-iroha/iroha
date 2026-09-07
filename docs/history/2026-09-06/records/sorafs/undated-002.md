# Historical project evidence

This page is historical evidence, not current release readiness.
See [the archive index](../../index.md) for provenance and reconstruction.


<a id="record-38fae34937dcb81c79140e6d91db730efee2014cda822f52a0e4f44e480252a4"></a>

<!-- Original context: Roadmap / Release and Stabilization -->
- SoraFS reputation V1 now has the deterministic snapshot/proof core and a
  native committed input journal. The journal pins a governed
  predecessor-bound recorder-policy history, one global contiguous sequence,
  exact source revisions and predecessors, provider/policy/authority/block-time
  binding, payload-free typed commit events, and a fixed-view finalized query.
  PoR terminals and stream-token outcomes have exact native append
  instructions. `RegisterCapacityDispute` appends `Opened` atomically with the
  canonical dispute record, while `ResolveSorafsCapacityDispute` updates the
  record and appends its exact revision-two `Resolved` event atomically;
  capacity telemetry may apply penalties and alerts but never creates a
  dispute. The existing snapshot core retains canonical Norito/JSON schemas,
  fixed-point provider scoring, fixed-point EigenTrust-style trust-edge
  iteration, degradation flags, snapshot Merkle roots/proofs, Governance DAG
  payload validation, and scoreboard consumption through
  `reputation_score_bps`. `sorafs_cli reputation verify` also validates
  archived Norito snapshots and optional provider Merkle proofs. The exported
  finalized multi-feed projector consumes the native proof, unified journal,
  repair, orderbook, reserve-event, and reserve-provider projections with five
  restart-safe cursors and a canonical bounded unsigned-material
  retry/dead-letter/acknowledgement outbox. Signed-result acknowledgement
  verifies the anchored public trust policy, quorum, signer revocation,
  signatures, freshness/future skew, and exact snapshot/evidence/signing
  digest. The competing Torii POST/OpenAPI operation and `sorafs_cli reputation
  publish` command are removed; the remaining reputation Torii/CLI surface is
  read-only. Historical snapshot lookup,
  latest-weight discovery, sequenced snapshot events, bounded latest/historical
  snapshot provider readback, and bounded CLI event watching are also covered
  locally, with deterministic `ETag`/`Cache-Control` validators on reputation
  GET responses and a live server-sent event stream plus
  `/v1/sorafs/reputation/events/ws`
  WebSocket parity for snapshot publications. The
  `iroha_js_host` NAPI bridge, `iroha_python_rs` bridge, and
  `connect_norito_bridge` JSON decoder now also carry telemetry
  `reputation_score_bps` into the SoraFS local-fetch scheduler, and the shared
  multi-peer parity fixture keeps the neutral 10_000 bps path pinned across
  Rust and JavaScript. The JavaScript/TypeScript and Python Torii
  clients also expose local convenience helpers for
  latest/provider/snapshot/weights reads, event polling, and SSE consumption with
  cache-validator options. Reputation publisher health metrics, a bounded
  top-provider score gauge, low-score threshold-crossing counters, a Grafana
  dashboard, and Prometheus alerts are defined. Strict standard-daemon
  configuration, dependency injection, supervision, status, and metrics are
  wired. An authenticated journal-transaction submitter remains injected;
  standard `irohad` has no validator-key, queue-backed, or current-head
  fallback. The exact historical query is now daemon-owned: startup opens and
  zero-gap reconciles the configured Kura-authenticated archive, and the V2
  apply corridor captures every fresh committed height before State
  publication. Startup validates the complete immutable bootstrap response
  against the exact request before opening the checkpoint. External signer qualification is
  already bound to the full canonical trust policy and fenced before/after
  every call; Governance DAG qualification binds both the configured publisher
  peer and Ed25519 public key.
  External threshold-signing, Governance DAG publication/readback/head-
  inclusion, and producer-owner adapters, integrated Rust validation, and
  reviewed four-peer live evidence remain open.
  `scripts/check_sorafs_reputation_rollout_evidence.py` now gates deployed
  SFM-3 rollout evidence: publish/latest/provider/event/proof replay artifacts,
  integer-unit metrics freshness/ingest lag, SSE/WebSocket transport delivery,
	  and routing/incentive consumption must all reference the same fresh
	  publish/latest `snapshot_id_hex`/`merkle_root_hex` tuple and stay payload-free
		  before the summary reports `ready`; the aggregate production-readiness gate
		  also requires `valid_snapshot_bindings` to match that top-level
		  `snapshot_id_hex`/`merkle_root_hex` pair and rechecks snapshot-bound artifact
		  fingerprints against `valid_snapshot_bindings` before final promotion,
		  rejecting raw
  snapshot/proof/provider records, request/response bodies, bearer tokens,
  signed transactions, private keys, and other payload-bearing fields. Metrics
  and transport artifacts must explicitly set
  `response_bodies_included` to `false`, and routing/incentive consumption
  artifacts must explicitly set `raw_provider_records_included` to `false`,
  before promotion can report ready, with a consolidated missing-field
  regression loop covering each required payload-safety artifact. Snapshot
  anchors, proof-verification, metrics, and routing/incentive consumption
  artifacts now also bind `provider_count` to the unique canonical
  `providers[].name` inventory and reject duplicate provider entries before
  promotion can report ready; those provider inventory names must use the same
  reviewed lowercase production `provider-*` shape without non-production
  markers as provider proof and verification `provider_id` fields. Required
  provider coverage now also requires the provider ID to appear in both
  provider-proof and proof-verification evidence, and the default gate requires
  at least one provider ID to be present in both sets so verification-only or
  proof-only coverage cannot satisfy readiness.
  Event-watch artifacts now also bind `count` to
  duplicate-free `events[].sequence` values in addition to `events[]` length
  and `limit` checks, and require every V1 event row in the batch to carry the
  same snapshot id, Merkle root, and provider count. Transport artifacts bind
  `sse_event_count` and `websocket_event_count` to reviewed
  `sse_events[].name` and `websocket_events[].name` inventories, require
  `reputation-sse-event-*` and `reputation-websocket-event-*` production labels
  without non-production markers, so malformed or repeated event/transport rows
  cannot inflate readiness. Snapshot
  binding failures are marked on the offending artifact before required-kind
  summary validity is reported, and malformed snapshot-binding validation
  inputs now fail closed on required/bound kind containers, binding-pair
  containers, snapshot-bound artifact row containers, and diagnostic labels
  before artifact/anchor matching. Custom
  required-evidence rows now also reject malformed evidence labels, required-kind
  labels, record-time artifact rows, artifact row containers, and artifact error
  strings before formatting rollout summary diagnostics. Custom required
  artifact recording also rejects malformed existing rows or artifact buckets
  before appending new evidence, so dirty row state cannot be normalized by a
  later valid artifact. The checker now exports the required top-level payload
  fields as `EVIDENCE_REQUIRED_FIELDS`, and the collection harness includes the
  checker-backed `evidence_contract` map in dry-run output for publish/latest,
  provider, events, verify, metrics, transport, and consumption artifacts.
  Schema-less directory evidence kind inference now uses only exact reviewed
  filename stems and exact `provider-`/`verify-` prefixes, rejecting uppercase,
  underscore, `fetch`, `proof`, `watch`, `sse`, `prometheus`, and loose routing
  aliases so untyped files cannot satisfy required rows by filename
  normalization.
  Standard artifact summaries sanitize malformed `schema`/`status` fields and
  fail otherwise-clean artifacts before those fields can enter rollout reports.
  Required evidence summaries now also reject artifact buckets containing
  non-object rows as malformed gate input, so scalar or mixed row sequences
  cannot be reported as present-but-invalid evidence. Required summary
  readiness now also requires empty error lists, non-empty artifact-row
  sequences, and explicitly valid artifact rows before a gate can report ready.
  Recognized artifact recording now rejects existing artifact buckets containing
  non-object rows before appending new evidence, so dirty bucket state cannot be
  normalized by a later valid artifact.
  Recognized artifact counts now only count mapping artifact rows, so malformed
  scalar or mixed row sequences cannot inflate standard or custom rollout
  summary totals.
  Gate status selection now only reports `ready` for an actual empty `list[str]`
  of canonical summary errors; malformed status containers or diagnostics stay
  blocked.
  Scalar binding checks now validate values before normalization or
  allowed-container inspection, so malformed values report the canonical value
  diagnostic even when allowed bindings are malformed too.
  Digest-reference checks now canonicalize anchor/allowed digest collections
  before truthiness, missing-anchor, or artifact-mutation branches run, so
  malformed anchor containers and malformed anchor digest values fail closed as
  gate-configuration errors instead of being treated as absent anchors or
  artifact mismatches.
  Tuple-bound reference checks now apply the same canonicalization to
  multi-field anchor bindings before missing-anchor or artifact-mutation
  branches run, so malformed anchor-binding containers and tuple values fail as
  gate-configuration errors instead of producing misleading artifact failures.
  The gate now also
  fails closed on invalid recognized artifacts, including stale duplicate
  evidence for an otherwise valid kind and invalid optional artifacts outside a
  narrowed `--require-kind` subset. `scripts/run_sorafs_reputation_rollout_evidence.py`
  now drives the bounded deployed collection path, including reviewed
  publication binding, provider fetch, proof replay, one exact event-watch
  poll, provider-proof coverage checks, shell-style `@ARGFILE` support, and the
  final gate invocation. Its strict source adapter accepts only schema-closed
  full CLI profiles, binds them to the reviewed deployment/snapshot/root/time,
  provider inventory and weights, proof geometry, score, and watch request,
  then emits payload-free canonical canaries. Raw sources use non-JSON
  extensions and every final checker input carries an explicit evidence kind,
  preventing recursive duplicate discovery. The collector also requires the
  same exact canonical HTTPS-or-loopback origin profile as the Rust CLI and
  redacts the runtime key-file path from dry-run JSON and `RUN` notices while
  retaining it only in the in-memory subprocess command. Reputation
  rollout summaries now also publish the common aggregate-readiness contract:
  full `required_kinds`, top-level evidence/artifact counts, required-row
  `present`/`artifact_count` fields, and reviewed deployment-context
  fingerprints on every required artifact so the final SoraFS production
  aggregate gate can consume real SFM-3 summaries directly, while the final
  aggregate deployment id must be reviewed and free of staging markers even when
  earlier per-lane rollout summaries came from staging. The static rollout
	  contract now imports every SoraFS rollout/release checker and the aggregate
	  gate together, requiring the aggregate schema list, default required gates,
	  checker `DEFAULT_REQUIRED_KINDS`, and collection-runner summary flags to stay
	  in lockstep before production readiness can be claimed. It now also scans
	  active SoraFS implementation, SDK, CLI, Torii/config, and operator-script
		  source paths for active follow-up markers, auto-discovers path-named
		  SoraFS sources outside the hand-curated roots, and carries adversarial
		  marker-detection controls, including lowercase and mixed-case active
		  markers such as `todo:` and `FixMe(...)`. The scanner now also treats
		  Swift as a first-release SoraFS source type, pins the Swift SoraFS
		  Torii client, native bridge, orchestrator client, options, and
		  reference-validator entry points into the active sweep, pins Python,
		  JavaScript/TypeScript declaration, Java Android, Kotlin/JVM, and C#
		  SoraFS client/instruction entry points that do not carry `sorafs` in
		  their path names, includes the SoraFS reference and shared Norito bridge
		  C headers, includes the SF1 vector checker and extensionless
		  `sorafs-gateway` operator wrapper, sweeps every checked-in SoraFS
		  argfile example under `scripts/examples`, auto-discovers SoraFS-bearing
		  Python and shell operator/support scripts under `scripts/` by content,
		  explicitly sweeps SoraFS-adjacent CI gates for the Norito bridge header,
		  docs portal, AGENTS dependency/privacy guardrails, and SoraNet/SoraFS
		  auth guard, self-audits that configured scan roots/files still exist,
		  and now inventories tracked active-marker lines that explicitly mention
		  SoraFS so only the contract test's negative controls may retain those
		  literals. The tracked `todo_list.txt` inventory now has stale closed
		  SoraFS rows removed and a contract guard preventing active-marker rows
		  for SoraFS from reappearing. The in-repository localization mirrors
		  were removed by the documentation split; the stale rollout-gate
		  wording guard now scans canonical SoraFS plans under `specs/` before
		  any `Add fail-closed ... rollout evidence gate` wording can reappear,
		  while public and localized copies remain sibling `iroha-docs`
		  responsibilities, so
		  first-release unfinished-work drift cannot re-enter unnoticed. Shared SoraFS
		  evidence sensitivity checks also treat
	  API/auth/session/x-api/id/OAuth/refresh/JWT tokens, cookies, passwords,
	  private keys, seed phrases, and signing keys as runtime-only material
	  across payload scans, archive labels, runner URLs, passthrough arguments,
	  and checked-in response-file examples while keeping payload-free
	  proof-token and public-key fingerprint labels valid, and those archive
	  labels plus runner URL/rendered-path/passthrough arguments now inspect the
	  same bounded URL-percent plus HTML-entity decoded variants before accepting
	  path separators, drive/scheme prefixes, or secret-looking components; the
		  final aggregate production-readiness gate applies that same decoding policy
		  to summary artifact paths and dry-run summary input paths before promotion,
		  and checker preflight now applies the same path-safety policy to
		  checker-rendered summary/evidence directories and explicit
		  `--evidence kind=path` inputs before any path-specific diagnostic can
		  echo them; runner command-plan artifacts, reserved output paths, and
		  secret-looking non-Path artifact labels now pass through that same
		  decoded plan-rendered path gate before artifact diagnostics can echo
		  them, while path-component secret fragments remain narrower than
		  payload-key sensitivity so exact `request_body`/`response_body` labels
		  are rejected without treating longer diagnostic labels such as
		  `test_sensitive_response_body_f0` as credentials;
		  moderation-panel
  evidence-viewer rollout artifacts now pin those shared token-alias rules at
  the lane checker boundary with sanitized diagnostics for `idToken`, JWT,
  OAuth, refresh-token, and set-cookie fields, and the static rollout
  contract now proves common sensitivity coverage through the shared helper
  plus standard validator path instead of stale per-checker key duplication,
  while every rollout/release checker `SENSITIVE_KEYS` definition must stay a
  single canonical lower-snake-case set literal with no exact or
  punctuation-insensitive duplicate aliases, and payload key-name scans now
  also evaluate bounded URL-percent/HTML-entity decoded variants before deciding
  whether a field name or `*included` marker is sensitive, while diagnostic
  path segments only echo raw, undecoded ASCII alphanumeric-edged label-like
  keys and collapse encoded, whitespace-bearing, non-ASCII, separator, dotted,
  or bracketed spellings before non-sensitive inclusion-marker errors can echo
  them; public
  starting diagnostic paths must also be canonical raw path expressions before
  any payload scan can append path-qualified errors, and evidence labels must
  be canonical raw ASCII alphanumeric-edged phrases without encoded or
  high-risk sensitive fragments before diagnostics can include them.
	  Runner URL preflight
	  now also rejects encoded drive-prefix and URI-scheme-like host labels such
	  as encoded `C:` or `http:` components before they can enter collection
	  plans, and the AI pre-screen/gateway compliance canary builders plus
	  rollout evidence URL-field validators now pin the same host/path-token
	  rejection with no-leak adversarial cases. The AI pre-screen, reputation,
	  and transparency collection runners also pin the shared URL preflight at
	  their dry-run CLI boundary before rendering command plans or launching
	  verifiers. The release-pipeline operator plan now documents the
	  host/path-token URL and passthrough preflight. It now also requires
	  the shared checker summary writer, every
	  checked-in SoraFS canary atomic writer, the transparency deployment-context
	  artifact rewriter, the orchestrator fixture generator, and the Android
	  codegen replay writer, plus the shell-gate adoption, SDK parity,
	  gateway-compliance, reference-header, release-packager, and docs pin-release
	  writers to fsync output parent directories after descriptor writes or
	  atomic replacement, with sanitized negative controls for
	  parent-fsync failures before release evidence can be treated as durable.
	  Narrowed aggregate
  runs now also reject explicit summaries for unrequired lanes at both the
  checker and collection-runner layers, so failed or stale lane evidence cannot
  be hidden behind a smaller `--require-gate` selection. Narrowed per-lane
  rollout/release collection runs now also reject typed evidence supplied for
  excluded `--require-kind` entries before dry-run output or verifier
  execution, keeping the external evidence map aligned with the requested gate
  surface. The SFM-3 reputation
  collection runner now also validates its schema-closed collection-plan
  envelope, external evidence map, checker-backed evidence contract, and
  command steps before dry-run output or live collection.
  The repository-wide rollout-gate static contract now also requires every
  SoraFS rollout/release checker and collection runner to keep checked-in
  operator argfile examples, and requires every rollout/release plus aggregate
  production-readiness checker and runner to use the shared bounded response
  file expander and shared shell-like response-line parser for reviewed
  `@ARGFILE` inputs, with aggregate production-readiness checker and runner
  entrypoints retaining direct symlink-leaf and malformed-line negative tests,
  and malformed scalar, bytearray, mapping, non-string,
  empty, padded, or control-character argument tokens fail closed before
  character-wise expansion, rollout plus aggregate checker and
  collection-runner threshold, timeout, limit, and deterministic-clock
  arguments must use shared argparse integer parsers for positive and
  non-negative operator-supplied values; those parsers now require
  canonical ASCII decimal spellings and reject plus signs, whitespace,
  leading-zero, underscore, non-ASCII digit, and negative-zero coercions, with
  static coverage that pins the current runner positive/non-negative option
  classes and self-checks integer-parser inventories against source usage, and
  collection runners that expose narrowed `--require-kind` gates plus the
  aggregate production-readiness runner's narrowed `--require-gate` gate
  selection must use the shared required-kind parser with the checker's
  `KIND_BY_NAME` or aggregate `GATE_BY_NAME` and default set, while
  scalar/bytearray/mapping raw values, non-string entries,
  padded/control-character kind names, malformed allowed-kind registries, and
  malformed default required-kind sets fail closed before character-wise
  parsing, trim-normalized entries, or unchecked defaults can satisfy a narrowed
  gate, collection runners must derive `required_kinds` and the aggregate
  runner must derive `required_gates` inside `parse_args` before validation or
  dry-run plan construction, and the
  shared contract now builds each checked-in runner example's dry-run plan and
  verifies the emitted `evidence_contract` schemas and payload-field lists
  match the checker constants for the selected required kinds, while
  `external_evidence`, verifier `--evidence` arguments, and verifier
  `--require-kind` arguments must all match the parser-derived inputs, and
  generated-canary runners must keep every planned artifact under the reviewed
  output directory consumed by the verifier `--evidence-dir`, while every
  explicit verifier `--evidence` file argument must be visible in dry-run
  `external_evidence` for operator review, dry-run `external_evidence` values
  must be well-shaped, unique, distinct from every planned output, and backed
  by matching `evidence_contract` entries, typed verifier evidence specs such
  as `kind=path` must match the same dry-run external evidence key and path,
  and SFM-3 explicit evidence specs now reject recognized payload schemas that
  belong to a different evidence kind plus same-path explicit hints that name
  conflicting evidence kinds, and
  every rendered dry-run step
  artifact must match an output argument in the command that produces it, with
  every dry-run collection-plan schema and verifier summary schema pinned to
  the imported checker constants, and the rollout contract now also pins
  appeal finance, orderbook, PoR, repair, transparency, gateway-load,
  PoP credentials, moderation-panel, and reserve-rent fixture inventories
  against each checker's exported required route, metric, status, class,
  cache-state, payload-kind, probe, and lifecycle sets, and every SoraFS
  rollout/release checker test now names its checker-exported required,
  allowed, and bound-kind constants, while the static rollout contract now
  scans every checker and checked-in canary builder for exported inventory
  constants so checker or builder drift cannot hide behind hard-coded fixture
  literals, and it now renders every checked-in rollout/release collection
  runner plus the aggregate production-readiness runner to verify exported
  plan-field, threshold-field, external-evidence, and deployment-context
  inventories against the emitted dry-run JSON, while local helper tests import
  and enforce the shared sensitive-key, rollout environment/review-label, and
  path-template field inventories, and the aggregate production-readiness
  checker is now part of the exported-inventory scan so PoP root/revocation
  bound fingerprint maps and schema-closed aggregate output field contracts
  cannot drift from local tests, and the static contract now imports every
  checker evidence-kind registry plus residual builder/checker inventory
  constants so schema maps, default required kinds, required-field tables,
  evidence-viewer digest fields, enforcement-route defaults, payload-kind
  thresholds, and hedging fixture status/name inventories
  cannot drift from local tests; every checked-in SoraFS canary builder now also
  calls the shared reviewed deployment-id and rollout-environment validators
  before canary JSON writes or checker prevalidation, so dev/test/mock markers,
  `stg`/staging deployment-id aliases, and other unreviewed deployment aliases
  cannot be promoted by builder-generated evidence; the static rollout contract
  also executes every checked-in canary `@ARGFILE` example with redirected
  output to prove reviewed examples still build canonical sorted SoraFS v1 JSON
  artifacts with reviewed deployment context, recursively false payload, raw,
  debug, divergence, rollback, and disclosure flags while passing the
  shared sensitive-field visitor, then replays every example with a forged
  deployment id and forged environment to prove example-driven builder runs fail
  before writing artifacts, with noncanonical
  plus nonpositive `--generated-at-unix` overrides, and with invalid
  `--now-unix` overrides for builders that expose the option, plus noncanonical
  overrides for non-timestamp positive/non-negative integer options, negative
  overrides for shared non-negative integer options, and nonpositive overrides
  for non-timestamp positive integer options, to prove the shared integer parser
  rejects operator argfile input before writes; every dry-run
  `evidence_contract` schema must be a canonical SoraFS v1 identifier, every
  dry-run plan must use only reviewed top-level keys, any dry-run
  `deployment_context` must match the parsed deployment id, normalized
  environment, and reviewed marker, and rendered
  dry-run `steps` must exactly match the command plan built from the reviewed
  argfile, with the verifier
  gate step remaining the final dry-run step and its rendered artifact matching
  the sole verifier `--summary-out` target while the gate command invokes the
  parser-selected checker path, and every dry-run threshold value must match
  the verifier gate command option of the same name,
  every checked-in rollout collection `@ARGFILE` example now also executes as
  a negative dry-run in the static contract and must fail closed before
  emitting stdout when its runtime evidence is absent, with only stable
  `ERROR:`/bullet diagnostics and no traceback, raw exception-class, or
  secret-looking token leakage,
  every checked-in rollout/release checker `@ARGFILE` example plus the
  production-readiness checker example now executes against absent runtime
  evidence in the same static contract and must fail closed without stdout,
  tracebacks, raw exception-class leakage, or secret-looking diagnostics, and
  gateway-compliance plus reputation invalid summaries now emit shared
  fail-closed checker error blocks instead of silent nonzero exits,
  every checked-in SoraFS argfile example must expand through the shared
  bounded response parser, with argfile resolve, stat, read, UTF-8, parse,
  recursion, size, depth, and expansion-limit failures reported as stable
  operator diagnostics instead of tracebacks, argfile leaves and parent chains
  must be symlink-free before parsing, descriptor reads must use no-follow
  final-component flags where available, and argfile stat/read/UTF-8 plus
	  response-line parser exception text must route through the shared
	  path/error-label sanitizer so malformed multi-line diagnostics cannot leak
	  through reviewed `@ARGFILE` expansion, while direct and parser-returned
	  non-string argument values now collapse to a constant diagnostic so
	  secret-looking bytes or object representations cannot be echoed, and shared
	  sensitive-key normalization now folds Unicode compatibility forms before
	  punctuation-insensitive matching so fullwidth private-key or bearer-token
	  names are rejected and redacted by payload scans and duplicate-key JSON
	  loader diagnostics, while the shared secret-looking scalar scanner now
	  checks decoded NFKC value aliases so fullwidth bearer headers,
	  assignment-style secret names, and secret-bearing URLs cannot bypass
	  payload-free evidence scans; runner preflight path, URL, and passthrough
	  argument safety now applies the same decoded NFKC alias boundary so
	  fullwidth sensitive components, slash separators, and drive/scheme tokens
	  fail before entering dry-run plans or subprocess arguments, and archive
	  artifact path labels now apply decoded NFKC aliases before accepting
	  bundle-relative paths so fullwidth separators, drive/scheme markers, and
	  sensitive labels cannot enter release bundles through compatibility-form
	  spellings, and checker `--evidence` parsing now treats URL/HTML-encoded
	  or compatibility-form `=` separators as malformed `KIND=PATH` specs
	  instead of rendering attacker-controlled path diagnostics. AI pre-screen and transparency
  collection runners now require exact `--iroha-arg=VALUE` passthrough spelling
  in direct argv and checked-in argfile examples, so split `--iroha-arg VALUE`
  forms cannot consume runner flags or leak secret-looking passthrough values;
  every checked-in SoraFS canary builder
  that accepts required route, metric, claim, role, target, package, or other
  name-set options now rejects duplicate operator values before writing canary
	  evidence, so repeated comma-separated or repeated-flag inputs cannot silently
	  collapse into valid coverage, every checked-in canary builder now preserves
	  exact comma-separated components so padded or empty CSV items fail validation
	  instead of being trimmed or dropped, every checked-in canary builder's
	  canonical-string helper now reuses the shared diagnostic text predicate so
	  Unicode control/format text fails before canary JSON is built, and the
	  shared static contract now imports
	  every name-set canary builder to exercise duplicate, unknown, missing
	  required-value, and exact CSV diagnostics directly; the orderbook canary builder now also has
  focused direct regressions for duplicate and unknown verified-claim, route,
  stream, SDK language, metric, and reconciliation-source inputs before any
  canary JSON is written, and the gateway-load canary builder has direct
  duplicate/unknown scenario and telemetry-metric regressions before any canary
  JSON is written. The gateway-compliance canary builder now has direct
  duplicate/unknown verified-claim, feed, toggle, denial-reason, and metric
  regressions before any canary JSON is written. The reputation canary builder
  now has direct duplicate/unknown metric regressions and malformed or
  placeholder SSE/WebSocket transport-event regressions, plus exact provider
  proof sibling digest duplicate tracking that ignores malformed uppercase
  siblings instead of lowercasing them into the seen set, before any canary JSON
  is written. The governance-DAG canary builder now has direct duplicate/unknown
  verified-claim, payload-kind, dashboard-route, and metric regressions before
  any canary JSON is written, and
	  the AI prescreen canary builder now has direct duplicate/unknown operator-route,
	  transparency-source-kind, Governance DAG producer, and workflow-step
	  regressions plus malformed or placeholder committee-result and Governance
	  edge-label regressions, padded Governance-edge tuple component regressions,
	  trailing-slash runner, committee, and operator base URL regressions,
	  and Unicode control/format regressions for notification case/round ids plus
	  executor bundle directories
	  before any canary JSON is written. The moderation panel canary
  builder now has direct duplicate/unknown verified-claim, route, viewer role,
  viewer security-control, viewer event-kind, viewer export-target, scenario,
  outcome, publication-target, and metric regressions before any canary JSON is
  written. The standalone evidence-viewer canary builder now has direct
  duplicate/unknown verified-claim, role, security-control, access-event-kind,
  export-target, malformed viewer-session, and placeholder viewer-session
  regressions before any canary JSON is written. The PoP
  credentials canary builder now has direct duplicate/unknown
  verified-claim, route, and metric regressions plus explicit
  `--route-body-blake3-hex` evidence for enrollment/verifier routes before any
  canary JSON is written. The appeal finance canary builder now has direct duplicate/unknown
  verified-claim, class, route, urgency, outcome, instruction-step,
  reconciliation-status, payload-kind, and metric regressions before any canary
  JSON is written. The hedging/billing canary
  builder now has direct duplicate/unknown verified-claim, feed, route, source,
  and metric regressions before any canary JSON is written. The PoR canary
  builder now has direct duplicate/malformed provider and challenge inventory
  regressions plus duplicate/unknown runtime-route, reporting-route, and metric
  regressions plus explicit `--route-body-blake3-hex` evidence for
  runtime/reporting routes before any canary JSON is written. The PoTR canary builder now has direct
  duplicate/unknown tier, route, and metric regressions plus provider duplicate
  inventory regression before any canary JSON is written, and the PDP canary
  builder now has direct duplicate/unknown provider-route and metric regressions
  plus provider duplicate inventory regression before any canary JSON is written.
  The reserve/rent canary builder now has direct duplicate/unknown fixed-inventory
  regressions plus provider-bake cycle duplicate regressions before any canary
  JSON is written, and the repair canary builder now has direct duplicate/unknown
  failure-source, route, lifecycle-status, handoff-target, and metric regressions
  plus padded failure-event tuple component regressions
  plus explicit `--route-body-blake3-hex` evidence for signed repair routes
  before any canary JSON is written; a shared static contract also imports every
  checked-in SoraFS canary builder and verifies existing output directories plus
  symlinked output-parent chains are rejected before canary writes, and pins the
  temp-file, no-follow, shared complete-byte descriptor-write, descriptor-fsync,
  atomic-replace, and cleanup shape of each builder's JSON writer, including
  forced write-failure cleanup that leaves neither final artifacts nor temporary
  files behind. The direct-mode smoke wrapper now also rejects symlinked or
  non-regular payload, summary, adoption-report, and policy scoreboard outputs,
  plus symlinked output-parent components, before invoking `sorafs_cli` or
  adoption checks, with focused adversarial wrapper coverage for each output
  class. The gateway self-cert wrapper now applies the same fail-closed
  preflight to the attestation output directory and manifest verification
  summary before evidence writers run, with negative tests for symlinked
  outputs and parent aliases. The
  SoraFS CLI release signing wrapper now rejects symlinked or non-regular
  bundle, detached-signature, sign-summary, and verify-summary outputs, plus
  symlinked parent chains, before signing or verification runs. It also rejects
  missing or option-shaped wrapper option values and symlinked, non-regular,
  missing, or parent-aliased manifest, chunk-plan, chunk-summary,
  identity-token-file, and prebuilt CLI inputs before release artifacts are
  prepared, with focused adversarial wrapper coverage for parser failures,
  input aliases, bundle, signature, and summary outputs.
  The gateway probe telemetry wrapper now applies the same output preflight to
  probe artifact directories, probe logs, probe JSON reports, and PagerDuty
  payloads before `cargo xtask sorafs-gateway-probe` launches, with adversarial
  tests for symlinked artifact roots, parent aliases, explicit reports, and
  PagerDuty outputs, and it now validates drill `--date`, `--start`, and
  `--end` overrides before creating artifacts or launching cargo so malformed
  drill timestamps cannot be hidden behind later probe failures. Configured
  rollback hooks must also exist as regular, non-symlink executable files before
  probe artifacts or cargo execution begin, with adversarial coverage for
  missing hooks, non-executable files, symlinked hooks, and symlinked hook
  parents. PagerDuty custom-detail keys must be canonical, unique, and unable to
  override wrapper-owned evidence fields such as `status`, `probe_log`, `host`,
  or `manifest_cid`, with negative coverage for reserved, duplicate, and
  malformed keys. PagerDuty endpoint overrides must also be credential-free
  HTTPS URLs without fragments, whitespace, or control characters before probe
  artifacts or cargo execution can start, with negative coverage for non-HTTPS,
  credentialed, and whitespace-bearing URLs. The drill-log writer and validator now also use
  descriptor-based no-follow append/read paths, reject symlinked drill-log
  leaves and parent aliases, reject non-regular existing log targets, and reject
  parent-directory segments before operator drill evidence is written or parsed,
  require the canonical Markdown separator row, require exact lowercase status
  values, and require real calendar dates plus range-checked UTC start/end times
  rather than only regex-shaped drill timestamps. Gateway probe wrapper options
  now also reject missing or option-shaped values before artifacts are prepared,
  while `--rollback-hook-arg` preserves flag-like hook arguments, with
  no-unbound-variable and option-swallowing regressions. The SoraDNS IR drill scheduler
  now derives automatic drill dates from the current UTC date and validates
  custom `--date`/`--start` overrides before touching the drill log, with
  no-traceback adversarial coverage for malformed operator inputs.
  Every checked-in SoraFS canary builder must also use the shared
  bounded `@ARGFILE` parser path and shared strict positive/non-negative integer
  parsers for numeric CLI options, every checked-in SoraFS canary builder test
  must retain response-file coverage, and the static rollout contract now
  requires every checked-in SoraFS canary builder test to exercise both symlink
  and directory `--out` targets through the builder `main(...)` path so parser
  and path-adversarial coverage cannot regress; rollout checkers now also reject duplicate
  metric rows in externally collected observability, telemetry/SLO, dashboard,
  metrics/alert, and metrics evidence before coverage can report ready, require
  `metric_count` to be present and bound to the unique canonical `metrics`
  inventory for those metric-bearing rollout artifacts, and
  reject duplicate scalar coverage rows for rollout tiers, denial reasons,
  payload kinds, repair sources/statuses/handoffs, moderation viewer roles/
  controls/events/exports, moderation scenarios, decision outcomes, and
  publication targets; checker evidence discovery now removes
  duplicate or aliased evidence identities from the returned parse list after
  recording the collision, so ambiguous files cannot still appear as parsed
  rollout artifacts beside their load errors, and duplicate or aliased reserved
  output identities now fail before evidence paths are scanned for output
  conflicts, while operator-supplied evidence directories must be inspectable
  non-symlink directories before scans and explicit plus directory-discovered
  JSON evidence candidates must exist as inspectable regular non-symlink files
  before they can enter the parse list or reserved-output conflict preflight;
  every per-lane, release, and aggregate production-readiness collection runner
  must keep a collection-named reviewed argfile example, and every runner
  example must parse through its own runner parser when loaded as the reviewed
  `@ARGFILE` plus `--dry-run`; every per-lane, release, and aggregate runner
  `main` function must convert argparse `SystemExit` failures into numeric
  exit codes for tests and operator wrappers, and caught argument parser
  `ValueError` diagnostics must route through the shared sanitized runner
  exception reporter, runner examples must include a `--dry-run` review
  command, runner collection-plan validation must reject
  non-object renderings, command-plan drift, and strict JSON rendering failures
  through shared preflight before dry-run output or execution, with aggregate
  production-readiness using the shared aggregate plan validator, runner dry-run
  plan rendering must reject non-object plan shapes before writing stdout and
  sanitize caught JSON render exceptions before returning plan diagnostics,
  every per-lane, release, and aggregate production-readiness runner must
  preflight its verifier as a non-symlink file under symlink-free parent chains
  and preflight summary/output targets plus runner input files/directories
  through shared helpers before emitting dry-run plans, including precise
  missing file versus directory diagnostics, existing non-symlink input files
  and directories under symlink-free parent chains, existing file and directory
  ancestors for output paths, symlink output directories, and symlink-free
  output parent chains for
  both missing and pre-existing output targets, summary-output and
  output-directory identity collisions, duplicate or aliased input paths across
  all runner input flags, local directory inputs, malformed input path
  containers or duplicate-identity maps, resolver failures, filesystem
  inspection failures, and malformed preflight diagnostic containers, existing
  diagnostic text, or labels failing before filesystem inspection; runner
	  stderr error emitters must reject malformed diagnostic containers and
	  noncanonical diagnostic text before printing partial headings or
	  character-split errors, and runner stderr notices must reject malformed or
	  multi-line messages before writing partial operator output; runner URL and
	  passthrough argument preflights now use the shared diagnostic-text predicate
	  before URL parsing or passthrough splitting, and command-plan executable
	  validation uses the same predicate before subprocess plans can report ready;
	  runner and
	  checker collected validation errors from caught malformed spec parsers must
  route through the shared error diagnostic sanitizer before entering stderr or
  rollout summaries, so raw multi-line exception text cannot be appended to
  gate diagnostics, and transparency runner generated-artifact annotation
  read/write failures must sanitize path and exception labels before returning
  collected diagnostics while rewriting reviewed deployment context through
  descriptor no-follow opens, complete byte-write loops, descriptor fsync, and
  output parent-directory fsync after a fresh parent-chain check;
  every per-lane, release, and aggregate production-readiness runner must execute
  plans through the shared command-plan runner so malformed scalar or mapping
  command plans, malformed step labels, non-Path step artifacts, empty command
  lists, empty or non-canonical command executables, embedded-NUL or
  control-character command entries, and non-string command vectors are
  rejected before output-directory creation, and duplicate
  planned artifacts, malformed, symlinked, symlink-parented, or
  duplicate/aliased reserved-output path containers or entries failing before
  planned-artifact inspection, planned
  artifact/output-directory identity collisions, output-directory creation
  failures, post-command output-directory swaps/removals, subprocess launch
  failures, symlink planned artifacts, symlink planned-artifact parent chains,
  pre-existing
  planned artifacts, missing expected artifacts, post-command symlink
  expected-artifact parent chains, zero-byte expected artifacts, and
  expected-artifact inspection failures surface as structured errors
  instead of tracebacks, with subprocess launch exception text routed through
  the shared diagnostic sanitizer before stderr output so raw multi-line OS
  messages cannot leak through command-plan execution; every rollout/release
  checker plus the aggregate production-readiness checker must preflight and
  write its optional summary output through the shared helper before/after
  evidence validation so output-parent creation and summary-write failures,
  summary-output target/parent inspection failures, summary-output symlinks
  plus symlinked or non-directory parent chains, and summary/evidence identity
  collisions surface as structured errors instead of tracebacks, with the
  aggregate production-readiness checker retaining direct entrypoint negatives
  for existing-directory and symlinked `--summary-out` leaves, symlinked or
  non-directory `--summary-out` parent chains, unsafe summary-output path
  components, and explicit evidence/output identity collisions, checker
  summaries must be JSON objects before rendering and summary text must be a
  string before optional output writes, and checker summary render exceptions
  must be sanitized before entering collected diagnostics, malformed
  preflight diagnostic containers, existing diagnostic text, and labels fail
  closed before filesystem inspection, and checker stderr error emitters reject
  malformed diagnostic containers before printing partial headings or
  character-split errors,
	  shared evidence-validation artifact builders, validation-error recorders,
	  gate-status checks, required-row artifact error collectors, and path labels
		  plus release-archive artifact labels now reuse the canonical diagnostic-text
		  predicate so zero-width, bidi, and other Unicode control/format text fails
		  closed before summary or artifact errors can be rendered or archived,
		  artifact kind names, explicit fingerprint value keys, and shared canonical
		  payload string fields now delegate to the same validation-label predicate
		  instead of helper-local strip-only prechecks,
		  shared artifact fingerprint field names and generated hedging fixture
		  inventory labels now use the same diagnostic-text predicate before any
		  fingerprint lookup or safe-path rendering, so empty, padded, zero-width, or
		  bidi labels collapse to stable no-echo diagnostics instead of local
				  strip-only checks, rollout checker, canary-builder, and final
				  aggregate production-readiness metadata non-production marker
				  checks now share the same numbered- and
					  compact-alias helper, so values such as `placeholder1`, `mock01`,
					  `stub2`, `placeholderreview`, `prodstub`, `devproduction`, or
					  sandwiched aliases like `prodplaceholderreview` fail wherever the
					  bare marker would fail instead of bypassing local exact-token scans,
		  checker stderr notices reject malformed or multi-line messages before partial
	  operator output, rollout/release plus aggregate checker evidence input
  preflight rejects malformed
  `--evidence`/`--evidence-dir` containers, non-Path `--evidence-dir` entries,
  and non-Path/non-spec `--evidence` entries before they can satisfy the
  evidence-source requirement or trigger summary-output inspection, with direct
  aggregate checker negatives for unsafe rendered `--evidence-dir` and
  explicit `--evidence` path components, and direct reputation checker coverage
  for unsafe rendered or empty `KIND=PATH` evidence specs before load-time kind
  parsing plus unknown kind-prefixed specs and malformed spec parser exceptions
  rendered into failed summaries without echoing supplied kind text or raw
  exception text, and untyped explicit evidence with no inferred kind reporting
  path-free failed-summary diagnostics, while stdout-mode unknown-schema,
  no-inferred-kind, schema-mismatch, and conflicting explicit-kind load errors
  remain schema/kind/path-free, stdout-mode required-provider diagnostics
  remain provider-id-free, and invalid provider/proof plus verify evidence
  provider IDs, hedging billing-cycle IDs, reserve-rent bake IDs, and PoR
  archive backend labels plus reference SDK signature algorithms are omitted
  from fingerprints and stdout summaries after canonical ID or closed-set
	  validation, while aggregate production-readiness `provider_ids`,
	  `valid_billing_cycles[].cycle_id`, and `valid_provider_bakes[].bake_id`
	  metadata now replay the SFM-3/SFM-5/SFM-6 canonical label and shared
	  compact-alias non-production marker policies, final production
	  deployment-id validation applies the same compact-alias policy to
	  stage/staging markers such as `stagingready`, and final-production
	  validation now rechecks
  payload-free `deployment_context`, object-list deployment metadata, and
  artifact fingerprint deployment context against the reviewed production
  deployment policy, while artifact fingerprint shape/freshness failures now
  use single-source sanitized diagnostics, name
  `fingerprint.generated_at_unix`, and reject sensitive fingerprint keys without
  echoing attacker-controlled key names or values, and shared evidence JSON
  duplicate-key load failures now collapse every current SoraFS checker
  `SENSITIVE_KEYS` entry before aggregate diagnostics can record those names,
  while the SoraFS orchestrator fixture generator and Android codegen replay
  fixture reader reuse the same strict duplicate-key/non-standard-number parser
  so encoded sensitive-key shadowing cannot enter SDK parity vectors,
  and both fixture utilities now reject secret-looking, control-character,
  parent/current, drive-prefix, or platform-specific path components before
  diagnostics can render those paths,
  and Android codegen replay now requires fixture metadata `payload_path` and
  `plan_file` entries to be safe relative paths before resolving files or
  launching manifest replay, with metadata `fixture` names constrained to safe
  single-component filenames before report paths are constructed,
  all Android fixture display file labels normalized as safe relative POSIX
  paths before generated examples can preserve them,
  and Android replay validates profile handles, storage classes, and numeric
  subprocess fields before building `sorafs_manifest_builder` arguments,
  while the hedging fixture-manifest checker rejects unsafe rendered
  `--manifest` paths before loading manifests or writing summaries,
  and hedging fixture entry names/paths reject secret-looking or unsafe
  components before skipped-entry summaries, generated-byte reads, or
  validation-command checks can echo them,
  and generated hedging JSON sidecar diagnostics sanitize unexpected
  top-level/nested field names and duplicate nested line IDs before summaries
  can echo attacker-controlled values,
  and hedging validator-command token drift now reports only a constant
  diagnostic before validator execution,
  while unmanifested generated hedging fixture inventory reports keep safe
  paths visible but collapse unsafe or secret-looking paths before summaries,
  and hedging checker filesystem inspection labels now collapse canonical but
  unsafe path components before symlink/read diagnostics,
  and missing hedging validator binaries now route supplied binary labels
  through the same unsafe-path sanitizer,
  and Android fixture replay read failures now sanitize descriptor-open
  filesystem paths and exception text before reporting loader failures,
  and orchestrator fixture descriptor read, size, and write failures now use
  sanitized path/error labels before fixture-generation diagnostics,
  and Android/orchestrator fixture directory creation failures now sanitize
  `mkdir` path/error diagnostics,
  and every checked-in SoraFS canary builder atomic writer now sanitizes
  output-parent creation and descriptor write exception details before
  reporting generated-canary artifact failures,
  before promotion can accept
  fingerprint-consistent forged summaries,
  including explicit evidence, evidence discovered through
  `--evidence-dir`, and reputation's
  kind-prefixed explicit evidence syntax, and the hedging fixture-manifest checker
  must use the same shared summary-output preflight before manifest reads,
  every rollout/release checker must reject empty, duplicate, or unknown
  narrowed `--require-kind` entries, and the aggregate production-readiness
  checker must reject duplicate, unknown, or malformed narrowed
  `--require-gate` entries, through direct shared-parser calls before evidence
  validation, every rollout/release checker plus the aggregate
  production-readiness checker must use shared evidence-file discovery
  that rejects reserved output-path reuse, duplicate explicit evidence paths,
  overlapping `--evidence-dir` scans, files provided by both `--evidence`
  and `--evidence-dir`, malformed explicit-evidence identity sets, explicit
  identity sets derived from uninspected or already-rejected evidence paths,
  and explicit membership checks that try to resolve uninspected candidate
  files, with direct aggregate checker negatives proving explicit evidence-file
  and evidence-directory symlinks plus summary-output files rediscovered through
  evidence-directory scans failing before validation, and duplicate explicit
  evidence and explicit/directory evidence overlaps plus overlapping
  evidence-directory scans become blocked summaries without leaking symlink,
  target, directory, or evidence file path text,
  while
  malformed scalar or mapping evidence/reserved-output path collections or
  symlinked, malformed, or duplicate reserved-output entries
  failing before evidence inspection, non-directory evidence
  parent chains, diagnostic containers, labels, sanitized evidence directory
  and conflict path/error labels, evidence directory inspection failures, direct
  JSON scans over uninspected or non-directory paths, and JSON scan failures
  surface as structured errors instead of tracebacks, and every
  checker must use shared path-identity
  helpers instead of raw `Path.resolve()` calls so resolver failures surface as
  structured gate errors with sanitized malformed path/error labels, and
  checker preflight filesystem inspectors now reuse the same sanitized
  path/error labels for malformed non-path inputs and noncanonical inspection
  failures before summary-output or evidence-source validation can leak raw
  path text, checker preflight non-inspection diagnostics for evidence inputs,
  summary/evidence collisions, summary parent creation, and summary writes now
  use the same sanitized path/error labels and descriptor no-follow output
  opens, collection-runner preflight
  filesystem inspectors now apply the
  same shared sanitized labels before verifier, input, output-directory,
  summary-output, and planned-artifact validation can leak raw malformed
  path/error diagnostics, runner artifact-size inspection now rejects symlink
  leaves and symlinked parent chains before measuring size through a no-follow
  descriptor `fstat`, and the
  remaining non-inspection runner diagnostics
  for missing inputs, duplicate identities, malformed reserved outputs,
  command-plan artifacts, and output-directory creation now use the same
  sanitized path/error labels, and the hedging fixture-manifest checker now
  applies the same sanitized path/error labels to malformed summary targets,
  manifest inspection/read failures, generated fixture byte reads, generated
  sidecar misses, manifest/generated sidecar bounded JSON decode failures, and
  generated fixture root scan failures, while rejecting symlinked generated
  fixture roots, symlinked generated fixture root parents, and symlinked
  generated inventory entries before they can be trusted as fixture evidence,
  shared evidence discovery and bounded JSON loading now delegate all path/error
  diagnostic labels to the same path-identity helper instead of carrying local
  sanitizer copies,
  malformed path-identity diagnostic containers, existing diagnostic text,
  labels, or failure templates, including unknown formatter fields, malformed
  formatter syntax, padded templates, and control-character templates, fail
  closed before filesystem identity checks can traceback, and failure-template
  validation uses a typed internal error branch instead of parsing exception
  message prefixes,
  every checker must load and digest
  bounded JSON evidence through the shared object-only loader so artifact hashes
  bind to the same bytes that were parsed before required-kind validity is
  reported, and filesystem, runtime, UTF-8, JSON, size, and object-shape
  failures become path-qualified evidence errors instead of tracebacks while
	  malformed bounded-JSON diagnostic containers or existing diagnostic text are
	  rejected before helper-local error recording can raise, direct bounded reads
	  inspect evidence files for symlink leaves, non-directory parent chains, and
	  non-files before opening them, and use a no-follow descriptor open for the
	  final path component where the platform exposes it, bounded byte-reader
	  oversize failures use a typed `ValueError` subclass so checkers do not parse
	  exception text to identify file-size limits, path/error fragments plus
  duplicate-key diagnostics use sanitized canonical labels for malformed values,
  and malformed summary-error sinks, existing summary-error text,
  validation path labels, or blank/control-character validation messages for
  shared evidence validation recording fail closed before path-qualified errors
  can partially append; the
  hedging fixture-manifest checker must also use the shared bounded object
  loader for manifest and generated JSON sidecar parsing and the shared bounded
  byte reader for generated Norito fixture bytes, every
  fingerprint-emitting checker must use the shared
  selected-field fingerprint helper with explicit local field tuples so
  summaries stay payload-free and cross-artifact binding fields remain pinned;
  fingerprint field tuples must contain only canonical, non-empty, duplicate-free
  string field names and reject scalar or bytearray field containers before
  iteration, so padded, control-character, byte-wise, or repeated fields cannot
  drift summary shape, and the static rollout-gate contract now rejects
  ambiguous base fingerprint field names unless they are reviewed deployment,
  schema, metric, count, or timestamp fields or advertise typed digest/hash/root,
  id, hex, timestamp, or count suffixes; AI notification transport now uses
  `manifest_body_blake3_hex`, reserve/rent scheduler canaries now use
  `scheduled_lifecycle_canary_last_tick_at_unix`, and the Rust CLI notification
  canary emits the same typed manifest digest key,
  and the shared sensitive-field scanner rejects malformed diagnostic sinks,
  starting path labels, evidence labels, mapping/scalar sensitive-key
  containers, and padded/control-character sensitive-key names before payload
  scanning can partially append or traceback, while shared
  artifact-error mirroring rejects malformed summary-error sinks, artifact error
  text, summary-error text, and artifact path labels before mutating artifact
  rows,
  basic checker object/string/positive-integer field validation now uses shared
  helper primitives across all rollout/release gates, and shared object,
  object-array, basic string, schema string-type, positive-int, string-equality,
  bool-true, non-negative-int, and count-equality helper labels reject malformed
  diagnostic text before payload lookup or object-item traversal, with basic
  string fields now requiring canonical non-empty payload values instead of
  trimming padded/control-character evidence into downstream checks, including AI
  pre-screen execution summary object validation,
  string-equality, including reputation canary schema exactness with
  context-specific diagnostics and AI pre-screen indexed route-schema
  exactness, bool-true, path-qualified indexed bool-true, transparency
  publication publisher-identity policy flags, non-negative-int,
  count-equality, including hedging reconciliation line-item parity, and
  moderation evidence-viewer logged-session parity, string-exclusion validation for the
  PoP privacy proof backend, and string-value equality validation for
  reputation provider proof identity now use shared helpers with canonical
  field, path, expected-value, disallowed-value, comparison-label, and
  allowed-value labels in every gate that needs them,
  false/false-or-absent/false-or-governed and non-negative-number validation
  now use shared helper primitives with canonical helper-label guards before
  payload lookup across the gates that enforce payload redaction,
  governance-gated hedge execution, and latency/lag ceilings,
  optional false fields must be exact `false` when present, including
  path-qualified route latency checks and maximum-number ceiling diagnostics,
  status-set helper labels reject malformed field/path text before status
  lookup or allowed-status diagnostics, optional hex, exact hex, and
  hex-string-array checks now use shared helper primitives with canonical
  field/path/count-label guards plus non-bool positive hex lengths,
  non-negative expected array lengths, and boolean required/unique option flags
  for the gates that need those narrower evidence contracts, sum-count
  zero-total bypasses and string-coverage scalar/trim controls now require
  exact boolean helper options, standard wrapper deployment-context enforcement
  now rejects malformed helper options, and the shared static contract requires
  every checker to keep canonical duplicate-free `EVIDENCE_REQUIRED_FIELDS`
  dry-run contracts while every reviewed-context checker must disclose
  `deployment_id`, `environment`, and `deployment_context_reviewed` in each
  default-kind contract, snapshot-bound artifact recording now
  rejects malformed valid flags and malformed kind containers before
  anchor/bound routing, string diagnostic quote switches now require exact
  booleans, default disallowed-string diagnostics no longer hit an undefined
  helper label, and score-bps helper labels reject malformed diagnostic text
  before payload lookup,
  inclusive integer-range validation now uses a shared helper with canonical
  field/path label guards and non-bool integer range thresholds for reputation
  basis-point fields while preserving the existing operator diagnostics,
  reputation event cursor advancement now uses a shared integer-pair helper
  with canonical current/next field labels,
  array count/length validation now uses shared helpers with canonical
  count/collection labels, non-bool non-negative count/length inputs, non-bool
  integer count and expected-count equality checks, and malformed
  collection-container rejection for route, probe, artifact, and reputation
  event arrays,
  count-sum validation now uses a shared helper with canonical part/total
  labels plus non-bool, non-negative total and part-count checks for appeal and
  proof probe accounting,
  zero-count validation now uses a shared helper for fail-closed mismatch,
  stale, missing-block, and unexpected-failure counters,
  minimum integer and computed minimum/maximum threshold validation now uses
  shared helpers with canonical field, computed-threshold label, and custom
  threshold-message guards plus non-bool integer computed-value and threshold
  inputs for rollout
  count gates plus governance DAG block/payload counts, moderation panel and
  peer counts, moderation sortition quorum ceilings, reserve bake timestamp
  ordering, reserve policy dimensions, reference SDK target/package counts,
  orderbook reconciliation peers, hedging bridge ABI floors, and computed
  canary coverage floors,
  maximum numeric and integer threshold validation now uses shared helpers with
  canonical payload field/path labels plus finite non-bool numeric and integer
  limit guards for route-indexed latency ceilings, single-field rollout latency
  and lag ceilings,
  governance DAG route latency plus pin/head age integer-unit ceilings,
  reputation metrics snapshot age and ingest lag,
  hedging feed/divergence, appeal route response body digests plus route/settlement integer-unit ceilings,
  reserve route response body digests plus lifecycle/route latency integer-unit ceilings, hedging route response
  body digests, PoR route response body digests plus route/scheduler/reporting integer-unit ceilings, repair
  route response body digests plus route/event/repair integer-unit ceilings,
  orderbook route response body digests plus matcher/stream lag, PoP route
  response body digests plus verifier service
  ceilings, moderation evidence-viewer
  URL TTL plus moderation route response body digests and route/event-lag integer-unit ceilings, and reference
  SDK smoke integer-unit ceilings, with timestamp freshness helpers
  also requiring non-bool non-negative current-time and max-age thresholds,
  passed-status validation now uses a shared helper in every rollout/release
  gate that requires a literal passed state, including reputation canary
  artifacts with their context-specific status labels,
	  mixed status-set validation now uses a shared helper for AI pre-screen
	  verified/passed evidence and reputation publish/latest
	  accepted/published/ready/ok snapshot states, rejecting malformed
	  optional-status flags, allowed-status containers, noncanonical
	  allowed-status labels, and
	  noncanonical observed status values before substring or mapping-key
	  membership can satisfy status gates,
	  shared string enum validation now rejects malformed allowed-value
	  containers plus noncanonical payload values before substring,
	  character-wise, trimmed-value, or mapping-key membership can satisfy
	  reviewed route-state checks,
	  schema string type validation now uses a shared helper in the same rollout
	  gates while preserving their artifact-specific unknown-schema diagnostics
	  for canonical unknown schemas and rejecting blank, padded, or
	  control-character schema values before unknown-schema lookup,
  schema recognition now uses a shared helper in standard rollout/release gates
  while preserving artifact-specific unknown-schema labels,
	  environment-bearing rollout/release gates now use the shared reviewed
	  environment validator so only exact reviewed labels are accepted; uppercase
	  or mixed-case aliases are not normalized, and padded/control-character
	  values plus `dev`, `test`, `mock`, `local`, and similar unreviewed labels
	  cannot satisfy production evidence,
	  deployment-id-bearing rollout/release gates now use the shared reviewed
	  deployment-id validator so missing, malformed, placeholder, compact
	  handoff-marker, compact non-production marker, padded/control-character
	  value, or otherwise non-reviewed ids cannot anchor artifact
	  fingerprints, and the shared required-evidence summary now rejects mixed
	  reviewed `deployment_id`/`environment` contexts across the same
	  rollout/release bundle while invalidating the required-kind matrix with a
	  row-level deployment-context diagnostic, marking the mismatched artifact
	  invalid, and keeping stable `errors: []` row buckets, and deployment-context
	  summary emission now routes through a shared canonical-label helper so
	  malformed containers or values cannot leak into summary JSON,
  `iroha_config` binding checks now flow through a shared helper across the
  rollout gates that require config-backed production behavior so environment
  or ad-hoc config sources cannot re-enter evidence validation locally,
  governance approval acceptance and vote-recording checks now flow through a
  shared helper across rollout and release gates so promotion evidence proves
  the same accepted, recorded governance decision everywhere,
  config-backed governance approval gates now compose that governance helper
  with shared `iroha_config` binding validation so accepted rollout decisions
  cannot drift away from config-backed production behavior,
  required policy digest binding now uses a shared helper across rollout and
  release gates, including governance approvals, AI pre-screen governance-DAG
  evidence, and reserve-rent policy/matrix anchors, so accepted decisions and
  policy-bound artifacts prove the same `policy_digest_hex` contract,
  policy-bound fixture tables and all-bound adversarial mismatch loops now
  cover appeal-finance, orderbook, PoR, governance-DAG, hedging/billing,
  gateway-load, and gateway-compliance rollout evidence, with the static
  rollout contract pinning each table to its checker-exported
  `POLICY_BOUND_KINDS` set,
  moderation-panel, PoP credential, PoTR, repair, and transparency rollout
  evidence now also pin each bound-fixture table to an exported lane checker
  kind set, and aggregate readiness imports the PoTR PQ/reputation,
  moderation-panel policy, repair handoff, and reserve-rent matrix bound-kind
  tuples from their lane checkers instead of duplicating them,
  route and probe HTTP status validation now uses a shared 2xx-status helper
  with canonical field/path label guards across every rollout gate that checks
  deployed endpoints,
  route/probe plus artifact/stream/event non-empty object-array validation now
  uses a shared helper with canonical field labels while each gate keeps its
  local endpoint and artifact policy checks, and malformed item rows now make
  the helper return no indexed records so placeholder objects cannot feed
  downstream count or route policy checks,
  timestamp freshness validation now uses a shared helper with explicit
  per-call freshness windows and canonical optional path-qualified diagnostics
  for reputation publish/latest snapshot checks,
  hex digest validation now uses shared exact-length helpers that accept only
  exact lowercase hex and reject padded, uppercase, or control-character values
  before downstream binding across every rollout/release checker that binds
  digest fields,
  hex-string array validation now uses a shared helper for proof sibling and
  statement digest lists with optional count and uniqueness checks, and dirty
  arrays with malformed, uppercase, duplicate, or length-mismatched rows now
  return no exact values for downstream binding,
	  checker string coverage requirements, including AI pre-screen operator route
	  names, now flow through a shared validation helper with canonical array,
	  item, observed-value, and required-value labels, with the transparency
	  checker explicitly pinned to its stricter dict-only exact-value mode, and
	  malformed observed values, required-value containers, or required-value
	  labels now fail closed before trimmed-value, character-wise, or mapping-key
	  coverage checks can satisfy required labels; the coverage helper no longer
	  exposes a trim mode, and the legacy string-value collector is exact by
	  default and only trims when a caller explicitly opts in, so helper reuse
	  cannot silently normalize padded evidence values,
	  cross-artifact summary invalidation now uses shared artifact-error recording
	  instead of checker-local nested helpers across every checker, including
	  reputation snapshot-bound errors with a separate required-kind summary
	  message, snapshot-bound anchor/bound classification now normalizes kind
	  containers before scalar or mapping-key membership can classify artifacts,
	  scalar and tuple binding helpers now canonicalize diagnostic messages,
	  formatter templates, missing-anchor summary errors, evidence-kind labels,
	  digest field selectors, per-kind digest field maps, and tuple binding
	  field lists before artifact errors can be recorded; direct scalar/tuple
	  membership then compares canonical observed binding values with canonical
	  allowed values exactly, so case-only drift is rejected instead of
	  lowercased into the same binding, and shared anchor-producing digest,
	  tuple, and snapshot-bound collectors plus checker-local `valid_*` anchor
	  sets now preserve exact canonical values instead of lowercasing anchors
	  before membership, so source and downstream evidence must agree byte-for-byte
	  on case as well as content, and artifact-error summary labels now use a
	  shared path-label helper
  and shared artifact-error recording rejects non-object artifact rows, so
  malformed artifact rows report `<unknown>` instead of raising on direct
  path indexing or mutation, while the shared artifact accessors and
  evidence-count helpers now fail closed on non-object rows, noncanonical
  artifact kind/detail-field/schema labels, malformed summary buckets, and
  scalar string/byte containers before validity, kind, fingerprint, detail,
  schema, or count lookups can traceback or report bogus character counts; shared
  validation-error recording now also rejects malformed error containers before
  strings can be split into per-character summary errors, and gate-status
  selection treats malformed error containers as `blocked` instead of `ready`;
  standard payload validation now rejects malformed payload objects, schema
  registries, and schema-kind names before shared rollout/release wrappers can
  raise or publish an unusable kind,
  standard status, hex, optional-hex, boolean/negative, numeric/range, count,
  object-array, string-equality, hex-array, config/governance, and
  string-coverage validators now reject malformed payload containers before
  direct field lookups can raise, integer-range validators canonicalize
  custom range diagnostics before emitting them, and timestamp validators fail
  closed before returning observed timestamps when diagnostic paths are malformed,
  standard artifact builders now normalize malformed validation-error buckets
  before artifact rows can publish scalar strings, non-string entries, or
  missing error lists, reject blank, padded, or control-character validation
  messages before malformed diagnostics can leak into summary rows, reject
  malformed payloads, fingerprint-field lists, and explicit fingerprint-value
  maps before artifact construction can traceback,
  validate explicit fingerprint override keys before merging any override
  values, require canonical artifact paths, canonical lowercase SHA-256
  artifact digests, and canonical non-empty kinded row names before rows can
  be marked valid, and standard
  artifact recording now rejects malformed bucket maps, noncanonical kind
  names, and artifact rows before appending to recognized evidence buckets,
  required-summary validity now also fails closed on malformed summary
  containers or rows instead of raising before reputation summaries can report,
  required-summary row invalidation now canonicalizes required-row kind names,
  rejects unhashable direct kind labels, moves malformed row keys to a safe
  `<unknown>` bucket, and validates summary errors before row mutation,
  required-summary kind-list construction now rejects malformed scalar,
  mapping, empty, duplicate, non-string, padded, or control-character
  required-kind containers before summary rows or metadata can be built from
  characters, object keys, noncanonical labels, or ambiguous duplicate rows,
  standard required-kind summary names reuse the same canonical non-empty/unique
  normalization before publishing `required_kinds`, standard schema-map
  extraction now rejects malformed kind registries before noncanonical names or
  blank/non-string/noncanonical schema metadata can seed summary rows, standard
  artifact-bucket initialization now reuses the same canonical non-empty/unique
  kind normalization before buckets can be materialized from characters,
  mapping keys, noncanonical names, or duplicate names, required-summary schema
  metadata now fails closed per row when a required kind is missing from the
  schema map or maps to a blank, padded/control-character, or non-string schema,
  and malformed artifact/schema metadata maps fail closed before summary
  construction can traceback, required-summary
  artifact buckets now reject scalar or mapping containers before character or
  object-key counts can satisfy required-evidence presence and record row-local
  malformed-artifact-bucket diagnostics, and required-kind predicates now reject
  malformed scalar, mapping, blank, padded/control-character, or non-string kind
  containers before character-wise or mapping-key membership can satisfy binding
  gates; shared
  evidence-value collection helpers now also reject scalar strings/bytes and
  mapping containers before value membership can split into characters or treat
  object keys as observed evidence; cross-artifact distinctness checks now
  reject malformed scalar or mapping containers instead of treating empty or
  single-character payloads as consistent, and cross-artifact value recording now
  reports non-string evidence values instead of silently dropping them,
  AI pre-screen runner, SFM-5 billing-cycle reconciliation, moderation
  roster/tally, reputation snapshot, and reserve-rent policy/matrix/ledger
  binding checks now use a shared exact tuple helper before artifact-error
  recording so local tuple membership predicates cannot drift,
	  scalar cross-artifact digest/id binding checks now use a shared exact value
	  helper and artifact-error recorder across the rollout/release gates that
	  anchor downstream evidence to source, config, policy, manifest, proof,
	  receipt, roster, or workflow artifacts; shared scalar and tuple binding
	  helpers now reject empty string components plus malformed value and
	  allowed-set containers and require validated allowed values to match
	  observed canonical values exactly before substring, character-wise, or
	  mapping-key membership can satisfy downstream bindings, and valid artifact
	  digest collection preserves exact canonical digest text while ignoring
	  empty, non-string, padded, or control-character fingerprint values so
	  malformed or case-drifted digest anchors cannot satisfy downstream
	  references, while bound-reference helpers now normalize `(kind, artifact)`
	  pair containers plus fingerprint field selectors before scalar or
	  mapping-key iteration can classify downstream references; the shared string
	  equality and inequality helpers now reject direct blank, non-string,
	  padded, or control-character values plus malformed comparison
	  labels/messages before returning provider/proof identity values,
  reputation optional provider ID/count observation, required/observed
  matching, and fallback presence checks now use a shared truthy non-bool
  hashable evidence-value normalizer with canonical string-value, required-row
  kind, and evidence-message guards so malformed observed values cannot
  traceback or drift behind local set-add/truthiness guards, distinct-value
  consistency now rejects malformed one-item collections before provider-count
  mismatch reporting can be skipped, and scalar cross-artifact consistency
  recording now only stores canonical non-empty string values with canonical
  context/key labels so malformed snapshot id/root values cannot become
  canonical state or leak through mismatch diagnostics,
  reputation custom artifact row construction now uses a shared kinded
  artifact builder so path/SHA/fingerprint/valid/error fields cannot drift,
  bounded JSON parse-and-digest coverage now includes every rollout/release
  checker, standard checker load failures use the shared path-qualified
  error recorder instead of local try/except append blocks, and
  reputation's custom loader also uses that shared path-qualified error
  recorder for parse and missing-file failures while preserving kind inference,
  reputation summary fingerprints
  seed through the shared artifact-fingerprint helper while preserving
  validated snapshot bindings; reputation
  cross-artifact `snapshot_id_hex` and `merkle_root_hex` consistency now uses a
  shared evidence-value recorder,
  checker summary JSON rendering, stdout emission, and optional summary-output
  writes now flow through the shared preflight helper so stdout/file summaries
  use one sorted, indented, newline-terminated format with human success notices
  on stderr,
  checker stderr error-line, incomplete-evidence block, and human notice
  reporting now flow through shared preflight emitters so operator-facing
  diagnostics cannot drift back to checker-local print loops,
  checker caught argument errors now also flow through shared `ERROR:` line
  exception emitters with deterministic exit code `2` and sanitized malformed
  exception text instead of argparse usage dumps, raw `str(error)` diagnostics,
  or `SystemExit` leaks from handled `ValueError` paths,
  bounded evidence JSON loading rejects duplicate top-level or nested object
  keys, direct non-byte decode inputs, and non-standard `NaN`/`Infinity`
  constants before digest-bound payload validation, and shared numeric rollout
  gates reject direct non-finite `float` values before latency/lag ceilings are
  evaluated,
  shared evidence discovery and bounded evidence JSON reads now reject
  symlinked parent components before path identity resolution, directory scans,
  reserved-output conflict checks, or rollout-gate parsing, including broken
  parent symlinks that would otherwise bypass `exists()`-only checks,
  checker summary and runner dry-run plan rendering now use strict
  `allow_nan=False` JSON output and report non-finite or non-serializable
  summary/plan values before writing stdout or summary files,
  the aggregate production-readiness runner help now labels
  `--deployment-id` and `--environment` as required final deployment context so
  operator review cannot drift from the enforced fail-closed promotion gate,
  and aggregate collection-plan validation independently rechecks that the
  rendered deployment context is a reviewed production deployment before
  dry-run output or verifier launch,
  collection-runner direct namespace threshold and timeout checks now use
  shared runner preflight validators that reject malformed diagnostic
  containers and non-snake-case namespace fields before non-integer and `bool`
  values or local count comparisons, so programmatic callers get structured
  operator diagnostics instead of `TypeError` tracebacks,
  `sorafs_chunk_store` and `sorafs_manifest_chunk_store` now expose the
  existing disk-backed chunk sink through `--chunk-dir-out=dir`, require the
  target directory to be absent or empty before persistence, reject symlink and
  non-directory targets before the sink can remove anything, and include
  deterministic persisted chunk file metadata in the JSON report; generated
  `sorafs_chunk_store` and `sorafs_manifest_chunk_store` JSON, chunk-fetch
  plan, PoR tree, proof, and sample output files now use the same no-follow
  descriptor writer as the fetch/node release paths, with output leaves and
  parent chains inspected before parent creation and non-regular opened targets
  rejected before bytes are written,
  checker raw argparse failures now return deterministic error codes from
  `main(argv)` instead of leaking `SystemExit` to programmatic callers,
  shared `@ARGFILE` expansion now fails closed on path-resolution errors such as
  symlink loops before stat/read, returning stable path-qualified diagnostics to
  every rollout checker and collection runner,
  shared `@ARGFILE` line parsing now reports malformed shell-style arguments
  with response-file path and line number so reviewed operator argfiles can be
  repaired without ambiguous bare parser errors, shared response-argument
  expansion now also rejects scalar string/byte or mapping argument containers,
  malformed parser-returned line-argument containers, non-string line arguments,
  non-string raw response lines, and non-string integer parser inputs before
  character-wise expansion or type errors can mask operator input mistakes,
  shared evidence discovery now reports missing or file-valued `--evidence-dir`
  paths as directory requirements, keeping operator diagnostics precise before
  JSON loading starts,
  checker evidence-source preflight now also flows through the shared checker
  preflight helper so missing `--evidence-dir`/`--evidence` inputs fail before
  discovery or validation without checker-local branches,
  collection-runner dry-run plan JSON rendering now flows through the shared
  runner preflight helper so every rollout/release collection plan uses one
  sorted, indented, newline-terminated stdout format,
  collection-runner dry-run plan emission is now guarded behind `--dry-run`
  across every runner so normal collection execution cannot leak command-plan
  JSON to stdout,
  collection-runner stderr error-line, incomplete-input block, and command-run
  notice reporting now flow through shared runner preflight emitters so
  operator-facing diagnostics cannot drift back to runner-local print loops,
  collection-runner caught argument errors now use shared `ERROR:` line
  exception emitters with sanitized malformed exception text and deterministic
  exit code `2` instead of argparse usage dumps or raw `str(error)` diagnostics
  from handled `ValueError` paths,
  standard required-kind summary finalization now uses a shared helper across
  rollout/release gates so present/valid/artifact-count rows and missing or
  invalid diagnostics cannot drift, and required summary validity now uses the
  shared fail-closed artifact validity helper so malformed validity fields
  cannot raise or pass through truthy values, while reputation intentionally
  retains its richer per-kind bucket flow but uses the same artifact validity
  predicate for artifact rows,
  standard required-kind summary name lists now use a shared helper across
  rollout/release gates so required-kind materialization cannot drift,
  standard required-kind schema lookups now use a shared canonical-label helper
  across rollout/release gates so required summary schema rows cannot drift,
  standard artifact bucket initialization now uses a shared helper across
  rollout/release gates so evidence classification starts from identical
  per-kind buckets,
  standard recognized-artifact bucket recording now uses a shared helper across
  rollout/release gates so artifact insertion cannot drift,
  standard artifact validity checks now use a shared fail-closed helper across
  rollout/release gates, including special hedging, reserve-rent, and
  reputation binding/summary comprehensions, so valid-artifact classification
  cannot drift,
  standard artifact fingerprint access now uses a shared fail-closed helper
  across rollout/release gates, including reputation snapshot binding, so
  malformed fingerprint fields cannot drift,
  standard artifact digest-set derivation now fails closed on malformed
  artifact containers, non-object rows, missing digest fields, and
  noncanonical digest values so Pop credential root/revocation anchor sets
  cannot be partially derived from dirty evidence buckets,
  reputation artifact kind lookups now use a shared fail-closed helper for
  snapshot-bound invalidation so malformed custom rows cannot drift from the
  standard artifact accessor pattern, and existing-row invalidation for
  snapshot-bound artifact errors now uses a shared helper so optional rows are
  skipped consistently, snapshot-bound required-kind membership now uses a
  shared helper so no-anchor failure checks cannot drift locally, and standard
  bound-evidence missing-anchor checks now use shared any-kind/all-kind
  membership helpers so local required-kind predicates cannot drift, standard
  scalar bound-digest reference checks now use a shared helper so digest
  matching, missing-anchor failure behavior, and artifact-error recording
  cannot drift across rollout/release gates, hedging tuple and reserve-rent
  multi-anchor bound-reference checks now use shared helpers so tuple
  matching, wider missing-anchor diagnostics, and summary errors cannot drift,
  reserve-rent rollout evidence now also requires the shared reviewed
  deployment context so reserve promotion cannot pass without deployment and
  environment binding, AI pre-screening, moderation-panel, transparency, and Pop
  credential rollout gates now route runner/workflow, case/roster/tally,
  source/cycle, and root/revocation anchor checks through the shared
  bound-reference helpers, including kind-dependent Pop credential digest-field
  dispatch,
  reputation required-row invalidation now uses a shared helper for custom
  provider/latest and snapshot-bound failures so row creation, validity flags,
  and malformed error-list recovery cannot drift locally, and required
  provider/proof presence now uses shared missing-value and missing-value error
  recording helpers so provider rollout requirements cannot drift behind
  checker-local membership branches;
  the fallback requirement for at least one verified provider proof now uses
  shared required-or-observed presence and error-recording helpers so omitted
  provider allowlists, empty observed provider sets, and mixed dirty aggregate
  value collections cannot drift locally,
	  reputation required summary readiness now uses a shared fail-closed helper
	  so malformed or truthy non-boolean row validity, malformed row errors,
	  empty or malformed artifact buckets, and invalid artifact rows cannot
	  satisfy the gate,
	  reputation custom required-row artifact recording and finalization now use
	  shared helpers so row list recovery, missing-row diagnostics, and per-row
	  artifact validity cannot drift locally, malformed row error buckets and
	  artifact error containers cannot reset silently or split into per-character
	  diagnostics, and provider-count mismatch now uses a shared
	  inconsistent-value error recorder that composes summary-wide
	  invalidation with the distinct-value consistency predicate so all required
	  rows fail consistently without local count-set semantics,
  standard artifact schema diagnostic labels now use a shared fail-closed
  helper in reserve-rent binding messages so malformed artifact rows cannot
  raise while recording errors,
  standard custom artifact detail reads now use a shared fail-closed helper in
  hedging billing-cycle and reserve-rent provider-bake binding flows, while
  appeal-finance and reserve-rent cached fingerprints are reused after their
  first derivation,
  standard artifact-row construction now uses a shared helper across
  rollout/release gates so path, SHA-256, schema, status, validity,
  error-bucket, and payload-free fingerprint fields cannot drift; reputation
  still retains its custom artifact row shape, but snapshot anchor/bound
  classification and downstream binding validation now use shared
  snapshot-binding helpers, and snapshot anchor recording now requires
  canonical non-empty string `snapshot_id_hex`/`merkle_root_hex` values before
  lowercase normalization and mirrors malformed anchor values into artifact
  errors so malformed anchors cannot traceback or become valid bindings, while
  malformed snapshot-binding `kind_name` values now invalidate the artifact
  before anchor/bound routing instead of being silently ignored,
  shared artifact-error recording now rebuilds dirty existing artifact error
  buckets before appending canonical diagnostics,
  standard digest-mismatch artifact recording now rejects malformed artifact
  containers or mixed non-object rows before mutating artifact validity/error
  buckets,
  standard scalar and tuple bound-reference missing-anchor checks now reject
  malformed required-kind and missing-anchor kind collections before mutating
  bound artifacts,
  standard recognized-artifact summary counts now use a shared helper across
  rollout/release gates and fail closed across the whole artifacts-by-kind map
  on malformed buckets or noncanonical kind labels, and reputation's custom
  recognized-list summary count now uses the shared list counter while its
  final recognized-list validity aggregate uses the shared explicit-true helper,
  standard evidence-file summary counts now use a shared helper across
  rollout/release gates so discovered-file counting cannot drift, and malformed
  scalar or mixed non-Path evidence-file rows cannot inflate summary counts,
  evidence-file discovery and missing-directory diagnostics now stay centralized
  in the shared path helper, including reputation's custom loader,
  standard ready/blocked summary status calculation now uses a shared helper
  across rollout/release gates so gate-status semantics cannot drift,
  standard path-qualified validation-error recording now uses a shared helper
  across rollout/release gates so explicit-path and recognized-artifact
  diagnostics cannot drift, while reputation intentionally retains its custom
  per-kind summary error flow, and shared checker stderr emitters now reject
  empty, padded, or control-character diagnostics before printing any
  `ERROR:` line or block heading, while shared checker summary rendering now
  rejects malformed top-level or nested summary keys before stdout or
  `--summary-out` writes, and shared `--summary-out` emission now writes
  through complete descriptor byte loops with descriptor `fsync` before close,
  explicit unrecognized evidence path diagnostics now use a shared helper
  across standard rollout/release gates so explicit-path detection and
  path-qualified validation error recording cannot drift,
  standard string coverage validation now rejects malformed present rows before
  coverage can pass, so object coverage arrays cannot hide scalar entries or
  missing/noncanonical field values and scalar coverage arrays cannot hide
  object/non-string rows,
  standard payload wrapper validation now uses a shared helper across
  rollout/release gates so schema recognition, reviewed deployment context,
  explicit `deployment_context_reviewed` markers, sensitive-field walking, and
  kind-specific callback dispatch cannot drift, and shared deployment-context
  consistency rejects empty fingerprint `deployment_id`/`environment` values as
  artifact errors instead of treating them as absent context, while deployment
  context summaries now emit only complete canonical deployment
  id/environment pairs instead of partial contexts,
  evidence file discovery now owns missing-directory diagnostics and every
  checker calls the shared discovery helper directly so directory existence,
  duplicate detection, resolver failure handling, and existing diagnostic
  text canonicalization cannot drift,
  every checker must use the shared punctuation-insensitive sensitive-field
  walker for camel-case, hyphenated, and high-risk compound
  secret/body/header key variants, including bounded URL-percent/HTML-entity
  decoded spellings and raw-undecoded ASCII alphanumeric-edged diagnostic path
  segment gating, canonical starting diagnostic path validation, canonical
  raw evidence labels without encoded or high-risk sensitive fragments, and
	  shared archive/runner path checks that decode both URL-percent and HTML-entity
	  variants before accepting archive labels, URL host/path labels, rendered path
	  components, passthrough arguments, final aggregate artifact paths, or
	  aggregate dry-run summary inputs, plus checker-rendered summary/evidence
	  paths, explicit `--evidence kind=path` specs, runner command-plan
	  artifacts, reserved output paths, and secret-looking non-Path artifact
	  labels,
	  with path-component secret fragments separated from payload-key sensitivity
	  so exact body payload labels stay blocked without false-positive failures on
	  longer diagnostic/test path names,
	  while preserving payload-free
  digest/absence metadata, bounding nesting depth with a structured error
  instead of recursion tracebacks, rejecting malformed sensitive-field
  diagnostic sinks, noncanonical, duplicate-normalized, or common-alias
  sensitive-key configuration, and non-string payload keys before key
  normalization can raise,
  and requiring inclusion markers to be exactly `false`, including bare
  `included` markers, and all examples must carry
  runtime-only or payload-free evidence handling guidance without
  handoff-placeholder comments, all-zero hex sentinel identifiers, or
  non-comment runtime secret option/field material, and the transparency
  collection example must cover every default source-entry kind required by the
  checker.
  The shared rollout deployment-id validator and final aggregate readiness
  preflight now also reject glued non-production aliases such as
  `testproduction`, `stageproduction`, `productionuat`, `testrelease`, and
  `localproduction`, so short marker families cannot be hidden by joining them
  to production/release labels before per-lane summaries or the final SoraFS
  production gate consume the reviewed deployment context. The same shared
  validator now rejects compact and tokenized pre-release aliases such as
  `prerelease`, `releasecandidate`, `candidateproduction`,
  `productionpreview`, `preprodrelease`, `pre-production`,
  `production-candidate`, `prod-rc`, `prod-preview`, and
  `preprod-production`, so release-candidate, preview, or pre-production
  evidence cannot satisfy reviewed production deployment context by leaning on
  allowed `release`/`prod` labels. The same validator also rejects synthetic
  rollout maturity labels such as `canary`, `alpha`, `beta`, `dry-run`,
  `pilot`, `experimental`, and `trial` even when joined to `prod`,
  `production`, or `release` labels, so canary or pilot evidence cannot be
  promoted by relabeling the deployment id. The shared deployment-id validator
  also rejects the `stg` staging abbreviation as a token or when glued to
  `prod`, `production`, or `release` labels, closing the abbreviated staging
  form before production-readiness summaries can consume it. The shared
  deployment-id validator rejects proof-of-concept labels such as `poc`,
  `proof-of-concept`, and `prototype` before final promotion, so experimental
  proof deployments cannot be relabeled into production-readiness evidence.
  The shared deployment-id validator also rejects smoke, fixture, stub, lab,
  temporary, benchmark, load-test, and work-in-progress deployment labels before
  final promotion, so test-harness or release-smoke identities cannot masquerade
  as production deployment evidence.
  The shared deployment-id validator rejects performance, stress, soak, chaos,
  burn-in, and scale-test deployment labels before final promotion, so resilience
  drills and perf runs cannot be relabeled as final production rollout evidence.
  The shared deployment-id validator rejects shadow, dark-launch, dogfood,
  rehearsal, training, drill, and game-day deployment labels before final
  promotion, so rollout rehearsals cannot be relabeled as final production
  deployment evidence.
  The shared deployment-id validator rejects cutover, blue-green, rollback,
  roll-forward, failover, fallback, and switchover deployment labels before
  final promotion, so transitional rollout operations cannot be relabeled as
  steady production evidence. The shared and final aggregate validators also
  collapse numeric non-production aliases such as `qa2`, `uat01`, `canary2`,
  and `staging1` to their underlying marker before promotion, so numbered
  staging, QA, UAT, or canary identities cannot satisfy final production
  deployment context.
  Provider-admission fixture validation now stages generated artifacts under a
  canonical temp root, preserving symlink-parent rejection in the production
  writer while keeping macOS validation runs out of `/var` symlink paths.
  `scripts/build_sorafs_reputation_canary.py` now builds payload-free
  publish/latest, provider, events, verify, metrics, transport, and
  consumption canary artifacts through the SFM-3 checker before rollout review,
  requiring reviewed deployment context, snapshot id/root bindings, reviewed
  `provider-*` provider names whose unique inventory matches `provider_count`,
  provider proof inputs where applicable, reviewed reputation metric names whose
  unique inventory matches `metric_count`, unique provider proof sibling hashes,
  non-negative integer snapshot-age/ingest-lag threshold facts, and a governed
  `weights_digest_hex` for publish/latest snapshot anchors before writing;
  metrics and transport canaries explicitly emit
  `response_bodies_included: false`, consumption canaries explicitly emit
  `raw_provider_records_included: false`, and duplicate or unknown metric inputs fail before any canary JSON is
  written. The SFM-3 rollout
  checker also rejects duplicate provider proof sibling hashes in externally
  supplied evidence, and metrics evidence now rejects missing, duplicate, or
  unknown reputation metric labels before promotion can report ready, so
  reviewed Merkle proof and metrics paths stay schema-closed outside the local
  canary builder. Event-watch evidence must carry a positive polling `limit`,
  exact `count`/`events[]` length agreement, `count <= limit`,
  duplicate-free sequences, and V1 event rows whose snapshot id, Merkle root,
  and provider count agree across the whole batch before readiness can report
  ready. Transport canaries also
  require reviewed `reputation-sse-event-*` and
  `reputation-websocket-event-*` labels without non-production markers matching
  `sse_event_count` and `websocket_event_count` before writing. Aggregate
  promotion now also requires `valid_reputation_weight_digests` to match
  publish/latest artifact fingerprints and rechecks every publish/latest
  artifact against that metadata before final readiness can report ready. The
  lane checker also has direct adversarial coverage that forges every
  snapshot-bound reputation artifact kind against the publish/latest
  `snapshot_id_hex`/`merkle_root_hex` binding before final readiness can report
  ready.
  The source tree now exports the bounded deterministic finalized-feed
  projector: it joins five physical committed feeds, keeps the unified
  PoR/dispute/token journal on one cursor, reconciles crash-staged state, and
  durably emits keyless signing material through an idempotent
  retry/dead-letter/acknowledgement outbox. Acknowledgement performs full
  anchored trust-policy, quorum, revocation, signature, and freshness
  verification using the locked finalized timestamp; callers cannot select
  that time. Strict `iroha_config`, the daemon-owned Kura-authenticated
  historical archive/query boundary, externally authenticated PoR/counting
  journal submission, and supervised finality reconciliation are wired through
  `irohad`.
  Latest/provider/weights/event routes read the fresh committed
  projection. Focused locked Rust validation, authenticated signed-head
  inclusion, restart revalidation, and exact retained historical snapshot-id
  reads are now complete. Remaining SFM-3 work is full-workspace validation,
  production historical-query and producer-owner wiring, external
  threshold-signing, genuine authenticated Governance DAG
  publication/readback adapters matching the signed-head contract, complete
  SDK/native validation, and reviewed four-peer deployment/recovery evidence.
  Those remaining components and the regional API must be deployed and produce
  live evidence that passes the gate. The obsolete local Torii snapshot POST and
  CLI `reputation publish` path are removed.
  The rollout-gate static contract pins only the still-unshipped supervised
  ingest command, snapshot publisher, regional public API/GraphQL gateway,
  S3/IPFS publication, and production-promotion surfaces with reusable matchers
  and negative controls. The static contract also scans CLI sources for nested
  deployed-only reputation spellings such as `reputation ingest`,
  `reputation publisher`, `reputation graphql`, and `reputation promote`, while
  preserving the read-only local `reputation snapshot|fetch|watch|verify` and
  canary commands. The canonical SF-3 node implementation plan documents the
  current OpenAPI-backed `/v1/sorafs/pin*` and `/v1/sorafs/storage/*` route
  surface, rejects legacy unversioned `/sorafs/*` prototype endpoint names and
  the removed `sorafs-storage` feature-flag wording, keeps node-storage readback
  routes on the OpenAPI `{manifest_id}` parameter, retires the unauthenticated
  local PoR sampling route in favor of authenticated
  `/v1/sorafs/proof/stream`, and carries a rollout static contract that keeps
  those docs and route labels
  aligned with the OpenAPI route strings. Public and localized copies are
  maintained separately in the sibling `iroha-docs` repository. The grouped
  Torii SoraFS storage tests
  now canonicalize temporary storage roots before exercising the backend,
  preserving the production no-symlink parent-chain guard while keeping the
  storage-pin/fetch/PoR round-trip green on symlinked system temp roots.

