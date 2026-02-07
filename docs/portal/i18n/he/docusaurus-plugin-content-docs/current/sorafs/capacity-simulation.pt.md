---
lang: he
direction: rtl
source: docs/portal/docs/sorafs/capacity-simulation.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: סימולציית קיבולת
כותרת: Runbook de simulação de capacidade do SoraFS
sidebar_label: Runbook de simulação de capacidade
תיאור: מפעילים או ערכת כלים של סימולציה לעשות את ה-Marketplace de capacidade SF-2c com גופי משוחזרים, ייצוא לעשות Prometheus e לוחות מחוונים לעשות Grafana.
---

:::הערה Fonte canônica
Esta página espelha `docs/source/sorafs/runbooks/sorafs_capacity_simulation.md`. Mantenha ambas as copias sincronizadas.
:::

Este runbook explica como executar or kit de simulação do marketplace de capacidade SF-2c e visualizar as métricas resultantes. Ele valida a negociação de cotas, o tratamento de failover e a remediação de slashing de ponta a ponta usando os fixtures determinísticos em `docs/examples/sorafs_capacity_simulation/`. Os payloads de capacidade ainda usam `sorafs_manifest_stub capacity`; השתמש ב-`iroha app sorafs toolkit pack` עבור os fluxos de empacotamento de manifest/CAR.

## 1. Gerar artefatos de CLI

```bash
cd $REPO_ROOT/docs/examples/sorafs_capacity_simulation
./run_cli.sh ./artifacts
```

Encapsula `run_cli.sh` `sorafs_manifest_stub capacity` עבור מטענים emitir Norito, blobs base64, corpos dequisição para Torii ו-JSON קורות חיים עבור:

- Três declarações de provedores que participam do cenário de negociação de cotas.
- Uma ordem de replicação que aloca o Manifesto em staging entre esses provedores.
- Snapshots de telemetria para a linha de base pré-falha, o intervalo de falha e recuperação de failover.
- אום מטען מחלוקת בקשות לחתוך לאחר סימולדה של פלחה.

Todos os artefatos vão para `./artifacts` (substitua passando um diretório diferente como primeiro argumento). Inspecione os arquivos `_summary.json` עבור ההקשר הרגלי.

## 2. Agregar resultados emitir métricas

```bash
./analyze.py --artifacts ./artifacts
```

הו מוצר אנליסט:

- `capacity_simulation_report.json` - alocações agregadas, deltas de failover e metadados de disputa.
- `capacity_simulation.prom` - מדדי טקסט ל-Prometheus (`sorafs_simulation_*`) adequadas עבור אספן קבצי טקסט לעשות צומת-יצואנית או לגרד עבודה עצמאית.

דוגמה לתצורה של גרידה ל-Prometheus:

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

Aponte of textfile collector para `capacity_simulation.prom` (למשל משתמש-node-exporter, copie para o diretório passado via `--collector.textfile.directory`).

## 3. יבוא לוח המחוונים לעשות Grafana

1. לא Grafana, ייבוא `dashboards/grafana/sorafs_capacity_simulation.json`.
2. Vincule ומגוון לעשות מקור נתונים `Prometheus` או לגרד יעד תצורת יעד.
3. בדוק את הכאב:
   - **הקצאת מכסות (GiB)** mostra os saldos comprometidos/atribuídos de cada provedor.
   - ** Trigger Failover** muda para *Failover Active* quando as métricas de falha chegam.
   - **ירידה בזמן הפעילות במהלך הפסקה** מציגה את ההוכחה של `alpha`.
   - **אחוז נטוי מבוקש** מראה את נקודת המפנה לתיקון.

## 4. Verificações esperadas

- `sorafs_simulation_quota_total_gib{scope="assigned"}` שווה ערך ל-`600` או עמידה כוללת >=600.
- `sorafs_simulation_failover_triggered` reporta `1` e a métrica do provedor substituto destaca `beta`.
- דיווח `sorafs_simulation_slash_requested` `0.15` (15% נטוי) עבור זיהוי מוכיח `alpha`.הפעל את `cargo test -p sorafs_car --features cli --test capacity_simulation_toolkit` עבור מכשירי עזר לאינדה são aceitos pelo esquema da CLI.