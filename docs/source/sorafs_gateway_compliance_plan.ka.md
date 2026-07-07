---
lang: ka
direction: ltr
source: docs/source/sorafs_gateway_compliance_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 8ba24dd8287ebe3f992227f751304ef7f464ffdf35ce84186536b272c8bf258e
source_last_modified: "2026-07-06T19:08:48.772896+00:00"
translation_last_reviewed: 2026-07-06
source_mtime: 2026-07-06T19:08:48.772896+00:00
---

# Gateway Compliance, Moderation & Transparency

## Current Status

SFM-4 is partially implemented. The gateway enforcement path, denylist helpers,
GAR policy payloads, proof-token utilities, honey-audit probing, operator bundle
tooling, and payload-free rollout evidence gate exist. The repository does not
yet ship an always-on central compliance controller daemon, deployed moderation
toggle service, deployed public receipt explorer, or full appeal-driven override
workflow. The local SFM-4c transparency ledger builder and readback surface are
shipped, but promotion evidence now has to prove controller runtime,
moderation-toggle, deployed-publication, and multi-gateway boundaries before
gateway compliance can be marked ready.

## Shipped Foundations

- `GatewayDenylist` supports provider, manifest digest, CID, URL, account id,
  account alias, and perceptual-family entries with TTL pruning, policy tiers,
  governance provenance, and active-pack metadata.
- `GatewayPolicy` evaluates manifest-envelope requirements, provider admission,
  denylist hits, rate limits, GAR CDN policy, TTL overrides, purge tags,
  moderation slugs, rate ceilings, geofences, and legal holds.
- Torii SoraFS endpoints reject denylisted manifests, CIDs, providers, and
  perceptual matches with structured error bodies and telemetry labels.
- `GarPolicyPayloadV1`, `GarCdnPolicyV1`, `GarModerationDirectiveV1`,
  `GarModerationAction`, and `GarEnforcementReceiptV1` provide deterministic
  Norito policy and enforcement evidence payloads.
- `iroha_crypto::sorafs::proof_token` implements the `SFGT` proof-token frame
  used for `Sora-Moderation-Token` style audit headers.
- `sorafs_car::policy::run_honey_probe` and
  `sorafs_cli moderation honey-audit` verify denied gateway responses,
  cache-version binding, and optional moderation proof-token evidence.
- `cargo xtask sorafs-gateway denylist pack|diff|verify` produces and validates
  deterministic denylist bundles, Merkle roots, Norito payloads, and diff
  reports for governance evidence.
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
  and toggle reports from different denylist bundle runs. Feed-promotion
  artifacts must also carry `policy_digest_hex`, and governance approval
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
- `ci/check_sorafs_gateway_denylist.sh` guards the denylist bundle tooling.

## Operator Commands

Pack a denylist bundle:

```sh
cargo xtask sorafs-gateway denylist pack \
  --input docs/examples/sorafs_gateway_denylist.json \
  --out artifacts/sorafs_gateway/denylist \
  --label global-core
```

Compare two bundles:

```sh
cargo xtask sorafs-gateway denylist diff \
  --old artifacts/sorafs_gateway/denylist/previous.json \
  --new artifacts/sorafs_gateway/denylist/current.json \
  --report-json artifacts/sorafs_gateway/denylist_diff.json
```

Verify a bundle before promotion:

```sh
cargo xtask sorafs-gateway denylist verify \
  --bundle artifacts/sorafs_gateway/denylist/current.json \
  --norito artifacts/sorafs_gateway/denylist/current.to \
  --root artifacts/sorafs_gateway/denylist/current_root.txt \
  --report-json artifacts/sorafs_gateway/denylist_verify.json
```

Run a moderation honey audit:

```sh
cargo run --locked -p sorafs_orchestrator --bin sorafs_cli -- \
  moderation honey-audit \
  --manifest-id=<hex32> \
  --honey=<digest_hex> \
  --provider name=<alias>,provider-id=<hex32>,base-url=<url>,stream-token=<base64> \
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

Rollout evidence must remain payload-free. The gate rejects raw denylist feeds,
probe response bodies, GAR receipts, appeal payloads, moderation-toggle
payloads, signed transactions, tokens, private keys, and response bodies;
operators should provide only digests, counts, stable labels, booleans, and
reviewed artifact paths. Every downstream controller/toggle/reload/probe/audit/
appeal/transparency/observability/governance artifact must bind back to the
promoted denylist bundle with
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
does not replace the missing deployed controller daemon, toggle service, live
honey-audit target, appeal feed, transparency publication hook, or governance
approval packet; it only standardizes reviewed promotion evidence once those
boundaries produce deployment facts.
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
- denylisted providers, manifest digests, CIDs, URLs, accounts, aliases, or
  perceptual fingerprints;
- client or CDN rate-limit violations;
- GAR TTL, purge-tag, moderation-slug, geofence, or legal-hold violations.

`PolicyViolation::telemetry_labels()` maps each denial to stable reason/detail
labels so dashboards and refusal guidance can aggregate behavior without
parsing response strings.

## Remaining Production Gates

- Ship the always-on compliance controller daemon that fetches external feeds,
  normalizes updates, signs them, distributes them to gateways, and tracks
  acknowledgements. The rollout gate now requires controller-runtime evidence
  for this boundary, but the daemon itself still needs production deployment.
- Persist denylist/catalog state and update history through the configured
  production storage path instead of relying only on local bundle ingestion.
- Implement moderation toggle APIs, approval workflows, expiry handling, and
  operator audit trails. The rollout gate now requires moderation-toggle
  evidence for `iroha_config` binding, approved-toggle count equality, operator
  role enforcement, approval workflow, expiry, cache invalidation, audit trail,
  rollback, and payload-free reporting, but the service itself still needs
  production deployment.
- Connect appeal outcomes to gateway policy overrides and cache invalidation.
  The existing appeal-override canary remains required for promotion evidence,
  but the full workflow is still production work.
- Wire deployed GAR receipts, proof-token indexes, and moderation events through
  the shipped local SFM-4c transparency source-entry and publication paths, then
  capture deployed publication evidence.
- Capture staged multi-gateway rollout artifacts that satisfy the SFM-4 evidence
  gate before promoting gateway compliance changes to production.

## Validation

Focused local checks for the shipped surface are:

```sh
ci/check_sorafs_gateway_denylist.sh
python3 -m pytest -q \
  scripts/tests/build_sorafs_gateway_compliance_canary_test.py \
  scripts/tests/check_sorafs_gateway_compliance_rollout_evidence_test.py \
  scripts/tests/run_sorafs_gateway_compliance_rollout_evidence_test.py
cargo test -p iroha_torii sorafs::gateway
cargo test -p sorafs_car policy
cargo test -p sorafs_orchestrator moderation
```

Run the broader gateway conformance suite before changing response bodies,
headers, or policy-denial telemetry labels.
