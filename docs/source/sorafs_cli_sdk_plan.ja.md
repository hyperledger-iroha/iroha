---
lang: ja
direction: ltr
source: docs/source/sorafs_cli_sdk_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 3d8fe44c775fa46e1655c98d8a57455a41b456397fe9853934a7bc5c4ea64a6c
source_last_modified: "2026-01-04T10:50:53.671894+00:00"
translation_last_reviewed: 2026-01-30
---

---
title: SoraFS CLI & SDK Plan (Draft Outline)
summary: High-level requirements for SF-6 CLI/SDK bindings.
---

# SoraFS CLI & SDK Plan (Draft)

## Scope

- Provide developer-friendly CLI (`sorafs`) and language SDKs (Rust, TypeScript, Go).
- Support Norito build/validation, CAR packing, manifest submission, proof verification.
- Integrate authentication (Sigstore/OIDC) for CI pipelines.
- Offer streaming APIs for PoR/PoTR, chunk fetch, and telemetry.

## CLI Goals

Commands (initial):
- `sorafs build-manifest` — compile Norito manifest from spec.
- `sorafs pack-car` — produce CARv2 with chunk plan and proof metadata.
- `sorafs submit-manifest` — POST to Torii `/v1/sorafs/pin/register`.
- `sorafs verify-proof` — validate PoR proof against manifest.
- `sorafs cert` — wrapper around self-cert kit for gateways (SF-5a).

Flags:
- `--auth oidc` / `--auth keyfile`
- `--output norito|json`
- `--fixtures /path/to/fixtures`

## SDK Targets

- Rust crate (`sorafs_sdk`) reusing core logic.
- TypeScript package (`@sora-org/sorafs-sdk`).
- Go package (`github.com/sora-org/sorafs-sdk-go`).

Features:
- Manifest builder API (typed Norito structs).
- CAR pack/unpack utilities.
- Proof verification functions.
- Streaming client for chunk-range endpoints.
- Token issuance helper for stream tokens (SF-5d).

## Authentication & CI

- Integrate Sigstore keyless signing.
- Provide reusable GitHub/GitLab templates.
- Document secret management best practices.

## Observability Hooks

- Expose instrumentation for CLI/SDK usage metrics (`sorafs_cli_*`).
- Provide optional OpenTelemetry exporters in SDK.

## Roadmap Alignment

- SF-6a: CI templates & release hooks (link to `docs/examples/sorafs_ci.md`).
- SF-6b/c: orchestrator integration will consume SDK.

## Open Questions

- Packaging/distribution strategy (Homebrew, npm, crates.io, Go modules).
- Versioning scheme (keep CLI + SDK in lockstep?).
