---
lang: ar
direction: rtl
source: docs/portal/docs/soranet/pq-ratchet-runbook.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
المعرف: pq-rachet-runbook
العنوان: تمرين PQ Ratchet في SoraNet
Sidebar_label : دليل PQ Ratchet
الوصف: تمرين خطوات المناوبة اخذ خطوات اخذ اخفاء جواز السفر PQ المرحلية مع التحقق من القياس عن بعد حتمي.
---

:::ملاحظة المصدر القياسي
احترام هذه الصفحة `docs/source/soranet/pq_ratchet_runbook.md`. حافظ على النسختين متطابقتين حتى يتم تقاعد الوثائق القديمة.
:::

##المحاصيل

هذا الدليل يوجه سلسلة نجاح نجاح لسياسة اخفاء ما بعد الكم (PQ) في SoraNet. يتدرب على الترقية (المرحلة أ -> المرحلة ب -> المرحلة ج) ويقلل من الضبط إلى المرحلة ب/أ عندما يحدد PQ. يحقق ذلك من خلال خطافات القياس عن بعد (`sorafs_orchestrator_policy_events_total`, `sorafs_orchestrator_brownouts_total`, `sorafs_orchestrator_pq_ratio_*`) ويجمع القطع الأثرية لسجل التدريب.

##متطلبات

- حدث ثنائي لـ `sorafs_orchestrator` مع قدرة الترجيح (التزم عند او بعد مرجع الملاعب في `docs/source/soranet/reports/pq_ratchet_validation.md`).
- الوصول الى الحزمة Prometheus/Grafana التي برجر `dashboards/grafana/soranet_pq_ratchet.json`.
- لقطة اسمي للـ دليل الحراسة. جلب وتحقق من نسخة قبل الضربة:

```bash
sorafs_cli guard-directory fetch \
  --url https://directory.soranet.dev/mainnet_snapshot.norito \
  --output ./artefacts/guard_directory_pre_drill.norito \
  --expected-directory-hash <directory-hash-hex>
```

اذا كان دليل التشغيل ينشر JSON فقط، فعد ترميزه الى Norito ثنائي عبر `soranet-directory build` قبل مساعدي التدوير.

- تم التقاط البيانات الوصفية وتجهيز المصنوعات اليدوية لإصدار الذاكرة عبر CLI:

```bash
soranet-directory inspect \
  --snapshot ./artefacts/guard_directory_pre_drill.norito
soranet-directory rotate \
  --snapshot ./artefacts/guard_directory_pre_drill.norito \
  --out ./artefacts/guard_directory_post_drill.norito \
  --keys-out ./artefacts/guard_issuer_rotation --overwrite
```

- نافذة تغيير معتمدة من فرق on-call الخاصة بالشبكات والمراقبة.

##خطوات الترقية

1. **تدقيق المرحلة**

   سجل هذه اللعبة الجديدة:

   ```bash
   sorafs_cli config get --config orchestrator.json sorafs.anonymity_policy
   ```

   توقع `anon-guard-pq` قبل الترقية.

2. **الترقية إلى المرحلة B (الأغلبية PQ)**

   ```bash
   sorafs_cli config set --config orchestrator.json \
     sorafs.anonymity_policy anon-majority-pq
   ```- انتظر >=5 دقيقة حتى بيانات تحديث البيانات.
   - في Grafana (لوحة `SoraNet PQ Ratchet Drill`) لكن ان لوحة "Policy Events" تم عرض `outcome=met` للـ `stage=anon-majority-pq`.
   - التقط لقطة شاشة او JSON للوحة وارفقها بسجلها.

3. **الترقية إلى المرحلة C (Strict PQ)**

   ```bash
   sorafs_cli config set --config orchestrator.json \
     sorafs.anonymity_policy anon-strict-pq
   ```

   - تحقق ان مخططات `sorafs_orchestrator_pq_ratio_*` تجهز الى 1.0.
   - تأكد ان عداد اللون البني يبقى ثابتًا؛ بخلاف ذلك اتبع الخطوات المنخفضة.

##تمرين الخفض / البنيوت

1. **احداث عيوب PQ اصطناعي**

   عطل المرحلات PQ في بيئة الملعب بتقليم دليل الحراسة الى الإدخالات كلاسيكي فقط، ثم اعد تحميل ذاكرة التخزين المؤقت الخاص بالـarchorator:

   ```bash
   sorafs_cli guard-cache prune --config orchestrator.json --keep-classical-only
   ```

2. **مراقبة القياس عن بعد الخاصة بـ brownout**

   - لوحة القيادة: لوحة "Brownout Rate" مقدمة فوق 0.
   -PromQL:`sum(rate(sorafs_orchestrator_brownouts_total{region="$region"}[5m]))`
   - يجب أن يعرف `sorafs_fetch` عن `anonymity_outcome="brownout"` مع `anonymity_reason="missing_majority_pq"`.

3. **التخفيض الى المرحلة ب / المرحلة أ**

   ```bash
   sorafs_cli config set --config orchestrator.json \
     sorafs.anonymity_policy anon-majority-pq
   ```

   اذا شاركت PQ غير كافية، اخفض الى `anon-guard-pq`. تنتهي عند استقرار عدادات اللون البني ويمكن تحسين الترقية.

4. ** استعادة دليل الحراسة **

   ```bash
   sorafs_cli guard-directory import \
     --config orchestrator.json \
     --input ./artefacts/guard_directory_pre_drill.json
   ```

## القياس عن بعد والمصنوعات اليدوية- **لوحة المعلومات:** `dashboards/grafana/soranet_pq_ratchet.json`
- **تنبيهات Prometheus:** متأكد من ان التنبيه brownout لـ `sorafs_orchestrator_policy_events_total` يبقى دون SLO مؤهل (&lt;5% ضمن اي نافذة 10 دقائق).
- **سجل الحوادث:** ارفق مقتطفات القياس عن بعد وملاحظات التشغيل الى `docs/examples/soranet_pq_ratchet_fire_drill.log`.
- **موقع التقاط:** استخدم `cargo xtask soranet-rollout-capture` لنسخ Drill log ولوحة النتائج الى `artifacts/soranet_pq_rollout/<timestamp>/`، وحساب BLAKE3Digest، وانتاج `rollout_capture.json` موقع.

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

ارفق البيانات المولدة والتوقيع بأزمة الحكم.

## التراجع

اذا كشفت عن اختلافات PQ الحقيقية، ابق على المرحلة أ، اخطر الشبكات TL، وارفق المقاييس المجمعة مع دليل فروقات الحرس الى تتبع لاحق. استخدم تصدير دليل الحراسة الذي تم التقاطه مؤخرًا إلى الخدمة الطبيعية.

:::تلميح تغطية كرة القدم
`cargo test -p sorafs_orchestrator pq_ratchet_fire_drill_records_metrics` يمنح شهادة صناعية الذي يدعم هذا العامل.
:::