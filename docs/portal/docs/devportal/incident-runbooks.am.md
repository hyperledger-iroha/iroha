---
lang: am
direction: ltr
source: docs/portal/docs/devportal/incident-runbooks.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 8599dbc1a8e4fe846965eed90af128deb5950f83dc61838fea583b326b92a011
source_last_modified: "2025-12-29T18:16:35.104300+00:00"
translation_last_reviewed: 2026-02-07
id: incident-runbooks
title: Incident Runbooks & Rollback Drills
sidebar_label: Incident Runbooks
description: Response guides for failed portal deployments, SoraFS replication degradation, analytics outages, and the quarterly rehearsal cadence required by DOCS-9.
translator: machine-google-reviewed
---

#ዓላማ

የመንገድ ካርታ ንጥል **DOCS-9** ተግባራዊ ሊሆኑ የሚችሉ የመጫወቻ ደብተሮች እና የመልመጃ እቅድ ይጠይቃል
ፖርታል ኦፕሬተሮች ሳይገመቱ ከማጓጓዣ ውድቀቶች ማገገም ይችላሉ። ይህ ማስታወሻ
ሶስት ከፍተኛ የሲግናል ክስተቶችን ይሸፍናል-ያልተሳኩ ማሰማራት፣ ማባዛት።
መበላሸት እና የትንታኔ መቋረጥ - እና ያንን የሩብ አመት ልምምድ መዝግቧል
አረጋግጥ ተለዋጭ ስም እና ሰራሽ ማረጋገጫ አሁንም ከጫፍ እስከ ጫፍ ይሰራሉ።

### ተዛማጅ ቁሳቁስ

- [`devportal/deploy-guide`](./deploy-guide) - ማሸግ ፣ መፈረም እና ተለዋጭ ስም
  የማስተዋወቂያ የስራ ፍሰት.
- [`devportal/observability`](./observability) - የመልቀቂያ መለያዎች ፣ ትንታኔዎች እና
  መመርመሪያዎች ከዚህ በታች ተጠቅሰዋል.
- `docs/source/sorafs_node_client_protocol.md`
  እና [`sorafs/pin-registry-ops`](../sorafs/pin-registry-ops)
  - የመመዝገቢያ ቴሌሜትሪ እና የመጨመር ደረጃዎች.
- `docs/portal/scripts/sorafs-pin-release.sh` እና `npm run probe:*` ረዳቶች
  በሁሉም የማረጋገጫ ዝርዝሮች ውስጥ ተጠቅሷል.

### የተጋራ ቴሌሜትሪ እና መገልገያ

| ምልክት / መሳሪያ | ዓላማ |
| ------------ | ------- |
| `torii_sorafs_replication_sla_total` (ተገናኝቶ/ያመለጡ/በመጠባበቅ ላይ) | የማባዛት ድንኳኖችን እና የ SLA ጥሰቶችን ያውቃል። |
| `torii_sorafs_replication_backlog_total`, `torii_sorafs_replication_completion_latency_epochs` | የመለኪያ ጥልቀት እና የማጠናቀቂያ መዘግየትን ይለካል። |
| `torii_sorafs_gateway_refusals_total`, `torii_sorafs_manifest_submit_total{status="error"}` | ብዙውን ጊዜ መጥፎ ስምሪትን የሚከተሉ የመግቢያ-ጎን ውድቀቶችን ያሳያል። |
| `npm run probe:portal` / `npm run probe:tryit-proxy` | በሩ የሚለቀቅ እና መመለሻዎችን የሚያረጋግጡ ሰራሽ ፍተሻዎች። |
| `npm run check:links` | የተሰበረ-አገናኝ በር; ከእያንዳንዱ ቅነሳ በኋላ ጥቅም ላይ ይውላል. |
| `sorafs_cli manifest submit … --alias-*` (በ I18NI0000028X ተጠቅልሎ) | ተለዋጭ የማስተዋወቂያ/የመቀየር ዘዴ። |
| `Docs Portal Publishing` I18NT0000000X ሰሌዳ (`dashboards/grafana/docs_portal.json`) | ድምር ድምር እምቢታ/ተለዋጭ ስም/TLS/ተባዛ ቴሌሜትሪ። PagerDuty ማንቂያዎች እነዚህን ፓነሎች ለማስረጃ ይጠቅሳሉ። |

## Runbook — ያልተሳካ ማሰማራት ወይም መጥፎ ቅርስ

### ቀስቃሽ ሁኔታዎች

- ቅድመ እይታ/ምርት መመርመሪያዎች አልተሳኩም (`npm run probe:portal -- --expect-release=…`)።
- Grafana ማንቂያዎች በ I18NI0000032X ወይም
  `torii_sorafs_manifest_submit_total{status="error"}` ከታቀደ በኋላ።
- በእጅ QA የተበላሹ መንገዶችን ወይም የፕሮክሲ ፕሮክሲ አለመሳካቶችን ወዲያውኑ ያስተውላል
  ተለዋጭ ስም ማስተዋወቅ.

### ወዲያውኑ መያዣ

1. ** የቀዘቀዙ ማሰማራት፡** የ CI ቧንቧ መስመርን በI18NI0000034X (GitHub) ምልክት ያድርጉበት።
   የስራ ፍሰት ግቤት) ወይም ተጨማሪ ቅርሶች እንዳይወጡ የጄንኪንስ ስራን ለአፍታ ያቁሙ።
2. ** ቅርሶችን ያንሱ፡** ያልተሳካውን ግንብ `build/checksums.sha256` አውርድ፣
   `portal.manifest*.{json,to,bundle,sig}`፣ እና መልሶ መመለስ እንዲችል የፍተሻ ውፅዓት
   የማጣቀሻ ትክክለኛ የምግብ መፍጫዎች.
3. **ለባለድርሻ አካላት ያሳውቁ፡** ማከማቻ SRE፣ ሰነዶች/ዴቭሬል አመራር እና አስተዳደር
   የግንዛቤ ኦፊሰር (በተለይ `docs.sora` ሲነካ)።

### የመመለሻ ሂደት

1. የመጨረሻው-የታወቀ-ጥሩ (LKG) አንጸባራቂን መለየት። የምርት የስራ ፍሰት መደብሮች
   እነሱን በ `artifacts/devportal/<release>/sorafs/portal.manifest.to`.
2. ተለዋጭ ስምውን ወደዚያ አንጸባራቂ በማጓጓዣ ረዳት እንደገና ያያይዙት፡-

```bash
cd docs/portal
./scripts/sorafs-pin-release.sh \
  --build-dir build \
  --artifact-dir artifacts/revert-$(date +%Y%m%d%H%M) \
  --sorafs-dir artifacts/revert-$(date +%Y%m%d%H%M)/sorafs \
  --pin-min-replicas 5 \
  --alias "docs-prod-revert" \
  --alias-namespace "${PIN_ALIAS_NAMESPACE}" \
  --alias-name "${PIN_ALIAS_NAME}" \
  --alias-proof "${PIN_ALIAS_PROOF_PATH}" \
  --torii-url "${TORII_URL}" \
  --submitted-epoch "$(date +%Y%m%d)" \
  --authority "${AUTHORITY}" \
  --private-key "${PRIVATE_KEY}" \
  --skip-submit

# swap in the LKG artefacts before submission
cp /secure/archive/lkg/portal.manifest.to artifacts/.../sorafs/portal.manifest.to
cp /secure/archive/lkg/portal.manifest.bundle.json artifacts/.../sorafs/

cargo run -p sorafs_orchestrator --bin sorafs_cli -- \
  manifest submit \
  --manifest artifacts/.../sorafs/portal.manifest.to \
  --chunk-plan artifacts/.../sorafs/portal.plan.json \
  --torii-url "${TORII_URL}" \
  --authority "${AUTHORITY}" \
  --private-key "${PRIVATE_KEY}" \
  --alias-namespace "${PIN_ALIAS_NAMESPACE}" \
  --alias-name "${PIN_ALIAS_NAME}" \
  --alias-proof "${PIN_ALIAS_PROOF_PATH}" \
  --metadata rollback_from="${FAILED_RELEASE}" \
  --summary-out artifacts/.../sorafs/rollback.submit.json
```

3. የመመለሻ ማጠቃለያውን በክስተቱ ትኬት ውስጥ ከLKG እና ጋር ይመዝግቡ
   ያልተሳካ አንጸባራቂ መፍጨት።

### ማረጋገጫ

1. `npm run probe:portal -- --expect-release=${LKG_TAG}`.
2. `npm run check:links`.
3. `sorafs_cli manifest verify-signature …` እና `sorafs_cli proof verify …`
   (የማሰማራቱን መመሪያ ይመልከቱ) በድጋሚ የተሻሻለው አንጸባራቂ አሁንም መመሳሰሉን ለማረጋገጥ
   በማህደር የተቀመጠው CAR.
4. Try-It staging proxy ተመልሶ መምጣቱን ለማረጋገጥ `npm run probe:tryit-proxy`።

### ከክስተቱ በኋላ

1. የመዘርጋት ቧንቧን እንደገና ማንቃት ዋናው ምክንያት ከተረዳ በኋላ ብቻ ነው።
2. Backfill [`devportal/deploy-guide`](./deploy-guide) "የተማሩ ትምህርቶች"
   ከአዲስ gotchas ጋር ግቤቶች።
3. ለተሳካው የሙከራ ስብስብ (መመርመሪያ፣ አገናኝ አራሚ፣ ወዘተ) ጉድለቶችን ፋይል ያድርጉ።

## Runbook - የማባዛት መበላሸት።

### ቀስቃሽ ሁኔታዎች

- ማንቂያ፡ ` ድምር(torii_sorafs_replication_sla_total{ውጤት=«ተገናኝቶ»}) /
  clamp_min( ድምር(torii_sorafs_replication_sla_total{ውጤት=~"ተገናኝቶ|ያመለጡ"))፣ 1) <
  0.95` ለ 10 ደቂቃዎች
- `torii_sorafs_replication_backlog_total > 10` ለ 10 ደቂቃዎች (ተመልከት
  `pin-registry-ops.md`)።
- አስተዳደር ከተለቀቀ በኋላ ቀርፋፋ ተለዋጭ ስም ሪፖርት ያደርጋል።

### መለያ

1. ለማረጋገጥ [I18NI0000047X](../sorafs/pin-registry-ops) ዳሽቦርዶችን መርምር
   የኋላ መዝገብ ወደ ማከማቻ ክፍል ወይም ለአቅራቢ መርከቦች የተተረጎመ ስለመሆኑ።
2. የ Torii ምዝግብ ማስታወሻዎችን ለ`sorafs_registry::submit_manifest` ማስጠንቀቂዎች ይመልከቱ
   ማስረከብ እራሳቸው አለመሳካታቸውን ይወስኑ።
3. ናሙና ጤና በ`sorafs_cli manifest status --manifest …` (ዝርዝሮች
   በአንድ አቅራቢ የማባዛት ውጤቶች).

### መቀነስ

1. አንጸባራቂውን ከፍ ባለ ብዜት ብዛት (`--pin-min-replicas 7`) በመጠቀም ያውጡ
   `scripts/sorafs-pin-release.sh` ስለዚህ መርሐግብር አውጪው ሸክሙን በትልቁ ላይ ያሰራጫል።
   አቅራቢ ስብስብ. በክስተቱ ምዝግብ ማስታወሻ ውስጥ አዲሱን አንጸባራቂ መግለጫ ይቅረጹ።
2. የኋላ መዝገብ ከአንድ አገልግሎት አቅራቢ ጋር የተቆራኘ ከሆነ፣ ለጊዜው ያሰናክሉት
   የማባዛት መርሐግብር አዘጋጅ (በ`pin-registry-ops.md` ውስጥ የተመዘገበ) እና አዲስ ያስገቡ
   ሌሎች አቅራቢዎች ተለዋጭ ስም እንዲያድሱ ማስገደድ።
3. ተለዋጭ ስም ትኩስነት ከማባዛት እኩልነት የበለጠ ወሳኝ በሚሆንበት ጊዜ፣ እንደገና ያያይዙት።
   ተለዋጭ ስም ሞቅ ያለ መግለጫ አስቀድሞ ተዘጋጅቷል (`docs-preview`) ፣ ከዚያ ያትሙ
   SRE የኋላ መዝገቡን ካጸዳ በኋላ የክትትል አንጸባራቂ።

### ማገገም እና መዝጋት

1. ለማረጋገጥ `torii_sorafs_replication_sla_total{outcome="missed"}` ይቆጣጠሩ
   አምባ መቁጠር.
2. እያንዳንዱ ቅጂ ለመሆኑ የI18NI0000055X ውጤትን ይቅረጹ
   በማክበር መመለስ.
3. ከድህረ ሞት በኋላ የማባዛቱን ሂደት በሚቀጥሉት እርምጃዎች ፋይል ያድርጉ ወይም ያዘምኑ
   (የአቅራቢው ልኬት፣ ቻንከር ማስተካከል፣ ወዘተ)።

## Runbook — ትንታኔ ወይም የቴሌሜትሪ መቋረጥ

### ቀስቃሽ ሁኔታዎች

- `npm run probe:portal` ተሳክቷል ነገር ግን ዳሽቦርዶች መመገብ አቁመዋል
  `AnalyticsTracker` ክስተቶች ለ> 15 ደቂቃዎች።
- የግላዊነት ግምገማ በተጣሉ ክስተቶች ላይ ያልተጠበቀ ጭማሪ ያሳያል።
- `npm run probe:tryit-proxy` በ I18NI0000059X መንገዶች ላይ አልተሳካም።

### ምላሽ

1. የግንባታ ጊዜ ግብዓቶችን ያረጋግጡ: `DOCS_ANALYTICS_ENDPOINT` እና
   `DOCS_ANALYTICS_SAMPLE_RATE` በመጥፋቱ የተለቀቀው ቅርስ (`build/release.json`)።
2. I18NI0000063X በ `DOCS_ANALYTICS_ENDPOINT` በመጠቆም እንደገና ያሂዱ
   መከታተያውን ለማረጋገጥ የዝግጅት ሰብሳቢው አሁንም ጭነቶችን እንደሚያመነጭ ያሳያል።
3. ሰብሳቢዎች ከወደቁ, `DOCS_ANALYTICS_ENDPOINT=""` ያዘጋጁ እና እንደገና ይገንቡ.
   መከታተያ አጭር-ወረዳዎች; በተፈጠረው የጊዜ መስመር ውስጥ የመውጫ መስኮቱን ይመዝግቡ.
4. I18NI0000066X አሁንም የጣት አሻራዎችን `checksums.sha256` ያረጋግጡ
   (የትንታኔ መቋረጥ የጣቢያ ካርታ ማረጋገጫን ማገድ የለበትም)።
5. ሰብሳቢው ካገገመ በኋላ፣ `npm run test:widgets` ን ያሂዱ
   እንደገና ከመታተሙ በፊት የትንታኔ አጋዥ ክፍል ሙከራዎች።

### ከክስተቱ በኋላ

1. ከማንኛውም አዲስ ሰብሳቢ ጋር [`devportal/observability`](./observability) አዘምን
   ገደቦች ወይም ናሙና መስፈርቶች.
2. ማንኛውም የትንታኔ ዳታ ወደ ውጭ ከተጣለ ወይም ከተቀየረ የአስተዳደር ማስታወቂያ
   ፖሊሲ.

## በየሩብ ዓመቱ የመቋቋም ልምምድ

በእያንዳንዱ ሩብ የመጀመሪያ ማክሰኞ ** (ጥር/ኤፕሪል/ጁላይ/ኦክቶበር) ሁለቱንም ልምምዶች ያካሂዱ።
ወይም ማንኛውም ትልቅ የመሠረተ ልማት ለውጥ በኋላ ወዲያውኑ. ቅርሶችን ስር ያከማቹ
`artifacts/devportal/drills/<YYYYMMDD>/`.

| መሰርሰሪያ | እርምጃዎች | ማስረጃ |
| --- | --- | -------- |
| ተለዋጭ መልመጃ | 1. በጣም የቅርብ ጊዜውን የምርት ዝርዝር መግለጫ በመጠቀም የ"ያልተሳካ ማሰማራት" መልሶ ማጫወት።<br/>2. መመርመሪያዎች ካለፉ በኋላ ወደ ምርት እንደገና ያስሩ።<br/>3. `portal.manifest.submit.summary.json` ይቅረጹ እና ምዝግብ ማስታወሻዎችን በመሰርሰሪያው አቃፊ ውስጥ። | `rollback.submit.json`፣ የመመርመሪያ ውፅዓት እና የልምምድ ልቀት መለያ። |
| ሰው ሠራሽ ማረጋገጫ ኦዲት | 1. `npm run probe:portal` እና I18NI0000074Xን ከምርት እና ዝግጅት ጋር ያሂዱ።<br/>2. `npm run check:links` ን ያሂዱ እና `build/link-report.json`ን ያስቀምጡ።<br/>3. የፍተሻ ስኬትን የሚያረጋግጡ የGrafana ፓነሎች ቅጽበታዊ ገጽ እይታዎችን/መላክን ያያይዙ። | የመመርመሪያ ምዝግብ ማስታወሻዎች + I18NI0000077X አንጸባራቂውን የጣት አሻራ በማጣቀስ። |

ያመለጡ ልምምዶችን ለሰነዶች/DevRel ሥራ አስኪያጅ እና ለኤስአርአይ የአስተዳደር ግምገማ ያሳድጉ፣
ፍኖተ ካርታው ቆራጥ፣ የሩብ ዓመት ማስረጃ ስለሚፈልግ ሁለቱም ተለዋጭ ስሞች
የመመለሻ እና የፖርታል ምርመራዎች ጤናማ ሆነው ይቆያሉ።

## ፔጀርዱቲ እና በጥሪ ላይ ማስተባበር

- የፔጄርዱቲ አገልግሎት ** የሰነዶች ፖርታል ህትመት ** የሚመነጩ ማንቂያዎችን በባለቤትነት ይይዛል
  `dashboards/grafana/docs_portal.json`. ደንቦቹ `DocsPortal/GatewayRefusals`፣
  `DocsPortal/AliasCache`፣ እና `DocsPortal/TLSExpiry` የሰነዶች/DevRel ገጽ
  የመጀመሪያ ደረጃ ከማከማቻ SRE ጋር እንደ ሁለተኛ ደረጃ።
- ገጽ ሲደረግ፣ `DOCS_RELEASE_TAG`ን ያካትቱ፣ የተጎዱትን ቅጽበታዊ ገጽ እይታዎች ያያይዙ
  Grafana ፓነሎች፣ እና የአገናኝ መፈተሻ/አገናኝ-ቼክ ውፅዓት ከዚህ በፊት በተከሰቱት ማስታወሻዎች ውስጥ
  ቅነሳ ይጀምራል.
- ከተቀነሰ በኋላ (ተመለስ ወይም እንደገና ከተሰራ) በኋላ `npm run probe:portal` ን እንደገና ያሂዱ ፣
  `npm run check:links`፣ እና አዲስ የI18NT0000004X ቅጽበታዊ ገጽ እይታዎችን ያንሱ መለኪያዎችን
  በገደቦች ውስጥ ተመለስ ። ሁሉንም ማስረጃዎች ከ PagerDuty ክስተት በፊት ያያይዙ
  መፍታት.
- ሁለት ማንቂያዎች በአንድ ጊዜ ከተቃጠሉ (ለምሳሌ TLS ጊዜው ያለፈበት እና የኋላ መዝገብ) ፣ መለያ
  መጀመሪያ እምቢ ማለት (ማተምን አቁም)፣ የመልሶ ማቋቋሚያ ሂደቱን አስፈጽም፣ ከዚያም አጽዳ
  በድልድዩ ላይ የTLS/የኋላ መዝገብ ዕቃዎች ከማከማቻ SRE ጋር።