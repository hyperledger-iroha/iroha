---
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/developer-deployment.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
المعرف: نشر المطور
العنوان: ملاحظات النشر da SoraFS
Sidebar_label: ملاحظات النشر
الوصف: قائمة مرجعية لتعزيز خط الأنابيب SoraFS de CI للإنتاج.
---

:::ملاحظة فونتي كانونيكا
هذه الصفحة تحمل عنوان `docs/source/sorafs/developer/deployment.md`. Mantenha ambas as copias sincronzadas.
:::

# ملاحظات النشر

إن سير عمل التعبئة في SoraFS يعزز التحديد، بما في ذلك مرور CI لإنتاج عمليات حواجز الحماية بشكل أساسي. استخدم قائمة التحقق هذه كأدوات للبوابات ومزودي تخزين حقيقيين.

## ما قبل الرحلة

- **إضافة التسجيل** - تأكد من أن بيانات التقطيع والبيانات تشير إلى نفس المستوى `namespace.name@semver` (`docs/source/sorafs/chunker_registry.md`).
- **سياسة القبول** - قم بمراجعة إعلانات مقدمي الخدمة التي تم الاعتداء عليها والأسماء المستعارة اللازمة لـ `manifest submit` (`docs/source/sorafs/provider_admission_policy.md`).
- **Runbook do pin Registration** - احتفظ بـ `docs/source/sorafs/runbooks/pin_registry_ops.md` من أجل سيناريوهات الاسترداد (تدوير الاسم المستعار، falhas de النسخ المتماثل).

## تكوين البيئة- يجب على البوابات تمكين نقطة نهاية تدفق إثبات (`POST /v1/sorafs/proof/stream`) حتى يقوم CLI بإصدار استئناف القياس عن بعد.
- قم بتكوين السياسة `sorafs_alias_cache` باستخدام الإعدادات في `iroha_config` أو مساعد CLI (`sorafs_cli manifest submit --alias-*`).
- رموز دفق Forneca (ou credenciais Torii) عبر um Secret manager seguro.
- مصدرو القياس عن بعد المؤهلون (`torii_sorafs_proof_stream_*`، `torii_sorafs_chunk_range_*`) ويرسلون إلى مكدسهم Prometheus/OTel.

## استراتيجية الطرح

1. **يظهر باللون الأزرق/الأخضر**
   - استخدم `manifest submit --summary-out` للحصول على إجابات كل طرح.
   - لاحظ `torii_sorafs_gateway_refusals_total` لالتقاط عدم تطابق سعة السعة.
2. **التحقق من البراهين**
   - قم بالتعامل مع `sorafs_cli proof stream` مثل حواجز النشر؛ تشير صور زمن الوصول إلى التقييد الذي تم إثباته أو تكوينه بشكل غير صحيح.
   - `proof verify` يجب أن يقوم بجزء من اختبار الدخان pos-pin لضمان أن يكون مستشفى CAR في المنزل مطابقًا لملخص البيان.
3. **لوحات القياس عن بعد**
   - استيراد `docs/examples/sorafs_proof_streaming_dashboard.json` ولا Grafana.
   - أضف المزيد من التفاصيل إلى سجل الدبوس (`docs/source/sorafs/runbooks/pin_registry_ops.md`) وإحصائيات النطاق.
4. ** هابيليتاكاو متعدد المصادر **
   - قم بتمرير خطوات التشغيل على الخطوات في `docs/source/sorafs/runbooks/multi_source_rollout.md` للتأثير على المنسق واحفظ عناصر لوحة النتائج/القياس عن بعد للمستمعين.

## علاج الحوادث- متابعة طرق التصعيد على `docs/source/sorafs/runbooks/`:
  - `sorafs_gateway_operator_playbook.md` لبوابة البوابة ورمز الدفق المميز.
  - `dispute_revocation_runbook.md` عند إجراء نزاعات النسخ المتماثلة.
  - `sorafs_node_ops.md` للصيانة على مستوى العقدة.
  - `multi_source_rollout.md` لتجاوزات orquestrador وإدراج النظراء في القائمة السوداء وعمليات الطرح في الخطوات.
- قم بتسجيل أخطاء البراهين والشذوذات في زمن الانتظار في GovernanceLog عبر واجهات برمجة التطبيقات الخاصة بـ PoR لتتبع الموجودات حتى تتمكن الإدارة من تحقيق أو تصميم الموثقين.

## بروكسيموس باسوس

- قم بدمج محرك البحث الآلي (`sorafs_car::multi_fetch`) أثناء تشغيل محرك الجلب متعدد المصادر (SF-6b).
- ترقيات مرافقة لـ PDP/PoTR sob SF-13/SF-14؛ o سيتم تطوير CLI والتوثيق لاستعراض المهام واختيار المستويات عند تثبيت هذه الأدلة.

من خلال الجمع بين ملاحظات النشر مع البدء السريع وإيصالات CI، يمكن للمعدات تمرير التجارب الموضعية لخطوط الأنابيب SoraFS من خلال إنتاج عملية متكررة وملاحظة.