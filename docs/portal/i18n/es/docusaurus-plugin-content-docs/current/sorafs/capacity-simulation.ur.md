---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/capacity-simulation.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: simulación de capacidad
título: SoraFS کپیسٹی سمیولیشن رَن بُک
sidebar_label: کپیسٹی سمیولیشن رَن بُک
descripción: accesorios reproducibles, exportaciones Prometheus, tableros de instrumentos Grafana کے ساتھ SF-2c کپیسٹی مارکیٹ پلیس سمیولیشن ٹول کٹ چلانا۔
---

:::nota ماخذِ مستند
یہ صفحہ `docs/source/sorafs/runbooks/sorafs_capacity_simulation.md` کی آئینہ دار ہے۔ جب تک پرانا Sphinx دستاویزی مجموعہ مکمل طور پر منتقل نہیں ہو جاتا دونوں نقول کو ہم آہنگ رکھیں۔
:::

یہ رن بُک وضاحت کرتی ہے کہ SF-2c کپیسٹی مارکیٹ پلیس سمیولیشن کِٹ کیسے چلائیں اور حاصل شدہ میٹرکس کیسے دیکھیں۔ یہ `docs/examples/sorafs_capacity_simulation/` میں موجود accesorios deterministas کے ذریعے cuota مذاکرات، conmutación por error ہینڈلنگ اور remediación de corte کو de extremo a extremo y کرتی ہے۔ Cargas útiles کپیسٹی اب بھی `sorafs_manifest_stub capacity` استعمال کرتے ہیں؛ manifiesto/CAR پیکجنگ کے لیے `iroha app sorafs toolkit pack` استعمال کریں۔

## 1. CLI آرٹی فیکٹس تیار کریں

```bash
cd $REPO_ROOT/docs/examples/sorafs_capacity_simulation
./run_cli.sh ./artifacts
```

`run_cli.sh`, `sorafs_manifest_stub capacity` incluye cargas útiles Norito, blobs base64, cuerpos de solicitud Torii y resúmenes JSON. برائے:

- cuota مذاکرات والے منظرنامے میں شریک تین proveedores کی declaraciones۔
- Orden de replicación, manifiesto por etapas y proveedores de datos.
- línea base previa a la interrupción, intervalo de interrupción, recuperación de conmutación por error e instantáneas de telemetría
- interrupción simulada کے بعد reducción درخواست کرنے والا carga útil de disputa۔تمام آرٹی فیکٹس `./artifacts` کے تحت جمع ہوتے ہیں (پہلے آرگومنٹ میں مختلف ڈائریکٹری دے کر اووررائیڈ کر سکتے ہیں)۔ انسانی سمجھ کے لیے `_summary.json` فائلیں دیکھیں۔

## 2. نتائج جمع کریں اور métricas جاری کریں

```bash
./analyze.py --artifacts ./artifacts
```

اینالائزر تیار کرتا ہے:

- `capacity_simulation_report.json`: asignación de asignaciones, deltas de conmutación por error y metadatos de disputa
- `capacity_simulation.prom` - Métricas de archivos de texto Prometheus (`sorafs_simulation_*`) y recopilador de archivos de texto exportador de nodo y trabajo de raspado independiente کے لیے موزوں ہیں۔

Configuración de raspado Prometheus Este es el siguiente:

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

recopilador de archivos de texto کو `capacity_simulation.prom` کی طرف پوائنٹ کریں (exportador de nodos استعمال کریں تو اسے `--collector.textfile.directory` والی ڈائریکٹری میں کاپی کریں)۔

## 3. Grafana ڈیش بورڈ امپورٹ کریں

1. Grafana میں `dashboards/grafana/sorafs_capacity_simulation.json` امپورٹ کریں۔
2. Fuente de datos `Prometheus` ویری ایبل کو اوپر دیے گئے scrape target سے جوڑیں۔
3. پینلز چیک کریں:
   - **Asignación de cuota (GiB)** ہر proveedor کے saldos comprometidos/asignados دکھاتا ہے۔
   - **Disparador de conmutación por error** métricas de interrupción آنے پر *Failover Active* ہو جاتا ہے۔
   - **Disminución del tiempo de actividad durante una interrupción** proveedor `alpha` کے لیے فیصدی نقصان دکھاتا ہے۔
   - **Porcentaje de barra diagonal solicitada** accesorio de disputa سے نکلا índice de remediación دکھاتا ہے۔

## 4. متوقع چیکس- `sorafs_simulation_quota_total_gib{scope="assigned"}` کی قدر `600` رہتی ہے جب تک total comprometido >=600 رہے۔
- `sorafs_simulation_failover_triggered` قدر `1` دیتا ہے اور proveedor de reemplazo métrico میں `beta` نمایاں ہوتا ہے۔
- `sorafs_simulation_slash_requested` proveedor `alpha` کے لیے `0.15` (barra del 15%) رپورٹ کرتا ہے۔

`cargo test -p sorafs_car --features cli --test capacity_simulation_toolkit` Configuración de accesorios y esquema CLI Configuración de configuración