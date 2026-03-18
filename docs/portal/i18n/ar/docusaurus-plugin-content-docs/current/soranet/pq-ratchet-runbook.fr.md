---
lang: ar
direction: rtl
source: docs/portal/docs/soranet/pq-ratchet-runbook.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
المعرف: pq-rachet-runbook
العنوان: محاكاة PQ السقاطة SoraNet
Sidebar_label: Runbook PQ Ratchet
الوصف: حلقات تدريب عند الطلب لتعزيز أو إرجاع سياسة إخفاء الهوية PQ والتحقق من تحديد القياس عن بعد.
---

:::ملاحظة المصدر الكنسي
هذه الصفحة تعكس `docs/source/soranet/pq_ratchet_runbook.md`. احتفظ بنسختين متحاذيتين حتى نهاية الوثائق القديمة.
:::

## موضوعي

يرشد كتاب التشغيل هذا تسلسل التدريبات النارية لسياسة المجهول بعد الكم (PQ) ومن SoraNet. يكرر المشغلون الترويج (المرحلة أ -> المرحلة ب -> المرحلة ج) بالإضافة إلى التحكم في التراجع مقابل المرحلة ب/أ عند عرض PQ أعلى. يطبق المثقاب خطافات القياس عن بعد (`sorafs_orchestrator_policy_events_total`، `sorafs_orchestrator_brownouts_total`، `sorafs_orchestrator_pq_ratio_*`) ويجمع العناصر لسجل بروفة الحادث.

## المتطلبات الأساسية

- Dernier binaire `sorafs_orchestrator` مع ترجيح القدرة (التزم بالمساواة أو الخلفية بمرجع الحفر في `docs/source/soranet/reports/pq_ratchet_validation.md`).
- الوصول إلى المكدس Prometheus/Grafana الذي سيرت `dashboards/grafana/soranet_pq_ratchet.json`.
- لقطة اسمية من دليل الحراسة. استرجع وتحقق من نسخة قبل الحفر:

```bash
sorafs_cli guard-directory fetch \
  --url https://directory.soranet.dev/mainnet_snapshot.norito \
  --output ./artefacts/guard_directory_pre_drill.norito \
  --expected-directory-hash <directory-hash-hex>
```

إذا لم يتم نشر الدليل المصدر JSON، فأعد ترميزه في الملف الثنائي Norito مع `soranet-directory build` قبل تنفيذ مساعدات التدوير.

- التقاط البيانات الوصفية وإعداد عناصر التدوير للمصدر باستخدام واجهة سطر الأوامر (CLI) مسبقًا:

```bash
soranet-directory inspect \
  --snapshot ./artefacts/guard_directory_pre_drill.norito
soranet-directory rotate \
  --snapshot ./artefacts/guard_directory_pre_drill.norito \
  --out ./artefacts/guard_directory_post_drill.norito \
  --keys-out ./artefacts/guard_issuer_rotation --overwrite
```- تمت الموافقة على نافذة التغيير من قبل أجهزة التواصل وإمكانية المراقبة عند الطلب.

## نصوص الترويج

1. **مرحلة التدقيق**

   تسجيل مرحلة المغادرة:

   ```bash
   sorafs_cli config get --config orchestrator.json sorafs.anonymity_policy
   ```

   حضور `anon-guard-pq` الترويج المسبق.

2. **الترقية مقابل المرحلة ب (الأغلبية PQ)**

   ```bash
   sorafs_cli config set --config orchestrator.json \
     sorafs.anonymity_policy anon-majority-pq
   ```

   - انتظر >= 5 دقائق حتى تنفجر البيانات.
   - في Grafana (لوحة المعلومات `SoraNet PQ Ratchet Drill`) أكد أن لوحة "أحداث السياسة" تعرض `outcome=met` لـ `stage=anon-majority-pq`.
   - التقط لقطة شاشة أو لوحة JSON وأرفقها في سجل الحادث.

3. **الترويج مقابل المرحلة ج (PQ الصارم)**

   ```bash
   sorafs_cli config set --config orchestrator.json \
     sorafs.anonymity_policy anon-strict-pq
   ```

   - تحقق من أن الرسوم البيانية `sorafs_orchestrator_pq_ratio_*` تميل إلى الإصدار 1.0.
   - تأكد من بقاء جهاز الكمبيوتر البني في مكانه؛ Sinon suivez les etapes de retrogradation.

## حفر التراجع / انقطاع التيار الكهربائي

1. **الحصول على نقص في مادة PQ الاصطناعية**

   قم بإلغاء تنشيط مرحلات PQ في البيئة المحيطة بدليل الحماية الخاص بكل إدخالات كلاسيكية، ثم قم بإعادة شحن مُنسق ذاكرة التخزين المؤقت:

   ```bash
   sorafs_cli guard-cache prune --config orchestrator.json --keep-classical-only
   ```

2. **مراقبة انقطاع القياس عن بعد**

   - لوحة المعلومات: لوحة "Brownout Rate" تصل إلى 0.
   -PromQL:`sum(rate(sorafs_orchestrator_brownouts_total{region="$region"}[5m]))`
   - `sorafs_fetch` مراسل doit `anonymity_outcome="brownout"` مع `anonymity_reason="missing_majority_pq"`.

3. **المتراجع مقابل المرحلة ب / المرحلة أ**

   ```bash
   sorafs_cli config set --config orchestrator.json \
     sorafs.anonymity_policy anon-majority-pq
   ```إذا كان عرض PQ غير كافٍ، فسيتم الرجوع إلى `anon-guard-pq`. يتم إنهاء المثقاب عندما تستقر الحواسيب وقد يتم إعادة تطبيق العروض الترويجية.

4. ** دليل مطعم لو جارد **

   ```bash
   sorafs_cli guard-directory import \
     --config orchestrator.json \
     --input ./artefacts/guard_directory_pre_drill.json
   ```

## القياس عن بعد والمصنوعات اليدوية

- **لوحة المعلومات:** `dashboards/grafana/soranet_pq_ratchet.json`
- **التنبيهات Prometheus:** تأكد من تنبيه انقطاع التيار الكهربائي لـ `sorafs_orchestrator_policy_events_total` أثناء تكوين SLO (أقل من 5% على كل نافذة لمدة 10 دقائق).
- **سجل الأحداث:** أضف مقتطفات القياس عن بعد والملاحظات التي تعمل على `docs/examples/soranet_pq_ratchet_fire_drill.log`.
- **توقيع الالتقاط:** استخدم `cargo xtask soranet-rollout-capture` لنسخ سجل الحفر ولوحة النتائج في `artifacts/soranet_pq_rollout/<timestamp>/`، وحساب خلاصات BLAKE3 وإنتاج علامة `rollout_capture.json`.

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

قم بتوصيل البيانات الوصفية التي تم إنشاؤها والتوقيع على ملف الإدارة.

## التراجع

إذا كشف الحفر عن نقص حقيقي في PQ، فابق على المرحلة A، وأخطر Networking TL وأرفق المقاييس المجمعة بالإضافة إلى اختلافات دليل الحماية من خلال متتبع الحوادث. استخدم تصدير دليل الحماية بالإضافة إلى استعادة الخدمة العادية.

:::تلميح غطاء الانحدار
`cargo test -p sorafs_orchestrator pq_ratchet_fire_drill_records_metrics` يوفر التحقق من الصحة الاصطناعية التي تدعم CE الحفر.
:::