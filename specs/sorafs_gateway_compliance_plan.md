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
accepted-appeal and history state; and evaluates legal/safety holds before
accepted appeals and baseline policy. It also defines an address-pinned runtime
feed-transport boundary with HTTPS allowlists, public-address validation, DNS
revalidation, SPKI pins, redirect, size, time, and decompression limits.

`iroha_config` now carries the non-secret controller policy, feed allowlists and
pins, resource bounds, checkpoint path, governed signer identities, canonical
region and gateway deployment identities, and an independent exact
handle/revision/policy-digest binding for the feed transport. ACME has its own
separate exact provider binding. Torii accepts runtime-injected ACME and
authenticated feed transports, transfers them through `IrohaRuntimeDeps` and
`ToriiRuntimeDeps`, constructs the durable controller from the exact resolved
configuration, retains it in `AppState`, and fails closed when an enabled
runtime dependency is absent, unavailable, substituted, stale, test-marked, or
only partially configured. Each DNS resolution, HTTPS fetch, and ACME order is
fenced by identity checks before and after the operation; returned addresses,
response bytes, certificates, and keys are discarded on drift. This
controller/runtime integration ships locally. The standard launcher does not
construct a process-local feed transport or ACME fallback: configured provider
bindings require the exact deployment-owned injected instances before Torii
startup.

Torii now exposes six canonical, account-signed, governed-operator routes:
authenticated feed fetch and durable status reads plus canonical-Norito-JSON
stage, acknowledgement, promotion, and rollback mutations. Live SoraFS content
serving evaluates the promoted catalog for manifest digest, canonical CID, and
provider subjects across global, configured-region, and configured-gateway
scopes. Missing, stale, or poisoned serving state fails closed. Enabling the
governed controller also rejects every obsolete unsigned denylist path,
catalog, pack, and jurisdiction bootstrap source, so no configured local
authority can compete with the signed durable catalog. V1 has no local
compliance packs, file-backed compliance authority, catalog-mutation CLI, or
unsigned compatibility route.

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
  decoded, redirect, DNS-answer, and elapsed-time inputs. Its independent
  `iroha_config` binding pins a production handle, non-zero revision, and the
  non-zero digest of the exact canonical hostname/SPKI inventory. Startup and
  every individual resolve/fetch operation check that identity before and
  after use.
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
  `iroha_config`. Only stable handles, non-zero revisions, and lowercase
  non-zero public-policy digests are configured. Torii maps every resolved
  field into the ACME/controller runtime, constructs the durable controller,
  retains the controller and transport in `AppState`, rejects dependencies
  injected while their feature is disabled, and refuses startup when an
  enabled dependency or exact provider binding is missing or mismatched.
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
- `scripts/check_sorafs_gateway_compliance_rollout_evidence.py` validates
  payload-free SFM-4 promotion evidence for catalog promotion, controller
  runtime, governed moderation controls, gateway reload, exact denial probes,
  adversarial probes, precedence, transparency publication, observability, and
  governance approval. The checker exports its exact top-level payload fields
  as `EVIDENCE_REQUIRED_FIELDS`, and the companion
  `scripts/run_sorafs_gateway_compliance_rollout_evidence.py` runner emits the
  verifier command and dry-run collection plan from reviewed artifact paths,
  including a checker-backed `evidence_contract` map with the schema and
  required payload fields for each selected evidence kind.
  Every non-promotion artifact must carry the same lowercase
  `catalog_digest_hex` as the single valid `catalog_promotion` artifact.
  Promotion evidence binds the digest to
  `promoted_catalog_digest_hex`, a non-zero
  `predecessor_catalog_digest_hex`, a contiguous
  `predecessor_catalog_sequence`, threshold-verified unique signers, bounded
  `catalog_entries` and `catalog_changes`, and at least two signed
  acknowledgements from gateways with distinct `administration_id` values.
  Acknowledgements also carry distinct canonical `region_id` values for the two
  regional deployment boundaries.
  Every acknowledgement must name the promoted catalog digest; split-gateway
  catalog evidence fails closed.
  Controller and gateway-reload evidence carry both
  `predecessor_catalog_digest_hex` and `predecessor_catalog_sequence`; those
  fields must match the promotion's complete current/predecessor catalog
  history, and the reload rollback digest must name that same predecessor.
  Controller evidence also carries bounded, signed `source_anchors` and proves
  predecessor validation, durable history reconciliation, last-known-good
  recovery, and atomic replacement. Enforcement and adversarial probe rows must
  record exact HTTP 451 responses with
  `error = "gateway_compliance_denied"`, a recognized `source` of `baseline` or
  `legal_safety_hold`, the lowercase promoted catalog digest, and the canonical
  private no-store cache policy. Enforcement source coverage is derived from
  `routes[].source` and must include both recognized denial paths. Adversarial
  coverage is derived from `probes[].attack` and must include exactly
  `stale_catalog`, `wrong_predecessor`, `invalid_signature`, and
  `split_gateway_catalog`; separate declared coverage counts or inventories are
  not accepted. Old status codes, response codes, headers, and local-policy
  fields are rejected.
  Precedence evidence must demonstrate
  `legal_safety_hold > accepted_appeal > baseline` from a finalized-chain
  projection. Governance approval must match the promotion's
  `policy_digest_hex`.
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
  lane-proven digest relationships: catalog-bound artifact fingerprints must
  match `valid_catalog_digests`, predecessor-bound controller and reload
  fingerprints must match the single atomic current/predecessor object in
  `valid_catalog_history_bindings`, and policy-bound artifact fingerprints must
  match `valid_policy_digests`. Gateway compliance rollout summaries must
  expose exactly one active promoted catalog digest, exactly one complete
  catalog-history binding, and exactly one active policy digest; mixed catalog,
  predecessor, or policy anchors fail closed before final promotion can report
  ready.
- `scripts/build_sorafs_gateway_compliance_canary.py` is a payload-free
  canonicalizer, not an evidence generator. It requires a bounded
  `--probe-artifact` produced by a real authenticated probe, validates every
  observed field through the release checker, and requires explicit
  `--deployment-id`, `--environment`, and `--generated-at-unix` values that
  exactly match the probe before atomically writing canonical JSON without
  following symlinks. It never creates positive verification claims.
  `--non-production-fixture` changes `status` and `evidence_scope` so fixture
  output is explicit and cannot satisfy the release gate. The checked-in
  examples use only those marked non-production fixtures; operators must
  replace the fixture path and remove the flag for genuine evidence.

## Operator Commands

Catalog control is API-only in V1. Operators inspect authenticated feed and
status reads, submit a bounded canonical catalog to `stage`, collect distinct
gateway acknowledgements, call `promote`, and use `rollback` only for the
durable last-known-good catalog. Catalog construction and threshold signing
remain external to Torii; no local pack, diff, verify, or mutation CLI is
supported.

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

Canonicalize observed payload-free gateway-compliance probes:

```sh
python3 scripts/build_sorafs_gateway_compliance_canary.py \
  @scripts/examples/sorafs_gateway_compliance_catalog_promotion_canary.args.example
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
  @scripts/examples/sorafs_gateway_compliance_precedence_canary.args.example
python3 scripts/build_sorafs_gateway_compliance_canary.py \
  @scripts/examples/sorafs_gateway_compliance_transparency_publication_canary.args.example
python3 scripts/build_sorafs_gateway_compliance_canary.py \
  @scripts/examples/sorafs_gateway_compliance_observability_canary.args.example
python3 scripts/build_sorafs_gateway_compliance_canary.py \
  @scripts/examples/sorafs_gateway_compliance_governance_approval_canary.args.example
```

Rollout evidence must remain payload-free. The gate rejects source documents,
probe response bodies, accepted-appeal records, signed transactions, tokens,
private keys, and other raw payloads. Every downstream artifact binds to the
single promoted signed catalog with `catalog_digest_hex`; governance approval
also binds `policy_digest_hex`. The gate records mismatches on the offending
artifact. The runner's dry-run `evidence_contract` exposes the exact schema and
command plan; the plan itself is never promotion evidence.
The canonicalizer only accepts already observed probe artifacts and cannot
replace authenticated source collection, finalized appeal/hold publication,
threshold signing, or independently administered gateway deployment.
Gateway compliance readiness schemas are exact. Removed local payload flags,
local catalog/report fields, legacy response headers, and unknown fields are
rejected rather than represented as false booleans.
Moderation-toggle evidence also records a successful
`control_api_status_code`; non-2xx control responses fail the lane.

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
- Supply independently audited threshold-signing integration for governed
  source catalogs. The controller deliberately holds no signing key, source
  credential, ACME account credential, or DNS-provider credential and cannot
  invent those production dependencies.
- Wire deployed GAR receipts and moderation events through
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
