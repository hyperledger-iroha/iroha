---
title: SF-6 Security Review
summary: Findings and follow-up items from the independent assessment of release signing, proof streaming, and manifest submission pipelines.
---

# SF-6 Security Review

**Assessment window:** 2026-02-10 → 2026-02-18
**Review leads:** Security Engineering Guild (`@sec-eng`), Tooling Working Group (`@tooling-wg`)
**Scope:** SoraFS CLI/SDK (`sorafs_cli`, `sorafs_car`, `sorafs_manifest`), proof streaming APIs, Torii manifest handling, raw-Ed25519 release authentication, provenance, and CI release hooks.
**Artifacts:**
- CLI source and tests (`crates/sorafs_orchestrator/src/bin/sorafs_cli.rs`)
- Torii manifest/proof handlers (`crates/iroha_torii/src/sorafs/api.rs`)
- Release automation (`ci/check_sorafs_cli_release.sh`, `scripts/release_sorafs_cli.sh`)
- Deterministic parity harness (`crates/sorafs_orchestrator/tests/sorafs_cli.rs`, `specs/sorafs/reports/orchestrator_ga.md`)

> **Current qualification note (audited 2026-08-03):** This dated review is
> historical input, not evidence that the current release tree passed its fuzz,
> vulnerability, or deployment gates. Those gates remain open until their
> pinned workflows execute and archive genuine results for the final source.

## Methodology

1. **Threat modelling workshops** mapped attacker capabilities for developer workstations, CI systems, and Torii nodes.
2. **Code review** focused on release-key custody, signer/verifier pinning, Norito manifest validation, provenance separation, and proof streaming back-pressure.
3. **Dynamic testing** was recorded as replaying fixture manifests and simulating failure modes (token replay, manifest tampering, truncated proof streams) with the parity harness and bespoke fuzz drives. This historical methodology statement is not current fuzz-run evidence.
4. **Configuration inspection** validated `iroha_config` defaults, CLI flag handling, and release scripts to ensure deterministic, auditable runs.
5. **Process interview** confirmed remediation flow, escalation paths, and audit evidence capture with Tooling WG release owners.

## Findings Summary

| ID | Severity | Area | Finding | Resolution |
|----|----------|------|---------|------------|
| SF6-SR-01 | High | Release signing | The retired CLI path derived an ephemeral key from unverified OIDC token bytes and trusted a public key carried in the same self-asserted bundle. | Removed that CLI surface and all production callers. Releases now require `signing_provider=authenticated_external_signer` with exact `signing_backend=software`, a governed raw Ed25519 public key and reviewed fingerprint, plus a SHA256-pinned native `sorafs-validate release-manifest` receipt reporting `signer_qualification=software-key-qualified` (`specs/sorafs/developer/releases.md`). |
| SF6-SR-02 | Medium | Proof streaming | Back-pressure paths accepted unbounded subscriber buffers, enabling memory exhaustion. | `sorafs_cli proof stream` enforces bounded channel sizes with deterministic truncation, logging Norito summaries and aborting the stream; Torii mirror updated to bound response chunks (`crates/iroha_torii/src/sorafs/api.rs`). |
| SF6-SR-03 | Medium | Manifest submission | CLI accepted manifests without verifying embedded chunk plans when `--plan` was absent. | `sorafs_cli manifest submit` now recomputes and compares CAR digests unless `--expect-plan-digest` is provided, rejecting mismatches and surfacing remediation hints. Tests cover success/failure cases (`crates/sorafs_orchestrator/tests/sorafs_cli.rs`). |
| SF6-SR-04 | Low | Audit trail | Release checklist lacked a signed approval log for the security review. | Added `specs/sorafs/developer/releases.md` section requiring attachment of review memo hashes and sign-off ticket URL before GA. |

All high/medium findings were fixed during the review window and validated through the existing parity harness. No latent critical issues remain.

## Control Validation

- **Release authenticity:** The release helper requires the complete signer and
  pinned-verifier tuple, rejects unsafe inputs, and fails closed on any raw
  Ed25519 or native verification error. OIDC/cosign remains provenance only.
- **Deterministic replay:** Updated tests cover positive/negative manifest submission flows, ensuring mismatched digests remain non-deterministic failures and are surfaced before touching the network.
- **Proof streaming back-pressure:** Torii now streams PoR/PoTR items over bounded channels, and the CLI retains only truncated latency samples + five failure exemplars, preventing unbounded subscriber growth while keeping deterministic summaries.
- **Observability:** Proof streaming counters (`torii_sorafs_proof_stream_*`) and CLI summaries capture abort reasons, providing operators with audit breadcrumbs.
- **Documentation:** Developer guides (`specs/sorafs/developer/index.md`, `specs/sorafs_cli.md`) call out security-sensitive flags and escalation workflows.

## Release Checklist Additions

Release managers **must** attach the following evidence when promoting a GA candidate:

1. Hash of the latest security review memo (this document).
2. Link to the tracked remediation ticket (e.g., `governance/tickets/SF6-SR-2026.md`).
3. Output of `scripts/release_sorafs_cli.sh` showing the reviewed signer
   fingerprint, native-verifier SHA256, and successful raw-Ed25519 verification.
4. Captured logs from the parity harness (`cargo test -p sorafs_orchestrator --test sorafs_cli proof_stream_consumes_ndjson_and_reports_metrics -- --nocapture`).
5. Confirmation that Torii release notes include bounded proof streaming telemetry counters.

Failure to collect the artefacts above blocks GA sign-off.

**Historical reference artefact hashes (2026-02-20 sign-off; not the digest of
this superseding file):**

- `sf6_security_review.md` — `66001d0b53d8e7ed5951a07453121c075dea931ca44c11f1fcd1571ed827342a`

## Outstanding Follow-ups

- **Threat model refresh:** Repeat this review quarterly or before major CLI flag additions.
- **Fuzzing coverage:** A proof-stream libFuzzer target exists in source at
  `fuzz/proof_stream_transport.rs` for identity, gzip, deflate, and zstd. It is
  not currently release-wired or qualified; executing the pinned target and
  archiving its results remains outstanding.
- **Incident rehearsal:** Schedule an operator exercise simulating token compromise and manifest rollback, ensuring documentation reflects practised procedures.

## Approval

- Security Engineering Guild representative: @sec-eng (2026-02-20)
- Tooling Working Group representative: @tooling-wg (2026-02-20)

Store signed approvals alongside the release artefact bundle.
