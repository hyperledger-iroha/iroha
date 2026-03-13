---
lang: am
direction: ltr
source: docs/portal/versioned_docs/version-2025-q2/sorafs/pin-registry-ops.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 0dc64bb4067d734250852a74a65a2100bd68e5ff35f9e8e9dbf3bd2b86f00cfa
source_last_modified: "2026-01-22T14:35:36.898296+00:00"
translation_last_reviewed: 2026-02-07
id: pin-registry-ops-am
title: Pin Registry Operations
sidebar_label: Pin Registry Operations
description: Monitor and triage the SoraFS pin registry and replication SLA metrics.
translator: machine-google-reviewed
slug: /sorafs/pin-registry-ops-am
---

::: ማስታወሻ ቀኖናዊ ምንጭ
መስተዋቶች `docs/source/sorafs/runbooks/pin_registry_ops.md`. ሁለቱንም ስሪቶች በሁሉም ልቀቶች ላይ ያቆዩ።
::

## አጠቃላይ እይታ

ይህ Runbook የSoraFS ፒን መዝገብ እና የማባዛት አገልግሎት ደረጃ ስምምነቶችን (SLAs) እንዴት እንደሚከታተል እና እንደሚለይ ያሳያል። መለኪያዎቹ የሚመጡት ከ`iroha_torii` ነው እና በPrometheus በኩል በ`torii_sorafs_*` የስም ቦታ ይላካሉ። Torii የመመዝገቢያ ሁኔታን ከበስተጀርባ በ 30 ሰከንድ ልዩነት ውስጥ ናሙና ይሰጣል፣ ስለዚህ ዳሽቦርዶች ምንም ኦፕሬተሮች የ`/v2/sorafs/pin/*` የመጨረሻ ነጥቦችን ባይመርጡም አሁንም እንደነበሩ ይቆያሉ። ለአገልግሎት ዝግጁ የሆነ Grafana አቀማመጥ ከታች ባሉት ክፍሎች ላይ ካርታውን የሚይዘውን ዳሽቦርድ (`docs/source/grafana_sorafs_pin_registry.json`) አስመጣ።

## ሜትሪክ ማመሳከሪያ

| መለኪያ | መለያዎች | መግለጫ |
| ------ | ------ | -------- |
| `torii_sorafs_registry_manifests_total` | `status` (`pending` \| `approved` \| `retired`) | በሰንሰለት ላይ አንጸባራቂ ክምችት በህይወት ዑደት ሁኔታ። |
| `torii_sorafs_registry_aliases_total` | - | በመዝገቡ ውስጥ የተመዘገቡ የነቁ አንጸባራቂ ተለዋጭ ስሞች ብዛት። |
| `torii_sorafs_registry_orders_total` | `status` (`pending` \| `completed` \| `expired`) | የማባዛት ትዕዛዝ የኋላ መዝገብ በሁኔታ የተከፋፈለ። |
| `torii_sorafs_replication_backlog_total` | - | የምቾት መለኪያ `pending` ትዕዛዞችን ማንጸባረቅ። |
| `torii_sorafs_replication_sla_total` | `outcome` (`met` \| `missed` \| `pending`) | SLA ሒሳብ: `met` የተጠናቀቁ ትዕዛዞችን በጊዜ ገደብ ውስጥ ይቆጥራል, `missed` ዘግይተው የተጠናቀቁትን + ጊዜያቸውን ያጠቃለለ, `pending` ምርጥ ትዕዛዞችን ያሳያል። |
| `torii_sorafs_replication_completion_latency_epochs` | `stat` (`avg` \| `p95` \| `max` \| `count`) | የተዋሃደ የማጠናቀቂያ መዘግየት (በመወጣት እና በማጠናቀቅ መካከል ያሉ ወቅቶች)። |
| `torii_sorafs_replication_deadline_slack_epochs` | `stat` (`avg` \| `p95` \| `max` \| `count`) | በመጠባበቅ ላይ-ትዕዛዝ ደካማ መስኮቶች (የተሰጠው የጊዜ ገደብ የቀነሰው ዘመን)። |

ሁሉም መለኪያዎች በእያንዳንዱ ቅጽበታዊ ፎቶ ላይ ዳግም ይጀመራሉ፣ ስለዚህ ዳሽቦርዶች በ`1m` cadaence ወይም በፍጥነት ናሙና መሆን አለባቸው።

## Grafana ዳሽቦርድ

ዳሽቦርዱ JSON የኦፕሬተር የስራ ፍሰቶችን የሚሸፍኑ ሰባት ፓነሎች አሉት። የታወቁ ገበታዎችን መገንባት ከመረጡ ጥያቄዎቹ ለፈጣን ማጣቀሻ ከዚህ በታች ተዘርዝረዋል።

1. ** የህይወት ኡደትን አሳይ ** - `torii_sorafs_registry_manifests_total` (በ `status` የተሰበሰበ)።
2. ** ተለዋጭ ስም ካታሎግ አዝማሚያ *** - `torii_sorafs_registry_aliases_total`.
3. ** ወረፋ በሁኔታዎች ይዘዙ ** - `torii_sorafs_registry_orders_total` (በ `status` የተሰበሰበ)።
4. ** Backlog vs የአገልግሎት ጊዜው ያለፈባቸው ትዕዛዞች *** - `torii_sorafs_replication_backlog_total` እና `torii_sorafs_registry_orders_total{status="expired"}` ወደ ላይ ሙሌት ያጣምራል።
5. ** SLA የስኬት ጥምርታ *** -

   ```promql
   sum(torii_sorafs_replication_sla_total{outcome="met"})
   /
   clamp_min(
     sum(torii_sorafs_replication_sla_total{outcome=~"met|missed"}),
     1
   )
   ```

6. ** መዘግየት vs ቀነ-ገደብ መዘግየት** - ተደራቢ `torii_sorafs_replication_completion_latency_epochs{stat="p95"}` እና `torii_sorafs_replication_deadline_slack_epochs{stat="avg"}`። ፍፁም ደካማ ወለል ሲፈልጉ `min_over_time` እይታዎችን ለመጨመር Grafana ትራንስፎርሜሽን ይጠቀሙ፡-

   ```promql
   min_over_time(torii_sorafs_replication_deadline_slack_epochs{stat="avg"}[15m])
   ```

7. ** ያመለጡ ትዕዛዞች (1ሰዓት ተመን) *** -

   ```promql
   sum(increase(torii_sorafs_replication_sla_total{outcome="missed"}[1h]))
   ```

## የማንቂያ ገደቦች- ** SLA ስኬት  0**
  - ገደብ: `increase(torii_sorafs_registry_orders_total{status="expired"}[5m]) > 0`
  - እርምጃ፡ የአቅራቢዎችን መጨናነቅ ለማረጋገጥ የአስተዳደርን ሁኔታ ይፈትሹ።
- ** ማጠናቀቂያ p95 > የጊዜ ገደብ ደካማ አማካይ ***
  - ገደብ: `torii_sorafs_replication_completion_latency_epochs{stat="p95"} > torii_sorafs_replication_deadline_slack_epochs{stat="avg"}`
  - እርምጃ: አቅራቢዎች ከማለቂያ ጊዜ በፊት እየፈጸሙ መሆናቸውን ያረጋግጡ; እንደገና ምደባዎችን መስጠት ያስቡበት።

### ምሳሌ Prometheus ህጎች

```yaml
groups:
  - name: sorafs-pin-registry
    rules:
      - alert: SorafsReplicationSlaDrop
        expr: sum(torii_sorafs_replication_sla_total{outcome="met"}) /
          clamp_min(sum(torii_sorafs_replication_sla_total{outcome=~"met|missed"}), 1) < 0.95
        for: 15m
        labels:
          severity: page
        annotations:
          summary: "SoraFS replication SLA below target"
          description: "SLA success ratio stayed under 95% for 15 minutes."

      - alert: SorafsReplicationBacklogGrowing
        expr: torii_sorafs_replication_backlog_total > 10
        for: 10m
        labels:
          severity: page
        annotations:
          summary: "SoraFS replication backlog above threshold"
          description: "Pending replication orders exceeded the configured backlog budget."

      - alert: SorafsReplicationExpiredOrders
        expr: increase(torii_sorafs_registry_orders_total{status="expired"}[5m]) > 0
        for: 0m
        labels:
          severity: ticket
        annotations:
          summary: "SoraFS replication orders expired"
          description: "At least one replication order expired in the last five minutes."
```

## የመለያ የስራ ፍሰት

1. **ምክንያቱን መለየት**
   - የኋላ መዝገብ ዝቅተኛ ሆኖ ሳለ SLA ሹል ካመለጠ በአቅራቢው አፈጻጸም ላይ ያተኩሩ (የPoR ውድቀቶች፣ ዘግይተው የተጠናቀቁ)።
   - የኋላ መዝገብ በተረጋጋ ሚስቶች ካደገ፣ የምክር ቤቱን ማጽደቅ የሚጠባበቁ ምልክቶችን ለማረጋገጥ መግቢያን (`/v2/sorafs/pin/*`) ይፈትሹ።
2. ** የአቅራቢውን ሁኔታ ያረጋግጡ ***
   - `iroha app sorafs providers list` ን ያሂዱ እና የታወቁት ችሎታዎች የማባዛት መስፈርቶች ጋር የሚዛመዱ መሆናቸውን ያረጋግጡ።
   - የጊቢ እና የPoR ስኬትን ለማረጋገጥ የ`torii_sorafs_capacity_*` መለኪያዎችን ያረጋግጡ።
3. ** ማባዛትን እንደገና መድቡ ***
   - backlog slack (`stat="avg"`) ከ5 epochs በታች ሲወርድ በ`sorafs_manifest_stub capacity replication-order` በኩል አዲስ ትዕዛዞችን ያቅርቡ (ማኒፌስት/CAR ማሸጊያ `iroha app sorafs toolkit pack` ይጠቀማል)።
   - ተለዋጭ ስሞች የነቃ አንጸባራቂ ማሰሪያዎች ከሌሉ ለአስተዳደር ያሳውቁ (`torii_sorafs_registry_aliases_total` ሳይታሰብ ይወድቃል)።
4. **የሰነድ ውጤት**
   - በSoraFS ኦፕሬሽኖች ምዝግብ ማስታወሻዎች በጊዜ ማህተም እና በተጎዱ አንጸባራቂ ምግቦች ውስጥ የአደጋ ማስታወሻዎችን ይመዝግቡ።
   - አዲስ የብልሽት ሁነታዎች ወይም ዳሽቦርዶች ከገቡ ይህን Runbook ያዘምኑ።

## የልቀት እቅድ

በምርት ውስጥ ተለዋጭ መሸጎጫ ፖሊሲን ሲያነቁ ወይም ሲያጠናክሩ ይህን ደረጃውን የጠበቀ አሰራር ይከተሉ፡1. ** ማዋቀርን አዘጋጁ ***
   - `torii.sorafs_alias_cache` በ `iroha_config` (ተጠቃሚ → ትክክለኛው) በተስማሙት ቲቲኤሎች እና ጸጋ መስኮቶች ያዘምኑ፡ `positive_ttl`፣ `refresh_window`፣ `hard_expiry`፣ Prometheus `rotation_max_age`፣ `successor_grace`፣ እና `governance_grace`። ነባሪዎች በ`docs/source/sorafs_alias_policy.md` ውስጥ ካለው መመሪያ ጋር ይዛመዳሉ።
   - ለኤስዲኬዎች፣ ተመሳሳይ እሴቶችን በማዋቀሪያ ንብርቦቻቸው (`AliasCachePolicy::new(positive, refresh, hard, negative, revocation, rotation, successor, governance)` in Rust/NAPI/ Python bindings) ያሰራጩ ስለዚህ የደንበኛ ማስፈጸሚያ ከመግቢያው ጋር ይዛመዳል።
2. **በደረቅ አሂድ መድረክ**
   - የውቅረት ለውጡን የምርት ቶፖሎጂን ወደሚያሳይ ወደ ማዘጋጃ ክላስተር ያሰማሩ።
   - ቀኖናዊ ተለዋጭ ስሞችን አሁንም ዲኮድ እና የዙር ጉዞን ለማረጋገጥ `cargo xtask sorafs-pin-fixtures` ን ያሂዱ; ማንኛውም አለመዛመድ መጀመሪያ መስተካከል ያለበትን ወደላይ አንጸባራቂ ተንሳፋፊን ያመለክታል።
   - የ`/v2/sorafs/pin/{digest}` እና `/v2/sorafs/aliases` የመጨረሻ ነጥቦችን ትኩስ፣ አድስ-መስኮት፣ ጊዜ ያለፈባቸው እና ጊዜ ያለፈባቸው ጉዳዮችን በሚሸፍኑ ሰው ሠራሽ ማረጋገጫዎች መልመጃ ያድርጉ። የኤችቲቲፒ ሁኔታ ኮዶችን ፣ ራስጌዎችን (`Sora-Proof-Status` ፣ `Retry-After` ፣ `Warning`) እና የJSON የሰውነት ክፍሎችን ከዚህ ሩጫ መጽሐፍ ያረጋግጡ።
3. ** በምርት ውስጥ አንቃ ***
   - አዲሱን ውቅር በመደበኛ የለውጥ መስኮት በኩል ያውጡ። መጀመሪያ ወደ Torii ያመልክቱ፣ ከዚያ መስቀለኛ መንገዱ አዲሱን መመሪያ በምዝግብ ማስታወሻዎች ውስጥ ካረጋገጠ በኋላ ጌትዌይስ/ኤስዲኬ አገልግሎቶችን እንደገና ያስጀምሩ።
   - `docs/source/grafana_sorafs_pin_registry.json`ን ወደ Grafana አስመጣ (ወይም ያሉትን ዳሽቦርዶች አዘምን) እና ተለዋጭ መሸጎጫ ማደሻ ፓነሎችን ከNOC የስራ ቦታ ጋር ይሰኩት።
4. ** ከስራ በኋላ ማረጋገጫ **
   - `torii_sorafs_alias_cache_refresh_total` እና `torii_sorafs_alias_cache_age_seconds` ለ 30 ደቂቃዎች ተቆጣጠር። በ `error`/`expired` ኩርባዎች ውስጥ ያሉ ስፒሎች ከፖሊሲ ማደስ መስኮቶች ጋር መያያዝ አለባቸው። ያልተጠበቀ እድገት ማለት ኦፕሬተሮች ከመቀጠላቸው በፊት ተለዋጭ ማስረጃዎችን እና የጤና አቅራቢዎችን መመርመር አለባቸው።
   - የደንበኛ-ጎን ምዝግብ ማስታወሻዎች ተመሳሳይ የመመሪያ ውሳኔዎችን ያሳያሉ (ማስረጃው ጊዜ ያለፈበት ወይም ጊዜው ሲያበቃ ኤስዲኬዎች ስህተቶችን ያሳያሉ)። የደንበኛ ማስጠንቀቂያዎች አለመኖራቸው የተሳሳተ ውቅረትን ያሳያል።
5. ** መውደቅ ***
   - ተለዋጭ ስም ማውጣት ወደ ኋላ ከቀረ እና የማደስ መስኮቱ ብዙ ጊዜ የሚሄድ ከሆነ `refresh_window` እና `positive_ttl`ን በማዋቀር ፖሊሲውን ለጊዜው ዘና ይበሉ እና እንደገና ይቅጠሩ። `hard_expiry` እንደተጠበቀ ያቆዩት ስለዚህም በእውነት ያረጁ ማስረጃዎች አሁንም ውድቅ ይደረጋሉ።
   - ቴሌሜትሪ ከፍ ያለ የ`error` ቆጠራዎችን ማሳየቱን ከቀጠለ የቀደመውን የ`iroha_config` ቅጽበተ ፎቶን ወደነበረበት በመመለስ ወደ ቀድሞው ውቅር ይመለሱ እና ተለዋጭ ስም ትውልድ መዘግየቶችን ለመፈለግ አንድ ክስተት ይክፈቱ።

## ተዛማጅ ቁሶች

- `docs/source/sorafs/pin_registry_plan.md` - የትግበራ ፍኖተ ካርታ እና የአስተዳደር አውድ።
- `docs/source/sorafs/runbooks/sorafs_node_ops.md` - የማከማቻ ሰራተኛ ስራዎች, ይህንን የመመዝገቢያ መጫወቻ መጽሐፍ ያሟላሉ.
