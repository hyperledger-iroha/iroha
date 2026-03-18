---
lang: am
direction: ltr
source: docs/source/ministry/red_team_status.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: f4a2e50e18749f64212dee5186bfd3a3d034ac906d1a506d420cdcb1b5117517
source_last_modified: "2025-12-29T18:16:35.979926+00:00"
translation_last_reviewed: 2026-02-07
title: Ministry Red-Team Status (MINFO-9)
summary: Snapshot of the chaos drill program covering upcoming runs, last completed scenario, and remediation items.
translator: machine-google-reviewed
---

# ሚኒስቴር ቀይ-ቡድን ሁኔታ

ይህ ገጽ [የአወያይ ቀይ ቡድን ዕቅድ](moderation_red_team_plan.md) ያሟላል።
በቅርብ ጊዜ ያለውን የቁፋሮ ቀን መቁጠሪያ፣የማስረጃ ጥቅሎችን እና እርማትን በመከታተል
ሁኔታ. ከእያንዳንዱ ሩጫ በኋላ ከስር ከተያዙት ቅርሶች ጋር ያዘምኑት።
`artifacts/ministry/red-team/<YYYY-MM>/<scenario>/`.

## መጪ ልምምዶች

| ቀን (UTC) | ሁኔታ | ባለቤት(ዎች) | ማስረጃ ዝግጅት | ማስታወሻ |
|--------|--------|
| 2026-11-12 | **ኦፕሬሽን ዓይነ ስውር** — የታይካይ ቅይጥ ሞድ የኮንትሮባንድ ልምምዶች በመግቢያ መንገድ የማውረድ ሙከራዎች | የደህንነት ኢንጂነሪንግ (ሚዩ ሳቶ), ሚኒስቴር ኦፕስ (ሊያም ኦኮኖር) | `scripts/ministry/scaffold_red_team_drill.py` ጥቅል `docs/source/ministry/reports/red_team/2026-11-operation-blindfold.md` + የዝግጅት ማውጫ `artifacts/ministry/red-team/2026-11/operation-blindfold/` | የGAR/Taikai መደራረብ እና የዲ ኤን ኤስ አለመሳካት መልመጃዎች; ከመጀመሩ በፊት denylist Merkle ቅጽበተ ፎቶን ይፈልጋል እና `export_red_team_evidence.py` ዳሽቦርዶች ከተያዙ በኋላ ይሮጣል። |

## የመጨረሻው የመሰርሰሪያ ቅጽበታዊ ገጽ እይታ

| ቀን (UTC) | ሁኔታ | ማስረጃ ቅርቅብ | ማሻሻያ እና ክትትል |
|------------|--------|
| 2026-08-18 | ** Operation SeaGlass *** — የጌት ዌይ ኮንትሮባንድ፣ የአስተዳደር ድጋሚ ጨዋታ እና የማስጠንቀቂያ ብሩክ ልምምዶች | `artifacts/ministry/red-team/2026-08/operation-seaglass/` (Grafana ወደ ውጭ መላክ ፣ የአለርትማኔጅ ምዝግብ ማስታወሻዎች ፣ `seaglass_evidence_manifest.json`) | ** ክፈት: ** እንደገና አጫውት የማኅተም አውቶሜሽን (`MINFO-RT-17`, ባለቤት: የአስተዳደር ኦፕስ, በ 2026-09-05 ምክንያት); የፒን ዳሽቦርድ ወደ SoraFS (`MINFO-RT-18`፣ ታዛቢነት፣ በ2026-08-25 ምክንያት) ቀርቷል። **ዝግ፡** የመመዝገቢያ ደብተር አብነት Norito አንጸባራቂ ሃሾችን ለመያዝ ተዘምኗል። |

## መከታተያ እና መገልገያ

- መርፌን ለማሸግ `scripts/ministry/moderation_payload_tool.py` ይጠቀሙ
  በየሁኔታው የሚጫኑ እና የመካድ ዝርዝሮች።
- ዳሽቦርድ / ምዝግብ ማስታወሻዎችን በ `scripts/ministry/export_red_team_evidence.py` ይቅረጹ
  ከእያንዳንዱ መሰርሰሪያ በኋላ ወዲያውኑ የማስረጃው ሰነድ የተፈረመ ሃሽ ይይዛል።
- CI guard `ci/check_ministry_red_team.sh` የፈፀሙትን የመሰርሰሪያ ሪፖርቶችን ያስፈጽማል
  የቦታ ያዥ ጽሑፍ አልያዘም እና የተጠቀሱ ቅርሶች ከዚህ በፊት አሉ።
  መቀላቀል.

ለተጠቀሰው የቀጥታ ማጠቃለያ `status.md` (§ *የሚኒስቴር ቀይ ቡድን ሁኔታ*) ይመልከቱ
በየሳምንቱ የማስተባበር ጥሪዎች.