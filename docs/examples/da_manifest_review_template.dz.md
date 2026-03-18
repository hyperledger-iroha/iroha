---
lang: dz
direction: ltr
source: docs/examples/da_manifest_review_template.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: c5c959bd6654d095d2b3785a02e9c2ec162e699ad985b342760b952e38766a66
source_last_modified: "2025-12-29T18:16:35.069812+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# གནད་སྡུད་ཐོབ་ཚུགསཔ་ ངོ་མཚར་ཅན་གྱི་གཞུང་སྐྱོང་ཐུམ་སྒྲིལ་ (Teplate)

སྤྱི་ཚོགས་ཀྱི་པེ་ནཱལ་ཚུ་གིས་ གྲོགས་རམ་གྱི་དོན་ལུ་ གསལ་སྟོན་ཚུ་ བསྐྱར་ཞིབ་འབད་བའི་སྐབས་ ཊེམ་པེལེཊི་འདི་ལག་ལེན་འཐབ།
བླུག་སའི་ཡང་ན་ བཀག་འཛིན་གྱི་འགྱུར་བ་ (ལམ་སབ་ཁྲ་ DA-10)། མརཀ་ཌའོན་འདི་ འདྲ་བཤུས་རྐྱབས།
གཞུང་སྐྱོང་གི་ཤོག་འཛིན་དང་ ས་གནས་འཛིན་མི་ཚུ་བཀང་ཞིནམ་ལས་ ཡིག་སྣོད་མཇུག་བསྡུ་ཡོད་མི་དེ་ མཉམ་སྦྲགས་འབད་ནི།
འདི་དང་གཅིག་ཁར་ མཚན་རྟགས་བཀོད་ཡོད་པའི་ I18NT0000001X གླ་ཆ་དང་ CI ཅ་རྙིང་ཚུ་ གཤམ་གསལ་ལྟར་ཨིན།

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

ཚོགས་རྒྱན་བཙུགས་ནིའི་དོན་ལུ་ གཞུང་སྐྱོང་ DAG འཛུལ་བའི་འོག་ལུ་ ཐུམ་སྒྲིལ་མཇུག་བསྡུ་ཡོད་པའི་ ཐུམ་སྒྲིལ་ཚུ་ག་ར་ གཏན་མཛོད་འབད་ནི།
ཤུལ་མམ་གྱི་བསྐྱར་ཞིབ་ཚུ་གིས་ གསལ་སྟོན་འདི་ ཆ་ཚང་སྦེ་བསྐྱར་ལོག་མ་འབད་བར་ གསལ་སྟོན་འབད་ཚུགས།
མཛད་སྒོ།【dcs/souvernance_playbook.md:24】