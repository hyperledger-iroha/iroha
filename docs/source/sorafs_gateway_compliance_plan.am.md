---
lang: am
direction: ltr
source: docs/source/sorafs_gateway_compliance_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 68c0625dd44cff9b9a5dd0868875b9aff060e70746bf49e45b01a61e53c4f7f2
source_last_modified: "2026-07-02T10:51:00.284611+00:00"
translation_last_reviewed: 2026-07-02
title: Gateway Compliance, Moderation & Transparency
summary: SFM-4 implementation status for gateway denylist enforcement, GAR policy, proof tokens, honey-audit evidence, and remaining compliance services.
source_mtime: 2026-07-02T10:51:00.284611+00:00
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
  `denylist_entries[].name` inventories and reject duplicate gateway
  acknowledgement or denylist-entry entries before promotion can report ready.
  Controller-runtime artifacts also bind `external_feed_count`,
  `fetched_feed_count`, `normalized_feed_count`, and `signed_feed_count` to the
  unique canonical `feeds[].name` inventory and reject duplicate feed entries
  before promotion can report ready.
  Moderation-toggle artifacts also bind `toggle_count` and
  `approved_toggle_count` to the unique canonical `toggles[].name` inventory and
  reject duplicate toggle entries before promotion can report ready.
  Gateway-reload artifacts also bind `reload_ack_count` to the unique canonical
  `gateways[].name` inventory and reject duplicate gateway acknowledgement
  entries before promotion can report ready.
  Enforcement-probe artifacts also bind `route_count` and `passed_route_count`
  to the unique canonical `routes[].name` inventory and reject duplicate route
  entries before promotion can report ready.
  Honey-audit artifacts also bind `honey_probe_count` to the unique canonical
  `probes[].name` inventory and reject duplicate probe entries before promotion
  can report ready.
- `scripts/build_sorafs_gateway_compliance_canary.py` is a payload-free
  controller-runtime and moderation-toggle canary builder. It turns reviewed
  deployment facts into checked JSON artifacts, fixes `config_source` to
  `iroha_config`, requires every positive controller or moderation-toggle claim
  through explicit `--verified-claim` inputs, requires reviewed controller
  `--feed` names whose unique inventory matches `--feed-count`, forces raw-feed,
  requires reviewed moderation `--toggle` names whose unique inventory matches
  `--toggle-count`, forces raw-feed, toggle-payload, and response-body inclusion
  flags to `false`, validates the generated payload through the SFM-4 rollout
  gate contract before writing, and writes the canary atomically without
  following output symlinks.
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

Build reviewed payload-free controller-runtime and moderation-toggle canaries:

```sh
python3 scripts/build_sorafs_gateway_compliance_canary.py \
  @scripts/examples/sorafs_gateway_compliance_controller_canary.args.example
python3 scripts/build_sorafs_gateway_compliance_canary.py \
  @scripts/examples/sorafs_gateway_compliance_moderation_toggle_canary.args.example
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
reviewed controller-runtime and moderation-toggle deployment evidence so count
equality, `iroha_config` binding, payload-free inclusion flags, and checker
prevalidation stay consistent with the promotion gate. The payload-free controller-runtime and moderation-toggle canary builder does not replace the missing deployed controller daemon or toggle service; it only standardizes reviewed promotion evidence once those boundaries produce deployment facts.

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
