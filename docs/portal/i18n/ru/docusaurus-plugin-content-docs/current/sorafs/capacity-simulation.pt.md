---
lang: ru
direction: ltr
source: docs/portal/docs/sorafs/capacity-simulation.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

---
id: capacity-simulation
title: Runbook de simulação de capacidade do SoraFS
sidebar_label: Runbook de simulação de capacidade
description: Executar o toolkit de simulação do marketplace de capacidade SF-2c com fixtures reproduzíveis, exports do Prometheus e dashboards do Grafana.
---

:::note Fonte canônica
Esta página espelha `docs/source/sorafs/runbooks/sorafs_capacity_simulation.md`. Mantenha ambas as copias sincronizadas.
:::

Este runbook explica como executar o kit de simulação do marketplace de capacidade SF-2c e visualizar as métricas resultantes. Ele valida a negociação de cotas, o tratamento de failover e a remediação de slashing de ponta a ponta usando os fixtures determinísticos em `docs/examples/sorafs_capacity_simulation/`. Os payloads de capacidade ainda usam `sorafs_manifest_stub capacity`; use `iroha app sorafs toolkit pack` para os fluxos de empacotamento de manifest/CAR.

## 1. Gerar artefatos de CLI

```bash
cd $REPO_ROOT/docs/examples/sorafs_capacity_simulation
./run_cli.sh ./artifacts
```

`run_cli.sh` encapsula `sorafs_manifest_stub capacity` para emitir payloads Norito, blobs base64, corpos de requisição para Torii e resumos JSON para:

- Três declarações de provedores que participam do cenário de negociação de cotas.
- Uma ordem de replicação que aloca o manifesto em staging entre esses provedores.
- Snapshots de telemetria para a linha de base pré-falha, o intervalo de falha e a recuperação de failover.
- Um payload de disputa solicitando slashing após a falha simulada.

Todos os artefatos vão para `./artifacts` (substitua passando um diretório diferente como primeiro argumento). Inspecione os arquivos `_summary.json` para contexto legível.

## 2. Agregar resultados e emitir métricas

```bash
./analyze.py --artifacts ./artifacts
```

O analisador produz:

- `capacity_simulation_report.json` - alocações agregadas, deltas de failover e metadados de disputa.
- `capacity_simulation.prom` - métricas de textfile do Prometheus (`sorafs_simulation_*`) adequadas para o textfile collector do node-exporter ou um scrape job independente.

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

Aponte o textfile collector para `capacity_simulation.prom` (ao usar node-exporter, copie para o diretório passado via `--collector.textfile.directory`).

## 3. Importar o dashboard do Grafana

1. No Grafana, importe `dashboards/grafana/sorafs_capacity_simulation.json`.
2. Vincule a variável do datasource `Prometheus` ao scrape target configurado acima.
3. Verifique os painéis:
   - **Quota Allocation (GiB)** mostra os saldos comprometidos/atribuídos de cada provedor.
   - **Failover Trigger** muda para *Failover Active* quando as métricas de falha chegam.
   - **Uptime Drop During Outage** apresenta a perda percentual do provedor `alpha`.
   - **Requested Slash Percentage** visualiza a razão de remediação extraída do fixture de disputa.

## 4. Verificações esperadas

- `sorafs_simulation_quota_total_gib{scope="assigned"}` equivale a `600` enquanto o total comprometido permanece >=600.
- `sorafs_simulation_failover_triggered` reporta `1` e a métrica do provedor substituto destaca `beta`.
- `sorafs_simulation_slash_requested` reporta `0.15` (15% de slash) para o identificador de provedor `alpha`.

Execute `cargo test -p sorafs_car --features cli --test capacity_simulation_toolkit` para confirmar que os fixtures ainda são aceitos pelo esquema da CLI.
