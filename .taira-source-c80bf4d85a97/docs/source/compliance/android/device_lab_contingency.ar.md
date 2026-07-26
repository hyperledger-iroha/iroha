---
lang: ar
direction: rtl
source: docs/source/compliance/android/device_lab_contingency.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 4016b82d86dc61a9de5e345950d02aeadf26db4cc26777c60db336c57479ba15
source_last_modified: "2026-01-03T18:07:59.250950+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# سجل طوارئ معمل الجهاز

سجل كل تفعيل لخطة الطوارئ الخاصة بجهاز Android-lab هنا.
قم بتضمين تفاصيل كافية لمراجعات الامتثال وعمليات تدقيق الاستعداد المستقبلية.

| التاريخ | الزناد | الإجراءات المتخذة | متابعات | المالك |
|------|---------|--------------------|-------|------|------|---------|---|
| 2026-02-11 | انخفضت السعة إلى 78% بعد انقطاع ممر Pixel8 Pro وتأخير تسليم Pixel8a (انظر `android_strongbox_device_matrix.md`). | تمت ترقية حارة Pixel7 إلى هدف CI الأساسي، واستعارة أسطول Pixel6 المشترك، واختبارات الدخان المجدولة في Firebase Test Lab لعينة محفظة البيع بالتجزئة، وإشراك مختبر StrongBox الخارجي لكل خطة AND6. | استبدل محور USB-C المعيب لجهاز Pixel8 Pro (مستحق في 15/02/2026)؛ تأكيد وصول Pixel8a وتقرير سعة خط الأساس. | قائد مختبر الأجهزة |
| 2026-02-13 | تم استبدال محور Pixel8 Pro واعتماد GalaxyS24، مما أدى إلى استعادة السعة إلى 85%. | تمت إرجاع حارة Pixel7 إلى المرحلة الثانوية، وتم إعادة تمكين مهمة `android-strongbox-attestation` Buildkite مع العلامات `pixel8pro-strongbox-a` و`s24-strongbox-a`، ومصفوفة الاستعداد المحدثة + سجل الأدلة. | مراقبة الوقت المتوقع لتسليم Pixel8a (لا يزال معلقًا)؛ احتفظ بتوثيق مخزون المركز الاحتياطي. | قائد مختبر الأجهزة |