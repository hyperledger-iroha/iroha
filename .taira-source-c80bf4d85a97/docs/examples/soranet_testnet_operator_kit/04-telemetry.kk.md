---
lang: kk
direction: ltr
source: docs/examples/soranet_testnet_operator_kit/04-telemetry.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: a947c289c13c15b09dfbbf28c23ae1539fd3e29ca3943fa8522c3eca32c28bf5
source_last_modified: "2025-12-29T18:16:35.091070+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Телеметриялық талаптар

## Prometheus Мақсаттар

Реле мен оркестрді келесі белгілермен сүртіңіз:

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

## Міндетті бақылау тақталары

1. `dashboards/grafana/soranet_testnet_overview.json` *(жарияланады)* — JSON жүктеңіз, `region` және `relay_id` айнымалыларын импорттаңыз.
2. `dashboards/grafana/soranet_privacy_metrics.json` *(қолданыстағы SNNet-8 активі)* — құпиялылық шелек панельдерінің бос орындарсыз көрсетілетінін қамтамасыз етіңіз.

## Ескерту ережелері

Шекті мәндер ойын кітапшасының күткеніне сәйкес келуі керек:

- `soranet_privacy_circuit_events_total{kind="downgrade"}` ұлғаюы > 0 10 минут ішінде `critical` іске қосылады.
- `sorafs_orchestrator_policy_events_total{outcome="brownout"}` > 5 30 минутта `warning` іске қосылады.
- `up{job="soranet-relay"}` == 0 2 минут ішінде `critical` іске қосады.

`testnet-t0` қабылдағышымен Alertmanager бағдарламасына ережелерді жүктеңіз; `amtool check-config` арқылы растаңыз.

## Метрикаларды бағалау

14 күндік суретті біріктіріп, оны SNNet-10 валидаторына жіберіңіз:

```
cargo xtask soranet-testnet-metrics --input 07-metrics-sample.json --out metrics-report.json
```

- Тікелей деректермен жұмыс істегенде үлгі файлды экспортталған суретпен ауыстырыңыз.
- `status = fail` нәтижесі жылжытуды блоктайды; қайталаудан бұрын бөлектелген тексерулерді шешіңіз.

## Есеп беру

Әр апта сайын жүктеп салу:

- PQ қатынасын, тізбектің сәттілігін және PoW гистограммасын шешуді көрсететін сұрау суреттері (`.png` немесе `.pdf`).
- `soranet_privacy_throttles_per_minute` үшін Prometheus жазу ережесінің шығысы.
- Іске қосылған кез келген ескертулерді және жеңілдету қадамдарын (уақыт белгілерін қоса) сипаттайтын қысқаша баяндау.