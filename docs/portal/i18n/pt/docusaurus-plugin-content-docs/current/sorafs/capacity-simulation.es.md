---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/capacity-simulation.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: simulação de capacidade
título: Runbook de simulação de capacidade de SoraFS
sidebar_label: Runbook de simulação de capacidade
descrição: Execute o kit de ferramentas de simulação do mercado de capacidade SF-2c com luminárias reproduzíveis, exportações de Prometheus e painéis de controle de Grafana.
---

:::nota Fonte canônica
Esta página reflete `docs/source/sorafs/runbooks/sorafs_capacity_simulation.md`. Mantenha ambas as cópias sincronizadas até que o conjunto de documentação herdada no Sphinx seja migrado por completo.
:::

Este runbook explica como executar o kit de simulação do mercado de capacidade SF-2c e visualizar as métricas resultantes. Valide a negociação de cotas, o gerenciamento de failover e a remediação de corte de extremo a extremo usando os fixtures deterministas em `docs/examples/sorafs_capacity_simulation/`. As cargas úteis de capacidade também usam `sorafs_manifest_stub capacity`; usa `iroha app sorafs toolkit pack` para fluxos de pacote de manifesto/CAR.

## 1. Gerar artefatos de CLI

```bash
cd $REPO_ROOT/docs/examples/sorafs_capacity_simulation
./run_cli.sh ./artifacts
```

`run_cli.sh` envia `sorafs_manifest_stub capacity` para emitir payloads Norito, blobs base64, campos de solicitação para Torii e resumos JSON para:

- Três declarações de provedores que participam do cenário de negociação de cotas.
- Uma ordem de replicação que atribui o manifesto na encenação entre esses provedores.
- Instantâneos de telemetria para a linha de base anterior à queda, o intervalo de queda e a recuperação por failover.
- Uma carga útil de disputa solicitando corte após a queda simulada.

Todos os artefatos estão escritos abaixo de `./artifacts` (pode substituí-lo passando por um diretório diferente do primeiro argumento). Inspecione os arquivos `_summary.json` para contexto legível.

## 2. Agregar resultados e discutir considerações

```bash
./analyze.py --artifacts ./artifacts
```

O analisador produz:

- `capacity_simulation_report.json` - atribuições agregadas, deltas de failover e metadados de disputa.
- `capacity_simulation.prom` - Estatísticas de arquivo de texto de Prometheus (`sorafs_simulation_*`) adequadas para o coletor de arquivo de texto do exportador de nó ou um trabalho de raspagem independente.

Exemplo de configuração de scrape de Prometheus:

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

Insira o coletor de arquivo de texto em `capacity_simulation.prom` (se usar o node-exporter, copie-o no diretório passado via `--collector.textfile.directory`).

## 3. Importar o painel de Grafana

1. Em Grafana, importa `dashboards/grafana/sorafs_capacity_simulation.json`.
2. Vincule a variável da fonte de dados `Prometheus` ao destino de raspagem configurado acima.
3. Verifique os painéis:
   - **Quota Allocation (GiB)** mostra os saldos comprometidos/atribuídos de cada fornecedor.
   - **Failover Trigger** muda para *Failover Active* quando as métricas de queda são inseridas.
   - **Uptime Drop While Outage** gráfico de perda percentual para o fornecedor `alpha`.
   - **Porcentagem de barra solicitada** visualize a proporção de remediação extraída do dispositivo de disputa.

## 4. Comprovações esperadas- `sorafs_simulation_quota_total_gib{scope="assigned"}` equivale a `600` enquanto o total comprometido se mantém >=600.
- `sorafs_simulation_failover_triggered` reporta `1` e a métrica do provedor de reposição de realejo `beta`.
- `sorafs_simulation_slash_requested` reporta `0.15` (15% de barra) para o identificador de provedor `alpha`.

Execute `cargo test -p sorafs_car --features cli --test capacity_simulation_toolkit` para confirmar que os fixtures continuam sendo aceitos pelo esquema da CLI.