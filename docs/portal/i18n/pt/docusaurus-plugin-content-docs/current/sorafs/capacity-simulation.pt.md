---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/capacity-simulation.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: simulação de capacidade
título: Runbook de simulação de capacidade do SoraFS
sidebar_label: Runbook de simulação de capacidade
description: Executar o toolkit de simulação do marketplace de capacidade SF-2c com fixtures reproduzíveis, exportações do Prometheus e dashboards do Grafana.
---

:::nota Fonte canônica
Esta página espelha `docs/source/sorafs/runbooks/sorafs_capacity_simulation.md`. Mantenha ambas as cópias sincronizadas.
:::

Este runbook explica como executar o kit de simulação do mercado de capacidade SF-2c e visualizar as métricas resultantes. Ele valida a negociação de cotas, o tratamento de failover e a remediação de corte de ponta a ponta usando os fixtures determinísticos em `docs/examples/sorafs_capacity_simulation/`. As cargas úteis de capacidade ainda usam `sorafs_manifest_stub capacity`; use `iroha app sorafs toolkit pack` para os fluxos de empacotamento de manifest/CAR.

## 1. Gerar artefatos de CLI

```bash
cd $REPO_ROOT/docs/examples/sorafs_capacity_simulation
./run_cli.sh ./artifacts
```

`run_cli.sh` encapsula `sorafs_manifest_stub capacity` para emitir payloads Norito, blobs base64, corpos de requisição para Torii e resumos JSON para:

- Três declarações de fornecedores que participam do cenário de negociação de cotas.
- Uma ordem de replicação que aloca o manifesto em encenação entre esses provedores.
- Snapshots de telemetria para a linha de base pré-falha, o intervalo de falha e a recuperação de failover.
- Um payload de disputa solicitando slashing após uma falha simulada.

Todos os artistas vão para `./artifacts` (substituindo um diretório diferente como primeiro argumento). Inspecione os arquivos `_summary.json` para contexto legível.

## 2. Agregar resultados e opiniões

```bash
./analyze.py --artifacts ./artifacts
```

O relatório produzido:

- `capacity_simulation_report.json` - alocações agregadas, deltas de failover e metadados de disputa.
- `capacity_simulation.prom` - especificações de textfile do Prometheus (`sorafs_simulation_*`) adequadas para o coletor de textfile do node-exporter ou um scrape job independente.

Exemplo de configuração de scrape do Prometheus:

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

Aponte o coletor de arquivo de texto para `capacity_simulation.prom` (ao usar o node-exporter, copie para o diretório passado via `--collector.textfile.directory`).

## 3. Importar o painel do Grafana

1. Não Grafana, importe `dashboards/grafana/sorafs_capacity_simulation.json`.
2. Vincule a variável do datasource `Prometheus` ao scrape target configurado acima.
3. Verifique os painéis:
   - **Quota Allocation (GiB)** mostra os saldos comprometidos/atribuídos de cada provedor.
   - **Failover Trigger** muda para *Failover Active* quando as análises de falhas chegam.
   - **Queda de tempo de atividade durante interrupção** apresenta a porcentagem percentual do provedor `alpha`.
   - **Porcentagem de barra solicitada** visualize a razão de remediação extraída do fixture de disputa.

## 4. Verificações esperadas

- `sorafs_simulation_quota_total_gib{scope="assigned"}` equivale a `600` enquanto o total permanece >=600.
- `sorafs_simulation_failover_triggered` reporta `1` e a métrica do provedor substituto destaca `beta`.
- `sorafs_simulation_slash_requested` reporta `0.15` (15% de barra) para o identificador do provedor `alpha`.Execute `cargo test -p sorafs_car --features cli --test capacity_simulation_toolkit` para confirmar que os fixtures ainda são aceitos pelo esquema da CLI.