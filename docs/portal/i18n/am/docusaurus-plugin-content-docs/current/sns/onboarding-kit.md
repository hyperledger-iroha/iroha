---
lang: am
direction: ltr
source: docs/portal/docs/sns/onboarding-kit.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: SNS metrics & onboarding kit
description: Dashboard, pricing, and automation artifacts referenced by roadmap item SN-8.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# የኤስኤንኤስ መለኪያዎች እና የመሳፈሪያ መሣሪያ

የመንገድ ካርታ ንጥል **SN-8** ሁለት ቃል ኪዳኖችን ያጠቃልላል

1. ምዝገባዎችን፣ እድሳትን፣ ARPUን፣ አለመግባባቶችን እና የሚያሳዩ ዳሽቦርዶችን ያትሙ
   መስኮቶችን ለ`.sora`፣ I18NI0000018X፣ እና `.dao` ያቀዘቅዙ።
2. መዝጋቢዎች እና መጋቢዎች ዲ ኤን ኤስን፣ ዋጋን እና ዋጋን ማገናኘት እንዲችሉ የመሳፈሪያ ኪት ይላኩ።
   ማንኛውም ቅጥያ በቀጥታ ከመለቀቁ በፊት ያለማቋረጥ ኤፒአይዎች።

ይህ ገጽ የምንጭ ሥሪትን ያንጸባርቃል
[`docs/source/sns/onboarding_kit.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sns/onboarding_kit.md)
ስለዚህ የውጭ ገምጋሚዎች ተመሳሳይ አሰራርን ሊከተሉ ይችላሉ.

## 1. ሜትሪክ ጥቅል

### Grafana ዳሽቦርድ እና ፖርታል መክተት

- `dashboards/grafana/sns_suffix_analytics.json` ወደ I18NT0000002X (ወይም ሌላ አስመጣ)
  የትንታኔ አስተናጋጅ) በመደበኛ ኤፒአይ በኩል፡-

```bash
curl -H "Content-Type: application/json" \
     -H "Authorization: Bearer ${GRAFANA_TOKEN}" \
     -X POST https://grafana.sora.net/api/dashboards/db \
     --data-binary @dashboards/grafana/sns_suffix_analytics.json
```

- ተመሳሳዩ JSON የዚህን ፖርታል ገጽ iframe ያበረታታል ( ** SNS KPI Dashboard ይመልከቱ ** ይመልከቱ
  ዳሽቦርዱን ባጋጨህ ቁጥር ሩጥ
  `npm run build && npm run serve-verified-preview` በ `docs/portal` ወደ
  ሁለቱንም Grafana እና የተከተተውን ቆይታ በማመሳሰል ያረጋግጡ።

### ፓነሎች እና ማስረጃዎች

| ፓነል | መለኪያዎች | የአስተዳደር ማስረጃ |
|-------|--------|
| ምዝገባዎች እና እድሳት | `sns_registrar_status_total` (ስኬት + እድሳት ፈቺ መለያዎች) | በአንድ-ቅጥያ ልቀት + SLA መከታተያ. |
| ARPU / የተጣራ አሃዶች | `sns_bulk_release_payment_net_units`, `sns_bulk_release_payment_gross_units` | ፋይናንስ የመዝጋቢ መግለጫዎችን ከገቢው ጋር ማዛመድ ይችላል። |
| ውዝግቦች እና ውዝግቦች | `guardian_freeze_active`፣ `sns_dispute_outcome_total`፣ `sns_governance_activation_total` | ንቁ ቅዝቃዜዎችን፣ የግልግል ዳኝነትን እና የአሳዳጊ የስራ ጫናን ያሳያል። |
| SLA / ስህተት ተመኖች | `torii_request_duration_seconds`, `sns_registrar_status_total{status="error"}` | የ API regressions ደንበኞችን ከመነካታቸው በፊት ያደምቃል። |
| የጅምላ አንጸባራቂ መከታተያ | `sns_bulk_release_manifest_total`፣ የክፍያ መለኪያዎች ከ I18NI0000033X መለያዎች ጋር | የCSV ጠብታዎችን ወደ የሰፈራ ትኬቶች ያገናኛል። |

በወርሃዊው KPI ከGrafana (ወይም የተከተተው iframe) ፒዲኤፍ/CSV ወደ ውጪ ላክ
ይገምግሙ እና ከስር ከሚመለከተው አባሪ ጋር አያይዘው።
`docs/source/sns/regulatory/<suffix>/YYYY-MM.md`. መጋቢዎች SHA-256ን ይይዛሉ
ወደ ውጭ የተላከው ጥቅል በ `docs/source/sns/reports/` (ለምሳሌ ፣
`steward_scorecard_2026q1.md`) ስለዚህ ኦዲቶች የማስረጃ መንገዱን እንደገና ማጫወት ይችላሉ።

### አባሪ አውቶማቲክ

ገምጋሚዎች ሀ ለማግኘት በቀጥታ ከዳሽቦርድ ወደ ውጭ መላኪያ አባሪ ፋይሎችን ይፍጠሩ
ወጥነት ያለው የምግብ መፈጨት;

```bash
cargo xtask sns-annex \
  --suffix .sora \
  --cycle 2026-03 \
  --dashboard dashboards/grafana/sns_suffix_analytics.json \
  --dashboard-artifact artifacts/sns/regulatory/.sora/2026-03/sns_suffix_analytics.json \
  --output docs/source/sns/reports/.sora/2026-03.md \
  --regulatory-entry docs/source/sns/regulatory/eu-dsa/2026-03.md \
  --portal-entry docs/portal/docs/sns/regulatory/eu-dsa-2026-03.md
```

- ረዳቱ ወደ ውጭ መላኩን ያጭዳል፣ የ UID/መለያዎች/የፓነል ብዛት ይይዛል እና ሀ ይጽፋል
  የማርክ ዳውን አባሪ በI18NI0000037X (ይመልከቱ
  `.sora/2026-03` ናሙና ከዚህ ሰነድ ጋር ተፈፅሟል)።
- `--dashboard-artifact` ወደ ውጭ መላክ ይገለበጣል
  `artifacts/sns/regulatory/<suffix>/<cycle>/` ስለዚህ አባሪው የሚያመለክተው
  ቀኖናዊ ማስረጃ መንገድ; መጠቆም ሲፈልጉ ብቻ `--dashboard-label` ይጠቀሙ
  ከባንዱ ውጪ በሆነ መዝገብ ቤት።
- በአስተዳደር ማስታወሻ ላይ `--regulatory-entry` ነጥቦች። ረዳቱ ያስገባል (ወይም
  የሚተካ) የ I18NI0000043X ብሎክ የአባሪውን መንገድ፣ ዳሽቦርድ ይመዘግባል
  artefact፣ መፍጨት እና የጊዜ ማህተም ስለዚህ ማስረጃው በድጋሚ ከሮጠ በኋላ እንደተመሳሰለ ይቆያል።
- `--portal-entry` I18NT0000000X ቅጂን ያቆያል (`docs/portal/docs/sns/regulatory/*.md`)
  ተሰልፏል ስለዚህ ገምጋሚዎች የተለያዩ አባሪ ማጠቃለያዎችን በእጅ መለየት የለባቸውም።
- `--regulatory-entry`/I18NI0000047X ከዘለሉ የመነጨውን ፋይል ያያይዙ
  ማስታወሻዎቹ በእጅ እና አሁንም ከGrafana የተነሱ የፒዲኤፍ/CSV ቅጽበተ-ፎቶዎችን ይስቀሉ።
- ለተደጋጋሚ ወደ ውጭ መላክ፣ ቅጥያ/ዑደት ጥንዶችን ይዘርዝሩ
  `docs/source/sns/regulatory/annex_jobs.json` እና አሂድ
  `python3 scripts/run_sns_annex_jobs.py --verbose`. ረዳቱ በእያንዳንዱ መግቢያ ላይ ይሄዳል ፣
  ዳሽቦርዱን ወደ ውጭ መላክ ይገለበጣል (ወደ `dashboards/grafana/sns_suffix_analytics.json` ነባሪ
  ሳይገለጽ) እና በእያንዳንዱ የቁጥጥር ስር ያለውን አባሪ ብሎክ ያድሳል (እና፣
  ሲገኝ፣ ፖርታል) ማስታወሻ በአንድ ማለፊያ።
- የስራ ዝርዝሩ ተስተካክሎ/ ተቀናሽ ሆኖ መቆየቱን ለማረጋገጥ `python3 scripts/check_sns_annex_schedule.py --jobs docs/source/sns/regulatory/annex_jobs.json --regulatory-root docs/source/sns/regulatory --report-root docs/source/sns/reports` (ወይም `make check-sns-annex`) ያሂዱ፣ እያንዳንዱ ማስታወሻ የሚዛመደውን I18NI0000053X ማርከር ይይዛል፣ እና አባሪው ግንድ አለ። ረዳቱ በአስተዳደር እሽጎች ውስጥ ከሚጠቀሙት የአካባቢ/ሃሽ ማጠቃለያዎች ጎን I18NI0000054X ይጽፋል።
ይህ በእጅ የመገልበጥ/የመለጠፍ ደረጃዎችን ያስወግዳል እና የ SN-8 አባሪ ማስረጃ በሚቆይበት ጊዜ እንዲቆይ ያደርገዋል
በCI ውስጥ የጥበቃ መርሐግብር፣ ማርከር እና የትርጉም ጉዞ።

## 2. የቦርዲንግ ኪት ክፍሎች

### ቅጥያ ሽቦ

- የመመዝገቢያ ንድፍ + መራጭ ህጎች:
  [`docs/source/sns/registry_schema.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sns/registry_schema.md)
  እና [`docs/source/sns/local_to_global_toolkit.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sns/local_to_global_toolkit.md)።
- ዲ ኤን ኤስ አጽም ረዳት;
  [`scripts/sns_zonefile_skeleton.py`](https://github.com/hyperledger-iroha/iroha/blob/master/scripts/sns_zonefile_skeleton.py)
  በ ውስጥ ከተያዘው የልምምድ ፍሰት ጋር
  [ጌትዌይ/ዲኤንኤስ runbook](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs_gateway_dns_owner_runbook.md)።
- ለእያንዳንዱ ሬጅስትራር ማስጀመሪያ ከታች አጭር ማስታወሻ ያስገቡ
  `docs/source/sns/reports/` የመምረጫ ናሙናዎችን፣ GAR ማረጋገጫዎችን እና የዲ ኤን ኤስ ሃሾችን ማጠቃለል።

### የዋጋ ማጭበርበር

| የመለያ ርዝመት | ቤዝ ክፍያ (USD equiv) |
|-------------|
| 3 | 240 ዶላር |
| 4 | $90 |
| 5 | $30 |
| 6–9 | $ 12 |
| 10+ | $8 |

ድህረ-ቅጥያዎች፡- `.sora` = 1.0×፣ `.nexus` = 0.8×፣ `.dao` = 1.3×።  
የጊዜ ማባዣዎች፡ 2-አመት -5%፣ 5-አመት -12%; ጸጋ መስኮት = 30 ቀናት, ቤዛ
= 60 ቀናት (20% ክፍያ፣ ደቂቃ $5፣ ከፍተኛው $200)። በ ውስጥ የተደራደሩ ልዩነቶችን ይመዝግቡ
የመመዝገቢያ ትኬት.

### ፕሪሚየም ጨረታዎች ከእድሳት ጋር

1. ** ፕሪሚየም ገንዳ *** - የታሸገ-ጨረታ ቁርጠኝነት/መገለጥ (SN-3)። ጋር ጨረታዎችን ይከታተሉ
   `sns_premium_commit_total`፣ እና አንጸባራቂውን በስር ያትሙ
   `docs/source/sns/reports/`.
2. **ደች እንደገና ተከፈተ** — ከጸጋ + መቤዠት ካለቀ በኋላ የ7 ቀን የሆላንድ ሽያጭ ይጀምሩ
   በቀን 15% የሚበሰብስ በ 10 ×. መለያው በI18NI0000064X ይገለጻል።
   ዳሽቦርድ ወደላይ መሻሻል ይችላል።
3. ** እድሳት *** - ክትትል I18NI0000065X እና
   የራስ-አድሶ ማረጋገጫ ዝርዝሩን ይያዙ (ማሳወቂያዎች ፣ SLA ፣ የመመለሻ ክፍያ ሀዲዶች)
   በመዝጋቢ ትኬት ውስጥ።

### የገንቢ ኤፒአይዎች እና አውቶሜሽን

- API ኮንትራቶች፡ [I18NI0000066X](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sns/registrar_api.md)።
- የጅምላ ረዳት እና የCSV እቅድ፡
  [`docs/source/sns/bulk_onboarding_toolkit.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sns/bulk_onboarding_toolkit.md)።
- ምሳሌ ትዕዛዝ:

```bash
python3 scripts/sns_bulk_onboard.py registrations.csv \
  --ndjson artifacts/sns/releases/2026q2/requests.ndjson \
  --submission-log artifacts/sns/releases/2026q2/submissions.log \
  --submit-torii-url https://torii.sora.net \
  --submit-token-file ~/.config/sora/tokens/registrar.token
```

የሰነድ መታወቂያውን (`--submission-log` ውፅዓት) በKPI ዳሽቦርድ ማጣሪያ ውስጥ ያካትቱ
ስለዚህ ፋይናንስ በየልቀት የገቢ ፓነሎችን ማስታረቅ ይችላል።

### የማስረጃ ጥቅል

1. የመመዝገቢያ ትኬት ከእውቂያዎች፣ ከቅጥያ ወሰን እና ከክፍያ ሀዲዶች ጋር።
2. የዲ ኤን ኤስ / የመፍታት ማስረጃ (የዞንፋይል አፅሞች + GAR ማረጋገጫዎች).
3. የዋጋ አሰጣጥ ሉህ + በአስተዳደር የጸደቀ ማናቸውንም መሻር።
4. API/CLI የጭስ-ሙከራ ቅርሶች (`curl` ናሙናዎች፣ የCLI ግልባጮች)።
5. የKPI ዳሽቦርድ ቅጽበታዊ ገጽ እይታ + CSV ወደ ውጪ መላክ፣ ከወርሃዊው አባሪ ጋር ተያይዟል።

## 3. የማረጋገጫ ዝርዝርን አስጀምር

| ደረጃ | ባለቤት | Artefact |
|-------------|-------|
| ዳሽቦርድ ገብቷል | የምርት ትንታኔ | Grafana API ምላሽ + ዳሽቦርድ UID |
| ፖርታል መክተቱ ተረጋግጧል | ሰነዶች/DevRel | `npm run build` መዝገቦች + ቅድመ እይታ ቅጽበታዊ ገጽ እይታ |
| የዲኤንኤስ ልምምድ ተጠናቅቋል | አውታረ መረብ / ኦፕስ | `sns_zonefile_skeleton.py` ውጤቶች + runbook መዝገብ |
| ሬጅስትራር አውቶሜሽን ደረቅ ሩጫ | ሬጅስትራር ኢንጅነር | `sns_bulk_onboard.py` ማስገቢያ መዝገብ |
| የመንግስት ማስረጃ ቀረበ | አስተዳደር ምክር ቤት | አባሪ አገናኝ + SHA-256 ወደ ውጭ የተላከ ዳሽቦርድ |

መዝጋቢ ወይም ቅጥያ ከማንቃትዎ በፊት የማረጋገጫ ዝርዝሩን ይሙሉ። የተፈረመው
ጥቅል የ SN-8 የመንገድ ካርታ በርን ያጸዳል እና መቼ ኦዲተሮችን አንድ ማጣቀሻ ይሰጣል
የገበያ ቦታዎችን መገምገም.