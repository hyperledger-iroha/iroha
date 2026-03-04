---
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/provider-admission-policy.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

> التكيف مع [`docs/source/sorafs/provider_admission_policy.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/provider_admission_policy.md).

# سياسة القبول وهوية الموثقين SoraFS (Rascunho SF-2b)

هذه الملاحظة التي تم التقاطها من خلال الإجراءات المتداخلة لـ **SF-2b**: تحديد e
تطبيق تدفق القبول ومتطلبات الهوية والحمولات
التحقق من مخزون التخزين SoraFS. هذه عملية واسعة النطاق أو عالية
المستوى الموضح في RFC Arquitetura de SoraFS وتقسيم العمل المتبقي
Tarefas de engenharia rastreaveis.

## أهداف سياسية

- ضمان أن يقوم المشغلون الذين تم التحقق منهم بنشر السجلات `ProviderAdvertV1` التي يمكنك الحصول عليها.
- كل مرة يجب أن تعلن عن وثيقة هوية معتمدة على الإدارة ونقاط النهاية المعتمدة والمساهمة في الحد الأدنى من الحصة.
- توفر أدوات التحقق الحتمية التي تنطبق على Torii والبوابات و`sorafs-node` كرسائل تحقق.
- دعم التجديد وإعادة التشغيل في حالات الطوارئ دون تغيير التصميم أو بيئة العمل للأجهزة.

## متطلبات الهوية والحصة| المتطلبات | وصف | إنترغافيل |
|-----------|----------|------------|
| Proveniencia da chave de anuncio | يجب على المحامين التسجيل على قدم المساواة مع Ed25519 التي تقوم بتنفيذ كل إعلان. تم نشر حزمة القبول الممنوحة جنبًا إلى جنب مع عملية اغتيال حاكمة. | قم بالإشارة إلى `ProviderAdmissionProposalV1` مع `advert_key` (32 بايت) والمرجع إلى السجل (`sorafs_manifest::provider_admission`). |
| بونتيرو دي ستاك | يتطلب القبول رقم `StakePointer` بدون صفر لتجميع التجميع. | قم بإضافة التحقق من الصحة إلى `sorafs_manifest::provider_advert::StakePointer::validate()` وكشف الأخطاء في CLI/الاختبارات. |
| العلامات القانونية | يعلن المثبتون عن حقهم القانوني + الاتصال القانوني. | قم بطرح سؤال الاقتراح مع `jurisdiction_code` (ISO 3166-1 alpha-2) و`contact_uri` اختياريًا. |
| التحقق من نقطة النهاية | يجب أن يتم الإعلان عن كل نقطة نهاية من خلال علاقة شهادة mTLS أو QUIC. | تحديد الحمولة Norito `EndpointAttestationV1` وتخزينها من خلال نقطة النهاية داخل حزمة القبول. |

## تدفق القبول1. **التفكير في الاقتراح**
   - سطر الأوامر: إضافة `cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- provider-admission proposal ...`
     المنتج `ProviderAdmissionProposalV1` + حزمة التحقق.
   - التحقق من صحة: ضمان النطاقات المطلوبة، الحصة > 0، التعامل مع وحدة تقطيع الكنسي في `profile_id`.
2. ** إندوسو دي غوفرانكا **
   - ننصحك باستخدام `blake3("sorafs-provider-admission-v1" || canonical_bytes)` أو أدوات المغلف الموجودة
     (مودولو `sorafs_manifest::governance`).
   - المغلف والمستمر في `governance/providers/<provider_id>/admission.json`.
3. **التحميل بدون تسجيل**
   - تنفيذ أداة التحقق من المشاركة (`sorafs_manifest::provider_admission::validate_envelope`)
     تم إعادة استخدام que Torii/gateways/CLI.
   - تحديث طريق القبول Torii لتسجيل الإعلانات في المغلف أو انتهاء الصلاحية المختلفة.
4. **التجديد والتجديد**
   - إضافة `ProviderAdmissionRenewalV1` مع تحديثات خيارات نقطة النهاية/الحصة.
   - اعرض طريق CLI `--revoke` الذي يسجل سبب التجديد ويرسل حدثًا للإدارة.

## متطلبات التنفيذ

| المنطقة | طريفة | المالك (المالكون) | الحالة |
|------|--------|----------|--------|
| اسكيما | حدد `ProviderAdmissionProposalV1`، `ProviderAdmissionEnvelopeV1`، `EndpointAttestationV1` (Norito) في `crates/sorafs_manifest/src/provider_admission.rs`. تم التنفيذ بواسطة `sorafs_manifest::provider_admission` مع مساعدي التحقق.[F:crates/sorafs_manifest/src/provider_admission.rs#L1] | التخزين / الحوكمة | الخلاصة |
| فيرامينتاس كلي | Estender `sorafs_manifest_stub` مع الكوماندوز الفرعية: `provider-admission proposal`، `provider-admission sign`، `provider-admission verify`. | الأدوات مجموعة العمل | الخلاصة |تدفق CLI الآن هو حزم الشهادات الوسيطة (`--endpoint-attestation-intermediate`)، قم بإصدارها
البايتات الأساسية للاقتراح/المغلف والمصادقة على التوصيات خلال `sign`/`verify`. مشغلي بودم
إنشاء مجموعة إعلانات مباشرة أو إعادة استخدام الإعلانات المقتولة، ويمكن استخدام ملفات الاختراق
Fornecidos ao combinar `--council-signature-public-key` com `--council-signature-file` لتسهيل تشغيل السيارة.

### مرجع CLI

قم بتنفيذ cada comando عبر `cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- provider-admission ...`.-`proposal`
  - الأعلام المطلوبة: `--provider-id=<hex32>`، `--chunker-profile=<namespace.name@semver>`،
    `--stake-pool-id=<hex32>`، `--stake-amount=<amount>`، `--advert-key=<hex32>`،
    `--jurisdiction-code=<ISO3166-1>`، وأصغر من `--endpoint=<kind:host>`.
  - التحقق من نقطة النهاية المتوقعة `--endpoint-attestation-attested-at=<secs>`،
    `--endpoint-attestation-expires-at=<secs>`، معتمد عبر
    `--endpoint-attestation-leaf=<path>` (ما عدا `--endpoint-attestation-intermediate=<path>`
    اختياري لكل عنصر من العناصر) ومعرفات ALPN التجارية
    (`--endpoint-attestation-alpn=<token>`). نقاط النهاية QUIC podem fornecer relatorios de Transporte com
    `--endpoint-attestation-report[-hex]=...`.
  - صيدا: bytes canonicos de proposta Norito (`--proposal-out`) e um resumo JSON
    (ستدوت بادراو أو `--json-out`).
-`sign`
  - المدخلات: عرض (`--proposal`)، إعلان مُلحق (`--advert`)، مجموعة إعلان اختيارية
    (`--advert-body`)، عصر الاحتفاظ والشعر أقل من مجرد نصيحة. كما assinaaturas podem
    يمكنك تقديم الطلبات المضمّنة (`--council-signature=<signer_hex:signature_hex>`) أو عبر الملفات أو التجميع
    `--council-signature-public-key` كوم `--council-signature-file=<path>`.
  - إنتاج مظروف صالح (`--envelope-out`) وعلاقة JSON تشير إلى خلاصة ملخصة،
    عدوى القتلة وطريق الدخول.
-`verify`
  - وجود مظروف صالح (`--envelope`)، من خلال التحقق الاختياري من الاقتراح، الإعلان أو
    مراسل هيئة الإعلان. تقوم علاقة JSON بحذف قيم الملخص وحالة التحقق
    de assinaturas e quais artefatos opcionaisمراسلة.
-`renewal`- قم بملء المغلف بالموافقة المسبقة على الخلاصة التي تم التصديق عليها مسبقًا. يتطلب
    `--previous-envelope=<path>` والتابع `--envelope=<path>` (الحمولات النافعة Norito).
    يتحقق CLI من عدم تغيير الأسماء المستعارة للملفات والإمكانيات وأزرار الإعلانات،
    أثناء السماح بتحديث الحصة ونقاط النهاية والبيانات الوصفية. قم بإصدار بايتات نظام التشغيل canonicos
    `ProviderAdmissionRenewalV1` (`--renewal-out`) هو ملخص JSON.
-`revoke`
  - أرسل حزمة طوارئ `ProviderAdmissionRevocationV1` لمغلف مغلف خاص بك
    كن متقاعدًا. اطلب `--envelope=<path>`، `--reason=<text>`، أو أقل من `--council-signature`،
    و`--revoked-at`/`--notes` اختياري. يتم تشغيل CLI والتحقق من ملخص المراجعة وإخراج الحمولة
    Norito عبر `--revocation-out` وأطبع ملخص JSON مع الملخص ورقم الاغتيال.
| التحقق | قم بتنفيذ أداة التحقق المشتركة المستخدمة من خلال Torii والبوابات و`sorafs-node`. إثبات الوحدويات الوحدوية + تكامل CLI.[F:crates/sorafs_manifest/src/provider_admission.rs#L1][F:crates/iroha_torii/src/sorafs/admission.rs#L1] | الشبكات TL / التخزين | الخلاصة || انتيجراكاو Torii | قم بتمرير أداة التحقق من استيعاب الإعلانات رقم Torii، وتسجيل الإعلانات للمناسبات السياسية، وإصدار القياس عن بعد. | الشبكات TL | الخلاصة | Torii قبل نقل مغلفات الإدارة (`torii.sorafs.admission_envelopes_dir`)، التحقق من مراسلات الملخص/التثبيت أثناء التحميل وعرض القياس عن بعد admissao.[F:crates/iroha_torii/src/sorafs/admission.rs#L1][F:crates/iroha_torii/src/sorafs/discovery.rs#L1][F:crates/iroha_torii/src/sorafs/api.rs#L1] |
| رينوفاكاو | إضافة أمر التجديد/الإصلاح + مساعدي CLI، نشر دليل حلقة الحياة في مستنداتنا (إصدار دليل التشغيل بعد أوامر CLI em `provider-admission renewal`/`revoke`).[crates/sorafs_car/src/bin/sorafs_manifest_stub/provider_admission.rs#L477][docs/source/sorafs/provider_admission_policy.md:120] | التخزين / الحوكمة | الخلاصة |
| القياس عن بعد | تحديد لوحات المعلومات/التنبيهات `provider_admission` (renovacao ausente, expiracao deenvelve). | إمكانية الملاحظة | م التقدم | يا كونتادور `torii_sorafs_admission_total{result,reason}` موجود؛ لوحات المعلومات/التنبيهات المعلقة.[F:crates/iroha_telemetry/src/metrics.rs#L3798][F:docs/source/telemetry.md#L614] |

### Runbook de renovacao e revogacao#### جدول أعمال Renovacao (atualizacoes de حصص/طوبولوجيا)
1. إنشاء نفس الاقتراح/الإعلان اللاحق مع `provider-admission proposal` و`provider-admission sign`،
   تعزيز `--retention-epoch` وتحديث الحصة/نقاط النهاية المطابقة للضرورة.
2. نفذ
   ```bash
   cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- provider-admission \
     renewal \
     --previous-envelope=governance/providers/<id>/envelope.to \
     --envelope=governance/providers/<id>/envelope_next.to \
     --renewal-out=governance/providers/<id>/renewal.to \
     --json-out=governance/providers/<id>/renewal.json \
     --notes="stake top-up 2025-03"
   ```
   قم بالقيادة الصالحة لمجال السعة/الملف غير المتغير عبر `AdmissionRecord::apply_renewal`،
   قم بإصدار `ProviderAdmissionRenewalV1` وقم بطباعة الملخصات لسجل الإدارة.[crates/sorafs_car/src/bin/sorafs_manifest_stub/provider_admission.rs#L477][F:crates/sorafs_manifest/src/provider_admission.rs#L422]
3. استبدل المظروف الأمامي بـ `torii.sorafs.admission_envelopes_dir`، ثم أكد تجديد Norito/JSON
   لا يوجد مستودع إدارة وإضافة تجزئة للتجديد + فترة الاحتفاظ إلى `docs/source/sorafs/migration_ledger.md`.
4. قم بإعلام المشغلين بأن هذا هو ما تقوم بمراقبته
   `torii_sorafs_admission_total{result="accepted",reason="stored"}` لتأكيد عملية الهضم.
5. قم بإعادة إنشاء وتأكيد تركيبات Canonicos عبر `cargo run -p sorafs_car --bin provider_admission_fixtures --features cli`؛
   CI (`ci/check_sorafs_fixtures.sh`) يؤكد أن Norito موجود دائمًا.#### إعادة النظر في حالات الطوارئ
1. تحديد المغلف المخترق وإصداره مرة أخرى:
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
   O CLI assina o `ProviderAdmissionRevocationV1`، التحقق من مجموعة الاغتيالات عبر
   `verify_revocation_signatures` ويتعلق بملخص التجديد.[crates/sorafs_car/src/bin/sorafs_manifest_stub/provider_admission.rs#L593][F:crates/sorafs_manifest/src/provider_admission.rs#L486]
2. قم بإزالة المظروف `torii.sorafs.admission_envelopes_dir`، وقم بتوزيع Norito/JSON لاستعادة ذاكرات التخزين المؤقت
   قم بالقبول وتسجيل التجزئة للدافع على مهام الإدارة.
3. لاحظ `torii_sorafs_admission_total{result="rejected",reason="admission_missing"}` لتأكيد نظام التشغيل
   تم إلغاء تنزيل ذاكرات التخزين المؤقت أو إعادة الإعلان؛ الحفاظ على أعمال التجديد بأثر رجعي للأحداث.

## الخصيتين والقياس عن بعد- إضافة تركيبات ذهبية للاقتراحات ومغلفات القبول
  `fixtures/sorafs_manifest/provider_admission/`.
- Estender CI (`ci/check_sorafs_fixtures.sh`) لتجديد المقترحات والتحقق من المغلفات.
- تتضمن تركيبات نظام التشغيل `metadata.json` com ملخصات canonicos؛ الخصيتين afirmam المصب
  `proposal_digest_hex` == `ca8e73a1f319ae83d7bd958ccb143f9b790c7e4d9c8dfe1f6ad37fa29facf936`.
- إثبات الخصيتين التكامليتين:
  - Torii تحتوي على إعلانات مع مظاريف قبول مقبولة أو منتهية الصلاحية.
  - O CLI faz ذهابًا وإيابًا في الاقتراح -> المغلف -> التحقق.
  - يتم تجديد إدارة التحكم في نقطة النهاية دون تغيير أو إثبات الهوية.
- متطلبات القياس عن بعد:
  - أرسل رسائل نصية `provider_admission_envelope_{accepted,rejected}` إلى Torii. `torii_sorafs_admission_total{result,reason}` تعرض النتائج المقبولة/المرفوضة.
  - إضافة تنبيهات انتهاء الصلاحية إلى لوحات معلومات المراقبة (يتم تجديدها خلال 7 أيام).

## بروكسيموس باسوس1. كتغييرات في السؤال Norito للمنتدى النهائي ومساعدي التحقق من المنتدى المدمجين في `sorafs_manifest::provider_admission`. ناو ها تتميز بالأعلام.
2. تدفقات CLI (`proposal`, `sign`, `verify`, `renewal`, `revoke`) تم توثيقها وتمرينها عبر الخصيتين التكامليتين؛ الحفاظ على نصوص الإدارة المتزامنة مع دليل التشغيل.
3. Torii يقوم الإدخال/الاكتشاف بإدخال المغلفات وعرض أجهزة القياس عن بعد للاستقبال/الاستقبال.
4. التركيز على الملاحظة: الانتهاء من لوحات المعلومات/تنبيهات القبول لتجديد الأجزاء في مجموعة الأيام التي يتم فيها إلغاء التحذيرات (`torii_sorafs_admission_total`، مقاييس انتهاء الصلاحية).