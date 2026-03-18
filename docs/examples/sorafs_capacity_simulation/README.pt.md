---
lang: pt
direction: ltr
source: docs/examples/sorafs_capacity_simulation/README.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 727a648141405b0c8f12a131ff903d3e7ce5b74a7f899dd99fe9aa6490b55ef2
source_last_modified: "2025-11-05T17:59:15.481814+00:00"
translation_last_reviewed: 2026-01-01
---

# Toolkit de simulacao de capacidade SoraFS

Este diretorio entrega os artefatos reproduziveis para a simulacao do mercado de capacidade SF-2c. O toolkit exercita negociacao de quotas, tratamento de failover e remediacao de slashing usando os helpers de CLI de producao e um script de analise leve.

## Prerequisitos

- Toolchain Rust capaz de executar `cargo run` para membros do workspace.
- Python 3.10+ (somente biblioteca padrao).

## Inicio rapido

```bash
# 1. Gerar artefatos CLI canonicos
./run_cli.sh ./artifacts

# 2. Agregar os resultados e emitir metricas Prometheus
./analyze.py --artifacts ./artifacts
```

O script `run_cli.sh` invoca `sorafs_manifest_stub capacity` para construir:

- Declaracoes deterministas de providers para o set de fixtures de negociacao de quotas.
- Uma ordem de replicacao que corresponde ao cenario de negociacao.
- Snapshots de telemetria para a janela de failover.
- Um payload de disputa capturando o pedido de slashing.

O script escreve bytes Norito (`*.to`), payloads base64 (`*.b64`), corpos de requisicao
Torii e resumos legiveis (`*_summary.json`) no diretorio de artefatos escolhido.

`analyze.py` consome os resumos gerados, produz um relatorio agregado
(`capacity_simulation_report.json`) e emite um textfile Prometheus
(`capacity_simulation.prom`) contendo:

- Gauges `sorafs_simulation_quota_*` descrevendo capacidade negociada e share de alocacao
  por provider.
- Gauges `sorafs_simulation_failover_*` destacando deltas de downtime e o provider
  de substituicao selecionado.
- `sorafs_simulation_slash_requested` registrando a porcentagem de remediacao extraida
  do payload de disputa.

Importe o bundle Grafana em `dashboards/grafana/sorafs_capacity_simulation.json`
e aponte para um datasource Prometheus que scrape o textfile gerado (por exemplo
via o textfile collector do node-exporter). O runbook em
`docs/source/sorafs/runbooks/sorafs_capacity_simulation.md` descreve o fluxo completo,
incluindo dicas de configuracao do Prometheus.

## Fixtures

- `scenarios/quota_negotiation/` - Specs de declaracao de provider e ordem de replicacao.
- `scenarios/failover/` - Janelas de telemetria para a queda primaria e o lift de failover.
- `scenarios/slashing/` - Spec de disputa referenciando a mesma ordem de replicacao.

Esses fixtures sao validados em `crates/sorafs_car/tests/capacity_simulation_toolkit.rs`
para garantir que fiquem em sync com o schema da CLI.
