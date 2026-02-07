---
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/provider-admission-policy.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

> التكيف مع [`docs/source/sorafs/provider_admission_policy.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/provider_admission_policy.md).

# سياسة القبول وهوية مقدمي الخدمة SoraFS (Brouillon SF-2b)

هذه الملاحظة تلتقط الكائنات الحية من أجل **SF-2b**: تحديدها
تطبيق سير عمل القبول ومتطلبات الهوية والحمولات
شهادة لموفري المخزون SoraFS. لقد أكملت العملية
تم توضيح المستوى العالي في RFC للهندسة المعمارية SoraFS وفك العمل
Restest en tâches d'ingénierie traçables.

## أهداف السياسة

- ضمان أن المشغلين الذين تم التحقق منهم يمكنهم نشر التسجيلات
  `ProviderAdvertV1` مقبول على الشبكة.
- كل كلمة إعلان عن وثيقة هوية معتمدة من قبل الحكومة،
  نقاط النهاية المعتمدة ومساهمة الحد الأدنى من الحصة.
- توفير عملية التحقق التي تحدد Torii، البوابات
  ويتم إضافة `sorafs-node` إلى عناصر التحكم المماثلة.
- Supporter le renouvellement et la révocation d'urgence sans casser le
  تحديد وبيئة الأدوات.

## متطلبات الهوية والحصة| الضرورة | الوصف | قابل للعيش |
|----------|-------------|---------|
| مصدر الكلمة المفتاحية | يجب على مقدمي الخدمة تسجيل زوج من المفاتيح Ed25519 التي تشير إلى كل إعلان. تحتوي حزمة القبول على المفتاح العام مع توقيع الإدارة. | قم بتوسيع المخطط `ProviderAdmissionProposalV1` مع `advert_key` (32 بايت) والمرجع بعد التسجيل (`sorafs_manifest::provider_admission`). |
| نقطة الحصة | يتطلب القبول `StakePointer` غير محدد مقابل مجموعة تخزين نشطة. | قم بإضافة التحقق من الصحة في `sorafs_manifest::provider_advert::StakePointer::validate()` وقم بإصلاح الأخطاء في CLI/الاختبارات. |
| العلامات دي القضاء | يعلن مقدمو الخدمة عن الاختصاص القضائي + الاتصال القانوني. | قم بتوسيع مخطط الاقتراح باستخدام `jurisdiction_code` (ISO 3166-1 alpha-2) والخيار `contact_uri`. |
| شهادة نقطة النهاية | يجب أن يتم دعم كل نقطة نهاية مُعلنة من خلال تقرير شهادة mTLS أو QUIC. | قم بتعريف الحمولة Norito `EndpointAttestationV1` والمخزن عند نقطة النهاية في حزمة الإدخال. |

## سير العمل للقبول1. **إنشاء الاقتراح**
   - سطر الأوامر: ajouter `cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- provider-admission proposal ...`
     منتج `ProviderAdmissionProposalV1` + حزمة التصديق.
   - التحقق من الصحة: ​​ضمان الأبطال المطلوبة، الحصة > 0، التعامل مع مقسم الكنسي في `profile_id`.
2. **إقرار الحكم**
   - علامة المجلس `blake3("sorafs-provider-admission-v1" || canonical_bytes)` عبر المخرج
     المغلف موجود (الوحدة `sorafs_manifest::governance`).
   - يظل المغلف ثابتًا في `governance/providers/<provider_id>/admission.json`.
3. ** إدخال التسجيل **
   - تنفيذ مشاركة محققة (`sorafs_manifest::provider_admission::validate_envelope`)
     تم إعادة استخدامه وفقًا لـ Torii/gateways/CLI.
   - الاستمرار في استخدام نظام القبول Torii لرفض الإعلانات دون هضمها أو انتهاء صلاحيتها
     مختلف عن المغلف.
4. ** التجديد والإلغاء **
   - أضف `ProviderAdmissionRenewalV1` مع تحديث خيارات نقطة النهاية/الحصّة.
   - كشف عن برنامج CLI `--revoke` الذي يسجل سبب الإلغاء ويتمكن من إجراء حدث للإدارة.

## تقنيات التنفيذ| دومين | تاش | المالك (المالكون) | النظام الأساسي |
|--------|------|----------|--------|
| المخطط | حدد `ProviderAdmissionProposalV1`، `ProviderAdmissionEnvelopeV1`، `EndpointAttestationV1` (Norito) إلى `crates/sorafs_manifest/src/provider_admission.rs`. تم التنفيذ في `sorafs_manifest::provider_admission` مع مساعدي التحقق من الصحة. 【F:crates/sorafs_manifest/src/provider_admission.rs#L1】 | التخزين / الحوكمة | ✅ تيرميني |
| Outillage CLI | أدخل `sorafs_manifest_stub` مع الأوامر التالية: `provider-admission proposal`، `provider-admission sign`، `provider-admission verify`. | الأدوات مجموعة العمل | ✅ |

يقبل تدفق CLI تفكيك حزم الشهادات الوسيطة (`--endpoint-attestation-intermediate`)، ويؤدي إلى اقتراح/مغلف البايتات القياسية ويصادق على توقيعات النصيحة المعلقة على `sign`/`verify`. يمكن للمشغلين توفير مجموعة إعلانية مباشرة، أو إعادة استخدام الإعلانات الموقعة، ويمكن توفير ملفات التوقيع بالدمج بين `--council-signature-public-key` و`--council-signature-file` لتسهيل الأتمتة.

### مرجع CLI

قم بتنفيذ كل أمر عبر `cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- provider-admission ...`.-`proposal`
  - الأعلام تتطلب: `--provider-id=<hex32>`، `--chunker-profile=<namespace.name@semver>`،
    `--stake-pool-id=<hex32>`، `--stake-amount=<amount>`، `--advert-key=<hex32>`،
    `--jurisdiction-code=<ISO3166-1>`، وعلى الأقل `--endpoint=<kind:host>`.
  - شهادة نقطة النهاية الاسمية حضور `--endpoint-attestation-attested-at=<secs>`،
    `--endpoint-attestation-expires-at=<secs>`، شهادة عبر
    `--endpoint-attestation-leaf=<path>` (زائد `--endpoint-attestation-intermediate=<path>`
    optionnel pour chaque élement de chaîne) et tout ID ALPN négocié
    (`--endpoint-attestation-alpn=<token>`). يمكن لنقاط النهاية QUIC أن توفر اتصالات النقل عبرها
    `--endpoint-attestation-report[-hex]=...`.
  - الفرز: البايتات الأساسية للعرض Norito (`--proposal-out`) والسيرة الذاتية JSON
    (قياسي افتراضيًا أو `--json-out`).
-`sign`
  - الإدخالات: اقتراح (`--proposal`)، توقيع إعلان (`--advert`)، مجموعة إعلانية اختيارية
    (`--advert-body`)، فترة الاحتفاظ وأقل من توقيع المجلس. يمكن أن تكون التوقيعات موجودة
    Fournies مضمنة (`--council-signature=<signer_hex:signature_hex>`) أو عبر الملفات المجمعة
    `--council-signature-public-key` مع `--council-signature-file=<path>`.
  - قم بإنتاج مظروف صالح (`--envelope-out`) وتقرير JSON يشير إلى روابط الهضم،
    عدد التوقيعات وطرق الدخول.
-`verify`
  - صالح وجود مظروف (`--envelope`)، مع خيار التحقق من الاقتراح،
    الإعلان أو هيئة الإعلان المراسل. اجتمعت علاقة JSON مع قيم الهضم،حالة التحقق من التوقيع والعناصر الاختيارية المقابلة.
-`renewal`
  - ضع مظروفًا جديدًا تمت الموافقة عليه في ملخص مسبق تم التصديق عليه. مطلوب
    `--previous-envelope=<path>` والنجاح `--envelope=<path>` (حمولات مزدوجة Norito).
    يتحقق Le CLI من أن الأسماء المستعارة للملف الشخصي والقدرات وأزرار الإعلان قد تغيرت،
    قم بتمكين جميع بيانات الحصة اليومية ونقاط النهاية والبيانات الوصفية. Émet les bytes canoniques
    `ProviderAdmissionRenewalV1` (`--renewal-out`) هي أيضًا سيرة ذاتية JSON.
-`revoke`
  - قم بإنشاء حزمة عاجلة `ProviderAdmissionRevocationV1` لموفر لا يفعل ذلك
    كن متقاعدًا. يتطلب `--envelope=<path>`، `--reason=<text>`، أو أكثر
    `--council-signature`، والخيار `--revoked-at`/`--notes`. علامة CLI وصالحة
    خلاصة الإلغاء، وكتابة الحمولة Norito عبر `--revocation-out`، وإصدار تقرير JSON
    مع الهضم واسم التوقيعات.
| التحقق | قم بتنفيذ جزء من أداة التحقق المستخدمة بواسطة Torii والبوابات و`sorafs-node`. Fournir des الاختبارات الموحدة + d'intégration CLI.[F:crates/sorafs_manifest/src/provider_admission.rs#L1] 【F:crates/iroha_torii/src/sorafs/admission.rs#L1】 | الشبكات TL / التخزين | ✅ تيرميني || التكامل Torii | أدخل المدقق في عرض الإعلانات Torii، ورفض الإعلانات خارج السياسة، وقم ببثها عن بعد. | الشبكات TL | ✅ تيرميني | Torii قم بتفكيك مظاريف الإدارة (`torii.sorafs.admission_envelopes_dir`)، والتحقق من خلاصة/توقيع المراسلات عند الإدخال وكشف البيانات عن بعد القبول.[F:crates/iroha_torii/src/sorafs/admission.rs#L1][F:crates/iroha_torii/src/sorafs/discovery.rs#L1][F:crates/iroha_torii/src/sorafs/api.rs#L1] |
| تجديد | إضافة مخطط التجديد/الإلغاء + مساعدات CLI، ونشر دليل دورة الحياة في المستندات (يظهر دليل التشغيل ci-dessous ويأمر CLI `provider-admission renewal`/`revoke`).[crates/sorafs_car/src/bin/sorafs_manifest_stub/provider_admission.rs#L477][docs/source/sorafs/provider_admission_policy.md:120] | التخزين / الحوكمة | ✅ تيرميني |
| القياس عن بعد | تحديد لوحات المعلومات/التنبيهات `provider_admission` (مشكلة التجديد، انتهاء صلاحية المظروف). | إمكانية الملاحظة | 🟠 أون كورس | يوجد كمبيوتر `torii_sorafs_admission_total{result,reason}` ؛ لوحات المعلومات/التنبيهات واليقظة. 【F:crates/iroha_telemetry/src/metrics.rs#L3798】【F:docs/source/telemetry.md#L614】 |

### Runbook de renovellement et révocation#### برنامج التجديد (يتابع يوم الحصة/الطوبولوجيا)
1. قم بإنشاء الاقتراح المزدوج/الإعلان الناجح مع `provider-admission proposal` و`provider-admission sign`، بالإضافة إلى `--retention-epoch` ومع مراعاة نقاط النهاية/نقاط النهاية إذا طلبت ذلك.
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
   يتم تغيير الأوامر الصالحة للسعة/الملف الشخصي عبر
   `AdmissionRecord::apply_renewal`، أدخل `ProviderAdmissionRenewalV1`، وقم بتشغيل الملخصات من أجل
   le Journal de gouvernance. 【crates/sorafs_car/src/bin/sorafs_manifest_stub/provider_admission.rs#L477】【F:crates/sorafs_manifest/src/provider_admission.rs#L422】
3. استبدل المغلف السابق في `torii.sorafs.admission_envelopes_dir`، وقم بتجديد Norito/JSON في إيداع الإدارة، وأضف تجزئة التجديد + فترة الاحتفاظ إلى `docs/source/sorafs/migration_ledger.md`.
4. أبلغ المشغلين بأن المغلف الجديد نشط وقم بمراقبة `torii_sorafs_admission_total{result="accepted",reason="stored"}` لتأكيد الإدخال.
5. قم بإعادة التركيب وتثبيت التركيبات الأساسية عبر `cargo run -p sorafs_car --bin provider_admission_fixtures --features cli` ; CI (`ci/check_sorafs_fixtures.sh`) يتأكد من أن الطلعات Norito موجودة في الاسطبلات.#### إلغاء حالة الطوارئ
1. حدد المظروف الذي تعرض للاختراق وقم بإبطاله :
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
   علامة CLI على `ProviderAdmissionRevocationV1`، تحقق من مجموعة التوقيعات عبر
   `verify_revocation_signatures`، وتقرير ملخص الإلغاء. 【crates/sorafs_car/src/bin/sorafs_manifest_stub/provider_admission.rs#L593】【F:crates/sorafs_manifest/src/provider_admission.rs#L486】
2. قم بملء مظروف `torii.sorafs.admission_envelopes_dir`، وقم بتوزيع Norito/JSON لإلغاء ذاكرات الدخول المؤقتة، وقم بتسجيل تجزئة السبب في محضر الإدارة.
3. قم بمراقبة `torii_sorafs_admission_total{result="rejected",reason="admission_missing"}` للتأكد من أن ذاكرات التخزين المؤقت قد تخلت عن الإعلان المعاد؛ الحفاظ على آثار الإلغاء في أحداث استرجاعية.

## الاختبارات والقياس عن بعد- إضافة التركيبات الذهبية للمقترحات ومغلفات القبول
  `fixtures/sorafs_manifest/provider_admission/`.
- قم بتوسيع CI (`ci/check_sorafs_fixtures.sh`) لإنشاء المقترحات والتحقق من المغلفات.
- تتضمن التركيبات العامة `metadata.json` مع الملخصات الأساسية؛ ليه الاختبارات المصب صالحة
  `proposal_digest_hex` == `ca8e73a1f319ae83d7bd958ccb143f9b790c7e4d9c8dfe1f6ad37fa29facf936`.
- اختبارات التكامل :
  - Torii قم بإعادة نشر الإعلانات باستخدام مظاريف القبول القديمة أو المنتهية الصلاحية.
  - Le CLI fait un aller-retour المقترحة → المغلف → التحقق.
  - يتم إجراء تجديد الإدارة على شهادة نقطة النهاية دون تغيير معرف المزود.
- متطلبات القياس عن بعد :
  - Émettre les compteurs `provider_admission_envelope_{accepted,rejected}` dans Torii. ✅ `torii_sorafs_admission_total{result,reason}` يعرض اختلال النتائج المقبولة/المرفوضة.
  - إضافة تنبيهات انتهاء الصلاحية في لوحات المعلومات الخاصة بقابلية المراقبة (تجديدها خلال 7 أيام).

## Prochaines étapes1. ✅ الانتهاء من تعديلات المخطط Norito ودمج مساعدات التحقق من الصحة في
   `sorafs_manifest::provider_admission`. يتطلب علامة ميزة Aucun.
2. ✅ مسارات العمل CLI (`proposal`, `sign`, `verify`, `renewal`, `revoke`) هي وثائق وتمارين عبر اختبارات التكامل؛ قم بحماية نصوص الحوكمة المتزامنة مع دليل التشغيل.
3. ✅ الدخول/الاكتشاف Torii يقوم بإدخال المغلفات ويكشف عن حاسبات القبول/الرفض عن بعد.
4. إمكانية ملاحظة التركيز: قم بإنهاء لوحات المعلومات/تنبيهات الدخول لتخفيض التحذيرات خلال سبعة أيام متتالية (`torii_sorafs_admission_total`، مقاييس انتهاء الصلاحية).