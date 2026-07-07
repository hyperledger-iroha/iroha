---
lang: pt
direction: ltr
source: docs/source/sorafs_release_pipeline_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: f256aaf61e64742a07cbc567fc5cef49e9028602037cdf80a4afd486e9ba9cf3
source_last_modified: "2026-07-03T17:29:04.185345+00:00"
translation_last_reviewed: 2026-07-03
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
  The wrapper rejects symlinked or non-regular bundle, signature, sign-summary,
  and verify-summary targets, plus symlinked output-parent components, before
  invoking the signing CLI so release evidence cannot be written through
  ambiguous filesystem aliases.
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
- `scripts/check_sorafs_production_readiness.py` is the final aggregate
  SoraFS promotion gate over the per-lane rollout/release evidence summaries.
  It requires each selected lane summary to be `ready`, have empty summary and
  artifact/load-error lists, carry fresh reviewed deployment-context
  fingerprints, cover that lane checker's full default required-kind set with
  no extra `required` rows, keep top-level evidence/artifact counts consistent
  with the validated rows, require evidence file counts to match the distinct
  recognized artifact paths, publish threshold metadata as a non-empty
  canonical non-negative integer map that the aggregate row preserves for
  release review, reject extra top-level lane-summary fields outside the
  schema-closed payload-free lane summary contract, validate allowed top-level
  lane metadata as payload-free canonical strings, non-negative integers,
  booleans, objects, and lists with expected container shapes, bind those
  metadata fields to the lane-specific contract that emits them, validate exact
  lowercase-hex binding-list metadata shapes before aggregate promotion, require
  every fingerprint-backed top-level scalar hex, string-list, positive-integer
  list, and tuple binding-list metadata field to declare its owning required
  artifact kind or kinds before aggregate fingerprint matching,
  validate exact lowercase-hex and positive-integer scalar list metadata shapes
  before aggregate promotion, validate governance public-head identifiers as
  lowercase hex list metadata before aggregate promotion, validate exact
  object-list metadata shapes before aggregate promotion, reject exact duplicate
  object-list metadata entries while preserving artifact order, reject
  domain-duplicate object-list metadata identities before aggregate promotion, require every
  object-list metadata field to declare its owning required artifact kind before
  its detail rows can be matched to recognized artifact fingerprints, validate exact
  object metadata shapes before aggregate
  promotion, require set-derived lane metadata lists to be duplicate-free and
  sorted in canonical order, sanitize malformed sensitive-field path diagnostics
  before writing aggregate errors,
  require required-row and artifact schema labels to match the owning checker
  evidence schemas, reject extra required-row fields outside the schema-closed
  payload-free required-row contract, require canonical unique artifact paths
  whose raw or repeatedly percent-decoded components do not contain traversal,
  hidden separators, drive prefixes, URI-scheme-like path tokens, or
  secret-looking labels, require
  lowercase SHA-256 digests, reject explicit artifact `status` labels
  outside successful states such as `passed` or `verified`, reject
  extra artifact-row fields
  outside the schema-closed payload-free
  artifact contract, require per-lane rollout/release checkers to normalize
  artifact row paths through the shared archive-label helper before summary
  rendering, deriving labels relative to evidence directories or safe explicit
  basenames, require top-level
  recognized-artifact inventory and validate it against the per-kind required-row
  artifact counts and `(kind, path, sha256)` identities plus matching required
  artifact metadata instead of ignoring it, validate schema-closed aggregate lane
  rows with canonical path, lowercase SHA-256, count, timestamp, list, and error
  shapes before release review, validate the schema-closed aggregate summary
  envelope before writing the final production-readiness report, require
  aggregate status to match canonical aggregate diagnostics, require ready
  aggregate summaries to carry complete deployment context for a final
  `prod`/`production` environment and only present, valid required rows whose
  deployment context matches the aggregate deployment block, require each
  aggregate required row deployment_id must match aggregate deployment_id, and
  require each aggregate required row environment must match aggregate
  environment,
  validate final
  aggregate required rows for exact present and missing row output contracts,
  validate invalid aggregate required-row metadata before blocked rows are
  emitted for release review,
  require aggregate recognized-summary counts to match present required rows,
  pin deterministic missing-row diagnostics for absent lane summaries, pin
  deterministic duplicate-summary diagnostics for duplicate lane summaries,
  count every duplicate lane-summary input while keeping one duplicate row
  diagnostic per gate, pin aggregate blockers for unknown schemas and explicit
  unrequired summaries, avoid payload or secret-bearing
  fields, reject unknown summary schemas discovered in summary directories,
  require an explicit final `--deployment-id`/`--environment` pair even for
  direct checker invocations, and share the same reviewed `deployment_id` with
  no non-reviewed or staging deployment markers plus a final
  `prod`/`production` `environment`
  before emitting
  `sorafs.production_readiness.aggregate_gate.v1`. The companion
  `scripts/run_sorafs_production_readiness.py` accepts reviewed per-lane
  summary paths, requires exactly one summary input per required gate, requires
  an explicit canonical `--deployment-id`/`--environment` pair whose deployment
  id passes the reviewed deployment-id policy, carries no staging markers, and
  whose environment is `prod` or `production`, advertises both flags as
  required final deployment context in `--help`, supports `@ARGFILE`, rejects
  explicit summaries for lanes
  outside a narrowed
  `--require-gate` selection, validates the schema-closed collection plan
  envelope against the built command plan and independently rechecks final
  production deployment context before dry-run output or execution,
  rejects non-object or non-strict-JSON collection-plan renderings before
  stdout or verifier launch,
  and emits that dry-run collection plan so release operators can inspect the
  full production-readiness command before invoking the aggregate gate.

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
