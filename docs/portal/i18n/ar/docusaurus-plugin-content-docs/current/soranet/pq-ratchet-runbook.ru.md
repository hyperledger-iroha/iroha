---
lang: ar
direction: rtl
source: docs/portal/docs/soranet/pq-ratchet-runbook.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
المعرف: pq-rachet-runbook
العنوان: Учебная тевога PQ Ratchet SoraNet
Sidebar_label: Runbook PQ Ratchet
الوصف: بروفة عند الطلب لتعزيز أو متابعة سياسة عدم الكشف عن هوية PQ مع التحقق من صحة القياس عن بعد.
---

:::note Канонический источник
هذا الجزء من الزر `docs/source/soranet/pq_ratchet_runbook.md`. قم بالنسخ المتزامن حتى لا يتم عرض المستندات التالية من الاستثناءات.
:::

## روعة

يسرد هذا الدليل تدريبات مكافحة الحرائق اللاحقة لسياسة إخفاء الهوية بعد الكم (PQ) SoraNet. يقوم المشغلون بالترقية مثل (المرحلة أ -> المرحلة ب -> المرحلة ج)، وكذلك خفض الرتبة بشكل متحكم فيه إلى المرحلة ب/أ عند توريد PQ. حفر خطافات القياس عن بعد (`sorafs_orchestrator_policy_events_total`، `sorafs_orchestrator_brownouts_total`، `sorafs_orchestrator_pq_ratio_*`) واحصل على المصنوعات اليدوية لسجل بروفة الحادث.

## المتطلبات الأساسية

- أعلى `sorafs_orchestrator` ثنائي مع ترجيح القدرة (ارتكاب رافدين أو قم بإجراء تدريب مرجعي من `docs/source/soranet/reports/pq_ratchet_validation.md`).
- قم بالتوصيل إلى Prometheus/Grafana المكدس الذي يخدم `dashboards/grafana/soranet_pq_ratchet.json`.
- لقطة من دليل الحرس الاسمي. احصل على النسخة وتحقق منها أثناء الحفر:

```bash
sorafs_cli guard-directory fetch \
  --url https://directory.soranet.dev/mainnet_snapshot.norito \
  --output ./artefacts/guard_directory_pre_drill.norito \
  --expected-directory-hash <directory-hash-hex>
```

إذا قام الدليل المصدر بنشر JSON فقط، فقم بترميزه في Norito ثنائي عبر `soranet-directory build` قبل تشغيل مساعدي التدوير.

- قم بتزويد البيانات الوصفية وعناصر ما قبل المرحلة بتدوير المُصدر باستخدام CLI:

```bash
soranet-directory inspect \
  --snapshot ./artefacts/guard_directory_pre_drill.norito
soranet-directory rotate \
  --snapshot ./artefacts/guard_directory_pre_drill.norito \
  --out ./artefacts/guard_directory_post_drill.norito \
  --keys-out ./artefacts/guard_issuer_rotation --overwrite
```- تغيير نافذة الأوامر المتاحة عند الاتصال بالشبكات وإمكانية المراقبة.

## خطوات الترويج

1. **مرحلة التدقيق**

   إكمال المرحلة الأولى:

   ```bash
   sorafs_cli config get --config orchestrator.json sorafs.anonymity_policy
   ```

   قبل الترويج، راجع `anon-guard-pq`.

2. **الترقية في المرحلة ب (الأغلبية PQ)**

   ```bash
   sorafs_cli config set --config orchestrator.json \
     sorafs.anonymity_policy anon-majority-pq
   ```

   - تقديم >= 5 دقائق لبيانات الإخطار.
   - في Grafana (لوحة المعلومات `SoraNet PQ Ratchet Drill`) يتم الاتصال به، حيث تشير لوحة "أحداث السياسة" إلى `outcome=met` لـ `stage=anon-majority-pq`.
   - قم بسحب لقطة الشاشة أو لوحة JSON واستخدم سجل الحوادث.

3. **الترقية في المرحلة ج (PQ الصارمة)**

   ```bash
   sorafs_cli config set --config orchestrator.json \
     sorafs.anonymity_policy anon-strict-pq
   ```

   - التحقق من أن الرسم البياني `sorafs_orchestrator_pq_ratio_*` يمتد إلى 1.0.
   - تأكد من أن عداد انقطاع التيار الكهربائي يظل ممتلئًا ؛ لقد تم بالفعل حذف الرتبة.

## حفر التخفيض / انقطاع التيار الكهربائي

1. ** تكوين العجز الاصطناعي PQ **

   قم بإلغاء قفل مرحلات PQ في الملعب، وغطي دليل الحماية للإدخالات الكلاسيكية، ثم قم بإعادة تصميم ذاكرة التخزين المؤقت للمنسق:

   ```bash
   sorafs_cli guard-cache prune --config orchestrator.json --keep-classical-only
   ```

2. **ملاحظة انقطاع التيار الكهربائي عن بعد**

   - لوحة المعلومات: لوحة "Brownout Rate" تشير إلى 0.
   -PromQL:`sum(rate(sorafs_orchestrator_brownouts_total{region="$region"}[5m]))`
   - `sorafs_fetch` يجب الاتصال بـ `anonymity_outcome="brownout"` مع `anonymity_reason="missing_majority_pq"`.

3. **التخفيض إلى المرحلة ب / المرحلة أ**

   ```bash
   sorafs_cli config set --config orchestrator.json \
     sorafs.anonymity_policy anon-majority-pq
   ```

   إذا كان توريد PQ غير مستقر، فاتصل بـ `anon-guard-pq`. تم الانتهاء من الحفر عندما تستقر عدادات انقطاع التيار الكهربائي ويمكن أن تعود العروض الترويجية مرة أخرى.4. ** دليل حراسة التوريدات **

   ```bash
   sorafs_cli guard-directory import \
     --config orchestrator.json \
     --input ./artefacts/guard_directory_pre_drill.json
   ```

## القياس عن بعد والمصنوعات اليدوية

- **لوحة المعلومات:** `dashboards/grafana/soranet_pq_ratchet.json`
- **تنبيهات Prometheus:** تأكد من أن تنبيه انقطاع التيار الكهربائي `sorafs_orchestrator_policy_events_total` يخلف SLO صغيرًا جدًا (<5% في أي وقت خلال 10 دقائق).
- **سجل الحوادث:** قم باستخدام مقتطفات القياس عن بعد ومشغلي الهواتف في `docs/examples/soranet_pq_ratchet_fire_drill.log`.
- **الالتقاط الموقع:** استخدم `cargo xtask soranet-rollout-capture` لتتمكن من نسخ سجل الحفر ولوحة النتائج في `artifacts/soranet_pq_rollout/<timestamp>/`، وتصفح ملخصات BLAKE3 وإنشاء قائمة `rollout_capture.json`.

المثال التمهيدي:

```
cargo xtask soranet-rollout-capture \
  --log logs/pq_fire_drill.log \
  --artifact kind=scoreboard,path=artifacts/canary.scoreboard.json \
  --artifact kind=fetch-summary,path=artifacts/canary.fetch.json \
  --key secrets/pq_rollout_signing_ed25519.hex \
  --phase ramp \
  --label "drill-2026-02-21"
```

قم باستخدام البيانات الوصفية والتوقيع بشكل عام ضمن حزمة الحوكمة.

## التراجع

إذا قمت بالتمرين، قم بإلقاء نظرة حقيقية على PQ، والتوقف عن المرحلة A، وتعلم Networking TL، واستخدم مقاييس الجودة الموجودة في دليل الحماية diffs من تعقب الحوادث. استخدم نطاقًا واسعًا من تصدير دليل الحراسة اللازمة لدعم الخدمة العادية.

:::نصيحة تغطية الانحدار
`cargo test -p sorafs_orchestrator pq_ratchet_fire_drill_records_metrics` يوفر التحقق الاصطناعي الذي يدعم هذا المثقاب.
:::