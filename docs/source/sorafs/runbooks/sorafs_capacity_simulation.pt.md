<!-- Auto-generated stub for Portuguese (pt) translation. Replace this content with the full translation. -->

---
lang: pt
direction: ltr
source: docs/source/sorafs/runbooks/sorafs_capacity_simulation.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: a74e1cb5abc86822ff9d24b9ce42a6567d964cbc01ca4c619b49ca6d239101da
source_last_modified: "2025-11-05T18:02:08.787799+00:00"
translation_last_reviewed: 2025-12-28
---

# Runbook de simulação de capacidade do SoraFS

Este runbook explica como exercitar o toolkit de simulação do marketplace de capacidade SF-2c e visualizar as métricas resultantes. O objetivo é validar a negociação de cotas, o tratamento de failover e a remediação de slashing de ponta a ponta usando os fixtures reproduzíveis em `docs/examples/sorafs_capacity_simulation/`. Os payloads de capacidade ainda usam `sorafs_manifest_stub capacity`; use `iroha app sorafs toolkit pack` para os fluxos de empacotamento de manifest/CAR.

## 1. Gerar artefatos de CLI

```bash
cd $REPO_ROOT/docs/examples/sorafs_capacity_simulation
./run_cli.sh ./artifacts
```

O script invoca `sorafs_manifest_stub capacity` para emitir payloads Norito determinísticos, codificações base64, corpos de requisição para Torii e resumos JSON para:

- Três declarações de provedores que participam do cenário de negociação de cotas.
- Uma ordem de replicação que aloca o manifesto em staging entre os provedores.
- Snapshots de telemetria capturando a linha de base pré-falha, a janela de falha e a recuperação de failover.
- Um payload de disputa solicitando slashing após a falha simulada.

Os artefatos são gravados em `./artifacts` (ou no caminho fornecido como primeiro argumento). Inspecione os arquivos `_summary.json` para estado legível.

## 2. Agregar resultados e emitir métricas

```bash
./analyze.py --artifacts ./artifacts
```

O script de análise produz:

- `capacity_simulation_report.json` — alocações agregadas, deltas de failover e metadados de disputa.
- `capacity_simulation.prom` — métricas de textfile do Prometheus (`sorafs_simulation_*`) adequadas para importação via o textfile collector do node-exporter ou um scrape job independente do Prometheus.

Exemplo de configuração de scrape `prometheus.yml`:

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

Aponte o textfile collector para o `.prom` gerado (para node-exporter, copie para o `--collector.textfile.directory` configurado).

## 3. Importar dashboard do Grafana

1. No Grafana, importe `dashboards/grafana/sorafs_capacity_simulation.json`.
2. Vincule a entrada do datasource `Prometheus` à configuração de scrape acima.
3. Verifique os painéis:
   - **Quota Allocation (GiB)** mostra o saldo comprometido/atribuído de cada provedor.
   - **Failover Trigger** muda para *Failover Active* quando as métricas de falha são carregadas.
   - **Uptime Drop During Outage** apresenta a perda percentual do provedor `alpha`.
   - **Requested Slash Percentage** visualiza a razão de remediação extraída do fixture de disputa.

## 4. Verificações esperadas

- `sorafs_simulation_quota_total_gib{scope="assigned"}` equivale a 600 enquanto o total comprometido permanece ≥600.
- `sorafs_simulation_failover_triggered` reporta `1` e a métrica do provedor substituto destaca `beta`.
- `sorafs_simulation_slash_requested` reporta `0.15` (15% de slash) para o identificador de provedor `alpha`.

Execute `cargo test -p sorafs_car --features cli --test capacity_simulation_toolkit` para confirmar que os fixtures ainda validam com o esquema da CLI.
