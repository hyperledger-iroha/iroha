---
lang: az
direction: ltr
source: docs/portal/docs/sorafs/signing-ceremony.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 8a4be274ad087f292559c4b83a120ad20316a1e1dfe0ccbfb9aad42235ac136b
source_last_modified: "2026-01-05T09:28:11.909870+00:00"
translation_last_reviewed: 2026-02-07
id: signing-ceremony
title: Signing Ceremony Replacement
description: How the Sora Parliament approves and distributes SoraFS chunker fixtures (SF-1b).
sidebar_label: Signing Ceremony
---

> Roadmap: **SF-1b — Sora Parliament fixture approvals.**

The manual signing ritual used for SoraFS chunker fixtures is retired. All
approvals now flow through the **Sora Parliament**, the sortition-based DAO that
governs Nexus. Parliament members bond XOR to gain citizenship, rotate across
panels, and cast on-chain votes that approve, reject, or roll back fixture
releases. This guide explains the process and developer tooling.

## Parliament overview

- **Citizenship** — Operators bond the required XOR to enrol as citizens and
  become eligible for sortition.
- **Panels** — Responsibilities are split across rotating panels (Infrastructure,
  Moderation, Treasury, …). The Infrastructure Panel owns SoraFS fixture
  approvals.
- **Sortition & rotation** — Panel seats are re-drawn at the cadence specified in
  the Parliament constitution so no single group monopolises approvals.

## Fixture approval flow

1. **Proposal submission**
   - The Tooling WG uploads the candidate `manifest_blake3.json` bundle plus
     fixture diff to the on-chain registry via `sorafs.fixtureProposal`.
   - The proposal records the BLAKE3 digest, semantic version, and change notes.
2. **Review & voting**
   - The Infrastructure Panel receives the assignment through the Parliament task
     queue.
   - Panel members inspect CI artefacts, run parity tests, and cast weighted
     votes on-chain.
3. **Finalisation**
   - Once quorum is met, the runtime emits an approval event that includes the
     canonical manifest digest and Merkle commitment to the fixture payload.
   - The event is mirrored into the SoraFS registry so clients can fetch the
     latest Parliament-approved manifest.
4. **Distribution**
   - CLI helpers (`cargo xtask sorafs-fetch-fixture`) pull the approved manifest
     from Nexus RPC. The repository’s JSON/TS/Go constants stay in sync by
     re-running `export_vectors` and validating the digest against the on-chain
     record.

## Developer workflow

- Regenerate fixtures with:

```bash
cargo run -p sorafs_chunker --bin export_vectors
```

- Use the Parliament fetch helper to download the approved envelope, verify
  signatures, and refresh local fixtures. Point `--signatures` at the
  Parliament-published envelope; the helper resolves the accompanying manifest,
  recomputes the BLAKE3 digest, and enforces the canonical
  `sorafs.sf1@1.0.0` profile.

```bash
cargo xtask sorafs-fetch-fixture \
  --signatures https://nexus.example/api/sorafs/manifest_signatures.json \
  --out fixtures/sorafs_chunker
```

Pass `--manifest` if the manifest lives at a different URL. Unsigned envelopes
are refused unless `--allow-unsigned` is set for local smoke runs.

- When validating a manifest through a staging gateway, target Torii instead of
  local payloads:

```bash
sorafs-fetch \
  --plan=fixtures/chunk_fetch_specs.json \
  --gateway-provider=name=staging,provider-id=<hex>,base-url=https://gw-stage.example/,stream-token=<base64> \
  --gateway-manifest-id=<manifest_id_hex> \
  --gateway-chunker-handle=sorafs.sf1@1.0.0 \
  --json-out=reports/staging_gateway.json
```

- Local CI no longer requires a `signer.json` roster.
  `ci/check_sorafs_fixtures.sh` compares the repo state against the latest
  on-chain commitment and fails when they diverge.

## Governance notes

- The Parliament constitution governs quorum, rotation, and escalation—no
  crate-level configuration is needed.
- Emergency rollbacks are handled through the Parliament moderation panel. The
  Infrastructure Panel files a revert proposal referencing the prior manifest
  digest, which replaces the release once approved.
- Historical approvals remain available in the SoraFS registry for forensic
  replay.

## FAQ

- **Where did `signer.json` go?**  
  It was removed. All signer attribution lives on-chain; `manifest_signatures.json`
  in the repository is only a developer fixture that must match the latest
  approval event.

- **Do we still require local Ed25519 signatures?**  
  No. Parliament approvals are stored as on-chain artefacts. Local fixtures exist
  for reproducibility but are validated against the Parliament digest.

- **How do teams monitor approvals?**  
  Subscribe to the `ParliamentFixtureApproved` event or query the registry via
  Nexus RPC to retrieve the current manifest digest and panel roll call.
