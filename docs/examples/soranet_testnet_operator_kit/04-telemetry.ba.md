---
lang: ba
direction: ltr
source: docs/examples/soranet_testnet_operator_kit/04-telemetry.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: a947c289c13c15b09dfbbf28c23ae1539fd3e29ca3943fa8522c3eca32c28bf5
source_last_modified: "2025-12-29T18:16:35.091070+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Телеметрия талаптары

## I18NT000000000X Маҡсаттар

Эстафета һәм оркестрҙы түбәндәге этикеткалар менән ҡырҡырға:

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

## Кәрәкле приборҙар таҡталары

.
.

## Иҫкәртмә ҡағиҙәләре

Сәйәхәттәр пьесаларҙа көтөүгә тап килергә тейеш:

- `soranet_privacy_circuit_events_total{kind="downgrade"}` артыуы > 0 10 минут эсендә `critical` триггерҙары.
- 30 минутҡа `sorafs_orchestrator_policy_events_total{outcome="brownout"}` > 5 `warning` триггерҙары.
- I18NI0000012X == 0 2 минут өсөн I18NI0000013X триггерҙар.

Ҡағиҙәләрегеҙҙе `testnet-t0` приемнигы менән Alertmanager-ға йөкләгеҙ; `amtool check-config` менән раҫлау.

## метрика баһалау

көнлөк снимокты йыйып, уны SNNet-10 валидаторына ашатырға:

```
cargo xtask soranet-testnet-metrics --input 07-metrics-sample.json --out metrics-report.json
```

- Өлгө файлды һеҙҙең экспортланған снимок менән алмаштырырға тура килә, ҡасан ҡаршы йүгерә тере мәғлүмәттәр.
- `status = fail` һөҙөмтә блоктарын пропагандалау; хәл итеү өсөн айырып күрһәтелгән чек(тар) ҡабаттан тырышып.

## Отчет.

Аҙна һайын тейәү:

- Һорау снимоктар (`.png` йәки I18NI000000018X) PQ нисбәте, схема уңыш ставкаһы, һәм PoW гистограмма хәл итеү.
- Prometheus ҡағиҙәһе сығарыу өсөн I18NI000000019X.
- Ҡыҫҡаса хикәйәләү һүрәтләү ниндәй ҙә булһа иҫкәртмәләр, улар ата һәм йомшартыу аҙымдары (ваҡыт маркалары инә).