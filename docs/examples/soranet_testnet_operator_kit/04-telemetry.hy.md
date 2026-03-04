---
lang: hy
direction: ltr
source: docs/examples/soranet_testnet_operator_kit/04-telemetry.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: a947c289c13c15b09dfbbf28c23ae1539fd3e29ca3943fa8522c3eca32c28bf5
source_last_modified: "2025-12-29T18:16:35.091070+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Հեռուստաչափության պահանջներ

## Prometheus Թիրախներ

Քերեք ռելեին և նվագախմբին հետևյալ պիտակներով.

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

## Պահանջվող վահանակներ

1. `dashboards/grafana/soranet_testnet_overview.json` *(հրապարակվելու է)* — բեռնել JSON-ը, ներմուծել `region` և `relay_id` փոփոխականները:
2. `dashboards/grafana/soranet_privacy_metrics.json` *(առկա SNNet-8 ակտիվ)* — ապահովել, որ գաղտնիության շերեփի վահանակները մատուցվեն առանց բացերի:

## Զգուշացման կանոններ

Շեմերը պետք է համապատասխանեն խաղագրքի ակնկալիքներին.

- `soranet_privacy_circuit_events_total{kind="downgrade"}` աճը > 0 10 րոպեի ընթացքում գործարկում է `critical`:
- `sorafs_orchestrator_policy_events_total{outcome="brownout"}` > 5 30 րոպեի ընթացքում գործարկում է `warning`:
- `up{job="soranet-relay"}` == 0 2 րոպեի ընթացքում գործարկում է `critical`:

Բեռնեք ձեր կանոնները Alertmanager-ում `testnet-t0` ընդունիչով; վավերացնել `amtool check-config`-ով:

## Չափումների գնահատում

Հավաքեք 14-օրյա պատկերը և այն փոխանցեք SNNet-10 վավերացնողին.

```
cargo xtask soranet-testnet-metrics --input 07-metrics-sample.json --out metrics-report.json
```

- Փոխարինեք նմուշ ֆայլը ձեր արտահանված նկարով, երբ աշխատում է կենդանի տվյալների դեմ:
- `status = fail` արդյունքը արգելափակում է առաջխաղացումը; լուծեք ընդգծված չեկ(ները) նախքան նորից փորձելը:

## Հաշվետվություն

Ամեն շաբաթ վերբեռնում.

- Հարցման պատկերներ (`.png` կամ `.pdf`), որոնք ցույց են տալիս PQ հարաբերակցությունը, շղթայի հաջողության մակարդակը և PoW լուծման հիստոգրամը:
- Prometheus ձայնագրման կանոնի ելք `soranet_privacy_throttles_per_minute`-ի համար:
- Համառոտ պատմություն, որը նկարագրում է արձակված ցանկացած ահազանգ և մեղմացման քայլեր (ներառում է ժամանակի դրոշմանիշները):