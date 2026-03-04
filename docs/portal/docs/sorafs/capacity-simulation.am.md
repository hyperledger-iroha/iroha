---
lang: am
direction: ltr
source: docs/portal/docs/sorafs/capacity-simulation.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: de9c2a162d97ab896e51af36054e0f3342522b241bfba3ded9c1ec764e590159
source_last_modified: "2026-01-22T14:35:36.797990+00:00"
translation_last_reviewed: 2026-02-07
id: capacity-simulation
title: SoraFS Capacity Simulation Runbook
sidebar_label: Capacity Simulation Runbook
description: Exercising the SF-2c capacity marketplace simulation toolkit with reproducible fixtures, Prometheus exports, and Grafana dashboards.
translator: machine-google-reviewed
---

::: ማስታወሻ ቀኖናዊ ምንጭ
::

ይህ Runbook የ SF-2c አቅም የገበያ ቦታ ማስመሰያ ኪት እንዴት እንደሚሰራ ያብራራል እና የተገኙትን መለኪያዎች በዓይነ ሕሊናዎ ይሳሉ። በ`docs/examples/sorafs_capacity_simulation/` ውስጥ ያሉትን የመወሰኛ ዕቃዎች በመጠቀም የኮታ ድርድርን፣ ያልተሳካ አያያዝን እና የማስተካከያ ከጫፍ እስከ ጫፍ መቁረጥን ያረጋግጣል። የአቅም መጫዎቻዎች አሁንም `sorafs_manifest_stub capacity` ይጠቀማሉ; ለማንፀባረቅ/CAR ማሸጊያ ፍሰቶች `iroha app sorafs toolkit pack` ይጠቀሙ።

## 1. CLI ቅርሶችን መፍጠር

```bash
cd $REPO_ROOT/docs/examples/sorafs_capacity_simulation
./run_cli.sh ./artifacts
```

`run_cli.sh` `sorafs_manifest_stub capacity` ይጠቀልላል Norito ክፍያ ጭነቶች፣ base64 blobs፣ Torii የጥያቄ አካላት እና JSON ማጠቃለያዎች ለ፡

- በኮታ ድርድር ሁኔታ ውስጥ የሚሳተፉ ሶስት የአቅራቢዎች መግለጫዎች።
- የማባዛት ትእዛዝ በነዚያ አቅራቢዎች ላይ የተዘጋጀውን አንጸባራቂ የሚመደብ።
- የቴሌሜትሪ ቅጽበታዊ ገጽ እይታዎች ለቅድመ-ውጪ መነሻ መስመር፣ የመቋረጡ ክፍተት እና ያልተሳካ ማገገም።
- ከተመሳሰለው መቋረጥ በኋላ መቆራረጥን የሚጠይቅ የሙግት ጭነት።

ሁሉም ቅርሶች በ I18NI0000015X (የተለየ ማውጫን እንደ መጀመሪያው ክርክር በማለፍ ይሽሩት)። ሰው ሊነበብ ለሚችል አውድ የ`_summary.json` ፋይሎችን መርምር።

## 2. አጠቃላይ ውጤቶችን እና ልቀት መለኪያዎች

```bash
./analyze.py --artifacts ./artifacts
```

ተንታኙ የሚከተሉትን ያዘጋጃል-

- `capacity_simulation_report.json` - የተዋሃዱ ምደባዎች፣ ያልተሳካላቸው ዴልታዎች እና ሙግት ሜታዳታ።
- `capacity_simulation.prom` - Prometheus textfile metrics (`sorafs_simulation_*`) ለኖድ ላኪ የጨርቃጨርቅ ሰብሳቢ ወይም ራሱን የቻለ የመቧጨር ሥራ።

ምሳሌ Prometheus የመቧጨርቅ ውቅር፡

```yaml
scrape_configs:
  - job_name: sorafs-capacity-sim
    scrape_interval: 15s
    static_configs:
      - targets: ["localhost:9100"]
        labels:
          scenario: "capacity-sim"
    metrics_path: /metrics
    params:
      format: ["prometheus"]
```

የጽሑፍ ፋይል ሰብሳቢውን በ `capacity_simulation.prom` (node-exporter ሲጠቀሙ በ `--collector.textfile.directory` ወደ ተላለፈው ማውጫ ይቅዱት) ያመልክቱ።

## 3. Grafana ዳሽቦርዱን አስመጣ

1. በ I18NT0000004X, `dashboards/grafana/sorafs_capacity_simulation.json` አስመጣ.
2. የI18NI0000023X የውሂብ ምንጭ ተለዋዋጭን ከላይ ከተዋቀረው የቧጨራ ዒላማ ጋር ያስሩ።
3. ፓነሎችን ያረጋግጡ:
   - **የኮታ ድልድል (ጊቢ)** ለእያንዳንዱ አገልግሎት አቅራቢ የተሰጡ/የተመደቡ ሂሳቦችን ያሳያል።
   - ** ያልተሳካ ቀስቅሴ *** ወደ * ያልተሳካ ገቢር * የሚቀያየር የመውረጃ መለኪያዎች ወደ ውስጥ ሲገቡ።
   - **በአገልግሎት ጊዜ መቋረጡ** የአቅራቢውን `alpha` መቶኛ ኪሳራ ይገልፃል።
   - **የተጠየቀው Slash ፐርሰንት** ከክርክር አፕሊኬሽኑ የወጣውን የማስተካከያ ሬሾን በእይታ ያሳያል።

## 4. የሚጠበቁ ቼኮች

- `sorafs_simulation_quota_total_gib{scope="assigned"}` ከ `600` ጋር እኩል ሲሆን የተፈፀመው ድምር ይቀራል >= 600።
- `sorafs_simulation_failover_triggered` ሪፖርቶች `1` እና የምትክ አቅራቢው ሜትሪክ ድምቀቶች `beta`.
- `sorafs_simulation_slash_requested` ሪፖርት `0.15` (15% slash) ለI18NI0000032X አቅራቢ መለያ።

መጫዎቻዎቹ አሁንም በCLI ዕቅዱ ተቀባይነት እንዳላቸው ለማረጋገጥ `cargo test -p sorafs_car --features cli --test capacity_simulation_toolkit` ን ያሂዱ።