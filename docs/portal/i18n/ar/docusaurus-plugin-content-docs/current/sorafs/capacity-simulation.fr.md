---
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/capacity-simulation.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
المعرف: محاكاة القدرات
العنوان: Runbook de Simulation de capacité SoraFS
Sidebar_label: Runbook لمحاكاة القدرات
الوصف: قم بتمرين مجموعة محاكاة سوق السعة SF-2c مع التركيبات القابلة لإعادة الإنتاج والصادرات Prometheus واللوحات الخشبية Grafana.
---

:::ملاحظة المصدر الكنسي
هذه الصفحة تعكس `docs/source/sorafs/runbooks/sorafs_capacity_simulation.md`. قم بمزامنة النسختين حتى يتم هجرة مجموعة الوثائق التي ورثها أبو الهول بالكامل.
:::

يشرح كتاب التشغيل هذا التعليق على تنفيذ مجموعة محاكاة سوق السعة SF-2c ورؤية المقاييس الناتجة. يصادق على التفاوض بشأن الحصص وإدارة الفشل ومعالجة قطع النوبة بمساعدة التركيبات المحددة في `docs/examples/sorafs_capacity_simulation/`. تستخدم الحمولات ذات السعة دائمًا `sorafs_manifest_stub capacity`؛ استخدم `iroha app sorafs toolkit pack` لتدفق التفريغ الواضح/CAR.

## 1. إنشاء القطع الأثرية CLI

```bash
cd $REPO_ROOT/docs/examples/sorafs_capacity_simulation
./run_cli.sh ./artifacts
```

`run_cli.sh` يحتوي على `sorafs_manifest_stub capacity` لزيادة الحمولات Norito وblobs base64 ومجموعة الطلبات Torii والسيرة الذاتية JSON من أجل:- ثلاثة إعلانات من الموردين المشاركين في سيناريو التفاوض على الحصص.
- أمر نسخ يسمح بالبيان المرحلي بين هؤلاء الموردين.
- لقطات عن بعد للخط الأساسي المسبق والفاصل الزمني للشاشة واستعادة الفشل.
- حمولة من الدعاوى القضائية تتطلب تقطيعًا بعد اللوحة المحاكية.

جميع المنتجات محفوظة في `./artifacts` (استبدلها بمرجع آخر في الوسيطة الأولى). افحص الملفات `_summary.json` للحصول على سياق مقبول.

## 2. إضافة النتائج وتحسين المقاييس

```bash
./analyze.py --artifacts ./artifacts
```

منتج المحلل :

- `capacity_simulation_report.json` - التخصيصات المعتمدة وأجزاء تجاوز الفشل وتعديلات الدعاوى.
- `capacity_simulation.prom` - يتم تكييف ملفات textfile Prometheus (`sorafs_simulation_*`) مع مجمع ملفات نصية لعقدة المصدر أو لاستخراج مهمة مستقلة.

مثال لتكوين الكشط Prometheus :

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

Dirigez le textfile collector vers `capacity_simulation.prom` (si vous utilisez node-exporter, copiez-le dans le répertoire passé via `--collector.textfile.directory`).

## 3. المستورد لوحة القيادة Grafana1. في Grafana، قم باستيراد `dashboards/grafana/sorafs_capacity_simulation.json`.
2. قم بإقران متغير مصدر البيانات `Prometheus` بسلك الاستخراج الذي تم تكوينه.
3. التحقق من الألواح :
   - **تخصيص الحصة (GiB)** لعرض المبيعات المشاركة/المخصصة لكل مورد.
   - **Failover Trigger** أساسي على *Failover Active* عند وصول مقاييس اللوحة.
   - **انخفاض وقت التشغيل أثناء انقطاع التيار الكهربائي** تتبع النسبة المئوية لمزود `alpha`.
   - **النسبة المئوية للقطع المطلوبة** تصور نسبة العلاج الإضافية من تركيبات الدعوى.

## 4. حضور عمليات التحقق

- `sorafs_simulation_quota_total_gib{scope="assigned"}` متساوٍ مع `600` بحيث يكون إجمالي المشاركة >=600.
- `sorafs_simulation_failover_triggered` يشير إلى `1` ومقياس مزود الاستبدال المقدم `beta`.
- `sorafs_simulation_slash_requested` يشير إلى `0.15` (15% من الشرطة المائلة) لمعرف المورد `alpha`.

قم بتنفيذ `cargo test -p sorafs_car --features cli --test capacity_simulation_toolkit` للتأكد من أن التركيبات مقبولة دائمًا وفقًا لمخطط CLI.