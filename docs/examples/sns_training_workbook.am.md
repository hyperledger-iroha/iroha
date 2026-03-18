---
lang: am
direction: ltr
source: docs/examples/sns_training_workbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 7d6965998c392217380a1722e49098f831438e2f4499b9e3258398a66f905a35
source_last_modified: "2025-12-29T18:16:35.080069+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# የኤስኤንኤስ ስልጠና የስራ መጽሐፍ አብነት

ይህንን የሥራ መጽሐፍ ለእያንዳንዱ የሥልጠና ቡድን እንደ ቀኖናዊ የእጅ ጽሑፍ ይጠቀሙ። ተካ
ቦታ ያዢዎች (`<...>`) ለተሰብሳቢዎች ከማሰራጨቱ በፊት።

## የክፍለ-ጊዜ ዝርዝሮች
- ቅጥያ፡ I18NI0000004X
- ዑደት: I18NI0000005X
ቋንቋ: `<ar/es/fr/ja/pt/ru/ur>`
- አስተባባሪ: `<name>`

## ቤተ ሙከራ 1 - KPI ወደ ውጪ መላክ
1. ፖርታል KPI ዳሽቦርድ (`docs/portal/docs/sns/kpi-dashboard.md`) ይክፈቱ።
2. በ `<suffix>` ቅጥያ እና በጊዜ ክልል `<window>` አጣራ።
3. ፒዲኤፍ + CSV ቅጽበተ-ፎቶዎችን ወደ ውጭ ላክ።
4. ወደ ውጭ የተላከውን JSON/PDF SHA-256 እዚህ ይመዝግቡ፡ `______________________`።

## ቤተ ሙከራ 2 - አንጸባራቂ መሰርሰሪያ
1. የናሙና መግለጫውን ከ`artifacts/sns/training/<suffix>/<cycle>/manifests/<lang>.json` ያውጡ።
2. በ`cargo run --bin sns_manifest_check -- --input <file>` ያረጋግጡ።
3. ፈቺ አጽም በ `scripts/sns_zonefile_skeleton.py` ይፍጠሩ።
4. የልዩነት ማጠቃለያውን ለጥፍ፡-
   ```
   <git diff output>
   ```

## ቤተ ሙከራ 3 - ሙግት ማስመሰል
1. ማቀዝቀዝ ለመጀመር ሞግዚት CLI ይጠቀሙ (የጉዳይ መታወቂያ `<case-id>`)።
2. የክርክሩን ሃሽ ይመዝግቡ፡ `______________________`።
3. የማስረጃ መዝገብ ወደ `artifacts/sns/training/<suffix>/<cycle>/logs/` ይስቀሉ።

## ቤተ ሙከራ 4 - አባሪ አውቶሜሽን
1. I18NT0000000X ዳሽቦርድ JSON ወደ ውጭ ይላኩ እና ወደ `artifacts/sns/regulatory/<suffix>/<cycle>/sns_suffix_analytics.json` ይቅዱት።
2. ሩጡ፡-
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
3. የአባሪውን መንገድ + SHA-256 ውፅዓት ይለጥፉ፡ I18NI0000019X።

## የግብረመልስ ማስታወሻዎች
- ግልጽ ያልሆነው ነገር ምንድን ነው?
- በጊዜ ሂደት የሄዱት ቤተ ሙከራዎች የትኞቹ ናቸው?
- የመሳሪያ ስህተቶች ተስተውለዋል?

የተጠናቀቁ የስራ ደብተሮችን ወደ አስተባባሪው ይመልሱ; ስር ናቸው።
`artifacts/sns/training/<suffix>/<cycle>/workbooks/`.