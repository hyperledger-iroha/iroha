---
lang: am
direction: ltr
source: docs/portal/docs/nexus/nexus-default-lane-quickstart.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 6120b882618e0f9b6113948d3b12d97e0152a5fc5d4350681ba30aaf114e99d3
source_last_modified: "2026-01-22T14:45:01.354580+00:00"
translation_last_reviewed: 2026-02-07
id: nexus-default-lane-quickstart
title: Default lane quickstart (NX-5)
sidebar_label: Default Lane Quickstart
description: Configure and verify the Nexus default lane fallback so Torii and SDKs can omit lane_id in public lanes.
translator: machine-google-reviewed
---

::: ማስታወሻ ቀኖናዊ ምንጭ
ይህ ገጽ `docs/source/quickstart/default_lane.md` ያንጸባርቃል። ሁለቱንም ቅጂዎች ያስቀምጡ
በፖርታሉ ውስጥ ለትርጉም መሬቶች እስኪያጣ ድረስ የተስተካከለ።
::

# ነባሪ ሌይን ፈጣን ማስጀመሪያ (NX-5)

> ** የመንገድ ካርታ አውድ፡** NX-5 — ነባሪ የህዝብ መስመር ውህደት። የሩጫ ጊዜ አሁን
> የ I18NI0000015X ውድቀትን ያጋልጣል ስለዚህ Torii REST/gRPC
> የመጨረሻ ነጥቦች እና እያንዳንዱ ኤስዲኬ ትራፊክ በሚኖርበት ጊዜ `lane_id` በደህና መተው ይችላል።
> በቀኖናዊው የሕዝብ መስመር ላይ። ይህ መመሪያ ኦፕሬተሮችን በማዋቀር በኩል ይራመዳል
> ካታሎግ፣ በ`/status` ውስጥ ያለውን ውድቀት ማረጋገጥ እና ደንበኛውን የአካል ብቃት እንቅስቃሴ ማድረግ
> ባህሪ ከመጨረሻ እስከ መጨረሻ።

## ቅድመ ሁኔታዎች

- የ `irohad` የሶራ/I18NT0000001X ግንባታ (በ `irohad --sora --config ...` አሂድ)።
- የ `nexus.*` ክፍሎችን ማርትዕ እንዲችሉ የማዋቀሪያው ማከማቻ ይድረሱ።
- `iroha_cli` ከዒላማው ስብስብ ጋር ለመነጋገር ተዋቅሯል።
- `curl`/`jq` (ወይም ተመጣጣኝ) የ Torii I18NI0000024X ክፍያን ለመመርመር።

## 1. የሌይን እና የውሂብ ቦታ ካታሎግ ይግለጹ

በአውታረ መረቡ ላይ መኖር ያለባቸውን መስመሮችን እና የውሂብ ክፍተቶችን ያውጁ። ቅንጭቡ
ከታች (ከI18NI0000025X የተከረከመ) ሶስት የህዝብ መስመሮችን ይመዘግባል
በተጨማሪም ተዛማጅ የውሂብ ቦታ ተለዋጭ ስሞች

```toml
[nexus]
lane_count = 3

[[nexus.lane_catalog]]
index = 0
alias = "core"
description = "Primary execution lane"
dataspace = "universal"

[[nexus.lane_catalog]]
index = 1
alias = "governance"
description = "Governance & parliament traffic"
dataspace = "governance"

[[nexus.lane_catalog]]
index = 2
alias = "zk"
description = "Zero-knowledge attachments"
dataspace = "zk"

[[nexus.dataspace_catalog]]
alias = "universal"
id = 0
description = "Single-lane data space"
fault_tolerance = 1

[[nexus.dataspace_catalog]]
alias = "governance"
id = 1
description = "Governance proposals & manifests"
fault_tolerance = 1

[[nexus.dataspace_catalog]]
alias = "zk"
id = 2
description = "Zero-knowledge proofs and attachments"
fault_tolerance = 1
```

እያንዳንዱ `index` ልዩ እና ተከታታይ መሆን አለበት። የውሂብ ቦታ መታወቂያዎች 64-ቢት እሴቶች ናቸው;
ከላይ ያሉት ምሳሌዎች ግልጽነት ለማግኘት ከሌይን ኢንዴክሶች ጋር ተመሳሳይ የቁጥር እሴቶችን ይጠቀማሉ።

## 2. የማዞሪያ ነባሪዎችን እና አማራጭ መሻሮችን ያዘጋጁ

የ`nexus.routing_policy` ክፍል የመመለሻ መስመርን ይቆጣጠራል እና ይፈቅዳል
ለተወሰኑ መመሪያዎች ወይም የመለያ ቅድመ ቅጥያዎች ማዞሪያን መሻር። ደንብ ከሌለ
ግጥሚያዎች፣ መርሐግብር አውጪው ግብይቱን ወደ የተዋቀረው I18NI0000028X ያመራዋል።
እና `default_dataspace`. የራውተር አመክንዮ ይኖራል
`crates/iroha_core/src/queue/router.rs` እና ፖሊሲውን በግልፅ ለ
Torii REST/gRPC ንጣፎች።

```toml
[nexus.routing_policy]
default_lane = 0                # use the "core" lane when no rules match
default_dataspace = "universal"    # reuse the public dataspace for the fallback

[[nexus.routing_policy.rules]]
lane = 1
dataspace = "governance"
[nexus.routing_policy.rules.matcher]
instruction = "governance"
description = "Route governance instructions to the governance lane"

[[nexus.routing_policy.rules]]
lane = 2
dataspace = "zk"
[nexus.routing_policy.rules.matcher]
instruction = "smartcontract::deploy"
description = "Route contract deployments to the zk lane for proof tracking"
```

በኋላ አዲስ መስመሮችን ሲያክሉ መጀመሪያ ካታሎጉን ያዘምኑ፣ ከዚያ ማዘዋወሩን ያራዝሙ
ደንቦች. የኋለኛው መስመር ወደያዘው የህዝብ መስመር መጠቆሙን መቀጠል አለበት።

## 3. ከተተገበረው ፖሊሲ ጋር መስቀለኛ መንገድ አስነሳ

```bash
IROHA_CONFIG=/path/to/nexus/config.toml
irohad --sora --config "${IROHA_CONFIG}"
```

መስቀለኛ መንገድ በሚነሳበት ጊዜ የተገኘውን የማዞሪያ መመሪያ ይመዘግባል። ማንኛውም የማረጋገጫ ስህተቶች
(የጠፉ ኢንዴክሶች፣ የተባዙ ተለዋጭ ስሞች፣ ልክ ያልሆኑ የውሂብ ቦታ መታወቂያዎች) ከዚህ በፊት ታይተዋል።
ወሬ ይጀምራል።

## 4. የሌይን አስተዳደር ሁኔታን ያረጋግጡ

አንዴ መስቀለኛ መንገድ መስመር ላይ ከሆነ፣ ነባሪው መስመር መሆኑን ለማረጋገጥ የCLI አጋዥን ይጠቀሙ
የታሸገ (ግልጽ የተጫነ) እና ለትራፊክ ዝግጁ። የማጠቃለያ እይታ አንድ ረድፍ ያትማል
በየመንገድ

```bash
iroha_cli app nexus lane-report --summary
```

ምሳሌ ውፅዓት፡

```
Lane  Alias            Module           Status  Quorum  Validators  Detail
   0  core             parliament       ready      05           07  manifest ok
   1  governance       parliament       ready      05           07  manifest ok
   2  zk               parliament       sealed     03           05  manifest required
```

ነባሪው መስመር `sealed` ካሳየ ከዚህ በፊት የሌይን አስተዳደርን መጽሐፍ ይከተሉ
የውጭ ትራፊክን መፍቀድ. የ`--fail-on-sealed` ባንዲራ ለCI ምቹ ነው።

## 5. የ Torii ሁኔታን የሚጫኑ ጭነቶችን ይፈትሹ

የ`/status` ምላሽ ሁለቱንም የማዞሪያ ፖሊሲውን እና የየሌይን መርሐግብርን ያጋልጣል
ቅጽበታዊ ገጽ እይታ የተዋቀሩ ነባሪዎችን ለማረጋገጥ እና ያንን ለመፈተሽ `curl`/`jq` ይጠቀሙ
የኋላው መስመር ቴሌሜትሪ እያመረተ ነው፡-

```bash
curl -s http://127.0.0.1:8080/status | jq '.nexus.routing_policy'
```

የናሙና ውጤት፡

```json
{
  "default_lane": 0,
  "default_dataspace": "universal",
  "rules": [
    {"lane": 1, "dataspace": "governance", "matcher": {"instruction": "governance"}},
    {"lane": 2, "dataspace": "zk", "matcher": {"instruction": "smartcontract::deploy"}}
  ]
}
```

የቀጥታ መርሐግብር ቆጣሪዎችን ለመንገድ `0` ለመመርመር፡-

```bash
curl -s http://127.0.0.1:8080/status \
  | jq '.nexus_scheduler_lane_teu_status[] | select(.lane_id == 0)
        | {lane_id, alias, dataspace_alias, committed, manifest_ready, scheduler_utilization_pct}'
```

ይህ የTEU ቅጽበታዊ ገጽ እይታ፣ ሜታዳታ እየተለዋወጠ እና አንጸባራቂ ባንዲራዎች እንደሚሰመሩ ያረጋግጣል
ከማዋቀሩ ጋር. ተመሳሳይ ጭነት I18NT0000000X ፓነሎች ለ
ሌይን-ingest ዳሽቦርድ.

## 6. የደንበኛ ነባሪዎችን ይለማመዱ

** ዝገት/CLI
  `--lane-id` / `LaneSelector` ሳያልፉ ሲቀሩ። ወረፋው ራውተር ስለዚህ
  ወደ `default_lane` ይመለሳል። ግልጽ I18NI0000042X/I18NI0000043X ባንዲራዎችን ተጠቀም
  ነባሪ ያልሆነ መስመር ላይ ዒላማ ሲደረግ ብቻ።
- **ጄኤስ/ስዊፍት/አንድሮይድ
  እና በ`/status` ወደ ተገለጸው እሴት ይመለሱ። የማዘዣ መመሪያውን እንደገባ ያቆዩት።
  የሞባይል መተግበሪያዎች ድንገተኛ አያስፈልጋቸውም ስለዚህ በማዘጋጀት እና በማምረት ላይ ያመሳስሉ።
  መልሶ ማዋቀር.
- **የቧንቧ መስመር/ኤስኤስኢ ሙከራዎች።** የግብይቱ ክስተት ማጣሪያዎች ይቀበላሉ።
  `tx_lane_id == <u32>` ትንበያዎች (I18NI0000048X ይመልከቱ)። ይመዝገቡ
  የተላከ መሆኑን ለማረጋገጥ `/v2/pipeline/events/transactions` ከዚያ ማጣሪያ ጋር
  ያለ ግልጽ ሌይን ከወደ ኋላ ሌይን መታወቂያ ስር ይደርሳል።

## 7. ታዛቢነት እና የአስተዳደር መንጠቆዎች

- `/status` እንዲሁም `nexus_lane_governance_sealed_total` እና ያትማል።
  `nexus_lane_governance_sealed_aliases` ስለዚህ Alertmanager በማንኛውም ጊዜ ማስጠንቀቅ ይችላል ሀ
  ሌይን አንጸባራቂውን ያጣል። እነዚያ ማንቂያዎች ለዴቭኔትስ እንኳን እንደነቁ ያቆዩት።
- የጊዜ ሰሌዳ ሰጪው ቴሌሜትሪ ካርታ እና የሌይን አስተዳደር ዳሽቦርድ
  (`dashboards/grafana/nexus_lanes.json`) ተለዋጭ ስሞችን / ስሎግ መስኮችን ከ
  ካታሎግ. ተለዋጭ ስም ከቀየሩ፣ ተጓዳኝ የኩራ ማውጫዎችን እንደገና ይሰይሙ
  ኦዲተሮች ቆራጥ መንገዶችን ይጠብቃሉ (በNX-1 ስር ይከተላሉ)።
- ለነባሪ መስመሮች የፓርላማ ማጽደቂያዎች የመመለሻ እቅድን ማካተት አለባቸው። መዝገብ
  በእርስዎ ውስጥ ከዚህ ፈጣን ጅምር ጎን ለጎን ግልፅ ሃሽ እና የአስተዳደር ማስረጃ
  ኦፕሬተር runbook ስለዚህ የወደፊት ሽክርክሪቶች አስፈላጊውን ሁኔታ አይገምቱም.

አንዴ እነዚህ ቼኮች ካለፉ `nexus.routing_policy.default_lane` እንደ
በአውታረ መረቡ ላይ የኮድ መንገዶች.