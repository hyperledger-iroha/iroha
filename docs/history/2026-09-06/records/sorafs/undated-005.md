# Historical project evidence

This page is historical evidence, not current release readiness.
See [the archive index](../../index.md) for provenance and reconstruction.


<a id="record-7c5ac73c138fef5b808da5949e26597eed9364ca9da95aa2ab9016e8300af4ff"></a>

<!-- Original context: Roadmap / Release and Stabilization -->
- SoraFS economics/governance plan status is current for the remaining local
  production gaps: SFM-2 now has initial orderbook/streaming-settlement Norito
  payloads and validators in `sorafs_manifest::orderbook` plus Rust reference
  validator, reference FFI selectors, committed fixtures including bundle
  validation for runtime replay snapshots,
  deterministic pair and full-book snapshot matching/fee/settlement helpers,
  deterministic generated matcher invariant and permutation-stability coverage,
  canonical Norito local orderbook runtime replay snapshots with
  storage-data-dir checkpoint reload, committed fixture parity, and
  trade/receipt replay validation that rejects channel byte-total drift,
  remaining-byte drift, escrow drift, and out-of-channel receipt ranges,
  Rust Ed25519 signing helpers, shared encoded-payload signing through
  `sorafs_manifest::sign_orderbook_payload_bytes_ed25519_v1`,
  `sorafs-validate sign --kind orderbook` CLI signing, and
  JavaScript/Python/Kotlin/JVM/Java Android/Swift SDK
  `signOrderbookPayload` / `sign_orderbook_payload` wrappers for
  already-encoded order/cancel/receipt payload bytes, plus Rust/JavaScript/
  Python/Kotlin/JVM/Java Android/Swift field-level signed order/cancel/receipt
  payload builders,
  target dashboard/alert fixtures, Prometheus metric handles/helper methods for
  the `torii_sorafs_orderbook_*` families, `sorafs-validate orderbook` CLI
  coverage, JavaScript, Python, Kotlin/JVM, Java Android, and Swift SDK
  orderbook reference-validation bindings, JavaScript and Python Torii read
  helpers for local orderbook book/trades/channels/receipts/events, JavaScript
  and `iroha_python` local orderbook SSE/WebSocket stream helpers, JavaScript,
  `iroha_python`, and standalone `iroha_torii_client` local submit helpers for
  already signed Norito order/cancel/receipt bytes, a local in-memory `sorafs_node` orderbook mirror, local Torii
  order/cancel/receipt/book/trade/channel/event routes with `limit`-bounded
  book/trades/channels/receipts readbacks and full total counts, local settlement
  receipt application with duplicate-id and overlapping-range rejection,
  local orderbook settlement receipt Governance DAG publication, replayable
  local orderbook event history, local SSE/WebSocket event streams with
  frame-shape coverage,
  embedded Ed25519 payload signature digests/verification with local runtime
  enforcement, local request-authenticated orderbook POST envelope/account/
  signer binding, local known-channel receipt provider-role authorization,
  local provider-advert capability authorization for asks and known-channel
  receipts, local config-backed order admission policy for minimum order
  quantity and price tick, and local runtime metric emission for order flow,
  depth, matcher lag, settlement
  backlog, escrow runway, API error ratios, and mirror divergence. Local
  snapshots now also expose a deterministic `OrderbookSettlementLedger` derived
  from accepted receipt and channel state so replay/checkpoint restores can
  prove buyer debits, provider credits, retained fees, and remaining locked
  escrow without changing the canonical replay payload. The SFM-2
  rollout evidence gate now validates payload-free contract surface, durable
  matcher service, streaming-settlement service, authenticated API gateway,
  durable event streams, SDK release, observability, contract/mirror
  reconciliation, and governance approval evidence, requires matcher,
  settlement, API, stream, SDK, observability, reconciliation, and governance
  approval artifacts to carry a `contract_digest_hex` matching a valid
  contract-surface artifact in the same rollout bundle. The lane checker also
  has direct adversarial coverage that forges `contract_digest_hex` on every
  contract-bound downstream kind, so matcher, settlement, API, stream, SDK,
  observability, reconciliation, and governance evidence all fail against a
  mismatched contract surface before promotion. It requires
  contract-surface artifacts to carry `policy_digest_hex`, publishes valid
  contract-surface policies as `valid_policy_digests`, requires governance
  approval `policy_digest_hex` to match one of those valid policy digests, and
  binds matcher `accepted_order_count` and `matched_order_count` to reviewed
  duplicate-free `accepted_orders` and `matched_orders` inventories while
  requiring matched orders to come from the accepted-order set and order IDs to
  use reviewed lowercase `orderbook-order-*` labels without non-production
  markers, binds `rejected_invalid_order_count` to reviewed duplicate-free
  `rejected_invalid_orders` using the same order label family, binds settlement
  `open_channel_count`, `settled_receipt_count`, and
  `settlement_backlog_count` to reviewed duplicate-free
  `open_channels`, `settled_receipts`, and `settlement_backlog_channels`
  inventories with reviewed lowercase
  `orderbook-channel-*`/`orderbook-receipt-*` labels without non-production markers, binds API gateway
  `route_count` to the unique canonical `routes[].name` inventory, requires every
  API route response to carry a `body_blake3_hex` digest, plus reconciliation
  `peer_count` and `source_count` to the unique
  canonical `peers[].name` and `sources[].name` inventories with reviewed
  lowercase `orderbook-peer-*` labels without non-production markers so duplicate route,
  order, channel, receipt, peer, or source rows cannot inflate readiness and
  unknown route or source labels cannot expand the reviewed evidence surface,
  binds event-stream
  `stream_count` to the unique canonical `streams[].name` inventory before
  stream evidence can report ready while rejecting unknown stream labels, binds
  SDK release `language_count` to the unique canonical `languages[].name`
  inventory while rejecting unknown SDK language labels, binds SDK release
  `artifact_count` to the unique canonical `artifacts[].id` inventory so
  duplicate SDK language or artifact rows cannot inflate readiness, and
  requires at least one distinct SDK release artifact per reviewed SDK language
  before SDK release evidence can report ready, and now has consolidated
  missing-field regressions proving raw contract state/snapshot/receipt/ledger,
  response-body, divergence, critical-alert, and debug-artifact flags must be
  explicitly encoded as `false`, and requires
  the reviewed observability metric set, rejects duplicate or unknown
  observability metric rows, exports the reviewed `metrics` inventory plus
  `metric_count_values`, and requires aggregate production-readiness to tether
  both fields to observability artifact fingerprints before final promotion can
  report ready. The aggregate production-readiness gate now also rechecks
  contract-bound and policy-bound artifact fingerprints against
  `valid_contract_digests` and `valid_policy_digests` before final promotion
  can report ready, and the rollout checker marks
  contract-digest and policy-digest mismatches on the offending artifact through
  the shared scalar binding error recorder before required-kind summary validity
  is reported. The
  matching collection planner accepts reviewed
  staged evidence paths, supports `@ARGFILE`, forwards
  age, route-latency, stream-lag, matcher-lag, and reconciliation-peer
  thresholds, requires `routes[].latency_ms`, route `body_blake3_hex`,
  matcher `matcher_lag_ms`, and event-stream `lag_ms` evidence before those
  ceilings apply, and emits a dry-run-visible verifier command plus operator
  example args. It now also mirrors the checker's required payload fields in
  dry-run `evidence_contract` output so operators can preflight live orderbook
  artifact shape before collection, and validates the schema-closed
  collection-plan envelope plus canonical nested required-kind, threshold,
  external-evidence, checker-backed evidence-contract, and command-step shapes
  before dry-run output or verifier execution. A new
  `scripts/build_sorafs_orderbook_canary.py` helper
  builds payload-free SFM-2 canaries for every orderbook evidence kind from
  reviewed deployment facts, requires every positive proof claim and required
  coverage set explicitly, forces raw contract/snapshot/receipt/response/ledger
  payload flags to `false`, requires reviewed `--policy-digest-hex` input for
  contract-surface and governance-approval canaries, requires route-bearing API
  gateway `--route-body-blake3-hex` evidence, requires reviewed
  matcher `--accepted-order`/`--matched-order`, settlement
  `--open-channel`/`--settled-receipt`, and reconciliation `--peer` labels
  whose unique inventory matches `--peer-count` and whose names use reviewed
  `orderbook-order-*`, `orderbook-channel-*`, `orderbook-receipt-*`, and
  `orderbook-peer-*` production shapes without non-production markers or
  cross-lane generic families, derives `language_count` from
  reviewed SDK language coverage, validates generated artifacts through the orderbook checker
  contract, rejects duplicate SDK release artifact ids, rejects SDK release
  canaries with fewer than one distinct artifact per reviewed SDK language or
  artifact ids outside reviewed SDK language prefixes,
  ships contract, API, and
  reconciliation response-file examples, and writes atomically without following
  output symlinks. The
  rollout-gate static contract now pins
  the on-chain contract, durable matcher, daemonized settlement, escrow-custody,
  contract-stream, and live-dashboard orderbook service surfaces as unshipped
  in the SFM-2 plan with reusable matchers and segment-aware negative controls
  while preserving shipped local order/cancel/receipt/book/trade/channel/event
  APIs, local SSE/WebSocket streams, `sorafs-validate orderbook`, local
  submit/read helpers, and payload-free canary evidence labels. It also scans
  CLI sources for nested deployed-only `orderbook
  matcher-service|settlement-daemon|contract-submit|dashboard-serve` spellings
  without blocking shipped local `orderbook orders|cancel|receipts|book|trades`
  commands,
  but still needs the on-chain contract surface, durable matcher service,
  daemonized settlement receipt service with contract/on-chain escrow custody
  mutation,
  on-chain/governance-backed admission policy, contract-backed capability
  policy authorization, contract forwarding,
  durable contract/matcher-backed WebSocket/SSE streams,
  SDK release artifacts/live smoke evidence,
  live dashboard wiring and alert routing,
  contract/mirror reconciliation tests, and staged/live evidence that passes
  this gate;
  SFM-4b1 proof-of-personhood credentials now have local
  `sorafs_manifest::pop_credentials` payload foundations:
  `PopCredentialV1`, commitment-root, revocation-list, enrollment, renewal, and
  membership-proof schemas use canonical Norito, credentials/roots/revocations
  have domain-separated Ed25519 signing helpers, and
  `PopIssuedCredentialBundleV1` plus `issue_pop_credential_bundle_ed25519_v1`
  now form a local issuer-publication bundle that checks issuer id/public key,
  root, tree, revocation-list, and revoked-nonce consistency.
  `validate_pop_payload_bytes`
  plus `sorafs-validate pop` provide local reference/CI diagnostics, the public
  `sorafs_reference.h` C header now mirrors the PoP selector constants and
  `sorafs_reference_validate_pop_json` ABI contract, the
  SoraFS C/JNI bridge now exposes `connect_norito_sorafs_reference_validate_pop_json`
  with Kotlin/JVM, Java Android, and Swift selector wrappers, and the focused
  local tests cover Norito roundtrips plus production fail-closed rejection for
  transcript-digest-only proofs, local transcript-policy verification, expired
  credential, revoked nonce, wrong root, stale revocation list, replayed proof,
  transcript tamper, forged signature rejection, and reference/bridge validator
  acceptance/rejection. The SFM-4b1 rollout evidence gate now validates
  payload-free issuer-bundle,
  commitment-root, revocation-registry, enrollment-portal, juror-client,
  verifier-service, moderation-integration, metrics/alert, and
  governance-approval artifacts, rejects raw credentials/proofs/identities and
  stale registry publications, requires `iroha_config` governance binding,
  requires reviewed `deployment_id`/`environment` context on every artifact,
  requires issuer-bundle `issuer_id` to use a canonical lowercase `pop-issuer-*`
  label without non-production markers,
  requires the issuer bundle, published commitment root, revocation registry,
  juror sync, verifier service, moderation integration, metrics, and governance
  approval to agree on the active root/revocation-list digests, marks root and
  revocation disagreements on the offending artifacts through the shared
  scalar binding error recorder, requires verifier-service artifacts to carry
  `policy_digest_hex`, publishes valid verifier policy digests as
  `valid_policy_digests`, publishes valid juror-client root/revocation sync
  tuples as `valid_juror_sync_bindings`, publishes moderation
  `pop_snapshot_digest_hex` values as `valid_pop_snapshot_digests`, requires the
  aggregate production-readiness gate to tether both new metadata surfaces to
  recognized artifact fingerprints and recheck `valid_juror_sync_bindings`
  roots/revocations against `valid_root_digests` and
  `valid_revocation_list_digests`, requires aggregate promotion to recheck
  root-bound, revocation-bound, and policy-bound artifact fingerprints against
  `valid_root_digests`, `valid_revocation_list_digests`, and
  `valid_policy_digests`, and rejects mixed-anchor rollout summaries unless
  exactly one active root digest, revocation-list digest, verifier policy
  digest, and moderation PoP snapshot digest are established. The lane checker
  also has direct adversarial coverage
  that forges every root-bound, revocation-bound, and policy-bound downstream
  digest, so juror sync, verifier, moderation, metrics, and governance evidence
  all fail against detached root, registry, or verifier-policy anchors before
  promotion. It requires governance approval
  `policy_digest_hex` to match one of those valid verifier policies, binds
	  issuer-bundle `credential_count` to the unique canonical `credentials[].name`
	  inventory with reviewed lowercase `pop-credential-*` labels without
	  non-production markers, binds revocation-registry `revoked_nonce_count` to
	  the unique canonical `revoked_nonce_refs[].name` inventory with reviewed
	  lowercase `pop-revoked-nonce-*` labels while keeping raw nonce payloads
	  excluded, plus enrollment-portal and verifier-service `route_count` to the
	  unique canonical `routes[].name` inventory with unknown route rejection and verifier-service
	  `proof_probe_count` to the unique canonical `probes[].name` inventory with
  accepted/rejected proof counts bound to `probes[].accepted` partitions and
  partitioned reviewed `pop-valid-proof-*`/`pop-invalid-proof-*` labels without
  non-production markers, requires verifier-service artifacts to explicitly set
  `raw_proofs_included` and `holder_identity_disclosed` to `false`, and
  binds moderation-integration
  `sortition_probe_count` and `commit_reveal_probe_count` to the unique
  canonical `sortition_probes[].name` and `commit_reveal_probes[].name`
  inventories with reviewed lowercase `pop-sortition-probe-*` and
  `pop-commit-reveal-probe-*` labels without non-production markers, so duplicate credential, route, proof-probe, or moderation-probe
	  rows cannot inflate readiness, binds metrics/alert `metric_count` to the
	  unique canonical reviewed metrics inventory with unknown metric rejection,
	  requires metrics/alert artifacts to explicitly set `critical_alerts_firing`
	  and `response_bodies_included` to `false`,
	  now has consolidated missing-field regressions proving
	  `credential_payloads_included`, `holder_identities_included`,
	  `credential_leaves_included`, `rollback_detected`,
	  `revoked_nonces_included`, `pii_fields_included`,
	  `attestations_included`, `holder_identity_included`,
	  `proof_payloads_included`, `raw_proofs_included`,
	  `holder_identity_disclosed`, `identity_payloads_included`,
	  `critical_alerts_firing`, and `response_bodies_included` must be
	  explicitly encoded as `false`,
	  exports the reviewed metrics inventory plus `metric_count_values`, and
	  requires aggregate production-readiness to tether both fields to the
	  metrics/alert artifact fingerprint before final promotion can report ready,
	  and blocks promotion when governance still
  points at the local
  transcript-digest-only proof foundation instead of a production
  privacy-preserving proof backend; the production
  `verify_pop_membership_proof_v1` API now mirrors that policy by rejecting
  `TranscriptDigestV1` until the selected privacy verifier lands. The matching
  collection planner accepts
  reviewed staged evidence paths, supports `@ARGFILE`, forwards freshness and
  latency thresholds, and emits a dry-run-visible verifier command,
  checker-backed `evidence_contract` map for the selected required kinds, plus
  operator example args; it now validates the schema-closed collection-plan
  envelope plus canonical nested required-kind, threshold, external-evidence,
  checker-backed evidence-contract, and command-step shapes before dry-run
  output or verifier execution. A new
  `scripts/build_sorafs_pop_credentials_canary.py` helper builds
  payload-free SFM-4b1 canaries for issuer bundle, commitment root, revocation
  registry, enrollment portal, juror client, verifier service, moderation
  integration, metrics/alerts, and governance approval evidence from reviewed
  deployment facts, requires every positive proof claim and
  credential/proof-probe/route/metric coverage set explicitly with closed claim,
  route, and metric inventories, rejects malformed, generic-family, or non-production
  `--issuer-id` values, rejects duplicate or unknown verified-claim,
  route, and metric inputs before writing, requires explicit
  `--route-body-blake3-hex` evidence for enrollment/verifier routes, requires reviewed
  verifier/governance policy-digest
  input, forces raw credential/proof/identity/attestation/response-body flags
  to `false`, rejects the transcript-digest-only privacy
  backend before writing, requires reviewed `--credential`,
  `--accepted-proof-probe`, `--rejected-proof-probe`, `--sortition-probe`, and
  `--commit-reveal-probe` labels whose unique inventories match the credential,
  verifier proof, and moderation-integration probe counts and whose names use
  reviewed `pop-credential-*`, `pop-valid-proof-*`, `pop-invalid-proof-*`,
  `pop-sortition-probe-*`, and `pop-commit-reveal-probe-*` production shapes
  without non-production markers, validates generated artifacts through the PoP checker
  contract, and writes atomically without following output symlinks. The
  rollout-gate static contract now also pins the
  production verifier's fail-closed transcript-digest boundary and scans SoraFS
  docs so unshipped `sorafs pop sync|status|prove|revoke` commands can appear
  only as explicit not-shipped warnings until the service CLI/API actually
  lands, using a boundary-aware command matcher so canary/evidence/local labels
  that share the command prefixes do not count as shipped docs. It also scans
  CLI sources for nested `pop sync|status|prove|revoke` spellings so those
  unshipped operator commands cannot be exposed as source-level subcommands
  while local canary/evidence labels remain allowed. The rollout-gate static
  contract now also pins the enrollment portal,
  credential issuer daemon, credential registry service, juror wallet/client,
  privacy-preserving proof generator, deployed verifier service, and PoP
  promotion routes or subcommands as unshipped with segment-aware negative
  controls that avoid matching local canary/readback-style names while preserving
  `sorafs-validate pop`, the local issued-credential bundle helper, reference
  SDK/bridge validators, and the fail-closed transcript-digest verifier. The
  remaining SFM-4b1 work is the privacy-preserving membership proof
  backend, issuer/registry services, juror client storage and proof generation,
  moderation sortition/commit-reveal integration, service CLI/API surfaces, and
  captured deployed rollout evidence that passes this gate;
  SFM-4b2 appeal finance now has deterministic orchestrator pricing/settlement
  helpers, CLI quote/settle/disburse commands, read-only Torii config,
  readiness, and quote endpoints for the baseline pricing formula, and
  stateless Torii settlement/disbursement plan endpoints for baseline finance
  reports. SoraFS Governance DAG now has typed appeal finance report payloads
  plus local filesystem publishing, CAR queue entries, publish-index labels, and
  optional signed runtime DAG blocks for those reports. `sorafs_manifest` also
  ships deterministic weekly appeal-finance rollup aggregation and validation
  for transparency dashboards, and `sorafs_node` can publish weekly rollups to
  the local Governance DAG filesystem sink, CAR queue, publish-index, and
  optional signed runtime DAG. Torii exposes local publish-index-backed report
  and weekly-rollup dashboard read endpoints. The caller-supplied report and
  weekly-rollup POST routes are a V1 hard cut: canonical account authentication
  does not make caller-provided finance totals authoritative. Production
  publication remains outstanding until a supervised worker derives reports
  from one immutable finalized view, derives rollups from the exact
  authenticated report set, and obtains the configured deployment-selected
  signing-provider signature.
  A checked-in Grafana/Prometheus appeal-finance dashboard and alert
  pack now covers report/weekly-rollup/settlement-receipt publication
  freshness, failures, payload throughput, rollup lag, receipt/report lag, and
  Governance DAG backlog.
  SoraFS reconciliation reports now embed local appeal-finance rollup summaries
  for treasury review. Torii now exposes a canonical-authenticated native
  `OpenAssetLock` instruction builder plus participant-gated runtime asset-lock
  status lookup and confirmation gate for appeal deposits, and the local
  moderation ballot announcement intake now requires a matching confirmed
  deposit before admitting a case. Torii also builds ordered native
  `DrawdownAssetLock`/`CancelAssetLock` settlement instructions for confirmed
  appeal deposit locks, and it can reconcile the current runtime asset-lock
  ledger state after settlement transactions with deterministic audit digests
  for peer/operator comparison. The configured-signer submitter durably queues
  and reconciles the next pending native settlement step when the required
  governed authority key is active, and publishes a typed
  `SoraFsAppealFinanceSettlementReceiptV1` to the local Governance DAG when a
  publisher is configured, with the signing/queueing/reconciliation/receipt
  publication path now factored into a reusable internal helper. Torii now also
  starts a moderation-derived settlement worker when SoraFS storage and
  configured submitter keys are available; it replays/subscribes to local
  `BallotTallied` events, reconstructs the moderation-captured deposit
  fingerprint including evidence hashes, validates the runtime ledger, and
  queues pending settlement steps through the same helper, with a configured
  follow-up scan interval plus persisted transaction hash, pipeline status,
  attempt count, and last-error tracking for unchanged ledger states across
  process restarts and rejected/expired worker transaction retries.
  Deposit-backed
  local moderation tallies now derive and publish deterministic
  `SoraFsAppealFinanceReportV1` records from the final decision, confirmed
  deposit snapshot, evidence-hash fingerprint, panel roster, revealed jurors,
  and no-show jurors, and Torii now serves local published report, weekly
  rollup, and settlement receipt summaries for operator dashboards with
  aggregate totals over the full local publish-index plus `limit`-bounded source
  entry arrays, and the checked-in observability pack now
  covers report/weekly-rollup/settlement-receipt throughput, freshness,
  failures, receipt/report lag, and local Governance DAG backlog. The SFM-4b2
  rollout evidence gate now validates payload-free pricing/config, quote API,
	  native deposit lifecycle, settlement execution, configured-signer submitter,
	  moderation-derived worker, Governance DAG publication, hosted dashboard,
	  multi-peer reconciliation, and governance approval evidence, rejects raw
	  instructions, signed transactions, response bodies, signer material, raw
	  reports/rollups/receipts, and raw ledgers, now has missing-field
	  regressions proving those payload-safety fields and
	  `critical_alerts_firing` must be explicitly encoded as `false`, requires reviewed
	  `deployment_id`/`environment` context on every artifact, requires
		  quote/deposit/settlement/
		  submitter/worker/Governance DAG/dashboard/reconciliation/governance artifacts
  to bind back to a valid pricing-config `config_digest_hex` in the same
  evidence bundle. The lane checker also has direct adversarial coverage that
  forges `config_digest_hex` on every config-bound downstream kind, so quote,
  deposit, settlement, submitter, worker, Governance DAG, dashboard,
  reconciliation, and governance evidence all fail against a mismatched pricing
  config before promotion. It requires aggregate `valid_multi_peer_runs.config_digest_hex`
  values to appear in `valid_config_digests`, and aggregate promotion now
  rechecks config-bound and policy-bound artifact fingerprints against
  `valid_config_digests` and `valid_policy_digests`, requires governance approval
  `policy_digest_hex` to match a valid pricing-config `policy_digest_hex`,
  publishes valid staged pricing
  policies as `valid_policy_digests`, records config- and policy-digest
  mismatches on the offending artifact through the shared scalar binding error
  recorder before required-kind summary validity is reported, binds
  pricing-config `class_count` to the reviewed canonical `classes` inventory,
  requires pricing-config `config_version` to use a canonical lowercase
  `appeal-finance-config-name-vN` label without non-production markers,
  rejects duplicate or unknown pricing class rows before promotion can report
  ready, binds quote-API, deposit-lifecycle, and settlement-execution
  `route_count` to the unique canonical `routes[].name` inventories so
  duplicate or unknown route rows cannot inflate readiness, requires every
  quote/deposit/settlement route response to carry a `body_blake3_hex` digest,
  requires route `latency_ms`, quote/deposit `max_route_latency_ms`, and
  submitter/worker `max_settlement_lag_seconds` to be integer-unit evidence
  before latency or settlement-lag ceilings can pass, binds
  deposit-lifecycle `deposit_probe_count` to the unique
  canonical `deposit_probes[].name` inventory with reviewed
  `appeal-finance-deposit-probe-*` labels without non-production markers and
  `confirmed_deposit_count` bound to the `deposit_probes[].confirmed`
  partition, binds quote-API
  `quote_count` and `passed_quote_count` to the product of unique `classes` and
  `urgencies` inventories so duplicate or unknown quote dimension rows cannot
  inflate readiness, binds settlement-submitter `configured_signer_count` to the unique
  canonical `signers[].name` inventory using reviewed
  `appeal-finance-submitter-signer-*` labels and `queued_step_count` to the unique
  canonical `steps[].name` inventory using reviewed
  `appeal-finance-submitter-step-*` labels, all without non-production markers,
  with `submitted_step_count` bound to `steps[].submitted`, binds moderation-worker
  `ballot_replay_count` to the unique canonical `ballots[].name` inventory
  using reviewed `appeal-finance-worker-ballot-*` labels without
  non-production markers, binds settlement-execution
  `settlement_probe_count` to the unique `outcomes` inventory and
  `instruction_step_count` to the unique `instruction_steps` inventory while
  rejecting duplicate or unknown outcome, instruction-step, or
  reconciliation-status rows,
  binds Governance-DAG
  publication `report_count`, `weekly_rollup_count`, and
  `settlement_receipt_count` to the unique canonical `reports[].name`,
  `weekly_rollups[].name`, and `settlement_receipts[].name` inventories using
  reviewed `appeal-finance-report-*`, `appeal-finance-weekly-rollup-*`, and
  `appeal-finance-settlement-receipt-*` labels without non-production markers
  so malformed, placeholder, or duplicate publication rows cannot inflate
  readiness, requires
  `payload_kind_count` on Governance-DAG publication and dashboard metrics
  artifacts, binds it to the unique canonical `payload_kinds` inventory, and
  rejects missing, inflated, duplicate, or unknown payload-kind evidence, binds
  dashboard `metric_count` to the unique canonical `metrics` inventory so
  duplicate or unknown metric rows cannot inflate readiness, exports the
  reviewed `metrics` inventory plus `metric_count_values`, and requires
  aggregate production-readiness to tether both fields to dashboard-metrics
  artifact fingerprints before final promotion can report ready, binds multi-peer
  reconciliation `peer_count`, `validator_count`, and `case_count` to the
  unique canonical `peers[].name`, `validators[].name`, and `cases[].name`
  inventories, requires those names to use reviewed `appeal-finance-peer-*`,
  `appeal-finance-validator-*`, and `appeal-finance-case-*` labels without
  non-production markers, requires `case_count` to match the
  `cases[].reconciled` partition, rejects duplicate peer, validator, or case
  rows so reviewed inventory cannot inflate readiness, and requires at least
  four peers before promotion can report `ready`. The matching collection
  planner accepts
  reviewed staged evidence paths, supports `@ARGFILE`, forwards freshness,
  latency, settlement-lag, and peer-count thresholds, and emits a dry-run-visible
  verifier command, checker-backed `evidence_contract` map for the selected
  required kinds, plus operator example args; it now validates the
  schema-closed collection-plan envelope plus canonical nested required-kind,
  threshold, external-evidence, checker-backed evidence-contract, and command-step
  shapes before dry-run output or verifier execution. A checked-in
  `build_sorafs_appeal_finance_canary.py` helper now builds payload-free
  pricing/config, quote, deposit, settlement, submitter, moderation-worker,
  Governance DAG, dashboard, multi-peer reconciliation, and governance canary
  artifacts with explicit verified claims, complete required route/class/
  urgency/outcome/status/payload/metric coverage, shared config-digest binding,
  pricing-config/governance policy-digest input, pricing-config `--class-count`
  bound to the reviewed class inventory, quote-API `--quote-count` bound to the
  reviewed class/urgency product, route-bearing `--route-body-blake3-hex`
  evidence, reviewed role-scoped `--peer`, `--validator`,
  and `--reconciliation-case` labels plus confirmed/unconfirmed deposit-probe
  labels and settlement-submitter
  signer/step labels plus Governance-DAG report/weekly-rollup/
  settlement-receipt labels whose unique inventories match the multi-peer,
  deposit lifecycle, submitter, and publication count fields, derived
  `payload_kind_count` fields for reviewed payload-kind inventories, forced
  raw-payload omission
  flags, malformed, generic-family, or non-production `--config-version`
  rejection, direct
  duplicate/unknown verified-claim, class, route, urgency,
  outcome, instruction-step, reconciliation-status, payload-kind, metric, and
  multi-peer label-family input regressions before writing, checker-backed
  prevalidation, atomic symlink-safe writes, and
  pricing-config, quote API, and multi-peer reconciliation example argfiles. The
  rollout-gate static contract
  now pins the standalone pricing daemon, hosted/public appeal-finance
  dashboard promotion surface, and multi-peer ledger reconciliation promotion
  surface as unshipped with reusable matchers and segment-aware negative
  controls while preserving local pricing config/status/quote, deposit
  lifecycle, settlement submission/reconcile, report/weekly-rollup/settlement
  receipt APIs, rollout evidence, and payload-free canary labels. It also scans
  CLI sources for nested deployed-only `appeals pricing daemon`, `appeals
  finance public-dashboard`, `appeals finance reconcile multi-peer`, and
  `appeals finance promote` spellings without blocking shipped local
  `appeals pricing config|status|quote` or `appeals finance deposits|reports`
  commands. SFM-4b2 still
  needs hosted live/public dashboard evidence and multi-peer
  end-to-end ledger reconciliation evidence that passes this gate;
  SFM-4b4 now has SoraFS-specific moderation ballot context/commit/reveal
  payloads in `iroha_data_model::sorafs::moderation` that bind case ids,
  evidence bundle digests, appeal finance config versions, panel roster hashes,
  policy references, and `uphold`/`overturn`/`modify`/`escalate` choices, plus
  a local `sorafs_node` ballot lifecycle runtime for announcements, commit
  windows, durable local challenge records with pending/accepted challenge
  blocking and rejected-challenge unblocking, challenge-buffered reveals,
  deterministic quorum tallies, contested tie detection, validated Norito
  checkpoints for ballot records and the local event backlog under
  `moderation-ballots/ballots-snapshot.to`, restored local event replay, and
  Torii JSON endpoints for announcement, list/get, commit, challenge
  submission, challenge resolution, reveal, tally, and event backlog under
  `/v1/sorafs/moderation/ballots*`. The local ballot list endpoint keeps full
  totals visible while bounding returned records through `limit` (default 50,
  max 500), and ballot list/detail records bound embedded
  commit/reveal/challenge arrays with returned-count and truncation metadata.
  Announcement intake requires a confirmed native asset-lock appeal deposit
  bound to the same case, round, and evidence bundle. Local moderation ballot
  lifecycle and challenge submit/resolve events now publish into the SoraFS
  Governance DAG filesystem publisher's single authoritative publication
  envelope, assembled CAR queue, and optional signed runtime DAG.
  The SFM-4b moderation-panel rollout
  evidence gate now validates payload-free appeal intake, sortition roster,
  evidence viewer, operator workflow, juror notifications, commit/reveal,
  decision publication, settlement integration, transparency/reputation handoff,
  panel metrics, end-to-end panel simulation, and governance approval evidence,
  requires sortition/viewer/operator/notification/voting and downstream
  artifacts to bind back to the valid appeal-intake `case_digest_hex`, requires
  roster-bound artifacts to match a valid case-bound sortition
  `case_digest_hex`/`roster_hash_hex` pair, and requires publication,
  settlement, transparency/reputation, metrics, end-to-end, and governance
  artifacts to match a valid roster-bound commit/reveal
  `case_digest_hex`/`roster_hash_hex`/`tally_digest_hex` tuple; invalid
  sortition rosters and invalid commit/reveal runs do not anchor downstream
  rollout evidence, case-digest binding failures use the shared scalar binding
  error recorder, and roster/tally tuple binding failures use the shared
  string-tuple binding error recorder so artifact invalidation cannot drift
  from other rollout gates. End-to-end panel artifacts must now carry
  `policy_digest_hex`, valid e2e panel policy digests are published as
  `valid_policy_digests`, and governance approval `policy_digest_hex` must
  match one of those valid panel policy digests before the gate can report
  ready. The gate now also requires exactly one active case digest, roster
  binding, tally binding, and policy digest, clearing mixed
  `valid_case_digests`, `valid_roster_bindings`, `valid_tally_bindings`, or
  `valid_policy_digests` before bound artifact or aggregate metadata can
  promote. The lane checker also has direct adversarial coverage that forges every
  case-bound, roster-bound, tally-bound, and policy-bound downstream digest or
  tuple, so sortition, evidence viewer, workflow, notification, voting,
  publication, settlement, transparency/reputation, metrics, end-to-end, and
  governance evidence all fail against detached case, roster, tally, or policy
  anchors before promotion. End-to-end panel artifacts also bind `peer_count` and
  `validator_count` to the unique canonical `peers[].name` and
  `validators[].name` inventories and reject duplicate peer or validator
  entries before promotion can report ready, and require reviewed lowercase
  `moderation-peer-*` and `moderation-validator-*` labels without
  non-production markers. End-to-end panel artifacts also bind `case_count` to
  the unique canonical `cases[].name` inventory, require `case_count` to match
  the `cases[].passed` partition, require reviewed lowercase
  `moderation-case-*` labels without non-production markers, and reject
  duplicate end-to-end case entries before promotion can report ready.
  Appeal-intake artifacts also bind
  `case_count` to the unique canonical `cases[].name` inventory, require
  `accepted_case_count` to match the `cases[].accepted` partition, require
  reviewed lowercase `moderation-appeal-case-*` labels without non-production
  markers, and reject duplicate case entries before promotion can report ready.
  Sortition-roster
  artifacts also bind `panel_size` to the unique canonical `jurors[].name`
  inventory, require `panel_size` to match the `jurors[].eligible` partition,
  require reviewed lowercase `moderation-roster-juror-*` labels without
  non-production markers, and reject duplicate roster juror entries before
  promotion can report ready.
  Evidence-viewer artifacts also bind `role_count`, `security_control_count`,
  `access_event_kind_count`, and `export_target_count` to the unique reviewed
  role/security-control/event-kind/export-target inventories so missing,
  inflated, duplicate, or unknown scalar coverage cannot satisfy promotion.
  Appeal-intake,
  operator-workflow,
  commit/reveal, and decision-publication artifacts also bind `route_count` to
  the unique
  canonical `routes[].name` inventory so duplicate or unknown route rows cannot
  inflate readiness, require every route response to carry a
  `body_blake3_hex` digest, and route `latency_ms` evidence must be
  integer-unit before route ceilings can pass. Commit/reveal artifacts also bind
  `commit_count` and
  `reveal_count` to the unique canonical `commits[].name` and `reveals[].name`
  inventories, require reviewed lowercase `moderation-commit-*` and
  `moderation-reveal-*` labels without non-production markers, reject duplicate
  commit or reveal entries, and reject reveal totals above the reviewed commit
  total before promotion can report ready.
  Commit/reveal scenario coverage now also binds `scenario_count` to the unique
  `scenarios_exercised` inventory, and `max_event_lag_seconds` must be
  integer-unit evidence before the event-lag ceiling can pass; decision
  publication binds `outcome_count`
  to `outcomes`; and transparency/reputation handoff binds
  `publication_target_count` to `publication_targets`, with required minimum and
  exact-inventory checks plus unknown-value rejection for each scalar.
	  Metrics/alert artifacts also bind `metric_count` to the unique canonical
	  `metrics` inventory so duplicate or unknown metric rows cannot inflate
  readiness, exports the reviewed `metrics` inventory plus
  `metric_count_values`, and requires aggregate production-readiness to tether
  both fields to metrics/alert artifact fingerprints before final promotion can
  report ready.
  Settlement-integration artifacts also bind `settlement_count` to the unique
  canonical `settlements[].name` inventory, require reviewed lowercase
  `moderation-settlement-*` labels without non-production markers, and reject
  duplicate settlement entries before promotion can report ready. The
  moderation-panel gate also requires reviewed
  `deployment_id`/`environment` context on every artifact and blocks mixed
  reviewed deployment contexts across the same rollout bundle. The matching
  collection planner now validates the schema-closed collection-plan envelope
  plus canonical nested required-kind, threshold, external-evidence,
  checker-backed evidence-contract, and command-step shapes before dry-run
  output or verifier execution. The rollout-gate static contract now also pins
  the parent SFM-4b appeal intake service,
  persisted case lifecycle, panel sortition/roster service, decision
  publication, portal/jury workflow, durable public decision trail, and
  deployed moderation-panel promotion routes and nested CLI spellings as
  unshipped with reusable matchers and segment-aware negative controls while
  preserving the shipped local `ballots*` lifecycle API and adjacent local
  operator workflow tooling.
  A checked-in `build_sorafs_moderation_panel_canary.py` helper now
  builds payload-free appeal-intake, sortition, evidence-viewer, operator,
  juror-notification, commit/reveal, decision-publication, settlement,
  transparency/reputation, e2e panel, metrics, and governance canary artifacts
  with explicit verified claims, complete required route/viewer/scenario/
  publication/metric coverage, shared case/roster/tally digest bindings,
  route-bearing `--route-body-blake3-hex` evidence,
  reviewed appeal-intake case labels whose unique inventory matches
  `case_count`, reviewed `moderation-appeal-case-*` labels without
  non-production markers, reviewed sortition-roster juror labels whose unique
  inventory matches `panel_size`, reviewed `moderation-roster-juror-*` labels
  without non-production markers,
  reviewed evidence-viewer session labels whose unique inventory matches
  `session_count`, reviewed `moderation-viewer-session-*` labels without
  non-production markers, reviewed juror-notification labels whose unique
  inventories match `notification_count` and `juror_count`, reviewed
  `moderation-notification-*` and `moderation-juror-*` labels without
  non-production markers, reviewed
  commit/reveal labels whose unique inventories match `commit_count` and
  `reveal_count`, reviewed `moderation-commit-*`/`moderation-reveal-*` labels
  without non-production markers, reviewed
  settlement-integration labels whose unique inventory matches
  `settlement_count`, reviewed `moderation-settlement-*` labels without
  non-production markers, derived evidence-viewer role/security/event/export,
  commit/reveal scenario, decision outcome, and transparency publication-target
  count fields, reviewed e2e case labels whose unique inventory matches
  `case_count`, reviewed `moderation-case-*` labels without non-production
  markers, reviewed e2e peer/validator labels, reviewed
  `moderation-peer-*`/`moderation-validator-*` labels without non-production
  markers, reviewed e2e/governance policy-digest input, forced raw-payload
  omission flags with missing-field regressions proving those flags and
  `critical_alerts_firing` must be explicitly encoded as `false`, direct
  duplicate/unknown verified-claim, route, viewer role,
  viewer security-control, viewer event-kind, viewer export-target, scenario,
  outcome, publication-target, and metric input regressions before writing,
  checker-backed prevalidation,
  atomic symlink-safe writes, and appeal-intake, commit/reveal, and e2e example
  argfiles.
  The evidence-viewer canary now covers SFM-4b3-specific controls for
  role-scoped manifests, short-lived URLs, attested/logged sessions, strict CSP,
  disabled offline mode, watermark overlay/metadata hashing, append-only access
  logs, anomaly events, legal-hold binding, Governance DAG and
  transparency-ledger export coverage, daily digest publication, payload-free
  digest hashes for the session manifest, watermark metadata, access log,
  legal-hold receipt, transparency report, and audit digest, and rejection of
  audit-log tampering, watermark metadata mismatch, signed URLs, session tokens,
  watermark secrets, raw evidence, raw access logs, legal-hold receipt payloads,
  transparency report payloads, and response bodies. The new payload-free
  `evidence_viewer` canary builder now turns
  reviewed deployment facts into checker-validated digest-only canary JSON,
  rejects unreviewed deployment ids and environments before checker
  prevalidation, requires every positive viewer-control claim explicitly, forces
  raw evidence/session-token/signed-URL/watermark-secret/body flags to `false`,
  emits the checker-required role/security-control/access-event/export-target count
  fields from the reviewed inventories before prevalidation, rejects unknown or
  duplicate scalar coverage with direct duplicate/unknown verified-claim, role,
  security-control, access-event-kind, and export-target regressions before
  writing, requires explicit audit-log tamper and watermark metadata mismatch
  rejection, rejects overlong URL TTLs, invalid digest casing, and unsafe output
  targets before writing, and writes the artifact atomically for staged review.
  The rollout-gate static contract now publishes
  `valid_evidence_viewer_digest_sets` from valid `evidence_viewer` artifacts,
  including the viewer's canonical nested gateway catalog digest, and makes the
  final aggregate production-readiness gate tether those digest sets to
  recognized artifact fingerprints before reporting ready. When moderation
  and gateway compliance are selected together, that viewer catalog digest set
  must equal gateway compliance `valid_catalog_digests`. The aggregate gate
  also requires `valid_roster_bindings`, `valid_tally_bindings`,
  `valid_e2e_runs`, and `valid_evidence_viewer_digest_sets` to preserve the
  case, roster, tally, and gateway-catalog chain proven by the lane checkers
  before final promotion can report ready, and rechecks case-bound,
  roster-bound, tally-bound, and policy-bound artifact fingerprints against
  `valid_case_digests`, `valid_roster_bindings`, `valid_tally_bindings`, and
  `valid_policy_digests`. The SFM-4b3 reference service now ships
  finalized case/round/evidence authorization for current jurors and explicit
  auditor/legal roles, WebAuthn challenge consumption and replay prevention,
  rotating grants with a 15-minute session ceiling, authenticated range
  decryption, a strict-CSP/no-store embedded viewer shell, and Ed25519-signed
  hash-chained payload-free receipts. The catalogued `/v1/evidence/*` family
  covers session, manifest, range, interaction, audit/status, legal-hold,
  retention, and erasure operations; the former operator-only local
  viewer-session/access routes are no longer mounted or advertised. The
  crash-safe canonical Ed25519-signed checkpoint envelope retains digests and
  finalized anchors rather than assertions, credentials, bearer grants,
  signing keys, or evidence payloads. The checkpoint now lives in a signed
  predecessor-bound record under an injected qualified external CAS authority;
  reported-success and ambiguous writes require authoritative readback, stale
  replicas fail closed, and the hardened local file is only an exact or
  one-generation-behind verified cache. `iroha_config`, the sanitized daemon
  registry, standard `irohad`, and Torii carry the store's public
  handle/revision/policy digest and require the provider whenever the viewer is
  enabled. Legal holds take precedence over erasure without a check/commit
  race. The signed receipt checkpoint and exact predecessor-bound receipt
  projection are now the sole evidence-access audit authority. The old
  aggregate-audit POSTs are unmounted and unadvertised, so requests return
  `404 Not Found`; their scheduler and all Torii calls into the former local
  viewer registry are removed. Moderation GETs read only the fresh
  worker-owned finalized projection, while reconciliation runs outside request
  threads with supervised deadlines, non-overlap fencing, monotonic
  cursor/freshness/liveness checks, dead-letter readiness, and typed
  payload-free startup failures. The standard daemon now also qualifies the
  configured moderation strict-ingress handle, revision, and policy digest
  against Torii's fixed public V1 binding during the common pre-Tokio catalog
  projection; missing, substituted, stale, zero-qualified, or test-marked
  metadata fails before broker or durable-state construction. This closes the
  in-process ingress-binding gap without treating it as an external broker
  provider.
  SFM-4b3 remains open under `V1-BLOCK-MODERATION-VIEWER-RUNTIME-01`: construct
  qualified runtime authentication/signing/custody providers, a linearizable
  sealed-CAS checkpoint-store, immutable object-lock archive, and authenticated
  downstream providers;
  add durable notification delivery, real settlement/publication adapters, and
  the signed receipt-to-transparency adapter; fence semantic operation IDs and
  give the moderation orchestrator equivalent predecessor-bound monotonic
  checkpoint ownership; deploy the viewer CAS/archive path across replicas and
  prove the shipped signed replay-safe bounded compaction contract against real
  providers; and complete focused, adversarial, multi-instance, recovery, and
  four-validator evidence.
  The commit/reveal canary now covers SFM-4b4-specific controls for
  commit digest recomputation, duplicate-commit rejection, mismatched-reveal
  rejection, late commit/reveal rejection, missed-quorum detection, no-show
  failover, juror penalty planning, deterministic tally replay, contested
  challenge coverage, governance event digest binding, event-lag bounds, and
  absence of raw commit/reveal payloads. The matching collection planner accepts
  reviewed staged evidence paths,
  supports `@ARGFILE`, forwards freshness/latency/panel/peer thresholds, and
  emits a dry-run-visible verifier command, selected-kind `evidence_contract`,
  and operator example args; it now validates the schema-closed collection-plan
  envelope, required kinds, thresholds, external evidence map, checker-backed
  evidence contract, and command steps before dry-run output or verifier
  execution. NodeHandle-level commit/reveal regressions now also pin that late
  commits, ineligible commits, premature reveals, missing-commit reveals, late
  reveals, early tallies, and no-show quorum failures leave ballot state and
  lifecycle event streams unpromoted until a valid transition succeeds, while a
  full-panel reveal set can tally before the reveal deadline and duplicate
  tally attempts stay event-silent after finalization. NodeHandle now also
  derives payload-free no-show penalty plans for closed ballots, rejects open
  reveal windows and pending or accepted challenges, splits missing commits
  from unrevealed commitments, stays event-silent after quorum failures, and
  keeps the penalty-plan digest stable across replay after a successful tally.
  The local ballot
  checkpoint now persists successful
  announcement/commit/challenge-submit/challenge-resolve/reveal/tally
  transitions and the local event backlog when storage is enabled, restores them
  on startup, and rejects duplicate-commit, missing/duplicated/mutated challenge
  events, unresolved or accepted-challenge with reveal/tally, or bad-tally
  checkpoint corruption without promoting local state. The rollout-gate static
  contract now pins the
  SFM-4b4 voting contract, durable ballot orchestrator, juror CLI/portal,
  production challenge monitor/dispute service, contract/ledger recording, and
  public decision/challenge DAG as unshipped production-service work with
  reusable matchers and segment-aware negative controls while preserving the
  local `ballots*` API, shipped local challenge record/API event/Governance DAG
  publication, shipped local no-show penalty planning, shipped local CLI/client
  bridge, executor automation, and payload-free canary labels. The static
	  contract now also scans nested deployed-only CLI spellings such as
	  `moderation ballots service`, `moderation commit-reveal coordinator`,
	  `moderation juror portal`, and `sorafs juror`, while preserving local
	  `moderation ballots list|get|no-show-plan|events|commit|reveal|tally` and
	  executor/canary commands. The commit-reveal production-service route matcher
	  now also requires a left boundary so prefixed internal paths cannot satisfy
	  reserved public route checks, and the SoraFS docs warning-only scan covers
	  the unshipped voting-contract, ballot-orchestrator, juror portal, and
	  deployed ballot-service names across top-level `specs/` plans and nested
	  `specs/sorafs/**` docs before those names can appear outside an explicit
	  do-not-document-as-shipped warning. Public and localized copies belong in
	  sibling `iroha-docs`. SFM-4b4 still
	  needs production orchestration, on-chain or ledger recording, scheduled
  no-show dispatch/settlement handoff, production juror portal flows, public
  decision/challenge DAG
  rollout, end-to-end panel simulations, and deployed evidence that passes this
  gate;
  SFM-5 hedging/billing now has local deterministic Norito payloads and pure
  helpers for XOR/USD feed samples, weighted reference-price decisions,
  stale/rejected-feed refusal, divergence degradation, billing line items,
  statement totals, micro-XOR to USD-micro conversion, and BLAKE3 line/statement
  ids. The reference validator now gates those feed/decision/line/statement
  payloads through `validate_hedging_payload_bytes`, and `sorafs-validate
  hedging`/`billing` provides local operator validation for those artifacts.
  The source bridge surface now exposes the same validator through
	  `sorafs_reference_validate_hedging_json`, Connect C/JNI ABI 23
  `connect_norito_sorafs_reference_validate_hedging_json`, and Kotlin/JVM,
  Java Android, and Swift SDK wrappers. `sorafs_node` now also exports the
  durable `HedgingBillingService`, and standard `irohad` can supervise it under
  strict `iroha_config` policy, digest, bounds, and opaque runtime-adapter
  handles. The worker consumes bounded contiguous finalized event pages and
  authenticated period closes, deterministically projects accruals,
  per-account governed statements, aggregate XOR exposure, and hedge intents,
  then durably coordinates runtime-only signing, immutable publication,
  ambiguous-write lookup, authoritative acknowledgements, reconciliation,
  retry/dead-letter state, sealed epoch witnesses, and payload-free
  health/alert metrics. The finalized query, journal verifier, signer/custody
  provider, publisher, acknowledgement authority, and witness store are
  injected private-key-free interfaces; missing, test-marked, unready,
  substituted, or drifting providers fail startup. No execution adapter or timer is present in
  the worker, every intent has `automatic_execution=false`, and the separate
  explicitly authorized submission helper rejects adapters that advertise
  automatic execution. The SFM-5 rollout evidence gate now
  validates feed-collector, reference-price, billing-cycle,
  statement-publication, reconciliation, metrics/alert, native-bridge-release,
  and governance-approval artifacts, rejects payload-bearing evidence including
  common camel-case or hyphenated secret-key spellings, requires reviewed
  `deployment_id`/`environment` context on every artifact, binds
  feed-collector and reference-price `feed_count` to the unique canonical
  `feeds[].name` inventory while requiring `accepted_feed_count` to equal
  `feed_count`, requires coverage for the reviewed `feed-primary`,
  `feed-secondary`, and `feed-tertiary` price feeds so duplicate, unknown, or
  partial feed rows cannot inflate readiness, requires feed-collector
  `feed_lag_seconds` plus reference-price `divergence_bps` and
  `decision_lag_seconds` to be non-negative integer-unit evidence before
  satisfying rollout ceilings, caps `divergence_bps` and
  `--max-divergence-bps` at `10000`, requires each staged billing cycle to carry payload-free line-item, statement-bundle,
  reconciliation, and per-statement digest roots, requires the per-statement
  digest count to match the signed statement count, requires billing-cycle
  `cycle_id` values to match reviewed lowercase `billing-cycle-*` labels without
  non-production markers, binds billing-cycle `statement_count` to the unique
  canonical `statements[].name` inventory using reviewed
  `billing-statement-*` labels without non-production markers and
  `line_item_count` to the unique canonical `line_items[].name` inventory
  using reviewed `billing-line-item-*` labels without non-production markers
  so duplicate statement or line-item rows cannot inflate readiness, requires
  every staged billing cycle to
  reference a valid reference-price decision from the same
  rollout bundle, requires statement-publication, reconciliation, metrics/alert,
  and governance-approval artifacts to bind back to a valid staged billing
  cycle's `statement_bundle_digest_hex`/`reconciliation_digest_hex` tuple in
  the same rollout bundle, requires billing-cycle artifacts to carry
  `policy_digest_hex`, publishes valid cycle policy digests as
  `valid_policy_digests`, requires governance approval `policy_digest_hex` to
  match one of those valid cycle policies, marks reference-price, cycle-tuple,
  and policy-digest binding failures on the offending artifact through shared
  binding error recorders before required-kind summary validity is reported, and
	  the aggregate production-readiness gate now derives the expected cycle tuple
	  and policy digest sets from `valid_billing_cycles` so `valid_cycle_bindings`,
	  `valid_policy_digests`, and billing-cycle `reference_decision_id_hex` values
	  cannot drift from the complete staged-cycle metadata, and now rechecks
	  cycle-bound and policy-bound artifact fingerprints against
	  `valid_cycle_bindings` and `valid_policy_digests` before final promotion.
	  The hedging gate now also requires exactly one active cycle tuple and one
	  active policy digest, clearing split `valid_cycle_bindings` or
	  `valid_policy_digests` before bound artifact or aggregate metadata can
	  promote.
	  The lane checker also has direct adversarial coverage that forges the cycle
	  tuple on every cycle-bound downstream kind so statement-publication,
	  reconciliation, metrics/alert, and governance evidence all fail against a
	  mismatched billing-cycle tuple before final promotion. It also binds
	  statement-publication `route_count` to the unique canonical
  `routes[].name` inventory, requires every statement-publication route
  response to carry a `body_blake3_hex` digest, and binds
  `acknowledgement_probe_count` to reviewed duplicate-free
  `billing-ack-probe-*` `acknowledgement_probes` without non-production
  markers, plus reconciliation `source_count` to the unique
  canonical `sources[].name` inventory and reconciliation
  `line_item_count` to the unique canonical `line_items[].name` inventory
  using reviewed `billing-line-item-*` labels without non-production markers,
  and native-bridge release `artifact_count` to the unique canonical
  `artifacts[].id` inventory using reviewed `hedging-native-artifact-*` labels
  without non-production markers, requires native artifact ids to start with
  reviewed Swift/JNI family prefixes and cover both families, requires
  reference-price artifacts to
  explicitly set `degraded` to `false`, requires native-bridge release artifacts
  to explicitly set `debug_artifacts` to `false`, and now has consolidated
  missing-field regressions proving payload-byte, response-body,
  statement-body, raw-financial-record, metrics-alert, degraded, and debug
  flags must be explicitly encoded as `false`, so
  duplicate or unknown route/source rows plus duplicate acknowledgement-probe
  or artifact rows cannot
	  inflate readiness, binds
	  metrics/alert `metric_count` to the unique canonical `metrics` inventory so
  duplicate or unknown metric entries or missing metric counts cannot inflate
  readiness, exports the reviewed metrics inventory plus `metric_count_values`,
  and requires aggregate production-readiness to tether both fields to the
  metrics/alert artifact fingerprints before final promotion can report ready,
  and requires two distinct successful staged billing cycles before
  promotion can report `ready`. The checker and matching rollout
  collection planner now
  accept reviewed staged evidence paths, support `@ARGFILE`, forward gate
  thresholds, preflight the verifier script and output targets, and emit a
  dry-run-visible verifier command plus operator example args. The planner now
  also mirrors the checker's required payload fields in dry-run
  `evidence_contract` output so operators can preflight staged billing artifact
  shape before collection, and validates the schema-closed collection-plan
  envelope plus canonical nested required-kind, threshold, external-evidence,
  checker-backed evidence-contract, and command-step shapes before dry-run
  output or verifier execution. A new
  `scripts/build_sorafs_hedging_canary.py` helper builds payload-free SFM-5
  canaries for feed collector, reference price, billing cycle, statement
  publication, reconciliation, metrics/alerts,
  native-bridge release, and governance approval evidence from reviewed
  deployment facts, requires every positive proof claim and
  feed/line-item/route/source/metric coverage set explicitly, rejects duplicate
  or unknown verified-claim, feed, route, source, and metric inputs before
  writing, forces raw feed/statement/financial-record/response/debug payload
  flags to `false`, rejects ungoverned hedge-execution enablement and malformed,
  non-production, or duplicate native-bridge release artifact ids before
  writing, rejects native-bridge release canaries with fewer than two distinct
  artifact ids or without both reviewed Swift/JNI native bridge families, emits
  `metric_count` from the canonical metrics inventory, requires
  reviewed `--feed` labels whose unique inventory matches `--feed-count`,
  reviewed `--cycle-id` labels to match the gate's `billing-cycle-*` production shape,
  reviewed `--statement` labels in the `billing-statement-*` family whose
  unique inventory matches `--statement-digest-hex`, requires reviewed
  `--line-item` labels in the `billing-line-item-*` family whose unique
  inventory matches `--line-item-count`, requires reviewed
  `--acknowledgement-probe` labels in the `billing-ack-probe-*` family whose
  unique inventory matches `--acknowledgement-probe-count`, requires
  statement-publication `--route-body-blake3-hex` evidence, requires reviewed
  `--policy-digest-hex` input for
  billing-cycle and governance-approval canaries, validates generated artifacts through the hedging/billing
  checker contract, and writes atomically without following output symlinks. A checked-in
  SFM-5 Grafana dashboard and Prometheus alert/test pack now
  defines the hedging/billing observability contract for feed lag/divergence,
  exposure drift, statement generation failures, acknowledgement backlog, and
  escrow runway, and `iroha_telemetry::Metrics` now exposes helper methods for
  those metric families. A checked-in hedging/billing fixture generator,
  `fixtures/sorafs_manifest/hedging/fixture_manifest.json`, and
  `fixtures/sorafs_manifest/hedging/README.md` now define the target positive
  and negative `.to`/`.json` fixture suite for feed, reference-price, billing
  line, billing statement, stale decision, USD mismatch, and totals-mismatch
  cases plus the validator commands each generated payload must satisfy.
  `scripts/check_sorafs_hedging_fixture_manifest.py` now validates the manifest
  in pre-generation mode, including accepted/rejected path and reviewed
  `negative_case` contracts, rejects summary-output paths that alias the
  manifest before reading or writing, and fails closed on missing or mismatched
  generated `.to`/`.json` files in full mode, while skipping generated-byte
  reads for missing, malformed, absolute, or out-of-corridor fixture paths so
  bad manifests return blocked summaries instead of reading outside the fixture
  root. The checker also converts manifest read/hash failures, generated
  fixture read failures, and generated inventory scan failures into blocked
  summaries with structured errors instead of tracebacks. Validator-command
  tokenization and validator process-launch failures also route through the
  shared error diagnostic sanitizer, so malformed shell-parser or OS exception
  text becomes `<non-canonical-error>` instead of raw multi-line diagnostics.
  Full mode also runs
  the pinned `sorafs-validate hedging` command
  contract without shell execution and compares each generated payload against
  the manifest's accepted or rejected outcome, verifies the kind-specific
  top-level and nested JSON sidecar field set, checks V1 versioning, duplicate
  nested ids, account-id hex binding, and statement timestamp ordering,
  enforces even-length lowercase hex payload mirrors, positive prices,
  timestamps, canonical unsigned `u128` billing amount/quantity strings, and
  bounded basis-point fields, and rejects extra generated `.to` or `.json`
  files that are not pinned by the manifest. The generated positive and
  negative fixture byte suite is now checked in and pinned by the rollout
  contract so future deletions or unmanifested fixture drift fail closed. The
  rollout-gate static contract now distinguishes the shipped internal
  projector/supervisor, authenticated Torii API, signed Rust client, and
  standard `iroha_cli` commands from the still-external live collector,
  concrete finalized-query/journal-verifier/signing-custody/publisher/acknowledgement/
  witness providers, and governed manual venue adapter. There is deliberately
  no separate service-management daemon API. Automated hedge execution remains
  forbidden. The contract preserves local `sorafs-validate
  hedging`/`billing`, fixture validation, rollout evidence, runtime metrics, and
  payload-free canary labels. It also scans
  CLI sources for nested deployed-only `hedging
  daemon|price-feed-collector|hedge-execute|status` and `billing
  daemon|statement-publish|statement-ack|api` spellings without blocking local
  validator, fixture, reference-price, billing-cycle, statement-publication, or
  canary labels. The incentives service now requires a concrete reward budget
  approval before init, process, record/replay, shadow-run, or daemon payout
  handling; the missing-budget override and permissive reward-engine plumbing
  are removed down to the internal validation helpers, the relay incentive docs
  state that lab/staging fixtures must carry a signed Parliament hash, and the
  rollout contract pins that service-wide surface. SFM-5 still needs focused
  Rust verification, a live collector, production finalized-query and
  journal-verifier adapters, independently administered deployment-selected
  signing/custody with independent review,
  immutable publication, acknowledgement-authority and sealed-witness
  providers, any manually governed venue adapter, released native bridge
  artifacts, deployed scrape/alert-routing validation, governance approval,
  and staged billing evidence that passes the gate; SFM-6
  now uses only native reserve policy/provider/movement/rent/lifecycle/credit/
  repayment/appeal state and committed events. Exact caller-signed native
  transactions enter strict durable ingress; the supervised worker derives
  bounded rent/lifecycle operations from one finalized view and reconciles its
  durable outbox against exact Kura outcomes. Authenticated reads and event
  streams return typed finalized projections. The former local runtime,
  checkpoint, scheduler, mutation bodies, compatibility aliases, and CLI
  adapters are deleted. Reserve metrics are rebuilt from the typed finalized
  event journal and current provider accounts, use only bounded lifecycle/status
  labels, publish a finalized height, remain unready until pending movement and
  appeal totals reconcile, and bind the evidence artifact to a fresh scrape
  digest with zero recent projection failures. The SFM-6 gate also requires quote
  matrices to bind to a valid policy digest, ledger digests to bind to that policy/matrix tuple,
  and lifecycle, signed-route, movement, credit-line, appeal, metrics,
  provider-bake, and governance artifacts to carry the same payload-free
  `policy_digest_hex`/`matrix_digest_hex`/`ledger_digest_hex` tuple, and
  provider-bake artifacts must prove the config-backed scheduler canary ran,
  advanced defaulting providers, synced gateway compliance, and preserved
  orderbook rejection, require provider inventories to use reviewed lowercase
  `provider-*` labels without non-production markers, require provider-bake
  cycle inventories to use reviewed lowercase `reserve-rent-cycle-*`,
  `reserve-top-up-cycle-*`, and `reserve-appeal-cycle-*` labels without
  non-production markers, and provider-bake `provider_count`,
  `rent_cycle_count`, `top_up_cycle_count`, `appeal_cycle_count`, and
  `scheduled_lifecycle_canary_tick_count` fields must bind to unique canonical
  provider/cycle/tick inventories using reviewed `reserve-lifecycle-tick-*`
  tick labels so duplicate or placeholder provider-bake rows cannot inflate
  readiness, quote-matrix `scenario_count` and
  `passed_scenario_count` fields must bind to the product of unique
  `storage_classes`, `tiers`, and `durations` inventories so duplicate or
  unknown matrix dimension rows cannot inflate readiness, ledger-digest
  `ledger_count` and `instruction_count` fields must bind to unique reviewed
  `reserve-ledger-*` and `reserve-instruction-*` inventories so duplicate or
  placeholder ledger/instruction refs cannot inflate readiness, lifecycle
  `persisted_stage_count` fields must bind to unique reviewed
  `reserve-lifecycle-stage-*` inventories so duplicate or placeholder stage
  refs cannot inflate readiness, lifecycle and signed-route
  `route_count` fields must bind to the unique canonical `routes[].name`
  inventories and every lifecycle/signed route response must carry a
  `body_blake3_hex` digest so duplicate, unknown, or body-unbound route rows cannot inflate readiness, and
  lifecycle `max_lifecycle_lag_seconds`, route `latency_ms`, and signed-route
  `max_route_latency_ms` fields must be integer-unit evidence before their
  rollout ceilings can pass, so fractional lag or latency values cannot inflate
  readiness;
  reserve-movement `movement_count` fields must bind to the unique canonical
  `movements[].action` inventory so duplicate or unknown movement-action rows cannot
  inflate readiness, and consolidated missing-field regressions now prove
  policy, quote, ledger, transfer-instruction, response-body, transfer,
  instruction, appeal, critical-alert, and provider-bake payload-safety flags
  must be explicitly encoded as `false`,
  appeal-policy `appeal_probe_count` fields must bind to
  the unique canonical `appeal_probes[].name` inventory so duplicate or unknown
  appeal-probe rows cannot inflate readiness, and credit-line
  `credit_line_mutation_count`/`accrual_cycle_count` fields must bind to the
  unique canonical `credit_line_mutations[].name` and
  `accrual_cycles[].name` inventories so duplicate or unknown credit-line rows cannot
  inflate readiness, while reserve-movement artifacts must prove live chain
  submission coverage, submitted transaction-hash
  readback, automatic finality polling, confirmed-status polling, timeout
  rejection, submitted, confirmed, and rejected custody evidence plus
  confirmed-balance readback and confirmed-withdrawal underflow rejection, and
  credit-line artifacts must
  prove live account-state mutation/readback, accrual posting, manual-tier
  non-mutation, and account-state reconciliation, and governance artifacts must
  prove source-entry publication, downstream compliance application, consumer
  coverage, handoff verification, and non-reserve entry preservation, with
  governance approval `bake_id` values matched to valid provider-bake artifacts
  before promotion can report `ready`, and
  `downstream_compliance_consumer_count` bound to the unique canonical
  `downstream_compliance_consumers[].name` inventory using reviewed
  `reserve-compliance-consumer-*` labels without non-production markers before
  promotion can report `ready`, with metrics artifacts now required to include
  the actual runtime `torii_sorafs_reserve_*` families consumed by the reserve
  dashboards and alerts while rejecting duplicate or unknown metric labels,
  and
  the rollout summary now exports the sorted reviewed `metrics` inventory plus
  `metric_count_values` so the aggregate production-readiness gate can require
  both fields to match the metrics/alert artifact fingerprint before final
  promotion can report ready,
  with policy-digest binding failures recorded
  on the offending artifact through the shared scalar binding error recorder
  and tuple binding failures recorded through the shared string-tuple binding
  error recorder before required-kind summary validity is reported. The
  reserve/rent collection planner now mirrors the checker's required payload
  fields in dry-run `evidence_contract` output so operators can preflight live
  artifact shape before collection, and validates the schema-closed
  collection-plan envelope, canonical schema labels, required kinds,
  thresholds, external evidence map, checker-backed evidence contract, and
  command steps before dry-run output or verifier execution, with nested
  collection-plan shape diagnostics kept payload-free for malformed labels,
  unknown kinds, unrequired evidence, and checker-contract drift. A checked-in
  `build_sorafs_reserve_rent_canary.py`
	  helper now builds payload-free policy, matrix, ledger, lifecycle, signed-route,
	  movement, credit-line, appeal, metrics, provider-bake, and governance canary
	  artifacts with explicit verified claims, complete required route/metric/matrix
	  coverage, quote-matrix `--scenario-count` bound to the reviewed
	  storage-class/tier/duration product, duplicate or unknown fixed-inventory
	  rejection before writes, provider-bake `--bake-id` labels restricted to the
	  gate's `reserve-bake-*` production shape, provider-bake `--provider` labels
	  restricted to reviewed lowercase `provider-*` production shape, provider-bake
	  cycle labels restricted to reviewed lowercase `reserve-rent-cycle-*`,
	  `reserve-top-up-cycle-*`, and `reserve-appeal-cycle-*` production shapes,
	  provider-bake cycle duplicate rejection, shared digest bindings, forced
	  governance-approval `--bake-id` evidence for the accepted provider bake,
	  raw-payload omission flags, integer lag/latency thresholds,
  checker-backed prevalidation, atomic symlink-safe writes, and policy/provider
  bake/governance example argfiles. Valid provider-bake rows now require reviewed
  lowercase `reserve-bake-*` ids without non-production markers, require
  reviewed lowercase `provider-*` provider inventory labels without
  non-production markers, require reviewed lowercase `reserve-rent-cycle-*`,
  `reserve-top-up-cycle-*`, and `reserve-appeal-cycle-*` cycle labels without
  non-production markers, and carry the accepted
  `policy_digest_hex`/`matrix_digest_hex`/`ledger_digest_hex` tuple plus
  `scheduled_lifecycle_canary_last_tick_at_unix`,
	  `scheduled_lifecycle_canary_tick_count`, and
	  `scheduled_lifecycle_canary_defaulted_provider_count` inside
		  `valid_provider_bakes`, and the aggregate production gate validates those
		  fields as payload-free metadata while preserving the policy -> matrix ->
		  ledger -> provider-bake chain between `valid_policy_digests`,
		  `valid_policy_matrix_bindings`, `valid_policy_matrix_ledger_bindings`, and
		  `valid_provider_bakes` before promotion, and now rechecks policy-bound,
		  matrix-bound, and ledger-bound artifact fingerprints against
		  `valid_policy_digests`, `valid_policy_matrix_bindings`, and
		  `valid_policy_matrix_ledger_bindings`, and governance approval
		  bake-id fingerprints against `valid_provider_bakes`. The lane checker
		  also has direct adversarial coverage that forges every policy-bound,
		  matrix-bound, and ledger-bound SFM-6 artifact kind against those policy,
		  quote-matrix, and ledger anchors before reserve-rent promotion can report
		  ready, but SFM-6 still needs
  live chain custody submission and automatic finality polling for signed movement intents,
  live account mutation for local credit-line state, broader downstream
  compliance application of governance source entries, and live provider bake
  evidence, including scheduled lifecycle canaries, that passes the gate. The
  rollout-gate static contract now uses reusable route/CLI matchers with
  segment-aware negative controls to pin the live custody submitter, finality
  polling service, credit-line account mutator, provider-bake service,
  downstream governance-source application, and reserve promotion surfaces as
  unshipped while preserving the signed local reserve lifecycle, movement,
  custody, balance, credit-line, appeal, policy, scheduler, evidence-gate
  tooling, and payload-free canary labels. It also scans CLI sources for
  nested deployed-only `reserve
  finality-poller|credit-line-mutator|provider-bake live|promote` spellings
  without blocking shipped local `reserve lifecycle|top-up|withdraw|policy`
  commands. SF-12 source now ships canonical governance schemas and validators,
  deterministic filesystem publication objects, one typed publication authority
  for the `publish_index` and `car_queue`, a typed signed runtime head/index, the
  offline `sorafs_cli governance dag` archive/checkpoint/mirror tools, path-free
  NodeHandle snapshots for Torii publication/runtime queries, and the
  liveness-bound supervised mirror capability. The always-on publisher derives
  each CID under the fixed Kubo UnixFS profile, uses signed-HTTP-only head CAS,
  retains the fixed V1 mirror suffix, recovers derived mirror state from its
  sealed intent/checkpoint, repairs missing Kubo state before readiness, and
  performs full-first plus rotating steady audits. Both public control-plane
  adapters must qualify the exact exclusive receiver and complete replica set's
  shared sealed atomic replay namespace.
  The rollout checker, planner, canary builder, examples, and tests still need
  reconciliation to that fixed transport and ingress contract before their
  output can count as release evidence. SF-12 also needs supported package and
  supervisor integration, genuine deployment-owned providers, a two-instance
  deployment with runtime-only credentials, dashboard/alert-routing/recovery
  evidence, and a measured decision on whether the bounded authenticated JSON
  mirror needs a RocksDB/IPLD scale backend.

