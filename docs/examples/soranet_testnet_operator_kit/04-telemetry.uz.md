---
lang: uz
direction: ltr
source: docs/examples/soranet_testnet_operator_kit/04-telemetry.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: a947c289c13c15b09dfbbf28c23ae1539fd3e29ca3943fa8522c3eca32c28bf5
source_last_modified: "2025-12-29T18:16:35.091070+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Telemetriya talablari

## Prometheus Maqsadlar

O'rni va orkestrni quyidagi teglar bilan qirib tashlang:

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

## Kerakli boshqaruv paneli

1. `dashboards/grafana/soranet_testnet_overview.json` *(nashr qilinadi)* — JSONni yuklang, `region` va `relay_id` o‘zgaruvchilarni import qiling.
2. `dashboards/grafana/soranet_privacy_metrics.json` *(mavjud SNNet-8 aktivi)* — maxfiy paqir panellari boʻshliqlarsiz koʻrsatilishini taʼminlang.

## Ogohlantirish qoidalari

Eshiklar o'yin kitobi kutilganiga mos kelishi kerak:

- `soranet_privacy_circuit_events_total{kind="downgrade"}` ortishi > 0 10 daqiqa ichida `critical` ishga tushiriladi.
- `sorafs_orchestrator_policy_events_total{outcome="brownout"}` > 30 daqiqada 5 `warning` tetiklaydi.
- `up{job="soranet-relay"}` == 0 2 daqiqa davomida `critical` ni ishga tushiradi.

Qoidalaringizni `testnet-t0` qabul qiluvchisi bilan Alertmanager-ga yuklang; `amtool check-config` bilan tasdiqlang.

## Ko'rsatkichlarni baholash

14 kunlik suratni jamlang va uni SNNet-10 validatoriga yuboring:

```
cargo xtask soranet-testnet-metrics --input 07-metrics-sample.json --out metrics-report.json
```

- Jonli ma'lumotlarga qarshi ishlayotganda namuna faylini eksport qilingan surat bilan almashtiring.
- `status = fail` natijasi reklamani bloklaydi; qayta urinishdan oldin ta'kidlangan tekshiruv(lar)ni hal qiling.

## Hisobot

Har hafta yuklash:

- PQ nisbati, kontaktlarning zanglashiga olib borish darajasi va PoW gistogrammasini hal qiluvchi so'rov suratlari (`.png` yoki `.pdf`).
- `soranet_privacy_throttles_per_minute` uchun Prometheus yozish qoidasi chiqishi.
- Har qanday ogohlantirishlar va yumshatish bosqichlari (vaqt belgilarini o'z ichiga oladi) tavsiflovchi qisqacha hikoya.