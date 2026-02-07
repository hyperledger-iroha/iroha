---
lang: am
direction: ltr
source: docs/examples/soranet_gar_intake_form.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 6cd4da7e590d581719ed2607994d7d9eb16d153fbd06f85655d0da37c727853a
source_last_modified: "2025-12-29T18:16:35.085419+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# SoraNet GAR ማስገቢያ አብነት

የ GAR እርምጃ ሲጠይቁ ይህንን የመቀበያ ቅጽ ይጠቀሙ (ማጽዳት፣ ttl መሻር፣ ተመን
ጣሪያ፣ የአወያይ መመሪያ፣ ጂኦፌንስ ወይም ሕጋዊ መያዣ)። የቀረበው ቅጽ
ከ `gar_controller` ውጤቶች ጋር መሰካት አለበት ስለዚህ የኦዲት ምዝግብ ማስታወሻዎች እና
ደረሰኞች ተመሳሳይ ማስረጃዎችን URIs ይጠቅሳሉ።

| መስክ | ዋጋ | ማስታወሻ |
|-------|-------|------|
| የጥያቄ መታወቂያ |  | ጠባቂ/ops የቲኬት መታወቂያ። |
| የተጠየቀው በ |  | መለያ + እውቂያ። |
| ቀን/ሰዓት (UTC) |  | እርምጃው መጀመር ያለበት መቼ ነው። |
| የጋር ስም |  | ለምሳሌ፣ I18NI0000002X። |
| ቀኖናዊ አስተናጋጅ |  | ለምሳሌ፣ `docs.gw.sora.net`። |
| ድርጊት |  | `ttl_override` / `rate_limit_override` / `purge_static_zone` / `geo_fence` / `legal_hold` / `moderation`. |
| ቲቲኤል መሻር (ሰከንዶች) |  | ለ`ttl_override` ብቻ ያስፈልጋል። |
| የደረጃ ጣሪያ (RPS) |  | ለ`rate_limit_override` ብቻ ያስፈልጋል። |
| የተፈቀዱ ክልሎች |  | `geo_fence` ሲጠይቁ የ ISO ክልል ዝርዝር። |
| የተከለከሉ ክልሎች |  | `geo_fence` ሲጠይቁ የ ISO ክልል ዝርዝር። |
| ልከኛ ተንሸራታቾች |  | የGAR አወያይ መመሪያዎችን ያዛምዱ። |
| መለያዎችን አጽዳ |  | ከማገልገልዎ በፊት መጽዳት ያለባቸው መለያዎች። |
| መለያዎች |  | የማሽን መለያዎች (የክስተት መታወቂያ፣ የመሰርሰሪያ ስም፣ የፖፕ ወሰን)። |
| ማስረጃ URI |  | ጥያቄውን የሚደግፉ ምዝግብ ማስታወሻዎች / ዳሽቦርዶች / ዝርዝሮች። |
| ኦዲት URI |  | ከነባሪዎች የተለየ ከሆነ በየፖፕ ኦዲት URI። |
| የማለቂያ ጊዜ ተጠይቋል |  | ዩኒክስ የጊዜ ማህተም ወይም RFC3339; ለነባሪ ባዶ ይተዉት። |
| ምክንያት |  | በተጠቃሚው ፊት ለፊት ያለው ማብራሪያ; ደረሰኞች እና ዳሽቦርዶች ውስጥ ይታያል. |
| አጽዳቂ |  | ለጥያቄው ጠባቂ/ኮሚቴ አጽዳቂ። |

### የማስረከቢያ ደረጃዎች

1. ጠረጴዛውን ይሙሉ እና ከአስተዳደር ትኬት ጋር አያይዘው.
2. የGAR መቆጣጠሪያ ውቅረትን (`policies`/`pops`) በማዛመድ ያዘምኑ።
   `labels`/`evidence_uris`/`expires_at_unix`።
3. ክስተቶችን/ደረሰኞችን ለመልቀቅ `cargo xtask soranet-gar-controller ...`ን ያሂዱ።
4. I18NI0000020X፣ `gar_reconciliation_report.json` ጣል፣
   `gar_metrics.prom`፣ እና I18NI0000023X ወደ ተመሳሳይ ቲኬት። የ
   አጽዳቂው ከመላኩ በፊት የደረሰኝ ቆጠራ ከፖፕ ዝርዝር ጋር እንደሚመሳሰል ያረጋግጣል።