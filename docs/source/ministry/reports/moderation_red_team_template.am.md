---
lang: am
direction: ltr
source: docs/source/ministry/reports/moderation_red_team_template.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 22bfdf5696bf3a58e7899e7d7b2ba77e404a05fa81304f12d6c78eeb1e8035e5
source_last_modified: "2025-12-29T18:16:35.982646+00:00"
translation_last_reviewed: 2026-02-07
title: Red-Team Drill Report Template
summary: Copy this file for every MINFO-9 drill to capture metadata, evidence, and remediation actions.
translator: machine-google-reviewed
---

> ** እንዴት መጠቀም እንደሚቻል: ** ይህን አብነት ከእያንዳንዱ መሰርሰሪያ በኋላ ወዲያውኑ ወደ `docs/source/ministry/reports/<YYYY-MM>-mod-red-team-<scenario>.md` ያባዙት። የፋይል ስሞችን ንዑስ ሆሄያት፣ ሰረዞች እና አስተካክል በአለርትማኔጀር ከተመዘገበው መሰርሰሪያ መታወቂያ ጋር አስተካክል።

# የቀይ ቡድን ቁፋሮ ሪፖርት - `<SCENARIO NAME>`

- ** የመሰርሰሪያ መታወቂያ: *** `<YYYYMMDD>-<scenario>`
- ** ቀን እና መስኮት: *** `<YYYY-MM-DD HH:MMZ – HH:MMZ>`
- ** ትዕይንት ክፍል: ** `smuggling | bribery | gateway | ...`
- ** ኦፕሬተሮች: *** `<names / handles>`
- ** ዳሽቦርዶች ከቁርጠኝነት ታግደዋል፡** `<git SHA>`
- ** የማስረጃ ጥቅል:** `artifacts/ministry/red-team/<YYYY-MM>/<scenario>/`
- **SoraFS CID (አማራጭ):** `<cid>`  
- ** ተዛማጅ የመንገድ ካርታ እቃዎች:** `MINFO-9`፣ እንዲሁም ማንኛውም የተገናኙ ቲኬቶች።

## 1. ዓላማዎች እና የመግቢያ ሁኔታዎች

- ** ዋና ዓላማዎች ***
  - `<e.g. Verify denylist TTL enforcement under smuggling attack>`
- ** ቅድመ-ሁኔታዎች ተረጋግጠዋል ***
  - `emergency_canon_policy.md` ስሪት `<tag>`
  - `dashboards/grafana/ministry_moderation_overview.json` መፍጨት `<sha256>`
  - በጥሪ ላይ ባለስልጣንን ይሽሩ፡ `<name>`

## 2. የአፈፃፀም የጊዜ መስመር

| የጊዜ ማህተም (UTC) | ተዋናይ | ድርጊት / ትዕዛዝ | ውጤት / ማስታወሻዎች |
|-------------|
|  |  |  |  |

> የTorii የጥያቄ መታወቂያዎችን፣ ቸንክ ሃሾችን፣ ማጽደቆችን መሻር እና የማስጠንቀቂያ አስተዳዳሪ ማገናኛን ያካትቱ።

## 3. ምልከታዎች እና መለኪያዎች

| መለኪያ | ዒላማ | ተስተውሏል | ማለፍ/ውድቀት | ማስታወሻ |
|--------|--------|----------|-----------|---|
| የማንቂያ ምላሽ መዘግየት | `<X> min` | `<Y> min` | ✅/⚠️ |  |
| ልከኛ ማወቂያ መጠን | `>= <value>` |  |  |  |
| የጌትዌይ anomaly ማወቂያ | `Alert fired` |  |  |  |

- `Grafana export:` `artifacts/.../dashboards/ministry_moderation_overview.json`
- `Alert bundle:` `artifacts/.../alerts/ministry_moderation_rules.yml`
- `Norito manifests:` `<path>`

## 4. ግኝቶች እና ማሻሻያዎች

| ከባድነት | ማግኘት | ባለቤት | የዒላማ ቀን | ሁኔታ / አገናኝ |
|-------|--------|--------|------------|
| ከፍተኛ |  |  |  |  |

መለካት እንዴት እንደሚገለጥ፣የመካድ ፖሊሲዎች ወይም ኤስዲኬ/መሳሪያ መቀየር እንዳለበት መመዝገብ። ከ GitHub/Jira ጉዳዮች ጋር አገናኝ እና የታገዱ/ያልታገዱ ግዛቶች ማስታወሻ።

## 5. አስተዳደር እና ማፅደቂያዎች

- ** የክስተት አዛዥ ማቋረጥ: ** `<name / timestamp>`
- **የመንግስት ምክር ቤት ግምገማ ቀን:** `<meeting id>`
- ** የክትትል ማረጋገጫ ዝርዝር፡** `[ ] status.md updated`፣ `[ ] roadmap row updated`፣ `[ ] transparency packet annotated`

## 6. ማያያዣዎች

- `[ ] CLI logbook (`logs/.md`)`
- `[ ] Dashboard JSON export`
- `[ ] Alertmanager history`
- `[ ] SoraFS manifest / CAR`
- `[ ] Override audit log`

እያንዳንዱን ዓባሪ በ`[x]` አንድ ጊዜ ወደ የማስረጃ ጥቅል ከተሰቀለ እና SoraFS ቅጽበታዊ ፎቶ ጋር ምልክት ያድርጉ።

---

_መጨረሻ የዘመነው፡ {{ቀን | ነባሪ ("2026-02-20") }}_