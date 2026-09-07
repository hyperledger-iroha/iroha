# Historical project evidence

This page is historical evidence, not current release readiness.
See [the archive index](../../index.md) for provenance and reconstruction.


<a id="record-19b6a3ce092a5b1b3ec9d4a9c842bb2df2bf8de4ab357bca2a42d57f7bd71ae4"></a>

<!-- Original context: Roadmap / Release and Stabilization -->
- SoraFS repair command wiring now accepts exactly one caller-signed Iroha
  transaction on each `/v1/sorafs/audit/repair/*` command route, requires the
  matching native `SubmitSorafsRepairTask`,
  `ApplySorafsRepairTaskAction::{Escalate,Claim,Renew,Complete,Fail}`, or
  `SubmitSorafsRepairAppeal` instruction, and forwards the exact transaction
  through strict durable ingress. Native execution owns authority,
  provider-scoped permission, revision, lease generation, terminal,
  slash/appeal, and idempotency checks. Deleted
  `SignedAuditorRequestV1`/`RepairWorkerSignaturePayloadV1` bodies and raw
  report/proposal bodies are not compatibility formats. Status, task, and
  typed payload-free event reads are finalized ledger projections with exact
  block anchors and immutable exclusive cursors; the obsolete
  status-by-manifest, SSE, WebSocket, and process-local event-authority routes
  are absent from the generated OpenAPI source. The SF-8b rollout evidence gate
  now validates payload-free auditor
  roster, PoR/PoTR failure capture, signed auditor API, worker lifecycle, repair
  event streams, governance handoff, observability, and governance approval
  evidence, requires signed auditor API, worker, event stream, governance
  handoff, and approval artifacts to bind to the valid auditor-roster digest,
  requires worker/event/handoff artifacts to bind to the valid failure-capture
  evidence bundle digest, requires governance approval artifacts to bind to the
  valid governance handoff digest, requires governance handoff artifacts to
  carry `policy_digest_hex`, publishes valid handoff policy digests as
  `valid_policy_digests`, requires governance approval `policy_digest_hex` to
  match a valid governance handoff policy digest, and records roster,
  failure-bundle, handoff-digest, or policy-digest mismatches on the offending
  artifact through the shared scalar binding error recorder, binds signed
  auditor API, worker-lifecycle, and event-stream `route_count` to the unique
  canonical `routes[].name` inventories, and binds auditor-roster
  `auditor_count` to the unique canonical `auditors[].name` inventory, requires
  reviewed `repair-auditor-*` labels without non-production markers, binds
  failure-capture `failure_source_count` to the canonical `failure_sources`
  inventory, binds `failure_event_count` to reviewed `failure_events[].name`
  rows, requires reviewed `repair-failure-event-*` labels without
  non-production markers, and requires reviewed failure events to cover both
  PoR and PoTR sources,
  binds worker-lifecycle `status_count` to the canonical `statuses_observed`
  inventory, and binds governance-handoff `handoff_target_count` to the
  canonical `handoff_targets` inventory, so duplicate route, auditor,
  failure-source, failure-event, lifecycle-status, or handoff-target rows cannot
  inflate readiness, unknown route, failure-source, lifecycle-status,
  handoff-target, or metric labels cannot expand the reviewed evidence surface,
  exports the reviewed observability `metrics` inventory plus
  `metric_count_values` for aggregate production-readiness tethering, and
  requires aggregate promotion to recheck roster-bound, failure-bound,
  handoff-bound, and policy-bound artifact fingerprints against
  `valid_roster_digests`, `valid_failure_bundle_digests`,
  `valid_handoff_digests`, and `valid_policy_digests`, and
  reports those failures before required-kind summary validity is reported. The
  lane checker also has direct adversarial coverage that forges each roster,
  failure-bundle, handoff, and policy digest on every bound downstream kind, so
  signed auditor API, worker, event stream, governance handoff, and governance
  approval evidence all fail against detached roster/capture/handoff anchors
  before promotion. The repair gate now also has missing-field regressions
  proving raw
  roster/evidence/repair-payload/ledger flags, `response_bodies_included`, and
  `critical_alerts_firing` must be explicitly encoded as `false`. The
  matching collection planner accepts reviewed staged evidence
  paths, supports `@ARGFILE`, forwards age, route-latency, event-lag,
  repair-latency, and auditor-count thresholds, and emits a dry-run-visible
  verifier command, checker-backed `evidence_contract` map for the selected
  required kinds, plus operator example args; it now validates the
  route latency, event lag, and repair latency fields as non-negative
  integer-unit evidence before those thresholds can pass, and validates the
  schema-closed collection-plan envelope plus canonical nested required-kind,
  threshold, external-evidence, checker-backed evidence-contract, and
  command-step shapes before dry-run output or verifier execution.
  `scripts/build_sorafs_repair_canary.py`
	  builds payload-free auditor-roster, failure-capture, signed-auditor-API,
	  worker-lifecycle, event-stream, governance-handoff, observability, and
	  governance-approval canary artifacts through the same checker before rollout
		  review. The builder encodes reviewed operator assertions but does not
		  prove finalized task projection, exact-live-lease execution, durable
		  forwarding, restart reconciliation, or a single terminal outcome;
		  promotion requires genuine signed deployment artifacts. It requires
		  complete failure-source, failure-event, route,
	  lifecycle-status, handoff-target, and metric coverage plus
	  duplicate or unknown failure-source, route, lifecycle-status,
	  handoff-target, and metric rejection before writes,
	  roster/failure/handoff digest bindings, reviewed policy-digest input for
  governance handoff and approval canaries, reviewed `repair-auditor-*`
  `--auditor` labels whose unique inventory matches `--auditor-count`,
  reviewed `repair-failure-event-*` failure-event labels whose unique inventory
  matches `--failure-event-count`, default `--failure-event-count` values
  derived from the reviewed failure-source inventory, derived status and
  handoff-target counts for
  reviewed lifecycle-status and handoff-target inventories, and latency threshold
  facts before writing. Native repair task identity, leases, terminal outcomes,
  slash/appeal state, signed-transaction ingress, and finalized committed-event
  queries are implemented. The residual public repair manager, filesystem
  checkpoint, and competing GC/reconciliation dependency are deleted, and
  storage execution now fails closed unless it holds the exact finalized live
  lease. SF-8b still requires source validation and proof of one cross-peer
  terminal outcome. Live PoR/PoTR failure, repair, escalation, governance
  handoff, deployed-auditor-roster, and coordinator evidence remains external.
  The
  rollout-gate static contract now pins live operator-evidence capture,
  deployed auditor-roster, SF-9 coordinator runbook, production failure-capture,
  production handoff, and repair promotion routes or subcommands as unshipped
  with reusable matchers and segment-aware negative controls while preserving
  the signed-transaction report/slash/appeal and worker
  claim/heartbeat/complete/fail endpoints, finalized status/task/event routes,
  `iroha sorafs repair`,
  `iroha sorafs gc`, `sorafs-validate repair`, local repair telemetry, the
  fail-closed SF-8b rollout evidence gate, and payload-free canary evidence
  labels. It also scans CLI sources for nested deployed-only `repair
  live-operator-evidence|deployed-auditor-roster|production-handoff|promote`
  spellings without blocking shipped local `repair list|claim|complete|fail|escalate`
  commands.


<a id="record-872ab969dfbab06a6802cca0900f74dc60cedb61e262d945e483769f68c6202d"></a>

- SoraFS production promotion now has an aggregate readiness gate over the
  existing per-lane rollout/release evidence summaries:
  `scripts/check_sorafs_production_readiness.py` requires every selected
  SoraFS lane summary to be `ready`, payload-free, fresh at the artifact
  fingerprint layer, reviewed for deployment context, run with that lane
	  checker’s full default required-kind set, free of extra `required` rows,
	  carrying empty summary, artifact, and load-error diagnostics, rejects
	  live or broken final-path symlinked explicit evidence, evidence-directory,
	  and directory-discovered summary inputs, rejects live or broken
	  parent-symlinked explicit evidence and evidence-directory inputs at both
	  the shared evidence-path and aggregate-checker layers, applies the same
	  fail-closed symlink policy to reserved-output conflict scans, redacts
	  raw, encoded, format-control-obfuscated, or Unicode-normalized
	  secret-looking canonical path components through the shared path-identity
	  helper, treats non-secret Unicode control/format/private-use diagnostic text
	  as non-canonical across path-identity, evidence-path, and evidence-JSON
	  existing error sinks, labels, failure templates, duplicate-key diagnostics,
	  checker/runner emitted messages, notices, summary keys, artifact labels,
	  runner rendered-plan strings, reviewed response-file arguments,
	  required-kind labels, runner rendered paths/URLs/passthrough args/command
	  vectors, artifact-fingerprint field labels, and sensitive-field
	  error sinks, paths, evidence labels, plus evidence-validation summary
	  error lists, path labels, validation messages, required-row diagnostics,
	  artifact paths/kinds, gate-status errors, and production-readiness
	  aggregate summary/row metadata, and hedging fixture JSON sidecar field
	  labels, requires exact reviewed closed-set inventory membership without
	  trim-normalizing padded values, including custom Governance DAG
	  payload-kind gates, reputation explicit evidence/provider-proof specs, and
	  AI pre-screen/transparency source-entry specs, applies the same exact
	  parser policy to shared checker `--evidence KIND=PATH` preflight, and
	  excludes non-canonical scalar inventory values from product/count
	  derivations, sanitizes matching resolver
	  exception text before diagnostics can echo filesystem labels, and keeps
	  evidence/artifact counts consistent with the validated rows, with
	  recognized artifact totals derived from validated recognized-artifact objects, evidence
	  file counts matching the distinct archive-portable recognized artifact paths, final
	  aggregate lane rows rejecting evidence counts above recognized artifacts
	  or recognized artifact counts that drift from artifact counts, threshold
	  metadata kept as a non-empty canonical non-negative integer map and preserved
	  in each aggregate lane row for release review, extra top-level lane-summary
	  fields outside the schema-closed payload-free lane summary contract rejected,
	  lane-specific top-level metadata required and validated as payload-free
	  canonical strings, non-negative integers, booleans, objects, and lists with
	  expected non-empty container shapes, bound to the lane-specific contract that
	  emits them, exact lowercase-hex binding-list metadata shapes validated before
	  aggregate promotion and tuple binding-list metadata tethered to explicit
	  owning required artifact kinds before fingerprint matching, exact
	  lowercase-hex and positive-integer scalar list
	  metadata shapes validated before aggregate promotion, governance
	  public-head identifiers validated as lowercase hex list metadata before
	  aggregate promotion, exact object-list metadata shapes validated before
	  aggregate promotion, object-list detail rows including ids, counts, timing
	  fields, and digests tethered to the owning required-row artifact
	  fingerprints through an explicit per-gate owner-kind map before aggregate
	  promotion, aggregate artifact totals derived
	  from observed required-row artifact object rows instead of untrusted
	  claimed row counters, malformed non-object list entries, or missing/empty
	  artifact containers before release-review output, recognized-artifact
	  expected counts likewise derived from required artifact object rows, exact duplicate
	  object-list
	  metadata entries rejected while preserving artifact order,
	  domain-duplicate object-list metadata identities rejected before aggregate
	  promotion, exact object metadata shapes validated before aggregate promotion,
	  set-derived lane
	  metadata lists required to be duplicate-free and sorted in canonical order,
	  reputation provider id and provider-count lists tethered to recognized
	  artifact fingerprints before aggregate promotion, all hex-list anchor
	  metadata fields tethered to their owning required artifact kind or
	  jointly owning artifact kinds instead of any lane artifact before
	  aggregate promotion across the 17 rollout/release lanes with a regression
	  invariant and fail-closed aggregate-checker error for untethered future
	  hex-list fields, concrete validator coverage for every declared lane
	  metadata field with fail-closed errors for unclassified future fields,
	  aggregate summary `required_gates` labels required to be canonical,
	  duplicate-free, known gate names that match the requested release-review
	  gates, ready aggregate summary file counts required to match the requested
	  release-review gate count so no hidden summary files can be represented as
	  ready output, recognized-summary counters required not to exceed discovered
	  summary files or requested gate count, unknown aggregate `required` row keys
	  rejected without echoing forged labels, and summary-level aggregate thresholds schema-closed to
	  `max_summary_artifact_age_secs` so no extra promotion knobs can be carried
	  in final release-review output,
	  malformed
	  sensitive-field path diagnostics sanitized before aggregate errors are
	  written, secret-looking bearer/basic auth, cookie, JWT, and private-key PEM
	  scalar values, sensitive scalar assignments, URI userinfo, and sensitive
	  URI host, path, or query parameter names or values rejected from raw or
	  fully encoded HTTP(S), WebSocket, database-style, file, or other URI-like
	  strings, zero-width Unicode format controls removed before scalar
	  secret-value matching, and decoded multiline copied header/transcript
	  strings scanned line-by-line, including folded secret header
	  continuations after earlier non-sensitive headers, without echoing the
	  value, explicit final
	  `--deployment-id`/`--environment` required even
	  for direct checker invocations, and required-row schema labels kept
	  canonical when present while
	  extra required-row fields outside the schema-closed payload-free required-row
	  contract are rejected, before binding
	  to the same `deployment_id`/
  `environment` and emitting
  `sorafs.production_readiness.aggregate_gate.v1`. The companion
  aggregate row deployment contexts required to match the aggregate deployment
  block before any ready summary is emitted. The companion
  `scripts/run_sorafs_production_readiness.py` accepts reviewed summary paths,
  requires exactly one summary input per required gate, requires an explicit
  canonical `--deployment-id`/`--environment` pair, supports `@ARGFILE`, and
  validates the schema-closed collection plan envelope against the built command
  plan before dry-run output or execution, including canonical envelope field
  names, canonical collection and verifier schema labels, schema-closed final
  deployment context,
  canonical duplicate-free known required-gate labels,
  schema-closed non-negative/positive threshold fields, and an
  external-summary map whose canonical known gate keys and
  single summary-path lists must match the required gates, plus schema-closed
  per-gate summary contracts whose canonical fields, schemas, and canonical
  required-kind lists must match the selected gate contracts, and schema-closed
  command steps with canonical fields, labels, artifacts, and command arguments, while
  rejecting non-object or non-strict-JSON collection-plan renderings. The runner also rejects reviewed
  summary input paths with secret-looking, control-character, parent/current,
  or platform-specific components before they can be rendered into dry-run
  command plans. It also rejects plan-rendered verifier, output-directory, and
  summary-output paths plus runner input files/directories with secret-looking,
  control-character, parent/current, drive-prefix, or platform-specific
  components before dry-run output through the shared runner preflight, and the
  production-readiness runner now repeats rendered-path safety checks inside
  collection-plan envelope validation for external summaries, artifacts, and
  path-bearing command positions so validation drift cannot expose unsafe path
  strings, with adversarial coverage now pinning encoded unsafe output
  directories, summary-output paths, verifier paths, tampered external-summary
  entries, step artifact labels, and command-path arguments before dry-run plan
  emission. The shared runner preflight now evaluates raw and repeatedly
  percent-decoded path, URL host/path, and passthrough argument variants before
  classifying secret-looking components, traversal, separators, drive prefixes,
  and URI-scheme-looking path components, so encoded or double-encoded bypasses
  cannot enter dry-run command plans. Shared rollout artifact path validation
  now applies the same repeated percent-decoding to archive-relative labels, and
  the aggregate production-readiness gate rejects encoded traversal, hidden
  separators, drive prefixes, URI-like path components, and secret-looking
  components in recognized or required artifact paths before reporting ready.
  URL-rendering SoraFS collection runners now also use the shared URL preflight
  so deployed service URLs with userinfo, query strings, fragments, control
  characters, or secret-looking host/path components cannot enter dry-run
  command plans. Command passthrough arguments such as `--iroha-arg`,
  `--iroha-bin`, and `--sorafs-cli-bin` are also rejected before dry-run plan
  rendering when they contain secret-looking option names, values, paths, URLs,
  or control characters.
  Required rows and their artifact entries must carry schema labels that match
  the owning checker evidence schemas. Artifact entries must also carry
  canonical unique archive-relative paths without absolute, empty, current,
  parent, encoded, URI-scheme-like, platform-specific, or secret-looking path
  segments and lowercase SHA-256 digests,
  reject explicit artifact `status` labels outside successful states such as
  `passed` or `verified`, reject extra artifact-row
  fields outside the schema-closed payload-free artifact contract, and the
  required top-level
  `recognized_artifacts` inventory must be fully valid, kind-bound to that
  lane's full required-kind contract, matched per kind to the required-row
  artifact counts and `(kind, path, sha256)` identities plus required artifact
  metadata, fresh, and deployment-context reviewed. Complete fixture summaries
  from every required rollout/release lane now pass the aggregate gate contract
  directly, with lane fingerprints carrying `generated_at_unix`, `deployment_id`,
  `environment`, and `deployment_context_reviewed` while run/cycle/bake detail
  metadata stays out of schema-closed artifact rows; the aggregate CLI also
  assembles all real complete lane fixture summaries into a ready production
  summary once those deployment-context fields are normalized to one reviewed
  rollout target, and deployment-bearing top-level lane metadata such as
  `deployment_context`, `valid_billing_cycles`, `valid_e2e_runs`,
  `valid_multi_peer_runs`, and `valid_provider_bakes` must now match the
  artifact-derived deployment context before aggregate promotion. Scalar hex,
  string-list, string-array inventory, positive-integer list, digest-list, and
  tuple binding metadata such as reputation snapshot IDs/roots, provider
  IDs/counts, reviewed appeal-finance/gateway-compliance/gateway-load/Governance
	  DAG/hedging-billing/moderation-panel/orderbook/PDP/PoP
	  credentials/PoR/PoTR/repair/reserve-rent/reputation metrics and
	  metric counts, snapshot bindings, runner
	  bindings, policy/matrix/ledger
	  bindings, and roster/tally bindings
	  must also be backed by recognized artifact fingerprints from their declared
	  owner artifact kinds, so a lane summary cannot
	  claim payload-free release-review anchors that are absent from its artifacts.
		  The aggregate gate now also cross-checks moderation-panel
		  `valid_roster_bindings`, `valid_tally_bindings`, `valid_e2e_runs`, and
		  `valid_evidence_viewer_digest_sets` so forged summaries cannot skip the
		  case -> roster -> tally relationships proven by the lane checker, and
		  replays case-bound, roster-bound, tally-bound, and policy-bound artifact
		  fingerprints against the exported aggregate anchors.
	  Object-list detail metadata for billing cycles, E2E runs, multi-peer runs,
  and provider bakes must match the corresponding required artifact row
  cardinality and declare the same owner kind used for fingerprint tethering,
  with provider-bake detail rows also carrying lowercase
  policy/matrix/ledger digests, so release review cannot promote missing,
  extra, or digestless detail rows while the artifact inventory stays ready.
  Aggregate lane rows are also
  schema-closed before release review, with archive-relative summary path
  labels derived from evidence-directory membership or safe explicit basenames,
  lowercase SHA-256, count, timestamp, list, and error shapes checked after the
  row digest is attached, and the final aggregate summary envelope is
  schema-closed before the production-readiness report is written. Aggregate
  status must match canonical, duplicate-free
  aggregate diagnostics before release review, and ready aggregate summaries
  must carry complete deployment context with a reviewed deployment id, a final
  `prod`/`production` environment, and only present, valid required rows.
  Final aggregate required rows also have exact present and missing row output
  contracts, so failed or absent lane rows cannot grow extra payload-bearing
  fields while still being reported, and absent lanes must keep deterministic
  missing-row diagnostics. Invalid aggregate required rows must keep canonical
  thresholds, deployment labels, final production environment labels when
  present, duplicate-free diagnostics, exact required-kind count contracts,
  canonical numeric count fields, positive timestamp fields, and timestamp
  ordering before blocked rows are emitted for release review. The aggregate
  recognized-summary count must also match the final present required-row set,
  and duplicate lane summaries must keep deterministic duplicate-summary
  diagnostics while every duplicate input remains counted as a top-level
  blocker. Unknown summary schemas and explicit unrequired summaries must each
  leave matching aggregate blockers.
  Unknown summary schemas
  discovered in summary directories now fail closed instead of being ignored.
  This does not close the live deployment gaps above; it prevents
  production promotion from being claimed until those lane gates all pass
  together for the same deployment.


<a id="record-3aad4b8baadbd70be9573251d67b156aa8e1ddf40c843994f0acc2f9ac907a8e"></a>

- Keep SoraFS paid-pin SDK builders fail-closed before submit. Java/Kotlin
  pin-manifest builders now validate manifest digest, chunk digest, optional
  successor digest, alias-proof hex shape, and `Hot`/`Warm`/`Cold`
  storage-class policy values in both builder and argument decoding paths,
  alongside the existing content length, epoch, replica, and partial-alias
  guards. Python now also exposes raw and typed
  `register_sorafs_pin_manifest` helpers that validate/canonicalize manifest,
  chunk, successor, pin-policy, chunker, alias-proof, and credential-alias
  inputs before the Torii register request is emitted, and rejects duplicate
  camelCase/snake_case paid-pin request and typed-response aliases before any
  precedence rule can hide conflicting caller data. C# now exposes the same
  Torii register path through typed models and validates/canonicalizes request
  and response digest, policy, chunker, alias-proof, and fee-receipt fields.
  Swift now exposes matching async/completion register helpers and typed
  request/response models with the same digest, policy, chunker, alias-proof,
  and typed-response normalization before SDK callers observe a paid-pin
  receipt. The JavaScript Torii register helper now also rejects contradictory
  camelCase/snake_case paid-pin request and response aliases before submit or
  typed decoding, while accepting canonical snake_case successor and policy
  inputs. Torii gateway policy admission now also treats only an explicit,
  signed `X-SoraFS-Manifest-Envelope` bound to a paid-pin registry record as
  manifest-envelope evidence, so alias proof headers, malformed envelopes, and
  missing registry records cannot satisfy `require_manifest_envelope`, and a
  stale envelope no longer passes after registry chunk/profile metadata or
  approved envelope-digest rotation.

