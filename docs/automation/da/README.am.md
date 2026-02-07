---
lang: am
direction: ltr
source: docs/automation/da/README.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 8757f0bf8699b532ece29437af953353526b3201b4b129ebec7d6bf5d224f038
source_last_modified: "2025-12-29T18:16:35.061402+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# የውሂብ-ተገኝነት ስጋት-ሞዴል አውቶሜሽን (DA-1)

የፍኖተ ካርታ ንጥል DA-1 እና `status.md` የሚወስን አውቶማቲክ ዑደት ጥሪ
የ Norito PDP/PoTR ስጋት-ሞዴል ማጠቃለያዎችን ያወጣል።
`docs/source/da/threat_model.md` እና I18NT0000000X መስታወት። ይህ ማውጫ
በሚከተሉት የተጠቀሱ ቅርሶችን ይይዛል፡-

- `cargo xtask da-threat-model-report [--out <path|->] [--seed <u64|0xhex>] [--config <path>]`
- `.github/workflows/da-threat-model-nightly.yml`
- `make docs-da-threat-model` (`scripts/docs/render_da_threat_model_tables.py` የሚያሄድ)
- `cargo xtask da-commitment-reconcile --receipt <path> --block <path> [--json-out <path|->]`
- `cargo xtask da-privilege-audit --config <torii.toml> [--extra-path <path> ...] [--json-out <path|->]`

## ፍሰት

1. ** ሪፖርቱን ይፍጠሩ ***
   ```bash
   cargo xtask da-threat-model-report \
     --config configs/da/threat_model.toml \
     --out artifacts/da/threat_model_report.json
   ```
   የJSON ማጠቃለያ የተመሰለውን የማባዛት ውድቀት መጠን፣ chunker ይመዘግባል
   ገደቦች፣ እና በ PDP/PoTR መታጠቂያ ውስጥ የተገኙ ማንኛቸውም የመመሪያ ጥሰቶች
   `integration_tests/src/da/pdp_potr.rs`.
2. ** የማርከድ ሰንጠረዦችን ይስሩ**
   ```bash
   make docs-da-threat-model
   ```
   ይሄ `scripts/docs/render_da_threat_model_tables.py` እንደገና ለመፃፍ ይሰራል
   `docs/source/da/threat_model.md` እና I18NI0000020X።
3. **የቅርሶቹን በማህደር ያስቀምጡ** የJSON ዘገባ (እና አማራጭ የCLI ሎግ) በመገልበጥ
   `docs/automation/da/reports/<timestamp>-threat_model_report.json`. መቼ
   የአስተዳደር ውሳኔዎች በአንድ የተወሰነ ሩጫ ላይ ይመረኮዛሉ፣ የጂት መፈጸምን ሃሽ እና ያካትታሉ
   አስመሳይ ዘር በወንድም እህት `<timestamp>-metadata.md`.

## የማስረጃ የሚጠበቁ

- JSON ፋይሎች በgit ውስጥ መኖር እንዲችሉ <100 ኪቢ መቆየት አለባቸው። ትልቅ አፈጻጸም
  ዱካዎች በውጫዊ ማከማቻ ውስጥ ናቸው - የተፈረመባቸውን ሃሽ በዲበ ውሂቡ ውስጥ ያመልክቱ
  አስፈላጊ ከሆነ ማስታወሻ.
- እያንዳንዱ በማህደር የተቀመጠ ፋይል ዘሩን፣ ውቅር ዱካውን እና የሲሙሌተር ሥሪቱን መዘርዘር አለበት።
  የዲኤ መልቀቂያ በሮች ኦዲት በሚደረግበት ጊዜ ድጋሚዎች በትክክል ሊባዙ ይችላሉ።
- በማህደር የተቀመጠ ፋይል ከ`status.md` ወይም የመንገድ ካርታው መግቢያ በማንኛውም ጊዜ ይገናኙ
  የ DA-1 ተቀባይነት መስፈርት ቀድሟል፣ ገምጋሚዎች ማረጋገጥ መቻላቸውን ማረጋገጥ
  ማሰሪያውን እንደገና ሳያስኬድ መሰረታዊ.

## ቁርጠኝነት ማስታረቅ (ተከታታይ መቅረት)

DA የሚቀበሉትን ደረሰኞች ለማነፃፀር `cargo xtask da-commitment-reconcile` ይጠቀሙ
የዲኤ ቁርጠኝነት መዝገቦች፣ ተከታታዮች መቅረትን ወይም ማበላሸት

```bash
cargo xtask da-commitment-reconcile \
  --receipt artifacts/da/receipts/ \
  --block storage/blocks/ \
  --json-out artifacts/da/commitment_reconciliation.json
```

- ደረሰኞችን በ Norito ወይም JSON ቅፅ እና ቃል ኪዳኖችን ይቀበላል
  `SignedBlockWire`፣ `.norito`፣ ወይም JSON ጥቅሎች።
- ከብሎክ ሎግ ማንኛውም ቲኬት ሲጎድል ወይም hashes ሲለያይ አይሳካም;
  `--allow-unexpected` ሆን ብለው በሚወስኑበት ጊዜ የማገጃ-ብቻ ትኬቶችን ችላ ይላል
  ደረሰኙ ስብስብ.
- የተለቀቀውን JSON ከአስተዳደር ጥቅሎች/ለማስቀር ማንቂያ አስተዳዳሪ ጋር ያያይዙት።
  ማንቂያዎች; ለ `artifacts/da/commitment_reconciliation.json` ነባሪዎች።

## ልዩ ኦዲት (የሩብ ጊዜ መዳረሻ ግምገማ)

የDA መግለጫ/የድጋሚ አጫውት ማውጫዎችን ለመቃኘት `cargo xtask da-privilege-audit` ይጠቀሙ
(ከአማራጭ ተጨማሪ ዱካዎች በተጨማሪ) ለጠፉ፣ ማውጫ ላልሆኑ ወይም በዓለም ሊጻፍ የሚችል
ግቤቶች

```bash
cargo xtask da-privilege-audit \
  --config configs/torii.dev.toml \
  --extra-path /var/lib/iroha/da-manifests \
  --json-out artifacts/da/privilege_audit.json
```

- ከቀረበው Torii ውቅር የ DA ingest ዱካዎችን ያነባል እና ዩኒክስን ይመረምራል።
  ፍቃዶች በሚገኙበት ቦታ.
- የጠፉ/ማውጫ ያልሆነ/በአለም ሊፃፉ የሚችሉ መንገዶችን ባንዲራ እና ዜሮ ያልሆነ መውጫ ይመልሳል።
  ጉዳዮች በሚኖሩበት ጊዜ ኮድ.
- የJSON ቅርቅብ ይፈርሙ እና አያይዙ (`artifacts/da/privilege_audit.json` በ
  ነባሪ) ወደ ሩብ ወሩ የመዳረሻ-ግምገማ እሽጎች እና ዳሽቦርዶች።