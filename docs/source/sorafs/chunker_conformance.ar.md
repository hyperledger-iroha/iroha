---
lang: ar
direction: rtl
source: docs/source/sorafs/chunker_conformance.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 81b8dc9405d51c4f45c742d7072931cd2626c4aafbdf663195d64f63e19eca94
source_last_modified: "2026-01-04T16:50:33.260273+00:00"
translation_last_reviewed: 2026-01-30
---

# SoraFS Chunker Conformance Guide

This guide codifies the requirements every implementation must follow to stay
aligned with the SoraFS deterministic chunker profile (SF1). It also
documents the regeneration workflow, signing policy, and verification steps so
fixture consumers across SDKs remain in sync.

> **Portal:** Mirrored in `docs/portal/docs/sorafs/chunker-conformance.md`.
> Update both copies to keep reviewers aligned.

## Canonical Profile

- Profile handle: `sorafs.sf1@1.0.0`
- Input seed (hex): `0000000000dec0ded`
- Target size: 262 144 bytes (256 KiB)
- Minimum size: 65 536 bytes (64 KiB)
- Maximum size: 524 288 bytes (512 KiB)
- Rolling polynomial: `0x3DA3358B4DC173`
- Gear table seed: `sorafs-v1-gear`
- Break mask: `0x0000FFFF`

Reference implementation: `sorafs_chunker::chunk_bytes_with_digests_profile`.
Any SIMD acceleration must produce identical boundaries and digests.

## Fixture Bundle

`cargo run --locked -p sorafs_chunker --bin export_vectors` regenerates the
fixtures and emits the following files under `fixtures/sorafs_chunker/`:

- `sf1_profile_v1.{json,rs,ts,go}` — canonical chunk boundaries for Rust,
  TypeScript, and Go consumers. Each file advertises the canonical handle as the
  first (and only) entry in `profile_aliases`. The ordering is enforced by
  `ensure_charter_compliance` and MUST NOT be altered.
- `manifest_blake3.json` — BLAKE3-verified manifest covering every fixture file.
- `manifest_signatures.json` — Council signatures (Ed25519) over the manifest
  digest.
- `sf1_profile_v1_backpressure.json` and raw corpora inside `fuzz/` —
  deterministic streaming scenarios used by chunker back-pressure tests.

### Signing Policy

Fixture regeneration **must** include a valid council signature. The generator
rejects unsigned output unless `--allow-unsigned` is passed explicitly (intended
only for local experimentation). Signature envelopes are append-only and
deduplicated per signer.

To add a council signature:

```bash
cargo run --locked -p sorafs_chunker --bin export_vectors \
  --signing-key=<ed25519-private-key-hex> \
  --signature-out=fixtures/sorafs_chunker/manifest_signatures.json
```

## Verification

The CI helper `ci/check_sorafs_fixtures.sh` replays the generator with
`--locked`. If fixtures drift or signatures are missing, the job fails. Use
this script in nightly workflows and before submitting fixture changes.

Manual verification steps:

1. Run `cargo test -p sorafs_chunker`.
2. Invoke `ci/check_sorafs_fixtures.sh` locally.
3. Confirm `git status -- fixtures/sorafs_chunker` is clean.

## Upgrade Playbook

When proposing a new chunker profile or updating SF1:

See also: [`docs/source/sorafs/chunker_profile_authoring.md`](chunker_profile_authoring.md) for
metadata requirements, proposal templates, and validation checklists.

1. Draft a `ChunkProfileUpgradeProposalV1` (see RFC SF‑1) with new parameters.
2. Regenerate fixtures via `export_vectors` and record the new manifest digest.
3. Sign the manifest with the required council quorum. All signatures must be
   appended to `manifest_signatures.json`.
4. Update affected SDK fixtures (Rust/Go/TS) and ensure cross-runtime parity.
5. Regenerate fuzz corpora if parameters change.
6. Update this guide with the new profile handle, seeds, and digest.
7. Submit the change alongside updated tests and roadmap updates.

Changes that affect chunk boundaries or digests without following this process
are invalid and must not be merged.
