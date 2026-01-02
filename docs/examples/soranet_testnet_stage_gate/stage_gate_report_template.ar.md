---
lang: ar
direction: rtl
source: docs/examples/soranet_testnet_stage_gate/stage_gate_report_template.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 06dcd662ffb2b22a13d9cd5418f5d2e8e64a4cdadb71a054488ce75b1eb96188
source_last_modified: "2025-11-07T11:07:58.852037+00:00"
translation_last_reviewed: 2026-01-01
---

<div dir="rtl">

<!-- الترجمة العربية لـ docs/examples/soranet_testnet_stage_gate/stage_gate_report_template.md -->

# تقرير مرحلة البوابة SNNet-10 (T?_ -> T?_)

> استبدل كل placeholder (العناصر بين اقواس الزوايا) قبل الارسال. احتفظ بعناوين الاقسام حتى تتمكن اتمتة الحوكمة من قراءة الملف.

## 1. البيانات الوصفية

| الحقل | القيمة |
|-------|-------|
| الترقية | `<T0->T1 او T1->T2>` |
| نافذة التقرير | `<YYYY-MM-DD -> YYYY-MM-DD>` |
| relays ضمن النطاق | `<count + IDs او "انظر الملحق A">` |
| جهة الاتصال الرئيسية | `<name/email/Matrix handle>` |
| ارشيف التسليم | `<snnet10-stage-gate-YYYYMMDD.tar.zst>` |
| ارشيف SHA-256 | `<sha256:...>` |

## 2. ملخص المقاييس

| المقياس | المرصود | العتبة | اجتاز؟ | المصدر |
|--------|----------|--------|--------|--------|
| نسبة نجاح الدوائر | `<0.000>` | >=0.95 | ☐ / ☑ | `reports/metrics-report.json` |
| نسبة brownout للfetch | `<0.000>` | <=0.01 | ☐ / ☑ | `reports/metrics-report.json` |
| تباين مزيج GAR | `<+0.0%>` | <=+/-10% | ☐ / ☑ | `reports/metrics-report.json` |
| ثواني PoW p95 | `<0.0 s>` | <=3 s | ☐ / ☑ | `telemetry/pow_window.json` |
| كمون p95 | `<0 ms>` | <200 ms | ☐ / ☑ | `telemetry/latency_window.json` |
| نسبة PQ (متوسط) | `<0.00>` | >= target | ☐ / ☑ | `telemetry/pq_summary.json` |

**السرد:** `<summaries of anomalies, mitigations, overrides>`

## 3. سجل drill والحوادث

| الطابع الزمني (UTC) | المنطقة | النوع | معرف التنبيه | ملخص التخفيف |
|---------------------|--------|------|-------------|--------------|
| `<YYYY-MM-DD HH:MM>` | `<region>` | `Brownout drill` | `<alert://...>` | `<restored anon-guard-pq in 3m12s>` |

## 4. المرفقات والهاشات

| الاثر | المسار | SHA-256 |
|-------|-------|---------|
| لقطة المقاييس | `reports/metrics-window.json` | `<sha256>` |
| تقرير المقاييس | `reports/metrics-report.json` | `<sha256>` |
| نصوص guard rotation | `evidence/guard_rotation/*.log` | `<sha256>` |
| manifests exit bonding | `evidence/exit_bonds/*.to` | `<sha256>` |
| سجلات drill | `evidence/drills/*.md` | `<sha256>` |
| جاهزية MASQUE (T1->T2) | `reports/masque-readiness.md` | `<sha256 or n/a>` |
| خطة rollback (T1->T2) | `reports/downgrade_plan.md` | `<sha256 or n/a>` |

## 5. الموافقات

| الدور | الاسم | موقّع (نعم/لا) | ملاحظات |
|------|------|----------------|---------|
| Networking TL | `<name>` | ☐ / ☑ | `<comments>` |
| Governance rep | `<name>` | ☐ / ☑ | `<comments>` |
| SRE delegate | `<name>` | ☐ / ☑ | `<comments>` |

## الملحق A - قائمة relays

```
- relay-id-001 (AS64496, region=SJC)
- relay-id-002 (AS64497, region=NRT)
...
```

## الملحق B - ملخصات الحوادث

```
<Detailed context for any incidents or overrides referenced above.>
```

</div>
