---
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/provider-admission-policy.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

> متكيف من [`docs/source/sorafs/provider_admission_policy.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/provider_admission_policy.md).

# محقق تسليم السياسة وتحديد الهوية SoraFS (chernovic SF-2b)

يلخص هذا الكتاب النتائج العملية لـ **SF-2b**: قم بالاقتراح و
من المؤكد أن تبدأ عملية التوريد والحاجة إلى الهوية و
شهادة الحمولة النافعة لحماية الشاشة SoraFS. هذه هي المرة الأولى
عملية متقنة للغاية، مكتوبة في مهندسي RFC SoraFS، ومحددة
يتقن العمل على التفوق الهندسي.

## السياسة

- ضمان أنه يمكن لعدد كبير جدًا من المشغلين المعتمدين نشر المستند `ProviderAdvertV1` الذي سيتم تعيينه بشكل أساسي.
- توفير كل المفاتيح اللازمة لتطابق الوثيقة، والحوكمة الشاملة، ونقطة المصادقة، والحد الأدنى من الحصة.
- اقتراح أدوات القياس للفحص مثل Torii والشرائح و`sorafs-node` للفحص الفردي.
- تعزيز التعطيل والتعطيل دون زيادة القدرة على التحديد أو الأدوات الجيدة.

## البحث عن الهوية والحصة| تريبوفانيا | الوصف | النتيجة |
|------------|----------|-----------|
| صيانة مفاتيح الحفر | يجب على مقدمي الخدمة التسجيل باستخدام المفتاح Ed25519 الذي يعرض كل إعلان. يتم توفير النطاق للمفتاح العام في ظل الإدارة المضمنة. | قم بنسخ النظام `ProviderAdmissionProposalV1` إلى `advert_key` (32 بايت) ولا يرتبط بأي مسجل (`sorafs_manifest::provider_admission`). |
| حصة Указатель | من أجل الحصول على شبكة `StakePointer`، يتم وضعها على تجمع التوقيع المساحي النشط. | قم بالتحقق من الصحة في `sorafs_manifest::provider_advert::StakePointer::validate()` وقم بإعادة التحقق من صحة العناصر في CLI/الاختبار. |
| هذه الاختصاصات | يلتزم الموردون بالسلطة القضائية + جهة الاتصال القانونية. | قم بإعادة عرض المخطط المقترح `jurisdiction_code` (ISO 3166-1 alpha-2) و`contact_uri` الاختياري. |
| نقطة التصديق | قد تحتاج أي نقطة توصيل خاصة إلى الحصول على شهادة mTLS أو QUIC رائعة. | قم بتفضيل الحمولة Norito `EndpointAttestationV1` واشحنها عبر نقطة النهاية في النطاق. |

## عملية التسليم1. **اقتراحات الرسالة**
   - سطر الأوامر: إضافة `cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- provider-admission proposal ...`،
     نموذج `ProviderAdmissionProposalV1` + شهادة النطاق.
   - التحقق من الصحة: ​​يتم التحقق من خلال شريط ممتد، حصة > 0، مقبض مقسم قياسي في `profile_id`.
2. **الحوكمة الجيدة**
   - الوصف السوفيتي `blake3("sorafs-provider-admission-v1" || canonical_bytes)` يستخدم المعدات
     أدوات المغلف (الوحدة `sorafs_manifest::governance`).
   - المغلف محفوظ في `governance/providers/<provider_id>/admission.json`.
3. **الخسارة في السجل**
   - تحقيق المدقق الرئيسي (`sorafs_manifest::provider_admission::validate_envelope`)، الذي
     يستخدم Torii/shlюзы/CLI.
   - قم بالدخول إلى طريق الإيداع Torii لإلغاء استنساخ الإعلانات التي يتم حذفها من المغلف أو من خلال الملخص.
4. **الأخبار والملاحظات**
   - إضافة `ProviderAdmissionRenewalV1` مع نقاط/حصة اختيارية.
   - فتح طريق CLI `--revoke`، الذي يقوم بإصلاح سبب الخلل وإصلاح حوكمة الاشتراكات.

## تحقيق المزيد

| Область | زادا | المالك (المالكون) | الحالة |
|---------|-------|---------|--------|
| المخطط | قم باقتراح `ProviderAdmissionProposalV1`، `ProviderAdmissionEnvelopeV1`، `EndpointAttestationV1` (Norito) إلى `crates/sorafs_manifest/src/provider_admission.rs`. تم التحقق من صحتها في `sorafs_manifest::provider_admission` مع التحقق من الصحة. 【F:crates/sorafs_manifest/src/provider_admission.rs#L1】 | التخزين / الحوكمة | ✅ ممتاز |
| أدوات CLI | قم بالرد على `sorafs_manifest_stub`: `provider-admission proposal`، `provider-admission sign`، `provider-admission verify`. | الأدوات مجموعة العمل | ✅ ممتاز |CLI نقطة انطلاق لشهادة العصابات المعززة (`--endpoint-attestation-intermediate`),
قم باستعراض العروض/المغلفات القانونية والتحقق من إرسال الرسائل خلال الوقت `sign`/`verify`. يمكن للمشغلين
قم بإعادة نشر إعلانك أو الاستفادة من الإعلانات المصاحبة، ومن الممكن إضافة ملفات
قم بإعادة الاتصال بـ `--council-signature-public-key` مع `--council-signature-file` للتمتع بالأتمتة.

### CLI الصحيح

اضغط على الأمر من خلال `cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- provider-admission ...`.-`proposal`
  - الأعلام المخصصة: `--provider-id=<hex32>`، `--chunker-profile=<namespace.name@semver>`،
    `--stake-pool-id=<hex32>`، `--stake-amount=<amount>`، `--advert-key=<hex32>`،
    `--jurisdiction-code=<ISO3166-1>`، والحد الأدنى هو `--endpoint=<kind:host>`.
  - للتصديق على نقطة التعادل `--endpoint-attestation-attested-at=<secs>`,
    `--endpoint-attestation-expires-at=<secs>`، الشهادة عبر
    `--endpoint-attestation-leaf=<path>` (بالإضافة إلى `--endpoint-attestation-intermediate=<path>` الاختياري)
    لكل عنصر من العناصر) ومعرف ALPN المفضل
    (`--endpoint-attestation-alpn=<token>`). يمكن لنقاط QUIC أن تسبق منافذ النقل عبر الطريق
    `--endpoint-attestation-report[-hex]=...`.
  - الخروج: عروض البيانات الأساسية Norito (`--proposal-out`) وJSON-swодка
    (stdout по молчанию أو `--json-out`).
-`sign`
  - البيانات الشخصية: عرض (`--proposal`)، إعلان إضافي (`--advert`)، إعلان اختياري
    (`--advert-body`)، فترة الاحتفاظ والحد الأدنى من المعلومات. يمكن إعادة النشر
    مضمّن (`--council-signature=<signer_hex:signature_hex>`) أو عبر الملفات،
    `--council-signature-public-key` مع `--council-signature-file=<path>`.
  - نموذج مظروف صالح (`--envelope-out`) وJSON-Otchet مع ملخص خاص،
    числом подписантов и водными потями.
-`verify`
  - التحقق من المغلف الموجود (`--envelope`) والاختيار الاختياري للاقتراحات المرغوبة،
    إعلان أو إعلان آخر. JSON-outчеt подсвечивае значения ملخص, حالة التحقق من المشاركة
    وكيف يتم توفير القطع الأثرية الاختيارية.
-`renewal`- أرسل مظروفًا جديدًا مع مجموعة واسعة من الملخصات. شكرا
    `--previous-envelope=<path>` واللاحق `--envelope=<path>` (الحمولة الصافية Norito).
    تتحقق واجهة سطر الأوامر (CLI) من أن الأسماء المستعارة للملف الشخصي والإمكانيات ومفتاح الإعلان لا تحتوي على أي أسماء عند تنزيلها
    حصة الاشتراك والنقاط والبيانات الوصفية. استعادة البيانات الأساسية `ProviderAdmissionRenewalV1`
    (`--renewal-out`) بالإضافة إلى JSON-swодку.
-`revoke`
  - قم بشراء النطاق الترددي `ProviderAdmissionRevocationV1` لمقدم الخدمة، حيث يتعين عليك إخراج المغلف.
    مطلوب `--envelope=<path>`، `--reason=<text>`، كحد أدنى `--council-signature` واختياري
    `--revoked-at`/`--notes`. تقوم واجهة سطر الأوامر (CLI) بإدراج والتحقق من ملاحظة الملخص، وتسجيل الحمولة Norito من خلال
    `--revocation-out` وأرسل JSON مع الملخص ثم أكمل القراءة.
| بروفيركا | تحقيق المدقق الرئيسي، مستخدم Torii، والأجزاء، و`sorafs-node`. اقتراح الوحدة + CLI интеграционные тесты.[F:crates/sorafs_manifest/src/provider_admission.rs#L1] 【F:crates/iroha_torii/src/sorafs/admission.rs#L1】 | الشبكات TL / التخزين | ✅ ممتاز || التكامل Torii | قم بإغلاق المدقق للحصول على الإعلانات في Torii، وإلغاء استنساخ الإعلانات في السياسة، ونشر أجهزة القياس عن بعد. | الشبكات TL | ✅ ممتاز | Torii قم بتعبئة مظاريف الإدارة (`torii.sorafs.admission_envelopes_dir`)، وتحقق من الملخص/النشر عن طريق البريد الإلكتروني ومقياس الاتصال العام допуска.【F:crates/iroha_torii/src/sorafs/admission.rs#L1】【F:crates/iroha_torii/src/sorafs/discovery.rs#L1】【F:crates/iroha_torii/src/sorafs/api.rs#L1】 |
| الحداثة | إضافة مخططات الملاحظات/الملاحظات + إرشادات CLI، ونشر دورة الحياة في الوثائق (sm.runbook и и сманды CLI в `provider-admission renewal`/`revoke`).[crates/sorafs_car/src/bin/sorafs_manifest_stub/provider_admission.rs#L477][docs/source/sorafs/provider_admission_policy.md:120] | التخزين / الحوكمة | ✅ ممتاز |
| القياس عن بعد | قم باقتراح لوحات المعلومات/التنبيهات `provider_admission` (الملاحظة المحتملة، المغلف الخاص بالوظيفة). | إمكانية الملاحظة | 🟠 في العملية | Счетчик `torii_sorafs_admission_total{result,reason}` суествует; لوحات المعلومات/التنبيهات في العمل. 【F:crates/iroha_telemetry/src/metrics.rs#L3798】 【F:docs/source/telemetry.md#L614】 |

### مراجعة وملاحظة Runbook#### Planovoе obновление (حصة الملكية/الطوبولوجيا)
1. تعرف على الاقتراحات/الإعلانات التالية من خلال `provider-admission proposal` و`provider-admission sign`،
   `--retention-epoch` ممتاز واحصل على حصة/نقاط جديدة حسب الحاجة.
2. اختر
   ```bash
   cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- provider-admission \
     renewal \
     --previous-envelope=governance/providers/<id>/envelope.to \
     --envelope=governance/providers/<id>/envelope_next.to \
     --renewal-out=governance/providers/<id>/renewal.to \
     --json-out=governance/providers/<id>/renewal.json \
     --notes="stake top-up 2025-03"
   ```
   أمر للتحقق من عدم وجود القدرة/الملف الشخصي من خلال
   `AdmissionRecord::apply_renewal`، قم بتعبئة `ProviderAdmissionRenewalV1` وقم بقراءة الملخصات
   grates/sorafs_car/src/bin/sorafs_manifest_stub/provider_admission.rs#L477 【F:crates/sorafs_manifest/src/provider_admission.rs#L422】
3. قم بحفظ المظروف السابق في `torii.sorafs.admission_envelopes_dir`، وانضم إلى Norito/JSON
   في حوكمة المستودع وقم بإضافة تعريف التجزئة + فترة الاحتفاظ في `docs/source/sorafs/migration_ledger.md`.
4. قم بإبلاغ المشغل بتنشيط المغلف الجديد وتتبعه
   `torii_sorafs_admission_total{result="accepted",reason="stored"}` للتسليم.
5. قم بشراء التركيبات الأساسية وتأكيدها من خلال `cargo run -p sorafs_car --bin provider_admission_fixtures --features cli`؛
   يتحقق CI (`ci/check_sorafs_fixtures.sh`) من استقرار Norito.#### ملاحظة Аvarийный отзыв
1. قم بتقديم المغلف المخصص للكمبيوتر وأدخل الملاحظة:
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
   يدرج CLI `ProviderAdmissionRevocationV1`، وتحقق من نوع ما من خلاله
   `verify_revocation_signatures` و сообщает отзыва.[crates/sorafs_car/src/bin/sorafs_manifest_stub/provider_admission.rs#L593] 【F:crates/sorafs_manifest/src/provider_admission.rs#L486】
2. قم بتعبئة المظروف من `torii.sorafs.admission_envelopes_dir`، ثم قم بإدراج Norito/JSON في حالة القبول
   وقم بتثبيت عناصر التجزئة في بروتوكول الحوكمة.
3. قم بإسقاط `torii_sorafs_admission_total{result="rejected",reason="admission_missing"}` للتأكيد،
   ما الذي تم حذفه من الإعلان; تم العثور على القطع الأثرية الحجرية في حادثة استعادية.

## الاختبار والقياس عن بعد- قم بإضافة التركيبات الذهبية للمقترحات والمغلفات في
  `fixtures/sorafs_manifest/provider_admission/`.
- rasshirить CI (`ci/check_sorafs_fixtures.sh`) لنقل العروض والتحقق من المغلف.
- تتضمن التركيبات الهندسية `metadata.json` مع الملخصات القياسية؛ يتم تنفيذ اختبارات المصب
  `proposal_digest_hex` == `ca8e73a1f319ae83d7bd958ccb143f9b790c7e4d9c8dfe1f6ad37fa29facf936`.
- اقتراح اختبارات التكامل:
  - Torii يقوم بإلغاء استنساخ الإعلانات باستخدام مظاريف القبول أو مظاريف القبول اللاحقة.
  - تقدم CLI عروض ذهابًا وإيابًا → المظروف → التحقق.
  - تؤدي إدارة الحوكمة إلى تصديق نقطة النهاية دون تغيير معرف المزود.
-القياس عن بعد:
  - تنبعث عدادات `provider_admission_envelope_{accepted,rejected}` в Torii. ✅ تم قبول/رفض `torii_sorafs_admission_total{result,reason}`.
  - إضافة الأفضلية على وجودك في المراقبة (يجب أن تكون الموافقة في غضون 7 أيام).

## الخطوات التالية1. ✅ تحسين نظام التحسين Norito وإضافة التحقق من صحة التعزيزات في `sorafs_manifest::provider_admission`. أعلام الميزة ليست ضرورية.
2. ✅ خطوط CLI (`proposal`, `sign`, `verify`, `renewal`, `revoke`) الموثقة والمثبتة اختبارات التكامل; قم بتعزيز إدارة البرامج النصية من خلال المزامنة مع دليل التشغيل.
3. ✅Torii مبدأ القبول/الاكتشاف هو المغلفات ونشر بطاقات القياس عن بعد/إلغاء الحجب.
4. التركيز على المراقبة: إنهاء لوحات المعلومات/التنبيهات عند القبول للإخطارات والمتطلبات في كل يوم، بالإضافة إلى الاقتراحات (`torii_sorafs_admission_total`، مقاييس انتهاء الصلاحية).