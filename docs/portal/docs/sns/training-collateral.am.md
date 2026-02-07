---
lang: am
direction: ltr
source: docs/portal/docs/sns/training-collateral.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 76fb66d0b0380ea75c7515d3c9b5f73bafc98481f66f28eed05ab5afd3509abf
source_last_modified: "2025-12-29T18:16:35.177356+00:00"
translation_last_reviewed: 2026-02-07
id: training-collateral
title: SNS Training Collateral
description: Curriculum, localization workflow, and annex evidence capture required by SN-8.
translator: machine-google-reviewed
---

> መስተዋቶች `docs/source/sns/training_collateral.md`. አጭር መግለጫ በሚሰጡበት ጊዜ ይህንን ገጽ ይጠቀሙ
> ሬጅስትራር፣ ዲኤንኤስ፣ አሳዳጊ እና የፋይናንስ ቡድኖች ከእያንዳንዱ ቅጥያ ማስጀመሪያ በፊት።

## 1. የስርአተ ትምህርት ቅጽበታዊ እይታ

| ይከታተሉ | አላማዎች | አስቀድሞ የተነበበ |
|-------|------------|--------|
| ሬጅስትራር ops | መግለጫዎችን ያስገቡ፣ የKPI ዳሽቦርዶችን ይቆጣጠሩ፣ ስህተቶችን ያሳድጉ። | `sns/onboarding-kit`፣ `sns/kpi-dashboard`። |
| ዲ ኤን ኤስ & መግቢያ | ፈቺ አጽሞችን ይተግብሩ፣ በረዶዎችን/መመለሻዎችን ይለማመዱ። | `sorafs/gateway-dns-runbook`፣ ቀጥተኛ ሁነታ የፖሊሲ ናሙናዎች። |
| ጠባቂዎች እና ምክር ቤት | አለመግባባቶችን ያስፈጽም ፣ የአስተዳደር ተጨማሪን ያዘምኑ ፣ ተጨማሪዎችን ይመዝግቡ። | `sns/governance-playbook`፣ የመጋቢ የውጤት ካርዶች። |
| ፋይናንስ እና ትንታኔ | ARPU/ጅምላ መለኪያዎችን ያንሱ፣ አባሪ ቅርቅቦችን ያትሙ። | `finance/settlement-iso-mapping`፣ KPI ዳሽቦርድ JSON |

### የሞዱል ፍሰት

1. **M1 — KPI ዝንባሌ (30 ደቂቃ):** የእግር ቅጥያ ማጣሪያዎች፣ ወደ ውጭ መላክ እና መሸሽ
   የቀዘቀዘ ቆጣሪዎች. ሊደርስ የሚችል፡ ፒዲኤፍ/CSV ቅጽበታዊ ገጽ እይታዎች ከSHA-256 መፍጨት ጋር።
2. **M2 — የህይወት ኡደትን አሳይ (45 ደቂቃ):** የመዝጋቢ መግለጫዎችን ይገንቡ እና ያረጋግጡ፣
   በ `scripts/sns_zonefile_skeleton.py` በኩል የመፍታት አፅሞችን ያመነጫሉ. ሊደርስ የሚችል፡
   git diff አጽም + GAR ማስረጃን ያሳያል።
3. **M3 - የክርክር ልምምድ (40ደቂቃ):** የአሳዳጊ ቅዝቃዜን አስመስሎ + ይግባኝ, ቀረጻ
   ሞግዚት CLI ምዝግብ ማስታወሻዎች ከ `artifacts/sns/training/<suffix>/<cycle>/logs/` በታች።
4. **M4 — አባሪ ቀረጻ (25 ደቂቃ):** ዳሽቦርድ JSON ወደ ውጭ ላክ እና አሂድ፡

   ```bash
   cargo xtask sns-annex \
     --suffix <suffix> \
     --cycle <cycle> \
     --dashboard artifacts/sns/regulatory/<suffix>/<cycle>/sns_suffix_analytics.json \
     --dashboard-artifact artifacts/sns/regulatory/<suffix>/<cycle>/sns_suffix_analytics.json \
     --output docs/source/sns/reports/<suffix>/<cycle>.md \
     --regulatory-entry docs/source/sns/regulatory/<memo>.md \
     --portal-entry docs/portal/docs/sns/regulatory/<memo-id>.md
   ```

   ሊላክ የሚችል፡ የዘመነ አባሪ ማርክዳውን + ሬጉላቶሪ + የፖርታል ማስታወሻ ብሎኮች።

## 2. አካባቢያዊነት የስራ ፍሰት

- ቋንቋዎች፡ I18NI0000009X፣ `es`፣ `fr`፣ `ja`፣ `pt`፣ I18NI000000014X፣ I18NI105000000
- እያንዳንዱ ትርጉም ከምንጩ ፋይል አጠገብ ይኖራል
  (`docs/source/sns/training_collateral.<lang>.md`)። `status` + ያዘምኑ
  `translation_last_reviewed` ከማደስ በኋላ።
- ንብረቶች በአንድ ቋንቋ ስር ናቸው
  `artifacts/sns/training/<suffix>/<lang>/<cycle>/` (ስላይድ/፣ የስራ መጽሐፍት/፣
  ቅጂዎች/፣ መዝገቦች/)።
- እንግሊዝኛውን ካርትዑ በኋላ `python3 scripts/sync_docs_i18n.py --lang <code>` ን ያሂዱ
  ምንጭ ስለዚህ ተርጓሚዎች አዲሱን ሃሽ ያዩታል።

### የመላኪያ ማረጋገጫ ዝርዝር

1. የትርጉም ስቱብ (`status: complete`) አንዴ ከተተረጎመ ያዘምኑ።
2. ስላይዶችን ወደ ፒዲኤፍ ይላኩ እና ወደ የቋንቋ I18NI0000022X ማውጫ ይስቀሉ።
3. የ ≤10 ደቂቃ KPI የእግር ጉዞን ይመዝግቡ; ከቋንቋ ማገዶ.
4. የፋይል አስተዳደር ትኬት `sns-training` ስላይድ/የሥራ ደብተር የያዘ መለያ
   መፈጨት፣ አገናኞችን መቅዳት እና ማስረጃን ማያያዝ።

## 3. የስልጠና ንብረቶች

- የስላይድ ዝርዝር፡ `docs/examples/sns_training_template.md`።
- የስራ መጽሐፍ አብነት፡- I18NI0000025X (በአንድ ተሳታፊ አንድ)።
- ግብዣ + አስታዋሾች: `docs/examples/sns_training_invite_email.md`.
- የግምገማ ቅጽ: I18NI0000027X (ምላሾች
  በI18NI0000028X ስር ተቀምጧል።

## 4. መርሐግብር እና መለኪያዎች

| ዑደት | መስኮት | መለኪያዎች | ማስታወሻ |
|-------|--------|-----|------|
| 2026-03 | ለጥፍ KPI ግምገማ | መገኘት %፣ አባሪ ገብቷል | `.sora` + `.nexus` ስብስቦች |
| 2026-06 | ቅድመ `.dao` GA | የፋይናንስ ዝግጁነት ≥90% | የመመሪያ እድሳትን ያካትቱ |
| 2026-09 | መስፋፋት | የክርክር መሰርሰሪያ <20min, SLA ≤2 ቀናት | ከSN-7 ማበረታቻዎች ጋር አሰልፍ |

በI18NI0000032X ውስጥ የማይታወቅ ግብረ መልስ ያንሱ
ስለዚህ ተከታይ ቡድኖች አካባቢያዊነትን እና ቤተ ሙከራዎችን ማሻሻል ይችላሉ።