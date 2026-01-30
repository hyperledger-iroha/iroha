---
lang: he
direction: rtl
source: docs/source/sorafs_reference_sdk_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 177db0418a41419270d116c04bc9a9d451c2bd790741ba19771160291cd27b7c
source_last_modified: "2026-01-04T10:50:53.683804+00:00"
translation_last_reviewed: 2026-01-30
---

---
title: SoraFS Reference SDK & Validator
summary: Final specification for SF-11 covering crate architecture, CLI surface, error contracts, policy defaults, integrations, and testing.
---

# SoraFS Reference SDK & Validator

## Goals & Scope
- Ship a Rust reference crate and CLI that sign, verify, and enforce SoraFS governance policies for adverts, replication orders, PoR/PoTR artefacts, repairs, and governance envelopes.
- Provide deterministic machine-readable outcomes so operators, auditors, and CI systems can gate deployments without bespoke tooling.
- Expose bindings and artefacts other language teams can reuse (via FFI or generated code) while keeping Norito as the only canonical wire format.

This specification completes **SF-11 — Reference SDK + validators** and supersedes the previous draft.

> **Status:** specification only. The `crates/sorafs_reference` library and the
> `tools/sorafs-validate` CLI are not yet implemented in the workspace; see
> `roadmap.md` for the delivery schedule.

## Architecture Overview
| Component | Purpose | Notes |
|-----------|---------|-------|
| `crates/sorafs_reference` | Core library providing validation/signing helpers, policy enforcement, and error outcomes. | Depends on `sorafs_manifest`, `sorafs_car`, `sorafs_por` modules; no duplicate codecs. |
| `tools/sorafs-validate` (binary) | CLI wrapping the reference crate with task-focused subcommands and consistent output. | Built using `clap` + `norito::json` (no direct `serde_json`). |
| `ffi/` helpers | Optional C ABI surface for SDKs (Go/Swift/Node) built on top of the Rust crate. | Simple facade returning `ValidationOutcomeV1` Norito JSON. |
| `docs/examples/sorafs_reference_sdk/` | Planned cookbook with ready-to-run scenarios and sample payloads. | Mirrors CI fixtures once implemented. |

Internal modules (library):
- `validator::advert`, `validator::order`, `validator::por`, `validator::potr`, `validator::repair`, `validator::governance`.
- `policy` module encapsulates default thresholds (TTLs, SLAs, retry budgets).
- `outcome` module implements `ValidationOutcomeV1` (see below).
- `signing` module exposes builders for adverts, orders, and governance envelopes, ensuring domain separation strings stay centralised.

## CLI Surface
All commands accept `--format {table,json,yaml}` (default `table`) and `--telemetry-out <path>` to write the raw `ValidationOutcomeV1`.

| Command | Description | Key flags | Input |
|---------|-------------|-----------|-------|
| `sorafs-validate advert` | Validate `ProviderAdvertV1` payloads (signature, TTL, capability set). | `--input <file>` (`.to` or `.json`), `--policy <file>` overrides defaults, `--allow-missing-vrf` optional. | Norito bytes or JSON commentary. |
| `sorafs-validate admission` | Verify onboarding/renewal/revocation envelopes. | `--input <file>`, `--governance-keyset <path>` for council keys. | Norito bytes. |
| `sorafs-validate order` | Check replication orders (manifests, chunk plan digests, provider assignments). | `--order <file>`, `--manifest <file>` (optional: validate cross-links), `--car <file>` ensures CAR hash matches. | Norito + CAR. |
| `sorafs-validate por` | Replay PoR challenge/proof pair, enforce SLA window, attach repair linkage. | `--challenge <file>`, `--proof <file>`, `--manifest <file>`, `--epoch <id>` override. | Norito JSON bundle. |
| `sorafs-validate potr` | Validate PoTR probe/receipt (deadline proofs). | `--probe <file>`, `--receipt <file>`, `--profile hot|warm|cold`. | Norito JSON. |
| `sorafs-validate repair` | Inspect `RepairTaskV1` envelopes and ensure evidence references exist. | `--task <file>`, `--fixtures-dir <path>`. | Norito JSON. |
| `sorafs-validate governance` | Decode `GovernanceLogNodeV1`, verify signatures, check CID. | `--node <file>`, `--cid <cid>`, `--dag <car>` optional. | Norito bytes. |
| `sorafs-validate bundle` | Run a composite check on a manifest bundle (advert + order + manifest + proofs + governance). | `--bundle <dir>`, `--policy <file>`, `--skip-por` optional. | Directory matching CI sample layout. |
| `sorafs-validate sign` | Produce signed adverts/orders/governance documents using operator or governance keys. | `--kind advert|order|governance`, `--input <json>`, `--key <path>`. | JSON template -> Norito output. |

Exit codes: `0` success, `2` validation/policy/signature errors, `3` I/O errors, `4` configuration errors, `10` internal faults.

## Validation Policies
Default policy constants (override via Norito `PolicyOverrideV1` or CLI `--policy` file):
- Advert TTL ≥ 24h, ≤ 14 days; `provider_id` must match governance registry; capability set must include `chunk-range`, `trustless`, `por`.
- Admission envelopes must reference approved senior council keyset (multi-signature) and admission window ≤ 7 days.
- Replication orders validate:
  - Manifest digest and CAR digest equality (Blake3).
  - Order version supported by local `sorafs_manifest`.
  - Pricing schedule conforms to `sorafs_pricing.md` tiers.
- PoR:
  - Challenge/proof share identical seed, epoch, and manifest digest (via `derive_challenge_seed`).
  - Response deadline ≤ 10 minutes after issue time (policy override allowed down to 5, up to 20 with governance flag).
  - Sample coverage meets tier thresholds from SF-9.
- PoTR:
  - Probe durations respect hot/warm/cold SLAs (90s / 5m / 30m).
  - Receipt signature matches provider key; aggregated proofs cross-check with orchestrator metrics if provided.
- Repair tasks:
  - Evidence matches PoR/PoTR failure digests.
  - Escalation reason required for `escalated` status; `repair_task_id` must exist in history.
- Governance nodes:
  - Node CID recalculated and compared against input.
  - Payload kind resolved; unsupported types flagged.

Policy enforcement occurs in the `policy` module; CLI exposes `--policy` to load YAML/JSON override with explicit allowlist (fields: `advert_ttl_min`, `por_deadline_max`, etc.). Overrides require signature if `--require-signed-policy` is set (used in production pipelines).

## Error & Outcome Contract
`ValidationOutcomeV1` (Norito + JSON) includes:
```norito
struct ValidationOutcomeV1 {
    status: ValidationStatus, // Ok | Error
    code: String,             // SFS-VAL-001 etc.
    category: ValidationCategory, // validation | policy | signature | io | norito | internal
    message: String,
    action: Option<String>,
    docs_url: Option<String>,
    telemetry_tags: Vec<String>,
    context: ValidationContextV1,
    inputs: Vec<ValidationInputV1>, // file paths / CIDs
    version: u8,                    // outcome schema version
    generated_at: Timestamp,
}
```
`ValidationContextV1` is a map of structured fields (manifest CID, provider ID, etc.). The outcome is printed by CLI (`table` format compresses key fields).

Error catalogue (initial codes):
- `SFS-VAL-001` manifest digest mismatch.
- `SFS-VAL-002` unsupported schema version.
- `SFS-VAL-003` chunk profile incompatibility.
- `SFS-SIG-001` invalid signature.
- `SFS-POL-001` advert TTL violation.
- `SFS-POL-002` PoR deadline exceeded.
- `SFS-POR-001` missing sample coverage.
- `SFS-POR-002` proof sample digest mismatch.
- `SFS-POTR-001` deadline proof late.
- `SFS-REP-001` repair evidence missing.
- `SFS-GOV-001` governance key mismatch.
- `SFS-IO-001` input read failure.
- `SFS-NORITO-001` decode error.
- `SFS-INT-001` internal panic/unexpected.

CLI prints `docs_url` pointing to `docs/portal/docs/sorafs/reference-sdk/errors.md`.

## Library API Highlights
- `AdvertValidator::validate(&[u8], &Policy) -> ValidationOutcome`.
- `OrderValidator::validate(order_bytes, manifest_bytes, car_reader, policy)`.
- `PorValidator::validate(challenge, proof, manifest, policy)`.
- `Signer::sign_advert(advert, keypair) -> SignedAdvert`.
- `OutcomeRecorder::emit(outcome, writer)` for journaling results (JSONL).
- `FFI` functions: `sorafs_validate_advert`, `sorafs_validate_order`, etc., returning JSON strings (allocated via `CString`).

Public APIs avoid direct filesystem/CLI dependencies; CLI crate handles file I/O, streaming to validators via `Read` trait objects to support large payloads.

## Integration & Automation
- **CI template:** `ci/validate_sorafs_payloads.yml` runs CLI against fixtures in PRs.
- **Git hooks:** optional pre-commit to invoke `sorafs-validate bundle` when manifest files change.
- **Torii integration:** reference implementation for `POST /sorafs/validate` uses the library to validate uploads prior to registry inclusion.
- **Grafana dashboards:** aggregator ingests `telemetry_tags` to create `sorafs_reference_failures_total{code=...}` metrics; CLI emits `--telemetry-out` JSON for scraping.
- **Crash reporting:** optional `--on-error <script>` hook to integrate with incident tooling; script receives path to JSON outcome.

## Testing & Fixtures
- Unit tests cover policy edge cases, error codes, and positive paths.
- Golden tests compare CLI stdout (`--format table`) and JSON outputs for known fixtures.
- Integration tests replay CI bundles (`fixtures/sorafs_manifest/ci_sample*`) verifying combined validation.
- Cross-platform harness ensures CLI runs on macOS, Linux, Windows with the same outcomes (sha256 of JSON).
- Fuzzers: `cargo fuzz run advert_decode` verifying Norito decoding resilience.

## Documentation Requirements
- `docs/source/sorafs/reference_sdk_operator.md`: step-by-step operator guide.
- `docs/source/sorafs/reference_sdk_metrics.md`: telemetry mapping and Prometheus configuration.
- `docs/source/sorafs/reference_sdk_api.md`: library API doc (with doc comment extraction).
- Planned: `docs/examples/sorafs_reference_sdk/` sample scripts and README.
- Update docs portal to add Reference SDK section with error catalogue and CI integration notes.

## Packaging & Release
- Publish crate under `sorafs-reference-sdk` (Crates.io) with SemVer matching Norito schema major version.
- Provide static binaries (`sorafs-validate`) for x86_64/aarch64 macOS + Linux in release pipeline.
- Sign binaries with reproducible build pipeline (deterministic `cargo dist`).
- Release notes template references new/changed validations and error codes.

## Implementation Checklist
- [x] Document architecture and module responsibilities.
- [x] Specify CLI commands, flags, and output formats.
- [x] Define policy defaults and override mechanisms.
- [x] Finalise error/outcome contract and code table.
- [x] Outline API surface (Rust + FFI) and integration hooks.
- [x] Detail testing, fixtures, and CI expectations.
- [x] Capture documentation, packaging, and release tasks.

Delivering this specification, alongside the PoR/repair schemas, provides a complete blueprint for the reference SDK and validator tooling required by SF-11. Implementation teams can now proceed without ambiguity while maintaining deterministic behaviour across platforms.
