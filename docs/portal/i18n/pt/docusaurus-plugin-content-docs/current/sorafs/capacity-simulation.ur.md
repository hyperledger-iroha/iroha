---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/capacity-simulation.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: simulação de capacidade
título: SoraFS کپیسٹی سمیولیشن رَن بُک
sidebar_label: کپیسٹی سمیولیشن رَن بُک
description: luminárias reproduzíveis, exportações Prometheus, e painéis Grafana کے ساتھ SF-2c کپیسٹی مارکیٹ پلیس سمیولیشن ٹول کٹ چلانا۔
---

:::nota ماخذِ مستند
یہ صفحہ `docs/source/sorafs/runbooks/sorafs_capacity_simulation.md` کی آئینہ دار ہے۔ جب تک پرانا Esfinge دستاویزی مجموعہ مکمل طور پر منتقل نہیں ہو جاتا دونوں نقول کو ہم آہنگ رکھیں۔
:::

یہ رن بُک وضاحت کرتی ہے کہ SF-2c کپیسٹی مارکیٹ پلیس سمیولیشن کِٹ کیسے چلائیں Qual é a melhor opção para você یہ `docs/examples/sorafs_capacity_simulation/` میں موجود luminárias determinísticas کے ذریعے quota مذاکرات, failover ہینڈلنگ اور slashing remediation کو ponta a ponta ویلیڈیٹ کرتی ہے۔ Cargas úteis کپیسٹی اب بھی `sorafs_manifest_stub capacity` استعمال کرتے ہیں؛ manifesto/CAR پیکجنگ کے لیے `iroha app sorafs toolkit pack` استعمال کریں۔

## 1. CLI آرٹی فیکٹس تیار کریں

```bash
cd $REPO_ROOT/docs/examples/sorafs_capacity_simulation
./run_cli.sh ./artifacts
```

`run_cli.sh`, `sorafs_manifest_stub capacity` کو لپیٹ کر Cargas úteis Norito, blobs base64, corpos de solicitação Torii e resumos JSON تیار کرتا ہے برائے:

- cota مذاکرات والے منظرنامے میں شریک تین provedores کی declarações۔
- ایک ordem de replicação جو manifesto encenado کو provedores میں تقسیم کرتا ہے۔
- linha de base pré-interrupção, intervalo de interrupção, recuperação de failover e instantâneos de telemetria۔
- interrupção simulada کے بعد corte درخواست کرنے والا carga útil de disputa۔

تمام آرٹی فیکٹس `./artifacts` کے تحت جمع ہوتے ہیں (پہلے آرگومنٹ میں مختلف ڈائریکٹری دے کر اووررائیڈ کر سکتے ہیں)۔ انسانی سمجھ کے لیے `_summary.json` فائلیں دیکھیں۔

## 2. نتائج جمع کریں اور métricas جاری کریں

```bash
./analyze.py --artifacts ./artifacts
```

اینالائزر تیار کرتا ہے:

- `capacity_simulation_report.json` - Alocações de recursos, deltas de failover e metadados de disputa
- `capacity_simulation.prom` - Prometheus métricas de arquivo de texto (`sorafs_simulation_*`) e coletor de arquivo de texto do exportador de nó یا trabalho de raspagem independente کے لیے موزوں ہیں۔

Configuração de raspagem Prometheus Como fazer:

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

coletor de arquivo de texto کو `capacity_simulation.prom` کی طرف پوائنٹ کریں (node-exporter استعمال کریں تو اسے `--collector.textfile.directory` والی ڈائریکٹری میں کاپی کریں)۔

## 3. Grafana ڈیش بورڈ امپورٹ کریں

1. Grafana میں `dashboards/grafana/sorafs_capacity_simulation.json` امپورٹ کریں۔
2. Fonte de dados `Prometheus` ویری ایبل کو اوپر دیے گئے alvo de raspagem سے جوڑیں۔
3. O que fazer:
   - **Alocação de cota (GiB)** ہر provedor کے saldos comprometidos/atribuídos دکھاتا ہے۔
   - **Failover Trigger** métricas de interrupção آنے پر *Failover Active* ہو جاتا ہے۔
   - **Queda de tempo de atividade durante interrupção** provedor `alpha` کے لیے فیصدی نقصان دکھاتا ہے۔
   - **Porcentagem de barra solicitada** dispositivo de disputa سے نکلا taxa de remediação دکھاتا ہے۔

## 4. متوقع چیکس

- `sorafs_simulation_quota_total_gib{scope="assigned"}` کی قدر `600` رہتی ہے جب تک total comprometido >=600 رہے۔
- `sorafs_simulation_failover_triggered` قدر `1` دیتا ہے اور substituição fornecedor métrica میں `beta` نمایاں ہوتا ہے۔
- Provedor `sorafs_simulation_slash_requested` `alpha` کے لیے `0.15` (barra de 15%) رپورٹ کرتا ہے۔

`cargo test -p sorafs_car --features cli --test capacity_simulation_toolkit` چلائیں تاکہ تصدیق ہو سکے کہ fixtures اب بھی esquema CLI کے مطابق ہیں۔