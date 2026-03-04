---
lang: ur
direction: rtl
source: docs/portal/docs/sorafs/provider-admission-policy.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

> [`docs/source/sorafs/provider_admission_policy.md`] (https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/provider_admission_policy.md) سے موافقت پذیر۔

# فراہم کنندہ داخلہ اور شناختی پالیسی SoraFS (مسودہ SF-2B)

اس نوٹ میں ** SF-2B ** کے عملی نتائج کی دستاویز کی گئی ہے: شناخت کریں اور
داخلے کے عمل ، شناخت کی ضروریات اور نافذ کریں
اسٹوریج فراہم کرنے والوں کے لئے سرٹیفیکیشن پے لوڈ SoraFS۔ وہ پھیلتی ہے
اعلی سطح کا عمل آرکیٹیکچر RFC SoraFS اور بریک میں بیان کیا گیا ہے
ٹریک شدہ انجینئرنگ کے کاموں پر باقی کام۔

## پالیسی اہداف

- اس بات کو یقینی بنائیں کہ صرف تصدیق شدہ آپریٹرز `ProviderAdvertV1` ریکارڈ شائع کرسکتے ہیں جسے نیٹ ورک قبول کرتا ہے۔
- ہر اشتہار کی کلید کو گورننس سے منظور شدہ شناختی دستاویز ، مصدقہ اختتامی نکات اور کم سے کم حصص کی شراکت سے لنک کریں۔
- جانچ پڑتال کے اوزار فراہم کریں تاکہ Torii ، گیٹ ویز اور `sorafs-node` ایک ہی چیک کا اطلاق کریں۔
- ٹولز کے عزم یا استعمال کے بغیر سمجھوتہ کیے بغیر اپ ڈیٹ اور فیل اوور کی حمایت کریں۔

## شناخت اور داؤ کی ضروریات

| ضرورت | تفصیل | نتیجہ |
| ----------- | ---------- | ----------- |
| AD کلیدی اصل | فراہم کنندگان کو ED25519 کلیدی جوڑی کو رجسٹر کرنا ہوگا جس کے ساتھ ہر اشتہار پر دستخط ہوتے ہیں۔ اجازت کا بنڈل گورننس کے دستخط کے ساتھ ساتھ عوامی کلید کو بھی محفوظ کرتا ہے۔ | `ProviderAdmissionProposalV1` اسکیما کو `advert_key` فیلڈ (32 بائٹس) کے ساتھ بڑھاؤ اور اس سے رجسٹری (`sorafs_manifest::provider_admission`) سے رجوع کریں۔ |
| داؤ انڈیکس | داخلے کے لئے ایک غیر صفر `StakePointer` کی ضرورت ہوتی ہے جس میں ایک فعال اسٹیکنگ پول کی نشاندہی ہوتی ہے۔ | CLI/ٹیسٹوں میں `sorafs_manifest::provider_advert::StakePointer::validate()` اور آؤٹ پٹ غلطیوں میں توثیق شامل کریں۔ |
| دائرہ اختیار ٹیگز | فراہم کرنے والے دائرہ اختیار + قانونی رابطے کا اعلان کرتے ہیں۔ | پیش کش اسکیم کو فیلڈ `jurisdiction_code` (ISO 3166-1 الفا -2) اور اختیاری `contact_uri` کے ساتھ بڑھا دیں۔ |
| اختتامی نقطہ سرٹیفیکیشن | ہر اعلان کردہ اختتامی نقطہ کو ایم ٹی ایل ایس یا کوئیک سرٹیفکیٹ رپورٹ کے ذریعہ تعاون کرنا چاہئے۔ | Norito پے لوڈ `EndpointAttestationV1` کی وضاحت کریں اور داخلہ کے بنڈل میں ہر اختتامی نقطہ کے لئے اسٹور کریں۔ |

## داخلہ کا عمل

1. ** ایک پیش کش بنائیں **
   - CLI: `cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- provider-admission proposal ...` شامل کریں ،
     `ProviderAdmissionProposalV1` + سرٹیفیکیشن بنڈل تشکیل دینا۔
   - توثیق: `profile_id` میں مطلوبہ فیلڈز ، اسٹیک> 0 ، کیننیکل چنکر ہینڈل کی موجودگی کو یقینی بنائیں۔
2. ** گورننس کی منظوری **
   - کونسل کے اشارے `blake3("sorafs-provider-admission-v1" || canonical_bytes)` موجودہ کا استعمال کرتے ہوئے
     لفافے کے اوزار (ماڈیول `sorafs_manifest::governance`)۔
   - `governance/providers/<provider_id>/admission.json` میں لفافہ محفوظ کیا گیا ہے۔
3. ** رجسٹر میں اندراج **
   - ایک عام جائز (`sorafs_manifest::provider_admission::validate_envelope`) کو نافذ کریں
     Torii/گیٹ ویز/سی ایل آئی کو دوبارہ استعمال کریں۔
   - داخلے کا راستہ Torii ان اشتہارات کو مسترد کرنے کے لئے اپ ڈیٹ کریں جن کی ڈائجسٹ یا میعاد ختم ہونے کی تاریخ لفافے سے مختلف ہے۔
4. ** اپ ڈیٹ اور جائزہ **
   - اختیاری اختتامی نقطہ/اسٹیک اپڈیٹس کے ساتھ `ProviderAdmissionRenewalV1` شامل کریں۔
   - اوپن سی ایل آئی پاتھ Norito ، جو منسوخ کرنے کی وجہ کو اپنی گرفت میں لے جاتا ہے اور گورننس ایونٹ کو روانہ کرتا ہے۔

## نفاذ کے کام| خطہ | مسئلہ | مالک (زبانیں) | حیثیت |
| --------- | -------- | ---------- | -------- |
| اسکیم | `ProviderAdmissionProposalV1` ، `ProviderAdmissionEnvelopeV1` ، `EndpointAttestationV1` (Norito) سے `crates/sorafs_manifest/src/provider_admission.rs` کی وضاحت کریں۔ توثیق کے مددگاروں کے ساتھ `sorafs_manifest::provider_admission` میں نافذ کیا گیا ہے اسٹوریج / گورننس | ✅ مکمل |
| سی ایل آئی ٹولز | `sorafs_manifest_stub` کو سب کامنڈس کے ساتھ بڑھاؤ: `provider-admission proposal` ، `provider-admission sign` ، `provider-admission verify`۔ | ٹولنگ ڈبلیو جی | ✅ مکمل |

سی ایل آئی اسٹریم اب انٹرمیڈیٹ سرٹیفکیٹ بنڈل (`--endpoint-attestation-intermediate`) کو قبول کرتا ہے ،
`sign`/`verify` کے دوران کونسل کے دستخطوں کی تصدیق اور کونسل کے دستخطوں کی تصدیق کرتے ہیں۔ آپریٹرز کر سکتے ہیں
اشتہاری اداروں کو براہ راست منتقل کریں یا دستخط شدہ اشتہارات کو دوبارہ استعمال کریں ، اور دستخطی فائلیں ہوسکتی ہیں
آٹومیشن کی آسانی کے ل I `--council-signature-public-key` کو `--council-signature-file` کے ساتھ جوڑ کر منتقل کریں۔

### سی ایل آئی حوالہ

`cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- provider-admission ...` کے ذریعے ہر کمانڈ چلائیں۔- `proposal`
  - مطلوبہ جھنڈے: `--provider-id=<hex32>` ، `--chunker-profile=<namespace.name@semver>` ،
    `--stake-pool-id=<hex32>` ، `--stake-amount=<amount>` ، `--advert-key=<hex32>` ،
    `--jurisdiction-code=<ISO3166-1>` ، اور کم از کم ایک `--endpoint=<kind:host>`۔
  - `--endpoint-attestation-attested-at=<secs>` ہر اختتامی نقطہ کی سند کے لئے ضروری ہے ،
    `--endpoint-attestation-expires-at=<secs>` ، سرٹیفکیٹ کے ذریعے
    `--endpoint-attestation-leaf=<path>` (اس کے علاوہ اختیاری `--endpoint-attestation-intermediate=<path>`
    ہر چین عنصر کے لئے) اور کسی بھی مستقل ALPN IDs کے لئے
    (`--endpoint-attestation-alpn=<token>`)۔ کوئیک اختتامی نکات کے ل you ، آپ ٹرانسپورٹ رپورٹس کے ذریعے بھیج سکتے ہیں
    `--endpoint-attestation-report[-hex]=...`۔
  - آؤٹ پٹ: کیننیکل جملہ بائٹس Norito (`--proposal-out`) اور JSON خلاصہ
    (ڈیفالٹ یا `--json-out` کے ذریعہ stdout)۔
- `sign`
  - ان پٹ: پیش کش (`--proposal`) ، دستخط شدہ اشتہار (`--advert`) ، اختیاری باڈی ایڈورٹ
    (`--advert-body`) ، برقرار رکھنے کا دور اور کم از کم ایک کونسل کے دستخط۔ دستخطوں کو منتقل کیا جاسکتا ہے
    ان لائن (`--council-signature=<signer_hex:signature_hex>`) یا فائلوں کے ذریعے ، امتزاج
    `--council-signature-public-key` کے ساتھ `--council-signature-file=<path>`۔
  - ایک توثیق شدہ لفافہ (`--envelope-out`) اور ڈائجسٹ بائنڈنگ کے ساتھ JSON رپورٹ تیار کرتا ہے ،
    دستخط کرنے والوں اور داخلے کے راستوں کی تعداد۔
- `verify`
  - موجودہ لفافے (`--envelope`) کی جانچ پڑتال کرتا ہے اور اختیاری طور پر متعلقہ پیش کش کی جانچ پڑتال کرتا ہے ،
    اشتہار یا جسمانی اشتہار۔ JSON رپورٹ ڈائجسٹ اقدار ، دستخطی توثیق کی حیثیت کو اجاگر کرتی ہے
    اور کون سے اختیاری نمونے ملتے ہیں۔
- `renewal`
  - نئے منظور شدہ لفافے کو پہلے سے منظور شدہ ڈائجسٹ سے جوڑتا ہے۔ مطلوب
    `--previous-envelope=<path>` اور اس کے بعد `--envelope=<path>` (دونوں Norito پے لوڈ)۔
    سی ایل آئی کی جانچ پڑتال کرتی ہے کہ عرفیت ، صلاحیتوں اور اشتہار کی کلید میں کوئی تبدیلی نہیں ہوتی ہے ، جبکہ اجازت دیتے ہوئے
    داؤ ، اختتامی نقطہ اور میٹا ڈیٹا کی تازہ کاری۔ آؤٹ پٹس کیننیکل بائٹس `ProviderAdmissionRenewalV1`
    (`--renewal-out`) پلس JSON خلاصہ۔
- `revoke`
  - کسی ایسے فراہم کنندہ کے لئے ایک ہنگامی بنڈل `ProviderAdmissionRevocationV1` جاری کرتا ہے جس کے لفافے کو منسوخ کرنے کی ضرورت ہے۔
    `--envelope=<path>` ، `--reason=<text>` ، کم از کم ایک `--council-signature` اور اختیاری کی ضرورت ہے
    `--revoked-at`/`--notes`۔ Norito پے لوڈ لکھتا ہے
    `--revocation-out` اور ہضم اور دستخطوں کی تعداد کے ساتھ JSON رپورٹ پرنٹ کرتا ہے۔
| چیک | Torii ، گیٹ ویز اور `sorafs-node` کے ذریعہ استعمال کردہ ایک مشترکہ توثیق کار کو نافذ کریں۔ یونٹ + سی ایل آئی انضمام کے ٹیسٹ مہیا کریں نیٹ ورکنگ TL/اسٹوریج | ✅ مکمل |
| انضمام Torii | Torii میں اشتہارات قبول کرنے کے لئے توثیق کرنے والے کو مربوط کریں ، پالیسی سے باہر اشتہارات کو مسترد کریں اور ٹیلی میٹری شائع کریں۔ | نیٹ ورکنگ TL | ✅ مکمل | Torii اب گورننس لفافے (`torii.sorafs.admission_envelopes_dir`) لوڈ کرتا ہے ، استقبالیہ پر ہضم/دستخطی میچ چیک کرتا ہے اور ٹیلی میٹری کو شائع کرتا ہے داخلہ۔ 【f: کریٹس/اروہ_ٹوری/ایس آر سی/سرفس/داخلہ| اپ ڈیٹ | اپ ڈیٹ/یاد رکھنے والی اسکیمیں + سی ایل آئی مددگار شامل کریں ، دستاویزات میں لائف سائیکل شائع کریں (نیچے رن بک دیکھیں اور سی ایل آئی کمانڈز میں شامل کریں۔ `provider-admission renewal`/`revoke`). 【کریٹس/sorafs_car/src/sorafs_manifest_stub/فراہم کنندہ_ایڈیشن.رس#l477 】【 دستاویزات/ماخذ/sorafs/فراہم کنندہ_ایڈیشن_پولیسی.مڈی: 120】 | اسٹوریج / گورننس | ✅ مکمل |
| ٹیلی میٹری | ڈیش بورڈز/الرٹس `provider_admission` کی وضاحت کریں (یاد تازہ کاری ، لفافے کی میعاد ختم ہونے کی تاریخ)۔ | مشاہدہ | 🟠 ترقی میں | کاؤنٹر `torii_sorafs_admission_total{result,reason}` موجود ہے ؛ ڈیش بورڈز/انتباہات جاری ہیں

### رن بک اپڈیٹس اور آراء

#### شیڈول اپ ڈیٹ (داؤ/ٹوپولوجی اپ ڈیٹ)
1. `provider-admission proposal` اور `provider-admission sign` کے ذریعے فالو اپ آفر/اشتہاری جوڑی جمع کریں ،
   `--retention-epoch` میں اضافہ اور ضرورت کے مطابق اسٹیکس/اختتامی مقامات کو اپ ڈیٹ کرنا۔
2. عمل کریں
   ```bash
   cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- provider-admission \
     renewal \
     --previous-envelope=governance/providers/<id>/envelope.to \
     --envelope=governance/providers/<id>/envelope_next.to \
     --renewal-out=governance/providers/<id>/renewal.to \
     --json-out=governance/providers/<id>/renewal.json \
     --notes="stake top-up 2025-03"
   ```
   کمانڈ صلاحیت/پروفائل فیلڈز کے ذریعہ عدم استحکام کی جانچ پڑتال کرتا ہے
   `AdmissionRecord::apply_renewal` ، `ProviderAdmissionRenewalV1` کو جاری کرتا ہے اور پرنٹ ہضم ہوتا ہے
   گورننس میگزین۔ 【کریٹس/sorafs_car/src/bin/sorafs_manifest_stub/provider_admission.rs#l477 】【 f: creats/sorafs_manifest/src/فراہم کنندہ_اڈمیشن. rs#l422】
3. پچھلے لفافے کو `torii.sorafs.admission_envelopes_dir` میں تبدیل کریں ، Norito/JSON تازہ کاریوں کا ارتکاب کریں
   گورننس ریپوزٹری میں اور `docs/source/sorafs/migration_ledger.md` میں اپ ڈیٹ ہیش + برقرار رکھنے کے عہد کو شامل کریں۔
4. آپریٹرز کو مطلع کریں کہ نیا لفافہ فعال اور مانیٹر ہے
   `torii_sorafs_admission_total{result="accepted",reason="stored"}` استقبال کی تصدیق کے لئے۔
5. `cargo run -p sorafs_car --bin provider_admission_fixtures --features cli` کے ذریعے کیننیکل فکسچر کو دوبارہ بنائیں اور ٹھیک کریں۔
   CI (`ci/check_sorafs_fixtures.sh`) Norito آؤٹ پٹ کے استحکام کی جانچ کرتا ہے۔

#### ایمرجنسی یاد
1. سمجھوتہ شدہ لفافے کی شناخت کریں اور رائے جاری کریں:
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
   سی ایل آئی کے اشارے `ProviderAdmissionRevocationV1` ، کے ذریعہ سیٹ کے سیٹ کی تصدیق کرتے ہیں
   `verify_revocation_signatures` اور ایک جائزہ ڈائجسٹ کی اطلاع دیتا ہے۔ 【کریٹس/sorafs_car/src/bin/sorafs_manifest_stub/provider_admission.rs#l593 】【 f: creats/sorafs_manifest/src/فراہم کنندہ_ایڈیشن. rs#l486】
2. `torii.sorafs.admission_envelopes_dir` سے لفافہ کو ہٹا دیں ، Norito/JSON آراء کو داخلہ کیچوں میں تقسیم کریں
   اور گورننس پروٹوکول میں ہیش وجوہات کو ریکارڈ کریں۔
3. ٹریک `torii_sorafs_admission_total{result="rejected",reason="admission_missing"}` کی تصدیق کے ل .۔
   اس کیچوں نے منسوخ شدہ اشتہار کو ضائع کردیا۔ واقعہ کے پسپائیوں میں اسٹور یاد کرنے والے نمونے۔

## جانچ اور ٹیلی میٹری- پیش کشوں اور داخلے کے لفافے کے لئے گولڈن فکسچر شامل کریں
  `fixtures/sorafs_manifest/provider_admission/`۔
- پیش کشوں کو دوبارہ بنانے اور لفافے کو چیک کرنے کے لئے CI (`ci/check_sorafs_fixtures.sh`) میں توسیع کریں۔
- تیار کردہ فکسچر میں `metadata.json` شامل ہے جس میں کیننیکل ہضم ہوتا ہے۔ بہاو ​​ٹیسٹ تصدیق کرتے ہیں
  `proposal_digest_hex` == `ca8e73a1f319ae83d7bd958ccb143f9b790c7e4d9c8dfe1f6ad37fa29facf936`۔
- انضمام کے ٹیسٹ فراہم کریں:
  - Torii گمشدہ یا میعاد ختم ہونے والے داخلے کے لفافوں کے ساتھ اشتہارات کو مسترد کرتا ہے۔
  - CLI تجاویز کے ایک دور کے سفر سے گزرتا ہے → لفافہ → توثیق۔
  - گورننس اپ ڈیٹ فراہم کنندہ ID کو تبدیل کیے بغیر اختتامی نقطہ تصدیق کو گھوماتا ہے۔
- ٹیلی میٹری کی ضروریات:
  - `provider_admission_envelope_{accepted,rejected}` کاؤنٹرز کو Torii پر خارج کریں۔ ✅ `torii_sorafs_admission_total{result,reason}` اب قبول شدہ/مسترد دکھاتا ہے۔
  - مشاہدہ میں میعاد ختم ہونے والے انتباہات شامل کریں (7 دن کے اندر مطلوبہ تازہ کاری)۔

## اگلے اقدامات

1. I Norito اسکیموں میں تبدیلیاں مکمل ہوچکی ہیں اور توثیق کے مددگاروں کو `sorafs_manifest::provider_admission` میں شامل کیا گیا ہے۔ خصوصیت کے جھنڈوں کی ضرورت نہیں ہے۔
2. ✅ CLI اسٹریمز (`proposal` ، `sign` ، `verify` ، `renewal` ، `revoke`) کو انضمام کے ٹیسٹ کے ذریعہ دستاویزی اور تصدیق کی گئی ہے۔ رن بکس کے ساتھ ہم آہنگی میں گورننس اسکرپٹ رکھیں۔
3. ✅ Torii داخلہ/دریافت لفافے قبول کرتا ہے اور ٹیلی میٹک قبولیت/مسترد کاؤنٹرز کو شائع کرتا ہے۔
4. مشاہدہ پر فوکس کریں: داخلے پر مکمل ڈیش بورڈز/الرٹس تاکہ سات دن کے اندر مطلوبہ تازہ کاری انتباہات میں اضافہ کرے (`torii_sorafs_admission_total` ، میعاد ختم گیجز)۔