---
lang: am
direction: ltr
source: docs/portal/docs/nexus/nexus-elastic-lane.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 2ead8c13470d6e8766d8f161cdd9443ef29c72d3f87bd7aac27f179c3e1c98fb
source_last_modified: "2026-01-22T16:26:46.498151+00:00"
translation_last_reviewed: 2026-02-07
id: nexus-elastic-lane
title: Elastic lane provisioning (NX-7)
sidebar_label: Elastic Lane Provisioning
description: Bootstrap workflow for creating Nexus lane manifests, catalog entries, and rollout evidence.
translator: machine-google-reviewed
---

::: ማስታወሻ ቀኖናዊ ምንጭ
ይህ ገጽ `docs/source/nexus_elastic_lane.md` ያንጸባርቃል። ትርጉሙ በፖርታሉ ውስጥ እስኪያርፍ ድረስ ሁለቱንም ቅጂዎች አስተካክለው ያስቀምጡ።
::

# ላስቲክ ሌይን አቅርቦት መሣሪያ ስብስብ (NX-7)

> ** የመንገድ ካርታ ንጥል ነገር:** NX-7 - የላስቲክ ሌይን አቅርቦት መሳሪያ  
> ** ሁኔታ፡** የመሳሪያ ሥራ ተጠናቅቋል - መግለጫዎችን፣ ካታሎግ ቅንጥቦችን፣ Norito ክፍያ ጭነቶችን፣ የጭስ ሙከራዎችን፣
> እና የሎድ-ሙከራ ጥቅል ረዳቱ አሁን ማስገቢያ መዘግየት ጌቲንግ ሰፍቷል + ማስረጃው አረጋጋጭ መሆኑን ያሳያል
> የጭነት ሩጫዎች ያለ ስክሪፕት ሊታተሙ ይችላሉ።

ይህ መመሪያ አውቶማቲክ በሆነው አዲሱ `scripts/nexus_lane_bootstrap.sh` ረዳት በኩል ኦፕሬተሮችን ይራመዳል
የሌይን አንጸባራቂ ትውልድ፣ የሌይን/የመረጃ ቦታ ካታሎግ ቅንጥቦች እና የታቀዱ ማስረጃዎች። ግቡ ማድረግ ነው።
ብዙ ፋይሎችን በእጅ ሳያርትዑ ወይም አዲስ I18NT0000001X መስመሮችን (ይፋዊም ሆነ የግል) ማሽከርከር ቀላል ነው።
ካታሎግ ጂኦሜትሪ በእጅ እንደገና ማግኘት.

## 1. ቅድመ ሁኔታዎች

1. የሌይን ተለዋጭ ስም፣ የውሂብ ቦታ፣ የአረጋጋጭ ስብስብ፣ የስህተት መቻቻል (`f`) እና የሰፈራ ፖሊሲ የአስተዳደር ማጽደቅ።
2. የተጠናቀቀ አረጋጋጭ ዝርዝር (የመለያ መታወቂያዎች) እና የተጠበቀ የስም ቦታ ዝርዝር።
3. የመነጩ ቅንጣቢዎችን ማያያዝ እንዲችሉ ወደ መስቀለኛ ውቅረት ማከማቻ ይድረሱ።
4. የሌይን አንጸባራቂ መዝገብ ቤት መንገዶች (`nexus.registry.manifest_directory` ይመልከቱ እና
   `cache_directory`).
5. የቴሌሜትሪ እውቂያዎች/ፔጄርዱቲ ለመንኮራኩሩ መያዣዎች ማንቂያዎች ልክ እንደሌላው ሊጣበቁ ይችላሉ።
   መስመር ላይ ይመጣል.

## 2. የሌይን ቅርሶችን መፍጠር

ረዳቱን ከማጠራቀሚያ ስር ያሂዱ፡-

```bash
scripts/nexus_lane_bootstrap.sh \
  --lane-alias "Payments Lane" \
  --lane-id 3 \
  --dataspace-alias payments \
  --governance-module parliament \
  --settlement-handle xor_global \
  --validator <i105-account-id> \
  --validator <i105-account-id> \
  --validator <i105-account-id> \
  --protected-namespace payments \
  --description "High-throughput interbank payments lane" \
  --dataspace-description "Payments dataspace" \
  --route-instruction finance::pacs008 \
  --encode-space-directory \
  --space-directory-out artifacts/nexus/payments_lane/payments.manifest.to \
  --telemetry-contact payments-ops@sora.org \
  --output-dir artifacts/nexus/payments_lane
```

ቁልፍ ባንዲራዎች፡-

- `--lane-id` በ`nexus.lane_catalog` ውስጥ ካለው አዲሱ የመግቢያ ኢንዴክስ ጋር መዛመድ አለበት።
- `--dataspace-alias` እና `--dataspace-id/hash` የመረጃ ቦታ ካታሎግ ግቤትን ይቆጣጠራሉ (ነባሪ ለ
  የሌይን መታወቂያ ሲቀር)።
- `--validator` ሊደገም ወይም ከ `--validators-file` ሊመጣ ይችላል።
- `--route-instruction` / I18NI0000037X ለመለጠፍ ዝግጁ የሆኑ የማዞሪያ ደንቦችን ያወጣል።
- `--metadata key=value` (ወይም `--telemetry-contact/channel/runbook`) የ runbook እውቂያዎችን ያንሱ
  ዳሽቦርዶች ወዲያውኑ ትክክለኛዎቹን ባለቤቶች ይዘረዝራሉ.
- `--allow-runtime-upgrades` + `--runtime-upgrade-*` የሩጫ ጊዜ ማሻሻያ መንጠቆውን ወደ አንጸባራቂው ይጨምሩ
  መስመሩ የተራዘመ የኦፕሬተር መቆጣጠሪያዎችን ሲፈልግ.
- `--encode-space-directory` `cargo xtask space-directory encode` በራስ-ሰር ይጠራል። ጋር ያጣምሩት።
  `--space-directory-out` ኢንኮድ የተደረገውን I18NI0000045X ፋይል ከነባሪው ሌላ ቦታ ሲፈልጉ።

ስክሪፕቱ በ`--output-dir` ውስጥ ሶስት ቅርሶችን ያዘጋጃል (የአሁኑ ማውጫ ነባሪ)።
ኢንኮዲንግ ሲነቃ አማራጭ አራተኛ ሲደመር፡

1. `<slug>.manifest.json` — የአረጋጋጭ ምልአተ ጉባኤ፣ የተጠበቁ የስም ቦታዎች እና የሌይን አንጸባራቂ
   አማራጭ የሩጫ ጊዜ ማሻሻያ መንጠቆ ሜታዳታ።
2. `<slug>.catalog.toml` - የ TOML ቅንጭብ ከ I18NI0000049X ፣ `[[nexus.dataspace_catalog]]` ፣
   እና ማንኛውም የተጠየቀ የማዘዣ ደንቦች. `fault_tolerance` በመረጃ ቦታ ግቤት መጠን ላይ መዘጋጀቱን ያረጋግጡ
   የሌይን ማስተላለፊያ ኮሚቴ (`3f+1`)።
3. `<slug>.summary.json` — የጂኦሜትሪ (ስሎግ፣ ክፍልፋዮች፣ ሜታዳታ) እና የተገለጸውን የኦዲት ማጠቃለያ
   አስፈላጊ የመልቀቂያ ደረጃዎች እና ትክክለኛው የI18NI0000054X ትዕዛዝ (በስር
   `space_directory_encode.command`)። ለማስረጃ ይህን JSON ከመሳፈሪያ ትኬት ጋር ያያይዙት።
4. `<slug>.manifest.to` - `--encode-space-directory` ሲዘጋጅ የተለቀቀው; ለ Torii's ዝግጁ
   `iroha app space-directory manifest publish` ፍሰት.

ፋይሎችን ሳይጽፉ JSON/ ቅንጥቦችን አስቀድመው ለማየት `--dry-run` ይጠቀሙ እና ለመፃፍ I18NI0000060X ይጠቀሙ
ነባር ቅርሶች.

## 3. ለውጦቹን ይተግብሩ

1. አንጸባራቂውን JSON ወደ የተዋቀረው `nexus.registry.manifest_directory` (እና ወደ መሸጎጫው ውስጥ ይቅዱ
   መዝገቡ የርቀት ቅርቅቦችን የሚያንጸባርቅ ከሆነ ማውጫ)። መግለጫዎች ከተዘጋጁ ፋይሉን ያስገቡ
   የእርስዎ ውቅር repo.
2. የካታሎግ ቅንጣቢውን ወደ `config/config.toml` (ወይም ተገቢው I18NI0000063X) ላይ ጨምር። ያረጋግጡ
   `nexus.lane_count` ቢያንስ I18NI0000065X ነው፣ እና ማንኛውንም `nexus.routing_policy.rules` ያዘምኑ
   በአዲሱ መስመር ላይ መጠቆም አለበት.
3. ኢንኮድ (`--encode-space-directory` ከዘለሉ) እና አንጸባራቂውን ወደ ስፔስ ማውጫ ያትሙ
   በማጠቃለያው (`space_directory_encode.command`) የተያዘውን ትዕዛዝ በመጠቀም. ይህ ያፈራል
   `.manifest.to` ክፍያ Torii ለኦዲተሮች ማስረጃዎችን ይጠብቃል እና ይመዘግባል; በኩል ያስገቡ
   `iroha app space-directory manifest publish`.
4. `irohad --sora --config path/to/config.toml --trace-config` ን ያሂዱ እና የመከታተያ ውጤቱን በማህደር ያስቀምጡ
   የመልቀቅ ትኬቱ። ይህ አዲሱ ጂኦሜትሪ ከተፈጠረው slug/kura ክፍሎች ጋር እንደሚዛመድ ያረጋግጣል።
5. የአንጸባራቂ/የካታሎግ ለውጦች ከተሰማሩ በኋላ ለመስመሩ የተመደቡትን አረጋጋጮች እንደገና ያስጀምሩ። አቆይ
   ለወደፊት ኦዲቶች በቲኬቱ ውስጥ ያለው JSON ማጠቃለያ።

## 4. የመመዝገቢያ ማከፋፈያ ጥቅል ይገንቡ

ኦፕሬተሮች የሌይን አስተዳደር መረጃን ያለሱ ማሰራጨት እንዲችሉ የተፈጠረውን አንጸባራቂ እና ተደራቢ ያሽጉ
በእያንዳንዱ አስተናጋጅ ላይ ማስተካከያዎችን ማስተካከል. የጥቅል ረዳት ቅጂዎች ወደ ቀኖናዊ አቀማመጥ ይገለጣሉ፣
ለ`nexus.registry.cache_directory` የአማራጭ አስተዳደር ካታሎግ ተደራቢ ያዘጋጃል እና
ታርቦል ከመስመር ውጭ ማስተላለፎች;

```bash
scripts/nexus_lane_registry_bundle.sh \
  --manifest artifacts/nexus/payments_lane/payments.manifest.json \
  --output-dir artifacts/nexus/payments_lane/registry_bundle \
  --default-module parliament \
  --module name=parliament,module_type=parliament,param.quorum=2 \
  --bundle-out artifacts/nexus/payments_lane/registry_bundle.tar.gz
```

ውጤቶች፡

1. `manifests/<slug>.manifest.json` - እነዚህን ወደ የተዋቀረው ይቅዱ
   `nexus.registry.manifest_directory`.
2. `cache/governance_catalog.json` - ወደ I18NI0000076X ጣል. እያንዳንዱ `--module`
   መግቢያ የአስተዳደር-ሞዱል መለዋወጥን (NX-2) በማንቃት ሊሰካ የሚችል የሞዱል ትርጉም ይሆናል
   `config.toml` ከማርትዕ ይልቅ መሸጎጫውን በማዘመን ላይ።
3. `summary.json` - hashes፣ ተደራቢ ሜታዳታ እና የኦፕሬተር መመሪያዎችን ያካትታል።
4. አማራጭ I18NI0000080X — ለ SCP፣ S3፣ ወይም artifact trackers ዝግጁ።

መላውን ማውጫ (ወይም ማህደሩን) ከእያንዳንዱ አረጋጋጭ ጋር ያመሳስሉ፣ በአየር ክፍተት ካለባቸው አስተናጋጆች ያውጡ እና ይቅዱ።
Torii እንደገና ከመጀመራቸው በፊት የገለጻዎቹ + መሸጎጫ ወደ መዝገብ መሄጃዎቻቸው ተሸፍኗል።

## 5. አረጋጋጭ የጭስ ሙከራዎች

Torii እንደገና ከጀመረ በኋላ የሌይን ሪፖርቶችን I18NI0000081X ለማረጋገጥ አዲሱን የጭስ ረዳት ያሂዱ፣
መለኪያዎች የሚጠበቀውን የሌይን ብዛት ያጋልጣሉ፣ እና የታሸገው መለኪያ ግልጽ ነው። አንጸባራቂ የሚያስፈልጋቸው መስመሮች
ባዶ ያልሆነ `manifest_path` ማጋለጥ አለበት; መንገዱ ሲጠፋ ረዳቱ ወዲያውኑ ይወድቃል
እያንዳንዱ የNX-7 የማሰማራት መዝገብ የተፈረመውን አንጸባራቂ ማስረጃን ያካትታል፡-

```bash
scripts/nexus_lane_smoke.py \
  --status-url https://torii.example.com/v1/sumeragi/status \
  --metrics-url https://torii.example.com/metrics \
  --lane-alias payments \
  --expected-lane-count 3 \
  --min-da-quorum 0.95 \
  --max-oracle-staleness 75 \
  --expected-oracle-twap 60 \
  --oracle-twap-tolerance 5 \
  --max-oracle-haircut-bps 75 \
  --min-settlement-buffer 0.25 \
  --min-block-height 1000 \
  --max-finality-lag 4 \
  --max-settlement-backlog 0.5 \
  --max-headroom-events 0 \
  --max-slot-p95 1000 \
  --max-slot-p99 1100 \
  --min-slot-samples 10
```

በራስ የተፈረሙ አካባቢዎችን ሲሞክሩ `--insecure` ይጨምሩ። ሌይኑ ከሆነ ስክሪፕቱ ዜሮ ካልሆነ ይወጣል
የጎደለ፣ የታሸገ ወይም ሜትሪክስ/ቴሌሜትሪ ከሚጠበቁት እሴቶች ተንሳፈፈ። የሚለውን ተጠቀም
`--min-block-height`፣ `--max-finality-lag`፣ `--max-settlement-backlog`፣ እና
`--max-headroom-events` ማዞሪያዎች በአንድ ሌይን የማገጃ ቁመት/የመጨረሻ/የኋላ መዝገብ/ዋና ክፍል ቴሌሜትሪ ለማቆየት
በሚሰሩ ኤንቨሎፖችዎ ውስጥ፣ እና ከ`--max-slot-p95`/`--max-slot-p99` ጋር አያይዟቸው።
(ሲደመር `--min-slot-samples`) ረዳቱን ሳይለቁ NX-18 ማስገቢያ-ቆይታ ዒላማዎችን ለማስፈጸም።

ለአየር ክፍተት ማረጋገጫዎች (ወይም CI) በቀጥታ ከመምታት ይልቅ የተያዘውን Torii ምላሽ እንደገና ማጫወት ይችላሉ
የመጨረሻ ነጥብ፡-

```bash
scripts/nexus_lane_smoke.py \
  --status-file fixtures/nexus/lanes/status_ready.json \
  --metrics-file fixtures/nexus/lanes/metrics_ready.prom \
  --lane-alias core \
  --lane-alias payments \
  --expected-lane-count 3 \
  --min-da-quorum 0.95 \
  --max-oracle-staleness 75 \
  --expected-oracle-twap 60 \
  --oracle-twap-tolerance 5 \
  --max-oracle-haircut-bps 75 \
  --min-settlement-buffer 0.25 \
  --min-block-height 1000 \
  --max-finality-lag 4 \
  --max-settlement-backlog 0.5 \
  --max-headroom-events 0 \
  --max-slot-p95 1000 \
  --max-slot-p99 1100 \
  --min-slot-samples 10
```

በ I18NI0000091X ስር የተቀዳው የቤት እቃዎች በቡትስትራክቱ የተሰሩ ቅርሶችን ያንፀባርቃሉ
ረዳት ስለዚህ አዲስ መግለጫዎች ያለ ስክሪፕት መደርደር ይችላሉ። CI ተመሳሳይ ፍሰትን ይሠራል
`ci/check_nexus_lane_smoke.sh` እና `ci/check_nexus_lane_registry_bundle.sh`
(ተለዋጭ ስም፡ I18NI0000094X) የ NX-7 ጭስ ረዳት ከታተመ ጋር መቆየቱን ለማረጋገጥ
የመጫኛ ፎርማት እና የጥቅል መፈጨት/ተደራቢዎች ሊባዙ የሚችሉ መሆናቸውን ለማረጋገጥ።

መስመር ሲቀየር የ`nexus.lane.topology` ቴሌሜትሪ ክስተቶችን ያንሱ (ለምሳሌ በ
`journalctl -u irohad -o json | jq 'select(.msg=="nexus.lane.topology")'`) እና ወደ ውስጥ መልሰው ይመግቧቸው
የጭስ ረዳቱ. የ`--telemetry-file/--from-telemetry` ባንዲራ አዲሱን መስመር የተገደበ ሎግ እና ይቀበላል
`--require-alias-migration old:new` አንድ የI18NI0000099X ክስተት ዳግም ስሙን መዝግቧል፡

```bash
scripts/nexus_lane_smoke.py \
  --status-file fixtures/nexus/lanes/status_ready.json \
  --metrics-file fixtures/nexus/lanes/metrics_ready.prom \
  --telemetry-file fixtures/nexus/lanes/telemetry_alias_migrated.ndjson \
  --lane-alias payments \
  --lane-alias core \
  --expected-lane-count 3 \
  --min-da-quorum 0.95 \
  --max-oracle-staleness 75 \
  --expected-oracle-twap 60 \
  --oracle-twap-tolerance 5 \
  --max-oracle-haircut-bps 75 \
  --min-settlement-buffer 0.25 \
  --min-block-height 1000 \
  --max-finality-lag 4 \
  --max-settlement-backlog 0.5 \
  --max-headroom-events 0 \
  --max-slot-p95 1000 \
  --max-slot-p99 1100 \
  --min-slot-samples 10 \
  --require-alias-migration core:payments
```

የ`telemetry_alias_migrated.ndjson` መግጠሚያው CI ማረጋገጥ እንዲችል ቀኖናዊውን እንደገና መሰየም ናሙና ይዘጋል።
የቀጥታ መስቀለኛ መንገድን ሳያገኙ የቴሌሜትሪ ትንተና መንገድ።

## የአረጋጋጭ ጭነት ሙከራዎች (NX-7 ማስረጃ)

የመንገድ ካርታ **NX-7** ሊባዛ የሚችል አረጋጋጭ የጭነት ሩጫ ለመላክ እያንዳንዱ አዲስ መስመር ይፈልጋል። ተጠቀም
`scripts/nexus_lane_load_test.py` የጭስ ቼኮችን ፣ የመግቢያ ጊዜ በሮች እና የጭስ ማውጫውን ለመገጣጠም
አስተዳደር እንደገና ሊጫወት የሚችለውን ወደ አንድ የስነ-ጥበብ ስብስብ ያሳያል፡-

```bash
scripts/nexus_lane_load_test.py \
  --status-file artifacts/nexus/load/payments-2026q2/torii_status.json \
  --metrics-file artifacts/nexus/load/payments-2026q2/metrics.prom \
  --telemetry-file artifacts/nexus/load/payments-2026q2/nexus.lane.topology.ndjson \
  --lane-alias payments \
  --lane-alias core \
  --expected-lane-count 3 \
  --slot-range 81200-81600 \
  --workload-seed NX7-PAYMENTS-2026Q2 \
  --require-alias-migration core:payments \
  --out-dir artifacts/nexus/load/payments-2026q2
```

ረዳቱ ያገለገሉትን የDA ምልአተ ጉባኤ፣ የቃል፣ የመቋቋሚያ ቋት፣ TEU እና የመግቢያ ጊዜ በሮች ያስፈጽማል።
በጢስ ረዳቱ እና `smoke.log`፣ `slot_summary.json`፣ ማስገቢያ ጥቅል መግለጫ እና ይጽፋል።
`load_test_manifest.json` ወደ ተመረጠው `--out-dir` ስለዚህ የጭነት ሩጫዎች በቀጥታ ከ ጋር ማያያዝ ይችላሉ
የታቀዱ ቲኬቶች ያለ ስክሪፕት።

## 6. ቴሌሜትሪ እና የአስተዳደር ክትትል

- የሌይን ዳሽቦርዶችን (`dashboards/grafana/nexus_lanes.json` እና ተዛማጅ ተደራቢዎችን) ያዘምኑ
  አዲስ ሌይን መታወቂያ እና ሜታዳታ። የተፈጠሩት የሜታዳታ ቁልፎች (`contact`፣ `channel`፣ `runbook`፣ ወዘተ) ያደርጋሉ።
  መሰየሚያዎችን በቅድሚያ መሙላት ቀላል ነው.
- Wire PagerDuty/Alertmanager መግቢያን ከማንቃት በፊት ለአዲሱ መስመር ህግጋት። `summary.json`
  ቀጣይ-ደረጃ ድርድር የማረጋገጫ ዝርዝሩን በ[Nexus ክወናዎች](./nexus-operations) ያንጸባርቃል።
- የማረጋገጫው ስብስብ በቀጥታ ከተለቀቀ በኋላ የሰነድ ቅርቅቡን በጠፈር ማውጫ ውስጥ ያስመዝግቡት። ተመሳሳይ ይጠቀሙ
  በአስተዳዳሪው runbook መሰረት የተፈረመ በረዳት የተፈጠረ JSON አንጸባራቂ።
- [Sora I18NT0000003X ኦፕሬተር ተሳፍሮ](./nexus-operator-onboarding) ለጭስ ሙከራዎች (FindNetworkStatus፣ Torii) ተከተል።
  ሊደረስበት የሚችል) እና ማስረጃውን ከላይ በተዘጋጀው አርቲፊሻል ስብስብ ይያዙ.

## 7. ደረቅ አሂድ ምሳሌ

ፋይሎችን ሳይጽፉ ቅርሶቹን አስቀድመው ለማየት፡-

```bash
scripts/nexus_lane_bootstrap.sh \
  --lane-alias "Payments Lane" \
  --lane-id 3 \
  --dataspace-alias payments \
  --governance-module parliament \
  --settlement-handle xor_global \
  --validator <i105-account-id> \
  --validator <i105-account-id> \
  --dry-run
```

ትዕዛዙ የJSON ማጠቃለያን እና የTOML ቅንጥቡን ያትማል፣ ይህም በፍጥነት እንዲደጋገም ያስችላል።
እቅድ ማውጣት.

---

ለተጨማሪ አውድ የሚከተለውን ይመልከቱ፡-- [Nexus ኦፕሬሽኖች](I18NU0000022X) - የአሠራር ማረጋገጫ ዝርዝር እና የቴሌሜትሪ መስፈርቶች።
- [Sora I18NT0000005X ኦፕሬተር ኦንቦርዲንግ](./nexus-operator-onboarding) - ዝርዝር የመሳፈሪያ ፍሰትን የሚያመለክት
  አዲስ ረዳት.
- [Nexus ሌይን ሞዴል](./nexus-lane-model) - በመሳሪያው ጥቅም ላይ የሚውለው የሌይን ጂኦሜትሪ፣ ስሎግስ እና የማከማቻ አቀማመጥ።