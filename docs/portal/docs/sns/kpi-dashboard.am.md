---
lang: am
direction: ltr
source: docs/portal/docs/sns/kpi-dashboard.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 3649db9b00f9be968cfeb98bc34bbc797aaf22d7ac3936698b4f562094911073
source_last_modified: "2025-12-29T18:16:35.173090+00:00"
translation_last_reviewed: 2026-02-07
title: SNS KPI dashboard
description: Live Grafana panels that aggregate registrar, freeze, and revenue metrics for SN-8a.
translator: machine-google-reviewed
---

# የሶራ ስም አገልግሎት KPI ዳሽቦርድ

የKPI ዳሽቦርድ ለመጋቢዎች፣ አሳዳጊዎች እና ተቆጣጣሪዎች አንድ ቦታ ይሰጣል
የጉዲፈቻ፣ የስህተት እና የገቢ ምልክቶችን ከወርሃዊ አባሪው በፊት ይገምግሙ
(SN-8a) የ Grafana ፍቺ በማጠራቀሚያው ውስጥ በ
`dashboards/grafana/sns_suffix_analytics.json` እና ፖርታል መስተዋቶች አንድ አይነት ናቸው።
ፓነሎች በተገጠመ iframe በኩል ስለዚህ ልምዱ ከውስጣዊው I18NT0000001X ጋር ይዛመዳል
ለምሳሌ

## ማጣሪያዎች እና የውሂብ ምንጮች

- ** ቅጥያ ማጣሪያ *** - የ `sns_registrar_status_total{suffix}` መጠይቆችን ያንቀሳቅሳል
  `.sora`፣ `.nexus` እና `.dao` በተናጥል ሊመረመሩ ይችላሉ።
- ** የጅምላ መልቀቂያ ማጣሪያ *** - የ `sns_bulk_release_payment_*` መለኪያዎችን ይሸፍናል ።
  ፋይናንስ አንድ የተወሰነ የመዝጋቢ ዝርዝር መግለጫን ማስታረቅ ይችላል።
- ** መለኪያዎች *** - ከ Torii (`sns_registrar_status_total` ፣
  `torii_request_duration_seconds`)፣ አሳዳጊ CLI (`guardian_freeze_active`)፣
  `sns_governance_activation_total`፣ እና የጅምላ ቦርዲንግ አጋዥ መለኪያዎች።

## ፓነሎች

1. ** ምዝገባዎች (ያለፉት 24 ሰአታት) *** - የተሳካላቸው የመዝጋቢ ክስተቶች ብዛት
   የተመረጠ ቅጥያ.
2. **የመንግስት ማነቃቂያዎች (30ኛ)** - በቻርተር/በተጨማሪ የተቀረጹ አቤቱታዎች
   CLI
3. ** የመዝጋቢ ጊዜ ብዛት *** - የተሳካላቸው የመዝጋቢ ድርጊቶች በአንድ ቅጥያ መጠን።
4. ** የመዝጋቢ ስህተት ሁነታዎች *** - የ 5 ደቂቃ የስህተት ምልክት
   `sns_registrar_status_total` ቆጣሪዎች.
5. **ጠባቂ የቀዘቀዙ መስኮቶች *** - ቀጥታ መምረጫዎች `guardian_freeze_active`
   ክፍት የማረፊያ ትኬት ዘግቧል።
6. ** የተጣራ የክፍያ አሃዶች በንብረት** - አጠቃላይ ሪፖርት የተደረገ
   `sns_bulk_release_payment_net_units` በንብረት።
7. ** የጅምላ ጥያቄዎች በቅጥያ *** - አንጸባራቂ ጥራዞች በቅጥያ መታወቂያ።
8. ** የተጣራ አሃዶች በጥያቄ** - ከመልቀቁ የተገኘ የ ARPU-style ስሌት
   መለኪያዎች.

## ወርሃዊ የKPI ግምገማ ማረጋገጫ ዝርዝር

የፋይናንስ መሪው በየወሩ የመጀመሪያ ማክሰኞ ተደጋጋሚ ግምገማን ያንቀሳቅሳል፡-

1. የፖርታሉን ** ትንታኔ → SNS KPI ** ገጽ (ወይም Grafana ዳሽቦርድ `sns-kpis`) ይክፈቱ።
2. የመዝጋቢውን የውጤት መጠን እና የገቢ ሠንጠረዦችን ፒዲኤፍ/ሲኤስቪ ወደ ውጪ መላክ።
3. ለ SLA ጥሰቶች ቅጥያዎችን ያወዳድሩ (የስህተት መጠን ሹል፣ የቀዘቀዙ መራጮች>72ሰ፣
   ARPU ዴልታዎች >10%)።
4. የመግቢያ ማጠቃለያዎች + የእርምጃ ንጥሎችን በሚመለከተው አባሪ ግቤት ስር
   `docs/source/sns/regulatory/<suffix>/YYYY-MM.md`.
5. ወደ ውጭ የተላኩትን ዳሽቦርድ ቅርሶች ከአባሪው ቃል ጋር ያያይዙ እና ያገናኙዋቸው
   የምክር ቤቱ አጀንዳ.

ግምገማው የ SLA ጥሰቶችን ካገኘ ለተጎዳው የፔጄርዱቲ ክስተት ፋይል ያድርጉ
ባለቤት (የሬጅስትራር ተረኛ አስተዳዳሪ፣ ሞግዚት በጥሪ ላይ፣ ወይም መጋቢ ፕሮግራም መሪ) እና
ማሻሻያውን በአባሪ መዝገብ ውስጥ ይከታተሉ።