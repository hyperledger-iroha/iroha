---
lang: ar
direction: rtl
source: docs/examples/soranet_testnet_operator_kit/04-telemetry.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: a947c289c13c15b09dfbbf28c23ae1539fd3e29ca3943fa8522c3eca32c28bf5
source_last_modified: "2025-11-04T17:24:14.014911+00:00"
translation_last_reviewed: 2026-01-01
---

<div dir="rtl">

<!-- الترجمة العربية لـ docs/examples/soranet_testnet_operator_kit/04-telemetry.md -->

# متطلبات التليمترى

## اهداف Prometheus

اجمع مقاييس relay و orchestrator مع التسميات التالية:

```yaml
- job_name: "soranet-relay"
  static_configs:
    - targets: ["relay-host:9898"]
      labels:
        region: "testnet-t0"
        role: "relay"
- job_name: "sorafs-orchestrator"
  static_configs:
    - targets: ["orchestrator-host:9797"]
      labels:
        region: "testnet-t0"
        role: "orchestrator"
```

## لوحات المعلومات المطلوبة

1. `dashboards/grafana/soranet_testnet_overview.json` *(سيتم نشرها)* - حمّل JSON واستورد المتغيرات `region` و `relay_id`.
2. `dashboards/grafana/soranet_privacy_metrics.json` *(اصل SNNet-8 موجود)* - تاكد من ظهور لوحات privacy bucket بدون فراغات.

## قواعد التنبيه

يجب ان تطابق العتبات توقعات دليل التشغيل:

- زيادة `soranet_privacy_circuit_events_total{kind="downgrade"}` > 0 خلال 10 دقائق يطلق `critical`.
- `sorafs_orchestrator_policy_events_total{outcome="brownout"}` > 5 خلال 30 دقيقة يطلق `warning`.
- `up{job="soranet-relay"}` == 0 لمدة دقيقتين يطلق `critical`.

حمّل القواعد في Alertmanager مع المستقبل `testnet-t0`؛ تحقق باستخدام `amtool check-config`.

## تقييم المقاييس

اجمع لقطة لمدة 14 يوما ومررها الى مدقق SNNet-10:

```
cargo xtask soranet-testnet-metrics --input 07-metrics-sample.json --out metrics-report.json
```

- استبدل ملف العينة بلقطتك المصدرة عند التشغيل على بيانات حية.
- نتيجة `status = fail` تمنع الترقية؛ عالج التحقق(ات) المميزة قبل اعادة المحاولة.

## التقارير

ارفع كل اسبوع:

- لقطات الاستعلامات (`.png` او `.pdf`) التي تظهر نسبة PQ ومعدل نجاح الدوائر وهيستوغرام حل PoW.
- مخرجات قاعدة التسجيل Prometheus لـ `soranet_privacy_throttles_per_minute`.
- ملخص قصير يصف اي تنبيهات تم اطلاقها وخطوات التخفيف (ضمن timestamps).

</div>
