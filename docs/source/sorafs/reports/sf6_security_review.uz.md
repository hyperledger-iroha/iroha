---
lang: uz
direction: ltr
source: docs/source/sorafs/reports/sf6_security_review.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 7dc1f54841072bf9429f73ccb582f4a6afaddafde91a2bbeead4c3e47385d94f
source_last_modified: "2025-12-29T18:16:36.126299+00:00"
translation_last_reviewed: 2026-02-07
title: SF-6 Security Review
summary: Findings and follow-up items from the independent assessment of keyless signing, proof streaming, and manifest submission pipelines.
---

# SF-6 Security Review

**Assessment window:** 2026-02-10 → 2026-02-18  
**Review leads:** Security Engineering Guild (`@sec-eng`), Tooling Working Group (`@tooling-wg`)  
**Scope:** SoraFS CLI/SDK (`sorafs_cli`, `sorafs_car`, `sorafs_manifest`), proof streaming APIs, Torii manifest handling, Sigstore/OIDC integration, CI release hooks.  
**Artifacts:**  
- CLI source and tests (`crates/sorafs_car/src/bin/sorafs_cli.rs`)  
- Torii manifest/proof handlers (`crates/iroha_torii/src/sorafs/api.rs`)  
- Release automation (`ci/check_sorafs_cli_release.sh`, `scripts/release_sorafs_cli.sh`)  
- Deterministic parity harness (`crates/sorafs_car/tests/sorafs_cli.rs`, `docs/source/sorafs/reports/orchestrator_ga.md`)

## Methodology

1. **Threat modelling workshops** mapped attacker capabilities for developer workstations, CI systems, and Torii nodes.  
2. **Code review** focused on credential surfaces (OIDC token exchange, keyless signing), Norito manifest validation, and proof streaming back-pressure.  
3. **Dynamic testing** replayed fixture manifests and simulated failure modes (token replay, manifest tampering, truncated proof streams) using the parity harness and bespoke fuzz drives.  
4. **Configuration inspection** validated `iroha_config` defaults, CLI flag handling, and release scripts to ensure deterministic, auditable runs.  
5. **Process interview** confirmed remediation flow, escalation paths, and audit evidence capture with Tooling WG release owners.

## Findings Summary

| ID | Severity | Area | Finding | Resolution |
|----|----------|------|---------|------------|
| SF6-SR-01 | High | Keyless signing | OIDC token audience defaults were implicit in CI templates, risking cross-tenant replay. | Added explicit `--identity-token-audience` enforcement in release hooks and CI templates (`docs/source/sorafs/developer/releases.md`, `docs/examples/sorafs_ci.md`). CI now fails when the audience is omitted. |
| SF6-SR-02 | Medium | Proof streaming | Back-pressure paths accepted unbounded subscriber buffers, enabling memory exhaustion. | `sorafs_cli proof stream` enforces bounded channel sizes with deterministic truncation, logging Norito summaries and aborting the stream; Torii mirror updated to bound response chunks (`crates/iroha_torii/src/sorafs/api.rs`). |
| SF6-SR-03 | Medium | Manifest submission | CLI accepted manifests without verifying embedded chunk plans when `--plan` was absent. | `sorafs_cli manifest submit` now recomputes and compares CAR digests unless `--expect-plan-digest` is provided, rejecting mismatches and surfacing remediation hints. Tests cover success/failure cases (`crates/sorafs_car/tests/sorafs_cli.rs`). |
| SF6-SR-04 | Low | Audit trail | Release checklist lacked a signed approval log for the security review. | Added `docs/source/sorafs/developer/releases.md` section requiring attachment of review memo hashes and sign-off ticket URL before GA. |

All high/medium findings were fixed during the review window and validated through the existing parity harness. No latent critical issues remain.

## Control Validation

- **Credential scope:** Default CI templates now mandate explicit audience and issuer assertions; the CLI and release helper both fail fast unless `--identity-token-audience` accompanies `--identity-token-provider`.  
- **Deterministic replay:** Updated tests cover positive/negative manifest submission flows, ensuring mismatched digests remain non-deterministic failures and are surfaced before touching the network.  
- **Proof streaming back-pressure:** Torii now streams PoR/PoTR items over bounded channels, and the CLI retains only truncated latency samples + five failure exemplars, preventing unbounded subscriber growth while keeping deterministic summaries.  
- **Observability:** Proof streaming counters (`torii_sorafs_proof_stream_*`) and CLI summaries capture abort reasons, providing operators with audit breadcrumbs.  
- **Documentation:** Developer guides (`docs/source/sorafs/developer/index.md`, `docs/source/sorafs_cli.md`) call out security-sensitive flags and escalation workflows.

## Release Checklist Additions

Release managers **must** attach the following evidence when promoting a GA candidate:

1. Hash of the latest security review memo (this document).  
2. Link to the tracked remediation ticket (e.g., `governance/tickets/SF6-SR-2026.md`).  
3. Output of `scripts/release_sorafs_cli.sh --manifest ... --bundle-out ... --signature-out ...` showing explicit audience/issuer arguments.  
4. Captured logs from the parity harness (`cargo test -p sorafs_car -- --nocapture sorafs_cli::proof_stream::bounded_channels`).  
5. Confirmation that Torii release notes include bounded proof streaming telemetry counters.

Failure to collect the artefacts above blocks GA sign-off.

**Reference artefact hashes (2026-02-20 sign-off):**

- `sf6_security_review.md` — `66001d0b53d8e7ed5951a07453121c075dea931ca44c11f1fcd1571ed827342a`

## Outstanding Follow-ups

- **Threat model refresh:** Repeat this review quarterly or before major CLI flag additions.  
- **Fuzzing coverage:** Proof streaming transport encodings are fuzzed via `fuzz/proof_stream_transport`, covering identity, gzip, deflate, and zstd payloads.  
- **Incident rehearsal:** Schedule an operator exercise simulating token compromise and manifest rollback, ensuring documentation reflects practised procedures.

## Approval

- Security Engineering Guild representative: @sec-eng (2026-02-20)  
- Tooling Working Group representative: @tooling-wg (2026-02-20)

Store signed approvals alongside the release artefact bundle.
