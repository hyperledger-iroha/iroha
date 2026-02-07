---
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/provider-admission-policy.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

> التكيف مع [`docs/source/sorafs/provider_admission_policy.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/provider_admission_policy.md).

# سياسة القبول وهوية الموردين SoraFS (Borrador SF-2b)

تم التقاط هذه الملاحظة للعناصر القابلة للتنفيذ لـ **SF-2b**: تحديدها
تطبيق تدفق القبول ومتطلبات الهوية والحمولات
معتمد لمزودي المستودعات SoraFS. توسيع العملية العالية
المستوى الموصوف في RFC de Arquitectura de SoraFS وتقسيم العمل المتبقي
في أعمال الإبداع القابلة للتجزئة.

## أهداف السياسة

- ضمان أن المشغلين الوحيدين الذين تم التحقق منهم يمكنهم نشر السجلات
  `ProviderAdvertV1` الذي يقبله اللون الأحمر.
- كل مفتاح إعلان عن مستند هوية معتمد من قبل
  الإدارة ونقاط النهاية المعتمدة والحد الأدنى من المساهمة.
- إثبات أدوات التحقق المحددة لـ Torii والبوابات وال
  `sorafs-node` أضف عناصر التحكم نفسها.
- دعم التجديد والإلغاء في حالات الطوارئ دون الحاجة إلى التحديد
  بيئة العمل للأدوات.

## متطلبات الهوية والحصة| المتطلبات | الوصف | قابل للدمج |
|-----------|-------------|------------|
| Procedencia de la clave de anuncio | يجب على الموردين تسجيل رقم واحد من المفاتيح Ed25519 التي تثبت كل إعلان. تعد حزمة القبول بمثابة الفصل العام جنبًا إلى جنب مع شركة إدارة. | موسع المؤشر `ProviderAdmissionProposalV1` مع `advert_key` (32 بايت) والرجوع إليه من السجل (`sorafs_manifest::provider_admission`). |
| بونتيرو دي ستاك | يتطلب القبول `StakePointer` لا يؤدي إلى مجموعة من التراص النشط. | قم بإجراء التحقق من الصحة في `sorafs_manifest::provider_advert::StakePointer::validate()` وتوضيح الأخطاء في CLI/الاختبارات. |
| آداب القضاء | يعلن الموردون الاختصاص القضائي + الاتصال القانوني. | توسيع نطاق العرض باستخدام `jurisdiction_code` (ISO 3166-1 alpha-2) و`contact_uri` اختياري. |
| شهادة نقطة النهاية | سيتم الإعلان عن كل نقطة نهاية من خلال تقرير شهادة mTLS أو QUIC. | تحديد الحمولة Norito `EndpointAttestationV1` وتخزينها من خلال نقطة النهاية داخل حزمة القبول. |

## تدفق القبول1. **إنشاء مشروع**
   - سطر الأوامر: أنادير `cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- provider-admission proposal ...`
     تم إنتاج `ProviderAdmissionProposalV1` + حزمة معتمدة.
   - التحقق: تأمين النطاق المطلوب، الحصة > 0، التعامل مع قانون القطع في `profile_id`.
2. **إندوسو دي غوبيرنانزا**
   - النصيحة الثابتة `blake3("sorafs-provider-admission-v1" || canonical_bytes)` تستخدم الأدوات
     المغلف الموجود (الوحدة النمطية `sorafs_manifest::governance`).
   - يظل المغلف موجودًا في `governance/providers/<provider_id>/admission.json`.
3. **إدراج السجل**
   - تنفيذ وحدة التحقق المشتركة (`sorafs_manifest::provider_admission::validate_envelope`)
     تم إعادة استخدام que Torii/gateways/CLI.
   - قم بتفعيل طريق القبول في Torii لإعادة شراء الإعلانات التي تحتوي على ملخص أو انتهاء صلاحية المظروف.
4. **التجديد والإلغاء**
   - Añadir `ProviderAdmissionRenewalV1` مع تحديثات اختيارية لنقطة النهاية/الحصة.
   - قم بإظهار مسار CLI `--revoke` الذي يسجل سبب الإلغاء ويرسل حدث إدارة.

## خطوات التنفيذ

| المنطقة | تاريا | المالك (المالكون) | حالة |
|------|-------|----------|--------|
| اسكيما | تعريف `ProviderAdmissionProposalV1`، `ProviderAdmissionEnvelopeV1`، `EndpointAttestationV1` (Norito) من `crates/sorafs_manifest/src/provider_admission.rs`. تم التنفيذ على `sorafs_manifest::provider_admission` مع مساعدي التحقق من الصحة. 【F:crates/sorafs_manifest/src/provider_admission.rs#L1】 | التخزين / الحوكمة | ✅ كامل |
| الأدوات CLI | الموسع `sorafs_manifest_stub` مع الأوامر الفرعية: `provider-admission proposal`، `provider-admission sign`، `provider-admission verify`. | الأدوات مجموعة العمل | ✅ |يقبل تدفق CLI الآن حزم الشهادات الوسيطة (`--endpoint-attestation-intermediate`)، ويصدر وحدات البايت القانونية للنشر/المغلف والتحقق من صحة النصائح خلال `sign`/`verify`. يمكن للمشغلين توفير كتلة إعلانية مباشرة أو إعادة استخدام إعلانات الشركة، ويمكن لملفات الشركة دمج `--council-signature-public-key` مع `--council-signature-file` لتسهيل الأتمتة.

### مرجع CLI

قم بتشغيل كل أمر عبر `cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- provider-admission ...`.-`proposal`
  - الأعلام المطلوبة: `--provider-id=<hex32>`، `--chunker-profile=<namespace.name@semver>`،
    `--stake-pool-id=<hex32>`، `--stake-amount=<amount>`، `--advert-key=<hex32>`،
    `--jurisdiction-code=<ISO3166-1>`، وعلى الأقل `--endpoint=<kind:host>`.
  - التحقق من نقطة النهاية المتوقعة `--endpoint-attestation-attested-at=<secs>`،
    `--endpoint-attestation-expires-at=<secs>`، شهادة عبر
    `--endpoint-attestation-leaf=<path>` (والمزيد `--endpoint-attestation-intermediate=<path>`)
    اختياري لكل عنصر من السلسلة) وأي معرف ALPN متفاوض عليه
    (`--endpoint-attestation-alpn=<token>`). يمكن لنقاط النهاية QUIC تلخيص تقارير النقل
    `--endpoint-attestation-report[-hex]=...`.
  - الخروج: وحدات البايت الأساسية للعرض Norito (`--proposal-out`) واستئناف JSON
    (الإصدار القياسي بسبب الخلل o `--json-out`).
-`sign`
  - المدخلات: عرض (`--proposal`)، إعلان ثابت (`--advert`)، جسم إعلان اختياري
    (`--advert-body`)، عصر الاحتفاظ وأقل شركة للمشورة. يمكن أن تكون الشركات
    تلخيص مضمن (`--council-signature=<signer_hex:signature_hex>`) أو عبر الأرشيفات المجمعة
    `--council-signature-public-key` مع `--council-signature-file=<path>`.
  - إنتاج مظروف صالح (`--envelope-out`) وتقرير JSON يشير إلى روابط الملخص،
    حساب الشركات وطرق الدخول.
-`verify`
  - صلاحية المظروف الموجود (`--envelope`)، مع التحقق الاختياري من العرض،
    الإعلان أو جسم الإعلان المقابل. تقرير JSON destaca valores de Digest، estado
    التحقق من الشركات وما هي المصنوعات الاختيارية المتطابقة.
-`renewal`- تم استلام مظروف مغلف من قبل الملخص الذي تم التصديق عليه مسبقًا. مطلوب
    `--previous-envelope=<path>` والتابع `--envelope=<path>` (الحمولات النافعة Norito).
    يتحقق CLI من أن الأسماء المستعارة للملف الشخصي والإمكانات وأزرار الإعلانات يمكن أن تظل قابلة للتغيير،
    تتيح لك هذه الخطوات تحديث الحصة ونقاط النهاية والبيانات الوصفية. قم بإصدار البايتات القانونية
    `ProviderAdmissionRenewalV1` (`--renewal-out`) هو استئناف JSON.
-`revoke`
  - قم بإصدار حزمة طوارئ `ProviderAdmissionRevocationV1` لمصدر مظروف خاص بك
    retirarse. يتطلب `--envelope=<path>`، `--reason=<text>`، على الأقل
    `--council-signature`، واختياريًا `--revoked-at`/`--notes`. شركة CLI ثابتة وصالحة
    خلاصة الإلغاء، وكتابة الحمولة Norito عبر `--revocation-out`، وطباعة تقرير JSON
    مع الملخص وحساب الشركات.
| التحقق | قم بتنفيذ أداة التحقق المشاركة باستخدام Torii والبوابات و`sorafs-node`. إثبات الاختبارات الوحدوية + تكامل CLI. 【F:crates/sorafs_manifest/src/provider_admission.rs#L1】【F:crates/iroha_torii/src/sorafs/admission.rs#L1】 | الشبكات TL / التخزين | ✅ كامل || التكامل Torii | قم بتوصيل المدقق عبر عرض الإعلانات على Torii، وقم بإعادة عرض الإعلانات السياسية وأطلق القياس عن بعد. | الشبكات TL | ✅ كامل | Torii الآن مغلفات الشحن (`torii.sorafs.admission_envelopes_dir`)، التحقق من مصادفة الملخص/التأكد أثناء العرض وعرض القياس عن بعد قبول.[F:crates/iroha_torii/src/sorafs/admission.rs#L1][F:crates/iroha_torii/src/sorafs/discovery.rs#L1][F:crates/iroha_torii/src/sorafs/api.rs#L1] |
| تجديد | Añadir esquema de renovación/revocación + helpers de CLI، نشر دليل دورة الحياة في المستندات (إصدار runbook abajo y comandos CLI en `provider-admission renewal`/`revoke`).[crates/sorafs_car/src/bin/sorafs_manifest_stub/provider_admission.rs#L477][docs/source/sorafs/provider_admission_policy.md:120] | التخزين / الحوكمة | ✅ كامل |
| القياس عن بعد | تحديد لوحات المعلومات/التنبيهات `provider_admission` (تجديد سريع، انتهاء صلاحية المغلف). | إمكانية الملاحظة | 🟠 أتقدم | الكونتادور `torii_sorafs_admission_total{result,reason}` موجود؛ لوحات المعلومات/التنبيهات المعلقة. 【F:crates/iroha_telemetry/src/metrics.rs#L3798】【F:docs/source/telemetry.md#L614】 |

### دليل التجديد والإلغاء#### برنامج التجديد (تحديث الحصة/الطوبولوجيا)
1. قم ببناء نفس المستوى من الترويج/الإعلان اللاحق مع `provider-admission proposal` و`provider-admission sign`، وزيادة `--retention-epoch` وتحديث الحصة/نقاط النهاية حسب الضرورة.
2. إخراج
   ```bash
   cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- provider-admission \
     renewal \
     --previous-envelope=governance/providers/<id>/envelope.to \
     --envelope=governance/providers/<id>/envelope_next.to \
     --renewal-out=governance/providers/<id>/renewal.to \
     --json-out=governance/providers/<id>/renewal.json \
     --notes="stake top-up 2025-03"
   ```
   الأمر صالح لمجال القدرة/الوظيفة بدون تغيير عبر
   `AdmissionRecord::apply_renewal`، قم بإصدار `ProviderAdmissionRenewalV1` وقم بإخراج الملخصات إلى
   سجل gobernanza.[crates/sorafs_car/src/bin/sorafs_manifest_stub/provider_admission.rs#L477] 【F:crates/sorafs_manifest/src/provider_admission.rs#L422】
3. قم بإعادة تركيب المظروف السابق في `torii.sorafs.admission_envelopes_dir`، وقم بتأكيد تجديد Norito/JSON في مستودع الإدارة وأضف تجزئة التجديد + فترة الاحتفاظ إلى `docs/source/sorafs/migration_ledger.md`.
4. قم بإعلام المشغلين بأن المغلف الجديد نشط ويراقب `torii_sorafs_admission_total{result="accepted",reason="stored"}` لتأكيد عملية الإدخال.
5. قم بتجديد التركيبات القانونية وتأكيدها عبر `cargo run -p sorafs_car --bin provider_admission_fixtures --features cli`; CI (`ci/check_sorafs_fixtures.sh`) يعمل على التحقق من أن منافذ Norito قابلة للاستمرار.#### إلغاء الطوارئ
1. تحديد المظروف المخترق وإصدار إلغاء:
   ```bash
   cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- provider-admission \
     revoke \
     --envelope=governance/providers/<id>/envelope.to \
     --reason="endpoint compromise" \
     --revoked-at=$(date +%s) \
     --notes="incident-456" \
     --council-signature=<signer_hex:signature_hex> \
     --revocation-out=governance/providers/<id>/revocation.to \
     --json-out=governance/providers/<id>/revocation.json
   ```
   شركة CLI `ProviderAdmissionRevocationV1`، التحقق من مجموعة الشركات عبر
   `verify_revocation_signatures`، وأبلغ عن ملخص الإلغاء. 【crates/sorafs_car/src/bin/sorafs_manifest_stub/provider_admission.rs#L593】【F:crates/sorafs_manifest/src/provider_admission.rs#L486】
2. قم بسحب المظروف من `torii.sorafs.admission_envelopes_dir`، وقم بتوزيع Norito/JSON للإلغاء على ذاكرات الدخول المؤقتة، وقم بتسجيل تجزئة الدافع في إجراءات الإدارة.
3. لاحظ `torii_sorafs_admission_total{result="rejected",reason="admission_missing"}` لتأكيد قيام ذاكرات التخزين المؤقت بإزالة الإعلان المُلغى؛ الحفاظ على مصنوعات الإلغاء بأثر رجعي للحوادث.

## التحقق والقياس عن بعد- تركيبات Añadir ذهبية للتقديم ومغلفات الدخول باجو
  `fixtures/sorafs_manifest/provider_admission/`.
- موسع CI (`ci/check_sorafs_fixtures.sh`) لإعادة إنشاء المغلفات والتحقق منها.
- تتضمن التركيبات التي تم إنشاؤها `metadata.json` مع ملخصات قانونية؛ pruebas afirman المصب
  `proposal_digest_hex` == `ca8e73a1f319ae83d7bd958ccb143f9b790c7e4d9c8dfe1f6ad37fa29facf936`.
- إثبات اختبارات التكامل:
  - Torii قم بإعادة نشر الإعلانات مع مظاريف القبول الخاطئة أو منتهية الصلاحية.
  - El CLI يقوم ذهابًا وإيابًا بالعرض → المغلف → التحقق.
  - يقوم تجديد الإدارة بتدوير شهادة نقطة النهاية دون تغيير معرف المورّد.
- متطلبات القياس عن بعد:
  - يصدر اتصالات `provider_admission_envelope_{accepted,rejected}` وTorii. ✅`torii_sorafs_admission_total{result,reason}` الآن يعرض النتائج المقبولة/المرفوضة.
  - إضافة تنبيهات انتهاء الصلاحية إلى لوحات التحكم (التجديد خلال 7 أيام).

## الخطوة التالية1. ✅ الانتهاء من تعديلات السؤال Norito وتم دمج مساعدي التحقق في
   `sorafs_manifest::provider_admission`. لا توجد حاجة إلى علامات الميزات.
2. ✅ تدفقات CLI (`proposal`, `sign`, `verify`, `renewal`, `revoke`) موثقة وتم تنفيذها عبر اختبارات التكامل؛ احتفظ بنصوص الإدارة المتزامنة مع دليل التشغيل.
3. ✅ Torii الإدخال/الاكتشاف يقوم بإدخال المغلفات وعرض أجهزة القياس عن بعد للقبول/الاسترداد.
4. التركيز على الملاحظة: إنهاء لوحات المعلومات/تنبيهات القبول لإلغاء التحديثات المستحقة خلال عدة أيام (`torii_sorafs_admission_total`، مقاييس انتهاء الصلاحية).