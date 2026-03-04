---
lang: mn
direction: ltr
source: docs/examples/da_manifest_review_template.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: c5c959bd6654d095d2b3785a02e9c2ec162e699ad985b342760b952e38766a66
source_last_modified: "2025-12-29T18:16:35.069812+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Мэдээллийн бэлэн байдлын манифест засаглалын багц (загвар)

УИХ-ын зөвлөлүүд татаас авах тухай DA манифестийг хянан үзэх үед энэ загварыг ашиглана уу.
татан буулгах, эсвэл хадгалах өөрчлөлт (замын зураг DA-10). Markdown-г дотор нь хуулна уу
засаглалын тасалбар, орлуулагчийг бөглөж, бөглөсөн файлыг хавсаргана уу
гарын үсэг зурсан Norito даац болон доор дурдсан CI олдворуудын хамт.

```markdown
## Manifest Metadata
- Manifest name / version: <string>
- Blob class & governance tag: <taikai_segment · da.taikai.live>
- BLAKE3 digest (hex): `<digest>`
- Norito payload hash (optional): `<digest>`
- Source envelope / URL: <https://.../manifest_signatures.json>
- Torii policy snapshot ID: `<unix timestamp or git sha>`

## Signature Verification
- Manifest fetch source / storage ticket: `<hex>`
- Verification command/output: `cargo run -p sorafs_car --bin sorafs-manifest-stub -- manifest.json --manifest-signatures-in=manifest_signatures.json` (log excerpt attached?)
- `manifest_blake3` reported by tool: `<digest>`
- `chunk_digest_sha3_256` reported by tool: `<digest>`
- Council signer multihashes:
  - `<did:...>` / `<ed25519 multihash>`
- Verification timestamp (UTC): `<2026-02-20T11:04:33Z>`

## Retention Verification
| Field | Expected (policy) | Observed (manifest) | Evidence |
|-------|-------------------|---------------------|----------|
| Hot retention (seconds) | <e.g., 86400> | <value> | `<torii.da_ingest.replication_policy dump | CI link>` |
| Cold retention (seconds) | <e.g., 1209600> | <value> |  |
| Required replicas | <value> | <value> |  |
| Storage class | <hot / warm / cold> | <value> |  |
| Governance tag | <da.taikai.live> | <value> |  |

## Context
- Request type: <Subsidy | Takedown | Manifest rotation | Emergency freeze>
- Origin ticket / compliance reference: <link or ID>
- Subsidy / rent impact: <expected XOR change or “n/a”>
- Moderation appeal link (if any): <case_id or link>

## Decision Summary
- Panel: <Infrastructure | Moderation | Treasury>
- Vote tally: `<for>/<against>/<abstain>` (quorum `<threshold>` met?)
- Activation / rollback height or window: `<block/slot range>`
- Follow-up actions:
  - [ ] Notify Treasury / rent ops
  - [ ] Update transparency report (`TransparencyReportV1`)
  - [ ] Schedule buffer audit

## Escalation & Reporting
- Escalation track: <Subsidy | Compliance | Emergency Freeze>
- Transparency report link / ID (if updated): <`TransparencyReportV1` CID>
- Proof-token bundle or ComplianceUpdate reference: <path or ticket ID>
- Rent / reserve ledger delta (if applicable): <`ReserveSummaryV1` snapshot link>
- Telemetry snapshot URL(s): <Grafana permalink or artefact ID>
- Notes for Parliament minutes: <summary of deadlines / obligations>

## Attachments
- [ ] Signed Norito manifest (`.to`)
- [ ] JSON summary / CI artefact proving retention values
- [ ] Proof token or compliance packet (for takedowns)
- [ ] Buffer telemetry snapshot (`iroha_settlement_buffer_xor`)
```

Дууссан багц бүрийг Засаглалын DAG бичилтийн дор архивлаж, санал хураалт явуулна уу
дараагийн тоймууд нь бүрэн гүйцэд давталгүйгээр манифест тоймд лавлаж болно
ёслол.【docs/source/governance_playbook.md:24】