---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/capacity-simulation.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: simulação de capacidade
título: Ранбук симуляции емкости SoraFS
sidebar_label: Ранбук симуляции емкости
description: Simulando a simulação do SF-2c com a física desejada, o transporte Prometheus e o hardware Grafana.
---

:::nota História Canônica
Esta página está configurada para `docs/source/sorafs/runbooks/sorafs_capacity_simulation.md`. Faça uma cópia da sincronização, mas o Sphinx não será atualizado.
:::

Este é um problema que permite que você execute o simulador SF-2c e visualize a métrica de desempenho. Para testar o desempenho do seu computador, execute o failover e remedie o corte de ponta a ponta, use as configurações de definição `docs/examples/sorafs_capacity_simulation/`. Cargas úteis enviadas por meio de `sorafs_manifest_stub capacity`; use `iroha app sorafs toolkit pack` para exibir o manifesto/CAR.

## 1. Gerenciador de arquivos CLI

```bash
cd $REPO_ROOT/docs/examples/sorafs_capacity_simulation
./run_cli.sh ./artifacts
```

`run_cli.sh` оборачивает `sorafs_manifest_stub capacity`, чтобы выпускать Norito payloads, base64-блоб, тела запросов Torii и JSON para:

- Três declarações demonstrativas que você pode usar no cenário para passar pelo carro.
- Одного распоряжения о репликации, распределяющего staged-манифест между провайдерами.
- Снимков телеметрии para a linha de base do сбоя, interface сбоя e recuperação de failover.
- Payload спора с запросом на slashing после смоделированного сбоя.

Seus artefactos são colocados em `./artifacts` (pode ser alterado, verifique o catálogo do arquivo). Verifique os arquivos `_summary.json` para o contato direto.

## 2. Agilizar resultados e métricas de verificação

```bash
./analyze.py --artifacts ./artifacts
```

Formulário Analisador:

- `capacity_simulation_report.json` - capacidade de atualização, failover de falha e esporão de metadação.
- `capacity_simulation.prom` - métrica textfile Prometheus (`sorafs_simulation_*`), usado para coletor de arquivo de texto, nó-exportador ou trabalho de raspagem.

Exemplo de configuração raspar Prometheus:

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

Selecione o coletor de arquivo de texto em `capacity_simulation.prom` (o exportador de nó exportado o coleta no catálogo, fornecendo o `--collector.textfile.directory`).

## 3. Importe o pacote Grafana

1. Em Grafana importa `dashboards/grafana/sorafs_capacity_simulation.json`.
2. Use a fonte de dados `Prometheus` para removê-la do zero.
3. Painel de exibição:
   - **Alocação de cota (GiB)** permite equilibrar commit/assign para o provedor.
   - **Failover Trigger** funciona em *Failover Active*, que é a métrica gerada.
   - **Queda de tempo de atividade durante interrupção** отображает процент потери для провайдера `alpha`.
   - **Porcentagem de barra solicitada** visualize a correção da correção do jogo de ficção.

## 4. Ожидаемые проверки

- `sorafs_simulation_quota_total_gib{scope="assigned"}` é `600`, para que o commit seja >=600.
- `sorafs_simulation_failover_triggered` показывает `1`, uma métrica que prova o valor de `beta`.
- `sorafs_simulation_slash_requested` selecione `0.15` (barra de 15%) para identificar o testador `alpha`.

Abra `cargo test -p sorafs_car --features cli --test capacity_simulation_toolkit`, isso pode ser feito, essas configurações estão disponíveis na CLI.