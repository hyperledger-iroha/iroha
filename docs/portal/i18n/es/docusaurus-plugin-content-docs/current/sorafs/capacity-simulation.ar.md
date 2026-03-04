---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/capacity-simulation.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: simulación de capacidad
título: دليل تشغيل محاكاة سعة SoraFS
sidebar_label: دليل محاكاة السعة
descripción: تشغيل مجموعة أدوات محاكاة سوق السعة SF-2c باستخدام accesorios قابلة لإعادة الإنتاج وصادرات Prometheus ولوحات Grafana.
---

:::nota المصدر المعتمد
Utilice el botón `docs/source/sorafs/runbooks/sorafs_capacity_simulation.md`. حافظ على تزامن النسختين إلى أن تُنقَل مجموعة توثيق Sphinx القديمة بالكامل.
:::

يشرح هذا الدليل كيفية تشغيل مجموعة محاكاة سوق السعة SF-2c وعرض المقاييس الناتجة. Establece una conmutación por error y una reducción de los accesorios de `docs/examples/sorafs_capacity_simulation/`. Para cargas útiles السعة تستخدم `sorafs_manifest_stub capacity`؛ استخدم `iroha app sorafs toolkit pack` لتدفقات تغليف manifest/CAR.

## 1. إنشاء Artefactos خاصة بالـ CLI

```bash
cd $REPO_ROOT/docs/examples/sorafs_capacity_simulation
./run_cli.sh ./artifacts
```

Incluye `run_cli.sh` y `sorafs_manifest_stub capacity`, cargas útiles y blobs de Norito en base64 y archivos Torii y JSON. لـ:

- ثلاث تصريحات لمزوّدين مشاركين في سيناريو تفاوض الحصص.
- أمر نسخ يوزّع المانيفست المجهز عبر أولئك المزوّدين.
- Ajustes de seguridad y conmutación por error.
- carga útil نزاع يطلب recortando بعد الانقطاع المُحاكَى.

Utilice el botón `./artifacts` (es posible que el dispositivo esté conectado a Internet). راجع ملفات `_summary.json` للحصول على سياق مقروء.

## 2. تجميع النتائج وإصدار المقاييس

```bash
./analyze.py --artifacts ./artifacts
```

يقوم المحلل بإنتاج:- `capacity_simulation_report.json` - تخصيصات مجمعة, فروق failover, وبيانات نزاع وصفية.
- `capacity_simulation.prom` - Archivo de texto de Prometheus (`sorafs_simulation_*`) que incluye un recopilador de archivos de texto, un exportador de nodos y un trabajo de scrape.

Cómo raspar con Prometheus:

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

Y el recopilador de archivos de texto es `capacity_simulation.prom` (el exportador de nodos está basado en `--collector.textfile.directory`).

## 3. استيراد لوحة Grafana

1. Aquí Grafana, seleccione `dashboards/grafana/sorafs_capacity_simulation.json`.
2. اربط متغير مصدر البيانات `Prometheus` بهدف scrape المُهيأ أعلاه.
3. تحقق من اللوحات:
   - **Asignación de cuota (GiB)** يعرض الأرصدة الملتزم بها/المخصصة لكل مزوّد.
   - **Activador de conmutación por error** يتحول إلى *Failover Active* عند وصول مقاييس الانقطاع.
   - **Caída del tiempo de actividad durante una interrupción** يرسم نسبة الفقدان للمزوّد `alpha`.
   - **Porcentaje de barra diagonal solicitada** يعرض نسبة المعالجة المستخرجة من fixture النزاع.

## 4. الفحوصات المتوقعة

- `sorafs_simulation_quota_total_gib{scope="assigned"}` يساوي `600` طالما بقي الإجمالي الملتزم >=600.
- `sorafs_simulation_failover_triggered` يعرض `1` y مقياس المزوّد البديل `beta`.
- `sorafs_simulation_slash_requested` يعرض `0.15` (‏15% de barra diagonal) لمعرّف المزوّد `alpha`.

Utilice `cargo test -p sorafs_car --features cli --test capacity_simulation_toolkit` para instalar accesorios en la CLI.