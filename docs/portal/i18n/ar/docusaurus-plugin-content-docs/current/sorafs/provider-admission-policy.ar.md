---
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/provider-admission-policy.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

> مقتبس من [`docs/source/sorafs/provider_admission_policy.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/provider_admission_policy.md).

# لاتخاذ وهي مقبولة SoraFS (مسودة SF-2b)

ت هذه المذكرة المخرجات التجريبية لـ **SF-2b**: تعريف مسار تقبل، ومتطلبات الهوية، وحمولات
الاستيثاق لمزودي التخزين في SoraFS وتطبيقها. وهي المتطورة عالية المستوى الموضح في RFC
معمارية SoraFS وتجزئ العمل إلى مهام هندسية قابلة للتتبع.

## أهداف السياسة

- ضمان أن يقوموا بتشغيل المُدقّقين فقط ليتمكنوا من نشر السجلات `ProviderAdvertV1` التي تقبلها الشبكة.
- كل مفتاح ربط إعلان بووثيقة هوية معتمدة من الـ تور، ونقاط نهاية معتمدة، ومساهمة بحد أدنى من الـ حصة.
- توفير تحقق حتمية لكي يطبق Torii والبوابات و`‎sorafs-node` نفس الشيء.
- دعم و إلغاء إلغاء كسر الحتمية أو بيئة العمل.

##متطلبات الهوية والـحصة| المتطلب | الوصف | المخرج |
|---------|-------|--------|
| مصدر مفتاح الإعلان | يجب أن يسجل المزودون زوج مفاتيح Ed25519 يوقع كل إعلان. تقوم بتخزين المفتاح العام مع توقيع الوت. | النهائي المخطط `ProviderAdmissionProposalV1` بـ `advert_key` (32 بايت) والإشارة إلى السجل (`sorafs_manifest::provider_admission`). |
| حصة المؤشر | يلزم قبول `StakePointer` غير صفري يشير إلى مجمع التوقيع الرقمي. | إضافة التحقق في `sorafs_manifest::provider_advert::StakePointer::validate()` ووجد سبب في CLI/الاختبارات. |
| وسوم عدم التنظيم | أعلن عن عدم وجود + جهة اتصال. | المخطط المقترح بـ `jurisdiction_code` (ISO 3166-1 alpha-2) و`contact_uri` اختياري. |
| استيثاق نقطة النهاية | يجب أن تكون كل نقطة نهاية مُعلنة مدعومة بتقرير شهادة mTLS أو QUIC. | تعريف حمولة Norito `EndpointAttestationV1` وتخزينها لكل نقطة نهاية داخل حزمة قبول. |

## سير عمل تقبل1. ** إنشاء المقترح **
   - سطر الأوامر: إضافة `cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- provider-admission proposal ...`
     إنتاج `ProviderAdmissionProposalV1` + حزمة الاستيثاق.
   - التحقق: ضمان خصوصية، وstake > 0، ومقبض Chunker قياسي في `profile_id`.
2. ** اعتماد الاتّحاد **
   - يوقع المجلس `blake3("sorafs-provider-admission-v1" || canonical_bytes)` باستخدام أدوات المغلف الحالية
     (الوحدة `sorafs_manifest::governance`).
   - يتم حفظ الـ المغلف في `governance/providers/<provider_id>/admission.json`.
3. **إدخال السجل**
   - تنفيذ متدرب مشترك (`sorafs_manifest::provider_admission::validate_envelope`) إعادة استخدام Torii/البوابات/CLI.
   - تحديث مسار القبول في Torii لرفض الإعلانات التي تختلف عن الملخص أو تاريخ الانتهاء فيها عن الـ المغلف.
4. **التجديد والإلغاء**
   - إضافة `ProviderAdmissionRenewalV1` مع تحديثات اختيارية للنقاط النهائية/الـستيك.
   - إتاحة مسار CLI `--revoke` تسجيل تسجيل الإلغاء ويدفع سجل التاريخ.

##واجبات التنفيذ

| | | المالك (المالكون) | الحالة |
|--------|-------|----------|--------|
| Be | تعريف `ProviderAdmissionProposalV1` و`ProviderAdmissionEnvelopeV1` و`EndpointAttestationV1` (Norito) ضمن `crates/sorafs_manifest/src/provider_admission.rs`. مُنفذ داخل `sorafs_manifest::provider_admission` مع مساعدات تحقق.【F:crates/sorafs_manifest/src/provider_admission.rs#L1】 | التخزين / الحوكمة | ✅ مكتملة |
| أدوات سطر الأوامر | الأطراف `sorafs_manifest_stub` بأوامر فرعية: `provider-admission proposal`, `provider-admission sign`, `provider-admission verify`. | الأدوات مجموعة العمل | ✅ |مسار CLI يدعم الآن شهادات الوثائق (`--endpoint-attestation-intermediate`) ويدعم البايتات المتقدم للترشيح/الصدر المغلف ويتحقق من مجلس تواقيع خلال `sign`/`verify`. يمكن للمشغلين توفير أجسام الإعلانات مباشرة أو إعادة استخدام الإعلانات موقعة، ويمكن أن تنجح نتائج التواقيع عبر الجمع بين `--council-signature-public-key` و`--council-signature-file` لتسهيل الإيداع.

### مرجع CLI

نفّذ كل أمر عبر `cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- provider-admission ...`.-`proposal`
  - الأعلام المطلوب: `--provider-id=<hex32>`, `--chunker-profile=<namespace.name@semver>`,
    `--stake-pool-id=<hex32>`، `--stake-amount=<amount>`، `--advert-key=<hex32>`،
    `--jurisdiction-code=<ISO3166-1>`، وعلى الأقل `--endpoint=<kind:host>` واحد.
  - استيثاق كل نقطة نهاية يتطلب `--endpoint-attestation-attested-at=<secs>`,
    `--endpoint-attestation-expires-at=<secs>`، وشهادة عبر
    `--endpoint-attestation-leaf=<path>` (مع `--endpoint-attestation-intermediate=<path>` اختياري لكل عنصر في جبل) وأي معرفات ALPN متفاوض عليها
    (`--endpoint-attestation-alpn=<token>`). يمكن لنقاط نهاية QUIC تقديم تقارير النقل عبر
    `--endpoint-attestation-report[-hex]=...`.
  - المخرجات: بايتات طويلة لم يقترح Norito (`--proposal-out`) وملخص JSON
    (stdout افتراضي أو `--json-out`).
-`sign`
  - المدخلات: الآلة (`--proposal`)، موقع الإعلان (`--advert`)، جسم الإعلان اختياري
    (`--advert-body`)، حقبة الاحتفاظ، وتوقيع مجلس واحد على الأقل. يمكن أن تتطور
    مضمنة (`--council-signature=<signer_hex:signature_hex>`) أو عبر الملفات باستخدام
    `--council-signature-public-key` مع `--council-signature-file=<path>`.
  - ناتج المغلف مُحقق (`--envelope-out`) وتقرير JSON يوضح روابط الملخص عدد الموقّعين ومسارات الإدخال.
-`verify`
  - يتحقق من المغلف موجود (`--envelope`) مع فحص اختياري للتوافق مع الإعلان أو الإعلان أو إعلان الجسم. يسلط تقرير JSON الضوء على قيم الملخص وحالة تحقق التواقيع وأي مصنوعات يدوية اختيارية المقال.
-`renewal`
  - ربط المظروف مُعتمد جديد بالـ الملخص الذي تم الصديق عليه سابقاً. يجب
    `--previous-envelope=<path>` و`--envelope=<path>` التالي (كلاهما حمولة Norito).يحقق CLI من بقاء الأسماء المستعارة للملف الشخصي والقدرات ومفاتيح إعلان دون تغيير، مع الاشتراكات في الحصة ونقاط النهاية والـ البيانات الوصفية. بايت يصل إلى
    `ProviderAdmissionRenewalV1` (`--renewal-out`) تمت إضافته إلى ملخص JSON.
-`revoke`
  - يصدر حزمة طوارئ `ProviderAdmissionRevocationV1` لمزود يجب سحب المغلف الخاص به. يلزم `--envelope=<path>`, `--reason=<text>`, توقيع مجلس واحد على الأقل
    `--council-signature`، و`--revoked-at`/`--notes` اختيارية. يوقع CLI ويحقق ملخص الإلغاء، ويكتب حمولة Norito عبر `--revocation-out` ويطبع تقرير JSON بوجود الملخص وعدد التواقيع.
| التحقق | ينفذ مدقق مشترك يستخدمه Torii والبوابات و`‎sorafs-node`. توفير السيولة وحدة + تكامل CLI.【F:crates/sorafs_manifest/src/provider_admission.rs#L1】【F:crates/iroha_torii/src/sorafs/admission.rs#L1】 | الشبكات TL / التخزين | ✅ مكتملة |
| تكامل Torii | تاجر المرقم في عدد الإعلانات في Torii ورفض الإعلانات خارج السياسة وإصدار التليميترية. | الشبكات TL | ✅ مكتملة | يقوم Torii الآن بتحميل المظاريف الإلكترونية (`torii.sorafs.admission_envelopes_dir`) والتحقق من تطابق الملخص/التوقيع أثناء الاشتراك والبراز التليمتري قبول.【F:crates/iroha_torii/src/sorafs/admission.rs#L1】【F:crates/iroha_torii/src/sorafs/discovery.rs#L1】【F:crates/iroha_torii/src/sorafs/api.rs#L1】 || التجديد | إضافة مخطط/الإلغاء + مساعدات CLI ونشر دليل دورة الحياة في الوثائق (راجع الـ runbook أدناه وأوامر CLI في `provider-admission renewal`/`revoke`).[crates/sorafs_car/src/bin/sorafs_manifest_stub/provider_admission.rs#L477][docs/source/sorafs/provider_admission_policy.md:120] | التخزين / الحوكمة | ✅ مكتملة |
| التليمترية | تعريف اللوحات/تنبيهات `provider_admission` (تجديد مفقود، انتهاء الظرف). | إمكانية الملاحظة | 🟠 جار | العداد `torii_sorafs_admission_total{result,reason}` موجود؛ لوحات/تنبيهات قيد الانتظار. 【F:crates/iroha_telemetry/src/metrics.rs#L3798】【F:docs/source/telemetry.md#L614】 |

### دليل الإلغاء والإلغاء

#### تجديد جدول (تحديثات الحصة/الطوبولوجيا)
1. أنشئ اقتراح الزوج/إعلان اللاحق باستخدام `provider-admission proposal` و`provider-admission sign`، مع زيادة `--retention-epoch` وتحديث الحصة/نقاط النهاية حسب الحاجة.
2. ينفذ
   ```bash
   cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- provider-admission \
     renewal \
     --previous-envelope=governance/providers/<id>/envelope.to \
     --envelope=governance/providers/<id>/envelope_next.to \
     --renewal-out=governance/providers/<id>/renewal.to \
     --json-out=governance/providers/<id>/renewal.json \
     --notes="stake top-up 2025-03"
   ```
   يقوم الأمر بالتحقق من ثبات ملكية القدرة/الملف عبر `AdmissionRecord::apply_renewal`، ويصدر `ProviderAdmissionRenewalV1` ويطبع ملخصات للسجل الانضمام.[crates/sorafs_car/src/bin/sorafs_manifest_stub/provider_admission.rs#L477] 【F:crates/sorafs_manifest/src/provider_admission.rs#L422】
3. استبدال الـ Envelope السابق في `torii.sorafs.admission_envelopes_dir`، وثبّت التجديد Norito/JSON في مستودع الـ تور، وأضف التجزئة الأصلية + فترة الاحتفاظ إلى `docs/source/sorafs/migration_ledger.md`.
4. أخطر المشغلين بأن الـ Envelope الجديد أصبح نشطا وراقب `torii_sorafs_admission_total{result="accepted",reason="stored"}` لتأكيد الإدخال.
5. إعادة إنشاء وتثبيت التركيبات عبر `cargo run -p sorafs_car --bin provider_admission_fixtures --features cli`؛ CI (`ci/check_sorafs_fixtures.sh`) ويحق له إثبات مخرجات Norito.#### تعطيل طارئ
1.حدد الـ المغلف المخترق واصدر إلغاء:
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
   يوقع CLI `ProviderAdmissionRevocationV1`، ويتحقق من مجموعة التواقيع عبر `verify_revocation_signatures`، ولم يهضم الإلغاء. 【crates/sorafs_car/src/bin/sorafs_manifest_stub/provider_admission.rs#L593】【F:crates/sorafs_manifest/src/provider_admission.rs#L486】
2. أزل الـ المغلف من `torii.sorafs.admission_envelopes_dir`، وتوزيع Norito/JSON للإلغاء على ذاكرة التخزين المؤقت، اختراع التجزئة المبدع في محاضر الـتجديد.
3. راقب `torii_sorafs_admission_total{result="rejected",reason="admission_missing"}` لتأكيد أن الكاشات تُسقط إعلان الملغى؛ واحتفظ بآثار الإلغاء في المراجعات.

##الفرقة والتليمترية

- إضافة تركيبات ذهبية لأفكار مقترحة ومبتكرة تحت
  `fixtures/sorafs_manifest/provider_admission/`.
- توسيع CI (`ci/check_sorafs_fixtures.sh`) اقتراح مقترحات جديدة والتحقق من الـ المغلفات.
-تحتوي على تركيبات المولدة `metadata.json` مع الملخصات الممتدة؛ يؤكد قوة المصب أن
  `proposal_digest_hex` == `ca8e73a1f319ae83d7bd958ccb143f9b790c7e4d9c8dfe1f6ad37fa29facf936`.
- توفير السيولة تكامل:
  - رفض إعلانات Torii ذات المظاريف تقبل الرغبة أو منتهية الامتياز.
  - يقوم CLI بعملية ذهاب وإياب للمقترح → المغلف → التحقق.
  - يقوم بتحديث التدوير بتدوير استيثاق نقطة النهاية دون تغيير معرف المزود.
- المتطلبات التوضيحية:
  - اصدار عدادات `provider_admission_envelope_{accepted,rejected}` في Torii. ✅ `torii_sorafs_admission_total{result,reason}` يعرض نتائج الرفض/الرفض.
  - إضافة تحذيرات انتهاء الصلاحية إلى لوحات التوقيع (تجديد مستحق خلال 7 أيام).

##الخطوات التالية1. ✅ تم تغيير مخطط التغييرات في Norito ولم تعد تدخل في التحقق من `sorafs_manifest::provider_admission`. لا حاجة لميزات.
٢. حافظ على سكربتات الـ تور مع الـ runbook.
3. ✅ يقوم Torii بالقبول/الاكتشاف بالتأكيد على المغلفات ويعرض عدادات تليميترية للقبول/الرفض.
4.النتيجة النهائية: لوحات ديفيديا/تنبيهات النهائية لفورت تنبيهات عند قرب الاستحقاق خلال سبعة أيام (`torii_sorafs_admission_total`، مقاييس انتهاء الصلاحية).