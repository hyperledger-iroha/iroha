---
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/capacity-simulation.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
المعرف: محاكاة القدرات
العنوان: Runbook de simulación de capacidad de SoraFS
Sidebar_label: Runbook لمحاكاة القدرات
الوصف: قم بتشغيل مجموعة أدوات محاكاة سوق السعة SF-2c مع تركيبات قابلة للتكرار وتصدير Prometheus ولوحات معلومات Grafana.
---

:::ملاحظة فوينتي كانونيكا
هذه الصفحة تعكس `docs/source/sorafs/runbooks/sorafs_capacity_simulation.md`. حافظ على النسخ المتزامنة حتى يتم ترحيل مجموعة الوثائق المتوارثة في Sphinx بالكامل.
:::

يشرح هذا الدليل كيفية تشغيل مجموعة محاكاة سوق السعة SF-2c وعرض المقاييس الناتجة. التحقق من صحة التفاوض على القطع، وطريقة تجاوز الفشل، ومعالجة القطع من أقصى إلى أقصى باستخدام الإعدادات المحددة في `docs/examples/sorafs_capacity_simulation/`. الحمولات ذات السعة المستخدمة `sorafs_manifest_stub capacity`; الولايات المتحدة الأمريكية `iroha app sorafs toolkit pack` لتدفقات البيانات المعبأة/CAR.

## 1. إنشاء المصنوعات اليدوية لـ CLI

```bash
cd $REPO_ROOT/docs/examples/sorafs_capacity_simulation
./run_cli.sh ./artifacts
```

`run_cli.sh` يرسل `sorafs_manifest_stub capacity` لإصدار الحمولات النافعة Norito، وblobs base64، ومجموعة طلبات Torii واستئناف JSON لـ:- ثلاثة تصريحات من الموردين المشاركين في سيناريو مفاوضات الشراء.
- أمر نسخ يعين البيان على مراحل بين هؤلاء الموردين.
- لقطات القياس عن بعد لخط الأساس السابق للرأس، والفاصل الزمني للوقت، والاسترداد بعد الفشل.
- حمولة من طلب النزاع يتم قطعها عبر المحاكاة.

يتم كتابة جميع المصنوعات تحت عنوان `./artifacts` (يمكن استبدالها بالتمرير إلى دليل مختلف كحجة أولية). فحص الملفات `_summary.json` ليكون السياق مقروءًا.

## 2. قم بجمع النتائج وإخراج المقاييس

```bash
./analyze.py --artifacts ./artifacts
```

شركة التحليل تنتج:

- `capacity_simulation_report.json` - التعيينات المجمعة وبيانات تجاوز الفشل وبيانات التعريف المتنازع عليها.
- `capacity_simulation.prom` - مقاييس ملف نصي Prometheus (`sorafs_simulation_*`) مناسبة لمجمع عقدة الملفات النصية أو عملية استخراج مستقلة.

مثال لتكوين سكراب Prometheus:

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

قم بإضافة مُجمع الملفات النصية إلى `capacity_simulation.prom` (إذا تم استخدام مُصدر العقدة، قم بنسخ الدليل عبر `--collector.textfile.directory`).

## 3. استيراد لوحة القيادة من Grafana1. في Grafana، استيراد `dashboards/grafana/sorafs_capacity_simulation.json`.
2. قم بإغلاق متغير مصدر البيانات `Prometheus` لإلغاء تكوين الهدف.
3. التحقق من اللوحات:
   - **تخصيص الحصص (GiB)** يستخدم الأرصدة المتفق عليها/المخصصة لكل مورد.
   - **Failover Trigger** يقوم بتغيير *Failover Active* عند إدخال مقاييس الصندوق.
   - **انخفاض وقت التشغيل أثناء انقطاع التيار** تم رسم الخسارة الكبيرة للمورد `alpha`.
   - **نسبة القطع المائلة المطلوبة** عرض نسبة الإصلاح الإضافية لتركيبات النزاع.

## 4. التسويات المتوقعة

- `sorafs_simulation_quota_total_gib{scope="assigned"}` يعادل `600` عندما يتم الحفاظ على إجمالي التسوية >=600.
- `sorafs_simulation_failover_triggered` تقرير `1` ومقياس استبدال المنتج الناتج `beta`.
- تقرير `sorafs_simulation_slash_requested` `0.15` (شرطة مائلة بنسبة 15%) لمعرف المعرف `alpha`.

قم بتشغيل `cargo test -p sorafs_car --features cli --test capacity_simulation_toolkit` لتأكيد أن التركيبات ستكون مقبولة من خلال مؤشر CLI.