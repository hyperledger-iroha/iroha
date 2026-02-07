---
lang: zh-hant
direction: ltr
source: docs/examples/taikai_anchor_lineage_packet.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: a2037fed472e37a06559e7cd871c1b916b514b9804f309413fc369d5ded662b6
source_last_modified: "2025-12-29T18:16:35.095373+00:00"
translation_last_reviewed: 2026-02-07
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Taikai Anchor Lineage Packet Template (SN13-C)

Roadmap item **SN13-C — Manifests & SoraNS anchors** requires every alias
rotation to ship a deterministic evidence bundle. Copy this template into your
rollout artefact directory (for example
`artifacts/taikai/anchor/<event>/<alias>/<timestamp>/packet.md`) and replace
the placeholders before submitting the packet to governance.

## 1. Metadata

| Field | Value |
|-------|-------|
| Event ID | `<taikai.event.launch-2026-07-10>` |
| Stream / rendition | `<main-stage>` |
| Alias namespace / name | `<sora / docs>` |
| Evidence directory | `artifacts/taikai/anchor/<event>/<alias>/2026-07-10T18-00Z/` |
| Operator contact | `<name + email>` |
| GAR / RPT ticket | `<governance ticket or GAR digest>` |

## Bundle helper (optional)

Copy the spool artefacts and emit a JSON (optionally signed) summary before
filling in the remaining sections:

```bash
cargo xtask taikai-anchor-bundle \
  --spool config/da_manifests/taikai \
  --copy-dir artifacts/taikai/anchor/<event>/<alias>/<timestamp>/spool \
  --out artifacts/taikai/anchor/<event>/<alias>/<timestamp>/anchor_bundle.json \
  --signing-key <hex-ed25519-optional>
```

The helper pulls `taikai-anchor-request-*`, `taikai-trm-state-*`,
`taikai-lineage-*`, envelopes, and sentinels out of the Taikai spool directory
(`config.da_ingest.manifest_store_dir/taikai`) so the evidence folder already
contains the exact files referenced below.

## 2. Lineage ledger & hint

Attach both the on-disk lineage ledger and the hint JSON Torii wrote for this
window. These come directly from
`config.da_ingest.manifest_store_dir/taikai/taikai-trm-state-<alias>.json` and
`taikai-lineage-<lane>-<epoch>-<sequence>-<storage_ticket>-<fingerprint>.json`.

| Artefact | File | SHA-256 | Notes |
|----------|------|---------|-------|
| Lineage ledger | `taikai-trm-state-docs.json` | `<sha256>` | Proves the previous manifest digest/window. |
| Lineage hint | `taikai-lineage-l1-140-6a-b2b.json` | `<sha256>` | Captured before uploading to SoraNS anchor. |

```bash
sha256sum artifacts/taikai/anchor/<event>/<alias>/<ts>/taikai-trm-state-*.json \
  | tee artifacts/taikai/anchor/<event>/<alias>/<ts>/hashes/lineage.sha256
```

## 3. Anchor payload capture

Record the POST payload that Torii delivered to the anchor service. The payload
includes `envelope_base64`, `ssm_base64`, `trm_base64`, and the inline
`lineage_hint` object; audits rely on this capture to prove the hint that was
sent to SoraNS. Torii now writes this JSON automatically as
`taikai-anchor-request-<lane>-<epoch>-<sequence>-<ticket>-<fingerprint>.json`
inside the Taikai spool directory (`config.da_ingest.manifest_store_dir/taikai/`), so
operators can copy it directly instead of scraping HTTP logs.

| Artefact | File | SHA-256 | Notes |
|----------|------|---------|-------|
| Anchor POST | `requests/2026-07-10T18-00Z.json` | `<sha256>` | Raw request copied from `taikai-anchor-request-*.json` (Taikai spool). |

## 4. Manifest digest acknowledgement

| Field | Value |
|-------|-------|
| New manifest digest | `<hex digest>` |
| Previous manifest digest (from hint) | `<hex digest>` |
| Window start / end | `<start seq> / <end seq>` |
| Acceptance timestamp | `<ISO8601>` |

Reference the ledger/hint hashes recorded above so reviewers can verify the
window that was superseded.

## 5. Metrics / `taikai_alias_rotations`

- `taikai_trm_alias_rotations_total` snapshot: `<Prometheus query + export path>`
- `/status taikai_alias_rotations` dump (per alias): `<file path + hash>`

Provide the Prometheus/Grafana export or `curl` output that shows the counter
increment and the `/status` array for this alias.

## 6. Manifest for the evidence directory

Generate a deterministic manifest of the evidence directory (spool files,
payload capture, metrics snapshots) so governance can verify every hash without
unpacking the archive.

```bash
python3 scripts/repo_evidence_manifest.py \
  --root artifacts/taikai/anchor/<event>/<alias>/<ts> \
  --agreement-id <event/alias/window> \
  --output artifacts/taikai/anchor/<event>/<alias>/<ts>/manifest.json
```

| Artefact | File | SHA-256 | Notes |
|----------|------|---------|-------|
| Evidence manifest | `manifest.json` | `<sha256>` | Attach this to the governance packet / GAR. |

## 7. Checklist

- [ ] Lineage ledger copied + hashed.
- [ ] Lineage hint copied + hashed.
- [ ] Anchor POST payload captured and hashed.
- [ ] Manifest digest table filled in.
- [ ] Metrics snapshots exported (`taikai_trm_alias_rotations_total`, `/status`).
- [ ] Manifest generated with `scripts/repo_evidence_manifest.py`.
- [ ] Packet uploaded to governance with hashes + contact info.

Maintaining this template for every alias rotation keeps the SoraNS governance
bundle reproducible and ties lineage hints directly to the GAR/RPT evidence.
