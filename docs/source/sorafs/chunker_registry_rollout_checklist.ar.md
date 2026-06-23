---
lang: ar
direction: rtl
source: docs/source/sorafs/chunker_registry_rollout_checklist.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: e4d52f449ae561d75e4723d9573ad6e099fda92ccae1f0fe683baacbbcae494c
source_last_modified: "2026-01-03T18:07:58.428331+00:00"
translation_last_reviewed: 2026-01-30
---

# SoraFS Chunker Registry Rollout Checklist

Use this checklist when adding, promoting, or deprecating a SoraFS chunker
profile. It complements the governance charter, authoring guide, and registry
reference. The current registry is implemented in
`crates/sorafs_manifest/src/chunker_registry.rs` and contains `sorafs.sf1@1.0.0`
(profile id `1`) and `sorafs.sf2@1.0.0` (profile id `2`).

## Pre-Submission

- Reserve a positive, monotonically increasing `profile_id`; never reuse an
  existing id.
- Confirm the canonical handle has the form `namespace.name@semver` and is the
  first alias.
- Regenerate fixtures for Rust and companion SDK consumers under
  `fixtures/sorafs_chunker`.
- Create or update the `ChunkerProfileProposalV1` JSON under
  `docs/source/sorafs/proposals/`.
- Capture a determinism report under `docs/source/sorafs/reports/` with the exact
  commands, digests, fixture paths, and any deviations.

## Local Validation

Run the registry and fixture checks before requesting review:

```bash
cargo test -p sorafs_manifest chunker_registry
cargo test -p sorafs_chunker
ci/check_sorafs_fixtures.sh
```

Inspect the registry and candidate profile metadata:

```bash
cargo run -p sorafs_car --bin sorafs_manifest_chunk_store -- --list-profiles
cargo run -p sorafs_car --bin sorafs_manifest_chunk_store -- \
  --promote-profile=<handle> --json-out=-
cargo run -p sorafs_car --bin sorafs_manifest_stub -- \
  --chunker-profile=<handle> --json-out=-
```

For PoR coverage, replay a representative payload with proof sampling:

```bash
cargo run -p sorafs_car --bin sorafs_manifest_chunk_store -- ./payload.bin \
  --profile=<handle> --json-out=- --por-json-out=- --por-sample=8
```

## Review Packet

Attach the following to the governance or PR review thread:

- Proposal JSON and council envelope placeholder.
- Determinism report and fixture digests.
- Output from `--list-profiles` and `--promote-profile=<handle>`.
- Manifest stub output showing the selected profile id, canonical handle, and
  aliases.
- PoR sample output and proof verification result.
- SDK parity notes for any generated Go, JavaScript, Python, Swift, JVM/Android,
  or C# fixtures.

## Publication

After council approval:

1. Merge the registry update in `crates/sorafs_manifest/src/chunker_registry.rs`.
2. Merge fixture, proposal, determinism-report, and documentation updates in the
   same release change.
3. Keep the previous default profile active until governance explicitly approves
   default-profile migration.
4. Notify operators and SDK teams of the new canonical handle, aliases, profile
   id, fixture digest, and rollout window.
5. Update `status.md` and keep `roadmap.md` focused on remaining rollout work.

## Post-Rollout

- Verify staged manifests can be built with the new handle and still validate
  with the registered numeric id.
- Archive signed governance envelopes and determinism reports.
- Watch gateway refusal telemetry for unsupported-profile or alias-negotiation
  failures.
- Start deprecation tracking only after the dual-publish window and migration
  evidence are complete.
