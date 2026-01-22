<!-- Auto-generated stub for Arabic (ar) translation. Replace this content with the full translation. -->

---
lang: ar
direction: rtl
source: docs/source/sorafs/runbooks/sorafs_capacity_simulation.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: a74e1cb5abc86822ff9d24b9ce42a6567d964cbc01ca4c619b49ca6d239101da
source_last_modified: "2025-11-05T18:02:08.787799+00:00"
translation_last_reviewed: 2025-12-28
---

# دليل تشغيل محاكاة سعة SoraFS

يشرح هذا الدليل كيفية تشغيل مجموعة أدوات محاكاة سوق السعة SF-2c وعرض المقاييس الناتجة. الهدف هو التحقق من تفاوض الحصص، ومعالجة failover، ومعالجة slashing من الطرف إلى الطرف باستخدام fixtures القابلة لإعادة الإنتاج ضمن `docs/examples/sorafs_capacity_simulation/`. لا تزال payloads السعة تستخدم `sorafs_manifest_stub capacity`؛ استخدم `iroha app sorafs toolkit pack` لتدفقات تغليف manifest/CAR.

## 1. إنشاء Artifacts خاصة بالـ CLI

```bash
cd $REPO_ROOT/docs/examples/sorafs_capacity_simulation
./run_cli.sh ./artifacts
```

يستدعي السكربت `sorafs_manifest_stub capacity` لإخراج Norito payloads حتمية وترميزات base64 وأجسام طلبات Torii وملخصات JSON لـ:

- ثلاث تصريحات لمزوّدين مشاركين في سيناريو تفاوض الحصص.
- أمر نسخ يوزّع المانيفست المجهز عبر المزوّدين.
- لقطات تليمترية تلتقط خط الأساس قبل الانقطاع، ونافذة الانقطاع، وتعافي failover.
- payload نزاع يطلب slashing بعد الانقطاع المُحاكَى.

تُكتب الآرتيفاكت في `./artifacts` (أو في المسار المزوّد كأول وسيط). راجع ملفات `_summary.json` للحصول على حالة مقروءة.

## 2. تجميع النتائج وإصدار المقاييس

```bash
./analyze.py --artifacts ./artifacts
```

ينتج سكربت التحليل:

- `capacity_simulation_report.json` — تخصيصات مجمعة، فروق failover، وبيانات نزاع وصفية.
- `capacity_simulation.prom` — مقاييس textfile لـ Prometheus (`sorafs_simulation_*`) مناسبة للاستيراد عبر textfile collector الخاص بـ node-exporter أو scrape job مستقل في Prometheus.

مثال إعداد scrape في `prometheus.yml`:

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

وجّه textfile collector إلى ملف `.prom` المُولّد (بالنسبة إلى node-exporter انسخه إلى `--collector.textfile.directory` المُعدّ).

## 3. استيراد لوحة Grafana

1. في Grafana، استورد `dashboards/grafana/sorafs_capacity_simulation.json`.
2. اربط إدخال مصدر البيانات `Prometheus` بتكوين scrape أعلاه.
3. تحقق من اللوحات:
   - **Quota Allocation (GiB)** يعرض الرصيد الملتزم/المخصص لكل مزوّد.
   - **Failover Trigger** يتحول إلى *Failover Active* عند تحميل مقاييس الانقطاع.
   - **Uptime Drop During Outage** يرسم نسبة الفقدان للمزوّد `alpha`.
   - **Requested Slash Percentage** يعرض نسبة المعالجة المستخرجة من fixture النزاع.

## 4. الفحوصات المتوقعة

- `sorafs_simulation_quota_total_gib{scope="assigned"}` يساوي 600 طالما بقي الإجمالي الملتزم ≥600.
- `sorafs_simulation_failover_triggered` يعرض `1` ويبرز مقياس المزوّد البديل `beta`.
- `sorafs_simulation_slash_requested` يعرض `0.15` (‏15% slash) لمعرّف المزوّد `alpha`.

شغّل `cargo test -p sorafs_car --features cli --test capacity_simulation_toolkit` للتأكد من أن fixtures تواصل التحقق مع مخطط الـ CLI.
