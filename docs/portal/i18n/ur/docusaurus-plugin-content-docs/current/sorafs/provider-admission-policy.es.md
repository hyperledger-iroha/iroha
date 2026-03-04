---
lang: ur
direction: rtl
source: docs/portal/docs/sorafs/provider-admission-policy.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

> [`docs/source/sorafs/provider_admission_policy.md`] (https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/provider_admission_policy.md) سے موافقت پذیر۔

# سپلائر شناخت اور داخلہ پالیسی SoraFS (مسودہ SF-2B)

اس نوٹ میں ** SF-2B ** کے لئے قابل عمل فراہمی کو حاصل کیا گیا ہے: وضاحت کریں اور
داخلہ کے بہاؤ ، شناخت کی ضروریات اور پے لوڈ کا اطلاق کریں
اسٹوریج فراہم کرنے والوں کے لئے مصدقہ SoraFS۔ اسٹاپ کے عمل کو وسعت دیتا ہے
SoraFS آرکیٹیکچر آر ایف سی میں بیان کردہ سطح اور باقی کام کو تقسیم کریں
ٹریک ایبل انجینئرنگ کے کاموں میں۔

## پالیسی کے مقاصد

- اس بات کو یقینی بنائیں کہ صرف تصدیق شدہ آپریٹرز ہی ریکارڈ شائع کرسکتے ہیں
  `ProviderAdvertV1` جو نیٹ ورک قبول کرے گا۔
- ہر اشتہار کی کلید کو منظور شدہ ID دستاویز سے لنک کریں
  گورننس ، ہجوم کے اختتامی نکات ، اور کم سے کم اسٹیکنگ شراکت۔
- تشخیصی تصدیقی ٹولنگ فراہم کریں تاکہ Torii ، گیٹ ویز اور
  `sorafs-node` ایک ہی کنٹرول کا اطلاق کریں۔
- تعی .ن کو توڑے بغیر ہنگامی تجدید اور منسوخی کی حمایت کریں یا
  ٹولنگ کی ایرگونومکس۔

## شناخت اور داؤ کی ضروریات

| ضرورت | تفصیل | فراہمی |
| ----------- | ------------- | -------------- |
| AD کلیدی اصل | فراہم کنندگان کو ایک ED25519 کلیدی جوڑی رجسٹر کرنا ہوگی جو ہر اشتہار پر دستخط کرتی ہے۔ داخلہ بنڈل حکومت کے دستخط کے ساتھ ساتھ عوامی کلید کو بھی محفوظ کرتا ہے۔ | `ProviderAdmissionProposalV1` اسکیما کو `advert_key` (32 بائٹس) کے ساتھ بڑھاؤ اور اس کا رجسٹر (`sorafs_manifest::provider_admission`) سے حوالہ دیں۔ |
| اسٹیک پوائنٹر | داخلے کے لئے ایک غیر صفر `StakePointer` ایک فعال اسٹیکنگ پول کی طرف اشارہ کرنے کی ضرورت ہے۔ | `sorafs_manifest::provider_advert::StakePointer::validate()` پر توثیق شامل کریں اور CLI/ٹیسٹوں میں غلطیوں کو بے نقاب کریں۔ |
| دائرہ اختیار ٹیگز | سپلائی کرنے والے دائرہ اختیار + قانونی رابطے کا اعلان کرتے ہیں۔ | مجوزہ اسکیم کو `jurisdiction_code` (ISO 3166-1 الفا -2) اور اختیاری `contact_uri` کے ساتھ بڑھاؤ۔ |
| اینڈپوائنٹ ہجوم | ہر مشتہر اختتامی نقطہ کو ایم ٹی ایل ایس یا کوئیک سرٹیفکیٹ رپورٹ کے ذریعہ تعاون کرنا چاہئے۔ | پے لوڈ Norito `EndpointAttestationV1` کی وضاحت کریں اور داخلہ کے بنڈل کے اندر اسے فی اختتامی نقطہ اسٹور کریں۔ |

## انٹیک فلو

1. ** تجویز کی تخلیق **
   - CLI: `cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- provider-admission proposal ...` شامل کریں
     `ProviderAdmissionProposalV1` + تصدیقی بنڈل تیار کرنا۔
   - توثیق: `profile_id` میں مطلوبہ فیلڈز ، اسٹیک> 0 ، کیننیکل چنکر ہینڈل کو یقینی بنائیں۔
2. ** گورننس کی توثیق **
   - ٹولنگ کا استعمال کرتے ہوئے کونسل `blake3("sorafs-provider-admission-v1" || canonical_bytes)` کے اشارے
     موجودہ لفافہ (ماڈیول `sorafs_manifest::governance`)۔
   - لفافہ `governance/providers/<provider_id>/admission.json` میں برقرار ہے۔
3. ** لاگ انشن **
   - مشترکہ تصدیق کنندہ (`sorafs_manifest::provider_admission::validate_envelope`) کو نافذ کریں
     کہ Torii/گیٹ ویز/سی ایل آئی کا دوبارہ استعمال۔
   - Torii میں داخلہ کے راستے کو اپ ڈیٹ کریں تاکہ ان اشتہارات کو مسترد کریں جن کے ڈائجسٹ یا میعاد ختم ہونے سے لفافے سے مختلف ہوتا ہے۔
4. ** تجدید اور منسوخی **
   - اختیاری اختتامی نقطہ/اسٹیک اپڈیٹس کے ساتھ `ProviderAdmissionRenewalV1` شامل کریں۔
   - ایک CLI روٹ `--revoke` کو بے نقاب کریں جو منسوخ ہونے کی وجہ کو لاگ ان کرتا ہے اور گورننس ایونٹ بھیجتا ہے۔

## تعیناتی کے کام| علاقہ | ٹاسک | مالک (زبانیں) | حیثیت |
| ------ | ------- | ---------- | -------- |
| اسکیم | `ProviderAdmissionEnvelopeV1` ، `EndpointAttestationV1` (Norito) `crates/sorafs_manifest/src/provider_admission.rs` کے تحت وضاحت کریں۔ توثیق کے مددگاروں کے ساتھ `sorafs_manifest::provider_admission` میں نافذ کیا گیا ہے اسٹوریج / گورننس | ✅ مکمل |
| سی ایل آئی ٹولنگ | `sorafs_manifest_stub` کو سب کامنڈس کے ساتھ بڑھاؤ: `provider-admission proposal` ، `provider-admission sign` ، `provider-admission verify`۔ | ٹولنگ ڈبلیو جی | ✅ |

سی ایل آئی فلو اب انٹرمیڈیٹ سرٹیفکیٹ بنڈل (`--endpoint-attestation-intermediate`) ، مسئلے کو کیننیکل پروپوزل/لفافہ بائٹس کو قبول کرتا ہے ، اور `sign`/`verify` کے دوران کونسل کے دستخطوں کی توثیق کرتا ہے۔ آپریٹرز اشتہاری اداروں کو براہ راست فراہم کرسکتے ہیں یا دستخط شدہ اشتہارات کو دوبارہ استعمال کرسکتے ہیں ، اور آٹومیشن کی سہولت کے ل i `--council-signature-file` کے ساتھ `--council-signature-public-key` کو ملا کر دستخطی فائلیں فراہم کی جاسکتی ہیں۔

### سی ایل آئی حوالہ

`cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- provider-admission ...` کے ذریعے ہر کمانڈ پر عمل کریں۔- `proposal`
  - مطلوبہ جھنڈے: `--provider-id=<hex32>` ، `--chunker-profile=<namespace.name@semver>` ،
    `--stake-pool-id=<hex32>` ، `--stake-amount=<amount>` ، `--advert-key=<hex32>` ،
    `--jurisdiction-code=<ISO3166-1>` ، اور کم از کم ایک `--endpoint=<kind:host>`۔
  - اختتامی نقطہ کے ذریعہ تصدیق `--endpoint-attestation-attested-at=<secs>` کا انتظار کرتی ہے ،
    `--endpoint-attestation-expires-at=<secs>` ، ایک سرٹیفکیٹ کے ذریعے
    `--endpoint-attestation-leaf=<path>` (پلس `--endpoint-attestation-intermediate=<path>`
    چین میں ہر عنصر کے لئے اختیاری) اور کسی بھی ALPN ID نے بات چیت کی
    (`--endpoint-attestation-alpn=<token>`)۔ کوئیک اختتامی نکات نقل و حمل کی رپورٹیں فراہم کرسکتے ہیں
    `--endpoint-attestation-report[-hex]=...`۔
  - آؤٹ پٹ: کیننیکل پروپوزل بائٹس Norito (`--proposal-out`) اور ایک JSON خلاصہ
    (پہلے سے طے شدہ stdout یا `--json-out`)۔
- `sign`
  - ان پٹ: ایک تجویز (`--proposal`) ، ایک دستخط شدہ اشتہار (`--advert`) ، اختیاری اشتہاری باڈی
    (`--advert-body`) ، برقرار رکھنے کا دور اور کم از کم ایک بورڈ دستخط۔ دستخط کر سکتے ہیں
    ان لائن (`--council-signature=<signer_hex:signature_hex>`) یا فائلوں کے ساتھ مل کر فراہم کیا جائے
    `--council-signature-public-key` کے ساتھ `--council-signature-file=<path>`۔
  - ایک توثیق شدہ لفافہ (`--envelope-out`) اور ایک JSON رپورٹ تیار کرتا ہے جس میں ڈائجسٹ پابندیوں کی نشاندہی ہوتی ہے ،
    دستخطوں اور اندراج کے راستوں کی گنتی۔
- `verify`
  - تجویز کی اختیاری توثیق کے ساتھ ، ایک موجودہ لفافے (`--envelope`) کی توثیق کرتا ہے ،
    اشتہار یا اس سے متعلقہ اشتہاری ادارہ۔ JSON رپورٹ ڈائجسٹ اقدار ، حیثیت کو اجاگر کرتی ہے
    دستخطی توثیق اور کون سے اختیاری نمونے مماثل ہیں۔
- `renewal`
  - ایک نئے منظور شدہ لفافے کو پہلے کی توثیق شدہ ڈائجسٹ سے جوڑتا ہے۔ ضرورت ہے
    `--previous-envelope=<path>` اور جانشین `--envelope=<path>` (دونوں پے لوڈ Norito)۔
    سی ایل آئی اس بات کی تصدیق کرتی ہے کہ پروفائل عرفی ، صلاحیتوں اور اشتہاری چابیاں میں کوئی تبدیلی نہیں ہے ،
    داؤ ، اختتامی نقطہ اور میٹا ڈیٹا کی تازہ کاریوں کی اجازت دیتے ہوئے۔ کیننیکل بائٹس کو آؤٹ پٹ کریں
    `ProviderAdmissionRenewalV1` (`--renewal-out`) پلس ایک JSON خلاصہ۔
- `revoke`
  - ایک سپلائی کرنے والے کے لئے ایک ہنگامی بنڈل `ProviderAdmissionRevocationV1` جاری کرتا ہے جس کا لفافہ لازمی ہے
    ریٹائرمنٹ `--envelope=<path>` ، `--reason=<text>` ، کم از کم ایک کی ضرورت ہے
    `--council-signature` ، اور اختیاری طور پر `--revoked-at`/`--notes`۔ سی ایل آئی کی علامت ہے اور اس کی توثیق کرتی ہے
    منسوخی ڈائجسٹ ، `--revocation-out` کے ذریعے پے لوڈ Norito لکھتا ہے اور JSON رپورٹ پرنٹ کرتا ہے
    ڈائجسٹ اور دستخطی گنتی کے ساتھ۔
| توثیق | Torii ، گیٹ ویز اور `sorafs-node` کے ذریعہ استعمال شدہ مشترکہ تصدیق کنندہ کو نافذ کریں۔ CLI یونٹ + انضمام کے ٹیسٹ فراہم کریں۔ 【F: کریٹس/sorafs_manifest/src/provider_admission.rs#l1 】【 f: کریٹس/iroha_torii/src/sorafs/admission.rs#l1】 | نیٹ ورکنگ TL/اسٹوریج | ✅ مکمل || انضمام Torii | Torii میں اشتہارات کی کھجلی میں تصدیق کنندہ ، پالیسی سے باہر کے اشتہارات کو مسترد کریں اور ٹیلی میٹری کا اخراج کریں۔ | نیٹ ورکنگ TL | ✅ مکمل | Torii اب گورننس لفافے (`torii.sorafs.admission_envelopes_dir`) کو لوڈ کرتا ہے ، چیک ہضم/دستخط کے میچوں کے دوران ، اور بے نقاب ہوتا ہے داخلہ۔ 【f: کریٹس/اروہ_ٹوری/ایس آر سی/سرفس/داخلہ
| تجدید | تجدید/منسوخی اسکیم + سی ایل آئی مددگار شامل کریں ، دستاویزات میں لائف سائیکل گائیڈ شائع کریں (نیچے رن بک دیکھیں اور سی ایل آئی کمانڈز میں شامل کریں۔ `provider-admission renewal`/`revoke`). 【کریٹس/sorafs_car/src/sorafs_manifest_stub/فراہم کنندہ_ایڈیشن.رس#l477 】【 دستاویزات/ماخذ/sorafs/فراہم کنندہ_ایڈیشن_پولیسی.مڈی: 120】 | اسٹوریج / گورننس | ✅ مکمل |
| ٹیلی میٹری | ڈیش بورڈز/الرٹس `provider_admission` کی وضاحت کریں (تجدید کی تجدید ، لفافے کی میعاد ختم)۔ | مشاہدہ | 🟠 ترقی میں | کاؤنٹر `torii_sorafs_admission_total{result,reason}` موجود ہے ؛ ڈیش بورڈز/زیر التواء انتباہات

### تجدید اور منسوخ کرنے والی رن بک

#### شیڈول تجدید (داؤ/ٹوپولوجی کی تازہ کاری)
1. `provider-admission proposal` اور `provider-admission sign` کے ساتھ جانشین کی تجویز/اشتہار کی جوڑی بنائیں ، `--retention-epoch` میں اضافہ کریں اور ضرورت کے مطابق اسٹیکس/اختتامی مقامات کو اپ ڈیٹ کریں۔
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
   کمانڈ بغیر کسی تبدیلی کے اہلیت/پروفائل فیلڈز کی توثیق کرتا ہے
   `AdmissionRecord::apply_renewal` ، `ProviderAdmissionRenewalV1` کو جاری کرتا ہے اور پرنٹ ہضم ہوتا ہے
   گورننس لاگ۔ 【کریٹس/sorafs_car/src/bin/sorafs_manifest_stub/provider_admission.rs#l477 】【 f: creats/sorafs_manifest/src/provider_admission.rs#l422】
3. پرانے لفافے کو `torii.sorafs.admission_envelopes_dir` پر تبدیل کریں ، تجدید Norito/JSON کو گورننس ریپوزٹری میں مرتب کریں ، اور `docs/source/sorafs/migration_ledger.md` میں تجدید + برقرار رکھنے والے ایپوچ ہیش کو شامل کریں۔
4. آپریٹرز کو مطلع کرتا ہے کہ نیا لفافہ فعال ہے اور ingession کی تصدیق کے لئے `torii_sorafs_admission_total{result="accepted",reason="stored"}` پر نظر رکھتا ہے۔
5. `cargo run -p sorafs_car --bin provider_admission_fixtures --features cli` کے ذریعے کیننیکل فکسچر کو دوبارہ تخلیق کریں اور تصدیق کریں۔ CI (`ci/check_sorafs_fixtures.sh`) توثیق کرتا ہے کہ Norito آؤٹ پٹ مستحکم رہتے ہیں۔

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
   سی ایل آئی کے اشارے `ProviderAdmissionRevocationV1` ، کے ذریعہ طے شدہ دستخط کی تصدیق کرتے ہیں
   `verify_revocation_signatures` ، اور منسوخی ڈائجسٹ کی اطلاع دیں۔ 【کریٹس/sorafs_car/src/bin/sorafs_manifest_stub/provider_admission.rs#l593 】【 f: creats/sorafs_manifest/src/proper_admission.rs#l486】
2. `torii.sorafs.admission_envelopes_dir` لفافے کو ہٹا دیں ، منسوخی Norito/JSON کو داخلہ کیچوں میں تقسیم کریں ، اور گورننس ریکارڈ میں اس کی وجہ ہیش کو ریکارڈ کریں۔
3. `torii_sorafs_admission_total{result="rejected",reason="admission_missing"}` کا مشاہدہ کریں اس بات کی تصدیق کے لئے کہ کیچز منسوخ شدہ اشتہار کو ضائع کردیتے ہیں۔ واقعے کی پسپائیوں میں منسوخی کے نمونے کو محفوظ رکھتا ہے۔

## جانچ اور ٹیلی میٹری- تجاویز اور داخلہ لفافوں کے لئے سنہری فکسچر شامل کریں
  `fixtures/sorafs_manifest/provider_admission/`۔
- تجاویز کو دوبارہ تخلیق کرنے اور لفافوں کی توثیق کرنے کے لئے CI (`ci/check_sorafs_fixtures.sh`) میں توسیع کریں۔
- تیار کردہ فکسچر میں `metadata.json` شامل ہے جس میں کیننیکل ہضم ہوتا ہے۔ بہاو ​​ٹیسٹ تصدیق کرتے ہیں
  `proposal_digest_hex` == `ca8e73a1f319ae83d7bd958ccb143f9b790c7e4d9c8dfe1f6ad37fa29facf936`۔
- انضمام کے ٹیسٹ فراہم کریں:
  - Torii گمشدہ یا میعاد ختم ہونے والے داخلے کے لفافوں کے ساتھ اشتہارات کو مسترد کرتا ہے۔
  - CLI تجویز کا ایک گول سفر → لفافہ → توثیق کرتا ہے۔
  - گورننس کی تجدید فراہم کنندہ ID کو تبدیل کیے بغیر اختتامی نقطہ تصدیق کو گھوماتی ہے۔
- ٹیلی میٹری کی ضروریات:
  - Torii میں `provider_admission_envelope_{accepted,rejected}` جاری کریں۔ ✅ `torii_sorafs_admission_total{result,reason}` اب قبول شدہ/مسترد شدہ نتائج کو بے نقاب کرتا ہے۔
  - مشاہدہ کرنے والے ڈیش بورڈز میں میعاد ختم ہونے کے انتباہات شامل کریں (7 دن کے اندر تجدید کی تجدید)۔

## اگلے اقدامات

1. I Norito اسکیما میں ترمیم مکمل ہوچکی ہے اور توثیق کے مددگاروں کو شامل کیا گیا ہے
   `sorafs_manifest::provider_admission`۔ خصوصیت کے جھنڈوں کی ضرورت نہیں ہے۔
2. ✅ CLI فلوز (`proposal` ، `sign` ، `verify` ، `renewal` ، `revoke`) انضمام ٹیسٹ کے ذریعے دستاویزی اور استعمال کیا گیا ہے۔ رن بک کے ساتھ ہم آہنگی میں گورننس اسکرپٹ رکھیں۔
3. ✅ Torii داخلہ/دریافت لفافوں کو گھسیٹتا ہے اور قبولیت/مسترد ہونے کے لئے ٹیلی میٹری کاؤنٹرز کو بے نقاب کرتا ہے۔
4. مشاہدہ پر توجہ مرکوز کریں: داخلہ ڈیش بورڈز/الرٹس کو حتمی شکل دیں تاکہ سات دن کے اندر اندر تجدیدات کی انتباہات (`torii_sorafs_admission_total` ، میعاد ختم گیجز)۔