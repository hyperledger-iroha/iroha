---
lang: am
direction: ltr
source: docs/examples/soranet_testnet_operator_kit/04-telemetry.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: a947c289c13c15b09dfbbf28c23ae1539fd3e29ca3943fa8522c3eca32c28bf5
source_last_modified: "2025-12-29T18:16:35.091070+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# የቴሌሜትሪ መስፈርቶች

## Prometheus ኢላማዎች

ቅብብሎሹን እና ኦርኬስትራውን በሚከተለው መለያዎች ይከርክሙ።

```yaml
- job_name: "soranet-relay"
  static_configs:
    - targets: ["relay-host:9898"]
      labels:
        region: "testnet-t0"
        role: "relay"
- job_name: "sorafs-orchestrator"
  static_configs:
    - targets: ["orchestrator-host:9797"]
      labels:
        region: "testnet-t0"
        role: "orchestrator"
```

## የሚፈለጉ ዳሽቦርዶች

1. `dashboards/grafana/soranet_testnet_overview.json` *(የሚታተም)* - JSON ን ይጫኑ፣ ተለዋዋጮችን `region` እና I18NI0000006X አስመጣ።
2. `dashboards/grafana/soranet_privacy_metrics.json` *(ነባር SNNet-8 ንብረት)* - የግላዊነት ባልዲ ፓነሎች ያለ ክፍተት መስራታቸውን ያረጋግጡ።

## የማንቂያ ህጎች

ገደቦች ከመጫወቻ መጽሐፍ ከሚጠበቀው ጋር መዛመድ አለባቸው፡-

- `soranet_privacy_circuit_events_total{kind="downgrade"}` ጭማሪ> 0 በ 10 ደቂቃ ውስጥ `critical` ቀስቅሴዎች.
- `sorafs_orchestrator_policy_events_total{outcome="brownout"}`> 5 በ 30 ደቂቃ `warning` ቀስቅሴዎች.
- `up{job="soranet-relay"}` == 0 ለ 2 ደቂቃዎች ቀስቅሴዎች `critical`.

ደንቦችዎን በ `testnet-t0` መቀበያ ወደ Alertmanager ይጫኑ; በ `amtool check-config` ያረጋግጡ።

## የመለኪያዎች ግምገማ

የ14-ቀን ቅጽበታዊ ገጽ እይታን ያዋህዱ እና ለ SNNet-10 አረጋጋጭ ይስጡት፡-

```
cargo xtask soranet-testnet-metrics --input 07-metrics-sample.json --out metrics-report.json
```

- ከቀጥታ ውሂብ ጋር ሲወዳደር የናሙና ፋይሉን ወደ ውጭ በተላከው ቅጽበታዊ ገጽ እይታ ይተኩ።
- የ `status = fail` ውጤት ማስተዋወቅን ያግዳል; በድጋሚ ከመሞከርዎ በፊት የደመቁትን ቼኮች ይፍቱ።

## ሪፖርት ማድረግ

በየሳምንቱ ጭነት፡-

- የመጠይቅ ቅጽበተ-ፎቶዎች (`.png` ወይም `.pdf`) PQ ሬሾን፣ የወረዳ ስኬት መጠንን እና የPoW መፍታት ሂስቶግራምን ያሳያሉ።
- Prometheus የመቅጃ ደንብ ውፅዓት ለ I18NI0000019X።
- የተኩስ እና የመቀነስ እርምጃዎችን (የጊዜ ማህተሞችን ጨምሮ) ማንቂያዎችን የሚገልጽ አጭር ትረካ።