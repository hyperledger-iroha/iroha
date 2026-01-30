---
lang: ur
direction: rtl
source: docs/portal/docs/soranet/pq-ratchet-runbook.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

---
id: pq-ratchet-runbook
title: تمرين PQ Ratchet في SoraNet
sidebar_label: دليل PQ Ratchet
description: خطوات تمرين المناوبة لترقية او خفض سياسة اخفاء الهوية PQ المرحلية مع تحقق telemetry حتمي.
---

:::note المصدر القياسي
تعكس هذه الصفحة `docs/source/soranet/pq_ratchet_runbook.md`. حافظ على النسختين متطابقتين حتى يتم تقاعد الوثائق القديمة.
:::

## الغرض

هذا الدليل يوجه تسلسل تمرين الطوارئ لسياسة اخفاء الهوية post-quantum (PQ) المرحلية في SoraNet. يتدرب المشغلون على الترقية (Stage A -> Stage B -> Stage C) وعلى خفض منضبط الى Stage B/A عندما تنخفض امدادات PQ. يتحقق التمرين من telemetry hooks (`sorafs_orchestrator_policy_events_total`, `sorafs_orchestrator_brownouts_total`, `sorafs_orchestrator_pq_ratio_*`) ويجمع artefacts لسجل تدريب الحوادث.

## المتطلبات

- احدث binary لـ `sorafs_orchestrator` مع capability-weighting (commit عند او بعد مرجع التمرين المعروض في `docs/source/soranet/reports/pq_ratchet_validation.md`).
- الوصول الى حزمة Prometheus/Grafana التي تخدم `dashboards/grafana/soranet_pq_ratchet.json`.
- snapshot اسمي للـ guard directory. اجلب وتحقق من نسخة قبل التمرين:

```bash
sorafs_cli guard-directory fetch \
  --url https://directory.soranet.dev/mainnet_snapshot.norito \
  --output ./artefacts/guard_directory_pre_drill.norito \
  --expected-directory-hash <directory-hash-hex>
```

اذا كان source directory ينشر JSON فقط، فاعد ترميزه الى Norito binary عبر `soranet-directory build` قبل تشغيل helpers التدوير.

- التقط metadata وجهز مسبقا artefacts تدوير issuer عبر CLI:

```bash
soranet-directory inspect \
  --snapshot ./artefacts/guard_directory_pre_drill.norito
soranet-directory rotate \
  --snapshot ./artefacts/guard_directory_pre_drill.norito \
  --out ./artefacts/guard_directory_post_drill.norito \
  --keys-out ./artefacts/guard_issuer_rotation --overwrite
```

- نافذة تغيير معتمدة من فرق on-call الخاصة بالشبكات والمراقبة.

## خطوات الترقية

1. **تدقيق المرحلة**

   سجل المرحلة الابتدائية:

   ```bash
   sorafs_cli config get --config orchestrator.json sorafs.anonymity_policy
   ```

   توقع `anon-guard-pq` قبل الترقية.

2. **الترقية الى Stage B (Majority PQ)**

   ```bash
   sorafs_cli config set --config orchestrator.json \
     sorafs.anonymity_policy anon-majority-pq
   ```

   - انتظر >=5 دقائق حتى تتجدد manifests.
   - في Grafana (لوحة `SoraNet PQ Ratchet Drill`) اكد ان لوحة "Policy Events" تعرض `outcome=met` للـ `stage=anon-majority-pq`.
   - التقط لقطة شاشة او JSON للوحة وارفقها بسجل الحوادث.

3. **الترقية الى Stage C (Strict PQ)**

   ```bash
   sorafs_cli config set --config orchestrator.json \
     sorafs.anonymity_policy anon-strict-pq
   ```

   - تحقق ان مخططات `sorafs_orchestrator_pq_ratio_*` تتجه الى 1.0.
   - تاكد ان عداد brownout يبقى ثابتا؛ خلاف ذلك اتبع خطوات الخفض.

## تمرين الخفض / brownout

1. **احداث نقص PQ اصطناعي**

   عطل relays PQ في بيئة playground بتقليم guard directory الى entries كلاسيكية فقط، ثم اعد تحميل cache الخاص بالـ orchestrator:

   ```bash
   sorafs_cli guard-cache prune --config orchestrator.json --keep-classical-only
   ```

2. **مراقبة telemetry الخاصة بـ brownout**

   - Dashboard: لوحة "Brownout Rate" ترتفع فوق 0.
   - PromQL: `sum(rate(sorafs_orchestrator_brownouts_total{region="$region"}[5m]))`
   - يجب ان يبلغ `sorafs_fetch` عن `anonymity_outcome="brownout"` مع `anonymity_reason="missing_majority_pq"`.

3. **الخفض الى Stage B / Stage A**

   ```bash
   sorafs_cli config set --config orchestrator.json \
     sorafs.anonymity_policy anon-majority-pq
   ```

   اذا بقيت امدادات PQ غير كافية، اخفض الى `anon-guard-pq`. ينتهي التمرين عند استقرار عدادات brownout ويمكن اعادة الترقية.

4. **استعادة guard directory**

   ```bash
   sorafs_cli guard-directory import \
     --config orchestrator.json \
     --input ./artefacts/guard_directory_pre_drill.json
   ```

## Telemetry و artefacts

- **Dashboard:** `dashboards/grafana/soranet_pq_ratchet.json`
- **تنبيهات Prometheus:** تاكد من ان تنبيه brownout لـ `sorafs_orchestrator_policy_events_total` يبقى دون SLO المعتمد (&lt;5% ضمن اي نافذة 10 دقائق).
- **Incident log:** ارفق مقتطفات telemetry وملاحظات المشغل الى `docs/examples/soranet_pq_ratchet_fire_drill.log`.
- **التقاط موقع:** استخدم `cargo xtask soranet-rollout-capture` لنسخ drill log ولوحة النتائج الى `artifacts/soranet_pq_rollout/<timestamp>/`، وحساب BLAKE3 digests، وانتاج `rollout_capture.json` موقع.

مثال:

```
cargo xtask soranet-rollout-capture \
  --log logs/pq_fire_drill.log \
  --artifact kind=scoreboard,path=artifacts/canary.scoreboard.json \
  --artifact kind=fetch-summary,path=artifacts/canary.fetch.json \
  --key secrets/pq_rollout_signing_ed25519.hex \
  --phase ramp \
  --label "drill-2026-02-21"
```

ارفق البيانات المولدة والتوقيع بحزمة governance.

## Rollback

اذا كشف التمرين عن نقص PQ حقيقي، ابق على Stage A، اخطر Networking TL، وارفق المقاييس المجمعة مع فروقات guard directory الى متتبع الحوادث. استخدم تصدير guard directory الذي تم التقاطه سابقا لاستعادة الخدمة الطبيعية.

:::tip تغطية الانحدار
`cargo test -p sorafs_orchestrator pq_ratchet_fire_drill_records_metrics` يوفر التحقق الاصطناعي الذي يدعم هذا التمرين.
:::
