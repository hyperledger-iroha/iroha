---
lang: zh-hans
direction: ltr
source: docs/examples/da_manifest_review_template.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: c5c959bd6654d095d2b3785a02e9c2ec162e699ad985b342760b952e38766a66
source_last_modified: "2025-12-29T18:16:35.069812+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# 数据可用性清单治理数据包（模板）

当议会小组审查 DA 补贴清单时使用此模板，
删除或保留更改（路线图 DA-10）。将 Markdown 复制到
治理票证，填写占位符，并附加完整的文件
以及下面引用的签名的 Norito 有效负载和 CI 工件。

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

将每个已完成的数据包存档在治理 DAG 条目下进行投票，以便
后续评论可以参考清单摘要，而无需重复完整内容
仪式。【docs/source/governance_playbook.md:24】