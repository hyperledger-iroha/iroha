---
lang: am
direction: ltr
source: docs/portal/docs/sorafs/taikai-monitoring-dashboards.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: f5f8db3cc2a4255f29c4a196c14f7c14dcdf9019a0dc6e6a55ba4a9037815e58
source_last_modified: "2025-12-29T18:16:35.205443+00:00"
translation_last_reviewed: 2026-02-07
title: Taikai Monitoring Dashboards
description: Portal summary of the viewer/cache Grafana boards that back SN13-C evidence
translator: machine-google-reviewed
---

የታይካይ ማዞሪያ-ማኒፌስት (TRM) ዝግጁነት በሁለት I18NT0000001X ሰሌዳዎች እና በነሱ ላይ ተጣብቋል።
ተጓዳኝ ማንቂያዎች. This page mirrors the highlights from
`dashboards/grafana/taikai_viewer.json`, `dashboards/grafana/taikai_cache.json`,
እና `dashboards/alerts/taikai_viewer_rules.yml` ስለዚህ ገምጋሚዎች አብረው ይከተላሉ
without cloning the repository.

## Viewer dashboard (`taikai_viewer.json`)

- ** የቀጥታ ጠርዝ እና መዘግየት፡** ፓነሎች የ p95/p99 መዘግየት ሂስቶግራሞችን ይሳሉ።
  (`taikai_ingest_segment_latency_ms`, `taikai_ingest_live_edge_drift_ms`) per
  ክላስተር / ዥረት. ለp99>900ms ወይም ተንሸራታች>1.5s ይመልከቱ (ያነሳሳል።
  `TaikaiLiveEdgeDrift` ማንቂያ)።
- **የክፍል ስህተቶች፡** `taikai_ingest_segment_errors_total{reason}` ወደ
  ውድቀቶችን መፍታት፣ የዘር ተደጋጋሚ ሙከራዎችን ወይም አለመዛመዶችን ማጋለጥ።
  ይህ ፓነል በ ላይ በተነሳ ቁጥር ቅጽበታዊ ገጽ እይታዎችን ከ SN13-C ክስተቶች ጋር ያያይዙ
  "ማስጠንቀቂያ" ባንድ.
- ** የተመልካች እና CEK ጤና፡** ፓነሎች ከ `taikai_viewer_*` ሜትሪክስ ትራክ የተገኙ ፓነሎች
  የCEK ሽክርክር ዕድሜ፣ የPQ ጠባቂ ቅይጥ፣ የማቋቋሚያ ቆጠራዎች እና የማንቂያ ጥቅሎች። CEK
  ፓነል አዲስ ከማጽደቁ በፊት አስተዳደር የሚገመገሙትን ሽክርክር SLA ያስፈጽማል
  ተለዋጭ ስሞች
- ** ተለዋጭ ስም ቴሌሜትሪ ቅጽበተ-ፎቶ:** `/status → telemetry.taikai_alias_rotations`
  ጠረጴዚው በቀጥታ በቦርዱ ላይ ተቀምጧል ስለዚህ ኦፕሬተሮች አንጸባራቂ መፈጨትን ማረጋገጥ ይችላሉ።
  የአስተዳደር ማስረጃዎችን ከማያያዝ በፊት.

## መሸጎጫ ዳሽቦርድ (`taikai_cache.json`)

- ** የእርከን ግፊት: ** የፓነሎች ገበታ `sorafs_taikai_cache_{hot,warm,cold}_occupancy`
  እና `sorafs_taikai_cache_promotions_total`. TRM መሆኑን ለማየት እነዚህን ይጠቀሙ
  ማሽከርከር የተወሰኑ ደረጃዎችን ከመጠን በላይ መጫን ነው።
- ** QoS ውድቀቶች:** `sorafs_taikai_qos_denied_total` መሸጎጫ ግፊት በሚኖርበት ጊዜ ቦታዎች
  ስሮትልንግ ኃይሎች; ታሪፉ ከዜሮ በሚነሳበት ጊዜ ሁሉ የመሰርሰሪያ ምዝግብ ማስታወሻውን ያብራሩ።
- ** የEgress አጠቃቀም፡** የSoraFS መውጫዎች ከታይካይ ጋር መገናኘታቸውን ለማረጋገጥ ይረዳል
  የCMAF መስኮቶች ሲሽከረከሩ ተመልካቾች።

## ማንቂያዎች እና ማስረጃ ቀረጻ

- የፔጃጅ ህጎች በ `dashboards/alerts/taikai_viewer_rules.yml` እና በካርታ አንድ ይኖራሉ
  ከላይ ካሉት ፓነሎች ጋር ወደ አንዱ (`TaikaiLiveEdgeDrift`፣ `TaikaiIngestFailure`፣
  `TaikaiCekRotationLag`፣ የማረጋገጫ-ጤና ማስጠንቀቂያዎች)። እያንዳንዱን ምርት ያረጋግጡ
  የክላስተር ገመዶች እነዚህን ወደ Alertmanager.
- በልምምድ ወቅት የተነሱ ቅጽበታዊ ገጽ እይታዎች መቀመጥ አለባቸው
  `artifacts/sorafs_taikai/drills/<YYYYMMDD>/` ከ spool ፋይሎች ጋር እና
  `/status` JSON. `scripts/telemetry/log_sorafs_drill.sh --scenario taikai-anchor` ይጠቀሙ
  አፈፃፀሙን በጋራ መሰርሰሪያ መዝገብ ላይ ለማያያዝ።
- ዳሽቦርዶች ሲቀየሩ፣ የJSON ፋይልን SHA-256 ዳይጀስት በ ውስጥ ያካትቱ
  የፖርታል PR መግለጫ ኦዲተሮች የሚተዳደረውን Grafana አቃፊን ከ
  repo ስሪት.

## የማስረጃ ጥቅል ማረጋገጫ ዝርዝር

የ SN13-C ግምገማዎች እያንዳንዱ መሰርሰሪያ ወይም ክስተት የተዘረዘሩትን ተመሳሳይ ቅርሶች ለመላክ ይጠብቃሉ።
በታይካይ መልህቅ runbook ውስጥ። ጥቅሉ እንዲሆን ከታች ባለው ቅደም ተከተል ያዙዋቸው
ለአስተዳደር ግምገማ ዝግጁ

1. በጣም የቅርብ ጊዜውን `taikai-anchor-request-*.json` ይቅዱ፣
   `taikai-trm-state-*.json`፣ እና `taikai-lineage-*.json` ፋይሎች ከ
   `config.da_ingest.manifest_store_dir/taikai/`. እነዚህ spool artefacts ያረጋግጣል
   የትኛው የማዞሪያ አንጸባራቂ (TRM) እና የዘር መስኮት ንቁ ነበሩ። ረዳቱ
   `cargo xtask taikai-anchor-bundle --spool <dir> --copy-dir <out> --out <out>/anchor_bundle.json [--signing-key <ed25519>]`
   የስፑል ፋይሎችን ይገለብጣል፣ ሃሽ ይለቃል እና እንደ አማራጭ ማጠቃለያውን ይፈርማል።
2. መዝገብ I18NI0000031X ውፅዓት ተጣርቶ
   `.telemetry.taikai_alias_rotations[]` እና ከስፑል ፋይሎች አጠገብ ያከማቹ።
   ገምጋሚዎች የተዘገበው I18NI0000033X እና የመስኮት ወሰኖችን ያወዳድራሉ
   የተቀዳው የስፑል ሁኔታ.
3. ከላይ ለተዘረዘሩት መለኪያዎች I18NT0000000X ቅጽበተ-ፎቶዎችን ወደ ውጭ ይላኩ እና ቅጽበታዊ ገጽ እይታዎችን ያንሱ
   የተመልካች/መሸጎጫ ዳሽቦርዶች ከሚመለከታቸው ክላስተር/ዥረት ማጣሪያዎች ጋር
   እይታ. ጥሬውን JSON/CSV እና ቅጽበታዊ ገጽ እይታዎችን ወደ artefact ፎልደር ይጣሉት።
4. ህጎቹን የሚጠቅሱ የአለርት አስተዳዳሪ ክስተት መታወቂያዎችን (ካለ) ያካትቱ
   `dashboards/alerts/taikai_viewer_rules.yml` እና በራስ-የተዘጉ መሆናቸውን ልብ ይበሉ
   ሁኔታው ከተጣራ በኋላ.

ሁሉንም ነገር በ `artifacts/sorafs_taikai/drills/<YYYYMMDD>/` ስር ያከማቹ ስለዚህ ይሰርዙ
ኦዲት እና SN13-C የአስተዳደር ግምገማዎች አንድ ማህደር ማምጣት ይችላሉ።

## የቁፋሮ ቁፋሮ እና ምዝግብ ማስታወሻ

- በየወሩ የመጀመሪያ ማክሰኞ የታይካይ መልህቅን ልምምድ በ15:00UTC ያሂዱ።
  መርሐ ግብሩ ከSN13 አስተዳደር አመሳስል በፊት ማስረጃዎችን ትኩስ አድርጎ ያስቀምጣል።
- ከላይ ያሉትን ቅርሶች ከያዙ በኋላ አፈፃፀሙን በጋራ መዝገብ ላይ ያያይዙት።
  በ `scripts/telemetry/log_sorafs_drill.sh --scenario taikai-anchor`. የ
  አጋዥ በ`docs/source/sorafs/runbooks-index.md` የሚፈለገውን የJSON ግቤት ያወጣል።
- በ runbook ኢንዴክስ ውስጥ በማህደር የተቀመጡ ቅርሶችን ያገናኙ እና ያልተሳካውን ያሳድጉ
  በ48ሰአታት ውስጥ በሚዲያ መድረክ WG/SRE በኩል ማንቂያዎች ወይም ዳሽቦርድ ድግግሞሾች
  ቻናል.
- የመሰርሰሪያ ማጠቃለያ ቅጽበታዊ ገጽ እይታን ያቀናብሩ (ዘግይቶ ፣ ተንሸራታች ፣ ስህተቶች ፣ CEK ማሽከርከር ፣
  የመሸጎጫ ግፊት) ከስፑል ጥቅል ጎን ለጎን ኦፕሬተሮች እንዴት በትክክል ማሳየት ይችላሉ
  ዳሽቦርዶቹ በልምምድ ወቅት ያሳዩ ነበር።

ወደ [Taikai Anchor Runbook](./taikai-anchor-runbook.md) ይመልከቱ
ሙሉ የ Sev1 አሰራር እና የማስረጃ ዝርዝር. ይህ ገጽ የሚይዘው ብቻ ነው።
🈺 ከመሄድዎ በፊት SN13-C የሚፈልገው ዳሽቦርድ-ተኮር መመሪያ።