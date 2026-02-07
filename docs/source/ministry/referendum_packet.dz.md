---
lang: dz
direction: ltr
source: docs/source/ministry/referendum_packet.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 922d972376b67a2f8c0c03ded95db6576e16e229e4bcb62d920b0ffda49c93ac
source_last_modified: "2025-12-29T18:16:35.980526+00:00"
translation_last_reviewed: 2026-02-07
title: Referendum Packet Workflow (MINFO-4)
summary: Produce the complete referendum dossier (`ReferendumPacketV1`) combining the proposal, neutral summary, sortition artefacts, and impact report.
translator: machine-google-reviewed
---

# རི་ཕེ་ཧྥེ་རེཌ་ པེ་ཀེཊི་ ལཱ་གི་རྒྱུན་རིམ་ (MINFO-4).

ལམ་སྟོན་རྣམ་གྲངས་ **MINFO-4 — བསྐྱར་ཞིབ་ཚོགས་ཆུང་དང་འོས་འཚམ་གྱི་ མཉམ་སྦྱོར་འཕྲུལ་ཆས་ནང་** ད་ལྟོ་འདི་ཨིན།
`ReferendumPacketV1` Norito འཆར་གཞི་གསརཔ་གིས་ གྲུབ་ཡོདཔ་ཨིན།
གཤམ་གསལ་གྱི་འགྲེལ་བཤད་ནང་། ལཱ་གི་རྒྱུན་རིམ་འདི་གིས་ སྲིད་བྱུས་ཁྲིམས་དཔོན་གྱི་དོན་ལས་ དགོ་པའི་ ཅ་ཆས་ཚུ་ མཉམ་སྡེབ་འབདཝ་ཨིན།
ཚོགས་རྒྱན་ཚུ་ རྩིས་ཞིབ་པ་དང་ དྭངས་གསལ་གྱི་ JSON ཡིག་ཆ་ཅིག་ལུ་ ཚོགས་རྒྱན་བཙུགས་ནི།
དྲྭ་ཚིགས་ཚུ་གིས་ སྒྲུབ་བྱེད་ཚུ་ གཏན་འབེབས་བཟོ་ཚུགས།

## ཨིན་པུཊི་ཚུ།

1. **Agenda གྲོས་འཆར་** — དེ་དང་འདྲ་བར་ `cargo xtask ministry-agenda validate` གི་དོན་ལུ་ལག་ལེན་འཐབ་ཡོདཔ་ཨིན།
2. **volunteer flues** — བརྒྱུད་དེ་ linking གི་ཤུལ་ལས་ བཏོན་མི་ གནས་སྡུད་ཆ་ཚན་འདི།
   `cargo xtask ministry-transparency volunteer-validate`.
3. **ཚད་གཞིའི་གསལ་སྟོན་** — གཞུང་སྐྱོང་གིས་མིང་རྟགས་བཀོད་ཡོདཔ། `ModerationReproManifestV1`.
4. **དབྱེ་འབྱེད་བཅུད་དོན།** — གཏན་འབེབས་ཀྱི་ཅ་ལག་བཏོན་ཡོད།
   `cargo xtask ministry-agenda sortition`. JSON འདི་གཤམ་གསལ་ལྟར།
   [`PolicyJurySortitionV1`](./policy_jury_ballots.md) དེ་གཞུང་སྐྱོང་ཅན།
   POP གི་པར་བཞུ་དང་ བསྒུག་ཐོ་/འཐུས་ཤོར་གྱི་གློག་ཐག་འདི་ ལོག་བཏོན་དགོ།
༥. **གནོད་པའི་སྙན་ཞུ་** — བརྒྱུད་དེ་ ཧེཤ་-བཟའ་ཚང་/སྙན་ཞུ་བཟོ་བཏོན་འབད་ཡོདཔ།
   `cargo xtask ministry-agenda impact`.

## ཀླད་ཀོར་ལག་ལེན།

```bash
cargo xtask ministry-panel packet \
  --proposal artifacts/ministry/proposals/AC-2026-001.json \
  --volunteer artifacts/ministry/volunteer_briefs.json \
  --ai-manifest artifacts/ministry/ai_manifest.json \
  --panel-round RP-2026-05 \
  --sortition artifacts/ministry/agenda_sortition_2026Q1.json \
  --impact artifacts/ministry/impact/AC-2026-001.json \
  --summary-out artifacts/ministry/review_panel_summary.json \
  --output artifacts/ministry/referendum_packets/AC-2026-001.json
```

`packet` བརྡ་བཀོད་འོག་མ་འདི་གིས་ བར་ལམ་གྱི་བཅུད་བསྡུས་ (MINFO-4a), ལོག་ལག་ལེན་འཐབ་ཨིན།
ད་ལྟོ་ཡོད་པའི་ཁས་བླངས་པའི་སྒྲིག་བཀོད་ཚུ་གིས་ ཐོན་འབྲས་འདི་ མཐོ་དྲགས་བཟོཝ་ཨིན།

- `ReferendumSortitionEvidence` — ཨེལ་ཨཱལ་གོ་རི་དམ་དང་ སོན་དང་ རོ་སི་ཊར་ཚུ་ ནང་ལས་ བཞུ་བཅུགཔ་ཨིན།
  དབྱེ་སེལ་གྱི་ཅ་ཆས།
- `ReferendumPanelist[]` — གདམ་འཐུ་འབད་མི་ཚོགས་སྡེའི་འཐུས་མི་རེ་རེ་དང་ Merkle བདེན་དཔང་།
  ཁོང་གི་རི་མོ་རྩིས་ཞིབ་འབད་དགོཔ།
- `ReferendumImpactSummary` — ཧཤ་བཟའ་ཚང་རེ་ལུ་བསྡོམས་རྩིས་དང་ འཁྲུག་རྩོད་ཀྱི་ཐོ་འགོད་ཚུ།
  གནོད་སྐྱོན་སྙན་ཞུ།

ཁྱོད་ལུ་ད་ལྟོ་ཡང་ རང་དབང་གི་ `ReviewPanelSummaryV1` དགོ་པའི་སྐབས་ `--summary-out` ལག་ལེན་འཐབ།
ཡིག༌སྣོད; དེ་མེན་པ་ཅིན་ སྦུང་ཚན་འདི་གིས་ `review_summary` གི་འོག་ལུ་ བཅུད་བསྡུས་འདི་བཙུགས་ཡོདཔ་ཨིན།

## ཐོན་འབྲས་གཞི་བཀོད།

```bash
cargo xtask ministry-panel packet \
  --proposal artifacts/ministry/proposals/AC-2026-001.json \
  --volunteer artifacts/ministry/volunteer_briefs.json \
  --ai-manifest artifacts/ministry/ai_manifest.json \
  --panel-round RP-2026-05 \
  --sortition artifacts/ministry/agenda_sortition_2026Q1.json \
  --impact artifacts/ministry/impact/AC-2026-001.json \
  --summary-out artifacts/ministry/review_panel_summary.json \
  --output artifacts/ministry/referendum_packets/AC-2026-001.json
``` 2 2013 བར་ན།
`crates/iroha_data_model/src/ministry/mod.rs` དང་ SDKs ཚུ་ནང་ འཐོབ་ཚུགས།
ལྡེ་མིག་དབྱེ་ཚན་ཚུ་ནང་།

- `proposal` — `AgendaProposalV1` དངོས་པོ་ངོ་མ།
- `review_summary` — MINFO-4a གིས་བཏོན་མི་ འདྲ་མཉམ་གྱི་བཅུད་བསྡུས་འདི་ཨིན།
- `sortition` / `panelists` — སྡོད་གནས་ཀྱི་ཚོགས་སྡེ་གི་དོན་ལུ་ བསྐྱར་བཟོ་འབད་བཏུབ་པའི་བདེན་ཁུངས་ཚུ།
- `impact_summary` — ཧ་ཤི་བཟའ་ཚང་རེ་ལུ་ སྲིད་བྱུས་/སྲིད་དོན་གྱི་འཁྲུག་རྩོད་ཀྱི་སྒྲུབ་བྱེད་ཨིན།

དཔེ་ཚད་ཆ་ཚང་ཅིག་གི་དོན་ལུ་ `docs/examples/ministry/referendum_packet_example.json` ལུ་བལྟ།
མཚན་རྟགས་བཀོད་ཡོད་པའི་ཨེ་ཨའི་དང་གཅིག་ཁར་ འོས་འདེམས་ཀྱི་ཡིག་ཆ་ག་ར་ལུ་ བཟོ་བཏོན་འབད་ཡོད་པའི་སྦུང་ཚན་འདི་ མཉམ་སྦྲགས་འབད།
འོད་རྟགས་དབྱེ་ཚན་གྱིས་ གཞི་བསྟུན་འབད་ཡོད་པའི་ གསལ་སྟོན་དང་ དྭངས་གསལ་ཅན་གྱི་ ཅ་རྙིང་ཚུ།