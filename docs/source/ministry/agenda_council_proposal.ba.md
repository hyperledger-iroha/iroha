---
lang: ba
direction: ltr
source: docs/source/ministry/agenda_council_proposal.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: d2a7a47fdf0c80d189c912baafa5d6ce81a17a4c90f2b1797e532989a56f5060
source_last_modified: "2025-12-29T18:16:35.977493+00:00"
translation_last_reviewed: 2026-02-07
---

# Agenda Council Proposal Schema (MINFO-2a)

Roadmap reference: **MINFO-2a — Proposal format validator.**

The Agenda Council workflow batches citizen-submitted blacklist and policy change
proposals before the governance panels review them. This document defines the
canonical payload schema, evidence requirements, and duplication detection rules
consumed by the new validator (`cargo xtask ministry-agenda validate`) so
proposers can lint JSON submissions locally before uploading them to the portal.

## Payload overview

Agenda proposals use the `AgendaProposalV1` Norito schema
(`iroha_data_model::ministry::AgendaProposalV1`). Fields are encoded as JSON when
submitting through CLI/portal surfaces.

| Field | Type | Requirements |
|-------|------|--------------|
| `version` | `1` (u16) | Must equal `AGENDA_PROPOSAL_VERSION_V1`. |
| `proposal_id` | string (`AC-YYYY-###`) | Stable identifier; enforced during validation. |
| `submitted_at_unix_ms` | u64 | Milliseconds since Unix epoch. |
| `language` | string | BCP‑47 tag (`"en"`, `"ja-JP"`, etc.). |
| `action` | enum (`add-to-denylist`, `remove-from-denylist`, `amend-policy`) | Requested Ministry action. |
| `summary.title` | string | ≤256 chars recommended. |
| `summary.motivation` | string | Why the action is required. |
| `summary.expected_impact` | string | Outcomes if the action is accepted. |
| `tags[]` | lowercase strings | Optional triage labels. Allowed values: `csam`, `malware`, `fraud`, `harassment`, `impersonation`, `policy-escalation`, `terrorism`, `spam`. |
| `targets[]` | objects | One or more hash family entries (see below). |
| `evidence[]` | objects | One or more evidence attachments (see below). |
| `submitter.name` | string | Display name or organization. |
| `submitter.contact` | string | Email, Matrix handle, or phone; redacted from public dashboards. |
| `submitter.organization` | string (optional) | Visible in reviewer UI. |
| `submitter.pgp_fingerprint` | string (optional) | 40-hex uppercase fingerprint. |
| `duplicates[]` | strings | Optional references to previously submitted proposal IDs. |

### Target entries (`targets[]`)

Each target represents a hash family digest referenced by the proposal.

| Field | Description | Validation |
|-------|-------------|------------|
| `label` | Friendly name for reviewer context. | Non-empty. |
| `hash_family` | Hash identifier (`blake3-256`, `sha256`, etc.). | ASCII letters/digits/`-_.`, ≤48 chars. |
| `hash_hex` | Digest encoded in lowercase hex. | ≥16 bytes (32 hex chars) and must be valid hex. |
| `reason` | Short description of why the digest should be actioned. | Non-empty. |

The validator rejects duplicate `hash_family:hash_hex` pairs within the same
proposal and reports conflicts when the same fingerprint already exists in the
duplicate registry (see below).

### Evidence attachments (`evidence[]`)

Evidence entries document where reviewers can fetch supporting context.

| Field | Type | Notes |
|-------|------|-------|
| `kind` | enum (`url`, `torii-case`, `sorafs-cid`, `attachment`) | Determines digest requirements. |
| `uri` | string | HTTP(S) URL, Torii case ID, or SoraFS URI. |
| `digest_blake3_hex` | string | Required for `sorafs-cid` and `attachment` kinds; optional for others. |
| `description` | string | Optional free-form text for reviewers. |

### Duplicate registry

Operators can maintain a registry of existing fingerprints to prevent duplicate
cases. The validator accepts a JSON file shaped as:

```json
{
  "entries": [
    {
      "hash_family": "blake3-256",
      "hash_hex": "0d714bed4b7c63c23a2cf8ee9ce6c3cde1007907c427b4a0754e8ad31c91338d",
      "proposal_id": "AC-2025-014",
      "note": "Already handled in 2025-08 incident"
    }
  ]
}
```

When a proposal target matches an entry, the validator aborts unless
`--allow-registry-conflicts` is specified (warnings are still emitted).
Use [`cargo xtask ministry-agenda impact`](impact_assessment_tooling.md) to
generate the referendum-ready summary that cross-references the duplicate
registry and policy snapshots.

## CLI usage

Lint a single proposal and check it against a duplicate registry:

```bash
cargo xtask ministry-agenda validate \
  --proposal docs/examples/ministry/agenda_proposal_example.json \
  --registry docs/examples/ministry/agenda_duplicate_registry.json
```

Pass `--allow-registry-conflicts` to downgrade duplicate hits to warnings when
performing historical audits.

The CLI relies on the same Norito schema and validation helpers shipped in
`iroha_data_model`, so SDKs/portals can reuse the `AgendaProposalV1::validate`
method for consistent behaviour.

## Sortition CLI (MINFO-2b)

Roadmap reference: **MINFO-2b — Multi-slot sortition & audit log.**

The Agenda Council roster is now managed via deterministic sortition so citizens
can independently audit every draw. Use the new command:

```bash
cargo xtask ministry-agenda sortition \
  --roster docs/examples/ministry/agenda_council_roster.json \
  --slots 3 \
  --seed 0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef \
  --out artifacts/ministry/agenda_sortition_2026Q1.json
```

- `--roster` — JSON file describing every eligible member:

  ```json
  {
    "format_version": 1,
    "members": [
      {
        "member_id": "citizen:ada",
        "weight": 2,
        "role": "citizen",
        "organization": "Artemis Cooperative"
      },
      {
        "member_id": "citizen:erin",
        "weight": 1,
        "role": "citizen",
        "eligible": false
      }
    ]
  }
  ```

  The example file lives at
  `docs/examples/ministry/agenda_council_roster.json`. Optional fields (role,
  organization, contact, metadata) are captured in the Merkle leaf so auditors
  can prove the roster that fed the draw.

- `--slots` — number of council seats to fill.
- `--seed` — 32-byte BLAKE3 seed (64 lowercase hex characters) recorded in the
  governance minutes for the draw.
- `--out` — optional output path. When omitted, the JSON summary is printed to
  stdout.

### Output summary

The command emits a `SortitionSummary` JSON blob. Sample output is stored at
`docs/examples/ministry/agenda_sortition_summary_example.json`. Key fields:

| Field | Description |
|-------|-------------|
| `algorithm` | Sortition label (`agenda-sortition-blake3-v1`). |
| `roster_digest` | BLAKE3 + SHA-256 digests of the roster file (used to confirm audits operate over the same member list). |
| `seed_hex` / `slots` | Echo the CLI inputs so auditors can reproduce the draw. |
| `merkle_root_hex` | Root of the roster Merkle tree (`hash_node`/`hash_leaf` helpers in `xtask/src/ministry_agenda.rs`). |
| `selected[]` | Entries for each slot, including the canonical member metadata, eligible index, original roster index, deterministic draw entropy, leaf hash, and Merkle proof siblings. |

### Verifying a draw

1. Fetch the roster referenced by `roster_path` and verify its BLAKE3/SHA-256
   digests match the summary.
2. Re-run the CLI with the same seed/slots/roster; the resulting `selected[].member_id`
   order should match the published summary.
3. For a specific member, compute the Merkle leaf using the serialized member JSON
   (`norito::json::to_vec(&sortition_member)`) and fold in each proof hash. The final
   digest must equal `merkle_root_hex`. The helper in the example summary shows
   how to combine `eligible_index`, `leaf_hash_hex`, and `merkle_proof[]`.

These artefacts satisfy the MINFO-2b requirement for verifiable randomness,
k-of-m selection, and append-only audit logs until the on-chain API is wired.

## Validation error reference

`AgendaProposalV1::validate` emits `AgendaProposalValidationError` variants
whenever a payload fails linting. The table below summarises the most common
errors so portal reviewers can translate CLI output into actionable guidance.

| Error | Meaning | Remediation |
|-------|---------|-------------|
| `UnsupportedVersion { expected, found }` | Payload `version` differs from the validator’s supported schema. | Regenerate the JSON using the latest schema bundle so the version matches `expected`. |
| `MissingProposalId` / `InvalidProposalIdFormat { value }` | `proposal_id` is empty or not in `AC-YYYY-###` form. | Populate a unique identifier following the documented format before re-submitting. |
| `MissingSubmissionTimestamp` | `submitted_at_unix_ms` is zero or missing. | Record the submission timestamp in Unix milliseconds. |
| `InvalidLanguageTag { value }` | `language` is not a valid BCP‑47 tag. | Use a standard tag such as `en`, `ja-JP`, or another locale recognised by BCP‑47. |
| `MissingSummaryField { field }` | One of `summary.title`, `.motivation`, or `.expected_impact` is empty. | Provide non-empty text for the indicated summary field. |
| `MissingSubmitterField { field }` | `submitter.name` or `submitter.contact` missing. | Supply the missing submitter metadata so reviewers can contact the proposer. |
| `InvalidTag { value }` | `tags[]` entry not on the allowlist. | Remove or rename the tag to one of the documented values (`csam`, `malware`, etc.). |
| `MissingTargets` | `targets[]` array is empty. | Provide at least one target hash family entry. |
| `MissingTargetLabel { index }` / `MissingTargetReason { index }` | Target entry missing the `label` or `reason` fields. | Fill in the required field for the indexed entry before resubmitting. |
| `InvalidHashFamily { index, value }` | Unsupported `hash_family` label. | Restrict hash family names to ASCII alphanumerics plus `-_`. |
| `InvalidHashHex { index, value }` / `TargetDigestTooShort { index }` | Digest is not valid hex or is shorter than 16 bytes. | Provide a lowercase hex digest (≥32 hex chars) for the indexed target. |
| `DuplicateTarget { index, fingerprint }` | Target digest duplicates an earlier entry or registry fingerprint. | Remove duplicates or merge the supporting evidence into a single target. |
| `MissingEvidence` | No evidence attachments were supplied. | Attach at least one evidence record linking to reproduction material. |
| `MissingEvidenceUri { index }` | Evidence entry missing the `uri` field. | Provide the fetchable URI or case identifier for the indexed evidence entry. |
| `MissingEvidenceDigest { index }` / `InvalidEvidenceDigest { index, value }` | Evidence entry that requires a digest (SoraFS CID or attachment) is missing or has invalid `digest_blake3_hex`. | Supply a 64-character lowercase BLAKE3 digest for the indexed entry. |

## Examples

- `docs/examples/ministry/agenda_proposal_example.json` — canonical,
  lint-clean proposal payload with two evidence attachments.
- `docs/examples/ministry/agenda_duplicate_registry.json` — starter registry
  containing a single BLAKE3 fingerprint and rationale.

Reuse these files as templates when integrating portal tooling or writing CI
checks for automated submissions.
