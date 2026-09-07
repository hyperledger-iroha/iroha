# Historical project evidence

This page is historical evidence, not current release readiness.
See [the archive index](../../index.md) for provenance and reconstruction.


<a id="record-14033c17f06168029e7a92295c21099a6d16645072bf359e664fd04f38173452"></a>

<!-- Original context: Roadmap / Release and Stabilization -->
- SoraFS SF-14 PoTR-Lite is wired locally for ranged gateway receipt capture,
  embedded-node receipt recording, `sorafs_manifest::potr` receipt validation,
  and exact finalized-outcome lookup through `/v1/sorafs/proof/stream` with
  `proof_kind=potr`. The SF-14
  rollout evidence gate now validates payload-free multi-provider probe,
  receipt-validation, proof-stream, reputation-integration, observability, and
  governance-approval evidence, and requires validation/proof-stream/reputation/
  observability/governance artifacts to bind back to a valid multi-provider
  probe `receipt_summary_digest_hex` in the same evidence bundle, with binding
  failures marked on the offending artifact through the shared scalar binding
  error recorder before required-kind summary validity is reported. The
  multi-provider probe now also binds `tier_count` to the unique canonical
  `tiers_observed` inventory, binds `provider_count` to the unique canonical
  `providers[].name` inventory, and binds `receipt_count` to the unique canonical
  `receipts[].name` inventory, rejecting duplicate or unknown tier labels plus
  duplicate provider or receipt labels before readiness can report ready, while
  provider inventory labels must use reviewed lowercase `provider-*` IDs without
  non-production markers and receipt inventory labels must use reviewed
  lowercase `potr-receipt-*` labels without non-production markers; the
  matching collection planner accepts reviewed staged
  evidence paths, supports `@ARGFILE`, forwards age,
  route-latency, hot/warm deadline, provider-count, and receipt-count
  thresholds, and emits a dry-run-visible verifier command, checker-backed
  `evidence_contract` map for the selected required kinds, plus operator
  example args; the gate now requires route latency and hot/warm deadline
  latency to be non-negative integer-unit evidence before those thresholds can
  pass. The PoTR gate now also has missing-field regressions proving raw
  receipt/transcript/reputation-input flags, `response_bodies_included`, and
  `critical_alerts_firing` must be explicitly encoded as `false`. It now validates the schema-closed collection-plan envelope,
  plus canonical nested required-kind, threshold, external-evidence,
  checker-backed evidence-contract, and command-step shapes before dry-run
  output or verifier execution.
  `scripts/build_sorafs_potr_canary.py` builds payload-free
  multi-provider-probe, receipt-validation, proof-stream,
  reputation-integration, observability, and governance-approval canary
	  artifacts through the same checker before rollout review, requiring complete
	  tier, route, and metric coverage, pre-write duplicate or unknown tier/route/
	  metric input rejection, derived `tier_count` for reviewed hot/warm tier
	  coverage with unknown tier rejection, duplicate or unknown metric-row
	  rejection, reviewed provider and receipt inventories
  whose unique names match their scalar counts, with receipt labels using the
  reviewed `potr-receipt-*` production family before writing, binding proof-stream
  `route_count` to the unique canonical route-name inventory so duplicate or
  unknown route rows cannot inflate readiness, requiring every proof-stream route
  response to carry a `body_blake3_hex` digest, plus receipt-summary,
  PQ key-roster, and
  reputation-weight policy digest bindings, governance policy-digest metadata,
  and deadline threshold facts before writing. The SF-14 gate summary now also
  publishes governance approval `policy_digest_hex` values as
  `valid_policy_digests` so aggregate production readiness can tether PoTR
  promotion policy metadata to recognized governance artifacts, exports the
  reviewed observability `metrics` inventory plus `metric_count_values`, and
  requires the aggregate gate to tether both metric fields to observability
  artifact fingerprints before final promotion can report ready. The aggregate
  production-readiness gate now also rechecks receipt-summary-bound,
  PQ-key-roster-bound, and reputation-weight-bound artifact fingerprints against
  `valid_receipt_summary_digests`, `valid_pq_key_roster_digests`, and
  `valid_reputation_weight_policy_digests` before final promotion can report
  ready. The lane checker also has direct adversarial coverage that forges the
  receipt summary digest on every receipt-summary-bound downstream kind, plus
  PQ key-roster and reputation-weight policy digests on their bound artifacts,
  so validation, proof-stream, reputation, observability, and governance
  evidence all fail against detached probe/governance anchors before promotion.
  Local receipt capture, validation, and exact finalized proof-stream lookup are implemented,
  and latency breaches use a deterministic exactly-once native repair identity
  through the durable transaction handoff. SF-14 is not evidence-only: it
  still requires removal of residual local repair projections, cross-peer
  finalized reconciliation, live multi-provider receipt evidence, governed
  provider ML-DSA key
  distribution, reputation weighting evidence, and governance approval that
  passes this gate with receipt-validation artifacts
  bound to governance-approved `pq_key_roster_digest_hex` values and
  reputation artifacts bound to governance-approved
  `reputation_weight_policy_digest_hex` values. The rollout-gate static contract now also
  pins live multi-provider probe rollout, governed provider key-distribution,
  reputation-weight governance, SF-14 approval, and PoTR promotion routes or
  subcommands as unshipped with reusable matchers and segment-aware negative
  controls. It now also scans CLI sources for nested deployed-only
  `potr live-probes`, `potr multi-provider-probes`, `potr live-rollout`,
  `potr provider-key-distribution`, `potr ml-dsa-keys`,
  `potr pq-provider-keys`, `potr reputation-weights`,
  `potr governance-approval`, `potr promote`, and `proof stream potr live`
  spellings while preserving ranged gateway receipt capture, `Sora-PoTR-*`
  headers, embedded-node receipt recording, local receipt validation,
  exact finalized lookup through `/v1/sorafs/proof/stream` with
  `proof_kind=potr`,
  `sorafs_cli proof stream --proof-kind=potr`, proof-stream metrics,
  deadline-breach alert fixtures, and payload-free canary evidence labels.


<a id="record-1ab70044ed40e26f3ad8a5a40f9d937d96a994b6e46fd65d1936458d5f8d2cd1"></a>

- SoraFS provider admission observability now has a checked-in Grafana board
  (`dashboards/grafana/sorafs_provider_admission.json`) plus Prometheus alert
  rules and test vectors for missing admission envelopes, stale admission
  material, policy-reject spikes, and downgrade warnings. The SF-2b dashboard
  and alert placeholder is closed; keep new admission failure reasons mirrored
  in the dashboard variables, alert tests, and rollout docs when Torii adds
  labels.


<a id="record-d6335bf931f1b641e10e19514574a97be235b2350e46d2e5b8eedf1eed578cbb"></a>

- SoraFS SF-2d provider advert integration docs now reflect the implemented
  range-fetch state: provider discovery exposes parsed range metadata, CAR and
  chunk range endpoints enforce stream-token validation plus
  quota/byte-rate/concurrency guards, and the range-fetch telemetry metrics feed
  the SoraFS fetch dashboard. The local `/v1/sorafs/providers` discovery list
  and configured `/v1/sorafs/storage/peers` publish-discovery readback now also
  accept `limit` (default 50, max 500), preserve full configured/cache counts,
  and emit `returned_count` plus `truncated` metadata for bounded
  inventory/readback scripts. `sorafs_provider_advert` now emits advert,
  public-key, signature, and JSON report files through the same no-follow
  descriptor writer used by the other release CLIs, with output leaves and
  parent chains inspected before parent creation and non-regular opened targets
  rejected before bytes are written. The stale
  scheduler-telemetry/token-integration remaining-work note is closed.


<a id="record-ba4d746a542df760165cb30a1a356d0c56b9dd71a1c3c51f4aaab4e5d3341c91"></a>

- SoraFS SF-5a gateway load promotion now has a fail-closed rollout evidence
  gate: `scripts/check_sorafs_gateway_load_rollout_evidence.py` validates
  payload-free signed local conformance, live staging load, telemetry/SLO,
  transport-scope, and governance approval artifacts. The gate binds live
	  staging evidence back to the signed local conformance digest, requires
	  telemetry and governance artifacts to reference the staged load report,
	  requires governance approval `policy_digest_hex` to match a valid
	  staging-load `policy_digest_hex`, and the aggregate production-readiness
	  gate now rechecks suite-bound, staging-bound, and policy-bound artifact
	  fingerprints against `valid_suite_report_digests`,
	  `valid_staging_report_digests`, and `valid_policy_digests` before final
	  promotion. The lane checker also has direct adversarial coverage that
	  forges the staging report digest on every staging-bound downstream kind, so
	  telemetry/SLO, transport-scope, and governance artifacts all fail against a
	  mismatched staged-load report before final promotion. The gate requires
	  local conformance `scenario_count`
  to match the unique canonical scenario inventory, rejects duplicate or
  unknown scenario entries before required-kind validity is reported, requires
  local-conformance `cargo_command` evidence to match a reviewed gateway
  conformance command, requires staging-load `stream_count` and
  `provider_count` to match the unique canonical
  `streams[].name` and `providers[].name` inventories, rejects duplicate stream
  or provider entries before required-kind validity is reported, requires
  staging-load `gateway_version` evidence to match a concrete `iroha-gateway`
  release or release-candidate label, requires generated
  `gateway-load-stream-*` stream labels,
  reviewed `gateway-load-provider-*` provider labels and
  `gateway-load-hardware-*` hardware-profile labels without placeholder/test
  markers, and reviewed
  `cold-cache`/`warm-cache`/`mixed-cache` cache-state modes, binds
  staging-load `success_rate_bps` to positive integer basis-point evidence
  capped at `10000`, caps `error_rate_bps` and operator success/error bps
  thresholds at `10000`, and binds `p95_latency_ms` and `p99_latency_ms` to
  non-negative integer-unit SLO evidence before applying rollout ceilings,
  binds
  telemetry/SLO `metric_count` to the reviewed gateway-load metrics inventory,
  and rejects duplicate or unknown metric labels before telemetry evidence can
  report ready,
  rejects raw reports/response bodies/fixture payloads/runtime secrets, now has
  missing-field regressions proving payload-safety fields and non-applicable
  HTTP/3 booleans must be explicitly encoded as `false`, and keeps HTTP/3 load
  evidence explicitly non-applicable to V1; any later HTTP/3 transport project
  is separately scoped. `scripts/run_sorafs_gateway_load_rollout_evidence.py`
  emits the matching collection dry-run plan and evidence contract, and now
  validates the schema-closed collection-plan envelope plus canonical nested
  required-kind, threshold, external-evidence, checker-backed evidence-contract,
  and command-step shapes before dry-run output or verifier execution, while the
  new `scripts/build_sorafs_gateway_load_canary.py` helper builds payload-free
  checked-in canary artifacts for each SF-5a gate kind, requires complete
  deterministic scenario and gateway metric coverage where applicable, enforces
  reviewed `gateway-load-provider-*` staging-provider inventory, reviewed
  hardware/cache staging metadata, rejects `--http3-endpoint-committed` because
  HTTP/3 is outside the V1 contract, and generated
  per-stream inventory,
  suite/staging digest bindings, and SLO threshold facts before writing,
  rejects out-of-range `--success-rate-bps` values before staging-load evidence
  is written,
  rejects placeholder or malformed `--gateway-version` values before
  staging-load evidence is written,
  rejects unreviewed `--cargo-command` values before local conformance evidence
  is written,
  validates every generated artifact through the gateway-load checker, and
  ships local conformance and staging-load response-file examples. The
  static rollout contract now uses reusable route/CLI matchers with
  segment-aware negative controls to keep live gateway-load, staging-load,
  HTTP/3, soak, and promotion route/subcommand surfaces unshipped until
  deployed evidence passes this gate, now also scanning nested deployed-only
  `gateway load live`, `gateway load staging`, `gateway load http3`,
  `gateway load promote`, and `gateway load soak` CLI spellings while
  preserving local conformance, staging-canary/evidence, transport-scope
  canary, promotion-evidence, and payload-free gateway-load canary labels.


<a id="record-e192e4054b1589536d0335c40045fd3e13d2be03a13d10815c0d0dc7e146e118"></a>

- SoraFS Pin Registry validation and admission now use the native first-release
  hard cut. `POST /v1/sorafs/pin/register` accepts only a canonical versioned
  `SignedTransaction` for the exact runtime `NetworkId`, verifies its authority
  signature, and requires exactly one `RegisterPinManifest`. The instruction
  carries canonical `ManifestV1` bytes plus optional alias and non-zero
  predecessor; there is no JSON registration DTO or client-supplied lifecycle
  epoch. Core revalidates the manifest, derives submission, approval, and
  retirement epochs from block consensus time, enforces global/per-account
  count-and-byte ceilings plus lineage depth/fanout, maintains authenticated
  expiry and lifecycle-status indexes, and collects the public-pin fee.
  Submitter retirement or consensus-time expiry releases only live-content byte
  charges; retained-record counts and successor fanout remain charged while the
  lifecycle evidence remains in consensus state. Pinning is a paid public
  operation for authenticated accounts without a general pin
  permission token; aliases retain `CanBindSorafsAlias`, threshold approval
  authority comes from the bounded council envelope that any authenticated
  account may relay, and retirement is submitter-only. Rust, Kotlin/mirrored
  Java, JavaScript, Python, Swift, and C# builders reject retired epoch/DTO keys
  and emit the same native wire. `GET /v1/sorafs/pin` returns
  `PinManifestPageV1`: bounded summaries at one finalized height/hash, row and
  encoded-byte ceilings, an exclusive digest continuation key, and O(1)
  consensus-maintained `charged_usage`; offset/full-registry materialization is
  absent. `GET /v1/sorafs/pin/{digest_hex}` returns one exact bounded
  `PinManifestFinalizedRecordV1`, with alias and replication lists remaining
  separate. Prometheus reads only the maintained O(1) global retained-record
  and live-content summary; scan-derived inventory gauges are absent and the
  finalized query remains authoritative quota evidence. Keep future SF-4 work
  focused on source validation and rollout/production evidence.


<a id="record-a48166726d34f73cd0fcb2436926b1da3831f1f5856cc7a10a364b984d494b29"></a>

- SoraFS pricing docs now reflect the implemented egress accounting path:
  `RecordCapacityTelemetry.egress_bytes` is charged through
  `PricingScheduleRecord::egress_charge_bytes_nano`, recorded in the capacity
  fee ledger, folded into expected settlement, and debited from provider credit
  alongside storage fees. Operational reconciliation is wired through optional
  gateway/orchestrator telemetry counters, `torii_sorafs_egress_bytes`,
  `torii_sorafs_egress_drift_ratio`, the capacity dashboard drift panels, and
  the `SoraFSEgressCounterDrift` alert for sustained gateway/orchestrator drift.
  Relay treasury reconciliation now also reports negative, too-wide, or
  otherwise unrepresentable XOR amounts as explicit conversion errors instead
  of panicking or contributing a misleading zero to operator totals, and the CLI
  exposes those errors with nullable per-transfer `amount_nanos`. The incentive
  shadow-run operator summary uses the same checked conversion path and records
  malformed payout amounts in `payout_amount_conversion_errors` instead of
  silently folding them into payout totals as zero.


<a id="record-fde233944bcdece0c767c3143c028045b5a7d508f1f9df0787e089ebf9ce545e"></a>

- SoraFS capacity-state readback hardening is now shipped locally:
  `/v1/sorafs/capacity/state` accepts `limit` (default 50, max 500), bounds
  declarations, fee-ledger entries, credit-ledger entries, and disputes before
  JSON serialization, and preserves full totals with returned-count and
  truncation metadata for each array. Remaining production work stays focused
  on durable contract-backed capacity/dispute flows, reconciliation evidence,
  dashboard rollout, and live deployment validation.


<a id="record-cf5d34e222f15dbcfd0556e42c447a60557e9a8e56bb3aa977b2acaa58e45a39"></a>

- SoraFS embedded-storage metadata readback hardening is now shipped locally:
  `/v1/sorafs/storage/manifest/{manifest_id}` accepts an optional `limit`
  (max 500) to bound returned file descriptors while omitting `limit` preserves
  the complete file list required by remote gateway cache fetches; the response
  carries full file counts, returned counts, and truncation metadata either way.
  `/v1/sorafs/storage/plan/{manifest_id}` bounds `files`,
  `chunk_digests_blake3`, and `chunks` by `limit` (default 50, max 500) while
  preserving full plan counts and returned/truncation metadata for operator
  probes.

