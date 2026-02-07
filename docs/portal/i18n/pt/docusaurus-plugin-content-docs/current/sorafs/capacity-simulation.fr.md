---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/capacity-simulation.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: simulação de capacidade
título: Runbook de simulação de capacidade SoraFS
sidebar_label: Runbook de simulação de capacidade
descrição: Execute o kit de simulação do mercado de capacidade SF-2c com luminárias reproduzíveis, exportações Prometheus e tabelas de borda Grafana.
---

:::nota Fonte canônica
Esta página reflete `docs/source/sorafs/runbooks/sorafs_capacity_simulation.md`. Gardez as duas cópias sincronizadas junto com o conjunto de documentação Sphinx herité totalmente migrado.
:::

Este runbook explícito comenta executar o kit de simulação do mercado de capacidade SF-2c e visualizar as métricas resultantes. Ele valida a negociação de cotas, o gerenciamento de failover e a recuperação do corte de combate em combate com auxílio de luminárias determinadas em `docs/examples/sorafs_capacity_simulation/`. As cargas úteis de capacidade são usadas sempre `sorafs_manifest_stub capacity`; use `iroha app sorafs toolkit pack` para o fluxo de embalagem manifest/CAR.

## 1. Gerar artefatos CLI

```bash
cd $REPO_ROOT/docs/examples/sorafs_capacity_simulation
./run_cli.sh ./artifacts
```

`run_cli.sh` encapsula `sorafs_manifest_stub capacity` para enviar cargas úteis Norito, blobs base64, corpo de solicitação Torii e currículos JSON para:

- Três declarações de fornecedores participantes do cenário de negociação de cotas.
- Uma ordem de replicação que permite o manifesto na encenação entre esses fornecedores.
- Instantâneos de telefonia para a linha de base pré-pane, intervalo de pane e recuperação de failover.
- Uma carga útil de litígio exigia um corte após o painel simultâneo.

Todos os artefatos foram depositados sob `./artifacts` (substituem em passagem um outro repertório em primeiro argumento). Inspecione os arquivos `_summary.json` para um contexto legível.

## 2. Obtenha os resultados e obtenha as métricas

```bash
./analyze.py --artifacts ./artifacts
```

O produto do analisador:

- `capacity_simulation_report.json` - alocações aprovadas, deltas de failover e metas de litígio.
- `capacity_simulation.prom` - métricas de arquivo de texto Prometheus (`sorafs_simulation_*`) adaptadas ao coletor de arquivo de texto do exportador de nó ou a um trabalho de raspagem independente.

Exemplo de configuração de scrape Prometheus:

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

Direcione o coletor de arquivo de texto para `capacity_simulation.prom` (se você usar o node-exporter, copie-o no repertório passado via `--collector.textfile.directory`).

## 3. Importe o painel Grafana

1. Em Grafana, importe `dashboards/grafana/sorafs_capacity_simulation.json`.
2. Associe a variável de fonte de dados `Prometheus` ao código de raspagem configurado aqui.
3. Verifique os painéis:
   - **Alocação de cota (GiB)** exibe os soldados contratados/atribuídos para cada fornecedor.
   - **Failover Trigger** bascula em *Failover Active* quando as métricas de painel chegam.
   - **Queda de tempo de atividade durante interrupção** rastreie a perda em porcentagem para o fornecedor `alpha`.
   - **Porcentagem de barra solicitada** visualize a proporção de remédiation extrait du fixture de litige.

## 4. Participantes de verificações- `sorafs_simulation_quota_total_gib{scope="assigned"}` é igual a `600` desde que o total ocupado reste >=600.
- `sorafs_simulation_failover_triggered` indica `1` e a métrica do fornecedor de substituição encontrada antes de `beta`.
- `sorafs_simulation_slash_requested` indique `0.15` (15% de barra) para o identificador do fornecedor `alpha`.

Execute `cargo test -p sorafs_car --features cli --test capacity_simulation_toolkit` para confirmar se os fixtures ainda são aceitos pelo esquema CLI.