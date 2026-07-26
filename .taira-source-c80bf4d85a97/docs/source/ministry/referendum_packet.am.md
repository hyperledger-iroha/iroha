---
lang: am
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

# የሪፈረንደም ፓኬት የስራ ፍሰት (MINFO-4)

የመንገድ ካርታ ንጥል **MINFO-4 — በግምገማ ፓነል እና ሪፈረንደም ሰብሳቢ** አሁን አለ።
በአዲሱ የ`ReferendumPacketV1` Norito እቅድ እና በ CLI ረዳቶች ተሟልቷል
ከዚህ በታች ተብራርቷል. የስራ ፍሰቱ ለፖሊሲ-ዳኞች የሚያስፈልጉትን ሁሉንም ቅርሶች ያጠቃልላል
አስተዳደር፣ ኦዲተሮች እና ግልጽነት በአንድ JSON ሰነድ ውስጥ ድምጽ ይሰጣል
ፖርታል ማስረጃውን በቁርጠኝነት እንደገና ማጫወት ይችላል።

## ግብዓቶች

1. **የአጀንዳ ፕሮፖዛል** - ተመሳሳይ JSON ለ`cargo xtask ministry-agenda validate` ጥቅም ላይ ውሏል።
2. **የበጎ ፈቃደኞች አጭር መግለጫዎች** - ከታሸገ በኋላ የተሰራው የተሰበሰበ የውሂብ ስብስብ
   `cargo xtask ministry-transparency volunteer-validate`.
3. ** AI ልከኝነት አንጸባራቂ *** - በአስተዳደር የተፈረመ `ModerationReproManifestV1`።
4. **የድርድሩ ማጠቃለያ** — የሚወስነው በ የሚለቀቀው ቅርስ
   `cargo xtask ministry-agenda sortition`. JSON ይከተላል
   [`PolicyJurySortitionV1`](./policy_jury_ballots.md) ስለዚህ አስተዳደር ይችላል
   የPOP ቅጽበተ-ፎቶ ዳይጄስት እና የተጠባባቂ ዝርዝር/ያልተሳካው ሽቦ ማባዛት።
5. **የተጽዕኖ ሪፖርት** - ሃሽ-ቤተሰብ/ሪፖርት የተፈጠረው በ
   `cargo xtask ministry-agenda impact`.

## የ CLI አጠቃቀም

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

የ`packet` ንዑስ ትዕዛዝ ገለልተኛ-ማጠቃለያ አቀናባሪውን (MINFO-4a) ያካሂዳል፣ እንደገና ይጠቀማል
አሁን ያሉት የበጎ ፈቃደኞች እቃዎች፣ እና ውጤቱን በሚከተሉት ያበለጽጋል፡-

- `ReferendumSortitionEvidence` — አልጎሪዝም፣ ዘር እና የስም ዝርዝር መፈጨት ከ
  መደርደር artefact.
- `ReferendumPanelist[]` - እያንዳንዱ የተመረጠ የምክር ቤት አባል እና የመርክል ማረጋገጫ
  የእነሱን ስዕል ኦዲት ማድረግ ያስፈልጋል.
- `ReferendumImpactSummary` - በአንድ-ሃሽ-ቤተሰብ ድምር እና የግጭት ዝርዝሮች ከ
  ተጽዕኖ ሪፖርት.

አሁንም ብቻውን `ReviewPanelSummaryV1` ሲፈልጉ `--summary-out` ይጠቀሙ
ፋይል; አለበለዚያ ፓኬቱ ማጠቃለያውን በ `review_summary` ውስጥ አካቷል።

## የውጤት መዋቅር

`ReferendumPacketV1` ይኖራል
`crates/iroha_data_model/src/ministry/mod.rs` እና በኤስዲኬዎች ላይ ይገኛል።
ዋና ክፍሎች የሚከተሉትን ያካትታሉ:

- `proposal` - የመጀመሪያው `AgendaProposalV1` ነገር።
- `review_summary` - በ MINFO-4a የወጣው ሚዛናዊ ማጠቃለያ።
- `sortition` / `panelists` - ለተቀመጠው ምክር ቤት ሊባዙ የሚችሉ ማረጋገጫዎች።
- `impact_summary` - የተባዛ/የፖሊሲ ግጭት ማስረጃ በሃሽ ቤተሰብ።

ለሙሉ ናሙና `docs/examples/ministry/referendum_packet_example.json` ይመልከቱ።
የተፈጠረውን ፓኬት ከተፈረመው AI ጋር ለእያንዳንዱ የሪፈረንደም ዶሴ ያያይዙት።
በድምቀቶች ክፍል የተጠቀሱ አንጸባራቂ እና ግልጽነት ቅርሶች።