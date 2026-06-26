---
lang: ja
direction: ltr
source: docs/source/sorafs_cli_sdk_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: f573c782631379eac7fc97f8bc8f2b3050da1f409d2f229ae1462d7dd18a6ea5
source_last_modified: "2026-06-25T16:58:37+00:00"
translation_last_reviewed: 2026-01-30
---

# SoraFS CLI & SDK Plan

## Scope

- Use `sorafs_cli` as the operator CLI for manifest build/sign/verify/submit, CAR packing, storage prepare/pin, proof stream/verify, chunk fetch, reputation, PoR, proxy, Taikai, moderation, and appeal workflows.
- Use `sorafs-validate` plus `soranet_trustless_verifier --validation-outcome` as the reference validator surface for SDK and release smoke checks.
- Use `scripts/release_sorafs_cli.sh`, `ci/check_sorafs_cli_release.sh`, `scripts/sorafs_gateway_self_cert.sh`, and `cargo xtask sorafs-gateway-attest` for release and self-certification evidence.
- Keep language SDK parity on the existing SoraFS CI guards for pin-register builders and orchestrator smoke fixtures.

## CLI Goals

Implemented command families:
- `sorafs_cli car pack`, `manifest build`, `manifest sign`, `manifest verify-signature`, `manifest proposal`, and `manifest submit` cover manifest and release packaging.
- `sorafs_cli storage prepare`, `storage pin`, `fetch`, `proof stream`, and `proof verify` cover local storage preparation, gateway fetches, proof requests, and trustless verification.
- `sorafs_cli por status`, `por trigger`, `por export`, and `por report` cover the local PoR operator surface.
- `sorafs_cli reputation publish`, `snapshot`, `fetch`, `watch`, and `verify` cover the reputation workflow.
- `sorafs-validate` validates and signs reference SDK fixtures; `soranet_trustless_verifier --validation-outcome` emits the same outcome contract for manifest/CAR replay.
- `scripts/sorafs_gateway_self_cert.sh` and `cargo xtask sorafs-gateway-attest` generate and verify gateway conformance attestations.

Important flag patterns:
- `--identity-token`, `--identity-token-env`, `--identity-token-file`, and `--identity-token-provider=github-actions` for keyless manifest signing.
- `--private-key`, `--private-key-file`, `--authority`, and `--network-prefix` for signed live manifest submission.
- `--format table|json|yaml`, `--summary-out`, `--json-out`, and `--telemetry-out` for deterministic machine-readable evidence.

## SDK Targets

- Rust callers use the existing `sorafs_manifest`, `sorafs_car`, `sorafs_orchestrator`, and `reference_ffi` surfaces rather than a duplicate codec crate.
- JavaScript uses the checked `javascript/iroha_js/src/sorafs.js` and `dist/sorafs.js` helpers for SoraFS pin/orchestrator workflows.
- Swift, Kotlin/JVM, Android Java, C#, JavaScript, and Python pin-register parity is guarded by the `ci/check_sorafs_pin_register_*_sdk.sh` lanes.
- Orchestrator SDK parity uses `ci/sdk_sorafs_orchestrator.sh` and fixtures under `fixtures/sorafs_orchestrator/multi_peer_parity_v1/`.
- Go module publication is a release-packaging track that should consume the same committed fixtures and Norito schemas when cut.

## Authentication & CI

- Keyless manifest signing is implemented through `sorafs_cli manifest sign` with explicit OIDC token inputs.
- Reusable CI examples live in `docs/examples/sorafs_ci.md`; release checks are scripted by `ci/check_sorafs_cli_release.sh`.
- Release signing and manifest verification are wrapped by `scripts/release_sorafs_cli.sh`; gateway self-cert evidence is wrapped by `scripts/sorafs_gateway_self_cert.sh`.
- Runtime secrets such as identity tokens, private keys, and gateway bearer tokens must be supplied at execution time and not committed.

## Observability Hooks

- CLI commands emit deterministic summaries through `--summary-out`, `--json-out`, and `--telemetry-out` where applicable.
- Gateway, fetch, PoR, PDP/PoTR, admission, reputation, and conformance dashboards live under `dashboards/grafana/`.
- CI/release scripts archive command logs and generated evidence bundles under `artifacts/` or `dist/` for operator review.

## Roadmap Alignment

- SF-6a is locally covered by the CI cookbook, release check, release signing script, and gateway self-cert wrapper.
- SF-6b/c is locally covered by the orchestrator CLI, multi-provider fixtures, SDK parity harness, and mock-provider/orchestrator tests.
- Remaining release work is signed distribution evidence and live deployment capture, not missing local command surfaces.

## Release Packaging

- Package registries such as Homebrew, npm, crates.io, and Go modules should be populated only from signed release cuts using the existing release scripts and fixture smoke checks.
- Versioning should keep wire-format compatibility tied to Norito schema major versions while allowing SDK patch releases for host-specific fixes.
