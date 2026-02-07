---
lang: ka
direction: ltr
source: docs/portal/docs/sorafs/chunker-profile-authoring.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 855dd4bff7bf9581f485ac6ad7fa332f17595efb41127b74795c4b3a5a955406
source_last_modified: "2026-01-05T09:28:11.856260+00:00"
translation_last_reviewed: 2026-02-07
id: chunker-profile-authoring
title: SoraFS Chunker Profile Authoring Guide
sidebar_label: Chunker Authoring Guide
description: Checklist for proposing new SoraFS chunker profiles and fixtures.
---

:::note Canonical Source
:::

# SoraFS Chunker Profile Authoring Guide

This guide explains how to propose and publish new chunker profiles for SoraFS.
It complements the architecture RFC (SF-1) and the registry reference (SF-2a)
with concrete authoring requirements, validation steps, and proposal templates.
For a canonical example, see
`docs/source/sorafs/proposals/sorafs_sf1_profile_v1.json`
and the accompanying dry-run log in
`docs/source/sorafs/reports/sf1_determinism.md`.

## Overview

Every profile that enters the registry must:

- advertise deterministic CDC parameters and multihash settings identical across
  architectures;
- ship replayable fixtures (Rust/Go/TS JSON + fuzz corpora + PoR witnesses) that
  downstream SDKs can verify without bespoke tooling;
- include governance-ready metadata (namespace, name, semver) plus migration
- pass the deterministic diff suite before council review.

Follow the checklist below to prepare a proposal that satisfies those rules.

## Registry Charter Snapshot

Before drafting a proposal, confirm it conforms to the registry charter enforced
by `sorafs_manifest::chunker_registry::ensure_charter_compliance()`:

- Profile IDs are positive integers that increase monotonically without gaps.
- The canonical handle (`namespace.name@semver`) must appear in the alias list
  and **must** be the first entry.
- No alias may collide with another canonical handle or appear more than once.
- Aliases must be non-empty and trimmed of whitespace.

Handy CLI helpers:

```bash
# JSON listing of all registered descriptors (ids, handles, aliases, multihash)
cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- --list-profiles

# Emit metadata for a candidate default profile (canonical handle + aliases)
cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- \
  --promote-profile=sorafs.sf1@1.0.0 --json-out=-
```

These commands keep proposals aligned with the registry charter and provide the
canonical metadata needed in governance discussions.

## Required Metadata

| Field | Description | Example (`sorafs.sf1@1.0.0`) |
|-------|-------------|------------------------------|
| `namespace` | Logical grouping for related profiles. | `sorafs` |
| `name` | Human-readable label. | `sf1` |
| `semver` | Semantic version string for the parameter set. | `1.0.0` |
| `profile_id` | Monotonic numeric identifier assigned once the profile lands. Reserve the next id but do not reuse existing numbers. | `1` |
| `profile_aliases` | Optional additional handles exposed to clients during negotiation. Always include the canonical handle as the first entry. | `["sorafs.sf1@1.0.0"]` |
| `profile.min_size` | Minimum chunk length in bytes. | `65536` |
| `profile.target_size` | Target chunk length in bytes. | `262144` |
| `profile.max_size` | Maximum chunk length in bytes. | `524288` |
| `profile.break_mask` | Adaptive mask used by the rolling hash (hex). | `0x0000ffff` |
| `profile.polynomial` | Gear polynomial constant (hex). | `0x3da3358b4dc173` |
| `gear_seed` | Seed used to derive the 64 KiB gear table. | `sorafs-v1-gear` |
| `chunk_multihash.code` | Multihash code for per-chunk digests. | `0x1f` (BLAKE3-256) |
| `chunk_multihash.digest` | Digest of the canonical fixtures bundle. | `13fa...c482` |
| `fixtures_root` | Relative directory containing the regenerated fixtures. | `fixtures/sorafs_chunker/sorafs.sf1@1.0.0/` |
| `por_seed` | Seed for deterministic PoR sampling (`splitmix64`). | `0xfeedbeefcafebabe` (example) |

The metadata must appear both in the proposal document and inside the generated
fixtures so the registry, CLI tooling, and governance automation can confirm the
values without manual cross-referencing. When in doubt, run the chunk-store and
manifest CLIs with `--json-out=-` to stream the computed metadata into review
notes.

### CLI & Registry Touchpoints

- `sorafs_manifest_chunk_store --profile=<handle>` – re-run chunk metadata,
  manifest digest, PoR checks with the proposed parameters.
- `sorafs_manifest_chunk_store --json-out=-` – stream the chunk-store report to
  stdout for automated comparisons.
- `sorafs_manifest_stub --chunker-profile=<handle>` – confirm manifests and CAR
  plans embed the canonical handle plus aliases.
- `sorafs_manifest_stub --plan=-` – feed the previous `chunk_fetch_specs` back
  in to verify offsets/digests post-change.

Record the command output (digests, PoR roots, manifest hashes) in the proposal
so reviewers can reproduce them verbatim.

## Determinism & Validation Checklist

1. **Regenerate fixtures**
   ```bash
   cargo run --locked -p sorafs_chunker --bin export_vectors \
     --signature-out=fixtures/sorafs_chunker/manifest_signatures.json
   ```
2. **Run the parity suite** – `cargo test -p sorafs_chunker` and the
   cross-language diff harness (`crates/sorafs_chunker/tests/vectors.rs`) must be
   green with the new fixtures in place.
3. **Replay fuzz/back-pressure corpora** – execute `cargo fuzz list` and the
   streaming harness (`fuzz/sorafs_chunker`) against the regenerated assets.
4. **Verify Proof-of-Retrievability witnesses** – run
   `sorafs_manifest_chunk_store --por-sample=<n>` using the proposed profile and
   confirm the roots match the fixture manifest.
5. **CI dry run** – invoke `ci/check_sorafs_fixtures.sh` locally; the script
   should succeed with the new fixtures and existing `manifest_signatures.json`.
6. **Cross-runtime confirmation** – ensure Go/TS bindings consume the regenerated
   JSON and emit identical chunk boundaries and digests.

Document the commands and resulting digests in the proposal so the Tooling WG
can re-run them without guesswork.

### Manifest / PoR Confirmation

After regenerating fixtures, run the full manifest pipeline to ensure CAR
metadata and PoR proofs remain consistent:

```bash
# Validate chunk metadata + PoR with the new profile
cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- \
  --profile=sorafs.sf2@1.0.0 \
  --json-out=- --por-json-out=- fixtures/sorafs_chunker/input.bin

# Generate manifest + CAR and capture chunk fetch specs
cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- \
  fixtures/sorafs_chunker/input.bin \
  --chunker-profile=sorafs.sf2@1.0.0 \
  --chunk-fetch-plan-out=chunk_plan.json \
  --manifest-out=sf2.manifest \
  --car-out=sf2.car \
  --json-out=sf2.report.json

# Re-run using the saved fetch plan (guards against stale offsets)
cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- \
  fixtures/sorafs_chunker/input.bin \
  --chunker-profile=sorafs.sf2@1.0.0 \
  --plan=chunk_plan.json --json-out=-
```

Replace the input file with any representative corpus used by your fixtures
(e.g., the 1 GiB deterministic stream) and attach the resulting digests to the
proposal.

## Proposal Template

Proposals are submitted as `ChunkerProfileProposalV1` Norito records checked into
`docs/source/sorafs/proposals/`. The JSON template below illustrates the expected
shape (substitute your values as needed):


Provide a matching Markdown report (`determinism_report`) that captures the
command output, chunk digests, and any deviations encountered during validation.

## Governance Workflow

1. **Submit PR with proposal + fixtures.** Include the generated assets, the
   Norito proposal, and updates to `chunker_registry_data.rs`.
2. **Tooling WG review.** Reviewers re-run the validation checklist and confirm
   the proposal aligns with registry rules (no id reuse, determinism satisfied).
3. **Council envelope.** Once approved, council members sign the proposal digest
   (`blake3("sorafs-chunker-profile-v1" || canonical_bytes)`) and append their
   signatures to the profile envelope stored alongside the fixtures.
4. **Registry publish.** Merge bumps the registry, docs, and fixtures. The
   default CLI remains on the previous profile until governance declares the
   migration ready.
5. **Deprecation tracking.** After the migration window, update the registry to
   ledger.

## Authoring Tips

- Prefer even power-of-two bounds to minimise edge-case chunking behaviour.
- Avoid changing the multihash code without coordinating manifest and gateway
- Keep gear table seeds human-readable but globally unique to simplify audit
  trails.
- Store any benchmarking artefacts (e.g., throughput comparisons) under
  `docs/source/sorafs/reports/` for future reference.

For operational expectations during rollout refer to the migration ledger
(`docs/source/sorafs/migration_ledger.md`). For runtime conformance rules see
`docs/source/sorafs/chunker_conformance.md`.
