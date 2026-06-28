---
lang: pt
direction: ltr
source: docs/source/sorafs_release_pipeline_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: dfabd1c93c1d4536051cc5975c70eba195bd72a9ff153ad6f972cc38a9162706
source_last_modified: "2026-06-25T16:58:37+00:00"
translation_last_reviewed: 2026-01-30
---

# SoraFS CLI/SDK Release & Testing Pipeline

## Goals
- Automate build/test/release workflow for CLI and SDK packages.
- Ensure deterministic builds, signed artefacts, and changelog updates.
- Provide regression suites covering manifest build, CAR pack, proof streaming,
  and orchestrator integration.

## Pipeline Phases

1. **Lint & Format**
   - `cargo fmt`, `cargo clippy`, `eslint`, `go fmt` as applicable.
2. **Unit Tests**
   - CLI command tests (manifest parsing, proof request construction, signing,
     verification, and release guards).
   - SDK unit tests (API contracts).
3. **Integration Tests**
   - Run mock provider harness (SF-6c) + orchestrator tests.
   - Chunk-range smoketest.
   - Proof streaming checks.
4. **Packaging**
   - Build CLI binaries (Linux/macOS/Windows) via cross.
   - Package SDK surfaces that are present in this repository, including
     JavaScript, JVM/Android, Swift, C#, Python, and Rust crate artifacts.
5. **Signing & Attestation**
   - Sigstore `cosign sign-blob` for binaries.
   - SBOM/provenance generation for artifacts that have committed packaging hooks.
6. **Release Publishing**
   - Create Git tag, update changelog.
   - Publish committed package channels and governance release notes; add new
     package-channel workflows before advertising them.
   - Post release notes to governance channels.

## Testing Matrix

| Suite | Description | Trigger |
|-------|-------------|---------|
| `ci` | Lint + unit tests | PR |
| `integration` | Mock providers + orchestrator | nightly & release candidate |
| `self-cert` | Gateway conformance harness | release candidate |
| `smoketest` | Chunk-range CLI run | post-release |

## Tooling

- `ci/check_sorafs_cli_release.sh` is the committed local release gate for
  formatting, Clippy, shell syntax, and focused SoraFS crate tests.
- `docs/examples/sorafs_ci.md` carries the GitHub Actions release workflow
  template. Commit `.github/workflows/sorafs-cli-release.yml` from that
  template during release cutover if GitHub-hosted packaging is required.
- SDK companion guard lanes are already wired through
  `.github/workflows/pr_sorafs_pin_register_sdk.yml` and
  `ci/check_sorafs_pin_register_sdk_guard.sh`, with Swift, JVM, C#,
  JavaScript, and Python checks delegated to the matching
  `ci/check_sorafs_pin_register_*_sdk.sh` scripts.
- Orchestrator SDK parity runs through `ci/sdk_sorafs_orchestrator.sh` so
  multi-source fetch, proof, and pin-registration client behavior stays tied
  to the release matrix.
- Config templates for scripted runs live under `docs/examples/` (for example,
  `sorafs_cli_release.conf`), and both helper scripts fall back to the
  `fixtures/sorafs_manifest/ci_sample/` dataset so dry-run executions require no
  additional setup.
- Existing workflow steps call repository scripts directly; add reusable
  composite actions only when multiple committed release workflows share the
  same signing/publishing sequence.
- The repository `Jenkinsfile` remains the heavier integration/soak-test path;
  mirror the release-gate command order there when adding SoraFS stages.
- `scripts/release_sorafs_cli.sh` wraps `sorafs_cli manifest sign` and
  `manifest verify-signature`, producing signing/verification summaries so the
  release job fails fast if bundle metadata drifts before artefacts publish.
- `scripts/package_sorafs_validate_release.sh` builds or packages
  `sorafs-validate` into `dist/sorafs-validate-release/`, stages the checked
  `include/sorafs_reference.h` C FFI header for downstream SDK bindings,
  records binary/header/archive SHA256 digests, records manifest and staged-file
  digests, can emit a detached manifest signature with
  `--manifest-signing-key`, and runs fixture smoke checks before archive
  creation. Archive creation uses a metadata-normalized tar/gzip writer
  (sorted entries, zero mtime, fixed uid/gid, deterministic file modes) so
  identical staged inputs reproduce the same archive hash. Generated `dist/*`
  artifacts remain untracked; keep only `dist/.gitkeep` committed.
- `scripts/sorafs_gateway_self_cert.sh` can be invoked post-deploy with
  `--manifest`/`--manifest-bundle` to ensure staging gateways continue to serve
  the signed manifest expected by clients.

## Versioning Policy

- **CLI binaries (`sorafs_cli`, `sorafs_fetch`, and release artifacts named
  `sorafs-cli`)** follow SemVer. Breaking CLI flags, output schemas, or Norito
  layouts require a major bump once the first public major is cut; additive
  commands and fixtures require a minor bump; fixes and docs-only release notes
  use patch bumps.
- **Rust SoraFS crates** (`sorafs_manifest`, `sorafs_car`, `sorafs_chunker`,
  `sorafs_orchestrator`, and `sorafs_node`) should stay aligned in release
  notes even when Cargo crate versions move independently inside the workspace.
- **SDK surfaces** currently covered by the pin-register guard are Swift,
  JVM/Android, C#, JavaScript (`javascript/iroha_js`), and Python
  (`python/iroha_python`). Version and publish them through their package-native
  tooling, but keep release notes tied to the same fixture and schema hashes as
  the CLI.
- There is no committed `release/version-map.toml` in this checkout. Add that
  file together with the workflow step that consumes it if cross-package
  publishing starts requiring a machine-readable version map.

## SBOM & Vulnerability Scanning

- The committed SoraFS CLI gate currently covers formatting, Clippy, shell
  syntax checks, the `sorafs_reference.h`/`reference_ffi.rs` header-contract
  guard, and focused tests. Public artifact publishing should add SBOM
  generation and signing to the same scripts before the artifacts are
  advertised.
- Existing repository patterns to reuse include `scripts/js_sbom_provenance.sh`,
  `scripts/android_sbom_provenance.sh`, and the docs portal `syft` packaging
  path. Keep generated SBOMs and signatures beside the release hashes in the
  governance ticket.
- If release scanning needs central exceptions, add
  `security/vuln-exceptions.yaml` with expiry and approval fields in the same
  change that enforces the scanner. Do not reference exception files that are
  not committed.

## Changelog Automation

- Current SoraFS release evidence comes from `scripts/release_sorafs_cli.sh`,
  `scripts/package_sorafs_validate_release.sh`, `status.md`, `roadmap.md`, and
  the governance release notes template. Keep the package archive, manifest,
  manifest signature when produced, and all SHA256 sidecars hash-addressed in
  the release ticket.
- Add package-native changelog tooling in the same change as any new public SDK
  publishing workflow. Do not reference `.changeset`,
  `scripts/update_rust_changelog.sh`, or `scripts/update_go_changelog.sh` until
  those files are committed and wired into CI.
- Generated changelog updates should be reviewed with the release branch before
  publishing artifacts.
