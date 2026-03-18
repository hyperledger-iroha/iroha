---
lang: am
direction: ltr
source: docs/source/compliance/android/eu/legal_signoff_memo_2026-02.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: eb92b77765ced36213a0bde55581f29d59c262f398c658f35a1fb43a182fe296
source_last_modified: "2025-12-29T18:16:35.926476+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# AND6 የአውሮፓ ህብረት ህጋዊ የመግቢያ ማስታወሻ - 2026.1 GA (አንድሮይድ ኤስዲኬ)

## ማጠቃለያ

- **መልቀቅ / ባቡር፡** 2026.1 GA (አንድሮይድ ኤስዲኬ)
- ** የግምገማ ቀን: *** 2026-04-15
- ** አማካሪ / ገምጋሚ: *** ሶፊያ ማርቲንስ - ተገዢነት እና ህጋዊ
- ** ወሰን፡** ETSI EN 319 401 የደኅንነት ዒላማ፣ የGDPR DPIA ማጠቃለያ፣ የ SBOM ማረጋገጫ፣ AND6 የመሣሪያ-ላብራቶሪ ድንገተኛ ማስረጃ
- ** የተቆራኙ ቲኬቶች፡** `_android-device-lab`/ AND6-DR-202602፣ AND6 አስተዳደር መከታተያ (`GOV-AND6-2026Q1`)

## የአርቴፍክት ማረጋገጫ ዝርዝር

| Artefact | SHA-256 | አካባቢ / አገናኝ | ማስታወሻ |
|-------|--------|
| `security_target.md` | `385d17a55579d2b0b365e21090ee081ded79e44655690b2abfbf54068c9b55b0` | `docs/source/compliance/android/eu/security_target.md` | ግጥሚያዎች 2026.1 GA የመልቀቂያ መለያዎች እና የማስፈራሪያ ሞዴል ዴልታስ (Torii NRPC ተጨማሪዎች)። |
| `gdpr_dpia_summary.md` | `8ef338a20104dc5d15094e28a1332a604b68bdcfef1ff82fea784d43fdbd10b5` | `docs/source/compliance/android/eu/gdpr_dpia_summary.md` | ዋቢዎች AND7 የቴሌሜትሪ ፖሊሲ (`docs/source/sdk/android/telemetry_redaction.md`)። |
| `sbom_attestation.md` | `c2e0de176d4bb8c8e09329e2b9ee5dd93228d3f0def78225c1d8b777a5613f2d` | `docs/source/compliance/android/eu/sbom_attestation.md` + Sigstore ጥቅል (`android-sdk-release#4821`)። | CycloneDX + provenance ተገምግሟል; ግጥሚያዎች Buildkite ሥራ `android-sdk-release#4821`. |
| የማስረጃ መዝገብ | `0b2d2f9eddada06faa70620f608c3ad1ec38f378d2cbddc24b15d0a83fcc381d` | `docs/source/compliance/android/evidence_log.csv` (ረድፍ `android-device-lab-failover-20260220`) | በምዝግብ ማስታወሻ የተያዘ የጥቅል ሀሽ + የአቅም ቅጽበታዊ + ማስታወሻ ግቤት ያረጋግጣል። |
| የመሣሪያ-ላብራቶሪ የአደጋ ጊዜ ጥቅል | `faf32356dfc0bbca1459b14d75f3306ea1c10cb40f3180fe1758ac5105016f85` | `artifacts/android/device_lab_contingency/20260220-failover-drill/` | Hash ከ `bundle-manifest.json` የተወሰደ; ቲኬት AND6-DR-202602 ወደ ህጋዊ/አስፈፃሚነት እጅ መስጠትን ተመዝግቧል። |

## ግኝቶች እና ልዩነቶች

- ምንም የማገድ ጉዳዮች አልተለዩም። ቅርሶች ከ ETSI/GDPR መስፈርቶች ጋር ይጣጣማሉ; AND7 የቴሌሜትሪ እኩልነት በDPIA ማጠቃለያ ላይ ተጠቅሷል እና ምንም ተጨማሪ ቅነሳ አያስፈልግም።
- የውሳኔ ሃሳብ፡ የታቀደውን የ DR-2026-05-Q2 ልምምድ (ትኬት AND6-DR-202605) ተቆጣጠር እና ውጤቱን ከማስረጃ መዝገብ ጋር በሚቀጥለው የአስተዳደር ፍተሻ ጣቢያ ላይ ጨምር።

## ማጽደቅ

- ** ውሳኔ: ** ጸድቋል
- **ፊርማ/የጊዜ ማህተም፡** _ሶፊያ ማርቲንስ (በአሃዛዊ መልኩ የተፈረመ በአስተዳደር ፖርታል፣ 2026-04-15 14፡32 UTC)_
- ** ተከታይ ባለቤቶች፡** የመሣሪያ ላብ ኦፕስ (ከ2026-05-31 በፊት DR-2026-05-Q2 ማስረጃ ጥቅል ያቅርቡ)