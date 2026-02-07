---
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/capacity-simulation.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
المعرف: محاكاة القدرات
العنوان: محاكاة رانبوك SoraFS
Sidebar_label: محاكاة رانبوك
الوصف: مجموعة كبيرة من محاكاة SF-2c مع الرسومات والصادرات Prometheus ولوحات البيانات Grafana.
---

:::note Канонический источник
هذا الجزء من الزر `docs/source/sorafs/runbooks/sorafs_capacity_simulation.md`. بعد أن تكون النسخ متزامنة، من خلال مجموعة كبيرة من وثائق أبو الهول، لن يتم تغييرها بشكل كامل.
:::

هذا هو التدريب على البدء في محاكاة مجموعة من SF-2c وتصور المقاييس المحصلة. قم بالتحقق من التفضيلات المتعلقة بالأمر وتجاوز الفشل والمعالجة من خلال خفض النهاية إلى النهاية باستخدام عناصر التحديد في `docs/examples/sorafs_capacity_simulation/`. تستخدم الحمولات النافعة ذات التحميل المسبق `sorafs_manifest_stub capacity`; استخدم `iroha app sorafs toolkit pack` لتعبئة البيان/CAR.

## 1. تصميم عناصر CLI

```bash
cd $REPO_ROOT/docs/examples/sorafs_capacity_simulation
./run_cli.sh ./artifacts
```

`run_cli.sh` يشير إلى `sorafs_manifest_stub capacity`، لإرسال الحمولات الصافية Norito، وbase64-blob، والاتصال Torii، وJSON-сводки لل:

- ثلاثة إعلانات من مقدمي العروض، سعداء بالسيناريوهات التي تم تقديمها لهم.
- نشر واحد للنسخ المتماثلة، متضمنًا بيانًا منظمًا من خلال تقديم العروض.
- أجهزة قياس عن بعد للخطوط الأساسية حتى النهاية، وفاصل زمني، وفشل التحسين.
- قطع الحمولة النافعة مع قطعها بعد تعديلها.جميع القطع الأثرية موجودة في `./artifacts` (من الممكن إعادة الاقتراح، قبل الكتالوج الآخر الوسيطة الأولى). تحقق من الملفات `_summary.json` لسياق القراءة.

## 2. تجميع النتائج واستخدام المقاييس

```bash
./analyze.py --artifacts ./artifacts
```

نموذج المحلل:

- `capacity_simulation_report.json` - التوزيع التراكمي وتجاوز الفشل والجراثيم المتغيرة.
- `capacity_simulation.prom` - مقاييس ملف نصي Prometheus (`sorafs_simulation_*`)، يدعم عقدة مُجمع مجمع الملفات النصية أو مهمة كشط متأخرة.

تكوين التمهيدي كشط Prometheus:

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

قم بتنزيل أداة تجميع الملفات النصية على `capacity_simulation.prom` (عند استخدام مصدر العقدة، قم بنسخها في الكتالوج، من خلال `--collector.textfile.directory`).

## 3. استيراد لوحة البيانات Grafana

1. في Grafana قم باستيراد `dashboards/grafana/sorafs_capacity_simulation.json`.
2. قم باستخدام مصدر البيانات الدائم `Prometheus` في جميع أنحاء الكشط.
3. التحقق من اللوحات:
   - **تخصيص الحصص (GiB)** يعرض رصيد الالتزام/التخصيص لكل من مقدم الخدمة.
   - **Failover Trigger** يتم إيقافه على *Failover Active*، عند إعادة تشغيل المقاييس.
   - **انخفاض وقت التشغيل أثناء انقطاع التيار الكهربائي** يتم إرسال نسبة كبيرة من البطاريات للمعالج `alpha`.
   - **نسبة القطع المطلوبة** تصور العلاجات الفعالة من الصور الفوتوغرافية.

## 4.انتظر التحقيق- `sorafs_simulation_quota_total_gib{scope="assigned"}` رافين `600`، عند انتهاء الالتزام >=600.
- يشير `sorafs_simulation_failover_triggered` إلى `1`، ويعرض المقياس المصغر `beta`.
- `sorafs_simulation_slash_requested` يعرض `0.15` (شرطة مائلة بنسبة 15%) لموفر الهوية `alpha`.

قم بإغلاق `cargo test -p sorafs_car --features cli --test capacity_simulation_toolkit` للتأكد من أن الصور التي تدعم مخطط CLI الأساسي.