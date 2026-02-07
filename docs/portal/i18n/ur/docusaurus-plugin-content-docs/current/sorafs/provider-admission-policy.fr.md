---
lang: ur
direction: rtl
source: docs/portal/docs/sorafs/provider-admission-policy.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

> [`docs/source/sorafs/provider_admission_policy.md`] (https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/provider_admission_policy.md) سے موافقت پذیر۔

# فراہم کنندہ داخلہ اور شناخت کی پالیسی SoraFS (مسودہ SF-2B)

اس نوٹ میں ** SF-2B ** کے لئے قابل عمل فراہمی کو حاصل کیا گیا ہے: وضاحت کریں اور
داخلہ ورک فلو ، شناخت کی ضروریات اور پے لوڈ کا اطلاق کریں
اسٹوریج فراہم کرنے والوں کے لئے سرٹیفکیٹ SoraFS۔ یہ عمل میں توسیع کرتا ہے
اعلی سطح کے فن تعمیر میں آر ایف سی SoraFS میں بیان کیا گیا ہے اور کام کو توڑ دیتا ہے
ٹریک ایبل انجینئرنگ کے کاموں میں باقی۔

## پالیسی کے مقاصد

- اس بات کو یقینی بنائیں کہ صرف تصدیق شدہ آپریٹرز ریکارڈنگ پوسٹ کرسکتے ہیں
  `ProviderAdvertV1` نیٹ ورک کے ذریعہ قبول کیا گیا۔
- ہر اعلان کی کلید کو گورننس کے ذریعہ منظور شدہ شناخت دستاویز سے لنک کریں ،
  تصدیق شدہ اختتامی نکات اور کم سے کم حصص کی شراکت۔
- تشخیصی توثیق کے ٹولز فراہم کریں تاکہ Torii ، گیٹ وے
  اور `sorafs-node` ایک ہی کنٹرول کا اطلاق کریں۔
- توڑنے کے بغیر ہنگامی تجدید اور منسوخی کی حمایت کریں
  تعی .ن اور نہ ہی ٹولز کی ایرگونومکس۔

## شناخت اور اسٹیکنگ کی ضروریات

| ضرورت | تفصیل | فراہمی |
| --------- | ------------- | ---------- |
| AD کلیدی پروویژن | فراہم کنندگان کو ایک ED25519 کلیدی جوڑی رجسٹر کرنا ہوگی جو ہر اشتہار پر دستخط کرتی ہے۔ داخلہ بنڈل عوامی کلید کو گورننس کے دستخط کے ساتھ محفوظ کرتا ہے۔ | `advert_key` (32 بائٹس) کے ساتھ اسکیما `ProviderAdmissionProposalV1` میں توسیع کریں اور رجسٹر (`sorafs_manifest::provider_admission`) سے اس کا حوالہ دیں۔ |
| اسٹیک پوائنٹر | داخلے کے لئے ایک غیر صفر `StakePointer` ایک فعال اسٹیکنگ پول کی طرف اشارہ کرنے کی ضرورت ہے۔ | `sorafs_manifest::provider_advert::StakePointer::validate()` میں توثیق شامل کریں اور CLI/ٹیسٹ میں غلطیوں کی اطلاع دیں۔ |
| دائرہ اختیار ٹیگز | فراہم کرنے والے دائرہ اختیار + قانونی رابطے کا اعلان کرتے ہیں۔ | `jurisdiction_code` (ISO 3166-1 الفا -2) اور اختیاری `contact_uri` کے ساتھ پروپوزل اسکیما میں توسیع کریں۔ |
| اختتامی نقطہ سرٹیفیکیشن | ہر مشتہر اختتامی نقطہ کو ایم ٹی ایل ایس یا کوئیک سرٹیفکیٹ رپورٹ کے ذریعہ تعاون کرنا چاہئے۔ | پے لوڈ Norito `EndpointAttestationV1` کی وضاحت کریں اور اسے داخلہ بنڈل میں فی اختتامی نقطہ اسٹور کریں۔ |

## داخلہ ورک فلو1. ** تجویز تخلیق **
   - CLI: `cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- provider-admission proposal ...` شامل کریں
     `ProviderAdmissionProposalV1` + تصدیقی بنڈل تیار کرنا۔
   - توثیق: `profile_id` میں مطلوبہ فیلڈز ، اسٹیک> 0 ، کیننیکل چنکر ہینڈل کو یقینی بنائیں۔
2. ** گورننس کی توثیق **
   - کونسل `blake3("sorafs-provider-admission-v1" || canonical_bytes)` کے ذریعہ ٹولنگ کے ذریعے دستخط کرتی ہے
     موجودہ لفافہ (ماڈیول `sorafs_manifest::governance`)۔
   - لفافہ `governance/providers/<provider_id>/admission.json` میں برقرار ہے۔
3. ** رجسٹری انجشن **
   - مشترکہ چیکر (`sorafs_manifest::provider_admission::validate_envelope`) نافذ کریں
     Torii/گیٹ ویز/CLI کے ذریعہ دوبارہ استعمال کیا گیا۔
   - داخلے کا راستہ Torii ان اشتہارات کو مسترد کرنے کے لئے اپ ڈیٹ کریں جن کی ڈائجسٹ یا میعاد ختم ہونا
     لفافے سے مختلف ہے۔
4. ** تجدید اور منسوخی **
   - اختیاری اختتامی نقطہ/اسٹیک اپڈیٹس کے ساتھ `ProviderAdmissionRenewalV1` شامل کریں۔
   - ایک سی ایل آئی پاتھ `--revoke` کو بے نقاب کریں جو منسوخ ہونے کی وجہ کو لاگ ان کرتا ہے اور گورننس ایونٹ کو آگے بڑھاتا ہے۔

## نفاذ کے کام

| ڈومین | ٹاسک | مالک (زبانیں) | حیثیت |
| -------- | ------ | ---------- | -------- |
| اسکیما | `ProviderAdmissionEnvelopeV1` ، `EndpointAttestationV1` (Norito) `crates/sorafs_manifest/src/provider_admission.rs` کے تحت سیٹ کریں۔ توثیق کے مددگاروں کے ساتھ `sorafs_manifest::provider_admission` میں نافذ کیا گیا ہے اسٹوریج / گورننس | ✅ کیا ہوا |
| سی ایل آئی ٹولنگ | `sorafs_manifest_stub` کو سب کامنڈس کے ساتھ بڑھاؤ: `provider-admission proposal` ، `provider-admission sign` ، `provider-admission verify`۔ | ٹولنگ ڈبلیو جی | ✅ |

سی ایل آئی فلو اب انٹرمیڈیٹ سرٹیفکیٹ بنڈل (`--endpoint-attestation-intermediate`) ، جاری کردہ تجویز/لفافہ کیننیکل بائٹس کو قبول کرتا ہے ، اور `sign`/`verify` کے دوران بورڈ کے دستخطوں کی توثیق کرتا ہے۔ آپریٹرز براہ راست اشتہاری اداروں کو فراہم کرسکتے ہیں ، یا دستخط شدہ اشتہارات کو دوبارہ استعمال کرسکتے ہیں ، اور آٹومیشن کی سہولت کے ل i `--council-signature-file` کے ساتھ `--council-signature-public-key` کو ملا کر دستخطی فائلیں فراہم کی جاسکتی ہیں۔

### سی ایل آئی حوالہ

`cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- provider-admission ...` کے ذریعے ہر کمانڈ چلائیں۔- `proposal`
  - مطلوبہ جھنڈے: `--provider-id=<hex32>` ، `--chunker-profile=<namespace.name@semver>` ،
    `--stake-pool-id=<hex32>` ، `--stake-amount=<amount>` ، `--advert-key=<hex32>` ،
    `--jurisdiction-code=<ISO3166-1>` ، اور کم از کم ایک `--endpoint=<kind:host>`۔
  - اختتامی نقطہ کی تصدیق `--endpoint-attestation-attested-at=<secs>` کی توقع کرتی ہے ،
    `--endpoint-attestation-expires-at=<secs>` ، ایک سرٹیفکیٹ کے ذریعے
    `--endpoint-attestation-leaf=<path>` (پلس `--endpoint-attestation-intermediate=<path>`
    ہر تار عنصر کے لئے اختیاری) اور کوئی بھی مذاکرات ALPN ID
    (`--endpoint-attestation-alpn=<token>`)۔ کوئیک اختتامی نکات ٹرانسپورٹ رپورٹس کے ذریعے فراہم کرسکتے ہیں
    `--endpoint-attestation-report[-hex]=...`۔
  ۔
    (پہلے سے طے شدہ stdout یا `--json-out`)۔
- `sign`
  - ان پٹ: ایک تجویز (`--proposal`) ، ایک دستخط شدہ اشتہار (`--advert`) ، ایک اختیاری اشتہاری ادارہ
    (`--advert-body`) ، برقرار رکھنے کا دور اور بورڈ کے کم از کم ایک دستخط۔ دستخط ہوسکتے ہیں
    ان لائن (`--council-signature=<signer_hex:signature_hex>`) یا فائلوں کے ذریعے ملا کر فراہم کیا گیا ہے
    `--council-signature-public-key` کے ساتھ `--council-signature-file=<path>`۔
  - ایک توثیق شدہ لفافہ (`--envelope-out`) اور ایک JSON رپورٹ تیار کرتا ہے جس میں ڈائجسٹ لنکس کی نشاندہی ہوتی ہے ،
    دستخط کنندگان کی تعداد اور اندراج کے راستے۔
- `verify`
  - تجویز کی اختیاری توثیق کے ساتھ ، ایک موجودہ لفافے (`--envelope`) کی توثیق کرتا ہے ،
    متعلقہ اشتہار یا اشتہاری باڈی کا۔ JSON رپورٹ ڈائجسٹ اقدار کو اجاگر کرتی ہے ،
    دستخطی توثیق کی حالت اور اسی طرح کے اختیاری نمونے۔
- `renewal`
  - ایک نیا منظور شدہ لفافہ اس سے پہلے کی توثیق شدہ ڈائجسٹ سے لنک کرتا ہے۔ ضرورت ہے
    `--previous-envelope=<path>` اور جانشین `--envelope=<path>` (دو پے لوڈ Norito)۔
    سی ایل آئی کی تصدیق کرتی ہے کہ پروفائل عرفی ، صلاحیتوں اور اشتہاری چابیاں میں کوئی تبدیلی نہیں ہے ،
    داؤ ، اختتامی نقطہ اور میٹا ڈیٹا کی تازہ کاریوں کی اجازت دیتے ہوئے۔ کیننیکل بائٹس کو خارج کرتا ہے
    `ProviderAdmissionRenewalV1` (`--renewal-out`) نیز JSON خلاصہ۔
- `revoke`
  - کسی ایسے فراہم کنندہ کے لئے ایک ہنگامی بنڈل `ProviderAdmissionRevocationV1` جاری کرتا ہے جس کا لفافہ لازمی ہے
    ہٹا دیا جائے. `--envelope=<path>` ، `--reason=<text>` ، کم از کم ایک کی ضرورت ہے
    `--council-signature` ، اور اختیاری طور پر `--revoked-at`/`--notes`۔ سی ایل آئی کی علامت ہے اور اس کی توثیق کرتی ہے
    منسوخی ڈائجسٹ ، `--revocation-out` کے ذریعے پے لوڈ Norito لکھتا ہے ، اور JSON رپورٹ پرنٹ کرتا ہے
    ڈائجسٹ اور دستخطوں کی تعداد کے ساتھ۔
| توثیق | Torii ، گیٹ ویز اور `sorafs-node` کے ذریعہ استعمال شدہ مشترکہ چیکر کو نافذ کریں۔ یونٹ ٹیسٹنگ + سی ایل آئی انضمام فراہم کریں نیٹ ورکنگ TL / اسٹوریج | ✅ کیا ہوا || انضمام Torii | چیکر کو Torii اشتہار میں انجیکشن لگائیں ، پالیسی سے باہر کی اشتہارات کو مسترد کریں ، ٹیلی میٹری جاری کریں۔ | نیٹ ورکنگ TL | ✅ کیا ہوا | Torii اب گورننس لفافے (`torii.sorafs.admission_envelopes_dir`) لوڈ کرتا ہے ، چیک ہضم/دستخطی میچوں کے دوران اور ٹیلی میٹری کو بے نقاب کرتا ہے 【F: کریٹس/اروہہ_ٹوری/ایس آر سی/سرفس/داخلہ
| تجدید | تجدید/منسوخی اسکیما + سی ایل آئی مددگار شامل کریں ، دستاویزات میں لائف سائیکل گائیڈ پوسٹ کریں (نیچے رن بک دیکھیں اور سی ایل آئی کمانڈز دیکھیں۔ `provider-admission renewal`/`revoke`). 【کریٹس/sorafs_car/src/sorafs_manifest_stub/فراہم کنندہ_ایڈیشن.رس#l477 】【 دستاویزات/ماخذ/sorafs/فراہم کنندہ_ایڈیشن_پولیسی.مڈی: 120】 | اسٹوریج / گورننس | ✅ کیا ہوا |
| ٹیلی میٹری | ڈیش بورڈز/الرٹس `provider_admission` کی وضاحت کریں (تجدید کی تجدید ، لفافے کی میعاد ختم)۔ | مشاہدہ | 🟠 ترقی میں | کاؤنٹر `torii_sorafs_admission_total{result,reason}` موجود ہے ؛ زیر التواء ڈیش بورڈز/الرٹس

### تجدید اور منسوخ کرنے والی رن بک

#### شیڈول تجدید (داؤ/ٹوپولوجی کی تازہ کاری)
1. `provider-admission proposal` اور `provider-admission sign` کے ساتھ جانشین کی تجویز/اشتہار کی جوڑی بنائیں ، `--retention-epoch` میں اضافہ اور ضرورت کے مطابق اسٹیک/اختتامی مقامات کو اپ ڈیٹ کریں۔
2. چلائیں
   ```bash
   cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- provider-admission \
     renewal \
     --previous-envelope=governance/providers/<id>/envelope.to \
     --envelope=governance/providers/<id>/envelope_next.to \
     --renewal-out=governance/providers/<id>/renewal.to \
     --json-out=governance/providers/<id>/renewal.json \
     --notes="stake top-up 2025-03"
   ```
   کمانڈ غیر تبدیل شدہ صلاحیت/پروفائل فیلڈز کے ذریعے توثیق کرتا ہے
   `AdmissionRecord::apply_renewal` ، `ProviderAdmissionRenewalV1` کو جاری کرتا ہے ، اور ہضموں کو پرنٹ کرتا ہے
   گورننس لاگ۔ 【کریٹس/sorafs_car/src/sorafs_manifest_stub/provider_admission.rs#l477 】【 f: creats/sorafs_manifest/src/provider_admission.rs#l422】
3. `torii.sorafs.admission_envelopes_dir` میں پچھلے لفافے کو تبدیل کریں ، تجدید Norito/JSON کو گورننس ریپوزٹری میں مرتب کریں ، اور `docs/source/sorafs/migration_ledger.md` میں تجدید ہیش + برقرار رکھنے کے دور کو شامل کریں۔
4. آپریٹرز کو مطلع کریں کہ نیا لفافہ فعال ہے اور `torii_sorafs_admission_total{result="accepted",reason="stored"}` کی نگرانی کے لئے `torii_sorafs_admission_total{result="accepted",reason="stored"}` کی نگرانی کریں۔
5. `cargo run -p sorafs_car --bin provider_admission_fixtures --features cli` کے ذریعے کیننیکل فکسچر کو دوبارہ تخلیق کریں اور ان کا ارتکاب کریں۔ CI (`ci/check_sorafs_fixtures.sh`) توثیق کرتا ہے کہ Norito آؤٹ پٹ مستحکم رہتے ہیں۔

#### ہنگامی منسوخی
1. سمجھوتہ کرنے والے لفافے کی شناخت کریں اور منسوخیاں جاری کریں:
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
   CLI `ProviderAdmissionRevocationV1` پر دستخط کرتا ہے ، تمام دستخطوں کی تصدیق کرتا ہے
   `verify_revocation_signatures` ، اور منسوخی ڈائجسٹ کی اطلاع دیتا ہے۔ 【کریٹس/sorafs_car/src/sorafs_manifest_stub/provider_admission.rs#l593 】【 f: creats/sorafs_manifest/src/proper_admission.rs#l486】
2. `torii.sorafs.admission_envelopes_dir` سے ریپر کو ہٹا دیں ، منسوخی Norito/JSON کو داخلہ کیچوں میں تقسیم کریں ، اور گورننس کے منٹوں میں وجہ ہیش کو ریکارڈ کریں۔
3. مانیٹر `torii_sorafs_admission_total{result="rejected",reason="admission_missing"}` اس بات کی تصدیق کرنے کے لئے کہ کیچز منسوخ شدہ اشتہار کو چھوڑ رہے ہیں۔ واقعے کے پسپائیوں میں منسوخی کے نمونے برقرار رکھیں۔

## جانچ اور ٹیلی میٹری- تجاویز اور داخلہ لفافوں کے لئے سنہری فکسچر شامل کریں
  `fixtures/sorafs_manifest/provider_admission/`۔
- تجاویز کو دوبارہ تخلیق کرنے اور لفافوں کو چیک کرنے کے لئے CI (`ci/check_sorafs_fixtures.sh`) میں توسیع کریں۔
- تیار کردہ فکسچر میں `metadata.json` شامل ہے جس میں کیننیکل ہضم ہوتا ہے۔ بہاو ​​ٹیسٹ توثیق کرتے ہیں
  `proposal_digest_hex` == `ca8e73a1f319ae83d7bd958ccb143f9b790c7e4d9c8dfe1f6ad37fa29facf936`۔
- انضمام کے ٹیسٹ فراہم کریں:
  - Torii گمشدہ یا میعاد ختم ہونے والے داخلے کے لفافوں والے اشتہارات کو مسترد کرتا ہے۔
  - CLI ایک راؤنڈ ٹرپ تجویز → لفافہ → توثیق کرتا ہے۔
  - گورننس کی تجدید فراہم کنندہ ID کو تبدیل کیے بغیر اختتامی نقطہ تصدیق کو گھوماتی ہے۔
- ٹیلی میٹری کی ضروریات:
  - Torii میں `provider_admission_envelope_{accepted,rejected}` جاری کریں۔ ✅ `torii_sorafs_admission_total{result,reason}` اب قبول شدہ/مسترد شدہ نتائج کو بے نقاب کرتا ہے۔
  - مشاہدہ کرنے والے ڈیش بورڈز میں میعاد ختم ہونے کے انتباہات شامل کریں (7 دن کے اندر تجدید کی تجدید)۔

## اگلے اقدامات

1. Sch اسکیما Norito میں ہونے والی تبدیلیوں کو حتمی شکل دی اور توثیق کے مددگاروں کو مربوط کیا
   `sorafs_manifest::provider_admission`۔ کسی خصوصیت کے جھنڈے کی ضرورت نہیں ہے۔
2. ✅ CLI ورک فلوز (`proposal` ، `sign` ، `verify` ، `renewal` ، `revoke`) انضمام ٹیسٹ کے ذریعہ دستاویزی اور استعمال کیا گیا ہے۔ رن بک کے ساتھ ہم آہنگی میں گورننس اسکرپٹ رکھیں۔
3. ✅ داخلہ/دریافت Torii لفافوں کو کھاتا ہے اور قبولیت/مسترد ٹیلی میٹری کاؤنٹرز کو بے نقاب کرتا ہے۔
4. مشاہدہ کی توجہ کا مرکز: داخلہ ڈیش بورڈز/انتباہات کو مکمل کریں تاکہ سات دن کے اندر اندر تجدیدات ٹرگر وارننگ (`torii_sorafs_admission_total` ، میعاد ختم گیجز)۔