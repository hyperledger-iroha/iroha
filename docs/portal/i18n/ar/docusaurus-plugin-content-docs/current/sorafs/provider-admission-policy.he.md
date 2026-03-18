---
lang: he
direction: rtl
source: docs/portal/i18n/ar/docusaurus-plugin-content-docs/current/sorafs/provider-admission-policy.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 7dcf11ff347a6ce1e974d91349574ba044283bd054f8f898804aa33ad9e4c469
source_last_modified: "2026-01-03T18:08:02+00:00"
translation_last_reviewed: 2026-01-30
---


---
id: provider-admission-policy
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/provider-admission-policy.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---


> مقتبس من [`docs/source/sorafs/provider_admission_policy.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/provider_admission_policy.md).

# سياسة قبول وهوية مزودي SoraFS (مسودة SF-2b)

توثق هذه المذكرة المخرجات العملية لـ **SF-2b**: تعريف مسار القبول، ومتطلبات الهوية، وحمولات
الاستيثاق لمزودي التخزين في SoraFS وتطبيقها. وهي توسع العملية عالية المستوى الموضحة في RFC
معمارية SoraFS وتجزئ العمل المتبقي إلى مهام هندسية قابلة للتتبع.

## أهداف السياسة

- ضمان أن المشغلين المُدقَّقين فقط يمكنهم نشر سجلات `ProviderAdvertV1` التي تقبلها الشبكة.
- ربط كل مفتاح إعلان بوثيقة هوية معتمدة من الحوكمة، ونقاط نهاية مستوثقة، ومساهمة حد أدنى من الـ stake.
- توفير أدوات تحقق حتمية لكي يطبق Torii والبوابات و`‎sorafs-node` نفس الفحوصات.
- دعم التجديد وإلغاء الطوارئ دون كسر الحتمية أو ergonomics الأدوات.

## متطلبات الهوية والـ stake

| المتطلب | الوصف | المخرج |
|---------|-------|--------|
| مصدر مفتاح الإعلان | يجب أن يسجل المزودون زوج مفاتيح Ed25519 يوقع كل advert. تقوم حزمة القبول بتخزين المفتاح العام مع توقيع الحوكمة. | توسيع مخطط `ProviderAdmissionProposalV1` بـ `advert_key` (32 بايت) والإشارة إليه من السجل (`sorafs_manifest::provider_admission`). |
| مؤشر stake | يتطلب القبول `StakePointer` غير صفري يشير إلى مجمع staking نشط. | إضافة التحقق في `sorafs_manifest::provider_advert::StakePointer::validate()` وإظهار الأخطاء في CLI/الاختبارات. |
| وسوم الاختصاص القضائي | يعلن المزودون الاختصاص + جهة اتصال قانونية. | توسيع مخطط المقترح بـ `jurisdiction_code` (ISO 3166-1 alpha-2) و`contact_uri` اختياري. |
| استيثاق نقطة النهاية | يجب أن تكون كل نقطة نهاية مُعلن عنها مدعومة بتقرير شهادة mTLS أو QUIC. | تعريف حمولة Norito `EndpointAttestationV1` وتخزينها لكل نقطة نهاية داخل حزمة القبول. |

## سير عمل القبول

1. **إنشاء المقترح**
   - CLI: إضافة `cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- provider-admission proposal ...`
     لإنتاج `ProviderAdmissionProposalV1` + حزمة الاستيثاق.
   - التحقق: ضمان الحقول المطلوبة، وstake > 0، ومقبض chunker قياسي في `profile_id`.
2. **اعتماد الحوكمة**
   - يوقع المجلس `blake3("sorafs-provider-admission-v1" || canonical_bytes)` باستخدام أدوات envelope الحالية
     (وحدة `sorafs_manifest::governance`).
   - يتم حفظ الـ envelope في `governance/providers/<provider_id>/admission.json`.
3. **إدخال السجل**
   - تنفيذ مدقق مشترك (`sorafs_manifest::provider_admission::validate_envelope`) يعيد استخدامه Torii/البوابات/CLI.
   - تحديث مسار القبول في Torii لرفض adverts التي يختلف digest أو تاريخ الانتهاء فيها عن الـ envelope.
4. **التجديد والإلغاء**
   - إضافة `ProviderAdmissionRenewalV1` مع تحديثات اختيارية لنقاط النهاية/الـ stake.
   - إتاحة مسار CLI `--revoke` يسجل سبب الإلغاء ويدفع حدث حوكمة.

## مهام التنفيذ

| المجال | المهمة | Owner(s) | الحالة |
|--------|-------|----------|--------|
| المخطط | تعريف `ProviderAdmissionProposalV1` و`ProviderAdmissionEnvelopeV1` و`EndpointAttestationV1` (Norito) ضمن `crates/sorafs_manifest/src/provider_admission.rs`. مُنفذ داخل `sorafs_manifest::provider_admission` مع مساعدات تحقق.【F:crates/sorafs_manifest/src/provider_admission.rs#L1】 | Storage / Governance | ✅ مكتمل |
| أدوات CLI | توسيع `sorafs_manifest_stub` بأوامر فرعية: `provider-admission proposal`, `provider-admission sign`, `provider-admission verify`. | Tooling WG | ✅ |

مسار CLI يدعم الآن حزم الشهادات الوسيطة (`--endpoint-attestation-intermediate`) ويصدر bytes قياسية للمقترح/الـ envelope ويتحقق من تواقيع المجلس أثناء `sign`/`verify`. يمكن للمشغلين توفير أجسام adverts مباشرة أو إعادة استخدام adverts موقعة، ويمكن تمرير ملفات التواقيع عبر الجمع بين `--council-signature-public-key` و`--council-signature-file` لتسهيل الأتمتة.

### مرجع CLI

نفّذ كل أمر عبر `cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- provider-admission ...`.

- `proposal`
  - الأعلام المطلوبة: `--provider-id=<hex32>`, `--chunker-profile=<namespace.name@semver>`,
    `--stake-pool-id=<hex32>`, `--stake-amount=<amount>`, `--advert-key=<hex32>`,
    `--jurisdiction-code=<ISO3166-1>`, وعلى الأقل `--endpoint=<kind:host>` واحد.
  - استيثاق كل نقطة نهاية يتطلب `--endpoint-attestation-attested-at=<secs>`,
    `--endpoint-attestation-expires-at=<secs>`، وشهادة عبر
    `--endpoint-attestation-leaf=<path>` (مع `--endpoint-attestation-intermediate=<path>` اختياري لكل عنصر في السلسلة) وأي معرفات ALPN متفاوض عليها
    (`--endpoint-attestation-alpn=<token>`). يمكن لنقاط نهاية QUIC توفير تقارير النقل عبر
    `--endpoint-attestation-report[-hex]=...`.
  - المخرجات: bytes قياسية لمقترح Norito (`--proposal-out`) وملخص JSON
    (stdout افتراضي أو `--json-out`).
- `sign`
  - المدخلات: مقترح (`--proposal`)، advert موقع (`--advert`)، جسم advert اختياري
    (`--advert-body`)، epoch احتفاظ، وتوقيع مجلس واحد على الأقل. يمكن تمرير التواقيع
    inline (`--council-signature=<signer_hex:signature_hex>`) أو عبر ملفات باستخدام
    `--council-signature-public-key` مع `--council-signature-file=<path>`.
  - ينتج envelope مُحقق (`--envelope-out`) وتقرير JSON يوضح روابط digest وعدد الموقّعين ومسارات الإدخال.
- `verify`
  - يتحقق من envelope موجود (`--envelope`) مع فحص اختياري للمقترح المطابق أو advert أو جسم advert. يسلط تقرير JSON الضوء على قيم digest وحالة تحقق التواقيع وأي artefacts اختيارية مطابقة.
- `renewal`
  - يربط envelope مُعتمد جديد بالـ digest الذي تم التصديق عليه سابقا. يتطلب
    `--previous-envelope=<path>` و`--envelope=<path>` التالي (كلاهما حمولة Norito).
    يتحقق CLI من بقاء profile aliases والقدرات ومفاتيح advert دون تغيير، مع السماح بتحديثات stake ونقاط النهاية والـ metadata. ينتج bytes قياسية
    `ProviderAdmissionRenewalV1` (`--renewal-out`) إضافة إلى ملخص JSON.
- `revoke`
  - يصدر حزمة طوارئ `ProviderAdmissionRevocationV1` لمزود يجب سحب envelope الخاص به. يتطلب `--envelope=<path>`, `--reason=<text>`, توقيع مجلس واحد على الأقل
    `--council-signature`، و`--revoked-at`/`--notes` اختيارية. يوقع CLI ويُحقق digest الإلغاء، ويكتب حمولة Norito عبر `--revocation-out` ويطبع تقرير JSON يضم digest وعدد التواقيع.
| التحقق | تنفيذ مدقق مشترك يستخدمه Torii والبوابات و`‎sorafs-node`. توفير اختبارات وحدة + تكامل CLI.【F:crates/sorafs_manifest/src/provider_admission.rs#L1】【F:crates/iroha_torii/src/sorafs/admission.rs#L1】 | Networking TL / Storage | ✅ مكتمل |
| تكامل Torii | تمرير المدقق في إدخال adverts في Torii ورفض adverts خارج السياسة وإصدار التليمترية. | Networking TL | ✅ مكتمل | يقوم Torii الآن بتحميل envelopes الحوكمة (`torii.sorafs.admission_envelopes_dir`) والتحقق من تطابق digest/التوقيع أثناء الإدخال وإبراز تليمترية القبول.【F:crates/iroha_torii/src/sorafs/admission.rs#L1】【F:crates/iroha_torii/src/sorafs/discovery.rs#L1】【F:crates/iroha_torii/src/sorafs/api.rs#L1】 |
| التجديد | إضافة مخطط التجديد/الإلغاء + مساعدات CLI ونشر دليل دورة الحياة في الوثائق (راجع الـ runbook أدناه وأوامر CLI في `provider-admission renewal`/`revoke`).【crates/sorafs_car/src/bin/sorafs_manifest_stub/provider_admission.rs#L477】【docs/source/sorafs/provider_admission_policy.md:120】 | Storage / Governance | ✅ مكتمل |
| التليمترية | تعريف لوحات/تنبيهات `provider_admission` (تجديد مفقود، انتهاء envelope). | Observability | 🟠 جار | عداد `torii_sorafs_admission_total{result,reason}` موجود؛ لوحات/تنبيهات قيد الانتظار.【F:crates/iroha_telemetry/src/metrics.rs#L3798】【F:docs/source/telemetry.md#L614】 |

### دليل التجديد والإلغاء

#### تجديد مجدول (تحديثات stake/topology)
1. أنشئ زوج المقترح/advert اللاحق باستخدام `provider-admission proposal` و`provider-admission sign`، مع زيادة `--retention-epoch` وتحديث stake/endpoints حسب الحاجة.
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
   يقوم الأمر بالتحقق من ثبات حقول القدرة/الملف عبر `AdmissionRecord::apply_renewal`، ويصدر `ProviderAdmissionRenewalV1` ويطبع digests لسجل الحوكمة.【crates/sorafs_car/src/bin/sorafs_manifest_stub/provider_admission.rs#L477】【F:crates/sorafs_manifest/src/provider_admission.rs#L422】
3. استبدل الـ envelope السابق في `torii.sorafs.admission_envelopes_dir`، وثبّت تجديد Norito/JSON في مستودع الحوكمة، وأضف hash التجديد + retention epoch إلى `docs/source/sorafs/migration_ledger.md`.
4. أخطر المشغلين بأن الـ envelope الجديد أصبح نشطا وراقب `torii_sorafs_admission_total{result="accepted",reason="stored"}` لتأكيد الإدخال.
5. أعد توليد وتثبيت fixtures القياسية عبر `cargo run -p sorafs_car --bin provider_admission_fixtures --features cli`؛ CI (`ci/check_sorafs_fixtures.sh`) يتحقق من ثبات مخرجات Norito.

#### إلغاء طارئ
1. حدد الـ envelope المخترق واصدر إلغاء:
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
   يوقع CLI `ProviderAdmissionRevocationV1`، ويتحقق من مجموعة التواقيع عبر `verify_revocation_signatures`، ويبلغ digest الإلغاء.【crates/sorafs_car/src/bin/sorafs_manifest_stub/provider_admission.rs#L593】【F:crates/sorafs_manifest/src/provider_admission.rs#L486】
2. أزل الـ envelope من `torii.sorafs.admission_envelopes_dir`، ووزع Norito/JSON للإلغاء على caches القبول، وسجل hash السبب في محاضر الحوكمة.
3. راقب `torii_sorafs_admission_total{result="rejected",reason="admission_missing"}` لتأكيد أن caches تُسقط advert الملغى؛ واحتفظ بآثار الإلغاء في مراجعات الحوادث.

## الاختبارات والتليمترية

- إضافة fixtures ذهبية لمقترحات وأغلفة القبول تحت
  `fixtures/sorafs_manifest/provider_admission/`.
- توسيع CI (`ci/check_sorafs_fixtures.sh`) لإعادة توليد المقترحات والتحقق من الـ envelopes.
- تتضمن fixtures المولدة `metadata.json` مع digests قياسية؛ تؤكد اختبارات downstream أن
  `proposal_digest_hex` == `ca8e73a1f319ae83d7bd958ccb143f9b790c7e4d9c8dfe1f6ad37fa29facf936`.
- توفير اختبارات تكامل:
  - يرفض Torii adverts ذات envelopes قبول مفقودة أو منتهية الصلاحية.
  - يقوم CLI بعملية round-trip للمقترح → envelope → التحقق.
  - يقوم تجديد الحوكمة بتدوير استيثاق endpoint دون تغيير معرف المزود.
- متطلبات التليمترية:
  - إصدار عدادات `provider_admission_envelope_{accepted,rejected}` في Torii. ✅ `torii_sorafs_admission_total{result,reason}` يعرض نتائج القبول/الرفض.
  - إضافة تحذيرات انتهاء الصلاحية إلى لوحات المراقبة (تجديد مستحق خلال 7 أيام).

## الخطوات التالية

1. ✅ تم إنهاء تغييرات مخطط Norito وإضافة مساعدات التحقق في `sorafs_manifest::provider_admission`. لا حاجة لميزات.
2. ✅ تم توثيق مسارات CLI (`proposal`, `sign`, `verify`, `renewal`, `revoke`) وتجريبها عبر اختبارات التكامل؛ حافظ على تزامن سكربتات الحوكمة مع الـ runbook.
3. ✅ يقوم Torii admission/discovery بإدخال الـ envelopes ويعرض عدادات التليمترية للقبول/الرفض.
4. التركيز على الملاحظة: إنهاء لوحات/تنبيهات القبول بحيث تطلق تنبيهات عند قرب الاستحقاق خلال سبعة أيام (`torii_sorafs_admission_total`, expiry gauges).
