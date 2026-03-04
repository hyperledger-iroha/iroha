---
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/capacity-simulation.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
المعرف: محاكاة القدرات
العنوان: دليل تشغيل محاكاة حالة SoraFS
Sidebar_label: دليل محاكاة السعة
الوصف: تشغيل مجموعة أدوات محاكاة سوق السعة SF-2c باستخدام تركيبات غير قابلة لإعادة الإنتاج وصادرات Prometheus ولوحات Grafana.
---

:::ملحوظة المصدر مؤهل
احترام هذه الصفحة `docs/source/sorafs/runbooks/sorafs_capacity_simulation.md`. حافظ على تزامن النسختين إلى أن تُنقَل مجموعة أفلام Sphinx القديمة بالكامل.
:::

يشرح هذا الدليل كيفية تشغيل مجموعة محاكاة سوق السعة SF-2c الواسعة النطاق. عندما يكون من التفاوض الحصص، وتجاوز الفشل، وslashing من الطرف إلى الطرف باستخدام التركيبات الحتمية في `docs/examples/sorafs_capacity_simulation/`. لا تزال الحمولات المستخدمة `sorafs_manifest_stub capacity`؛ استخدم `iroha app sorafs toolkit pack` لتعبئة وتغليف ضخات البيان/CAR.

## 1. إنشاء قطع أثرية خاصة بالـ CLI

```bash
cd $REPO_ROOT/docs/examples/sorafs_capacity_simulation
./run_cli.sh ./artifacts
```

تقوم `run_cli.sh` بلف `sorafs_manifest_stub capacity` لإخراج Norito حمولات وblobs من base64 وأجسام طلبات Torii وملخصات JSON لـ:

- ثلاث حالات للتحكم في المشاركين في سيناريو التفاوض.
- أمر نسخة يوزّع المانيفست المجهز عبر منظمي الملابس.
- لقطات فيديو لخط الأساس قبل الانقطاع، وفترة الانقطاع، وتعافي الفشل.
- الحمولة التي يختارها القطع بعد الانقطاع المُحاكي.

تُكتب كل الآرتيفاكت تحت `./artifacts` (يمكن الاستبدال بتمرير دليل مختلف كأول وسيط). تمت إعادة النظر في المستندات `_summary.json` الخاصة بالمادة المعتمدة.

## 2. النتائج وإصدارات المعايير

```bash
./analyze.py --artifacts ./artifacts
```

يقوم بإنشاء إنتاج:- `capacity_simulation_report.json` - تخصيصات مجمعة، فروق الفشل، وبيانات نزاع وصفية.
- `capacity_simulation.prom` - معايير textfile لـ Prometheus (`sorafs_simulation_*`) مناسبة لـ textfile Collector الخاص بـ Node-exporter أو Scrape job مستقل.

مثال لإعداد كشط في Prometheus:

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

يوجه جامع الملفات النصية إلى `capacity_simulation.prom` (عند استخدام Node-exporter يرسل إلى دليل المسار عبر `--collector.textfile.directory`).

##3.استيراد اللوحة Grafana

1. في Grafana، استورد `dashboards/grafana/sorafs_capacity_simulation.json`.
2. ربط متغير مصدر البيانات `Prometheus` لغرض كشط المُهيأ فوق.
3. التحقق من الملصقات:
   - **تخصيص الحصص (GiB)** المقدمة الأرصدة الملتزمة بها/المخصصة لكل متحكم.
   - **Failover Trigger** يتحول إلى *Failover Active* عند وصول معايير الانقطاع.
   - **انخفاض وقت التشغيل أثناء الانقطاع** يرسم نسبة الفقدان للمنظم `alpha`.
   - **نسبة القطع المطلوبة** المقدمة بنسبة المعالجة المركزية المستخرجة من التركيبات المعترضة.

## 4. لحساب

- `sorafs_simulation_quota_total_gib{scope="assigned"}` يساوي `600` ويبقى الالتزام الملتزم >=600.
- `sorafs_simulation_failover_triggered` المعروضة `1` ويبرز مقياس المفرغ الكهربائي `beta`.
- `sorafs_simulation_slash_requested` المقدمة `0.15` (‏15% شرطة مائلة) لم يتم التعرف على المرخص `alpha`.

شغّل `cargo test -p sorafs_car --features cli --test capacity_simulation_toolkit` للتأكد من أن المباريات ما زالت مقبولة عبر مخطط الـ CLI.