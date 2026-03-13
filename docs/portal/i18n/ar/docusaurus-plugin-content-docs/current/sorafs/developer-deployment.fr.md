---
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/developer-deployment.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
المعرف: نشر المطور
العنوان: ملاحظات النشر SoraFS
Sidebar_label: ملاحظات النشر
الوصف: قائمة مرجعية لتعزيز خط الأنابيب SoraFS لـ CI مقابل الإنتاج.
---

:::ملاحظة المصدر الكنسي
:::

# ملاحظات النشر

يعزز سير عمل التعبئة والتغليف SoraFS التحديد، مما يؤدي إلى تمرير CI إلى الإنتاج الضروري خارج العمليات الوقائية. استخدم قائمة التحقق هذه عند نشر الاستخدام على بوابات ومزودي المخزون الحقيقي.

## المجلد المسبق

- **محاذاة السجل** — تأكد من أن ملفات التعريف والبيانات تشير إلى نفس الصف `namespace.name@semver` (`docs/source/sorafs/chunker_registry.md`).
- **سياسة القبول** — قم بإعادة عرض إعلانات الموردين الموقعة والأسماء المستعارة اللازمة لـ `manifest submit` (`docs/source/sorafs/provider_admission_policy.md`).
- **Runbook du pin Registration** — احتفظ بـ `docs/source/sorafs/runbooks/pin_registry_ops.md` للفتح لسيناريوهات التكرار (تدوير الأسماء المستعارة، وعمليات النسخ المتماثل).

## تكوين البيئة- تحتاج البوابات إلى تنشيط نقطة نهاية البث التجريبي (`POST /v2/sorafs/proof/stream`) حتى يتمكن CLI من استكمال السيرة الذاتية للقياس عن بعد.
- قم بتكوين السياسة `sorafs_alias_cache` باستخدام القيم الافتراضية `iroha_config` أو مساعد CLI (`sorafs_cli manifest submit --alias-*`).
- قم بتوفير الرموز المميزة (أو المعرفات Torii) عبر إدارة الأسرار الآمنة.
- تنشيط مُصدِّري الاتصالات عن بعد (`torii_sorafs_proof_stream_*`, `torii_sorafs_chunk_range_*`) وإرسالها إلى مكدسك Prometheus/OTel.

## استراتيجية الطرح1. **يظهر باللون الأزرق/الأخضر**
   - استخدم `manifest submit --summary-out` لأرشفة كل ردود الطرح.
   - مراقبة `torii_sorafs_gateway_refusals_total` لاكتشاف عدم تطابق السعة.
2. **التحقق من صحة الأدلة**
   - استخدم أقفال `sorafs_cli proof stream` كحواجز للنشر؛ تظهر صور الكمون أنها عبارة عن مزود اختناق أو طبقات تم تكوينها بشكل سيء.
   - `proof verify` قم بإجراء جزء من اختبار الدخان بعد الدبوس للتأكد من أن السيارة متوقفة من قبل الموردين تتوافق طوال الوقت مع ملخص البيان.
3. **لوحات التحكم عن بعد**
   - قم باستيراد `docs/examples/sorafs_proof_streaming_dashboard.json` في Grafana.
   - أضف اللوحات لسلامة السجل (`docs/source/sorafs/runbooks/pin_registry_ops.md`) وإحصائيات النطاق.
4. ** التنشيط متعدد المصادر **
   - متابعة خطوات الطرح التقدمية في `docs/source/sorafs/runbooks/multi_source_rollout.md` عند تنشيط المُنسق، وأرشفة لوحة النتائج/المقياس عن بعد لإجراء عمليات التدقيق.

## إدارة الحوادث- Suivez les chemins d'escalade dans `docs/source/sorafs/runbooks/` :
  - `sorafs_gateway_operator_playbook.md` لألواح البوابة وتشغيل الرموز المميزة للبث.
  - `dispute_revocation_runbook.md` عند إجراء النسخ المتماثل.
  - `sorafs_node_ops.md` لصيانة مستوى البيانات.
  - `multi_source_rollout.md` لتجاوزات التنسيق والقائمة السوداء للأقران وعمليات الطرح عبر الأشرطة.
- قم بتسجيل فحوصات البراهين وشذوذات التأخير في GovernanceLog عبر واجهة برمجة التطبيقات الخاصة بـ PoR لتتبع الموجودين حتى تتمكن الحوكمة من تقييم أداء الموردين.

## Prochaines étapes

- قم بدمج أتمتة المُنسق (`sorafs_car::multi_fetch`) عندما يكون الجلب المُنسق متعدد المصادر (SF-6b) متاحًا.
- متابعة التحديثات اليومية PDP/PoTR sous SF-13/SF-14 ؛ يتطور كل من CLI والمستندات لكشف المواعيد النهائية واختيار الطبقات مرة واحدة من هذه الأدلة المستقرة.

من خلال الجمع بين ملاحظات النشر هذه مع Quickstart وتسجيلات CI، يمكن للمعدات اجتياز التجارب المحلية على خطوط الأنابيب SoraFS في الإنتاج باستخدام عملية متكررة ويمكن ملاحظتها.