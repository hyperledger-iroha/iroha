---
lang: ar
direction: rtl
source: docs/portal/docs/soranet/pq-ratchet-runbook.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
المعرف: pq-rachet-runbook
العنوان: SoraNet PQ مثقاب النار السقاطة
Sidebar_label: PQ Ratchet Runbook
الوصف: سياسة إخفاء الهوية في حرب PQ للترويج أو التخفيض من رتبة خطوات التدريب عند الطلب والتحقق من صحة القياس عن بعد الحتمي.
---

:::ملاحظة المصدر الكنسي
هذه هي الصفحة `docs/source/soranet/pq_ratchet_runbook.md`. إذا لم يتم تعيين وثائق برانا للتقاعد، فلا داعي للمزامنة.
:::

## القصد

يحتوي كتاب التشغيل SoraNet على سياسة إخفاء الهوية المرحلية بعد الكم (PQ) والتي تعتمد على تسلسل التدريبات على الحرائق. ترقية المشغلين (المرحلة أ -> المرحلة ب -> المرحلة ج) وإمدادات PQ يتم التحكم فيها عن طريق تخفيض رتبة وابس المرحلة ب/أ دون التمرين. حفر خطافات القياس عن بعد (`sorafs_orchestrator_policy_events_total`، `sorafs_orchestrator_brownouts_total`، `sorafs_orchestrator_pq_ratio_*`) التحقق من صحة كرتا وسجل التدريب على الحوادث من المصنوعات اليدوية جمع كرتا ہے.

## المتطلبات الأساسية

- ترجيح القدرة هو أحدث ثنائي `sorafs_orchestrator` ثنائي (تم تحديد مرجع التدريب على التنفيذ أو بعده `docs/source/soranet/reports/pq_ratchet_validation.md`).
- Prometheus/Grafana مكدس رسالتك `dashboards/grafana/soranet_pq_ratchet.json` يخدم كرتا.
- لقطة دليل الحراسة الاسمية۔ قم بالتمرين ثم قم بنسخ الجلب والتحقق من:

```bash
sorafs_cli guard-directory fetch \
  --url https://directory.soranet.dev/mainnet_snapshot.norito \
  --output ./artefacts/guard_directory_pre_drill.norito \
  --expected-directory-hash <directory-hash-hex>
```

إذا قام الدليل المصدر بنشر JSON، فسيتم استخدام مساعدي التدوير في إعادة ترميز `soranet-directory build` باستخدام Norito.

- التقاط البيانات الوصفية لـ CLI وعناصر تناوب المُصدر في مرحلة ما قبل الإصدار:

```bash
soranet-directory inspect \
  --snapshot ./artefacts/guard_directory_pre_drill.norito
soranet-directory rotate \
  --snapshot ./artefacts/guard_directory_pre_drill.norito \
  --out ./artefacts/guard_directory_post_drill.norito \
  --keys-out ./artefacts/guard_issuer_rotation --overwrite
```- التواصل وإمكانية الملاحظة من خلال فرق تحت الطلب لتغيير النافذة.

## خطوات الترويج

1. **مرحلة التدقيق**

   بداية مرحلة التسجيل:

   ```bash
   sorafs_cli config get --config orchestrator.json sorafs.anonymity_policy
   ```

   الترويج سے پہلے `anon-guard-pq` توقع كریں.

2. **المرحلة B (الأغلبية PQ) للترويج للكريں**

   ```bash
   sorafs_cli config set --config orchestrator.json \
     sorafs.anonymity_policy anon-majority-pq
   ```

   - يظهر التحديث لمدة تزيد عن = 5 دقائق انتظارًا للقراءة.
   - Grafana (لوحة المعلومات `SoraNet PQ Ratchet Drill`) لوحة "أحداث السياسة" الموجودة على `stage=anon-majority-pq` للمتابعة `outcome=met`.
   - لقطة شاشة أو لوحة التقاط JSON وسجل الحوادث يمكن إرفاقها.

3. **المرحلة C (PQ الصارمة) للترويج للعبة**

   ```bash
   sorafs_cli config set --config orchestrator.json \
     sorafs.anonymity_policy anon-strict-pq
   ```

   - `sorafs_orchestrator_pq_ratio_*` الرسوم البيانية 1.0 رأس المال والتحقق من صحة.
   - عداد انقطاع التيار الكهربائي کا رہنا تأكيد کریں؛ رنہ خطوات خفض الرتبة فالو کریں۔

## حفر التخفيض / انقطاع التيار الكهربائي

1. ** نقص PQ الاصطناعي في الدم **

   تعمل بيئة الملعب على حماية الدليل الذي يستخدم الإدخالات الكلاسيكية لقصها وتعطيل مرحلات PQ وتعطيل إعادة تحميل ذاكرة التخزين المؤقت للمنسق:

   ```bash
   sorafs_cli guard-cache prune --config orchestrator.json --keep-classical-only
   ```

2. **ملاحظة قياس انقطاع التيار الكهربائي عن بعد**

   - لوحة المعلومات: لوحة "معدل انقطاع التيار الكهربائي" 0 أعلى ارتفاع کرے۔
   -PromQL:`sum(rate(sorafs_orchestrator_brownouts_total{region="$region"}[5m]))`
   - `sorafs_fetch` إلى `anonymity_outcome="brownout"` و`anonymity_reason="missing_majority_pq"` سجلنا ہئے.

3. **المرحلة ب / المرحلة أ للتخفيض من رتبتك**

   ```bash
   sorafs_cli config set --config orchestrator.json \
     sorafs.anonymity_policy anon-majority-pq
   ```إذا كان عرض PQ غير كافٍ، فسوف يكون `anon-guard-pq` قبل التخفيض. قم بالتمرين في الوقت المناسب بعد أن تستقر عدادات انقطاع التيار الكهربائي وترقيات جديدة مرة أخرى.

4. ** دليل الحرس بحال کریں **

   ```bash
   sorafs_cli guard-directory import \
     --config orchestrator.json \
     --input ./artefacts/guard_directory_pre_drill.json
   ```

## القياس عن بعد والمصنوعات اليدوية

- **لوحة المعلومات:** `dashboards/grafana/soranet_pq_ratchet.json`
- **تنبيهات Prometheus:** تنبيه `sorafs_orchestrator_policy_events_total` لتكوين تنبيه انقطاع التيار الكهربائي (SLO) (&lt;5% في نافذة مدتها 10 دقائق).
- **سجل الحوادث:** يتم إلحاق مقتطفات القياس عن بعد وملاحظات المشغل `docs/examples/soranet_pq_ratchet_fire_drill.log` بالبطاقة.
- **الالتقاط الموقع:** `cargo xtask soranet-rollout-capture` استخدم سجل الحفر ولوحة النتائج في `artifacts/soranet_pq_rollout/<timestamp>/`، قم بنسخه إلى الخارج، BLAKE3 هضم الحساب، ووقع `rollout_capture.json`.

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

يتم إرفاق بيانات التعريف التي تم إنشاؤها وحزمة التوقيع الخاصة بالحوكمة.

## التراجع

إذا قمت بحفر نقص حقيقي في PQ، فستظهر لك المرحلة أ في المرحلة الأولى، وستكون TL الخاصة بالشبكات أكثر وعيًا، والمقاييس المجمعة التي يختلف دليل الحراسة الثابتة التي يمكن إرفاقها بتعقب الحوادث. استخدم الالتقاط الأخير لدليل الحراسة وتصديره واستعادة الخدمة العادية.

:::نصيحة تغطية الانحدار
`cargo test -p sorafs_orchestrator pq_ratchet_fire_drill_records_metrics` هو مثقاب يدعم التحقق من الصحة الاصطناعية.
:::