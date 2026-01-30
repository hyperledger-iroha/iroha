---
lang: ur
direction: rtl
source: docs/portal/docs/sorafs/capacity-simulation.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

---
id: capacity-simulation
title: دليل تشغيل محاكاة سعة SoraFS
sidebar_label: دليل محاكاة السعة
description: تشغيل مجموعة أدوات محاكاة سوق السعة SF-2c باستخدام fixtures قابلة لإعادة الإنتاج وصادرات Prometheus ولوحات Grafana.
---

:::note المصدر المعتمد
تعكس هذه الصفحة `docs/source/sorafs/runbooks/sorafs_capacity_simulation.md`. حافظ على تزامن النسختين إلى أن تُنقَل مجموعة توثيق Sphinx القديمة بالكامل.
:::

يشرح هذا الدليل كيفية تشغيل مجموعة محاكاة سوق السعة SF-2c وعرض المقاييس الناتجة. يتحقق من تفاوض الحصص، ومعالجة failover، ومعالجة slashing من الطرف إلى الطرف باستخدام fixtures الحتمية في `docs/examples/sorafs_capacity_simulation/`. لا تزال payloads السعة تستخدم `sorafs_manifest_stub capacity`؛ استخدم `iroha app sorafs toolkit pack` لتدفقات تغليف manifest/CAR.

## 1. إنشاء Artifacts خاصة بالـ CLI

```bash
cd $REPO_ROOT/docs/examples/sorafs_capacity_simulation
./run_cli.sh ./artifacts
```

تقوم `run_cli.sh` بلف `sorafs_manifest_stub capacity` لإخراج Norito payloads وblobs من base64 وأجسام طلبات Torii وملخصات JSON لـ:

- ثلاث تصريحات لمزوّدين مشاركين في سيناريو تفاوض الحصص.
- أمر نسخ يوزّع المانيفست المجهز عبر أولئك المزوّدين.
- لقطات تليمترية لخط الأساس قبل الانقطاع، وفترة الانقطاع، وتعافي failover.
- payload نزاع يطلب slashing بعد الانقطاع المُحاكَى.

تُكتب كل الآرتيفاكت تحت `./artifacts` (يمكن الاستبدال بتمرير دليل مختلف كأول وسيط). راجع ملفات `_summary.json` للحصول على سياق مقروء.

## 2. تجميع النتائج وإصدار المقاييس

```bash
./analyze.py --artifacts ./artifacts
```

يقوم المحلل بإنتاج:

- `capacity_simulation_report.json` - تخصيصات مجمعة، فروق failover، وبيانات نزاع وصفية.
- `capacity_simulation.prom` - مقاييس textfile لـ Prometheus (`sorafs_simulation_*`) مناسبة لـ textfile collector الخاص بـ node-exporter أو scrape job مستقل.

مثال إعداد scrape في Prometheus:

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

وجّه textfile collector إلى `capacity_simulation.prom` (عند استخدام node-exporter انسخه إلى الدليل الممرر عبر `--collector.textfile.directory`).

## 3. استيراد لوحة Grafana

1. في Grafana، استورد `dashboards/grafana/sorafs_capacity_simulation.json`.
2. اربط متغير مصدر البيانات `Prometheus` بهدف scrape المُهيأ أعلاه.
3. تحقق من اللوحات:
   - **Quota Allocation (GiB)** يعرض الأرصدة الملتزم بها/المخصصة لكل مزوّد.
   - **Failover Trigger** يتحول إلى *Failover Active* عند وصول مقاييس الانقطاع.
   - **Uptime Drop During Outage** يرسم نسبة الفقدان للمزوّد `alpha`.
   - **Requested Slash Percentage** يعرض نسبة المعالجة المستخرجة من fixture النزاع.

## 4. الفحوصات المتوقعة

- `sorafs_simulation_quota_total_gib{scope="assigned"}` يساوي `600` طالما بقي الإجمالي الملتزم >=600.
- `sorafs_simulation_failover_triggered` يعرض `1` ويبرز مقياس المزوّد البديل `beta`.
- `sorafs_simulation_slash_requested` يعرض `0.15` (‏15% slash) لمعرّف المزوّد `alpha`.

شغّل `cargo test -p sorafs_car --features cli --test capacity_simulation_toolkit` للتأكد من أن fixtures ما تزال مقبولة عبر مخطط الـ CLI.
