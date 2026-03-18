---
lang: mn
direction: ltr
source: docs/examples/soranet_testnet_operator_kit/04-telemetry.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: a947c289c13c15b09dfbbf28c23ae1539fd3e29ca3943fa8522c3eca32c28bf5
source_last_modified: "2025-12-29T18:16:35.091070+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Телеметрийн шаардлага

## Prometheus Зорилтот

Реле болон найруулагчийг дараах шошготойгоор хус.

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

## Шаардлагатай хяналтын самбар

1. `dashboards/grafana/soranet_testnet_overview.json` *(хэвлэн нийтлэх)* — JSON-г ачаалах, `region` болон `relay_id` хувьсагчдыг импортлох.
2. `dashboards/grafana/soranet_privacy_metrics.json` *(одоо байгаа SNNet-8 хөрөнгө)* — нууцлалын хувин хавтангууд цоорхойгүй байх ёстой.

## Анхааруулах дүрэм

Босго нь тоглоомын номын хүлээлттэй тохирч байх ёстой:

- `soranet_privacy_circuit_events_total{kind="downgrade"}` ихсэх > 0 10 минутын дотор `critical`-ийг өдөөдөг.
- 30 минут тутамд `sorafs_orchestrator_policy_events_total{outcome="brownout"}` > 5 нь `warning`-ийг өдөөдөг.
- 2 минутын турш `up{job="soranet-relay"}` == 0 нь `critical`-г өдөөдөг.

`testnet-t0` хүлээн авагчтай Alertmanager руу дүрмээ ачаална уу; `amtool check-config` ашиглан баталгаажуулна уу.

## Метрикийн үнэлгээ

14 хоногийн агшин зуурын зургийг нэгтгэж, SNNet-10 баталгаажуулагч руу оруулна уу:

```
cargo xtask soranet-testnet-metrics --input 07-metrics-sample.json --out metrics-report.json
```

- Шууд өгөгдөлтэй ажиллахдаа жишээ файлыг экспортолсон агшин зуурын зургаар солино уу.
- `status = fail` үр дүн нь дэвшихийг хориглодог; Дахин оролдохын өмнө тодруулсан шалгалтыг шийдвэрлэнэ.

## Тайлан

Долоо хоног бүр байршуулах:

- PQ харьцаа, хэлхээний амжилтын хувь, PoW-ийн гистограм шийдлийг харуулсан асуулгын хормын хувилбарууд (`.png` эсвэл `.pdf`).
- `soranet_privacy_throttles_per_minute`-д зориулсан Prometheus бичлэгийн дүрмийн гаралт.
- Гал гарсан аливаа сэрэмжлүүлэг болон нөлөөллийг бууруулах алхмуудыг тайлбарласан товч өгүүлэл (цаг хугацааны тэмдэглэгээ орно).