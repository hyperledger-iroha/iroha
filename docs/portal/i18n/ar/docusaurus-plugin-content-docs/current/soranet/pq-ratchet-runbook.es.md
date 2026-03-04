---
lang: ar
direction: rtl
source: docs/portal/docs/soranet/pq-ratchet-runbook.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
المعرف: pq-rachet-runbook
العنوان: Simulacro de PQ Ratchet de SoraNet
Sidebar_label: Runbook de PQ Ratchet
الوصف: خطوة بخطوة لحماية تعزيز أو تغيير السياسة المجهولة PQ المتصاعدة مع التحقق من صحة القياس عن بعد.
---

:::ملاحظة فوينتي كانونيكا
هذه الصفحة تعكس `docs/source/soranet/pq_ratchet_runbook.md`. Manten ambas copias syncronizadas.
:::

## اقتراح

دليل التشغيل هذا هو دليل محاكاة الأمان لسياسة المجهول بعد الكم (PQ) المتصاعدة من SoraNet. يحاول المشغلون الترويج (المرحلة أ -> المرحلة ب -> المرحلة ج) مثل التحكم في التدهور من خلال العودة إلى المرحلة ب/أ عندما يتم توفير PQ. تعمل المحاكاة على التحقق من خطافات القياس عن بعد (`sorafs_orchestrator_policy_events_total`، `sorafs_orchestrator_brownouts_total`، `sorafs_orchestrator_pq_ratio_*`) وتجميع العناصر لسجل بروفة الأحداث.

## المتطلبات الأساسية

- آخر ثنائي `sorafs_orchestrator` مع ترجيح القدرة (يتم الالتزام به بعد مرجع محاكاة العرض في `docs/source/soranet/reports/pq_ratchet_validation.md`).
- قم بالوصول إلى Prometheus/Grafana الذي سيخدم `dashboards/grafana/soranet_pq_ratchet.json`.
- لقطة من دليل الحرس الاسمي. قم بتتبع النسخة والتحقق منها قبل المحاكاة:

```bash
sorafs_cli guard-directory fetch \
  --url https://directory.soranet.dev/mainnet_snapshot.norito \
  --output ./artefacts/guard_directory_pre_drill.norito \
  --expected-directory-hash <directory-hash-hex>
```

إذا قام الدليل المصدر بنشر JSON فقط، فأعد ترميز Norito الثنائي مع `soranet-directory build` قبل تشغيل مساعدي التدوير.

- التقاط البيانات الوصفية والتصنيع المسبق لتدوير المُصدر باستخدام CLI:

```bash
soranet-directory inspect \
  --snapshot ./artefacts/guard_directory_pre_drill.norito
soranet-directory rotate \
  --snapshot ./artefacts/guard_directory_pre_drill.norito \
  --out ./artefacts/guard_directory_post_drill.norito \
  --keys-out ./artefacts/guard_issuer_rotation --overwrite
```- نافذة تغيير مناسبة للمعدات عند الطلب للتواصل وإمكانية المراقبة.

## خطوة للترويج

1. **مرحلة التدقيق**

   تسجيل المرحلة الأولية:

   ```bash
   sorafs_cli config get --config orchestrator.json sorafs.anonymity_policy
   ```

   Espera `anon-guard-pq` قبل الترويج.

2. ** الترويج أ المرحلة ب (الأغلبية PQ) **

   ```bash
   sorafs_cli config set --config orchestrator.json \
     sorafs.anonymity_policy anon-majority-pq
   ```

   - ترقب >= 5 دقائق حتى يتم تجديد البيانات.
   - في Grafana (لوحة المعلومات `SoraNet PQ Ratchet Drill`) تؤكد لوحة "أحداث السياسة" أنها `outcome=met` لـ `stage=anon-majority-pq`.
   - التقط لقطة شاشة للوحة JSON وأضفها إلى سجل الأحداث.

3. ** الترويج للمرحلة C (PQ الصارم) **

   ```bash
   sorafs_cli config set --config orchestrator.json \
     sorafs.anonymity_policy anon-strict-pq
   ```

   - التحقق من أن المدرج الإحصائي `sorafs_orchestrator_pq_ratio_*` متصل بـ 1.0.
   - تأكد من أن جهاز التحكم في اللون البني المسطح ثابت؛ إذا لم يكن الأمر كذلك، sigue los pasos degradacion.

## محاكاة التحلل/البني

1. **الحث على الخروج من PQ**

   تقوم Deshabilita بترحيل PQ في ساحة اللعب التي ترجع دليل الحراسة إلى الإدخالات الكلاسيكية فقط، وتقوم بإعادة شحن ذاكرة التخزين المؤقت للمنسق:

   ```bash
   sorafs_cli guard-cache prune --config orchestrator.json --keep-classical-only
   ```

2. **ملاحظة قياس انقطاع التيار الكهربائي**

   - لوحة المعلومات: لوحة "Brownout Rate" تحت رقم 0.
   -PromQL:`sum(rate(sorafs_orchestrator_brownouts_total{region="$region"}[5m]))`
   - `sorafs_fetch` debe reportar `anonymity_outcome="brownout"` مع `anonymity_reason="missing_majority_pq"`.

3. **التدهور إلى المرحلة ب / المرحلة أ**

   ```bash
   sorafs_cli config set --config orchestrator.json \
     sorafs.anonymity_policy anon-majority-pq
   ```إذا كان إمداد PQ غير كافٍ، فسيتدهور إلى `anon-guard-pq`. تنتهي المحاكاة عندما تستقر قواطع التشغيل ويمكن تكرار العروض الترويجية.

4. **دليل مطعم الحارس**

   ```bash
   sorafs_cli guard-directory import \
     --config orchestrator.json \
     --input ./artefacts/guard_directory_pre_drill.json
   ```

## القياس عن بعد والمصنوعات اليدوية

- **لوحة المعلومات:** `dashboards/grafana/soranet_pq_ratchet.json`
- **التنبيهات Prometheus:** تأكد من الحفاظ على تنبيه انقطاع التيار الكهربائي `sorafs_orchestrator_policy_events_total` حتى يتم تكوين SLO (<5% في أي نافذة لمدة 10 دقائق).
- **سجل الحوادث:** ملحق بمقتطفات القياس عن بعد وملاحظات المشغل على `docs/examples/soranet_pq_ratchet_fire_drill.log`.
- **الالتقاط الثابت:** يستخدم `cargo xtask soranet-rollout-capture` لنسخ سجل الحفر ولوحة النتائج في `artifacts/soranet_pq_rollout/<timestamp>/`، وحساب الملخصات BLAKE3 وإنتاج `rollout_capture.json` الثابت.

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

قم بإضافة البيانات التعريفية التي تم إنشاؤها والشركة ضمن حزمة الإدارة.

## التراجع

إذا اكتشفت المحاكاة أنها حقيقية لـ PQ، فاستمر في المرحلة A، وإخطار Networking TL وأضف المقاييس المجمعة جنبًا إلى جنب مع اختلافات دليل الحماية مثل تعقب الحوادث. تم التقاط دليل حماية التصدير مسبقًا لاستعادة الخدمة العادية.

:::تلميح Cobertura de regression
`cargo test -p sorafs_orchestrator pq_ratchet_fire_drill_records_metrics` يقدم التحقق الاصطناعي الذي يتم الرد عليه بشكل محاكاة.
:::