---
lang: ur
direction: rtl
source: docs/portal/docs/sorafs/provider-admission-policy.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

> [`docs/source/sorafs/provider_admission_policy.md`] (https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/provider_admission_policy.md) سے موافقت پذیر۔

# فراہم کنندہ داخلہ اور شناخت کی پالیسی SoraFS (مسودہ SF-2B)

اس نوٹ میں ** SF-2B ** کے لئے قابل عمل فراہمی کو حاصل کیا گیا ہے: وضاحت کریں اور
داخلہ کے بہاؤ ، شناخت کی ضروریات ، اور سیکیورٹی پے لوڈ کا اطلاق کریں
اسٹوریج فراہم کرنے والوں کے لئے تصدیق SoraFS۔ یہ اعلی کارکردگی کے عمل کو وسعت دیتا ہے
SoraFS آرکیٹیکچر آر ایف سی میں بیان کردہ سطح اور بقیہ کام کو تقسیم کرتا ہے
ٹریک ایبل انجینئرنگ کے کام۔

## پالیسی کے مقاصد

- اس بات کو یقینی بنائیں کہ صرف تصدیق شدہ آپریٹرز `ProviderAdvertV1` ریکارڈ شائع کرسکتے ہیں جسے نیٹ ورک قبول کرتا ہے۔
- ہر اشتہار کی کلید کو گورننس سے منظور شدہ شناختی دستاویز ، تصدیق شدہ اختتامی نکات ، اور کم سے کم حصص کی شراکت سے لنک کریں۔
۔
- ٹولز کے عزم یا ایرگونومکس کو توڑے بغیر ہنگامی تجدید اور منسوخی کی حمایت کریں۔

## شناخت اور داؤ کی ضروریات

| ضرورت | تفصیل | فراہمی |
| ----------- | ----------- | -------------- |
| اشتہار کی کلید کا ماخذ | فراہم کنندگان کو ایک ED25519 کلیدی جوڑی رجسٹر کرنا ہوگی جو ہر اشتہار پر دستخط کرتی ہے۔ داخلہ بنڈل حکومت کے دستخط کے ساتھ ساتھ عوامی کلید کو بھی محفوظ کرتا ہے۔ | `advert_key` (32 بائٹس) کے ساتھ اسکیم `ProviderAdmissionProposalV1` میں توسیع کریں اور رجسٹر (`sorafs_manifest::provider_admission`) میں اس کا حوالہ دیں۔ |
| اسٹیک پوائنٹر | داخلے کے لئے ایک غیر صفر `StakePointer` ایک فعال اسٹیکنگ پول کی طرف اشارہ کرنے کی ضرورت ہے۔ | `sorafs_manifest::provider_advert::StakePointer::validate()` میں توثیق شامل کریں اور CLI/ٹیسٹوں میں غلطیوں کو بے نقاب کریں۔ |
| دائرہ اختیار ٹیگز | فراہم کرنے والے دائرہ اختیار + قانونی رابطے کا اعلان کرتے ہیں۔ | `jurisdiction_code` (ISO 3166-1 الفا -2) اور اختیاری `contact_uri` کے ساتھ پروپوزل اسکیما میں توسیع کریں۔ |
| اختتامی نقطہ کی تصدیق | ہر مشتہر اختتامی نقطہ کو ایم ٹی ایل ایس یا کوئیک سرٹیفکیٹ رپورٹ کے ذریعہ تعاون کرنا چاہئے۔ | پے لوڈ Norito `EndpointAttestationV1` کی وضاحت کریں اور داخلہ کے بنڈل کے اندر اسے فی اختتامی نقطہ اسٹور کریں۔ |

## داخلہ بہاؤ

1. ** تجویز کی تشکیل **
   - CLI: `cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- provider-admission proposal ...` شامل کریں
     `ProviderAdmissionProposalV1` + تصدیقی بنڈل تیار کرنا۔
   - توثیق: مطلوبہ فیلڈز ، اسٹیک> 0 ، `profile_id` میں کیننیکل چنکر ہینڈل کی ضمانت دیں۔
2. ** گورننس کی توثیق **
   - موجودہ لفافے ٹولنگ کا استعمال کرتے ہوئے کونسل `blake3("sorafs-provider-admission-v1" || canonical_bytes)` کے اشارے
     (ماڈیول `sorafs_manifest::governance`)۔
   - لفافہ `governance/providers/<provider_id>/admission.json` میں برقرار ہے۔
3. ** رجسٹریشن میں انٹیک **
   - مشترکہ چیکر (`sorafs_manifest::provider_admission::validate_envelope`) نافذ کریں
     کون سا Torii/گیٹ ویز/سی ایل آئی نے دوبارہ استعمال کیا۔
   - Torii داخلہ کے راستے کو اپ ڈیٹ کریں تاکہ ان اشتہارات کو مسترد کریں جن کے ڈائجسٹ یا میعاد ختم ہونے سے لفافے سے مختلف ہوتا ہے۔
4. ** تجدید اور منسوخی **
   - اختیاری اختتامی نقطہ/اسٹیک اپڈیٹس کے ساتھ `ProviderAdmissionRenewalV1` شامل کریں۔
   - ایک سی ایل آئی پاتھ `--revoke` کو بے نقاب کریں جو منسوخی کی وجہ کو ریکارڈ کرتا ہے اور گورننس ایونٹ بھیجتا ہے۔

## نفاذ کے کام| علاقہ | ٹاسک | مالک (زبانیں) | حیثیت |
| ------ | -------- | ---------- | -------- |
| اسکیم | `ProviderAdmissionProposalV1` ، `ProviderAdmissionEnvelopeV1` ، `EndpointAttestationV1` (Norito) `crates/sorafs_manifest/src/provider_admission.rs` پر سیٹ کریں۔ توثیق کے مددگاروں کے ساتھ `sorafs_manifest::provider_admission` میں نافذ کیا گیا ہے اسٹوریج / گورننس | مکمل |
| سی ایل آئی ٹولز | `sorafs_manifest_stub` کو سب کامنڈس کے ساتھ بڑھاؤ: `provider-admission proposal` ، `provider-admission sign` ، `provider-admission verify`۔ | ٹولنگ ڈبلیو جی | مکمل |

سی ایل آئی فلو اب انٹرمیڈیٹ سرٹیفکیٹ بنڈل (`--endpoint-attestation-intermediate`) ، مسائل کو قبول کرتا ہے
تجویز/لفافہ کیننیکل بائٹس اور `sign`/`verify` کے دوران بورڈ کے دستخطوں کی توثیق کرتا ہے۔ آپریٹرز کر سکتے ہیں
اشتہاری اداروں کو براہ راست فراہم کریں یا دستخط شدہ اشتہارات کو دوبارہ استعمال کریں ، اور دستخطی فائلیں ہوسکتی ہیں
آٹومیشن کی سہولت کے ل I `--council-signature-public-key` کو `--council-signature-file` کے ساتھ جوڑ کر فراہم کیا گیا ہے۔

### سی ایل آئی حوالہ

`cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- provider-admission ...` کے ذریعے ہر کمانڈ پر عمل کریں۔- `proposal`
  - مطلوبہ جھنڈے: `--provider-id=<hex32>` ، `--chunker-profile=<namespace.name@semver>` ،
    `--stake-pool-id=<hex32>` ، `--stake-amount=<amount>` ، `--advert-key=<hex32>` ،
    `--jurisdiction-code=<ISO3166-1>` ، اور کم از کم ایک `--endpoint=<kind:host>`۔
  - اختتامی نقطہ کی تصدیق `--endpoint-attestation-attested-at=<secs>` کی توقع کرتی ہے ،
    `--endpoint-attestation-expires-at=<secs>` ، ایک سرٹیفکیٹ کے ذریعے
    `--endpoint-attestation-leaf=<path>` (پلس `--endpoint-attestation-intermediate=<path>`
    چین میں ہر عنصر کے لئے اختیاری) اور کوئی بھی مذاکرات شدہ ALPN IDs
    (`--endpoint-attestation-alpn=<token>`)۔ کوئیک اینڈ پوائنٹس ٹرانسپورٹ کی رپورٹیں فراہم کرسکتے ہیں
    `--endpoint-attestation-report[-hex]=...`۔
  ۔
    (پہلے سے طے شدہ stdout یا `--json-out`)۔
- `sign`
  - ان پٹ: ایک تجویز (`--proposal`) ، ایک دستخط شدہ اشتہار (`--advert`) ، اختیاری اشتہاری باڈی
    (`--advert-body`) ، برقرار رکھنے کی مدت اور کم از کم ایک بورڈ دستخط۔ سبسکرپشن کر سکتے ہیں
    ان لائن (`--council-signature=<signer_hex:signature_hex>`) یا فائلوں کے ذریعے مل کر فراہم کیا جائے
    `--council-signature-public-key` کے ساتھ `--council-signature-file=<path>`۔
  - ایک توثیق شدہ لفافہ (`--envelope-out`) اور ایک JSON رپورٹ تیار کرتا ہے جس میں ڈائجسٹ پابندیوں کی نشاندہی ہوتی ہے ،
    صارفین کی گنتی اور اندراج کے راستے۔
- `verify`
  - ایک موجودہ لفافے (`--envelope`) کی توثیق کرتا ہے ، جس میں تجویز کی اختیاری توثیق ، ​​اشتہار یا
    متعلقہ اشتہاری باڈی۔ JSON رپورٹ ڈائجسٹ اقدار ، توثیق کی حیثیت کو اجاگر کرتی ہے
    دستخطوں کی اور کون سے اختیاری نمونے ان کا مماثل ہے۔
- `renewal`
  - ایک نئے منظور شدہ لفافے کو پہلے کی توثیق شدہ ڈائجسٹ سے جوڑتا ہے۔ ضرورت ہے
    `--previous-envelope=<path>` اور جانشین `--envelope=<path>` (دونوں پے لوڈ Norito)۔
    سی ایل آئی اس بات کی تصدیق کرتی ہے کہ پروفائل عرفی ، صلاحیتوں اور اشتہاری چابیاں میں کوئی تبدیلی نہیں ہے ،
    داؤ ، اختتامی نقطہ اور میٹا ڈیٹا کی تازہ کاریوں کی اجازت دیتے ہوئے۔ کیننیکل بائٹس کو آؤٹ پٹ کرتا ہے
    `ProviderAdmissionRenewalV1` (`--renewal-out`) پلس ایک JSON ڈائجسٹ۔
- `revoke`
  - ایک ہنگامی بنڈل `ProviderAdmissionRevocationV1` کو کسی ایسے فراہم کنندہ کو جاری کرتا ہے جس کا لفافہ لازمی ہے
    ہٹا دیا جائے. `--envelope=<path>` ، `--reason=<text>` ، کم از کم ایک `--council-signature` کی ضرورت ہے ،
    اور اختیاری `--revoked-at`/`--notes`۔ پے لوڈ لکھتا ہے
    Norito `--revocation-out` کے ذریعے اور دستخطوں کی تعداد اور تعداد کے ساتھ JSON کا خلاصہ پرنٹ کرتا ہے۔
| توثیق | Torii ، گیٹ ویز ، اور `sorafs-node` کے ذریعہ استعمال شدہ مشترکہ چیکر کو نافذ کریں۔ یونٹری + سی ایل آئی انضمام کے ٹیسٹ فراہم کریں نیٹ ورکنگ TL/اسٹوریج | مکمل |
| انضمام Torii | Torii پر اشتہارات داخل کرتے وقت چیکر کو پاس کریں ، پالیسی سے باہر اشتہارات کو مسترد کریں اور ٹیلی میٹری جاری کریں۔ | نیٹ ورکنگ TL | مکمل | Torii اب گورننس لفافے (`torii.sorafs.admission_envelopes_dir`) کو لوڈ کرتا ہے ، انجسٹ کے دوران ہضم/دستخطی میچ چیک کرتا ہے ، اور ڈیٹا ٹیلی میٹری کو بے نقاب کرتا ہے۔ داخلہ| تزئین و آرائش | تجدید/منسوخی اسکیم + سی ایل آئی مددگار شامل کریں ، دستاویزات میں لائف سائیکل گائیڈ شائع کریں (نیچے رن بک دیکھیں اور سی ایل آئی کمانڈز میں شامل کریں۔ `provider-admission renewal`/`revoke`) اسٹوریج / گورننس | مکمل |
| ٹیلی میٹری | ڈیش بورڈز/انتباہات `provider_admission` سیٹ کریں (تجدید سے محروم ، لفافے کی میعاد ختم)۔ | مشاہدہ | پیشرفت میں | کاؤنٹر `torii_sorafs_admission_total{result,reason}` موجود ہے ؛ ڈیش بورڈز/زیر التواء انتباہات

### تجدید اور منسوخ کرنے والی رن بک

#### شیڈول تجدید (داؤ/ٹوپولوجی کی تازہ کاری)
1. `provider-admission proposal` اور `provider-admission sign` کے ساتھ جانشین کی تجویز/اشتہار کی جوڑی بنائیں ،
   `--retention-epoch` میں اضافہ اور ضرورت کے مطابق حصص/اختتامی مقامات کو اپ ڈیٹ کرنا۔
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
   کمانڈ `AdmissionRecord::apply_renewal` کے ذریعہ بدلے ہوئے صلاحیت/پروفائل فیلڈز کی توثیق کرتا ہے ،
   مسائل `ProviderAdmissionRenewalV1` اور پرنٹ گورننس لاگ کو ہضم کرتے ہیں۔
3. پچھلے لفافے کو `torii.sorafs.admission_envelopes_dir` میں تبدیل کریں ، تجدید Norito/JSON کی تصدیق کریں
   گورننس ریپوزٹری میں اور `docs/source/sorafs/migration_ledger.md` میں تجدید ہیش + برقرار رکھنے کے دور کو شامل کریں۔
4. آپریٹرز کو مطلع کریں کہ نیا لفافہ فعال ہے اور نگرانی کرتا ہے
   `torii_sorafs_admission_total{result="accepted",reason="stored"}` ادخال کی تصدیق کرنے کے لئے۔
5. `cargo run -p sorafs_car --bin provider_admission_fixtures --features cli` کے ذریعے کیننیکل فکسچر کو دوبارہ تخلیق کریں اور تصدیق کریں۔
   CI (`ci/check_sorafs_fixtures.sh`) توثیق کرتا ہے کہ Norito آؤٹ پٹ مستحکم رہتے ہیں۔

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
   `verify_revocation_signatures` اور منسوخ ڈائجسٹ کی اطلاع دیتا ہے۔
2. `torii.sorafs.admission_envelopes_dir` لفافے کو ہٹا دیں ، Norito/منسوخ JSON کو کیچز میں تقسیم کریں
   داخلے اور گورننس منٹ میں وجہ کی ہیش کو ریکارڈ کریں۔
3. اس بات کی تصدیق کے ل I `torii_sorafs_admission_total{result="rejected",reason="admission_missing"}` کا مشاہدہ کریں
   کیچز نے منسوخ شدہ اشتہار کو ضائع کردیا۔ واقعے کے پسپائیوں میں منسوخی کے نمونے برقرار رکھیں۔

## ٹیسٹ اور ٹیلی میٹری- تجاویز اور داخلہ لفافوں میں سنہری فکسچر شامل کریں
  `fixtures/sorafs_manifest/provider_admission/`۔
- تجاویز کو دوبارہ تخلیق کرنے اور لفافوں کو چیک کرنے کے لئے CI (`ci/check_sorafs_fixtures.sh`) میں توسیع کریں۔
- تیار کردہ فکسچر میں `metadata.json` شامل ہے جس میں کیننیکل ہضم ہوتا ہے۔ بہاو ​​ٹیسٹ کا دعوی
  `proposal_digest_hex` == `ca8e73a1f319ae83d7bd958ccb143f9b790c7e4d9c8dfe1f6ad37fa29facf936`۔
- انضمام کے ٹیسٹ فراہم کریں:
  - Torii گمشدہ یا میعاد ختم ہونے والے داخلے کے لفافوں کے ساتھ اشتہارات کو مسترد کرتا ہے۔
  -CLI تجویز کا ایک گول سفر کرتا ہے -> لفافہ -> توثیق۔
  - گورننس کی تجدید فراہم کنندہ ID کو تبدیل کیے بغیر اختتامی نقطہ کی تصدیق کو گھومتی ہے۔
- ٹیلی میٹری کی ضروریات:
  - Torii میں `provider_admission_envelope_{accepted,rejected}` جاری کریں۔ `torii_sorafs_admission_total{result,reason}` اب قبول شدہ/مسترد شدہ نتائج کو بے نقاب کرتا ہے۔
  - مشاہدہ کرنے والے ڈیش بورڈز میں میعاد ختم ہونے کے انتباہات شامل کریں (7 دن کے اندر تجدید کی تجدید)۔

## اگلے اقدامات

1. Norito اسکیما میں تبدیلیوں کو حتمی شکل دی گئی ہے اور توثیق کے مددگاروں کو `sorafs_manifest::provider_admission` میں شامل کیا گیا ہے۔ کوئی خصوصیت کے جھنڈے نہیں ہیں۔
2. سی ایل آئی فلوز (`proposal` ، `sign` ، `verify` ، `renewal` ، `revoke`) کو انضمام ٹیسٹ کے ذریعے دستاویزی اور استعمال کیا گیا ہے۔ رن بک کے ساتھ گورننس اسکرپٹ کو ہم آہنگ رکھیں۔
3. Torii داخلہ/دریافت لفافوں کو گھسیٹتا ہے اور قبولیت/مسترد ہونے کے لئے ٹیلی میٹری کاؤنٹرز کو بے نقاب کرتا ہے۔
4. مشاہدہ پر توجہ مرکوز کریں: مکمل داخلہ ڈیش بورڈز/الرٹس تاکہ سات دن کے اندر اندر تجدیدات کی وجہ سے ٹرگر وارننگ (`torii_sorafs_admission_total` ، میعاد ختم گیجز)۔