---
lang: he
direction: rtl
source: docs/source/ministry/impact_assessment_tooling.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 89be62d7bb2bb79fd994d207489d310ef4c997be53447fbee8ac1f7b758d3beb
source_last_modified: "2026-01-03T18:07:57.641039+00:00"
translation_last_reviewed: 2026-01-30
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Impact Assessment Tooling (MINFO‑4b)

Roadmap reference: **MINFO‑4b — Impact assessment tooling.**  
Owner: Governance Council / Analytics

This note documents the `cargo xtask ministry-agenda impact` command that now
produces the automated hash-family diff required for referendum packets. The
tool consumes validated Agenda Council proposals, the duplicate registry, and
an optional denylist/policy snapshot so reviewers can see exactly which
fingerprints are new, which collide with existing policy, and how many entries
each hash family contributes.

## Inputs

1. **Agenda proposals.** One or more files that follow
   [`docs/source/ministry/agenda_council_proposal.md`](agenda_council_proposal.md).
   Pass them explicitly with `--proposal <path>` or point the command at a
   directory via `--proposal-dir <dir>` and every `*.json` file under that path
   is included.
2. **Duplicate registry (optional).** A JSON file matching
   `docs/examples/ministry/agenda_duplicate_registry.json`. Conflicts are
   reported under `source = "duplicate_registry"`.
3. **Policy snapshot (optional).** A lightweight manifest that lists every
   fingerprint already enforced by GAR/Ministry policy. The loader expects the
   schema shown below (see
   [`docs/examples/ministry/policy_snapshot_example.json`](../../examples/ministry/policy_snapshot_example.json)
   for a complete sample):

```json
{
  "snapshot_id": "denylist-2026-03",
  "generated_at": "2026-03-31T12:00:00Z",
  "entries": [
    {
      "hash_family": "blake3-256",
      "hash_hex": "…",
      "policy_id": "denylist-2025-014-entry-01",
      "note": "Already quarantined by GAR case CSAM-2025-014."
    }
  ]
}
```

Any entry whose `hash_family:hash_hex` fingerprint matches a proposal target is
reported under `source = "policy_snapshot"` with the referenced `policy_id`.

## Usage

```bash
cargo xtask ministry-agenda impact \
  --proposal docs/examples/ministry/agenda_proposal_example.json \
  --registry docs/examples/ministry/agenda_duplicate_registry.json \
  --policy-snapshot docs/examples/ministry/policy_snapshot_example.json \
  --out artifacts/ministry/impact/AC-2026-001.json
```

Additional proposals can be appended via repeated `--proposal` flags or by
supplying a directory that contains an entire referendum batch:

```bash
cargo xtask ministry-agenda impact \
  --proposal-dir artifacts/ministry/proposals/2026-03-31 \
  --registry state/agenda_duplicate_registry.json \
  --out artifacts/ministry/impact/2026-03-31.json
```

The command prints the generated JSON to stdout when `--out` is omitted.

## Output

The report is a signed-off artefact (record it under the referendum packet’s
`artifacts/ministry/impact/` directory) with the following structure:

```json
{
  "format_version": 1,
  "generated_at": "2026-03-31T12:34:56Z",
  "totals": {
    "proposals_analyzed": 4,
    "targets_analyzed": 17,
    "registry_conflicts": 2,
    "policy_conflicts": 1,
    "hash_families": [
      { "hash_family": "blake3-256", "targets": 12, "registry_conflicts": 2, "policy_conflicts": 0 },
      { "hash_family": "sha256", "targets": 5, "registry_conflicts": 0, "policy_conflicts": 1 }
    ]
  },
  "proposals": [
    {
      "proposal_id": "AC-2026-001",
      "action": "add-to-denylist",
      "total_targets": 2,
      "source_path": "docs/examples/ministry/agenda_proposal_example.json",
      "hash_families": [
        { "hash_family": "blake3-256", "targets": 2, "registry_conflicts": 1, "policy_conflicts": 0 }
      ],
      "conflicts": [
        {
          "source": "duplicate_registry",
          "hash_family": "blake3-256",
          "hash_hex": "0d714bed…1338d",
          "reference": "AC-2025-014",
          "note": "Already quarantined."
        }
      ],
      "registry_conflicts": 1,
      "policy_conflicts": 0
    }
  ]
}
```

Attach this JSON to every referendum dossier alongside the neutral summary so
panelists, jurors, and governance observers can see the exact blast radius of
each proposal. The output is deterministic (sorted by hash family) and safe to
include in CI/runbooks; if the duplicate registry or policy snapshot changes,
rerun the command and attach the refreshed artefact before the vote opens.

> **Next step:** feed the generated impact report into
> [`cargo xtask ministry-panel packet`](referendum_packet.md) so the
> `ReferendumPacketV1` dossier contains both the hash-family breakdown and the
> detailed conflict list for the proposal under review.
