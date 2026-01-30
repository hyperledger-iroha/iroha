---
lang: es
direction: ltr
source: docs/source/sorafs_release_pipeline_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 5cf9307516128252a10539647b7865bb134fd62bef96c73802c8f2a2ac94c472
source_last_modified: "2026-01-04T10:50:53.684167+00:00"
translation_last_reviewed: 2026-01-30
---

---
title: SoraFS CLI/SDK Release & Testing Pipeline (Draft)
summary: Outline for SF-6 release automation and QA.
---

# SoraFS CLI/SDK Release & Testing Pipeline (Draft)

## Goals
- Automate build/test/release workflow for CLI and SDK packages.
- Ensure deterministic builds, signed artefacts, and changelog updates.
- Provide regression suites covering manifest build, CAR pack, proof streaming, and orchestrator integration.

## Pipeline Phases

1. **Lint & Format**
   - `cargo fmt`, `cargo clippy`, `eslint`, `go fmt` as applicable.
2. **Unit Tests**
   - CLI command tests (manifest parsing, proof verification stubs).
   - SDK unit tests (API contracts).
3. **Integration Tests**
   - Run mock provider harness (SF-6c) + orchestrator tests.
   - Chunk-range smoketest.
   - Proof streaming checks.
4. **Packaging**
   - Build CLI binaries (Linux/macOS/Windows) via cross.
   - Package SDKs (npm, crates.io, Go modules).
5. **Signing & Attestation**
   - Sigstore `cosign sign-blob` for binaries.
   - SBOM generation (syft) and signing.
6. **Release Publishing**
   - Create Git tag, update changelog.
   - Publish CLI binaries, crates, npm package, Go module.
   - Post release notes to governance channels.

## Testing Matrix

| Suite | Description | Trigger |
|-------|-------------|---------|
| `ci` | Lint + unit tests | PR |
| `integration` | Mock providers + orchestrator | nightly & release candidate |
| `self-cert` | Gateway conformance harness | release candidate |
| `smoketest` | Chunk-range CLI run | post-release |

## Tooling

- GitHub Actions workflow (`.github/workflows/sorafs-cli-release.yml`) packages
  the CLI signing pipeline; future SDK releases will add companion jobs.
- Config templates for scripted runs live under `docs/examples/` (for example,
  `sorafs_cli_release.conf`), and both helper scripts fall back to the
  `fixtures/sorafs_manifest/ci_sample/` dataset so dry-run executions require no
  additional setup.
- Reusable composite actions for signing/publishing.
- Optionally, Jenkins pipeline for heavier integration tests.
- `scripts/release_sorafs_cli.sh` wraps `sorafs_cli manifest sign` and
  `manifest verify-signature`, producing signing/verification summaries so the
  release job fails fast if bundle metadata drifts before artefacts publish.
- `scripts/sorafs_gateway_self_cert.sh` can be invoked post-deploy with
  `--manifest`/`--manifest-bundle` to ensure staging gateways continue to serve
  the signed manifest expected by clients.

## Versioning Policy

- **CLI (`sorafs-cli`)** — Semantic versioning (`MAJOR.MINOR.PATCH`). Any breaking CLI flag or output change bumps
  `MAJOR`. Additive features bump `MINOR`; bug fixes bump `PATCH`.
- **Rust SDK (`sorafs_sdk`)** — Shares the CLI `MAJOR`/`MINOR` to keep docs aligned; may ship independent patch
- **TypeScript SDK (`@sora-org/sorafs-sdk`)** — Semantic versioning anchored to the CLI major version. Minor
  changes cover additional helpers; patch changes include bug fixes or dependency bumps.
- **Go SDK** — Module-aware tagging (`sorafs-sdk-go/vX.Y.Z`); release workflow updates `go.mod` and pushes tags.
- Release workflow reads `release/version-map.toml` to ensure all components target consistent versions before
  tagging; mismatches fail the run.

## SBOM & Vulnerability Scanning

- Generate SBOMs with `syft` for every artifact (CLI binaries, npm tarball, crate package, Go module).
- Scan SBOMs with `grype` (or Anchore Enterprise). Pipeline fails on high/critical findings unless an exception
  entry exists in `security/vuln-exceptions.yaml` (requires expiry date and approval).
- Additional scanners:
  - `cargo audit` for Rust crates.
  - `npm audit --omit=dev` for TypeScript.
  - `govulncheck ./...` for Go.
- Scan summaries uploaded as build artifacts and posted to the security Slack channel.

## Changelog Automation

- Adopt `changesets` for cross-language change tracking. Contributors add `.changeset/*.md` entries covering the
  CLI and SDKs impacted.
- Release workflow runs:
  1. `pnpm changeset version` → bumps package manifests, generates changelog entries for JS packages.
  2. Custom script `scripts/update_rust_changelog.sh` (wraps `cargo changelog`) to sync Rust `CHANGELOG.md`.
  3. `scripts/update_go_changelog.sh` to append Go module release notes derived from changeset metadata.
- Generated changelog updates are committed automatically to the release branch before publishing artifacts.
