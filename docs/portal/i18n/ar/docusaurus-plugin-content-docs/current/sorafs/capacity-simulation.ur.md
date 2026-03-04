---
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/capacity-simulation.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
المعرف: محاكاة القدرات
العنوان: SoraFS بيستس سيميوني رَن بوك
Sidebar_label: کپیستٹی سمیولیشن رَن بُك
الوصف: التركيبات القابلة للتكرار، وصادرات Prometheus، ولوحات المعلومات Grafana التي تحتوي على SF-2c لمنصة السوق العالمية.
---

:::ملاحظة مأخذ الوثيقة
هذه هي الصفحة `docs/source/sorafs/runbooks/sorafs_capacity_simulation.md`. عندما يتم إنشاء برانا أبو الهول بالكامل في مرحلة الانتقال، لا يمكننا أن نقول ما هو أفضل من ذلك.
:::

لقد أصبح واضحًا أن SF-2c سجل سوقي يكمل كل شيء وقد حقق علامة فارقة أخرى. يحتوي `docs/examples/sorafs_capacity_simulation/` على تركيبات حتمية موجودة تعمل على تحسين نظام الحصص وتجاوز الفشل والمعالجة المتقطعة من طرف إلى طرف. تستخدم الحمولات الصافية `sorafs_manifest_stub capacity` أيضًا؛ استخدام البيان/بيانات CAR `iroha app sorafs toolkit pack`.

## 1. قصة نجاح CLI

```bash
cd $REPO_ROOT/docs/examples/sorafs_capacity_simulation
./run_cli.sh ./artifacts
```

`run_cli.sh`، `sorafs_manifest_stub capacity` للحمولات النافعة Norito، وbase64 blobs، وهيئات الطلب Torii وملخصات JSON التالية:

- الحصص مذاكرات وےنامنامے میں شریک تین مقدمي الإعلانات کی ۔
- هذا أمر النسخ المتماثل الذي يقدم بيانًا منظمًا من قبل مقدمي الخدمة.
- خط الأساس قبل الانقطاع، والفاصل الزمني للانقطاع، واستعادة تجاوز الفشل، ولقطات القياس عن بعد.
- محاكاة الانقطاع بعد قطع حمولة الحمولة والنزاع.تم جمع كل المقالات `./artifacts` تحت هذه المقالة (تم نشر مقالات مختلفة من قبل باحثين يابانيين في سكتة دماغية). تم إنشاء رمز إنساني لـ `_summary.json`.

## 2. نتائج البيانات المالية والمقاييس الجارية

```bash
./analyze.py --artifacts ./artifacts
```

الفائز بالجائزة:

- `capacity_simulation_report.json` - مجموع التخصيصات ودلتا تجاوز الفشل والبيانات التعريفية للنزاع.
- `capacity_simulation.prom` - Prometheus مقاييس الملفات النصية (`sorafs_simulation_*`) عبارة عن أداة تجميع الملفات النصية لمصدر العقدة أو مهمة كشط مستقلة للملفات النصية.

Prometheus تكوين الكشط على سبيل المثال:

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

مُجمِّع textfile هو `capacity_simulation.prom` للبطاقة الدولية (يُستخدم مُصدِّر العقدة `--collector.textfile.directory` للبطاقة الدولية).

## 3.Grafana ميناء بورسعيد

1. Grafana بطاقة `dashboards/grafana/sorafs_capacity_simulation.json`.
2. `Prometheus` مصدر البيانات الأول هو كشط الهدف.
3. دبابيس صغيرة:
   - **تخصيص الحصص (GiB)** المزود هو الأرصدة الملتزم بها/المخصصة دکھاتا.
   - **مشغل تجاوز الفشل** مقاييس انقطاع الخدمة في *تجاوز الفشل النشط* في كل مكان.
   - **انخفاض وقت التشغيل أثناء انقطاع التيار** مزود `alpha` لتخفيض عيوب النقصان.
   - **نسبة القطع المائلة المطلوبة** تركيب النزاع سے نکلا نسبة المعالجة دکھاتا ہے۔

## 4. التوقع المتوقع- `sorafs_simulation_quota_total_gib{scope="assigned"}` قوة `600` إجمالي الالتزام >=600 ر.
- `sorafs_simulation_failover_triggered` قوة `1` ديتا ومقياس مزود الاستبدال `beta` لا يزال موجودًا.
- موفر `sorafs_simulation_slash_requested` `alpha` لـ `0.15` (شرطة مائلة بنسبة 15%).

`cargo test -p sorafs_car --features cli --test capacity_simulation_toolkit` قم بنقرة واحدة على جدول المباريات وفقًا لمخطط CLI.