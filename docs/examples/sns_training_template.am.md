---
lang: am
direction: ltr
source: docs/examples/sns_training_template.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: dd9da5045f5f40dbc31837145ad13bf79b4d751b0803c0b6d69bab49885ed1b4
source_last_modified: "2025-12-29T18:16:35.079313+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# የኤስኤንኤስ ስልጠና ስላይድ አብነት

ይህ የማርክታውን ንድፍ አመቻቾች መላመድ ያለባቸውን ስላይዶች ያንጸባርቃል
የቋንቋ ጓዶቻቸው። እነዚህን ክፍሎች ወደ Keynote/PowerPoint/Google ይቅዱ
እንደ አስፈላጊነቱ የነጥብ ነጥቦቹን ፣ ቅጽበታዊ ገጽ እይታዎችን እና ሥዕላዊ መግለጫዎችን ያንሸራትቱ እና አካባቢያዊ ያድርጓቸው።

## ርዕስ ስላይድ
- ፕሮግራም: "የሶራ ስም አገልግሎት በመሳፈር ላይ"
- የትርጉም ጽሑፍ፡ ቅጥያ + ዑደት ይግለጹ (ለምሳሌ፡ I18NI0000002X)
- አቅራቢዎች + ትስስር

## የ KPI አቀማመጥ
- የ `docs/portal/docs/sns/kpi-dashboard.md` ቅጽበታዊ ገጽ እይታ ወይም መክተት
- የቅጥያ ማጣሪያዎችን የሚያብራራ የነጥብ ዝርዝር ፣ የ ARPU ጠረጴዛ ፣ የቀዘቀዘ መከታተያ
- ፒዲኤፍ/CSV ወደ ውጭ ለመላክ ጥሪዎች

## የህይወት ዑደትን ያሳያል
- ሥዕላዊ መግለጫ: ሬጅስትራር → Torii → አስተዳደር → ዲ ኤን ኤስ / ጌትዌይ
- `docs/source/sns/registry_schema.md` የሚያመለክቱ ደረጃዎች
- ምሳሌ አንጸባራቂ ማብራሪያ ከ ማብራሪያዎች ጋር

## ሙግት እና ልምምዶችን ያቀዘቅዙ
- ለአሳዳጊ ጣልቃገብነት ፍሰት ንድፍ
- የማረጋገጫ ዝርዝር `docs/source/sns/governance_playbook.md`
- ምሳሌ የቀዘቀዙ ቲኬቶች የጊዜ መስመር

## አባሪ ቀረጻ
- የትእዛዝ ቅንጣቢ `cargo xtask sns-annex ... --portal-entry ...` ያሳያል
- Grafana JSON በ `artifacts/sns/regulatory/<suffix>/<cycle>/` ስር ለማስቀመጥ አስታዋሽ
- ወደ `docs/source/sns/reports/.<suffix>/<cycle>.md` አገናኝ

## ቀጣይ እርምጃዎች
- የስልጠና ግብረ-መልስ አገናኝ (`docs/examples/sns_training_eval_template.md` ይመልከቱ)
- Slack/Matrix ሰርጥ መያዣዎች
- መጪ ወሳኝ ቀናት