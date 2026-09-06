# Historical project evidence

This page is historical evidence, not current release readiness.
See [the archive index](../../index.md) for provenance and reconstruction.


<a id="record-d1c192ca22bb6367cea3b2476d2cd9b97608a0871bc2814dc8a4ea7d311edd08"></a>

<!-- Original context: Roadmap / Release and Stabilization -->
- SFM-4a gateway moderation foundations now use only the governed compliance
  controller. Canonically authenticated operator routes fetch bounded configured
  feeds and expose controller status, then stage, acknowledge, promote, or roll
  back predecessor-bound threshold-signed catalogs. Competing process-local
  catalog/pack routes and mutation surfaces are absent. Serving evaluates the
  active catalog fail-closed at every content boundary: a governed denial is
  HTTP 451 `gateway_compliance_denied` with its bounded decision source and
  catalog digest, while unavailable, stale, conflicting, or poisoned controller
  state is HTTP 503 `gateway_compliance_unavailable`. Torii CID lookup and
  `/.well-known/sorafs/manifest` metadata accept `limit` (default 50, max 500)
  for embedded site-file listings, preserve full file counts/returned
  counts/truncation flags, and keep `manifest_b64` plus gateway content serving
  complete. `sorafs_node` now also admits validated moderation
  reproducibility and adversarial corpus manifests into a local model registry
  snapshot, rejects reproducibility manifests with zero digests, duplicate
  model IDs or model artefact/weights digests, unsupported non-17 model opsets,
  out-of-range thresholds or model weights, all-zero model weights, duplicate
  adversarial family/variant identifiers, and reused match fingerprints,
  rejects conflicting manifest ids, keys corpus entries by
  canonical Norito BLAKE3 digest, persists the snapshot as a Norito checkpoint
  when storage is enabled, reloads it on node startup, and exposes canonical-
	  authenticated Torii admission plus bounded readback endpoints under
	  `/v1/sorafs/moderation/model-registry`. `iroha sorafs moderation registry
	  submit-repro|submit-corpus|list` now wraps local model-registry
	  admission/readback, validates JSON or Norito manifest inputs, and sends
	  canonical Norito manifest bytes through signed Torii requests.
	  `sorafs_cli moderation registry-serve` now exposes a standalone persistent
	  HTTP model-registry service backed by an atomic Norito checkpoint, with
	  status, bounded snapshot readback, and base64 canonical Norito
	  repro/corpus admission endpoints that reuse the data-model validators and
	  reject conflicting manifest ids.
  `iroha::client` and `iroha sorafs moderation ballots
  list|get|no-show-plan|events|commit|reveal|tally` now wrap the local
  moderation ballot readback and signed committee lifecycle endpoints,
  validating JSON or Norito commit/reveal payloads and submitting canonical
  Norito bytes to Torii.
  Torii now also exposes the payload-free local
  `/v1/sorafs/moderation/ballots/{case_id}/{round_id}/no-show-plan` readback
  route for closed ballots, using server-side network time to reject open
  reveal windows and unresolved or accepted challenges while returning only
  no-show counts, juror identifiers, and the stable penalty-plan digest.
  `iroha::client` and `iroha sorafs transparency
  cycles|explorer|tokens|source-entry` now wrap the local transparency
  readback and signed source-entry ingest surface so operators can inspect
  published cycles, entry proofs, explorer snapshots, proof-token issuance
  indexes, and submit typed source-entry JSON for later publication.
  `iroha::client` and `iroha sorafs appeals pricing
  config|status|quote` plus `iroha sorafs appeals finance` now wrap the local
  appeal pricing, asset-lock deposit, settlement, reconciliation, and finance
  report readback endpoints. Pricing quote submission validates and
  canonicalizes JSON; finance mutation and deposit readback calls use canonical
  Iroha request signing; deposit get normalizes 32-byte escrow ids; finance
  report, weekly-rollup, and settlement-receipt readbacks support bounded
  `limit` queries. Torii, `iroha::client`, and `iroha sorafs moderation
  quarantine appeal-handoff` now also expose local reviewed-quarantine appeal
  handoff: pending/released records fail closed, reviewed records produce a
  baseline pricing quote, quote-bound deposit request, and native
  `OpenAssetLock` instruction for payer signing. Torii, `iroha::client`, and
  `iroha sorafs moderation quarantine appeal-ballot` now also verify confirmed
  handoff-bound appeal deposits and announce the existing local moderation
  ballot, failing closed when the quarantine record is not reviewed or the
  deposit evidence omits the deterministic quarantine handoff hash.
  `sorafs_cli moderation run-local`
  now validates governance-signed reproducibility manifests, reads payload
  bytes, derives deterministic local model scores from the manifest
  seed/material and payload digest, and emits Torii-compatible
  screening-result JSON for local admission fixtures. `sorafs_node` also persists
  deterministic local screening-result records and pending local quarantine
  records under `moderation-screening/screening-snapshot.to`; `quarantine` and
  `escalate` verdicts enqueue pending review records, and Torii exposes
  canonical-authenticated `POST /v1/sorafs/moderation/screening-results` plus
  bounded readback through `GET /v1/sorafs/moderation/screening-results` and
  `GET /v1/sorafs/moderation/quarantine`. The local quarantine queue now also
  advances records through reviewed and released states with checkpointed
  operator metadata via canonical-authenticated accounts assigned the
  `sorafs_moderation_operator` role at
  `POST /v1/sorafs/moderation/quarantine/{quarantine_id_hex}/review` and
  `POST /v1/sorafs/moderation/quarantine/{quarantine_id_hex}/release`.
  `iroha sorafs moderation screening submit|list` now bridges deterministic
  local runner output into the signed screening-result admission endpoint and
  bounded readback endpoint, validating the runner JSON before submission.
  `iroha sorafs moderation quarantine list|review|release` now wraps those
  local queue endpoints, validates 16-byte quarantine ids, applies canonical
  signing for review/release, and defaults operator identities to the CLI
  account when omitted. `sorafs_node` now also seals quarantined payload bytes
  into encrypted local Norito object envelopes under the storage data
  directory, persists a separate object-index checkpoint, reloads it on
  restart, and verifies plaintext digests plus envelope authentication before
  returning payload bytes. Torii now exposes canonical-authenticated and
  `sorafs_moderation_operator` role-gated local object store/readback at
  `POST`/`GET /v1/sorafs/moderation/quarantine/{quarantine_id_hex}/object`,
  accepting base64 payload bytes on store and returning `payload_b64` on
  verified reads. `iroha sorafs moderation quarantine object store|read` now
  wraps that local object API for operators, reading store payload bytes from
  `--payload-file`, rejecting empty payload files, signing store/read requests,
  and printing object metadata or payload readback JSON. Torii,
  `iroha::client`, and
  `iroha sorafs moderation quarantine operator-panel` now also expose a
  `sorafs_moderation_operator` role-gated local workflow read model for one
  quarantine record, bundling encrypted-object metadata status, matching local
  appeal ballots, operator routes, and next-action hints without returning
  payload bytes. `iroha sorafs moderation quarantine bridge-plan` now derives a
  payload-free local automation plan from that read model, emitting ordered
  handoff, ballot, tally, and transparency CLI actions while failing closed if
  the panel response unexpectedly contains payload bytes.
  `sorafs_cli moderation runner-serve` now promotes the deterministic local
  runner into a locked-manifest HTTP service mode: status endpoints report the
  active governance manifest and disabled outbound-network posture, while
  `POST /v1/sorafs/moderation/runner/screen` returns the same
  Torii-compatible screening-result JSON as `run-local` from explicit
  request input. `sorafs_cli moderation runner-grpc-serve` now exposes the
  production unary gRPC runner surface
  (`sorafs.moderation.runner.v1.Runner/Status` and `/Screen`) over a
  locked governance manifest, accepting payload bytes directly and returning
  deterministic screening-result DTOs with outbound network disabled.
  `iroha sorafs moderation quarantine operator-serve` now exposes a local
  payload-free HTTP operator workflow service with a browser UI at `/` and
  `/v1/sorafs/moderation/operator-panel/ui`, health/status, operator-panel, and
  bridge-plan and juror-plan GET routes backed by the signed Torii
  operator-panel read model; it rejects request bodies, validates 16-byte
  quarantine ids, supports bounded ballot limits, and fails closed if the
  upstream panel unexpectedly includes payload bytes. The juror-plan view
  reports per-juror commit/reveal readiness, signing accounts, signed Torii
  routes, and CLI command templates without embedding private commit or reveal
  payload bytes. The companion juror-notifications view emits deterministic
  operator-managed delivery records with dedup keys, subjects, message bodies,
  signed Torii routes, and CLI command templates for external
  mail/webhook/scheduler transport.
  `iroha sorafs moderation quarantine notifications deliver --manifest PATH
  [--out-dir DIR] [--webhook-url URL]` now validates those payload-free
  notification manifests, writes canonical outbox JSON and/or POSTs each
  notification to a webhook, rejects private-payload flags, and emits
  payload-free delivery evidence with notification and response body hashes.
  `iroha sorafs moderation quarantine notifications canary --manifest PATH
  --webhook-url URL [--out PATH]` now probes deployed notification transport
  webhooks with the same payload-free manifest, records passed/failed probe
  status, notification body hashes, and response body hashes, and can write
  `sorafs.moderation.juror_notifications.transport_canary.v1` evidence without
  archiving message or response bodies.
  The companion commit-reveal-status view
  emits payload-free quorum readiness, missing-juror lists, next actions, and
  tally-ready request templates. The same service now forwards signed review,
  release,
  appeal-handoff, appeal-ballot, and ballot-tally POST requests to Torii after
  rejecting `payload_b64`, rejecting mutation query parameters, defaulting
  review/release actors to the configured CLI account when omitted,
  canonicalizing appeal JSON bodies before forwarding, and deriving tally
  `case_id`/`round_id` values from the payload-free operator-panel ballot view
  when they are not supplied explicitly.
  `iroha sorafs moderation quarantine operator-canary` now captures
  payload-free rollout evidence from a deployed operator workflow service by
  probing health/status, browser UI, operator-panel, bridge-plan, juror-plan,
  juror-notifications, and commit-reveal-status routes, verifying expected
  schemas and UI markers, rejecting payload bytes, and archiving response
  hashes instead of response bodies.
  `iroha sorafs moderation ballots execute --status PATH
  [--commit-payload PATH...] [--reveal-payload PATH...] [--submit-tally]` now
  consumes the payload-free commit/reveal coordination status, validates local
  commit/reveal payload files against pending juror lists, submits only pending
  signed commit/reveal/tally requests through Torii, and emits response
  status/body hashes without replaying private reveal payload internals.
  `iroha sorafs moderation ballots executor-bundle --status PATH
  --bundle-out DIR [--commit-payload PATH...] [--reveal-payload PATH...]
  [--submit-tally]` now generates a payload-free scheduled executor job bundle
  with `executor.env`, executable `run.sh`, systemd service/timer files,
  launchd plist, README, and
  `sorafs.moderation.ballots.executor_bundle.v1` metadata without copying
  private commit/reveal payload files.
  `iroha sorafs moderation ballots executor-canary --bundle DIR
  [--execution-summary PATH] [--out PATH]` now verifies generated executor
  bundles and optional payload-free `ballots execute` summaries, records
  artifact hashes, scheduler checks, summary hashes, and pass/fail status, and
  emits `sorafs.moderation.ballots.executor_canary.v1` evidence without
  archiving private payload files or response bodies.
  `sorafs_cli moderation runner-bundle` now generates the
  supervised HTTP runner deployment bundle for a validated locked manifest,
  including the manifest copy, `runner.env`, executable `run.sh`, systemd
  unit, launchd plist, README, and
  `sorafs.moderation.runner.bundle.v1` metadata JSON. `sorafs_cli moderation
  runner-canary` now probes deployed locked-manifest HTTP runners, verifies
  status and screening responses against the manifest id, runner hash, payload
  digest, score range, and threshold-derived verdict, and emits payload-free
  `sorafs.moderation.runner.rollout_evidence.v1` JSON for rollout archives.
  `sorafs_cli moderation committee-run` now validates the same locked
  reproducibility manifest, rejects payload-bearing runner outputs, verifies
  manifest/runner/subject consistency and threshold-derived verdicts, and
  emits payload-free `sorafs.moderation.committee.aggregate.v1` JSON using a
  deterministic median score under the requested quorum. `sorafs_cli moderation
  committee-serve` now locks that manifest and quorum into a bounded local HTTP
  service with status and payload-free aggregation endpoints. `sorafs_cli
  moderation committee-bundle` now generates supervised HTTP committee
  deployment artifacts, and `sorafs_cli moderation committee-canary` verifies
  deployed committee status plus payload-free aggregate responses against the
  locked manifest and deterministic local aggregation.
  `scripts/check_sorafs_ai_prescreen_rollout_evidence.py` now gates SFM-4a
  promotion on payload-free deployed runner, committee, operator workflow,
  juror notification transport, commit/reveal executor, moderation
  transparency source-entry, Governance DAG, and end-to-end workflow evidence,
  requires reviewed `deployment_id`/`environment` context on every artifact,
  rejects unsafe external URL evidence fields with the shared SoraFS URL
  preflight before staged evidence can report ready,
  requires notification manifest paths, commit/reveal executor artifact paths,
  execution-summary paths, and transparency payload paths to be archive-portable
  after repeated percent-decoding,
  requires committee evidence to match the valid runner manifest/hash/subject
  tuple, and requires operator workflow, notification transport, executor,
  transparency, and Governance DAG artifacts to bind back to the valid
  end-to-end `workflow_digest_hex`. Runner artifacts must carry
  `evidence_digest_hex` and `policy_digest_hex` evidence anchors, and
  Governance DAG artifacts now also bind their `policy_digest_hex` to a valid
  runner policy digest, with workflow and policy scalar binding failures
  recorded through the shared scalar binding error recorder and runner tuple
  binding failures recorded through the shared string-tuple binding error
  recorder so artifact invalidation cannot drift from other rollout gates,
  rejects runner and committee artifacts whose `subject` is not a reviewed
  lowercase `cid:*` reference or contains non-production markers,
  rejects runner and committee artifacts whose `verdict` is not one of the
  shipped moderation labels (`pass`, `warn`, `quarantine`, `escalate`, or
  `block`),
  requires runner `combined_score_bps` and committee `aggregated_score_bps` to
  be integer basis-point evidence in the inclusive `0..=10000` range before
  promotion can report ready,
  binds committee `result_count` to the unique canonical `results[].name`
  inventory and requires reviewed `ai-prescreen-committee-result-*` labels
  without non-production markers so malformed or duplicate committee-result rows
  cannot inflate readiness,
  binds operator-workflow `route_count` and `passed_route_count` to the unique
  canonical `routes[].name` inventory so duplicate or unknown route rows cannot
  inflate readiness, requires reviewed `GET` methods and exact operator route
  paths for each route name and quarantine id, binds route URLs back to the
  slashless top-level `operator_url` plus the reviewed path without
  normalization, requires runner, committee, and operator base URLs to be
  slashless before derived route evidence can count, requires JSON/browser
  content types to match the reviewed route table, requires operator route
  `body_bytes` to be positive, and requires every route response to carry a
  `body_blake3_hex` digest,
  validates notification, transparency, executor bundle, and execution-summary
  byte counts before evidence can report ready, and emits
  `sorafs.moderation.ai_prescreen.rollout_evidence_gate.v1` summaries.
  `scripts/run_sorafs_ai_prescreen_rollout_evidence.py` now provides the
  matching collection planner/runner, composing the shipped runner, committee,
  operator workflow, notification transport, executor, and transparency
  canaries before invoking the gate with the required external Governance DAG
  and end-to-end workflow evidence files.
  Its dry-run JSON now exports the SFM-4a `evidence_contract` map so operators
  can inspect each required schema and payload field before collecting staged
  screening rollout evidence, and the runner validates the schema-closed
  collection-plan envelope, external evidence map, checker-backed evidence
  contract, and command steps before dry-run output or live canaries. It also
  rejects duplicate or unsupported `--source-entry` kinds before dry-run output
  or live canaries.
  `scripts/build_sorafs_ai_prescreen_canary.py` now builds payload-free
  checked-in SFM-4a canary artifacts for each gate kind, fails closed on
  missing runner/workflow binding inputs, committee-result inventory
  mismatches or labels outside the `ai-prescreen-committee-result-*` production
  family, incomplete operator-route, transparency-source, Governance DAG
  producer, or workflow-step coverage, non-production or malformed
  `--subject` references outside the gate's `cid:*` production shape,
  non-production or malformed
  `--workflow-id` labels outside the gate's `sfm-4a-*` production shape,
  duplicate or unknown reviewed route,
  source-kind, producer, and workflow-step inputs, Governance DAG
  `--edge-count` binding to the required producer inventory, or Governance edge
  names outside the `ai-prescreen-governance-edge-*` production family,
  notification transport evidence that lacks both shipped `submit_commit` and
  `submit_reveal` delivery actions or supplies a `--probe-count` value too
  small to cover them, defaulting generated notification canaries to the
  shipped action inventory,
  commit/reveal executor action-count breakdowns that do not sum to
  `--action-count`,
  rejects unsupported `--verdict` labels and out-of-range `--score-bps` values
  before runner or committee evidence is written,
  runs runner/committee/operator/webhook URL inputs through the shared URL
  preflight before writing evidence,
  rejects unsafe `--manifest-path` and `--execution-summary-path` path labels
  before writing evidence,
  validates every generated artifact through the AI pre-screen rollout checker,
  and ships response-file examples for the end-to-end workflow anchor, juror
  notification transport, and commit/reveal executor canaries. Valid
  notification-transport artifacts now publish their reviewed
  `manifest_body_blake3_hex` values as `valid_notification_manifest_digests`, and
  valid commit/reveal executor artifacts publish their top-level
  `execution_summary_digest_hex` values as `valid_executor_summary_digests`;
  the final SoraFS aggregate gate accepts both metadata fields only when they
  match recognized artifact fingerprints. The final gate also rechecks SFM-4a
  runner-bound, workflow-bound, and policy-bound recognized artifact
  fingerprints against `valid_runner_bindings`, `valid_workflow_digests`, and
  `valid_policy_digests`, so forged aggregate summaries cannot swap committee
  subjects or detached workflow/policy digests while preserving valid anchors.
  The lane checker also has direct adversarial coverage that forges every
  runner-bound, workflow-bound, and policy-bound SFM-4a artifact kind against
  those anchors before promotion can report ready.
  The SFM-4a rollout checker now also requires exactly one active runner
  binding, workflow digest, notification manifest digest, executor summary
  digest, and policy digest before runner-bound, workflow-bound, policy-bound,
  transport, executor, or aggregate metadata can satisfy final promotion.
  Juror notification transport
  artifacts now bind `probe_count` and `accepted_count` to the unique canonical
  `probes[].delivery_id` inventory, require reviewed
  `ai-prescreen-notification-delivery-*` labels without non-production markers,
  require unique dedup keys bound to each reviewed delivery ID, and require
  shipped `submit_commit`/`submit_reveal` actions plus non-empty case, round,
  and juror identifiers. Transparency publication artifacts bind `probe_count`,
  `passed_probe_count`, and `source_entry_probe_count` to the unique canonical
  `probes[].source_kind` inventory, so malformed or duplicate delivery rows or
  duplicate/unknown source-entry probe rows cannot inflate readiness.
  Notification transport evidence now also requires at least one accepted
  delivery, positive notification byte counts, and non-negative webhook response
  byte counts, and transparency source-entry evidence requires coverage for
  every required moderation source kind plus positive request byte counts and
  non-negative response byte counts, while transparency publication artifacts
  must explicitly set `payload_bytes_included`, `private_payloads_included`, and
  `response_bodies_included` to `false` before readiness can report. The
  AI pre-screen rollout tests now also cover missing required false fields
  across operator routes, notification probes, executor artifacts and execution
  summaries, transparency probes, Governance DAG artifacts, and end-to-end
  workflow artifacts. Commit/reveal executor
  artifacts also bind `artifact_count` and `passed_artifact_count` to the unique
  canonical `artifacts[].name` inventory, so duplicate or unknown executor
  bundle artifact rows cannot inflate readiness; the reviewed executor artifact
  inventory must cover both `executor.env` and `run.sh` and now binds each
  reviewed artifact name to its exact archive-relative path and expected kind
  (`executor.env`/`env`, `run.sh`/`script`) so swapped-path or mislabeled
  executor bundle evidence cannot pass, and executor evidence must use the
  reviewed `sorafs-moderation-ballots-executor` service identity. Executor
  evidence now also validates bundle metadata bytes/digests, service interval
  seconds, artifact byte counts, execution-summary byte counts, and requires
  commit/reveal/tally action counts to sum to `action_count` before readiness
  can report. Governance DAG artifacts also bind
  `producer_count` to the unique canonical `producers[].name` inventory and
  `edge_count` to reviewed `edges[].name` rows and the required governance
  producer inventory while requiring edge coverage for every required
  governance producer, and end-to-end workflow artifacts require reviewed
  lowercase `sfm-4a-*` workflow ids without non-production markers, then bind
  `step_count` and `passed_step_count` to
  the unique canonical `steps[].name` inventory while requiring every required
  workflow phase, so duplicate/unknown producer rows, duplicate edge rows,
  inflated edge counts, missing workflow phases, or duplicate/unknown
  workflow-step rows cannot inflate readiness. The
  rollout-gate static contract now also keeps
  unshipped moderation portal commands such as `sorafs moderation jury-accept`
  and `sorafs moderation open-case` warning-only in SoraFS docs until the
  corresponding service and CLI handlers exist, with the same boundary-aware
  command matcher preserving canary/evidence labels, and scans CLI sources for
  nested `moderation open-case|panel service|jury-accept|portal` spellings so
  those reserved operator commands cannot land as source-level subcommands by
  accident. Doc-only unshipped operator command families are now meta-audited
  too: every `UNSHIPPED_*_DOC_COMMANDS` constant must use the shared
  boundary-aware matcher, including a left boundary that rejects prefixed fake
  command tokens, have negative controls, and feed a docs exposure test with a
  fail-closed `violations == {}` assertion. The shared doc-command matcher now
  also runs adversarial samples for every reserved doc-only command family,
  rejecting `x...`, `not-...`, `/...`, `/internal/...`, and canary/evidence/
  local/fixture suffixed fragments so diagnostic command text cannot satisfy an
  exact warning-only exposure check. Those warning-only command scans now cover
  top-level SoraFS plans under `specs/` and nested `specs/sorafs/**` docs, so
  reserved operator commands cannot be published through the repository-local
  implementation-coupled documentation. Public and localized mirrors belong
  in the sibling `iroha-docs` repository. It now also pins deployed AI
  pre-screening workflow promotion
  surfaces for deployed runner/committee promotion, deployed juror notification
  transport, deployed commit/reveal executor, end-to-end release workflow, and
  AI pre-screen promotion as unshipped with negative controls preserving the
  shipped local runner, committee, operator, notification, executor,
  transparency, Governance DAG, and rollout evidence tooling. Its route scanner
  now uses the same reusable segment-aware matcher as the other SoraFS
  unshipped service guards, so deployed-runner, deployed-committee, deployed
  notification/executor, workflow, and promotion canary/evidence suffixes stay
  local while exact deployed route stems remain blocked. The static contract now
  also scans CLI sources for nested deployed-only spellings such as
  `moderation runner promote`,
  `moderation juror-notification-transport service`,
  `moderation commit-reveal-executor service`, `ai-prescreen release workflow`,
  and `ai-prescreen promote`, while preserving local quarantine workflow,
  notification canary, operator canary, and `moderation ballots` executor
  commands. The local moderation operator service parser
  now rejects POST
  mutation requests that omit `Content-Length` and rejects undeclared body bytes
  on any request; the parser and TCP reader also reject trailing bytes after a
  declared `Content-Length` body instead of truncating them, keeping ambiguous
  raw HTTP bodies from reaching workflow routes or CSRF handling.
  Remaining rollout work stays focused on captured deployed juror notification
  transport service rollout evidence, captured deployed commit/reveal executor
  job rollout evidence, and a live
  ingest/quarantine/appeal/transparency evidence bundle that passes the gate
  rather than local
  catalog, metadata readback, registry-admission/checkpoint/API/CLI hardening,
  standalone persistent model-registry service, local screening/quarantine
  evidence persistence and API state transitions, deterministic local runner CLI
  output, locked-manifest local HTTP runner service mode, supervised HTTP runner
  bundle generation, production unary gRPC runner service, local committee
  aggregation CLI, local committee aggregation HTTP service, HTTP runner canary
  rollout evidence tooling, supervised committee bundle generation, HTTP
  committee canary rollout evidence tooling, local moderation ballot
  readback/commit/reveal/tally client and CLI
  bridge, local transparency readback/source-entry client and CLI bridge, local
  appeal pricing/deposit/readback client and CLI bridge, local
  reviewed-quarantine appeal handoff and appeal-ballot API/CLI, local
  screening-result submit/list CLI, local encrypted quarantine object
  envelopes/API/CLI, local quarantine CLI queue/review/release commands, the
  local operator-panel read model, local bridge-plan CLI, local payload-free
  operator workflow service, local signed operator workflow mutation
  forwarding, local payload-free juror notification planning, local
  payload-free juror notification delivery manifests, local payload-free
  juror notification outbox/webhook delivery CLI automation, local
  payload-free juror notification transport canary evidence tooling, local
  payload-free commit/reveal coordination status, local commit/reveal executor
  CLI automation, local supervised commit/reveal executor job bundle
  generation, local commit/reveal executor canary evidence tooling, local
  operator workflow canary evidence tooling, the local AI pre-screening rollout
  evidence gate, collection planner, and payload-free canary artifact builder,
  the local quarantine operator role gate, or the documented production
  role-provisioning runbook.


<a id="record-dbc2982e555f7e5ae1de5756ea152cf03935a88d7047476383cee8ceeddf83ef"></a>

- SFM-4 gateway compliance has one payload-free rollout evidence gate.
  `scripts/check_sorafs_gateway_compliance_rollout_evidence.py` validates
  `catalog_promotion`, controller runtime, moderation toggles, gateway reload,
  enforcement probes, honey-audit probes, precedence, SFM-4c transparency
  publication, observability, and governance approval before reporting
  `ready`. The catalog promotion is the sole anchor: it binds
  `catalog_digest_hex`, `catalog_entry_count`/`catalog_entries`,
  `catalog_change_count`/`catalog_changes`,
  `predecessor_catalog_digest_hex`, `predecessor_catalog_sequence`, and
  independently administered `gateway_acknowledgements`. Every downstream
  artifact binds that same `catalog_digest_hex`; controller and reload evidence
  additionally bind the promotion's exact predecessor digest and sequence.
  Summaries expose `valid_catalog_digests` and the atomic
  `valid_catalog_history_bindings` object, and mixed, missing, stale,
  duplicate, or predecessor-inconsistent anchors fail closed.
  Enforcement evidence must show HTTP 451
  `gateway_compliance_denied` with a bounded decision source of
  `baseline` or `legal_safety_hold`; the required two-source coverage is derived
  from route records. Honey-audit coverage is likewise derived from its probe
  records and requires all four canonical attacks. Unavailable, stale,
  conflicting, or poisoned controller state is a distinct fail-closed service
  error. Evidence
  never contains raw feeds, catalogs, request/response bodies, appeal payloads,
  signed transactions, tokens, credentials, or signing material. The matching
  collection planner and canary builder accept the canonical
  `min_catalog_entries` and `min_catalog_changes` thresholds and reject every
  retired kind, field, option, route, and competing local policy tool.
  The governed controller is constructed in Torii `AppState`; six canonical
  X-Iroha-signed, role-gated feed/status/stage/acknowledge/promote/rollback
  routes expose it, and manifest/CID/provider serving evaluates only the
  promoted catalog across global, configured-region, and configured-gateway
  scopes. ACME and feed transport now have separate exact non-secret
  `iroha_config` handle/revision/policy-digest bindings; startup and every
  ACME-order/DNS/HTTPS operation reject unavailable, substituted, stale,
  malformed, or test-marked providers before accepting returned material.
  Standard `irohad` no longer constructs an in-process compliance-feed fallback;
  configured compliance and ACME bindings must resolve to the exact
  deployment-owned injected adapters before Torii startup.
  Remaining SFM-4 production work is independently audited
  standard-daemon feed and ACME adapters, finalized precedence/hold catalog
  producers, external threshold signing, deployed SFM-4c receipt publication,
  and genuine staged evidence from both independently administered regional
  gateways.

