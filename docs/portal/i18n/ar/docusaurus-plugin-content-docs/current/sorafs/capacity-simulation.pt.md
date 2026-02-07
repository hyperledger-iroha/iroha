---
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/capacity-simulation.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
المعرف: محاكاة القدرات
العنوان: Runbook de simulação de capacidade do SoraFS
Sidebar_label: Runbook لمحاكاة القدرات
الوصف: تنفيذ مجموعة أدوات محاكاة سوق السعة SF-2c مع إعادة إنتاج التركيبات وتصدير Prometheus ولوحات المعلومات لـ Grafana.
---

:::ملاحظة Fonte canônica
هذه الصفحة مخصصة `docs/source/sorafs/runbooks/sorafs_capacity_simulation.md`. Mantenha ambas as copias sincronzadas.
:::

يشرح دليل التشغيل هذا كيفية تنفيذ مجموعة محاكاة لسوق القدرة SF-2c وتصور المقاييس الناتجة. إنه التحقق من صحة التفاوض على القطع أو معالجة الفشل أو معالجة قطع النقطة باستخدام محددات التركيبات في `docs/examples/sorafs_capacity_simulation/`. توجد حمولات السعة الصافية usam `sorafs_manifest_stub capacity`; استخدم `iroha app sorafs toolkit pack` لتدفقات سعة البيان/CAR.

## 1.Gerar artefatos de CLI

```bash
cd $REPO_ROOT/docs/examples/sorafs_capacity_simulation
./run_cli.sh ./artifacts
```

`run_cli.sh` مغلف `sorafs_manifest_stub capacity` لإصدار الحمولات الصافية Norito وblobs base64 ومجموعة الطلبات لـ Torii واستئناف JSON لـ:

- ثلاثة إقرارات من الموردين الذين شاركوا في سيناريو مفاوضات القطع.
- ترتيب النسخ المتماثل لتقسيم البيان على مراحل بين هؤلاء المراجعين.
- لقطات القياس عن بعد للخط الأساسي للخطأ أو الفاصل الزمني للخطأ واسترداد الفشل.
- يتم قطع حمولة النزاع على شكل محاكاة خاطئة.جميع المصنوعات موجودة لـ `./artifacts` (استبدال تمرير بدليل مختلف كحجة أولى). فحص الملفات `_summary.json` للسياق القانوني.

## 2. جمع النتائج وإخراج المقاييس

```bash
./analyze.py --artifacts ./artifacts
```

يا محلل المنتج:

- `capacity_simulation_report.json` - التخصيصات المجمعة وأجزاء تجاوز الفشل وبيانات التعريف الخلافية.
- `capacity_simulation.prom` - مقاييس ملف نصي لـ Prometheus (`sorafs_simulation_*`) كافية لمجمع ملفات نصية لتصدير العقدة أو استخراج مهمة مستقلة.

مثال لتكوين الكشط لـ Prometheus:

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

قم بإنشاء مجمع ملفات نصية لـ `capacity_simulation.prom` (أو استخدم مُصدر العقدة، وانسخ للدليل الذي تم تمريره عبر `--collector.textfile.directory`).

## 3. قم باستيراد لوحة المعلومات للقيام بـ Grafana

1. لا يوجد Grafana، استيراد `dashboards/grafana/sorafs_capacity_simulation.json`.
2. قم بإلغاء تحديد مصدر البيانات المتغير `Prometheus` لاستخراج الهدف الذي تم تكوينه أعلاه.
3. التحقق من الألم:
   - **تخصيص الحصص (GiB)** يعرض الأموال المتوافقة/الميزات من كل مصدر.
   - **Failover Trigger** يستخدم لـ *Failover Active* عند بدء تشغيل مقاييس الفشل.
   - **انخفاض وقت التشغيل أثناء انقطاع التيار** يمثل فقدان النسبة المئوية للدليل `alpha`.
   - **نسبة القطع المائلة المطلوبة** عرض حل المشكلة الإضافية لتركيب النزاع.

## 4. التحققات المتوقعة- `sorafs_simulation_quota_total_gib{scope="assigned"}` يعادل `600` في حالة إجمالي بقاء التسوية >=600.
- `sorafs_simulation_failover_triggered` تقرير `1` ومقياس لإثبات بديل `beta`.
- تقرير `sorafs_simulation_slash_requested` `0.15` (شرطة مائلة بنسبة 15%) لمعرف المعرف `alpha`.

قم بتنفيذ `cargo test -p sorafs_car --features cli --test capacity_simulation_toolkit` لتأكيد أن التركيبات موجودة على نفس مستوى CLI.