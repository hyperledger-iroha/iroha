---
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/provider-admission-policy.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

> [`docs/source/sorafs/provider_admission_policy.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/provider_admission_policy.md) سے ماهر۔

# SoraFS قرار الموافقة على القبول و شناخت كاي باليس (SF-2b مسودہ)

هناك ملاحظة **SF-2b** صالحة للعمل مع سمات: SoraFS نموذج قبول قبول العمل، والاختلافات المطلوبة، والحمولات الصافية کی تعريف ونفاذ ۔ يحتوي SoraFS Architecture RFC على سطح عمل كامل ومساحة عمل رائعة وإمكانية استخدام الكمبيوتر.

## باليسى کے اداف

- هذه إحدى الشركات التي تم رصدها باستخدام ريكارد `ProviderAdvertV1` الشائع قبوله كأداة حربية.
- تم الإعلان عن كل ما هو جديد في القائمة، وتوثيق نقاط النهاية، وإنهاء شراكتنا.
- Torii، والبوابات، و`sorafs-node` هو واحد من أفضل أدوات التشفير التي تساعد في إنشاء قاعدة بيانات إلكترونية.
- العديد من وسائل الراحة أو بيئة العمل التي ستساعدك على الاستمتاع بالمزيد من الراحة والمتعة.

## شناخت اور حصة تطالبے| تقاضي | وضاحت | غيليوريبل |
|-------|-------|-----------|
| إعلان كليد كأخذ | قم بالإعلان عن زوج المفاتيح Ed25519 باستخدام مسجل المفاتيح. حزمة القبول جورنس دستخط کے ساتھ پبلک مفتاح محفوظ کرتا ہے. | `ProviderAdmissionProposalV1` يحتوي على `advert_key` (32 بايت) ويشتمل على تسجيل ومسجل (`sorafs_manifest::provider_admission`) وهو عبارة عن ريفرنس. |
| حصة پوينٹر | تم قبول الموافقة على تجمع التوقيع المساحي النشط من خلال الإشارة إلى الصفر وغير الصفر `StakePointer`. | يتضمن `sorafs_manifest::provider_advert::StakePointer::validate()` أخطاء في اختبارات CLI وCLI/الاختبارات. |
| الولاية القضائية ٹیگز | قانون الولاية القضائية + الارتباط القانوني للمحكمة. | يتضمن مشروع المشروع `jurisdiction_code` (ISO 3166-1 alpha-2) واختراع `contact_uri`. |
| نقطة النهاية توثیق | إن الإعلان عن نقطة نهاية لـ mTLS أو تقرير سريع للرياضة أمر ضروري. | Norito الحمولة `EndpointAttestationV1` هي تسعيرة البطاقة وحزمة القبول الخاصة بها هي نقطة النهاية للاندرويد. |

## قبولیت کا وک فلو1. **منتجات البروبوزل**
   - سطر الأوامر: `cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- provider-admission proposal ...`
     تتضمن الحزمة `ProviderAdmissionProposalV1` + الحزمة الصحية.
   - ويليمي: القطع الأساسي، الحصة > 0، و`profile_id` هو مقبض القطع الكنسي الذي يستخدم.
2. **الإمساك بالطعام**
   - استخدام أدوات المغلف الموجودة (ماڈیول `sorafs_manifest::governance`)
     `blake3("sorafs-provider-admission-v1" || canonical_bytes)` خط الاتصال المباشر.
   - Envelope کو `governance/providers/<provider_id>/admission.json` محفوظ کيا جاتا ۔
3. **رجستري ميريديان**
   - أداة التحقق هذه (`sorafs_manifest::provider_admission::validate_envelope`) تستخدم مرة أخرى Torii/gateways/CLI.
   - Torii هو تسجيل القبول في بطاقة الائتمان والإعلانات التي يتم سردها في الملخص أو مظروف انتهاء الصلاحية بشكل مختلف.
4. **تجديد ومنسوخي**
   - تتضمن نقطة النهاية/الحصص المختارة رقم `ProviderAdmissionRenewalV1` القراءة.
   - تم إنشاء `--revoke` من خلال CLI وهو عبارة عن مجموعة من الألعاب والألعاب.

## عمل درآمد کے کام

| علاقة | كام | المالك (المالكون) | حالة |
|-------|-----|----------|------|
| اسكيمہ | `crates/sorafs_manifest/src/provider_admission.rs` تحت `ProviderAdmissionProposalV1`، `ProviderAdmissionEnvelopeV1`، `EndpointAttestationV1` (Norito) هي بأسعار معقولة. `sorafs_manifest::provider_admission` مساعدون ويليونيون ينفذون هذه المهمة.[F:crates/sorafs_manifest/src/provider_admission.rs#L1] | التخزين / الحوكمة | ✅ مكمل |
| CLI ٹولنگ | `sorafs_manifest_stub` کومانڈز کامنز توسیع دیں: `provider-admission proposal`, `provider-admission sign`, `provider-admission verify`. | الأدوات مجموعة العمل | ✅ مكمل |CLI فلو اب درمیانی سرٹیفکیٹ حزم (`--endpoint-attestation-intermediate`) قبول کرتا ہے،
بايتات الاقتراح/المغلف المتعارف عليها جارى كرتا ہے، و `sign`/`verify` کے دوران توقيعات الكونسل کو یری فائی كرتا ہے. تستخدم الهيئات الإعلانية لتوضيح الإعلانات الموقعة أو الإعلانات الموقعة مرة أخرى، وملفات التوقيع التي `--council-signature-public-key` والتي تنتهي بـ `--council-signature-file`. هناك أتمتة سهلة.

### حوالة CLI

لقد تم التحكم به من خلال `cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- provider-admission ...`.-`proposal`
  - دركار فليغز: `--provider-id=<hex32>`، `--chunker-profile=<namespace.name@semver>`،
    `--stake-pool-id=<hex32>`، `--stake-amount=<amount>`، `--advert-key=<hex32>`،
    `--jurisdiction-code=<ISO3166-1>`، وأكمل من `--endpoint=<kind:host>`.
  - نقطة النهاية لـ `--endpoint-attestation-attested-at=<secs>`,
    `--endpoint-attestation-expires-at=<secs>`، ذرية متكاملة
    `--endpoint-attestation-leaf=<path>` (هذا هو العنصر الذي اخترناه `--endpoint-attestation-intermediate=<path>`) و
    تشتمل هذه المذكرة أيضًا على معرفات ALPN (`--endpoint-attestation-alpn=<token>`). نقاط النهاية QUIC نقل وحمل التقارير
    `--endpoint-attestation-report[-hex]=...` تم الانتهاء منه.
  - آؤٹ پٹ: بايتات اقتراح Norito الأساسية (`--proposal-out`) وخلاصة JSON
    (ڈیفالٹ stdout أو `--json-out`).
-`sign`
  - ان پٹس: ایك پروپوزل (`--proposal`)، ایك إعلان موقع (`--advert`)، اختیاری نص الإعلان
    (`--advert-body`)، فترة الاحتفاظ، وآخر توقيع للمستشار. التوقيعات کو مضمنة
    (`--council-signature=<signer_hex:signature_hex>`) أو الفقاعات التي تم قطعها بسرعة
    `--council-signature-public-key` إلى `--council-signature-file=<path>` تم إرساله بواسطة ملاييا.
  - مظروف تم التحقق منه (`--envelope-out`) ومجلد JSON يحتوي على ملخص للارتباطات وعدد الموقعين ومسارات الإدخال.
-`verify`
  - مظروف موجود (`--envelope`) يتم فحصه، ويتم تصنيعه لمطابقة الاقتراح أو الإعلان أو نص الإعلان. يحتوي تقرير JSON على القيم الملخصة وحالة التحقق من التوقيع والعناصر الاختيارية والمطابقة الصحيحة.
-`renewal`- يتم عرض مظروف يتم قراءته من خلال هضمه. إنه لأمر مؤسف
    `--previous-envelope=<path>` و`--envelope=<path>` (دونوں الحمولات Norito) درکار.
    يقوم CLI بتصفح الكلمات والأسماء المستعارة للملف الشخصي والإمكانيات ومفاتيح الإعلان التي تستبدل الحصة الواحدة ونقاط النهاية والبيانات الوصفية. `ProviderAdmissionRenewalV1` بايت المتعارف عليه (`--renewal-out`) وJSON ہ آؤٹ پٹ أوتا ہے.
-`revoke`
  - مزود الخدمة `ProviderAdmissionRevocationV1` حزمة جارى كارتا ومغلف وابس لنا ضروري.
    `--envelope=<path>`، `--reason=<text>`، أكمل من `--council-signature`، و
    `--revoked-at`/`--notes` مختلف. ملخص إلغاء سطر الأوامر (CLI) التوقيع/التحقق من البطاقة، حمولة Norito
    `--revocation-out` تم تسجيله، وعدد الملخصات والتوقيعات التي تم تسجيلها في تقرير JSON.
| ویریفیکیشن | Torii، والبوابات، و`sorafs-node` هي أداة تحقق فعالة. وحدة + اختبارات التكامل CLI. الشبكات TL / التخزين | ✅ مكمل || Torii الأغاني | يتيح المدقق Torii استيعاب الإعلانات بشكل شامل وشامل للإعلانات والقياس عن بعد. | الشبكات TL | ✅ مكمل | Torii مغلفات الإدارة (`torii.sorafs.admission_envelopes_dir`) تسجيل الدخول، استيعاب الملخص/تطابق التوقيع، تسجيل الدخول، وقياس القبول عن بعد ہے.[F:crates/iroha_torii/src/sorafs/admission.rs#L1][F:crates/iroha_torii/src/sorafs/discovery.rs#L1][F:crates/iroha_torii/src/sorafs/api.rs#L1] |
| تجدید | تجد/تجد/منسوخة الاسكيم + مساعدات CLI تتضمن القراءة ودليل دورة الحياة للمستندات الشائعة (جديد runbook و`provider-admission renewal`/`revoke` CLI) دیکھیں).[crates/sorafs_car/src/bin/sorafs_manifest_stub/provider_admission.rs#L477] 【docs/source/sorafs/provider_admission_policy.md:120】 | التخزين / الحوكمة | ✅ مكمل |
| ٹيليميٹری | `provider_admission` لوحات المعلومات والتنبيهات ذات الأسعار المعقولة (تجديدها، انتهاء صلاحية المغلف). | إمكانية الملاحظة | 🟠 جاری | كاؤنٹر `torii_sorafs_admission_total{result,reason}` موجود؛ لوحات المعلومات/التنبيهات زر التوا ہیں.【F:crates/iroha_telemetry/src/metrics.rs#L3798】【F:docs/source/telemetry.md#L614】 |

### تجدید ومنسوخی کا رن بک#### شيول شيلد (الحصة/الطوبولوجيا)
1. `provider-admission proposal` و`provider-admission sign` هو أول اقتراح/إعلان،
   تتميز `--retention-epoch` بالقدرة والضرورة التي تتوافق مع حصة/نقاط النهاية.
2. البهجة
   ```bash
   cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- provider-admission \
     renewal \
     --previous-envelope=governance/providers/<id>/envelope.to \
     --envelope=governance/providers/<id>/envelope_next.to \
     --renewal-out=governance/providers/<id>/renewal.to \
     --json-out=governance/providers/<id>/renewal.json \
     --notes="stake top-up 2025-03"
   ```
   تم تشغيل `AdmissionRecord::apply_renewal` من خلال القدرة/الملف الشخصي الذي تم تغييره مرة أخرى،
   `ProviderAdmissionRenewalV1` بطاقة يومية، ومواصلة هضم بطاقة الطفل ہے۔【crates/sorafs_car/src/bin/sorafs_manifest_stub/provider_admission.rs#L477】【F:crates/sorafs_manifest/src/provider_admission.rs#L422】
3. `torii.sorafs.admission_envelopes_dir` يحتوي على مغلف بديل للبطاقة، ويتم تجديد Norito/JSON مع الالتزام بالنشر،
   وتجزئة التجديد + عصر الاحتفاظ الذي يشمل `docs/source/sorafs/migration_ledger.md`.
4. قم بتخزين مغلف جديد وتنشيطه واستيعاب النقرات
   طريقة `torii_sorafs_admission_total{result="accepted",reason="stored"}`.
5. `cargo run -p sorafs_car --bin provider_admission_fixtures --features cli` يتم تحديث التركيبات الأساسية والالتزام بها؛
   CI (`ci/check_sorafs_fixtures.sh`) Norito هناك المزيد من الاستقرار.#### ہنگامی منسوخی
1. مظروف متعدد الاستخدامات للبطاقات الائتمانية والإلكترونية:
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
   CLI `ProviderAdmissionRevocationV1` خط كتابة البطاقة، `verify_revocation_signatures` توقيع التوقيعات السريعة،
   وملخص الإلغاء رپورٹ کرتا ہے.【crates/sorafs_car/src/bin/sorafs_manifest_stub/provider_admission.rs#L593】【F:crates/sorafs_manifest/src/provider_admission.rs#L486】
2. `torii.sorafs.admission_envelopes_dir` مغلف، إلغاء Norito/JSON وذاكرة التخزين المؤقت للقبول مقيد،
   وهناك أيضًا تجزئة للمحتوى.
3.`torii_sorafs_admission_total{result="rejected",reason="admission_missing"}` لم يتم إبطال مسح ذاكرة التخزين المؤقت للإعلان وإسقاطه؛
   تعتبر المصنوعات اليدوية للإلغاء بأثر رجعي للحوادث محفوظًا.

## ٹيسٹنگ و ٹيليمتری- مقترحات القبول والمظاريف للتركيبات الذهبية `fixtures/sorafs_manifest/provider_admission/` تحت شامل کریں.
- CI (`ci/check_sorafs_fixtures.sh`) يمكنك من خلاله إعادة تقديم المقترحات والمغلفات.
- تحتوي التركيبات السابقة على الملخصات الأساسية التي تتضمن `metadata.json` وتشتمل على ما يلي؛ تؤكد الاختبارات النهائية على الأمر
  `proposal_digest_hex` == `ca8e73a1f319ae83d7bd958ccb143f9b790c7e4d9c8dfe1f6ad37fa29facf936`.
- اختبارات التكامل
  - Torii إعلان مسترد كرتا ہے جن ے مظاريف القبول غائب أو میعاد ختم ہو چکے ہوں.
  - اقتراح CLI → المغلف → التحقق کا ذهابا وإيابا چلاتا ہے۔
  - يمكنك العثور على معرف الموفر الذي يمكنك من خلاله تغيير نقطة النهاية لتدوير البطاقة.
- ٹيليمتري الأساسيات:
  - Torii `provider_admission_envelope_{accepted,rejected}` تنبعث الكريات. ✅ `torii_sorafs_admission_total{result,reason}` اب النتائج المقبولة/المرفوضة دکھا ہے۔
  - تحتوي لوحات المعلومات الخاصة بقابلية المراقبة على تحذيرات انتهاء الصلاحية تتضمن نقرًا سريعًا (7 أيام في الاندرويد تجدها في وقت لاحق).

##اگلے اجراءات1. ✅ Norito يحتوي على مساعدين للتحقق من الصحة يشتملان على كل ما تحتاجه. هذه الأعلام المميزة ليست ضرورية.
2. ✅ CLI فلو (`proposal`, `sign`, `verify`, `renewal`, `revoke`) دستاويزی ہیں واختبارات التكامل سے گزارے گئے ہیں؛ لقد أصبح استخدام النصوص البرمجية في دليل التشغيل أمرًا سهلاً للغاية.
3. ✅ Torii مظاريف القبول/الاكتشاف استيعاب کرتا ہے اور قبولیت/رد کے عدادات القياس عن بعد دکھاتا ہے۔
4. إمكانية الملاحظة في التوجيه: لوحات معلومات القبول/التنبيهات مكتملة في وقت متأخر من الليل لتجديد عقود التنبيه (`torii_sorafs_admission_total`، مقاييس انتهاء الصلاحية).