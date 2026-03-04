---
lang: ar
direction: rtl
source: docs/examples/android_device_lab_request.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 8a8e6a4981a11faac56d9b04432773e94fd59f8e2524fa4c552be459291c7c39
source_last_modified: "2025-11-12T08:31:44.643013+00:00"
translation_last_reviewed: 2026-01-01
---

<div dir="rtl">

<!-- الترجمة العربية لـ docs/examples/android_device_lab_request.md -->

# قالب طلب حجز مختبر اجهزة Android

انسخ هذا القالب الى قائمة Jira `_android-device-lab` عند حجز العتاد. ارفق روابط
خطوط Buildkite، artefacts الامتثال، واي تذاكر شريك تعتمد على التشغيل.

```
الملخص: <Milestone / workload> - <lane(s)> - <date/time UTC>

المعلم / التتبع:
- Roadmap item: AND6 / AND7 / AND8 (choose)
- Related ticket(s): <link to ANDx issue>, <partner-sla reference if any>

الطالب / جهة الاتصال:
- Primary engineer:
- Backup engineer:
- Slack channel / pager escalation:

تفاصيل الحجز:
- Lanes required: <pixel8pro-strongbox-a / pixel8a-ci-b / pixel7-fallback / firebase-burst / strongbox-external>
- Desired slot: <YYYY-MM-DD HH:MM UTC> for <duration>
- Workload type: <CI smoke / attestation sweep / chaos rehearsal / partner demo>
- Tooling to run: <scripts/buildkite job names>
- Artefacts produced: <logs, attestation bundles, dashboards>

التبعيات:
- Capacity snapshot reference: link to `android_strongbox_capture_status.md`
- Readiness matrix rows touched: link to `android_strongbox_device_matrix.md`
- Compliance linkage (if any): AND6 checklist row, evidence log ID

خطة البديل:
- If primary slot unavailable, alternate slot is:
- Needs fallback pool / Firebase? (yes/no)
- External StrongBox retainer required? (yes/no - include lead time)

الموافقات:
- Hardware Lab Lead:
- Android Foundations TL (when CI lanes impacted):
- Program Lead (if StrongBox retainer invoked):

قائمة ما بعد التشغيل:
- Attach Buildkite URL(s):
- Update evidence log row: <ID/date>
- Note deviations/overruns:
```

</div>
