---
lang: ar
direction: rtl
source: docs/portal/docs/soranet/pq-ratchet-runbook.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
المعرف: pq-rachet-runbook
العنوان: Simulacro PQ Ratchet do SoraNet
Sidebar_label: Runbook de PQ Ratchet
الوصف: خطوات التدريب عند الطلب لتعزيز أو إصلاح السياسة المجهولة PQ في المواقف مع التحقق من صحة القياس عن بعد.
---

:::ملاحظة فونتي كانونيكا
هذه الصفحة espelha `docs/source/soranet/pq_ratchet_runbook.md`. Mantenha ambas as copias sincronzadas.
:::

## اقتراح

دليل التشغيل هذا عبارة عن تسلسل محاكاة لسياسة مجهولة بعد الكم (PQ) في مواقع SoraNet. يقوم المشغلون بالترويج (المرحلة أ -> المرحلة ب -> المرحلة ج) ويتم التحكم في مستوى الطاقة في المرحلة ب/أ عند تقديم عرض PQ. محاكاة خطافات القياس عن بعد (`sorafs_orchestrator_policy_events_total`، `sorafs_orchestrator_brownouts_total`، `sorafs_orchestrator_pq_ratio_*`) ومجموعة من الأدوات اليدوية لسجل بروفة الأحداث.

## المتطلبات الأساسية

- Ultimo binario `sorafs_orchestrator` com لترجيح القدرة (التزم بالمرجع الصحيح أو الخلفي للحفر على `docs/source/soranet/reports/pq_ratchet_validation.md`).
- الوصول إلى المكدس Prometheus/Grafana الذي يخدم `dashboards/grafana/soranet_pq_ratchet.json`.
- لقطة اسمية تقوم بدليل الحراسة. ابحث عن نسخة صالحة للمحاكاة مسبقًا:

```bash
sorafs_cli guard-directory fetch \
  --url https://directory.soranet.dev/mainnet_snapshot.norito \
  --output ./artefacts/guard_directory_pre_drill.norito \
  --expected-directory-hash <directory-hash-hex>
```

إذا نشر الدليل المصدر apenas JSON، فأعد تشفيره لـ Norito ثنائي com `soranet-directory build` قبل توجيه مساعدي الدوران.

- التقاط البيانات الوصفية والتحف الفنية الدوارة التي تم إصدارها بواسطة CLI:

```bash
soranet-directory inspect \
  --snapshot ./artefacts/guard_directory_pre_drill.norito
soranet-directory rotate \
  --snapshot ./artefacts/guard_directory_pre_drill.norito \
  --out ./artefacts/guard_directory_post_drill.norito \
  --keys-out ./artefacts/guard_issuer_rotation --overwrite
```- Janela de mudanca aprovada pelos times on call de Networking e Observability.

## Passos de promocao

1. **مرحلة التدقيق**

   التسجيل في المرحلة الأولية:

   ```bash
   sorafs_cli config get --config orchestrator.json sorafs.anonymity_policy
   ```

   الرجاء `anon-guard-pq` قبل الترويج.

2. **Promova para Stage B (أغلبية PQ)**

   ```bash
   sorafs_cli config set --config orchestrator.json \
     sorafs.anonymity_policy anon-majority-pq
   ```

   - ضمان >= 5 دقائق لبيانات محدثة تمامًا.
   - لا يوجد Grafana (لوحة المعلومات `SoraNet PQ Ratchet Drill`) يؤكد أن "أحداث السياسة" تظهر `outcome=met` لـ `stage=anon-majority-pq`.
   - التقط لقطة شاشة أو قم برسم JSON وإرفاقها بسجل الأحداث.

3. **Promova para Stage C (Strict PQ)**

   ```bash
   sorafs_cli config set --config orchestrator.json \
     sorafs.anonymity_policy anon-strict-pq
   ```

   - التحقق من أن الرسوم البيانية `sorafs_orchestrator_pq_ratio_*` تميل إلى 1.0.
   - تأكد من بقاء جهاز التحكم في اللون البني ثابتًا؛ Caso Contrario، siga os passos de despromocao.

##ديسبروموكاو / مثقاب براون أوت

1. **الحصول على خلاص من PQ**

   مرحلات إزالة PQ في الملعب المحيطي أو حماية الدليل من الإدخالات الكلاسيكية فقط، بعد إعادة تشغيل ذاكرة التخزين المؤقت للمنسق:

   ```bash
   sorafs_cli guard-cache prune --config orchestrator.json --keep-classical-only
   ```

2. **مراقبة قياس الانخفاض عن بعد**

   - لوحة القيادة: قم برسم "Brownout Rate" حتى النقطة 0.
   -PromQL:`sum(rate(sorafs_orchestrator_brownouts_total{region="$region"}[5m]))`
   - `sorafs_fetch` dere reportar `anonymity_outcome="brownout"` com `anonymity_reason="missing_majority_pq"`.

3. **ديسبروموفا للمرحلة ب / المرحلة أ**

   ```bash
   sorafs_cli config set --config orchestrator.json \
     sorafs.anonymity_policy anon-majority-pq
   ```إذا كان عرض PQ غير كافٍ، فسيتم حذفه لـ `anon-guard-pq`. يمكن للمحاكاة أن تتكرر عندما يتم تثبيت أجهزة التحكم في اللون البني والترويج لها.

4. ** دليل المطاعم والحراسة **

   ```bash
   sorafs_cli guard-directory import \
     --config orchestrator.json \
     --input ./artefacts/guard_directory_pre_drill.json
   ```

## القياس عن بعد والأدوات اليدوية

- **لوحة المعلومات:** `dashboards/grafana/soranet_pq_ratchet.json`
- **التنبيهات Prometheus:** تضمن تنبيه انقطاع التيار الكهربائي لـ `sorafs_orchestrator_policy_events_total` بعد تكوين SLO (&lt;5% في كل 10 دقائق).
- **سجل الحوادث:** ملحق بخطوات القياس عن بعد وملاحظات المشغل على `docs/examples/soranet_pq_ratchet_fire_drill.log`.
- **تم التقاط اللقطة:** استخدم `cargo xtask soranet-rollout-capture` لنسخ سجل الحفر ولوحة النتائج لـ `artifacts/soranet_pq_rollout/<timestamp>/`، والخلاصات الحسابية BLAKE3 وإنتاج `rollout_capture.json`.

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

ملحق البيانات الوصفية الجيدة ومضاف إليها حزمة الحوكمة.

## التراجع

إذا تم الكشف عن المحاكاة الحقيقية لـ PQ، فاستمر في المرحلة A، وأبلغ عن Networking TL وقم بإرفاقها كمقاييس مجمعة جنبًا إلى جنب مع الاختلافات التي تحمي الدليل أو متعقب الحوادث. استخدم التصدير أو حماية الدليل الذي تم التقاطه مسبقًا لاستعادة الخدمة العادية.

:::تلميح Cobertura de regressao
`cargo test -p sorafs_orchestrator pq_ratchet_fire_drill_records_metrics` يتطلب التحقق من الصحة التي تدعم هذه المحاكاة.
:::