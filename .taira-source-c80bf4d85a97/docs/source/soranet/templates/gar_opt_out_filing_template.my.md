---
lang: my
direction: ltr
source: docs/source/soranet/templates/gar_opt_out_filing_template.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 4bd110456bdef0ac3cd7ef2e71babaeac062b2c28b13fcb92768740b80b47e6f
source_last_modified: "2025-12-29T18:16:36.202124+00:00"
translation_last_reviewed: 2026-02-07
title: SoraNet Opt-Out Filing Template
summary: Boilerplate request package for SNNet-9 compliance filings that add jurisdiction or manifest entries to the GAR opt-out catalogue.
---

# SoraNet Opt-Out Filing Template (SNNet-9)

Use this template when requesting updates to
`governance/compliance/soranet_opt_outs.json`. It captures the metadata the
Governance Council, Legal, and Operations teams need before merging a new
jurisdictional or blinded-CID opt-out. Submitters can either copy this document
into their request ticket or export the filled template to Norito/Markdown for
archival.

## How to use this template

1. Duplicate the **Request Metadata**, **Jurisdiction Opt-Out Entries**, and
   **Blinded-CID Opt-Out Entries** sections per filing.
2. Replace every `{{ placeholder }}` with the requested value.
3. Attach the signed attestation(s) referenced by `digest_hex` plus any regional
   approvals cited in the legal basis column.
4. Include the rendered JSON patch (see the sample below) so reviewers can diff
   the requested change against the canonical catalogue.

## Request Metadata

| Field | Description | Example |
| --- | --- | --- |
| **Request ID** | Deterministic identifier (`GAR-REQ-YYYYMMDD-ORG`) for audit logs. | `GAR-REQ-20250317-ASTER` |
| **Submission date** | UTC timestamp when the request is filed. | `2025-03-17T11:45:00Z` |
| **Submitting org / contact** | Legal entity and escalation contacts (email + pager/Matrix). | `Aster Labs — legal@soranet.example, +1-555-0100` |
| **Change owner(s)** | Operational lead approving the rollout. | `SoraNet Ops — ops@soranet.org` |
| **Requested activation window** | Earliest/Latest acceptable activation timestamps. | `No earlier than 2025-03-24T00:00Z; no later than 2025-03-31T23:59Z` |
| **Reason summary** | One sentence describing why the opt-out is required. | `Regulator mandated direct-only transport for CA-bound traffic.` |
| **Attachments** | List of signed PDFs/Norito manifests bundled with the request. | `CA-ruling.pdf`, `CAN-privacy-opinion.norito` |

## Jurisdiction Opt-Out Entries

Populate this table for every ISO-3166 alpha-2 code being added or removed. Omit
the section when no jurisdictional changes are proposed.

| ISO code | Legal basis / citation | Scope notes | Supporting attestation digest (Blake2b-256 hex) | Proposed expiry | Telemetry requirements |
| --- | --- | --- | --- | --- | --- |
| `{{ code }}` | `{{ statute and section, docket ID, regulator memo, etc. }}` | `{{ e.g., “all exit relays serving CA residents” }}` | `{{ 64-char hex digest }}` | `{{ YYYY-MM-DD or “None” }}` | `{{ dashboards/alerts that prove enforcement }}` |

> **Tip:** When multiple statutes cover the same country, submit one row per
> statute so reviewers can track the independent expiry / repeal status.

## Blinded-CID Opt-Out Entries

Use this table for manifest-level carve-outs. Provide the Blake2b-256 digest
matching the manifest or Taikai envelope that must remain in direct-only mode.

| Manifest / dataset name | CID digest (hex) | Reason & data classification | Controller contact | Evidence digest | Monitoring notes |
| --- | --- | --- | --- | --- | --- |
| `{{ slug }}` | `{{ 64-char hex digest }}` | `{{ e.g., “medical research dataset — privacy embargo” }}` | `{{ name/email }}` | `{{ hash of supporting memo }}` | `{{ dashboards / alert IDs }}` |

> **Reminder:** Manifest digests must align with the CID recorded on-ledger. If
> you are blocking an envelope series, enumerate each digest explicitly to keep
> the GAR catalogue deterministic.

## Attestation & Evidence Checklist

- [ ] Signed legal order or regulator memo bundled with the request.
- [ ] Blake2b-256 digest (`digest_hex`) published for every attachment.
- [ ] Contact matrix for post-activation verification (governance, operator,
      regulator).
- [ ] Planned communication (status page, operator mailing list, SDK release
      note) included in the request.
- [ ] Rollback or expiry criteria defined (what evidence removes the opt-out).

## JSON Patch Snippet

Provide the exact JSON that will be merged into `soranet_opt_outs.json`. Use the
following scaffold as a starting point:

```jsonc
{
  "jurisdiction_opt_outs": [
    "CA",
    "US"
  ],
  "blinded_cid_opt_outs": [
    "C6B434E5F23ABD318F01FEDB834B34BD16B46E0CC44CD70536233A632DFA3828",
    "7F8B1E9D04878F1AEAB553C1DB0A3E3A2AB689F75FE6BE17469F85A4D201B4AC"
  ],
  "attestations": [
    {
      "jurisdiction": "CA",
      "document_uri": "https://gov.example.org/gar/2025-03-ca-ruling.pdf",
      "digest_hex": "37E8AA2E0C9BE3D64B2812747307D534642856A3F234D6F087E42A54B3A8380F",
      "issued_at_ms": 1742088000000,
      "expires_at_ms": 1750051200000
    },
    {
      "document_uri": "https://gov.example.org/gar/medical-dataset.json",
      "digest_hex": "9D0E16C6F42449F57AD1BB221B35A484AF881B40B3F99588635B9A5FB1F88DA2",
      "issued_at_ms": 1742088000000
    }
  ]
}
```

Include the Blake2b input artefacts and the CLI command you used to derive the
digests (for example: `b2sum legal-order.pdf`).

## Sign-off Checklist

- [ ] Legal review completed (link to approval minutes or Norito envelope).
- [ ] Governance Council vote recorded with quorum reference.
- [ ] Operations acknowledged the activation window and telemetry impact.
- [ ] Communications prepared the status-page copy and SDK notices.
- [ ] Repository tag (`gar-opt-out-YYYYMMDD`) reserved for the rollout.

Archive the filled template alongside the signed artefacts so auditors can trace
how the opt-out entered production. Update
`docs/source/soranet/gar_compliance_playbook.md` if your workflow introduces new
steps or artefacts.
