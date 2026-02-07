---
lang: az
direction: ltr
source: docs/examples/soranet_testnet_operator_kit/04-telemetry.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: a947c289c13c15b09dfbbf28c23ae1539fd3e29ca3943fa8522c3eca32c28bf5
source_last_modified: "2025-12-29T18:16:35.091070+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Telemetriya Tələbləri

## Prometheus Hədəflər

Röleyi və orkestratoru aşağıdakı etiketlərlə kazıyın:

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

## Tələb Panellər

1. `dashboards/grafana/soranet_testnet_overview.json` *(dərc olunacaq)* — JSON-u yükləyin, `region` və `relay_id` dəyişənlərini idxal edin.
2. `dashboards/grafana/soranet_privacy_metrics.json` *(mövcud SNNet-8 aktivi)* — məxfilik panellərinin boşluqlar olmadan göstərilməsini təmin edin.

## Xəbərdarlıq Qaydaları

Hədlər oyun kitabının gözləntilərinə uyğun olmalıdır:

- `soranet_privacy_circuit_events_total{kind="downgrade"}` artım > 0 10 dəqiqə ərzində `critical` tetikler.
- `sorafs_orchestrator_policy_events_total{outcome="brownout"}` > 30 dəqiqədə 5 `warning` tetikler.
- 2 dəqiqə üçün `up{job="soranet-relay"}` == 0 `critical` tetikler.

Qaydalarınızı `testnet-t0` qəbuledicisi ilə Alertmanager-ə yükləyin; `amtool check-config` ilə doğrulayın.

## Metriklərin Qiymətləndirilməsi

14 günlük görüntünü toplayın və onu SNNet-10 validatoruna göndərin:

```
cargo xtask soranet-testnet-metrics --input 07-metrics-sample.json --out metrics-report.json
```

- Canlı dataya qarşı işləyərkən nümunə faylı ixrac edilmiş snapşotunuzla əvəz edin.
- `status = fail` nəticəsi irəliləməni bloklayır; yenidən cəhd etməzdən əvvəl vurğulanmış yoxlamaları həll edin.

## Hesabat

Hər həftə yükləmə:

- PQ nisbətini, dövrə müvəffəqiyyət dərəcəsini və PoW-ni göstərən sorğu şəkilləri (`.png` və ya `.pdf`) histoqramı həll edir.
- `soranet_privacy_throttles_per_minute` üçün Prometheus qeyd qaydası çıxışı.
- Yandırılan hər hansı xəbərdarlıqları və təsirin azaldılması addımlarını (zaman möhürləri daxil olmaqla) təsvir edən qısa hekayə.