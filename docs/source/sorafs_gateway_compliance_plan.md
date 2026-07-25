---
title: Gateway Compliance, Moderation & Transparency
summary: SFM-4 implementation status for governed compliance catalogs, GAR policy, proof tokens, honey-audit evidence, and remaining compliance services.
---

# Gateway Compliance, Moderation & Transparency

## Current Status

SFM-4 is partially implemented. The gateway enforcement path, GAR policy
payloads, proof-token utilities, honey-audit probing, payload-free rollout
evidence gate, and gateway-scoped governed compliance controller core exist.
The core admits bounded threshold-signed, predecessor-bound catalogs; persists
candidate, acknowledgement, promotion, last-known-good, rollback, toggle,
appeal-override, and history state; and evaluates legal/safety holds before
accepted appeals and baseline policy. It also defines an address-pinned runtime
feed-transport boundary with HTTPS allowlists, public-address validation, DNS
revalidation, SPKI pins, redirect, size, time, and decompression limits.

`iroha_config` now carries the non-secret controller policy, feed allowlists and
pins, resource bounds, checkpoint path, governed signer identities, and the
canonical region and gateway deployment identities. Torii accepts
runtime-injected ACME and authenticated feed transports, transfers them through
`IrohaRuntimeDeps` and `ToriiRuntimeDeps`, constructs the durable controller
from the exact resolved configuration, retains it in `AppState`, and fails
closed when an enabled runtime dependency is absent or mismatched. This
controller/runtime integration ships locally.

Torii now exposes six canonical, account-signed, governed-operator routes:
authenticated feed fetch and durable status reads plus canonical-Norito-JSON
stage, acknowledgement, promotion, and rollback mutations. Live SoraFS content
serving evaluates the promoted catalog for manifest digest, canonical CID, and
provider subjects across global, configured-region, and configured-gateway
scopes. Missing, stale, or poisoned serving state fails closed. Enabling the
governed controller also rejects every obsolete unsigned bootstrap path and
local catalog source, so no configured local authority can compete with the
signed durable catalog. V1 has no local compliance packs, file-backed
compliance authority, catalog-mutation CLI, or unsigned compatibility route.

Real authenticated feed and ACME adapters for the standard daemon, finalized
accepted-appeal and legal/safety-hold catalog producers, independently audited
threshold-signing integration, and deployment across two independently
administered regional gateways remain open. The local SFM-4c transparency
ledger builder and readback surface are shipped, but deployed publication,
public-explorer, and two-gateway evidence remain open. Accordingly the lane
remains open: local source and synthetic-test coverage are not deployment
evidence and cannot mark gateway compliance ready.

## Shipped Foundations

- The promoted catalog evaluator supports provider, manifest digest, CID, URL,
  account id, account alias, and perceptual-family rules with TTL pruning,
  policy tiers, and governance provenance.
- `GatewayComplianceController` provides canonical Norito feed/catalog,
  signature, acknowledgement, rollback, and checkpoint contracts; deterministic
  normalization and merging; strict Ed25519 threshold/revocation checks;
  predecessor/sequence enforcement; durable idempotent staging; two-gateway
  acknowledgement quorum; atomic promotion; last-known-good rollback without
  rewriting the predecessor-chain head; bounded history; and
  legal/safety-hold > accepted-appeal > baseline precedence.
- `GatewayComplianceFeedTransport` is runtime-injected and must connect only to
  controller-pinned public DNS answers. The core revalidates DNS after each
  response, verifies the connected address and configured SPKI digest, validates
  every redirect against the exact HTTPS host allowlist, and bounds encoded,
  decoded, redirect, DNS-answer, and elapsed-time inputs.
- `FileGatewayComplianceStore` uses bounded canonical checkpoints, no-follow
  opens, regular-file checks, fsync, and atomic rename. Catalog signing keys,
  feed credentials, bearer tokens, and DNS-provider credentials are not owned by
  the controller.
- `iroha_config` defines the complete non-secret compliance policy and validates
  canonical signer, revocation, feed, host, SPKI-pin, resource, freshness, and
  history bounds before producing the runtime configuration.
- `IrohaRuntimeDeps` and `ToriiRuntimeDeps` accept runtime-owned ACME and
  authenticated feed-transport implementations. The standard daemon forwards
  those runtime-only dependencies without putting credentials in
  `iroha_config`. Torii maps every resolved field into the ACME/controller
  runtime, constructs the durable controller, retains the controller and
  transport in `AppState`, rejects dependencies injected while their feature is
  disabled, and refuses startup when an enabled dependency is missing.
- The control surface provides authenticated
  `GET /v1/sorafs/gateway/compliance/feeds/{feed_id}` and
  `GET /v1/sorafs/gateway/compliance/status` reads plus
  `POST /v1/sorafs/gateway/compliance/stage`,
  `POST /v1/sorafs/gateway/compliance/acknowledge`,
  `POST /v1/sorafs/gateway/compliance/promote`, and
  `POST /v1/sorafs/gateway/compliance/rollback` mutations. Every route requires
  canonical X-Iroha account-request authentication and the governed
  `sorafs_gateway_compliance_operator` role. Mutation bodies are bounded exact
  canonical Norito JSON; freshness uses server time, and the controller
  revalidates catalog, acknowledgement, and rollback signatures before
  committing durable state.
- Live range serving evaluates the promoted signed catalog for the request's
  manifest digest, canonical lowercase base32 CID, and provider id. Evaluation
  applies global, configured-region, and configured-gateway rules in one
  precedence pass (`legal/safety hold > accepted appeal > baseline`), returns a
  no-store denial without the matched subject, and returns service unavailable
  when governed serving state cannot be trusted.
- Controller configuration requires a canonical region identity and a gateway
  identity naming one active, non-revoked configured gateway signer. The same
  configuration parser and the Torii construction boundary reject every local
  unsigned bootstrap source whenever the governed controller is enabled.
- `GatewayPolicy` evaluates manifest-envelope requirements, provider admission,
  promoted-catalog decisions, rate limits, GAR CDN policy, TTL overrides, purge
  tags, moderation slugs, rate ceilings, geofences, and legal holds.
- Torii SoraFS endpoints reject manifests, CIDs, providers, and perceptual
  matches blocked by the promoted catalog with structured error bodies and
  telemetry labels.
- `GarPolicyPayloadV1`, `GarCdnPolicyV1`, `GarModerationDirectiveV1`,
  `GarModerationAction`, and `GarEnforcementReceiptV1` provide deterministic
  Norito policy and enforcement evidence payloads.
- `iroha_crypto::sorafs::proof_token` implements the `SFGT` proof-token frame
  used for `Sora-Moderation-Token` style audit headers.
- `sorafs_car::policy::run_honey_probe` and
  `sorafs_cli moderation honey-audit` verify denied gateway responses,
  cache-version binding, and optional moderation proof-token evidence.
- `scripts/check_sorafs_gateway_compliance_rollout_evidence.py` validates
  payload-free SFM-4 promotion evidence for feed promotion, controller runtime,
  moderation-toggle canaries, gateway reload, enforcement probes, honey-audit
  denial proof, appeal override, transparency publication, observability, and
  governance approval artifacts. The checker exports its required top-level
  payload fields as `EVIDENCE_REQUIRED_FIELDS`, and the companion
  `scripts/run_sorafs_gateway_compliance_rollout_evidence.py` runner emits the
  verifier command and dry-run collection plan from reviewed artifact paths,
  including a checker-backed `evidence_contract` map with the schema and
  required payload fields for each selected evidence kind.
  Controller, moderation-toggle, reload, enforcement, honey-audit, appeal,
  transparency, observability, and governance artifacts must carry the same
  `bundle_digest_hex` as a valid feed-promotion artifact in the same bundle, so
  promotion evidence cannot mix probes, dashboards, approvals, or controller
  and toggle reports from different governed catalog promotion runs.
  Feed-promotion artifacts must also carry `policy_digest_hex`, and governance approval
  artifacts must match that promoted policy digest before promotion. Bundle and
  policy mismatches are recorded on the offending artifact in the JSON summary
  before required-kind validity is reported.
  Feed-promotion artifacts also bind `gateway_ack_count` and
  `denylist_entry_count` to the unique canonical `gateways[].name` and
  `denylist_entries[].name` inventories, require reviewed
  `gateway-compliance-gateway-*` and `gateway-denylist-entry-*` labels without
  non-production markers, and reject duplicate gateway acknowledgement or
  denylist-entry entries before promotion can report ready.
  Controller-runtime artifacts also require `controller_instance_id` to match a
  reviewed lowercase `gateway-compliance-controller-*` label without
  non-production markers, bind
  `external_feed_count`, `fetched_feed_count`, `normalized_feed_count`, and
  `signed_feed_count` to the unique canonical `feeds[].name` inventory, require
  coverage for the reviewed `ofac`, `eu-sanctions`, `malware`, `csam-hash`,
  `legal-hold`, `regional-blocklist`, and `appeal-overrides` controller feeds,
  and reject duplicate or unknown feed entries before promotion can report
  ready.
  Moderation-toggle artifacts also bind `toggle_count` and
  `approved_toggle_count` to the unique canonical `toggles[].name` inventory and
  require coverage for the reviewed `provider-deny`, `appeal-override`,
  `legal-hold`, and `regional-emergency` toggle paths. Duplicate or unknown
  toggle entries are rejected before promotion can report ready.
  External moderation-toggle `toggle_api_url` evidence is also validated with
  the shared SoraFS URL preflight, so userinfo, query strings, encoded
  traversal, encoded separators, encoded drive prefixes, and secret-looking
  host/path components cannot enter accepted staged evidence.
  Gateway-reload artifacts also bind `reload_ack_count` to the unique canonical
  `gateways[].name` inventory, require reviewed
  `gateway-compliance-gateway-*` labels without non-production markers, and
  reject duplicate gateway acknowledgement entries before promotion can report
  ready. Gateway-reload `max_reload_latency_ms` must be positive integer-unit
  evidence before it can satisfy the reload-latency ceiling.
  Enforcement-probe artifacts also bind `denial_reason_count` to the unique
  canonical `denial_reasons_observed` inventory and bind `route_count` and
  `passed_route_count` to the unique canonical `routes[].name` inventory,
  require the reviewed `manifest`, `cid`, and `provider` route probes, and
  reject duplicate or unknown denial-reason and route entries before promotion
  can report ready. Enforcement route `routes[].latency_ms` values must be
  positive integer-unit evidence before they can satisfy the route-latency
  ceiling, and every enforcement route response must include a
  `body_blake3_hex` digest.
  Honey-audit artifacts also bind `honey_probe_count` to the unique canonical
  `probes[].name` inventory, require reviewed `gateway-honey-probe-*` labels
  without non-production markers, and reject duplicate probe entries before
  promotion can report ready.
  Observability artifacts also bind `metric_count` to the unique canonical
  `metrics` inventory, require the reviewed gateway compliance metrics
  inventory, and reject duplicate or unknown metric entries before promotion can
  report ready.
  The reviewed inventory names the emitted
  `torii_sorafs_gateway_compliance_requests_total`,
  `torii_sorafs_gateway_compliance_serving_decisions_total`,
  `torii_sorafs_gateway_compliance_failures_total`,
  `torii_sorafs_gateway_compliance_serving_catalog_sequence`,
  `torii_sorafs_gateway_compliance_serving_catalog_valid_until_seconds`, and
  `torii_sorafs_gateway_compliance_ready` families. Request, decision, and
  failure dimensions use closed label vocabularies; provider, CID, manifest,
  rule, feed, and payload values are never labels. The
  `dashboards/grafana/sorafs_gateway_compliance.json` dashboard and
  `dashboards/alerts/sorafs_gateway_compliance_rules.yml` rules cover control
  outcomes, serving decisions, bounded failure classes, serving-catalog
  sequence skew, expiry, and fail-closed readiness. The checked-in Prometheus
  rule tests exercise both firing and healthy cases. Legacy placeholder metric
  names cannot satisfy promotion evidence.
  The summary exports the sorted reviewed `metrics` inventory plus
  `metric_count_values`, and the aggregate production-readiness gate requires
  those fields to match the observability artifact fingerprint before final
  promotion can report ready. Aggregate promotion also rechecks the
  lane-proven digest relationships: bundle-bound artifact fingerprints must
  match `valid_bundle_digests`, and policy-bound artifact fingerprints must
  match `valid_policy_digests`. Gateway compliance rollout summaries must
  expose exactly one active feed-promotion bundle digest and exactly one active
  policy digest; mixed valid bundle or policy anchors fail closed before final
  promotion can report ready.
- `scripts/build_sorafs_gateway_compliance_canary.py` is a payload-free
  full-surface canary builder for feed-promotion, controller-runtime,
  moderation-toggle, gateway-reload, enforcement-probe, honey-audit,
  appeal-override, transparency-publication, observability, and governance
  approval evidence. It turns reviewed deployment facts into checked JSON
  artifacts, fixes config-backed artifacts to `config_source = "iroha_config"`,
  requires every positive controller or moderation-toggle claim through explicit
  `--verified-claim` inputs, requires reviewed controller
  `--controller-instance-id` labels to match the same
  `gateway-compliance-controller-*` production shape enforced by the gate,
  requires reviewed controller `--feed` names whose
  unique inventory matches `--feed-count` and covers every required controller
  feed, requires reviewed moderation `--toggle` names whose unique inventory
  matches `--toggle-count` and covers every required toggle path, requires
  fixed gateway, denylist-entry, and honey-probe inventories to use reviewed
  `gateway-compliance-gateway-*`, `gateway-denylist-entry-*`, and
  `gateway-honey-probe-*` labels without non-production markers, requires
  reviewed enforcement `--denial-reason` labels covering the required denial
  inventory, requires reviewed observability `--metric` names matching the
  required gateway compliance metrics inventory, rejects duplicate or unknown
  `--verified-claim`, `--feed`, `--toggle`, `--denial-reason`, and `--metric`
  inputs before writing, admits moderation-toggle
  `--toggle-api-url` values only through the shared URL preflight, requires
  an explicit `--route-body-blake3-hex` digest for enforcement canaries, forces
  raw-feed, raw-catalog, raw-toggle-payload, raw-probe-response,
  raw-appeal-payload, raw-receipt, and response-body inclusion flags to
  `false`, validates every generated payload through the SFM-4 rollout gate
  contract before writing, and writes canaries atomically without following
  output symlinks.

## Operator Commands

Catalog control is API-only in V1. Operators inspect authenticated feed and
status reads, submit a bounded canonical catalog to `stage`, collect distinct
gateway acknowledgements, call `promote`, and use `rollback` only for the
durable last-known-good catalog. Catalog construction and threshold signing
remain external to Torii; no local pack, diff, verify, or mutation CLI is
supported.

Run a moderation honey audit:

```sh
cargo run --locked -p sorafs_orchestrator --bin sorafs_cli -- \
  moderation honey-audit \
  --manifest-id=<hex32> \
  --honey=<digest_hex> \
  --provider name=<alias>,provider-id=<hex32>,gateway-key=<ed25519-public-key-hex>,base-url=https://gateway.example/,stream-token=<base64> \
  --json-out artifacts/sorafs_gateway/honey_audit.json \
  --markdown-out artifacts/sorafs_gateway/honey_audit.md
```

Validate staged gateway compliance promotion evidence:

```sh
python3 scripts/check_sorafs_gateway_compliance_rollout_evidence.py \
  @scripts/examples/sorafs_gateway_compliance_rollout_evidence.args.example
```

Preview the collection runner command before promotion:

```sh
python3 scripts/run_sorafs_gateway_compliance_rollout_evidence.py \
  @scripts/examples/sorafs_gateway_compliance_rollout_collection.args.example \
  --dry-run
```

Build reviewed payload-free gateway-compliance canaries:

```sh
python3 scripts/build_sorafs_gateway_compliance_canary.py \
  @scripts/examples/sorafs_gateway_compliance_feed_promotion_canary.args.example
python3 scripts/build_sorafs_gateway_compliance_canary.py \
  @scripts/examples/sorafs_gateway_compliance_controller_canary.args.example
python3 scripts/build_sorafs_gateway_compliance_canary.py \
  @scripts/examples/sorafs_gateway_compliance_moderation_toggle_canary.args.example
python3 scripts/build_sorafs_gateway_compliance_canary.py \
  @scripts/examples/sorafs_gateway_compliance_gateway_reload_canary.args.example
python3 scripts/build_sorafs_gateway_compliance_canary.py \
  @scripts/examples/sorafs_gateway_compliance_enforcement_probe_canary.args.example
python3 scripts/build_sorafs_gateway_compliance_canary.py \
  @scripts/examples/sorafs_gateway_compliance_honey_audit_canary.args.example
python3 scripts/build_sorafs_gateway_compliance_canary.py \
  @scripts/examples/sorafs_gateway_compliance_appeal_override_canary.args.example
python3 scripts/build_sorafs_gateway_compliance_canary.py \
  @scripts/examples/sorafs_gateway_compliance_transparency_publication_canary.args.example
python3 scripts/build_sorafs_gateway_compliance_canary.py \
  @scripts/examples/sorafs_gateway_compliance_observability_canary.args.example
python3 scripts/build_sorafs_gateway_compliance_canary.py \
  @scripts/examples/sorafs_gateway_compliance_governance_approval_canary.args.example
```

Rollout evidence must remain payload-free. The gate rejects raw compliance feeds,
probe response bodies, GAR receipts, appeal payloads, moderation-toggle
payloads, signed transactions, tokens, private keys, and response bodies;
operators should provide only digests, counts, stable labels, booleans, and
reviewed artifact paths. Every downstream controller/toggle/reload/probe/audit/
appeal/transparency/observability/governance artifact must bind back to the
promoted signed catalog with
`bundle_digest_hex`; governance approval artifacts must also bind to the
promoted feed policy with `policy_digest_hex`. If those bindings do not match a
valid feed-promotion artifact, the gate marks the downstream artifact invalid
in the emitted summary instead of only blocking the top-level status. Use the runner's dry-run
`evidence_contract` output to review the exact payload fields before collecting
or promoting staged gateway compliance evidence; the runner validates the
schema-closed collection plan, required kinds, thresholds, external evidence
map, evidence contract, and command steps before dry-run output or verifier
execution. The shared runner plan guard also rejects non-canonical nested
required-kind, threshold, external-evidence, evidence-contract, and command-step
shapes before any live gateway-compliance contact. Use the canary builder for
reviewed deployment evidence across every gateway-compliance rollout kind so
count equality, `iroha_config` binding, payload-free inclusion flags,
observability metric inventory binding, and checker prevalidation stay
consistent with the promotion gate. The payload-free full-surface canary builder
does not replace real authenticated feed and ACME adapters, finalized
appeal/hold catalog producers, a live honey-audit target, transparency
publication hooks, governance approval packets, or two-gateway deployment; it
only standardizes reviewed promotion evidence once those boundaries produce
deployment facts.
Gateway compliance payload-safety artifacts must explicitly set
`raw_feeds_included`, `feed_payloads_included`, `raw_toggle_payloads_included`,
`raw_catalog_included`, `raw_probe_responses_included`,
`raw_appeal_payload_included`, `raw_receipts_included`,
`critical_alerts_firing`, and `response_bodies_included` to `false` before
promotion can report ready.

## Enforcement Semantics

Gateway policy decisions fail closed for:

- missing manifest envelopes when envelopes are required;
- missing or unadmitted providers when admission enforcement is enabled;
- providers, manifest digests, CIDs, URLs, accounts, aliases, or perceptual
  fingerprints blocked by the promoted compliance catalog;
- client or CDN rate-limit violations;
- GAR TTL, purge-tag, moderation-slug, geofence, or legal-hold violations.

`PolicyViolation::telemetry_labels()` maps each denial to stable reason/detail
labels so dashboards and refusal guidance can aggregate behavior without
parsing response strings.

## Remaining Production Gates

- Resolve `V1-BLOCK-GATEWAY-CONTROLLER-RUNTIME-01`: supply independently audited
  real authenticated feed-transport and ACME adapters for the standard daemon
  and its supervised reference-deployment packaging. Configuration, runtime
  dependency transfer, fail-closed startup checks, durable controller
  construction, authenticated control routes, live serving enforcement, and
  unsigned-bootstrap mutual exclusion already ship locally and must not be
  reopened as missing work.
- Connect finalized accepted-appeal outcomes and legal/safety-hold producers to
  signed catalog construction and cache invalidation. The core models and
  enforces these records, but no finalized-chain producer currently submits
  them.
- Supply independently audited threshold-signing integration for the required
  external feeds. The controller deliberately holds no signing key, feed
  credential, ACME account credential, or DNS-provider credential and cannot
  invent those production dependencies.
- Wire deployed GAR receipts, proof-token indexes, and moderation events through
  the shipped local SFM-4c transparency source-entry and publication paths, then
  capture deployed publication evidence.
- Capture staged multi-gateway rollout artifacts that satisfy the SFM-4 evidence
  gate before promoting gateway compliance changes to production.

## Validation

Focused local checks for the shipped surface are:

```sh
python3 -m pytest -q \
  scripts/tests/check_sorafs_gateway_tls_runtime_contract_test.py \
  scripts/tests/build_sorafs_gateway_compliance_canary_test.py \
  scripts/tests/check_sorafs_gateway_compliance_rollout_evidence_test.py \
  scripts/tests/run_sorafs_gateway_compliance_rollout_evidence_test.py
cargo test -p iroha_torii sorafs::gateway
cargo test -p sorafs_car policy
cargo test -p sorafs_orchestrator moderation
```

The source-only ACME boundary guard is:

```sh
python3 scripts/check_sorafs_gateway_tls_runtime_contract.py
```

Run the broader gateway conformance suite before changing response bodies,
headers, or policy-denial telemetry labels.
