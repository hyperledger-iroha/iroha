---
lang: pt
direction: ltr
source: docs/examples/soranet_testnet_operator_kit/04-telemetry.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: a947c289c13c15b09dfbbf28c23ae1539fd3e29ca3943fa8522c3eca32c28bf5
source_last_modified: "2025-11-04T17:24:14.014911+00:00"
translation_last_reviewed: 2026-01-01
---

# Requisitos de telemetria

## Targets do Prometheus

Faca scrape do relay e do orchestrator com as labels abaixo:

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

## Dashboards requeridos

1. `dashboards/grafana/soranet_testnet_overview.json` *(a publicar)* - carregue o JSON e importe as variaveis `region` e `relay_id`.
2. `dashboards/grafana/soranet_privacy_metrics.json` *(asset SNNet-8 existente)* - garanta que os paineis de privacy bucket renderizem sem lacunas.

## Regras de alerta

Os limites devem corresponder ao esperado no playbook:

- `soranet_privacy_circuit_events_total{kind="downgrade"}` increase > 0 em 10 minutos dispara `critical`.
- `sorafs_orchestrator_policy_events_total{outcome="brownout"}` > 5 por 30 minutos dispara `warning`.
- `up{job="soranet-relay"}` == 0 por 2 minutos dispara `critical`.

Carregue suas regras no Alertmanager com o receiver `testnet-t0`; valide com `amtool check-config`.

## Avaliacao de metricas

Agregue um snapshot de 14 dias e envie ao validador SNNet-10:

```
cargo xtask soranet-testnet-metrics --input 07-metrics-sample.json --out metrics-report.json
```

- Substitua o arquivo sample pelo seu snapshot exportado ao rodar com dados reais.
- Um resultado `status = fail` bloqueia a promocao; resolva os checks destacados antes de tentar novamente.

## Reporte

Toda semana envie:

- Capturas de queries (`.png` ou `.pdf`) mostrando ratio PQ, taxa de sucesso de circuitos e histograma de resolucao PoW.
- Saida da regra de recording do Prometheus para `soranet_privacy_throttles_per_minute`.
- Um breve relato descrevendo alertas disparados e passos de mitigacao (inclua timestamps).
