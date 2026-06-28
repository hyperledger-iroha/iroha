---
lang: ba
direction: ltr
source: docs/source/sorafs_gateway_compliance_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 2097b9161df4a9ab8c6b8cbfc5b526cfc971c424e3144da15ae4b305030a583c
source_last_modified: "2026-06-25T17:24:16+00:00"
translation_last_reviewed: 2026-06-25
title: Gateway Compliance, Moderation & Transparency
summary: SFM-4 implementation status for gateway denylist enforcement, GAR policy, proof tokens, honey-audit evidence, and remaining compliance services.
---

# Gateway Compliance, Moderation & Transparency

## Current Status

SFM-4 is partially implemented. The gateway enforcement path, denylist helpers,
GAR policy payloads, proof-token utilities, honey-audit probing, operator bundle
tooling, and payload-free rollout evidence gate exist. The repository does not
yet ship a central compliance controller daemon, moderation toggle service,
SFM-4c transparency ledger builder, public receipt explorer, or full
appeal-driven override workflow.

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
  payload-free SFM-4 promotion evidence for feed promotion, gateway reload,
  enforcement probes, honey-audit denial proof, appeal override, transparency
  publication, observability, and governance approval artifacts. The companion
  `scripts/run_sorafs_gateway_compliance_rollout_evidence.py` runner emits the
  verifier command and dry-run collection plan from reviewed artifact paths.
  Reload, enforcement, honey-audit, appeal, transparency, observability, and
  governance artifacts must carry the same `bundle_digest_hex` as a valid
  feed-promotion artifact in the same bundle, so promotion evidence cannot mix
  probes, dashboards, or approvals from different denylist bundle runs. Bundle
  mismatches are recorded on the offending artifact in the JSON summary before
  required-kind validity is reported.
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

Rollout evidence must remain payload-free. The gate rejects raw denylist feeds,
probe response bodies, GAR receipts, appeal payloads, signed transactions,
tokens, private keys, and response bodies; operators should provide only
digests, counts, stable labels, booleans, and reviewed artifact paths. Every
downstream reload/probe/audit/appeal/transparency/observability/governance
artifact must bind back to the promoted denylist bundle with
`bundle_digest_hex`. If that binding does not match a valid feed-promotion
artifact, the gate marks the downstream artifact invalid in the emitted summary
instead of only blocking the top-level status.

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

- Ship a central compliance controller that fetches external feeds, normalizes
  updates, signs them, distributes them to gateways, and tracks acknowledgements.
- Persist denylist/catalog state and update history through the configured
  production storage path instead of relying only on local bundle ingestion.
- Implement moderation toggle APIs, approval workflows, expiry handling, and
  operator audit trails.
- Connect appeal outcomes to gateway policy overrides and cache invalidation.
- Publish GAR receipts, proof-token indexes, and moderation events through the
  SFM-4c transparency ledger once that builder exists.
- Capture staged multi-gateway rollout artifacts that satisfy the SFM-4 evidence
  gate before promoting gateway compliance changes to production.

## Validation

Focused local checks for the shipped surface are:

```sh
ci/check_sorafs_gateway_denylist.sh
python3 -m pytest -q \
  scripts/tests/check_sorafs_gateway_compliance_rollout_evidence_test.py \
  scripts/tests/run_sorafs_gateway_compliance_rollout_evidence_test.py
cargo test -p iroha_torii sorafs::gateway
cargo test -p sorafs_car policy
cargo test -p sorafs_orchestrator moderation
```

Run the broader gateway conformance suite before changing response bodies,
headers, or policy-denial telemetry labels.
